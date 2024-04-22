//! Meter logic - LoA packet processing.

use std::{
    borrow::Cow,
    collections::{btree_map::Entry, BTreeMap},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Context;
use parking_lot::Mutex;

use crate::{
    capture::PacketHandler,
    definitions::{Boss, Class, HitFlag, HitOption},
    packet::{
        PktInitEnv, PktInitPc, PktNewNpc, PktNewPc, PktNewProjectile, PktParalyzationStateNotify,
        PktRaidBossKillNotify, PktRaidResult, PktSkillDamageAbnormalMoveNotify,
        PktSkillDamageNotify, PktTriggerBossBattleStatus, PktTriggerStartNotify, SkillDamageEvent,
    },
    parser::Packet,
    util::snappy_file_reader,
};

#[allow(dead_code)]
pub mod log {
    use serde::Serialize;
    use std::collections::BTreeMap;

    // milliseconds since start
    #[derive(Serialize)]
    pub struct Timestamp(u64);
    #[derive(Serialize)]
    pub struct Damage(i64);
    #[derive(Debug, Copy, Clone, Serialize)]
    pub struct EntityIndex(usize);
    #[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Serialize)]
    pub struct SkillId(u32);
    #[derive(Serialize)]
    pub struct SpeciesId(u32);

    #[derive(Serialize)]
    pub enum Status {
        Wipe,
        Clear,
    }

    #[derive(Serialize)]
    pub struct Log {
        pub end: Timestamp,
        pub entities: Vec<Entity>,
        pub pov: Option<EntityIndex>,
        pub targets: Vec<EntityIndex>,
        pub status: Option<Status>,
    }

    #[derive(Serialize)]
    pub struct Entity {
        pub name: Option<String>,
        pub damage: Vec<(Timestamp, Damage)>,
        pub casts: Vec<(Timestamp, SkillId)>,
        pub skills: BTreeMap<SkillId, Skill>,
        pub kind: EntityKind,
    }

    #[derive(Serialize)]
    pub enum EntityKind {
        Player,
        Npc(SpeciesId),
    }

    #[derive(Serialize)]
    pub struct Skill {
        pub name: Option<String>,
        pub hits: Vec<(Timestamp, SkillHit)>,
    }

    #[derive(Serialize)]
    pub struct SkillHit {
        pub damage: Damage,
        pub target: EntityIndex,
        pub is_crit: bool,
        pub is_back_attack: bool,
        pub is_front_attack: bool,
    }

    impl Log {
        pub fn from_encounter(
            enc: &crate::meter::Encounter,
            env: &crate::meter::Environment,
        ) -> Option<Self> {
            let start = enc.start;
            let to_ts = |instant: std::time::Instant| -> Timestamp {
                instant
                    .duration_since(start)
                    .as_millis()
                    .try_into()
                    .map(Timestamp)
                    .expect("millis to fit in u64")
            };
            let end = to_ts(enc.end.unwrap());
            let mut entities = Vec::new();
            let mut entity_map: BTreeMap<u64, EntityIndex> = BTreeMap::new();
            for (&id, player) in &env.players {
                let Some(enc_data) = enc.players.get(&id) else {
                    continue;
                };
                let entity = Entity {
                    name: player.name.clone(),
                    damage: enc_data
                        .damage
                        .iter()
                        .map(|&(i, dmg)| (to_ts(i), Damage(dmg)))
                        .collect(),
                    casts: enc_data
                        .casts
                        .iter()
                        .map(|&(i, id)| (to_ts(i), SkillId(id)))
                        .collect(),
                    skills: BTreeMap::new(),
                    kind: EntityKind::Player,
                };
                entity_map.insert(id, EntityIndex(entities.len()));
                entities.push(entity);
            }

            for (&id, npc) in &env.npcs {
                let entity = Entity {
                    name: Some(npc.name.clone()),
                    damage: Vec::new(),
                    casts: Vec::new(),
                    skills: BTreeMap::new(),
                    kind: EntityKind::Npc(SpeciesId(npc.kind)),
                };
                entity_map.insert(id, EntityIndex(entities.len()));
                entities.push(entity);
            }

            for (i, entity) in entities.iter_mut().enumerate() {
                // need to get their id back out
                let (id, _) = entity_map
                    .iter()
                    .find(|(_, &EntityIndex(idx))| idx == i)
                    .unwrap();
                let Some(enc_data) = enc.players.get(&id) else {
                    continue;
                };
                let skills = enc_data
                    .skills
                    .iter()
                    .map(|(&id, skill)| {
                        let id = SkillId(id);
                        let skill = Skill {
                            name: skill.name.clone(),
                            hits: skill
                                .hits
                                .iter()
                                .filter_map(|(i, hit)| {
                                    // TODO: unsure what to do when target is not in env
                                    let Some(target) = entity_map.get(&hit.target_id).copied()
                                    else {
                                        println!("target wasn't in env: {}", hit.target_id);
                                        return None;
                                    };
                                    Some((
                                        to_ts(*i),
                                        SkillHit {
                                            damage: Damage(hit.damage),
                                            target,
                                            is_crit: hit.is_crit,
                                            is_back_attack: hit.is_back_attack,
                                            is_front_attack: hit.is_front_attack,
                                        },
                                    ))
                                })
                                .collect(),
                        };
                        (id, skill)
                    })
                    .collect();
                entity.skills = skills;
            }

            let pov = env.pov.and_then(|id| entity_map.get(&id)).copied();
            let targets = enc
                .tracked
                .iter()
                .flat_map(|(id, _)| entity_map.get(&id))
                .copied()
                .collect();

            let status = if enc.clear {
                Some(Status::Clear)
            } else if enc.wipe {
                Some(Status::Wipe)
            } else {
                None
            };

            let log = Self {
                end,
                entities,
                pov,
                targets,
                status,
            };

            Some(log)
        }
    }
}

/// Processes packets and updates [`Data`].
pub struct Meter {
    ui_ctx: egui::Context,
    data: Arc<Mutex<Data>>,
    skill_data: SkillData,

    #[cfg(feature = "packet_logging")]
    log: Vec<u8>,
}

impl Meter {
    pub fn new(ui_ctx: egui::Context, data: Arc<Mutex<Data>>) -> anyhow::Result<Self> {
        Ok(Self {
            ui_ctx,
            data,
            skill_data: SkillData::load()?,

            #[cfg(feature = "packet_logging")]
            log: Vec::new(),
        })
    }

    // process an incoming set of damage events
    fn process_damages<'a>(
        &mut self,
        source_id: u64,
        skill_id: u32,
        events: impl Iterator<Item = &'a SkillDamageEvent>,
    ) -> anyhow::Result<()> {
        let timestamp = Instant::now();

        let data = &mut *self.data.lock();
        let mut id = source_id;
        while let Some(p) = data.current_env().projectiles.get(&id) {
            println!("projectile");
            id = p.owner_id;
        }

        // add dummy player to environment
        if let Entry::Vacant(e) = data.current_env_mut().players.entry(id) {
            let info = Player {
                name: None,
                class: match self.skill_data.class_for(skill_id) {
                    Some(p) => p,
                    None => return Ok(()),
                },
                ilvl: 0.0,
                character_id: None,
            };
            println!("adding dummy player {}", id);
            e.insert(info);
            // println!("players: {:#?}", data.current_env().players);
        }

        let enc = {
            let len = data.encounters.len();
            &mut data.encounters[len - 1]
        };

        let player = enc.players.entry(id).or_insert_with(Default::default);
        let party = data.live.parties.get(&id).copied();
        let has_ap_buff = data.live.player_has_ap_buff(id);
        let has_ident_buff = data.live.player_has_ident_buff(id);
        let mut target_is_boss = false;

        for evt in events {
            let overkill = evt.cur_hp.min(0).abs();
            let damage = evt.damage.saturating_sub(overkill).max(0);
            let flag = evt.flag()?;
            if damage == 0 {
                continue;
            }
            if flag == HitFlag::DamageShare
            /* && skill_id == 0 */
            {
                println!("is this sidereal damage?");
                continue;
            }
            let option = evt.option()?;
            let branded = data.live.target_has_brand(id, evt.target_id, party);
            player.damage.push((timestamp, damage));
            player.dmg_dealt += damage;
            player.hits += 1;

            let skill = player.skills.entry(skill_id).or_insert_with(|| SkillUsage {
                name: match self.skill_data.name(skill_id) {
                    None if skill_id == 0 && flag.is_dot() => Some("Bleed"),
                    rest => rest,
                }
                .map(ToOwned::to_owned),
                ..Default::default()
            });

            player.casts.push((timestamp, skill_id));

            let hit = SkillHit {
                damage,
                target_id: evt.target_id,
                is_crit: flag.is_crit(),
                is_back_attack: matches!(option, HitOption::BackAttack),
                is_front_attack: matches!(option, HitOption::FrontalAttack),
            };
            skill.hits.push((timestamp, hit));

            skill.count += 1;
            skill.damage += damage;
            if flag.is_crit() {
                skill.crits += 1;
            }
            match option {
                HitOption::BackAttack => skill.back += 1,
                HitOption::FrontalAttack => skill.front += 1,
                _ => {}
            }
            if branded {
                skill.brand += 1;
                player.brand_hits += 1;
                player.brand_dmg += damage;
            }
            if has_ap_buff {
                skill.ap_buff += 1;
                player.ap_hits += 1;
                player.ap_dmg += damage;
            }
            if has_ident_buff {
                skill.ident_buff += 1;
                player.ident_hits += 1;
                player.ident_dmg += damage;
            }

            for &(id, tracked) in &enc.tracked {
                if id == evt.target_id {
                    target_is_boss = true;
                    data.live.recently_tracked = Some(id);
                    data.live.tracked.insert(
                        id,
                        BossInfo {
                            max_hp: evt.max_hp,
                            cur_hp: evt.cur_hp,
                            bar_count: tracked.max_bars(),
                        },
                    );
                }
            }
        }

        let encounter = data.current_enc_mut();
        if encounter.tracked.is_empty() || target_is_boss {
            if encounter.first_damage.is_none() {
                println!("first damage set");
                encounter.first_damage = Some(timestamp);
            }
            encounter.last_damage = Some(timestamp);
        }

        self.ui_ctx.request_repaint();
        Ok(())
    }

    // defer starting a new encounter for a few seconds as some final events may be missed
    // if swapping to new encounter immediately
    fn defer_new_encounter(&self) {
        let data = Arc::clone(&self.data);
        // TODO keep this thread around instead of spawning new one each time
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(3));
            println!("waking up");
            data.lock().new_encounter();
        });
    }

    #[cfg(feature = "packet_logging")]
    fn log_packet<S>(&mut self, pkt: &S)
    where
        S: Packet + serde::Serialize,
    {
        serde_bare::to_writer(&mut self.log, &S::OPCODE.to_u16()).unwrap();
        serde_bare::to_writer(&mut self.log, pkt).unwrap();
    }
}

