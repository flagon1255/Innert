//! Bosses drawn out of jointed pieces rather than baked frames: the golems, the great bears, the
//! Broodmother and the Rimeweaver, the Ancient Treant, the alpha wolves, the knights of the dead,
//! the warlord and the necromancer, and the bosses with builds of their own. Each piece, a torso,
//! a head, an upper arm, a thigh, is voxelized once at its size, see models(), and every frame each
//! is turned about its joint and drawn where the pose puts it, see instances(), as a dragon's are,
//! see src\dragons.rs. So they're posed afresh every frame: walking with their weight going from
//! foot to foot, breathing and looking about as they stand, gathering themselves before every blow
//! and following through after it, flinching and reeling when they're hit hard, roaring between the
//! phases of a raid and falling when they die, rather than snapping between a few frames.
//!
//! What a boss is up to comes from src\bosses.rs as a Moment; how it holds itself for that is a pose
//! of its build's, eased from frame to frame so nothing ever snaps, with its walk, its breath and
//! how it flinches laid over it; and the pose is worked out into where each piece goes, its Bones.
//! What walks on two legs is posed in biped.rs, and each kind's pieces and moves are in a file of
//! their own.

use std::f64::consts::{PI, TAU};
use std::sync::OnceLock;

use bytemuck::Pod;
use nalgebra::{Matrix3, Rotation3, UnitQuaternion, Vector3};

use crate::animals::{self, Animal, AnimalInstance, Look, MODEL_RES, MOVING, Model, Part};
use crate::bosses::{Carry, Move};
use crate::worldgen::material::AIR;

mod beast;
mod biped;
mod caster;
mod crawler;
mod giant;
mod golem;
mod knight;
mod treant;

pub type V = Vector3<f64>;
pub type Q = UnitQuaternion<f64>;

/// Every boss drawn jointed, in the order their pieces are among the models
pub const KINDS: [Look; 23] = [
    Look::Treant,
    Look::Spider,
    Look::Rimeweaver,
    Look::Warlord,
    Look::Necromancer,
    Look::Ironhide,
    Look::Frostfang,
    Look::Direwolf,
    Look::WinterAlpha,
    Look::Golem,
    Look::FrostGolem,
    Look::MagmaGolem,
    Look::Colossus,
    Look::GraveKnight,
    Look::CryptTitan,
    Look::Ogre,
    Look::RimeJotun,
    Look::StormGiant,
    Look::SandPharaoh,
    Look::BogHag,
    Look::HollowMother,
    Look::Elderhorn,
    Look::BleedingHart,
];

/// Pieces each has room for among the models, those it doesn't have left empty
pub const PIECES: usize = 24;

/// How many models the jointed bosses have, after the frames of every look, see
/// animals::RIG_MODELS_AT
pub const MODELS: usize = KINDS.len() * PIECES;

/// Which of KINDS `look` is, if it's drawn jointed
pub fn kind(look: Look) -> Option<usize> {
    KINDS.iter().position(|&k| k == look)
}

/// Whether `look` is drawn out of jointed pieces here
pub fn rigged(look: Look) -> bool {
    kind(look).is_some()
}

/// How a boss is built, which decides how it's posed
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Family {
    Golem,
    Knight,
    Giant,
    Caster,
    Beast,
    Treant,
    Crawler,
}

fn family(look: Look) -> Family {
    match look {
        Look::GraveKnight | Look::CryptTitan => Family::Knight,
        Look::Treant => Family::Treant,
        Look::Spider | Look::Rimeweaver => Family::Crawler,
        Look::Ogre | Look::RimeJotun | Look::StormGiant => Family::Giant,
        Look::SandPharaoh | Look::BogHag | Look::HollowMother | Look::Warlord | Look::Necromancer => Family::Caster,
        Look::Elderhorn | Look::BleedingHart | Look::Ironhide | Look::Frostfang | Look::Direwolf | Look::WinterAlpha => Family::Beast,
        _ => Family::Golem,
    }
}

// Turns, and finding where limbs bend

