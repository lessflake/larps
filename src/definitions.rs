//! Various static LoA internal structures and definitions
//! as appearing in packet data.

pub use crate::generated::opcode::Opcode;

#[derive(Default, serde::Serialize)]
pub struct SkillOptionData {
    pub layer_index: Option<u8>,
    pub start_stage_index: Option<u8>,
    pub transit_index: Option<u32>,
    pub stage_start_time: Option<u32>,
    pub farmost_dist: Option<u32>,
    pub tripod_index: Option<TripodIndex>,
    pub tripod_level: Option<TripodLevel>,
}

#[derive(Default, serde::Serialize)]
pub struct SkillMoveOptionData {
    pub move_time: Option<u32>,
    pub stand_up_time: Option<u32>,
    pub down_time: Option<u32>,
    pub freeze_time: Option<u32>,
    pub move_height: Option<u32>,
    pub farmost_dist: Option<u32>,
}

#[derive(Default, serde::Serialize)]
pub struct MoveOptionData {
    pub modifier: Option<u8>,
    pub speed: Option<u32>,
    pub next_pos: Option<u64>,
}

#[derive(serde::Serialize)]
pub struct TripodIndex {
    pub first: u8,
    pub second: u8,
    pub third: u8,
}

#[derive(serde::Serialize)]
pub struct TripodLevel {
    pub first: u16,
    pub second: u16,
    pub third: u16,
}

// TODO: generate these definitions

#[derive(Debug, Copy, Clone, serde::Serialize)]
pub enum Class {
    Warrior,   // yellow
    Berserker, // ecd935
    Destroyer, // b15d42
    Gunlancer, // 8a0c0c now d15217
    Paladin,   // f8f387

    FemaleWarrior,
    Slayer,

    Mage,      // blue
    Arcanist,  // 318eff
    Summoner,  // 31dcff
    Bard,      // 8ac6f6
    Sorceress, // 47a9f9

    MartialArtist, // green
    Wardancer,     // 20e01f
    Scrapper,      // 6fdb1d
    Soulfist,      // 1fe073
    Glaivier,      // 4ed514

    MaleMartialArtist,
    Striker, // 0c8f26

    Assassin,     // purple
    Deathblade,   // ac51f4
    Shadowhunter, // 7b34df
    Reaper,       // a131e1
    Souleater,

    Gunner,       // red
    Sharpshooter, // ef3131
    Deadeye,      // bb0b0b
    Artillerist,  // e2a450
    Scouter,      // d64921

    FemaleGunner,
    Gunslinger, // e37718

    Specialist, // pink
    Artist,     // ea5ad9
    Aeromancer, // e16f9f

    Unknown, // ffffff
}

impl Class {
    pub fn from_id(id: u16) -> Self {
        match id {
            101 => Self::Warrior,
            201 => Self::Mage,
            301 => Self::MartialArtist,
            401 => Self::Assassin,
            501 => Self::Gunner,
            601 => Self::Specialist,
            102 => Self::Berserker,
            103 => Self::Destroyer,
            104 => Self::Gunlancer,
            105 => Self::Paladin,
            202 => Self::Arcanist,
            203 => Self::Summoner,
            204 => Self::Bard,
            205 => Self::Sorceress,
            302 => Self::Wardancer,
            303 => Self::Scrapper,
            304 => Self::Soulfist,
            305 => Self::Glaivier,
            402 => Self::Deathblade,
            403 => Self::Shadowhunter,
            404 => Self::Reaper,
            405 => Self::Souleater,
            502 => Self::Sharpshooter,
            503 => Self::Deadeye,
            504 => Self::Artillerist,
            505 => Self::Scouter,
            511 => Self::FemaleGunner,
            512 => Self::Gunslinger,
            311 => Self::MaleMartialArtist,
            312 => Self::Striker,
            602 => Self::Artist,
            603 => Self::Aeromancer,
            111 => Self::FemaleWarrior,
            112 => Self::Slayer,
            _ => Self::Unknown,
        }
    }

    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            Self::Warrior => (0xff, 0xff, 0x00),
            Self::Berserker => (0xfe, 0xea, 0x30),
            Self::Destroyer => (0xb1, 0x5d, 0x42),
            Self::Gunlancer => (0xc9, 0x22, 0x57),
            Self::Paladin => (0xf8, 0xf3, 0x87),

            Self::FemaleWarrior => (0xff, 0xff, 0x00),
            Self::Slayer => (0x99, 0x00, 0x14),

            Self::Mage => (0x00, 0x00, 0xff),
            Self::Arcanist => (0x31, 0x8e, 0xff),
            Self::Summoner => (0x31, 0xdc, 0xff),
            Self::Bard => (0x8a, 0xc6, 0xf6),
            Self::Sorceress => (0x47, 0xa9, 0xf9),

            Self::MartialArtist => (0x00, 0xff, 0x7f),
            Self::Wardancer => (0x20, 0xe0, 0x1f),
            Self::Scrapper => (0x92, 0xdb, 0x1d),
            Self::Soulfist => (0x1f, 0xe0, 0x73),
            Self::Glaivier => (0x4e, 0xd5, 0x14),

            Self::MaleMartialArtist => (0x7f, 0xff, 0x00),
            Self::Striker => (0x0c, 0x8f, 0x26),

