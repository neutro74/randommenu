use std::ffi::{c_char, CStr};
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

extern "system" {
    fn GetEnvironmentVariableA(name: *const c_char, buf: *mut c_char, size: u32) -> u32;
}

#[derive(Serialize, Deserialize)]
pub struct LoaderConfig {
    // url to fetch the latest menu dll from
    pub menu_url: String,
    // local path to save the downloaded dll
    pub menu_dll_name: String,
}

impl Default for LoaderConfig {
    fn default() -> Self {
        LoaderConfig {
            menu_url: "https://github.com/neutro74/randommenu/releases/latest/download/randommenu.dll".into(),
            menu_dll_name: "randommenu.dll".into(),
        }
    }
}

// returns %APPDATA%\randommenu or falls back to current dir
pub fn config_dir() -> PathBuf {
    let mut buf = vec![0i8; 512];
    let name = b"APPDATA\0";
    let len = unsafe {
        GetEnvironmentVariableA(name.as_ptr() as *const c_char, buf.as_mut_ptr(), buf.len() as u32)
    };
    if len > 0 {
        let s = unsafe { CStr::from_ptr(buf.as_ptr()) };
        let mut path = PathBuf::from(s.to_string_lossy().into_owned());
        path.push("randommenu");
        let _ = std::fs::create_dir_all(&path);
        path
    } else {
        PathBuf::from(".")
    }
}

pub fn load_config() -> LoaderConfig {
    let path = config_dir().join("loader.json");
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(cfg) = serde_json::from_str(&data) {
            return cfg;
        }
    }
    let cfg = LoaderConfig::default();
    let _ = std::fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap());
    cfg
}
