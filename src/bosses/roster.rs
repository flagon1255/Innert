//! Bosses for where there were fewest of them. The Cinder Crown had no lesser bosses of its own,
//! only those borrowed from fire and stone, and the desert had a pharaoh and a golem and no raid boss
//! at all: the Slagwright, a smith of black iron with slag in its crucible, and the Cinderback
//! Tortoise, burning rocks spat from the vents on its shell, are the Cinder Crown's guardians and
//! keep the desert's caverns and ruins; the Sunstinger, a scorpion of the sand, keeps its
//! colonnades; and the Gilded Sphinx, wheeling jets of fire round it, is a raid boss of its
//! ruins. The Dreaming Red had nothing of its own but its raid bosses: the Lantern Widow, closing
//! rings of gloom on the player, and the Dreadmare, hooves running lines of blood-fire along the
//! ground, guard it and keep its churches, towers and colonnades, and the widow haunts the flooded
//! churches of the land too. The jungle's shrines and colonnades have the Grovemaul, a great ape
//! that leaps down on the player with vines bursting up round it. And the Rimelands, whose raid
//! bosses were all lesser bosses made tougher, have the Hoarback Mammoth, rearing and stamping
//! out shockwave after shockwave of frost, which keeps the snowbound castles of the land too.
//!
//! Each is drawn jointed, see src\rig.rs, its pieces among its build's in src\rig. What's here is
//! what each is, the moves of its own and how they play out, as Deeds for src\game.rs.

use super::*;
use crate::animals::Skin;
use crate::worldgen::material::*;

/// Every one of them, in the order they're added among the looks
pub const LOOKS: [Look; 8] = [Look::Slagwright, Look::Cinderback, Look::LanternWidow, Look::Dreadmare, Look::Sunstinger, Look::Grovemaul, Look::Hoarback, Look::GildedSphinx];

/// Whether `look` is one of them
pub fn has(look: Look) -> bool {
    LOOKS.contains(&look)
}

/// Whether `m` is one of their own moves, played out here
pub fn owns(m: Move) -> bool {
    matches!(m, Move::Slagrain | Move::Vents | Move::Gloomgate | Move::Hoofbrand | Move::Sting | Move::Pounce | Move::Rimestomp | Move::Sunwheel)
}

/// How much bigger its model is than its build's
pub fn scale(look: Look) -> f64 {
    match look {
        Look::Slagwright => 1.6,
        Look::Cinderback => 1.5,
        Look::LanternWidow => 1.55,
        Look::Dreadmare => 1.45,
        Look::Sunstinger => 1.15,
        Look::Grovemaul => 1.55,
        Look::Hoarback => 2.4,
        Look::GildedSphinx => 2.2,
        _ => 1.0,
    }
}

/// How much damage it takes, as tough as the bosses of its tier: the lesser about the giants', the
/// pharaoh's and the stags', the raid bosses about the Dreaming Red's
pub fn health(look: Look) -> f64 {
    match look {
        Look::Slagwright => 3200.0,
        Look::Cinderback => 3300.0,
        Look::LanternWidow => 2500.0,
        Look::Dreadmare => 2500.0,
        Look::Sunstinger => 2600.0,
        Look::Grovemaul => 3000.0,
        Look::Hoarback => 14500.0,
        Look::GildedSphinx => 13500.0,
        _ => 2500.0,
    }
}

/// How fast it walks and runs, in meters a second
pub fn speeds(look: Look) -> (f64, f64) {
    match look {
        Look::Slagwright => (1.4, 3.0),
        // Slow as a tortoise is, but it gets there
        Look::Cinderback => (0.9, 1.9),
        Look::LanternWidow => (1.2, 2.6),
        Look::Dreadmare => (2.2, 8.0),
        Look::Sunstinger => (2.2, 5.5),
        Look::Grovemaul => (1.6, 4.6),
        Look::Hoarback => (1.6, 5.0),
        Look::GildedSphinx => (1.8, 6.0),
        _ => (1.2, 3.0),
    }
}

/// Meters covered by one cycle of its walk, at its build's size
pub fn stride(look: Look) -> f64 {
    match look {
        Look::Slagwright | Look::Grovemaul => 1.7,
        Look::Cinderback => 1.0,
        Look::LanternWidow => 1.2,
        Look::Dreadmare => 2.6,
        Look::Sunstinger => 1.6,
        Look::Hoarback => 3.0,
        Look::GildedSphinx => 2.8,
        _ => 1.5,
    }
}