            Self::Assassin => (0x7f, 0x00, 0xff),
            Self::Deathblade => (0xac, 0x51, 0xf4),
            Self::Shadowhunter => (0x7b, 0x34, 0xdf),
            Self::Reaper => (0xa1, 0x31, 0xe1),
            Self::Souleater => (0xa8, 0x7f, 0xfa),

            Self::Gunner => (0xff, 0x00, 0x00),
            Self::Sharpshooter => (0xef, 0x31, 0x31),
            Self::Deadeye => (0xbb, 0x0b, 0x0b),
            Self::Artillerist => (0xe2, 0xa4, 0x50),
            Self::Scouter => (0xd6, 0x49, 0x21),

            Self::FemaleGunner => (0xff, 0x00, 0x7f),
            Self::Gunslinger => (0xe3, 0x77, 0x18),

            Self::Specialist => (0xff, 0x00, 0xff),
            Self::Artist => (0xea, 0x5a, 0xd9),
            Self::Aeromancer => (0xe1, 0x6f, 0x9f),

            Self::Unknown => (0xff, 0xff, 0xff),
        }
    }

    pub fn icon_index(&self) -> Option<usize> {
        Some(match self {
            Class::Warrior => 0,
            Class::Berserker => 1,
            Class::Destroyer => 2,
            Class::Gunlancer => 3,
            Class::Paladin => 4,

            Class::FemaleWarrior => 5,
            Class::Slayer => 6,

            Class::Mage => 7,
            Class::Arcanist => 8,
            Class::Summoner => 9,
            Class::Bard => 10,
            Class::Sorceress => 11,

            Class::MartialArtist => 12,
            Class::Wardancer => 13,
            Class::Scrapper => 14,
            Class::Soulfist => 15,
            Class::Glaivier => 16,

            Class::MaleMartialArtist => 17,
            Class::Striker => 18,

            Class::Assassin => 19,
            Class::Deathblade => 20,
            Class::Shadowhunter => 21,
            Class::Reaper => 22,
            Class::Souleater => 23,

            Class::Gunner => 24,
            Class::Sharpshooter => 25,
            Class::Deadeye => 26,
            Class::Artillerist => 27,
            Class::Scouter => 28,

            Class::FemaleGunner => 29,
            Class::Gunslinger => 30,

            Class::Specialist => 31,
            Class::Artist => 32,
            Class::Aeromancer => 33,

            _ => return None,
        })
    }

    pub fn name(&self) -> &str {
        match self {
            Class::Warrior => "Warrior",
            Class::Mage => "Mage",
            Class::MartialArtist => "Martial Artist",
            Class::Assassin => "Assassin",
            Class::Gunner => "Gunner",
            Class::Specialist => "Specialist",
            Class::Berserker => "Berserker",
            Class::Destroyer => "Destroyer",
            Class::Gunlancer => "Gunlancer",
            Class::Paladin => "Paladin",
            Class::Arcanist => "Arcanist",
            Class::Summoner => "Summoner",
            Class::Bard => "Bard",
            Class::Sorceress => "Sorceress",
            Class::Wardancer => "Wardancer",
            Class::Scrapper => "Scrapper",
            Class::Soulfist => "Soulfist",
            Class::Glaivier => "Glaivier",
            Class::Deathblade => "Deathblade",
            Class::Shadowhunter => "Shadowhunter",
            Class::Reaper => "Reaper",
            Class::Souleater => "Souleater",
            Class::Sharpshooter => "Sharpshooter",
            Class::Deadeye => "Deadeye",
            Class::Artillerist => "Artillerist",
            Class::Scouter => "Scouter",
            Class::FemaleGunner => "Female Gunner",
            Class::Gunslinger => "Gunslinger",
            Class::MaleMartialArtist => "Male Martial Artist",
            Class::Striker => "Striker",
            Class::Artist => "Artist",
            Class::Aeromancer => "Aeromancer",
            Class::FemaleWarrior => "Female Warrior",
            Class::Slayer => "Slayer",
            Class::Unknown => "UNKNOWN",
        }
    }
}

impl std::fmt::Display for Class {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HitFlag {
    Normal,
    Critical,
    Miss,
    Invincible,
    Dot,
    Immmune,
    ImmuneSilenced,
    FontSilenced,
    DotCritical,
    Dodge,
    Reflect,
    DamageShare,
    DodgeHit,
}

impl HitFlag {
    pub fn from_raw(raw: u8) -> Option<Self> {
        Some(match raw {
            0 => Self::Normal,
            1 => Self::Critical,
            2 => Self::Miss,
            3 => Self::Invincible,
            4 => Self::Dot,
            5 => Self::Immmune,
            6 => Self::ImmuneSilenced,
            7 => Self::FontSilenced,
            8 => Self::DotCritical,
            9 => Self::Dodge,
            10 => Self::Reflect,
            11 => Self::DamageShare,
            12 => Self::DodgeHit,
            // 13 => unreachable!(), // "max"
            _ => return None,
        })
    }

    pub fn is_dot(&self) -> bool {
        matches!(self, Self::Dot | Self::DotCritical)
    }

