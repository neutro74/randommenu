/// NetworkSystem + NetPlayer — wraps the managed networking layer
/// (`NetworkSystem` singleton and `NetPlayer` abstract class).
use crate::mono::{MonoBridge, types::*};
use super::types::NetPlayerInfo;

// ---------------------------------------------------------------------------
// Cached lookup tables for NetworkSystem
// ---------------------------------------------------------------------------

pub struct NetworkCache {
    ns_class:        *mut MonoClass,
    ns_vtable:       *mut MonoVTable,
    f_ns_instance:   *mut MonoClassField,

    // Abstract property getters on NetworkSystem
    pg_in_room:      *mut MonoMethod,
    pg_room_name:    *mut MonoMethod,
    pg_player_count: *mut MonoMethod,
    pg_is_master:    *mut MonoMethod,
    pg_local_player: *mut MonoMethod,
    pg_all_players:  *mut MonoMethod,
    pg_local_id:     *mut MonoMethod,

    // NetPlayer property getters
    np_class:            *mut MonoClass,
    npg_actor_number:    *mut MonoMethod,
    npg_user_id:         *mut MonoMethod,
    npg_nick_name:       *mut MonoMethod,
    npg_is_local:        *mut MonoMethod,
    npg_is_master:       *mut MonoMethod,
}

unsafe impl Send for NetworkCache {}
unsafe impl Sync for NetworkCache {}

impl NetworkCache {
    pub unsafe fn build(bridge: &MonoBridge) -> Result<Self, String> {
        // NetworkSystem lives in the GorillaNetworking namespace.
        let ns_cls = bridge
            .find_class("GorillaNetworking", "NetworkSystem")
            .ok_or("NetworkSystem class not found")?;
        let ns_vt = bridge.vtable(ns_cls).ok_or("NetworkSystem vtable not found")?;

        macro_rules! ns_field {
            ($name:literal) => {
                bridge.field(ns_cls, $name).ok_or(concat!("NetworkSystem field not found: ", $name))?
            };
        }
        macro_rules! ns_prop {
            ($name:literal) => {
                bridge.property_getter(ns_cls, $name).ok_or(concat!("NetworkSystem prop not found: ", $name))?
            };
        }

        // NetPlayer is in the root namespace.
        let np_cls = bridge
            .find_class("", "NetPlayer")
            .ok_or("NetPlayer class not found")?;

        macro_rules! np_prop {
            ($name:literal) => {
                bridge.property_getter(np_cls, $name).ok_or(concat!("NetPlayer prop not found: ", $name))?
            };
        }

        Ok(NetworkCache {
            ns_class:       ns_cls,
            ns_vtable:      ns_vt,
            f_ns_instance:  ns_field!("Instance"),
            pg_in_room:     ns_prop!("InRoom"),
            pg_room_name:   ns_prop!("RoomName"),
            pg_player_count:ns_prop!("RoomPlayerCount"),
            pg_is_master:   ns_prop!("IsMasterClient"),
            pg_local_player:ns_prop!("LocalPlayer"),
            pg_all_players: ns_prop!("AllNetPlayers"),
            pg_local_id:    ns_prop!("LocalPlayerID"),

            np_class:         np_cls,
            npg_actor_number: np_prop!("ActorNumber"),
            npg_user_id:      np_prop!("UserId"),
            npg_nick_name:    np_prop!("NickName"),
            npg_is_local:     np_prop!("IsLocal"),
            npg_is_master:    np_prop!("IsMasterClient"),
        })
    }
}

// ---------------------------------------------------------------------------
// NetworkSystem public handle
// ---------------------------------------------------------------------------

pub struct NetworkSystem<'a> {
    obj:    *mut MonoObject,
    cache:  &'a NetworkCache,
    bridge: &'a MonoBridge,
}

impl<'a> NetworkSystem<'a> {
    pub unsafe fn instance(bridge: &'a MonoBridge, cache: &'a NetworkCache) -> Option<Self> {
        let obj = bridge.get_static_field::<*mut MonoObject>(cache.ns_vtable, cache.f_ns_instance);
        if obj.is_null() { return None; }
        Some(NetworkSystem { obj, cache, bridge })
    }

    // -------------------------------------------------------------------
    // Basic room queries
    // -------------------------------------------------------------------

