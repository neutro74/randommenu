/// GTPlayer — wraps `GorillaLocomotion.GTPlayer` (the physics/locomotion
/// singleton). All methods require that the caller is on the Unity main thread.
use std::ffi::c_void;
use crate::mono::{MonoBridge, types::*};
use super::types::{Vector3, Quaternion};

pub struct GTPlayerCache {
    class:              *mut MonoClass,
    vtable:             *mut MonoVTable,
    // static field
    f_instance:         *mut MonoClassField,
    // instance fields
    f_averaged_vel:     *mut MonoClassField,
    f_current_vel:      *mut MonoClassField,
    f_scale_multiplier: *mut MonoClassField,
    f_native_scale:     *mut MonoClassField,
    f_disable_movement: *mut MonoClassField,
    f_left_hand:        *mut MonoClassField,  // HandState struct
    f_right_hand:       *mut MonoClassField,  // HandState struct
    // methods (4 params = position, rotation, keepVelocity, center)
    m_teleport_to:      *mut MonoMethod,
    m_set_velocity:     *mut MonoMethod,
    m_add_force:        *mut MonoMethod,
    m_apply_knockback:  *mut MonoMethod,
    m_set_scale:        *mut MonoMethod,
    // property getters
    pg_head_center:     *mut MonoMethod,
    pg_rb_velocity:     *mut MonoMethod,
    pg_scale:           *mut MonoMethod,
    pg_in_water:        *mut MonoMethod,
    pg_is_climbing:     *mut MonoMethod,
    pg_hand_contact:    *mut MonoMethod,
}

// HandState is a struct embedded inside GTPlayer. The fields below are
// relative offsets — we read them by getting the raw struct value via the
// field, then manually projecting. However, because HandState is a non-
// serialised struct in the field list, we access the wrapper getters:
//   GTPlayer.GetHandPosition(isLeft, StiltID.None)
//   GTPlayer.IsHandTouching(isLeft)
pub struct HandStateFields {
    // These are on GTPlayer directly (forwarded getters)
    m_get_hand_pos:     *mut MonoMethod,
    m_is_hand_touching: *mut MonoMethod,
}

unsafe impl Send for GTPlayerCache {}
unsafe impl Sync for GTPlayerCache {}

unsafe impl Send for HandStateFields {}
unsafe impl Sync for HandStateFields {}

impl GTPlayerCache {
    pub unsafe fn build(bridge: &MonoBridge) -> Result<(Self, HandStateFields), String> {
        let cls = bridge
            .find_class("GorillaLocomotion", "GTPlayer")
            .ok_or("GTPlayer class not found")?;
        let vt = bridge.vtable(cls).ok_or("GTPlayer vtable not found")?;

        macro_rules! field {
            ($name:literal) => {
                bridge.field(cls, $name).ok_or(concat!("GTPlayer field not found: ", $name))?
            };
        }
        macro_rules! meth {
            ($name:literal, $n:expr) => {
                bridge.method(cls, $name, $n).ok_or(concat!("GTPlayer method not found: ", $name))?
            };
        }
        macro_rules! prop {
            ($name:literal) => {
                bridge.property_getter(cls, $name).ok_or(concat!("GTPlayer prop not found: ", $name))?
            };
        }

        let cache = GTPlayerCache {
            class:              cls,
            vtable:             vt,
            f_instance:         field!("_instance"),
            f_averaged_vel:     field!("averagedVelocity"),
            f_current_vel:      field!("currentVelocity"),
            f_scale_multiplier: field!("scaleMultiplier"),
            f_native_scale:     field!("nativeScale"),
            f_disable_movement: field!("disableMovement"),
            f_left_hand:        field!("leftHand"),
            f_right_hand:       field!("rightHand"),
            m_teleport_to:      meth!("TeleportTo", 4),
            m_set_velocity:     meth!("SetPlayerVelocity", 1),
            m_add_force:        meth!("AddForce", 2),
            m_apply_knockback:  meth!("ApplyKnockback", 3),
            m_set_scale:        meth!("SetScaleMultiplier", 1),
            pg_head_center:     prop!("HeadCenterPosition"),
            pg_rb_velocity:     prop!("RigidbodyVelocity"),
            pg_scale:           prop!("scale"),
            pg_in_water:        prop!("InWater"),
            pg_is_climbing:     prop!("isClimbing"),
            pg_hand_contact:    prop!("HandContactingSurface"),
        };

        let hs = HandStateFields {
            m_get_hand_pos:     meth!("GetHandPosition", 2),
            m_is_hand_touching: meth!("IsHandTouching", 1),
        };

        Ok((cache, hs))
    }
}

