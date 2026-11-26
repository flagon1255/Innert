//! The Tidewrack Kraken and the Grotto Squid. The kraken lies in the deep water of the lakes and
//! the sea, sunk under it till someone comes near, then rises up out of it, its mantle crusted
//! with barnacles and an old harpoon still stuck in it, eight arms spread round it on the water and
//! two long tentacles. It sends its arms up out of the water all round the player to slam down
//! where they showed, flings a tentacle out to seize them and drag them in to its beak, squirts a
//! cloud of ink over them that blinds them, jets water at them in a stream that knocks them back,
//! and spins the water round it into a whirlpool that draws whoever's in it in to its beak. The
//! Grotto Squid, smaller and pale, glowing with the light of the deep caves, keeps the flooded
//! caves and the smaller lakes, and fights the same way but for the arms and the whirlpool.
//!
//! Each is drawn a piece at a time, as a great serpent is: its mantle, its head, and every piece of
//! every arm and tentacle, one model of each drawn as many times as there are pieces, see Body.
//! Its arms are chains of pieces, each reaching for where what it's doing wants its tip, every
//! piece following the one before it, so they wave and curl and lash rather than snap about.
//! What it does to the player goes out as Deeds, its own as Deed::Kraken, see src\game\brine.rs.

use std::sync::OnceLock;

use nalgebra::UnitQuaternion;

use super::*;
use crate::animals::{AnimalInstance, MODEL_RES, MOVING, Model, Part, ball, ellipsoid, limb, voxelize};
use crate::player::WaterLevel;
use crate::worldgen::material::*;

type V3 = Vector3<f64>;

/// Every one of them, in the order their models are in
pub const LOOKS: [Look; 2] = [Look::Kraken, Look::GrottoSquid];

/// Whether `look` is one of them
pub fn has(look: Look) -> bool {
    LOOKS.contains(&look)
}

/// Whether `m` is one of their own moves, played out here
pub fn owns(m: Move) -> bool {
    matches!(m, Move::Lashrise | Move::Ensnare | Move::Inkveil | Move::Brinejet | Move::Maelstrom)
}

/// How big it is next to the kraken
pub fn size(look: Look) -> f64 {
    if look == Look::GrottoSquid { 0.45 } else { 1.0 }
}

/// The kind for water `depth` meters deep, for a lair in it, see serpent_lairs.rs: the kraken in
/// the deep water, the squid in the shallower
pub fn for_water(depth: f64, _roll: u32) -> Look {
    if depth >= DEEP { Look::Kraken } else { Look::GrottoSquid }
}

/// How deep the water has to be for the kraken, and for the squid, in meters
pub const DEEP: f64 = 4.0;
pub const SHALLOW: f64 = 1.6;

/// How many arms it has and how many pieces each is made of, and its two long tentacles; how long
/// the first piece of an arm and of a tentacle is at the kraken's size, in meters
const ARMS: usize = 8;
const ARM_PIECES: usize = 5;
const TENTACLES: usize = 2;
const TENTACLE_PIECES: usize = 7;
const ARM_ROOT: f64 = 1.7;
const TENTACLE_ROOT: f64 = 1.75;
/// How long its mantle is, and how big round its head, at the kraken's size, in meters
const MANTLE: f64 = 4.8;
const HEAD: f64 = 1.2;
/// The arms sent up out of the water round the player: how many pieces each, how long the first
/// of them is, and seconds after they start rising that they come down, that they've done, and
/// that they're gone back under
const RISING_PIECES: usize = 5;
const RISING_ROOT: f64 = 1.5;
const RISE_SLAM: f64 = 0.75;
const RISE_LIE: f64 = 1.35;
const RISE_GONE: f64 = 2.0;
/// Seconds into its lash its tentacle reaches what it's flung at, how long it holds whoever it
/// caught, dragging them in, before its beak bites, and how near its tip they have to be, in meters
const ENSNARE_REACH: f64 = 0.35;
const ENSNARE_HOLD: f64 = 1.4;
const ENSNARE_CATCH: f64 = 1.8;
/// Seconds its whirlpool spins, and how far round it reaches at the kraken's size, in meters
const MAELSTROM_SECONDS: f64 = 3.0;
const MAELSTROM_REACH: f64 = 11.0;
/// Seconds it jets water, and radians a second it turns after the player as it does
const JET_SECONDS: f64 = 1.4;
const JET_TURN: f64 = 0.6;
/// Seconds it takes to die, rearing up with a last cry and sinking away
pub const THROES: f64 = 2.8;

/// Its models, in order: its mantle, its head, a piece of an arm, and the club at a tentacle's end
const MANTLE_MODEL: usize = 0;
const HEAD_MODEL: usize = 1;
const PIECE_MODEL: usize = 2;
const CLUB_MODEL: usize = 3;
const PARTS: usize = 4;

/// How many models they add after the animals' own, see animals::KRAKEN_MODELS_AT
pub const MODELS: usize = LOOKS.len() * PARTS;

/// What each of its parts is called, for its model files, see src\assets.rs
pub const PART_NAMES: [&str; PARTS] = ["mantle", "head", "arm", "club"];

/// What it's made of: its skin, its spots and fins, its belly, its eyes, what glows on it, its
/// suckers, and what crusts it
struct Hide {
    skin: u8,
    dark: u8,
    belly: u8,
    eye: u8,
    glow: u8,
    sucker: u8,
    crust: u8,
}

fn hide(look: Look) -> Hide {
    match look {
        // Pale as the cave fish, spotted teal, the light of the deep caves glowing in rows down it
        Look::GrottoSquid => Hide { skin: FUR_WHITE, dark: TEMPLE_TEAL, belly: LIMESTONE, eye: GLOW_ARCANE, glow: GLOW_ARCANE, sucker: FUR_CREAM, crust: ICE },
        // Deep red, mottled black, a cream belly and suckers, eyes of old gold, the void glowing in
        // spots down its sides, crusted with barnacles
        _ => Hide { skin: CLOTH, dark: OBSIDIAN, belly: FUR_CREAM, eye: GOLD, glow: GLOW_VOID, sucker: FUR_CREAM, crust: LIMESTONE },
    }
}

/// How far from its middle any of it reaches, in meters: its arms flung out
pub fn reach(look: Look) -> f64 {
    size(look) * (MANTLE * 0.5 + 9.0)
}

/// The body to aim at: how high its middle is over where it floats, and how big it is, in meters
pub fn body(look: Look) -> (f64, f64) {
    let k = size(look);
    (0.7 * k, 1.35 * k)
}

/// How fast it swims, and lunges through the water, in meters a second
pub fn speeds(look: Look) -> (f64, f64) {
    match look {
        Look::GrottoSquid => (1.8, 4.8),
        _ => (1.5, 4.0),
    }
}

