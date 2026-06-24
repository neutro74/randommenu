/// GameMode — wraps `GorillaGameModes.GameMode` static class and the
/// `GorillaGameManager` base class for the active mode.
use std::ffi::c_void;
use crate::mono::{MonoBridge, types::*};
use super::types::GameModeType;

pub struct GameModeCache {
    gm_class:           *mut MonoClass,
    gm_vtable:          *mut MonoVTable,
    // Static properties on GameMode
    pg_active_mode:     *mut MonoMethod,
    pg_current_type:    *mut MonoMethod,
    m_is_playing:       *mut MonoMethod, // IsPlaying(GameModeType) -> bool
    m_local_is_tagged:  *mut MonoMethod, // LocalIsTagged(NetPlayer) -> bool

    // GorillaGameManager base
    ggm_class:          *mut MonoClass,
    // GorillaTagManager (Infection mode) — has IsInfected(NetPlayer) -> bool
    gtm_class:          *mut MonoClass,
    gtm_m_is_infected:  *mut MonoMethod,
}

unsafe impl Send for GameModeCache {}
unsafe impl Sync for GameModeCache {}

impl GameModeCache {
    pub unsafe fn build(bridge: &MonoBridge) -> Result<Self, String> {
        let gm_cls = bridge
            .find_class("GorillaGameModes", "GameMode")
            .ok_or("GameMode class not found")?;
        let gm_vt = bridge.vtable(gm_cls).ok_or("GameMode vtable not found")?;

        macro_rules! gm_prop {
            ($name:literal) => {
                bridge.property_getter(gm_cls, $name).ok_or(concat!("GameMode prop not found: ", $name))?
            };
        }
        macro_rules! gm_meth {
            ($name:literal, $n:expr) => {
                bridge.method(gm_cls, $name, $n).ok_or(concat!("GameMode method not found: ", $name))?
            };
        }

        let ggm_cls = bridge
            .find_class("", "GorillaGameManager")
            .ok_or("GorillaGameManager class not found")?;

        let gtm_cls = bridge
            .find_class("", "GorillaTagManager")
            .ok_or("GorillaTagManager class not found")?;
        let gtm_infected = bridge
            .method(gtm_cls, "IsInfected", 1)
            .ok_or("GorillaTagManager.IsInfected not found")?;

        Ok(GameModeCache {
            gm_class:          gm_cls,
            gm_vtable:         gm_vt,
            pg_active_mode:    gm_prop!("ActiveGameMode"),
            pg_current_type:   gm_prop!("CurrentGameModeType"),
            m_is_playing:      gm_meth!("IsPlaying", 1),
            m_local_is_tagged: gm_meth!("LocalIsTagged", 1),
            ggm_class:         ggm_cls,
            gtm_class:         gtm_cls,
            gtm_m_is_infected: gtm_infected,
        })
    }
}

// ---------------------------------------------------------------------------
// GameMode public handle
// ---------------------------------------------------------------------------

pub struct GameMode<'a> {
    cache:  &'a GameModeCache,
    bridge: &'a MonoBridge,
}

impl<'a> GameMode<'a> {
    pub fn new(cache: &'a GameModeCache, bridge: &'a MonoBridge) -> Self {
        GameMode { cache, bridge }
    }

    // -------------------------------------------------------------------
    // Mode identification
    // -------------------------------------------------------------------

    /// The active game mode type. Returns `GameModeType::None` if no match.
    pub unsafe fn current_mode_type(&self) -> GameModeType {
        let raw = self.bridge
            .invoke0_unbox::<i32>(self.cache.pg_current_type, std::ptr::null_mut())
            .unwrap_or(-1);
        GameModeType::from_raw(raw)
    }

    /// Check whether a specific game mode is currently running.
    pub unsafe fn is_playing(&self, mode: GameModeType) -> bool {
        let mut m = mode as i32;
        let mut params: [*mut c_void; 1] = [&mut m as *mut _ as *mut c_void];
        // IsPlaying is a static method — pass null as obj.
        self.bridge
            .invoke(self.cache.m_is_playing, std::ptr::null_mut(), &mut params)
            .pipe(|r| if r.is_null() { false } else { unbox::<u8>(r) != 0 })
    }

    /// Raw pointer to the active `GorillaGameManager` instance, or null if in
    /// the lobby / no active mode.
    pub unsafe fn active_game_manager(&self) -> *mut MonoObject {
        // Static property getter — invoke on null object.
        self.bridge.invoke0(self.cache.pg_active_mode, std::ptr::null_mut())
    }

    // -------------------------------------------------------------------
    // Tag / infection status
    // -------------------------------------------------------------------

    /// Whether the given NetPlayer object is currently tagged / infected in
    /// the active game mode.
    pub unsafe fn is_player_tagged(&self, net_player_obj: *mut MonoObject) -> bool {
        if net_player_obj.is_null() { return false; }
        let mut params: [*mut c_void; 1] = [net_player_obj as *mut c_void];
        self.bridge
            .invoke(self.cache.m_local_is_tagged, std::ptr::null_mut(), &mut params)
            .pipe(|r| if r.is_null() { false } else { unbox::<u8>(r) != 0 })
    }

    /// Whether the given NetPlayer is infected specifically in `GorillaTagManager`
    /// (i.e. in Infection or InfectionCompetitive modes).
    pub unsafe fn is_player_infected(&self, net_player_obj: *mut MonoObject) -> bool {
        if net_player_obj.is_null() { return false; }
        let mgr = self.active_game_manager();
        if mgr.is_null() { return false; }
        // Verify the active manager IS a GorillaTagManager before calling.
        let cls = (self.bridge.api.object_get_class)(mgr);
        if cls != self.cache.gtm_class { return false; }
        let mut params: [*mut c_void; 1] = [net_player_obj as *mut c_void];
        self.bridge
            .invoke(self.cache.gtm_m_is_infected, mgr, &mut params)
            .pipe(|r| if r.is_null() { false } else { unbox::<u8>(r) != 0 })
    }
}

trait Pipe: Sized {
    fn pipe<F: FnOnce(Self) -> R, R>(self, f: F) -> R { f(self) }
}
impl<T> Pipe for T {}
