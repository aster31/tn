use log::debug;
use nalgebra::DMatrix;
use std::ffi::{CString, c_void};
use std::mem;
use std::rc::Rc;
use tcl_sys::*;

#[derive(Default, Clone)]
pub struct TnArray {
    data: Option<Rc<DMatrix<f64>>>,
}

impl TnArray {
    unsafe fn unchecked_from_obj(tcl_obj: *mut Tcl_Obj) -> *mut TnArray {
        return unsafe { (*tcl_obj).internalRep.otherValuePtr as *mut TnArray };
    }

    unsafe fn try_from_obj(obj: *mut Tcl_Obj) -> Option<*mut TnArray> {
        let type_ptr = unsafe { (*obj).typePtr };
        if type_ptr == (&ARRAY_TYPE as *const Tcl_ObjType) {
            unsafe { Some(TnArray::unchecked_from_obj(obj)) }
        } else {
            None
        }
    }

    /// Attempts to convert tcl_obj into TnArray
    ///
    /// If it is already a TnArray, cast it directly,
    /// if not, attempt to do a conversion from a tcl (nested) list.
    pub fn cast_from_obj(
        _interp: *mut Tcl_Interp,
        obj: *mut Tcl_Obj,
    ) -> Result<*mut TnArray, CString> {
        if obj.is_null() {
            panic!("Internal error: null passed to get_array_from_obj.");
        }

        // *mut Tcl_Objs are usually acquired from the Tcl interp, so hopefully good memory to deref
        unsafe {
            if let Some(obj) = TnArray::try_from_obj(obj) {
                return Ok(obj);
            }
        }

        todo!()
    }
}

extern "C" fn free_int_rep(tcl_obj: *mut Tcl_Obj) {
    debug!("{tcl_obj:?} - free_int_rep");
    unsafe {
        let arr = mem::take(&mut *TnArray::unchecked_from_obj(tcl_obj));

        match arr.data.as_ref() {
            Some(x) => debug!("data refc: {}", Rc::strong_count(x)),
            None => debug!("data is none"),
        }
        // just making a point
        drop(arr);
    }
}

extern "C" fn dup_int_rep(src_obj: *mut Tcl_Obj, dup_obj: *mut Tcl_Obj) {
    debug!("{src_obj:?} to {dup_obj:?} - dup_int_rep");
    unsafe {
        let arr = TnArray::unchecked_from_obj(src_obj);
        (*dup_obj).internalRep.otherValuePtr = arr.clone() as *mut c_void;

        match (*TnArray::unchecked_from_obj(src_obj)).data.as_ref() {
            Some(x) => debug!("new src data refc: {}", Rc::strong_count(x)),
            None => debug!("new src data is none"),
        }
    }
}

const ARRAY_TYPE: Tcl_ObjType = Tcl_ObjType {
    name: c"::tn::array".as_ptr(),
    freeIntRepProc: Some(free_int_rep),
    dupIntRepProc: Some(dup_int_rep),
    updateStringProc: None,
    setFromAnyProc: None,
};

pub fn array_init(_interp: *mut Tcl_Interp) {
    unsafe { Tcl_RegisterObjType(&ARRAY_TYPE) };
}