/// Radius of a sphere round the whole of it, however it's posed, in meters: weapons raised high,
/// rearing up, a tail curled over its back
pub fn reach(look: Look) -> f64 {
    let s = look.scale();
    s * match look {
        Look::Slagwright => 4.6,
        Look::Cinderback => 2.3,
        Look::LanternWidow => 3.05,
        Look::Dreadmare => 3.85,
        Look::Sunstinger => 3.4,
        Look::Grovemaul => 3.3,
        Look::Hoarback => 5.0,
        Look::GildedSphinx => 3.0,
        _ => 3.0,
    }
}

/// The body to aim at: how high its middle is, and how big it is, in meters
pub fn body(look: Look) -> (f64, f64) {
    let s = look.scale();
    let (high, round) = match look {
        Look::Slagwright => (1.5, 0.55),
        Look::Cinderback => (0.85, 0.8),
        Look::LanternWidow => (1.0, 0.36),
        Look::Dreadmare => (1.25, 0.5),
        Look::Sunstinger => (0.8, 0.85),
        Look::Grovemaul => (1.35, 0.6),
        Look::Hoarback => (1.6, 0.95),
        Look::GildedSphinx => (1.3, 0.65),
        _ => (1.0, 0.5),
    };
    (high * s, round * s)
}

pub fn name(look: Look) -> &'static str {
    match look {
        Look::Slagwright => "Slagwright",
        Look::Cinderback => "Cinderback Tortoise",
        Look::LanternWidow => "Lantern Widow",
        Look::Dreadmare => "Dreadmare",
        Look::Sunstinger => "Sunstinger",
        Look::Grovemaul => "Grovemaul",
        Look::Hoarback => "Hoarback Mammoth",
        Look::GildedSphinx => "Gilded Sphinx",
        _ => "boss",
    }
}

/// What its attacks are made of
pub fn element(look: Look) -> Option<Element> {
    match look {
        Look::Slagwright | Look::Cinderback | Look::GildedSphinx => Some(Element::Fire),
        Look::LanternWidow => Some(Element::Void),
        Look::Dreadmare => Some(Element::Blood),
        Look::Sunstinger | Look::Grovemaul => Some(Element::Poison),
        Look::Hoarback => Some(Element::Frost),
        _ => None,
    }
}

/// What it's made of: its body, darker parts, things on it, what glows, and one more
pub(crate) fn skin(look: Look) -> Skin {
    let make = |body, dark, accent, glow, extra| Skin { body, dark, accent, glow, extra };
    match look {
        // Black iron, darker steel, a leather apron, the forge's fire in its seams, bronze bands
        Look::Slagwright => make(OBSIDIAN, DARK_STEEL, LEATHER, GLOW_FIRE, BRONZE),
        // Sandy hide, a shell of black glass plated with rock, fire in its vents, dark claws
        Look::Cinderback => make(SANDSTONE_DARK, OBSIDIAN, ROCK, GLOW_FIRE, BARK_DARK),
        // Pale hands under black mourning, a crook of dark iron, the void in her lantern's bronze cage
        Look::LanternWidow => make(FUR_CREAM, OBSIDIAN, DARK_STEEL, GLOW_VOID, BRONZE),
        // A coat dark as night, black hooves, its ribs showing pale, blood burning in its mane
        Look::Dreadmare => make(FUR_DARK, OBSIDIAN, BONE, GLOW_BLOOD, TEMPLE_RED),
        // Plates the color of the sand banded darker and gold, venom glowing in its sting
        Look::Sunstinger => make(SANDSTONE, SANDSTONE_DARK, GOLD, GLOW_POISON, OBSIDIAN),
        // Dark fur, a leathery face and hands, moss on its back, gold eyes, vines hanging off it
        Look::Grovemaul => make(FUR_DARK, LEATHER, MOSS, GOLD, VINE),
        // White wool over grey, ice on its back, frost in its eyes, tusks of old bone
        Look::Hoarback => make(FUR_WHITE, FUR_GRAY, ICE, GLOW_FROST, BONE),
        // Carved sandstone come alive, darker in its carving, gold, the sun in its eyes, a teal
        // striped headdress
        Look::GildedSphinx => make(SANDSTONE, SANDSTONE_DARK, GOLD, GLOW_FIRE, TEMPLE_TEAL),
        _ => make(ROCK, GRAVEL, MOSS, GLOW_FROST, CRYSTAL),
    }
}

