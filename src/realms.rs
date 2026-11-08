//! The realms of the journey played in Realms: the normal world it sets out from, the sky isles,
//! the dreaming realm half drowned in red, and the frozen realm. Each has three raid bosses, each of
//! whom leaves an Innert block when beaten. Set into the Innert Gate in the normal world, a realm's
//! three open the way to the next; all four realms' together restore them all. A portal at the
//! arrival point of each realm leads back to the gate.
//!
//! What each realm is, kept apart from how its land is made, as plain data: its raid bosses, the
//! monsters that roam it, how much tougher its foes are and how much better its loot, see Realm.
//! What the player's done on the journey is in the save, see save::RealmsSave, and where things
//! are in the realm they're in is worked out from the landscape, see Places.

use crate::animals::Look;
use crate::worldgen::{self, TerrainSettings};

/// One of the realms
pub struct Realm {
    /// What it's called in saves, see save::Save::worlds
    pub id: &'static str,
    pub name: &'static str,
    /// The landscape it's made of
    pub seed: u32,
    /// The three who each leave an Innert block, and about where their lairs are from where the
    /// realm's come to, in meters east and north: on the nearest dry land to there
    pub raid_bosses: [(Look, [f32; 2]); 3],
    /// The monsters that roam it, a pack at a time. Each works on any land, the normal world's
    /// too: those that fly come down to fight, for night raids from one realm into another.
    pub monsters: &'static [Pack],
    pub scaling: Scaling,
    /// The color of its rune on the gate, and its Innert blocks', in the HUD
    pub color: [u8; 3],
}

/// Monsters going about together: which, and how many such packs there are about at most. For
/// the night raids that bring a realm's monsters into another, see monsters_of().
#[allow(dead_code)]
pub struct Pack {
    pub looks: &'static [Look],
    pub most: usize,
}

/// How much harder a realm is than the normal world: how many times the health its bosses and
/// monsters have, how much harder they hit, and how much luckier the weapons found are, as for
/// Library::roll_rarity()
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Scaling {
    pub toughness: f64,
    pub damage: f64,
    pub luck: f64,
}

impl Scaling {
    /// The normal world as it is, and Free Roam
    pub const NONE: Scaling = Scaling { toughness: 1.0, damage: 1.0, luck: 1.0 };

    /// The same again, harder, each time the journey's been made to the end
    pub fn after(self, journeys: u32) -> Scaling {
        let more = 1.0 + 0.5 * journeys as f64;
        Scaling { toughness: self.toughness * more, damage: self.damage * (1.0 + 0.3 * journeys as f64), luck: self.luck * more }
    }
}

/// The landscape of the normal world in Realms: the one Innert starts in
pub const NORMAL_SEED: u32 = 1;
/// The landscapes of the realms past it. Free Roam's seeds are kept below these, see
/// FREE_ROAM_SEEDS, so the land made for a realm only turns up in it.
pub const SKY_SEED: u32 = 0xF100_0001;
pub const DREAM_SEED: u32 = 0xF200_0001;
pub const ICE_SEED: u32 = 0xF300_0001;
/// The most a Free Roam seed can be
pub const FREE_ROAM_SEEDS: u32 = 0xEFFF_FFFF;

/// How many times the health of a raid boss a realm's three of has, when it's not one of the huge
/// raid bosses already: as hard to beat as one of those
pub const RAID_PROMOTION: f64 = 5.0;