pub fn rx(a: f64) -> Q {
    Q::from_axis_angle(&V::x_axis(), a)
}

/// Turned forward by `a`, its nose going down
pub fn ry(a: f64) -> Q {
    Q::from_axis_angle(&V::y_axis(), a)
}

/// Turned to the left by `a`
pub fn rz(a: f64) -> Q {
    Q::from_axis_angle(&V::z_axis(), a)
}

/// `v` as it is across the middle of the body, for the left side from the right
pub fn flip(v: V) -> V {
    V::new(v.x, -v.y, v.z)
}

/// The turn that takes a limb lying along `from`, bent towards `from_side`, to lie along `to` bent
/// towards `to_side`, so it doesn't twist about its length as it swings
pub fn aim(from: V, from_side: V, to: V, to_side: V) -> Q {
    let basis = |d: V, s: V| -> Matrix3<f64> {
        let x = d.try_normalize(1e-9).unwrap_or_else(|| -V::z());
        let y = (s - x * s.dot(&x)).try_normalize(1e-9).unwrap_or_else(|| {
            let any = if x.z.abs() < 0.9 { V::z() } else { V::x() };
            (any - x * any.dot(&x)).normalize()
        });
        Matrix3::from_columns(&[x, y, x.cross(&y)])
    };
    let r = basis(to, to_side) * basis(from, from_side).transpose();
    Q::from_rotation_matrix(&Rotation3::from_matrix_unchecked(r))
}

/// Where the middle joint goes of a limb from `root` reaching for `target`, `upper` and `lower`
/// long, bending towards `pole`, and where its end gets to, which is short of `target` if it's out
/// of reach
pub fn bend(root: V, target: V, upper: f64, lower: f64, pole: V) -> (V, V) {
    let to = target - root;
    let d = to.norm().clamp(((upper - lower).abs() + 1e-3).max(1e-3), (upper + lower) * 0.999);
    let dir = to.try_normalize(1e-9).unwrap_or_else(|| -V::z());
    let along = (upper * upper - lower * lower + d * d) / (2.0 * d);
    let out = (upper * upper - along * along).max(0.0).sqrt();
    let side = (pole - dir * pole.dot(&dir)).try_normalize(1e-9).unwrap_or_else(|| dir.cross(&V::y()).try_normalize(1e-9).unwrap_or_else(V::x));
    (root + dir * along + side * out, root + dir * d)
}

// Easing

/// From 0 to 1 as `f` does, easing in and out
pub fn smooth(f: f64) -> f64 {
    let f = f.clamp(0.0, 1.0);
    f * f * (3.0 - 2.0 * f)
}

/// Quick at first and slowing to a stop: gathering itself up
pub fn gather(f: f64) -> f64 {
    let f = f.clamp(0.0, 1.0);
    1.0 - (1.0 - f) * (1.0 - f) * (1.0 - f)
}

/// Slow at first and quicker and quicker: a blow coming down
pub fn snap(f: f64) -> f64 {
    let f = f.clamp(0.0, 1.0);
    f * f * f
}

/// Going a little past 1 and settling back
pub fn overshoot(f: f64) -> f64 {
    let f = f.clamp(0.0, 1.0);
    let s = 1.7;
    let g = f - 1.0;
    1.0 + g * g * ((s + 1.0) * g + s)
}

/// How each stretch of a run of poses is eased into, see along()
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ease {
    Smooth,
    Gather,
    Snap,
    Over,
    Linear,
}

impl Ease {
    fn of(self, f: f64) -> f64 {
        match self {
            Ease::Smooth => smooth(f),
            Ease::Gather => gather(f),
            Ease::Snap => snap(f),
            Ease::Over => overshoot(f),
            Ease::Linear => f.clamp(0.0, 1.0),
        }
    }
}

fn numbers<T: Pod>(t: &T) -> &[f64] {
    bytemuck::cast_slice(bytemuck::bytes_of(t))
}

fn numbers_mut<T: Pod>(t: &mut T) -> &mut [f64] {
    bytemuck::cast_slice_mut(bytemuck::bytes_of_mut(t))
}