/// The kinds of weapon it leaves most of the time, as weapon families' ids, see bosses::taste():
/// the smith's hammers and hand cannons, the tortoise's bombs, the widow's censers and bells, the
/// mare's whips and naginatas, the scorpion's katars, the ape's mauls and totems, the mammoth's
/// heavy polearms and the sphinx's war fans and sceptres
pub fn taste(look: Look) -> &'static [&'static str] {
    match look {
        Look::Slagwright => &["hammer", "maul", "handcannon", "flail"],
        Look::Cinderback => &["bombs", "maul", "blunderbuss", "hammer"],
        Look::LanternWidow => &["censer", "bell", "tome", "wand"],
        Look::Dreadmare => &["whip", "naginata", "scythe", "spear"],
        Look::Sunstinger => &["katar", "dagger", "whip", "crossbow"],
        Look::Grovemaul => &["maul", "totem", "axe", "hammer"],
        Look::Hoarback => &["maul", "halberd", "naginata", "hammer"],
        Look::GildedSphinx => &["warfan", "scepter", "orb", "katana"],
        _ => &[],
    }
}

/// What its hoard's called, as it spills out of it, see Game::reap()
pub fn hoard(look: Look) -> Option<&'static str> {
    Some(match look {
        Look::Slagwright => "forge-hoard",
        Look::Cinderback => "ember-hoard",
        Look::LanternWidow => "hoard of mourning",
        Look::Dreadmare => "dream-tithe",
        Look::Sunstinger => "desert cache",
        Look::Grovemaul => "grove-hoard",
        Look::Hoarback => "tusk-hoard",
        Look::GildedSphinx => "gilded hoard",
        _ => return None,
    })
}

/// A blow at what's in front: how far ahead it lands and how far round it reaches in meters, how
/// hard it hits, and how big a hole it blows
pub fn slam(look: Look) -> (f64, f64, f64, f64) {
    match look {
        // A forge hammer, a tortoise's forefeet, a crook, a mare's hooves, pincers, an ape's fists
        Look::Slagwright => (2.5, 3.0, 32.0, 0.8),
        Look::Cinderback => (2.0, 2.8, 28.0, 0.6),
        Look::LanternWidow => (1.7, 2.0, 18.0, 0.0),
        Look::Dreadmare => (2.3, 2.5, 26.0, 0.4),
        Look::Sunstinger => (2.0, 1.9, 22.0, 0.0),
        Look::Grovemaul => (2.4, 2.9, 30.0, 0.7),
        // A mammoth's forefeet and a sphinx's paws, raid bosses' blows
        Look::Hoarback => (4.6, 4.4, 38.0, 1.0),
        Look::GildedSphinx => (3.8, 3.8, 36.0, 0.6),
        _ => (2.0, 2.6, 22.0, 0.5),
    }
}

/// Seconds the Cinderback spits rocks for, between each, how many, and how far out each ring of
/// them comes down, in meters at its size
const VENT_SECONDS: f64 = 1.6;
const VENT_EVERY: f64 = 0.1;
const VENT_ROCKS: usize = 15;
const VENT_RINGS: [f64; 3] = [3.2, 6.2, 9.2];
/// Seconds the Lantern Widow's ring of gloom takes to close, and how far round it is to start with
/// and once it's shut, in meters
const GLOOM_SECONDS: f64 = 2.0;
const GLOOM_FROM: f64 = 6.5;
const GLOOM_TO: f64 = 1.8;
/// Seconds into its hoofbrand the Dreadmare brings its hooves down the second time, and how far
/// apart the bursts along each line are, in meters
const BRAND_AGAIN: f64 = 0.45;
const BRAND_SPACING: f64 = 1.7;
/// Seconds into its sting the Sunstinger stabs the second and third time, how far its tail reaches
/// at its build's size, and how far round each stab hurts, in meters
const STINGS: [f64; 2] = [0.42, 0.84];
const STING_REACH: f64 = 3.4;
const STING_RADIUS: f64 = 1.4;
/// Seconds into its rimestomp each of the Hoarback's shockwaves starts
const RIME_WAVES: [f64; 3] = [0.0, 0.55, 1.1];
/// Seconds the Gilded Sphinx's jets of fire wheel round, and radians a second they turn
const SUN_SECONDS: f64 = 2.6;
const SUN_TURN: f64 = 0.75;
/// Where the slag comes down round the player, in meters
const SLAG_RING: f64 = 3.4;

/// How far the Hoarback's shockwaves run out, in meters
fn rime_reach(look: Look) -> f64 {
    quake_reach(look) * 0.75
}