/// Where what it squirts comes out of it, in meters with +x ahead, see bosses::mouth()
pub fn mouth(look: Look) -> [f64; 3] {
    let k = size(look);
    [1.5 * k, 0.0, 0.4 * k]
}

/// A blow of its arms at what's in front: how far ahead it lands and how far round it reaches in
/// meters, how hard it hits, and how big a hole it blows
pub fn slam(look: Look) -> (f64, f64, f64, f64) {
    match look {
        Look::GrottoSquid => (3.2, 2.0, 20.0, 0.0),
        _ => (6.0, 3.0, 30.0, 0.0),
    }
}

/// How near it comes before stopping to strike, and how far short of the player, in meters
pub fn closeness(look: Look) -> (f64, f64) {
    let k = size(look);
    (6.5 * k + 1.0, 3.0 * k)
}

/// Seconds each of its moves takes to get ready for, to make and to get over, and how long till it
/// can be made again
pub fn timing(look: Look, m: Move) -> (f64, f64, f64, f64) {
    let rising = rising_spots(look, true) as f64 * 0.12 + RISE_SLAM + 0.2;
    match m {
        // The water stirring where its arms will come up, long enough to step out of it
        Move::Lashrise => (1.15, rising, 0.9, 8.0),
        Move::Ensnare => (0.8, ENSNARE_REACH + ENSNARE_HOLD + 0.25, 0.8, 9.0),
        // Its mantle swelling as it draws in water, then squirting it out black
        Move::Inkveil => (0.95, 0.45, 0.7, 12.0),
        // Rearing up out of the water and sucking it in, then jetting it: the jet's over, it's open
        Move::Brinejet => (0.85, JET_SECONDS, 1.0, 7.0),
        // Slow to get over the whirlpool: the moment to get at it
        Move::Maelstrom => (1.25, MAELSTROM_SECONDS, 1.4, 14.0),
        _ => (0.8, 0.5, 0.8, 6.0),
    }
}

/// The moves each makes in a fight and from how far off, in meters, the first ready and in range
/// being what it does: the kraken's whirlpool and arms at whoever comes close, a tentacle flung at
/// whoever's a little further, and from further still its arms sent up out of the water round
/// them, water jetted at them and ink squirted over them; the squid all but the whirlpool and its
/// arms sent up, as it's not big enough
pub fn menu(look: Look) -> &'static [(Move, f64, f64)] {
    match look {
        Look::GrottoSquid => &[(Move::Slam, 0.0, 4.5), (Move::Ensnare, 3.0, 11.0), (Move::Brinejet, 3.0, 16.0), (Move::Inkveil, 5.0, 22.0)],
        _ => &[(Move::Maelstrom, 0.0, 10.0), (Move::Slam, 0.0, 7.5), (Move::Ensnare, 5.0, 15.0), (Move::Lashrise, 7.0, 30.0), (Move::Brinejet, 4.0, 22.0), (Move::Inkveil, 9.0, 30.0)],
    }
}

/// How many arms it sends up round the player, angry or not
fn rising_spots(_look: Look, enraged: bool) -> usize {
    if enraged { 6 } else { 4 }
}

/// Which way round each turns things, its own
fn own_turn(boss: &Boss) -> f64 {
    ((boss.key[0] ^ boss.key[1].wrapping_mul(13)) as f64 * 0.29).rem_euclid(TAU)
}

/// Where its arms come up round the player at `aim`: on them, and in a ring round them, the first
/// of it on the side towards it, where it's seen coming
fn lash_spots(look: Look, boss: &Boss, from: V3, vpm: f64) -> Vec<V3> {
    let n = rising_spots(look, boss.enraged);
    let turn = heading_to(boss.aim, from) + 0.3 * (own_turn(boss) - PI).signum();
    std::iter::once(boss.aim)
        .chain((0..n - 1).map(|k| {
            let a = turn + k as f64 / (n - 1) as f64 * TAU;
            boss.aim + Vector3::new(a.cos(), a.sin(), 0.0) * 3.2 * vpm
        }))
        .collect()
}

/// How far round its whirlpool reaches, in meters
fn whirl_reach(look: Look) -> f64 {
    MAELSTROM_REACH * size(look).sqrt()
}

/// Shown on the ground and the water as it gets ready to make move `m`, `t` seconds into getting
/// ready of `whole`: where it'll come down, a ring closing in on each place, the water stirring
pub fn winding(m: Move, t: f64, whole: f64, a: &Animal, boss: &mut Boss, vpm: f64) {
    let closing = (1.0 - t / whole.max(0.1)).clamp(0.0, 1.0);
    match m {
        Move::Lashrise => {
            boss.trail -= 1.0 / 60.0;
            let stir = boss.trail <= 0.0;
            if stir {
                boss.trail = 0.1;
            }
            for at in lash_spots(a.look, boss, a.position, vpm) {
                boss.deeds.push(Deed::Mark { at, radius: 1.9, closing });
                if stir {
                    boss.deeds.push(Deed::Burrow { at });
                }
            }
        }
        Move::Maelstrom => boss.deeds.push(Deed::Mark { at: a.position, radius: whirl_reach(a.look), closing }),
        // Where its ink will come down
        Move::Inkveil => boss.deeds.push(Deed::Mark { at: boss.aim, radius: ink_radius(a.look), closing }),
        _ => {}
    }
}

/// How far round its cloud of ink spreads, in meters
pub fn ink_radius(look: Look) -> f64 {
    if look == Look::GrottoSquid { 3.4 } else { 5.0 }
}

/// It making move `m`, now it's ready
pub fn begin(m: Move, a: &mut Animal, boss: &mut Boss, forward: V3, vpm: f64) {
    let look = a.look;
    let k = size(look);
    let Some(body) = boss.kraken.as_deref_mut() else {
        return;
    };
    let (beak, siphon) = (body.beak, body.siphon);
    match m {
        // Its arms sent off under the water, to come up all round the player one after another
        Move::Lashrise => {
            let spots = {
                let b = &*boss;
                lash_spots(look, b, a.position, vpm)
            };
            let body = boss.kraken.as_deref_mut().unwrap();
            for (n, spot) in spots.into_iter().enumerate() {
                // From a little nearer it, so it comes up beside where it slams down
                let back = (a.position - spot).xy().try_normalize(1e-6).unwrap_or_else(|| -forward.xy()) * 1.6 * vpm;
                body.rising.push(Rising { base: spot + Vector3::new(back.x, back.y, 0.0), spot, t: -0.12 * n as f64, joints: Vec::new(), struck: false, surfaced: false });
            }
            boss.deeds.push(Deed::Kraken(Act::Surge { at: a.position, size: 1.2 * k }));
        }
        Move::Ensnare => {
            body.held = None;
            boss.deeds.push(Deed::Swipe(beak, 1));
        }
        // A cloud of ink squirted out of its siphon to come down over where the player stood
        Move::Inkveil => {
            let to = boss.aim + Vector3::new(0.0, 0.0, 1.0 * vpm);
            let time = ((to - siphon).norm() / vpm / 16.0).clamp(0.4, 1.6);
            boss.deeds.push(Deed::Kraken(Act::Ink { from: siphon, to, time, radius: ink_radius(look) }));
        }
        Move::Brinejet => boss.deeds.push(Deed::Kraken(Act::Surge { at: a.position, size: 0.8 * k })),
        Move::Maelstrom => {
            boss.spun_from = a.heading;
            boss.deeds.push(Deed::Kraken(Act::Surge { at: a.position, size: 1.5 * k }));
        }
        _ => {}
    }
}