/// `f` of the way from pose `a` to pose `b`, every number of it
pub fn mix<T: Pod>(a: &T, b: &T, f: f64) -> T {
    let mut out = *a;
    for (o, (x, y)) in numbers_mut(&mut out).iter_mut().zip(numbers(a).iter().zip(numbers(b))) {
        *o = x + (y - x) * f;
    }
    out
}

/// `pose` eased towards `to` for `dt` seconds, `rate` being about the share of the way it goes in a
/// second
pub fn ease<T: Pod>(pose: &mut T, to: &T, rate: f64, dt: f64) {
    let k = 1.0 - (-rate * dt).exp();
    let target = numbers(to).to_vec();
    for (o, t) in numbers_mut(pose).iter_mut().zip(target) {
        *o += (t - *o) * k;
    }
}

/// The pose `t` seconds along a run of them: each reached at its second, eased into as it says,
/// held at the first before it and at the last after. And whether that's a blow coming down, to
/// be followed quickly.
pub fn along<T: Pod>(t: f64, keys: &[(f64, T, Ease)]) -> (T, bool) {
    let Some(first) = keys.first() else {
        panic!("no poses to go along");
    };
    if t <= first.0 {
        return (first.1, false);
    }
    for pair in keys.windows(2) {
        let ((t0, a, _), (t1, b, how)) = (&pair[0], &pair[1]);
        if t < *t1 {
            let f = if t1 > t0 { (t - t0) / (t1 - t0) } else { 1.0 };
            return (mix(a, b, how.of(f)), *how == Ease::Snap);
        }
    }
    (keys[keys.len() - 1].1, false)
}

// What a boss is up to, from src\bosses.rs

/// Which part of a move a boss is at
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    /// Getting ready, so it's seen coming
    Wind,
    /// Doing it
    Act,
    /// Getting over it
    Recover,
    /// Dazed from running into something
    Stun,
    /// Reeling from the blows it's taken
    Reel,
}

/// A move being made: which, how far into which part of it in seconds, and how long that part
/// takes all told. Winding up, `held` is how long it's held the blow back past its usual time.
#[derive(Clone, Copy, Debug)]
pub struct Doing {
    pub m: Move,
    pub stage: Stage,
    pub t: f64,
    pub len: f64,
    pub held: f64,
}

/// Everything about what a boss is doing that's drawn, this frame
#[derive(Clone, Copy, Debug)]
pub struct Moment {
    /// Seconds since the last, 0 while it's frozen solid or held fast
    pub dt: f64,
    /// How far through its walk it is, 0 to 1, and how fast it's going, in meters a second, beside
    /// how fast it walks and runs, and how far one cycle of its walk takes it
    pub phase: f64,
    pub speed: f64,
    pub walk: f64,
    pub run: f64,
    pub stride: f64,
    /// Radians a second it's turning
    pub turning: f64,
    pub doing: Option<Doing>,
    /// What it's watching, in meters at its build's size from its feet, +x ahead
    pub target: Option<V>,
    pub engaged: bool,
    pub enraged: bool,
    /// For a raid boss, which phase of its fight it's in
    pub stage: u8,
    /// Seconds since it was killed
    pub dying: Option<f64>,
    /// What the Colossus has hold of, tearing up a boulder
    pub carry: Option<Carry>,
    /// Meters it's off the ground, leaping
    pub air: f64,
}

impl Moment {
    /// Standing still, doing nothing, for a boss just made
    pub fn still(look: Look) -> Moment {
        Moment {
            dt: 0.0,
            phase: 0.0,
            speed: 0.0,
            walk: look.walk_speed(),
            run: look.run_speed(),
            stride: look.stride() / look.scale().max(1e-3),
            turning: 0.0,
            doing: None,
            target: None,
            engaged: false,
            enraged: false,
            stage: 0,
            dying: None,
            carry: None,
            air: 0.0,
        }
    }

