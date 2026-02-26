use rb_sys::rb_thread_call_without_gvl;
use std::ffi::c_void;

pub fn without_gvl<F, R>(f: F) -> R
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    // Check env var bypass
    if std::env::var("WREQ_RB_NO_GVL_RELEASE").is_ok() {
        return f();
    }

    extern "C" fn call<F, R>(data: *mut c_void) -> *mut c_void
    where
        F: FnOnce() -> R + Send,
        R: Send,
    {
        let func: Box<F> = unsafe { Box::from_raw(data as *mut F) };
        let result = func();
        Box::into_raw(Box::new(result)) as *mut c_void
    }

    let data = Box::into_raw(Box::new(f)) as *mut c_void;
    let result =
        unsafe { rb_thread_call_without_gvl(Some(call::<F, R>), data, None, std::ptr::null_mut()) };
    let result: Box<R> = unsafe { Box::from_raw(result as *mut R) };
    *result
}