/// It `t` seconds into making move `m`, in a frame of `dt` seconds
pub fn during(m: Move, t: f64, dt: f64, a: &mut Animal, boss: &mut Boss, player: Option<V3>, vpm: f64) {
    let look = a.look;
    let k = size(look);
    let crossed = |at: f64| t >= at && t - dt < at;
    let Some(body) = boss.kraken.as_deref_mut() else {
        return;
    };
    match m {
        // Its tentacle flung out at the player: seizing them if they're still where it reaches,
        // dragging them in to its beak and biting
        Move::Ensnare => {
            let tip = body.chains[ARMS][TENTACLE_PIECES];
            if crossed(ENSNARE_REACH) {
                let caught = player.filter(|p| (p + Vector3::new(0.0, 0.0, 1.0 * vpm) - tip).norm() < ENSNARE_CATCH * vpm || (p - boss.aim).xy().norm() < 1.2 * vpm);
                let caught = caught.filter(|p| (p - body.beak).norm() < tentacle_length(look) * 1.15 * vpm);
                body.held = caught.map(|_| 0.0);
                boss.deeds.push(Deed::Kraken(Act::Lash { from: body.beak, to: tip, caught: caught.is_some() }));
            }
            if let Some(held) = &mut body.held {
                *held += dt;
                boss.deeds.push(Deed::Kraken(Act::Drag { to: body.beak, dt }));
                if *held >= ENSNARE_HOLD {
                    body.held = None;
                    let (_, radius, damage, _) = slam(look);
                    let at = player.unwrap_or(body.beak);
                    boss.deeds.push(Deed::Slam { at, radius: radius * 0.8, damage: damage * 0.9, crater: 0.0 });
                    boss.deeds.push(Deed::Swipe(at, 2));
                }
            }
        }
        // Water jetted at the player out of its siphon, turning slowly after them
        Move::Brinejet => {
            if let Some(p) = player {
                let turn = (heading_to(a.position, p) - a.heading + PI).rem_euclid(TAU) - PI;
                a.target_heading = a.heading + turn.clamp(-JET_TURN * dt, JET_TURN * dt);
            }
            let from = body.beak;
            let forward = Vector3::new(a.heading.cos(), a.heading.sin(), 0.0);
            let to = player.map_or(from + forward * 10.0 * vpm, |p| p + Vector3::new(0.0, 0.0, 0.9 * vpm));
            let ahead = (to - from).xy().norm().max(1.0);
            let dir = Vector3::new(forward.x * ahead, forward.y * ahead, to.z - from.z).normalize();
            boss.deeds.push(Deed::Kraken(Act::Jet { from, dir, dt, reach: 18.0 * k.sqrt() }));
        }
        // The water spun round it, its arms sweeping round with it, whoever's in it drawn round and
        // in; then its beak snapping at whoever's been drawn close
        Move::Maelstrom => {
            a.target_heading = a.heading + 0.9 * dt;
            let r = whirl_reach(look);
            boss.deeds.push(Deed::Kraken(Act::Swirl { at: a.position, radius: r, dt }));
            let f = (t / MAELSTROM_SECONDS).min(1.0);
            boss.deeds.push(Deed::Mark { at: a.position, radius: r * (1.0 - 0.55 * f), closing: 1.0 - f });
            if crossed(MAELSTROM_SECONDS - 0.1) {
                let (reach, radius, damage, _) = slam(look);
                let forward = Vector3::new(a.heading.cos(), a.heading.sin(), 0.0);
                let at = a.position + forward * reach * 0.45 * vpm;
                boss.deeds.push(Deed::Slam { at, radius: radius * 1.3, damage, crater: 0.0 });
                boss.deeds.push(Deed::Kraken(Act::Surge { at, size: 1.2 * k }));
            }
        }
        _ => {}
    }
}

/// How long a tentacle reaches, all told, in meters
fn tentacle_length(look: Look) -> f64 {
    (0..TENTACLE_PIECES).map(|j| TENTACLE_ROOT * size(look) * tentacle_taper(j)).sum()
}

fn arm_taper(j: usize) -> f64 {
    1.0 - 0.55 * j as f64 / ARM_PIECES as f64
}

fn tentacle_taper(j: usize) -> f64 {
    0.85 - 0.4 * j as f64 / TENTACLE_PIECES as f64
}

fn rising_taper(j: usize) -> f64 {
    1.0 - 0.5 * j as f64 / RISING_PIECES as f64
}

/// What it does to the player that no other boss does, for src\game\brine.rs to see to. Positions
/// are in voxels.
#[derive(Clone, Copy, Debug)]
pub enum Act {
    /// Breaking up through the top of the water at `at`, `size` big: itself rising out of it, an
    /// arm sent up out of it, or its whirlpool starting
    Surge { at: V3, size: f64 },
    /// A tentacle flung out from `from` to `to`, and whether it seized the player
    Lash { from: V3, to: V3, caught: bool },
    /// The player held in a tentacle, dragged in towards its beak at `to` for `dt` seconds
    Drag { to: V3, dt: f64 },
    /// Ink squirted from `from` to come down at `to` in `time` seconds and spread `radius` meters
    /// round in a cloud
    Ink { from: V3, to: V3, time: f64, radius: f64 },
    /// Water jetted out of `from` along `dir` for `dt` seconds, `reach` meters
    Jet { from: V3, dir: V3, dt: f64, reach: f64 },
    /// The water spun round `at` for `dt` seconds, `radius` meters round, drawing in whoever's in it
    Swirl { at: V3, radius: f64, dt: f64 },
}

/// One of its arms sent up out of the water round the player: where it comes up out of and where
/// it comes down, in voxels, seconds since it started rising, where each of its joints is, and
/// whether it's come down and come up yet
pub struct Rising {
    base: V3,
    spot: V3,
    t: f64,
    joints: Vec<V3>,
    struck: bool,
    surfaced: bool,
}

