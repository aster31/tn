use log::debug;
use nalgebra::{DMatrix, Dim, Dyn, VecStorage};
use std::alloc::{Layout, alloc, dealloc, handle_alloc_error};
use std::ffi::{CString, c_int, c_void};
use std::ptr::null_mut;
use std::str::FromStr;
use tcl_sys::*;

// TODO split into two types, U = *mut T, where "U" is the public interface with all the impl's, T is just a struct with the memory layout
// that allows for a more safe API
pub struct TnArray {
    // TODO implement rc and make data a *const DMatrix<f64>
    pub data: Option<DMatrix<f64>>,
    refc: usize,
}

impl Clone for TnArray {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            refc: 1,
        }
    }
}

impl TnArray {
    pub unsafe fn to_obj(self_: *mut Self) -> *mut Tcl_Obj {
        unsafe {
            let res = Tcl_NewObj();
            (*res).internalRep.otherValuePtr = self_ as *mut c_void;
            (*res).typePtr = &ARRAY_TYPE;
            (*res).bytes = null_mut();
            (*res).length = 0;
            return res;
        }
    }

    pub fn cols(&self) -> usize {
        match &self.data {
            None => 0,
            Some(x) => x.ncols(),
        }
    }

    pub fn rows(&self) -> usize {
        match &self.data {
            None => 0,
            Some(x) => x.nrows(),
        }
    }

    pub unsafe fn clone(self_: *mut Self) -> *mut Self {
        unsafe {
            match &(&*self_).data {
                None => return Self::from_vec(0, 0, vec![]),
                Some(x) => Self::from_slice((&*self_).cols(), (&*self_).rows(), x.as_slice()),
            }
        }
    }

    pub unsafe fn free(self_: *mut Self) {
        let layout = Layout::new::<TnArray>();
        unsafe {
            dealloc(self_ as *mut u8, layout);
        }
    }

    pub fn from_slice(cols: usize, rows: usize, data: &[f64]) -> *mut TnArray {
        return Self::from_vec(cols, rows, data.to_vec());
    }

    pub fn from_vec(cols: usize, rows: usize, data: Vec<f64>) -> *mut TnArray {
        unsafe {
            let layout = Layout::new::<TnArray>();
            let res = alloc(layout) as *mut TnArray;
            // for now panic on alloc failures
            if res.is_null() {
                handle_alloc_error(layout);
            }
            if cols > 0 && rows > 0 {
                (*res).data = Some(DMatrix::from_vec(rows, cols, data));
            } else {
                (*res).data = None;
            }
            (*res).refc = 1;
            return res;
        }
    }

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
    pub unsafe fn cast_from_obj(
        interp: *mut Tcl_Interp,
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

        // convert from tcl list
        // note: Tcl_ListObjGetElements will convert string->list if possible

        let mut elements: *mut *mut Tcl_Obj = null_mut();
        let mut rows: c_int = 0;
        unsafe {
            if Tcl_ListObjGetElements(interp, obj, &mut rows, &mut elements) != TCL_OK {
                // not a list or string parseable into a list
                let mut value: f64 = 0.0;
                if Tcl_GetDoubleFromObj(interp, obj, &mut value) == TCL_OK {
                    return Ok(TnArray::from_vec(1, 1, vec![value]));
                }
                // could be a single element - e.g. a double - TODO
                return Err(c"Object can't be converted to ::tn::array type.".to_owned());
            }
        }

        if rows == 0 {
            // EMPTY ARRAY
            return Ok(TnArray::from_vec(0, 0, vec![]));
        }

        let mut sub_elements: *mut *mut Tcl_Obj = null_mut();
        let mut cols: c_int = 0;

        unsafe {
            if Tcl_ListObjGetElements(interp, *elements, &mut cols, &mut sub_elements) != TCL_OK {
                // some non-list type
                cols = 1;
            }
        }

        let mut data = vec![0.; cols as usize * rows as usize];

        for row in 0..rows as usize {
            let mut sub_elements: *mut *mut Tcl_Obj = null_mut();
            let mut c_cols: c_int = 0;

            // NON LIST ELEMENT
            unsafe {
                if Tcl_ListObjGetElements(
                    interp,
                    *elements.add(row),
                    &mut c_cols,
                    &mut sub_elements,
                ) != TCL_OK
                {
                    if cols != 1 {
                        return Err(CString::from_str(&format!(
                            "Found a single element at row {row}, expected a list {cols} long."
                        ))
                        // from_str fails if it contains a nul byte internally, which we are sure it won't
                        .unwrap());
                    }
                    let mut value: f64 = 0.0;
                    if Tcl_GetDoubleFromObj(interp, *elements.add(row), &mut value) != TCL_OK {
                        return Err(CString::from_str(&format!(
                            "Found a non-numeric element at row {row}."
                        ))
                        .unwrap());
                    }
                    data[row] = value;
                }
            }

            // LIST ELEMENT
            if c_cols != cols {
                return Err(CString::from_str(&format!(
                    "List at row {row} is {c_cols} long, but expected {cols}."
                ))
                .unwrap());
            }

            for col in 0..cols as usize {
                let mut value: f64 = 0.0;
                unsafe {
                    if Tcl_GetDoubleFromObj(interp, *sub_elements.add(col), &mut value) != TCL_OK {
                        return Err(CString::from_str(&format!(
                            "Element at row {row} col {col} is not a valid number."
                        ))
                        .unwrap());
                    }
                }
                data[col * rows as usize + row] = value;
            }
        }

        return Ok(TnArray::from_vec(cols as usize, rows as usize, data));
    }
}

