//! Demi-bosses: something worse than the monsters about, not so bad as a boss, waiting down at the
//! bottom of one in five of the open ravines, see worldgen\ravines.rs. Each is one of the monsters
//! grown big and strong down there in the dark: a grave brute, a ghoul, a cave spider or a skeleton
//! archer, drawn bigger and tinted, with a title of its own, some with a name, fighting as its kind
//! does, every blow and every arrow as its kind's. It has DEMI_HEALTH, about four ordinary monsters'
//! worth, a level between theirs and a boss's, see levels::Slain::Demi, and leaves more than a
//! monster does, not so much as a boss, see Game::demi_spoils(). Once beaten it stays beaten, kept in
//! the save as a boss's lair is.

use std::sync::LazyLock;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::animals::Look;
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

pub const KINDS: [Kind; 4] = [
    // Hulking, a gravestone for a club, gone pale and cold down there
    Kind { look: Look::GraveBrute, grown: 1.3, skin: skin(70, 150, 160, 95), title: "{} the Deep-Dweller" },
    // Crouched in the dark, its claws thrown out as it leaps, red as raw meat
    Kind { look: Look::Ghoul, grown: 1.35, skin: skin(170, 35, 45, 110), title: "Chasm Lurker" },
    // A cave spider grown fat on whatever falls in
    Kind { look: Look::Spiderling, grown: 1.5, skin: skin(60, 30, 90, 120), title: "Chasm Widow" },
    // Shooting down whatever comes down to it
    Kind { look: Look::BoneArcher, grown: 1.3, skin: skin(120, 60, 170, 95), title: "{} the Hollow-Eyed" },
];

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
        _ => (mix(key) % KINDS.len() as u32) as u8,
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
        let mut seen = [0; KINDS.len()];
        for x in -40..40 {
            let key = [x * 137, 9000 - x * 71, 31];
            seen[kind_at(key) as usize] += 1;
            assert_eq!(name(kind_at(key), key), name(kind_at(key), key));
            assert!(!name(kind_at(key), key).contains("{}"));
        }
        assert!(seen.iter().all(|&n| n > 5), "{seen:?}");
        assert_eq!(name(0, [1, 2, 3]).split(' ').nth(1), Some("the"));
    }
}