/// Its whole body as it's drawn: its head and its mantle, which way each is turned, and every joint
/// of every arm and tentacle, the arms first, in voxels; what it's reaching for with each; its beak
/// and siphon, which what it does comes out of; and its arms sent up round the player
pub struct Body {
    look: Look,
    pub head: V3,
    turn: UnitQuaternion<f64>,
    mantle: V3,
    mantle_turn: UnitQuaternion<f64>,
    chains: Vec<Vec<V3>>,
    tips: Vec<V3>,
    pub beak: V3,
    pub siphon: V3,
    /// How far it's reared up out of the water, 0 to 1, and its mantle swollen, -1 to 1
    rear: f64,
    pulse: f64,
    clock: f64,
    /// How far round its arms have swept, spinning the whirlpool
    spin: f64,
    pub rising: Vec<Rising>,
    /// Seconds it's held the player in its tentacle, if it has them
    pub held: Option<f64>,
    /// Whether it's up out of the water, to splash as it comes up
    up: bool,
    laid: bool,
}

impl Body {
    pub fn new(look: Look) -> Option<Body> {
        if !has(look) {
            return None;
        }
        let chains: Vec<Vec<V3>> = (0..ARMS + TENTACLES).map(|c| vec![V3::zeros(); pieces_of(c) + 1]).collect();
        Some(Body {
            look,
            head: V3::zeros(),
            turn: UnitQuaternion::identity(),
            mantle: V3::zeros(),
            mantle_turn: UnitQuaternion::identity(),
            tips: vec![V3::zeros(); chains.len()],
            chains,
            beak: V3::zeros(),
            siphon: V3::zeros(),
            rear: 0.0,
            pulse: 0.0,
            clock: 0.0,
            spin: 0.0,
            rising: Vec::new(),
            held: None,
            up: false,
            laid: false,
        })
    }

    /// The world moved over by `shift` voxels
    pub fn recenter(&mut self, shift: V3) {
        self.head -= shift;
        self.mantle -= shift;
        self.beak -= shift;
        self.siphon -= shift;
        for c in &mut self.chains {
            for p in c.iter_mut() {
                *p -= shift;
            }
        }
        for t in &mut self.tips {
            *t -= shift;
        }
        for r in &mut self.rising {
            r.base -= shift;
            r.spot -= shift;
            for p in &mut r.joints {
                *p -= shift;
            }
        }
    }

    /// Whether it's been posed where it is yet, see pose()
    pub fn placed(&self) -> bool {
        self.laid
    }

    /// Where along it to try hitting, in voxels: its head, its mantle and its arms near it
    pub fn hit_points(&self, vpm: f64) -> Vec<V3> {
        let k = size(self.look);
        let mut out = vec![self.head, self.mantle + self.mantle_turn * Vector3::new(-0.4 * MANTLE * k, 0.0, 0.0) * vpm];
        out.extend(self.chains.iter().map(|c| c[1]));
        out
    }
}

/// How many pieces chain `c` of its arms and tentacles has
fn pieces_of(c: usize) -> usize {
    if c < ARMS { ARM_PIECES } else { TENTACLE_PIECES }
}

/// How long piece `j` of chain `c` is, in meters
fn length_of(look: Look, c: usize, j: usize) -> f64 {
    let k = size(look);
    if c < ARMS { ARM_ROOT * k * arm_taper(j) } else { TENTACLE_ROOT * k * tentacle_taper(j) }
}

/// Chain `joints`, `lengths` long between them, its first fixed, reaching its last for `target` as
/// near as it reaches, each joint following the one before it from where it was: so it bends
/// smoothly after what it's reaching for rather than snapping straight
fn reach_for(joints: &mut [V3], lengths: &[f64], target: V3) {
    let n = lengths.len();
    let root = joints[0];
    let total: f64 = lengths.iter().sum();
    if (target - root).norm() >= total * 0.999 {
        let way = (target - root).try_normalize(1e-9).unwrap_or_else(V3::z);
        for j in 0..n {
            joints[j + 1] = joints[j] + way * lengths[j];
        }
        return;
    }
    for _ in 0..4 {
        joints[n] = target;
        for j in (0..n).rev() {
            let way = (joints[j] - joints[j + 1]).try_normalize(1e-9).unwrap_or_else(V3::z);
            joints[j] = joints[j + 1] + way * lengths[j];
        }
        joints[0] = root;
        for j in 0..n {
            let way = (joints[j + 1] - joints[j]).try_normalize(1e-9).unwrap_or_else(V3::z);
            joints[j + 1] = joints[j] + way * lengths[j];
        }
    }
}

/// Turned to face along `way`, round and up, never rolled over
fn along(way: V3) -> UnitQuaternion<f64> {
    let yaw = way.y.atan2(way.x);
    let pitch = -(way.z / way.norm().max(1e-9)).clamp(-1.0, 1.0).asin();
    UnitQuaternion::from_axis_angle(&Vector3::z_axis(), yaw) * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), pitch)
}

fn rz(a: f64) -> UnitQuaternion<f64> {
    UnitQuaternion::from_axis_angle(&Vector3::z_axis(), a)
}

fn ry(a: f64) -> UnitQuaternion<f64> {
    UnitQuaternion::from_axis_angle(&Vector3::y_axis(), a)
}

fn ease(f: f64) -> f64 {
    let f = f.clamp(0.0, 1.0);
    f * f * (3.0 - 2.0 * f)
}

/// The top of the water under `at`, or the ground if there's none over it, in voxels
fn surface(world: &World, water: WaterLevel, at: V3) -> f64 {
    let vpm = world.voxels_per_meter as f64;
    let ground = ground_under(world, at + Vector3::new(0.0, 0.0, 3.0 * vpm)).map_or(at.z, |g| g.z);
    let w = water(at.x, at.y);
    if w.is_finite() && w > ground { w } else { ground }
}

/// Kraken `a` posed for what it's doing this frame, `dt` seconds on: its head and mantle risen out
/// of the water as far as it's reared, its arms reaching for what its move wants of them, its
/// arms sent up round the player rising and slamming down. What they do as they come down goes out
/// as its deeds. Each frame after it's thought, see animals::Animal::update().
pub fn animate(a: &mut Animal, world: &World, water: WaterLevel, player: Option<V3>, dt: f64) {
    let Some(mut boss) = a.boss.take() else {
        return;
    };
    if let Some(mut body) = boss.kraken.take() {
        pose(a, &mut boss, &mut body, world, water, player, dt);
        boss.kraken = Some(body);
    }
    a.boss = Some(boss);
}

