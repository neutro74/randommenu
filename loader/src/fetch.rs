use std::ffi::{c_char, c_void, CString};

// wininet handles for http requests
type HINTERNET = *mut c_void;

extern "system" {
    fn InternetOpenA(
        agent: *const c_char,
        access_type: u32,
        proxy: *const c_char,
        proxy_bypass: *const c_char,
        flags: u32,
    ) -> HINTERNET;
    fn InternetOpenUrlA(
        internet: HINTERNET,
        url: *const c_char,
        headers: *const c_char,
        headers_len: u32,
        flags: u32,
        context: usize,
    ) -> HINTERNET;
    fn InternetReadFile(
        file: HINTERNET,
        buf: *mut c_void,
        bytes_to_read: u32,
        bytes_read: *mut u32,
    ) -> i32;
    fn InternetCloseHandle(internet: HINTERNET) -> i32;
}

// INTERNET_OPEN_TYPE_DIRECT and INTERNET_FLAG_RELOAD
const OPEN_TYPE_DIRECT: u32 = 1;
const FLAG_RELOAD: u32 = 0x80000000;
const FLAG_NO_CACHE_WRITE: u32 = 0x04000000;

// downloads url to a Vec<u8>, returns None on failure
pub fn download(url: &str) -> Option<Vec<u8>> {
    let agent = CString::new("randommenu-loader/1.0").ok()?;
    let url_c = CString::new(url).ok()?;

    unsafe {
        let inet = InternetOpenA(
            agent.as_ptr(),
            OPEN_TYPE_DIRECT,
            std::ptr::null(),
            std::ptr::null(),
            0,
        );
        if inet.is_null() {
            return None;
        }

        let handle = InternetOpenUrlA(
            inet,
            url_c.as_ptr(),
            std::ptr::null(),
            0,
            FLAG_RELOAD | FLAG_NO_CACHE_WRITE,
            0,
        );
        if handle.is_null() {
            InternetCloseHandle(inet);
            return None;
        }

        let mut out = Vec::new();
        let mut chunk = vec![0u8; 4096];
        loop {
            let mut read: u32 = 0;
            let ok = InternetReadFile(
                handle,
                chunk.as_mut_ptr() as *mut c_void,
                chunk.len() as u32,
                &mut read,
            );
            if ok == 0 || read == 0 {
                break;
            }
            out.extend_from_slice(&chunk[..read as usize]);
        }

        InternetCloseHandle(handle);
        InternetCloseHandle(inet);

        if out.is_empty() { None } else { Some(out) }
    }
}
