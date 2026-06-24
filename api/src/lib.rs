// gorilla-api — native interaction API for Gorilla Tag (Mono/PC build).
#![allow(dead_code)]
//
// Designed to be compiled as a Windows DLL and injected into the game
// process (e.g. via BepInEx NativeLibrary or manual DLL injection under
// Steam Proton).
//
// All exported functions are `extern "C"` with a C-compatible ABI.  Include
// `../include/gorilla_api.h` in your C/C++ mod code.
//
// Thread safety: every function must be called from the Unity main thread.
// The Mono embedding API is not thread-safe and Unity objects are single-
// threaded by design.

#![allow(clippy::missing_safety_doc)]

mod mono;
mod gorilla;

use std::sync::OnceLock;
use std::ffi::{c_char, c_int, c_void};
use mono::MonoBridge;
use gorilla::{
    player::{GTPlayerCache, HandStateFields},
    rig::VRRigCache,
    network::NetworkCache,
    gamemode::GameModeCache,
    types::{GameModeType, NetPlayerInfo, Quaternion, Vector3},
};

// ---------------------------------------------------------------------------
// Global state — initialised once by gorilla_init().
// ---------------------------------------------------------------------------

struct GlobalState {
    bridge:   MonoBridge,
    player:   GTPlayerCache,
    hand_st:  HandStateFields,
    rig:      VRRigCache,
    net:      NetworkCache,
    gamemode: GameModeCache,
}

// SAFETY: only accessed from the Unity main thread.
unsafe impl Send for GlobalState {}
unsafe impl Sync for GlobalState {}

static STATE: OnceLock<GlobalState> = OnceLock::new();

#[repr(C)]
#[derive(Debug, PartialEq, Eq)]
pub enum GorillaResult {
    Ok                 = 0,
    AlreadyInitialised = 1,
    MonoNotFound       = 2,
    ClassNotFound      = 3,
    NullInstance       = 4,
    InvalidArg         = 5,
}

// ---------------------------------------------------------------------------
// Initialisation
// ---------------------------------------------------------------------------

/// Initialise the API.  Must be called once before any other function, from
/// the Unity main thread, after the game has finished loading its assemblies.
///
/// Returns `GorillaResult::Ok` on success, or an error code if something
/// could not be resolved.
#[no_mangle]
pub unsafe extern "C" fn gorilla_init() -> GorillaResult {
    if STATE.get().is_some() {
        return GorillaResult::AlreadyInitialised;
    }
    let bridge = match MonoBridge::init() {
        Ok(b)  => b,
        Err(_) => return GorillaResult::MonoNotFound,
    };
    let (player, hand_st) = match GTPlayerCache::build(&bridge) {
        Ok(p)  => p,
        Err(_) => return GorillaResult::ClassNotFound,
    };
    let rig = match VRRigCache::build(&bridge) {
        Ok(r)  => r,
        Err(_) => return GorillaResult::ClassNotFound,
    };
    let net = match NetworkCache::build(&bridge) {
        Ok(n)  => n,
        Err(_) => return GorillaResult::ClassNotFound,
    };
    let gamemode = match GameModeCache::build(&bridge) {
        Ok(g)  => g,
        Err(_) => return GorillaResult::ClassNotFound,
    };
    let _ = STATE.set(GlobalState { bridge, player, hand_st, rig, net, gamemode });
    GorillaResult::Ok
}

// Helper macros for boilerplate
macro_rules! state {
    () => {{
        match STATE.get() { Some(s) => s, None => return }
    }};
    (ret $ret:expr) => {{
        match STATE.get() { Some(s) => s, None => return $ret }
    }};
}

// ---------------------------------------------------------------------------
// ── LOCAL PLAYER (GTPlayer) ─────────────────────────────────────────────────
// ---------------------------------------------------------------------------

/// Write the head-centre world position into *x, *y, *z.
#[no_mangle]
pub unsafe extern "C" fn gorilla_player_get_position(x: *mut f32, y: *mut f32, z: *mut f32) {
    let s = state!();
    let Some(p) = gorilla::player::GTPlayer::instance(&s.bridge, &s.player, &s.hand_st) else { return };
    let v = p.head_position();
    if !x.is_null() { *x = v.x; }
    if !y.is_null() { *y = v.y; }
    if !z.is_null() { *z = v.z; }
}

/// Write the averaged (smoothed) velocity into *x, *y, *z.
#[no_mangle]
pub unsafe extern "C" fn gorilla_player_get_velocity(x: *mut f32, y: *mut f32, z: *mut f32) {
    let s = state!();
    let Some(p) = gorilla::player::GTPlayer::instance(&s.bridge, &s.player, &s.hand_st) else { return };
    let v = p.averaged_velocity();
    if !x.is_null() { *x = v.x; }
    if !y.is_null() { *y = v.y; }
    if !z.is_null() { *z = v.z; }
}

