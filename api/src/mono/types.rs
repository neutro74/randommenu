use std::ffi::c_void;

// Opaque Mono runtime types — all accessed through the embedding API.
pub type MonoDomain     = c_void;
pub type MonoAssembly   = c_void;
pub type MonoImage      = c_void;
pub type MonoClass      = c_void;
pub type MonoClassField = c_void;
pub type MonoMethod     = c_void;
pub type MonoObject     = c_void;
pub type MonoString     = c_void;
pub type MonoArray      = c_void;
pub type MonoVTable     = c_void;
pub type MonoProperty   = c_void;

// MonoObject header on x64: two 8-byte pointers (vtable + sync), so unboxed
// value starts at byte offset 16.
pub const MONO_OBJECT_HEADER_SIZE: usize = 16;

/// Read a value-type field from a boxed MonoObject (e.g., a boxed Vector3).
/// Safety: caller must ensure `obj` is a non-null boxed value type of the
/// correct layout.
pub unsafe fn unbox<T: Copy>(obj: *mut MonoObject) -> T {
    let ptr = (obj as *const u8).add(MONO_OBJECT_HEADER_SIZE) as *const T;
    ptr.read()
}