    /// How much it's walking, 0 standing to 1 at its walking pace, and how much running, 0 at its
    /// walking pace to 1 at its run
    pub fn pace(&self) -> (f64, f64) {
        let walking = (self.speed / self.walk.max(0.1)).clamp(0.0, 1.0);
        let running = ((self.speed - self.walk) / (self.run - self.walk).max(0.1)).clamp(0.0, 1.0);
        (walking, running)
    }
}

/// Where a piece goes this frame, in meters at its build's size from the middle of its feet with
/// +x ahead: where its joint is, how it's turned, and whether it's drawn
#[derive(Clone, Copy, Debug)]
pub struct Bone {
    pub at: V,
    pub turn: Q,
    pub shown: bool,
}

impl Default for Bone {
    fn default() -> Self {
        Bone { at: V::zeros(), turn: Q::identity(), shown: false }
    }
}

impl Bone {
    pub fn new(at: V, turn: Q) -> Bone {
        Bone { at, turn, shown: true }
    }
}

/// Where things come out of a boss and what it holds, in meters at its build's size from the
/// middle of its feet with +x ahead: its mouth, its right hand and its left, the tip of what's in
/// its right hand, the middle of what it's lifting, and where what it spits, casts or flings comes
/// from
#[derive(Clone, Copy, Debug, Default)]
pub struct Sockets {
    pub mouth: V,
    pub hands: [V; 2],
    pub tip: V,
    pub grip: V,
    pub cast: V,
}

/// How a boss of each family holds itself, eased from frame to frame
#[derive(Clone, Copy)]
enum Pose {
    Biped(biped::Stance),
    Beast(beast::Stance),
    Crawler(crawler::Stance),
}

/// The state of a jointed boss's pose, kept from frame to frame
pub struct Rig {
    pub look: Look,
    k: usize,
    pose: Pose,
    /// Whether it's been posed at all yet, so the first pose isn't eased into from nothing
    posed: bool,
    pub bones: [Bone; PIECES],
    pub sockets: Sockets,
    /// How far it's been knocked back by blows, twisted round to the left and tipped over to the
    /// right, in radians, and how fast that's going: a spring, so it recoils and settles
    flinch: [f64; 3],
    flinch_v: [f64; 3],
    /// What's left of the blows it's taken lately, as shares of its health, for whether it reels
    pub poise: f64,
    /// Seconds it's been about
    clock: f64,
    /// Where it's glancing as it stands about, round and down in radians, and seconds till it
    /// looks somewhere else
    glance: [f64; 3],
    glance_to: [f64; 2],
    /// How far through stepping round on the spot it is, turning
    shuffle: f64,
    seed: u32,
    /// Its pieces as they were when it started to fall apart, dying, for those that do
    fallen: Option<[Bone; PIECES]>,
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

fn random(seed: &mut u32) -> f64 {
    *seed = hash(*seed);
    (*seed >> 8) as f64 / (1u32 << 24) as f64
}

impl Rig {
    pub fn new(look: Look, seed: u32) -> Option<Rig> {
        let k = kind(look)?;
        let pose = match family(look) {
            Family::Golem => Pose::Biped(golem::rest(look)),
            Family::Knight => Pose::Biped(knight::rest(look)),
            Family::Giant => Pose::Biped(giant::rest(look)),
            Family::Caster => Pose::Biped(caster::rest(look)),
            Family::Beast => Pose::Beast(beast::rest(look)),
            Family::Treant => Pose::Biped(treant::rest(look)),
            Family::Crawler => Pose::Crawler(crawler::rest(look)),
        };
        let mut start = hash(seed);
        let mut rig = Rig {
            look,
            k,
            pose,
            posed: false,
            bones: [Bone::default(); PIECES],
            sockets: Sockets::default(),
            flinch: [0.0; 3],
            flinch_v: [0.0; 3],
            poise: 0.0,
            clock: random(&mut start) * 10.0,
            glance: [0.0, 0.0, 1.0],
            glance_to: [0.0; 2],
            shuffle: 0.0,
            seed: hash(seed ^ 0x51e5),
            fallen: None,
        };
        rig.step(&Moment::still(look));
        Some(rig)
    }