/// Seconds each of their moves takes to get ready for, to make and to get over, and how long till
/// it can be made again
pub fn timing(look: Look, m: Move) -> (f64, f64, f64, f64) {
    match m {
        Move::Slagrain => (1.1, 0.5, 0.9, 8.0),
        // Slow to come back out of its shell: the moment to get in close
        Move::Vents => (1.2, VENT_SECONDS, 1.3, 10.0),
        Move::Gloomgate => (1.0, GLOOM_SECONDS, 0.8, 9.0),
        Move::Hoofbrand => (0.9, 0.9, 0.8, 6.5),
        Move::Sting => (0.8, STINGS[1] + 0.4, 0.9, 5.0),
        // In the air as long as leap() says
        Move::Pounce => (0.8, 0.9, 0.9, 6.5),
        Move::Rimestomp => (1.3, RIME_WAVES[2] + rime_reach(look) / QUAKE_PACE, 1.4, 10.0),
        // Getting up off its haunches after: the moment to hit it
        Move::Sunwheel => (1.2, SUN_SECONDS, 1.6, 12.0),
        _ => (0.8, 0.5, 0.8, 6.0),
    }
}

/// The moves each makes in a fight and from how far off, the first ready and in range being what
/// it does, its own first so it makes them whenever they're ready; a raid boss's in meters at its
/// build's size
pub fn menu(look: Look) -> &'static [(Move, f64, f64)] {
    match look {
        Look::Slagwright => &[(Move::Slagrain, 4.5, 26.0), (Move::Swat, 0.0, 3.6), (Move::Slam, 0.0, 4.6), (Move::Quake, 3.0, 12.0)],
        Look::Cinderback => &[(Move::Vents, 0.0, 12.0), (Move::Slam, 0.0, 3.8), (Move::Quake, 3.0, 12.0), (Move::Spit, 4.5, 26.0)],
        Look::LanternWidow => &[(Move::Blink, 0.0, 2.6), (Move::Gloomgate, 3.0, 22.0), (Move::Slam, 0.0, 3.0), (Move::Spit, 3.0, 26.0)],
        Look::Dreadmare => &[(Move::Hoofbrand, 3.5, 18.0), (Move::Slam, 0.0, 4.0), (Move::Trample, 7.0, 30.0)],
        Look::Sunstinger => &[(Move::Sting, 0.0, 6.0), (Move::Slam, 0.0, 3.2), (Move::Spit, 5.0, 24.0)],
        Look::Grovemaul => &[(Move::Pounce, 6.0, 22.0), (Move::Swat, 0.0, 3.6), (Move::Slam, 0.0, 4.4), (Move::Throw, 7.0, 30.0)],
        Look::Hoarback => &[
            (Move::Summon, 0.0, 14.0),
            (Move::Cataclysm, 0.0, 14.0),
            (Move::Rimestomp, 0.0, 5.5),
            (Move::Slam, 0.0, 2.2),
            (Move::Trample, 3.0, 14.0),
            (Move::Breath, 1.2, 4.4),
            (Move::Throw, 3.5, 14.0),
        ],
        Look::GildedSphinx => &[(Move::Summon, 0.0, 14.0), (Move::Cataclysm, 0.0, 14.0), (Move::Sunwheel, 0.0, 4.4), (Move::Slam, 0.0, 2.2), (Move::Leap, 3.0, 10.0), (Move::Breath, 1.2, 4.6)],
        _ => &[],
    }
}

/// `n` points round `middle`, in voxels, `r` meters out, starting `turn` radians round
fn ring(middle: Vector3<f64>, n: usize, r: f64, turn: f64, vpm: f64) -> impl Iterator<Item = Vector3<f64>> {
    (0..n).map(move |k| {
        let a = turn + k as f64 / n as f64 * TAU;
        middle + Vector3::new(a.cos(), a.sin(), 0.0) * r * vpm
    })
}

/// Which way round its lair a boss turns things, its own, so no two are alike
fn own_turn(boss: &Boss) -> f64 {
    ((boss.key[0] ^ boss.key[1].wrapping_mul(7)) as f64 * 0.37).rem_euclid(TAU)
}

/// Where the Slagwright's slag comes down: on the player and in a ring round them, more of it once
/// it's angry
fn slag_spots(boss: &Boss, vpm: f64) -> Vec<Vector3<f64>> {
    let n = if boss.enraged { 8 } else { 6 };
    std::iter::once(boss.aim).chain(ring(boss.aim, n, SLAG_RING, own_turn(boss), vpm)).collect()
}

/// Where the Sunstinger's tail comes down at the player at `at`: no further off than it reaches
fn sting_spot(a: &Animal, at: Vector3<f64>, vpm: f64) -> Vector3<f64> {
    let off = (at - a.position).xy();
    let d = off.norm() / vpm;
    let most = STING_REACH * a.look.scale();
    if d <= most {
        return at;
    }
    let way = off / off.norm().max(1e-6);
    Vector3::new(a.position.x + way.x * most * vpm, a.position.y + way.y * most * vpm, at.z)
}

