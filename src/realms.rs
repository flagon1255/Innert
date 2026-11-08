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
use crate::worldgen::material::{GLOW_BLOOD, GLOW_FROST, GLOW_HOLY, GLOW_POISON, GLOW_STORM};
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
    /// Where each of them is said to be, for the compass and the map: "a sea cave" and the like
    pub lair_names: [&'static str; 3],
    /// The realm's finale, if it has one: who rises once its three raid bosses are beaten, about
    /// where from where the realm's come to, and where that's said to be, see on_raids_beaten()
    pub finale: Option<(Look, [f32; 2], &'static str)>,
    /// The monsters that roam it, a pack at a time. Each works on any land, the normal world's
    /// too: those that fly come down to fight, for night raids from one realm into another.
    pub monsters: &'static [Pack],
    pub scaling: Scaling,
    /// The color of its rune on the gate, and its Innert blocks', in the HUD, and what the rune's
    /// made of
    pub color: [u8; 3],
    pub rune: u8,
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
pub const WATER_SEED: u32 = 0xF400_0001;
/// The most a Free Roam seed can be
pub const FREE_ROAM_SEEDS: u32 = 0xEFFF_FFFF;

/// How many times the health of a raid boss a realm's three of has, when it's not one of the huge
/// raid bosses already: as hard to beat as one of those
pub const RAID_PROMOTION: f64 = 5.0;
/// And how many times that again a realm's finale has, see on_raids_beaten()
pub const FINALE_PROMOTION: f64 = 2.5;

/// How far the grappling hook throws, in meters, in the sky isles and anywhere else: every island
/// there has neighbours across gaps well within it, see worldgen::sky_island()
pub const HOOK_REACH_M: f32 = 40.0;

/// The sky isles: the grappling hook lies by their portal home, given on first coming to them, so
/// every island's within reach from the start, and the first of their raid bosses beaten leaves
/// wings to glide with. Both are sold in the normal world too, once the way there's open.
pub const HOOK_REALM: usize = 1;
pub const WINGS_REALM: usize = 1;

/// How many realms there are, the normal world among them
pub const REALM_COUNT: usize = 5;

/// The realms in the order the journey goes through them: each opened by the one before's three
/// Innert blocks set into the gate. To change the order, move them about here.
pub const REALMS: [Realm; REALM_COUNT] = [
    Realm {
        id: "normal",
        name: "The Normal World",
        seed: NORMAL_SEED,
        raid_bosses: [(Look::Colossus, [260.0, 140.0]), (Look::ElderWyrm, [-220.0, 280.0]), (Look::CryptTitan, [-140.0, -300.0])],
        lair_names: ["the high meadows", "the old burn", "the barrows"],
        finale: None,
        monsters: &[
            Pack { looks: &[Look::Wolf, Look::Wolf, Look::Wolf], most: 3 },
            Pack { looks: &[Look::Bandit, Look::Crossbowman, Look::Shieldbearer, Look::BanditBrute], most: 2 },
            Pack { looks: &[Look::Skeleton, Look::Zombie, Look::ArmoredSkeleton], most: 2 },
            Pack { looks: &[Look::Tusker, Look::Tusker], most: 1 },
        ],
        scaling: Scaling::NONE,
        color: [255, 214, 120],
        rune: GLOW_HOLY,
    },
    Realm {
        id: "sky",
        name: "The Sky Isles",
        seed: SKY_SEED,
        raid_bosses: [(Look::StormDrake, [240.0, 60.0]), (Look::Golem, [-60.0, 300.0]), (Look::Drake, [-260.0, -160.0])],
        lair_names: ["the storm isle", "the great isle", "the far isle"],
        finale: None,
        monsters: &[
            Pack { looks: &[Look::Gloomwing, Look::Gloomwing, Look::Gloomwing, Look::Gloomwing], most: 3 },
            Pack { looks: &[Look::Crossbowman, Look::Crossbowman, Look::Shieldbearer], most: 2 },
            Pack { looks: &[Look::BoneArcher, Look::ArmoredSkeleton, Look::ArmoredSkeleton], most: 1 },
        ],
        scaling: Scaling { toughness: 1.6, damage: 1.35, luck: 1.8 },
        color: [140, 200, 255],
        rune: GLOW_STORM,
    },
    Realm {
        id: "dream",
        name: "The Dreaming Red",
        seed: DREAM_SEED,
        raid_bosses: [(Look::GraveKnight, [280.0, -40.0]), (Look::Spider, [-120.0, 280.0]), (Look::Treant, [-200.0, -240.0])],
        lair_names: ["the bleeding fields", "the soft hollows", "the dreaming wood"],
        finale: None,
        monsters: &[
            Pack { looks: &[Look::Ghoul, Look::Ghoul, Look::Bloater], most: 2 },
            Pack { looks: &[Look::Necromancer, Look::Zombie, Look::ArmoredZombie], most: 2 },
            Pack { looks: &[Look::GraveBrute, Look::BoneArcher, Look::BoneArcher], most: 1 },
            Pack { looks: &[Look::MireLurker, Look::MireLurker], most: 1 },
        ],
        scaling: Scaling { toughness: 2.4, damage: 1.75, luck: 2.8 },
        color: [235, 60, 70],
        rune: GLOW_BLOOD,
    },
    Realm {
        id: "ice",
        name: "The Rimelands",
        seed: ICE_SEED,
        raid_bosses: [(Look::FrostWyrm, [250.0, 180.0]), (Look::FrostGolem, [-240.0, 160.0]), (Look::WinterAlpha, [40.0, -310.0])],
        lair_names: ["the frozen heights", "the glacier", "the white woods"],
        finale: None,
        monsters: &[
            Pack { looks: &[Look::RimeWraith, Look::RimeWraith, Look::RimeWraith], most: 3 },
            Pack { looks: &[Look::Wolf, Look::Wolf, Look::Wolf, Look::Wolf], most: 2 },
            Pack { looks: &[Look::ArmoredSkeleton, Look::BoneArcher, Look::Skeleton], most: 1 },
        ],
        scaling: Scaling { toughness: 3.4, damage: 2.2, luck: 4.0 },
        color: [150, 235, 255],
        rune: GLOW_FROST,
    },
    Realm {
        id: "water",
        name: "The Sunken Reach",
        seed: WATER_SEED,
        // Stand-ins for the serpents' own raid bosses, which take their places when they're in
        raid_bosses: [(Look::VenomDrake, [270.0, 90.0]), (Look::Ironhide, [-150.0, 260.0]), (Look::Direwolf, [-210.0, -220.0])],
        lair_names: ["the sea cave", "the reef", "the whirlpool"],
        // And for the serpent that rises from the deep once they're beaten
        finale: Some((Look::ElderWyrm, [60.0, -330.0], "the deep")),
        monsters: &[
            Pack { looks: &[Look::MireLurker, Look::MireLurker, Look::MireLurker], most: 3 },
            Pack { looks: &[Look::Crossbowman, Look::Shieldbearer, Look::BanditBrute], most: 2 },
            Pack { looks: &[Look::Zombie, Look::ArmoredZombie, Look::Bloater], most: 2 },
            Pack { looks: &[Look::Gloomwing, Look::Gloomwing, Look::Gloomwing], most: 1 },
        ],
        scaling: Scaling { toughness: 4.6, damage: 2.7, luck: 5.2 },
        color: [70, 215, 200],
        rune: GLOW_POISON,
    },
];

/// What rises once the three raid bosses of realm `realm` are beaten, if anything, and about where
/// from where the realm's come to, in meters: a hook for the realms' finales, see Realm::finale
pub fn on_raids_beaten(realm: usize) -> Option<(Look, [f32; 2])> {
    REALMS.get(realm)?.finale.map(|(look, off, _)| (look, off))
}

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
    /// Where the realm's finale rises, if it has one, see on_raids_beaten()
    pub finale: Option<[f64; 2]>,
}

impl Places {
    /// Where things are in realm `realm`, made of `settings`' landscape, which is come to round
    /// `start`, in meters, looking along `ahead`: the gate's just ahead there, on the nearest
    /// level dry land, facing back the way it's looked at from, and the raid bosses' lairs on
    /// the nearest dry land to where the realm says
    pub fn find(realm: usize, settings: &TerrainSettings, start: [f64; 2], ahead: [f64; 2]) -> Places {
        let r = &REALMS[realm];
        let dry = |near: [f64; 2], within: f32, room: f32| worldgen::dry_land_near(settings, [near[0] as f32, near[1] as f32], within, room).map_or(near, |p| [p[0] as f64, p[1] as f64]);
        // Room for the gate's dais all round it, and for a raid boss to fight in
        let gate = dry([start[0] + ahead[0] * 14.0, start[1] + ahead[1] * 14.0], 80.0, 4.0);
        let back = [start[0] - gate[0], start[1] - gate[1]];
        let length = back[0].hypot(back[1]).max(1e-6);
        let facing = back[1].atan2(back[0]);
        // As far out in front of it as there's land to stand on, as on a floating island
        let out = |d: f64| [gate[0] + back[0] / length * d, gate[1] + back[1] / length * d];
        let stand = (0..16).map(|k| ARRIVAL_M - k as f64 * 0.5).find(|&d| worldgen::is_dry_land(settings, out(d).map(|v| v as f32), 1.0)).unwrap_or(ARRIVAL_M);
        let arrival = out(stand);
        let lairs = r.raid_bosses.map(|(_, off)| dry([gate[0] + off[0] as f64, gate[1] + off[1] as f64], 160.0, 7.0));
        let finale = on_raids_beaten(realm).map(|(_, off)| dry([gate[0] + off[0] as f64, gate[1] + off[1] as f64], 200.0, 9.0));
        Places { realm, arrival, gate, facing, lairs, finale }
    }

    /// What raid boss `slot`'s lair is called, to remember it's beaten: where it is in the
    /// landscape to the decimeter, as other lairs are, and which of the three it is
    pub fn lair_key(&self, slot: usize) -> [i64; 3] {
        let [x, y] = self.lairs[slot];
        [(x * 10.0).round() as i64, (y * 10.0).round() as i64, LAIR_KEY - slot as i64]
    }

    /// The same for the realm's finale's lair, if it has one, see is_finale_key()
    pub fn finale_key(&self) -> Option<[i64; 3]> {
        let [x, y] = self.finale?;
        Some([(x * 10.0).round() as i64, (y * 10.0).round() as i64, LAIR_KEY - RAIDS as i64])
    }
}

/// Whether `key` is a realm's finale's lair's, see Places::finale_key()
pub fn is_finale_key(key: &[i64; 3]) -> bool {
    key[2] == LAIR_KEY - RAIDS as i64
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
        // The last realm's blocks set in restore them all
        let all = [3; REALM_COUNT];
        assert_eq!(opened(&all[..REALM_COUNT - 1]), ((0..REALM_COUNT as u32).collect(), false));
        assert_eq!(opened(&all), ((0..REALM_COUNT as u32).collect(), true));
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
        for p in a.lairs {
            assert!(worldgen::is_dry_land(&settings, [p[0] as f32, p[1] as f32], 7.0), "{p:?}");
        }
        assert!(worldgen::is_dry_land(&settings, [a.gate[0] as f32, a.gate[1] as f32], 4.0));
        // Its gate faces the way it's come at from, the player stands in front of it
        let to_arrival = [a.arrival[0] - a.gate[0], a.arrival[1] - a.gate[1]];
        assert!((to_arrival[1].atan2(to_arrival[0]) - a.facing).abs() < 1e-6);
        assert!((3.0..=ARRIVAL_M + 1e-6).contains(&to_arrival[0].hypot(to_arrival[1])));
        // Each lair's key is its own, and none is a landscape lair's
        assert!(a.lair_key(0) != a.lair_key(1) && a.lair_key(1) != a.lair_key(2));
        assert!((0..3).all(|k| a.lair_key(k)[2] < -1000));
    }

    /// The dreaming realm is soft west of its divide and bleeding east of it, its gate on the soft
    /// side where the player comes to it, its raid bosses' lairs on dry land, and a piece of it
    /// across the divide builds, with all that grows there
    #[test]
    fn the_dreaming_realm_has_two_halves() {
        let settings = TerrainSettings { width_m: 64, height_m: 64, voxels_per_meter: 4, seed: DREAM_SEED, grass_tufts: false };
        let p = Places::find(2, &settings, [200.0, 180.0], [0.0, 1.0]);
        assert!(worldgen::is_dry_land(&settings, [p.gate[0] as f32, p.gate[1] as f32], 4.0));
        for lair in p.lairs {
            assert!(worldgen::is_dry_land(&settings, [lair[0] as f32, lair[1] as f32], 7.0), "{lair:?}");
        }
        assert!(worldgen::dream_blood(p.gate[0] as f32, p.gate[1] as f32) < 0.01, "the gate's in the soft half");
        assert_eq!(worldgen::dream_blood(320.0, 0.0), 1.0);
        assert_eq!(worldgen::dream_blood(150.0, 0.0), 0.0);
        let (terrain, world) = worldgen::generate_area(&settings, [200 * 4, 150 * 4], [64 * 4, 64 * 4], &worldgen::RiverCache::default());
        assert!(world.brick_count() > 0);
        // No one lives there
        assert!(terrain.sites().iter().all(|s| !matches!(s.kind, "town" | "royal court" | "bandit camp")));
    }

    /// The frozen realm's snow from end to end, its sea and rivers iced over so nothing flows
    /// anywhere in it, its gate and raid bosses' lairs on dry land
    #[test]
    fn the_frozen_realm_is_iced_over() {
        use crate::worldgen::material::{GRASS, ICE, JUNGLE_GRASS, SAND, SNOW};
        let settings = TerrainSettings { width_m: 64, height_m: 64, voxels_per_meter: 4, seed: ICE_SEED, grass_tufts: false };
        let p = Places::find(3, &settings, [200.0, 180.0], [0.0, 1.0]);
        assert!(worldgen::is_dry_land(&settings, [p.gate[0] as f32, p.gate[1] as f32], 4.0));
        for lair in p.lairs {
            assert!(worldgen::is_dry_land(&settings, [lair[0] as f32, lair[1] as f32], 7.0), "{lair:?}");
        }
        // A piece of it round somewhere low, by the sea or a river
        let low = (0..900).map(|k| [(k % 30) as f32 * 40.0 - 600.0, (k / 30) as f32 * 40.0 - 600.0]).find(|&at| !worldgen::is_dry_land(&settings, at, 0.5)).expect("somewhere low");
        let corner = [(low[0] as i64 - 32) * 4, (low[1] as i64 - 32) * 4];
        let (terrain, world) = worldgen::generate_area(&settings, corner, [64 * 4, 64 * 4], &worldgen::RiverCache::default());
        assert!(terrain.springs().is_empty(), "nothing flows");
        let mut tops = [0usize; 256];
        for y in (0..256).step_by(2) {
            for x in (0..256).step_by(2) {
                if let Some(z) = (0..256).rev().find(|&z| !worldgen::is_passable(world.material_at(x, y, z))) {
                    tops[world.material_at(x, y, z) as usize] += 1;
                }
            }
        }
        assert!(tops[ICE as usize] > 0 && tops[SNOW as usize] > 0, "ice {} snow {}", tops[ICE as usize], tops[SNOW as usize]);
        assert_eq!([GRASS, JUNGLE_GRASS, SAND].map(|m| tops[m as usize]), [0; 3], "no grass or sand");
    }

    /// The sunken reach is islands in deep water, joined up by shallows, so all of them can be got to
    /// from the portal on foot, wading no deeper than a meter: its gate, the raid bosses' lairs and
    /// its finale's on dry land
    #[test]
    fn the_sunken_reach_can_be_crossed_on_foot() {
        let settings = TerrainSettings { width_m: 224, height_m: 64, voxels_per_meter: 16, seed: WATER_SEED, grass_tufts: true };
        let p = Places::find(4, &settings, [200.0, 180.0], [0.0, 1.0]);
        assert!(worldgen::is_dry_land(&settings, [p.gate[0] as f32, p.gate[1] as f32], 4.0), "the portal's on an island");
        for lair in p.lairs.iter().chain(p.finale.as_ref()) {
            assert!(worldgen::is_dry_land(&settings, [lair[0] as f32, lair[1] as f32], 7.0), "{lair:?}");
        }
        // Over a square 600 m across round the portal, every bit of dry land that can be walked to
        // without wading deeper than a meter, from the portal
        const N: usize = 300;
        let step = 2.0;
        let corner = [p.gate[0] as f32 - N as f32, p.gate[1] as f32 - N as f32];
        let over: Vec<f32> = (0..N * N).map(|k| worldgen::seen_from_above(&settings, corner[0] + (k % N) as f32 * step, corner[1] + (k / N) as f32 * step).unwrap()).collect();
        let mut reached = vec![false; N * N];
        let mut todo = vec![N / 2 + N / 2 * N];
        reached[todo[0]] = true;
        while let Some(k) = todo.pop() {
            let (x, y) = (k % N, k / N);
            for (nx, ny) in [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)] {
                if nx < N && ny < N && !reached[nx + ny * N] && over[nx + ny * N] > -1.0 {
                    reached[nx + ny * N] = true;
                    todo.push(nx + ny * N);
                }
            }
        }
        let dry = (0..N * N).filter(|&k| over[k] > 0.5).count();
        let got = (0..N * N).filter(|&k| over[k] > 0.5 && reached[k]).count();
        let deep = (0..N * N).filter(|&k| over[k] < -3.0).count();
        assert!(got as f32 > dry as f32 * 0.95, "only {got} of {dry} dry spots reached on foot");
        assert!(deep as f32 > (N * N) as f32 * 0.3, "it's mostly sea: {deep} of {} deep", N * N);
    }

    /// In the sky isles too: the gate, where the player comes to it and every raid boss's lair are
    /// on a floating island, and from each island there's another within a grappling hook's reach
    #[test]
    fn the_sky_isles_have_room_and_are_within_reach_of_each_other() {
        let settings = TerrainSettings { width_m: 224, height_m: 64, voxels_per_meter: 16, seed: SKY_SEED, grass_tufts: true };
        let p = Places::find(1, &settings, [200.0, 180.0], [0.0, 1.0]);
        assert!(worldgen::is_dry_land(&settings, [p.gate[0] as f32, p.gate[1] as f32], 4.0), "the gate's on an island");
        assert!(worldgen::is_dry_land(&settings, [p.arrival[0] as f32, p.arrival[1] as f32], 1.0), "the player comes to an island");
        for lair in p.lairs {
            assert!(worldgen::is_dry_land(&settings, [lair[0] as f32, lair[1] as f32], 7.0), "{lair:?}");
        }
        // Over a square kilometer, from every island's edge another's within reach, a gap of no
        // more than the hook throws less a little, along the ground, and no higher than it
        // throws less what it pulls up
        let island = |x: f32, y: f32| worldgen::sky_island(SKY_SEED, x, y);
        let mut islands = 0;
        let mut x = -500.0;
        while x < 500.0 {
            let mut y = -500.0;
            while y < 500.0 {
                if island(x, y).is_some() {
                    islands += 1;
                }
                y += 2.0;
            }
            x += 2.0;
        }
        let share = islands as f32 / (500.0 * 500.0);
        assert!((0.15..0.6).contains(&share), "{share} of the sky is island");
        // Every island has a neighbour across a gap the hook reaches over with room to spare, the
        // gap measured along the line between their middles, and none is so much higher than its
        // lowest neighbour that the hook can't pull up to it
        for cy in -10..10 {
            for cx in -10..10 {
                let (a, _, low) = worldgen::sky_isle(SKY_SEED, cx, cy);
                let mut gaps = Vec::new();
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)] {
                    let (b, _, high) = worldgen::sky_isle(SKY_SEED, cx + dx, cy + dy);
                    let steps = ((b[0] - a[0]).hypot(b[1] - a[1]) / 0.5) as usize;
                    let (mut gap, mut widest) = (0.0f32, 0.0f32);
                    for s in 0..steps {
                        let t = s as f32 / steps as f32;
                        if island(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t).is_none() {
                            gap += 0.5;
                            widest = widest.max(gap);
                        } else {
                            gap = 0.0;
                        }
                    }
                    gaps.push((widest, high - low));
                }
                let reachable = gaps.iter().filter(|(gap, rise)| *gap < HOOK_REACH_M - 10.0 && *rise < HOOK_REACH_M * 0.5).count();
                assert!(reachable >= 2, "the island of square {cx}, {cy}: gaps and rises to its neighbours {gaps:?}");
            }
        }
    }
}
