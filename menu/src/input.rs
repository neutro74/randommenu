// wraps OVRInput from Oculus.VR.dll to read controller buttons via Mono

use std::ffi::{c_char, c_void, CStr};
use std::sync::OnceLock;

extern "system" {
    fn GetModuleHandleA(name: *const c_char) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
}

type FnAssemblyForeach  = unsafe extern "C" fn(unsafe extern "C" fn(*mut c_void, *mut c_void), *mut c_void);
type FnAssemblyGetImage = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type FnImageGetName     = unsafe extern "C" fn(*mut c_void) -> *const c_char;
type FnClassFromName    = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> *mut c_void;
type FnClassGetMethod   = unsafe extern "C" fn(*mut c_void, *const c_char, i32) -> *mut c_void;
type FnRuntimeInvoke    = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void, *mut *mut c_void) -> *mut c_void;
type FnObjectUnbox      = unsafe extern "C" fn(*mut c_void) -> *mut c_void;

struct OvrInput {
    get_down: *mut c_void,    // OVRInput.GetDown(Button, Controller)
    runtime_invoke: FnRuntimeInvoke,
    object_unbox: FnObjectUnbox,
}

unsafe impl Send for OvrInput {}
unsafe impl Sync for OvrInput {}

static OVR: OnceLock<OvrInput> = OnceLock::new();

// OVRInput.Button.Two = 2  (Y on left controller, B on right)
// OVRInput.Controller.LTouch = 4  (left touch controller only)
const BUTTON_TWO: i32 = 2;
const CONTROLLER_LTOUCH: i32 = 4;

unsafe fn mono_sym<T: Copy>(mono: *mut c_void, name: &[u8]) -> Option<T> {
    let p = GetProcAddress(mono, name.as_ptr() as *const c_char);
    if p.is_null() { None } else { Some(std::mem::transmute_copy(&p)) }
}

struct FindState<'a> {
    target: &'a str,
    image: *mut c_void,
    get_image: FnAssemblyGetImage,
    get_name: FnImageGetName,
}
unsafe extern "C" fn find_cb(asm: *mut c_void, data: *mut c_void) {
    let s = &mut *(data as *mut FindState);
    if !s.image.is_null() { return; }
    let img = (s.get_image)(asm);
    if img.is_null() { return; }
    let np = (s.get_name)(img);
    if np.is_null() { return; }
    if CStr::from_ptr(np).to_str().unwrap_or("") == s.target { s.image = img; }
}

pub unsafe fn init() -> bool {
    let mono = GetModuleHandleA(b"mono-2.0-bdwgc.dll\0".as_ptr() as *const c_char);
    if mono.is_null() { return false; }

    let assembly_foreach: FnAssemblyForeach = match mono_sym(mono, b"mono_assembly_foreach\0") { Some(f) => f, None => return false };
    let assembly_get_image: FnAssemblyGetImage = match mono_sym(mono, b"mono_assembly_get_image\0") { Some(f) => f, None => return false };
    let image_get_name: FnImageGetName = match mono_sym(mono, b"mono_image_get_name\0") { Some(f) => f, None => return false };
    let class_from_name: FnClassFromName = match mono_sym(mono, b"mono_class_from_name\0") { Some(f) => f, None => return false };
    let class_get_method: FnClassGetMethod = match mono_sym(mono, b"mono_class_get_method_from_name\0") { Some(f) => f, None => return false };
    let runtime_invoke: FnRuntimeInvoke = match mono_sym(mono, b"mono_runtime_invoke\0") { Some(f) => f, None => return false };
    let object_unbox: FnObjectUnbox = match mono_sym(mono, b"mono_object_unbox\0") { Some(f) => f, None => return false };

    // find Oculus.VR assembly image
    let mut state = FindState { target: "Oculus.VR", image: std::ptr::null_mut(), get_image: assembly_get_image, get_name: image_get_name };
    assembly_foreach(find_cb, &mut state as *mut _ as *mut c_void);
    if state.image.is_null() { return false; }

    // OVRInput is a top-level class (empty namespace)
    let class = class_from_name(state.image, b"\0".as_ptr() as *const c_char, b"OVRInput\0".as_ptr() as *const c_char);
    if class.is_null() { return false; }

    // GetDown(OVRInput.Button, OVRInput.Controller) — 2 args
    let get_down = class_get_method(class, b"GetDown\0".as_ptr() as *const c_char, 2);
    if get_down.is_null() { return false; }

    let _ = OVR.set(OvrInput { get_down, runtime_invoke, object_unbox });
    true
}

// returns true on the frame the Y button is first pressed (GetDown, not GetKey)
pub unsafe fn y_button_down() -> bool {
    let ovr = match OVR.get() { Some(o) => o, None => return false };

    let mut btn = BUTTON_TWO;
    let mut ctrl = CONTROLLER_LTOUCH;
    let mut args: [*mut c_void; 2] = [
        &mut btn  as *mut i32 as *mut c_void,
        &mut ctrl as *mut i32 as *mut c_void,
    ];
    let mut exc: *mut c_void = std::ptr::null_mut();
    let result = (ovr.runtime_invoke)(ovr.get_down, std::ptr::null_mut(), args.as_mut_ptr(), &mut exc);
    if result.is_null() || !exc.is_null() { return false; }

    // GetDown returns bool (boxed) — unbox to read the byte
    let unboxed = (ovr.object_unbox)(result);
    if unboxed.is_null() { return false; }
    *(unboxed as *const u8) != 0
}