    /// A pose of its build's, to ease
    fn biped(&mut self) -> &mut biped::Stance {
        match &mut self.pose {
            Pose::Biped(s) => s,
            _ => panic!("{:?} isn't a biped", self.look),
        }
    }

    /// A pose of a four-legged build's, to ease
    fn beast(&mut self) -> &mut beast::Stance {
        match &mut self.pose {
            Pose::Beast(s) => s,
            _ => panic!("{:?} isn't a beast", self.look),
        }
    }

    /// A pose of an eight-legged build's, to ease
    fn crawler(&mut self) -> &mut crawler::Stance {
        match &mut self.pose {
            Pose::Crawler(s) => s,
            _ => panic!("{:?} isn't a crawler", self.look),
        }
    }

    /// Struck by a blow taking `share` of its health, from `from`, which way the blow came from in
    /// its own frame: it flinches away from it, the harder the blow the more, and big as it is the
    /// less. Returns how shaken it is, for whether it reels.
    pub fn hit(&mut self, share: f64, from: V) -> f64 {
        let size = self.look.size().max(1.0);
        let push = (share * 70.0 / size.sqrt()).min(2.6);
        let from = from.try_normalize(1e-9).unwrap_or_else(V::x);
        let wobble = random(&mut self.seed) - 0.5;
        self.flinch_v[0] += push * (from.x * 0.9 + 0.25);
        self.flinch_v[1] += push * (from.y * 0.8 + wobble * 0.5);
        self.flinch_v[2] += push * (from.y * 0.45 + wobble * 0.2);
        self.poise += share;
        self.poise
    }

    /// Posed for what it's doing now, `m`, its pieces and where things come out of it worked out
    pub fn step(&mut self, m: &Moment) {
        let dt = m.dt;
        self.clock += dt;
        // The spring of its flinching, stiffer the smaller it is
        let size = self.look.size().max(1.0);
        let (stiff, damp) = (130.0 / size, 11.0 / size.sqrt());
        for i in 0..3 {
            let a = -stiff * self.flinch[i] - damp * self.flinch_v[i];
            self.flinch_v[i] += a * dt;
            self.flinch[i] = (self.flinch[i] + self.flinch_v[i] * dt).clamp(-0.6, 0.6);
        }
        self.poise *= (-0.7 * dt).exp();
        // Glancing about now and then as it stands, or watching what it's after
        self.glance[2] -= dt;
        if self.glance[2] <= 0.0 {
            self.glance[2] = 1.6 + 3.5 * random(&mut self.seed);
            self.glance_to = [(random(&mut self.seed) - 0.5) * 1.1, (random(&mut self.seed) - 0.4) * 0.35];
        }
        let want = match m.target.filter(|_| m.engaged) {
            Some(t) => {
                let round = t.y.atan2(t.x).clamp(-0.9, 0.9);
                let down = (-(t.z - 2.0) / t.xy().norm().max(1.0)).atan().clamp(-0.5, 0.5);
                [round, down]
            }
            None => self.glance_to,
        };
        let k = 1.0 - (-3.0 * dt).exp();
        for i in 0..2 {
            self.glance[i] += (want[i] - self.glance[i]) * k;
        }
        self.shuffle = (self.shuffle + m.turning.abs() * dt / 1.4).fract();
        match family(self.look) {
            Family::Golem => golem::animate(self, m),
            Family::Knight => knight::animate(self, m),
            Family::Giant => giant::animate(self, m),
            Family::Caster => caster::animate(self, m),
            Family::Beast => beast::animate(self, m),
            Family::Treant => treant::animate(self, m),
            Family::Crawler => crawler::animate(self, m),
        }
        self.posed = true;
    }

