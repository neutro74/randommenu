use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use crate::mods::ALL_MODS;
use crate::config;

pub struct MenuState {
    pub open: bool,
    pub selected_index: usize,
    // ids of enabled mods
    pub enabled: HashSet<String>,
    // true after the first time state is loaded from disk
    pub loaded: bool,
}

impl MenuState {
    fn new() -> Self {
        MenuState {
            open: false,
            selected_index: 0,
            enabled: HashSet::new(),
            loaded: false,
        }
    }

    pub fn load_from_disk(&mut self) {
        let cfg = config::load();
        for id in &cfg.enabled_mods {
            self.enabled.insert(id.clone());
        }
        self.loaded = true;
    }

    pub fn save_to_disk(&self) {
        let mut cfg = config::load();
        cfg.enabled_mods = self.enabled.iter().cloned().collect();
        config::save(&cfg);
    }

    pub fn toggle(&mut self, id: &str) {
        if self.enabled.contains(id) {
            self.enabled.remove(id);
            if let Some(m) = ALL_MODS.iter().find(|m| m.id == id) {
                (m.on_disable)();
            }
        } else {
            self.enabled.insert(id.to_string());
            if let Some(m) = ALL_MODS.iter().find(|m| m.id == id) {
                (m.on_enable)();
            }
        }
        self.save_to_disk();
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled.contains(id)
    }

    pub fn scroll_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    pub fn scroll_down(&mut self) {
        if self.selected_index + 1 < ALL_MODS.len() {
            self.selected_index += 1;
        }
    }
}

static STATE_CELL: OnceLock<Mutex<MenuState>> = OnceLock::new();

// global state accessor
#[allow(non_snake_case)]
pub fn STATE() -> &'static Mutex<MenuState> {
    STATE_CELL.get_or_init(|| Mutex::new(MenuState::new()))
}
