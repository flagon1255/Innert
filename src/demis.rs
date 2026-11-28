//! Demi-bosses: something worse than the monsters about, not so bad as a boss, waiting down at the
//! bottom of one in five of the open ravines, see worldgen\ravines.rs. Each is one of the monsters
//! grown big and strong down there in the dark: a grave brute, a ghoul, a cave spider or a skeleton
//! archer, drawn bigger and tinted, with a title of its own, some with a name, fighting as its kind
//! does, every blow and every arrow as its kind's. It has DEMI_HEALTH, about four ordinary monsters'
//! worth, a level between theirs and a boss's, see levels::Slain::Demi, and leaves more than a
//! monster does, not so much as a boss, see Game::demi_spoils(). Once beaten it stays beaten, kept in
//! the save as a boss's lair is.
//!
//! Four more grow where the land's their own, each with a move its kind doesn't have: in the
//! ravines of the desert a burrower that spits venom as it comes up out of the sand, in those of the
//! snow a wraith that calls the frost down round the player, in those of the jungle's swamps a lurker
//! that spits the bog's venom from further than its tongue reaches, and out in front of ruins
//! without a boss an armoured dead thing, spitting grave-rot and calling it down round the player.

use std::sync::LazyLock;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::animals::Look;
use crate::bosses::Move;
use crate::levels;

/// A kind of demi-boss: what it grew from, how much bigger it's drawn, the skin it's taken on, as
/// AnimalInstance::skinned() takes it, and its title, with `{}` where its name goes if it has one
pub struct Kind {
    pub look: Look,
    pub grown: f64,
    pub skin: u32,
    pub title: &'static str,
}

/// Red, green and blue and how far towards them, a byte each, little end first
const fn skin(r: u32, g: u32, b: u32, much: u32) -> u32 {
    r | g << 8 | b << 16 | much << 24
}

pub const KINDS: [Kind; 8] = [
    // Hulking, a gravestone for a club, gone pale and cold down there
    Kind { look: Look::GraveBrute, grown: 1.3, skin: skin(70, 150, 160, 95), title: "{} the Deep-Dweller" },
    // Crouched in the dark, its claws thrown out as it leaps, red as raw meat
    Kind { look: Look::Ghoul, grown: 1.35, skin: skin(170, 35, 45, 110), title: "Chasm Lurker" },
    // A cave spider grown fat on whatever falls in
    Kind { look: Look::Spiderling, grown: 1.5, skin: skin(60, 30, 90, 120), title: "Chasm Widow" },
    // Shooting down whatever comes down to it
    Kind { look: Look::BoneArcher, grown: 1.3, skin: skin(120, 60, 170, 95), title: "{} the Hollow-Eyed" },
    // Sun-bleached and gold-banded, spitting venom up out of the sand
    Kind { look: Look::DuneBurrower, grown: 1.5, skin: skin(215, 170, 70, 110), title: "{} the Sand-Drinker" },
    // Pale blue as the deep ice, the frost falling round it
    Kind { look: Look::RimeWraith, grown: 1.4, skin: skin(90, 170, 240, 120), title: "Rimeshade" },
    // Bloated and bog-green, the venom of the swamp in it
    Kind { look: Look::MireLurker, grown: 1.5, skin: skin(60, 140, 40, 110), title: "{} the Bog-Tyrant" },
    // Its old armour green with grave-rot, keeping the ruins
    Kind { look: Look::ArmoredZombie, grown: 1.45, skin: skin(110, 150, 60, 100), title: "{} the Barrow-Warden" },
];

/// How many of KINDS turn up in any ravine; those after are their places' own, see place()
const ANYWHERE: usize = 4;
/// Which of KINDS keep the desert's ravines, the snow's, the swamps' and the ruins
pub const SAND: u8 = 4;
pub const RIME: u8 = 5;
pub const BOG: u8 = 6;
pub const TOMB: u8 = 7;
/// How a lair's key says which kind keeps it, when it's one of a place's own: so many over its
/// height, see keyed()
const TAG: i64 = 1_000_000;

/// The key of a lair at `key` kept by kind `kind`, one of a place's own
pub fn keyed(key: [i64; 3], kind: u8) -> [i64; 3] {
    [key[0], key[1], key[2].rem_euclid(TAG) + (kind as i64 + 1) * TAG]
}

/// The key of the lair of the ravine at `key`, in a land that's `snow`, `desert` and `jungle` that
/// much, 0 to 1 each: kept by the land's own kind where it has one
pub fn ravine(key: [i64; 3], snow: f32, desert: f32, jungle: f32) -> [i64; 3] {
    if snow > 0.5 {
        keyed(key, RIME)
    } else if desert > 0.5 {
        keyed(key, SAND)
    } else if jungle > 0.5 {
        keyed(key, BOG)
    } else {
        key
    }
}