    /// How far it's flinched, for builds to lay over their pose
    fn flinched(&self) -> [f64; 3] {
        self.flinch
    }
}

/// The pieces of `look`, each in its own frame about its joint at its build's size, in the order of
/// its build's pieces, see biped::PELVIS and the rest
fn pieces(look: Look) -> Vec<Vec<Part>> {
    match family(look) {
        Family::Golem => golem::pieces(look),
        Family::Knight => knight::pieces(look),
        Family::Giant => giant::pieces(look),
        Family::Caster => caster::pieces(look),
        Family::Beast => beast::pieces(look),
        Family::Treant => treant::pieces(look),
        Family::Crawler => crawler::pieces(look),
    }
}

/// Turns shapes about a joint, in meters, into a model at `res` voxels per meter just big enough
/// for what's solid in them, and where it puts the middle of its bottom, in the same meters. Shapes
/// of air cut into those after them without making the model any bigger.
fn bake(parts: &[Part], res: f64) -> (Model, V) {
    let parts: Vec<Part> = parts.iter().map(|p| p.thickened(res)).collect();
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for part in parts.iter().filter(|p| p.materials().0 != AIR) {
        let (a, b) = part.reach();
        for i in 0..3 {
            lo[i] = lo[i].min(a[i]);
            hi[i] = hi[i].max(b[i]);
        }
    }
    if lo[0] > hi[0] {
        return (Model { size: [1, 1, 1], voxels: vec![AIR] }, V::zeros());
    }
    let size = [0, 1, 2].map(|i| (((hi[i] - lo[i]) * res).ceil() as u32).clamp(1, 1023));
    let mut voxels = vec![AIR; (size[0] * size[1] * size[2]) as usize];
    let mut taken = vec![false; voxels.len()];
    for part in &parts {
        let (a, b) = part.reach();
        let from = [0, 1, 2].map(|i| (((a[i] - lo[i]) * res - 0.5).floor().max(0.0) as u32).min(size[i]));
        let to = [0, 1, 2].map(|i| (((b[i] - lo[i]) * res + 0.5).ceil().max(0.0) as u32).min(size[i]));
        let (material, stripes) = part.materials();
        for z in from[2]..to[2] {
            for y in from[1]..to[1] {
                for x in from[0]..to[0] {
                    let i = (x + (y + z * size[1]) * size[0]) as usize;
                    let p = [lo[0] + (x as f64 + 0.5) / res, lo[1] + (y as f64 + 0.5) / res, lo[2] + (z as f64 + 0.5) / res];
                    if !taken[i] && part.contains(p) {
                        taken[i] = true;
                        let band = stripes != AIR && (z / 2) % 3 == 0;
                        voxels[i] = if band { stripes } else { material };
                    }
                }
            }
        }
    }
    let anchor = V::new(lo[0] + size[0] as f64 * 0.5 / res, lo[1] + size[1] as f64 * 0.5 / res, lo[2]);
    (Model { size, voxels }, anchor)
}

/// Every jointed boss's pieces, voxelized at their own size, on every core at once
fn build_models() -> Vec<(Model, V)> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 8);
    let looks: Vec<Vec<Vec<Part>>> = KINDS.iter().map(|&look| pieces(look)).collect();
    let mut made: Vec<(usize, (Model, V))> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let job = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if job >= MODELS {
                            break out;
                        }
                        let (k, p) = (job / PIECES, job % PIECES);
                        let s = KINDS[k].scale();
                        let parts: Vec<Part> = looks[k].get(p).map_or(Vec::new(), |parts| parts.iter().map(|part| part.scaled(s)).collect());
                        out.push((job, bake(&parts, MODEL_RES)));
                    }
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("making a jointed boss's model failed")).collect()
    });
    made.sort_unstable_by_key(|(job, _)| *job);
    made.into_iter().map(|(_, m)| m).collect()
}

static LAYOUT: OnceLock<Vec<(V, [u32; 3])>> = OnceLock::new();

/// Where each piece's model has the middle of its bottom, in meters at its size from its joint,
/// and how big it is in voxels
fn layout() -> &'static [(V, [u32; 3])] {
    LAYOUT.get_or_init(|| build_models().into_iter().map(|(m, anchor)| (anchor, m.size)).collect())
}

/// The jointed bosses' models, to go after the frames of every look, see animals::animal_models()
pub fn models() -> Vec<Model> {
    let made = build_models();
    let _ = LAYOUT.set(made.iter().map(|(m, anchor)| (*anchor, m.size)).collect());
    made.into_iter().map(|(m, _)| m).collect()
}

