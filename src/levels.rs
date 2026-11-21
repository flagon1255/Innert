//! Levels: the player's, earned by experience, and those of the monsters and bosses they fight and
//! the weapons they find. Monsters and bosses everywhere come at the player's level, see
//! foe_level(); each realm's band of levels, see BANDS, is only the levels it's meant to be passed
//! through at. What a monster can take and how hard it hits come from its level, see
//! toughness() and damage(), and a weapon hits harder for its item level, see weapon_power().
//!
//! Everything here is plain numbers, kept apart from the game, so it can be tested and tuned on
//! its own, see realm_balance in game.rs.

/// The highest level the player can reach
pub const LEVEL_CAP: u32 = 50;
/// The highest item level a weapon can have, with every jewel set into it, and the highest one
/// can drop at: the cap and the best roll over it, see DROP_OFFSETS
pub const ITEM_CAP: u32 = 55;
pub const DROP_CAP: u32 = LEVEL_CAP + 3;

/// The levels the monsters of a realm keep to, lowest and highest
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Band {
    pub lo: u32,
    pub hi: u32,
}

impl Band {
    /// Free Roam, which has no realms: everything comes at the player's level, whatever it is
    pub const ANY: Band = Band { lo: 1, hi: LEVEL_CAP };

    /// The level of what's here, with the player at level `player`: theirs, wherever it is. The
    /// band is only where a realm's levels are meant to be passed through, for the realm's name
    /// and the balance printout; monsters everywhere come at the player's level.
    pub fn level_for(self, player: u32) -> u32 {
        player.max(1)
    }
}

/// Each realm's band, in the order of realms::REALMS: the normal world, the sky isles, the
/// dreaming red, the rimelands, the sunken reach and the cinder crown
pub const BANDS: [Band; 6] = [
    Band { lo: 1, hi: 8 },
    Band { lo: 9, hi: 16 },
    Band { lo: 17, hi: 24 },
    Band { lo: 25, hi: 32 },
    Band { lo: 33, hi: 41 },
    Band { lo: 42, hi: 50 },
];

/// How much of the way to the band of the realm furthest on that's been reached the bands of the
/// realms gone past are lifted, the normal world among them: each new realm reached lifts them a
/// step, so going home's never trivial, never as hard as that realm itself
pub const REACHED_SHARE: f64 = 0.3;

/// The band of realm `realm` with the journey come as far as realm `reached`: its own, lifted part
/// of the way to that of the realm reached, see REACHED_SHARE
pub fn band_in(realm: usize, reached: usize) -> Band {
    let own = BANDS[realm.min(BANDS.len() - 1)];
    let top = BANDS[reached.min(BANDS.len() - 1)];
    let toward = |a: u32, b: u32| if b > a { a + ((b - a) as f64 * REACHED_SHARE).round() as u32 } else { a };
    Band { lo: toward(own.lo, top.lo), hi: toward(own.hi, top.hi) }
}

/// How many times its own health a monster or boss of level `level` has: at each realm's floor
/// what that realm's monsters had before there were levels, rising between them and on past the
/// cap the way it rises to it
pub fn toughness(level: u32) -> f64 {
    const AT: [(f64, f64); 7] = [(1.0, 1.0), (9.0, 1.9), (17.0, 2.0), (25.0, 2.6), (33.0, 2.8), (42.0, 3.2), (50.0, 3.6)];
    let l = level.max(1) as f64;
    let k = AT.windows(2).position(|w| l <= w[1].0).unwrap_or(AT.len() - 2);
    let ((x0, y0), (x1, y1)) = (AT[k], AT[k + 1]);
    y0 + (y1 - y0) * (l - x0) / (x1 - x0)
}

/// What an ordinary monster has of its own health, before its level's toughness, see
/// Look::health(): each kind as much as this for what it is, a swarmer's a little under and a
/// brute's a little over, within MONSTER_SPREAD of it, so monsters of a level are about as hard to
/// kill whatever realm they're from, see realm_balance in game.rs
#[allow(dead_code)]
pub const MONSTER_HEALTH: f64 = 175.0;
#[allow(dead_code)]
pub const MONSTER_SPREAD: [f64; 2] = [0.68, 1.4];
/// The same for a lesser boss, a guardian among them, and for a raid boss; those of a realm's
/// three that aren't raid bosses already are raised to about as much, see realms::RAID_PROMOTION,
/// and its finale far past it, see realms::FINALE_PROMOTION
#[allow(dead_code)]
pub const BOSS_HEALTH: f64 = 2600.0;
#[allow(dead_code)]
pub const BOSS_SPREAD: [f64; 2] = [0.9, 1.16];
#[allow(dead_code)]
pub const RAID_HEALTH: f64 = 14500.0;
#[allow(dead_code)]
pub const RAID_SPREAD: [f64; 2] = [0.93, 1.07];

