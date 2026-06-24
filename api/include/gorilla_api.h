/**
 * gorilla_api.h — Gorilla Tag native interaction API
 *
 * Include this header in your C/C++ mod.  The implementations live in
 * gorilla_api.dll (compiled from the Rust crate in this repo).
 *
 * All functions must be called from the Unity main thread.
 */

#pragma once
#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>

/* -------------------------------------------------------------------------
 * Result codes
 * ---------------------------------------------------------------------- */

typedef enum {
    GORILLA_OK                  = 0,
    GORILLA_ALREADY_INITIALISED = 1,
    GORILLA_MONO_NOT_FOUND      = 2,
    GORILLA_CLASS_NOT_FOUND     = 3,
    GORILLA_NULL_INSTANCE       = 4,
    GORILLA_INVALID_ARG         = 5,
} GorillaResult;

/* -------------------------------------------------------------------------
 * Game mode enum  (mirrors GorillaGameModes.GameModeType)
 * ---------------------------------------------------------------------- */

typedef enum {
    GAMEMODE_CASUAL               =  0,
    GAMEMODE_INFECTION            =  1,
    GAMEMODE_HUNT_DOWN            =  2,
    GAMEMODE_PAINTBRAWL           =  3,
    GAMEMODE_AMBUSH               =  4,
    GAMEMODE_FREEZE_TAG           =  5,
    GAMEMODE_GHOST                =  6,
    GAMEMODE_CUSTOM               =  7,
    GAMEMODE_GUARDIAN             =  8,
    GAMEMODE_PROP_HUNT            =  9,
    GAMEMODE_INFECTION_COMPETITIVE = 10,
    GAMEMODE_SUPER_INFECT         = 11,
    GAMEMODE_SUPER_CASUAL         = 12,
    GAMEMODE_NONE                 = -1,
} GorillaGameMode;

/* -------------------------------------------------------------------------
 * Unity ForceMode values
 * ---------------------------------------------------------------------- */

typedef enum {
    FORCE_MODE_FORCE          = 0,
    FORCE_MODE_ACCELERATION   = 1,
    FORCE_MODE_IMPULSE        = 2,
    FORCE_MODE_VELOCITY_CHANGE = 3,
} GorillaForceMode;

/* -------------------------------------------------------------------------
 * NetPlayer descriptor (filled by gorilla_network_get_all_players)
 * ---------------------------------------------------------------------- */

typedef struct {
    int32_t  actor_number;
    int32_t  is_local;          /**< 1 if this is the local player */
    int32_t  is_master_client;  /**< 1 if this player is the Photon master */
    char     nick_name[64];     /**< Display name (null-terminated) */
    char     user_id[64];       /**< Photon/PlayFab user ID */
} GorillaNetPlayer;

/* =========================================================================
 * Initialisation
 * ====================================================================== */

/**
 * Initialise the API.  Call once, after the game has loaded Assembly-CSharp
 * (i.e. in your mod's OnApplicationStart / DllMain, NOT before scene load).
 *
 * Returns GORILLA_OK on success.  Safe to call multiple times — subsequent
 * calls return GORILLA_ALREADY_INITIALISED without reinitialising.
 */
GorillaResult gorilla_init(void);

/* =========================================================================
 * Local player physics  (GTPlayer)
 * ====================================================================== */

/** World-space head-centre position of the local player. */
void gorilla_player_get_position(float *x, float *y, float *z);

/** Smoothed, averaged velocity (m/s). */
void gorilla_player_get_velocity(float *x, float *y, float *z);

/** Directly override the rigidbody velocity.  Fills the velocity history. */
void gorilla_player_set_velocity(float x, float y, float z);

/**
 * Teleport to world position (px,py,pz) with quaternion rotation
 * (rx,ry,rz,rw).
 *   keep_velocity : 1 = preserve momentum, 0 = zero it out.
 *   center        : 1 = offset so the camera (HMD) lands at the target,
 *                   0 = root object snaps to target.
 */
void gorilla_player_teleport(
    float px, float py, float pz,
    float rx, float ry, float rz, float rw,
    int keep_velocity,
    int center);

