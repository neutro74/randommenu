pub mod ffi;
pub mod types;

use std::ffi::{c_void, CStr, CString};
use ffi::MonoApi;
use types::*;

// ---------------------------------------------------------------------------
// Helper: iterate all loaded assemblies to find one by image name.
// ---------------------------------------------------------------------------

struct FindImageState {
    target: &'static str,
    result: *mut MonoImage,
    api: *const MonoApi,
}

unsafe extern "C" fn find_image_cb(assembly: *mut MonoAssembly, data: *mut c_void) {
    let state = &mut *(data as *mut FindImageState);
    if !state.result.is_null() {
        return;
    }
    let api = &*state.api;
    let image = (api.assembly_get_image)(assembly);
    if image.is_null() {
        return;
    }
    let name_ptr = (api.image_get_name)(image);
    if name_ptr.is_null() {
        return;
    }
    let name = CStr::from_ptr(name_ptr).to_str().unwrap_or("");
    if name == state.target {
        state.result = image;
    }
}

// ---------------------------------------------------------------------------
// MonoBridge — the central context for all Mono interactions.
// ---------------------------------------------------------------------------

pub struct MonoBridge {
    pub api: MonoApi,
    pub domain: *mut MonoDomain,
    pub csharp_image: *mut MonoImage,
}

// SAFETY: MonoBridge is only ever accessed from the Unity main thread.
unsafe impl Send for MonoBridge {}
unsafe impl Sync for MonoBridge {}

impl MonoBridge {
    pub unsafe fn init() -> Result<Self, String> {
        let api = MonoApi::load()?;

        let domain = (api.get_root_domain)();
        if domain.is_null() {
            return Err("mono_get_root_domain returned null".into());
        }

        // Attach the current thread so Mono GC knows about it.
        (api.thread_attach)(domain);

        // Find Assembly-CSharp by iterating loaded assemblies.
        let mut state = FindImageState {
            target: "Assembly-CSharp",
            result: std::ptr::null_mut(),
            api: &api as *const MonoApi,
        };
        (api.assembly_foreach)(find_image_cb, &mut state as *mut _ as *mut c_void);

        if state.result.is_null() {
            return Err("Assembly-CSharp image not found".into());
        }

        Ok(MonoBridge { api, domain, csharp_image: state.result })
    }

    // -------------------------------------------------------------------
    // Class / field / method lookup helpers
    // -------------------------------------------------------------------

    pub unsafe fn find_class(&self, namespace: &str, name: &str) -> Option<*mut MonoClass> {
        let ns  = CString::new(namespace).ok()?;
        let nm  = CString::new(name).ok()?;
        let cls = (self.api.class_from_name)(self.csharp_image, ns.as_ptr(), nm.as_ptr());
        if cls.is_null() { None } else { Some(cls) }
    }

    pub unsafe fn field(&self, class: *mut MonoClass, name: &str) -> Option<*mut MonoClassField> {
        let nm = CString::new(name).ok()?;
        let f  = (self.api.class_get_field_from_name)(class, nm.as_ptr());
        if f.is_null() { None } else { Some(f) }
    }

    pub unsafe fn method(&self, class: *mut MonoClass, name: &str, param_count: i32) -> Option<*mut MonoMethod> {
        let nm = CString::new(name).ok()?;
        let m  = (self.api.class_get_method_from_name)(class, nm.as_ptr(), param_count);
        if m.is_null() { None } else { Some(m) }
    }

    pub unsafe fn property_getter(&self, class: *mut MonoClass, name: &str) -> Option<*mut MonoMethod> {
        let nm   = CString::new(name).ok()?;
        let prop = (self.api.class_get_prop_from_name)(class, nm.as_ptr());
        if prop.is_null() { return None; }
        let getter = (self.api.prop_get_get_method)(prop);
        if getter.is_null() { None } else { Some(getter) }
    }

    pub unsafe fn vtable(&self, class: *mut MonoClass) -> Option<*mut MonoVTable> {
        let vt = (self.api.class_vtable)(self.domain, class);
        if vt.is_null() { None } else { Some(vt) }
    }