unsafe extern "C" fn free_int_rep(tcl_obj: *mut Tcl_Obj) {
    debug!("{tcl_obj:?} - free_int_rep");
    unsafe {
        let arr = &mut *TnArray::unchecked_from_obj(tcl_obj);
        match arr.data.as_ref() {
            Some(_) => debug!("data old refc: {}", arr.refc),
            None => debug!("data is none"),
        }

        TnArray::free(arr);
        (*tcl_obj).internalRep.otherValuePtr = null_mut();
    }
}

unsafe extern "C" fn dup_int_rep(src_obj: *mut Tcl_Obj, dup_obj: *mut Tcl_Obj) {
    debug!("{src_obj:?} to {dup_obj:?} - dup_int_rep");
    unsafe {
        let arr = TnArray::unchecked_from_obj(src_obj);
        (*dup_obj).internalRep.otherValuePtr = arr.clone() as *mut c_void;

        match (*arr).data.as_ref() {
            Some(_) => debug!("new src data refc: {}", (*arr).refc),
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

// TODO make some macro that makes it easier to write Tcl API more safely - handle objc, objv,
// conversions, error handling with CString
// do this through traits
extern "C" fn tn_array(
    _cdata: ClientData,
    interp: *mut Tcl_Interp,
    objc: c_int,
    objv: *const *mut Tcl_Obj,
) -> c_int {
    if objc != 2 {
        unsafe {
            Tcl_WrongNumArgs(interp, 1, objv, c"list".as_ptr());
            return TCL_ERROR;
        }
    }

    unsafe {
        let data = match TnArray::cast_from_obj(interp, *objv.add(1)) {
            Ok(x) => x,
            Err(x) => {
                // TODO memory leak
                Tcl_SetResult(interp, x.into_raw(), TCL_STATIC);
                return TCL_ERROR;
            }
        };
        Tcl_SetObjResult(interp, TnArray::to_obj(data));
    }
    return TCL_OK;
}

extern "C" fn tn_pretty_print(
    _cdata: ClientData,
    interp: *mut Tcl_Interp,
    objc: c_int,
    objv: *const *mut Tcl_Obj,
) -> c_int {
    if objc != 2 {
        unsafe {
            Tcl_WrongNumArgs(interp, 1, objv, c"arr".as_ptr());
            return TCL_ERROR;
        }
    }

    let data = match unsafe { TnArray::cast_from_obj(interp, *objv.add(1)) } {
        Ok(x) => x,
        Err(x) => {
            // TODO memory leak
            unsafe {
                Tcl_SetResult(interp, x.into_raw(), TCL_STATIC);
                return TCL_ERROR;
            }
        }
    };

    unsafe {
        let Some(data) = &(*data).data else {
            println!("⎡ ⎦");
            return TCL_OK;
        };

        for row in 0..data.nrows() {
            let last = data.nrows() - 1;
            match row {
                0 => print!("⎡"),
                n if n == last => print!("⎣"),
                _ => print!("⎢"),
            }

            for col in 0..data.ncols() {
                print!("{} ", data[(row, col)]);
            }
            println!();
            // TODO: align columns nicely, print right side with ⎤ ⎦
        }
    }

    return TCL_OK;
}

pub unsafe fn array_init(interp: *mut Tcl_Interp) {
    unsafe {
        Tcl_RegisterObjType(&ARRAY_TYPE);
        Tcl_CreateObjCommand(
            interp,
            c"::tn::array".as_ptr(),
            Some(tn_array),
            null_mut(),
            None,
        );
        Tcl_CreateObjCommand(
            interp,
            c"::tn::pretty".as_ptr(),
            Some(tn_pretty_print),
            null_mut(),
            None,
        );
    }
}
