use std::sync::{Mutex, OnceLock};
use crate::config;

pub struct MenuState {
    // bitmask of which mods are on, bit 0 = speed, 1 = fly, 2 = long arms, 3 = freeze, 4 = ghost, 5 = bounce
    pub enabled: u32,
    pub loaded: bool,
    // tracks previous bitmask so we can detect enable/disable transitions
    pub prev_enabled: u32,
    pub api_ready: bool,
}

impl MenuState {
    fn new() -> Self {
        MenuState { enabled: 0, loaded: false, prev_enabled: 0, api_ready: false }
    }

    pub fn load(&mut self) {
        let cfg = config::load();
        self.enabled = cfg.enabled_bitmask;
        self.loaded = true;
    }

    pub fn toggle(&mut self, index: u32) {
        self.enabled ^= 1 << index;
        let mut cfg = config::load();
        cfg.enabled_bitmask = self.enabled;
        config::save(&cfg);
    }
}

static STATE_CELL: OnceLock<Mutex<MenuState>> = OnceLock::new();

pub fn state() -> &'static Mutex<MenuState> {
    STATE_CELL.get_or_init(|| Mutex::new(MenuState::new()))
}