fn pose(a: &Animal, boss: &mut Boss, body: &mut Body, world: &World, water: WaterLevel, player: Option<V3>, dt: f64) {
    let vpm = world.voxels_per_meter as f64;
    let look = a.look;
    let k = size(look);
    body.clock += dt;
    let heading = a.drawn_heading();
    let forward = Vector3::new(heading.cos(), heading.sin(), 0.0);
    let up = V3::z();
    let doing = if a.health > 0.0 { boss.doing() } else { None };
    let dying = boss.dying.filter(|_| a.health <= 0.0);
    let top = {
        let w = water(a.position.x, a.position.y);
        if w.is_finite() && w > a.position.z - 1.0 * vpm { w } else { a.position.z }
    };
    // How far it rears up out of the water for what it's doing
    use crate::rig::Stage;
    let rear_to = match (doing, dying) {
        (_, Some(t)) => if t < 0.7 { 1.0 } else { 0.0 },
        (Some(d), _) if matches!(d.m, Move::Brinejet | Move::Inkveil | Move::Slam | Move::Roar) && d.stage != Stage::Recover => 0.95,
        (Some(d), _) if matches!(d.m, Move::Ensnare | Move::Lashrise) => 0.7,
        (Some(d), _) if d.m == Move::Maelstrom => 0.35,
        _ if boss.engaged => 0.5,
        _ => 0.15,
    };
    body.rear += (rear_to - body.rear) * (1.0 - (-3.0 * dt).exp());
    // Its mantle swelling as it draws in water and squeezing as it squirts it, and breathing
    let pulse_to = match doing {
        Some(d) if matches!(d.m, Move::Inkveil | Move::Brinejet) && d.stage == Stage::Wind => 1.0,
        Some(d) if matches!(d.m, Move::Inkveil | Move::Brinejet) && d.stage == Stage::Act => -1.0,
        _ => 0.0,
    };
    body.pulse += (pulse_to - body.pulse) * (1.0 - (-6.0 * dt).exp());
    let sink = boss.sink + dying.map_or(0.0, |t| ((t - 1.0) / (THROES - 1.0)).clamp(0.0, 1.0) * 5.0 * k);
    // Its head half out of the water lying low, its eyes over it, high out of it reared
    let lift = (0.15 + 1.15 * body.rear) * k - sink;
    body.head = Vector3::new(a.position.x, a.position.y, top) - forward * 0.3 * body.rear * k * vpm + up * lift * vpm;
    body.turn = rz(heading) * ry(-0.2 * body.rear + 0.04 * (body.clock * 1.1).sin());
    body.mantle = body.head + body.turn * Vector3::new(-0.85 * HEAD * k, 0.0, 0.25 * HEAD * k) * vpm;
    let tilt = 0.12 + 1.0 * body.rear + 0.05 * (body.clock * 1.3).sin();
    body.mantle_turn = rz(heading + 0.06 * (body.clock * 0.7).sin()) * ry(tilt);
    body.beak = body.head + body.turn * Vector3::new(1.45 * k, 0.0, -0.35 * k) * vpm;
    body.siphon = body.head + body.turn * Vector3::new(0.55 * k, 0.0, -1.0 * k) * vpm;
    // Up out of the water now, throwing it off
    let risen = sink < 0.6 * k && a.health > 0.0;
    if risen && !body.up {
        boss.deeds.push(Deed::Kraken(Act::Surge { at: Vector3::new(body.head.x, body.head.y, top), size: 1.6 * k }));
    }
    body.up = risen;
    // Its arms spread round it on the water, a few raised and waving, each reaching for where what
    // it's doing wants it
    let engaged = boss.engaged;
    let spinning = matches!(doing, Some(d) if d.m == Move::Maelstrom && d.stage == Stage::Act);
    if spinning {
        body.spin += 2.4 * dt;
    } else {
        body.spin *= (-1.5 * dt).exp();
    }
    for c in 0..ARMS + TENTACLES {
        let n = pieces_of(c);
        let lengths: Vec<f64> = (0..n).map(|j| length_of(look, c, j) * vpm).collect();
        let longest: f64 = lengths.iter().sum();
        let ring = if c < ARMS { TAU * c as f64 / ARMS as f64 + 0.2 } else { -PI / 2.0 + if c == ARMS { -0.35 } else { 0.35 } };
        let root = body.head + body.turn * Vector3::new(0.95 * HEAD * k, 0.6 * HEAD * k * ring.cos(), 0.5 * HEAD * k * ring.sin() - 0.15 * k) * vpm;
        let side = if c < ARMS { (c as f64 - 3.5) / 3.5 } else if c == ARMS { -0.25 } else { 0.25 };
        let wave = (body.clock * 0.8 + c as f64 * 1.3).sin();
        let fan = match doing {
            Some(d) if d.m == Move::Brinejet => 2.4,
            Some(d) if d.m == Move::Inkveil => 0.8,
            _ => 1.3,
        };
        let turn = heading + side * fan + 0.18 * wave + body.spin;
        let out = Vector3::new(turn.cos(), turn.sin(), 0.0);
        let raised = if c < ARMS && c % 2 == 1 { (if engaged { 3.0 } else { 0.7 } + 1.1 * (body.clock * 0.9 + c as f64).sin()).max(0.0) * k } else { 0.0 };
        let mut want = root + out * longest * if c < ARMS { 0.7 } else { 0.5 } + up * ((top - root.z) / vpm - 0.25 * k + raised) * vpm;
        if spinning {
            want += up * 0.8 * k * vpm;
        }
        let mut quick = 4.0;
        match doing {
            // Two arms raised high over the water, then brought down on what's in front
            Some(d) if d.m == Move::Slam && (c == 3 || c == 4) => {
                let (reach, _, _, _) = slam(look);
                match d.stage {
                    Stage::Wind => want = root + up * 5.0 * k * vpm + forward * 0.8 * k * vpm,
                    _ => {
                        want = a.position + forward * reach * vpm + up * ((top - a.position.z) / vpm) * vpm;
                        quick = 14.0;
                    }
                }
            }
            // A tentacle drawn back high, then flung out at the player, holding them if it caught
            // them, or drawn back in if it didn't
            Some(d) if d.m == Move::Ensnare && c == ARMS => match d.stage {
                Stage::Wind => want = root + up * 3.2 * k * vpm - forward * 1.0 * k * vpm,
                Stage::Act => {
                    quick = 16.0;
                    want = match (body.held, player) {
                        (Some(_), Some(p)) => p + up * 1.0 * vpm,
                        _ if d.t < ENSNARE_REACH => boss.aim + up * 1.0 * vpm,
                        _ => root + forward * 2.0 * k * vpm + up * 1.0 * k * vpm,
                    };
                }
                _ => {}
            },
            // Its arms curled in round it, squeezing
            Some(d) if d.m == Move::Inkveil && c < ARMS => want = root + out * longest * 0.35 + up * 0.8 * k * vpm,
            // Some of its arms gone off under the water, to come up round the player
            Some(d) if d.m == Move::Lashrise && c < ARMS && c % 2 == 0 => want = root + out * longest * 0.4 - up * 3.5 * k * vpm,
            _ => {}
        }
        if dying.is_some() {
            want = root + out * longest * 0.6 + up * ((top - root.z) / vpm - 1.0 * k) * vpm;
        }
        if !body.laid {
            body.tips[c] = want;
            let joints = &mut body.chains[c];
            for (j, p) in joints.iter_mut().enumerate() {
                *p = root + out * (j as f64 * longest / n as f64);
            }
        }
        let was = body.tips[c];
        body.tips[c] = was + (want - was) * (1.0 - (-quick * dt).exp());
        let joints = &mut body.chains[c];
        joints[0] = root;
        // A little weight in it, sagging down to the water between its ends
        for p in joints.iter_mut().skip(1) {
            p.z -= 0.6 * k * vpm * dt;
        }
        let tip = body.tips[c];
        reach_for(joints, &lengths, tip);
    }
    body.laid = true;
    // Its arms sent up round the player: up out of the water beside where they'll come down, rising
    // high and curling over, slamming down there, lying a moment, and gone back under
    let rlengths: Vec<f64> = (0..RISING_PIECES).map(|j| RISING_ROOT * k * rising_taper(j) * vpm).collect();
    let tall: f64 = rlengths.iter().sum();
    for r in &mut body.rising {
        r.t += dt;
        if r.t < 0.0 {
            continue;
        }
        let base_top = surface(world, water, r.base);
        let spot_top = surface(world, water, r.spot);
        let sunk = ((r.t - RISE_LIE) / (RISE_GONE - RISE_LIE)).clamp(0.0, 1.0) * tall * 0.9;
        let root = Vector3::new(r.base.x, r.base.y, base_top - 0.6 * vpm - sunk);
        if r.joints.is_empty() {
            r.joints = (0..=RISING_PIECES).map(|j| root - up * j as f64 * 0.3 * vpm).collect();
        }
        if !r.surfaced {
            r.surfaced = true;
            boss.deeds.push(Deed::Kraken(Act::Surge { at: Vector3::new(root.x, root.y, base_top), size: 0.9 * k }));
        }
        let toward = (r.spot - r.base).xy().try_normalize(1e-6).unwrap_or_else(|| forward.xy());
        let toward = Vector3::new(toward.x, toward.y, 0.0);
        let want = if r.t < 0.5 {
            root + up * tall * (0.3 + 0.6 * ease(r.t / 0.5)) - toward * 0.6 * vpm
        } else if r.t < RISE_SLAM - 0.05 {
            r.spot + up * (spot_top - r.spot.z + 3.2 * vpm)
        } else {
            Vector3::new(r.spot.x, r.spot.y, spot_top + 0.3 * vpm)
        };
        r.joints[0] = root;
        reach_for(&mut r.joints, &rlengths, want);
        if !r.struck && r.t >= RISE_SLAM {
            r.struck = true;
            let at = Vector3::new(r.spot.x, r.spot.y, spot_top);
            let damage = if boss.enraged { 24.0 } else { 20.0 };
            boss.deeds.push(Deed::Slam { at, radius: 1.9, damage, crater: 0.0 });
            boss.deeds.push(Deed::Kraken(Act::Surge { at, size: 1.1 * k }));
        }
    }
    body.rising.retain(|r| r.t < RISE_GONE);
}