/// The ways the Gilded Sphinx's jets of fire point `t` seconds into its sunwheel, more once it's
/// in the last phase of its fight
fn jets(boss: &Boss, t: f64) -> Vec<f64> {
    let n = if boss.enraged { 6 } else { 4 };
    let sign = if boss.key[0].rem_euclid(2) == 0 { 1.0 } else { -1.0 };
    (0..n).map(|k| boss.spun_from + k as f64 / n as f64 * TAU + sign * SUN_TURN * t).collect()
}

/// Where the Gilded Sphinx's jets come from, in voxels, along `turn`: low round its chest
fn jet_from(a: &Animal, turn: f64, vpm: f64) -> Vector3<f64> {
    let s = a.look.scale();
    a.position + Vector3::new(turn.cos() * 1.3 * s, turn.sin() * 1.3 * s, 1.1 * s) * vpm
}

/// A line of the Dreadmare's blood-fire, from in front of its hooves to the player at `to` and on
/// past them, each burst a little after the one before; three of them fanned out once it's angry
fn brand(a: &Animal, boss: &mut Boss, to: Vector3<f64>, vpm: f64) {
    let s = a.look.scale();
    let forward = Vector3::new(a.heading.cos(), a.heading.sin(), 0.0);
    let start = a.position + forward * 1.4 * s * vpm;
    let ahead = (to - start).xy().try_normalize(1e-6).unwrap_or_else(|| forward.xy());
    let lanes: &[f64] = if boss.enraged { &[-0.38, 0.0, 0.38] } else { &[0.0] };
    for &turn in lanes {
        let (c, sn) = (turn.cos(), turn.sin());
        let way = Vector3::new(ahead.x * c - ahead.y * sn, ahead.x * sn + ahead.y * c, 0.0);
        for k in 0..8 {
            let at = start + way * (0.8 + BRAND_SPACING * k as f64) * vpm + Vector3::new(0.0, 0.0, 1.0 * vpm);
            boss.deeds.push(Deed::Flare { at, delay: 0.18 + 0.08 * k as f64, radius: 1.3, damage: 18.0 });
        }
    }
    boss.deeds.push(Deed::Stomp(start));
    boss.deeds.push(Deed::Slam { at: start, radius: 0.0, damage: 0.0, crater: 0.0 });
}

/// The Sunstinger's tail stabbing down at `at`: a burst of venom where it strikes
fn stab(boss: &mut Boss, at: Vector3<f64>) {
    boss.deeds.push(Deed::Slam { at, radius: STING_RADIUS, damage: 20.0, crater: 0.0 });
    boss.deeds.push(Deed::Flare { at, delay: 0.0, radius: 1.0, damage: 0.0 });
    boss.deeds.push(Deed::Swipe(at, 2));
}

/// Shown on the ground as boss `a` gets ready to make move `m`, `t` seconds into getting ready of
/// `whole`: where it'll come down, a ring closing in on each place
pub fn winding(m: Move, t: f64, whole: f64, a: &Animal, boss: &mut Boss, forward: Vector3<f64>, vpm: f64) {
    let look = a.look;
    let s = look.scale();
    let closing = (1.0 - t / whole.max(0.1)).clamp(0.0, 1.0);
    match m {
        Move::Slagrain => {
            for at in slag_spots(boss, vpm) {
                boss.deeds.push(Deed::Mark { at, radius: 1.6, closing });
            }
        }
        // How far each ring of rocks will come down
        Move::Vents => {
            for r in VENT_RINGS {
                boss.deeds.push(Deed::Mark { at: a.position, radius: r * s / 1.5, closing });
            }
        }
        Move::Gloomgate => boss.deeds.push(Deed::Mark { at: boss.aim, radius: GLOOM_FROM, closing }),
        // The start of the line its hooves will send running at the player
        Move::Hoofbrand => {
            let start = a.position + forward * 1.4 * s * vpm;
            let ahead = (boss.aim - start).xy().try_normalize(1e-6).unwrap_or_else(|| forward.xy());
            for k in 0..4 {
                let at = start + Vector3::new(ahead.x, ahead.y, 0.0) * (0.8 + BRAND_SPACING * k as f64) * vpm;
                boss.deeds.push(Deed::Mark { at, radius: 1.3, closing });
            }
        }
        Move::Sting => boss.deeds.push(Deed::Mark { at: sting_spot(a, boss.aim, vpm), radius: STING_RADIUS, closing }),
        Move::Pounce => boss.deeds.push(Deed::Mark { at: boss.aim, radius: leap(look).2, closing }),
        Move::Rimestomp => boss.deeds.push(Deed::Mark { at: a.position, radius: rime_reach(look), closing }),
        // Which way its jets will come out, fixed as it starts getting ready, shown along each
        Move::Sunwheel => {
            if t <= 0.0 {
                boss.spun_from = a.heading + PI / 4.0;
            }
            for turn in jets(boss, 0.0) {
                let from = jet_from(a, turn, vpm);
                for d in [2.5, 5.5, 8.5] {
                    boss.deeds.push(Deed::Mark { at: from + Vector3::new(turn.cos(), turn.sin(), 0.0) * d * vpm - Vector3::new(0.0, 0.0, 1.1 * s * vpm), radius: 1.1, closing });
                }
            }
        }
        _ => {}
    }
}