/// What to draw for jointed boss `a`: each of its pieces, into `out`. `position` and `turn` place
/// the middle of its feet in the world, in voxels.
pub fn instances(a: &Animal, rig: &Rig, position: V, turn: Q, vpm: f64, out: &mut Vec<AnimalInstance>) {
    let layout = layout();
    let base = animals::RIG_MODELS_AT + rig.k * PIECES;
    let s = a.look.scale();
    let tint = a.status.tint();
    for (p, bone) in rig.bones.iter().enumerate() {
        if !bone.shown {
            continue;
        }
        let (anchor, size) = layout[rig.k * PIECES + p];
        if size == [1, 1, 1] {
            continue;
        }
        let [x, y, z] = size.map(|v| v as f64 / MODEL_RES);
        let reach = (x * x * 0.25 + y * y * 0.25 + z * z).sqrt() * vpm;
        let piece = turn * bone.turn;
        let at = position + turn * bone.at * s * vpm + piece * anchor * vpm;
        out.push(AnimalInstance::new((base + p) as u32, at, piece, vpm / MODEL_RES, reach, MOVING).tinted(tint));
    }
}

/// How long a jointed boss of `look` takes to die, falling, before it's gone and its hoard bursts
/// out of it
pub fn death_seconds(look: Look) -> f64 {
    match family(look) {
        Family::Golem => golem::DEATH,
        Family::Knight => knight::DEATH,
        Family::Giant => giant::DEATH,
        Family::Caster => caster::DEATH,
        Family::Beast => beast::DEATH,
        Family::Treant => treant::DEATH,
        Family::Crawler => crawler::DEATH,
    }
}

/// How far a knight's greatsword reaches as it sweeps it round, how far round its bone spikes
/// burst up, in meters, and where on it, +x ahead, its blade is as it's swept out, for what flies
/// off it
pub fn knight_reach(look: Look) -> (f64, f64, [f64; 3]) {
    knight::reach(look)
}

/// Seconds into dying a jointed boss of `look` hits the ground, for the ground to shake
pub fn thuds(look: Look) -> &'static [f64] {
    match family(look) {
        Family::Golem => &golem::THUDS,
        Family::Knight => &knight::THUDS,
        Family::Giant => &giant::THUDS,
        Family::Caster => &caster::THUDS,
        Family::Beast => &beast::THUDS,
        Family::Treant => &treant::THUDS,
        Family::Crawler => &crawler::THUDS,
    }
}

/// Whether `a` is a jointed boss still falling, killed, so it's not to be taken away yet
pub fn dying(a: &Animal) -> bool {
    rigged(a.look) && a.boss.as_ref().is_some_and(|b| b.rig.is_some() && b.dying.is_none_or(|t| t < death_seconds(a.look)))
}

