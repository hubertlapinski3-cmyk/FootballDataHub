//! Small JNI bridge for Android View/Configuration APIs that NativeActivity does not expose.
//!
//! Every call runs inside a JNI local frame and clears any Java exception, so a missing API
//! (older Android) or a refused call degrades to `None` / no-op instead of crashing.
//! Desktop builds use the stub at the bottom of this file.

#[cfg(target_os = "android")]
mod imp {
    use jni_sys::*;
    use std::ffi::{c_void, CString};
    use std::ptr::null_mut;
    use winit::platform::android::activity::AndroidApp;

    /// JNI environment of the current thread. A thread that was not attached already (a background
    /// job) is attached for the lifetime of this value and detached on drop: a native thread that
    /// exits while attached aborts the process on Android.
    struct Env(*mut JNIEnv, Option<*mut JavaVM>);

    impl Drop for Env {
        fn drop(&mut self) {
            if let Some(vm) = self.1 {
                unsafe {
                    if let Some(detach) = (**vm).DetachCurrentThread {
                        detach(vm);
                    }
                }
            }
        }
    }

    impl Env {
        unsafe fn attach(app: &AndroidApp) -> Option<Env> {
            let vm = app.vm_as_ptr() as *mut JavaVM;
            if vm.is_null() {
                return None;
            }
            let mut env: *mut c_void = null_mut();
            let get = (**vm).GetEnv?;
            let mut attached_here = None;
            if get(vm, &mut env, JNI_VERSION_1_6) != JNI_OK {
                let att = (**vm).AttachCurrentThread?;
                if att(vm, &mut env, null_mut()) != JNI_OK {
                    return None;
                }
                attached_here = Some(vm);
            }
            Some(Env(env as *mut JNIEnv, attached_here))
        }
        unsafe fn f(&self) -> &JNINativeInterface_ {
            &**self.0
        }
        /// Clears a pending Java exception; true when there was one.
        unsafe fn failed(&self) -> bool {
            if (self.f().ExceptionCheck.unwrap())(self.0) != 0 {
                (self.f().ExceptionClear.unwrap())(self.0);
                true
            } else {
                false
            }
        }
        unsafe fn method(&self, obj: jobject, name: &str, sig: &str) -> Option<jmethodID> {
            let cls = (self.f().GetObjectClass.unwrap())(self.0, obj);
            let (n, s) = (CString::new(name).ok()?, CString::new(sig).ok()?);
            let m = (self.f().GetMethodID.unwrap())(self.0, cls, n.as_ptr(), s.as_ptr());
            if self.failed() || m.is_null() {
                return None;
            }
            Some(m)
        }
        unsafe fn obj(&self, obj: jobject, name: &str, sig: &str, args: &[jvalue]) -> Option<jobject> {
            let m = self.method(obj, name, sig)?;
            let r = (self.f().CallObjectMethodA.unwrap())(self.0, obj, m, args.as_ptr());
            if self.failed() || r.is_null() {
                return None;
            }
            Some(r)
        }
        unsafe fn int(&self, obj: jobject, name: &str, sig: &str, args: &[jvalue]) -> Option<jint> {
            let m = self.method(obj, name, sig)?;
            let r = (self.f().CallIntMethodA.unwrap())(self.0, obj, m, args.as_ptr());
            if self.failed() {
                return None;
            }
            Some(r)
        }
        unsafe fn void(&self, obj: jobject, name: &str, sig: &str, args: &[jvalue]) -> Option<()> {
            let m = self.method(obj, name, sig)?;
            (self.f().CallVoidMethodA.unwrap())(self.0, obj, m, args.as_ptr());
            if self.failed() {
                return None;
            }
            Some(())
        }
        unsafe fn field_id(&self, obj: jobject, name: &str, sig: &str) -> Option<jfieldID> {
            let cls = (self.f().GetObjectClass.unwrap())(self.0, obj);
            let (n, s) = (CString::new(name).ok()?, CString::new(sig).ok()?);
            let id = (self.f().GetFieldID.unwrap())(self.0, cls, n.as_ptr(), s.as_ptr());
            if self.failed() || id.is_null() {
                return None;
            }
            Some(id)
        }
        unsafe fn class(&self, name: &str) -> Option<jclass> {
            let n = CString::new(name).ok()?;
            let c = (self.f().FindClass.unwrap())(self.0, n.as_ptr());
            if self.failed() || c.is_null() {
                return None;
            }
            Some(c)
        }
        unsafe fn new(&self, class: &str, sig: &str, args: &[jvalue]) -> Option<jobject> {
            let cls = self.class(class)?;
            let (n, s) = (CString::new("<init>").ok()?, CString::new(sig).ok()?);
            let m = (self.f().GetMethodID.unwrap())(self.0, cls, n.as_ptr(), s.as_ptr());
            if self.failed() || m.is_null() {
                return None;
            }
            let o = (self.f().NewObjectA.unwrap())(self.0, cls, m, args.as_ptr());
            if self.failed() || o.is_null() {
                return None;
            }
            Some(o)
        }
        unsafe fn sdk_int(&self) -> Option<jint> {
            let cls = self.class("android/os/Build$VERSION")?;
            let (n, s) = (CString::new("SDK_INT").ok()?, CString::new("I").ok()?);
            let id = (self.f().GetStaticFieldID.unwrap())(self.0, cls, n.as_ptr(), s.as_ptr());
            if self.failed() || id.is_null() {
                return None;
            }
            Some((self.f().GetStaticIntField.unwrap())(self.0, cls, id))
        }
        unsafe fn jstr(&self, s: &str) -> Option<jstring> {
            let c = CString::new(s).ok()?;
            let r = (self.f().NewStringUTF.unwrap())(self.0, c.as_ptr());
            if self.failed() || r.is_null() {
                return None;
            }
            Some(r)
        }
        unsafe fn to_rust(&self, s: jstring) -> Option<String> {
            let p = (self.f().GetStringUTFChars.unwrap())(self.0, s, null_mut());
            if p.is_null() {
                self.failed();
                return None;
            }
            let out = std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned();
            (self.f().ReleaseStringUTFChars.unwrap())(self.0, s, p);
            Some(out)
        }
        /// `obj.toString()` as a Rust string.
        unsafe fn to_string_of(&self, obj: jobject) -> Option<String> {
            let s = self.obj(obj, "toString", "()Ljava/lang/String;", &[])?;
            self.to_rust(s)
        }
        unsafe fn static_obj_field(&self, class: &str, name: &str, sig: &str) -> Option<jobject> {
            let cls = self.class(class)?;
            let (n, s) = (CString::new(name).ok()?, CString::new(sig).ok()?);
            let id = (self.f().GetStaticFieldID.unwrap())(self.0, cls, n.as_ptr(), s.as_ptr());
            if self.failed() || id.is_null() {
                return None;
            }
            let o = (self.f().GetStaticObjectField.unwrap())(self.0, cls, id);
            if self.failed() || o.is_null() {
                return None;
            }
            Some(o)
        }
        /// Run `f` inside a local reference frame so per-frame calls never leak local refs.
        unsafe fn scoped<T>(&self, f: impl FnOnce(&Env) -> Option<T>) -> Option<T> {
            if (self.f().PushLocalFrame.unwrap())(self.0, 32) != 0 {
                self.failed();
                return None;
            }
            let r = f(self);
            (self.f().PopLocalFrame.unwrap())(self.0, null_mut());
            r
        }
        unsafe fn decor_view(&self, app: &AndroidApp) -> Option<jobject> {
            let activity = app.activity_as_ptr() as jobject;
            let window = self.obj(activity, "getWindow", "()Landroid/view/Window;", &[])?;
            self.obj(window, "getDecorView", "()Landroid/view/View;", &[])
        }
    }