/// How many times harder than its own a monster or boss of level `level` hits: a fifth more every
/// realm's floor on
pub fn damage(level: u32) -> f64 {
    1.0 + 0.025 * (level.max(1) - 1) as f64
}

/// How many times harder a weapon of item level `level` hits than one of level 1. As the square
/// root of how much tougher a monster of that level is: the realms further on give rarer weapons
/// too, see game::loot_scaling(), and between the two a weapon of a realm's level kills its
/// monsters about as quickly in every realm, see realm_balance in game.rs
pub fn weapon_power(level: u32) -> f64 {
    toughness(level).sqrt()
}

/// The player's health at full at level `level`, before anything worn or learnt
pub fn max_health(level: u32) -> f64 {
    100.0 + (level.max(1) - 1) as f64
}

/// Experience it takes to go from level `level` to the next: a little more each level
pub fn needed(level: u32) -> f64 {
    100.0 + 20.0 * (level.max(1) - 1) as f64
}

/// Experience it takes to reach level `level` from the first
#[allow(dead_code)]
pub fn total_to(level: u32) -> f64 {
    (1..level.max(1)).map(needed).sum()
}

/// What something slain was, for the experience it gives
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slain {
    Monster,
    /// A lesser boss, a guardian among them
    Boss,
    /// One of a realm's raid bosses, or a huge raid boss
    Raid,
    /// A realm's finale, or an ultra
    Finale,
}

/// How many kills of monsters at their own level it takes to go up a level
const KILLS_A_LEVEL: f64 = 30.0;

/// Experience for slaying something of level `level`: a monster's a 30th of what that level takes,
/// a boss's eight times that, a raid boss's thirty and a finale's sixty
pub fn experience(level: u32, slain: Slain) -> f64 {
    let times = match slain {
        Slain::Monster => 1.0,
        Slain::Boss => 8.0,
        Slain::Raid => 30.0,
        Slain::Finale => 60.0,
    };
    (needed(level) / KILLS_A_LEVEL * times).round().max(1.0)
}

/// Experience for coming to a realm for the first time: half a level's worth at its floor
pub fn first_visit(realm: usize) -> f64 {
    (needed(BANDS[realm.min(BANDS.len() - 1)].lo) * 0.5).round()
}

/// How a weapon's item level is rolled from the level of what gave it: from one under to three
/// over, near it most of the time and three over now and then, for a lucky find. Those of bosses'
/// hoards lean to the top, raid bosses' and finales' the most, and merchants' keep to within one.
pub const DROP_OFFSETS: [i32; 5] = [-1, 0, 1, 2, 3];
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    /// A monster, a chest, a bandits' hoard
    Found,
    /// A lesser boss's hoard
    Boss,
    /// A raid boss's or a finale's
    Raid,
    /// A merchant's stall
    Merchant,
}

/// How likely each of DROP_OFFSETS is for `source`
pub fn offset_weights(source: Source) -> [f64; 5] {
    match source {
        Source::Found => [0.20, 0.35, 0.25, 0.13, 0.07],
        Source::Boss => [0.05, 0.20, 0.32, 0.27, 0.16],
        Source::Raid => [0.0, 0.10, 0.25, 0.35, 0.30],
        Source::Merchant => [0.30, 0.45, 0.25, 0.0, 0.0],
    }
}

/// The item level of a weapon from something of level `level`, by `roll`, 0-1: see DROP_OFFSETS,
/// never under 1 nor over DROP_CAP
pub fn drop_level(level: u32, source: Source, roll: f64) -> u32 {
    let weights = offset_weights(source);
    let mut left = roll.clamp(0.0, 0.999_999) * weights.iter().sum::<f64>();
    let mut offset = DROP_OFFSETS[DROP_OFFSETS.len() - 1];
    for (k, w) in weights.iter().enumerate() {
        if left < *w {
            offset = DROP_OFFSETS[k];
            break;
        }
        left -= w;
    }
    (level as i32 + offset).clamp(1, DROP_CAP as i32) as u32
}

/// The highest level a monster or boss can be: a raid boss's at the player's cap
pub const FOE_CAP: u32 = LEVEL_CAP + 3;

/// The level of a monster or boss with the player at level `player`, by `roll`, 0-1, wherever it
/// is: a monster one either side of the player's level, a boss one or two over, a raid boss or
/// finale three over
pub fn foe_level(band: Band, player: u32, slain: Slain, roll: f64) -> u32 {
    let at = band.level_for(player) as i32;
    let level = match slain {
        Slain::Monster => at + (roll * 3.0).floor().min(2.0) as i32 - 1,
        Slain::Boss => at + 1 + (roll * 2.0).floor().min(1.0) as i32,
        Slain::Raid | Slain::Finale => at + 3,
    };
    level.clamp(1, FOE_CAP as i32) as u32
}

/// The level a player from a save from before there were levels starts at: the floor of the band
/// of the realm furthest on they've reached
pub fn legacy_level(reached: usize) -> u32 {
    BANDS[reached.min(BANDS.len() - 1)].lo
}