/// Boss `a` making move `m`, now it's ready
pub fn begin(m: Move, a: &mut Animal, boss: &mut Boss, player: Option<Vector3<f64>>, forward: Vector3<f64>, vpm: f64) {
    let look = a.look;
    let s = look.scale();
    match m {
        // Its hammer down on the crucible in its left hand, the slag flung up out of it to come
        // down where it showed, the first on the player and the rest one after another round them
        Move::Slagrain => {
            let from = emit(a, boss, vpm);
            for (k, to) in slag_spots(boss, vpm).into_iter().enumerate() {
                boss.deeds.push(Deed::Lob { from, to, time: 0.85 + 0.07 * k as f64, damage: 18.0, radius: 1.6, rock: true });
            }
            boss.deeds.push(Deed::Slam { at: a.position + forward * 1.1 * s * vpm, radius: 0.0, damage: 0.0, crater: 0.0 });
            boss.deeds.push(Deed::Swipe(a.position, 1));
        }
        Move::Vents => {
            boss.trail = 0.0;
            boss.deeds.push(Deed::Stomp(a.position));
        }
        // Her wisps flying out from her lantern to the ring she's closing on the player
        Move::Gloomgate => {
            boss.ring = (boss.aim, GLOOM_FROM, 1.0);
            let from = emit(a, boss, vpm);
            for to in ring(boss.aim + Vector3::new(0.0, 0.0, 0.5 * vpm), 6, GLOOM_FROM, own_turn(boss), vpm) {
                boss.deeds.push(Deed::Lob { from, to, time: 0.55, damage: 8.0, radius: 1.0, rock: false });
            }
        }
        Move::Hoofbrand => brand(a, boss, boss.aim, vpm),
        // The first stab where it showed, then after the player again
        Move::Sting => {
            let at = sting_spot(a, boss.aim, vpm);
            stab(boss, at);
            boss.aim = sting_spot(a, player.unwrap_or(at), vpm);
        }
        // Its forefeet coming down, frost bursting up under them, and the first shockwave
        Move::Rimestomp => {
            boss.leap.0 = a.position;
            boss.deeds.push(Deed::Flare { at: a.position + forward * 2.4 * s * vpm + Vector3::new(0.0, 0.0, 1.0 * vpm), delay: 0.0, radius: 2.6, damage: 0.0 });
        }
        Move::Sunwheel => boss.deeds.push(Deed::Stomp(a.position)),
        _ => {}
    }
}

