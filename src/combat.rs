//! Fighting in real time with the weapons from src\weapons.rs: swinging swords, drawing bows and
//! loosing arrows, casting spells that fly, and what their effects and synergies do to the animals
//! they hit. Positions are in voxels of the main world, the numbers in the weapon data in meters
//! and seconds.

use std::f64::consts::{PI, TAU};
use std::rc::Rc;

use nalgebra::{Matrix3, Rotation3, UnitQuaternion, Vector3};

use crate::animals::{Animal, AnimalInstance, HELD, Look, MODEL_RES, MOVING, Model, STILL, TINT_BURNING, TINT_CHILLED, TINT_FROZEN, TINT_HIT, TINT_NONE, TINT_POISONED, TINT_SHOCKED};
use crate::audio::{Cue, Sound};
use crate::player::WaterLevel;
use crate::weapons::{
    ARM_RES, ARMS, ARROW_RES, BOLT_RES, BOULDER, DROP_ASH, DROP_BLOOD, DROP_EMBER, DROP_GOLD, DROP_ICE, DROP_MUD, DROP_VENOM, DROP_VOID, DROP_WATER, ELEMENT_LOOKS, Effect, Element, FINE_SPARK_RES, HELD_ARROW_RES,
    ICE_BURST, Kind, PILLAR, Profile, RING, RING_RES, ROOTS, THORN, WEB, Special, SLOT_ARROW, SLOT_BOLT, SLOT_DROP, SLOT_FINE, SLOT_FIREBALL, SLOT_HELD_ARROW, SLOT_SPARK, SPARK_RES, Style, Synergy, Weapon, effect_models,
};
use crate::worldgen::material::{BARK_DARK, CLOTH, DIRT, MOSS, OBSIDIAN, SNOW};
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
const MAX_LIGHTS: usize = 6;

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
    /// Seconds it goes on flashing white from being struck
    flash: f64,
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
    /// A boss is only ever frozen or knocked off its feet for a moment
    pub fn limit_for_boss(&mut self) {
        self.frozen = self.frozen.min(0.6);
        self.staggered = self.staggered.min(0.25);
    }

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
        if self.flash > 0.0 {
            TINT_HIT
        } else if self.frozen > 0.0 {
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
    /// How fast the player's going over the ground in meters a second, and how far they've walked
    pub speed: f64,
    pub stride: f64,
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
    /// R or the middle button pressed this frame: the weapon's special attack
    pub special: bool,
}

/// What a fight happens in
pub struct Arena<'a> {
    pub world: &'a World,
    pub animals: &'a mut Vec<Animal>,
    pub view: &'a View,
    /// Where the player's feet are, None while flying
    pub player: Option<Vector3<f64>>,
    /// Where the top of the water is, for splashes
    pub water: WaterLevel<'a>,
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
    /// `combo` is which blow of a combo it is: a forehand, then a backhand, then a finisher
    Swing { t: f64, length: f64, heavy: bool, struck: bool, combo: u8 },
    Draw { t: f64 },
    Loose { t: f64 },
    Cast { t: f64, cast: bool },
    /// A special attack being unleashed, see special_move()
    Special { t: f64, length: f64, special: Special },
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
    /// Which arrowhead, see weapons::ARROW_HEADS
    head: usize,
    /// Shot from a bow drawn all the way, so it streaks
    power: bool,
    /// Trail sparks owed
    trail: f64,
    /// What sort of spell it is, for a spell
    form: Form,
    /// How much it falls, 1 like anything else, and how big it's drawn
    gravity: f64,
    size: f64,
    /// Seconds it pins what it hits to the spot, if it does
    pin: f64,
}

/// How a spell flies and goes off, which its element decides. Holy magic isn't one of these: it
/// calls light down from the sky, see Strike.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Form {
    /// A quick glowing orb, without an element
    Bolt,
    /// A big ball of fire, slower, dropping as it goes
    Fireball,
    /// Shards of ice, three in a spread
    Shard,
    /// Lightning, there almost at once
    Lightning,
    /// A glob of venom, lobbed high
    Glob,
    /// Little missiles that seek out what's near, three one after another
    Missile,
    /// A slow orb of void dragging in everything it passes
    Orb,
    /// A thin lance of blood, flying fast
    Lance,
}

impl Form {
    fn of(element: Option<Element>) -> Form {
        match element {
            Some(Element::Fire) => Form::Fireball,
            Some(Element::Frost) => Form::Shard,
            Some(Element::Storm) => Form::Lightning,
            Some(Element::Poison) => Form::Glob,
            Some(Element::Arcane) => Form::Missile,
            Some(Element::Void) => Form::Orb,
            Some(Element::Blood) => Form::Lance,
            _ => Form::Bolt,
        }
    }

    /// How fast it flies, how much it falls and how big it's drawn, next to a plain bolt, and how
    /// far its blast reaches next to a plain bolt's
    fn flight(self) -> (f64, f64, f64, f64) {
        match self {
            Form::Bolt => (1.0, 0.0, 1.0, 1.0),
            Form::Fireball => (0.7, 0.25, 1.0, 1.45),
            Form::Shard => (1.3, 0.05, 0.7, 0.55),
            Form::Lightning => (12.0, 0.0, 0.0, 0.9),
            Form::Glob => (0.75, 0.8, 1.3, 1.1),
            Form::Missile => (1.1, 0.0, 0.6, 0.6),
            Form::Orb => (0.55, 0.0, 1.0, 1.3),
            Form::Lance => (1.6, 0.0, 0.8, 0.8),
        }
    }
}

/// Fire left burning on the ground, or a cloud of venom hanging over it, hurting what's in it.
/// Void draws everything in.
struct Lingering {
    position: Vector3<f64>,
    /// In meters
    radius: f64,
    life: f64,
    element: Element,
    profile: Rc<Profile>,
    /// How much each tick of it hurts, and how long until the next
    damage: f64,
    tick: f64,
    /// Whether its ticks bring the weapon's effects, like freezing
    effects: bool,
    /// How hard it goes off as it ends, if it does
    collapse: f64,
}

/// Part of a special attack still to come, `delay` seconds from now
struct Pending {
    delay: f64,
    act: Act,
    profile: Rc<Profile>,
    damage: f64,
}

#[derive(Clone, Copy)]
enum Act {
    /// A meteor falling onto here
    Meteor(Vector3<f64>),
    /// An arrow falling onto here
    Arrow(Vector3<f64>),
    /// Lightning striking down onto the animal with this id
    Bolt(u32),
    /// The ground bursting open here
    Rupture(Vector3<f64>),
    /// Spinning, hitting everything round the player
    Spin,
    /// A slash at everything in front
    Slash,
    /// A seeking star sent off from here
    Star(Vector3<f64>),
    /// The player thrown along, in voxels a second
    Dash(Vector3<f64>),
    /// Coming down from a leap: crashing down once on the ground, or anyway in `wait` seconds
    Land { wait: f64 },
    /// An arrow of a pinning volley, this many degrees off where it's aimed
    Volley(f64),
    /// Lightning leaping from `from` to the animal with id `to`, which it came from `last`, the
    /// `hop`th leap, hurting more each time up to `cap`
    Arc { from: Vector3<f64>, to: u32, last: u32, hop: u32, cap: f64 },
    /// The `step`th crest of a wave rolling on along `dir`, here
    Wave { at: Vector3<f64>, dir: Vector3<f64>, step: u32 },
    /// The `k`th gout of flame poured out
    Gout(u32),
}

/// Standing guard for a riposte: seconds left, and what answers a blow turned aside
struct Guard {
    left: f64,
    profile: Rc<Profile>,
    damage: f64,
}

/// Damage the weapon in hand has to do to charge its special attack
const SPECIAL_CHARGE: f64 = 450.0;
/// Seconds it takes to charge by itself
const SPECIAL_SECONDS: f64 = 60.0;

/// Light coming down from the sky onto where a holy spell was aimed, a moment after it's cast
struct Strike {
    position: Vector3<f64>,
    delay: f64,
    profile: Rc<Profile>,
    damage: f64,
}

/// Something swelling up and dying away where a spell went off: spikes of ice, or a pillar of light,
/// see weapons::ICE_BURST and PILLAR
struct Swell {
    model: usize,
    position: Vector3<f64>,
    radius: f64,
    life: f64,
    lifetime: f64,
}

/// An arrow stuck where it landed, or a thorn a boss shot
struct Stuck {
    position: Vector3<f64>,
    rotation: UnitQuaternion<f64>,
    look: usize,
    /// Which arrowhead, or None for a thorn
    head: Option<usize>,
    element: Option<Element>,
    life: f64,
    /// Seconds it goes on burning, crackling or such, for an arrow with an element
    glowing: f64,
}

/// What a boss throws or spits at the player
#[derive(Clone, Copy, PartialEq)]
pub enum HostileKind {
    /// A boulder torn up: rock, or ice or molten rock for a boss of that element
    Boulder(Option<Element>),
    /// A ball of the boss's element spat out. Venom is lobbed so it falls, the rest fly straight.
    Spell(Element),
    /// A thorn shot off a tree, hurting only what it hits
    Thorn,
    /// A ball of web, lobbed, leaving the ground round where it lands sticky
    Web,
}

impl HostileKind {
    /// How hard it falls, next to things falling as they do
    fn gravity(self) -> f64 {
        match self {
            HostileKind::Boulder(_) | HostileKind::Spell(Element::Poison) | HostileKind::Web => 1.0,
            HostileKind::Thorn => 0.25,
            HostileKind::Spell(_) => 0.0,
        }
    }

    fn element(self) -> Option<Element> {
        match self {
            HostileKind::Boulder(e) => e,
            HostileKind::Spell(e) => Some(e),
            HostileKind::Web => Some(Element::Poison),
            HostileKind::Thorn => None,
        }
    }
}

/// How big a thorn is next to an arrow
const THORN_SIZE: f64 = 1.5;

/// Something a boss threw or spat, flying at the player
struct Hostile {
    kind: HostileKind,
    position: Vector3<f64>,
    velocity: Vector3<f64>,
    life: f64,
    /// How hard it hits, and how far its blast reaches in meters
    damage: f64,
    radius: f64,
    /// Where it was aimed to come down, and how many seconds it was to take getting there
    target: Vector3<f64>,
    flight: f64,
}

