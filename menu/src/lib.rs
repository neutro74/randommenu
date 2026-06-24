#![allow(dead_code)]

mod config;
mod mods;
mod state;

use std::ffi::c_void;
use gorilla_api::{gorilla_init, GorillaResult};

// called once by the C# loader plugin after it loads this DLL
#[no_mangle]
pub unsafe extern "C" fn menu_init() {
    let mut s = match state::state().lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    s.load();
}

// called every frame by the C# plugin's Update()
// bitmask = which mods are currently enabled (bit 0 = speed, 1 = fly, etc.)
// C# owns the bitmask state; we just apply effects and detect transitions
#[no_mangle]
pub unsafe extern "C" fn menu_tick(bitmask: u32) {
    // lazy init gorilla API — retry until the game assemblies are loaded
    let mut s = match state::state().lock() {
        Ok(g) => g,
        Err(_) => return,
    };

    if !s.api_ready {
        let r = gorilla_init();
        if r == GorillaResult::Ok || r == GorillaResult::AlreadyInitialised {
            s.api_ready = true;
        } else {
            return;
        }
    }

    // detect transitions and call on_enable / on_disable
    let changed = bitmask ^ s.prev_enabled;
    for i in 0..6u32 {
        if changed & (1 << i) != 0 {
            if bitmask & (1 << i) != 0 {
                mods::on_enable(i);
            } else {
                mods::on_disable(i);
            }
        }
    }
    s.prev_enabled = bitmask;
    drop(s);

    // per-frame tick for enabled mods that need it
    for i in 0..6u32 {
        if bitmask & (1 << i) != 0 {
            mods::tick(i);
        }
    }
}

// returns the saved bitmask so C# can restore last session state on startup
#[no_mangle]
pub unsafe extern "C" fn menu_load_saved() -> u32 {
    let s = match state::state().lock() {
        Ok(g) => g,
        Err(_) => return 0,
    };
    s.enabled
}

// called by C# when a mod is toggled — saves to disk
#[no_mangle]
pub unsafe extern "C" fn menu_save(bitmask: u32) {
    let mut cfg = config::load();
    cfg.enabled_bitmask = bitmask;
    config::save(&cfg);
}

#[no_mangle]
pub unsafe extern "system" fn DllMain(
    _module: *mut c_void,
    _reason: u32,
    _reserved: *mut c_void,
) -> i32 {
    1
}