pub const REALMS: [Realm; 4] = [
    Realm {
        id: "normal",
        name: "The Normal World",
        seed: NORMAL_SEED,
        raid_bosses: [(Look::Colossus, [260.0, 140.0]), (Look::ElderWyrm, [-220.0, 280.0]), (Look::CryptTitan, [-140.0, -300.0])],
        monsters: &[
            Pack { looks: &[Look::Wolf, Look::Wolf, Look::Wolf], most: 3 },
            Pack { looks: &[Look::Bandit, Look::Crossbowman, Look::Shieldbearer, Look::BanditBrute], most: 2 },
            Pack { looks: &[Look::Skeleton, Look::Zombie, Look::ArmoredSkeleton], most: 2 },
            Pack { looks: &[Look::Tusker, Look::Tusker], most: 1 },
        ],
        scaling: Scaling::NONE,
        color: [255, 214, 120],
    },
    Realm {
        id: "sky",
        name: "The Sky Isles",
        seed: SKY_SEED,
        raid_bosses: [(Look::StormDrake, [240.0, 60.0]), (Look::Golem, [-60.0, 300.0]), (Look::Drake, [-260.0, -160.0])],
        monsters: &[
            Pack { looks: &[Look::Gloomwing, Look::Gloomwing, Look::Gloomwing, Look::Gloomwing], most: 3 },
            Pack { looks: &[Look::Crossbowman, Look::Crossbowman, Look::Shieldbearer], most: 2 },
            Pack { looks: &[Look::BoneArcher, Look::ArmoredSkeleton, Look::ArmoredSkeleton], most: 1 },
        ],
        scaling: Scaling { toughness: 1.6, damage: 1.35, luck: 1.8 },
        color: [140, 200, 255],
    },
    Realm {
        id: "dream",
        name: "The Dreaming Red",
        seed: DREAM_SEED,
        raid_bosses: [(Look::GraveKnight, [280.0, -40.0]), (Look::Spider, [-120.0, 280.0]), (Look::Treant, [-200.0, -240.0])],
        monsters: &[
            Pack { looks: &[Look::Ghoul, Look::Ghoul, Look::Bloater], most: 2 },
            Pack { looks: &[Look::Necromancer, Look::Zombie, Look::ArmoredZombie], most: 2 },
            Pack { looks: &[Look::GraveBrute, Look::BoneArcher, Look::BoneArcher], most: 1 },
            Pack { looks: &[Look::MireLurker, Look::MireLurker], most: 1 },
        ],
        scaling: Scaling { toughness: 2.4, damage: 1.75, luck: 2.8 },
        color: [235, 60, 70],
    },
    Realm {
        id: "ice",
        name: "The Rimelands",
        seed: ICE_SEED,
        raid_bosses: [(Look::FrostWyrm, [250.0, 180.0]), (Look::FrostGolem, [-240.0, 160.0]), (Look::WinterAlpha, [40.0, -310.0])],
        monsters: &[
            Pack { looks: &[Look::RimeWraith, Look::RimeWraith, Look::RimeWraith], most: 3 },
            Pack { looks: &[Look::Wolf, Look::Wolf, Look::Wolf, Look::Wolf], most: 2 },
            Pack { looks: &[Look::ArmoredSkeleton, Look::BoneArcher, Look::Skeleton], most: 1 },
        ],
        scaling: Scaling { toughness: 3.4, damage: 2.2, luck: 4.0 },
        color: [150, 235, 255],
    },
];

/// Raid bosses a realm has, and Innert blocks a realm's raid bosses leave
pub const RAIDS: usize = 3;

/// The realm with id `id`, by its number, as saves call them
#[allow(dead_code)]
pub fn by_id(id: &str) -> Option<usize> {
    REALMS.iter().position(|r| r.id == id)
}

/// The monsters of realm `realm`, for a night raid somewhere else: which go about together, and
/// how many such packs at most, as tough and as hard-hitting as in their own realm
#[allow(dead_code)]
pub fn monsters_of(realm: usize) -> (&'static [Pack], Scaling) {
    let r = &REALMS[realm.min(REALMS.len() - 1)];
    (r.monsters, r.scaling)
}

/// How far in front of the gate or a portal the player stands on coming to a realm, in meters: far
/// enough to see all of it
const ARRIVAL_M: f64 = 11.0;

/// What a raid boss of a realm's three is called in its key, see Places::lair_key(): its lair's
/// key's third number, as no lair in the landscape has one so low
const LAIR_KEY: i64 = -9000;

/// Where what matters is in the realm the player's in, in meters from the corner of its landscape:
/// where they come to it, the gate or the portal home, and the raid bosses' lairs
#[derive(Clone, Debug, PartialEq)]
pub struct Places {
    pub realm: usize,
    /// Where the player stands on coming to the realm, in front of the gate or the portal
    pub arrival: [f64; 2],
    /// The middle of the foot of the gate, or the portal, and which way its front faces
    pub gate: [f64; 2],
    pub facing: f64,
    pub lairs: [[f64; 2]; RAIDS],
}

impl Places {
    /// Where things are in realm `realm`, made of `settings`' landscape, which is come to round
    /// `start`, in meters, looking along `ahead`: the gate's just ahead there, on the nearest
    /// level dry land, facing back the way it's looked at from, and the raid bosses' lairs on
    /// the nearest dry land to where the realm says
    pub fn find(realm: usize, settings: &TerrainSettings, start: [f64; 2], ahead: [f64; 2]) -> Places {
        let r = &REALMS[realm];
        let dry = |near: [f64; 2], within: f32| worldgen::dry_land_near(settings, [near[0] as f32, near[1] as f32], within).map_or(near, |p| [p[0] as f64, p[1] as f64]);
        let gate = dry([start[0] + ahead[0] * 14.0, start[1] + ahead[1] * 14.0], 60.0);
        let back = [start[0] - gate[0], start[1] - gate[1]];
        let length = back[0].hypot(back[1]).max(1e-6);
        let facing = back[1].atan2(back[0]);
        let arrival = [gate[0] + back[0] / length * ARRIVAL_M, gate[1] + back[1] / length * ARRIVAL_M];
        let lairs = r.raid_bosses.map(|(_, off)| dry([gate[0] + off[0] as f64, gate[1] + off[1] as f64], 140.0));
        Places { realm, arrival, gate, facing, lairs }
    }