impl PacketHandler for Meter {
    fn on_trigger_start_notify(&mut self, pkt: PktTriggerStartNotify) -> anyhow::Result<()> {
        use crate::definitions::trigger_signal;
        let mut data = self.data.lock();
        match pkt.trigger_signal_type {
            trigger_signal::DUNGEON_PHASE1_FAIL
            | trigger_signal::DUNGEON_PHASE2_FAIL
            | trigger_signal::DUNGEON_PHASE3_FAIL
            | trigger_signal::DUNGEON_PHASE4_FAIL => {
                data.current_enc_mut().wipe = true;
            }
            trigger_signal::DUNGEON_PHASE1_CLEAR
            | trigger_signal::DUNGEON_PHASE2_CLEAR
            | trigger_signal::DUNGEON_PHASE3_CLEAR
            | trigger_signal::DUNGEON_PHASE4_CLEAR => {
                data.current_enc_mut().clear = true;
            }
            _ => {}
        }
        Ok(())
    }

    fn on_new_projectile(&mut self, pkt: PktNewProjectile) -> anyhow::Result<()> {
        let id = pkt.projectile_info.projectile_id;
        let projectile = Projectile::from_raw(pkt.projectile_info);
        self.data
            .lock()
            .current_env_mut()
            .add_projectile(id, projectile);
        Ok(())
    }

