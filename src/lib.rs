use env_logger;
use std::{
    ffi::{CStr, c_int},
    ptr::null_mut,
};
use tcl_sys::*;

mod array;

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Tn_Init(interp: *mut Tcl_Interp) -> c_int {
    env_logger::init();

    unsafe {
        Tcl_CreateNamespace(interp, c"tn".as_ptr(), null_mut(), None);
        array::array_init(interp);
    }

    return unsafe { Tcl_PkgProvide(interp, c"tn".as_ptr(), c"0.1".as_ptr()) };
}
