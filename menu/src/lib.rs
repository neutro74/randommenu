#![allow(dead_code)]

mod config;
mod hook;
mod mods;
mod render;
mod state;
mod ui;

use std::ffi::{c_char, c_void};
use std::sync::OnceLock;
use gorilla_api::{gorilla_init, GorillaResult};

// windows api we need
extern "system" {
    fn GetModuleHandleA(name: *const c_char) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
}

// opaque types for mono api calls inside this module
type MonoClass = c_void;
type MonoMethod = c_void;
type MonoDomain = c_void;

// global hook, kept alive so the trampoline stays valid
static HOOK: OnceLock<hook::Hook> = OnceLock::new();

// our replacement for GorillaTagger.LateUpdate
// unity calls this via the hook we install
unsafe extern "C" fn hooked_late_update() {
    // call the original LateUpdate first so game logic runs normally
    if let Some(h) = HOOK.get() {
        let orig = h.call_original();
        orig();
    }
    // then run our menu tick
    ui::tick();
}

// finds the JIT-compiled native address of GorillaTagger.LateUpdate
unsafe fn find_late_update_fn() -> Option<*mut u8> {
    let mono_mod = {
        let name = b"mono-2.0-bdwgc.dll\0";
        GetModuleHandleA(name.as_ptr() as *const c_char)
    };
    if mono_mod.is_null() { return None; }

    macro_rules! sym {
        ($name:literal) => {{
            let ptr = GetProcAddress(mono_mod, concat!($name, "\0").as_ptr() as *const c_char);
            if ptr.is_null() { return None; }
            ptr
        }};
    }

    // function pointer types we need here
    type FnGetRootDomain    = unsafe extern "C" fn() -> *mut MonoDomain;
    type FnAssemblyForeach  = unsafe extern "C" fn(unsafe extern "C" fn(*mut c_void, *mut c_void), *mut c_void);
    type FnAssemblyGetImage = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
    type FnImageGetName     = unsafe extern "C" fn(*mut c_void) -> *const c_char;
    type FnClassFromName    = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> *mut MonoClass;
    type FnMethodFromName   = unsafe extern "C" fn(*mut MonoClass, *const c_char, i32) -> *mut MonoMethod;
    type FnCompileMethod    = unsafe extern "C" fn(*mut MonoMethod) -> *mut c_void;

    let _get_root_domain:   FnGetRootDomain    = std::mem::transmute(sym!("mono_get_root_domain"));
    let assembly_foreach:   FnAssemblyForeach  = std::mem::transmute(sym!("mono_assembly_foreach"));
    let assembly_get_image: FnAssemblyGetImage = std::mem::transmute(sym!("mono_assembly_get_image"));
    let image_get_name:     FnImageGetName     = std::mem::transmute(sym!("mono_image_get_name"));
    let class_from_name:    FnClassFromName    = std::mem::transmute(sym!("mono_class_from_name"));
    let method_from_name:   FnMethodFromName   = std::mem::transmute(sym!("mono_class_get_method_from_name"));
    let compile_method:     FnCompileMethod    = std::mem::transmute(sym!("mono_compile_method"));

    // find Assembly-CSharp image by iterating all loaded assemblies
    struct FindState { image: *mut c_void, get_image: FnAssemblyGetImage, get_name: FnImageGetName }
    unsafe extern "C" fn find_cb(assembly: *mut c_void, data: *mut c_void) {
        let s = &mut *(data as *mut FindState);
        if !s.image.is_null() { return; }
        let img = (s.get_image)(assembly);
        if img.is_null() { return; }
        let name_ptr = (s.get_name)(img);
        if name_ptr.is_null() { return; }
        let name = std::ffi::CStr::from_ptr(name_ptr).to_str().unwrap_or("");
        if name == "Assembly-CSharp" { s.image = img; }
    }
    let mut find_state = FindState { image: std::ptr::null_mut(), get_image: assembly_get_image, get_name: image_get_name };
    assembly_foreach(find_cb, &mut find_state as *mut _ as *mut c_void);
    if find_state.image.is_null() { return None; }

    // find GorillaTagger class in the root namespace
    let class = class_from_name(
        find_state.image,
        b"\0".as_ptr() as *const c_char,
        b"GorillaTagger\0".as_ptr() as *const c_char,
    );
    if class.is_null() { return None; }

    // get LateUpdate method
    let method = method_from_name(
        class,
        b"LateUpdate\0".as_ptr() as *const c_char,
        0,
    );
    if method.is_null() { return None; }

    // JIT compile it to get the native code address
    let native_fn = compile_method(method);
    if native_fn.is_null() { None } else { Some(native_fn as *mut u8) }
}

// called by the loader after it loads this dll
#[no_mangle]
pub unsafe extern "C" fn menu_start() {
    // init gorilla api (resolves all class/field/method references)
    let result = gorilla_init();
    if result != GorillaResult::Ok && result != GorillaResult::AlreadyInitialised {
        return;
    }

    // create the in-world Unity GameObjects for the menu panel
    render::init_render();

    // find and hook GorillaTagger.LateUpdate
    if let Some(late_update_ptr) = find_late_update_fn() {
        let h = hook::Hook::install(late_update_ptr, hooked_late_update as *const () as usize);
        let _ = HOOK.set(h);
    }
}

#[no_mangle]
pub unsafe extern "system" fn DllMain(
    _module: *mut c_void,
    _reason: u32,
    _reserved: *mut c_void,
) -> i32 {
    1
}