    fn on_init_env(&mut self, pkt: PktInitEnv) -> anyhow::Result<()> {
        println!("init env: player id {}", pkt.player_id);
        let mut environment = Environment {
            pov: Some(pkt.player_id),
            ..Default::default()
        };
        let mut data = self.data.lock();

        // pov player must be retained in new environment
        if let Some(player) = data.current_env().pov() {
            environment.add_player(pkt.player_id, player.clone());
        }
        // this is a map change, meaning all entity IDs changed and we need a new
        // environment to store them in
        data.environments.push(environment);
        data.live.clear_all();
        data.new_encounter();
        Ok(())
    }

    fn on_raid_boss_kill_notify(&mut self, pkt: PktRaidBossKillNotify) -> anyhow::Result<()> {
        println!("raid boss kill notify");
        self.defer_new_encounter();
        Ok(())
    }

    fn on_trigger_boss_battle_status(
        &mut self,
        pkt: PktTriggerBossBattleStatus,
    ) -> anyhow::Result<()> {
        {
            let data = self.data.lock();
            let enc = data.current_enc();
            if enc.clear || enc.wipe {
                println!("boss battle status trigger after clear or wipe");
            } else {
                println!("boss battle status trigger without clear or wipe set");
            }
        }
        self.defer_new_encounter();
        Ok(())
    }

    fn on_raid_result(&mut self, pkt: PktRaidResult) -> anyhow::Result<()> {
        println!("raid result");
        self.defer_new_encounter();
        Ok(())
    }

