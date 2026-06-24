/// VRRig — wraps the managed `VRRig` class (visual/networked gorilla
/// representation). Both the local rig and remote rigs are VRRig instances.
use crate::mono::{MonoBridge, types::*};
use super::types::{Vector3, Quaternion};

pub struct VRRigCache {
    class:             *mut MonoClass,
    vtable:            *mut MonoVTable,
    // static private field for the local rig
    f_local_rig:       *mut MonoClassField,
    // instance fields
    f_red:             *mut MonoClassField,
    f_green:           *mut MonoClassField,
    f_blue:            *mut MonoClassField,
    f_is_frozen:       *mut MonoClassField,
    f_is_my_player:    *mut MonoClassField,
    f_is_offline_rig:  *mut MonoClassField,
    f_sync_rotation:   *mut MonoClassField,
    f_creator:         *mut MonoClassField,
    f_name_visible:    *mut MonoClassField,
    f_muted:           *mut MonoClassField,
    // property getters
    pg_is_local:       *mut MonoMethod,
    pg_scale_factor:   *mut MonoMethod,
    pg_sync_pos:       *mut MonoMethod,
}

unsafe impl Send for VRRigCache {}
unsafe impl Sync for VRRigCache {}

impl VRRigCache {
    pub unsafe fn build(bridge: &MonoBridge) -> Result<Self, String> {
        let cls = bridge
            .find_class("", "VRRig")
            .ok_or("VRRig class not found")?;
        let vt = bridge.vtable(cls).ok_or("VRRig vtable not found")?;

        macro_rules! field {
            ($name:literal) => {
                bridge.field(cls, $name).ok_or(concat!("VRRig field not found: ", $name))?
            };
        }
        macro_rules! prop {
            ($name:literal) => {
                bridge.property_getter(cls, $name).ok_or(concat!("VRRig prop not found: ", $name))?
            };
        }

        Ok(VRRigCache {
            class:            cls,
            vtable:           vt,
            f_local_rig:      field!("gLocalRig"),
            f_red:            field!("red"),
            f_green:          field!("green"),
            f_blue:           field!("blue"),
            f_is_frozen:      field!("IsFrozen"),   // backing field for property
            f_is_my_player:   field!("isMyPlayer"),
            f_is_offline_rig: field!("isOfflineVRRig"),
            f_sync_rotation:  field!("syncRotation"),
            f_creator:        field!("creator"),
            f_name_visible:   field!("playerNameVisible"),
            f_muted:          field!("muted"),
            pg_is_local:      prop!("isLocal"),
            pg_scale_factor:  prop!("scaleFactor"),
            pg_sync_pos:      prop!("syncPos"),
        })
    }
}

// ---------------------------------------------------------------------------
// VRRig public handle
// ---------------------------------------------------------------------------

pub struct VRRig<'a> {
    pub obj:    *mut MonoObject,
    cache:      &'a VRRigCache,
    bridge:     &'a MonoBridge,
}

impl<'a> VRRig<'a> {
    // Build a VRRig handle around an arbitrary MonoObject*.
    pub unsafe fn from_obj(obj: *mut MonoObject, cache: &'a VRRigCache, bridge: &'a MonoBridge) -> Option<Self> {
        if obj.is_null() { return None; }
        Some(VRRig { obj, cache, bridge })
    }

    /// The local player's VRRig (the visual rig driven by the HMD).
    pub unsafe fn local_rig(cache: &'a VRRigCache, bridge: &'a MonoBridge) -> Option<Self> {
        let obj = bridge.get_static_field::<*mut MonoObject>(cache.vtable, cache.f_local_rig);
        Self::from_obj(obj, cache, bridge)
    }

    // -------------------------------------------------------------------
    // Colour
    // -------------------------------------------------------------------

    pub unsafe fn color(&self) -> (f32, f32, f32) {
        let r = self.bridge.get_field::<f32>(self.obj, self.cache.f_red);
        let g = self.bridge.get_field::<f32>(self.obj, self.cache.f_green);
        let b = self.bridge.get_field::<f32>(self.obj, self.cache.f_blue);
        (r, g, b)
    }

    /// Set the gorilla's body colour. Values should be 0.0–1.0 floats.
    /// Note: you still need to call GorillaTagger.Instance.UpdateColor() on
    /// the managed side for the change to propagate fully to all materials.
    pub unsafe fn set_color(&self, r: f32, g: f32, b: f32) {
        self.bridge.set_field::<f32>(self.obj, self.cache.f_red,   r);
        self.bridge.set_field::<f32>(self.obj, self.cache.f_green, g);
        self.bridge.set_field::<f32>(self.obj, self.cache.f_blue,  b);
    }

    // -------------------------------------------------------------------
    // Position / rotation
    // -------------------------------------------------------------------

    /// World-space position (from the `syncPos` property, which is the
    /// authoritative network position).
    pub unsafe fn position(&self) -> Vector3 {
        self.bridge
            .invoke0_unbox::<Vector3>(self.cache.pg_sync_pos, self.obj)
            .unwrap_or(Vector3::ZERO)
    }

    /// World-space rotation.
    pub unsafe fn rotation(&self) -> Quaternion {
        self.bridge.get_field::<Quaternion>(self.obj, self.cache.f_sync_rotation)
    }

    // -------------------------------------------------------------------
    // State
    // -------------------------------------------------------------------

    /// True if this is the local player's offline (HMD-driven) rig.
    pub unsafe fn is_offline_rig(&self) -> bool {
        self.bridge.get_field::<u8>(self.obj, self.cache.f_is_offline_rig) != 0
    }

    pub unsafe fn is_my_player(&self) -> bool {
        self.bridge.get_field::<u8>(self.obj, self.cache.f_is_my_player) != 0
    }

    pub unsafe fn is_frozen(&self) -> bool {
        self.bridge.get_field::<u8>(self.obj, self.cache.f_is_frozen) != 0
    }

    pub unsafe fn is_muted(&self) -> bool {
        self.bridge.get_field::<u8>(self.obj, self.cache.f_muted) != 0
    }

    /// Combined scale (ScaleMultiplier × NativeScale).
    pub unsafe fn scale_factor(&self) -> f32 {
        self.bridge
            .invoke0_unbox::<f32>(self.cache.pg_scale_factor, self.obj)
            .unwrap_or(1.0)
    }

    // -------------------------------------------------------------------
    // Name
    // -------------------------------------------------------------------

    /// The sanitised display name for this player (read from
    /// `playerNameVisible`). May differ from the Photon nickname.
    pub unsafe fn player_name(&self) -> String {
        let ms = self.bridge.get_field::<*mut MonoString>(self.obj, self.cache.f_name_visible);
        self.bridge.mono_string_to_rust(ms)
    }

    // -------------------------------------------------------------------
    // NetPlayer cross-reference
    // -------------------------------------------------------------------

    /// Raw pointer to the `NetPlayer` object that created this rig, or null
    /// if the rig hasn't been initialised over the network yet.
    pub unsafe fn creator_obj(&self) -> *mut MonoObject {
        self.bridge.get_obj_field(self.obj, self.cache.f_creator)
    }
}