    /// `Resources.getConfiguration().fontScale` (1.0 = default).
    pub fn font_scale(app: &AndroidApp) -> Option<f32> {
        unsafe {
            let env = Env::attach(app)?;
            env.scoped(|e| {
                let activity = app.activity_as_ptr() as jobject;
                let res = e.obj(activity, "getResources", "()Landroid/content/res/Resources;", &[])?;
                let cfg = e.obj(res, "getConfiguration", "()Landroid/content/res/Configuration;", &[])?;
                let id = e.field_id(cfg, "fontScale", "F")?;
                let v = (e.f().GetFloatField.unwrap())(e.0, cfg, id);
                (v > 0.0).then_some(v)
            })
        }
    }

    /// Pixels at the bottom of the window covered by the soft keyboard (or the navigation bar),
    /// from `View.getWindowVisibleDisplayFrame`.
    pub fn bottom_obscured_px(app: &AndroidApp) -> Option<i32> {
        unsafe {
            let env = Env::attach(app)?;
            env.scoped(|e| {
                let decor = e.decor_view(app)?;
                let rect = e.new("android/graphics/Rect", "()V", &[])?;
                e.void(decor, "getWindowVisibleDisplayFrame", "(Landroid/graphics/Rect;)V", &[jvalue { l: rect }])?;
                let id = e.field_id(rect, "bottom", "I")?;
                let bottom = (e.f().GetIntField.unwrap())(e.0, rect, id);
                let h = e.int(decor, "getHeight", "()I", &[])?;
                Some((h - bottom).max(0))
            })
        }
    }