    pub fn is_crit(&self) -> bool {
        matches!(self, Self::Critical | Self::DotCritical)
    }
}

#[derive(Debug)]
pub enum HitOption {
    None,
    BackAttack,
    FrontalAttack,
    FlankAttack,
}

impl HitOption {
    pub fn from_raw(raw: u8) -> Option<Self> {
        Some(match raw {
            0 => Self::None,
            1 => Self::BackAttack,
            2 => Self::FrontalAttack,
            3 => Self::FlankAttack,
            _ => return None,
        })
    }
}

pub mod trigger_signal {
    pub const NONE: u32 = 1;
    pub const OUT: u32 = 2;
    pub const CLICK_CLICK: u32 = 3;
    pub const DOOR_OPEN: u32 = 4;
    pub const DOOR_CLOSE: u32 = 5;
    pub const SWITCH_ON: u32 = 6;
    pub const SWITCH_OFF: u32 = 7;
    pub const HIT_HIT: u32 = 8;
    pub const HIT_DESTRUCT: u32 = 9;
    pub const GRIP_GRIP: u32 = 10;
    pub const VOLUME_ENTER: u32 = 11;
    pub const VOLUME_LEAVE: u32 = 12;
    pub const VOLUME_ON: u32 = 13;
    pub const VOLUME_OFF: u32 = 14;
    pub const NPC_SPAWN: u32 = 15;
    pub const NPC_DEAD: u32 = 16;
    pub const NPC_EVENT_1: u32 = 17;
    pub const NPC_EVENT_2: u32 = 18;
    pub const NPC_EVENT_3: u32 = 19;
    pub const NPC_EVENT_4: u32 = 20;
    pub const NPC_EVENT_5: u32 = 21;
    pub const YES: u32 = 22;
    pub const NO: u32 = 23;
    pub const PROP_PICKUP: u32 = 24;
    pub const PROP_ROTATE_START: u32 = 25;
    pub const PROP_ROTATE_CANCEL: u32 = 26;
    pub const PROP_ROTATE_END: u32 = 27;
    pub const ASSEMBLED: u32 = 28;
    pub const USER_SIGNAL: u32 = 29;
    pub const VOLUME_INPUTKEY: u32 = 30;
    pub const SHARED_CLICK: u32 = 31;
    pub const SHARED_DESPAWN: u32 = 32;
    pub const DUNGEON_CLEARED: u32 = 33;
    pub const COOP_QUEST_START: u32 = 34;
    pub const COOP_QUEST_COMPLETE: u32 = 35;
    pub const COOP_QUEST_FAIL: u32 = 36;
    pub const USER_SHIP_WRECK: u32 = 37;
    pub const TOWER_HIT: u32 = 38;
    pub const TOWER_DESTRUCT: u32 = 39;
    pub const VEHICLE_ENTER: u32 = 40;
    pub const VEHICLE_LEAVE: u32 = 41;
    pub const INSTANCEZONE_LOAD_COMPLETE: u32 = 42;
    pub const RANDOM_CASE_1: u32 = 43;
    pub const RANDOM_CASE_2: u32 = 44;
    pub const RANDOM_CASE_3: u32 = 45;
    pub const RANDOM_CASE_4: u32 = 46;
    pub const RANDOM_CASE_5: u32 = 47;
    pub const NPC_PATHEVENT: u32 = 48;
    pub const JOINT_ATTACH: u32 = 49;
    pub const JOINT_DETACH: u32 = 50;
    pub const HIT_ON2: u32 = 51;
    pub const HIT_ON3: u32 = 52;
    pub const COOP_QUEST_CANCEL: u32 = 53;
    pub const DUNGEON_ENTER: u32 = 54;
    pub const STATION_DISABLE: u32 = 55;
    pub const ALL_DEAD: u32 = 56;
    pub const ALL_EXIT: u32 = 57;
    pub const DUNGEON_PHASE1_CLEAR: u32 = 58;
    pub const DUNGEON_PHASE1_FAIL: u32 = 59;
    pub const DUNGEON_PHASE2_CLEAR: u32 = 60;
    pub const DUNGEON_PHASE2_FAIL: u32 = 61;
    pub const DUNGEON_PHASE3_CLEAR: u32 = 62;
    pub const DUNGEON_PHASE3_FAIL: u32 = 63;
    pub const DUNGEON_PHASE4_CLEAR: u32 = 64;
    pub const DUNGEON_PHASE4_FAIL: u32 = 65;
    pub const USER_STATUS_EFFECT: u32 = 66;
    pub const INSTANCE_TIMER_START: u32 = 67;
    pub const INSTANCE_TIMER_END: u32 = 68;
    pub const INSTANCE_TIMER_CANCEL: u32 = 69;
    pub const INSTANCE_TIMER_EVENT_1: u32 = 70;
    pub const INSTANCE_TIMER_EVENT_2: u32 = 71;
    pub const INSTANCE_TIMER_EVENT_3: u32 = 72;
    pub const INSTANCE_TIMER_EVENT_4: u32 = 73;
    pub const INSTANCE_TIMER_EVENT_5: u32 = 74;
    pub const DUNGEON_PHASE5_CLEAR: u32 = 75;
    pub const DUNGEON_PHASE5_FAIL: u32 = 76;
    pub const DUNGEON_PHASE6_CLEAR: u32 = 77;
    pub const DUNGEON_PHASE6_FAIL: u32 = 78;
    pub const DUNGEON_PHASE1_RESUME: u32 = 79;
    pub const DUNGEON_PHASE2_RESUME: u32 = 80;
    pub const DUNGEON_PHASE3_RESUME: u32 = 81;
    pub const DUNGEON_PHASE4_RESUME: u32 = 82;
    pub const DUNGEON_PHASE5_RESUME: u32 = 83;
    pub const DUNGEON_PHASE6_RESUME: u32 = 84;
    pub const TRACK_MOVE_START_1: u32 = 85;
    pub const TRACK_MOVE_START_2: u32 = 86;
    pub const TRACK_MOVE_START_3: u32 = 87;
    pub const TRACK_MOVE_FAILURE: u32 = 88;
    pub const HIT_RESTORE_START: u32 = 89;
    pub const HIT_RESTORE_FINISH: u32 = 90;
    pub const ZONE_LEVEL_NORMAL: u32 = 91;
    pub const ZONE_LEVEL_HARD: u32 = 92;
    pub const ZONE_LEVEL_HELLCHAOS: u32 = 93;
    pub const ZONE_LEVEL_CHALLENGE: u32 = 94;
    pub const ZONE_LEVEL_SPECIAL: u32 = 95;
    pub const OCCUPATION_NONE_TO_RED: u32 = 96;
    pub const OCCUPATION_NONE_TO_BLUE: u32 = 97;
    pub const OCCUPATION_BLUE_TO_RED: u32 = 98;
    pub const OCCUPATION_RED_TO_BLUE: u32 = 99;
    pub const INSTANCE_TIMER_EVENT_6: u32 = 100;
    pub const INSTANCE_TIMER_EVENT_7: u32 = 101;
    pub const INSTANCE_TIMER_EVENT_8: u32 = 102;
    pub const INSTANCE_TIMER_EVENT_9: u32 = 103;
    pub const INSTANCE_TIMER_EVENT_10: u32 = 104;
    pub const USER_STATUS_EFFECT_REMOVED: u32 = 105;
}

#[derive(Debug)]
pub enum Trigger {
    None,
    Out,
    ClickClick,
    DoorOpen,
    DoorClose,
    SwitchOn,
    SwitchOff,
    HitHit,
    HitDestruct,
    GripGrip,
    VolumeEnter,
    VolumeLeave,
    VolumeOn,
    VolumeOff,
    NpcSpawn,
    NpcDead,
    NpcEvent1,
    NpcEvent2,
    NpcEvent3,
    NpcEvent4,
    NpcEvent5,
    Yes,
    No,
    PropPickup,
    PropRotateStart,
    PropRotateCancel,
    PropRotateEnd,
    Assembled,
    UserSignal,
    VolumeInputkey,
    SharedClick,
    SharedDespawn,
    DungeonCleared,
    CoopQuestStart,
    CoopQuestComplete,
    CoopQuestFail,
    UserShipWreck,
    TowerHit,
    TowerDestruct,
    VehicleEnter,
    VehicleLeave,
    InstancezoneLoadComplete,
    RandomCase1,
    RandomCase2,
    RandomCase3,
    RandomCase4,
    RandomCase5,
    NpcPathevent,
    JointAttach,
    JointDetach,
    HitOn2,
    HitOn3,
    CoopQuestCancel,
    DungeonEnter,
    StationDisable,
    AllDead,
    AllExit,
    DungeonPhase1Clear,
    DungeonPhase1Fail,
    DungeonPhase2Clear,
    DungeonPhase2Fail,
    DungeonPhase3Clear,
    DungeonPhase3Fail,
    DungeonPhase4Clear,
    DungeonPhase4Fail,
    UserStatusEffect,
    InstanceTimerStart,
    InstanceTimerEnd,
    InstanceTimerCancel,
    InstanceTimerEvent1,
    InstanceTimerEvent2,
    InstanceTimerEvent3,
    InstanceTimerEvent4,
    InstanceTimerEvent5,
    DungeonPhase5Clear,
    DungeonPhase5Fail,
    DungeonPhase6Clear,
    DungeonPhase6Fail,
    DungeonPhase1Resume,
    DungeonPhase2Resume,
    DungeonPhase3Resume,
    DungeonPhase4Resume,
    DungeonPhase5Resume,
    DungeonPhase6Resume,
    TrackMoveStart1,
    TrackMoveStart2,
    TrackMoveStart3,
    TrackMoveFailure,
    HitRestoreStart,
    HitRestoreFinish,
    ZoneLevelNormal,
    ZoneLevelHard,
    ZoneLevelHellchaos,
    ZoneLevelChallenge,
    ZoneLevelSpecial,
    OccupationNoneToRed,
    OccupationNoneToBlue,
    OccupationBlueToRed,
    OccupationRedToBlue,
    InstanceTimerEvent6,
    InstanceTimerEvent7,
    InstanceTimerEvent8,
    InstanceTimerEvent9,
    InstanceTimerEvent10,
    UserStatusEffectRemoved,
}

impl Trigger {
    pub fn from_raw(raw: u32) -> Option<Self> {
        Some(match raw {
            1 => Self::None,
            2 => Self::Out,
            3 => Self::ClickClick,
            4 => Self::DoorOpen,
            5 => Self::DoorClose,
            6 => Self::SwitchOn,
            7 => Self::SwitchOff,
            8 => Self::HitHit,
            9 => Self::HitDestruct,
            10 => Self::GripGrip,
            11 => Self::VolumeEnter,
            12 => Self::VolumeLeave,
            13 => Self::VolumeOn,
            14 => Self::VolumeOff,
            15 => Self::NpcSpawn,
            16 => Self::NpcDead,
            17 => Self::NpcEvent1,
            18 => Self::NpcEvent2,
            19 => Self::NpcEvent3,
            20 => Self::NpcEvent4,
            21 => Self::NpcEvent5,
            22 => Self::Yes,
            23 => Self::No,
            24 => Self::PropPickup,
            25 => Self::PropRotateStart,
            26 => Self::PropRotateCancel,
            27 => Self::PropRotateEnd,
            28 => Self::Assembled,
            29 => Self::UserSignal,
            30 => Self::VolumeInputkey,
            31 => Self::SharedClick,
            32 => Self::SharedDespawn,
            33 => Self::DungeonCleared,
            34 => Self::CoopQuestStart,
            35 => Self::CoopQuestComplete,
            36 => Self::CoopQuestFail,
            37 => Self::UserShipWreck,
            38 => Self::TowerHit,
            39 => Self::TowerDestruct,
            40 => Self::VehicleEnter,
            41 => Self::VehicleLeave,
            42 => Self::InstancezoneLoadComplete,
            43 => Self::RandomCase1,
            44 => Self::RandomCase2,
            45 => Self::RandomCase3,
            46 => Self::RandomCase4,
            47 => Self::RandomCase5,
            48 => Self::NpcPathevent,
            49 => Self::JointAttach,
            50 => Self::JointDetach,
            51 => Self::HitOn2,
            52 => Self::HitOn3,
            53 => Self::CoopQuestCancel,
            54 => Self::DungeonEnter,
            55 => Self::StationDisable,
            56 => Self::AllDead,
            57 => Self::AllExit,
            58 => Self::DungeonPhase1Clear,
            59 => Self::DungeonPhase1Fail,
            60 => Self::DungeonPhase2Clear,
            61 => Self::DungeonPhase2Fail,
            62 => Self::DungeonPhase3Clear,
            63 => Self::DungeonPhase3Fail,
            64 => Self::DungeonPhase4Clear,
            65 => Self::DungeonPhase4Fail,
            66 => Self::UserStatusEffect,
            67 => Self::InstanceTimerStart,
            68 => Self::InstanceTimerEnd,
            69 => Self::InstanceTimerCancel,
            70 => Self::InstanceTimerEvent1,
            71 => Self::InstanceTimerEvent2,
            72 => Self::InstanceTimerEvent3,
            73 => Self::InstanceTimerEvent4,
            74 => Self::InstanceTimerEvent5,
            75 => Self::DungeonPhase5Clear,
            76 => Self::DungeonPhase5Fail,
            77 => Self::DungeonPhase6Clear,
            78 => Self::DungeonPhase6Fail,
            79 => Self::DungeonPhase1Resume,
            80 => Self::DungeonPhase2Resume,
            81 => Self::DungeonPhase3Resume,
            82 => Self::DungeonPhase4Resume,
            83 => Self::DungeonPhase5Resume,
            84 => Self::DungeonPhase6Resume,
            85 => Self::TrackMoveStart1,
            86 => Self::TrackMoveStart2,
            87 => Self::TrackMoveStart3,
            88 => Self::TrackMoveFailure,
            89 => Self::HitRestoreStart,
            90 => Self::HitRestoreFinish,
            91 => Self::ZoneLevelNormal,
            92 => Self::ZoneLevelHard,
            93 => Self::ZoneLevelHellchaos,
            94 => Self::ZoneLevelChallenge,
            95 => Self::ZoneLevelSpecial,
            96 => Self::OccupationNoneToRed,
            97 => Self::OccupationNoneToBlue,
            98 => Self::OccupationBlueToRed,
            99 => Self::OccupationRedToBlue,
            100 => Self::InstanceTimerEvent6,
            101 => Self::InstanceTimerEvent7,
            102 => Self::InstanceTimerEvent8,
            103 => Self::InstanceTimerEvent9,
            104 => Self::InstanceTimerEvent10,
            105 => Self::UserStatusEffectRemoved,
            _ => return None,
        })
    }
}

pub mod stat_type {
    pub const NONE: u8 = 0;
    pub const HP: u8 = 1;
    pub const MP: u8 = 2;
    pub const STR: u8 = 3;
    pub const AGI: u8 = 4;
    pub const INT: u8 = 5;
    pub const CON: u8 = 6;
    pub const STR_X: u8 = 7;
    pub const AGI_X: u8 = 8;
    pub const INT_X: u8 = 9;
    pub const CON_X: u8 = 10;
    pub const STR_X_X_DELETED___: u8 = 11;
    pub const AGI_X_X_DELETED___: u8 = 12;
    pub const INT_X_X_DELETED___: u8 = 13;
    pub const CON_X_X_DELETED___: u8 = 14;
    pub const CRITICALHIT: u8 = 15;
    pub const SPECIALTY: u8 = 16;
    pub const OPPRESSION: u8 = 17;
    pub const RAPIDITY: u8 = 18;
    pub const ENDURANCE: u8 = 19;
    pub const MASTERY: u8 = 20;
    pub const CRITICALHIT_X: u8 = 21;
    pub const SPECIALTY_X: u8 = 22;
    pub const OPPRESSION_X: u8 = 23;
    pub const RAPIDITY_X: u8 = 24;
    pub const ENDURANCE_X: u8 = 25;
    pub const MASTERY_X: u8 = 26;
    pub const MAX_HP: u8 = 27;
    pub const MAX_MP: u8 = 28;
    pub const MAX_HP_X: u8 = 29;
    pub const MAX_MP_X: u8 = 30;
    pub const MAX_HP_X_X: u8 = 31;
    pub const MAX_MP_X_X: u8 = 32;
    pub const NORMAL_HP_RECOVERY: u8 = 33;
    pub const COMBAT_HP_RECOVERY: u8 = 34;
    pub const NORMAL_HP_RECOVERY_RATE: u8 = 35;
    pub const COMBAT_HP_RECOVERY_RATE: u8 = 36;
    pub const NORMAL_MP_RECOVERY: u8 = 37;
    pub const COMBAT_MP_RECOVERY: u8 = 38;
    pub const NORMAL_MP_RECOVERY_RATE: u8 = 39;
    pub const COMBAT_MP_RECOVERY_RATE: u8 = 40;
    pub const SELF_RECOVERY_RATE: u8 = 41;
    pub const DRAIN_HP_DAM_RATE: u8 = 42;
    pub const DRAIN_MP_DAM_RATE: u8 = 43;
    pub const DAM_REFLECTION_RATE: u8 = 44;
    pub const MIN_WEAPON_DAM_DELETED___: u8 = 45;
    pub const MAX_WEAPON_DAM_DELETED___: u8 = 46;
    pub const CHAR_ATTACK_DAM: u8 = 47;
    pub const SKILL_EFFECT_DAM_ADDEND: u8 = 48;
    pub const ATTACK_POWER_RATE: u8 = 49;
    pub const SKILL_DAMAGE_RATE: u8 = 50;
    pub const ATTACK_POWER_RATE_X: u8 = 51;
    pub const SKILL_DAMAGE_RATE_X: u8 = 52;
    pub const COOLDOWN_REDUCTION: u8 = 53;
    pub const PARALYZATION_POINT_RATE: u8 = 54;
    pub const DEF: u8 = 55;
    pub const RES: u8 = 56;
    pub const DEF_X: u8 = 57;
    pub const RES_X: u8 = 58;
    pub const DEF_X_X: u8 = 59;
    pub const RES_X_X: u8 = 60;
    pub const DEF_DEC_DELETED___: u8 = 61;
    pub const RES_DEC_DELETED___: u8 = 62;
    pub const DEF_DEC_X_DELETED___: u8 = 63;
    pub const RES_DEC_X_DELETED___: u8 = 64;
    pub const DEF_DEC_X_X_DELETED___: u8 = 65;
    pub const RES_DEC_X_X_DELETED___: u8 = 66;
    pub const DEF_PEN_RATE: u8 = 67;
    pub const RES_PEN_RATE: u8 = 68;
    pub const PHYSICAL_INC_RATE: u8 = 69;
    pub const MAGICAL_INC_RATE: u8 = 70;
    pub const SELF_SHIELD_RATE: u8 = 71;
    pub const HIT_RATE: u8 = 72;
    pub const DODGE_RATE: u8 = 73;
    pub const CRITICAL_HIT_RATE: u8 = 74;
    pub const CRITICAL_RES_RATE: u8 = 75;
    pub const CRITICAL_DAM_RATE: u8 = 76;
    pub const ATTACK_SPEED: u8 = 77;
    pub const ATTACK_SPEED_RATE: u8 = 78;
    pub const MOVE_SPEED: u8 = 79;
    pub const MOVE_SPEED_RATE: u8 = 80;
    pub const PROP_MOVE_SPEED: u8 = 81;
    pub const PROP_MOVE_SPEED_RATE: u8 = 82;
    pub const VEHICLE_MOVE_SPEED: u8 = 83;
    pub const VEHICLE_MOVE_SPEED_RATE: u8 = 84;
    pub const SHIP_MOVE_SPEED: u8 = 85;
    pub const SHIP_MOVE_SPEED_RATE: u8 = 86;
    pub const FIRE_DAM_RATE: u8 = 87;
    pub const ICE_DAM_RATE: u8 = 88;
    pub const ELECTRICITY_DAM_RATE: u8 = 89;
    pub const WIND_DAM_RATE_DELETED___: u8 = 90;
    pub const EARTH_DAM_RATE: u8 = 91;
    pub const DARK_DAM_RATE: u8 = 92;
    pub const HOLY_DAM_RATE: u8 = 93;
    pub const ELEMENTS_DAM_RATE: u8 = 94;
    pub const FIRE_RES_RATE: u8 = 95;
    pub const ICE_RES_RATE: u8 = 96;
    pub const ELECTRICITY_RES_RATE: u8 = 97;
    pub const WIND_RES_RATE_DELETED___: u8 = 98;
    pub const EARTH_RES_RATE: u8 = 99;
    pub const DARK_RES_RATE: u8 = 100;
    pub const HOLY_RES_RATE: u8 = 101;
    pub const ELEMENTS_RES_RATE: u8 = 102;
    pub const MOVE_CC_RES_RATE_DELETED___: u8 = 103;
    pub const CONDITION_CC_RES_RATE_DELETED___: u8 = 104;
    pub const SELF_CC_TIME_RATE: u8 = 105;
    pub const ENEMY_CC_TIME_RATE: u8 = 106;
    pub const IDENTITY_VALUE1: u8 = 107;
    pub const IDENTITY_VALUE2: u8 = 108;
    pub const IDENTITY_VALUE3: u8 = 109;
    pub const AWAKENING_DAM_RATE: u8 = 110;
    pub const ITEM_DROP_RATE: u8 = 111;
    pub const GOLD_RATE: u8 = 112;
    pub const EXP_RATE: u8 = 113;
    pub const DAM_ATTR_VALUE_DELETED___: u8 = 114;
    pub const CHAR_ATTR_ATTACK_DAM_DELETED___: u8 = 115;
    pub const FIRE_DEF_DELETED___: u8 = 116;
    pub const ICE_DEF_DELETED___: u8 = 117;
    pub const ELECTRICITY_DEF_DELETED___: u8 = 118;
    pub const EARTH_DEF_DELETED___: u8 = 119;
    pub const DARK_DEF_DELETED___: u8 = 120;
    pub const HOLY_DEF_DELETED___: u8 = 121;
    pub const ELEMENTAL_DEF_DELETED___: u8 = 122;
    pub const ATTACK_POWER_ADDEND: u8 = 123;
    pub const ATTR_ATTACK_POWER_ADDEND_DELETED___: u8 = 124;
    pub const NPC_SPECIES_HUMANOID_DAM_RATE: u8 = 125;
    pub const NPC_SPECIES_DEVIL_DAM_RATE: u8 = 126;
    pub const NPC_SPECIES_SUBSTANCE_DAM_RATE: u8 = 127;
    pub const NPC_SPECIES_UNDEAD_DAM_RATE: u8 = 128;
    pub const NPC_SPECIES_PLANT_DAM_RATE: u8 = 129;
    pub const NPC_SPECIES_INSECT_DAM_RATE: u8 = 130;
    pub const NPC_SPECIES_SPIRIT_DAM_RATE: u8 = 131;
    pub const NPC_SPECIES_WILD_BEAST_DAM_RATE: u8 = 132;
    pub const NPC_SPECIES_MECHANIC_DAM_RATE: u8 = 133;
    pub const NPC_SPECIES_ANCIENT_DAM_RATE: u8 = 134;
    pub const NPC_SPECIES_GOD_DAM_RATE: u8 = 135;
    pub const NPC_SPECIES_ARCHFIEND_DAM_RATE: u8 = 136;
    pub const VITALITY: u8 = 137;
    pub const SHIP_BOOTER_SPEED: u8 = 138;
    pub const SHIP_WRECK_SPEED_RATE: u8 = 139;
    pub const ISLAND_SPEED_RATE: u8 = 140;
    pub const ATTACK_POWER_SUB_RATE_2: u8 = 141;
    pub const ATTACK_POWER_SUB_RATE_3: u8 = 142;
    pub const PHYSICAL_INC_SUB_RATE_2: u8 = 143;
    pub const PHYSICAL_INC_SUB_RATE_3: u8 = 144;
    pub const MAGICAL_INC_SUB_RATE_2: u8 = 145;
    pub const MAGICAL_INC_SUB_RATE_3: u8 = 146;
    pub const SKILL_DAMAGE_SUB_RATE_2: u8 = 147;
    pub const SKILL_DAMAGE_SUB_RATE_3: u8 = 148;
    pub const RESOURCE_RECOVERY_RATE: u8 = 149;
    pub const NPC_ADAPTATION_DELETED___: u8 = 150;
    pub const WEAPON_DAM: u8 = 151;
    pub const MAX: u8 = 152;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Boss {
    ValtanG1,
    ValtanG2,
    ValtanG2Ghost,

    VykasG1,
    VykasG2,
    VykasG3,

    KakulSaydonG1,
    KakulSaydonG2,
    KakulSaydonG3,
    KakulSaydonG3Bingo,

    BrelshazaG1Dogs,
    BrelshazaG1Pre,
    BrelshazaG1,
    BrelshazaG2Prokel,
    BrelshazaG2,
    BrelshazaG3,
    BrelshazaG4,
    BrelshazaG5Cube,
    BrelshazaG5,
    BrelshazaG6,

    AkkanG1,
    AkkanG2,
    AkkanG3,
    AkkanG3Bonus,

    Bird,
    Tienis,
    Prunya,
    Lauriel,

    Deskaluda,
    Kungelanium,
    Caliligos,
    Hanumatan,
    Sonavel,

    Golem,
}

impl Boss {
    pub fn name(&self) -> &'static str {
        // TODO: temp system
        match self {
            Boss::ValtanG1 => "valtan-g1",
            Boss::ValtanG2 => "valtan-g2",
            Boss::ValtanG2Ghost => "valtan-g2-ghost",
            Boss::VykasG1 => "vykas-g1",
            Boss::VykasG2 => "vykas-g2",
            Boss::VykasG3 => "vykas-g3",
            Boss::KakulSaydonG1 => "clown-g1",
            Boss::KakulSaydonG2 => "clown-g2",
            Boss::KakulSaydonG3 => "clown-g3",
            Boss::KakulSaydonG3Bingo => "clown-g3-bingo",
            Boss::BrelshazaG1Dogs => "brel-g1-dogs",
            Boss::BrelshazaG1Pre => "brel-g1-pre",
            Boss::BrelshazaG1 => "brel-g1",
            Boss::BrelshazaG2Prokel => "brel-g2-prokel",
            Boss::BrelshazaG2 => "brel-g2",
            Boss::BrelshazaG3 => "brel-g3",
            Boss::BrelshazaG4 => "brel-g4",
            Boss::BrelshazaG5Cube => "brel-g5-cube",
            Boss::BrelshazaG5 => "brel-g5",
            Boss::BrelshazaG6 => "brel-g6",
            Boss::AkkanG1 => "akkan-g1",
            Boss::AkkanG2 => "akkan-g2",
            Boss::AkkanG3 => "akkan-g3",
            Boss::AkkanG3Bonus => "akkan-g3-bonus",
            Boss::Bird => "kayangel-bird",
            Boss::Tienis => "kayangel-g1",
            Boss::Prunya => "kayangel-g2",
            Boss::Lauriel => "kayangel-g3",
            Boss::Deskaluda => "deskaluda",
            Boss::Kungelanium => "kungelanium",
            Boss::Caliligos => "caliligos",
            Boss::Hanumatan => "hanumatan",
            Boss::Sonavel => "sonavel",
            Boss::Golem => "golem",
        }
    }

