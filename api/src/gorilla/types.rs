/// Unity's Vector3 — mirrors the managed struct layout exactly.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };
    pub const UP: Self   = Self { x: 0.0, y: 1.0, z: 0.0 };
    pub const FWD: Self  = Self { x: 0.0, y: 0.0, z: 1.0 };

    pub fn new(x: f32, y: f32, z: f32) -> Self { Self { x, y, z } }

    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

/// Unity's Quaternion — mirrors the managed struct layout exactly.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Quaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quaternion {
    pub const IDENTITY: Self = Self { x: 0.0, y: 0.0, z: 0.0, w: 1.0 };
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self { Self { x, y, z, w } }
}

impl Default for Quaternion {
    fn default() -> Self { Self::IDENTITY }
}

/// Matches `GorillaGameModes.GameModeType` enum values.
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum GameModeType {
    Casual              = 0,
    Infection           = 1,
    HuntDown            = 2,
    Paintbrawl          = 3,
    Ambush              = 4,
    FreezeTag           = 5,
    Ghost               = 6,
    Custom              = 7,
    Guardian            = 8,
    PropHunt            = 9,
    InfectionCompetitive = 10,
    SuperInfect         = 11,
    SuperCasual         = 12,
    None                = -1,
}

impl GameModeType {
    pub fn from_raw(v: i32) -> Self {
        match v {
            0  => Self::Casual,
            1  => Self::Infection,
            2  => Self::HuntDown,
            3  => Self::Paintbrawl,
            4  => Self::Ambush,
            5  => Self::FreezeTag,
            6  => Self::Ghost,
            7  => Self::Custom,
            8  => Self::Guardian,
            9  => Self::PropHunt,
            10 => Self::InfectionCompetitive,
            11 => Self::SuperInfect,
            12 => Self::SuperCasual,
            _  => Self::None,
        }
    }
}

/// Info about a remote (or local) player, filled by NetworkSystem queries.
#[repr(C)]
#[derive(Clone, Debug)]
pub struct NetPlayerInfo {
    pub actor_number: i32,
    pub is_local: bool,
    pub is_master_client: bool,
    pub nick_name: [u8; 64],
    pub user_id: [u8; 64],
}

impl Default for NetPlayerInfo {
    fn default() -> Self {
        Self {
            actor_number: -1,
            is_local: false,
            is_master_client: false,
            nick_name: [0u8; 64],
            user_id: [0u8; 64],
        }
    }
}

impl NetPlayerInfo {
    pub fn nick_name_str(&self) -> &str {
        let end = self.nick_name.iter().position(|&b| b == 0).unwrap_or(64);
        std::str::from_utf8(&self.nick_name[..end]).unwrap_or("")
    }
    pub fn user_id_str(&self) -> &str {
        let end = self.user_id.iter().position(|&b| b == 0).unwrap_or(64);
        std::str::from_utf8(&self.user_id[..end]).unwrap_or("")
    }
}

fn copy_str_into(src: &str, dst: &mut [u8; 64]) {
    dst.fill(0);
    let bytes = src.as_bytes();
    let len = bytes.len().min(63);
    dst[..len].copy_from_slice(&bytes[..len]);
}

impl NetPlayerInfo {
    pub fn set_nick_name(&mut self, s: &str) { copy_str_into(s, &mut self.nick_name); }
    pub fn set_user_id(&mut self, s: &str)   { copy_str_into(s, &mut self.user_id); }
}