struct Spark {
    position: Vector3<f64>,
    velocity: Vector3<f64>,
    look: usize,
    life: f64,
    lifetime: f64,
    /// How much it falls, 1 like anything else
    gravity: f64,
    /// Which kind: SLOT_SPARK, a tiny one off the weapon in hand, SLOT_FINE, or a drop of
    /// something splashed, SLOT_DROP, which falls and stops on the ground
    slot: usize,
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
    /// Which blow of a combo the next swing is, and how long until the combo's dropped
    combo: u8,
    combo_timer: f64,
    /// Seconds the swing stops dead for as a blow lands, so it feels like it hit something
    hitstop: f64,
    /// The weapon jolting back from a blow, 0-1
    recoil: f64,
    /// How hard the camera shakes, wearing off
    pub shake: f64,
    /// How fast the player is going in meters a second, and how far they've walked, to bob what's
    /// in hand as they go
    speed: f64,
    stride: f64,
    /// Hits on the player from what bosses threw: how hard, from where, and of what element
    pub player_hits: Vec<(f64, Vector3<f64>, Option<Element>)>,
    /// Where the tip of the weapon in hand was last frame, while it leaves a trail
    trail: Option<Vector3<f64>>,
    /// Sparks owed to the weapon's aura
    aura: f64,
    projectiles: Vec<Projectile>,
    hostiles: Vec<Hostile>,
    lingering: Vec<Lingering>,
    strikes: Vec<Strike>,
    swells: Vec<Swell>,
    /// Rings on the ground this frame showing where things are coming down, and how big round
    marks: Vec<(Vector3<f64>, f64)>,
    /// Where balls of web landed since game.rs last looked
    pub webs: Vec<Vector3<f64>>,
    /// Where what bosses threw only just missed the player, since game.rs last looked
    pub near_misses: Vec<Vector3<f64>>,
    pending: Vec<Pending>,
    /// Standing guard, ready to riposte
    guard: Option<Guard>,
    /// A riptide's wave crashed down on something big, so the rest of it's gone
    wave_broken: bool,
    /// How charged the special attack is, 0-1
    pub charge: f64,
    /// Health a special attack gives back, and how it throws the player, for game.rs to see to
    pub heal: f64,
    pub dash: Vector3<f64>,
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
    /// Stains splashed on the ground: where, how big in meters, of what material, and whether
    /// they burn off what grows there
    pub stains: Vec<(Vector3<f64>, f64, u8, bool)>,
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

/// The last blow of a combo, which hits hardest
const FINISHER: u8 = 2;
/// How much harder
const FINISHER_POWER: f64 = 1.4;

/// When a swing in `style` starts coming down, and when it lands, as parts of the whole swing.
/// `combo` is which blow of a combo it is.
fn timing(style: Style, heavy: bool, combo: u8) -> (f64, f64) {
    if combo == FINISHER && !heavy {
        return match style {
            Style::Slash => (0.3, 0.45),
            Style::Smash => (0.45, 0.6),
            Style::Stab => (0.35, 0.5),
            Style::Thrust => (0.3, 0.47),
            Style::Lash => (0.4, 0.5),
            _ => (0.4, 0.52),
        };
    }
    match (style, heavy) {
        // A whip's wound back slowly and snapped forward fast
        (Style::Lash, false) => (0.3, 0.4),
        (Style::Stab | Style::Thrust, false) => (0.25, 0.4),
        (Style::Stab | Style::Thrust, true) => (0.35, 0.5),
        (Style::Smash, false) => (0.4, 0.58),
        (Style::Smash, true) => (0.45, 0.6),
        (Style::Chop | Style::Reap, false) => (0.3, 0.47),
        (_, true) => (0.4, 0.52),
        _ => (0.3, 0.45),
    }
}

/// The poses a swing in `style` goes through, for animate(). `combo` is which blow of a combo it
/// is: the second goes back the other way, the third finishes it off.
fn swing(style: Style, heavy: bool, combo: u8) -> Vec<(f64, Pose, bool)> {
    // A whip's swing depends on how rare it is too, see lash_keys(). Without knowing, a rare one's.
    if style == Style::Lash {
        return lash_keys(heavy, combo, LASH_DOUBLE);
    }
    let (comes, lands) = timing(style, heavy, combo);
    let flip = combo == 1;
    let finisher = combo == FINISHER && !heavy;
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
    if finisher {
        return match style {
            // Drawn back to the hip and driven straight in
            Style::Slash => {
                let lunge = pose([0.08, 0.72, -0.15], [-0.1, 0.99, 0.1], [-1.0, 0.0, 0.0]);
                let mut held = lunge.clone();
                held.hand.y -= 0.04;
                vec![(comes, pose([0.3, 0.2, -0.3], [0.05, 0.95, 0.3], [-1.0, 0.0, 0.0]), false), (lands, lunge, true), (lands + 0.12, held, false)]
            }
            // Raised high and slammed down, as a heavy blow
            Style::Smash | Style::Stab => swing(style, true, 0),
            // A wide sweep of the spear
            Style::Thrust => sweep(comes, lands),
            _ => overhead(),
        };
    }
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
        // The blade kept pointing across to the left, the pole swept round from the right, then
        // back the other way, its point leading
        (Style::Reap, false) if flip => vec![
            (comes, pose([-0.05, 0.36, -0.12], [-0.6, 0.35, 0.72], [1.0, 0.2, 0.0]), false),
            (lands, pose([0.16, 0.55, -0.25], [0.0, 0.95, 0.3], [1.0, 0.1, 0.0]), true),
            (lands + 0.13, pose([0.34, 0.45, -0.3], [0.7, 0.6, 0.2], [0.6, -0.8, 0.0]), false),
        ],
        (Style::Reap, false) => vec![
            (comes, pose([0.3, 0.32, -0.12], [0.6, 0.35, 0.72], [-1.0, 0.2, 0.0]), false),
            (lands, pose([0.12, 0.55, -0.25], [0.1, 0.95, 0.3], [-1.0, 0.1, 0.0]), true),
            (lands + 0.13, pose([-0.1, 0.48, -0.3], [-0.7, 0.6, 0.2], [-0.6, -0.8, 0.0]), false),
        ],
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

// What a whip can do grows with its grade, see Combat::lash(). A common one cracks at the first
// thing in line. From uncommon it hits everything along the line, from rare its finisher cracks
// twice, from epic a heavy blow snares what it hits and drags it in, from legendary it arcs
// through a wide fan, and a mythic one's finisher goes all the way round first.
const LASH_LINE: usize = 1;
const LASH_DOUBLE: usize = 2;
const LASH_SNARE: usize = 3;
const LASH_FAN: usize = 4;
const LASH_CIRCLE: usize = 5;
/// When a double crack's second crack lands, as part of the whole swing, and how hard it hits
const LASH_SECOND: f64 = 0.68;
const LASH_SECOND_POWER: f64 = 0.8;
/// How wide a whip's fan reaches, as the cosine of the angle out from straight ahead, and how
/// hard it hits each thing in it, and in the circle
const LASH_FAN_CONE: f64 = 0.5;
const LASH_SPREAD_POWER: f64 = 0.85;
/// A whip hits harder with its tip: what's in the furthest LASH_TIP_REACH of its reach
const LASH_TIP: f64 = 1.2;
const LASH_TIP_REACH: f64 = 0.35;
/// How far a snare drags what it catches at most, how close it brings it, in meters, and how
/// long it's knocked off its feet
const LASH_SNARE_PULL: f64 = 2.5;
const LASH_SNARE_CLOSEST: f64 = 1.5;
const LASH_SNARE_STAGGER: f64 = 0.7;
/// What's thrown slides to a stop as e^(-PUSH_DECAY t), see Animal's update, so it goes 1/PUSH_DECAY
/// of the speed it's thrown at
const PUSH_DECAY: f64 = 6.0;

/// The poses a whip's swing goes through, for animate(), by `combo` as for swing(), and by its
/// `grade` as what it can do grows with it, see LASH_LINE and the rest: wound back over the
/// shoulder or out to the side, then snapped forward to crack out at the crosshair. The lash is
/// stiff in the model, so these are worked out for the curve weapons::whip() gives it, its tip
/// about 22° off the line of the handle towards the model's x.
fn lash_keys(heavy: bool, combo: u8, grade: usize) -> Vec<(f64, Pose, bool)> {
    let (comes, lands) = timing(Style::Lash, heavy, combo);
    let finisher = combo == FINISHER && !heavy;
    // Raised over the shoulder, the lash out of sight behind
    let over = pose([0.27, 0.3, 0.03], [0.1, -0.75, 0.65], [0.0, 0.65, 0.75]);
    let raised = pose([0.24, 0.24, 0.08], [0.05, -0.88, 0.47], [0.0, 0.47, 0.88]);
    // Cracked out at the crosshair, then falling after it
    let cracked = pose([0.16, 0.62, -0.17], [-0.2, 0.93, 0.33], [0.0, 0.33, -0.93]);
    let fallen = pose([0.15, 0.58, -0.23], [-0.15, 0.97, 0.05], [0.1, 0.1, -0.99]);
    if heavy && grade >= LASH_SNARE {
        // Cracked out level, caught, and yanked back in
        let out = pose([0.13, 0.68, -0.15], [-0.17, 0.95, 0.26], [0.0, 0.26, -0.97]);
        let mut caught = out.clone();
        caught.hand.y -= 0.02;
        let yanked = pose([0.3, 0.36, -0.26], [0.0, 0.45, 0.9], [0.0, 0.9, -0.45]);
        return vec![(comes, raised, false), (lands, out, true), (lands + 0.04, caught, false), (lands + 0.16, yanked, true)];
    }
    if finisher && grade >= LASH_CIRCLE {
        // Whirled round from the right, across and out of sight to the left, round behind, and
        // back in from the right to crack out ahead
        let right = pose([0.55, 0.3, -0.2], [0.95, 0.2, 0.25], [-0.2, 0.98, 0.0]);
        let across = pose([0.15, 0.6, -0.2], [0.38, 0.9, 0.15], [-0.92, 0.38, 0.0]);
        let left = pose([-0.5, 0.38, -0.25], [-0.8, 0.55, 0.15], [-0.55, -0.83, 0.0]);
        return vec![(comes, right.clone(), false), (lands, across, true), (lands + 0.07, left, false), (lands + 0.08, right, false), (LASH_SECOND, cracked, true), (LASH_SECOND + 0.12, fallen, false)];
    }
    if heavy || finisher {
        // Raised high and cracked down hard, then for a double crack flicked up and cracked again
        let first = pose([0.14, 0.64, -0.16], [-0.18, 0.9, 0.4], [0.0, 0.4, -0.9]);
        if finisher && grade >= LASH_DOUBLE {
            let flicked = pose([0.16, 0.56, -0.12], [-0.12, 0.72, 0.68], [0.0, 0.68, -0.72]);
            let second = pose([0.12, 0.66, -0.2], [-0.16, 0.94, 0.3], [0.05, 0.3, -0.95]);
            return vec![(comes, raised, false), (lands, first, true), (lands + 0.09, flicked, false), (LASH_SECOND, second, true), (LASH_SECOND + 0.12, fallen, false)];
        }
        return vec![(comes, raised, false), (lands, first, true), (lands + 0.14, fallen, false)];
    }
    match (grade >= LASH_FAN, combo == 1) {
        // Swept round from the right across to the left, the lash arcing through a wide fan
        (true, false) => vec![
            (comes, pose([0.45, 0.35, -0.08], [0.7, 0.1, 0.7], [-0.1, 0.95, -0.2]), false),
            (lands, pose([0.16, 0.62, -0.18], [0.3, 0.92, 0.25], [-0.95, 0.3, 0.05]), true),
            (lands + 0.13, pose([-0.05, 0.55, -0.22], [-0.55, 0.8, 0.15], [-0.8, -0.55, 0.0]), false),
        ],
        // Then backhanded, from the left across to the right
        (true, true) => vec![
            (comes, pose([0.0, 0.38, -0.05], [-0.6, 0.15, 0.78], [0.15, 0.95, -0.1]), false),
            (lands, pose([0.2, 0.62, -0.18], [-0.42, 0.88, 0.22], [0.9, 0.42, 0.0]), true),
            (lands + 0.13, pose([0.36, 0.52, -0.24], [0.45, 0.88, 0.1], [0.88, -0.45, 0.0]), false),
        ],
        // Flicked round from out at the side
        (false, true) => vec![
            (comes, pose([0.42, 0.36, -0.1], [0.8, -0.35, 0.45], [0.4, 0.9, 0.0]), false),
            (lands, pose([0.18, 0.62, -0.2], [0.3, 0.94, 0.12], [-0.95, 0.3, 0.0]), true),
            (lands + 0.12, pose([0.1, 0.6, -0.25], [-0.2, 0.97, -0.05], [-0.97, -0.2, -0.1]), false),
        ],
        // Cracked down from over the shoulder
        (false, false) => vec![(comes, over, false), (lands, cracked, true), (lands + 0.12, fallen, false)],
    }
}

/// How long a special attack takes with a weapon of `kind` and `style`, and the poses the weapon
/// goes through, for animate()
fn special_move(special: Special, kind: Kind, style: Style) -> (f64, Vec<(f64, Pose, bool)>) {
    // Forehand and backhand slashes, each wound up and landed
    let forehand = (pose([0.3, 0.34, -0.08], [0.45, 0.3, 0.84], [-0.83, 0.2, 0.55]), pose([-0.12, 0.52, -0.38], [-0.8, 0.5, -0.3], [-0.35, -0.1, -0.93]));
    let backhand = (pose([-0.02, 0.36, -0.06], [-0.45, 0.3, 0.84], [0.83, 0.2, 0.55]), pose([0.36, 0.52, -0.36], [0.8, 0.5, -0.3], [0.35, -0.1, -0.93]));
    // A bow drawn and loosed, kicking a little each time
    let drawn = pose([-0.1, 0.66, -0.08], [-0.33, 0.02, 0.94], [0.0, 1.0, 0.0]);
    let kicked = |side: f64| {
        let mut p = drawn.clone();
        p.hand.y -= 0.04;
        p.hand.x += 0.03 * side;
        p
    };
    let staff = kind == Kind::Magic;
    match special {
        // Swept right round, twice, out of sight behind and back in from the right
        Special::Whirlwind => {
            let right = pose([0.55, 0.3, -0.25], [0.9, 0.35, 0.2], [-0.3, 1.0, 0.0]);
            let middle = pose([0.0, 0.62, -0.22], [0.1, 0.99, 0.05], [-1.0, 0.1, 0.0]);
            let left = pose([-0.6, 0.35, -0.3], [-0.9, 0.3, 0.1], [-0.3, -1.0, 0.0]);
            let low = pose([0.35, 0.3, -0.7], [-0.05, 0.62, 0.78], [0.0, 1.0, 0.0]);
            let keys = vec![(0.08, right.clone(), false), (0.22, middle.clone(), true), (0.36, left.clone(), false), (0.37, right, false), (0.52, middle, true), (0.68, left, false), (0.7, low, false)];
            (0.75, keys)
        }
        // Five quick slashes back and forth
        Special::Flurry => {
            let length = 0.55;
            let keys = (1..=5)
                .flat_map(|k| {
                    let (wound, landed) = if k % 2 == 1 { forehand.clone() } else { backhand.clone() };
                    [((0.08 * k as f64 - 0.04) / length, wound, false), (0.08 * k as f64 / length, landed, true)]
                })
                .collect();
            (length, keys)
        }
        // Raised high overhead as it leaps, then brought straight down
        Special::Skyfall => {
            let slammed = pose([0.04, 0.62, -0.42], [0.0, 0.6, -0.8], [0.0, -0.8, -0.6]);
            let mut settled = slammed.clone();
            settled.hand.z -= 0.05;
            (1.0, vec![(0.3, pose([0.1, 0.3, 0.06], [0.05, -0.25, 0.97], [0.0, 1.0, 0.0]), false), (0.45, slammed, true), (0.6, settled, false)])
        }
        // Held across the face to guard
        Special::Riposte => {
            let guard = pose([0.12, 0.42, -0.12], [-0.95, 0.15, 0.25], [0.0, 1.0, 0.1]);
            let mut braced = guard.clone();
            braced.hand.y -= 0.06;
            (1.0, vec![(0.25, guard, false), (0.3, braced.clone(), false), (0.85, braced, false)])
        }
        // Three arrows loosed, drawn again between
        Special::Volley => (0.7, vec![(0.14, drawn.clone(), false), (0.18, kicked(1.0), true), (0.27, drawn.clone(), false), (0.35, kicked(-1.0), true), (0.45, drawn.clone(), false), (0.52, kicked(0.0), true)]),
        // Thrust out, held as the lightning leaps
        Special::Stormchain => {
            let out = pose([0.18, 0.66, -0.18], [-0.18, 0.9, 0.4], [1.0, 0.0, 0.0]);
            (1.2, vec![(0.1, out.clone(), true), (0.75, out, false)])
        }
        // Drawn back low and swept up and out, sending the wave
        Special::Riptide if staff => {
            let swept = pose([0.0, 0.65, -0.1], [-0.5, 0.75, 0.45], [0.0, 0.0, 1.0]);
            (1.0, vec![(0.25, pose([0.36, 0.4, -0.45], [0.3, 0.85, -0.3], [0.0, 0.0, 1.0]), false), (0.4, swept.clone(), true), (0.7, swept, false)])
        }
        Special::Riptide => {
            let swept = pose([0.12, 0.78, -0.15], [-0.1, 0.97, 0.2], [-1.0, 0.0, 0.0]);
            (1.0, vec![(0.25, pose([0.3, 0.3, -0.4], [0.05, 0.9, -0.4], [-1.0, 0.0, 0.0]), false), (0.4, swept.clone(), true), (0.7, swept, false)])
        }
        // Braced and levelled, pouring out flame
        Special::Breath => {
            let levelled = pose([0.14, 0.7, -0.2], [-0.1, 0.99, 0.02], [-1.0, 0.0, 0.0]);
            let mut pushed = levelled.clone();
            pushed.hand.y += 0.03;
            (1.9, vec![(0.08, pose([0.3, 0.15, -0.32], [0.0, 0.97, 0.2], [-1.0, 0.0, 0.0]), false), (0.12, levelled, true), (0.88, pushed, false)])
        }
        // The rest: a bow drawn up at the sky and loosed
        _ if kind == Kind::Ranged => {
            let up = pose([-0.08, 0.5, 0.1], [-0.3, 0.1, 0.95], [0.0, 0.6, 0.8]);
            let mut loosed = up.clone();
            loosed.hand.y -= 0.05;
            (0.8, vec![(0.3, up, false), (0.4, loosed.clone(), true), (0.7, loosed, false)])
        }
        // Or the weapon raised to the sky, then thrust out at where it's aimed
        _ => {
            let raised = pose([0.15, 0.4, 0.1], [0.0, 0.2, 0.98], [-1.0, 0.0, 0.0]);
            let thrust = if style == Style::Thrust || staff { pose([0.12, 0.7, -0.18], [-0.1, 0.99, 0.05], [-1.0, 0.0, 0.0]) } else { pose([0.1, 0.62, -0.25], [-0.1, 0.95, 0.25], [-1.0, 0.0, 0.0]) };
            (0.8, vec![(0.3, raised, false), (0.45, thrust.clone(), true), (0.7, thrust, false)])
        }
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
const MOST_SPARKS: usize = 140;

/// Meters walked from one footfall to the next, for bobbing what's in hand
const STEP_LENGTH: f64 = 0.8;

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
            combo: 0,
            combo_timer: 0.0,
            hitstop: 0.0,
            recoil: 0.0,
            shake: 0.0,
            speed: 0.0,
            stride: 0.0,
            player_hits: Vec::new(),
            trail: None,
            aura: 0.0,
            projectiles: Vec::new(),
            hostiles: Vec::new(),
            lingering: Vec::new(),
            strikes: Vec::new(),
            swells: Vec::new(),
            marks: Vec::new(),
            webs: Vec::new(),
            near_misses: Vec::new(),
            pending: Vec::new(),
            guard: None,
            wave_broken: false,
            charge: 0.0,
            heal: 0.0,
            dash: Vector3::zeros(),
            sparks: Vec::new(),
            stuck: Vec::new(),
            flashes: Vec::new(),
            echoes: Vec::new(),
            popups: Vec::new(),
            messages: Vec::new(),
            cues: Vec::new(),
            craters: Vec::new(),
            stains: Vec::new(),
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
        for h in &mut self.hostiles {
            h.position -= shift;
            h.target -= shift;
        }
        for l in &mut self.lingering {
            l.position -= shift;
        }
        for p in &mut self.pending {
            match &mut p.act {
                Act::Meteor(at) | Act::Arrow(at) | Act::Rupture(at) | Act::Star(at) => *at -= shift,
                Act::Arc { from, .. } => *from -= shift,
                Act::Wave { at, .. } => *at -= shift,
                Act::Bolt(_) | Act::Spin | Act::Slash | Act::Dash(_) | Act::Land { .. } | Act::Volley(_) | Act::Gout(_) => {}
            }
        }
        for s in &mut self.strikes {
            s.position -= shift;
        }
        for i in &mut self.swells {
            i.position -= shift;
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
        self.hostiles.clear();
        self.guard = None;
        self.wave_broken = false;
        self.lingering.clear();
        self.strikes.clear();
        self.swells.clear();
        self.pending.clear();
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
        self.marks.clear();
        self.vpm = arena.view.vpm;
        self.speed = arena.view.speed;
        self.stride = arena.view.stride;
        self.time += dt;
        self.mana = (self.mana + MANA_REGEN * dt).min(MAX_MANA);
        self.cooldown -= dt;
        self.frenzy -= dt;
        self.mana_warning -= dt;
        self.hitstop -= dt;
        self.recoil = (self.recoil - dt * 5.0).max(0.0);
        self.shake *= (-7.0 * dt).exp();

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
                None => self.melee(arena, &echo.profile, echo.heavy, echo.power, false),
            }
        }

        self.charge = (self.charge + dt / SPECIAL_SECONDS).min(1.0);
        self.follow_up(arena, dt);
        self.fly(arena, dt);
        self.fly_hostiles(arena, dt);
        self.linger(arena, dt);
        self.afflict(arena, dt);

        for s in &mut self.sparks {
            s.velocity.z -= 9.8 * arena.view.vpm * s.gravity * dt;
            s.velocity *= (-1.5 * dt).exp();
            s.position += s.velocity * dt;
            s.life -= dt;
            // Drops stop where they land, on the ground or in water
            if s.slot == SLOT_DROP && s.velocity.z < 0.0 {
                let p = s.position;
                if p.z <= (arena.water)(p.x, p.y) || !is_passable(arena.world.material_at(p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64)) {
                    s.life = 0.0;
                }
            }
        }
        self.sparks.retain(|s| s.life > 0.0);
        // Arrows with an element go on burning, crackling and such for a while where they stuck
        let vpm = self.vpm;
        let mut motes = Vec::new();
        for s in &mut self.stuck {
            s.life -= dt;
            s.glowing -= dt;
            if s.glowing > 0.0 && s.element.is_some() {
                motes.push((s.position - s.rotation * Vector3::x() * 0.1 * vpm, s.element));
            }
        }
        for (at, element) in motes {
            if random(&mut self.rng) < dt * 12.0 {
                self.mote(at, element, 1.0);
            }
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
        if controls.special && self.raise >= 1.0 {
            match &profile.special {
                Some((special, _)) if self.charge >= 1.0 => {
                    self.charge = 0.0;
                    self.unleash(arena, weapon, *special);
                }
                Some((special, _)) => {
                    self.messages.push(format!("{} isn't charged yet: {:.0}%.", special.name(), self.charge * 100.0));
                }
                None => self.messages.push("Only weapons with a legendary or mythic part have a special attack.".into()),
            }
        }
        // Nothing else till a special attack's done
        if let Action::Special { t, length, special } = self.action {
            self.action = if t + dt >= length { Action::Idle } else { Action::Special { t: t + dt, length, special } };
            return;
        }
        match weapon.kind() {
            Kind::Melee => {
                if let Action::Swing { t, length, heavy, struck, combo } = self.action {
                    let (comes, lands) = timing(profile.style, heavy, combo);
                    let finisher = combo == FINISHER && !heavy;
                    // Stopped dead a moment as a blow lands
                    let dt = if self.hitstop > 0.0 { 0.0 } else { dt };
                    // Whooshing as it comes, and landing partway through the swing
                    if t < length * comes && t + dt >= length * comes {
                        let (sound, pitch) = match profile.style {
                            Style::Stab => (Sound::Stab, if heavy || finisher { 0.85 } else { 1.1 }),
                            Style::Thrust => (Sound::Stab, if heavy { 0.7 } else { 0.85 }),
                            Style::Smash => (Sound::HeavySwing, if heavy || finisher { 0.7 } else { 0.85 }),
                            Style::Chop => (if heavy || finisher { Sound::HeavySwing } else { Sound::Swing }, 0.9),
                            Style::Reap => (if heavy || finisher { Sound::HeavySwing } else { Sound::Swing }, 0.75),
                            // A thin, high whistle as the lash comes round. It cracks in lash().
                            Style::Lash => (Sound::Swing, if heavy || finisher { 1.25 } else { 1.45 }),
                            _ => (if heavy || finisher { Sound::HeavySwing } else { Sound::Swing }, if combo == 1 { 1.08 } else { 1.0 }),
                        };
                        self.cues.push(Cue::here(sound, if finisher { 0.75 } else { 0.6 }).pitched(pitch * if finisher { 0.9 } else { 1.0 }));
                    }
                    let t = t + dt;
                    let land = !struck && t >= length * lands;
                    self.action = if t >= length { Action::Idle } else { Action::Swing { t, length, heavy, struck: struck || land, combo } };
                    if land {
                        self.melee(arena, &profile, heavy, if finisher { FINISHER_POWER } else { 1.0 }, finisher);
                    }
                } else {
                    // A combo's dropped if the next blow doesn't follow soon enough
                    self.combo_timer -= dt;
                    if self.combo_timer <= 0.0 {
                        self.combo = 0;
                    }
                    if (controls.attack || controls.holding || controls.heavy) && self.cooldown <= 0.0 && self.raise >= 1.0 {
                        let heavy = controls.heavy;
                        let combo = if heavy { 0 } else { self.combo };
                        let finisher = combo == FINISHER && !heavy;
                        let length = (if heavy { 1.6 } else if finisher { 1.25 } else { 1.0 }) / (profile.stat("attackSpeed") * speedup).max(0.1);
                        // Light blows go back and forth, then finish with a harder one
                        self.combo = if heavy { 0 } else { (combo + 1) % (FINISHER + 1) };
                        self.combo_timer = 0.5;
                        self.action = Action::Swing { t: 0.0, length, heavy, struck: false, combo };
                        self.cooldown = length;
                    }
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

    /// The special attack of the weapon in hand, see weapons::Special
    fn unleash(&mut self, arena: &mut Arena, weapon: &Weapon, special: Special) {
        let view = arena.view;
        let vpm = view.vpm;
        let profile = weapon.profile.clone();
        let base = match profile.kind {
            Kind::Magic => profile.stat("spellDamage"),
            _ => profile.stat("physicalDamage"),
        } + profile.stat("elementalDamage");
        let target = self.aim(arena);
        let feet = arena.player.unwrap_or(view.eye - Vector3::new(0.0, 0.0, 1.6 * vpm));
        let forward = view.forward();
        let flat = Vector3::new(forward.x, forward.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x());
        let look = Element::look(profile.element);
        let disc = |rng: &mut u32, radius: f64| {
            let a = random(rng) * TAU;
            let r = random(rng).sqrt() * radius * vpm;
            Vector3::new(a.cos() * r, a.sin() * r, 0.0)
        };
        // The weapon goes through its own moves for it, see special_move()
        let (length, _) = special_move(special, weapon.kind(), profile.style);
        self.action = Action::Special { t: 0.0, length, special };
        self.cooldown = length;
        // It goes off with a flash, a shout of its element, and the view shaking
        self.flashes.push(Flash { position: view.eye, color: profile.element.map_or([1.0, 0.9, 0.7], Element::light), reach: 8.0, life: 0.4, lifetime: 0.4 });
        self.cues.push(Cue::here(Sound::Cast(look as u8), 1.0).pitched(0.65));
        self.shake = self.shake.max(0.5);
        self.messages.push(format!("{}!", special.name()));
        let later = |pending: &mut Vec<Pending>, delay: f64, act: Act, damage: f64| pending.push(Pending { delay, act, profile: profile.clone(), damage });
        // How far the player can dash on ahead, in meters, before running into something
        let room = arena
            .animals
            .iter()
            .filter(|a| a.health > 0.0 && !a.look.is_pet())
            .filter_map(|a| {
                let to = a.position - feet;
                let d = to.xy().norm() / vpm;
                (d < 14.0 && to.xy().normalize().dot(&flat.xy()) > 0.8).then_some(d - a.look.body().1 - 1.5)
            })
            .fold(f64::MAX, f64::min)
            .max(0.0);
        match special {
            Special::Meteors => {
                for k in 0..8 {
                    let at = target + disc(&mut self.rng, 3.5);
                    later(&mut self.pending, 0.15 + k as f64 * 0.16, Act::Meteor(at), base * 2.6);
                }
            }
            Special::Rain => {
                for k in 0..28 {
                    let at = target + disc(&mut self.rng, 4.5);
                    later(&mut self.pending, 0.1 + k as f64 * 0.035, Act::Arrow(at), base * 0.7);
                }
            }
            Special::Tempest => {
                let mut near: Vec<(f64, u32)> = arena
                    .animals
                    .iter()
                    .filter(|a| a.health > 0.0 && !a.look.is_pet())
                    .map(|a| ((a.position - feet).norm(), a.id))
                    .filter(|&(d, _)| d < 25.0 * vpm)
                    .collect();
                near.sort_by(|a, b| a.0.total_cmp(&b.0));
                for (k, &(_, id)) in near.iter().take(10).enumerate() {
                    later(&mut self.pending, 0.1 + k as f64 * 0.12, Act::Bolt(id), base * 1.5);
                }
                if near.is_empty() {
                    later(&mut self.pending, 0.1, Act::Rupture(target), base);
                }
            }
            Special::Singularity => {
                self.lingering.push(Lingering { position: target, radius: 4.5, life: 2.2, element: Element::Void, profile: profile.clone(), damage: base * 0.2, tick: 0.0, effects: false, collapse: base * 2.2 });
            }
            Special::Blizzard => {
                self.lingering.push(Lingering { position: target, radius: 5.0, life: 4.0, element: Element::Frost, profile: profile.clone(), damage: base * 0.3, tick: 0.0, effects: true, collapse: 0.0 });
                self.stains.push((target, 5.0, SNOW, false));
            }
            Special::Plague => {
                self.lingering.push(Lingering { position: target, radius: 5.5, life: 7.0, element: Element::Poison, profile: profile.clone(), damage: base * 0.25, tick: 0.0, effects: true, collapse: 0.0 });
                for _ in 0..50 {
                    let v = self.any_direction() * 0.8 + Vector3::new(0.0, 0.0, 1.0 + random(&mut self.rng));
                    self.drop(target, v * 4.0 * vpm, DROP_VENOM, 1.5);
                }
                self.stains.push((target, 5.0, MOSS, false));
            }
            Special::Fissure => {
                for k in 1..=10 {
                    let ahead = feet + flat * (2.0 + k as f64 * 1.5) * vpm + Vector3::new(0.0, 0.0, 2.0 * vpm);
                    let down = Vector3::new(0.0, 0.0, -5.0 * vpm);
                    let at = solid_along(arena.world, ahead, ahead + down).map_or(ahead + down, |t| ahead + down * t);
                    later(&mut self.pending, k as f64 * 0.07, Act::Rupture(at), base * 1.3);
                }
            }
            // As the blade comes round in front each time
            Special::Whirlwind => {
                later(&mut self.pending, 0.165, Act::Spin, base * 1.3);
                later(&mut self.pending, 0.39, Act::Spin, base * 1.3);
            }
            // Up and on, then driven down onto the ground
            Special::Skyfall => {
                self.dash = flat * (room / 0.45).min(12.0) * vpm + Vector3::new(0.0, 0.0, 9.0 * vpm);
                later(&mut self.pending, 0.25, Act::Land { wait: 1.5 }, base);
                later(&mut self.pending, 0.4, Act::Dash(Vector3::new(0.0, 0.0, -20.0 * vpm)), 0.0);
            }
            Special::Riposte => {
                self.guard = Some(Guard { left: 1.2, profile: profile.clone(), damage: base });
            }
            // Three waves of three arrows, the middle first, fanning out
            Special::Volley => {
                for (k, degrees) in [0.0, -5.0, 5.0, -10.0, 10.0, -15.0, 15.0, -20.0, 20.0].into_iter().enumerate() {
                    later(&mut self.pending, 0.1 + 0.12 * (k / 3) as f64, Act::Volley(degrees), base * 0.9);
                }
            }
            // At what's nearest where it's aimed, or else whatever's ahead
            Special::Stormchain => {
                let tip = self.held_point(weapon, view, false);
                let alive = |a: &&Animal| a.health > 0.0 && !a.look.is_pet();
                let near_aim = arena.animals.iter().filter(alive).map(|a| ((a.center(vpm as u32) - target).norm(), a.id)).filter(|&(d, _)| d < 4.0 * vpm).min_by(|a, b| a.0.total_cmp(&b.0));
                let ahead = || {
                    arena
                        .animals
                        .iter()
                        .filter(alive)
                        .filter_map(|a| {
                            let to = a.center(vpm as u32) - view.eye;
                            (to.norm() < 20.0 * vpm && to.normalize().dot(&forward) > 0.7).then_some((to.norm(), a.id))
                        })
                        .min_by(|a, b| a.0.total_cmp(&b.0))
                };
                match near_aim.or_else(ahead) {
                    Some((_, id)) => later(&mut self.pending, 0.12, Act::Arc { from: tip, to: id, last: u32::MAX, hop: 0, cap: base * 2.0 }, base * 0.8),
                    None => {
                        self.bolt_line(tip, target, Element::look(Some(Element::Storm)));
                        self.burst(arena, &profile, target, 1.8, base * 1.5, 0, false, Some(Element::Storm), None);
                    }
                }
            }
            // A wave rolling on ahead over the ground
            Special::Riptide => {
                self.wave_broken = false;
                for k in 1..=12 {
                    let ahead = feet + flat * (1.5 + k as f64 * 1.2) * vpm + Vector3::new(0.0, 0.0, 2.0 * vpm);
                    let down = Vector3::new(0.0, 0.0, -5.0 * vpm);
                    let at = solid_along(arena.world, ahead, ahead + down).map_or(ahead + down, |t| ahead + down * t);
                    later(&mut self.pending, k as f64 * 0.06, Act::Wave { at, dir: flat, step: k }, base);
                }
            }
            Special::Breath => {
                for k in 0..8 {
                    later(&mut self.pending, 0.1 + 0.2 * k as f64, Act::Gout(k), base);
                }
            }
            // A hop carries the player about a third of a meter on for each meter a second
            Special::Flurry => {
                self.dash = flat * (room / 0.31).min(26.0) * vpm + Vector3::new(0.0, 0.0, 3.0 * vpm);
                for k in 1..=5 {
                    later(&mut self.pending, k as f64 * 0.08, Act::Slash, base * 0.7);
                }
            }
            Special::Nova => {
                // A ring of light sweeping out from the player, throwing everything back
                let holy = Element::look(Some(Element::Holy));
                for k in 0..72 {
                    let a = k as f64 / 72.0 * TAU;
                    for (height, speed) in [(0.4, 9.0), (1.0, 7.5)] {
                        let at = feet + Vector3::new(a.cos() * 0.8 * vpm, a.sin() * 0.8 * vpm, height * vpm);
                        self.sparks.push(Spark { position: at, velocity: Vector3::new(a.cos(), a.sin(), 0.0) * speed * vpm, look: holy, life: 0.6, lifetime: 0.6, gravity: 0.0, slot: SLOT_SPARK });
                    }
                }
                self.cap_sparks();
                self.flashes.push(Flash { position: feet + Vector3::new(0.0, 0.0, 1.0 * vpm), color: Element::Holy.light(), reach: 12.0, life: 0.5, lifetime: 0.5 });
                self.cues.push(Cue::here(Sound::Burst, 1.0).pitched(1.1));
                let near: Vec<usize> = (0..arena.animals.len())
                    .filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet() && (arena.animals[i].position - feet).norm() < 7.0 * vpm + arena.animals[i].look.body().1 * vpm)
                    .collect();
                for i in near {
                    let away = arena.animals[i].position - feet;
                    arena.animals[i].status.push = Vector3::new(away.x, away.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x()) * 9.0 * vpm;
                    let at = arena.animals[i].center(vpm as u32);
                    self.element_hit(at, Some(Element::Holy), 1.5);
                    self.strike(arena, &profile, i, Blow { damage: base * 1.8, crit: false, heavy: true, from: feet, depth: 0, element: Some(Element::Holy) });
                }
                self.heal += 30.0;
            }
            Special::Bloodmoon => {
                let chest = feet + Vector3::new(0.0, 0.0, 1.2 * vpm);
                let near: Vec<usize> = (0..arena.animals.len())
                    .filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet() && (arena.animals[i].position - feet).norm() < 14.0 * vpm)
                    .collect();
                let blood = Element::look(Some(Element::Blood));
                for i in near {
                    let center = arena.animals[i].center(vpm as u32);
                    self.bolt_line(center, chest, blood);
                    self.strike(arena, &profile, i, Blow { damage: base * 1.2, crit: false, heavy: true, from: chest, depth: 0, element: Some(Element::Blood) });
                    self.heal += base * 0.3;
                }
                self.heal = self.heal.min(45.0);
            }
            Special::Starfall => {
                for k in 0..12 {
                    let from = feet + Vector3::new(0.0, 0.0, 3.0 * vpm) + disc(&mut self.rng, 1.5);
                    later(&mut self.pending, k as f64 * 0.05, Act::Star(from), base * 0.6);
                }
            }
        }
    }

    /// A projectile set off as it is, without multishot or echo
    #[allow(clippy::too_many_arguments)]
    fn launch(&mut self, profile: &Rc<Profile>, arrow: Option<(usize, bool)>, form: Form, from: Vector3<f64>, velocity: Vector3<f64>, damage: f64, gravity: f64) {
        let (head, power) = arrow.unwrap_or((0, false));
        let (_, _, size, _) = form.flight();
        self.projectiles.push(Projectile {
            arrow: arrow.is_some(),
            position: from,
            velocity,
            damage,
            profile: profile.clone(),
            pierce: 0,
            hit: Vec::new(),
            life: 4.0,
            head,
            power,
            trail: 0.0,
            form,
            gravity,
            size,
            pin: 0.0,
        });
    }

    /// The parts of special attacks whose time has come
    fn follow_up(&mut self, arena: &mut Arena, dt: f64) {
        let vpm = arena.view.vpm;
        let feet = arena.player.unwrap_or(arena.view.eye - Vector3::new(0.0, 0.0, 1.6 * vpm));
        // Nothing came at the player while they stood guard: a slash anyway
        if let Some(guard) = &mut self.guard {
            guard.left -= dt;
            if guard.left <= 0.0 {
                let guard = self.guard.take().unwrap();
                self.pending.push(Pending { delay: 0.0, act: Act::Slash, profile: guard.profile, damage: guard.damage * 1.2 });
            }
        }
        for mut p in std::mem::take(&mut self.pending) {
            p.delay -= dt;
            if p.delay > 0.0 {
                self.pending.push(p);
                continue;
            }
            let profile = p.profile.clone();
            match p.act {
                Act::Meteor(at) => {
                    let from = at + Vector3::new((random(&mut self.rng) - 0.5) * 8.0, (random(&mut self.rng) - 0.5) * 8.0, 28.0) * vpm;
                    self.launch(&profile, None, Form::Fireball, from, (at - from).normalize() * 32.0 * vpm, p.damage, 0.0);
                }
                Act::Arrow(at) => {
                    let from = at + Vector3::new((random(&mut self.rng) - 0.5) * 3.0, (random(&mut self.rng) - 0.5) * 3.0, 30.0) * vpm;
                    let head = (random(&mut self.rng) * 3.0) as usize % 3;
                    self.launch(&profile, Some((head, true)), Form::Bolt, from, (at - from).normalize() * 45.0 * vpm, p.damage, ARROW_GRAVITY);
                }
                Act::Bolt(id) => {
                    if let Some(i) = arena.animals.iter().position(|a| a.id == id && a.health > 0.0) {
                        let center = arena.animals[i].center(vpm as u32);
                        self.bolt_line(center + Vector3::new(0.0, 0.0, 14.0 * vpm), center, Element::look(Some(Element::Storm)));
                        self.burst(arena, &profile, center, 1.8, p.damage, 0, false, Some(Element::Storm), None);
                    }
                }
                Act::Rupture(at) => {
                    for _ in 0..10 {
                        let v = self.any_direction() + Vector3::new(0.0, 0.0, 1.5);
                        self.drop(at, v * 4.0 * vpm, DROP_MUD, 1.2);
                    }
                    self.burst(arena, &profile, at, 1.7, p.damage, 0, false, profile.element.or(Some(Element::Void)), None);
                }
                Act::Spin => {
                    // A ring of light round the player as the blade goes round
                    let look = Element::look(profile.element);
                    for k in 0..36 {
                        let a = k as f64 / 36.0 * TAU;
                        self.fine(feet + Vector3::new(a.cos() * 2.0 * vpm, a.sin() * 2.0 * vpm, 1.1 * vpm), Vector3::new(-a.sin(), a.cos(), 0.0) * 3.0 * vpm, look, 0.3, 0.0);
                    }
                    self.cues.push(Cue::here(Sound::HeavySwing, 0.9).pitched(0.8));
                    let reach = (profile.stat("range") + 0.8) * vpm;
                    let around: Vec<usize> = (0..arena.animals.len())
                        .filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet() && (arena.animals[i].center(vpm as u32) - feet).norm() < reach + arena.animals[i].look.body().1 * vpm)
                        .collect();
                    for i in around {
                        let at = arena.animals[i].center(vpm as u32);
                        self.element_hit(at, profile.element, 1.5);
                        self.strike(arena, &profile, i, Blow { damage: p.damage, crit: false, heavy: true, from: feet, depth: 0, element: profile.element });
                    }
                    self.shake = self.shake.max(0.4);
                }
                Act::Slash => {
                    let view = arena.view;
                    let forward = view.forward();
                    let reach = 6.0 * vpm;
                    let ahead: Vec<usize> = (0..arena.animals.len())
                        .filter(|&i| {
                            let a = &arena.animals[i];
                            let to = a.center(vpm as u32) - view.eye;
                            a.health > 0.0 && !a.look.is_pet() && to.norm() < reach + a.look.body().1 * vpm && to.normalize().dot(&forward) > 0.2
                        })
                        .collect();
                    self.cues.push(Cue::here(Sound::Swing, 0.7).pitched(1.2 + 0.2 * random(&mut self.rng) as f32));
                    let look = Element::look(profile.element);
                    let side = self.any_direction() * 0.6 * vpm;
                    self.bolt_line(view.eye + forward * 1.2 * vpm - side, view.eye + forward * 1.2 * vpm + side, look);
                    for i in ahead {
                        let at = arena.animals[i].center(vpm as u32);
                        self.element_hit(at, profile.element, 1.2);
                        self.strike(arena, &profile, i, Blow { damage: p.damage, crit: false, heavy: false, from: view.eye, depth: 0, element: profile.element });
                    }
                }
                Act::Star(from) => {
                    // At whatever's nearest, or on ahead
                    let target = arena
                        .animals
                        .iter()
                        .filter(|a| a.health > 0.0 && !a.look.is_pet() && (a.position - feet).norm() < 25.0 * vpm)
                        .map(|a| a.center(vpm as u32))
                        .min_by(|a, b| (a - from).norm().total_cmp(&(b - from).norm()))
                        .unwrap_or(from + arena.view.forward() * 15.0 * vpm);
                    let direction = ((target - from).normalize() + self.any_direction() * 0.6).normalize();
                    self.launch(&profile, None, Form::Missile, from, direction * 20.0 * vpm, p.damage, 0.0);
                }
                Act::Dash(velocity) => self.dash += velocity,
                Act::Land { wait } => {
                    // Not down yet: looking again in a moment
                    let (x, y, z) = (feet.x.floor() as i64, feet.y.floor() as i64, feet.z.floor() as i64);
                    let down = arena.player.is_none() || (1..=(0.4 * vpm) as i64).any(|k| !is_passable(arena.world.material_at(x, y, z - k)));
                    if !down && wait > 0.0 {
                        self.pending.push(Pending { delay: 0.02, act: Act::Land { wait: wait - 0.02 }, profile, damage: p.damage });
                        continue;
                    }
                    // Crashing down: a crater, a ring of sparks and earth sweeping out along the
                    // ground, everything near hit, knocked off its feet and thrown back, then the
                    // ground bursting open in a ring
                    self.craters.push((feet, 1.2));
                    let look = Element::look(profile.element);
                    for k in 0..48 {
                        let a = k as f64 / 48.0 * TAU;
                        let out = Vector3::new(a.cos(), a.sin(), 0.0);
                        self.sparks.push(Spark { position: feet + out * 0.8 * vpm + Vector3::new(0.0, 0.0, 0.3 * vpm), velocity: out * 10.0 * vpm, look, life: 0.45, lifetime: 0.45, gravity: 0.0, slot: SLOT_SPARK });
                        if k % 2 == 0 {
                            self.drop(feet + out * 1.2 * vpm, (out * 3.0 + Vector3::new(0.0, 0.0, 4.0)) * vpm, DROP_MUD, 1.0);
                        }
                    }
                    let near: Vec<usize> = (0..arena.animals.len())
                        .filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet() && (arena.animals[i].position - feet).norm() < (6.0 + arena.animals[i].look.body().1) * vpm)
                        .collect();
                    for i in near {
                        let away = arena.animals[i].position - feet;
                        if away.norm() < (4.0 + arena.animals[i].look.body().1) * vpm {
                            let at = arena.animals[i].center(vpm as u32);
                            self.element_hit(at, profile.element, 2.0);
                            self.strike(arena, &profile, i, Blow { damage: p.damage * 2.4, crit: false, heavy: true, from: feet, depth: 0, element: profile.element });
                        }
                        let s = &mut arena.animals[i].status;
                        s.staggered = s.staggered.max(0.8);
                        s.push = Vector3::new(away.x, away.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x()) * 10.0 * vpm;
                    }
                    self.cap_sparks();
                    for k in 0..8 {
                        let a = k as f64 / 8.0 * TAU;
                        let at = feet + Vector3::new(a.cos(), a.sin(), 0.0) * 3.5 * vpm;
                        self.pending.push(Pending { delay: 0.12, act: Act::Rupture(at), profile: profile.clone(), damage: p.damage * 0.8 });
                    }
                    self.cues.push(Cue::at(Sound::Quake, feet, 1.0));
                    self.shake = self.shake.max(1.0);
                }
                Act::Volley(degrees) => {
                    let view = arena.view;
                    let direction = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), degrees.to_radians()) * view.forward();
                    let from = view.eye + view.rotation * Vector3::new(-0.1, 0.6, -0.1) * vpm;
                    let head = (random(&mut self.rng) * 3.0) as usize % 3;
                    let speed = profile.stat("projectileSpeed") * 1.1 * vpm;
                    self.launch(&profile, Some((head, true)), Form::Bolt, from, direction * speed, p.damage, ARROW_GRAVITY * 0.5);
                    if let Some(arrow) = self.projectiles.last_mut() {
                        arrow.pin = 2.5;
                    }
                    if degrees.abs() < 7.0 {
                        self.cues.push(Cue::here(Sound::Twang, 0.8).pitched(1.1));
                    }
                }
                Act::Arc { from, to, last, hop, cap } => {
                    let Some(i) = arena.animals.iter().position(|a| a.id == to && a.health > 0.0) else {
                        continue;
                    };
                    let center = arena.animals[i].center(vpm as u32);
                    self.bolt_line(from, center, Element::look(Some(Element::Storm)));
                    // Only the first leap brings the weapon's own effects
                    self.burst(arena, &profile, center, 1.2, p.damage, if hop == 0 { 0 } else { 1 }, false, Some(Element::Storm), None);
                    self.cues.push(Cue::at(Sound::Crackle, center, 0.8).pitched(1.0 + 0.05 * hop as f32));
                    // On to the nearest other, back and forth between two, or striking the one
                    // there is again
                    let next = arena
                        .animals
                        .iter()
                        .filter(|a| a.health > 0.0 && !a.look.is_pet() && a.id != to && a.id != last)
                        .map(|a| ((a.center(vpm as u32) - center).norm(), a.id))
                        .filter(|&(d, _)| d < 9.0 * vpm)
                        .min_by(|a, b| a.0.total_cmp(&b.0))
                        .map(|(_, id)| id)
                        .or_else(|| arena.animals.iter().any(|a| a.id == to && a.health > 0.0).then_some(to));
                    match next {
                        Some(next) if hop < 11 => {
                            self.pending.push(Pending { delay: 0.09, act: Act::Arc { from: center, to: next, last: to, hop: hop + 1, cap }, profile, damage: (p.damage * 1.15).min(cap) });
                        }
                        _ => self.burst(arena, &profile, center, 2.5, p.damage * 1.2, 0, false, Some(Element::Storm), None),
                    }
                }
                Act::Wave { at, dir, step } => {
                    // Already crashed down on something big
                    if self.wave_broken {
                        self.wave_broken = step < 12;
                        continue;
                    }
                    // A crest of water across seven meters
                    let side = Vector3::new(-dir.y, dir.x, 0.0);
                    for _ in 0..28 {
                        let across = (random(&mut self.rng) - 0.5) * 7.0;
                        let v = dir * 2.0 + Vector3::new(0.0, 0.0, 3.0 + 2.0 * random(&mut self.rng));
                        self.drop(at + side * across * vpm, v * vpm, DROP_WATER, 0.9);
                    }
                    if step % 3 == 0 {
                        self.splash(at, 1.0);
                    }
                    // Sweeping along everything it reaches
                    let caught: Vec<usize> = (0..arena.animals.len())
                        .filter(|&i| {
                            let a = &arena.animals[i];
                            let off = a.position - at;
                            let body = a.look.body().1;
                            a.health > 0.0 && !a.look.is_pet() && off.dot(&side).abs() < (3.5 + body) * vpm && off.dot(&dir).abs() < (0.9 + body) * vpm
                        })
                        .collect();
                    // Crashing down at the end, or on the first thing too big to sweep along
                    let crash = step == 12 || caught.iter().any(|&i| arena.animals[i].boss.is_some());
                    for i in caught {
                        arena.animals[i].status.push = dir * 14.0 * vpm;
                        self.strike(arena, &profile, i, Blow { damage: p.damage * 0.15, crit: false, heavy: false, from: at - dir * vpm, depth: if step % 4 == 0 { 0 } else { 1 }, element: profile.element });
                    }
                    if crash {
                        self.splash(at, 2.0);
                        self.burst(arena, &profile, at, 3.5, p.damage * 1.8, 0, false, profile.element.or(Some(Element::Frost)), None);
                        self.wave_broken = step < 12;
                    }
                }
                Act::Gout(k) => {
                    let view = arena.view;
                    let forward = view.forward();
                    let element = profile.element.unwrap_or(Element::Fire);
                    let from = view.eye + view.rotation * Vector3::new(0.15, 0.9, -0.2) * vpm;
                    self.breathe(from, forward, 0.2, element);
                    if k == 0 {
                        self.cues.push(Cue::here(Sound::Breath, 1.0));
                    }
                    let caught: Vec<usize> = (0..arena.animals.len())
                        .filter(|&i| {
                            let a = &arena.animals[i];
                            let to = a.center(vpm as u32) - view.eye;
                            a.health > 0.0 && !a.look.is_pet() && to.norm() < (9.0 + a.look.body().1) * vpm && to.normalize().dot(&forward) > 0.8
                        })
                        .collect();
                    for i in caught {
                        self.strike(arena, &profile, i, Blow { damage: p.damage * 0.45, crit: false, heavy: false, from: view.eye, depth: k % 2, element: Some(element) });
                    }
                    // Leaving flames, venom or frost on the ground where it pours
                    if k % 2 == 1 {
                        let out = view.eye + forward * (4.0 + 3.0 * random(&mut self.rng)) * vpm;
                        let down = Vector3::new(0.0, 0.0, -6.0 * vpm);
                        let at = solid_along(arena.world, out, out + down).map_or(out + down, |t| out + down * t);
                        let lingers = if matches!(element, Element::Poison | Element::Frost) { element } else { Element::Fire };
                        self.lingering.push(Lingering { position: at, radius: 1.5, life: 3.0, element: lingers, profile: profile.clone(), damage: p.damage * 0.1, tick: 0.0, effects: false, collapse: 0.0 });
                    }
                }
            }
        }
    }