/// The moves a demi-boss of `look` makes that its kind doesn't, with those it does, as
/// bosses::Boss::menu() has them: its own first. `buried` for a burrower dug in.
pub fn menu(look: Look, buried: bool) -> Option<&'static [(Move, f64, f64)]> {
    Some(match look {
        Look::DuneBurrower if buried => &[(Move::Erupt, 0.0, 2.4)],
        Look::DuneBurrower => &[(Move::Slam, 0.0, 2.6), (Move::Spit, 4.0, 16.0), (Move::Burrow, 7.0, 40.0)],
        Look::RimeWraith => &[(Move::Blink, 0.0, 3.0), (Move::Storm, 6.0, 20.0), (Move::Slam, 0.0, 2.3), (Move::Spit, 4.5, 24.0)],
        Look::MireLurker => &[(Move::Slam, 0.0, 2.6), (Move::Lash, 3.0, 10.0), (Move::Spit, 4.0, 16.0)],
        Look::ArmoredZombie => &[(Move::Slam, 0.0, 2.4), (Move::Storm, 5.0, 18.0), (Move::Spit, 3.5, 18.0)],
        _ => return None,
    })
}

/// What a demi-boss has of health before its level's toughness: about four ordinary monsters'
pub const DEMI_HEALTH: f64 = 4.0 * levels::MONSTER_HEALTH;

/// The names some go by
const NAMES: [&str; 12] = ["Gorrak", "Vessa", "Murn", "Ulgrith", "Skarn", "Thessaly", "Orvek", "Draum", "Hollis", "Brakka", "Sereth", "Molg"];

/// The kind every demi-boss is, for trying each out, see force(); u8::MAX for each its own
static FORCED: AtomicU8 = AtomicU8::new(u8::MAX);

/// Every demi-boss of the kind grown from what's called `name`, the last word of what it grew from,
/// as in `--set demi=archer`, `brute`, `ghoul` or `spider`, for trying it out. False if there's none.
pub fn force(name: &str) -> bool {
    let found = KINDS.iter().position(|k| k.look.name().rsplit(' ').next().is_some_and(|w| w.eq_ignore_ascii_case(name)));
    if let Some(k) = found {
        FORCED.store(k as u8, Ordering::Relaxed);
    }
    found.is_some()
}

/// Which kind of demi-boss is down in the ravine whose lair's at `key`, see Game::set_demis()
pub fn kind_at(key: [i64; 3]) -> u8 {
    match FORCED.load(Ordering::Relaxed) {
        k if (k as usize) < KINDS.len() => k,
        _ => match key[2].div_euclid(TAG) {
            tag if (1..=KINDS.len() as i64).contains(&tag) => (tag - 1) as u8,
            _ => (mix(key) % ANYWHERE as u32) as u8,
        },
    }
}

/// Every kind's title with every name in it, made once
static NAMED: LazyLock<Vec<Vec<&'static str>>> = LazyLock::new(|| KINDS.iter().map(|k| NAMES.iter().map(|n| &*Box::leak(k.title.replace("{}", n).into_boxed_str())).collect()).collect());

/// What the demi-boss of kind `kind` whose lair's at `key` is called
pub fn name(kind: u8, key: [i64; 3]) -> &'static str {
    NAMED[kind as usize % KINDS.len()][(mix(key) >> 8) as usize % NAMES.len()]
}

fn mix(key: [i64; 3]) -> u32 {
    let mut h = 0x9e37_79b9u32;
    for v in key {
        h = (h ^ v as u32 ^ (v >> 32) as u32).wrapping_mul(0x85eb_ca6b);
        h ^= h >> 15;
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every kind's a monster that fights on its own feet, drawn bigger, tinted and titled, with
    /// three to five times what an ordinary monster has, and lairs turn up of every kind, each
    /// called the same every time
    #[test]
    fn demi_bosses_sit_between_monsters_and_bosses() {
        for k in &KINDS {
            assert!(k.look.is_monster() && !k.look.flies() && !crate::rig::rigged(k.look), "{:?}", k.look);
            assert!((1.2..=1.6).contains(&k.grown) && k.skin >> 24 > 0);
        }
        let times = DEMI_HEALTH / levels::MONSTER_HEALTH;
        assert!((3.0..=5.0).contains(&times) && DEMI_HEALTH < levels::BOSS_HEALTH * 0.5);
        let mut seen = [0; ANYWHERE];
        for x in -40..40 {
            let key = [x * 137, 9000 - x * 71, 31];
            seen[kind_at(key) as usize] += 1;
            assert_eq!(name(kind_at(key), key), name(kind_at(key), key));
            assert!(!name(kind_at(key), key).contains("{}"));
        }
        assert!(seen.iter().all(|&n| n > 5), "{seen:?}");
        // Each place's own kind where it's keyed so, and kept so
        for kind in [SAND, RIME, BOG, TOMB] {
            let key = keyed([311, -42, 655], kind);
            assert_eq!(kind_at(key), kind);
            assert_eq!(keyed(key, kind), key);
            assert!(menu(KINDS[kind as usize].look, false).is_some_and(|m| m.len() >= 3), "{kind}");
        }
        assert_eq!(kind_at(ravine([311, -42, 655], 0.9, 0.0, 0.0)), RIME);
        assert_eq!(kind_at(ravine([311, -42, 655], 0.0, 0.8, 0.0)), SAND);
        assert_eq!(kind_at(ravine([311, -42, 655], 0.0, 0.0, 0.7)), BOG);
        assert!((kind_at(ravine([311, -42, 655], 0.0, 0.0, 0.0)) as usize) < ANYWHERE);
        assert_eq!(name(0, [1, 2, 3]).split(' ').nth(1), Some("the"));
    }
}
