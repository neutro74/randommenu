use std::ffi::{c_char, CStr};
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

extern "system" {
    fn GetEnvironmentVariableA(name: *const c_char, buf: *mut c_char, size: u32) -> u32;
}

#[derive(Serialize, Deserialize, Default)]
pub struct MenuConfig {
    pub enabled_bitmask: u32,
}

fn config_dir() -> PathBuf {
    let mut buf = vec![0i8; 512];
    let len = unsafe {
        GetEnvironmentVariableA(b"APPDATA\0".as_ptr() as *const c_char, buf.as_mut_ptr(), 512)
    };
    if len > 0 {
        let s = unsafe { CStr::from_ptr(buf.as_ptr()) };
        let mut p = PathBuf::from(s.to_string_lossy().into_owned());
        p.push("randommenu");
        let _ = std::fs::create_dir_all(&p);
        p
    } else {
        PathBuf::from(".")
    }
}

pub fn load() -> MenuConfig {
    let path = config_dir().join("config.json");
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(cfg) = serde_json::from_str(&data) {
            return cfg;
        }
    }
    MenuConfig::default()
}

pub fn save(cfg: &MenuConfig) {
    let path = config_dir().join("config.json");
    if let Ok(s) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(path, s);
    }
}
