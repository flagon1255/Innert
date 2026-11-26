//! Bosses of the wild places that had none of their own: the deep water, the open sand, the ash
//! of the Cinder Crown and the high bare rock. The Tidewrack Kraken lies in the deep water of the
//! lakes and the sea, and the Grotto Squid in the flooded caves and the smaller lakes, see
//! kraken.rs; the Dune Wyrm bores through the desert and the Caldera Wyrm through the Cinder Crown,
//! see wyrms.rs; and the Skymane, a great maned lion on wings of feathers, keeps the towers and
//! colonnades up on the mountains' bare rock and the plains' high ruins, see src\dragons\skymane.rs.
//!
//! What's here is what each is to the rest of the game: its name, how hard it is to kill, what
//! it's made of, what it leaves and what its hoard's called.

use super::*;

/// Every one of them, in the order they're added among the looks
pub const LOOKS: [Look; 5] = [Look::Kraken, Look::GrottoSquid, Look::DuneWyrm, Look::CalderaWyrm, Look::Skymane];

/// Whether `look` is one of them
pub fn has(look: Look) -> bool {
    LOOKS.contains(&look)
}

/// Whether `m` is one of their own moves, played out in kraken.rs or wyrms.rs
pub fn owns(m: Move) -> bool {
    kraken::owns(m) || wyrms::owns(m)
}

pub fn name(look: Look) -> &'static str {
    match look {
        Look::Kraken => "Tidewrack Kraken",
        Look::GrottoSquid => "Grotto Squid",
        Look::DuneWyrm => "Dune Wyrm",
        Look::CalderaWyrm => "Caldera Wyrm",
        Look::Skymane => "Skymane",
        _ => "boss",
    }
}

/// How much damage it takes, as much as the lesser bosses of its kind and size, see
/// levels::BOSS_HEALTH: the kraken as the heaviest of them, the squid as a drake, the wyrms as the
/// great serpents and the Skymane as the Elder Cinder Wyrm's lesser kin
pub fn health(look: Look) -> f64 {
    match look {
        Look::Kraken => 3200.0,
        Look::GrottoSquid => 2200.0,
        Look::DuneWyrm => 2800.0,
        Look::CalderaWyrm => 3000.0,
        Look::Skymane => 2900.0,
        _ => 2600.0,
    }
}

/// What its attacks are made of: the void in the kraken's ink, the light of the deep caves in
/// the squid's, the wyrms' acid and fire, and the Skymane's holy light
pub fn element(look: Look) -> Option<Element> {
    match look {
        Look::Kraken => Some(Element::Void),
        Look::GrottoSquid => Some(Element::Arcane),
        Look::DuneWyrm => Some(Element::Poison),
        Look::CalderaWyrm => Some(Element::Fire),
        Look::Skymane => Some(Element::Holy),
        _ => None,
    }
}

/// The kinds of weapon it leaves most of the time, as weapon families' ids, see bosses::taste():
/// the kraken's harpoons and the spears and orbs of the drowned, the squid's harpoons and wands,
/// the Dune Wyrm's katars and halberds out of what it swallowed, the Caldera Wyrm's mauls and hand
/// cannons, and the Skymane's spears, greatswords, bows and talismans, a hero's
pub fn taste(look: Look) -> &'static [&'static str] {
    match look {
        Look::Kraken => &["harpoon", "spear", "naginata", "orb"],
        Look::GrottoSquid => &["harpoon", "dagger", "wand", "orb"],
        Look::DuneWyrm => &["katar", "halberd", "twinprod", "talisman"],
        Look::CalderaWyrm => &["maul", "handcannon", "bombs", "battlestaff"],
        Look::Skymane => &["spear", "greatsword", "bow", "talisman"],
        _ => &[],
    }
}

