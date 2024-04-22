use std::{
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

use larps::{
    definitions::Class,
    meter::{Data, Encounter, Environment, LiveData, Player, PlayerData},
    ui,
};

// Run UI without backend using mocked data.
pub fn main() -> anyhow::Result<()> {
    let environment = Environment::new()
        .with_player(0, Player::new("Berserker", Class::Berserker, 0.0))
        .with_player(1, Player::new("Destroyer", Class::Destroyer, 0.0))
        .with_player(2, Player::new("Gunlancer", Class::Gunlancer, 10.0))
        .with_player(3, Player::new("Paladin", Class::Paladin, 0.0))
        .with_player(4, Player::new("Slayer", Class::Slayer, 0.0))
        .with_player(5, Player::new("Arcanist", Class::Arcanist, 0.0))
        .with_player(6, Player::new("Summoner", Class::Summoner, 0.0))
        .with_player(7, Player::new("Bard", Class::Bard, 0.0))
        .with_player(8, Player::new("Sorceress", Class::Sorceress, 0.0))
        .with_player(9, Player::new("Wardancer", Class::Wardancer, 0.0))
        .with_player(10, Player::new("Scrapper", Class::Scrapper, 0.0))
        .with_player(11, Player::new("Soulfist", Class::Soulfist, 0.0))
        .with_player(12, Player::new("Glaivier", Class::Glaivier, 0.0))
        .with_player(13, Player::new("Striker", Class::Striker, 0.0))
        .with_player(14, Player::new("Deathblade", Class::Deathblade, 0.0))
        .with_player(15, Player::new("Shadowhunter", Class::Shadowhunter, 0.0))
        .with_player(16, Player::new("Reaper", Class::Reaper, 0.0))
        .with_player(17, Player::new("Sharpshooter", Class::Sharpshooter, 0.0))
        .with_player(18, Player::new("Deadeye", Class::Deadeye, 0.0))
        .with_player(19, Player::new("Artillerist", Class::Artillerist, 0.0))
        .with_player(20, Player::new("Scouter", Class::Scouter, 0.0))
        .with_player(21, Player::new("Gunslinger", Class::Gunslinger, 0.0))
        .with_player(22, Player::new("Artist", Class::Artist, 0.0))
        .with_player(23, Player::new("Aeromancer", Class::Aeromancer, 0.0));
    let players = environment
        .players
        .iter()
        .map(|_| PlayerData {
            dmg_dealt: 380459873948538,
            ..Default::default()
        })
        .into_iter()
        .enumerate()
        .map(|(x, y)| (x as u64, y))
        .collect();
    let data = Data {
        live: LiveData {
            tracked: [(
                1238342,
                larps::meter::BossInfo {
                    max_hp: 234234234,
                    cur_hp: 123123123,
                    bar_count: Some(160),
                },
            )]
            .into_iter()
            .collect(),
            recently_tracked: Some(1238342),
            ..Default::default()
        },
        environments: vec![environment],
        encounters: vec![Encounter {
            start: Instant::now() - Duration::from_secs(10),
            end: None,
            first_damage: Some(Instant::now()),
            last_damage: None,
            environment: 0,
            players,
            tracked: Vec::new(),
            wipe: false,
            clear: false,
        }],
    };

    let data = Arc::new(parking_lot::Mutex::new(data));
    let (ctx_oneshot_tx, _) = mpsc::channel();
    ui::run(ctx_oneshot_tx, data, 50)
}