    // -------------------------------------------------------------------
    // Field value accessors
    // -------------------------------------------------------------------

    pub unsafe fn get_field<T: Copy>(&self, obj: *mut MonoObject, field: *mut MonoClassField) -> T {
        let mut val = std::mem::MaybeUninit::<T>::uninit();
        (self.api.field_get_value)(obj, field, val.as_mut_ptr() as *mut c_void);
        val.assume_init()
    }

    pub unsafe fn set_field<T: Copy>(&self, obj: *mut MonoObject, field: *mut MonoClassField, val: T) {
        let mut v = val;
        (self.api.field_set_value)(obj, field, &mut v as *mut T as *mut c_void);
    }

    pub unsafe fn get_static_field<T: Copy>(&self, vt: *mut MonoVTable, field: *mut MonoClassField) -> T {
        let mut val = std::mem::MaybeUninit::<T>::uninit();
        (self.api.field_static_get_value)(vt, field, val.as_mut_ptr() as *mut c_void);
        val.assume_init()
    }

    pub unsafe fn set_static_field<T: Copy>(&self, vt: *mut MonoVTable, field: *mut MonoClassField, val: T) {
        let mut v = val;
        (self.api.field_static_set_value)(vt, field, &mut v as *mut T as *mut c_void);
    }

    // -------------------------------------------------------------------
    // Method invocation helpers
    // -------------------------------------------------------------------

    /// Invoke a method with no parameters. Returns null for void methods.
    pub unsafe fn invoke0(&self, method: *mut MonoMethod, obj: *mut MonoObject) -> *mut MonoObject {
        (self.api.runtime_invoke)(method, obj as *mut c_void, std::ptr::null_mut(), std::ptr::null_mut())
    }

    /// Invoke a method with a pre-built params slice (each element is a
    /// *mut c_void pointing to the value — pass structs by pointer, objects
    /// directly).
    pub unsafe fn invoke(&self, method: *mut MonoMethod, obj: *mut MonoObject, params: &mut [*mut c_void]) -> *mut MonoObject {
        (self.api.runtime_invoke)(
            method,
            obj as *mut c_void,
            params.as_mut_ptr(),
            std::ptr::null_mut(),
        )
    }

    /// Invoke and unbox the returned value type.
    pub unsafe fn invoke0_unbox<T: Copy>(&self, method: *mut MonoMethod, obj: *mut MonoObject) -> Option<T> {
        let result = self.invoke0(method, obj);
        if result.is_null() { return None; }
        Some(unbox::<T>(result))
    }

    // -------------------------------------------------------------------
    // String helpers
    // -------------------------------------------------------------------

    /// Convert a MonoString* (returned from a field or method) to a Rust String.
    /// Returns empty string on null input.
    pub unsafe fn mono_string_to_rust(&self, ms: *mut MonoString) -> String {
        if ms.is_null() {
            return String::new();
        }
        let raw = (self.api.string_to_utf8)(ms);
        if raw.is_null() {
            return String::new();
        }
        let s = CStr::from_ptr(raw).to_string_lossy().into_owned();
        (self.api.free)(raw as *mut c_void);
        s
    }

    /// Null-safe field getter that returns the raw pointer for object fields.
    pub unsafe fn get_obj_field(&self, obj: *mut MonoObject, field: *mut MonoClassField) -> *mut MonoObject {
        self.get_field::<*mut MonoObject>(obj, field)
    }

    // -------------------------------------------------------------------
    // Array helpers
    // -------------------------------------------------------------------

    pub unsafe fn array_len(&self, arr: *mut MonoArray) -> usize {
        (self.api.array_length)(arr)
    }

    /// Get element at index as a raw pointer. element_size is sizeof the
    /// element type (e.g. sizeof(MonoObject*) = 8).
    pub unsafe fn array_get<T: Copy>(&self, arr: *mut MonoArray, idx: usize) -> T {
        let elem_size = std::mem::size_of::<T>();
        let ptr = (self.api.array_addr_with_size)(arr, elem_size as i32, idx) as *const T;
        ptr.read()
    }
}
