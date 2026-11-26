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

    struct Env(*mut JNIEnv);

    impl Env {
        unsafe fn attach(app: &AndroidApp) -> Option<Env> {
            let vm = app.vm_as_ptr() as *mut JavaVM;
            if vm.is_null() {
                return None;
            }
            let mut env: *mut c_void = null_mut();
            let get = (**vm).GetEnv?;
            if get(vm, &mut env, JNI_VERSION_1_6) != JNI_OK {
                let att = (**vm).AttachCurrentThread?;
                if att(vm, &mut env, null_mut()) != JNI_OK {
                    return None;
                }
            }
            Some(Env(env as *mut JNIEnv))
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