    /// What raid boss `slot`'s lair is called, to remember it's beaten: where it is in the
    /// landscape to the decimeter, as other lairs are, and which of the three it is
    pub fn lair_key(&self, slot: usize) -> [i64; 3] {
        let [x, y] = self.lairs[slot];
        [(x * 10.0).round() as i64, (y * 10.0).round() as i64, LAIR_KEY - slot as i64]
    }
}

/// How far along the journey's got, from the blocks set into the gate, `placed` for each realm:
/// the realms the way's open to, the normal world always, and whether all of them are restored
pub fn opened(placed: &[u32]) -> (Vec<u32>, bool) {
    let full = |r: usize| placed.get(r).is_some_and(|&n| n as usize >= RAIDS);
    let mut open = vec![0];
    for r in 1..REALMS.len() {
        if full(r - 1) {
            open.push(r as u32);
        } else {
            break;
        }
    }
    let restored = full(REALMS.len() - 1) && open.len() == REALMS.len();
    (open, restored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_realm_opens_once_the_one_before_has_all_its_blocks_in() {
        assert_eq!(opened(&[]), (vec![0], false));
        assert_eq!(opened(&[2]), (vec![0], false));
        assert_eq!(opened(&[3]), (vec![0, 1], false));
        assert_eq!(opened(&[3, 3, 1]), (vec![0, 1, 2], false));
        // Blocks set in further on don't open what's past a realm still sealed
        assert_eq!(opened(&[3, 0, 3, 3]), (vec![0, 1], false));
        assert_eq!(opened(&[3, 3, 3]), (vec![0, 1, 2, 3], false));
        assert_eq!(opened(&[3, 3, 3, 3]), (vec![0, 1, 2, 3], true));
    }

    #[test]
    fn realms_are_harder_and_richer_further_on() {
        for pair in REALMS.windows(2) {
            let (a, b) = (pair[0].scaling, pair[1].scaling);
            assert!(b.toughness > a.toughness && b.damage > a.damage && b.luck > a.luck, "{} to {}", pair[0].name, pair[1].name);
        }
        let again = REALMS[3].scaling.after(1);
        assert!(again.toughness > REALMS[3].scaling.toughness);
        // Every realm's three are bosses, each different, and its monsters are monsters
        for r in &REALMS {
            for (k, (look, _)) in r.raid_bosses.iter().enumerate() {
                assert!(look.is_boss(), "{}", look.name());
                assert!(r.raid_bosses[k + 1..].iter().all(|(other, _)| other != look));
            }
            assert!(r.monsters.iter().all(|p| p.looks.iter().all(|l| l.is_monster())));
            assert!(r.seed == NORMAL_SEED || r.seed > FREE_ROAM_SEEDS);
            assert_eq!(by_id(r.id).map(|k| REALMS[k].name), Some(r.name));
        }
    }

    #[test]
    fn a_realms_places_are_on_dry_land_and_the_same_every_time() {
        let settings = TerrainSettings { width_m: 224, height_m: 64, voxels_per_meter: 16, seed: NORMAL_SEED, grass_tufts: true };
        let a = Places::find(0, &settings, [200.0, 180.0], [0.0, 1.0]);
        assert_eq!(a, Places::find(0, &settings, [200.0, 180.0], [0.0, 1.0]));
        for p in a.lairs.iter().chain([&a.gate]) {
            assert!(worldgen::is_dry_land(&settings, [p[0] as f32, p[1] as f32]), "{p:?}");
        }
        // Its gate faces the way it's come at from, the player stands in front of it
        let to_arrival = [a.arrival[0] - a.gate[0], a.arrival[1] - a.gate[1]];
        assert!((to_arrival[1].atan2(to_arrival[0]) - a.facing).abs() < 1e-6);
        assert!((to_arrival[0].hypot(to_arrival[1]) - ARRIVAL_M).abs() < 1e-6);
        // Each lair's key is its own, and none is a landscape lair's
        assert!(a.lair_key(0) != a.lair_key(1) && a.lair_key(1) != a.lair_key(2));
        assert!((0..3).all(|k| a.lair_key(k)[2] < -1000));
    }
}
