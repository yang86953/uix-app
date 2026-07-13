use std::ffi::{c_char, c_void, CString};

pub(crate) type Id = *mut c_void;
type Sel = *mut c_void;
type Class = *mut c_void;
type Ivar = *mut c_void;

pub(crate) const INVALID_IVAR_OFFSET: isize = -1;

#[repr(C)]
struct ObjcSuper {
    receiver: Id,
    super_class: Class,
}

#[link(name = "objc")]
unsafe extern "C" {
    fn class_addIvar(
        cls: Class,
        name: *const c_char,
        size: usize,
        alignment: u8,
        types: *const c_char,
    ) -> i8;
    fn class_getInstanceVariable(cls: Class, name: *const c_char) -> Ivar;
    fn class_getSuperclass(cls: Class) -> Class;
    fn ivar_getOffset(ivar: Ivar) -> isize;
    fn objc_msgSendSuper();
}

/// Adds a non-Objective-C `void *` ivar and returns its byte offset.
///
/// The caller must invoke this before registering `class` and use the returned
/// offset only with instances of that class.
pub(crate) unsafe fn add_raw_pointer_ivar(class: Class, name: &str) -> Option<isize> {
    if class.is_null() {
        return None;
    }
    let name = CString::new(name).ok()?;
    let encoding = CString::new("^v").ok()?;
    let alignment = std::mem::align_of::<*mut c_void>().trailing_zeros() as u8;
    if class_addIvar(
        class,
        name.as_ptr(),
        std::mem::size_of::<*mut c_void>(),
        alignment,
        encoding.as_ptr(),
    ) == 0
    {
        return None;
    }
    let ivar = class_getInstanceVariable(class, name.as_ptr());
    if ivar.is_null() {
        return None;
    }
    let offset = ivar_getOffset(ivar);
    (offset >= 0).then_some(offset)
}

/// Moves a Rust allocation into a zero-initialized raw-pointer ivar.
///
/// On failure, ownership is returned to the caller; no Objective-C retain or
/// release operation is ever applied to the Rust pointer.
pub(crate) unsafe fn install_box<T>(
    object: Id,
    offset: isize,
    value: Box<T>,
) -> Result<(), Box<T>> {
    let Some(slot) = raw_pointer_slot::<T>(object, offset) else {
        return Err(value);
    };
    if !(*slot).is_null() {
        return Err(value);
    }
    slot.write(Box::into_raw(value));
    Ok(())
}

/// Borrows the pointer stored in a raw-pointer ivar without changing ownership.
pub(crate) unsafe fn box_ptr<T>(object: Id, offset: isize) -> *mut T {
    raw_pointer_slot::<T>(object, offset)
        .map(|slot| *slot)
        .unwrap_or(std::ptr::null_mut())
}

/// Clears a raw-pointer ivar and recovers its Rust allocation exactly once.
pub(crate) unsafe fn take_box<T>(object: Id, offset: isize) -> Option<Box<T>> {
    let slot = raw_pointer_slot::<T>(object, offset)?;
    let pointer = slot.replace(std::ptr::null_mut());
    if pointer.is_null() {
        None
    } else {
        Some(Box::from_raw(pointer))
    }
}

/// Finishes a dynamically implemented `dealloc` by invoking the superclass.
pub(crate) unsafe fn call_super_dealloc(object: Id, current_class: Class) {
    if object.is_null() || current_class.is_null() {
        return;
    }
    let superclass = class_getSuperclass(current_class);
    if superclass.is_null() {
        return;
    }
    let mut receiver = ObjcSuper {
        receiver: object,
        super_class: superclass,
    };
    type FnType = unsafe extern "C" fn(*mut ObjcSuper, Sel);
    let function: FnType = std::mem::transmute(objc_msgSendSuper as unsafe extern "C" fn());
    function(&mut receiver, selector("dealloc"));
}

unsafe fn raw_pointer_slot<T>(object: Id, offset: isize) -> Option<*mut *mut T> {
    if object.is_null() || offset < 0 {
        return None;
    }
    Some(object.cast::<u8>().offset(offset).cast::<*mut T>())
}

unsafe fn selector(name: &str) -> Sel {
    #[link(name = "objc")]
    unsafe extern "C" {
        fn sel_registerName(name: *const c_char) -> Sel;
    }

    CString::new(name)
        .ok()
        .map(|name| sel_registerName(name.as_ptr()))
        .unwrap_or(std::ptr::null_mut())
}