/// Skill points for reaching level `level`: one a level past the first
#[allow(dead_code)]
pub fn points_for(level: u32) -> u32 {
    level.max(1) - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_follow_on_and_cover_every_level() {
        assert_eq!(BANDS[0].lo, 1);
        assert_eq!(BANDS[BANDS.len() - 1].hi, LEVEL_CAP);
        for pair in BANDS.windows(2) {
            assert_eq!(pair[1].lo, pair[0].hi + 1);
        }
        // Wherever it is, a monster's within one of the player's level, a boss over it, a raid boss
        // three over
        for band in BANDS {
            for player in 1..=LEVEL_CAP {
                for k in 0..10 {
                    let roll = k as f64 / 10.0;
                    for slain in [Slain::Monster, Slain::Boss, Slain::Raid, Slain::Finale] {
                        let l = foe_level(band, player, slain, roll);
                        assert!((1..=FOE_CAP).contains(&l) && l + 1 >= player, "{band:?} {player} {slain:?} at {l}");
                        assert_eq!(l, foe_level(BANDS[0], player, slain, roll), "the same in every realm");
                    }
                    assert!(foe_level(band, player, Slain::Boss, roll) >= foe_level(band, player, Slain::Monster, roll));
                }
                assert_eq!(foe_level(band, player, Slain::Raid, 0.5), player + 3);
            }
        }
    }

    /// Each realm reached lifts the bands of those gone past, a step at a time, never as high as
    /// its own; a realm's no higher for being reached itself
    #[test]
    fn going_home_is_never_trivial() {
        for realm in 0..BANDS.len() {
            assert_eq!(band_in(realm, realm), BANDS[realm]);
            assert_eq!(band_in(realm, 0), BANDS[realm]);
            let mut last = BANDS[realm];
            for reached in realm + 1..BANDS.len() {
                let b = band_in(realm, reached);
                assert!(b.lo > last.lo && b.hi > last.hi, "{realm} with {reached}");
                assert!(b.lo < BANDS[reached].lo && b.hi < BANDS[reached].hi);
                last = b;
            }
        }
    }

    #[test]
    fn tougher_and_harder_hitting_with_every_level() {
        for l in 1..=ITEM_CAP {
            assert!(toughness(l + 1) > toughness(l) && damage(l + 1) > damage(l) && weapon_power(l + 1) > weapon_power(l), "{l}");
            assert!(needed(l + 1) > needed(l));
        }
        assert_eq!(toughness(1), 1.0);
        assert!((toughness(42) - 3.2).abs() < 1e-9 && (toughness(9) - 1.9).abs() < 1e-9);
        assert!((damage(42) - 2.025).abs() < 1e-9);
        assert_eq!(total_to(1), 0.0);
        assert_eq!(total_to(3), needed(1) + needed(2));
    }

    /// Dropped weapons are one under to three over what gave them, near it most of the time,
    /// never past DROP_CAP; a merchant's within one, and bosses' lean high
    #[test]
    fn drops_are_rolled_round_their_source() {
        let spread = |level: u32, source: Source| {
            let mut counts = [0usize; 5];
            for k in 0..10_000 {
                let l = drop_level(level, source, k as f64 / 10_000.0);
                counts[(l as i32 - level as i32 + 1) as usize] += 1;
            }
            counts
        };
        let found = spread(5, Source::Found);
        assert!(found[1] > found[3] && found[3] > found[4] && found[4] > 0 && found[0] > 0, "{found:?}");
        assert!((found[1] as f64 / 10_000.0 - 0.35).abs() < 0.01);
        let merchant = spread(20, Source::Merchant);
        assert!(merchant[3] == 0 && merchant[4] == 0);
        let mean = |c: [usize; 5]| c.iter().enumerate().map(|(k, n)| (k as f64 - 1.0) * *n as f64).sum::<f64>() / 10_000.0;
        assert!(mean(spread(5, Source::Raid)) > mean(spread(5, Source::Boss)) && mean(spread(5, Source::Boss)) > mean(found));
        assert_eq!(drop_level(1, Source::Found, 0.0), 1);
        assert_eq!(drop_level(LEVEL_CAP, Source::Found, 0.9999), DROP_CAP);
        assert_eq!(drop_level(DROP_CAP, Source::Raid, 0.9999), DROP_CAP);
    }

    #[test]
    fn slaying_bigger_foes_gives_more() {
        for l in 1..LEVEL_CAP {
            assert!(experience(l, Slain::Monster) <= experience(l + 1, Slain::Monster));
            assert!(experience(l, Slain::Monster) < experience(l, Slain::Boss));
            assert!(experience(l, Slain::Boss) < experience(l, Slain::Raid));
            assert!(experience(l, Slain::Raid) < experience(l, Slain::Finale));
        }
        assert_eq!(legacy_level(0), 1);
        assert_eq!(legacy_level(5), 42);
    }
}
