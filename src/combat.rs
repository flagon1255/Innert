//! Fighting in real time with the weapons from src\weapons.rs: swinging swords, drawing bows and
//! loosing arrows, casting spells that fly, and what their effects and synergies do to the animals
//! they hit. Positions are in voxels of the main world, the numbers in the weapon data in meters
//! and seconds.

use std::rc::Rc;

use nalgebra::{Matrix3, Rotation3, UnitQuaternion, Vector3};

use crate::animals::{Animal, AnimalInstance, HELD, MODEL_RES, MOVING, Model, STILL, TINT_BURNING, TINT_CHILLED, TINT_FROZEN, TINT_NONE, TINT_POISONED, TINT_SHOCKED};
use crate::audio::{Cue, Sound};
use crate::weapons::{ARM_RES, ARROW_RES, BOLT_RES, ELEMENT_LOOKS, Effect, Element, FINE_SPARK_RES, HELD_ARROW_RES, Kind, Profile, SPARK_RES, Style, Synergy, Weapon, effect_models};
use crate::worldgen::{World, is_passable};

pub const MAX_MANA: f64 = 100.0;
/// Mana coming back every second
const MANA_REGEN: f64 = 9.0;
/// Heavy swings hit this much harder
const HEAVY: f64 = 1.8;
/// How far the crosshair looks for something to aim at, in meters
const AIM_RANGE: f64 = 150.0;
/// Arrows fall this much slower than things do, so they carry
const ARROW_GRAVITY: f64 = 0.55;
/// The most lights fights add to the world's at once
const MAX_LIGHTS: usize = 10;

/// A synergy's chain reactions stop this many hits deep, whatever the data says
const MAX_DEPTH: u32 = 6;
/// How far a spell's own explosion reaches where it lands, in meters
const MAGIC_BLAST: f64 = 1.6;

/// What weapons have done to an animal, wearing off over time
#[derive(Default)]
pub struct Status {
    burn: f64,
    burn_dps: f64,
    poison: f64,
    poison_stacks: u32,
    poison_dps: f64,
    bleed: f64,
    bleed_stacks: u32,
    bleed_dps: f64,
    /// Builds up with each frost hit, freezing it solid at 1
    chill: f64,
    slow: f64,
    slowed: f64,
    frozen: f64,
    staggered: f64,
    shocked: f64,
    /// Thrown back by a blow, in voxels per second
    pub push: Vector3<f64>,
    /// Seconds its health bar stays up for
    pub shown: f64,
    /// Damage over time not shown yet, and when to show it
    ticked: f64,
    tick_timer: f64,
    /// Plague Bloom carried by its poison: how far it spreads, and how many deaths down it is
    plague: Option<(f64, u32, u32)>,
    /// Blood Reaper carried by its bleeding: how much faster killing it makes attacks, and for how long
    reaper: Option<(f64, f64)>,
}

impl Status {
    /// How fast it can move: 1 as normal, 0 frozen or knocked off its feet
    pub fn pace(&self) -> f64 {
        if self.frozen > 0.0 || self.staggered > 0.0 {
            0.0
        } else if self.slowed > 0.0 {
            1.0 - self.slow
        } else {
            1.0
        }
    }

    pub fn tint(&self) -> u32 {
        if self.frozen > 0.0 {
            TINT_FROZEN
        } else if self.shocked > 0.0 {
            TINT_SHOCKED
        } else if self.burn > 0.0 {
            TINT_BURNING
        } else if self.poison > 0.0 {
            TINT_POISONED
        } else if self.slowed > 0.0 {
            TINT_CHILLED
        } else {
            TINT_NONE
        }
    }
}

/// Where the camera is and which way it looks, for placing what's held and aiming
pub struct View {
    pub eye: Vector3<f64>,
    /// Turns the camera's right, forward and up, x, y and z, into the world's
    pub rotation: UnitQuaternion<f64>,
    /// Tangents of half the field of view across and up
    pub tan: [f64; 2],
    pub vpm: f64,
}

impl View {
    pub fn forward(&self) -> Vector3<f64> {
        self.rotation * Vector3::y()
    }

    /// Where on screen a point is, 0-1 across and down, if it's in front of the camera
    pub fn project(&self, p: Vector3<f64>) -> Option<[f64; 2]> {
        let local = self.rotation.inverse() * (p - self.eye);
        if local.y <= 0.0 {
            return None;
        }
        let (x, y) = (local.x / local.y / self.tan[0], local.z / local.y / self.tan[1]);
        Some([(x + 1.0) * 0.5, (1.0 - y) * 0.5])
    }
}

/// The mouse buttons, as far as fighting goes: the left attacks, the right swings heavy or draws a bow
#[derive(Default, Clone, Copy)]
pub struct Controls {
    /// The left button pressed this frame
    pub attack: bool,
    /// The left button held down
    pub holding: bool,
    /// The right button pressed this frame
    pub heavy: bool,
    /// The right button held down
    pub aiming: bool,
}

/// What a fight happens in
pub struct Arena<'a> {
    pub world: &'a World,
    pub animals: &'a mut Vec<Animal>,
    pub view: &'a View,
}

/// A number or a word floating up from where something was hit
pub struct Popup {
    pub position: Vector3<f64>,
    pub text: String,
    pub color: [u8; 3],
    pub size: f32,
    pub age: f64,
}

#[derive(Clone, Copy, PartialEq)]
enum Action {
    Idle,
    /// `flip` swings the other way, a backhand after a forehand
    Swing { t: f64, length: f64, heavy: bool, struck: bool, flip: bool },
    Draw { t: f64 },
    Loose { t: f64 },
    Cast { t: f64, cast: bool },
}

/// One hit landing
#[derive(Clone, Copy)]
struct Blow {
    damage: f64,
    crit: bool,
    heavy: bool,
    /// Where it came from, to be thrown back away from
    from: Vector3<f64>,
    /// How many hits deep in a chain reaction it is. Only the first brings the weapon's effects.
    depth: u32,
    element: Option<Element>,
}

struct Projectile {
    arrow: bool,
    position: Vector3<f64>,
    velocity: Vector3<f64>,
    damage: f64,
    profile: Rc<Profile>,
    /// How many more animals it can go through
    pierce: u32,
    /// Animals it went through already
    hit: Vec<u32>,
    life: f64,
}

/// An arrow stuck where it landed
struct Stuck {
    position: Vector3<f64>,
    rotation: UnitQuaternion<f64>,
    look: usize,
    life: f64,
}

struct Spark {
    position: Vector3<f64>,
    velocity: Vector3<f64>,
    look: usize,
    life: f64,
    lifetime: f64,
    /// How much it falls, 1 like anything else
    gravity: f64,
    /// A tiny one, trailing off the weapon in hand
    fine: bool,
}

/// A fireball of glowing voxels swelling up and then shrinking away
struct Blast {
    position: Vector3<f64>,
    look: usize,
    /// How big it gets, in meters
    radius: f64,
    life: f64,
    lifetime: f64,
}

/// A burst of light, fading
struct Flash {
    position: Vector3<f64>,
    color: [f32; 3],
    reach: f64,
    life: f64,
    lifetime: f64,
}

/// An attack repeating itself a moment later
struct Echo {
    delay: f64,
    profile: Rc<Profile>,
    shot: Option<Projectile>,
    heavy: bool,
    power: f64,
}

pub struct Combat {
    pub mana: f64,
    action: Action,
    /// Seconds until the weapon in hand can attack again
    cooldown: f64,
    /// Faster attacks for a while, from Blood Reaper
    frenzy: f64,
    frenzy_bonus: f64,
    /// Which weapon is in hand, and how far it's come up into view, 0-1
    held: Option<usize>,
    raise: f64,
    /// Whether the next swing is a backhand
    flip: bool,
    /// Where the tip of the weapon in hand was last frame, while it leaves a trail
    trail: Option<Vector3<f64>>,
    /// Sparks owed to the weapon's aura
    aura: f64,
    projectiles: Vec<Projectile>,
    sparks: Vec<Spark>,
    stuck: Vec<Stuck>,
    flashes: Vec<Flash>,
    echoes: Vec<Echo>,
    pub popups: Vec<Popup>,
    /// What to tell the player, and sounds to play
    pub messages: Vec<String>,
    pub cues: Vec<Cue>,
    /// Holes explosions blow in the world: where, and how big in meters
    pub craters: Vec<(Vector3<f64>, f64)>,
    /// Fireballs swelling up where explosions go off
    blasts: Vec<Blast>,
    /// Arrows, spells and sparks, see weapons::effect_models()
    pub models: Vec<(Model, Vector3<f64>)>,
    /// Index of the first of `models` in the buffer of all models
    first_model: u32,
    time: f64,
    mana_warning: f64,
    /// Voxels per meter of the world, as of the last update
    vpm: f64,
    rng: u32,
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^ (x >> 16)
}

fn random(seed: &mut u32) -> f64 {
    *seed = hash(seed.wrapping_add(0x9e3779b9));
    (*seed >> 8) as f64 / 16777216.0
}

fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How far from `a` to `b`, 0-1, the line first comes within `radius` of `center`
fn sphere_hit(a: Vector3<f64>, b: Vector3<f64>, center: Vector3<f64>, radius: f64) -> Option<f64> {
    let d = b - a;
    let f = a - center;
    let c = f.dot(&f) - radius * radius;
    if c <= 0.0 {
        return Some(0.0);
    }
    let aa = d.dot(&d);
    if aa < 1e-9 {
        return None;
    }
    let bb = 2.0 * f.dot(&d);
    let disc = bb * bb - 4.0 * aa * c;
    if disc < 0.0 {
        return None;
    }
    let t = (-bb - disc.sqrt()) / (2.0 * aa);
    (0.0..=1.0).contains(&t).then_some(t)
}