    /// First item of the primary clip as text (`ClipboardManager.getPrimaryClip`, `coerceToText`).
    /// Android 10+ only returns it while the app is in the foreground, which is the case on a tap.
    pub fn get_clipboard_text(app: &AndroidApp) -> Option<String> {
        unsafe {
            let env = Env::attach(app)?;
            env.scoped(|e| {
                let activity = app.activity_as_ptr() as jobject;
                let name = e.jstr("clipboard")?;
                let cm = e.obj(activity, "getSystemService", "(Ljava/lang/String;)Ljava/lang/Object;", &[jvalue { l: name }])?;
                let clip = e.obj(cm, "getPrimaryClip", "()Landroid/content/ClipData;", &[])?;
                if e.int(clip, "getItemCount", "()I", &[])? < 1 {
                    return None;
                }
                let item = e.obj(clip, "getItemAt", "(I)Landroid/content/ClipData$Item;", &[jvalue { i: 0 }])?;
                let text = e.obj(item, "coerceToText", "(Landroid/content/Context;)Ljava/lang/CharSequence;", &[jvalue { l: activity }])?;
                e.to_string_of(text)
            })
        }
    }

    /// Insert a PNG into the shared Pictures collection (MediaStore, API 29+; no permission needed)
    /// and return its content:// URI. The system chooses nothing: the file lands in Pictures/DataHub.
    pub fn save_png_to_pictures(app: &AndroidApp, filename: &str, png: &[u8]) -> Option<String> {
        unsafe {
            let env = Env::attach(app)?;
            env.scoped(|e| {
                if e.sdk_int()? < 29 {
                    return None;
                }
                let activity = app.activity_as_ptr() as jobject;
                let cv = e.new("android/content/ContentValues", "()V", &[])?;
                for (k, v) in [("_display_name", filename), ("mime_type", "image/png"), ("relative_path", "Pictures/DataHub")] {
                    let (jk, jv) = (e.jstr(k)?, e.jstr(v)?);
                    e.void(cv, "put", "(Ljava/lang/String;Ljava/lang/String;)V", &[jvalue { l: jk }, jvalue { l: jv }])?;
                }
                let resolver = e.obj(activity, "getContentResolver", "()Landroid/content/ContentResolver;", &[])?;
                let table = e.static_obj_field("android/provider/MediaStore$Images$Media", "EXTERNAL_CONTENT_URI", "Landroid/net/Uri;")?;
                let uri = e.obj(resolver, "insert", "(Landroid/net/Uri;Landroid/content/ContentValues;)Landroid/net/Uri;", &[jvalue { l: table }, jvalue { l: cv }])?;
                let os = e.obj(resolver, "openOutputStream", "(Landroid/net/Uri;)Ljava/io/OutputStream;", &[jvalue { l: uri }])?;
                let arr = (e.f().NewByteArray.unwrap())(e.0, png.len() as jsize);
                if e.failed() || arr.is_null() {
                    return None;
                }
                (e.f().SetByteArrayRegion.unwrap())(e.0, arr, 0, png.len() as jsize, png.as_ptr() as *const jbyte);
                e.void(os, "write", "([B)V", &[jvalue { l: arr }])?;
                e.void(os, "close", "()V", &[])?;
                e.to_string_of(uri)
            })
        }
    }