/// Whether kraken `a` is still sinking away, killed, so it's not to be taken away yet
pub fn dying(a: &Animal) -> bool {
    has(a.look) && a.boss.as_ref().is_some_and(|b| b.dying.is_none_or(|t| t < THROES))
}

// Its models

/// The shapes of each of its parts, in meters at its size
fn part_shapes(look: Look, part: usize) -> Vec<Part> {
    let h = hide(look);
    let k = size(look);
    let mut p = Vec::new();
    match part {
        MANTLE_MODEL => {
            let l = MANTLE * k;
            // Fins either side of its tip, and the spots and lights down it
            for s in [-1.0, 1.0] {
                p.push(ellipsoid([-0.84 * l, s * 0.75 * k, 0.05 * k], [0.62 * k, 0.72 * k, 0.1 * k], h.dark));
                for j in 0..5 {
                    let x = -0.15 * l - 0.13 * l * j as f64;
                    let r = 1.2 * k * (1.0 - 0.13 * j as f64);
                    p.push(ball([x, s * 0.82 * r, 0.12 * k], 0.09 * k, h.glow));
                }
            }
            for (x, y, z, r) in [(0.2, 0.25, 0.95, 0.22), (0.35, -0.3, 0.85, 0.18), (0.5, 0.15, 0.75, 0.16), (0.62, -0.1, 0.55, 0.13), (0.28, -0.55, 0.6, 0.15)] {
                p.push(ball([-x * l, y * k, z * k], r * k, h.dark));
            }
            if look == Look::Kraken {
                // Barnacles crusting it, and the old harpoon still stuck in it
                for (x, y, z) in [(0.1, 0.5, 0.95), (0.14, 0.62, 0.8), (0.18, 0.4, 1.02), (0.45, -0.45, 0.7), (0.48, -0.55, 0.55), (0.7, 0.2, 0.42)] {
                    p.push(ball([-x * l, y * k, z * k], 0.14 * k, h.crust));
                }
                p.push(limb([-0.3 * l, 0.15 * k, 1.05 * k], [-0.12 * l, 0.75 * k, 2.6 * k], 0.06 * k, 0.05 * k, DARK_STEEL));
                p.push(limb([-0.33 * l, 0.12 * k, 0.95 * k], [-0.28 * l, 0.42 * k, 1.25 * k], 0.04 * k, 0.04 * k, BRONZE));
                p.push(limb([-0.12 * l, 0.75 * k, 2.6 * k], [-0.04 * l, 0.95 * k, 2.95 * k], 0.04 * k, 0.03 * k, WOOD));
            }
            p.push(limb([0.0, 0.0, -0.35 * k], [-0.7 * l, 0.0, -0.25 * k], 1.0 * k, 0.6 * k, h.belly));
            p.push(limb([0.0, 0.0, 0.0], [-0.55 * l, 0.0, 0.05 * k], 1.25 * k, 1.05 * k, h.skin));
            p.push(limb([-0.55 * l, 0.0, 0.05 * k], [-l, 0.0, 0.0], 1.05 * k, 0.22 * k, h.skin));
        }
        HEAD_MODEL => {
            let r = HEAD * k;
            for s in [-1.0, 1.0] {
                // Its great eyes, a bar of a pupil across each, under a heavy brow
                p.push(ellipsoid([0.28 * r, s * 1.12 * r, 0.22 * r], [0.12 * r, 0.06 * r, 0.22 * r], OBSIDIAN));
                p.push(ball([0.22 * r, s * 0.92 * r, 0.22 * r], 0.38 * r, h.eye));
                p.push(limb([-0.15 * r, s * 0.85 * r, 0.62 * r], [0.6 * r, s * 0.78 * r, 0.55 * r], 0.17 * r, 0.11 * r, h.dark));
                p.push(ball([-0.45 * r, s * 0.6 * r, 0.75 * r], 0.08 * r, h.glow));
            }
            // Its beak, black and hooked, in the middle of where its arms come out
            p.push(limb([1.15 * r, 0.0, -0.18 * r], [1.5 * r, 0.0, -0.4 * r], 0.2 * r, 0.04 * r, OBSIDIAN));
            p.push(limb([1.1 * r, 0.0, -0.42 * r], [1.38 * r, 0.0, -0.36 * r], 0.14 * r, 0.03 * r, OBSIDIAN));
            // The web its arms come out of, and its siphon under its head
            p.push(ellipsoid([0.9 * r, 0.0, -0.1 * r], [0.42 * r, 0.88 * r, 0.78 * r], h.skin));
            p.push(limb([-0.3 * r, 0.0, -0.75 * r], [0.5 * r, 0.0, -0.95 * r], 0.3 * r, 0.22 * r, h.dark));
            p.push(ellipsoid([0.0, 0.0, -0.25 * r], [0.95 * r, 1.0 * r, 0.7 * r], h.belly));
            p.push(ellipsoid([0.0, 0.0, 0.0], [1.1 * r, 1.05 * r, 0.95 * r], h.skin));
        }
        PIECE_MODEL => {
            // A piece of an arm along +x, as long as the first of them, suckers under it
            let s = ARM_ROOT * k;
            for x in [0.15, 0.45, 0.75] {
                p.push(ball([x * s, 0.0, -0.2 * s], 0.065 * s, h.sucker));
                p.push(ball([(x + 0.15) * s, 0.09 * s, -0.17 * s], 0.05 * s, h.sucker));
                p.push(ball([(x + 0.15) * s, -0.09 * s, -0.17 * s], 0.05 * s, h.sucker));
            }
            p.push(ball([0.5 * s, 0.0, 0.2 * s], 0.06 * s, h.dark));
            p.push(limb([0.0, 0.0, -0.07 * s], [s, 0.0, -0.06 * s], 0.2 * s, 0.17 * s, h.belly));
            p.push(limb([0.0, 0.0, 0.0], [s, 0.0, 0.0], 0.25 * s, 0.22 * s, h.skin));
        }
        _ => {
            // A tentacle's club: a broad pad of suckers ringed with hooks, tapering to its tip
            let s = ARM_ROOT * k;
            for x in [0.2, 0.4, 0.6, 0.8] {
                for y in [-0.12, 0.12] {
                    p.push(ball([x * s, y * s, -0.15 * s], 0.06 * s, h.sucker));
                    p.push(limb([x * s, y * 1.9 * s, -0.08 * s], [x * s + 0.05 * s, y * 2.2 * s, -0.22 * s], 0.035 * s, 0.01 * s, BONE));
                }
            }
            p.push(limb([0.8 * s, 0.0, 0.0], [1.3 * s, 0.0, 0.0], 0.13 * s, 0.03 * s, h.skin));
            p.push(ellipsoid([0.5 * s, 0.0, 0.0], [0.55 * s, 0.32 * s, 0.17 * s], h.skin));
            p.push(ball([0.5 * s, 0.0, 0.1 * s], 0.08 * s, h.glow));
        }
    }
    p
}