/// How far from `a` to `b`, 0-1, the line first runs into something solid
fn solid_along(world: &World, a: Vector3<f64>, b: Vector3<f64>) -> Option<f64> {
    let steps = ((b - a).norm() * 2.0).ceil().max(1.0) as usize;
    (1..=steps).map(|k| k as f64 / steps as f64).find(|&t| {
        let p = a + (b - a) * t;
        p.z < 0.0 || !is_passable(world.material_at(p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64))
    })
}

/// How a weapon is held, in the camera's right, forward and up, in meters
#[derive(Clone)]
struct Pose {
    hand: Vector3<f64>,
    turn: UnitQuaternion<f64>,
}

/// The hand at `hand`, the weapon's length pointing `along` and its x, the edge of a blade or
/// where a bow shoots, roughly `edge`
fn pose(hand: [f64; 3], along: [f64; 3], edge: [f64; 3]) -> Pose {
    let z = Vector3::from(along).normalize();
    let e = Vector3::from(edge);
    let x = (e - z * e.dot(&z)).normalize();
    let y = z.cross(&x);
    let turn = UnitQuaternion::from_rotation_matrix(&Rotation3::from_matrix_unchecked(Matrix3::from_columns(&[x, y, z])));
    Pose { hand: Vector3::from(hand), turn }
}

fn blend(a: &Pose, b: &Pose, t: f64) -> Pose {
    Pose { hand: a.hand.lerp(&b.hand, t), turn: a.turn.try_slerp(&b.turn, t, 1e-9).unwrap_or(b.turn) }
}

/// The pose `f` of the way through a move going from `rest` through `keys` and back: poses, each
/// with when it's reached, 0-1, and whether it's reached speeding up, as a blow is, instead of
/// easing in and out
fn animate(rest: &Pose, keys: &[(f64, Pose, bool)], f: f64) -> Pose {
    let mut from = (0.0, rest);
    for (at, pose, quick) in keys {
        if f < *at {
            let k = ((f - from.0) / (at - from.0)).clamp(0.0, 1.0);
            return blend(from.1, pose, if *quick { k * k } else { smooth(k) });
        }
        from = (*at, pose);
    }
    blend(from.1, rest, smooth((f - from.0) / (1.0 - from.0)))
}

/// When a swing in `style` starts coming down, and when it lands, as parts of the whole swing
fn timing(style: Style, heavy: bool) -> (f64, f64) {
    match (style, heavy) {
        (Style::Stab | Style::Thrust, false) => (0.25, 0.4),
        (Style::Stab | Style::Thrust, true) => (0.35, 0.5),
        (Style::Smash, false) => (0.4, 0.58),
        (Style::Smash, true) => (0.45, 0.6),
        (Style::Chop, false) => (0.3, 0.47),
        (_, true) => (0.4, 0.52),
        _ => (0.3, 0.45),
    }
}

/// The poses a swing in `style` goes through, for animate()
fn swing(style: Style, heavy: bool, flip: bool) -> Vec<(f64, Pose, bool)> {
    let (comes, lands) = timing(style, heavy);
    // Wound up only as far as the edge of the screen, so the weapon stays in sight.
    // Raised high and brought straight down: the overhead blow of a heavy sword or axe
    let overhead = || {
        vec![
            (comes, pose([0.22, 0.36, -0.02], [0.15, 0.25, 0.96], [0.0, 0.96, -0.25]), false),
            (0.62, pose([0.06, 0.55, -0.36], [0.0, 0.95, -0.3], [0.0, -0.3, -0.95]), true),
        ]
    };
    // A sweep from the right across to the left, the edge leading
    let sweep = |comes: f64, lands: f64| {
        vec![
            (comes, pose([0.22, 0.36, -0.16], [0.75, 0.35, 0.55], [-0.3, 1.0, 0.0]), false),
            (lands, pose([0.16, 0.55, -0.2], [0.3, 0.93, 0.15], [-1.0, 0.3, 0.0]), true),
            (lands + 0.13, pose([-0.1, 0.5, -0.28], [-0.8, 0.55, 0.05], [-0.55, -0.8, 0.0]), false),
        ]
    };
    match (style, heavy) {
        (Style::Slash, false) if flip => vec![
            // The backhand, from the upper left down to the right
            (comes, pose([-0.02, 0.36, -0.06], [-0.45, 0.3, 0.84], [0.83, 0.2, 0.55]), false),
            (0.55, pose([0.36, 0.52, -0.36], [0.8, 0.5, -0.3], [0.35, -0.1, -0.93]), true),
        ],
        (Style::Slash, false) => vec![
            (comes, pose([0.3, 0.34, -0.08], [0.45, 0.3, 0.84], [-0.83, 0.2, 0.55]), false),
            (0.55, pose([-0.12, 0.52, -0.38], [-0.8, 0.5, -0.3], [-0.35, -0.1, -0.93]), true),
        ],
        (Style::Chop, false) if flip => vec![
            // Cleaving down at a slant
            (comes, pose([0.24, 0.36, -0.04], [0.3, 0.25, 0.92], [0.0, 1.0, -0.3]), false),
            (lands, pose([0.0, 0.6, -0.3], [-0.4, 0.75, -0.5], [-0.2, 0.2, -1.0]), true),
            (lands + 0.13, pose([-0.05, 0.5, -0.4], [-0.5, 0.4, -0.75], [-0.2, 0.2, -1.0]), false),
        ],
        (Style::Chop, false) => sweep(comes, lands),
        (Style::Smash, false) if flip => sweep(comes, lands),
        (Style::Smash, _) => {
            // Heaved up over the shoulder and brought down hard, staying down a moment
            let (raised, struck) = if heavy {
                (pose([0.2, 0.32, -0.04], [0.1, 0.05, 0.99], [0.0, 1.0, -0.05]), pose([0.06, 0.6, -0.3], [0.0, 0.85, -0.5], [0.0, -0.5, -0.85]))
            } else {
                (pose([0.22, 0.36, -0.1], [0.12, 0.2, 0.97], [0.0, 1.0, -0.2]), pose([0.1, 0.6, -0.3], [0.0, 0.8, -0.6], [0.0, -0.6, -0.8]))
            };
            let mut settled = struck.clone();
            settled.hand.z -= 0.04;
            vec![(comes, raised, false), (lands, struck, true), (lands + 0.12, settled, false)]
        }
        (Style::Stab, false) => {
            // Drawn back and driven in, a little to the side every other time
            let lunge = if flip { pose([0.14, 0.7, -0.24], [-0.25, 0.95, 0.2], [0.0, 0.0, 1.0]) } else { pose([0.08, 0.72, -0.16], [-0.1, 0.98, 0.15], [0.0, 0.0, 1.0]) };
            let mut held = lunge.clone();
            held.hand.y -= 0.03;
            vec![(comes, pose([0.22, 0.3, -0.26], [-0.1, 0.9, 0.4], [0.0, 0.0, 1.0]), false), (lands, lunge, true), (lands + 0.1, held, false)]
        }
        (Style::Stab, true) => {
            // Raised point down and plunged in
            let plunge = pose([0.06, 0.62, -0.34], [-0.05, 0.7, -0.7], [0.0, -0.7, -0.7]);
            let mut held = plunge.clone();
            held.hand.z -= 0.03;
            vec![(comes, pose([0.2, 0.32, 0.02], [-0.05, 0.6, 0.8], [0.0, 1.0, 0.0]), false), (lands, plunge, true), (lands + 0.1, held, false)]
        }
        (Style::Thrust, _) => {
            // Drawn back along the shaft and lunged on at the crosshair
            let (back, lunge) = if heavy {
                (pose([0.32, 0.05, -0.25], [0.05, 0.95, 0.3], [-1.0, 0.0, 0.0]), pose([0.12, 0.75, -0.2], [-0.1, 0.99, 0.0], [-1.0, 0.0, 0.0]))
            } else {
                (pose([0.3, 0.12, -0.3], [0.0, 0.97, 0.2], [-1.0, 0.0, 0.0]), pose([0.16, 0.62, -0.22], [-0.12, 0.99, 0.04], [-1.0, 0.0, 0.0]))
            };
            let held = blend(&lunge, &back, 0.1);
            vec![(comes, back, false), (lands, lunge, true), (lands + 0.12, held, false)]
        }
        _ => overhead(),
    }
}

/// The hand at `p` in the world, holding a model's point `grip` there, gives the model this instance
#[allow(clippy::too_many_arguments)]
fn place(model: u32, look: &(Model, Vector3<f64>), res: f64, grip: Vector3<f64>, p: Vector3<f64>, rotation: UnitQuaternion<f64>, vpm: f64, size: f64, kind: u32) -> AnimalInstance {
    let (m, anchor) = look;
    let [x, y, z] = m.size.map(|s| s as f64 / res);
    let reach = (x * x * 0.25 + y * y * 0.25 + z * z).sqrt() * vpm * size;
    let position = p - rotation * (grip - anchor) * vpm * size;
    AnimalInstance::new(model, position, rotation, vpm / res * size, reach, kind)
}

/// Where an arrow's head is on its model
const ARROW_HEAD: f64 = 0.68;

/// How long casting a spell takes, in seconds
const CAST_TIME: f64 = 0.45;