// ---------------------------------------------------------------------------
// GTPlayer — public handle
// ---------------------------------------------------------------------------

pub struct GTPlayer<'a> {
    obj:   *mut MonoObject,
    cache: &'a GTPlayerCache,
    hs:    &'a HandStateFields,
    bridge: &'a MonoBridge,
}

impl<'a> GTPlayer<'a> {
    /// Get the current GTPlayer singleton. Returns None if it hasn't been
    /// initialised yet (e.g. called before the scene loads).
    pub unsafe fn instance(
        bridge: &'a MonoBridge,
        cache: &'a GTPlayerCache,
        hs: &'a HandStateFields,
    ) -> Option<Self> {
        let obj = bridge.get_static_field::<*mut MonoObject>(cache.vtable, cache.f_instance);
        if obj.is_null() { return None; }
        Some(GTPlayer { obj, cache, hs, bridge })
    }

    // -------------------------------------------------------------------
    // Position / velocity
    // -------------------------------------------------------------------

    /// Position of the center of the player's head collider.
    pub unsafe fn head_position(&self) -> Vector3 {
        self.bridge
            .invoke0_unbox::<Vector3>(self.cache.pg_head_center, self.obj)
            .unwrap_or(Vector3::ZERO)
    }

    /// Physics velocity of the rigidbody this frame.
    pub unsafe fn rigidbody_velocity(&self) -> Vector3 {
        self.bridge
            .invoke0_unbox::<Vector3>(self.cache.pg_rb_velocity, self.obj)
            .unwrap_or(Vector3::ZERO)
    }

    /// Smoothed velocity averaged over the velocity history window.
    pub unsafe fn averaged_velocity(&self) -> Vector3 {
        self.bridge.get_field::<Vector3>(self.obj, self.cache.f_averaged_vel)
    }

    /// Instantaneous (single-frame) velocity.
    pub unsafe fn instantaneous_velocity(&self) -> Vector3 {
        self.bridge.get_field::<Vector3>(self.obj, self.cache.f_current_vel)
    }

    // -------------------------------------------------------------------
    // Locomotion control
    // -------------------------------------------------------------------

    /// Teleport to a world-space position + rotation.
    /// `keep_velocity` preserves the current physics velocity.
    /// `center` offsets so the camera is placed at `position`.
    pub unsafe fn teleport_to(&self, position: Vector3, rotation: Quaternion, keep_velocity: bool, center: bool) {
        let mut pos = position;
        let mut rot = rotation;
        let mut kv  = keep_velocity as u8;
        let mut ctr = center as u8;
        let mut params: [*mut c_void; 4] = [
            &mut pos  as *mut _ as *mut c_void,
            &mut rot  as *mut _ as *mut c_void,
            &mut kv   as *mut _ as *mut c_void,
            &mut ctr  as *mut _ as *mut c_void,
        ];
        self.bridge.invoke(self.cache.m_teleport_to, self.obj, &mut params);
    }

    /// Directly override the rigidbody velocity (sets all history slots too).
    pub unsafe fn set_velocity(&self, vel: Vector3) {
        let mut v = vel;
        let mut params: [*mut c_void; 1] = [&mut v as *mut _ as *mut c_void];
        self.bridge.invoke(self.cache.m_set_velocity, self.obj, &mut params);
    }

    /// Apply a force. `mode` matches Unity's `ForceMode`:
    ///   0 = Force, 1 = Acceleration, 2 = Impulse, 3 = VelocityChange.
    pub unsafe fn add_force(&self, force: Vector3, mode: i32) {
        let mut f = force;
        let mut m = mode;
        let mut params: [*mut c_void; 2] = [
            &mut f as *mut _ as *mut c_void,
            &mut m as *mut _ as *mut c_void,
        ];
        self.bridge.invoke(self.cache.m_add_force, self.obj, &mut params);
    }