/// What its hoard's called, as it spills out of it, see Game::reap()
pub fn hoard(look: Look) -> Option<&'static str> {
    Some(match look {
        Look::Kraken => "drowned hoard",
        Look::GrottoSquid => "pearl-hoard",
        Look::DuneWyrm => "gullet-hoard",
        Look::CalderaWyrm => "slag-gullet hoard",
        Look::Skymane => "crest-hoard",
        _ => return None,
    })
}

/// Seconds each of their own moves takes to get ready for, to make and to get over, and how long
/// till it can be made again
pub fn timing(look: Look, m: Move) -> (f64, f64, f64, f64) {
    if kraken::owns(m) { kraken::timing(look, m) } else { wyrms::timing(look, m) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each is a boss with a name and an element of its own, leaving weapons of families there are,
    /// with a hoard of its own, and makes three moves at least of its own no boss or monster before
    /// it made: the kraken and the wyrms here in bosses.rs, the Skymane as the dragons do
    #[test]
    fn each_has_moves_of_its_own() {
        let library = crate::weapons::Library::load();
        let older: Vec<Move> = Look::ALL.into_iter().filter(|l| l.build().is_some() && !l.is_dragon() && !l.is_winged() && !has(*l)).flat_map(repertoire).collect();
        let mut names = Vec::new();
        for look in LOOKS {
            assert!(look.is_boss() && !look.is_monster(), "{look:?}");
            assert!(look.element().is_some() && hoard(look).is_some(), "{look:?}");
            assert!(!names.contains(&look.name()) && Look::ALL.iter().filter(|l| l.name() == look.name()).count() == 1, "{look:?}");
            names.push(look.name());
            for family in taste(look) {
                assert!(library.families.iter().any(|f| f.id == *family), "{look:?} leaves {family}, which isn't a family");
            }
            if look == Look::Skymane {
                assert!(crate::dragons::own_moves(look).len() >= 3);
                continue;
            }
            let own: Vec<Move> = repertoire(look).into_iter().filter(|m| !older.contains(m)).collect();
            assert!(own.len() >= 3 && own.iter().all(|&m| owns(m)), "{look:?}: {own:?}");
            assert!((3..=8).contains(&repertoire(look).len()), "{look:?}");
        }
    }

    /// Every one of them turns up somewhere it can be found: in place of another boss in the land
    /// it lives in, among a realm's guardians, or at a site of its own
    #[test]
    fn each_turns_up_somewhere() {
        use crate::worldgen::Land;
        let bases = [Look::Golem, Look::Drake, Look::Ironhide, Look::Treant, Look::Direwolf, Look::Spider, Look::Ogre, Look::Colossus, Look::Elderhorn];
        let grounds = [GRASS, SNOW, SAND, JUNGLE_GRASS, STONE];
        let lands = [Land::Normal, Land::Dream, Land::Ice, Land::Dragon];
        for look in LOOKS {
            let in_land = bases.iter().any(|b| lands.iter().any(|&l| grounds.iter().any(|&g| b.for_land(l).for_ground(g) == look)));
            let in_realm = crate::realms::REALMS.iter().any(|r| r.raid_bosses.iter().any(|(l, _)| *l == look) || r.guardians.contains(&look));
            assert!(in_land || in_realm, "{look:?} is nowhere");
        }
        // And the kraken and the squid in Free Roam's waters, see serpent_lairs.rs
        assert_eq!(kraken::for_water(6.0, 0), Look::Kraken);
        assert_eq!(kraken::for_water(2.0, 0), Look::GrottoSquid);
    }

    /// Each is given a level when it turns up, as every boss is, a level or two over the player's,
    /// see levels::foe_level()
    #[test]
    fn each_is_given_a_level() {
        for look in LOOKS {
            // A lesser boss, levelled as one, see Game::foe_level_of()
            assert!(look.is_boss() && !look.is_raid() && !look.is_monster(), "{look:?}");
            let band = crate::levels::BANDS[1];
            let level = crate::levels::foe_level(band, band.lo + 2, crate::levels::Slain::Boss, 0.5);
            assert!(level > band.lo + 2 && level <= band.lo + 4, "{look:?} is level {level}");
        }
    }
}
