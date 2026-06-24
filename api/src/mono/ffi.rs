use std::ffi::{c_char, c_int, c_void};
use super::types::*;

// ---------------------------------------------------------------------------
// Function pointer types for the Mono embedding API
// ---------------------------------------------------------------------------

pub type FnGetRootDomain         = unsafe extern "C" fn() -> *mut MonoDomain;
pub type FnAssemblyGetImage      = unsafe extern "C" fn(*mut MonoAssembly) -> *mut MonoImage;
pub type FnClassFromName         = unsafe extern "C" fn(*mut MonoImage, *const c_char, *const c_char) -> *mut MonoClass;
pub type FnClassGetFieldFromName = unsafe extern "C" fn(*mut MonoClass, *const c_char) -> *mut MonoClassField;
pub type FnClassGetMethodFromName = unsafe extern "C" fn(*mut MonoClass, *const c_char, c_int) -> *mut MonoMethod;
pub type FnClassGetPropFromName  = unsafe extern "C" fn(*mut MonoClass, *const c_char) -> *mut MonoProperty;
pub type FnPropGetGetMethod      = unsafe extern "C" fn(*mut MonoProperty) -> *mut MonoMethod;
pub type FnRuntimeInvoke         = unsafe extern "C" fn(*mut MonoMethod, *mut c_void, *mut *mut c_void, *mut *mut MonoObject) -> *mut MonoObject;
pub type FnFieldGetValue         = unsafe extern "C" fn(*mut MonoObject, *mut MonoClassField, *mut c_void);
pub type FnFieldSetValue         = unsafe extern "C" fn(*mut MonoObject, *mut MonoClassField, *mut c_void);
pub type FnFieldStaticGetValue   = unsafe extern "C" fn(*mut MonoVTable, *mut MonoClassField, *mut c_void);
pub type FnFieldStaticSetValue   = unsafe extern "C" fn(*mut MonoVTable, *mut MonoClassField, *mut c_void);
pub type FnClassVtable           = unsafe extern "C" fn(*mut MonoDomain, *mut MonoClass) -> *mut MonoVTable;
pub type FnStringToUtf8          = unsafe extern "C" fn(*mut MonoString) -> *mut c_char;
pub type FnStringNew             = unsafe extern "C" fn(*mut MonoDomain, *const c_char) -> *mut MonoString;
pub type FnFree                  = unsafe extern "C" fn(*mut c_void);
pub type FnObjectGetClass        = unsafe extern "C" fn(*mut MonoObject) -> *mut MonoClass;
pub type FnObjectUnbox           = unsafe extern "C" fn(*mut MonoObject) -> *mut c_void;
pub type FnArrayLength           = unsafe extern "C" fn(*mut MonoArray) -> usize;
pub type FnArrayAddrWithSize     = unsafe extern "C" fn(*mut MonoArray, c_int, usize) -> *mut c_void;
pub type FnImageGetName          = unsafe extern "C" fn(*mut MonoImage) -> *const c_char;
pub type FnAssemblyForeach       = unsafe extern "C" fn(unsafe extern "C" fn(*mut MonoAssembly, *mut c_void), *mut c_void);
pub type FnThreadAttach          = unsafe extern "C" fn(*mut MonoDomain) -> *mut c_void;

// ---------------------------------------------------------------------------
// MonoApi — all function pointers packed into one struct, loaded once.
// ---------------------------------------------------------------------------