    /// A blow at the player at `feet` from `from` turned aside, if they're standing guard, and
    /// answered: a dash back at it and three quick slashes
    pub fn parry(&mut self, from: Vector3<f64>, feet: Vector3<f64>) -> bool {
        let Some(guard) = self.guard.take() else {
            return false;
        };
        let vpm = self.vpm;
        let towards = Vector3::new(from.x - feet.x, from.y - feet.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros);
        self.deflect(feet + Vector3::new(0.0, 0.0, 1.3 * vpm) + towards * 0.8 * vpm, &guard.profile);
        self.pending.push(Pending { delay: 0.08, act: Act::Dash(towards * 14.0 * vpm), profile: guard.profile.clone(), damage: 0.0 });
        for delay in [0.12, 0.22, 0.32] {
            self.pending.push(Pending { delay, act: Act::Slash, profile: guard.profile.clone(), damage: guard.damage * 1.6 });
        }
        self.action = Action::Special { t: 0.0, length: 0.45, special: Special::Flurry };
        true
    }

    /// Something turned aside at `at`: sparks flying, a ring of steel and the swing stopping dead
    fn deflect(&mut self, at: Vector3<f64>, profile: &Profile) {
        self.hitstop = 0.15;
        self.shake = self.shake.max(0.5);
        self.sparks(at, 24, 5.0, Element::look(profile.element), 0.6, 0.4);
        self.flashes.push(Flash { position: at, color: [1.0, 0.95, 0.8], reach: 6.0, life: 0.25, lifetime: 0.25 });
        self.cues.push(Cue::here(Sound::Hit, 1.0).pitched(1.5));
        self.cues.push(Cue::here(Sound::ArrowHit, 0.8).pitched(0.7));
        self.messages.push("Parried!".into());
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
    /// `finisher` is the last blow of a combo.
    fn melee(&mut self, arena: &mut Arena, profile: &Rc<Profile>, heavy: bool, power: f64, finisher: bool) {
        if profile.style == Style::Lash {
            self.lash(arena, profile, heavy, power, finisher);
            return;
        }
        let view = arena.view;
        let vpm = view.vpm;
        let reach = (profile.stat("range") + if heavy || finisher { 0.3 } else { 0.0 }) * vpm;
        // Heavy blows come down on less, but cleave through one more. A spear's finishing sweep
        // takes in a wide arc.
        let sweep = finisher && profile.style == Style::Thrust;
        let cone = if sweep { profile.cone - 0.45 } else { (profile.cone + if heavy { 0.15 } else { 0.0 }).min(0.95) };
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
        targets.truncate(profile.targets + heavy as usize + finisher as usize + 2 * sweep as usize);
        let thud = match profile.style {
            Style::Smash => 0.7,
            Style::Chop | Style::Reap => 0.85,
            Style::Stab => 1.25,
            Style::Thrust => 1.1,
            _ => 1.0,
        };
        let mut any_crit = false;
        for (k, &(_, i)) in targets.iter().enumerate() {
            let crit = self.roll_crit(profile);
            any_crit |= crit;
            let damage = base * if crit { profile.stat("critDamage") } else { 1.0 };
            let center = arena.animals[i].center(vpm as u32);
            let at = center + (view.eye - center).normalize() * arena.animals[i].look.body().1 * vpm;
            if k == 0 {
                self.cues.push(Cue::at(Sound::Hit, at, if heavy || finisher { 1.0 } else { 0.8 }).pitched(thud));
            }
            let strength = 1.0 + 0.5 * (crit as u8 + heavy as u8 + finisher as u8) as f64;
            self.sparks(at, (6.0 * strength) as usize, 3.0, Element::look(None), 0.8, 0.35);
            self.element_hit(at, profile.element, strength);
            self.strike(arena, profile, i, Blow { damage, crit, heavy, from: view.eye, depth: 0, element: profile.element });
        }
        // The blade stops dead a moment as it bites, jolts back, and a big one shakes the view
        if !targets.is_empty() && power >= 1.0 {
            let big = heavy || finisher || any_crit;
            self.hitstop = if big { 0.085 } else { 0.045 };
            self.recoil = 1.0;
            self.shake = self.shake.max(if big { 0.6 } else { 0.25 });
        }
        // A heavy hammer blow dents the ground in front, knocking back what's around it, and a
        // finishing one a little
        let flat = Vector3::new(forward.x, forward.y, 0.0);
        if power >= 1.0 && (heavy || finisher) && profile.style == Style::Smash && flat.norm() > 0.2 {
            let ahead = view.eye + flat.normalize() * reach * 0.7;
            let down = Vector3::new(0.0, 0.0, -2.5 * vpm);
            if let Some(t) = solid_along(arena.world, ahead, ahead + down) {
                let at = ahead + down * t;
                self.craters.push((at, if heavy { 0.45 } else { 0.3 }));
                self.shake = self.shake.max(0.8);
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

    /// A whip cracking, for melee(). What it hits grows with its grade, see LASH_LINE and the rest:
    /// the first thing along the line it cracks down, everything along it, a wide fan, or all
    /// round. A double crack's second crack comes back through here as an echo, with `power` under
    /// 1 as for other echoes.
    fn lash(&mut self, arena: &mut Arena, profile: &Rc<Profile>, heavy: bool, power: f64, finisher: bool) {
        let view = arena.view;
        let vpm = view.vpm;
        let grade = profile.grade;
        let forward = view.forward();
        let reach = profile.stat("range").max(1.0) * vpm;
        let snare = heavy && grade >= LASH_SNARE;
        let circle = finisher && grade >= LASH_CIRCLE;
        let fan = !snare && !circle && grade >= LASH_FAN;
        // The lash in hand ends where its tip was last frame, as it trails, and reaches on from there
        let tip = self.trail.unwrap_or_else(|| view.eye + view.rotation * Vector3::new(0.12, 1.5, -0.15) * vpm);
        // How far out from the line it cracks along it still hits, a little near and more further on
        let spread = (1.0 - profile.cone * profile.cone).max(0.0).sqrt() / profile.cone.max(0.1);
        let mut hits: Vec<(f64, usize)> = arena
            .animals
            .iter()
            .enumerate()
            .filter(|(_, a)| a.health > 0.0 && !a.look.is_pet())
            .filter_map(|(i, a)| {
                let to = a.center(vpm as u32) - view.eye;
                let body = a.look.body().1 * vpm;
                let d = to.norm();
                let ahead = to.dot(&forward);
                let inside = if circle {
                    true
                } else if fan {
                    ahead > LASH_FAN_CONE * d || d < body * 1.5
                } else {
                    ahead > 0.0 && (to - forward * ahead).norm() < 0.3 * vpm + ahead * spread + body
                };
                (d < reach + body && inside).then_some((d, i))
            })
            .collect();
        // It reaches far, but not through walls
        hits.retain(|&(_, i)| solid_along(arena.world, view.eye, arena.animals[i].center(vpm as u32)).is_none_or(|t| t > 0.85));
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        let most = if snare || grade < LASH_LINE {
            1
        } else if circle {
            profile.targets * 2
        } else {
            profile.targets + heavy as usize + finisher as usize
        };
        hits.truncate(most);
        // Where it cracks: at what it caught along the line, or out at full stretch
        let stretch = view.eye + forward * reach;
        let stretch = solid_along(arena.world, view.eye, stretch).map_or(stretch, |t| view.eye + forward * reach * t);
        let end = match hits.last() {
            Some(&(_, i)) if !fan && !circle => arena.animals[i].center(vpm as u32),
            _ => stretch,
        };

        let spreading = if fan || circle { LASH_SPREAD_POWER } else { 1.0 };
        let base = (profile.stat("physicalDamage") + profile.stat("elementalDamage")) * if heavy { HEAVY } else { 1.0 } * power * spreading;
        let look = Element::look(profile.element);
        let mut any_crit = false;
        for (k, &(d, i)) in hits.iter().enumerate() {
            let crit = self.roll_crit(profile);
            any_crit |= crit;
            let at_tip = !circle && d > reach * (1.0 - LASH_TIP_REACH);
            let damage = base * if at_tip { LASH_TIP } else { 1.0 } * if crit { profile.stat("critDamage") } else { 1.0 };
            let center = arena.animals[i].center(vpm as u32);
            let at = center + (view.eye - center).normalize() * arena.animals[i].look.body().1 * vpm;
            if k == 0 {
                self.cues.push(Cue::at(Sound::Hit, at, if heavy || finisher { 0.9 } else { 0.7 }).pitched(1.2));
            }
            let strength = 1.0 + 0.5 * (crit as u8 + heavy as u8 + finisher as u8 + at_tip as u8) as f64;
            self.sparks(at, (5.0 * strength) as usize, 3.0, Element::look(None), 0.8, 0.3);
            self.element_hit(at, profile.element, strength);
            self.strike(arena, profile, i, Blow { damage, crit, heavy, from: view.eye, depth: 0, element: profile.element });
            // Snared: yanked in towards the player and off its feet a moment. After strike(), so
            // no knockback throws it the other way.
            if snare && arena.animals[i].health > 0.0 {
                let a = &mut arena.animals[i];
                let mut toward = view.eye - a.position;
                toward.z = 0.0;
                let pull = (toward.norm() / vpm - LASH_SNARE_CLOSEST).clamp(0.0, LASH_SNARE_PULL);
                if let Some(toward) = toward.try_normalize(1e-6) {
                    a.status.push = toward * pull * PUSH_DECAY * vpm;
                }
                a.status.staggered = a.status.staggered.max(LASH_SNARE_STAGGER);
                self.cues.push(Cue::at(Sound::Twang, at, 0.8).pitched(0.7));
                self.say_at(center, "Snared!", [235, 205, 150], 18.0);
            }
        }

        // The crack: sparks off the tip of the lash in hand, a flash where it cracks, and the lash
        // flicking out there, sweeping a fan, or going right round
        for _ in 0..if circle { 20 } else { 8 } {
            let v = self.any_direction() * (1.0 + random(&mut self.rng)) * vpm;
            self.fine(tip, v, look, 0.16, 0.3);
        }
        self.flashes.push(Flash { position: end, color: profile.element.map_or([1.0, 0.92, 0.75], Element::light), reach: 2.5, life: 0.07, lifetime: 0.07 });
        self.cues.push(Cue::at(Sound::Crack, tip, if heavy || finisher { 0.9 } else { 0.7 }).pitched(1.55 + 0.2 * random(&mut self.rng) as f32));
        if circle {
            let flat = Vector3::new(forward.x, forward.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
            for k in 0..28 {
                let out = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), k as f64 / 28.0 * TAU) * flat;
                let at = view.eye + out * reach * 0.9 - Vector3::new(0.0, 0.0, 0.4 * vpm);
                let life = 0.25 * (0.7 + 0.3 * random(&mut self.rng));
                self.sparks.push(Spark { position: at, velocity: Vector3::new(-out.y, out.x, 0.0) * 4.0 * vpm, look, life, lifetime: life, gravity: 0.0, slot: SLOT_SPARK });
            }
            self.sparks(end, 16, 4.0, look, 0.5, 0.4);
            self.cues.push(Cue::here(Sound::HeavySwing, 0.8).pitched(1.3));
            self.shake = self.shake.max(0.5);
        } else if fan {
            // An arc out at full stretch, swept from the right across to the left
            let up = view.rotation * Vector3::z();
            for k in 0..18 {
                let a = (k as f64 / 17.0 * 2.0 - 1.0) * LASH_FAN_CONE.acos();
                let out = UnitQuaternion::from_scaled_axis(up * a) * forward;
                let life = 0.18 * (0.7 + 0.3 * random(&mut self.rng));
                self.sparks.push(Spark { position: view.eye + out * reach * 0.95, velocity: up.cross(&out) * 3.0 * vpm, look, life, lifetime: life, gravity: 0.0, slot: SLOT_SPARK });
            }
            self.lash_line(tip, stretch, look);
        } else {
            self.lash_line(tip, end, look);
        }
        self.cap_sparks();

        if !hits.is_empty() && power >= 1.0 {
            let big = heavy || finisher || any_crit;
            self.hitstop = if big { 0.085 } else { 0.045 };
            self.recoil = 1.0;
            self.shake = self.shake.max(if big { 0.5 } else { 0.2 });
        }
        // A double crack: flicked up and cracked again as the swing comes round, when lash_keys()
        // has it land. The swing stands still while it's stopped dead, so that's added on.
        if finisher && power >= 1.0 && grade >= LASH_DOUBLE {
            let length = if let Action::Swing { length, .. } = self.action { length } else { 0.6 };
            let (_, lands) = timing(Style::Lash, false, FINISHER);
            let delay = (LASH_SECOND - lands) * length + self.hitstop.max(0.0);
            self.echoes.push(Echo { delay, profile: profile.clone(), shot: None, heavy: false, power: LASH_SECOND_POWER });
        }
        if power >= 1.0 && profile.has(Effect::Echo) && random(&mut self.rng) < profile.param(Effect::Echo, "chance", 0.35) {
            let delay = profile.param(Effect::Echo, "delay", 0.25);
            let power = profile.param(Effect::Echo, "power", 0.6) * profile.potency(Effect::Echo).max(1.0);
            self.echoes.push(Echo { delay, profile: profile.clone(), shot: None, heavy, power });
            self.cues.push(Cue::here(Sound::Swing, 0.3));
            self.sparks(tip, 10, 2.5, Element::look(Some(Element::Arcane)), 0.0, 0.4);
        }
    }

    /// A whip's lash flicking out from `from` to `to`, there for a moment: a line of fine sparks
    /// close by, and sparks the size of the world's voxels further out, where fine ones would be
    /// too small to see
    fn lash_line(&mut self, from: Vector3<f64>, to: Vector3<f64>, look: usize) {
        let vpm = self.vpm;
        let steps = ((to - from).norm() / (0.15 * vpm)).ceil().clamp(2.0, 30.0) as usize;
        let drift = (to - from).try_normalize(1e-6).unwrap_or_else(Vector3::zeros) * 0.6 * vpm;
        for k in 0..=steps {
            let at = from.lerp(&to, k as f64 / steps as f64);
            if (at - from).norm() < 2.0 * vpm {
                self.fine(at, drift, look, 0.12, 0.0);
            } else {
                let life = 0.1 * (0.7 + 0.3 * random(&mut self.rng));
                self.sparks.push(Spark { position: at, velocity: drift, look, life, lifetime: life, gravity: 0.0, slot: SLOT_SPARK });
            }
        }
        self.cap_sparks();
    }

    /// Lets an arrow fly from the bow in hand, drawn `draw` of the way
    fn loose(&mut self, arena: &mut Arena, weapon: &Weapon, draw: f64) {
        let profile = weapon.profile.clone();
        let vpm = arena.view.vpm;
        let from = self.held_point(weapon, arena.view, true);
        let target = self.aim(arena);
        // Drawn all the way, it flies a little harder, streaking
        let power = draw >= 0.98;
        let speed = profile.stat("projectileSpeed") * (0.45 + 0.55 * draw) * if power { 1.1 } else { 1.0 } * vpm;
        let damage = (profile.stat("physicalDamage") + profile.stat("elementalDamage")) * (0.3 + 0.7 * draw * draw) * if power { 1.1 } else { 1.0 };
        self.shoot(arena.view, &profile, Some((weapon.arrowhead(), power)), Form::Bolt, from, (target - from).normalize(), speed, damage);
        self.cues.push(Cue::here(Sound::Twang, 0.4 + 0.4 * draw as f32).pitched(if power { 1.1 } else { 1.0 }));
        if power {
            self.shake = self.shake.max(0.2);
        }
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
        let form = Form::of(profile.element);
        let ahead = target - from;
        let up = Vector3::new(0.0, 0.0, 1.0);
        let turn = |degrees: f64| UnitQuaternion::from_axis_angle(&(arena.view.rotation * Vector3::z_axis()), degrees.to_radians());
        match (profile.element, form) {
            // A ring of light marks where it's aimed, and light comes down on it a moment later
            (Some(Element::Holy), _) => {
                self.strikes.push(Strike { position: target, delay: 0.45, profile: profile.clone(), damage: damage * 1.2 });
                for k in 0..24 {
                    let a = k as f64 / 24.0 * TAU;
                    self.fine(target + Vector3::new(a.cos(), a.sin(), 0.05) * 1.2 * vpm, Vector3::new(0.0, 0.0, 0.3 * vpm), look, 0.5, 0.0);
                }
            }
            (_, Form::Shard) => {
                for degrees in [-5.0, 0.0, 5.0] {
                    self.shoot(arena.view, &profile, None, form, from, turn(degrees) * ahead.normalize(), speed, damage * 0.45);
                }
            }
            (_, Form::Missile) => {
                // One now, the others following, wavering apart
                for k in 0..3 {
                    let before = self.projectiles.len();
                    let wander = Vector3::new(random(&mut self.rng) - 0.5, random(&mut self.rng) - 0.5, random(&mut self.rng) * 0.5) * 0.25;
                    self.shoot(arena.view, &profile, None, form, from, (ahead.normalize() + wander).normalize(), speed, damage * 0.42);
                    if k > 0 {
                        let later: Vec<Projectile> = self.projectiles.drain(before..).collect();
                        for shot in later {
                            self.echoes.push(Echo { delay: 0.09 * k as f64, profile: profile.clone(), shot: Some(shot), heavy: false, power: 1.0 });
                        }
                    }
                }
            }
            (_, Form::Lightning) => {
                self.bolt_line(from, target, look);
                self.flashes.push(Flash { position: target, color: Element::Storm.light(), reach: 6.0, life: 0.15, lifetime: 0.15 });
                self.shoot(arena.view, &profile, None, form, from, ahead.normalize(), speed, damage);
            }
            // Lobbed up, to come down on it, the further the higher
            (_, Form::Glob) => self.shoot(arena.view, &profile, None, form, from, (ahead + up * ahead.norm() * 0.2).normalize(), speed, damage),
            (_, Form::Fireball) => self.shoot(arena.view, &profile, None, form, from, (ahead + up * ahead.norm() * 0.06).normalize(), speed, damage * 1.15),
            (_, Form::Lance) => self.shoot(arena.view, &profile, None, form, from, ahead.normalize(), speed, damage * 1.1),
            _ => self.shoot(arena.view, &profile, None, form, from, ahead.normalize(), speed, damage),
        }
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
    /// maybe again a moment later with echo. `arrow` is the arrowhead and whether the bow was drawn
    /// all the way, for arrows, None for spells, of `form`.
    #[allow(clippy::too_many_arguments)]
    fn shoot(&mut self, view: &View, profile: &Rc<Profile>, arrow: Option<(usize, bool)>, form: Form, from: Vector3<f64>, direction: Vector3<f64>, speed: f64, damage: f64) {
        let pierce = if profile.has(Effect::Pierce) { (profile.param(Effect::Pierce, "count", 2.0) * profile.potency(Effect::Pierce).max(1.0)).round() as u32 } else { 0 };
        let (head, power) = arrow.unwrap_or((0, false));
        let (pace, gravity, size, _) = if arrow.is_some() { (1.0, ARROW_GRAVITY, 1.0, 1.0) } else { form.flight() };
        let make = |direction: Vector3<f64>, damage: f64| Projectile {
            arrow: arrow.is_some(),
            position: from,
            velocity: direction * speed * pace,
            damage,
            profile: profile.clone(),
            pierce,
            hit: Vec::new(),
            life: if arrow.is_some() { 4.0 } else if form == Form::Orb { 6.0 } else { 3.0 },
            head,
            power,
            trail: 0.0,
            form,
            gravity,
            size,
            pin: 0.0,
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
            // Missiles seek out the nearest animal ahead
            if !p.arrow && p.form == Form::Missile {
                let direction = p.velocity.normalize();
                let target = arena
                    .animals
                    .iter()
                    .filter(|a| a.health > 0.0 && !a.look.is_pet() && !p.hit.contains(&a.id))
                    .map(|a| a.center(vpm as u32) - p.position)
                    .filter(|to| to.norm() < 25.0 * vpm && to.normalize().dot(&direction) > 0.3)
                    .min_by(|a, b| a.norm().total_cmp(&b.norm()));
                if let Some(to) = target {
                    p.velocity = direction.lerp(&to.normalize(), (5.0 * dt).min(1.0)).normalize() * p.velocity.norm();
                }
            }
            // A void orb drags in what it passes
            if !p.arrow && p.form == Form::Orb {
                for animal in arena.animals.iter_mut().filter(|a| a.health > 0.0 && !a.look.is_pet()) {
                    let to = p.position - animal.center(vpm as u32);
                    if to.norm() < 3.5 * vpm {
                        animal.status.push = to.normalize() * 3.0 * vpm;
                    }
                }
            }
            p.velocity.z -= 9.8 * p.gravity * vpm * dt;
            let (start, end) = (p.position, p.position + p.velocity * dt);

            // Spells and arrows with an element leave it trailing behind them, and arrows shot at
            // full draw a streak. A fireball sheds more, venom and blood drip.
            if (!p.arrow && p.form != Form::Lightning) || profile.element.is_some() || p.power {
                p.trail += dt * if !p.arrow && p.form == Form::Fireball { 150.0 } else { 80.0 };
                while p.trail >= 1.0 {
                    p.trail -= 1.0;
                    let at = start.lerp(&end, random(&mut self.rng));
                    if profile.element.is_none() && p.arrow {
                        self.fine(at, Vector3::zeros(), 0, 0.15, 0.0);
                    } else {
                        self.mote(at, profile.element, if p.form == Form::Fireball && !p.arrow { 2.0 } else { 1.6 });
                    }
                    let drip = match profile.element {
                        Some(Element::Poison) => Some(DROP_VENOM),
                        Some(Element::Blood) => Some(DROP_BLOOD),
                        _ => None,
                    };
                    if let Some(kind) = drip.filter(|_| random(&mut self.rng) < 0.3) {
                        self.drop(at, Vector3::zeros(), kind, 1.0);
                    }
                }
            }

            let mut gone = false;
            let wall = solid_along(arena.world, start, end).unwrap_or(f64::MAX);
            // Splashing into water: a spell goes off on it, an arrow carries on, slowed
            let (high, low) = ((arena.water)(start.x, start.y), (arena.water)(end.x, end.y));
            if start.z > high && end.z <= low && low.is_finite() {
                let t = ((start.z - low) / (start.z - end.z)).clamp(0.0, 1.0);
                if t < wall {
                    let at = start + (end - start) * t;
                    self.splash(at, if p.arrow { 0.6 } else { 1.0 });
                    if !p.arrow {
                        self.impact(arena, &p, at);
                        continue;
                    }
                    p.velocity *= 0.3;
                }
            }
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
                self.sparks(at, if crit { 10 } else { 6 }, 3.0, Element::look(None), 0.8, 0.35);
                self.element_hit(at, profile.element, if crit || p.power { 1.5 } else { 1.0 });
                self.cues.push(Cue::at(Sound::Hit, at, 0.7));
                p.hit.push(arena.animals[i].id);
                self.strike(arena, &profile, i, Blow { damage, crit, heavy: false, from: start, depth: 0, element: profile.element });
                // Pinned where it stands
                if p.pin > 0.0 {
                    let s = &mut arena.animals[i].status;
                    s.staggered = s.staggered.max(p.pin);
                    s.push = Vector3::zeros();
                }
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
                    self.stuck.push(Stuck {
                        position: at + direction * 0.12 * vpm,
                        rotation,
                        look: Element::look(profile.element),
                        head: Some(p.head),
                        element: profile.element,
                        life: 20.0,
                        glowing: if profile.element.is_some() { 5.0 } else { 0.0 },
                    });
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
            // Spells go off where they land, each kind its own way. A void orb draws everything in
            // first.
            let (_, _, _, reach) = p.form.flight();
            if p.form == Form::Orb {
                self.burst(arena, profile, at, MAGIC_BLAST * reach * 1.5, 0.0, 1, true, Some(Element::Void), None);
            }
            self.burst(arena, profile, at, MAGIC_BLAST * reach, p.damage * 0.5, 1, false, profile.element.or(Some(Element::Arcane)), None);
        } else {
            self.sparks(at, 4, 1.5, look, 1.0, 0.3);
            if profile.element.is_some() {
                self.element_hit(at, profile.element, if p.power { 1.6 } else { 1.1 });
            }
            self.cues.push(Cue::at(Sound::ArrowHit, at, 0.8));
        }
    }

    /// Hurts everything within `radius` meters of `at`, less further out, with a flash: an explosion,
    /// or with `pull` an implosion drawing them in. Skips `skip`.
    #[allow(clippy::too_many_arguments)]
    fn burst(&mut self, arena: &mut Arena, profile: &Rc<Profile>, at: Vector3<f64>, radius: f64, damage: f64, depth: u32, pull: bool, element: Option<Element>, skip: Option<usize>) {
        let vpm = arena.view.vpm;
        let look = Element::look(element);
        // Bigger ones boom deeper
        let pitch = if pull { 1.3 } else { (1.25 - radius * 0.12).clamp(0.6, 1.2) as f32 };
        self.cues.push(Cue::at(Sound::Burst, at, if pull { 0.6 } else { (0.5 + radius * 0.25).min(1.2) as f32 }).pitched(pitch));
        self.flashes.push(Flash { position: at, color: element.map_or([1.0, 0.8, 0.5], Element::light), reach: radius * 3.0 + 4.0, life: 0.35, lifetime: 0.35 });
        if pull {
            self.sparks(at, (16.0 + radius * 12.0) as usize, -4.0, look, 0.4, 0.6 + radius * 0.1);
        } else {
            self.explode(at, element, radius, profile, damage);
            // Felt more the nearer and bigger it is
            let near = (1.0 - (at - arena.view.eye).norm() / vpm / 30.0).max(0.0);
            self.shake = self.shake.max(near * (0.3 + radius * 0.25));
        }
        let r = radius * vpm;
        // Measured to the nearest of its body, so a big one's caught by a blast at its feet
        let inside: Vec<(usize, f64)> = arena
            .animals
            .iter()
            .enumerate()
            .filter(|&(i, a)| Some(i) != skip && a.health > 0.0 && !a.look.is_pet())
            .map(|(i, a)| (i, ((a.center(vpm as u32) - at).norm() - a.look.body().1 * vpm).max(0.0)))
            .filter(|&(_, d)| d < r)
            .collect();
        for (i, d) in inside {
            let falloff = 1.0 - 0.5 * d / r;
            if pull {
                let towards = at - arena.animals[i].position;
                arena.animals[i].status.push = towards.normalize() * 5.0 * vpm;
            }
            if damage > 0.0 {
                self.strike(arena, profile, i, Blow { damage: damage * falloff, crit: false, heavy: false, from: at, depth, element });
            }
        }
    }

    /// What an explosion of `element` `radius` meters round looks like and leaves behind: each
    /// goes off its own way, and most mark the ground. Fire and venom linger, hurting what's in
    /// them for `damage` a tick.
    fn explode(&mut self, at: Vector3<f64>, element: Option<Element>, radius: f64, profile: &Rc<Profile>, damage: f64) {
        let vpm = self.vpm;
        let look = Element::look(element);
        let count = |k: f64| (k * (0.6 + radius * 0.4)) as usize;
        // How big a hole it blows, next to its size
        let hole = match element {
            Some(Element::Frost) => 0.35,
            Some(Element::Storm) => 0.5,
            Some(Element::Poison) => 0.2,
            Some(Element::Arcane) => 0.55,
            Some(Element::Void) => 0.9,
            Some(Element::Blood) => 0.25,
            Some(Element::Holy) => 0.4,
            _ => 0.75,
        };
        self.craters.push((at, radius * hole));
        match element {
            // A fireball, embers flying up, flames running out along the ground, a scorch mark and
            // fire left burning
            Some(Element::Fire) | None => {
                self.blasts.push(Blast { position: at, look, radius, life: 0.4, lifetime: 0.4 });
                self.sparks(at, count(20.0), 4.0 + radius * 2.5, look, -0.2, 0.8);
                for k in 0..count(16.0) {
                    let a = k as f64 / count(16.0) as f64 * TAU;
                    self.sparks.push(Spark { position: at, velocity: Vector3::new(a.cos(), a.sin(), 0.1) * (3.0 + radius) * vpm, look, life: 0.5, lifetime: 0.5, gravity: 0.0, slot: SLOT_SPARK });
                }
                self.stains.push((at, radius * 0.9, BARK_DARK, true));
                if element.is_some() {
                    self.lingering.push(Lingering { position: at, radius: radius * 0.6, life: 3.0, element: Element::Fire, profile: profile.clone(), damage: damage * 0.15, tick: 0.0, effects: false, collapse: 0.0 });
                }
                for k in 0..count(14.0) {
                    let v = self.any_direction() + Vector3::new(0.0, 0.0, 1.2);
                    self.drop(at, v * 3.0 * vpm, if k % 3 == 0 { DROP_EMBER } else { DROP_ASH }, 1.2);
                }
            }
            // Spikes of ice bursting up, shards falling, frost spreading over the ground
            Some(Element::Frost) => {
                self.swells.push(Swell { model: ICE_BURST, position: at, radius, life: 0.9, lifetime: 0.9 });
                self.sparks(at, count(16.0), 3.5, look, 1.6, 0.7);
                self.element_hit(at, element, 1.0 + radius * 0.4);
                for _ in 0..count(12.0) {
                    let v = self.any_direction() + Vector3::new(0.0, 0.0, 1.0);
                    self.drop(at, v * 3.5 * vpm, DROP_ICE, 1.0);
                }
                self.stains.push((at, radius * 1.1, SNOW, false));
            }
            // Lightning striking down onto it, a flash and a crackling burst
            Some(Element::Storm) => {
                self.element_hit(at, element, 2.0);
                self.blasts.push(Blast { position: at, look, radius: radius * 0.5, life: 0.25, lifetime: 0.25 });
                self.sparks(at, count(18.0), 6.0, look, 0.0, 0.25);
                self.stains.push((at, radius * 0.4, BARK_DARK, true));
            }
            // A splash of venom thrown up and out, a pool of it, and a poisonous cloud
            Some(Element::Poison) => {
                for _ in 0..count(28.0) {
                    let v = self.any_direction() * 0.8 + Vector3::new(0.0, 0.0, 0.9 + random(&mut self.rng));
                    self.drop(at, v * (2.5 + radius) * vpm, DROP_VENOM, 1.4);
                }
                self.element_hit(at, element, 1.5);
                self.stains.push((at, radius, MOSS, false));
                self.lingering.push(Lingering { position: at, radius: radius * 0.9, life: 4.0, element: Element::Poison, profile: profile.clone(), damage: damage * 0.12, tick: 0.0, effects: false, collapse: 0.0 });
            }
            // Rings of runes turning, and a burst of arcane light
            Some(Element::Arcane) => {
                self.element_hit(at, element, 1.5 + radius * 0.4);
                self.element_hit(at + Vector3::new(0.0, 0.0, 0.5 * vpm), element, 1.0);
                self.blasts.push(Blast { position: at, look, radius: radius * 0.7, life: 0.35, lifetime: 0.35 });
                self.sparks(at, count(16.0), 3.0, look, -0.1, 0.6);
            }
            // A sphere of darkness swelling and collapsing, leaving the ground blackened
            Some(Element::Void) => {
                self.blasts.push(Blast { position: at, look, radius: radius * 1.1, life: 0.6, lifetime: 0.6 });
                self.sparks(at, count(20.0), -3.5, look, 0.0, 0.5);
                for _ in 0..count(10.0) {
                    let v = self.any_direction() + Vector3::new(0.0, 0.0, 0.8);
                    self.drop(at, v * 3.0 * vpm, DROP_VOID, 1.0);
                }
                self.stains.push((at, radius * 0.8, OBSIDIAN, true));
            }
            // Blood sprayed everywhere, a red mist, and the ground stained
            Some(Element::Blood) => {
                for _ in 0..count(36.0) {
                    let v = self.any_direction() + Vector3::new(0.0, 0.0, 0.7);
                    let speed = 2.0 + radius * 1.5 * random(&mut self.rng);
                    self.drop(at, v * speed * vpm, DROP_BLOOD, 1.3);
                }
                self.sparks(at, count(12.0), 1.0, look, 0.1, 1.0);
                self.stains.push((at, radius * 0.9, CLOTH, false));
            }
            // A pillar of light, a ring of gold, and a gleaming burst
            Some(Element::Holy) => {
                self.element_hit(at, element, 2.5);
                self.blasts.push(Blast { position: at, look, radius: radius * 0.6, life: 0.4, lifetime: 0.4 });
                for k in 0..count(20.0) {
                    let a = k as f64 / count(20.0) as f64 * TAU;
                    self.sparks.push(Spark { position: at, velocity: Vector3::new(a.cos(), a.sin(), 0.2) * (2.5 + radius) * vpm, look, life: 0.6, lifetime: 0.6, gravity: 0.0, slot: SLOT_SPARK });
                }
                for _ in 0..count(8.0) {
                    let v = self.any_direction() + Vector3::new(0.0, 0.0, 1.5);
                    self.drop(at, v * 2.5 * vpm, DROP_GOLD, 1.0);
                }
            }
        }
        self.cap_sparks();
    }

    /// Fire burning and venom hanging where spells went off, hurting what's in them; light coming
    /// down where holy spells were aimed; ice melting away
    fn linger(&mut self, arena: &mut Arena, dt: f64) {
        let vpm = arena.view.vpm;
        for mut l in std::mem::take(&mut self.lingering) {
            l.life -= dt;
            l.tick -= dt;
            // Flames flickering up, or bubbles and a haze of venom
            let mut owed = dt * 25.0 * l.radius;
            while owed > 0.0 {
                if random(&mut self.rng) < owed {
                    let a = random(&mut self.rng) * TAU;
                    let r = random(&mut self.rng).sqrt() * l.radius * vpm;
                    let at = l.position + Vector3::new(a.cos() * r, a.sin() * r, 0.2 * vpm);
                    self.mote(at, Some(l.element), 2.0);
                }
                owed -= 1.0;
            }
            // The void draws everything round it in
            if l.element == Element::Void {
                for animal in arena.animals.iter_mut().filter(|a| a.health > 0.0 && !a.look.is_pet()) {
                    let to = l.position - animal.position;
                    if to.xy().norm() < l.radius * 1.6 * vpm {
                        animal.status.push = Vector3::new(to.x, to.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros) * 4.0 * vpm;
                    }
                }
                if random(&mut self.rng) < dt * 60.0 {
                    let from = l.position + self.any_direction() * l.radius * vpm;
                    self.sparks.push(Spark { position: from, velocity: (l.position - from) * 2.0, look: Element::look(Some(Element::Void)), life: 0.5, lifetime: 0.5, gravity: 0.0, slot: SLOT_SPARK });
                }
            }
            if l.tick <= 0.0 {
                l.tick = 0.5;
                let inside: Vec<usize> = (0..arena.animals.len())
                    .filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet())
                    .filter(|&i| (arena.animals[i].position - l.position).xy().norm() < l.radius * vpm + arena.animals[i].look.body().1 * vpm)
                    .collect();
                let profile = l.profile.clone();
                let depth = if l.effects { 0 } else { 1 };
                for i in inside {
                    self.strike(arena, &profile, i, Blow { damage: l.damage, crit: false, heavy: false, from: l.position, depth, element: Some(l.element) });
                }
            }
            if l.life > 0.0 {
                self.lingering.push(l);
            } else if l.collapse > 0.0 {
                let profile = l.profile.clone();
                self.burst(arena, &profile, l.position, l.radius * 0.8, l.collapse, 0, false, Some(l.element), None);
            }
        }
        for mut s in std::mem::take(&mut self.strikes) {
            s.delay -= dt;
            if s.delay > 0.0 {
                // Motes rising from the ring as it gathers
                if random(&mut self.rng) < dt * 40.0 {
                    let a = random(&mut self.rng) * TAU;
                    let at = s.position + Vector3::new(a.cos(), a.sin(), 0.0) * 1.2 * vpm;
                    self.fine(at, Vector3::new(0.0, 0.0, 1.5 * vpm), Element::look(Some(Element::Holy)), 0.5, 0.0);
                }
                self.strikes.push(s);
                continue;
            }
            // A pillar of light comes down on it
            let holy = Element::look(Some(Element::Holy));
            self.swells.push(Swell { model: PILLAR, position: s.position, radius: 1.0, life: 0.7, lifetime: 0.7 });
            for k in 0..40 {
                let a = random(&mut self.rng) * TAU;
                let r = random(&mut self.rng).sqrt() * 0.35 * vpm;
                let at = s.position + Vector3::new(a.cos() * r, a.sin() * r, k as f64 * 0.15 * vpm);
                let life = 0.35 + 0.25 * random(&mut self.rng);
                self.sparks.push(Spark { position: at, velocity: Vector3::new(0.0, 0.0, -2.0 * vpm), look: holy, life, lifetime: life, gravity: 0.0, slot: SLOT_SPARK });
            }
            self.element_hit(s.position, Some(Element::Holy), 3.0);
            self.cues.push(Cue::at(Sound::Cast(holy as u8), s.position, 1.0).pitched(0.7));
            let profile = s.profile.clone();
            self.burst(arena, &profile, s.position, MAGIC_BLAST * 1.3, s.damage * 0.8, 1, false, Some(Element::Holy), None);
        }
        for i in &mut self.swells {
            i.life -= dt;
        }
        self.swells.retain(|i| i.life > 0.0);
    }

    /// A drop of something splashed, `kind` one of weapons::DROP_WATER and the rest, flying off at
    /// `velocity` voxels a second until it lands
    fn drop(&mut self, at: Vector3<f64>, velocity: Vector3<f64>, kind: usize, lifetime: f64) {
        let life = lifetime * (0.6 + 0.4 * random(&mut self.rng));
        self.sparks.push(Spark { position: at, velocity, look: kind, life, lifetime: life, gravity: 1.0, slot: SLOT_DROP });
        self.cap_sparks();
    }

    /// Something hitting water at `at`, `size` 1 for a spell: a splash of drops thrown up, and a
    /// ring of spray
    fn splash(&mut self, at: Vector3<f64>, size: f64) {
        let vpm = self.vpm;
        // A column of it thrown straight up, and a crown of it out round that
        for _ in 0..(40.0 * size) as usize {
            let out = self.any_direction();
            let v = Vector3::new(out.x * 0.35, out.y * 0.35, 0.9 + 0.5 * random(&mut self.rng)) * (3.0 + 3.5 * random(&mut self.rng)) * size.sqrt();
            self.drop(at + Vector3::new(0.0, 0.0, 0.1 * vpm), v * vpm, DROP_WATER, 1.4);
        }
        let ring = (26.0 * size) as usize;
        for k in 0..ring {
            let a = k as f64 / ring as f64 * TAU;
            self.drop(at + Vector3::new(0.0, 0.0, 0.1 * vpm), Vector3::new(a.cos() * 2.2, a.sin() * 2.2, 1.6) * vpm * size.sqrt(), DROP_WATER, 0.6);
        }
        self.cues.push(Cue::at(Sound::Splash, at, (0.5 + 0.4 * size) as f32));
    }

    /// Lightning from `from` to `to`, jagged, there for a moment
    fn bolt_line(&mut self, from: Vector3<f64>, to: Vector3<f64>, look: usize) {
        let vpm = self.vpm;
        let steps = ((to - from).norm() / (0.2 * vpm)).clamp(8.0, 120.0) as usize;
        let mut offset = Vector3::zeros();
        for k in 0..=steps {
            let f = k as f64 / steps as f64;
            // Wandering off and back, pinned at both ends
            offset = (offset + self.any_direction() * 0.12 * vpm) * (1.0 - f).min(f * 4.0).min(1.0);
            self.sparks.push(Spark { position: from.lerp(&to, f) + offset, velocity: Vector3::zeros(), look, life: 0.25, lifetime: 0.25, gravity: 0.0, slot: SLOT_SPARK });
        }
        self.cap_sparks();
        self.cues.push(Cue::at(Sound::Crackle, to, 0.8));
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
        // Every blow charges the special attack
        self.charge = (self.charge + damage / SPECIAL_CHARGE).min(1.0);
        arena.animals[i].status.shown = 4.0;
        // Flashing white, less for what follows on from a blow
        let s = &mut arena.animals[i].status;
        s.flash = s.flash.max(if blow.depth == 0 { 0.09 } else { 0.05 });
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
                self.sparks.push(Spark { position: at, velocity: Vector3::zeros(), look, life: 0.18, lifetime: 0.18, gravity: 0.0, slot: SLOT_SPARK });
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
            s.flash -= dt;
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
            self.sparks.push(Spark { position, velocity, look, life, lifetime: life, gravity, slot: SLOT_SPARK });
        }
        self.cap_sparks();
    }

    /// A fine spark at `at`, moving at `velocity` voxels a second
    fn fine(&mut self, at: Vector3<f64>, velocity: Vector3<f64>, look: usize, lifetime: f64, gravity: f64) {
        let life = lifetime * (0.6 + 0.4 * random(&mut self.rng));
        self.sparks.push(Spark { position: at, velocity, look, life, lifetime: life, gravity, slot: SLOT_FINE });
        self.cap_sparks();
    }

    fn cap_sparks(&mut self) {
        if self.sparks.len() > MOST_SPARKS {
            let extra = self.sparks.len() - MOST_SPARKS;
            self.sparks.drain(..extra);
        }
    }

    /// A direction picked at random
    fn any_direction(&mut self) -> Vector3<f64> {
        loop {
            let v = Vector3::new(random(&mut self.rng), random(&mut self.rng), random(&mut self.rng)) * 2.0 - Vector3::repeat(1.0);
            if v.norm() > 0.1 && v.norm() <= 1.0 {
                return v.normalize();
            }
        }
    }

    /// One spark of `element` drifting off from `at` as its kind does: embers rising off fire,
    /// frost settling, poison dripping, lightning crackling off, and so on. `size` scales how long
    /// it lasts, and above 1 it's a spark the size of a voxel of the world rather than a fine one,
    /// to be seen from further off.
    fn mote(&mut self, at: Vector3<f64>, element: Option<Element>, size: f64) {
        let vpm = self.vpm;
        let look = Element::look(element);
        let wander = self.any_direction();
        let (velocity, life, gravity) = match element {
            Some(Element::Fire) => (wander * 0.2 + Vector3::new(0.0, 0.0, 0.6), 0.5, -0.05),
            Some(Element::Frost) => (wander * 0.15, 0.7, 0.04),
            Some(Element::Storm) => (wander * 1.6, 0.12, 0.0),
            Some(Element::Poison) => (wander * 0.08, 0.6, 0.35),
            Some(Element::Arcane) => (wander * 0.35, 0.45, -0.02),
            Some(Element::Void) => (wander * -0.25, 0.4, 0.0),
            Some(Element::Blood) => (wander * 0.1, 0.5, 0.8),
            Some(Element::Holy) => (wander * 0.1 + Vector3::new(0.0, 0.0, 0.45), 0.6, -0.03),
            None => (wander * 0.2, 0.3, 0.0),
        };
        if size > 1.0 {
            let life = life * size;
            self.sparks.push(Spark { position: at, velocity: velocity * vpm, look, life, lifetime: life, gravity, slot: SLOT_SPARK });
            self.cap_sparks();
        } else {
            self.fine(at, velocity * vpm, look, life * size, gravity);
        }
    }

    /// What a hit with `element` looks like where it lands at `at`, `strength` 1 for an ordinary
    /// hit and more for a harder one
    fn element_hit(&mut self, at: Vector3<f64>, element: Option<Element>, strength: f64) {
        let vpm = self.vpm;
        let look = Element::look(element);
        let n = |k: f64| (k * strength).round().max(1.0) as usize;
        match element {
            None => {}
            // Embers flying up, and a puff of flame
            Some(Element::Fire) => {
                self.sparks(at, n(8.0), 2.0, look, -0.25, 0.7);
                self.blasts.push(Blast { position: at, look, radius: 0.3 * strength, life: 0.25, lifetime: 0.25 });
            }
            // Shards of ice falling, and frost spreading out round it
            Some(Element::Frost) => {
                self.sparks(at, n(8.0), 3.0, look, 1.6, 0.5);
                let count = n(14.0);
                for k in 0..count {
                    let a = k as f64 / count as f64 * TAU;
                    self.fine(at, Vector3::new(a.cos(), a.sin(), 0.08) * 1.8 * vpm, look, 0.45, 0.0);
                }
            }
            // Crackling, and on a hard hit lightning striking down from the sky onto it
            Some(Element::Storm) => {
                self.sparks(at, n(8.0), 4.5, look, 0.0, 0.2);
                if strength >= 1.4 {
                    let top = at + Vector3::new(0.0, 0.0, 10.0 * vpm);
                    let steps = 50;
                    for k in 0..=steps {
                        let f = k as f64 / steps as f64;
                        let jitter = Vector3::new(random(&mut self.rng) - 0.5, random(&mut self.rng) - 0.5, 0.0) * (1.0 - f) * 0.9 * vpm;
                        self.sparks.push(Spark { position: top.lerp(&at, f) + jitter, velocity: Vector3::zeros(), look, life: 0.28, lifetime: 0.28, gravity: 0.0, slot: SLOT_SPARK });
                    }
                    self.flashes.push(Flash { position: at, color: Element::Storm.light(), reach: 8.0, life: 0.18, lifetime: 0.18 });
                    self.cues.push(Cue::at(Sound::Crackle, at, 0.9).pitched(0.8));
                    self.shake = self.shake.max(0.3);
                }
            }
            // Bubbles rising slowly
            Some(Element::Poison) => {
                for _ in 0..n(12.0) {
                    let offset = self.any_direction() * 0.3 * vpm;
                    let rise = Vector3::new(0.0, 0.0, (0.25 + 0.3 * random(&mut self.rng)) * vpm);
                    self.fine(at + offset, rise, look, 1.1, -0.02);
                }
            }
            // A ring of runes turning round it
            Some(Element::Arcane) => {
                let count = n(14.0);
                for k in 0..count {
                    let a = k as f64 / count as f64 * TAU;
                    let out = Vector3::new(a.cos(), a.sin(), 0.0);
                    let round = Vector3::new(-a.sin(), a.cos(), 0.15);
                    self.fine(at + out * 0.45 * vpm, round * 1.4 * vpm, look, 0.6, 0.0);
                }
            }
            // Drawn in on itself
            Some(Element::Void) => self.sparks(at, n(12.0), -3.0, look, 0.0, 0.45),
            // Droplets splashing out and falling
            Some(Element::Blood) => self.sparks(at, n(10.0), 2.2, look, 1.8, 0.6),
            // A pillar of light rising
            Some(Element::Holy) => {
                for _ in 0..n(16.0) {
                    let a = random(&mut self.rng) * TAU;
                    let r = random(&mut self.rng).sqrt() * 0.25 * vpm;
                    let rise = Vector3::new(0.0, 0.0, (2.0 + 2.5 * random(&mut self.rng)) * vpm);
                    self.fine(at + Vector3::new(a.cos() * r, a.sin() * r, 0.0), rise, look, 0.7, -0.05);
                }
                self.flashes.push(Flash { position: at, color: Element::Holy.light(), reach: 4.0, life: 0.25, lifetime: 0.25 });
            }
        }
    }

    /// A monster killed, bursting apart its own way: a wolf bleeding, the dead falling to bones or
    /// gore, a spider bursting
    pub fn perish(&mut self, a: &Animal) {
        let vpm = self.vpm;
        let (height, size) = a.look.body();
        let at = a.position + Vector3::new(0.0, 0.0, height * vpm);
        let spray = |combat: &mut Combat, count: usize, kind: usize, speed: f64, life: f64| {
            for _ in 0..count {
                let v = combat.any_direction() + Vector3::new(0.0, 0.0, 1.2);
                combat.drop(at, v * speed * (0.6 + 0.4 * size) * vpm, kind, life);
            }
        };
        match a.look {
            Look::Wolf => {
                spray(self, 14, DROP_BLOOD, 3.0, 1.0);
                self.cues.push(Cue::at(Sound::Roar, at, 0.35).pitched(2.4));
            }
            // Bones clattering down, and the light going out of its eyes
            Look::Skeleton | Look::ArmoredSkeleton => {
                spray(self, 16, DROP_WATER, 4.0, 1.6);
                spray(self, 5, DROP_ASH, 2.5, 1.2);
                let head = a.position + Vector3::new(0.0, 0.0, 1.6 * vpm);
                for _ in 0..6 {
                    let v = self.any_direction() * 0.3 + Vector3::new(0.0, 0.0, 0.6);
                    self.fine(head, v * vpm, Element::look(Some(Element::Frost)), 0.8, -0.1);
                }
                self.cues.push(Cue::at(Sound::Rattle, at, 0.9).pitched(1.2));
                self.cues.push(Cue::at(Sound::Crack, at, 0.6).pitched(1.5));
            }
            Look::Zombie | Look::ArmoredZombie => {
                spray(self, 12, DROP_VENOM, 3.0, 1.2);
                spray(self, 8, DROP_BLOOD, 2.5, 1.2);
                self.cues.push(Cue::at(Sound::Squelch, at, 0.9).pitched(0.8));
            }
            _ => {
                spray(self, 10, DROP_ASH, 3.0, 1.0);
                spray(self, 8, DROP_VENOM, 3.0, 1.0);
                self.cues.push(Cue::at(Sound::Hiss, at, 0.6).pitched(1.8));
                self.cues.push(Cue::at(Sound::Crack, at, 0.5).pitched(1.8));
            }
        }
        // Armor ringing as it falls
        if matches!(a.look, Look::ArmoredSkeleton | Look::ArmoredZombie) {
            self.sparks(at, 8, 3.0, Element::look(None), 0.8, 0.4);
            self.cues.push(Cue::at(Sound::Clang, at, 0.6).pitched(0.7));
        }
        self.flashes.push(Flash { position: at, color: [1.0, 1.0, 1.0], reach: 4.0, life: 0.12, lifetime: 0.12 });
        self.cues.push(Cue::at(Sound::Hit, at, 0.8).pitched(0.6));
        self.shake = self.shake.max(0.2);
        self.cap_sparks();
    }

    /// A boss falling at `at`, `size` meters round: a burst of light and sparks
    pub fn fell(&mut self, at: Vector3<f64>, size: f64) {
        let holy = Element::look(Some(Element::Holy));
        self.sparks(at, 60, 5.0, holy, 0.6, 1.2);
        self.element_hit(at, Some(Element::Holy), 2.5);
        self.blasts.push(Blast { position: at, look: holy, radius: size * 1.6, life: 0.7, lifetime: 0.7 });
        self.flashes.push(Flash { position: at, color: [1.0, 0.9, 0.6], reach: 16.0, life: 0.7, lifetime: 0.7 });
        self.shake = self.shake.max(1.0);
    }

    /// Throws `kind` from `from` to land at `to` after `time` seconds: boulders and venom fall in
    /// an arc as things do, fire and the like fly straight. It hits for `damage`, its blast
    /// reaching `radius` meters.
    pub fn hurl(&mut self, kind: HostileKind, from: Vector3<f64>, to: Vector3<f64>, time: f64, damage: f64, radius: f64) {
        let gravity = kind.gravity() * 9.8 * self.vpm;
        let velocity = (to - from) / time + Vector3::new(0.0, 0.0, 0.5 * gravity * time);
        self.hostiles.push(Hostile { kind, position: from, velocity, life: time + 3.0, damage, radius, target: to, flight: time });
    }

    /// For this frame, a red ring on the ground `radius` meters round `at` showing where something's
    /// about to come down, and a second ring closing in on it, `closing` going from 1 down to 0 as
    /// it comes
    pub fn mark(&mut self, at: Vector3<f64>, radius: f64, closing: f64) {
        // Just over the grass, so it shows
        let at = at + Vector3::new(0.0, 0.0, 0.2 * self.vpm);
        self.marks.push((at, radius));
        if closing > 0.05 {
            self.marks.push((at, radius * closing.min(1.0)));
        }
    }

    /// `element` breathed out from `from` along `direction` for `dt` seconds: flame rising, frost
    /// and venom sinking as they spread
    pub fn breathe(&mut self, from: Vector3<f64>, direction: Vector3<f64>, dt: f64, element: Element) {
        let vpm = self.vpm;
        let look = Element::look(Some(element));
        let (gravity, spread) = match element {
            Element::Fire => (-0.12, 0.16),
            Element::Poison => (0.08, 0.22),
            _ => (0.04, 0.14),
        };
        let count = (dt * 110.0 + random(&mut self.rng)) as usize;
        for _ in 0..count {
            let spread = self.any_direction() * spread;
            let velocity = (direction + spread).normalize() * (8.0 + 4.0 * random(&mut self.rng)) * vpm;
            let life = 0.55 + 0.35 * random(&mut self.rng);
            self.sparks.push(Spark { position: from, velocity, look, life, lifetime: life, gravity, slot: SLOT_SPARK });
        }
        // Venom dripping and frost settling where it goes
        if element != Element::Fire && random(&mut self.rng) < dt * 30.0 {
            let v = (direction + self.any_direction() * 0.2) * (6.0 + 3.0 * random(&mut self.rng)) * vpm;
            self.drop(from, v, if element == Element::Poison { DROP_VENOM } else { DROP_ICE }, 1.2);
        }
        self.cap_sparks();
        if random(&mut self.rng) < dt * 20.0 {
            self.flashes.push(Flash { position: from + direction * 3.0 * vpm, color: element.light(), reach: 7.0, life: 0.12, lifetime: 0.12 });
        }
    }

    /// What a boss threw going off at `at`, and what it leaves on the ground
    fn land(&mut self, kind: HostileKind, at: Vector3<f64>) {
        let vpm = self.vpm;
        let look = Element::look(kind.element());
        match kind {
            HostileKind::Boulder(None) => {
                self.craters.push((at, 0.8));
                self.cues.push(Cue::at(Sound::Burst, at, 1.0).pitched(0.5));
                // Earth thrown up
                for _ in 0..18 {
                    let v = self.any_direction() + Vector3::new(0.0, 0.0, 1.3);
                    self.drop(at, v * 3.5 * vpm, DROP_MUD, 1.3);
                }
            }
            // Shattering into spikes and shards, frost spreading out round it
            HostileKind::Boulder(Some(Element::Frost)) => {
                self.craters.push((at, 0.5));
                self.swells.push(Swell { model: ICE_BURST, position: at, radius: 1.8, life: 1.2, lifetime: 1.2 });
                self.element_hit(at, Some(Element::Frost), 2.0);
                for _ in 0..22 {
                    let v = self.any_direction() + Vector3::new(0.0, 0.0, 1.2);
                    self.drop(at, v * 4.0 * vpm, DROP_ICE, 1.2);
                }
                self.stains.push((at, 3.0, SNOW, false));
                self.cues.push(Cue::at(Sound::Burst, at, 0.8).pitched(0.75));
                self.cues.push(Cue::at(Sound::Crack, at, 0.9).pitched(1.2));
            }
            // Bursting into flame, molten rock splashing out, the ground scorched
            HostileKind::Boulder(Some(_)) => {
                self.craters.push((at, 0.9));
                self.blasts.push(Blast { position: at, look, radius: 1.8, life: 0.5, lifetime: 0.5 });
                self.flashes.push(Flash { position: at, color: Element::Fire.light(), reach: 9.0, life: 0.4, lifetime: 0.4 });
                for k in 0..24 {
                    let v = self.any_direction() + Vector3::new(0.0, 0.0, 1.3);
                    self.drop(at, v * 4.0 * vpm, if k % 2 == 0 { DROP_EMBER } else { DROP_ASH }, 1.4);
                }
                self.stains.push((at, 2.6, BARK_DARK, true));
                self.cues.push(Cue::at(Sound::Burst, at, 1.0).pitched(0.45));
            }
            HostileKind::Spell(Element::Fire) => {
                self.craters.push((at, 0.7));
                self.element_hit(at, Some(Element::Fire), 2.0);
                self.blasts.push(Blast { position: at, look, radius: 1.3, life: 0.4, lifetime: 0.4 });
                self.flashes.push(Flash { position: at, color: Element::Fire.light(), reach: 8.0, life: 0.35, lifetime: 0.35 });
                self.stains.push((at, 1.8, BARK_DARK, true));
                self.cues.push(Cue::at(Sound::Burst, at, 1.0).pitched(0.85));
            }
            // A splash of venom and a pool of it
            HostileKind::Spell(Element::Poison) => {
                self.craters.push((at, 0.3));
                for _ in 0..30 {
                    let v = self.any_direction() * 0.8 + Vector3::new(0.0, 0.0, 0.9 + random(&mut self.rng));
                    self.drop(at, v * 3.0 * vpm, DROP_VENOM, 1.4);
                }
                self.element_hit(at, Some(Element::Poison), 1.8);
                self.stains.push((at, 2.2, MOSS, false));
                self.cues.push(Cue::at(Sound::Splash, at, 1.0).pitched(0.7));
            }
            HostileKind::Spell(Element::Frost) => {
                self.craters.push((at, 0.35));
                self.swells.push(Swell { model: ICE_BURST, position: at, radius: 1.2, life: 0.9, lifetime: 0.9 });
                self.element_hit(at, Some(Element::Frost), 1.6);
                self.stains.push((at, 2.2, SNOW, false));
                self.cues.push(Cue::at(Sound::Crack, at, 0.9).pitched(1.25));
            }
            HostileKind::Spell(e) => {
                self.craters.push((at, 0.5));
                self.element_hit(at, Some(e), 2.0);
                self.blasts.push(Blast { position: at, look, radius: 1.1, life: 0.35, lifetime: 0.35 });
                self.flashes.push(Flash { position: at, color: e.light(), reach: 7.0, life: 0.3, lifetime: 0.3 });
                self.cues.push(Cue::at(Sound::Burst, at, 1.0).pitched(0.9));
            }
            // A few splinters, and the thorn left standing in the ground
            HostileKind::Thorn => {
                self.sparks(at, 4, 1.5, look, 0.6, 0.4);
                self.cues.push(Cue::at(Sound::Stab, at, 0.6).pitched(0.8));
            }
            // Strands of web and venom splattered round, the ground left sticky
            HostileKind::Web => {
                for k in 0..24 {
                    let v = self.any_direction() * 0.9 + Vector3::new(0.0, 0.0, 0.6);
                    self.drop(at, v * 3.0 * vpm, if k % 3 == 0 { DROP_VENOM } else { DROP_WATER }, 1.0);
                }
                self.element_hit(at, Some(Element::Poison), 1.0);
                self.webs.push(at);
                self.cues.push(Cue::at(Sound::Web, at, 0.8));
            }
        }
        self.cap_sparks();
    }

    /// The ground stirring at `at`, `radius` meters round, for `dt` seconds, as before roots burst
    /// up out of it: drops of `kind`, one of weapons::DROP_MUD and the rest, jumping up out of it,
    /// most round its edge
    pub fn stir(&mut self, at: Vector3<f64>, radius: f64, dt: f64, kind: usize) {
        let vpm = self.vpm;
        let mut owed = dt * 40.0;
        while owed > 0.0 {
            if random(&mut self.rng) < owed {
                let a = random(&mut self.rng) * TAU;
                let r = if random(&mut self.rng) < 0.6 { radius } else { random(&mut self.rng).sqrt() * radius };
                let from = at + Vector3::new(a.cos() * r, a.sin() * r, 0.1) * vpm;
                let up = 2.0 + 1.5 * random(&mut self.rng);
                self.drop(from, Vector3::new(0.0, 0.0, up) * vpm, kind, 0.6);
            }
            owed -= 1.0;
        }
    }

    /// Roots bursting up out of the ground at `at`, `radius` meters round
    pub fn erupt(&mut self, at: Vector3<f64>, radius: f64) {
        let vpm = self.vpm;
        self.craters.push((at, 0.6));
        self.swells.push(Swell { model: ROOTS, position: at - Vector3::new(0.0, 0.0, 0.3 * vpm), radius: radius * 1.2, life: 1.4, lifetime: 1.4 });
        for _ in 0..26 {
            let v = self.any_direction() + Vector3::new(0.0, 0.0, 1.4);
            self.drop(at, v * 4.0 * vpm, DROP_MUD, 1.3);
        }
        self.stains.push((at, radius * 1.2, DIRT, false));
        self.cues.push(Cue::at(Sound::Burst, at, 0.7).pitched(0.4));
        self.cues.push(Cue::at(Sound::Crack, at, 0.8).pitched(0.8));
        self.cap_sparks();
    }

    /// Moves what bosses threw and spat, which goes off where it lands, hurting the player if
    /// they're near enough
    fn fly_hostiles(&mut self, arena: &mut Arena, dt: f64) {
        let vpm = arena.view.vpm;
        let hostiles = std::mem::take(&mut self.hostiles);
        for mut h in hostiles {
            // Turned back at what threw it, by a player standing guard
            let to = h.position - arena.view.eye;
            if self.guard.is_some() && to.norm() < 2.5 * vpm && to.dot(&arena.view.forward()) > 0.0 {
                let guard = self.guard.take().unwrap();
                self.deflect(h.position, &guard.profile);
                let back = -h.velocity.try_normalize(1e-6).unwrap_or_else(|| arena.view.forward());
                self.launch(&guard.profile, None, Form::of(h.kind.element()), h.position, back * 30.0 * vpm, h.damage * 1.5 + guard.damage, 0.0);
                continue;
            }
            h.velocity.z -= h.kind.gravity() * 9.8 * vpm * dt;
            // Fire and the like trailing off it, and frost off a ball of ice; dust off a rock and
            // a streak behind a thorn
            match h.kind.element() {
                Some(e) if random(&mut self.rng) < dt * 60.0 => self.mote(h.position, Some(e), 1.4),
                None if random(&mut self.rng) < dt * 40.0 => {
                    let back = -h.velocity * 0.05;
                    self.drop(h.position, back, if h.kind == HostileKind::Thorn { DROP_ASH } else { DROP_MUD }, 0.4);
                }
                _ => {}
            }
            // Where what falls is coming down, on the ground under where it was aimed: what's
            // lobbed, and what's coming steeply down out of the sky
            if h.kind.gravity() > 0.5 || h.velocity.z < -2.0 * h.velocity.xy().norm() {
                let (x, y) = (h.target.x.floor() as i64, h.target.y.floor() as i64);
                let top = h.target.z.floor() as i64 + vpm as i64;
                let ground = (top - 4 * vpm as i64..=top).rev().find(|&z| !is_passable(arena.world.material_at(x, y, z))).map_or(h.target.z, |z| z as f64 + 1.0);
                let left = (h.life - 3.0) / h.flight;
                self.mark(Vector3::new(h.target.x, h.target.y, ground), h.radius, left);
            }
            let (start, end) = (h.position, h.position + h.velocity * dt);
            h.life -= dt;
            // Straight into the player, or into the ground or water
            let body = arena.player.map(|feet| feet + Vector3::new(0.0, 0.0, 1.0 * vpm));
            let direct = body.and_then(|b| sphere_hit(start, end, b, 0.8 * vpm));
            let wall = solid_along(arena.world, start, end);
            let (high, low) = ((arena.water)(start.x, start.y), (arena.water)(end.x, end.y));
            let water = (start.z > high && end.z <= low && low.is_finite()).then(|| ((start.z - low) / (start.z - end.z)).clamp(0.0, 1.0));
            let landed = [direct, wall, water].into_iter().flatten().reduce(f64::min);
            let Some(t) = landed.or((h.life <= 0.0).then_some(1.0)) else {
                h.position = end;
                self.hostiles.push(h);
                continue;
            };
            let at = start + (end - start) * t;
            // A thorn only hurts if it hits; the rest go off, hurting what's near
            let hit = direct.is_some_and(|d| d <= t);
            if let Some(b) = body {
                let d = (b - at).norm() / vpm;
                if h.kind == HostileKind::Thorn {
                    if hit {
                        self.player_hits.push((h.damage, at, None));
                    } else if d < 1.5 {
                        self.near_misses.push(at);
                    }
                } else if d < h.radius {
                    self.player_hits.push((h.damage * (1.0 - 0.5 * d / h.radius), at, h.kind.element()));
                } else if d < h.radius + 1.2 {
                    self.near_misses.push(at);
                }
            }
            if h.kind != HostileKind::Thorn {
                let near = (1.0 - (at - arena.view.eye).norm() / vpm / 30.0).max(0.0);
                self.shake = self.shake.max(near * 0.5);
            }
            if water.is_some_and(|w| w <= t) && !hit {
                self.splash(at, if h.kind == HostileKind::Thorn { 0.5 } else { 1.4 });
                continue;
            }
            self.land(h.kind, at);
            if h.kind == HostileKind::Thorn && !hit && wall.is_some_and(|w| w <= t) {
                // Left standing in the ground
                let direction = h.velocity.normalize();
                let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &direction).unwrap_or_else(UnitQuaternion::identity);
                self.stuck.push(Stuck { position: at + direction * 0.12 * vpm, rotation, look: Element::look(None), head: None, element: None, life: 12.0, glowing: 0.0 });
                if self.stuck.len() > 40 {
                    self.stuck.remove(0);
                }
            }
        }
    }

    /// The weapon in hand leaving a trail off its tip as it comes down on something, and sparks of
    /// its element drifting off it
    fn flourish(&mut self, view: &View, weapon: &Weapon, dt: f64) {
        let vpm = view.vpm;
        let profile = &weapon.profile;
        let look = Element::look(profile.element);
        let swinging = match self.action {
            Action::Swing { t, length, heavy, combo, .. } => {
                let (comes, lands) = timing(profile.style, heavy, combo);
                (comes..lands + 0.1).contains(&(t / length))
            }
            Action::Cast { t, .. } => profile.style == Style::Flick && t < 0.2,
            Action::Special { .. } => weapon.kind() == Kind::Melee,
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
                // Out to the right, clear of the crosshair
                let rest = match style {
                    // Daggers up a little to be seen, spears levelled on ahead
                    Style::Stab => pose([0.28, 0.42, -0.23], [-0.08, 0.75, 0.65], [0.0, 1.0, 0.0]),
                    Style::Thrust => pose([0.34, 0.3, -0.33], [0.05, 0.97, 0.2], [-1.0, 0.0, 0.0]),
                    // A scythe's pole leant across, its blade out to the left at the top
                    Style::Reap => pose([0.31, 0.4, -0.4], [-0.2, 0.45, 0.86], [-1.0, 0.0, 0.25]),
                    // A whip held out low, its lash hanging on ahead and down in a loose S
                    Style::Lash => pose([0.3, 0.45, -0.2], [-0.08, 0.97, -0.2], [0.5, 0.15, -0.85]),
                    _ => pose([0.3, 0.46, -0.27], [-0.05, 0.62, 0.78], [0.0, 1.0, 0.0]),
                };
                match self.action {
                    Action::Swing { t, length, heavy, combo, .. } if style == Style::Lash => animate(&rest, &lash_keys(heavy, combo, weapon.grade), t / length),
                    Action::Swing { t, length, heavy, combo, .. } => animate(&rest, &swing(style, heavy, combo), t / length),
                    Action::Special { t, length, special } => animate(&rest, &special_move(special, Kind::Melee, style).1, t / length),
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
                    Action::Special { t, length, special } => animate(&rest, &special_move(special, Kind::Ranged, style).1, t / length),
                    _ => rest,
                }
            }
            Kind::Magic if style == Style::Flick => {
                // A wand drawn back and flicked at the target
                let rest = pose([0.29, 0.44, -0.24], [-0.04, 0.55, 0.83], [1.0, 0.0, 0.0]);
                let keys = [
                    (0.1, pose([0.24, 0.4, -0.19], [0.0, 0.1, 1.0], [1.0, 0.0, 0.0]), false),
                    (0.24, pose([0.16, 0.62, -0.2], [-0.12, 0.95, 0.25], [1.0, 0.0, 0.0]), true),
                    (0.4, pose([0.16, 0.62, -0.21], [-0.12, 0.93, 0.2], [1.0, 0.0, 0.0]), false),
                ];
                match self.action {
                    Action::Cast { t, .. } => animate(&rest, &keys, t / CAST_TIME),
                    Action::Special { t, length, special } => animate(&rest, &special_move(special, Kind::Magic, style).1, t / length),
                    _ => rest,
                }
            }
            Kind::Magic => {
                let rest = pose([0.31, 0.45, -0.3], [-0.02, 0.5, 0.86], [1.0, 0.0, 0.0]);
                // The spell leaves the staff at 0.1 seconds in
                let release = 0.1 / CAST_TIME;
                let point = pose([0.2, 0.62, -0.3], [-0.15, 0.85, 0.5], [1.0, 0.0, 0.0]);
                let keys = match weapon.profile.element {
                    // Raised up high, then brought down to point
                    Some(Element::Frost | Element::Holy) => {
                        vec![(release * 0.55, pose([0.2, 0.5, -0.02], [-0.05, 0.3, 0.95], [1.0, 0.0, 0.0]), false), (release + 0.05, point, true)]
                    }
                    // Swept round from the side
                    Some(Element::Storm | Element::Arcane | Element::Void) => {
                        vec![(release * 0.55, pose([0.34, 0.42, -0.24], [0.35, 0.45, 0.82], [0.0, 1.0, 0.0]), false), (release + 0.05, point, true)]
                    }
                    // Scooped up from low down
                    Some(Element::Poison) => {
                        vec![(release * 0.55, pose([0.26, 0.48, -0.42], [0.0, 0.9, -0.25], [1.0, 0.0, 0.0]), false), (release + 0.05, point, true)]
                    }
                    // Thrust straight out
                    _ => vec![(0.12 / CAST_TIME, point, false)],
                };
                match self.action {
                    Action::Cast { t, .. } => animate(&rest, &keys, t / CAST_TIME),
                    Action::Special { t, length, special } => animate(&rest, &special_move(special, Kind::Magic, style).1, t / length),
                    _ => rest,
                }
            }
        };
        // Swaying a little, and coming up into view when picked
        p.hand.x += 0.004 * (self.time * 1.1).sin();
        p.hand.z += 0.006 * (self.time * 2.3).sin() - 0.4 * (1.0 - self.raise).powi(2);
        // Bobbing with each step, side to side and down on each footfall
        let step = (self.stride / STEP_LENGTH * PI).sin();
        let walking = (self.speed / 4.0).min(1.0);
        p.hand.z -= 0.014 * step.abs() * walking;
        p.hand.x += 0.009 * step * walking;
        // Lowered and tipped forward while running, unless it's being used
        if self.action == Action::Idle {
            let running = smooth((self.speed - 5.5) / 2.5);
            p.hand.z -= 0.07 * running;
            p.hand.y -= 0.04 * running;
            p.turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.45 * running) * p.turn;
        }
        // Jolting back from a blow
        p.hand.y -= 0.035 * self.recoil;
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

