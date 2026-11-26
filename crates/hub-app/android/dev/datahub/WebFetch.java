package dev.datahub;

import android.app.Activity;
import android.os.Handler;
import android.os.Looper;
import android.webkit.ValueCallback;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;

import java.lang.reflect.Method;

/**
 * Loads a page in an off-screen {@link WebView} (a real browser engine on the device's own network)
 * and hands back the rendered DOM. Used by the Rust side as a fallback for pages that sit behind a
 * JavaScript challenge, which plain HTTP cannot pass.
 *
 * Everything touching the WebView runs on the main thread; Rust only polls {@link #state()} and
 * {@link #html()} from its own thread. Built to DEX by scripts/build_webfetch_dex.sh (no lambdas).
 */
public final class WebFetch implements Runnable {
    private final Activity activity;
    private final String url;
    private final String needle;
    private final long timeoutMs;
    private final Handler main = new Handler(Looper.getMainLooper());
    private WebView web;
    private long startedAt;
    private volatile String state = "loading"; // loading | done | error | cancelled
    private volatile String html = "";
    private volatile String message = "";
    private volatile String title = "";
    private volatile boolean finished;

    public WebFetch(Activity activity, String url, String needle, long timeoutMs) {
        this.activity = activity;
        this.url = url;
        this.needle = needle;
        this.timeoutMs = timeoutMs;
    }

    /** Starts the job on the main thread. */
    public void start() {
        activity.runOnUiThread(this);
    }

    public String state() {
        return state;
    }

    public String html() {
        return html;
    }

    public String message() {
        return message;
    }

    public String title() {
        return title;
    }

    /** Stops the job and frees the WebView. Safe to call more than once. */
    public void cancel() {
        main.post(new Runnable() {
            public void run() {
                if (!finished) {
                    state = "cancelled";
                    finished = true;
                }
                destroy();
            }
        });
    }

    /** Main thread: build the WebView and start loading. */
    public void run() {
        try {
            startedAt = System.currentTimeMillis();
            web = new WebView(activity);
            WebSettings s = web.getSettings();
            s.setJavaScriptEnabled(true);
            s.setDomStorageEnabled(true);
            s.setLoadsImagesAutomatically(false);
            s.setBlockNetworkImage(true);
            web.setWebViewClient(new WebViewClient() {
                public void onPageFinished(WebView view, String u) {
                    schedulePoll(700);
                }
            });
            web.loadUrl(url);
            schedulePoll(1500);
        } catch (Throwable t) {
            fail("WebView unavailable: " + t);
        }
    }

    private void schedulePoll(long delay) {
        main.postDelayed(new Runnable() {
            public void run() {
                poll();
            }
        }, delay);
    }

    private void poll() {
        if (finished || web == null) {
            return;
        }
        if (System.currentTimeMillis() - startedAt > timeoutMs) {
            fail("The page did not finish loading in time");
            return;
        }
        try {
            title = String.valueOf(web.getTitle());
            // evaluateJavascript is API 19+: called reflectively so the helper compiles against old stubs
            Method m = WebView.class.getMethod("evaluateJavascript", String.class, ValueCallback.class);
            String script = "(function(){var h=document.documentElement.outerHTML;"
                    + "return h.indexOf(" + quote(needle) + ")>=0?h:'';})()";
            m.invoke(web, script, new ValueCallback<String>() {
                public void onReceiveValue(String value) {
                    onDom(value);
                }
            });
        } catch (Throwable t) {
            fail("Could not read the page: " + t);
        }
    }

    private void onDom(String jsonValue) {
        if (finished) {
            return;
        }
        String text = decode(jsonValue);
        if (text.length() > 0) {
            html = text;
            state = "done";
            finished = true;
            destroy();
        } else {
            // challenge pages resolve and navigate by themselves: keep polling until the timeout
            schedulePoll(1000);
        }
    }

    private void fail(String why) {
        if (finished) {
            return;
        }
        message = why;
        state = "error";
        finished = true;
        destroy();
    }

    private void destroy() {
        WebView w = web;
        web = null;
        if (w != null) {
            try {
                w.stopLoading();
                w.destroy();
            } catch (Throwable ignored) {
            }
        }
    }

    private static String quote(String s) {
        StringBuilder b = new StringBuilder("\"");
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c == '"' || c == '\\') {
                b.append('\\').append(c);
            } else if (c < 0x20) {
                b.append(' ');
            } else {
                b.append(c);
            }
        }
        return b.append('"').toString();
    }

    /** JSON string literal (as returned by evaluateJavascript) to plain text. */
    private static String decode(String json) {
        if (json == null || json.length() < 2 || json.charAt(0) != '"') {
            return "";
        }
        StringBuilder out = new StringBuilder(json.length());
        int end = json.length() - 1;
        for (int i = 1; i < end; i++) {
            char c = json.charAt(i);
            if (c != '\\' || i + 1 >= end) {
                out.append(c);
                continue;
            }
            char n = json.charAt(++i);
            switch (n) {
                case 'n': out.append('\n'); break;
                case 't': out.append('\t'); break;
                case 'r': out.append('\r'); break;
                case 'b': out.append('\b'); break;
                case 'f': out.append('\f'); break;
                case 'u':
                    if (i + 4 < json.length()) {
                        try {
                            out.append((char) Integer.parseInt(json.substring(i + 1, i + 5), 16));
                        } catch (NumberFormatException e) {
                            // leave malformed escapes out
                        }
                        i += 4;
                    }
                    break;
                default: out.append(n); // \" \\ \/
            }
        }
        return out.toString();
    }
}