/// Most sparks flying about at once
const MOST_SPARKS: usize = 220;

impl Combat {
    /// `first_model` is where the arrow, spell and spark models will be in the buffer of all models
    pub fn new(seed: u32, first_model: u32) -> Self {
        Combat {
            mana: MAX_MANA,
            action: Action::Idle,
            cooldown: 0.0,
            frenzy: 0.0,
            frenzy_bonus: 0.0,
            held: None,
            raise: 0.0,
            flip: false,
            trail: None,
            aura: 0.0,
            projectiles: Vec::new(),
            sparks: Vec::new(),
            stuck: Vec::new(),
            flashes: Vec::new(),
            echoes: Vec::new(),
            popups: Vec::new(),
            messages: Vec::new(),
            cues: Vec::new(),
            craters: Vec::new(),
            blasts: Vec::new(),
            models: effect_models(),
            first_model,
            time: 0.0,
            mana_warning: 0.0,
            vpm: 8.0,
            rng: hash(seed ^ 0xc0b47),
        }
    }

    /// The world moved over by `shift` voxels
    pub fn recenter(&mut self, shift: Vector3<f64>) {
        for p in &mut self.projectiles {
            p.position -= shift;
        }
        for s in &mut self.sparks {
            s.position -= shift;
        }
        if let Some(t) = &mut self.trail {
            *t -= shift;
        }
        for s in &mut self.stuck {
            s.position -= shift;
        }
        for f in &mut self.flashes {
            f.position -= shift;
        }
        for b in &mut self.blasts {
            b.position -= shift;
        }
        for p in &mut self.popups {
            p.position -= shift;
        }
    }

    /// Clears everything in flight, for a new world
    pub fn clear(&mut self) {
        self.projectiles.clear();
        self.sparks.clear();
        self.stuck.clear();
        self.flashes.clear();
        self.blasts.clear();
        self.echoes.clear();
        self.popups.clear();
        self.craters.clear();
    }

    /// How far the bow in hand is drawn, 0-1
    fn drawn(&self, profile: &Profile) -> f64 {
        match self.action {
            Action::Draw { t } => (t * profile.stat("drawSpeed") * self.speedup()).min(1.0),
            _ => 0.0,
        }
    }

    fn speedup(&self) -> f64 {
        if self.frenzy > 0.0 { 1.0 + self.frenzy_bonus } else { 1.0 }
    }

    /// Advances the fight by `dt` seconds: the weapon in hand, `weapon` with its index, attacking
    /// as `controls` say, things flying, and what's been done to the animals wearing off
    pub fn update(&mut self, arena: &mut Arena, weapon: Option<(usize, &Weapon)>, controls: Controls, dt: f64) {
        self.vpm = arena.view.vpm;
        self.time += dt;
        self.mana = (self.mana + MANA_REGEN * dt).min(MAX_MANA);
        self.cooldown -= dt;
        self.frenzy -= dt;
        self.mana_warning -= dt;

        let index = weapon.map(|(i, _)| i);
        if index != self.held {
            self.held = index;
            self.raise = 0.0;
            self.action = Action::Idle;
            self.trail = None;
            if let Some((_, weapon)) = weapon {
                let pitch = match weapon.kind() {
                    Kind::Melee => 1.0,
                    Kind::Ranged => 0.7,
                    Kind::Magic => 1.3,
                };
                self.cues.push(Cue::here(Sound::Equip, 0.35).pitched(pitch));
            }
        }
        self.raise = (self.raise + dt / 0.3).min(1.0);

        if let Some((_, weapon)) = weapon {
            self.wield(arena, weapon, controls, dt);
            self.flourish(arena.view, weapon, dt);
        }

        // Attacks repeating themselves
        let mut k = 0;
        while k < self.echoes.len() {
            self.echoes[k].delay -= dt;
            if self.echoes[k].delay > 0.0 {
                k += 1;
                continue;
            }
            let echo = self.echoes.swap_remove(k);
            match echo.shot {
                Some(shot) => {
                    self.sparks(shot.position, 6, 2.0, Element::look(Some(Element::Arcane)), 0.0, 0.3);
                    self.projectiles.push(shot);
                }
                None => self.melee(arena, &echo.profile, echo.heavy, echo.power),
            }
        }

        self.fly(arena, dt);
        self.afflict(arena, dt);

        for s in &mut self.sparks {
            s.velocity.z -= 9.8 * arena.view.vpm * s.gravity * dt;
            s.velocity *= (-1.5 * dt).exp();
            s.position += s.velocity * dt;
            s.life -= dt;
        }
        self.sparks.retain(|s| s.life > 0.0);
        for s in &mut self.stuck {
            s.life -= dt;
        }
        self.stuck.retain(|s| s.life > 0.0);
        for f in &mut self.flashes {
            f.life -= dt;
        }
        self.flashes.retain(|f| f.life > 0.0);
        for b in &mut self.blasts {
            b.life -= dt;
        }
        self.blasts.retain(|b| b.life > 0.0);
        let rise = 0.9 * arena.view.vpm;
        for p in &mut self.popups {
            p.age += dt;
            p.position.z += rise * dt;
        }
        self.popups.retain(|p| p.age < 1.2);
    }

    /// Attacking with the weapon in hand
    fn wield(&mut self, arena: &mut Arena, weapon: &Weapon, controls: Controls, dt: f64) {
        let profile = weapon.profile.clone();
        let speedup = self.speedup();
        match weapon.kind() {
            Kind::Melee => {
                if let Action::Swing { t, length, heavy, struck, flip } = self.action {
                    let (comes, lands) = timing(profile.style, heavy);
                    // Whooshing as it comes, and landing partway through the swing
                    if t < length * comes && t + dt >= length * comes {
                        let (sound, pitch) = match profile.style {
                            Style::Stab => (Sound::Stab, if heavy { 0.85 } else { 1.1 }),
                            Style::Thrust => (Sound::Stab, if heavy { 0.7 } else { 0.85 }),
                            Style::Smash => (Sound::HeavySwing, if heavy { 0.7 } else { 0.85 }),
                            Style::Chop => (if heavy { Sound::HeavySwing } else { Sound::Swing }, 0.9),
                            _ => (if heavy { Sound::HeavySwing } else { Sound::Swing }, if flip { 1.08 } else { 1.0 }),
                        };
                        self.cues.push(Cue::here(sound, 0.6).pitched(pitch));
                    }
                    let t = t + dt;
                    let land = !struck && t >= length * lands;
                    self.action = if t >= length { Action::Idle } else { Action::Swing { t, length, heavy, struck: struck || land, flip } };
                    if land {
                        self.melee(arena, &profile, heavy, 1.0);
                    }
                } else if (controls.attack || controls.holding || controls.heavy) && self.cooldown <= 0.0 && self.raise >= 1.0 {
                    let heavy = controls.heavy;
                    let length = (if heavy { 1.6 } else { 1.0 }) / (profile.stat("attackSpeed") * speedup).max(0.1);
                    // Light blows go back and forth
                    let flip = self.flip && !heavy;
                    self.flip = !flip && !heavy;
                    self.action = Action::Swing { t: 0.0, length, heavy, struck: false, flip };
                    self.cooldown = length;
                }
            }
            // The right button draws the bow and holds it drawn, the left lets the arrow fly.
            // Letting go of the right lowers it again.
            Kind::Ranged => match self.action {
                Action::Draw { t } => {
                    let draw = self.drawn(&profile);
                    if controls.attack && draw >= 0.15 {
                        self.loose(arena, weapon, draw);
                        self.action = Action::Loose { t: 0.0 };
                        self.cooldown = 0.25;
                    } else if controls.aiming {
                        self.action = Action::Draw { t: t + dt };
                    } else {
                        self.action = Action::Idle;
                    }
                }
                Action::Loose { t } => {
                    self.action = if t + dt > 0.2 { Action::Idle } else { Action::Loose { t: t + dt } };
                }
                _ => {
                    if controls.aiming && self.cooldown <= 0.0 && self.raise >= 1.0 {
                        self.action = Action::Draw { t: 0.0 };
                        self.cues.push(Cue::here(Sound::Draw, 0.4).pitched((1.0 / profile.stat("drawSpeed").max(0.3)).clamp(0.8, 1.25) as f32));
                    }
                }
            },
            Kind::Magic => {
                if let Action::Cast { t, cast } = self.action {
                    let t = t + dt;
                    if !cast && t >= 0.1 {
                        self.cast(arena, weapon);
                    }
                    self.action = if t >= CAST_TIME { Action::Idle } else { Action::Cast { t, cast: cast || t >= 0.1 } };
                }
                if (controls.attack || controls.holding) && self.cooldown <= 0.0 && self.raise >= 1.0 {
                    let cost = profile.stat("manaCost");
                    if self.mana >= cost {
                        self.mana -= cost;
                        self.action = Action::Cast { t: 0.0, cast: false };
                        self.cooldown = 1.0 / (profile.stat("castSpeed") * speedup).max(0.1);
                    } else if self.mana_warning <= 0.0 {
                        self.messages.push("Not enough mana.".into());
                        self.mana_warning = 2.0;
                    }
                }
            }
        }
    }