    pub unsafe fn in_room(&self) -> bool {
        self.bridge.invoke0_unbox::<u8>(self.cache.pg_in_room, self.obj).unwrap_or(0) != 0
    }

    pub unsafe fn room_name(&self) -> String {
        let ms = self.bridge.invoke0(self.cache.pg_room_name, self.obj) as *mut MonoString;
        self.bridge.mono_string_to_rust(ms)
    }

    pub unsafe fn player_count(&self) -> i32 {
        self.bridge.invoke0_unbox::<i32>(self.cache.pg_player_count, self.obj).unwrap_or(0)
    }

    pub unsafe fn is_master_client(&self) -> bool {
        self.bridge.invoke0_unbox::<u8>(self.cache.pg_is_master, self.obj).unwrap_or(0) != 0
    }

    pub unsafe fn local_player_id(&self) -> i32 {
        self.bridge.invoke0_unbox::<i32>(self.cache.pg_local_id, self.obj).unwrap_or(0)
    }

    // -------------------------------------------------------------------
    // Player enumeration
    // -------------------------------------------------------------------

    /// Fill `out` with info for every connected player. Returns the count
    /// written. Capped by `out.len()`.
    pub unsafe fn all_players(&self, out: &mut Vec<NetPlayerInfo>) {
        out.clear();
        // AllNetPlayers returns NetPlayer[] — a MonoArray of object refs.
        let arr = self.bridge.invoke0(self.cache.pg_all_players, self.obj) as *mut MonoArray;
        if arr.is_null() { return; }
        let len = self.bridge.array_len(arr);
        for i in 0..len {
            let np_obj: *mut MonoObject = self.bridge.array_get(arr, i);
            if np_obj.is_null() { continue; }
            if let Some(info) = self.read_net_player(np_obj) {
                out.push(info);
            }
        }
    }

    /// Fill info for the local player specifically.
    pub unsafe fn local_player_info(&self) -> Option<NetPlayerInfo> {
        let np_obj = self.bridge.invoke0(self.cache.pg_local_player, self.obj);
        if np_obj.is_null() { return None; }
        self.read_net_player(np_obj)
    }

    /// Raw MonoObject* for the local NetPlayer.
    pub unsafe fn local_player_obj(&self) -> *mut MonoObject {
        self.bridge.invoke0(self.cache.pg_local_player, self.obj)
    }

    /// Find the raw NetPlayer MonoObject* for the given actor number by
    /// iterating the full player list.  Returns null if not found.
    pub unsafe fn find_player_obj_by_actor(&self, actor_number: i32) -> *mut MonoObject {
        let arr = self.bridge.invoke0(self.cache.pg_all_players, self.obj) as *mut MonoArray;
        if arr.is_null() { return std::ptr::null_mut(); }
        let len = self.bridge.array_len(arr);
        for i in 0..len {
            let np_obj: *mut MonoObject = self.bridge.array_get(arr, i);
            if np_obj.is_null() { continue; }
            if let Some(num) = self.bridge.invoke0_unbox::<i32>(self.cache.npg_actor_number, np_obj) {
                if num == actor_number { return np_obj; }
            }
        }
        std::ptr::null_mut()
    }

    unsafe fn read_net_player(&self, np_obj: *mut MonoObject) -> Option<NetPlayerInfo> {
        let actor = self.bridge
            .invoke0_unbox::<i32>(self.cache.npg_actor_number, np_obj)
            .unwrap_or(-1);
        let is_local = self.bridge
            .invoke0_unbox::<u8>(self.cache.npg_is_local, np_obj)
            .unwrap_or(0) != 0;
        let is_master = self.bridge
            .invoke0_unbox::<u8>(self.cache.npg_is_master, np_obj)
            .unwrap_or(0) != 0;

        let uid_ms = self.bridge.invoke0(self.cache.npg_user_id, np_obj) as *mut MonoString;
        let nick_ms = self.bridge.invoke0(self.cache.npg_nick_name, np_obj) as *mut MonoString;
        let uid  = self.bridge.mono_string_to_rust(uid_ms);
        let nick = self.bridge.mono_string_to_rust(nick_ms);

        let mut info = NetPlayerInfo {
            actor_number: actor,
            is_local,
            is_master_client: is_master,
            ..Default::default()
        };
        info.set_nick_name(&nick);
        info.set_user_id(&uid);
        Some(info)
    }
}