/// Pieces scattered as something falls apart dying, `f` 0 as it starts to 1 lying in a heap: each
/// dropping to the ground where it was, turning over as it goes, settling partway into it, and the
/// heap spread round where it fell
fn crumble(bones: &mut [Bone; PIECES], from: &[Bone; PIECES], f: f64, seed: u32) {
    let mut seed = seed;
    for (bone, was) in bones.iter_mut().zip(from) {
        let (spin, tip, out, settle) = (random(&mut seed) - 0.5, random(&mut seed) - 0.5, random(&mut seed), 0.12 + 0.25 * random(&mut seed));
        if !was.shown {
            continue;
        }
        // Falling as things do, slowly at first, and bouncing a little at the bottom
        let drop = (f * 1.6).min(1.0);
        let fall = drop * drop;
        let low = settle.min(was.at.z);
        let spread = V::new(was.at.x, was.at.y, 0.0).try_normalize(1e-6).unwrap_or_else(V::x) * out * 0.6 * f.min(1.0);
        let at = V::new(was.at.x, was.at.y, was.at.z + (low - was.at.z) * fall) + spread;
        let bounce = if drop >= 1.0 { 0.06 * (1.0 - ((f - 0.625) / 0.375).clamp(0.0, 1.0)) * ((f - 0.625) * 25.0).sin().abs() } else { 0.0 };
        let turn = rz(spin * 1.2 * f) * ry(tip * 1.4 * fall) * was.turn;
        *bone = Bone { at: at + V::new(0.0, 0.0, bounce), turn, shown: true };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every jointed boss's pieces: small enough for the shader, and posed, never reaching past
    /// how far it's culled at, whatever it's doing
    #[test]
    fn jointed_bosses_stay_within_their_reach() {
        let _ = models();
        let layout = layout();
        for (k, &look) in KINDS.iter().enumerate() {
            let mut solid = 0;
            for p in 0..PIECES {
                let (_, size) = layout[k * PIECES + p];
                assert!(size.iter().all(|&s| s < 255), "{look:?}'s piece {p} is {size:?} voxels");
                if size != [1, 1, 1] {
                    solid += 1;
                }
            }
            assert!(solid >= 6, "{look:?} has only {solid} pieces");
            let mut rig = Rig::new(look, 3).unwrap();
            for (m, stage) in poses(look) {
                for f in 0..=8 {
                    let mut moment = Moment::still(look);
                    moment.dt = 1.0 / 30.0;
                    moment.engaged = true;
                    moment.doing = Some(Doing { m, stage, t: f as f64 * 0.12, len: 1.0, held: 0.0 });
                    rig.step(&moment);
                    let furthest = furthest(&rig, k);
                    assert!(furthest <= look.reach() * 1.1, "{look:?} doing {m:?} {stage:?} reaches {furthest:.2} m, well past its {:.2}", look.reach());
                }
            }
        }
    }

    /// The moves each boss makes and the parts of them, to try every pose
    fn poses(look: Look) -> Vec<(Move, Stage)> {
        let mut out = Vec::new();
        for m in crate::bosses::repertoire(look) {
            for stage in [Stage::Wind, Stage::Act, Stage::Recover] {
                out.push((m, stage));
            }
        }
        out.push((Move::Charge, Stage::Stun));
        out.push((Move::Roar, Stage::Reel));
        out
    }

    /// How far from the middle of its feet the furthest corner of any of its pieces is, in meters
    fn furthest(rig: &Rig, k: usize) -> f64 {
        let layout = layout();
        let s = rig.look.scale();
        let mut far = 0.0f64;
        for (p, bone) in rig.bones.iter().enumerate().filter(|(_, b)| b.shown) {
            let (anchor, size) = layout[k * PIECES + p];
            if size == [1, 1, 1] {
                continue;
            }
            let half = V::new(size[0] as f64, size[1] as f64, 0.0) * 0.5 / MODEL_RES;
            let high = size[2] as f64 / MODEL_RES;
            for c in 0..8 {
                let corner = anchor + V::new(if c & 1 == 0 { -half.x } else { half.x }, if c & 2 == 0 { -half.y } else { half.y }, if c & 4 == 0 { 0.0 } else { high });
                far = far.max((bone.at * s + bone.turn * corner).norm());
            }
        }
        far
    }

    /// Contact sheets of every jointed boss walking, standing, making each of its moves, hit, and
    /// dying, drawn on the processor, to judge how they move without the game:
    /// `RIG_SHEETS=<folder> cargo test --release rig_sheets -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn rig_sheets() {
        let Ok(folder) = std::env::var("RIG_SHEETS") else {
            println!("set RIG_SHEETS to a folder to draw them into");
            return;
        };
        let only = std::env::var("RIG_ONLY").ok();
        let models = models();
        for &look in &KINDS {
            if only.as_ref().is_some_and(|o| !format!("{look:?}").eq_ignore_ascii_case(o)) {
                continue;
            }
            for (name, frames) in preview::reels(look) {
                let path = std::path::Path::new(&folder).join(format!("{look:?}_{name}.png"));
                preview::sheet(look, &models, &frames, &path);
                println!("{}", path.display());
            }
        }
    }
}

#[cfg(test)]
pub mod preview;