    /// System share sheet (`ACTION_SEND`, image/png) for a content:// URI such as the one returned by
    /// `save_png_to_pictures`. The chooser lets the user pick Drive, Files, a messenger, and so on.
    pub fn share_png(app: &AndroidApp, uri: &str) -> Option<()> {
        unsafe {
            let env = Env::attach(app)?;
            env.scoped(|e| {
                let activity = app.activity_as_ptr() as jobject;
                let action = e.jstr("android.intent.action.SEND")?;
                let intent = e.new("android/content/Intent", "(Ljava/lang/String;)V", &[jvalue { l: action }])?;
                let mime = e.jstr("image/png")?;
                e.obj(intent, "setType", "(Ljava/lang/String;)Landroid/content/Intent;", &[jvalue { l: mime }])?;
                let juri = e.jstr(uri)?;
                let cls = e.class("android/net/Uri")?;
                let (n, sg) = (CString::new("parse").ok()?, CString::new("(Ljava/lang/String;)Landroid/net/Uri;").ok()?);
                let mid = (e.f().GetStaticMethodID.unwrap())(e.0, cls, n.as_ptr(), sg.as_ptr());
                if e.failed() || mid.is_null() {
                    return None;
                }
                let parsed = (e.f().CallStaticObjectMethodA.unwrap())(e.0, cls, mid, [jvalue { l: juri }].as_ptr());
                if e.failed() || parsed.is_null() {
                    return None;
                }
                let key = e.jstr("android.intent.extra.STREAM")?;
                e.obj(intent, "putExtra", "(Ljava/lang/String;Landroid/os/Parcelable;)Landroid/content/Intent;", &[jvalue { l: key }, jvalue { l: parsed }])?;
                e.obj(intent, "addFlags", "(I)Landroid/content/Intent;", &[jvalue { i: 1 }])?; // FLAG_GRANT_READ_URI_PERMISSION
                let ccls = e.class("android/content/Intent")?;
                let (cn, cs) = (CString::new("createChooser").ok()?, CString::new("(Landroid/content/Intent;Ljava/lang/CharSequence;)Landroid/content/Intent;").ok()?);
                let cm = (e.f().GetStaticMethodID.unwrap())(e.0, ccls, cn.as_ptr(), cs.as_ptr());
                if e.failed() || cm.is_null() {
                    return None;
                }
                let title = e.jstr("Share board")?;
                let chooser = (e.f().CallStaticObjectMethodA.unwrap())(e.0, ccls, cm, [jvalue { l: intent }, jvalue { l: title }].as_ptr());
                if e.failed() || chooser.is_null() {
                    return None;
                }
                e.void(activity, "startActivity", "(Landroid/content/Intent;)V", &[jvalue { l: chooser }])
            })
        }
    }