    /// Where the crosshair is on: the first animal or solid thing along it
    fn aim(&self, arena: &Arena) -> Vector3<f64> {
        let view = arena.view;
        let forward = view.forward();
        let far = view.eye + forward * AIM_RANGE * view.vpm;
        let mut t = solid_along(arena.world, view.eye, far).unwrap_or(1.0);
        for animal in arena.animals.iter().filter(|a| a.health > 0.0) {
            let radius = animal.look.body().1 * view.vpm;
            if let Some(hit) = sphere_hit(view.eye, far, animal.center(view.vpm as u32), radius) {
                t = t.min(hit);
            }
        }
        view.eye + (far - view.eye) * t
    }

    fn roll_crit(&mut self, profile: &Profile) -> bool {
        random(&mut self.rng) < profile.stat("critChance")
    }

    /// A sword swing landing: hits what's in front within reach. Echoes hit for `power` of a swing.
    fn melee(&mut self, arena: &mut Arena, profile: &Rc<Profile>, heavy: bool, power: f64) {
        let view = arena.view;
        let vpm = view.vpm;
        let reach = (profile.stat("range") + if heavy { 0.3 } else { 0.0 }) * vpm;
        // Heavy blows come down on less, but cleave through one more
        let cone = (profile.cone + if heavy { 0.15 } else { 0.0 }).min(0.95);
        let forward = view.forward();
        let mut targets: Vec<(f64, usize)> = arena
            .animals
            .iter()
            .enumerate()
            .filter(|(_, a)| a.health > 0.0 && !a.look.is_pet())
            .filter_map(|(i, a)| {
                let to = a.center(vpm as u32) - view.eye;
                let body = a.look.body().1 * vpm;
                let d = to.norm();
                (d < reach + body && (to.dot(&forward) > cone * d || d < body * 1.5)).then_some((d, i))
            })
            .collect();
        targets.sort_by(|a, b| a.0.total_cmp(&b.0));
        let base = (profile.stat("physicalDamage") + profile.stat("elementalDamage")) * if heavy { HEAVY } else { 1.0 } * power;
        targets.truncate(profile.targets + heavy as usize);
        let thud = match profile.style {
            Style::Smash => 0.7,
            Style::Chop => 0.85,
            Style::Stab => 1.25,
            Style::Thrust => 1.1,
            _ => 1.0,
        };
        for (k, &(_, i)) in targets.iter().enumerate() {
            let crit = self.roll_crit(profile);
            let damage = base * if crit { profile.stat("critDamage") } else { 1.0 };
            let center = arena.animals[i].center(vpm as u32);
            let at = center + (view.eye - center).normalize() * arena.animals[i].look.body().1 * vpm;
            if k == 0 {
                self.cues.push(Cue::at(Sound::Hit, at, if heavy { 1.0 } else { 0.8 }).pitched(thud));
            }
            self.sparks(at, if crit { 14 } else { 8 }, 3.0, Element::look(profile.element), 0.6, 0.45);
            self.strike(arena, profile, i, Blow { damage, crit, heavy, from: view.eye, depth: 0, element: profile.element });
        }
        // A heavy hammer blow dents the ground in front, knocking back what's around it
        let flat = Vector3::new(forward.x, forward.y, 0.0);
        if power >= 1.0 && heavy && profile.style == Style::Smash && flat.norm() > 0.2 {
            let ahead = view.eye + flat.normalize() * reach * 0.7;
            let down = Vector3::new(0.0, 0.0, -2.5 * vpm);
            if let Some(t) = solid_along(arena.world, ahead, ahead + down) {
                let at = ahead + down * t;
                self.craters.push((at, 0.45));
                self.sparks(at, 10, 2.0, Element::look(profile.element), 1.0, 0.4);
                self.cues.push(Cue::at(Sound::Burst, at, 0.7).pitched(0.55));
                let around: Vec<usize> = (0..arena.animals.len())
                    .filter(|&i| !targets.iter().any(|&(_, j)| j == i))
                    .filter(|&i| arena.animals[i].health > 0.0 && (arena.animals[i].position - at).norm() < 2.2 * vpm)
                    .collect();
                for i in around {
                    self.strike(arena, profile, i, Blow { damage: base * 0.3, crit: false, heavy: true, from: at, depth: 0, element: profile.element });
                }
            }
        }
        if power >= 1.0
            && profile.has(Effect::Echo)
            && random(&mut self.rng) < profile.param(Effect::Echo, "chance", 0.35)
        {
            let delay = profile.param(Effect::Echo, "delay", 0.25);
            let power = profile.param(Effect::Echo, "power", 0.6) * profile.potency(Effect::Echo).max(1.0);
            self.echoes.push(Echo { delay, profile: profile.clone(), shot: None, heavy, power });
            self.cues.push(Cue::here(Sound::Swing, 0.3));
            let ahead = view.eye + forward * 1.5 * vpm;
            self.sparks(ahead, 10, 2.5, Element::look(Some(Element::Arcane)), 0.0, 0.4);
        }
    }

    /// Lets an arrow fly from the bow in hand, drawn `draw` of the way
    fn loose(&mut self, arena: &mut Arena, weapon: &Weapon, draw: f64) {
        let profile = weapon.profile.clone();
        let vpm = arena.view.vpm;
        let from = self.held_point(weapon, arena.view, true);
        let target = self.aim(arena);
        let speed = profile.stat("projectileSpeed") * (0.45 + 0.55 * draw) * vpm;
        let damage = (profile.stat("physicalDamage") + profile.stat("elementalDamage")) * (0.3 + 0.7 * draw * draw);
        self.shoot(arena.view, &profile, true, from, (target - from).normalize(), speed, damage);
        self.cues.push(Cue::here(Sound::Twang, 0.4 + 0.4 * draw as f32));
    }

    /// Sends a spell flying from the crystal of the staff in hand
    fn cast(&mut self, arena: &mut Arena, weapon: &Weapon) {
        let profile = weapon.profile.clone();
        let vpm = arena.view.vpm;
        let from = self.held_point(weapon, arena.view, false);
        let target = self.aim(arena);
        let speed = profile.stat("projectileSpeed") * vpm;
        let damage = profile.stat("spellDamage") + profile.stat("elementalDamage");
        let look = Element::look(profile.element);
        self.flashes.push(Flash { position: from, color: profile.element.map_or([0.5, 0.9, 1.0], Element::light), reach: 5.0, life: 0.2, lifetime: 0.2 });
        self.shoot(arena.view, &profile, false, from, (target - from).normalize(), speed, damage);
        for _ in 0..12 {
            let v = Vector3::new(random(&mut self.rng), random(&mut self.rng), random(&mut self.rng)) * 2.0 - Vector3::repeat(1.0);
            self.fine(from, v * 0.6 * vpm, look, 0.3, 0.0);
        }
        self.cues.push(Cue::here(Sound::Cast(look as u8), 0.5));
        if profile.style == Style::Flick {
            self.cues.push(Cue::here(Sound::Flick, 0.4));
        }
    }

    /// Arrows or spells leaving `from` towards `direction`: more than one with multishot, and
    /// maybe again a moment later with echo
    #[allow(clippy::too_many_arguments)]
    fn shoot(&mut self, view: &View, profile: &Rc<Profile>, arrow: bool, from: Vector3<f64>, direction: Vector3<f64>, speed: f64, damage: f64) {
        let pierce = if profile.has(Effect::Pierce) { (profile.param(Effect::Pierce, "count", 2.0) * profile.potency(Effect::Pierce).max(1.0)).round() as u32 } else { 0 };
        let make = |direction: Vector3<f64>, damage: f64| Projectile {
            arrow,
            position: from,
            velocity: direction * speed,
            damage,
            profile: profile.clone(),
            pierce,
            hit: Vec::new(),
            life: if arrow { 4.0 } else { 3.0 },
        };
        let mut shots = vec![make(direction, damage)];
        if profile.has(Effect::Multishot) {
            let extra = profile.param(Effect::Multishot, "extra", 2.0).round() as usize;
            let spread = profile.param(Effect::Multishot, "spread", 7.0).to_radians();
            let power = profile.param(Effect::Multishot, "power", 0.6);
            let up = view.rotation * Vector3::z_axis();
            for k in 0..extra {
                let side = if k % 2 == 0 { 1.0 } else { -1.0 } * (k / 2 + 1) as f64;
                let turn = UnitQuaternion::from_axis_angle(&up, spread * side);
                shots.push(make(turn * direction, damage * power));
            }
        }
        if profile.has(Effect::Echo) && random(&mut self.rng) < profile.param(Effect::Echo, "chance", 0.35) {
            let power = profile.param(Effect::Echo, "power", 0.6);
            let delay = profile.param(Effect::Echo, "delay", 0.25);
            self.echoes.push(Echo { delay, profile: profile.clone(), shot: Some(make(direction, damage * power)), heavy: false, power });
        }
        self.projectiles.extend(shots);
    }