/// Directly set the rigidbody velocity.
#[no_mangle]
pub unsafe extern "C" fn gorilla_player_set_velocity(x: f32, y: f32, z: f32) {
    let s = state!();
    let Some(p) = gorilla::player::GTPlayer::instance(&s.bridge, &s.player, &s.hand_st) else { return };
    p.set_velocity(Vector3::new(x, y, z));
}

/// Teleport the player to world position (px,py,pz) with quaternion rotation
/// (rx,ry,rz,rw).  Pass keep_velocity=1 to preserve current momentum.
/// Pass center=1 to place the camera (not the root) at the target position.
#[no_mangle]
pub unsafe extern "C" fn gorilla_player_teleport(
    px: f32, py: f32, pz: f32,
    rx: f32, ry: f32, rz: f32, rw: f32,
    keep_velocity: c_int,
    center: c_int,
) {
    let s = state!();
    let Some(p) = gorilla::player::GTPlayer::instance(&s.bridge, &s.player, &s.hand_st) else { return };
    p.teleport_to(
        Vector3::new(px, py, pz),
        Quaternion::new(rx, ry, rz, rw),
        keep_velocity != 0,
        center != 0,
    );
}

/// Add a force.  mode: 0=Force, 1=Acceleration, 2=Impulse, 3=VelocityChange.
#[no_mangle]
pub unsafe extern "C" fn gorilla_player_add_force(x: f32, y: f32, z: f32, mode: c_int) {
    let s = state!();
    let Some(p) = gorilla::player::GTPlayer::instance(&s.bridge, &s.player, &s.hand_st) else { return };
    p.add_force(Vector3::new(x, y, z), mode);
}

/// Apply a knockback impulse in `(dx,dy,dz)` direction at `speed` m/s.
/// force_off_ground=1 pops the player off the surface first.
#[no_mangle]
pub unsafe extern "C" fn gorilla_player_apply_knockback(
    dx: f32, dy: f32, dz: f32,
    speed: f32,
    force_off_ground: c_int,
) {
    let s = state!();
    let Some(p) = gorilla::player::GTPlayer::instance(&s.bridge, &s.player, &s.hand_st) else { return };
    p.apply_knockback(Vector3::new(dx, dy, dz), speed, force_off_ground != 0);
}

/// Returns the combined scale (scaleMultiplier × nativeScale).
#[no_mangle]
pub unsafe extern "C" fn gorilla_player_get_scale() -> f32 {
    let s = state!(ret 1.0);
    gorilla::player::GTPlayer::instance(&s.bridge, &s.player, &s.hand_st)
        .map(|p| p.scale())
        .unwrap_or(1.0)
}

/// Set the scale multiplier (1.0 = default size).
#[no_mangle]
pub unsafe extern "C" fn gorilla_player_set_scale(scale: f32) {
    let s = state!();
    let Some(p) = gorilla::player::GTPlayer::instance(&s.bridge, &s.player, &s.hand_st) else { return };
    p.set_scale_multiplier(scale);
}

/// Write the last-frame finalised world position of the given hand.
/// is_left: 1 = left hand, 0 = right hand.
#[no_mangle]
pub unsafe extern "C" fn gorilla_player_get_hand_position(
    is_left: c_int,
    x: *mut f32, y: *mut f32, z: *mut f32,
) {
    let s = state!();
    let Some(p) = gorilla::player::GTPlayer::instance(&s.bridge, &s.player, &s.hand_st) else { return };
    let v = p.hand_position(is_left != 0);
    if !x.is_null() { *x = v.x; }
    if !y.is_null() { *y = v.y; }
    if !z.is_null() { *z = v.z; }
}

/// Returns 1 if the given hand was touching a surface last physics frame.
#[no_mangle]
pub unsafe extern "C" fn gorilla_player_is_hand_touching(is_left: c_int) -> c_int {
    let s = state!(ret 0);
    gorilla::player::GTPlayer::instance(&s.bridge, &s.player, &s.hand_st)
        .map(|p| p.is_hand_touching(is_left != 0) as c_int)
        .unwrap_or(0)
}

/// Freeze (1) or un-freeze (0) player movement entirely.
#[no_mangle]
pub unsafe extern "C" fn gorilla_player_set_movement_disabled(disabled: c_int) {
    let s = state!();
    let Some(p) = gorilla::player::GTPlayer::instance(&s.bridge, &s.player, &s.hand_st) else { return };
    p.set_disable_movement(disabled != 0);
}