    /// Loads `url` in an off-screen `WebView` and returns the rendered page HTML once it contains
    /// `needle` (a real browser engine passes JavaScript challenges that plain HTTP cannot).
    ///
    /// The WebView lives in `dev.datahub.WebFetch`, a small Java class compiled to DEX by
    /// `scripts/build_webfetch_dex.sh` and loaded here with `InMemoryDexClassLoader` (API 26+),
    /// because a native thread cannot implement `Runnable` or `ValueCallback` itself. Blocks the
    /// calling thread, which must be a background thread, polling every 400 ms.
    pub fn webview_fetch(app: &AndroidApp, url: &str, needle: &str, timeout_secs: u64, cancelled: &dyn Fn() -> bool, progress: &mut dyn FnMut(&str)) -> Result<String, String> {
        static DEX: &[u8] = include_bytes!("../assets/webfetch.dex");
        if DEX.len() < 64 {
            return Err("The built-in browser helper is not part of this build".into());
        }
        unsafe {
            let env = Env::attach(app).ok_or("No JNI environment")?;
            let e = &env;
            if e.sdk_int().unwrap_or(0) < 26 {
                return Err("The built-in browser needs Android 8 or newer".into());
            }
            let activity = app.activity_as_ptr() as jobject;
            // ---- load the helper class from the embedded DEX and create the job (global ref: it outlives local frames)
            let job: jobject = e
                .scoped(|e| {
                    let arr = (e.f().NewByteArray.unwrap())(e.0, DEX.len() as jsize);
                    if e.failed() || arr.is_null() {
                        return None;
                    }
                    (e.f().SetByteArrayRegion.unwrap())(e.0, arr, 0, DEX.len() as jsize, DEX.as_ptr() as *const jbyte);
                    let bcls = e.class("java/nio/ByteBuffer")?;
                    let (wn, ws) = (CString::new("wrap").ok()?, CString::new("([B)Ljava/nio/ByteBuffer;").ok()?);
                    let wrap = (e.f().GetStaticMethodID.unwrap())(e.0, bcls, wn.as_ptr(), ws.as_ptr());
                    if e.failed() || wrap.is_null() {
                        return None;
                    }
                    let buf = (e.f().CallStaticObjectMethodA.unwrap())(e.0, bcls, wrap, [jvalue { l: arr }].as_ptr());
                    if e.failed() || buf.is_null() {
                        return None;
                    }
                    let parent = e.obj(activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?;
                    let loader = e.new("dalvik/system/InMemoryDexClassLoader", "(Ljava/nio/ByteBuffer;Ljava/lang/ClassLoader;)V", &[jvalue { l: buf }, jvalue { l: parent }])?;
                    let name = e.jstr("dev.datahub.WebFetch")?;
                    let cls = e.obj(loader, "loadClass", "(Ljava/lang/String;)Ljava/lang/Class;", &[jvalue { l: name }])? as jclass;
                    let (cn, cs) = (CString::new("<init>").ok()?, CString::new("(Landroid/app/Activity;Ljava/lang/String;Ljava/lang/String;J)V").ok()?);
                    let ctor = (e.f().GetMethodID.unwrap())(e.0, cls, cn.as_ptr(), cs.as_ptr());
                    if e.failed() || ctor.is_null() {
                        return None;
                    }
                    let (ju, jn) = (e.jstr(url)?, e.jstr(needle)?);
                    let o = (e.f().NewObjectA.unwrap())(e.0, cls, ctor, [jvalue { l: activity }, jvalue { l: ju }, jvalue { l: jn }, jvalue { j: (timeout_secs * 1000) as jlong }].as_ptr());
                    if e.failed() || o.is_null() {
                        return None;
                    }
                    let g = (e.f().NewGlobalRef.unwrap())(e.0, o);
                    (!g.is_null()).then_some(g)
                })
                .ok_or("The built-in browser could not be started")?;
            let free = |env: &Env| {
                let _ = env.scoped(|e| {
                    (e.f().DeleteGlobalRef.unwrap())(e.0, job);
                    Some(())
                });
            };
            let call = |name: &str| -> Option<String> { env.scoped(|e| e.obj(job, name, "()Ljava/lang/String;", &[]).and_then(|s| e.to_rust(s))) };
            if env.scoped(|e| e.void(job, "start", "()V", &[])).is_none() {
                free(&env);
                return Err("The built-in browser could not be started".into());
            }
            let t0 = std::time::Instant::now();
            let mut last_title = String::new();
            let result = loop {
                if cancelled() {
                    let _ = env.scoped(|e| e.void(job, "cancel", "()V", &[]));
                    break Err("Cancelled".to_string());
                }
                std::thread::sleep(std::time::Duration::from_millis(400));
                match call("state").as_deref() {
                    Some("done") => break call("html").filter(|h| !h.is_empty()).ok_or_else(|| "The browser returned an empty page".to_string()),
                    Some("error") => break Err(call("message").unwrap_or_else(|| "The built-in browser failed".into())),
                    Some("cancelled") => break Err("Cancelled".to_string()),
                    Some(_) => {
                        let title = call("title").unwrap_or_default();
                        if title != last_title {
                            last_title = title.clone();
                            progress(&format!("browser: {}", if title.is_empty() || title == "null" { "loading" } else { &title }));
                        }
                    }
                    None => break Err("The built-in browser stopped answering".to_string()),
                }
                if t0.elapsed().as_secs() > timeout_secs + 10 {
                    let _ = env.scoped(|e| e.void(job, "cancel", "()V", &[]));
                    break Err("The built-in browser timed out".to_string());
                }
            };
            free(&env);
            result
        }
    }

    /// `View.setSystemGestureExclusionRects` on the decor view (API 29+). Rects are window pixels
    /// [left, top, right, bottom]; an empty slice clears the exclusion.
    pub fn set_gesture_exclusion(app: &AndroidApp, rects: &[[i32; 4]]) -> Option<()> {
        unsafe {
            let env = Env::attach(app)?;
            env.scoped(|e| {
                if e.sdk_int()? < 29 {
                    return None;
                }
                let decor = e.decor_view(app)?;
                let list = e.new("java/util/ArrayList", "()V", &[])?;
                for r in rects {
                    let jr = e.new("android/graphics/Rect", "(IIII)V", &[jvalue { i: r[0] }, jvalue { i: r[1] }, jvalue { i: r[2] }, jvalue { i: r[3] }])?;
                    let m = e.method(list, "add", "(Ljava/lang/Object;)Z")?;
                    (e.f().CallBooleanMethodA.unwrap())(e.0, list, m, [jvalue { l: jr }].as_ptr());
                    if e.failed() {
                        return None;
                    }
                }
                e.void(decor, "setSystemGestureExclusionRects", "(Ljava/util/List;)V", &[jvalue { l: list }])
            })
        }
    }
}

#[cfg(target_os = "android")]
pub use imp::*;