    fn on_init_pc(&mut self, pkt: PktInitPc) -> anyhow::Result<()> {
        println!("init pc");
        let mut data = self.data.lock();
        let player = Player {
            name: Some(pkt.name.to_owned()),
            class: Class::from_id(pkt.class_id),
            ilvl: pkt.gear_level,
            character_id: Some(pkt.character_id),
        };

        if let Some(party) = data.live.parties.remove(&pkt.character_id) {
            data.live.parties.insert(pkt.player_id, party);
            // TODO: not sure if it's worth putting character_id into the map
            data.current_env_mut().players.remove(&pkt.character_id);
        }

        data.current_env_mut().add_player(pkt.player_id, player);

        Ok(())
    }

    fn on_new_pc(&mut self, pkt: PktNewPc) -> anyhow::Result<()> {
        let id = pkt.pc_struct.player_id;
        let mut data = self.data.lock();
        println!("new player: {}", pkt.pc_struct.name);
        let player = Player::from_raw(pkt.pc_struct);
        data.current_env_mut().add_player(id, player);
        Ok(())
    }

    fn on_new_npc(&mut self, pkt: PktNewNpc) -> anyhow::Result<()> {
        let npc = Npc {
            id: pkt.npc_struct.object_id,
            kind: pkt.npc_struct.type_id,
            name: "Boss".to_owned(),
        };
        let mut data = self.data.lock();
        if let Some(boss) = crate::definitions::Boss::from_id(npc.kind) {
            println!("boss found: {}", npc.kind);
            if data.current_enc().tracked.is_empty() {
                data.new_encounter();
            }
            data.current_enc_mut().tracked.push((npc.id, boss));
        }
        data.current_env_mut().add_npc(npc.id, npc);
        Ok(())
    }

    fn on_skill_damage_notify(&mut self, pkt: PktSkillDamageNotify) -> anyhow::Result<()> {
        self.process_damages(pkt.source_id, pkt.skill_id, pkt.skill_damage_events.iter())
    }

    fn on_skill_damage_abnormal_move_notify(
        &mut self,
        pkt: PktSkillDamageAbnormalMoveNotify,
    ) -> anyhow::Result<()> {
        self.process_damages(
            pkt.source_id,
            pkt.skill_id,
            pkt.skill_damage_abnormal_move_events
                .iter()
                .map(|e| &e.skill_damage_event),
        )
    }

