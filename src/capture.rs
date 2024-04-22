//! LoA packet capture.

use anyhow::Context;

use crate::{
    definitions::Opcode,
    oodle::OodleDecompressor,
    packet,
    parser::{Event, Packet, Parser},
    socket::{SelectError, Sockets},
    util,
};

use std::time::{Duration, Instant};

const XOR_TABLE: &[u8] = include_bytes!("generated/xor");

/// Capture LoA packets and feed them to a [`PacketHandler`] implementor.
pub fn run<P: PacketHandler>(mut handler: P) -> anyhow::Result<!> {
    let pid = util::pids_for_window_class(b"EFLaunchUnrealUWindowsClient\0")
        .first()
        .cloned()
        .context("couldn't find game process")?;
    let mut sockets = Sockets::new(pid, 6040)?;
    let mut oodle = OodleDecompressor::init(pid)?;
    let mut bump = bumpalo::Bump::new();

    // several buffers for receiving data, unpacking it, combining fragmented packets
    let mut buf = vec![0u8; 65535];
    let mut unpacked_buf = vec![0u8; 65535];
    let mut fragmented = Vec::with_capacity(65535);
    let mut combined_frag;

    // how often to refresh the list of connections
    let refresh_interval = Duration::from_millis(250);
    let mut next_refresh = Instant::now();

    loop {
        // adjust `select` timeout based on time since last refresh
        let sleep_time = next_refresh.saturating_duration_since(Instant::now());
        let selected = match sockets.select(sleep_time) {
            Ok(s) => s,
            Err(SelectError::Timeout) => {
                next_refresh += refresh_interval;
                sockets.refresh().context("socket refreshing failed")?;
                continue;
            }
            Err(SelectError::WinSock(code)) => anyhow::bail!("select error, code {code}"),
        };

        'inner: for socket in selected.into_iter() {
            let _len = socket.recv(&mut buf)?;

            let version = (buf[0] & 0xF0) >> 4;
            if version != 4 {
                println!("received IPv6 packet");
                continue;
            };
            let ihl = buf[0] & 0xF;
            let tcp_hdr = 4 * ihl as usize;
            let len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
            let protocol = buf[9];
            if protocol != 6 {
                println!("received non-TCP packet");
                continue;
            };

            let offset = 4 * ((buf[tcp_hdr + 12] & 0xF0) >> 4) as usize;
            let hdr_len = tcp_hdr + offset;
            let mut loa_packets = &mut buf[hdr_len..len];
            while !loa_packets.is_empty() {
                if !fragmented.is_empty() {
                    fragmented.extend_from_slice(loa_packets);
                    combined_frag = fragmented;
                    fragmented = vec![];
                    loa_packets = &mut combined_frag[..];
                }
                if loa_packets.len() < 8 {
                    fragmented = loa_packets.to_vec();
                    continue 'inner;
                }

                let loa_packet_size = u16::from_ne_bytes(loa_packets[0..2].try_into()?);
                if loa_packets[7] != 1 || loa_packets.len() < 8 || loa_packet_size < 9 {
                    fragmented.clear();
                    continue 'inner;
                }
                if loa_packet_size as usize > loa_packets.len() {
                    fragmented = loa_packets.to_vec();
                    continue 'inner;
                }

                if loa_packets.len() < loa_packet_size as usize {
                    continue 'inner;
                }

                match parse_loa_packet(
                    &mut handler,
                    &mut oodle,
                    &mut loa_packets[..loa_packet_size as usize],
                    &mut unpacked_buf,
                    &mut bump,
                ) {
                    Ok(_) => {}
                    Err(e) => eprintln!("{:#}", e),
                }

                loa_packets = &mut loa_packets[loa_packet_size as usize..];
                bump.reset();
            }
        }
    }
}

// Parse, but append additional context in case of failure
fn parse_with_context<'bump, T>(
    parser: &mut Parser,
    bump: &'bump mut bumpalo::Bump,
) -> anyhow::Result<T::Out>
where
    T: Event<'bump>,
{
    T::parse(parser, bump)
        .with_context(|| format!("{} failed to parse", std::any::type_name::<T>()))
}

// struct RawLog {
//     pov: Option<u64>,
//     data: Vec<u8>,
// }