pub struct MonoApi {
    pub get_root_domain:           FnGetRootDomain,
    pub assembly_get_image:        FnAssemblyGetImage,
    pub class_from_name:           FnClassFromName,
    pub class_get_field_from_name: FnClassGetFieldFromName,
    pub class_get_method_from_name: FnClassGetMethodFromName,
    pub class_get_prop_from_name:  FnClassGetPropFromName,
    pub prop_get_get_method:       FnPropGetGetMethod,
    pub runtime_invoke:            FnRuntimeInvoke,
    pub field_get_value:           FnFieldGetValue,
    pub field_set_value:           FnFieldSetValue,
    pub field_static_get_value:    FnFieldStaticGetValue,
    pub field_static_set_value:    FnFieldStaticSetValue,
    pub class_vtable:              FnClassVtable,
    pub string_to_utf8:            FnStringToUtf8,
    pub string_new:                FnStringNew,
    pub free:                      FnFree,
    pub object_get_class:          FnObjectGetClass,
    pub object_unbox:              FnObjectUnbox,
    pub array_length:              FnArrayLength,
    pub array_addr_with_size:      FnArrayAddrWithSize,
    pub image_get_name:            FnImageGetName,
    pub assembly_foreach:          FnAssemblyForeach,
    pub thread_attach:             FnThreadAttach,
}

// ---------------------------------------------------------------------------
// Windows API to grab function pointers from the already-loaded mono DLL.
// Under Proton/Wine the DLL is already mapped into the process.
// ---------------------------------------------------------------------------

extern "system" {
    fn GetModuleHandleA(name: *const c_char) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
}

unsafe fn resolve(module: *mut c_void, name: &[u8]) -> Result<*mut c_void, String> {
    let ptr = GetProcAddress(module, name.as_ptr() as *const c_char);
    if ptr.is_null() {
        Err(format!(
            "GetProcAddress failed for {}",
            std::str::from_utf8(name).unwrap_or("?")
        ))
    } else {
        Ok(ptr)
    }
}

macro_rules! fn_ptr {
    ($module:expr, $name:literal) => {{
        std::mem::transmute(resolve($module, concat!($name, "\0").as_bytes())?)
    }};
}

impl MonoApi {
    /// Load all Mono embedding API function pointers from the already-resident
    /// `mono-2.0-bdwgc.dll` (loaded by Unity/Proton before our code runs).
    pub unsafe fn load() -> Result<Self, String> {
        let mono_dll = b"mono-2.0-bdwgc.dll\0";
        let module = GetModuleHandleA(mono_dll.as_ptr() as *const c_char);
        if module.is_null() {
            return Err("mono-2.0-bdwgc.dll is not loaded in this process".into());
        }
        Ok(MonoApi {
            get_root_domain:           fn_ptr!(module, "mono_get_root_domain"),
            assembly_get_image:        fn_ptr!(module, "mono_assembly_get_image"),
            class_from_name:           fn_ptr!(module, "mono_class_from_name"),
            class_get_field_from_name: fn_ptr!(module, "mono_class_get_field_from_name"),
            class_get_method_from_name:fn_ptr!(module, "mono_class_get_method_from_name"),
            class_get_prop_from_name:  fn_ptr!(module, "mono_class_get_property_from_name"),
            prop_get_get_method:       fn_ptr!(module, "mono_property_get_get_method"),
            runtime_invoke:            fn_ptr!(module, "mono_runtime_invoke"),
            field_get_value:           fn_ptr!(module, "mono_field_get_value"),
            field_set_value:           fn_ptr!(module, "mono_field_set_value"),
            field_static_get_value:    fn_ptr!(module, "mono_field_static_get_value"),
            field_static_set_value:    fn_ptr!(module, "mono_field_static_set_value"),
            class_vtable:              fn_ptr!(module, "mono_class_vtable"),
            string_to_utf8:            fn_ptr!(module, "mono_string_to_utf8"),
            string_new:                fn_ptr!(module, "mono_string_new"),
            free:                      fn_ptr!(module, "mono_free"),
            object_get_class:          fn_ptr!(module, "mono_object_get_class"),
            object_unbox:              fn_ptr!(module, "mono_object_unbox"),
            array_length:              fn_ptr!(module, "mono_array_length"),
            array_addr_with_size:      fn_ptr!(module, "mono_array_addr_with_size"),
            image_get_name:            fn_ptr!(module, "mono_image_get_name"),
            assembly_foreach:          fn_ptr!(module, "mono_assembly_foreach"),
            thread_attach:             fn_ptr!(module, "mono_thread_attach"),
        })
    }
}