    fn on_paralyzation_state_notify(
        &mut self,
        pkt: PktParalyzationStateNotify,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_status_effect_add_notify(
        &mut self,
        pkt: crate::packet::PktStatusEffectAddNotify,
    ) -> anyhow::Result<()> {
        // Bard
        // Note Brand (3s: Sound Shock, Harp, Stigma, Note Bundle) 210230
        // Note Brand (4s: Rhapsody) 212610
        // Note Brand (5s: Sonatina) 212906
        // Heavenly Tune AP 211601
        // Sonic Vibration AP 211749
        // Serenade of Courage, 1 bar (party) 211400
        // Serenade of Courage, 2 bar (party) 211410
        // Serenade of Courage, 3 bar (party) 211420

        // Paladin
        // Light's Vestige (6s: Light Shock) 360506
        // Light's Vestige (10s: Sword of Justice) 360804
        // Light's Vestige (12s: Holy Explosion) 361004
        // Light's Vestige (12s: Godsent Law) 361505
        // Wrath of God AP 361708
        // Heavenly Blessings AP 362000
        // Blessed Aura 500150

        // Artist
        // Ink Brand (i12s: Paint: Drawing Orchids) 314260
        // Paint: Sunsketch AP 314004
        // Paint: Sun Well AP 314181
        // Moonfall 310501

        // println!(
        //     "status effect add: {} ({})",
        //     pkt.status_effect_data.status_effect_id, pkt.status_effect_data.effect_instance_id
        // );

        let mut data = self.data.lock();
        data.live.buffs.entry(pkt.object_id).or_default().insert(
            pkt.status_effect_data.status_effect_id,
            BuffInfo {
                stacks: pkt.status_effect_data.stack_count,
                applicant: pkt.status_effect_data.source_id,
            },
        );
        data.live.instance_id_lookup.insert(
            pkt.status_effect_data.effect_instance_id,
            pkt.status_effect_data.status_effect_id,
        );

        Ok(())
    }

    fn on_status_effect_remove_notify(
        &mut self,
        pkt: crate::packet::PktStatusEffectRemoveNotify,
    ) -> anyhow::Result<()> {
        let mut data = self.data.lock();
        for instance_id in &pkt.status_effect_ids {
            if let Some(effect_id) = data.live.instance_id_lookup.get(instance_id).copied() {
                if let Some(buff_map) = data.live.buffs.get_mut(&pkt.object_id) {
                    buff_map.remove(&effect_id);
                }
            }
            data.live.instance_id_lookup.remove(instance_id);
        }
        Ok(())
    }

    fn on_party_status_effect_add_notify(
        &mut self,
        pkt: crate::packet::PktPartyStatusEffectAddNotify,
    ) -> anyhow::Result<()> {
        let data = &mut *self.data.lock();
        if let Some(object_id) = data.current_env().players.iter().find_map(|(&id, p)| {
            p.character_id
                .is_some_and(|pid| pid == pkt.character_id && id != pid)
                .then_some(id)
        }) {
            let entry = data.live.buffs.entry(object_id).or_default();
            for eff in &pkt.status_effect_datas {
                entry.insert(
                    eff.status_effect_id,
                    BuffInfo {
                        stacks: eff.stack_count,
                        applicant: eff.source_id,
                    },
                );
                data.live
                    .instance_id_lookup
                    .insert(eff.effect_instance_id, eff.status_effect_id);
            }
        }

        Ok(())
    }

    fn on_party_status_effect_remove_notify(
        &mut self,
        pkt: crate::packet::PktPartyStatusEffectRemoveNotify,
    ) -> anyhow::Result<()> {
        let mut data = self.data.lock();
        if let Some(object_id) = data.current_env().players.iter().find_map(|(&id, p)| {
            p.character_id
                .is_some_and(|pid| pid == pkt.character_id && id != pid)
                .then_some(id)
        }) {
            for instance_id in &pkt.status_effect_ids {
                if let Some(effect_id) = data.live.instance_id_lookup.get(instance_id).copied() {
                    if let Some(buff_map) = data.live.buffs.get_mut(&object_id) {
                        buff_map.remove(&effect_id);
                    }
                }
                data.live.instance_id_lookup.remove(instance_id);
            }
        }
        Ok(())
    }

    fn on_party_status_effect_result_notify(
        &mut self,
        pkt: crate::packet::PktPartyStatusEffectResultNotify,
    ) -> anyhow::Result<()> {
        let mut data = self.data.lock();
        if let Some(id) = data.current_env().players.iter().find_map(|(&id, p)| {
            p.character_id
                .is_some_and(|pcid| pcid == pkt.character_id && id != pcid)
                .then_some(id)
        }) {
            data.live.parties.insert(id, pkt.party_instance_id);
        } else {
            // TODO: maybe cleanup character_id stuff
            data.live
                .parties
                .insert(pkt.character_id, pkt.party_instance_id);
        }

        Ok(())
    }

    fn on_party_info(&mut self, pkt: crate::packet::PktPartyInfo) -> anyhow::Result<()> {
        let mut data = self.data.lock();
        let party_id = pkt.party_instance_id;
        let needs_pov_id = data.current_env().pov().is_none();

        for member_data in &pkt.member_datas {
            let (id, player) = match data
                .current_env_mut()
                .players
                .iter_mut()
                .find(|(&pid, p)| {
                    p.character_id == Some(member_data.character_id)
                        && member_data.character_id != pid
                })
                .map(|(&id, p)| (Some(id), p))
            {
                Some(x) => x,
                None => {
                    let key = if needs_pov_id
                        && let Some(id) = data.current_env().pov
                        && let Some(cid) = data.current_env().pov_char_id
                        && cid == member_data.character_id
                    {
                        println!("found pov from party info");
                        id
                    } else {
                        member_data.character_id
                    };

                    let p = data.current_env_mut().players.entry(key).or_default();
                    (None, p)
                }
            };

            if player.name == None {
                player.name = member_data.name.to_owned().into();
                player.class = Class::from_id(member_data.class_id);
                player.ilvl = member_data.gear_level;
                player.character_id = Some(member_data.character_id);
            }

            if let Some(id) = id {
                data.live.parties.insert(id, party_id);
            } else {
                data.live.parties.insert(member_data.character_id, party_id);
            }
        }

        Ok(())
    }

    fn on_migration_execute(
        &mut self,
        pkt: crate::packet::PktMigrationExecute,
    ) -> anyhow::Result<()> {
        let char_id = pkt.account_character_id1.min(pkt.account_character_id2);
        println!(
            "migration execute: pov {:?} -> {}",
            self.data
                .lock()
                .current_env()
                .pov()
                .and_then(|p| p.character_id),
            char_id
        );
        let data = &mut *self.data.lock();
        if data.current_env().pov().is_none() {
            data.current_env_mut().pov_char_id = Some(char_id);
            if let Some(id) = data.current_env().pov {
                if let Some((&prev_key, _)) = data
                    .current_env()
                    .players
                    .iter()
                    .find(|(_, p)| p.character_id == Some(char_id))
                {
                    if let Some(player) = data.current_env_mut().players.remove(&prev_key) {
                        println!("found pov from migration");
                        data.current_env_mut().players.insert(id, player);
                    }
                }
            }
        }
        Ok(())
    }

    fn on_packet<P>(&mut self, pkt: &P)
    where
        P: Packet + serde::Serialize,
    {
        #[cfg(feature = "packet_logging")]
        {
            if P::OPCODE == crate::definitions::Opcode::InitEnv {
                if !self.log.is_empty() {
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    let write_log = |ts, log| {
                        use std::io::Write as _;
                        let f = std::fs::File::create(format!("logs/{}", ts))?;
                        let mut w = snap::write::FrameEncoder::new(f);
                        w.write_all(log)
                    };
                    match write_log(timestamp, self.log.as_slice()) {
                        Ok(()) => println!("wrote log"),
                        Err(e) => println!("error writing logfile: {}", e),
                    }

                    self.log.clear();
                }

                if let Some(player) = self.data.lock().current_env().pov() {
                    let _ = serde_bare::to_writer(&mut self.log, &true);
                    let _ = serde_bare::to_writer(&mut self.log, &player);
                } else {
                    let _ = serde_bare::to_writer(&mut self.log, &false);
                }
            }

            self.log_packet(pkt);
        }

        // println!("{:?}", P::OPCODE);
    }

    //     fn filter(opcode: &Opcode) -> bool {
    //         match opcode {
    //             Opcode::SkillDamageNotify
    //             | Opcode::SkillDamageAbnormalMoveNotify
    //             | Opcode::TriggerStartNotify
    //             | Opcode::InitEnv
    //             | Opcode::NewProjectile
    //             | Opcode::RaidBossKillNotify
    //             | Opcode::TriggerBossBattleStatus
    //             | Opcode::RaidResult
    //             | Opcode::InitPc
    //             | Opcode::NewPc
    //             | Opcode::NewNpc
    //             // | Opcode::RemoveObject
    //             | Opcode::DeathNotify
    //             | Opcode::SkillStartNotify
    //             | Opcode::SkillStageNotify
    //             | Opcode::StatChangeOriginNotify
    //             | Opcode::StatusEffectAddNotify
    //             | Opcode::PartyStatusEffectAddNotify
    //             | Opcode::StatusEffectRemoveNotify
    //             | Opcode::PartyStatusEffectRemoveNotify
    //             | Opcode::CounterAttackNotify
    //             | Opcode::NewNpcSummon
    //             | Opcode::BlockSkillStateNotify
    //             | Opcode::PartyInfo
    //             | Opcode::StatusEffectSyncDataNotify
    //             | Opcode::ParalyzationStateNotify
    //             | Opcode::MigrationExecute => true,
    //             _ => false,
    //         }
    //     }
}

#[derive(Debug, Clone)]
pub struct BossInfo {
    pub max_hp: i64,
    pub cur_hp: i64,
    pub bar_count: Option<u16>,
}

#[derive(Default)]
pub struct LiveData {
    pub tracked: BTreeMap<u64, BossInfo>,
    pub recently_tracked: Option<u64>,
    pub parties: BTreeMap<u64, u32>,
    pub buffs: BTreeMap<u64, BTreeMap<u32, BuffInfo>>,
    pub instance_id_lookup: BTreeMap<u32, u32>,
}

impl LiveData {
    fn clear_encounter_data(&mut self) {
        self.tracked.clear();
        self.recently_tracked = None;
    }

