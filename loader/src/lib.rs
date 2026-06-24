mod config;
mod fetch;

use std::ffi::{c_char, c_void, CString};

extern "system" {
    fn LoadLibraryA(name: *const c_char) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
    fn CreateThread(
        attrs: *mut c_void,
        stack: usize,
        func: unsafe extern "system" fn(*mut c_void) -> u32,
        param: *mut c_void,
        flags: u32,
        id: *mut u32,
    ) -> *mut c_void;
}

// entry point called by the thread we spawn on DLL attach
unsafe extern "system" fn init_thread(_: *mut c_void) -> u32 {
    let cfg = config::load_config();
    let dir = config::config_dir();
    let dll_path = dir.join(&cfg.menu_dll_name);

    // try to download the latest menu dll
    if let Some(bytes) = fetch::download(&cfg.menu_url) {
        let _ = std::fs::write(&dll_path, bytes);
    }

    // load the menu dll (use downloaded version, fall back to whatever is on disk)
    let path_str = dll_path.to_string_lossy().into_owned();
    let path_c = CString::new(path_str).unwrap();
    let module = LoadLibraryA(path_c.as_ptr());
    if module.is_null() {
        return 1;
    }

    // call menu_start inside the loaded dll
    let fn_name = b"menu_start\0";
    let start_fn = GetProcAddress(module, fn_name.as_ptr() as *const c_char);
    if !start_fn.is_null() {
        let start: unsafe extern "C" fn() = std::mem::transmute(start_fn);
        start();
    }

    0
}

#[no_mangle]
pub unsafe extern "system" fn DllMain(
    _module: *mut c_void,
    reason: u32,
    _reserved: *mut c_void,
) -> i32 {
    // DLL_PROCESS_ATTACH = 1
    if reason == 1 {
        CreateThread(
            std::ptr::null_mut(),
            0,
            init_thread,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
        );
    }
    1
}