/// One of its models: where it's put from the point it's made round, and how far any of it reaches
/// from that point, in meters
struct Shape {
    anchor: V3,
    furthest: f64,
}

fn parts_made() -> &'static (Vec<Model>, Vec<Shape>) {
    static MADE: OnceLock<(Vec<Model>, Vec<Shape>)> = OnceLock::new();
    MADE.get_or_init(|| {
        let sets: Vec<Vec<Part>> = LOOKS.iter().flat_map(|&l| (0..PARTS).map(move |p| part_shapes(l, p))).collect();
        let mut made: Vec<(Model, V3)> = std::thread::scope(|scope| {
            let handles: Vec<_> = sets.iter().map(|parts| scope.spawn(move || voxelize(parts, MODEL_RES))).collect();
            handles.into_iter().map(|h| h.join().expect("making a kraken's model failed")).collect()
        });
        // Pieces drawn from files in assets\models, see src\assets.rs
        crate::assets::krakens(&mut made);
        let shapes = made
            .iter()
            .map(|(model, anchor)| {
                let size = Vector3::new(model.size[0] as f64, model.size[1] as f64, model.size[2] as f64) / MODEL_RES;
                let lo = anchor - Vector3::new(size.x * 0.5, size.y * 0.5, 0.0);
                let corners = (0..8).map(|c| Vector3::new(if c & 1 == 0 { lo.x } else { lo.x + size.x }, if c & 2 == 0 { lo.y } else { lo.y + size.y }, if c & 4 == 0 { lo.z } else { lo.z + size.z }));
                Shape { anchor: *anchor, furthest: corners.map(|c| c.norm()).fold(0.0, f64::max) }
            })
            .collect();
        (made.into_iter().map(|(m, _)| m).collect(), shapes)
    })
}

/// Its models, to go after the pieces of the baked frames, see animals::animal_models()
pub fn models() -> Vec<Model> {
    parts_made().0.iter().map(|m| Model { size: m.size, voxels: m.voxels.clone() }).collect()
}

/// Where each of its models has the middle of its bottom, from the point it's made round, in
/// meters: for the model files, see src\assets.rs
pub fn anchors() -> Vec<V3> {
    parts_made().1.iter().map(|s| s.anchor).collect()
}

/// The instance drawing part `part` of `look` with the point it's made round at `at`, in voxels,
/// turned by `rotation` and `scale` times as big
fn place(look: Look, part: usize, at: V3, rotation: UnitQuaternion<f64>, scale: f64, vpm: f64) -> AnimalInstance {
    let index = LOOKS.iter().position(|&l| l == look).unwrap_or(0) * PARTS + part;
    let shape = &parts_made().1[index];
    let position = at + rotation * shape.anchor * scale * vpm;
    let model = (crate::animals::KRAKEN_MODELS_AT + index) as u32;
    AnimalInstance::new(model, position, rotation, vpm / MODEL_RES * scale, (shape.furthest + shape.anchor.norm()) * scale * vpm, MOVING)
}