/// Returns 1 if the local player is currently submerged in water.
#[no_mangle]
pub unsafe extern "C" fn gorilla_player_in_water() -> c_int {
    let s = state!(ret 0);
    gorilla::player::GTPlayer::instance(&s.bridge, &s.player, &s.hand_st)
        .map(|p| p.in_water() as c_int)
        .unwrap_or(0)
}

/// Returns 1 if the local player is currently climbing.
#[no_mangle]
pub unsafe extern "C" fn gorilla_player_is_climbing() -> c_int {
    let s = state!(ret 0);
    gorilla::player::GTPlayer::instance(&s.bridge, &s.player, &s.hand_st)
        .map(|p| p.is_climbing() as c_int)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// ── LOCAL VISUAL RIG (VRRig) ────────────────────────────────────────────────
// ---------------------------------------------------------------------------

/// Write the local rig's body colour into *r, *g, *b (0.0–1.0 range).
#[no_mangle]
pub unsafe extern "C" fn gorilla_rig_get_color(r: *mut f32, g: *mut f32, b: *mut f32) {
    let s = state!();
    let Some(rig) = gorilla::rig::VRRig::local_rig(&s.rig, &s.bridge) else { return };
    let (rv, gv, bv) = rig.color();
    if !r.is_null() { *r = rv; }
    if !g.is_null() { *g = gv; }
    if !b.is_null() { *b = bv; }
}

/// Set the local rig's body colour (0.0–1.0 per channel).
#[no_mangle]
pub unsafe extern "C" fn gorilla_rig_set_color(r: f32, g: f32, b: f32) {
    let s = state!();
    let Some(rig) = gorilla::rig::VRRig::local_rig(&s.rig, &s.bridge) else { return };
    rig.set_color(r, g, b);
}

/// Returns 1 if the local player is currently frozen (Freeze Tag mode).
#[no_mangle]
pub unsafe extern "C" fn gorilla_rig_is_frozen() -> c_int {
    let s = state!(ret 0);
    gorilla::rig::VRRig::local_rig(&s.rig, &s.bridge)
        .map(|r| r.is_frozen() as c_int)
        .unwrap_or(0)
}

/// Returns 1 if the local player is currently muted.
#[no_mangle]
pub unsafe extern "C" fn gorilla_rig_is_muted() -> c_int {
    let s = state!(ret 0);
    gorilla::rig::VRRig::local_rig(&s.rig, &s.bridge)
        .map(|r| r.is_muted() as c_int)
        .unwrap_or(0)
}

/// Returns the combined scale factor of the local rig.
#[no_mangle]
pub unsafe extern "C" fn gorilla_rig_get_scale() -> f32 {
    let s = state!(ret 1.0);
    gorilla::rig::VRRig::local_rig(&s.rig, &s.bridge)
        .map(|r| r.scale_factor())
        .unwrap_or(1.0)
}

/// Copy the local player's display name into `buf` (max `buf_len` bytes,
/// null-terminated).  Returns the number of bytes written (excluding null).
#[no_mangle]
pub unsafe extern "C" fn gorilla_rig_get_name(buf: *mut c_char, buf_len: c_int) -> c_int {
    if buf.is_null() || buf_len <= 0 { return 0; }
    let s = state!(ret 0);
    let Some(rig) = gorilla::rig::VRRig::local_rig(&s.rig, &s.bridge) else { return 0 };
    let name = rig.player_name();
    let bytes = name.as_bytes();
    let copy  = bytes.len().min((buf_len as usize).saturating_sub(1));
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, copy);
    *buf.add(copy) = 0;
    copy as c_int
}

// ---------------------------------------------------------------------------
// ── NETWORK ─────────────────────────────────────────────────────────────────
// ---------------------------------------------------------------------------

/// Returns 1 if the local player is currently in a multiplayer room.
#[no_mangle]
pub unsafe extern "C" fn gorilla_network_in_room() -> c_int {
    let s = state!(ret 0);
    gorilla::network::NetworkSystem::instance(&s.bridge, &s.net)
        .map(|n| n.in_room() as c_int)
        .unwrap_or(0)
}

/// Copy the current room name into `buf`.  Returns bytes written.
#[no_mangle]
pub unsafe extern "C" fn gorilla_network_room_name(buf: *mut c_char, buf_len: c_int) -> c_int {
    if buf.is_null() || buf_len <= 0 { return 0; }
    let s = state!(ret 0);
    let Some(n) = gorilla::network::NetworkSystem::instance(&s.bridge, &s.net) else { return 0 };
    let name = n.room_name();
    let bytes = name.as_bytes();
    let copy  = bytes.len().min((buf_len as usize).saturating_sub(1));
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, copy);
    *buf.add(copy) = 0;
    copy as c_int
}