    /// Apply a directional knockback impulse. `force_off_ground` pops the
    /// player off any surface they are touching.
    pub unsafe fn apply_knockback(&self, direction: Vector3, speed: f32, force_off_ground: bool) {
        let mut dir = direction;
        let mut spd = speed;
        let mut fog = force_off_ground as u8;
        let mut params: [*mut c_void; 3] = [
            &mut dir as *mut _ as *mut c_void,
            &mut spd as *mut _ as *mut c_void,
            &mut fog as *mut _ as *mut c_void,
        ];
        self.bridge.invoke(self.cache.m_apply_knockback, self.obj, &mut params);
    }

    // -------------------------------------------------------------------
    // Scale
    // -------------------------------------------------------------------

    /// Combined scale (scaleMultiplier × nativeScale).
    pub unsafe fn scale(&self) -> f32 {
        self.bridge
            .invoke0_unbox::<f32>(self.cache.pg_scale, self.obj)
            .unwrap_or(1.0)
    }

    /// Override the cosmetic scale multiplier (1.0 = default size).
    pub unsafe fn set_scale_multiplier(&self, s: f32) {
        let mut v = s;
        let mut params: [*mut c_void; 1] = [&mut v as *mut _ as *mut c_void];
        self.bridge.invoke(self.cache.m_set_scale, self.obj, &mut params);
    }

    /// Raw scale multiplier field value (not including nativeScale).
    pub unsafe fn scale_multiplier(&self) -> f32 {
        self.bridge.get_field::<f32>(self.obj, self.cache.f_scale_multiplier)
    }

    // -------------------------------------------------------------------
    // Movement toggle
    // -------------------------------------------------------------------

    /// When true the player's physics movement is frozen in place.
    pub unsafe fn disable_movement(&self) -> bool {
        self.bridge.get_field::<u8>(self.obj, self.cache.f_disable_movement) != 0
    }

    pub unsafe fn set_disable_movement(&self, v: bool) {
        self.bridge.set_field::<u8>(self.obj, self.cache.f_disable_movement, v as u8);
    }

    // -------------------------------------------------------------------
    // State queries
    // -------------------------------------------------------------------

    pub unsafe fn in_water(&self) -> bool {
        self.bridge.invoke0_unbox::<u8>(self.cache.pg_in_water, self.obj).unwrap_or(0) != 0
    }

    pub unsafe fn is_climbing(&self) -> bool {
        self.bridge.invoke0_unbox::<u8>(self.cache.pg_is_climbing, self.obj).unwrap_or(0) != 0
    }

    /// True if at least one hand is touching a surface.
    pub unsafe fn hand_contacting_surface(&self) -> bool {
        self.bridge.invoke0_unbox::<u8>(self.cache.pg_hand_contact, self.obj).unwrap_or(0) != 0
    }

    // -------------------------------------------------------------------
    // Hand queries
    // -------------------------------------------------------------------

    /// World position of the given hand (last-frame finalised position).
    /// Uses `GTPlayer.GetHandPosition(isLeft, StiltID.None)`.
    /// StiltID.None = 0 as int (enum).
    pub unsafe fn hand_position(&self, is_left: bool) -> Vector3 {
        let mut left: u8  = is_left as u8;
        let mut stilt: i32 = 0; // StiltID.None
        let mut params: [*mut c_void; 2] = [
            &mut left  as *mut _ as *mut c_void,
            &mut stilt as *mut _ as *mut c_void,
        ];
        self.bridge
            .invoke(self.hs.m_get_hand_pos, self.obj, &mut params)
            .pipe(|r| unbox::<Vector3>(r))
    }

    /// Whether the given hand was touching a surface last frame.
    pub unsafe fn is_hand_touching(&self, is_left: bool) -> bool {
        let mut left: u8 = is_left as u8;
        let mut params: [*mut c_void; 1] = [&mut left as *mut _ as *mut c_void];
        self.bridge
            .invoke(self.hs.m_is_hand_touching, self.obj, &mut params)
            .pipe(|r| unbox::<u8>(r)) != 0
    }
}

trait Pipe: Sized {
    fn pipe<F: FnOnce(Self) -> R, R>(self, f: F) -> R { f(self) }
}
impl<T> Pipe for T {}