    fn clear_all(&mut self) {
        self.tracked.clear();
        self.recently_tracked = None;
        self.parties.clear();
        self.buffs.clear();
        self.instance_id_lookup.clear();
    }

    fn player_has_ap_buff(&self, player_id: u64) -> bool {
        if let Some(buffs) = self.buffs.get(&player_id) {
            if buffs.contains_key(&211601) // bard
                || buffs.contains_key(&211749)
                || buffs.contains_key(&361708) // paladin
                || buffs.contains_key(&362000)
                || buffs.contains_key(&314004) // artist
                || buffs.contains_key(&314181)
            {
                return true;
            }
        }
        false
    }

    fn player_has_ident_buff(&self, player_id: u64) -> bool {
        if let Some(buffs) = self.buffs.get(&player_id) {
            if buffs.contains_key(&211400) // bard
                || buffs.contains_key(&211410)
                || buffs.contains_key(&211420)
                // || buffs.contains_key(&500128)
                // || buffs.contains_key(&500146)
                || buffs.contains_key(&500153) // paladin
                || buffs.contains_key(&310501)
            {
                return true;
            }
        }
        false
    }

    fn target_has_brand(&self, source_id: u64, target_id: u64, party: Option<u32>) -> bool {
        let parties = &self.parties;
        // let Some(party) = party else { return false };
        if let Some(buffs) = self.buffs.get(&target_id) {
            for (id, info) in buffs.iter().take_while(|&(&id, _)| id <= 361505) {
                if matches!(
                    id,
                    210230 | 212610 | 212906 | // bard
                    360506 | 360804 | 361004 | 361505 | // paladin
                    314260 // artist
                ) {
                    if party.is_some() && parties.get(&info.applicant).copied() == party
                        || info.applicant == source_id
                    {
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[derive(Debug)]
pub struct BuffInfo {
    pub stacks: u8,
    pub applicant: u64,
}

/// Collection of [`Environment`]s and [`Encounter`]s recorded during runtime.
pub struct Data {
    // pub live: Option<BossInfo>,
    pub live: LiveData,
    pub environments: Vec<Environment>,
    pub encounters: Vec<Encounter>,
}

impl Data {
    pub fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            live: LiveData::default(),
            environments: vec![Environment::default()],
            encounters: vec![Encounter::default()],
        }))
    }

    pub fn current_env(&self) -> &Environment {
        self.environments.last().unwrap()
    }
    pub fn current_env_mut(&mut self) -> &mut Environment {
        self.environments.last_mut().unwrap()
    }
    pub fn current_enc(&self) -> &Encounter {
        self.encounters.last().unwrap()
    }
    pub fn current_enc_mut(&mut self) -> &mut Encounter {
        self.encounters.last_mut().unwrap()
    }

    /// Returns an iterator of recent encounters where at least one player performed one action.
    pub fn recent_encounters(&self) -> impl Iterator<Item = (usize, &Encounter)> + '_ {
        self.encounters
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, e)| e.first_damage.is_some())
            .filter(|(_, e)| !e.players.is_empty())
            .filter(|(_, e)| !self.environments[e.environment].players.is_empty())
    }