fn parse_loa_packet<P: PacketHandler>(
    handler: &mut P,
    oodle: &mut OodleDecompressor,
    packet: &mut [u8],
    buf: &mut [u8],
    bump: &mut bumpalo::Bump,
) -> anyhow::Result<()> {
    let size = u16::from_ne_bytes(packet[0..2].try_into()?);
    let opcode_raw = u16::from_ne_bytes(packet[4..6].try_into()?);
    let opcode = match Opcode::from_u16(opcode_raw).filter(P::filter) {
        Some(opcode) => opcode,
        None => return Ok(()),
    };

    let compression_method = packet[6];
    let payload = &mut packet[8..size as usize];
    let mut cipher_seed = opcode_raw as usize;
    for byte in payload.iter_mut() {
        *byte ^= XOR_TABLE[cipher_seed % XOR_TABLE.len()];
        cipher_seed += 1;
    }

    let packet = match compression_method {
        3 => oodle
            .decompress(buf, payload)
            .with_context(|| format!("failed decompression: opcode {:?}", opcode))?,
        2 => {
            let mut decoder = snap::raw::Decoder::new();
            decoder.decompress(payload, buf)?;
            &buf[16..]
        }
        0 => &payload[16..],
        _ => anyhow::bail!(
            "compression method unimplemented ({compression_method}): opcode {:?}",
            opcode
        ),
    };

    let mut parser = Parser::new(packet);
    match opcode {
        Opcode::RaidBossKillNotify => {
            let pkt = parse_with_context::<packet::PktRaidBossKillNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_raid_boss_kill_notify(pkt)?;
        }
        Opcode::NewPc => {
            let pkt = parse_with_context::<packet::PktNewPc>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_new_pc(pkt)?;
        }
        Opcode::SkillDamageAbnormalMoveNotify => {
            let pkt =
                parse_with_context::<packet::PktSkillDamageAbnormalMoveNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_skill_damage_abnormal_move_notify(pkt)?;
        }
        Opcode::AddonSkillFeatureChangeNotify => {
            let pkt =
                parse_with_context::<packet::PktAddonSkillFeatureChangeNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_addon_skill_feature_change_notify(pkt)?;
        }
        Opcode::StatusEffectDurationNotify => {
            let pkt =
                parse_with_context::<packet::PktStatusEffectDurationNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_status_effect_duration_notify(pkt)?;
        }
        Opcode::PassiveStatusEffectRemoveNotify => {
            let pkt = parse_with_context::<packet::PktPassiveStatusEffectRemoveNotify>(
                &mut parser,
                bump,
            )?;
            handler.on_packet(&pkt);
            handler.on_passive_status_effect_remove_notify(pkt)?;
        }
        Opcode::StatusEffectRemoveNotify => {
            let pkt = parse_with_context::<packet::PktStatusEffectRemoveNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_status_effect_remove_notify(pkt)?;
        }
        Opcode::StatusEffectSyncDataNotify => {
            let pkt =
                parse_with_context::<packet::PktStatusEffectSyncDataNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_status_effect_sync_data_notify(pkt)?;
        }
        Opcode::TroopMemberUpdateMinNotify => {
            let pkt =
                parse_with_context::<packet::PktTroopMemberUpdateMinNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_troop_member_update_min_notify(pkt)?;
        }
        Opcode::InitItem => {
            let pkt = parse_with_context::<packet::PktInitItem>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_init_item(pkt)?;
        }
        Opcode::ActiveAbilityNotify => {
            let pkt = parse_with_context::<packet::PktActiveAbilityNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_active_ability_notify(pkt)?;
        }
        Opcode::SkillStageNotify => {
            let pkt = parse_with_context::<packet::PktSkillStageNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_skill_stage_notify(pkt)?;
        }
        Opcode::StatusEffectAddNotify => {
            let pkt = parse_with_context::<packet::PktStatusEffectAddNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_status_effect_add_notify(pkt)?;
        }
        Opcode::NewNpc => {
            let pkt = parse_with_context::<packet::PktNewNpc>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_new_npc(pkt)?;
        }
        Opcode::DeathNotify => {
            let pkt = parse_with_context::<packet::PktDeathNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_death_notify(pkt)?;
        }
        Opcode::InitPc => {
            let pkt = parse_with_context::<packet::PktInitPc>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_init_pc(pkt)?;
        }
        Opcode::IdentityStanceChangeNotify => {
            let pkt =
                parse_with_context::<packet::PktIdentityStanceChangeNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_identity_stance_change_notify(pkt)?;
        }
        Opcode::SkillDamageNotify => {
            let pkt = parse_with_context::<packet::PktSkillDamageNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_skill_damage_notify(pkt)?;
        }
        Opcode::ParalyzationStateNotify => {
            let pkt = parse_with_context::<packet::PktParalyzationStateNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_paralyzation_state_notify(pkt)?;
        }
        Opcode::EquipLifeToolChangeNotify => {
            let pkt =
                parse_with_context::<packet::PktEquipLifeToolChangeNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_equip_life_tool_change_notify(pkt)?;
        }
        Opcode::AuthTokenResult => {
            let pkt = parse_with_context::<packet::PktAuthTokenResult>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_auth_token_result(pkt)?;
        }
        Opcode::CounterAttackNotify => {
            let pkt = parse_with_context::<packet::PktCounterAttackNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_counter_attack_notify(pkt)?;
        }
        Opcode::TriggerBossBattleStatus => {
            let pkt = parse_with_context::<packet::PktTriggerBossBattleStatus>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_trigger_boss_battle_status(pkt)?;
        }
        Opcode::PartyStatusEffectAddNotify => {
            let pkt =
                parse_with_context::<packet::PktPartyStatusEffectAddNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_party_status_effect_add_notify(pkt)?;
        }
        Opcode::InitAbility => {
            let pkt = parse_with_context::<packet::PktInitAbility>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_init_ability(pkt)?;
        }
        Opcode::SkillCastNotify => {
            let pkt = parse_with_context::<packet::PktSkillCastNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_skill_cast_notify(pkt)?;
        }
        Opcode::NewTrap => {
            let pkt = parse_with_context::<packet::PktNewTrap>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_new_trap(pkt)?;
        }
        Opcode::BlockSkillStateNotify => {
            let pkt = parse_with_context::<packet::PktBlockSkillStateNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_block_skill_state_notify(pkt)?;
        }
        Opcode::NewNpcSummon => {
            let pkt = parse_with_context::<packet::PktNewNpcSummon>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_new_npc_summon(pkt)?;
        }
        Opcode::PartyStatusEffectResultNotify => {
            let pkt =
                parse_with_context::<packet::PktPartyStatusEffectResultNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_party_status_effect_result_notify(pkt)?;
        }
        Opcode::ZoneStatusEffectAddNotify => {
            let pkt =
                parse_with_context::<packet::PktZoneStatusEffectAddNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_zone_status_effect_add_notify(pkt)?;
        }
        Opcode::ZoneObjectUnpublishNotify => {
            let pkt =
                parse_with_context::<packet::PktZoneObjectUnpublishNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_zone_object_unpublish_notify(pkt)?;
        }
        Opcode::InitEnv => {
            let pkt = parse_with_context::<packet::PktInitEnv>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_init_env(pkt)?;
        }
        Opcode::IdentityGaugeChangeNotify => {
            let pkt =
                parse_with_context::<packet::PktIdentityGaugeChangeNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_identity_gauge_change_notify(pkt)?;
        }
        Opcode::SkillStartNotify => {
            let pkt = parse_with_context::<packet::PktSkillStartNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_skill_start_notify(pkt)?;
        }
        Opcode::InitLocal => {
            let pkt = parse_with_context::<packet::PktInitLocal>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_init_local(pkt)?;
        }
        Opcode::PartyLeaveResult => {
            let pkt = parse_with_context::<packet::PktPartyLeaveResult>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_party_leave_result(pkt)?;
        }
        Opcode::PassiveStatusEffectAddNotify => {
            let pkt =
                parse_with_context::<packet::PktPassiveStatusEffectAddNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_passive_status_effect_add_notify(pkt)?;
        }
        Opcode::PartyPassiveStatusEffectAddNotify => {
            let pkt = parse_with_context::<packet::PktPartyPassiveStatusEffectAddNotify>(
                &mut parser,
                bump,
            )?;
            handler.on_packet(&pkt);
            handler.on_party_passive_status_effect_add_notify(pkt)?;
        }
        Opcode::PartyInfo => {
            let pkt = parse_with_context::<packet::PktPartyInfo>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_party_info(pkt)?;
        }
        Opcode::TriggerFinishNotify => {
            let pkt = parse_with_context::<packet::PktTriggerFinishNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_trigger_finish_notify(pkt)?;
        }
        Opcode::PartyStatusEffectRemoveNotify => {
            let pkt =
                parse_with_context::<packet::PktPartyStatusEffectRemoveNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_party_status_effect_remove_notify(pkt)?;
        }
        Opcode::TriggerStartNotify => {
            let pkt = parse_with_context::<packet::PktTriggerStartNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_trigger_start_notify(pkt)?;
        }
        Opcode::ZoneMemberLoadStatusNotify => {
            let pkt =
                parse_with_context::<packet::PktZoneMemberLoadStatusNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_zone_member_load_status_notify(pkt)?;
        }
        Opcode::NewProjectile => {
            let pkt = parse_with_context::<packet::PktNewProjectile>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_new_projectile(pkt)?;
        }
        Opcode::ZoneStatusEffectRemoveNotify => {
            let pkt =
                parse_with_context::<packet::PktZoneStatusEffectRemoveNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_zone_status_effect_remove_notify(pkt)?;
        }
        Opcode::RemoveObject => {
            let pkt = parse_with_context::<packet::PktRemoveObject>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_remove_object(pkt)?;
        }
        Opcode::StatChangeOriginNotify => {
            let pkt = parse_with_context::<packet::PktStatChangeOriginNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_stat_change_origin_notify(pkt)?;
        }
        Opcode::PartyPassiveStatusEffectRemoveNotify => {
            let pkt = parse_with_context::<packet::PktPartyPassiveStatusEffectRemoveNotify>(
                &mut parser,
                bump,
            )?;
            handler.on_packet(&pkt);
            handler.on_party_passive_status_effect_remove_notify(pkt)?;
        }
        Opcode::RaidResult => {
            let pkt = parse_with_context::<packet::PktRaidResult>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_raid_result(pkt)?;
        }
        Opcode::AbilityChangeNotify => {
            let pkt = parse_with_context::<packet::PktAbilityChangeNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_ability_change_notify(pkt)?;
        }
        Opcode::MigrationExecute => {
            let pkt = parse_with_context::<packet::PktMigrationExecute>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_migration_execute(pkt)?;
        }
        Opcode::EquipChangeNotify => {
            let pkt = parse_with_context::<packet::PktEquipChangeNotify>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_equip_change_notify(pkt)?;
        }
        Opcode::RaidBegin => {
            let pkt = parse_with_context::<packet::PktRaidBegin>(&mut parser, bump)?;
            handler.on_packet(&pkt);
            handler.on_raid_begin(pkt)?;
        }
    }
    Ok(())
}