    /// Moves everything in flight, and sees what it hits
    fn fly(&mut self, arena: &mut Arena, dt: f64) {
        let vpm = arena.view.vpm;
        let projectiles = std::mem::take(&mut self.projectiles);
        let mut keep = Vec::new();
        for mut p in projectiles {
            let profile = p.profile.clone();
            // Homing ones turn towards the nearest animal ahead of them
            if profile.has(Effect::Homing) {
                let direction = p.velocity.normalize();
                let range = profile.param(Effect::Homing, "range", 18.0) * vpm;
                let target = arena
                    .animals
                    .iter()
                    .filter(|a| a.health > 0.0 && !a.look.is_pet() && !p.hit.contains(&a.id))
                    .map(|a| a.center(vpm as u32) - p.position)
                    .filter(|to| to.norm() < range && to.normalize().dot(&direction) > 0.5)
                    .min_by(|a, b| a.norm().total_cmp(&b.norm()));
                if let Some(to) = target {
                    let turn = (profile.param(Effect::Homing, "turn", 4.0) * dt).min(1.0);
                    p.velocity = direction.lerp(&to.normalize(), turn).normalize() * p.velocity.norm();
                }
            }
            if p.arrow {
                p.velocity.z -= 9.8 * ARROW_GRAVITY * vpm * dt;
            }
            let (start, end) = (p.position, p.position + p.velocity * dt);

            let mut gone = false;
            let wall = solid_along(arena.world, start, end).unwrap_or(f64::MAX);
            // Animals it passes through on the way, nearest first
            let mut hits: Vec<(f64, usize)> = arena
                .animals
                .iter()
                .enumerate()
                .filter(|(_, a)| a.health > 0.0 && !a.look.is_pet() && !p.hit.contains(&a.id))
                .filter_map(|(i, a)| sphere_hit(start, end, a.center(vpm as u32), a.look.body().1 * vpm * 1.1).map(|t| (t, i)))
                .filter(|&(t, _)| t < wall)
                .collect();
            hits.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (t, i) in hits {
                let at = start + (end - start) * t;
                let crit = self.roll_crit(&profile);
                let damage = p.damage * if crit { profile.stat("critDamage") } else { 1.0 };
                self.sparks(at, if crit { 14 } else { 8 }, 3.0, Element::look(profile.element), 0.6, 0.45);
                self.cues.push(Cue::at(Sound::Hit, at, 0.7));
                p.hit.push(arena.animals[i].id);
                self.strike(arena, &profile, i, Blow { damage, crit, heavy: false, from: start, depth: 0, element: profile.element });
                if p.pierce > 0 {
                    p.pierce -= 1;
                } else {
                    self.impact(arena, &p, at);
                    gone = true;
                    break;
                }
            }
            if gone {
                continue;
            }
            if wall <= 1.0 {
                let at = start + (end - start) * wall;
                self.impact(arena, &p, at);
                if p.arrow && !profile.has(Effect::Explosive) {
                    // Stuck in where it landed, its head buried
                    let direction = p.velocity.normalize();
                    let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &direction).unwrap_or_else(UnitQuaternion::identity);
                    self.stuck.push(Stuck { position: at + direction * 0.12 * vpm, rotation, look: Element::look(profile.element), life: 20.0 });
                    if self.stuck.len() > 40 {
                        self.stuck.remove(0);
                    }
                }
                continue;
            }
            p.position = end;
            p.life -= dt;
            if p.life > 0.0 {
                keep.push(p);
            } else if !p.arrow {
                self.sparks(p.position, 6, 1.5, Element::look(profile.element), 0.0, 0.4);
            }
        }
        keep.append(&mut self.projectiles);
        self.projectiles = keep;
    }

    /// Something in flight landing at `at`
    fn impact(&mut self, arena: &mut Arena, p: &Projectile, at: Vector3<f64>) {
        let profile = &p.profile;
        let look = Element::look(profile.element);
        if profile.has(Effect::Explosive) {
            let radius = profile.param(Effect::Explosive, "radius", 2.2) * profile.stat("area").max(1.0);
            let damage = p.damage * profile.param(Effect::Explosive, "power", 0.7) * profile.potency(Effect::Explosive).max(1.0);
            self.burst(arena, profile, at, radius, damage, 1, false, profile.element.or(Some(Element::Fire)), None);
        } else if !p.arrow {
            // Spells go off in a magic explosion where they land
            self.burst(arena, profile, at, MAGIC_BLAST, p.damage * 0.5, 1, false, profile.element.or(Some(Element::Arcane)), None);
        } else {
            self.sparks(at, 4, 1.5, look, 1.0, 0.3);
            self.cues.push(Cue::at(Sound::ArrowHit, at, 0.8));
        }
    }

    /// Hurts everything within `radius` meters of `at`, less further out, with a flash: an explosion,
    /// or with `pull` an implosion drawing them in. Skips `skip`.
    #[allow(clippy::too_many_arguments)]
    fn burst(&mut self, arena: &mut Arena, profile: &Rc<Profile>, at: Vector3<f64>, radius: f64, damage: f64, depth: u32, pull: bool, element: Option<Element>, skip: Option<usize>) {
        let vpm = arena.view.vpm;
        let look = Element::look(element);
        let sparks = (16.0 + radius * 12.0) as usize;
        self.sparks(at, sparks, if pull { -4.0 } else { 4.0 + radius * 2.5 }, look, 0.4, 0.6 + radius * 0.1);
        // Bigger ones boom deeper
        let pitch = if pull { 1.3 } else { (1.25 - radius * 0.12).clamp(0.6, 1.2) as f32 };
        self.cues.push(Cue::at(Sound::Burst, at, if pull { 0.6 } else { (0.5 + radius * 0.25).min(1.2) as f32 }).pitched(pitch));
        self.flashes.push(Flash { position: at, color: element.map_or([1.0, 0.8, 0.5], Element::light), reach: radius * 3.0 + 4.0, life: 0.35, lifetime: 0.35 });
        if !pull {
            // A fireball, and a hole blown in whatever it went off against
            self.blasts.push(Blast { position: at, look, radius, life: 0.4, lifetime: 0.4 });
            self.craters.push((at, radius * 0.75));
        }
        let r = radius * vpm;
        let inside: Vec<(usize, f64)> = arena
            .animals
            .iter()
            .enumerate()
            .filter(|&(i, a)| Some(i) != skip && a.health > 0.0 && !a.look.is_pet())
            .map(|(i, a)| (i, (a.center(vpm as u32) - at).norm()))
            .filter(|&(_, d)| d < r)
            .collect();
        for (i, d) in inside {
            let falloff = 1.0 - 0.5 * d / r;
            if pull {
                let towards = at - arena.animals[i].position;
                arena.animals[i].status.push = towards.normalize() * 5.0 * vpm;
            }
            self.strike(arena, profile, i, Blow { damage: damage * falloff, crit: false, heavy: false, from: at, depth, element });
        }
    }

    /// One hit landing on animal `i`: its damage, then the weapon's effects and synergies
    fn strike(&mut self, arena: &mut Arena, profile: &Rc<Profile>, i: usize, blow: Blow) {
        let vpm = arena.view.vpm;
        if arena.animals[i].health <= 0.0 || arena.animals[i].look.is_pet() || blow.depth > MAX_DEPTH {
            return;
        }
        let center = arena.animals[i].center(vpm as u32);
        let mut damage = blow.damage;

        if profile.has(Effect::Execute) && arena.animals[i].vitality() < profile.param(Effect::Execute, "threshold", 0.3) {
            damage *= 1.0 + profile.param(Effect::Execute, "bonus", 1.0) * profile.potency(Effect::Execute).max(1.0);
        }

        // Shatter: hitting something frozen breaks the ice, chilling everything around it
        if profile.synergy(Synergy::Shatter) && arena.animals[i].status.frozen > 0.0 {
            damage *= profile.synergy_param(Synergy::Shatter, "multiplier", 2.5);
            arena.animals[i].status.frozen = 0.0;
            let r = profile.synergy_param(Synergy::Shatter, "radius", 2.5) * vpm;
            for (j, other) in arena.animals.iter_mut().enumerate() {
                if j != i && !other.look.is_pet() && (other.center(vpm as u32) - center).norm() < r {
                    let s = &mut other.status;
                    s.chill += 0.5;
                    s.slowed = s.slowed.max(3.0);
                    s.slow = s.slow.max(0.45);
                }
            }
            self.sparks(center, 30, 5.0, Element::look(Some(Element::Frost)), 0.8, 0.7);
            self.cues.push(Cue::at(Sound::Crackle, center, 1.0));
            self.say_at(center, "Shatter!", Element::Frost.color(), 22.0);
        }

        // Solar Collapse: hitting something burning turns its flames into a void explosion
        let limit = profile.synergy_param(Synergy::SolarCollapse, "chainLimit", 3.0) as u32;
        if profile.synergy(Synergy::SolarCollapse) && arena.animals[i].status.burn > 0.0 && blow.depth < limit {
            let s = &mut arena.animals[i].status;
            let flames = s.burn_dps * s.burn;
            s.burn = 0.0;
            let radius = profile.synergy_param(Synergy::SolarCollapse, "radius", 3.0) * profile.stat("area").max(1.0);
            let power = profile.synergy_param(Synergy::SolarCollapse, "power", 1.5);
            damage += flames;
            self.say_at(center, "Solar Collapse!", Element::Void.color(), 22.0);
            self.burst(arena, profile, center, radius, blow.damage * power + flames, blow.depth + 1, false, Some(Element::Void), Some(i));
        }

        let died = arena.animals[i].damage(damage, blow.from);
        arena.animals[i].status.shown = 4.0;
        let color = blow.element.map_or([255, 255, 255], Element::color);
        let text = if blow.crit { format!("{:.0}!", damage) } else { format!("{:.0}", damage) };
        self.popups.push(Popup { position: center, text, color, size: if blow.crit { 24.0 } else { 17.0 }, age: 0.0 });

        // Siphon gives mana back. Blood Reaper makes crits on bleeding targets give back more.
        if profile.has(Effect::Siphon) {
            let mut fraction = profile.param(Effect::Siphon, "fraction", 0.2) * profile.potency(Effect::Siphon).max(1.0);
            if profile.synergy(Synergy::BloodReaper) && blow.crit && arena.animals[i].status.bleed > 0.0 {
                fraction *= profile.synergy_param(Synergy::BloodReaper, "siphon", 3.0);
            }
            self.mana = (self.mana + damage * fraction).min(MAX_MANA);
        }

        if blow.element == Some(Element::Storm) {
            arena.animals[i].status.shocked = 0.4;
        }

        if blow.depth == 0 && !died {
            self.afflict_hit(arena, profile, i, damage, blow);
        }
        if blow.depth == 0 && profile.has(Effect::Chain) {
            self.chain(arena, profile, i, damage);
        }
        if died {
            self.died(arena, i);
        }
    }

    /// The weapon's effects taking hold of animal `i`, hit for `damage`
    fn afflict_hit(&mut self, arena: &mut Arena, profile: &Rc<Profile>, i: usize, damage: f64, blow: Blow) {
        let vpm = arena.view.vpm;
        let center = arena.animals[i].center(vpm as u32);
        let mut froze = false;
        let mut implode = false;
        {
            let animal = &mut arena.animals[i];
            let s = &mut animal.status;
            if profile.has(Effect::Burn) {
                s.burn = profile.param(Effect::Burn, "duration", 3.0);
                s.burn_dps = s.burn_dps.max(damage * profile.param(Effect::Burn, "damage", 0.25) * profile.potency(Effect::Burn));
            }
            if profile.has(Effect::Frostbite) {
                s.slowed = profile.param(Effect::Frostbite, "duration", 3.0);
                s.slow = profile.param(Effect::Frostbite, "slow", 0.45);
                s.chill += profile.param(Effect::Frostbite, "chill", 0.35) * profile.potency(Effect::Frostbite);
                if s.chill >= 1.0 {
                    s.chill = 0.0;
                    s.frozen = profile.param(Effect::Frostbite, "freeze", 2.5);
                    froze = true;
                }
            }
            if profile.has(Effect::Poison) {
                let most = profile.param(Effect::Poison, "stacks", 5.0) as u32;
                s.poison_stacks = (s.poison_stacks + 1).min(most);
                s.poison = profile.param(Effect::Poison, "duration", 5.0);
                s.poison_dps = s.poison_dps.max(damage * profile.param(Effect::Poison, "damage", 0.12) * profile.potency(Effect::Poison));
                if profile.synergy(Synergy::PlagueBloom) {
                    let radius = profile.synergy_param(Synergy::PlagueBloom, "radius", 3.0);
                    let limit = profile.synergy_param(Synergy::PlagueBloom, "chainLimit", 3.0) as u32;
                    s.plague = Some((radius, limit, 0));
                }
            }
            if profile.has(Effect::Bleed) {
                let most = profile.param(Effect::Bleed, "stacks", 3.0) as u32;
                s.bleed_stacks = (s.bleed_stacks + 1).min(most);
                s.bleed = profile.param(Effect::Bleed, "duration", 4.0);
                s.bleed_dps = s.bleed_dps.max(damage * profile.param(Effect::Bleed, "damage", 0.15) * profile.potency(Effect::Bleed));
                if profile.synergy(Synergy::BloodReaper) {
                    s.reaper = Some((profile.synergy_param(Synergy::BloodReaper, "frenzy", 0.4), profile.synergy_param(Synergy::BloodReaper, "duration", 5.0)));
                }
            }
            if profile.has(Effect::Stagger) && (blow.heavy || random(&mut self.rng) < profile.param(Effect::Stagger, "chance", 0.5)) {
                s.staggered = profile.param(Effect::Stagger, "duration", 0.6);
            }
            let knock = profile.param(Effect::Knockback, "speed", 0.0) * profile.potency(Effect::Knockback).max(1.0) + profile.stat("knockback");
            if knock > 0.0 {
                let mut away = center - blow.from;
                away.z = 0.0;
                if away.norm() > 1e-6 {
                    s.push = away.normalize() * knock * vpm * if blow.heavy { 1.5 } else { 1.0 };
                }
            }
            if profile.has(Effect::VoidBurst) && random(&mut self.rng) < profile.param(Effect::VoidBurst, "chance", 0.25) {
                implode = true;
            }
        }
        if froze {
            self.say_at(center, "Frozen!", Element::Frost.color(), 19.0);
        }
        if implode {
            let radius = profile.param(Effect::VoidBurst, "radius", 1.8) * profile.stat("area").max(1.0);
            let power = profile.param(Effect::VoidBurst, "power", 0.8) * profile.potency(Effect::VoidBurst).max(1.0);
            self.burst(arena, profile, center, radius, damage * power, 1, true, Some(Element::Void), Some(i));
        }
    }

    /// Lightning jumping on from animal `i` to the nearest others. With Storm Conductor it goes
    /// further and grows with each jump.
    fn chain(&mut self, arena: &mut Arena, profile: &Rc<Profile>, i: usize, damage: f64) {
        let vpm = arena.view.vpm;
        let conductor = profile.synergy(Synergy::StormConductor);
        let hops = if conductor { profile.synergy_param(Synergy::StormConductor, "hops", 4.0) } else { profile.param(Effect::Chain, "targets", 1.0) * profile.potency(Effect::Chain).max(1.0) } as usize;
        let range = profile.param(Effect::Chain, "range", 6.0) * vpm;
        let mut power = profile.param(Effect::Chain, "power", 0.6);
        let mut from = i;
        let mut visited = vec![i];
        for _ in 0..hops {
            let here = arena.animals[from].center(vpm as u32);
            let next = arena
                .animals
                .iter()
                .enumerate()
                .filter(|(j, a)| !visited.contains(j) && a.health > 0.0 && !a.look.is_pet())
                .map(|(j, a)| (j, (a.center(vpm as u32) - here).norm()))
                .filter(|&(_, d)| d < range)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            let Some((j, _)) = next else {
                break;
            };
            let there = arena.animals[j].center(vpm as u32);
            // The arc, as a line of sparks
            let look = Element::look(Some(Element::Storm));
            let steps = ((there - here).norm() / (0.3 * vpm)).ceil().max(2.0) as usize;
            for k in 0..=steps {
                let jitter = Vector3::new(random(&mut self.rng) - 0.5, random(&mut self.rng) - 0.5, random(&mut self.rng) - 0.5) * 0.25 * vpm;
                let at = here.lerp(&there, k as f64 / steps as f64) + jitter;
                self.sparks.push(Spark { position: at, velocity: Vector3::zeros(), look, life: 0.18, lifetime: 0.18, gravity: 0.0, fine: false });
            }
            self.flashes.push(Flash { position: there, color: Element::Storm.light(), reach: 5.0, life: 0.15, lifetime: 0.15 });
            self.cues.push(Cue::at(Sound::Crackle, there, 0.7));
            arena.animals[j].status.shocked = 0.5;
            self.strike(arena, profile, j, Blow { damage: damage * power, crit: false, heavy: false, from: here, depth: 1, element: Some(Element::Storm) });
            if conductor {
                let growth = profile.synergy_param(Synergy::StormConductor, "growth", 0.25);
                let cap = profile.synergy_param(Synergy::StormConductor, "cap", 2.0);
                power = (power * (1.0 + growth)).min(cap);
            }
            visited.push(j);
            from = j;
        }
        if conductor && visited.len() > 2 {
            let at = arena.animals[visited[visited.len() - 1]].center(vpm as u32);
            self.say_at(at, "Storm Conductor!", Element::Storm.color(), 20.0);
        }
    }

    /// Animal `i` was just killed: synergies its afflictions carry go off
    fn died(&mut self, arena: &mut Arena, i: usize) {
        let vpm = arena.view.vpm;
        let center = arena.animals[i].center(vpm as u32);
        let s = &arena.animals[i].status;
        if let Some((bonus, duration)) = s.reaper
            && s.bleed > 0.0
        {
            self.frenzy = duration;
            self.frenzy_bonus = bonus;
            self.say_at(center, "Blood Reaper: frenzy!", Element::Blood.color(), 20.0);
        }
        if let Some((radius, limit, depth)) = s.plague
            && s.poison > 0.0
            && depth < limit
        {
            let (stacks, dps, time) = (s.poison_stacks, s.poison_dps, s.poison.max(3.0));
            let r = radius * vpm;
            for (j, other) in arena.animals.iter_mut().enumerate() {
                if j != i && other.health > 0.0 && !other.look.is_pet() && (other.center(vpm as u32) - center).norm() < r {
                    let o = &mut other.status;
                    o.poison_stacks = o.poison_stacks.max(stacks);
                    o.poison_dps = o.poison_dps.max(dps);
                    o.poison = o.poison.max(time);
                    o.plague = Some((radius, limit, depth + 1));
                    o.shown = 4.0;
                }
            }
            self.sparks(center, 30, 2.5, Element::look(Some(Element::Poison)), -0.1, 1.0);
            self.say_at(center, "Plague Bloom!", Element::Poison.color(), 20.0);
        }
    }

    /// Damage over time, and everything wearing off
    fn afflict(&mut self, arena: &mut Arena, dt: f64) {
        let vpm = arena.view.vpm;
        for i in 0..arena.animals.len() {
            if arena.animals[i].health <= 0.0 {
                continue;
            }
            let center = arena.animals[i].center(vpm as u32);
            let s = &mut arena.animals[i].status;
            let mut dot = 0.0;
            let mut color = [255, 255, 255];
            if s.burn > 0.0 {
                dot += s.burn_dps * dt;
                s.burn -= dt;
                color = Element::Fire.color();
            } else {
                s.burn_dps = 0.0;
            }
            if s.poison > 0.0 {
                dot += s.poison_dps * s.poison_stacks as f64 * dt;
                s.poison -= dt;
                color = Element::Poison.color();
            } else {
                s.poison_stacks = 0;
                s.poison_dps = 0.0;
            }
            if s.bleed > 0.0 {
                dot += s.bleed_dps * s.bleed_stacks as f64 * dt;
                s.bleed -= dt;
                color = Element::Blood.color();
            } else {
                s.bleed_stacks = 0;
                s.bleed_dps = 0.0;
            }
            s.frozen -= dt;
            s.staggered -= dt;
            s.shocked -= dt;
            s.slowed -= dt;
            s.shown -= dt;
            s.chill = (s.chill - 0.1 * dt).max(0.0);
            let (burning, poisoned) = (s.burn > 0.0, s.poison > 0.0);

            s.ticked += dot;
            s.tick_timer -= dt;
            let show = if s.tick_timer <= 0.0 && s.ticked >= 1.0 {
                s.tick_timer = 0.5;
                Some(std::mem::take(&mut s.ticked))
            } else {
                None
            };
            if dot > 0.0 {
                s.shown = s.shown.max(1.0);
                let animal = &mut arena.animals[i];
                animal.health -= dot;
                if animal.health <= 0.0 {
                    animal.health = 0.0;
                    self.died(arena, i);
                }
            }
            if let Some(amount) = show {
                self.popups.push(Popup { position: center, text: format!("{amount:.0}"), color, size: 14.0, age: 0.0 });
            }
            if burning && random(&mut self.rng) < dt * 14.0 {
                self.sparks(center, 1, 1.0, Element::look(Some(Element::Fire)), -0.25, 0.6);
            }
            if poisoned && random(&mut self.rng) < dt * 6.0 {
                self.sparks(center, 1, 0.6, Element::look(Some(Element::Poison)), -0.1, 0.8);
            }
        }
    }

    fn say_at(&mut self, at: Vector3<f64>, text: &str, color: [u8; 3], size: f32) {
        self.popups.push(Popup { position: at + Vector3::new(0.0, 0.0, 6.0), text: text.into(), color, size, age: 0.0 });
    }

    /// `count` sparks flying out from `at` at about `speed` m/s, or in towards it if negative
    fn sparks(&mut self, at: Vector3<f64>, count: usize, speed: f64, look: usize, gravity: f64, lifetime: f64) {
        let vpm = self.vpm;
        for _ in 0..count {
            let direction = loop {
                let v = Vector3::new(random(&mut self.rng), random(&mut self.rng), random(&mut self.rng)) * 2.0 - Vector3::repeat(1.0);
                if v.norm() > 0.1 && v.norm() <= 1.0 {
                    break v.normalize();
                }
            };
            let life = lifetime * (0.5 + 0.5 * random(&mut self.rng));
            let (position, velocity) = if speed < 0.0 {
                // Drawn in: starting out where it would have got to
                (at + direction * -speed * vpm * 0.4, -direction * -speed * vpm)
            } else {
                (at, direction * speed * vpm * (0.4 + 0.6 * random(&mut self.rng)))
            };
            self.sparks.push(Spark { position, velocity, look, life, lifetime: life, gravity, fine: false });
        }
        self.cap_sparks();
    }

    /// A fine spark at `at`, moving at `velocity` voxels a second
    fn fine(&mut self, at: Vector3<f64>, velocity: Vector3<f64>, look: usize, lifetime: f64, gravity: f64) {
        let life = lifetime * (0.6 + 0.4 * random(&mut self.rng));
        self.sparks.push(Spark { position: at, velocity, look, life, lifetime: life, gravity, fine: true });
        self.cap_sparks();
    }

    fn cap_sparks(&mut self) {
        if self.sparks.len() > MOST_SPARKS {
            let extra = self.sparks.len() - MOST_SPARKS;
            self.sparks.drain(..extra);
        }
    }

    /// The weapon in hand leaving a trail off its tip as it comes down on something, and sparks of
    /// its element drifting off it
    fn flourish(&mut self, view: &View, weapon: &Weapon, dt: f64) {
        let vpm = view.vpm;
        let profile = &weapon.profile;
        let look = Element::look(profile.element);
        let swinging = match self.action {
            Action::Swing { t, length, heavy, .. } => {
                let (comes, lands) = timing(profile.style, heavy);
                (comes..lands + 0.1).contains(&(t / length))
            }
            Action::Cast { t, .. } => profile.style == Style::Flick && t < 0.2,
            _ => false,
        };
        if swinging {
            let tip = self.held_point(weapon, view, false);
            if let Some(last) = self.trail {
                let steps = ((tip - last).norm() / (0.012 * vpm)).ceil().min(24.0) as usize;
                for k in 1..=steps {
                    let at = last.lerp(&tip, k as f64 / steps as f64);
                    self.fine(at, Vector3::zeros(), look, if profile.element.is_some() { 0.22 } else { 0.12 }, 0.0);
                }
            }
            self.trail = Some(tip);
        } else {
            self.trail = None;
        }

        // Fire's embers rise, frost settles, poison drips, lightning crackles off
        if profile.element.is_none() && weapon.kind() != Kind::Magic {
            return;
        }
        let (rate, speed, gravity, life) = match profile.element {
            Some(Element::Fire) => (16.0, 0.25, -0.12, 0.6),
            Some(Element::Frost) => (10.0, 0.08, 0.05, 0.8),
            Some(Element::Poison) => (8.0, 0.04, 0.4, 0.6),
            Some(Element::Storm) => (14.0, 0.9, 0.0, 0.15),
            _ => (10.0, 0.12, -0.02, 0.6),
        };
        self.aura += dt * rate;
        while self.aura >= 1.0 {
            self.aura -= 1.0;
            // From the crystal of a staff, anywhere along a blade
            let along = if weapon.kind() == Kind::Magic { 1.0 } else { 0.35 + 0.65 * random(&mut self.rng) };
            let at = self.held_at(weapon, view, weapon.look.grip.lerp(&weapon.look.tip, along));
            let v = Vector3::new(random(&mut self.rng), random(&mut self.rng), random(&mut self.rng)) * 2.0 - Vector3::repeat(1.0);
            self.fine(at, v * speed * vpm, look, life, gravity);
        }
    }

    /// The pose of the weapon in hand now, in the camera's frame
    fn pose(&self, weapon: &Weapon) -> Pose {
        // Held like in Skyrim: a sword or staff in the right hand down at the side, pointing on
        // ahead, a bow in the left
        let style = weapon.profile.style;
        let mut p = match weapon.kind() {
            Kind::Melee => {
                let rest = match style {
                    // Daggers up a little to be seen, spears levelled on ahead
                    Style::Stab => pose([0.2, 0.42, -0.22], [-0.15, 0.75, 0.65], [0.0, 1.0, 0.0]),
                    Style::Thrust => pose([0.28, 0.3, -0.32], [0.0, 0.97, 0.2], [-1.0, 0.0, 0.0]),
                    _ => pose([0.22, 0.46, -0.26], [-0.12, 0.62, 0.78], [0.0, 1.0, 0.0]),
                };
                match self.action {
                    Action::Swing { t, length, heavy, flip, .. } => animate(&rest, &swing(style, heavy, flip), t / length),
                    _ => rest,
                }
            }
            Kind::Ranged => {
                // Held low at the left, then raised and drawn back to the cheek
                let rest = pose([-0.25, 0.48, -0.28], [-0.35, 0.35, 0.87], [0.1, 1.0, 0.0]);
                let drawn = pose([-0.1, 0.66, -0.08], [-0.33, 0.02, 0.94], [0.0, 1.0, 0.0]);
                match self.action {
                    Action::Draw { t } => blend(&rest, &drawn, smooth(t * 4.0)),
                    Action::Loose { t } => {
                        // A little kick as the string snaps forward
                        let mut p = blend(&drawn, &rest, smooth(t / 0.2));
                        p.hand.y -= 0.03 * (1.0 - t / 0.2);
                        p
                    }
                    _ => rest,
                }
            }
            Kind::Magic if style == Style::Flick => {
                // A wand drawn back and flicked at the target
                let rest = pose([0.22, 0.44, -0.22], [-0.1, 0.55, 0.83], [1.0, 0.0, 0.0]);
                let keys = [
                    (0.1, pose([0.24, 0.4, -0.19], [0.0, 0.1, 1.0], [1.0, 0.0, 0.0]), false),
                    (0.24, pose([0.16, 0.62, -0.2], [-0.12, 0.95, 0.25], [1.0, 0.0, 0.0]), true),
                    (0.4, pose([0.16, 0.62, -0.21], [-0.12, 0.93, 0.2], [1.0, 0.0, 0.0]), false),
                ];
                match self.action {
                    Action::Cast { t, .. } => animate(&rest, &keys, t / CAST_TIME),
                    _ => rest,
                }
            }
            Kind::Magic => {
                let rest = pose([0.24, 0.45, -0.28], [-0.08, 0.5, 0.86], [1.0, 0.0, 0.0]);
                let keys = [(0.12 / CAST_TIME, pose([0.2, 0.62, -0.3], [-0.15, 0.85, 0.5], [1.0, 0.0, 0.0]), false)];
                match self.action {
                    Action::Cast { t, .. } => animate(&rest, &keys, t / CAST_TIME),
                    _ => rest,
                }
            }
        };
        // Swaying a little, and coming up into view when picked
        p.hand.x += 0.004 * (self.time * 1.1).sin();
        p.hand.z += 0.006 * (self.time * 2.3).sin() - 0.4 * (1.0 - self.raise).powi(2);
        p
    }

    /// Where a point of the weapon in hand is in the world: the arrow's nock on a bow with
    /// `nock`, otherwise its tip
    fn held_point(&self, weapon: &Weapon, view: &View, nock: bool) -> Vector3<f64> {
        self.held_at(weapon, view, if nock { self.nock(weapon) } else { weapon.look.tip })
    }

    /// Where `point` on the weapon in hand's model is in the world
    fn held_at(&self, weapon: &Weapon, view: &View, point: Vector3<f64>) -> Vector3<f64> {
        let p = self.pose(weapon);
        let rotation = view.rotation * p.turn;
        view.eye + view.rotation * p.hand * view.vpm + rotation * (point - weapon.look.grip) * view.vpm
    }

    /// Where the arrow's nock is on the bow in hand, following the string as it's drawn
    fn nock(&self, weapon: &Weapon) -> Vector3<f64> {
        let nocks = &weapon.look.nocks;
        if nocks.is_empty() {
            return weapon.look.tip;
        }
        let draw = self.drawn(&weapon.profile) * (nocks.len() - 1) as f64;
        let k = (draw.floor() as usize).min(nocks.len() - 1);
        let next = (k + 1).min(nocks.len() - 1);
        nocks[k].lerp(&nocks[next], draw - k as f64) + Vector3::new(0.0, 0.028, 0.0)
    }

    fn effect_model(&self, kind: usize, look: usize) -> (u32, &(Model, Vector3<f64>)) {
        let index = kind * ELEMENT_LOOKS + look;
        (self.first_model + index as u32, &self.models[index])
    }

    /// An arm whose fist holds `grip` round `axis`, its forearm reaching back towards `elbow`, all
    /// in the world. `which` is 0 for the right arm, 1 the left.
    fn arm(&self, which: usize, grip: Vector3<f64>, axis: Vector3<f64>, elbow: Vector3<f64>, vpm: f64) -> AnimalInstance {
        let z = axis.normalize();
        let along = grip - elbow;
        let y = (along - z * along.dot(&z)).normalize();
        let x = y.cross(&z);
        let rotation = UnitQuaternion::from_rotation_matrix(&Rotation3::from_matrix_unchecked(Matrix3::from_columns(&[x, y, z])));
        let index = 6 * ELEMENT_LOOKS + which;
        place(self.first_model + index as u32, &self.models[index], ARM_RES, Vector3::zeros(), grip, rotation, vpm, 1.0, HELD)
    }

    /// What to draw for the weapon in hand and the arms holding it, `first_model` being where the
    /// weapon's models are
    pub fn held_instances(&self, weapon: Option<(&Weapon, u32)>, view: &View) -> Vec<AnimalInstance> {
        let vpm = view.vpm;
        let mut out = Vec::new();
        if let Some((weapon, first_model)) = weapon {
            let p = self.pose(weapon);
            let look = &weapon.look;
            let rotation = view.rotation * p.turn;
            let hand = view.eye + view.rotation * p.hand * vpm;
            let frame = if look.frames.len() > 1 {
                ((self.drawn(&weapon.profile) * (look.frames.len() - 1) as f64).round() as usize).min(look.frames.len() - 1)
            } else {
                0
            };
            out.push(place(first_model + frame as u32, &look.frames[frame], look.res, look.grip, hand, rotation, vpm, 1.0, HELD));
            // Where the elbows are, in the camera's frame
            let elbow = |x: f64, y: f64, z: f64| view.eye + view.rotation * Vector3::new(x, y, z) * vpm;
            let axis = rotation * Vector3::z();
            let left = weapon.kind() == Kind::Ranged;
            out.push(self.arm(left as usize, hand, axis, if left { elbow(-0.3, 0.12, -0.7) } else { elbow(0.3, 0.12, -0.7) }, vpm));
            // The arrow on the string while drawing. The hand pulling it back is by the cheek, out
            // of sight.
            if matches!(self.action, Action::Draw { .. }) {
                let nock = hand + rotation * (self.nock(weapon) - look.grip) * vpm;
                let (model, arrow) = self.effect_model(3, Element::look(weapon.profile.element));
                out.push(place(model, arrow, HELD_ARROW_RES, Vector3::zeros(), nock, rotation, vpm, 1.0, HELD));
            }
        }
        out
    }

    /// What to draw for everything flying about, and arrows stuck where they landed, with how far
    /// each is from the camera
    pub fn flying_instances(&self, view: &View) -> Vec<(f64, AnimalInstance)> {
        let vpm = view.vpm;
        let mut out = Vec::new();
        let mut add = |at: Vector3<f64>, instance: AnimalInstance| out.push(((at - view.eye).norm(), instance));
        for p in &self.projectiles {
            let direction = p.velocity.normalize();
            let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &direction).unwrap_or_else(UnitQuaternion::identity);
            let look = Element::look(p.profile.element);
            if p.arrow {
                let (model, arrow) = self.effect_model(0, look);
                add(p.position, place(model, arrow, ARROW_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), p.position, rotation, vpm, 1.0, MOVING));
            } else {
                let (model, bolt) = self.effect_model(1, look);
                add(p.position, place(model, bolt, BOLT_RES, Vector3::zeros(), p.position, rotation, vpm, 1.0, MOVING));
            }
        }
        for s in &self.stuck {
            let (model, arrow) = self.effect_model(0, s.look);
            add(s.position, place(model, arrow, ARROW_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), s.position, s.rotation, vpm, 1.0, STILL));
        }
        for s in self.sparks.iter().filter(|s| !s.fine) {
            let (model, spark) = self.effect_model(2, s.look);
            let size = (s.life / s.lifetime).clamp(0.2, 1.0);
            add(s.position, place(model, spark, SPARK_RES, Vector3::zeros(), s.position, UnitQuaternion::identity(), vpm, size, MOVING));
        }
        // Fireballs, swelling quickly to the size of the explosion and then shrinking away, turning
        // as they go
        for b in &self.blasts {
            let (model, ball) = self.effect_model(4, b.look);
            let t = 1.0 - b.life / b.lifetime;
            let grow = if t < 0.25 { t / 0.25 } else { 1.0 - (t - 0.25) / 0.75 };
            let size = b.radius * 0.8 * grow.max(0.05);
            let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), t * 2.0);
            add(b.position, place(model, ball, MODEL_RES, Vector3::zeros(), b.position, spin, vpm, size, MOVING));
        }
        out
    }

    /// What to draw for the fine sparks trailing off the weapon in hand. They don't need to cast
    /// shadows, so they can go last with the leaves.
    pub fn fine_instances(&self, view: &View) -> Vec<AnimalInstance> {
        self.sparks
            .iter()
            .filter(|s| s.fine)
            .map(|s| {
                let (model, spark) = self.effect_model(5, s.look);
                let size = (s.life / s.lifetime).clamp(0.3, 1.0);
                place(model, spark, FINE_SPARK_RES, Vector3::zeros(), s.position, UnitQuaternion::identity(), view.vpm, size, MOVING)
            })
            .collect()
    }

    /// Lights from spells flying, flashes, burning animals and a glowing staff in hand, in the
    /// form main.rs keeps the world's lights in
    pub fn lights(&self, weapon: Option<&Weapon>, animals: &[Animal], view: &View) -> Vec<[f32; 8]> {
        let vpm = view.vpm;
        let light = |p: Vector3<f64>, reach: f64, color: [f32; 3], strength: f32| {
            [p.x as f32, p.y as f32, p.z as f32, (reach * vpm) as f32, color[0] * strength, color[1] * strength, color[2] * strength, 0.0]
        };
        let mut lights = Vec::new();
        for f in &self.flashes {
            lights.push(light(f.position, f.reach, f.color, 3.0 * (f.life / f.lifetime) as f32));
        }
        for p in &self.projectiles {
            if !p.arrow {
                lights.push(light(p.position, 6.0, p.profile.element.map_or([0.5, 0.9, 1.0], Element::light), 1.2));
            } else if let Some(e) = p.profile.element {
                lights.push(light(p.position, 3.0, e.light(), 0.5));
            }
        }
        if let Some(weapon) = weapon
            && weapon.kind() == Kind::Magic
        {
            let tip = self.held_point(weapon, view, false);
            lights.push(light(tip, 3.0, weapon.profile.element.map_or([0.5, 0.9, 1.0], Element::light), 0.35));
        }
        for a in animals {
            if a.status.burn > 0.0 && a.health > 0.0 {
                lights.push(light(a.center(vpm as u32), 4.0, Element::Fire.light(), 0.8));
            }
        }
        // The nearest first
        lights.sort_by(|a, b| {
            let d = |l: &[f32; 8]| (Vector3::new(l[0] as f64, l[1] as f64, l[2] as f64) - view.eye).norm();
            d(a).total_cmp(&d(b))
        });
        lights.truncate(MAX_LIGHTS);
        lights
    }

    /// The attack speed bonus from a frenzy, if there is one, and how long it has left
    pub fn frenzy(&self) -> Option<(f64, f64)> {
        (self.frenzy > 0.0).then_some((self.frenzy_bonus, self.frenzy))
    }

    /// How far the bow in hand is drawn, 0-1, for the crosshair
    pub fn draw(&self, weapon: &Weapon) -> f64 {
        self.drawn(&weapon.profile)
    }
}