impl Body {
    /// Its mantle, its head and every piece of its arms and tentacles, and its arms sent up round
    /// the player, to draw, tinted as weapons have left it, see animals::TINT_NONE
    pub fn instances(&self, vpm: f64, tint: u32, out: &mut Vec<AnimalInstance>) {
        let look = self.look;
        let k = size(look);
        let first = ARM_ROOT * k;
        let from = out.len();
        out.push(place(look, MANTLE_MODEL, self.mantle, self.mantle_turn, 1.0 + 0.06 * self.pulse + 0.015 * (self.clock * 1.6).sin(), vpm));
        out.push(place(look, HEAD_MODEL, self.head, self.turn, 1.0, vpm));
        for (c, joints) in self.chains.iter().enumerate() {
            let n = pieces_of(c);
            for j in 0..n {
                let way = joints[j + 1] - joints[j];
                let tip = c >= ARMS && j + 1 == n;
                let scale = length_of(look, c, j) / first;
                out.push(place(look, if tip { CLUB_MODEL } else { PIECE_MODEL }, joints[j], along(way), scale, vpm));
            }
        }
        for r in self.rising.iter().filter(|r| !r.joints.is_empty()) {
            for j in 0..RISING_PIECES {
                let way = r.joints[j + 1] - r.joints[j];
                let scale = RISING_ROOT * k * rising_taper(j) / first;
                out.push(place(look, PIECE_MODEL, r.joints[j], along(way), scale, vpm));
            }
        }
        for i in &mut out[from..] {
            *i = i.tinted(tint);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lake 192 m across at 8 voxels a meter, its bottom 4 m up and its top 12 m up
    fn lake() -> World {
        let size = [1536u32, 1536, 128];
        let (tx, ty) = (size[0] as usize / 4, size[1] as usize / 4);
        let mut occupancy = vec![0u128; tx * ty * (size[2] as usize / 8)];
        occupancy[..tx * ty * 4].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, SAND);
        world.voxels_per_meter = 8;
        world
    }

    /// However it's posed, every piece of its arms is as long as it's meant to be, each joined to the
    /// one before it, rather than stretched apart or pulled through each other
    #[test]
    fn its_arms_hold_together() {
        let world = lake();
        let vpm = 8.0;
        let water = |_: f64, _: f64| 12.0 * vpm;
        for look in LOOKS {
            let home = Vector3::new(96.0, 96.0, 12.0) * vpm;
            let mut kraken = Animal::boss(look, home, [1, 2, -1], 5);
            kraken.boss.as_mut().unwrap().provoke();
            let player = Some(home + Vector3::new(9.0, 2.0, 0.0) * vpm);
            for _ in 0..400 {
                kraken.update(&world, &water, player, 1.0 / 30.0);
                kraken.boss.as_mut().unwrap().deeds.clear();
            }
            let body = kraken.kraken().unwrap();
            for (c, joints) in body.chains.iter().enumerate() {
                for j in 0..pieces_of(c) {
                    let gap = (joints[j + 1] - joints[j]).norm() / vpm;
                    let meant = length_of(look, c, j);
                    assert!((gap - meant).abs() < 0.05 * meant + 0.02, "{look:?}'s arm {c} piece {j} is {gap:.2} m, not {meant:.2}");
                }
            }
            let mut drawn = Vec::new();
            body.instances(vpm, 0, &mut drawn);
            assert!(drawn.len() >= 2 + ARMS * ARM_PIECES && drawn.len() < 90, "{look:?} draws {} pieces", drawn.len());
        }
    }

    /// Lying in wait it's sunk under the water; turned on, it rises up out of it, splashing, and its
    /// arms sent up round the player come up out of the water and slam down where they showed
    #[test]
    fn it_rises_and_its_arms_come_down_where_they_showed() {
        let world = lake();
        let vpm = 8.0;
        let water = |_: f64, _: f64| 12.0 * vpm;
        let home = Vector3::new(96.0, 96.0, 12.0) * vpm;
        let mut kraken = Animal::boss(Look::Kraken, home, [1, 2, -1], 5);
        for _ in 0..90 {
            kraken.update(&world, &water, None, 1.0 / 30.0);
        }
        let lying = kraken.kraken().unwrap().head.z;
        assert!(lying < 12.0 * vpm, "it lies in wait under the water");
        let boss = kraken.boss.as_mut().unwrap();
        boss.deeds.clear();
        boss.forced = Some(Move::Lashrise);
        boss.only = Some(Move::Lashrise);
        let player = Some(home + Vector3::new(16.0, 0.0, 0.0) * vpm);
        let (mut marks, mut slams, mut surged) = (Vec::new(), Vec::new(), false);
        for _ in 0..300 {
            kraken.update(&world, &water, player, 1.0 / 30.0);
            for d in kraken.boss.as_mut().unwrap().deeds.drain(..) {
                match d {
                    Deed::Mark { at, .. } => marks.push(at),
                    Deed::Slam { at, damage, .. } if damage > 0.0 => slams.push(at),
                    Deed::Kraken(Act::Surge { .. }) => surged = true,
                    _ => {}
                }
            }
        }
        assert!(surged, "it splashed up out of the water");
        assert!(kraken.kraken().unwrap().head.z > lying + 1.0 * vpm, "it rose up");
        assert!(slams.len() >= 4, "only {} arms came down", slams.len());
        for s in &slams {
            assert!(marks.iter().any(|m| (m - s).xy().norm() < 0.3 * vpm), "an arm came down where none showed");
        }
    }

    /// Flinging its tentacle at the player standing still, it catches them and drags them in, then
    /// bites; at the player who's stepped away, it misses
    #[test]
    fn its_tentacle_catches_whoever_stays() {
        let world = lake();
        let vpm = 8.0;
        let water = |_: f64, _: f64| 12.0 * vpm;
        for stay in [true, false] {
            let home = Vector3::new(96.0, 96.0, 12.0) * vpm;
            let mut kraken = Animal::boss(Look::Kraken, home, [1, 2, -1], 5);
            let boss = kraken.boss.as_mut().unwrap();
            boss.provoke();
            boss.forced = Some(Move::Ensnare);
            boss.only = Some(Move::Ensnare);
            let mut player = home + Vector3::new(10.0, 0.0, 0.0) * vpm;
            let (mut caught, mut drags) = (None, 0);
            for _ in 0..300 {
                if !stay && matches!(kraken.boss.as_ref().unwrap().doing(), Some(d) if d.m == Move::Ensnare && d.stage == crate::rig::Stage::Act) {
                    player.y += 5.0 * vpm;
                }
                kraken.update(&world, &water, Some(player), 1.0 / 30.0);
                for d in kraken.boss.as_mut().unwrap().deeds.drain(..) {
                    match d {
                        Deed::Kraken(Act::Lash { caught: c, .. }) => caught = caught.or(Some(c)),
                        Deed::Kraken(Act::Drag { .. }) => drags += 1,
                        _ => {}
                    }
                }
                if caught.is_some() && drags > 30 {
                    break;
                }
            }
            assert_eq!(caught, Some(stay), "stayed {stay}");
            assert_eq!(drags > 0, stay);
        }
    }
}