    /// Model `look` of the set at `slot`, see weapons::SLOT_ARROW and the rest
    fn effect_model(&self, slot: usize, look: usize) -> (u32, &(Model, Vector3<f64>)) {
        self.model_at(slot * ELEMENT_LOOKS + look)
    }

    fn model_at(&self, index: usize) -> (u32, &(Model, Vector3<f64>)) {
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
        let (model, arm) = self.model_at(ARMS + which);
        place(model, arm, ARM_RES, Vector3::zeros(), grip, rotation, vpm, 1.0, HELD)
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
            out.push(self.arm(left as usize, hand, axis, if left { elbow(-0.3, 0.12, -0.7) } else { elbow(0.37, 0.12, -0.7) }, vpm));
            // The arrow on the string while drawing. The hand pulling it back is by the cheek, out
            // of sight.
            if matches!(self.action, Action::Draw { .. }) {
                let nock = hand + rotation * (self.nock(weapon) - look.grip) * vpm;
                let (model, arrow) = self.effect_model(SLOT_HELD_ARROW + weapon.arrowhead(), Element::look(weapon.profile.element));
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
                let (model, arrow) = self.effect_model(SLOT_ARROW + p.head, look);
                add(p.position, place(model, arrow, ARROW_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), p.position, rotation, vpm, 1.0, MOVING));
            } else if matches!(p.form, Form::Fireball | Form::Orb) {
                // A ball of fire or darkness, churning as it goes
                let spin = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.time * 6.0) * rotation;
                let (model, ball) = self.effect_model(SLOT_FIREBALL, look);
                add(p.position, place(model, ball, MODEL_RES, Vector3::zeros(), p.position, spin, vpm, if p.form == Form::Orb { 0.13 } else { 0.15 }, MOVING));
            } else if p.form != Form::Lightning {
                let (model, bolt) = self.effect_model(SLOT_BOLT, look);
                add(p.position, place(model, bolt, BOLT_RES, Vector3::zeros(), p.position, rotation, vpm, p.size, MOVING));
            }
        }
        for s in &self.stuck {
            let instance = match s.head {
                Some(head) => {
                    let (model, arrow) = self.effect_model(SLOT_ARROW + head, s.look);
                    place(model, arrow, ARROW_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), s.position, s.rotation, vpm, 1.0, STILL)
                }
                None => {
                    let (model, thorn) = self.model_at(THORN);
                    place(model, thorn, MODEL_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), s.position, s.rotation, vpm, THORN_SIZE, STILL)
                }
            };
            add(s.position, instance);
        }
        // What bosses throw, tumbling
        for h in &self.hostiles {
            let spin = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.time * 5.0) * UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 3.0);
            let instance = match h.kind {
                HostileKind::Boulder(None) => {
                    let (model, boulder) = self.model_at(BOULDER);
                    place(model, boulder, MODEL_RES, Vector3::zeros(), h.position, spin, vpm, 1.0, MOVING)
                }
                // Ice or molten rock, the size of a boulder
                HostileKind::Boulder(Some(e)) => {
                    let (model, ball) = self.effect_model(SLOT_FIREBALL, Element::look(Some(e)));
                    place(model, ball, MODEL_RES, Vector3::zeros(), h.position, spin, vpm, 0.5, MOVING)
                }
                HostileKind::Spell(e) => {
                    let (model, ball) = self.effect_model(SLOT_FIREBALL, Element::look(Some(e)));
                    place(model, ball, MODEL_RES, Vector3::zeros(), h.position, spin, vpm, 0.35, MOVING)
                }
                HostileKind::Web => {
                    let (model, web) = self.model_at(WEB);
                    place(model, web, MODEL_RES, Vector3::zeros(), h.position, spin, vpm, 1.0, MOVING)
                }
                // Point first along the way it's going
                HostileKind::Thorn => {
                    let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &h.velocity.normalize()).unwrap_or_else(UnitQuaternion::identity);
                    let (model, thorn) = self.model_at(THORN);
                    place(model, thorn, MODEL_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), h.position, rotation, vpm, THORN_SIZE, MOVING)
                }
            };
            add(h.position, instance);
        }
        // Where things are coming down
        for &(at, radius) in &self.marks {
            let (model, ring) = self.model_at(RING);
            add(at, place(model, ring, RING_RES, Vector3::zeros(), at, UnitQuaternion::identity(), vpm, radius, MOVING));
        }
        // Spikes of ice shooting up and pillars of light coming down, then dying away
        for i in &self.swells {
            let t = 1.0 - i.life / i.lifetime;
            let grow = if t < 0.12 { t / 0.12 } else { 1.0 - ((t - 0.12) / 0.88).powi(2) };
            let grow = if i.model == PILLAR { grow.sqrt() } else { grow };
            let (model, shape) = self.model_at(i.model);
            add(i.position, place(model, shape, MODEL_RES, Vector3::zeros(), i.position, UnitQuaternion::identity(), vpm, i.radius * 0.9 * grow.max(0.05), MOVING));
        }
        for s in self.sparks.iter().filter(|s| s.slot != SLOT_FINE) {
            let (model, spark) = self.effect_model(s.slot, s.look);
            // Drops stay whole till they land, sparks dwindle
            let size = if s.slot == SLOT_DROP { 1.0 } else { (s.life / s.lifetime).clamp(0.2, 1.0) };
            add(s.position, place(model, spark, SPARK_RES, Vector3::zeros(), s.position, UnitQuaternion::identity(), vpm, size, MOVING));
        }
        // Fireballs, swelling quickly to the size of the explosion and then shrinking away, turning
        // as they go
        for b in &self.blasts {
            // White-hot as it bursts, swelling fast and easing to its full size, then dying away
            let t = 1.0 - b.life / b.lifetime;
            let (model, ball) = self.effect_model(SLOT_FIREBALL, if t < 0.15 { Element::look(Some(Element::Holy)) } else { b.look });
            let grow = if t < 0.2 { 1.0 - (1.0 - t / 0.2).powi(3) } else { 1.0 - ((t - 0.2) / 0.8).powi(2) };
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
            .filter(|s| s.slot == SLOT_FINE)
            .map(|s| {
                let (model, spark) = self.effect_model(SLOT_FINE, s.look);
                let size = (s.life / s.lifetime).clamp(0.3, 1.0);
                place(model, spark, FINE_SPARK_RES, Vector3::zeros(), s.position, UnitQuaternion::identity(), view.vpm, size, MOVING)
            })
            .collect()
    }

    /// Lights from spells flying, flashes, burning animals and a glowing staff in hand, in the
    /// form main.rs keeps the world's lights in. They're marked as moving, so the shader works
    /// their light out fresh every frame rather than keeping it, see Light in shaders\traverse.comp.
    pub fn lights(&self, weapon: Option<&Weapon>, animals: &[Animal], view: &View) -> Vec<[f32; 8]> {
        let vpm = view.vpm;
        let light = |p: Vector3<f64>, reach: f64, color: [f32; 3], strength: f32| {
            [p.x as f32, p.y as f32, p.z as f32, (reach * vpm) as f32, color[0] * strength, color[1] * strength, color[2] * strength, 1.0]
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
        for h in &self.hostiles {
            match h.kind.element() {
                Some(Element::Fire) => lights.push(light(h.position, 7.0, Element::Fire.light(), 1.5)),
                Some(e) => lights.push(light(h.position, 5.0, e.light(), 0.8)),
                None => {}
            }
        }
        // Fire left burning, and holy light gathering
        for l in self.lingering.iter().filter(|l| l.element == Element::Fire) {
            lights.push(light(l.position + Vector3::new(0.0, 0.0, 0.5 * vpm), 3.0 + l.radius, Element::Fire.light(), 0.7 * (l.life / 1.0).min(1.0) as f32));
        }
        for s in &self.strikes {
            lights.push(light(s.position + Vector3::new(0.0, 0.0, 1.0 * vpm), 4.0, Element::Holy.light(), 0.6));
        }
        // Fire arrows burning where they stuck
        for s in self.stuck.iter().filter(|s| s.glowing > 0.0 && s.element == Some(Element::Fire)) {
            lights.push(light(s.position, 2.5, Element::Fire.light(), 0.6 * (s.glowing / 5.0).min(1.0) as f32));
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
    /// Seconds the fight's been going on for, to pulse things by
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Voxels per meter of the world, as of the last update
    pub fn vpm(&self) -> f64 {
        self.vpm
    }

    pub fn frenzy(&self) -> Option<(f64, f64)> {
        (self.frenzy > 0.0).then_some((self.frenzy_bonus, self.frenzy))
    }

    /// How far the bow in hand is drawn, 0-1, for the crosshair
    pub fn draw(&self, weapon: &Weapon) -> f64 {
        self.drawn(&weapon.profile)
    }
}
