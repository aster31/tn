use env_logger;
use log::debug;
use std::{
    ffi::{CStr, c_int},
    ptr::null_mut,
};
use tcl_sys::*;

mod array;

extern "C" fn tn_test(
    _cdata: ClientData,
    interp: *mut Tcl_Interp,
    objc: c_int,
    objv: *const *mut Tcl_Obj,
) -> c_int {
    println!("OBJC: {}", objc);

    for i in 0..(objc as usize) {
        // Unsafe: we assume the Tcl interpreter returned a correct objc
        let obj: *mut Tcl_Obj = unsafe { *objv.add(i) };
        let mut length: c_int = 0;
        let str_val = unsafe { CStr::from_ptr(Tcl_GetStringFromObj(obj, &mut length)) };
        println!("{}: {}", i, str_val.to_str().unwrap_or("invalid utf8"));
    }

    unsafe {
        // Unsafe: third argument is the proc called by Tcl to free it, None=TCL_STATIC.
        Tcl_SetResult(interp, c"hello world!".as_ptr() as *mut i8, None);
    }
    return TCL_OK as c_int;
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Tn_Init(interp: *mut Tcl_Interp) -> c_int {
    env_logger::init();
    array::array_init(interp);

    unsafe {
        let _ = Tcl_CreateNamespace(interp, c"tn".as_ptr(), null_mut(), None);
    }

    unsafe {
        Tcl_CreateObjCommand(
            interp,
            c"::tn::test".as_ptr(),
            Some(tn_test),
            null_mut(),
            None,
        );
    }

    return unsafe { Tcl_PkgProvide(interp, c"tn".as_ptr(), c"0.1".as_ptr()) };
}