/// Returns the number of players currently in the room (including local).
#[no_mangle]
pub unsafe extern "C" fn gorilla_network_player_count() -> c_int {
    let s = state!(ret 0);
    gorilla::network::NetworkSystem::instance(&s.bridge, &s.net)
        .map(|n| n.player_count())
        .unwrap_or(0)
}

/// Returns 1 if the local client is the Photon master client.
#[no_mangle]
pub unsafe extern "C" fn gorilla_network_is_master_client() -> c_int {
    let s = state!(ret 0);
    gorilla::network::NetworkSystem::instance(&s.bridge, &s.net)
        .map(|n| n.is_master_client() as c_int)
        .unwrap_or(0)
}

/// Returns the local player's Photon actor number.
#[no_mangle]
pub unsafe extern "C" fn gorilla_network_local_player_id() -> c_int {
    let s = state!(ret -1);
    gorilla::network::NetworkSystem::instance(&s.bridge, &s.net)
        .map(|n| n.local_player_id())
        .unwrap_or(-1)
}

/// Fill `out_players` (array of `GorillaNetPlayer`) with all connected
/// players.  `max_count` is the capacity of the output array.  Returns the
/// number of players written.
///
/// See the C header for the `GorillaNetPlayer` struct layout.
#[no_mangle]
pub unsafe extern "C" fn gorilla_network_get_all_players(
    out_players: *mut NetPlayerInfo,
    max_count: c_int,
) -> c_int {
    if out_players.is_null() || max_count <= 0 { return 0; }
    let s = state!(ret 0);
    let Some(n) = gorilla::network::NetworkSystem::instance(&s.bridge, &s.net) else { return 0 };
    let mut players = Vec::new();
    n.all_players(&mut players);
    let count = players.len().min(max_count as usize);
    for (i, p) in players[..count].iter().enumerate() {
        out_players.add(i).write(p.clone());
    }
    count as c_int
}

// ---------------------------------------------------------------------------
// ── GAME MODE ───────────────────────────────────────────────────────────────
// ---------------------------------------------------------------------------

/// Returns the current game mode as an integer matching `GorillaGameMode` in
/// the C header (-1 = None/lobby, 0 = Casual, 1 = Infection, …).
#[no_mangle]
pub unsafe extern "C" fn gorilla_gamemode_current() -> c_int {
    let s = state!(ret -1);
    let gm = gorilla::gamemode::GameMode::new(&s.gamemode, &s.bridge);
    gm.current_mode_type() as c_int
}

/// Returns 1 if `mode_type` matches the currently running game mode.
#[no_mangle]
pub unsafe extern "C" fn gorilla_gamemode_is_playing(mode_type: c_int) -> c_int {
    let s = state!(ret 0);
    let gm = gorilla::gamemode::GameMode::new(&s.gamemode, &s.bridge);
    gm.is_playing(GameModeType::from_raw(mode_type)) as c_int
}

/// Returns 1 if the local player is currently tagged / infected in the active
/// mode.  Uses `GameMode.LocalIsTagged(localNetPlayer)`.
/// Requires gorilla_network_in_room() == 1.
#[no_mangle]
pub unsafe extern "C" fn gorilla_gamemode_local_is_tagged() -> c_int {
    let s = state!(ret 0);
    let Some(net) = gorilla::network::NetworkSystem::instance(&s.bridge, &s.net) else { return 0 };
    if !net.in_room() { return 0; }
    let local_np = net.local_player_obj();
    if local_np.is_null() { return 0; }
    let gm = gorilla::gamemode::GameMode::new(&s.gamemode, &s.bridge);
    gm.is_player_tagged(local_np) as c_int
}

/// Returns 1 if the given actor number's player is infected (Infection modes
/// only — uses GorillaTagManager.IsInfected).
#[no_mangle]
pub unsafe extern "C" fn gorilla_gamemode_is_actor_infected(actor_number: c_int) -> c_int {
    let s = state!(ret 0);
    let Some(net) = gorilla::network::NetworkSystem::instance(&s.bridge, &s.net) else { return 0 };
    if !net.in_room() { return 0; }
    let np_obj = net.find_player_obj_by_actor(actor_number);
    if np_obj.is_null() { return 0; }
    let gm = gorilla::gamemode::GameMode::new(&s.gamemode, &s.bridge);
    gm.is_player_infected(np_obj) as c_int
}

// ---------------------------------------------------------------------------
// DLL entry point (Windows)
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn DllMain(
    _hmodule: *mut c_void,
    _fdw_reason: u32,
    _lpv_reserved: *mut c_void,
) -> i32 {
    // DLL_PROCESS_ATTACH = 1
    // Nothing to do here — gorilla_init() must be called explicitly after
    // the game has loaded its assemblies.
    1
}