    pub fn from_id(id: u32) -> Option<Self> {
        match id {
            720011 => Some(Self::Golem),

            480005 | 480026 | // Leader Lugaru 
            480006 | 480031 | // Destroyer Lucas
            480009 | 480010 // Dark Mountain Predator
            => Some(Self::ValtanG1),
            42063041 | 42063042 | 42063043 | 42063044 => Some(Self::ValtanG2),
            480007 => Some(Self::ValtanG2Ghost),

            480208 | // Incubus Morphe
            480209 // Nightmarish Morphe
            => Some(Self::VykasG1),
            480210 => Some(Self::VykasG2),
            480211 => Some(Self::VykasG3),

            480691 | 480601
            // | 480779 | 480780 | 480781 | 480782
            // | 480603 | 480604 | 480605
            // | 480606 | 480607 | 480621
            => Some(Self::KakulSaydonG1), // Saydon
            480696 | 480611 | 480612 => Some(Self::KakulSaydonG2), // Kakul
            480631 => Some(Self::KakulSaydonG3), // Kakul-Saydon
            480635 => Some(Self::KakulSaydonG3Bingo), // Encore-Desiring Kakul-Saydon

            480805 | // Crushing Phantom Wardog
            480874 | // Molting Phantom Wardog
            480875 | // Echoing Phantom Wardog
            480876 // Raging Phantom Wardog
            => Some(Self::BrelshazaG1Dogs),
            480803 => Some(Self::BrelshazaG1Pre), // Nightmare Gehenna
            480802 => Some(Self::BrelshazaG1), // Gehenna Helkasirs
            480808 => Some(Self::BrelshazaG2Prokel), // Prokel
            480809 => Some(Self::BrelshazaG2), // Prokel's Spiritual Echo
            480810 => Some(Self::BrelshazaG3), // Ashtarot
            480811 => Some(Self::BrelshazaG4), // Primordial Nightmare
            4221463 | // Pseudospace Primordial Nightmare
            4221464 // Imagined Primordial Nightmare
            => Some(Self::BrelshazaG5Cube),
            480813 | 480815 => Some(Self::BrelshazaG5), // Brelshaza, Monarch of Nightmares
            480814 => Some(Self::BrelshazaG6), // Phantom Legion Commander Brelshaza

            480920 | 480934 | 480935 | 480954 | 480955 => Some(Self::AkkanG1), // Griefbringer Maurug
            // Lord of Degradation Akkan
            481085 | 480902 | 480930 | 480931 | 480932 | 480936 | 480996 | 480997 | 480998 |
            481050 | 481051 | 481053 | 481057 | 480059 | 481060 | 481061 | 481066 | 481067 |
            481068 | 481069 | 481070 => Some(Self::AkkanG2), // Lord of Degradation Akkan
            481076 | 480903 | 480905 |
            886045 | 131770 | 820109 => Some(Self::AkkanG3), // Plague Legion Commander Akkan
            481078 | 481079 | 481080 | 481081 | 480904 | 480964 | 480965 | 480966 | 480967 |
            480968 | 480969 => Some(Self::AkkanG3Bonus),

            620260 => Some(Self::Deskaluda),
            620250 => Some(Self::Caliligos),
            620280 => Some(Self::Hanumatan),
            620400 => Some(Self::Sonavel),
            _ => None,
        }
    }

    pub fn max_bars(&self) -> Option<u16> {
        match self {
            Self::ValtanG1 => Some(50),
            Self::ValtanG2 => Some(160),
            Self::ValtanG2Ghost => Some(40),
            Self::VykasG1 => Some(60),
            Self::VykasG2 => Some(160),
            Self::VykasG3 => Some(180),
            Self::KakulSaydonG1 => Some(160),
            Self::KakulSaydonG2 => Some(140),
            Self::KakulSaydonG3 => Some(180),
            Self::KakulSaydonG3Bingo => Some(77),
            Self::BrelshazaG1Dogs => Some(20),
            Self::BrelshazaG1Pre => Some(40),
            Self::BrelshazaG1 => Some(120),
            Self::BrelshazaG2Prokel => Some(160),
            Self::BrelshazaG2 => Some(80),
            Self::BrelshazaG3 => Some(170),
            Self::BrelshazaG4 => Some(190),
            Self::BrelshazaG5Cube => Some(20),
            Self::BrelshazaG5 => Some(200),
            Self::BrelshazaG6 => Some(250),
            _ => None,
        }
    }

    pub fn id(&self) -> u32 {
        match self {
            Self::Golem => 720011,

            _ => 0, // TODO
        }
    }
}