/// Boss `a` `t` seconds into making move `m`, in a frame of `dt` seconds
#[allow(clippy::too_many_arguments)]
pub fn during(m: Move, t: f64, dt: f64, a: &mut Animal, boss: &mut Boss, player: Option<Vector3<f64>>, vpm: f64) {
    let look = a.look;
    let s = look.scale();
    let crossed = |at: f64| t >= at && t - dt < at;
    match m {
        // Burning rocks spat out of its vents one after another, a ring of them at a time, each
        // ring further out, gaps between them turning round, one in each at the player
        Move::Vents => {
            let due = ((t / VENT_EVERY) as usize).min(VENT_ROCKS);
            let done = boss.trail as usize;
            for k in done..due {
                let (ring, i) = (k / 5, k % 5);
                let r = VENT_RINGS[ring.min(2)] * s / 1.5;
                let to = match player {
                    Some(p) if i == 4 => p,
                    _ => {
                        let turn = own_turn(boss) + ring as f64 * 0.63 + i as f64 / 4.0 * TAU;
                        a.position + Vector3::new(turn.cos(), turn.sin(), 0.0) * r * vpm
                    }
                };
                let vent = k as f64 * 2.4;
                let from = on_body(a, [vent.cos() * 0.45 * s, vent.sin() * 0.45 * s, 1.55 * s], vpm);
                boss.deeds.push(Deed::Lob { from, to, time: 0.75 + 0.12 * ring as f64, damage: 15.0, radius: 1.5, rock: true });
            }
            boss.trail = due as f64;
        }
        // The ring of gloom closing in on where the player stood, more wisps flying out to it, then
        // flying in from all round it, and whoever's still inside it as it shuts caught in it
        Move::Gloomgate => {
            let (middle, _, _) = boss.ring;
            let f = (t / GLOOM_SECONDS).min(1.0);
            let r = GLOOM_FROM + (GLOOM_TO - GLOOM_FROM) * f * f * (3.0 - 2.0 * f);
            boss.deeds.push(Deed::Mark { at: middle, radius: r, closing: 1.0 - f });
            if crossed(0.9) {
                let from = emit(a, boss, vpm);
                for to in ring(middle + Vector3::new(0.0, 0.0, 0.5 * vpm), 6, r, own_turn(boss) + 0.5, vpm) {
                    boss.deeds.push(Deed::Lob { from, to, time: 0.45, damage: 8.0, radius: 1.0, rock: false });
                }
            }
            if crossed(GLOOM_SECONDS - 0.55) {
                for from in ring(middle + Vector3::new(0.0, 0.0, 1.3 * vpm), 6, r, own_turn(boss), vpm) {
                    boss.deeds.push(Deed::Lob { from, to: middle + Vector3::new(0.0, 0.0, 0.4 * vpm), time: 0.55, damage: 10.0, radius: 1.2, rock: false });
                }
            }
            if crossed(GLOOM_SECONDS) {
                boss.deeds.push(Deed::Slam { at: middle, radius: GLOOM_TO + 0.4, damage: 30.0, crater: 0.0 });
                boss.deeds.push(Deed::Flare { at: middle + Vector3::new(0.0, 0.0, 1.0 * vpm), delay: 0.0, radius: GLOOM_TO, damage: 0.0 });
            }
        }
        // Up again and down, at the player where they are now
        Move::Hoofbrand => {
            if let Some(p) = player {
                a.target_heading = heading_to(a.position, p);
            }
            if crossed(BRAND_AGAIN) {
                brand(a, boss, player.unwrap_or(boss.aim), vpm);
            }
        }
        // Following the player with its tail, shown where it'll come down, then stabbing there
        Move::Sting => {
            let beat = STINGS.iter().copied().find(|&b| t < b + dt);
            if let Some(p) = player {
                a.target_heading = heading_to(a.position, p);
                if beat.is_some_and(|b| t < b - 0.15) {
                    boss.aim = sting_spot(a, p, vpm);
                }
            }
            if let Some(b) = beat {
                let closing = ((b - t) / (STINGS[0])).clamp(0.0, 1.0);
                boss.deeds.push(Deed::Mark { at: boss.aim, radius: STING_RADIUS, closing });
                if crossed(b) {
                    let at = boss.aim;
                    stab(boss, at);
                }
            }
        }
        // Shockwave after shockwave of frost running out from under it, each as it stamps again
        Move::Rimestomp => {
            let reach = rime_reach(look);
            for start in RIME_WAVES {
                let local = t - start;
                if local <= 0.0 || (local - dt) * QUAKE_PACE >= reach {
                    continue;
                }
                if local - dt <= 0.0 && start > 0.0 {
                    boss.deeds.push(Deed::Stomp(a.position));
                    let forward = Vector3::new(a.heading.cos(), a.heading.sin(), 0.0);
                    boss.deeds.push(Deed::Flare { at: a.position + forward * 2.4 * s * vpm + Vector3::new(0.0, 0.0, 1.0 * vpm), delay: 0.0, radius: 2.0, damage: 0.0 });
                }
                let to = (local * QUAKE_PACE).min(reach);
                boss.deeds.push(Deed::Quake { at: boss.leap.0, from: ((local - dt) * QUAKE_PACE).max(0.0), to, dt });
            }
        }
        // Its jets of fire wheeling round it
        Move::Sunwheel => {
            for turn in jets(boss, t) {
                let from = jet_from(a, turn, vpm);
                let direction = Vector3::new(turn.cos(), turn.sin(), -0.07).normalize();
                boss.deeds.push(Deed::Breath { from, direction, dt });
            }
        }
        _ => {}
    }
}