/**
 * Add a physics force.
 *   mode: FORCE_MODE_FORCE / _ACCELERATION / _IMPULSE / _VELOCITY_CHANGE
 */
void gorilla_player_add_force(float x, float y, float z, int mode);

/**
 * Apply a directional knockback impulse.
 *   direction       : unit vector in world space.
 *   speed           : m/s.
 *   force_off_ground: 1 = pop the player off any surface first.
 */
void gorilla_player_apply_knockback(
    float dx, float dy, float dz,
    float speed,
    int force_off_ground);

/** Returns the combined scale (scaleMultiplier × nativeScale).  Default 1.0. */
float gorilla_player_get_scale(void);

/** Set the scale multiplier.  1.0 = default gorilla size. */
void gorilla_player_set_scale(float scale);

/**
 * World position of the given hand (finalised last-frame position).
 *   is_left : 1 = left, 0 = right.
 */
void gorilla_player_get_hand_position(int is_left,
                                      float *x, float *y, float *z);

/** Returns 1 if the given hand was touching a surface last physics frame. */
int gorilla_player_is_hand_touching(int is_left);

/** Freeze (1) or unfreeze (0) all player movement. */
void gorilla_player_set_movement_disabled(int disabled);

/** Returns 1 if the local player is submerged in water. */
int gorilla_player_in_water(void);

/** Returns 1 if the local player is currently climbing. */
int gorilla_player_is_climbing(void);

/* =========================================================================
 * Local visual rig  (VRRig)
 * ====================================================================== */

/** Read the gorilla body colour (0.0–1.0 per channel). */
void gorilla_rig_get_color(float *r, float *g, float *b);

/** Set the gorilla body colour (0.0–1.0 per channel). */
void gorilla_rig_set_color(float r, float g, float b);

/** Returns 1 if the local player is currently frozen (Freeze Tag). */
int gorilla_rig_is_frozen(void);

/** Returns 1 if the local player is muted. */
int gorilla_rig_is_muted(void);

/** Returns the visual rig scale factor. */
float gorilla_rig_get_scale(void);

/**
 * Copy the local player's display name into buf (max buf_len bytes,
 * null-terminated).  Returns the number of bytes written (excluding null).
 */
int gorilla_rig_get_name(char *buf, int buf_len);

/* =========================================================================
 * Networking
 * ====================================================================== */

/** Returns 1 if the local player is in a multiplayer room. */
int gorilla_network_in_room(void);

/** Copy current room name into buf.  Returns bytes written. */
int gorilla_network_room_name(char *buf, int buf_len);

/** Number of players in the room including the local player. */
int gorilla_network_player_count(void);

/** Returns 1 if the local client is the Photon master client. */
int gorilla_network_is_master_client(void);

/** Photon actor number of the local player. */
int gorilla_network_local_player_id(void);

/**
 * Fill out_players[0..max_count] with all connected players.
 * Returns the number of entries written.
 *
 * Example:
 *   GorillaNetPlayer players[16];
 *   int count = gorilla_network_get_all_players(players, 16);
 *   for (int i = 0; i < count; i++) { ... players[i] ... }
 */
int gorilla_network_get_all_players(GorillaNetPlayer *out_players,
                                    int max_count);

/* =========================================================================
 * Game mode
 * ====================================================================== */

/**
 * Returns the current game mode as a GorillaGameMode value.
 * Returns GAMEMODE_NONE (-1) when in the lobby or between rounds.
 */
int gorilla_gamemode_current(void);

/** Returns 1 if mode_type is the currently running game mode. */
int gorilla_gamemode_is_playing(int mode_type);

/**
 * Returns 1 if the local player is currently tagged / infected in the active
 * game mode (via GameMode.LocalIsTagged).
 * Always returns 0 when not in a room.
 */
int gorilla_gamemode_local_is_tagged(void);

/**
 * Returns 1 if the player with the given actor number is infected.
 * Uses GorillaTagManager.IsInfected — only meaningful in Infection /
 * InfectionCompetitive modes.  Returns 0 in other modes or if not in a room.
 */
int gorilla_gamemode_is_actor_infected(int actor_number);

#ifdef __cplusplus
} /* extern "C" */
#endif