#[rustfmt::skip]
pub trait PacketHandler {
    fn on_raid_boss_kill_notify(&mut self, _: packet::PktRaidBossKillNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_new_pc(&mut self, _: packet::PktNewPc) -> anyhow::Result<()> { Ok(()) }
    fn on_skill_damage_abnormal_move_notify(&mut self, _: packet::PktSkillDamageAbnormalMoveNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_addon_skill_feature_change_notify(&mut self, _: packet::PktAddonSkillFeatureChangeNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_status_effect_duration_notify(&mut self, _: packet::PktStatusEffectDurationNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_passive_status_effect_remove_notify(&mut self, _: packet::PktPassiveStatusEffectRemoveNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_status_effect_remove_notify(&mut self, _: packet::PktStatusEffectRemoveNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_status_effect_sync_data_notify(&mut self, _: packet::PktStatusEffectSyncDataNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_troop_member_update_min_notify(&mut self, _: packet::PktTroopMemberUpdateMinNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_init_item(&mut self, _: packet::PktInitItem) -> anyhow::Result<()> { Ok(()) }
    fn on_active_ability_notify(&mut self, _: packet::PktActiveAbilityNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_skill_stage_notify(&mut self, _: packet::PktSkillStageNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_status_effect_add_notify(&mut self, _: packet::PktStatusEffectAddNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_new_npc(&mut self, _: packet::PktNewNpc) -> anyhow::Result<()> { Ok(()) }
    fn on_death_notify(&mut self, _: packet::PktDeathNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_init_pc(&mut self, _: packet::PktInitPc) -> anyhow::Result<()> { Ok(()) }
    fn on_identity_stance_change_notify(&mut self, _: packet::PktIdentityStanceChangeNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_skill_damage_notify(&mut self, _: packet::PktSkillDamageNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_paralyzation_state_notify(&mut self, _: packet::PktParalyzationStateNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_equip_life_tool_change_notify(&mut self, _: packet::PktEquipLifeToolChangeNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_auth_token_result(&mut self, _: packet::PktAuthTokenResult) -> anyhow::Result<()> { Ok(()) }
    fn on_counter_attack_notify(&mut self, _: packet::PktCounterAttackNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_trigger_boss_battle_status(&mut self, _: packet::PktTriggerBossBattleStatus) -> anyhow::Result<()> { Ok(()) }
    fn on_party_status_effect_add_notify(&mut self, _: packet::PktPartyStatusEffectAddNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_init_ability(&mut self, _: packet::PktInitAbility) -> anyhow::Result<()> { Ok(()) }
    fn on_skill_cast_notify(&mut self, _: packet::PktSkillCastNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_new_trap(&mut self, _: packet::PktNewTrap) -> anyhow::Result<()> { Ok(()) }
    fn on_block_skill_state_notify(&mut self, _: packet::PktBlockSkillStateNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_new_npc_summon(&mut self, _: packet::PktNewNpcSummon) -> anyhow::Result<()> { Ok(()) }
    fn on_party_status_effect_result_notify(&mut self, _: packet::PktPartyStatusEffectResultNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_zone_status_effect_add_notify(&mut self, _: packet::PktZoneStatusEffectAddNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_zone_object_unpublish_notify(&mut self, _: packet::PktZoneObjectUnpublishNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_init_env(&mut self, _: packet::PktInitEnv) -> anyhow::Result<()> { Ok(()) }
    fn on_identity_gauge_change_notify(&mut self, _: packet::PktIdentityGaugeChangeNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_skill_start_notify(&mut self, _: packet::PktSkillStartNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_init_local(&mut self, _: packet::PktInitLocal) -> anyhow::Result<()> { Ok(()) }
    fn on_party_leave_result(&mut self, _: packet::PktPartyLeaveResult) -> anyhow::Result<()> { Ok(()) }
    fn on_passive_status_effect_add_notify(&mut self, _: packet::PktPassiveStatusEffectAddNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_party_passive_status_effect_add_notify(&mut self, _: packet::PktPartyPassiveStatusEffectAddNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_party_info(&mut self, _: packet::PktPartyInfo) -> anyhow::Result<()> { Ok(()) }
    fn on_trigger_finish_notify(&mut self, _: packet::PktTriggerFinishNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_party_status_effect_remove_notify(&mut self, _: packet::PktPartyStatusEffectRemoveNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_trigger_start_notify(&mut self, _: packet::PktTriggerStartNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_zone_member_load_status_notify(&mut self, _: packet::PktZoneMemberLoadStatusNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_new_projectile(&mut self, _: packet::PktNewProjectile) -> anyhow::Result<()> { Ok(()) }
    fn on_zone_status_effect_remove_notify(&mut self, _: packet::PktZoneStatusEffectRemoveNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_remove_object(&mut self, _: packet::PktRemoveObject) -> anyhow::Result<()> { Ok(()) }
    fn on_stat_change_origin_notify(&mut self, _: packet::PktStatChangeOriginNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_party_passive_status_effect_remove_notify(&mut self, _: packet::PktPartyPassiveStatusEffectRemoveNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_raid_result(&mut self, _: packet::PktRaidResult) -> anyhow::Result<()> { Ok(()) }
    fn on_ability_change_notify(&mut self, _: packet::PktAbilityChangeNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_migration_execute(&mut self, _: packet::PktMigrationExecute) -> anyhow::Result<()> { Ok(()) }
    fn on_equip_change_notify(&mut self, _: packet::PktEquipChangeNotify) -> anyhow::Result<()> { Ok(()) }
    fn on_raid_begin(&mut self, _: packet::PktRaidBegin) -> anyhow::Result<()> { Ok(()) }

    fn on_packet<P>(&mut self, _: &P) where P: Packet + serde::Serialize {}

    /// Used to filter out unnecessary opcodes before parsing.
    fn filter(_: &Opcode) -> bool {
        true
    }
}