    /// Begins a new encounter.
    fn new_encounter(&mut self) {
        println!("encounter reset");
        let timestamp = Instant::now();
        self.current_enc_mut().end = Some(timestamp);

        // save log
        // let enc = self.current_enc();
        // if let Some((_, boss)) = enc.tracked.first()
        //     && enc.duration() > Duration::from_secs(5)
        // {
        //     if let Some(log) = log::Log::from_encounter(enc, self.current_env()) {
        //         // save to file
        //         let timestamp = std::time::SystemTime::now()
        //             .duration_since(std::time::SystemTime::UNIX_EPOCH)
        //             .unwrap()
        //             .as_secs();

        //         let write_log = |ts, log| -> anyhow::Result<()> {
        //             let f = std::fs::File::create(format!("proc_logs/{}_{}", ts, boss.name()))?;
        //             let w = snap::write::FrameEncoder::new(f);
        //             serde_bare::to_writer(w, log)?;
        //             Ok(())
        //         };
        //         match write_log(timestamp, &log) {
        //             Ok(()) => println!("wrote processed log"),
        //             Err(e) => println!("error writing logfile: {}", e),
        //         }
        //     }
        // }

        self.live.clear_encounter_data();
        self.encounters.push(Encounter {
            start: timestamp,
            environment: self.environments.len() - 1,
            ..Default::default()
        });
    }
}

/// List of entities present in a map during one or more [`Encounter`]s.
#[derive(Default)]
pub struct Environment {
    /// The ID of the player running the meter, the point of view. This may not be known.
    pub pov: Option<u64>,
    pov_char_id: Option<u64>,
    /// Map of player IDs to their metadata.
    pub players: BTreeMap<u64, Player>,
    pub npcs: BTreeMap<u64, Npc>,
    projectiles: BTreeMap<u64, Projectile>,
}

impl Environment {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn pov(&self) -> Option<&Player> {
        let pov = self.pov?;
        self.players.get(&pov)
    }

    pub fn with_pov(mut self, id: u64) -> Self {
        self.pov = Some(id);
        self
    }

    pub fn with_player(mut self, id: u64, player: Player) -> Self {
        self.add_player(id, player);
        self
    }

    fn add_player(&mut self, id: u64, player: Player) {
        self.players.insert(id, player);
    }

    fn add_npc(&mut self, id: u64, npc: Npc) {
        self.npcs.insert(id, npc);
    }

    fn add_projectile(&mut self, id: u64, projectile: Projectile) {
        self.projectiles.insert(id, projectile);
    }
}

/// Metadata about a player -- their name, class, ilvl.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Player {
    pub name: Option<String>,
    pub class: Class,
    pub ilvl: f32,
    pub character_id: Option<u64>,
    // pub party: Option<u32>,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            name: None,
            class: Class::Unknown,
            ilvl: 0.0,
            character_id: None,
        }
    }
}

impl Player {
    pub fn new<'a, O, C>(name: O, class: Class, ilvl: f32) -> Self
    where
        O: Into<Option<C>>,
        C: Into<Cow<'static, str>>,
    {
        Self {
            name: name.into().map(Into::into).map(Cow::into_owned),
            class,
            ilvl,
            character_id: None,
            // party: None,
        }
    }