/// The Grovemaul come down from its pounce at `at`: vines bursting up in a ring round it, for
/// whoever got out from under it but not far enough
pub fn landed(a: &Animal, boss: &mut Boss, at: Vector3<f64>, vpm: f64) {
    let turn = own_turn(boss) + a.heading;
    for to in ring(at + Vector3::new(0.0, 0.0, 1.0 * vpm), 6, 4.4, turn, vpm) {
        boss.deeds.push(Deed::Roots { at: to });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each is a boss drawn jointed, with a name and an element of its own, leaving weapons of
    /// families there are, and makes a move of its own no boss or monster before it made, which
    /// plays out here; the two raid bosses are raid bosses, the rest aren't
    #[test]
    fn each_has_a_move_of_its_own() {
        let library = crate::weapons::Library::load();
        let older: Vec<Move> = Look::ALL.into_iter().filter(|l| l.build().is_some() && !l.is_dragon() && !l.is_winged() && !has(*l)).flat_map(repertoire).collect();
        let mut names = Vec::new();
        for look in LOOKS {
            assert!(look.is_boss() && crate::rig::rigged(look), "{look:?}");
            assert_eq!(look.is_raid(), matches!(look, Look::Hoarback | Look::GildedSphinx), "{look:?}");
            assert!(look.element().is_some() && hoard(look).is_some());
            assert!(!names.contains(&look.name()));
            names.push(look.name());
            for family in taste(look) {
                assert!(library.families.iter().any(|f| f.id == *family), "{look:?} leaves {family}, which isn't a family");
            }
            let own: Vec<Move> = repertoire(look).into_iter().filter(|m| !older.contains(m)).collect();
            assert!(!own.is_empty() && own.iter().all(|&m| owns(m)), "{look:?}: {own:?}");
            assert!(repertoire(look).len() >= 3, "{look:?}");
        }
    }

    /// Every one of them turns up somewhere it can be found: in place of another boss in the land
    /// it lives in, or among a realm's raid bosses or guardians
    #[test]
    fn each_turns_up_somewhere() {
        use crate::worldgen::Land;
        let bases = [Look::Golem, Look::Drake, Look::Ironhide, Look::Treant, Look::Direwolf, Look::Spider, Look::Ogre, Look::Colossus, Look::Elderhorn];
        let grounds = [GRASS, SNOW, SAND, JUNGLE_GRASS, STONE];
        let lands = [Land::Normal, Land::Dream, Land::Ice];
        for look in LOOKS {
            let in_land = bases.iter().any(|b| lands.iter().any(|&l| grounds.iter().any(|&g| b.for_land(l).for_ground(g) == look)));
            let in_realm = crate::realms::REALMS.iter().any(|r| r.raid_bosses.iter().any(|(l, _)| *l == look) || r.guardians.contains(&look));
            let at_sites = matches!(look, Look::LanternWidow | Look::Cinderback | Look::GildedSphinx);
            assert!(in_land || in_realm || at_sites, "{look:?} is nowhere");
        }
    }

    /// What each move shows on the ground as it's got ready is where it then comes down: the
    /// Slagwright's slag where its rings were, and the Sunstinger's tail no further off than it
    /// reaches
    #[test]
    fn what_shows_is_where_it_lands() {
        let vpm = 16.0;
        let mut a = Animal::boss(Look::Slagwright, Vector3::new(500.0, 500.0, 100.0), [3, 4, 5], 7);
        let mut boss = a.boss.take().unwrap();
        boss.aim = Vector3::new(600.0, 520.0, 100.0);
        winding(Move::Slagrain, 0.5, 1.0, &a, &mut boss, Vector3::x(), vpm);
        let marks: Vec<Vector3<f64>> = boss.deeds.drain(..).filter_map(|d| if let Deed::Mark { at, .. } = d { Some(at) } else { None }).collect();
        assert_eq!(marks.len(), 7);
        let aim = boss.aim;
        begin(Move::Slagrain, &mut a, &mut boss, Some(aim), Vector3::x(), vpm);
        let lobs: Vec<Vector3<f64>> = boss.deeds.drain(..).filter_map(|d| if let Deed::Lob { to, .. } = d { Some(to) } else { None }).collect();
        assert_eq!(marks, lobs);
        let near = Animal::boss(Look::Sunstinger, Vector3::new(500.0, 500.0, 100.0), [3, 4, 5], 7);
        let far = sting_spot(&near, Vector3::new(900.0, 500.0, 100.0), vpm);
        assert!(((far - near.position).norm() / vpm - STING_REACH * Look::Sunstinger.scale()).abs() < 1e-6);
    }
}