    fn from_raw(pc_struct: crate::packet::PcStruct) -> Self {
        let name = Some(pc_struct.name.to_owned());
        let class = Class::from_id(pc_struct.class_id);
        let ilvl = pc_struct.avg_item_level;
        let character_id = Some(pc_struct.character_id);
        Self {
            name,
            class,
            ilvl,
            character_id,
            // party: None,
        }
    }
}

/// Representation of a game encounter.
pub struct Encounter {
    /// Time recording began.
    pub start: Instant,
    /// Time recording ended.
    pub end: Option<Instant>,
    /// Time of first damage dealt, if any.
    pub first_damage: Option<Instant>,
    /// Time of last damage dealt before recording ended, if any.
    pub last_damage: Option<Instant>,
    /// The environment that referenced entity IDs are valid in.
    pub environment: usize,
    /// Maps player ID found in [`Environment`] to their metrics.
    pub players: BTreeMap<u64, PlayerData>,
    /// Main target of the encounter, if any.
    pub tracked: Vec<(u64, Boss)>,
    /// Whether the encounter ended in failure.
    pub wipe: bool,
    /// Whether the encounter ended in success.
    pub clear: bool,
}

impl Default for Encounter {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            end: None,
            first_damage: None,
            last_damage: None,
            environment: 0,
            players: BTreeMap::new(),
            tracked: Vec::new(),
            wipe: false,
            clear: false,
        }
    }
}

impl Encounter {
    pub fn duration(&self) -> Duration {
        self.last_damage
            .or(self.end)
            .unwrap_or_else(Instant::now)
            .duration_since(self.first_damage.unwrap_or(self.start))
    }
}

/// Metrics for a player.
#[derive(Debug, Default)]
pub struct PlayerData {
    /// Total damage dealt by player.
    pub dmg_dealt: i64,
    pub hits: u64,
    pub brand_dmg: i64,
    pub brand_hits: u64,
    pub ap_dmg: i64,
    pub ap_hits: u64,
    pub ident_dmg: i64,
    pub ident_hits: i64,
    /// Map of skill ID to data about that skill.
    pub skills: BTreeMap<u32, SkillUsage>,

    pub damage: Vec<(Instant, i64)>,
    pub casts: Vec<(Instant, u32)>,
}

/// Information about a skill used by a player.
#[derive(Debug, Clone, Default)]
pub struct SkillUsage {
    /// Name of the skill, if the name exists in the skill database.
    pub name: Option<String>,

    /// How many times this skill hit.
    pub count: usize,
    /// How many times this skill crit.
    pub crits: usize,
    /// Total damage dealt by this skill.
    pub damage: i64,
    /// How many times this skill registered as a back attack.
    pub back: usize,
    /// How many times this skill registered as a front attack.
    pub front: usize,
    pub brand: usize,
    pub ap_buff: usize,
    pub ident_buff: usize,

    pub hits: Vec<(Instant, SkillHit)>,
}

#[derive(Debug, Clone)]
pub struct SkillHit {
    pub damage: i64,
    pub target_id: u64,
    pub is_crit: bool,
    pub is_back_attack: bool,
    pub is_front_attack: bool,
}

#[derive(Debug)]
pub struct Npc {
    pub id: u64,
    pub kind: u32,
    pub name: String,
}

#[allow(dead_code)]
#[derive(Debug)]
struct Projectile {
    id: u64,
    owner_id: u64,
}

impl Projectile {
    fn from_raw(raw: crate::packet::ProjectileInfo) -> Self {
        let id = raw.projectile_id;
        let owner_id = raw.owner_id;
        Self { id, owner_id }
    }
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
struct SkillInfo {
    name: String,
    class_id: Option<u16>,
    icon: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct SkillData(std::collections::HashMap<u32, SkillInfo>);

impl SkillData {
    fn load() -> anyhow::Result<Self> {
        Ok(serde_bare::from_reader(snappy_file_reader(
            "resources/skills",
        )?)?)
    }

    fn name(&self, id: u32) -> Option<&str> {
        self.0.get(&id).map(|info| info.name.as_str())
    }

    fn class_for(&self, id: u32) -> Option<Class> {
        self.0
            .get(&id)
            .and_then(|info| info.class_id)
            .map(Class::from_id)
    }
}

impl SkillDamageEvent {
    fn flag(&self) -> anyhow::Result<HitFlag> {
        let raw = self.modifier & 0xf;
        HitFlag::from_raw(raw).with_context(|| format!("damage hit flag invalid, value: {}", raw))
    }

    fn option(&self) -> anyhow::Result<HitOption> {
        let raw = (self.modifier >> 4) & 0x7;
        HitOption::from_raw(raw)
            .with_context(|| format!("damage hit option invalid, value: {}", raw))
    }
}
