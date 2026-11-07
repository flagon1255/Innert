//! Fighting in real time with the weapons from src\weapons.rs: swinging swords, drawing bows and
//! loosing arrows, casting spells that fly, and what their effects and synergies do to the animals
//! they hit. Positions are in voxels of the main world, the numbers in the weapon data in meters
//! and seconds.

use std::collections::HashMap;
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::rc::Rc;

use nalgebra::{Matrix3, Rotation3, Unit, UnitQuaternion, Vector3};

use crate::animals::{Animal, AnimalInstance, HELD, Look, MODEL_RES, MOVING, Model, STILL, TINT_BURNING, TINT_CHILLED, TINT_FROZEN, TINT_HIT, TINT_NONE, TINT_POISONED, TINT_SHOCKED};
use crate::audio::{Cue, Sound};
use crate::effects::{BLAST_LIVE, BLAST_REACH, BLAST_RES, BLAST_RING_RES, BLAST_RINGS, BLAST_STAGES, BLASTS, ICE_RES, ICE_STAGES, MOON, MOON_RES, PILLAR_RES, PILLAR_STAGES, STREAK_RES, STREAKS, blast_stage, leaves_smoke};
use crate::player::WaterLevel;
use crate::weapons::{
    ARM_RES, ARMS, ARROW_HEADS, ARROW_RES, BOLT_RES, BOMB_RES, BOULDER, Build, CLOUD_RES, PUDDLE_RES, QUARREL_HEADS, SLOT_BOMB, SLOT_CLOUD, SLOT_PUDDLE, TWISTER, WAVE, CROSSBOW_SPANNED, DROP_ASH, DROP_BLOOD, DROP_EMBER, DROP_GOLD, DROP_ICE, DROP_MUD, DROP_VENOM, DROP_VOID, DROP_WATER, ELEMENT_LOOKS, Effect, Element, FINE_SPARK_RES,
    GUN_COCKED, GUN_FIRED, HELD_ARROW_RES, ICE_BURST, Kind, LASH_CHAIN, LASH_CRACKERS, LASH_HEMP, LASH_RES, LASH_SEGMENTS, LASH_TAPERS, MUZZLE, MUZZLE_RES, NET, PILLAR, Profile, QUARREL_HARPOON, QUARREL_HEAD, RING, RING_RES, ROOTS, RUNE, SHELL, SHOT_RES, SLOT_ARROW, SLOT_BOLT,
    SLOT_DROP, SLOT_FINE, SLOT_FIREBALL, SLOT_HELD_ARROW, SLOT_HELD_QUARREL, SLOT_QUARREL, SLOT_SHOT, SLOT_SMOKE, SLOT_SPARK, SMOKE_RES, SPARK_RES, STONE_PILLAR, Shot, Special, Style, Synergy, THORN, WEB, Weapon, effect_models, Heavy,
    Casting, Finish, Knack, COIL, SERPENT,
};
use crate::weapons::{Burst, CRACKER_HOOK};
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
/// A crossbow's bolts fly flatter, a gun's balls flatter still, and a shell drops
const QUARREL_GRAVITY: f64 = 0.3;
const BALL_GRAVITY: f64 = 0.12;
const SHELL_GRAVITY: f64 = 0.6;
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
    /// A totem's hex eating away at it, slowing it: seconds left, and what it does a second. It
    /// passes on to whatever's nearest when it dies.
    hex: f64,
    hex_dps: f64,
    /// Seconds it's held to the spot by a net or a snare, still able to strike out
    rooted: f64,
    /// Seconds it's marked by a weapon with the mark effect, and how much more it takes from
    /// every blow and shot while it is
    marked: f64,
    mark_bonus: f64,
}

impl Status {
    /// A boss is only ever frozen or knocked off its feet for a moment
    pub fn limit_for_boss(&mut self) {
        self.frozen = self.frozen.min(0.6);
        self.staggered = self.staggered.min(0.25);
        self.rooted = self.rooted.min(1.2);
    }

    /// How fast it can move: 1 as normal, 0 frozen, knocked off its feet or rooted
    pub fn pace(&self) -> f64 {
        if self.frozen > 0.0 || self.staggered > 0.0 || self.rooted > 0.0 {
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

/// Special ammo loaded in the bow, crossbow or gun in hand, see Library::loaded()
pub struct Loaded {
    /// Which of Library::ammo, for game.rs to take one of off its stack each shot
    pub ammo: usize,
    /// What the weapon's shots do with it in
    pub profile: Rc<Profile>,
    /// Its arrowhead or bolt head, if not the weapon's own
    pub head: Option<usize>,
    /// How it bursts apart, if it does
    pub burst: Option<Burst>,
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
    /// A crossbow being spanned, or a gun loaded with powder and ball, `length` seconds in all
    Reload { t: f64, length: f64 },
    /// A crossbow or gun just shot, kicking back, and a repeater's lever spanning it again
    Fire { t: f64, length: f64 },
    /// A siege crossbow being braced on its leg, `t` seconds so far, ready once it's down, and
    /// whether to loose as soon as it is
    Brace { t: f64, loose: bool },
    /// An orb pouring out its beam, for `t` seconds so far
    Channel { t: f64 },
    /// A bell being raised to ring, harder the longer it's held, `t` seconds so far
    Charge { t: f64 },
    /// A spell being cast, `length` seconds in all, see cast_length(), and whether it's been let go
    Cast { t: f64, length: f64, cast: bool },
    /// A special attack being unleashed, see special_move()
    Special { t: f64, length: f64, special: Special },
    /// A bomb drawn back and lobbed, `length` seconds in all, and whether it's left the hand yet
    Throw { t: f64, length: f64, thrown: bool },
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

/// An arrow, a crossbow's bolt or a gun's shot as it's shot: what it is, its head, whether it was
/// shot at full power so it streaks, and how big it's drawn next to as it's made
#[derive(Clone, Copy)]
struct Missile {
    shot: Shot,
    head: usize,
    power: bool,
    size: f64,
}

impl Missile {
    fn arrow(head: usize, power: bool) -> Missile {
        Missile { shot: Shot::Arrow, head, power, size: 1.0 }
    }
}

struct Projectile {
    /// Shot from a bow, crossbow or gun, rather than cast
    arrow: bool,
    /// What it is, if it's shot
    shot: Shot,
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
    /// How hard it turns towards what's near, in turns a second, for a swarm of seekers
    seek: f64,
    /// How far round it bursts where it lands, in meters, whatever its weapon does, for shells
    /// falling in a barrage
    blast: f64,
    /// What else it does, for a harpoon or a special attack's bolt
    trick: Trick,
}

/// What a bolt does beyond flying and hitting
#[derive(Clone, Copy, PartialEq)]
enum Trick {
    None,
    /// A harpoon on its line, catching what it hits or where it strikes, see Tether
    Harpoon,
    /// Glancing on from what it hits to the next, this many more times, harder each time
    Ricochet(u32),
    /// Glanced off something already, by the ricochet effect, so it doesn't again
    Glanced,
    /// Bursting apart into a fan this many seconds into its flight, or where it strikes first
    Split(f64),
    /// One of a split bolt's fan, splitting once more into three where it strikes
    Splinter,
    /// A net, bursting open over the ground when it gets this close to where it's aimed
    Net(Vector3<f64>),
    /// Blowing up in what it hits, the blast leaping on from one to the next this many times
    Detonate(u32),
    /// A bomb, its fuse burning down this many more seconds, bouncing where it lands till it goes
    /// off, scattering this many bomblets as it does, and blowing a small hole if it was thrown
    /// by hand
    Bomb { fuse: f64, cluster: u8, hole: bool },
    /// Bursting ammo, see weapons::Burst: bursting into a spray of shards once this many more
    /// seconds have gone by in the air, or where it strikes if that's sooner, see Combat::airburst()
    /// and Combat::shatter()
    Burst { fuse: f64, burst: Burst },
    /// One of those shards, going off in a little blast where it lands, see Combat::pop()
    Fragment,
    /// A comet of ice coming down on here
    Comet(Vector3<f64>),
    /// A shard of a comet, going off quietly where it lands
    Shard,
    /// A rocket going up, bursting when it gets up to here
    Rocket(Vector3<f64>),
    /// An arrow bursting where it strikes into a ring of thorns
    Thornburst,
    /// One of those thorns, drawn as a thorn and sticking where it lands
    Thorn,
}

/// The ground under `at`, looked for from two meters over it to ten under, or `at` itself if
/// there's none
fn ground_under(world: &World, at: Vector3<f64>, vpm: f64) -> Vector3<f64> {
    let top = at + Vector3::new(0.0, 0.0, 2.0 * vpm);
    let down = Vector3::new(0.0, 0.0, -12.0 * vpm);
    solid_along(world, top, top + down).map_or(at, |t| top + down * t)
}

/// How fast to lob something from `from` so it comes down on `to`, in voxels a second, falling as
/// a bomb does, and how long it takes: the further, the longer and higher
fn lobbed(from: Vector3<f64>, to: Vector3<f64>, vpm: f64) -> (Vector3<f64>, f64) {
    let d = to - from;
    let time = 0.75 + 0.025 * d.norm() / vpm;
    let g = Vector3::new(0.0, 0.0, -9.8 * BOMB_GRAVITY * vpm);
    ((d - g * 0.5 * time * time) / time, time)
}

/// Seconds a censer takes to swing out and back, once for each cloud it billows
fn censer_period(profile: &Profile, speedup: f64) -> f64 {
    1.0 / (profile.stat("castSpeed") * speedup).max(0.3)
}

/// A harpoon's line, fast in what it caught or where it struck: reeling in what's small enough,
/// or the player to what's too big, or to the ground or wall it bit into
struct Tether {
    /// The animal it caught, by id, or None for where it struck, `at`
    caught: Option<u32>,
    at: Vector3<f64>,
    /// Seconds it's been pulling
    t: f64,
    /// Whether it drags what it caught in, rather than the player out to it
    reel: bool,
}

/// A rune scattered on the ground by Rune Mines, going off when anything comes near
struct Mine {
    position: Vector3<f64>,
    life: f64,
    /// Seconds before it's armed
    arming: f64,
    profile: Rc<Profile>,
    damage: f64,
}

/// A beam swept across what's ahead: seconds gone and in all, and how much it does a second
struct Sweep {
    t: f64,
    length: f64,
    profile: Rc<Profile>,
    damage: f64,
    /// What it's struck already, each only once
    hit: Vec<u32>,
}

/// The weapon in hand thrown spinning out ahead and coming back: seconds gone and in all, the
/// way it was thrown along the ground, how far, and what it's hit going out and coming back
struct Thrown {
    t: f64,
    length: f64,
    from: Vector3<f64>,
    direction: Vector3<f64>,
    reach: f64,
    position: Vector3<f64>,
    hit: Vec<u32>,
    back: bool,
    profile: Rc<Profile>,
    damage: f64,
}

/// Shackle Chain: chains shot out of the crossbow, a hook on the end of each, one to each of what's
/// caught near where it's aimed, then hauled on, dragging them all in together onto `center`, then
/// crashing them into each other, see Combat::drag()
struct Shackle {
    /// Where they're dragged to: the feet of the biggest it caught, or where it was aimed
    center: Vector3<f64>,
    chains: Vec<ShackleChain>,
    /// The biggest it caught, which the rest are dragged to, and how big round it is in meters
    anchor: Option<u32>,
    girth: f64,
    /// Where the chains leave the crossbow, as it's held now
    from: Vector3<f64>,
    /// Seconds since it was unleashed, see SHACKLE_RELEASE and the rest
    t: f64,
    profile: Rc<Profile>,
    damage: f64,
}

/// One of a shackle's chains
struct ShackleChain {
    /// What it's flung at, by id, or None when nothing was near and it's flung at the ground
    id: Option<u32>,
    /// Where that is now
    at: Vector3<f64>,
    /// Seconds after the chains leave the crossbow that it gets there and hooks in
    reach: f64,
    hooked: bool,
}

/// Seconds into Shackle Chain that the chains leave the crossbow, that they're hauled on, and that
/// what they caught crash into each other, as own_move() has the crossbow kick, haul and jolt
const SHACKLE_RELEASE: f64 = 0.2;
const SHACKLE_HAUL: f64 = 0.62;
const SHACKLE_CRASH: f64 = 1.05;
/// How far from the player a shackle's chains catch anything, and how far one's flung at the
/// ground when nothing's near where it's aimed, in meters
const SHACKLE_REACH: f64 = 30.0;
const CHAIN_FLING: f64 = 22.0;
/// How far a shackle reels in the one thing it caught when that's all it caught, at most, in meters
const SHACKLE_REEL: f64 = 6.0;
/// The most stretches each of a shackle's chains is drawn with, and how thick it's drawn for each
/// meter it is from the eye, in meters, see chain_instances()
const CHAIN_STRETCHES: usize = 5;
const CHAIN_THICK: f64 = 0.0065;

/// Where along a shackle's chain `length` meters long, `near` meters from the eye at one end and
/// `far` at the other, each of its stretches of links begins and ends, 0-1 of the way, for
/// chain_instances(): each longer the further off it is, so they all look about as long, as many
/// as it takes for none to look longer than about seven degrees, and at most `most`
fn chain_stretches(length: f64, near: f64, far: f64, most: usize) -> Vec<f64> {
    let (near, far) = (near.max(0.3), far.max(0.3));
    let even = (far - near).abs() < 0.05;
    // How long it looks, in radians, near enough, taking it as running straight from one end to
    // the other and getting further off evenly along it
    let looks = if even { length / near } else { length * (far / near).ln() / (far - near) };
    let n = ((looks / 0.12).ceil() as usize).clamp(1, most.max(1));
    (0..=n)
        .map(|k| {
            let f = k as f64 / n as f64;
            if even { f } else { ((near * (far / near).powf(f) - near) / (far - near)).clamp(0.0, 1.0) }
        })
        .collect()
}

/// Seconds after a shackle's chains leave the crossbow that the `k`th of them, flung `meters` off,
/// gets there: a little after the one before, and quicker the further it has to go, so even the
/// last has hooked in before they're hauled on
fn chain_flight(k: usize, meters: f64) -> f64 {
    0.03 * k.min(5) as f64 + 0.07 + 0.2 * (meters / SHACKLE_REACH).min(1.0)
}

/// Blood Moon: a red moon rising over the player, its light reddening everything, the weapon's
/// shots flying out at what's near, or a blade cutting down, blood torn out of each and streaming
/// back to heal the player, see Combat::wax()
struct BloodMoon {
    /// Seconds since it was unleashed, and when the shots leave the weapon or the blade comes down
    t: f64,
    release: f64,
    /// Where the moon hangs in the sky
    moon: Vector3<f64>,
    /// What's to be drained, by id, nearest first, and the shots flying at each
    targets: Vec<u32>,
    shots: Vec<Bloodshot>,
    streams: Vec<Stream>,
    /// The drops of blood streaming back into the pool, see Bead
    beads: Vec<Bead>,
    /// Seconds in that the pool was drunk down
    drunk: Option<f64>,
    /// Whether it's been let go yet, and whether it's a blade's, which tears the blood out itself
    /// rather than shooting
    released: bool,
    blade: bool,
    /// What the weapon shoots, to draw its shots as, None for a blood orb
    shot: Option<Shot>,
    /// Health given back so far, and how much each one drained gives
    healed: f64,
    heal: f64,
    /// How many streams of blood have got to the pool ahead of the player, see blood_pool()
    gathered: usize,
    profile: Rc<Profile>,
    damage: f64,
    /// Seconds in that it was all done, and the moon began to sink away
    ending: Option<f64>,
}

impl BloodMoon {
    /// Seconds in that the moon begins to sink away, if it has
    fn end(&self) -> f64 {
        self.ending.unwrap_or(f64::MAX)
    }
}

/// A Blood Moon's shot, flying from the weapon at what it drains, or up at the moon if there's
/// nothing near
struct Bloodshot {
    target: Option<u32>,
    from: Vector3<f64>,
    to: Vector3<f64>,
    position: Vector3<f64>,
    /// Seconds it's been flying, and how long it takes to get there
    t: f64,
    arrive: f64,
}

/// A drop of blood streaming back from what Blood Moon drained into the pool ahead of the player,
/// flying as a spark does. Kept and drawn with the moon rather than among the sparks, so however
/// busy it gets they're never what's left out, and there are only ever BEADS of them.
struct Bead {
    position: Vector3<f64>,
    velocity: Vector3<f64>,
    life: f64,
}

/// The most of Blood Moon's drops of blood in the air at once, and how many a second stream back
/// from each it drained
const BEADS: usize = 14;
const BEAD_RATE: f64 = 9.0;

/// Blood streaming back out of what a Blood Moon drained to the player
struct Stream {
    /// Where it pours out from
    from: Vector3<f64>,
    /// Seconds it's been pouring, sparks owed, and whether the health it brings has come yet
    t: f64,
    owed: f64,
    arrived: bool,
}

/// How long a Blood Moon hangs at least, how far off it rises, in meters, how high up in the view
/// it rises, as part of the way from the middle to the top, and the least it rises over the
/// horizon, in radians, and how far round the player it drains, in meters
const MOON_SECONDS: f64 = 2.4;
const MOON_OFF: f64 = 42.0;
const MOON_IN_VIEW: f64 = 0.35;
const MOON_LOWEST: f64 = 0.12;
const MOON_REACH: f64 = 14.0;
/// Where Blood Moon's blood pools before the player drinks it, in meters ahead of and under the eye
const POOL_AHEAD: f64 = 2.2;
const POOL_LOW: f64 = 0.55;

/// Where Blood Moon's blood pools before the player drinks it, as the view's turned now: a little
/// way ahead and low, where it's seen gathering without filling the view
fn blood_pool(view: &View) -> Vector3<f64> {
    view.eye + view.rotation * Vector3::new(0.0, POOL_AHEAD, -POOL_LOW) * view.vpm
}

/// Where a Blood Moon rises for a view looking `forward`, with the tangent of half its field of
/// view up and down `tan_up`, from `eye`: on ahead, well up in the view, but never under the horizon
fn moon_rise(eye: Vector3<f64>, forward: Vector3<f64>, tan_up: f64, vpm: f64) -> Vector3<f64> {
    let flat = Vector3::new(forward.x, forward.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
    let pitch = forward.z.clamp(-1.0, 1.0).asin();
    let up = (pitch + tan_up.atan() * MOON_IN_VIEW).clamp(MOON_LOWEST, 1.2);
    eye + (flat * up.cos() + Vector3::new(0.0, 0.0, up.sin())) * MOON_OFF * vpm
}

/// How long the moon takes to sink away once it's all done, in seconds, and how big it is drawn,
/// in meters round
const MOON_SINK: f64 = 0.6;
const MOON_SIZE: f64 = 5.0;
/// How long blood takes to stream back to the player, and how long each stream pours, in seconds,
/// and how much it falls as it arcs over, 1 as fast as things do
const STREAM_FLIGHT: f64 = 0.42;
const STREAM_SECONDS: f64 = 0.7;
const STREAM_GRAVITY: f64 = 0.5;
/// How much of its speed a spark loses to the air each second, as Combat::update() slows them
const SPARK_DRAG: f64 = 1.5;
/// The most Blood Moon heals in all
const MOON_HEAL: f64 = 45.0;
/// How fast Blood Moon's shots fly, in meters a second: slow enough to be seen on their way
const BLOODSHOT_PACE: f64 = 30.0;

/// How fast to send a spark off from `from`, in voxels a second, so it gets to `to` in `flight`
/// seconds, slowing in the air as sparks do and falling `gravity` as fast as things do, so it
/// arcs over on its way
fn streamed(from: Vector3<f64>, to: Vector3<f64>, flight: f64, gravity: f64, vpm: f64) -> Vector3<f64> {
    let g = 9.8 * gravity * vpm;
    let carry = (1.0 - (-SPARK_DRAG * flight).exp()) / SPARK_DRAG;
    let up = Vector3::new(0.0, 0.0, g / SPARK_DRAG);
    (to - from + up * flight) / carry - up
}

/// A bomb of Cluster Volley waiting to be fired, as own_move() has the weapon kick: seconds till it
/// goes, and where it's aimed to come down
struct Lob {
    delay: f64,
    to: Vector3<f64>,
    k: usize,
    profile: Rc<Profile>,
    damage: f64,
}

/// How big a bomblet is, next to a bomb, and how big a hole it blows, in meters, and the shards of
/// bursting ammo, see weapons::Burst: how far round each one's blast reaches, and how big a hole
/// the first to go off blows
const BOMBLET_SIZE: f64 = 0.7;
const BOMBLET_HOLE: f64 = 0.6;
const FRAGMENT_BLAST: f64 = 1.1;
const FRAGMENT_HOLE: f64 = 0.6;
/// Bursting ammo that bursts in the air does so this much of the way to what it's aimed at, no
/// sooner than this many seconds out, taking what's aimed at as no further off than this, in meters
const AIRBURST_AT: f64 = 0.45;
const AIRBURST_SOONEST: f64 = 0.05;
const AIRBURST_REACH: f64 = 35.0;
/// How far round an airburst's own blast in the air reaches, in meters, and how much of the shot's
/// damage it does
const AIRBURST_BLAST: f64 = 1.6;
const AIRBURST_POWER: f64 = 0.3;
/// How big a shard of bursting ammo is drawn, next to a ball of shot, how much of its damage its
/// little blast does to what's round it, and how fast shards fly: from a burst in the air, as part
/// of the shot's speed, and from one where it strikes, in meters a second
const FRAGMENT_SIZE: f64 = 2.6;
const POP_SHARE: f64 = 0.4;
const AIRBURST_SHARDS: f64 = 0.4;
const STRIKE_SHARDS: f64 = 11.0;

/// The way the `k`th of `n` shards of bursting ammo flies, bursting apart going `way`, spread
/// `spread` degrees round it at most: round a golden-angle spiral out over the cone, so however
/// many there are they're spread evenly over it, turned `jitter` radians round it so no two bursts
/// are alike
fn shard_way(way: Vector3<f64>, spread: f64, k: usize, n: usize, jitter: f64) -> Vector3<f64> {
    let way = way.try_normalize(1e-6).unwrap_or_else(Vector3::z);
    let side = way.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::x);
    let up = side.cross(&way);
    let a = k as f64 * 2.399_963 + jitter;
    let off = (spread.to_radians() * ((k as f64 + 0.5) / n.max(1) as f64).sqrt()).min(PI * 0.6);
    (way * off.cos() + (side * a.cos() + up * a.sin()) * off.sin()).normalize()
}

/// How much further a cluster shell's blast reaches than a plain explosion's, and how much bigger
/// a hole a bomb packed with bomblets blows than a plain bomb, see Combat::impact() and bomb_burst()
const CLUSTER_BLAST: f64 = 1.25;
/// Seconds into Cluster Volley that a crossbow or gun fires its first bomb, and between each and
/// the next, as own_move() has it kick
const VOLLEY_FIRST: f64 = 0.22;
const VOLLEY_GAP: f64 = 0.07;
/// When bombs flung out underarm leave the hand, seconds in
const VOLLEY_FLUNG: f64 = 0.3;

/// How long a special attack takes and the poses the weapon goes through, see special_move()
type SpecialMove = (f64, Vec<(f64, Pose, bool)>);

/// The moves of the special attacks that have their own, for special_move(), or None for the
/// rest: Shackle Chain's crossbow levelled and shot, hauled back hard as the chains bite and jolted
/// as what they caught crashes together; Blood Moon's, see blood_moon_move(); Cluster Volley's bombs
/// fired off one after another, each kicking the weapon up, or flung out underarm
fn own_move(special: Special, kind: Kind, style: Style) -> Option<SpecialMove> {
    let shouldered = matches!(style, Style::Crank | Style::Powder);
    match special {
        Special::Shackle if shouldered => {
            let length = 1.25;
            let s = |t: f64| t / length;
            let level = shouldered_level();
            // Hauled back and down, its nose tipped up, kept low and out to the side so what's
            // being dragged in can still be seen, straining, then jolted forward and down as the
            // chains go slack
            let mut hauled = level.clone();
            hauled.hand += Vector3::new(0.05, -0.07, -0.05);
            hauled.turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 0.3) * UnitQuaternion::from_axis_angle(&Vector3::z_axis(), -0.12) * hauled.turn;
            let mut strain = hauled.clone();
            strain.hand += Vector3::new(0.006, -0.02, -0.008);
            let mut slack = level.clone();
            slack.hand += Vector3::new(0.0, 0.04, -0.05);
            slack.turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.15) * slack.turn;
            Some((
                length,
                vec![
                    (s(0.12), level.clone(), false),
                    (s(SHACKLE_RELEASE), kicked(&level, 1.2), true),
                    (s(0.36), level.clone(), false),
                    (s(SHACKLE_HAUL - 0.06), level.clone(), false),
                    (s(SHACKLE_HAUL), hauled, true),
                    (s(SHACKLE_CRASH - 0.05), strain, false),
                    (s(SHACKLE_CRASH), slack, true),
                    (s(1.15), level, false),
                ],
            ))
        }
        Special::Bloodmoon => Some(blood_moon_move(kind, style)),
        // Raised off to the side, aimed up to lob them, and fired five times, kicking each time
        Special::ClusterBomb if shouldered => {
            let length = 0.95;
            let raised = pose([0.24, 0.4, -0.16], [0.08, -0.55, 0.83], [-0.08, 0.83, 0.55]);
            let mut keys = vec![(0.14 / length, raised.clone(), false)];
            for k in 0..5 {
                let at = VOLLEY_FIRST + VOLLEY_GAP * k as f64;
                keys.push((at / length, kicked(&raised, 0.8), true));
                keys.push(((at + 0.035) / length, raised.clone(), false));
            }
            keys.push((0.75 / length, raised, false));
            Some((length, keys))
        }
        // A bomb held low, swung back and flung out underarm, all five leaving the hand together
        Special::ClusterBomb if style == Style::Lob => {
            let length = 0.9;
            let low = pose([0.3, 0.3, -0.36], [0.1, -0.4, 0.9], [1.0, 0.0, 0.0]);
            let flung = pose([0.18, 0.62, -0.12], [0.0, 0.8, 0.6], [1.0, 0.0, 0.0]);
            let after = pose([0.2, 0.55, -0.24], [0.0, 0.7, -0.7], [1.0, 0.0, 0.0]);
            Some((length, vec![(0.16 / length, low, false), (VOLLEY_FLUNG / length, flung, true), (0.55 / length, after, false)]))
        }
        _ => None,
    }
}

/// A crossbow or gun levelled at the crosshair, for own_move()
fn shouldered_level() -> Pose {
    pose([0.14, 0.42, -0.17], [0.04, -0.12, 0.99], [-0.1, 1.0, 0.08])
}

/// `p` kicked back and up by a shot, `kick` 1 for a hard one
fn kicked(p: &Pose, kick: f64) -> Pose {
    let mut k = p.clone();
    k.hand.y -= 0.05 * kick;
    k.hand.z += 0.02 * kick;
    k.turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 0.3 * kick) * k.turn;
    k
}

/// Blood Moon's moves: a crossbow or gun levelled and shot, a bow drawn and loosed, bombs flung,
/// or a staff thrust out, at what's near, then raised to the moon as the blood comes back; or a
/// blade raised high to the moon, held trembling, then cut down hard across, which tears the blood
/// out, and held low as it streams back. The shots go, or the blade comes down, on the first quick
/// key, see first_release().
fn blood_moon_move(kind: Kind, style: Style) -> (f64, Vec<(f64, Pose, bool)>) {
    let length = 1.3;
    let s = |t: f64| t / length;
    let keys = if matches!(style, Style::Crank | Style::Powder) {
        let level = shouldered_level();
        let mut raised = level.clone();
        raised.hand.z += 0.06;
        raised.turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 0.45) * raised.turn;
        vec![(s(0.1), level.clone(), false), (s(0.26), kicked(&level, 1.0), true), (s(0.42), level, false), (s(0.7), raised.clone(), false), (s(1.1), raised, false)]
    } else if style == Style::Lob {
        let low = pose([0.3, 0.3, -0.36], [0.1, -0.4, 0.9], [1.0, 0.0, 0.0]);
        let flung = pose([0.18, 0.62, -0.12], [0.0, 0.8, 0.6], [1.0, 0.0, 0.0]);
        let raised = pose([0.22, 0.45, 0.1], [0.0, 0.2, 0.98], [1.0, 0.0, 0.0]);
        vec![(s(0.16), low, false), (s(0.28), flung, true), (s(0.48), raised.clone(), false), (s(1.1), raised, false)]
    } else if kind == Kind::Ranged {
        let drawn = pose([-0.1, 0.66, -0.08], [-0.33, 0.02, 0.94], [0.0, 1.0, 0.0]);
        let mut loosed = drawn.clone();
        loosed.hand.y -= 0.04;
        let up = pose([-0.08, 0.5, 0.1], [-0.3, 0.1, 0.95], [0.0, 0.6, 0.8]);
        vec![(s(0.16), drawn.clone(), false), (s(0.26), loosed, true), (s(0.4), drawn, false), (s(0.7), up.clone(), false), (s(1.1), up, false)]
    } else if kind == Kind::Magic {
        let raised = pose([0.15, 0.4, 0.1], [0.0, 0.2, 0.98], [-1.0, 0.0, 0.0]);
        let thrust = pose([0.12, 0.7, -0.18], [-0.1, 0.99, 0.05], [-1.0, 0.0, 0.0]);
        vec![(s(0.2), raised.clone(), false), (s(0.32), thrust.clone(), true), (s(0.5), thrust, false), (s(0.8), raised.clone(), false), (s(1.1), raised, false)]
    } else {
        // Raised up ahead towards the moon, the blade standing up into the view, not out of sight
        // overhead
        let raised = pose([0.2, 0.5, -0.1], [0.06, 0.5, 0.86], [-1.0, 0.0, 0.0]);
        let mut shaking = raised.clone();
        shaking.hand += Vector3::new(-0.012, 0.01, 0.025);
        shaking.turn = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), 0.05) * shaking.turn;
        let cut = pose([0.06, 0.62, -0.16], [-0.35, 0.92, 0.1], [-0.75, -0.2, -0.6]);
        let low = pose([-0.2, 0.5, -0.36], [-0.85, 0.45, -0.25], [-0.3, -0.2, -0.93]);
        vec![(s(0.22), raised, false), (s(0.42), shaking, false), (s(0.52), cut, true), (s(0.68), low.clone(), false), (s(1.1), low, false)]
    };
    (length, keys)
}

/// Seconds into a move of `length` seconds that it lets go: its first quick key
fn first_release(length: f64, keys: &[(f64, Pose, bool)]) -> f64 {
    keys.iter().find(|k| k.2).map_or(length * 0.3, |k| k.0 * length)
}

/// Something a coil bind holds fast, by id: rooted where it stands, crushed every so often as the
/// coils draw tighter, the coils drawn round it, see Combat::writhe()
struct Bind {
    id: u32,
    life: f64,
    tick: f64,
    profile: Rc<Profile>,
    damage: f64,
    /// Where its middle is and how big round it is, in meters, to draw the coils round it
    at: Vector3<f64>,
    girth: f64,
}

/// Seconds a coil bind holds, and between its squeezes
const BIND_SECONDS: f64 = 4.0;
const BIND_SQUEEZE: f64 = 0.5;

/// One strand of a serpent lash, slithering along the ground after what it's after, or on ahead
/// if nothing, weaving from side to side, biting what it reaches, see Combat::writhe()
struct Serpent {
    position: Vector3<f64>,
    /// Which way it's heading, level
    heading: Vector3<f64>,
    target: Option<u32>,
    life: f64,
    /// Where it is in its weave, and seconds till it leaves a spark behind
    phase: f64,
    trail: f64,
    profile: Rc<Profile>,
    damage: f64,
}

/// How many strands a serpent lash splits into, how fast they slither, in meters a second, and
/// how long they go on
const SERPENTS: usize = 5;
const SERPENT_SPEED: f64 = 11.0;
const SERPENT_SECONDS: f64 = 2.4;

/// How far a thunder crack's cone of sound reaches, in meters, and how wide it is either side of
/// straight ahead, in radians
const THUNDER_REACH: f64 = 14.0;
const THUNDER_CONE: f64 = 0.6;

/// How many stretches a burning brand's line catches in, one after another, and how far either
/// side of it burns, in meters
const BRAND_STRETCHES: u32 = 8;
const BRAND_WIDTH: f64 = 0.9;

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
    /// A totem's hex, drifting slowly after what's near and clinging to it, eating away at it
    Hex,
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
            Form::Hex => (1.0, 0.0, 0.8, 0.6),
        }
    }
}

/// Seconds a hex goes on eating away at what it caught, how much of the hit it does each second,
/// and how much it slows it
const HEX_SECONDS: f64 = 6.0;
const HEX_DAMAGE: f64 = 0.4;
const HEX_SLOW: f64 = 0.35;
/// Seconds between the hurts of a beam, and how much of an orb's damage it does a second for each
/// cast a second it has
const BEAM_TICK: f64 = 0.15;
const BEAM_POWER: f64 = 0.6;
/// The colors a prismatic sweep runs through as it goes across
const PRISM: [Element; 5] = [Element::Fire, Element::Frost, Element::Storm, Element::Arcane, Element::Holy];
/// The most familiars a tome can keep conjured at once, how long each lasts and how often it casts
const FAMILIARS: usize = 3;
const FAMILIAR_SECONDS: f64 = 8.0;
const FAMILIAR_PACE: f64 = 0.75;

/// Something conjured hanging in the air, casting at whatever's nearest: a tome's familiar, or a
/// phantom of the Phantom Host
struct Familiar {
    position: Vector3<f64>,
    life: f64,
    /// Seconds until it casts again
    wait: f64,
    profile: Rc<Profile>,
    damage: f64,
    phantom: bool,
}

/// Spectral blades circling the player: seconds left, how far round they've gone, and seconds
/// until they cut again
struct Orbit {
    left: f64,
    angle: f64,
    tick: f64,
    profile: Rc<Profile>,
    damage: f64,
}

/// A bubble of stilled time: what's in it all but stops
struct Stasis {
    position: Vector3<f64>,
    /// In meters
    radius: f64,
    life: f64,
    tick: f64,
    profile: Rc<Profile>,
    damage: f64,
}

/// A soul siphon's beam, draining what's ahead: seconds left, and how much it does a second
struct Siphon {
    left: f64,
    profile: Rc<Profile>,
    damage: f64,
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
    /// Flames leaping high off it, a wall of them, rather than flickering low over the ground
    tall: bool,
    /// A cloud hanging in the air, billowing, rather than something lying on the ground: what a
    /// censer leaves, or what something killed by miasma bursts into
    cloud: bool,
    /// A pool left by a weapon's effect, drawn as a puddle of its element lying on the ground
    pool: bool,
}

/// Seconds after an explosion blows a hole before another can, so a pack going up at once
/// doesn't hold up a frame changing the world
const SCAR_GAP: f64 = 0.12;

/// How high a sunburst's sun hangs over the ground, in meters
const SUN_HEIGHT: f64 = 4.2;

/// How far a bomb's blast reaches, in meters, for each of its area, and how far round the hole
/// one thrown by hand blows
const BOMB_BLAST: f64 = 2.8;
const BOMB_HOLE: f64 = 0.5;
/// How much a bomb falls next to things falling as they do, and how long its fuse burns on after
/// it was meant to land
const BOMB_GRAVITY: f64 = 1.0;
const BOMB_FUSE: f64 = 0.35;
/// When a bomb leaves the hand, as part of the throw
const BOMB_RELEASE: f64 = 0.45;

/// The most pools and clouds the effects of weapons leave lying about at once
const MOST_POOLS: usize = 10;

/// A ring rolling out along the ground from `center`, hurting and throwing back each thing once
/// as it passes: from a maul's blow, a blow with the shockwave effect, or an earthquake. Drawn
/// with sparks and earth thrown up along its front, never by changing the ground.
struct Ripple {
    center: Vector3<f64>,
    /// How far out it's got and how far it goes, in meters, and how fast, in meters a second
    radius: f64,
    reach: f64,
    speed: f64,
    /// Animals it's passed, by id
    hit: Vec<u32>,
    profile: Rc<Profile>,
    damage: f64,
    /// How hard it throws what it passes away, in meters a second, and how long it knocks it off
    /// its feet
    push: f64,
    stagger: f64,
    /// Whether its hits bring the weapon's effects, and whether boulders heave up along it
    effects: bool,
    rocks: bool,
    /// Sparks and boulders owed along its front
    owed: f64,
}

/// What sort of thing a Zone is
#[derive(Clone, Copy, PartialEq, Debug)]
enum ZoneKind {
    /// A thunderhead hanging over the ground, lightning striking what's under it
    Storm,
    /// A bubbling bog, miring and poisoning what's in it
    Mire,
    /// A sun hanging over the ground, searing what's under it with rays, then bursting
    Sun,
    /// A whirlwind wandering on ahead, dragging in and battering what it passes
    Cyclone,
    /// A ball of lightning drifting on ahead, lashing what's near it
    Ball,
    /// A swarm going from one enemy to the next
    Swarm,
    /// A boulder rolling on ahead, bowling over what's in its way
    Boulder,
    /// A crescent of light cut loose from a blade, flying on ahead through everything it meets
    Crescent,
    /// A keg set down, its fuse burning, going off as it ends
    Keg,
    /// A weapon's head flung out on its chain, going round the player wider and wider
    Orbit,
    /// A burning brand's line of fire along the ground, from where it is along `velocity`, which
    /// isn't a speed here but the line itself
    Brand,
}

/// One of the special attacks that goes on a while somewhere, moving or not, see Combat::zones()
struct Zone {
    kind: ZoneKind,
    position: Vector3<f64>,
    /// In voxels a second
    velocity: Vector3<f64>,
    /// How far round it reaches, in meters
    radius: f64,
    life: f64,
    lifetime: f64,
    /// Seconds until it hurts again
    tick: f64,
    profile: Rc<Profile>,
    damage: f64,
    /// What it's struck, by id: once each for a boulder, what it's searing for a sun, and where
    /// it's been for a swarm
    hit: Vec<u32>,
    /// What a swarm's after, and how long it's been at it
    target: Option<u32>,
    dwell: f64,
    /// Sparks owed
    owed: f64,
    /// Where each stretch of a burning brand's line lies on the ground
    path: Vec<Vector3<f64>>,
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
    /// Lightning strung between everything round about, the `k`th time
    Web(u32),
    /// The `k`th wave of knives whirling out
    Knives(u32),
    /// A shell falling onto here
    Shell(Vector3<f64>),
    /// A bolt plunging down onto the animal with this id
    Plunge(u32),
    /// A blast leaping from `from` on to the animal with id `to`, the `hop`th, which came from
    /// `last`
    Blast { from: Vector3<f64>, to: u32, last: u32, hop: u32 },
    /// Fire leaping from `from` on to the animal with id `to`, the `hop`th
    Kindle { from: Vector3<f64>, to: u32, hop: u32 },
    /// A great sweep through everything in front, as a harvest's pulled it in
    Reap,
    /// The `k`th ring of an earthquake rolling out
    Quake(u32),
    /// Spikes of ice bursting up here, this many meters across, as a shatterwave tears through
    Spike(Vector3<f64>, f64),
    /// The ground bursting into flame under the animal with this id
    Erupt(u32),
    /// Or here, where there's nothing to erupt under
    Geyser(Vector3<f64>),
    /// Boulders let loose to tumble on ahead, as the weapon comes down
    Avalanche,
    /// A shatterwave torn out ahead, as the weapon sweeps up
    Shatter,
    /// The `k`th moment of a charge: what's in front skewered and carried along
    Skewer(u32),
    /// The end of a charge: what was skewered flung off
    Fling,
    /// The `k`th ring of a coiling lash cracked out round the player
    Coil(u32),
    /// The `k`th crack of a flaying storm
    Flay(u32),
    /// The `k`th arrow of a quickdraw
    Quick(u32),
    /// A shot at the animal with this id
    Reckon(u32),
    /// A pillar of stone bursting up here
    Pillar(Vector3<f64>),
    /// A lasso's loop dropping round the animal with this id, or it yanked in
    Lasso { id: u32, yank: bool },
    /// A coil bind's lash drawn tight round the animal with this id and what's beside it
    Bind(u32),
    /// The `k`th ring of a thunder crack's cone of sound rolling out
    Boom(u32),
    /// A serpent lash splitting into its strands where it struck the ground
    Serpents,
    /// The `k`th eighth of a trip sweep going round
    Trip(u32),
    /// The `k`th stretch of a burning brand's line catching, from `from` along to `to`
    Brand { from: Vector3<f64>, to: Vector3<f64>, k: u32 },
    /// A chain swing: swung off from where the lash caught, or come down again
    Swing { land: bool },
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
    /// A crossbow's bolt rather than an arrow
    quarrel: bool,
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
    /// A crossbow bolt loosed by a bandit, flying almost flat, hurting only what it hits
    Bolt,
}

impl HostileKind {
    /// How hard it falls, next to things falling as they do
    fn gravity(self) -> f64 {
        match self {
            HostileKind::Boulder(_) | HostileKind::Spell(Element::Poison) | HostileKind::Web => 1.0,
            HostileKind::Thorn => 0.25,
            HostileKind::Bolt => 0.12,
            HostileKind::Spell(_) => 0.0,
        }
    }

    fn element(self) -> Option<Element> {
        match self {
            HostileKind::Boulder(e) => e,
            HostileKind::Spell(e) => Some(e),
            HostileKind::Web => Some(Element::Poison),
            HostileKind::Thorn | HostileKind::Bolt => None,
        }
    }

    /// Whether it only hurts what it hits, sticking in where it lands, rather than going off
    fn pierces(self) -> bool {
        matches!(self, HostileKind::Thorn | HostileKind::Bolt)
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

/// The smoke an explosion leaves drifting up and thinning away after it's gone, drawn with its
/// last stages, see effects::BLAST_STAGES
struct Remnant {
    position: Vector3<f64>,
    look: usize,
    radius: f64,
    yaw: f64,
    life: f64,
    lifetime: f64,
}

/// Seconds an explosion's smoke drifts after it, and how many drift at once at most
const REMNANT_SECONDS: f64 = 0.7;
const MOST_REMNANTS: usize = 8;
/// How many explosions at once are drawn with a ring thrown up round them on the ground
const MOST_RINGS: usize = 6;

/// Which way an explosion at `at` is turned, so no two look the same
fn blast_yaw(at: Vector3<f64>) -> f64 {
    let h = hash((at.x * 3.0) as i64 as u32 ^ hash((at.y * 3.0) as i64 as u32 ^ hash((at.z * 3.0) as i64 as u32)));
    (h >> 8) as f64 / 16777216.0 * TAU
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

/// How many points a whip's lash is simulated as, from where it leaves the handle to its tip
const LASH_POINTS: usize = 15;
/// Steps of the simulation each frame, and passes in each keeping its points the right way apart
const LASH_STEPS: usize = 4;
const LASH_PASSES: usize = 10;
/// How much of its speed the air takes away each second
const LASH_DRAG: f64 = 1.2;
/// How hard it's pulled into the shape a swing steers it into at full strength, per second squared
const LASH_PULL: f64 = 1600.0;
/// How far the lash's root is from the eye as it cracks, in meters: the lash is as long as the
/// whip's reach less this, so its tip snaps out about as far as the whip reaches
const LASH_SHORT: f64 = 0.8;
/// How thick the lash is at its tip, next to where it leaves the handle
const LASH_THIN: f64 = 0.5;
/// How close to the eye the lash can come, in meters, so it goes round the player's head and
/// never sweeps right across the view up close
const LASH_HEAD: f64 = 0.4;

/// Whether `p` is in something solid
fn solid(world: &World, p: Vector3<f64>) -> bool {
    p.z < 0.0 || !is_passable(world.material_at(p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64))
}

/// Which way something going at `velocity` goes on glancing off the solid thing it hit at `at`:
/// turned back along each of its axes that runs on into it, or straight back if none seems to
fn glance(world: &World, at: Vector3<f64>, velocity: Vector3<f64>) -> Vector3<f64> {
    let step = velocity.normalize() * 0.7;
    let back = at - step;
    let mut v = velocity;
    for axis in 0..3 {
        let mut probe = back;
        probe[axis] += step[axis] * 1.5;
        if solid(world, probe) {
            v[axis] = -v[axis];
        }
    }
    if v == velocity { -velocity } else { v }
}

/// The direction `t` of the way round from `a` to `b`, both of them unit vectors
fn swing_toward(a: Vector3<f64>, b: Vector3<f64>, t: f64) -> Vector3<f64> {
    let angle = a.dot(&b).clamp(-1.0, 1.0).acos();
    if angle < 1e-6 {
        return b;
    }
    let axis = a.cross(&b).try_normalize(1e-6).or_else(|| a.cross(&Vector3::z()).try_normalize(1e-6)).unwrap_or_else(Vector3::x);
    UnitQuaternion::from_axis_angle(&nalgebra::Unit::new_unchecked(axis), angle * t) * a
}

/// A whip's lash, simulated as a rope: LASH_POINTS points, in the world's voxels, from its root at
/// the top of the handle to its tip, moved on by Verlet integration under gravity and drag, kept a
/// fixed distance apart, and kept out of the ground and the player's head. Its root goes where
/// the hand takes the handle, so walking and turning sway it and swings throw it about.
struct Lash {
    points: Vec<Vector3<f64>>,
    /// Where each point was a step ago, which is how fast it's going
    before: Vec<Vector3<f64>>,
    /// How far apart its points are kept, in voxels
    segment: f64,
    /// Where its root and the eye were after the last frame's steps, to move them smoothly
    /// through this frame's
    root: Vector3<f64>,
    eye: Vector3<f64>,
}

impl Lash {
    /// Hanging from `root` down to the ground and lying along it ahead
    fn hanging(arena: &Arena, root: Vector3<f64>, segment: f64) -> Lash {
        let view = arena.view;
        let ahead = Vector3::new(view.forward().x, view.forward().y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let down = (ahead * 0.35 - Vector3::z()).normalize();
        let mut points = vec![root];
        for _ in 1..LASH_POINTS {
            let last = *points.last().unwrap();
            let next = [last + down * segment, last + ahead * segment].into_iter().find(|&p| !solid(arena.world, p)).unwrap_or(last + Vector3::new(0.0, 0.0, segment));
            points.push(next);
        }
        Lash { before: points.clone(), points, segment, root, eye: view.eye }
    }

    fn tip(&self) -> Vector3<f64> {
        self.points[LASH_POINTS - 1]
    }

    /// Moves it on `dt` seconds, its root moving to `root` and the eye to `eye` over that, and
    /// each point pulled towards where `guide` steers it, if it does: how far from the root, in
    /// voxels, and how hard, 0-1. `frame` is how long the frame really took, which is longer in
    /// slow motion.
    fn step(&mut self, arena: &Arena, root: Vector3<f64>, eye: Vector3<f64>, guide: Option<&[(Vector3<f64>, f64)]>, dt: f64, frame: f64) {
        let vpm = arena.view.vpm;
        let h = dt / LASH_STEPS as f64;
        let gravity = Vector3::new(0.0, 0.0, -9.8 * vpm);
        let keep = (-LASH_DRAG * h).exp();
        // How fast the hand's really going, so the pull steers it relative to that. The player can
        // still turn and move in the slow motion of a blow landing.
        let carried = (root - self.root) / frame.max(dt).max(1e-6);
        let (from_root, from_eye) = (self.root, self.eye);
        for step in 1..=LASH_STEPS {
            let k = step as f64 / LASH_STEPS as f64;
            let (r, e) = (from_root.lerp(&root, k), from_eye.lerp(&eye, k));
            for i in 1..LASH_POINTS {
                let p = self.points[i];
                let moved = (p - self.before[i]) * keep;
                let mut a = gravity;
                if let Some(&(offset, strength)) = guide.and_then(|g| g.get(i))
                    && strength > 0.0
                {
                    // A critically damped pull, a little under, so it snaps there with a hint of
                    // overshoot
                    let pull = LASH_PULL * strength;
                    a += (r + offset - p) * pull - (moved / h - carried) * (1.2 * pull.sqrt());
                }
                self.before[i] = p;
                self.points[i] = p + moved + a * h * h;
            }
            self.points[0] = r;
            self.before[0] = r;
            // Each stretch kept the same length, from the root out, the root not giving
            for _ in 0..LASH_PASSES {
                for i in 0..LASH_POINTS - 1 {
                    let d = self.points[i + 1] - self.points[i];
                    let n = d.norm();
                    if n < 1e-9 {
                        continue;
                    }
                    let fix = d * ((n - self.segment) / n);
                    if i == 0 {
                        self.points[1] -= fix;
                    } else {
                        self.points[i] += fix * 0.5;
                        self.points[i + 1] -= fix * 0.5;
                    }
                }
            }
            for i in 1..LASH_POINTS {
                let mut p = self.points[i];
                // Round the player's head
                let off = p - e;
                if off.norm() < LASH_HEAD * vpm {
                    p = e + off.try_normalize(1e-9).unwrap_or_else(Vector3::z) * LASH_HEAD * vpm;
                }
                // Out of the ground, onto it, sliding to a stop along it
                if solid(arena.world, p) {
                    let lifted = Vector3::new(p.x, p.y, p.z.floor() + 1.02);
                    p = if solid(arena.world, lifted) { self.before[i] } else { lifted };
                    let b = self.before[i];
                    self.before[i] = Vector3::new(p.x - (p.x - b.x) * 0.7, p.y - (p.y - b.y) * 0.7, p.z);
                }
                self.points[i] = p;
            }
        }
        self.root = root;
        self.eye = eye;
    }

    /// The world moved over by `shift` voxels
    fn recenter(&mut self, shift: Vector3<f64>) {
        for p in self.points.iter_mut().chain(self.before.iter_mut()) {
            *p -= shift;
        }
        self.root -= shift;
        self.eye -= shift;
    }
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
    /// Seconds the swing goes on in slow motion for as a blow lands, so it feels like it hit
    /// something, see HITSTOP_PACE
    hitstop: f64,
    /// The weapon jolting back from a blow, 0-1
    recoil: f64,
    /// The pose the weapon in hand was drawn in this frame, how fast it was moving, in meters and
    /// radians a second, and the move it was drawn for, so a new move blends in from wherever it
    /// really was rather than snapping
    drawn: Option<Pose>,
    drawn_velocity: (Vector3<f64>, Vector3<f64>),
    drawn_action: Action,
    blending: Option<Blending>,
    /// The lash of the whip in hand, if it's a whip
    lash: Option<Lash>,
    /// Where a whip's special attack flings its lash, while it goes on: round what it lassos or
    /// binds, or what it's caught on to swing from
    lash_aim: Option<Vector3<f64>>,
    /// What coil binds hold fast, see Bind, and a serpent lash's strands, see Serpent
    binds: Vec<Bind>,
    serpents: Vec<Serpent>,
    /// Spells cast since the weapon was taken in hand, for a rare staff's twirl every third, see
    /// cast_twirl()
    casts: u32,
    /// How hard the camera shakes, wearing off
    pub shake: f64,
    /// How far a shot has thrown the view up, in radians, and how fast it's going, springing back
    /// down, for main.rs to turn the camera by
    pub kick: f64,
    kick_speed: f64,
    /// Shots loaded in each crossbow and gun, by its place among Game::weapons, so one put away
    /// loaded is still loaded taken out again. One that isn't here is fully loaded.
    loads: HashMap<usize, u32>,
    /// Seconds the muzzle flash of the gun in hand goes on showing, and which way round it's
    /// turned, so each looks a little different
    flare: f64,
    flare_roll: f64,
    /// The head and element of the special ammo in the bow, crossbow or gun in hand, if there is
    /// any, for drawing the arrow on its string or the bolt in its groove
    nocked: Option<(Option<usize>, Option<Element>)>,
    /// Which special ammo, of Library::ammo, was shot since game.rs last looked, one for each shot
    pub spent: Vec<usize>,
    /// Seconds until a beam hurts again, and until its hum sounds again
    beam_tick: f64,
    beam_hum: f64,
    familiars: Vec<Familiar>,
    orbit: Option<Orbit>,
    stases: Vec<Stasis>,
    siphon: Option<Siphon>,
    /// A harpoon's line, fast in something, and where it leaves the harpoon crossbow in hand
    tether: Option<Tether>,
    line_from: Option<Vector3<f64>>,
    /// How far a siege crossbow in hand is braced on its leg, 0-1
    brace: f64,
    /// What's rooted to the spot, where, and how big, to draw a net over each
    snared: Vec<(Vector3<f64>, f64)>,
    mines: Vec<Mine>,
    sweep: Option<Sweep>,
    thrown: Option<Thrown>,
    shackle: Option<Shackle>,
    blood_moon: Option<BloodMoon>,
    /// Cluster Volley's bombs waiting to be fired, see Lob
    lobs: Vec<Lob>,
    /// Rings rolling out along the ground, see Ripple
    ripples: Vec<Ripple>,
    /// Special attacks going on a while somewhere, see Zone
    zones: Vec<Zone>,
    /// Seconds until a weapon's effects can leave another pool, so a quick one doesn't flood the
    /// ground with them
    pool_timer: f64,
    /// Seconds until the censer in hand billows again as it's swung
    puff: f64,
    /// Seconds until an explosion can blow a hole or stain the ground again, see SCAR_GAP
    scar_timer: f64,
    /// While set, explosions blow no holes and leave no stains, for a special attack's many small
    /// ones going off together
    scarless: bool,
    /// Beams to draw solid this frame: from, to and look
    rays: Vec<(Vector3<f64>, Vector3<f64>, usize)>,
    /// How fast the player is going in meters a second, and how far they've walked, to bob what's
    /// in hand as they go
    speed: f64,
    stride: f64,
    /// Hits on the player from what bosses threw: how hard, from where, and of what element
    pub player_hits: Vec<(f64, Vector3<f64>, Option<Element>)>,
    /// Where the tip of the weapon in hand was last frame, while it leaves a trail
    trail: Option<Vector3<f64>>,
    /// Where along its blade an epic or rarer one's glow trails from was last frame, and seconds
    /// its glow goes on lighting what's round it, see TRAIL_GRADE
    ribbon: Option<Vector3<f64>>,
    glow: f64,
    glints: Vec<Glint>,
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
    /// A blow struck in hand is landing, right in front of the eye: what flies off it is kept
    /// small, fine sparks rather than ones the size of the world's voxels, see melee()
    close: bool,
    /// Seconds until blood can stain the ground again, so a flurry of blows doesn't paint it all
    blood_timer: f64,
    /// How charged the special attack is, 0-1
    pub charge: f64,
    /// Health a special attack gives back, and how it throws the player, for game.rs to see to
    pub heal: f64,
    pub dash: Vector3<f64>,
    /// Seconds a hunter's mark lasts yet, and the ids of what's marked
    hunt: f64,
    marked: Vec<u32>,
    /// Over the heads of what's marked, to draw its marks
    marked_at: Vec<Vector3<f64>>,
    /// What an impaling charge has skewered, by id, and which way it's charging
    skewered: Vec<u32>,
    charging: Vector3<f64>,
    /// Seconds the weapon in hand has been at rest, for a flourish now and then, see idle_keys()
    idle: f64,
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
    /// How high each of `blasts` is over the ground under it, for the ring it throws up there,
    /// worked out as each is new, see age_blasts(); and the smoke they leave after
    blast_floors: Vec<f64>,
    remnants: Vec<Remnant>,
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

/// `n` directions spread evenly all round
fn sphere_directions(n: usize) -> Vec<Vector3<f64>> {
    (0..n)
        .map(|k| {
            let z = 1.0 - 2.0 * (k as f64 + 0.5) / n as f64;
            let r = (1.0 - z * z).sqrt();
            let a = k as f64 * 2.399_963;
            Vector3::new(a.cos() * r, a.sin() * r, z)
        })
        .collect()
}

/// What flies out of a creature of `look` when it's struck, one of weapons::DROP_BLOOD and the
/// rest, and what it stains the ground with, if anything: blood, a spider's green ichor, or chips
/// of bone, stone, ice, embers or wood off the dead, golems and the treant
fn ichor(look: Look) -> (usize, Option<u8>) {
    match look {
        Look::Skeleton | Look::ArmoredSkeleton | Look::GraveKnight | Look::BoneArcher | Look::Necromancer => (DROP_WATER, None),
        // A bloater spills venom wherever it's cut
        Look::Bloater => (DROP_VENOM, Some(MOSS)),
        Look::Golem => (DROP_MUD, None),
        Look::FrostGolem => (DROP_ICE, None),
        Look::MagmaGolem => (DROP_EMBER, None),
        Look::Treant => (DROP_ASH, None),
        Look::Spider | Look::Spiderling | Look::Rimeweaver | Look::DuneBurrower => (DROP_VENOM, Some(MOSS)),
        // Shards of frost off a wraith, and the swamp water a lurker's full of
        Look::RimeWraith => (DROP_ICE, None),
        Look::MireLurker => (DROP_WATER, Some(MOSS)),
        _ => (DROP_BLOOD, Some(CLOTH)),
    }
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

/// `p` mirrored from the right of the view to the left
fn mirrored(p: &Pose) -> Pose {
    let q = p.turn.quaternion();
    Pose { hand: Vector3::new(-p.hand.x, p.hand.y, p.hand.z), turn: UnitQuaternion::new_normalize(nalgebra::Quaternion::new(q.w, q.i, -q.j, -q.k)) }
}

/// `p` for a crossbow mirrored from the right of the view to the left, as a hand crossbow goes
/// through its moves in the left hand. mirrored() flips the weapon's own x, which a blade swung the
/// other way round can take, but a crossbow's x is the way it shoots: flipped, it would turn round
/// and point back at the player. So it's turned half round its own length too, which puts the
/// way it shoots ahead again and only swaps its two sides, as a crossbow looks the same either way.
fn mirrored_shouldered(p: &Pose) -> Pose {
    let mut m = mirrored(p);
    m.turn *= UnitQuaternion::from_axis_angle(&Vector3::z_axis(), PI);
    m
}

fn blend(a: &Pose, b: &Pose, t: f64) -> Pose {
    Pose { hand: a.hand.lerp(&b.hand, t), turn: turn_toward(&a.turn, &b.turn, t) }
}

/// `a` turned `t` of the shortest way round to `b`, or on past it for `t` over 1
fn turn_toward(a: &UnitQuaternion<f64>, b: &UnitQuaternion<f64>, t: f64) -> UnitQuaternion<f64> {
    a * UnitQuaternion::from_scaled_axis((a.inverse() * b).scaled_axis() * t)
}

/// `a` turned `t` of the way to `b` along the great arc between them as quaternions, or on past
/// it. Unlike turn_toward() this never jumps to going round the other way when `a` and `b` are
/// turned more than half a turn apart, which the mixes inside a spline can be, so long as the
/// keys it's made from have their signs lined up.
fn arc_toward(a: &UnitQuaternion<f64>, b: &UnitQuaternion<f64>, t: f64) -> UnitQuaternion<f64> {
    let (qa, qb) = (a.coords, b.coords);
    let theta = qa.dot(&qb).clamp(-1.0, 1.0).acos();
    let q = if theta < 1e-6 {
        qa.lerp(&qb, t)
    } else {
        let s = theta.sin();
        qa * (((1.0 - t) * theta).sin() / s) + qb * ((t * theta).sin() / s)
    };
    UnitQuaternion::new_normalize(nalgebra::Quaternion::from(q))
}

/// A pose `k` of the way from `a` to `b`, the hand pushed out by `push` meters: an in-between key,
/// so a blade comes round in an arc rather than cutting the corner straight across
fn through(a: &Pose, b: &Pose, k: f64, push: [f64; 3]) -> Pose {
    let mut p = blend(a, b, k);
    p.hand += Vector3::from(push);
    p
}

/// A lunge reaching full stretch at `full`: it lands `meters` short of that along the blade, still
/// driving hard, carries on to full stretch, then is drawn back by `back` meters and held a moment,
/// rather than already slowing as the blow lands
fn lunge_keys(lands: f64, full: Pose, meters: f64, back: [f64; 3]) -> [(f64, Pose, bool); 3] {
    let mut contact = full.clone();
    contact.hand -= full.turn * Vector3::z() * meters;
    let mut held = full.clone();
    held.hand += Vector3::from(back);
    [(lands, contact, true), (lands + 0.05, full, false), (lands + 0.15, held, false)]
}

/// Keys closer together than this, as parts of a move, are a cut: the weapon's out of sight there,
/// gone round behind, so it jumps from one to the next rather than sweeping across the view
const CUT: f64 = 0.025;
/// How far a weapon's tip moves for each radian it turns, in meters, for spacing keys by how far
/// apart they look rather than just how far the hand moves
const LEVER: f64 = 0.45;
/// How much faster than the move's own pace it's going as it reaches a key it speeds up into
const QUICK: f64 = 1.7;

/// Catmull-Rom through four points, by Barry and Goldman's pyramid of mixes, at `s` between the
/// knots of the middle two. With `mix` turning rotations it gives a rotation spline whose turning
/// speed carries on smoothly through each key, as the hand's speed does.
fn catmull_rom<T>(p: [&T; 4], k: [f64; 4], s: f64, mix: impl Fn(&T, &T, f64) -> T) -> T {
    let r = |i: usize, j: usize| (s - k[i]) / (k[j] - k[i]);
    let a1 = mix(p[0], p[1], r(0, 1));
    let a2 = mix(p[1], p[2], r(1, 2));
    let a3 = mix(p[2], p[3], r(2, 3));
    let b1 = mix(&a1, &a2, r(0, 2));
    let b2 = mix(&a2, &a3, r(1, 3));
    mix(&b1, &b2, r(1, 2))
}

/// The cubic from `s0` at `t0` to `s1` at `t1`, leaving and arriving with slopes `m0` and `m1`
fn hermite(t0: f64, t1: f64, s0: f64, s1: f64, m0: f64, m1: f64, t: f64) -> f64 {
    let h = t1 - t0;
    let x = ((t - t0) / h).clamp(0.0, 1.0);
    let (x2, x3) = (x * x, x * x * x);
    (2.0 * x3 - 3.0 * x2 + 1.0) * s0 + (x3 - 2.0 * x2 + x) * h * m0 + (3.0 * x2 - 2.0 * x3) * s1 + (x3 - x2) * h * m1
}

/// The pose `f` of the way through a move going from `rest` through `keys` and back: poses, each
/// with when it's reached, 0-1, and whether it's reached speeding up, as a blow is.
///
/// The hand follows a centripetal Catmull-Rom spline through the keys, and the weapon turns along
/// the same spline made of turns, so neither stops dead at a key. Keys are spaced along it by how
/// far apart they look. Time is warped onto it by a monotone cubic, so the move eases out of rest
/// and back into it, speeds up into a quick key and lingers a little on the one before, all
/// without its speed ever dropping to nothing partway.
fn animate(rest: &Pose, keys: &[(f64, Pose, bool)], f: f64) -> Pose {
    let f = f.clamp(0.0, 1.0);
    let mut all: Vec<(f64, &Pose, bool)> = Vec::with_capacity(keys.len() + 2);
    all.push((0.0, rest, false));
    for (at, pose, quick) in keys {
        // Keys out of order are taken as coming straight after the one before
        let at = at.clamp(all.last().unwrap().0, 1.0);
        all.push((at, pose, *quick));
    }
    all.push((1.0, rest, false));
    // The piece between cuts that `f` is in: from the key after the last cut before it to the
    // key before the next cut. In the moment of a cut it's already gone on to the next.
    let mut start = 0;
    let mut end = all.len() - 1;
    for i in 0..all.len() - 1 {
        if all[i + 1].0 - all[i].0 < CUT {
            if f > all[i].0 {
                start = i + 1;
            } else {
                end = i;
                break;
            }
        }
    }
    let piece = &all[start..=end];
    if piece.len() == 1 {
        return piece[0].1.clone();
    }
    let n = piece.len();
    // Each key's turn as the quaternion nearest the one before, so the spline goes the short
    // way round between them
    let mut poses: Vec<Pose> = piece.iter().map(|k| k.1.clone()).collect();
    for i in 1..n {
        if poses[i].turn.coords.dot(&poses[i - 1].turn.coords) < 0.0 {
            poses[i].turn = UnitQuaternion::new_unchecked(-poses[i].turn.into_inner());
        }
    }
    // Knots spaced by the square root of how far apart keys look: centripetal, so it neither
    // overshoots on short hops nor loops round at sharp turns
    let apart = |a: &Pose, b: &Pose| (a.hand - b.hand).norm() + LEVER * (a.turn.inverse() * b.turn).angle();
    let mut knots = vec![0.0; n];
    for i in 1..n {
        knots[i] = knots[i - 1] + apart(&poses[i - 1], &poses[i]).max(2e-3).sqrt();
    }
    // The time warp's slope at each key, in knots a part of the move: easing out of and into rest,
    // carrying on at pace through a cut, in between the harmonic mean of the slopes either side
    let pace: Vec<f64> = (0..n - 1).map(|i| (knots[i + 1] - knots[i]) / (piece[i + 1].0 - piece[i].0).max(1e-6)).collect();
    let mut slope = vec![0.0; n];
    slope[0] = if start == 0 { 0.0 } else { pace[0] };
    slope[n - 1] = if end == all.len() - 1 { 0.0 } else { pace[n - 2] };
    for i in 1..n - 1 {
        let (h0, h1) = (piece[i].0 - piece[i - 1].0, piece[i + 1].0 - piece[i].0);
        let (w0, w1) = (2.0 * h1 + h0, h1 + 2.0 * h0);
        slope[i] = (w0 + w1) / (w0 / pace[i - 1] + w1 / pace[i]);
    }
    // Speeding up into a blow: reaching it QUICK times as fast as it went on average on the way
    // there. The spline itself goes slower or faster through a key depending on how far the keys
    // either side are, so that's made up for by how fast the time warp goes through it. A wind-up
    // before it lingers by itself, as the hand turns round there.
    for i in 1..n {
        if !piece[i].2 {
            continue;
        }
        if i + 1 == n {
            slope[i] = slope[i].max(QUICK * pace[i - 1]);
            continue;
        }
        // How far it looks to move for each knot of the spline, going through the key
        let (d0, d1) = (knots[i] - knots[i - 1], knots[i + 1] - knots[i]);
        let w = d0 / (d0 + d1);
        let hand = (poses[i].hand - poses[i - 1].hand) * ((1.0 - w) / d0) + (poses[i + 1].hand - poses[i].hand) * (w / d1);
        let turn = (poses[i - 1].turn.inverse() * poses[i].turn).scaled_axis() * ((1.0 - w) / d0) + (poses[i].turn.inverse() * poses[i + 1].turn).scaled_axis() * (w / d1);
        let along = hand.norm() + LEVER * turn.norm();
        let average = apart(&poses[i - 1], &poses[i]) / (piece[i].0 - piece[i - 1].0).max(1e-6);
        slope[i] = slope[i].max(QUICK * average / along.max(0.05));
    }
    // Kept from overshooting, so it never runs back along the spline (Fritsch and Carlson)
    for i in 0..n - 1 {
        let (a, b) = (slope[i] / pace[i], slope[i + 1] / pace[i]);
        if a * a + b * b > 9.0 {
            let tau = 3.0 / (a * a + b * b).sqrt();
            slope[i] = tau * a * pace[i];
            slope[i + 1] = tau * b * pace[i];
        }
    }
    let i = (0..n - 1).find(|&i| f <= piece[i + 1].0).unwrap_or(n - 2);
    let s = hermite(piece[i].0, piece[i + 1].0, knots[i], knots[i + 1], slope[i], slope[i + 1], f);
    // The four keys round it, the ends of the piece mirrored to make up the missing ones: `a`
    // carried on past by as much again as it came from `b`
    let mirror = |a: &Pose, b: &Pose| {
        let (qa, qb) = (a.turn.coords, b.turn.coords);
        let turn = UnitQuaternion::new_normalize(nalgebra::Quaternion::from(qa * (2.0 * qa.dot(&qb)) - qb));
        Pose { hand: a.hand * 2.0 - b.hand, turn }
    };
    let get = |j: isize| -> (f64, Pose) {
        if j < 0 {
            (knots[0] - (knots[1] - knots[0]), mirror(&poses[0], &poses[1]))
        } else if j as usize >= n {
            (knots[n - 1] + (knots[n - 1] - knots[n - 2]), mirror(&poses[n - 1], &poses[n - 2]))
        } else {
            (knots[j as usize], poses[j as usize].clone())
        }
    };
    let quad = [get(i as isize - 1), get(i as isize), get(i as isize + 1), get(i as isize + 2)];
    let k = [quad[0].0, quad[1].0, quad[2].0, quad[3].0];
    let hand = catmull_rom([&quad[0].1.hand, &quad[1].1.hand, &quad[2].1.hand, &quad[3].1.hand], k, s, |a, b, t| a.lerp(b, t));
    let turn = catmull_rom([&quad[0].1.turn, &quad[1].1.turn, &quad[2].1.turn, &quad[3].1.turn], k, s, arc_toward);
    Pose { hand, turn }
}

/// What's left of a jump in pose, dying away smoothly over a moment: one number's worth of
/// inertialization (Bollo), a quintic that starts at the jump `x0` going at `v0` and comes to rest
/// at nothing, still, `length` seconds on
#[derive(Clone, Copy)]
struct Fade {
    x0: f64,
    v0: f64,
    a0: f64,
    a: f64,
    b: f64,
    c: f64,
    length: f64,
}

impl Fade {
    fn new(x0: f64, v0: f64, length: f64) -> Fade {
        // Moving away from where it's going would only overshoot, so that's let go of, and it's
        // shortened if it's coming in fast enough to get there sooner
        let v0 = v0.min(0.0);
        let length = if v0 < 0.0 { length.min(-5.0 * x0 / v0).max(1e-3) } else { length };
        let t = length;
        let a0 = (-8.0 * v0 * t - 20.0 * x0) / (t * t);
        let a = -(a0 * t * t + 6.0 * v0 * t + 12.0 * x0) / (2.0 * t.powi(5));
        let b = (3.0 * a0 * t * t + 16.0 * v0 * t + 30.0 * x0) / (2.0 * t.powi(4));
        let c = -(3.0 * a0 * t * t + 12.0 * v0 * t + 20.0 * x0) / (2.0 * t.powi(3));
        Fade { x0, v0, a0, a, b, c, length }
    }

    fn at(&self, t: f64) -> f64 {
        if t >= self.length {
            return 0.0;
        }
        let t = t.max(0.0);
        ((((self.a * t + self.b) * t + self.c) * t + self.a0 * 0.5) * t + self.v0) * t + self.x0
    }
}

/// How long a new move takes to blend in from wherever the weapon really was, in seconds
const BLEND: f64 = 0.08;
/// How fast time goes for a swing while it's stopped by a blow landing: heavy slow motion, not a
/// dead stop, which would read as a stutter
const HITSTOP_PACE: f64 = 0.1;

/// The pose drawn last frame blending into a new move's: how far off the hand and the turn were
/// when it started, which way, and how long ago
struct Blending {
    age: f64,
    hand: (Vector3<f64>, Fade),
    turn: (Vector3<f64>, Fade),
}

impl Blending {
    /// From `drawn`, moving at `velocity` (meters and radians a second), into `target`
    fn new(drawn: &Pose, velocity: (Vector3<f64>, Vector3<f64>), target: &Pose) -> Blending {
        let off = drawn.hand - target.hand;
        let away = off.try_normalize(1e-9).unwrap_or_else(Vector3::x);
        let turned = (drawn.turn * target.turn.inverse()).scaled_axis();
        let about = turned.try_normalize(1e-9).unwrap_or_else(Vector3::x);
        Blending {
            age: 0.0,
            hand: (away, Fade::new(off.norm(), velocity.0.dot(&away), BLEND)),
            turn: (about, Fade::new(turned.norm(), velocity.1.dot(&about), BLEND)),
        }
    }

    fn apply(&self, target: &Pose) -> Pose {
        let (away, hand) = &self.hand;
        let (about, turn) = &self.turn;
        Pose { hand: target.hand + away * hand.at(self.age), turn: UnitQuaternion::from_scaled_axis(about * turn.at(self.age)) * target.turn }
    }

    fn done(&self) -> bool {
        self.age >= self.hand.1.length.max(self.turn.1.length)
    }
}

/// Whether the weapon went from `before` into a new move, rather than carrying on with the same one
fn new_move(before: Action, now: Action) -> bool {
    match (before, now) {
        (Action::Idle, Action::Idle) => false,
        (Action::Swing { t: a, .. }, Action::Swing { t: b, .. })
        | (Action::Special { t: a, .. }, Action::Special { t: b, .. })
        | (Action::Cast { t: a, .. }, Action::Cast { t: b, .. })
        | (Action::Draw { t: a }, Action::Draw { t: b })
        | (Action::Reload { t: a, .. }, Action::Reload { t: b, .. })
        | (Action::Fire { t: a, .. }, Action::Fire { t: b, .. })
        | (Action::Channel { t: a }, Action::Channel { t: b })
        | (Action::Charge { t: a }, Action::Charge { t: b })
        | (Action::Loose { t: a }, Action::Loose { t: b })
        | (Action::Throw { t: a, .. }, Action::Throw { t: b, .. }) => b < a,
        _ => true,
    }
}

/// The last blow of a combo, which hits hardest
const FINISHER: u8 = 2;
/// How much harder
const FINISHER_POWER: f64 = 1.4;

/// The style a weapon of `style` swings in: a battle staff like a scythe, sweeping with its blade
/// held out across the top, a maul like a hammer, the rest as they are
fn moves_like(style: Style) -> Style {
    match style {
        Style::Glaive => Style::Reap,
        Style::Quake => Style::Smash,
        style => style,
    }
}

/// The kind of weapon one of `profile`'s goes through its special attack's moves as: a battle
/// staff and bombs as a weapon in the hand, the rest as what they are
fn moves_kind(profile: &Profile) -> Kind {
    if matches!(profile.style, Style::Glaive | Style::Lob) { Kind::Melee } else { profile.kind }
}

/// Which blow of a combo in `style` is its finisher: a blade's chain goes four blows, each a
/// different way, a whip's, a flail's and a maul's three
fn finisher(style: Style) -> u8 {
    match style {
        Style::Slash | Style::Chop | Style::Smash | Style::Stab | Style::Thrust | Style::Reap => FINISHER + 1,
        _ => FINISHER,
    }
}

/// Whether blow `combo` of a chain in `style` is a finisher that goes `finish`'s way rather than
/// its style's
fn finishes_own_way(style: Style, finish: Finish, heavy: bool, combo: u8) -> bool {
    !heavy && finish != Finish::Style && combo == finisher(style) && !matches!(style, Style::Lash | Style::Whirl)
}

/// When a blow starts coming down and lands, as timing() has it, for a weapon whose heavy blow
/// goes `move_` and whose finisher goes `finish` rather than as its style's
fn blow_timing(style: Style, move_: Heavy, finish: Finish, heavy: bool, combo: u8) -> (f64, f64) {
    if finishes_own_way(style, finish, heavy, combo) {
        return finish_timing(finish);
    }
    if !heavy || move_ == Heavy::Style {
        return timing(style, heavy, combo);
    }
    match move_ {
        // A long, still gathering at the top before it all comes down
        Heavy::Heave => (0.55, 0.66),
        // Drawn and cut in one, fast once it goes
        Heavy::Draw => (0.42, 0.5),
        Heavy::Spin => (0.4, 0.54),
        Heavy::Twirl | Heavy::Overhead => (0.44, 0.58),
        Heavy::Sweep => (0.4, 0.55),
        // Drawn right back and held a moment, then driven out fast
        Heavy::Lunge => (0.46, 0.55),
        _ => (0.45, 0.58),
    }
}

/// The poses a blow goes through, as swing() has them, for a weapon whose heavy blow goes
/// `move_` and whose finisher goes `finish` rather than as its style's
fn blow_keys(style: Style, move_: Heavy, finish: Finish, heavy: bool, combo: u8) -> Vec<(f64, Pose, bool)> {
    if finishes_own_way(style, finish, heavy, combo) {
        return finish_keys(finish);
    }
    if !heavy || move_ == Heavy::Style {
        return swing(style, heavy, combo);
    }
    heavy_keys(style, move_)
}

/// When a finisher that goes `finish`'s way starts coming down and lands
fn finish_timing(finish: Finish) -> (f64, f64) {
    match finish {
        Finish::Lunge => (0.36, 0.47),
        Finish::Rising => (0.34, 0.48),
        Finish::Spin => (0.36, 0.5),
        Finish::Slam | Finish::Style => (0.4, 0.54),
    }
}

/// The poses of a finisher that goes `finish`'s way rather than as its weapon's style has it,
/// for animate()
fn finish_keys(finish: Finish) -> Vec<(f64, Pose, bool)> {
    let (comes, lands) = finish_timing(finish);
    let midway = comes + 0.55 * (lands - comes);
    match finish {
        // Drawn back low to the hip, the point leading, and driven out on a long step
        Finish::Lunge => {
            let back = pose([0.32, 0.12, -0.34], [0.04, 0.96, 0.25], [-1.0, 0.0, 0.0]);
            let lunge = pose([0.1, 0.76, -0.17], [-0.1, 0.99, 0.06], [-1.0, 0.0, 0.0]);
            let towards = (back.hand - lunge.hand) * 0.1;
            let mut keys = vec![(comes * 0.45, pose([0.34, 0.26, -0.28], [0.06, 0.9, 0.42], [-1.0, 0.0, 0.0]), false), (comes, back, false)];
            keys.extend(lunge_keys(lands, lunge, 0.18, [towards.x, towards.y, towards.z]));
            keys
        }
        // Dipped low on the left, then swept up through the middle and out high to the right,
        // the edge leading, and let ease back down
        Finish::Rising => {
            let low = pose([-0.06, 0.36, -0.5], [-0.55, 0.45, -0.7], [0.3, 0.45, 0.85]);
            let struck = pose([0.14, 0.62, -0.12], [0.18, 0.9, 0.4], [0.15, -0.4, 0.9]);
            vec![
                (comes * 0.45, pose([0.18, 0.38, -0.44], [-0.2, 0.6, -0.75], [0.2, 0.75, 0.6]), false),
                (comes, low.clone(), false),
                (midway, through(&low, &struck, 0.5, [0.0, 0.06, -0.05]), false),
                (lands, struck, true),
                (lands + 0.12, pose([0.36, 0.5, 0.06], [0.55, 0.4, 0.73], [0.0, -0.87, 0.5]), false),
                (lands + 0.26, pose([0.38, 0.46, -0.02], [0.6, 0.33, 0.73], [0.0, -0.9, 0.42]), false),
            ]
        }
        // Wound back to the right and turned right round, through in front, out to the left,
        // round behind out of sight and back in from below
        Finish::Spin => {
            let wound = pose([0.44, 0.26, -0.16], [0.82, -0.3, 0.48], [0.35, 1.0, 0.0]);
            let right = pose([0.5, 0.4, -0.22], [0.94, 0.3, 0.16], [-0.3, 1.0, 0.0]);
            let middle = pose([0.05, 0.64, -0.22], [0.08, 0.99, 0.06], [-1.0, 0.1, 0.0]);
            let left = pose([-0.54, 0.36, -0.26], [-0.92, 0.32, 0.08], [-0.3, -1.0, 0.0]);
            vec![
                (comes * 0.5, wound, false),
                (comes, right, false),
                (lands, middle, true),
                (lands + 0.12, left, false),
                (lands + 0.22, pose([-0.68, 0.12, -0.34], [-0.85, -0.45, 0.1], [0.4, -1.0, 0.0]), false),
                (lands + 0.23, pose([0.36, 0.3, -0.66], [0.0, 0.5, -0.85], [1.0, 0.0, 0.0]), false),
            ]
        }
        // Raised high and brought straight down, driving on into what it hits
        Finish::Slam | Finish::Style => {
            let raised = pose([0.2, 0.3, 0.04], [0.1, 0.0, 0.99], [0.0, 1.0, -0.05]);
            let struck = pose([0.07, 0.61, -0.3], [0.0, 0.83, -0.55], [0.0, -0.55, -0.83]);
            vec![
                (comes * 0.4, pose([0.3, 0.42, -0.3], [0.0, 0.6, 0.8], [0.0, 1.0, -0.1]), false),
                (comes, raised.clone(), false),
                (midway, through(&raised, &struck, 0.5, [0.0, 0.03, 0.08]), false),
                (lands, struck, true),
                (lands + 0.06, pose([0.07, 0.62, -0.38], [0.0, 0.76, -0.65], [0.0, -0.65, -0.76]), false),
                (lands + 0.22, pose([0.07, 0.58, -0.36], [0.0, 0.8, -0.6], [0.0, -0.6, -0.8]), false),
            ]
        }
    }
}

/// Blows a second of a weapon that swings neither heavy nor light, see weight()
const EVEN_SWINGS: f64 = 1.4;
/// At the heaviest, how much longer a blow's wind-up is, as a part of the whole blow, and how much
/// quicker it comes down once it goes; at the lightest the other way round
const WIND: f64 = 0.25;
const STRIKE: f64 = 0.2;
/// At the heaviest, how much further a blow carries on through after it lands; at the lightest,
/// how much shorter it's checked
const FOLLOW: f64 = 0.3;

/// Cracks a second of a whip that cracks neither heavy nor light, and how much more a whip's pace
/// counts towards how heavy it cracks than a blade's, as whips' paces lie closer together
const EVEN_CRACKS: f64 = 2.2;
const WHIP_WEIGHT: f64 = 1.8;

/// How heavy a weapon swings, from its blows a second, so it goes with whatever its parts, their
/// rarity and its size made of it: 1 for a maul, a blow every second and a half, 0 for a sword,
/// -1 for a dagger, three a second. A whip's next to other whips: a chain or a scourge weighted
/// with iron coils back longer and cracks harder, a light one of silk flicks out fast.
fn weight(profile: &Profile) -> f64 {
    if profile.style == Style::Lash {
        return ((EVEN_CRACKS / profile.stat("attackSpeed").max(0.1)).log2() * WHIP_WEIGHT).clamp(-1.0, 1.0);
    }
    (EVEN_SWINGS / profile.stat("attackSpeed").max(0.1)).log2().clamp(-1.0, 1.0)
}

/// When a blow timed `(comes, lands)` for its style starts coming down and lands for a weapon
/// swinging `weight` heavy: a heavy one gathered longer and brought down faster once it goes, a
/// light one snapped out sooner
fn weighted(timing: (f64, f64), weight: f64) -> (f64, f64) {
    let (comes, lands) = timing;
    let c = comes * (1.0 + WIND * weight);
    (c, (c + (lands - comes) * (1.0 - STRIKE * weight)).min(0.9))
}

/// `at`, a moment of a blow timed `from`, moved to the same moment of it timed `to`: its wind-up,
/// its stroke and its follow-through each stretched or squeezed to fit
fn retimed(at: f64, from: (f64, f64), to: (f64, f64)) -> f64 {
    let fit = |a0: f64, a1: f64, b0: f64, b1: f64| b0 + (at - a0) * (b1 - b0) / (a1 - a0).max(1e-9);
    if at <= from.0 {
        fit(0.0, from.0, 0.0, to.0)
    } else if at <= from.1 {
        fit(from.0, from.1, to.0, to.1)
    } else {
        fit(from.1, 1.0, to.1, 1.0)
    }
}

/// A blow's poses, `keys` as blow_keys() has them for a blow timed `timing`, for a weapon swinging
/// `weight` heavy: retimed as weighted() has it, and carried on further through after it lands
/// the heavier it is, checked sooner the lighter. A cut stays a cut, out of sight, and nothing
/// that wasn't one becomes one.
fn weighted_keys(keys: Vec<(f64, Pose, bool)>, timing: (f64, f64), weight: f64) -> Vec<(f64, Pose, bool)> {
    let to = weighted(timing, weight);
    let follow = 1.0 + FOLLOW * weight;
    let mut out: Vec<(f64, Pose, bool)> = Vec::with_capacity(keys.len());
    // The key before, when it was and when it is now
    let (mut was, mut now) = (0.0, 0.0);
    // Where the blow landed, while what comes after is still its follow-through
    let mut landed: Option<Pose> = None;
    for (at, mut pose, quick) in keys {
        let at = at.max(was);
        let gap = at - was;
        let t = retimed(at, timing, to);
        let t = if gap < CUT { t.clamp(now, now + CUT - 1e-6) } else { t.max(now + CUT + 1e-6) };
        // The follow-through ends at a cut, out of sight, or with a move of its own
        if gap < CUT || quick {
            landed = None;
        }
        if let Some(from) = &landed {
            pose.hand = from.hand + (pose.hand - from.hand) * follow;
            pose.turn = from.turn * UnitQuaternion::from_scaled_axis((from.turn.inverse() * pose.turn).scaled_axis() * follow);
        }
        if quick && (at - timing.1).abs() < 1e-9 {
            landed = Some(pose.clone());
        }
        (was, now) = (at, t);
        out.push((t, pose, quick));
    }
    out
}

/// When a blow of the weapon with `profile` starts coming down and lands, as parts of the whole
/// swing, for how heavy it swings. A whip's is when its first crack lands, see lash_swing().
fn blow_times(profile: &Profile, heavy: bool, combo: u8) -> (f64, f64) {
    if profile.style == Style::Lash {
        let swing = lash_swing(heavy, combo, profile.grade, weight(profile));
        return (swing.comes, swing.lands);
    }
    weighted(blow_timing(moves_like(profile.style), profile.heavy, profile.finish, heavy, combo), weight(profile))
}

/// The poses a blow of the weapon with `profile` goes through, for animate(), for how heavy it
/// swings, landing as blow_times() has it
fn blow_poses(profile: &Profile, heavy: bool, combo: u8) -> Vec<(f64, Pose, bool)> {
    if profile.style == Style::Lash {
        return lash_swing(heavy, combo, profile.grade, weight(profile)).keys;
    }
    let style = moves_like(profile.style);
    let timing = blow_timing(style, profile.heavy, profile.finish, heavy, combo);
    weighted_keys(blow_keys(style, profile.heavy, profile.finish, heavy, combo), timing, weight(profile))
}

/// How much longer a whip's swing is for each turn it twirls its lash round after its last
/// crack, as a part of the swing, and how hard its finisher's twirl lets it hit to make up for it
const LASH_TWIRL: f64 = 0.32;

/// How many times a whip of `grade` twirls its lash round after blow `combo` cracks: a legendary
/// one's finisher once, a mythic one's every blow, its finisher twice. Never a heavy blow.
fn lash_turns(grade: usize, heavy: bool, combo: u8) -> f64 {
    if heavy || grade < FLOURISH_GRADE {
        return 0.0;
    }
    match (grade > FLOURISH_GRADE, combo == FINISHER) {
        (true, true) => 2.0,
        (true, false) | (false, true) => 1.0,
        (false, false) => 0.0,
    }
}

/// A whip's swing worked out for how heavy it cracks: its handle's poses, the lash's cracks,
/// when the first crack starts to come and lands, when a double crack's second lands, and how
/// many times longer than its plain length the whole swing takes, for a twirl after the cracks
struct LashSwing {
    keys: Vec<(f64, Pose, bool)>,
    cracks: Vec<Crack>,
    comes: f64,
    lands: f64,
    second: Option<f64>,
    stretch: f64,
}

/// A whip's swing, light, heavy or a finisher, of `grade`, for one that cracks `weight` heavy, see
/// weight(): a heavy one coils back longer before it throws, a light one flicks out sooner, the
/// throw and what comes after it going at their own pace, so the whole swing's that much longer or
/// shorter, and its cracks land just as its handle's checked. A legendary or mythic one's lash is
/// then twirled round after its last crack, see lash_turns(): what comes before squeezed into the
/// start of a longer swing for a finisher, and fitted into the follow-through of a light blow.
fn lash_swing(heavy: bool, combo: u8, grade: usize, weight: f64) -> LashSwing {
    let timing = timing(Style::Lash, heavy, combo);
    // The wind-up's stretched, and everything after it moved on by as much
    let extra = timing.0 * WIND * weight;
    let coiled = 1.0 + extra;
    let retime = |t: f64| if t <= timing.0 { t * (1.0 + WIND * weight) } else { t + extra } / coiled;
    let mut keys: Vec<(f64, Pose, bool)> = lash_keys(heavy, combo, grade).into_iter().map(|(t, p, q)| (retime(t), p, q)).collect();
    let mut cracks = lash_cracks(heavy, combo, grade);
    // Held out a little longer by a heavy one, as it bites
    for c in &mut cracks {
        c.from = retime(c.from);
        c.at = retime(c.at);
        c.hold *= 1.0 + 0.6 * weight.max(0.0);
    }
    let (comes, lands) = (retime(timing.0), retime(timing.1));
    let second = (combo == FINISHER && !heavy && grade >= LASH_DOUBLE).then(|| retime(LASH_SECOND));
    let turns = lash_turns(grade, heavy, combo);
    if turns <= 0.0 {
        return LashSwing { keys, cracks, comes, lands, second, stretch: coiled };
    }
    // A finisher's made longer for its twirls, everything before them coming at the same pace
    let stretch = if combo == FINISHER { 1.0 + LASH_TWIRL * turns / coiled } else { 1.0 };
    let squeeze = 1.0 / stretch;
    for k in &mut keys {
        k.0 *= squeeze;
    }
    for c in &mut cracks {
        c.from *= squeeze;
        c.at *= squeeze;
    }
    let (comes, lands, second) = (comes * squeeze, lands * squeeze, second.map(|s| s * squeeze));
    // From just after the last crack's first follow-through, to a little before the end
    let last = cracks.last().map_or(lands, |c| c.at);
    let follow = keys.iter().position(|k| k.0 > last + 1e-9).unwrap_or(keys.len() - 1);
    keys.truncate(follow + 1);
    let start = keys[follow].0 + 0.06;
    let end = 0.93;
    // Up out of the follow-through, the hand circling to whirl the lash round, and easing back
    // down to rest as it slows
    let (rest_hand, rest_along) = (Vector3::from(LASH_REST.0), Vector3::from(LASH_REST.1));
    let loops = (turns * 5.0) as usize;
    for j in 0..=loops {
        let u = j as f64 / loops as f64;
        let a = u * turns * TAU;
        let lift = (PI * u).sin();
        let settle = smooth((u - 0.5) / 0.5);
        let hand = Vector3::new(0.26, 0.5, -0.1).lerp(&rest_hand, settle) + Vector3::new(0.05 * a.cos(), 0.03, 0.12 + 0.05 * a.sin()) * lift;
        let along = Vector3::new(0.15 * a.cos() * lift, 0.55, 0.82).lerp(&rest_along, settle);
        keys.push((start + (end - start) * u, grip(hand.into(), along.into()), false));
    }
    cracks.push(Crack { from: start, at: end, wind: Wind::Twirl(turns), hold: 0.0, sweep: 0.0 });
    LashSwing { keys, cracks, comes, lands, second, stretch: stretch * coiled }
}

/// How a whip's held at rest, its hand and which way its handle points, as target_pose() has it
const LASH_REST: ([f64; 3], [f64; 3]) = ([0.3, 0.45, -0.21], [-0.1, 0.9, 0.3]);

/// From which grade a weapon's blows trail a glow of its rarity's color, see Combat::flourish(),
/// and from which its combo chains are given a flourish, see twirl()
const TRAIL_GRADE: usize = 3;
const FLOURISH_GRADE: usize = 4;

/// A glint of an epic or rarer weapon's trail, see TRAIL_GRADE: a fine spark a few times as big,
/// hanging where the blade passed and dwindling, so a row of them reads as a glowing arc
struct Glint {
    position: Vector3<f64>,
    look: usize,
    life: f64,
}

/// Seconds a glint lasts, how much bigger than a fine spark it starts, and the most there are at
/// once: a blade's length of arc behind the tip and further down it
const GLINT_LIFE: f64 = 0.18;
const GLINT_SIZE: f64 = 2.0;
const MOST_GLINTS: usize = 44;

/// The looks of the two elements whose sparks come nearest `color`, and how much of the time the
/// first is to be used, so a trail of them comes out about that color: an epic weapon's purple is
/// void's, a legendary one's orange fire's, a mythic one's pink between blood's and arcane's
fn rarity_looks(color: [u8; 3]) -> (usize, usize, f64) {
    let apart = |e: Element| {
        let c = e.color();
        (0..3).map(|i| (c[i] as f64 - color[i] as f64).powi(2)).sum::<f64>().sqrt()
    };
    let mut near: Vec<(f64, Element)> = Element::ALL.iter().map(|&e| (apart(e), e)).collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    let ((d0, e0), (d1, e1)) = (near[0], near[1]);
    // Close enough to the nearest, that alone
    if d0 < 30.0 {
        return (Element::look(Some(e0)), Element::look(Some(e0)), 1.0);
    }
    (Element::look(Some(e0)), Element::look(Some(e1)), d1 / (d0 + d1))
}

/// How far round its own length a weapon of `grade` is twirled in the hand, `f` of the way through
/// light blow `combo` of a chain, swung in `style` and landing at `lands`, in radians: from
/// legendary its finisher's twirled once round as it follows through, and from mythic each blow
/// is, the finisher twice. It's a whole number of turns, done before the blow's over. Also how
/// far the hand's lifted for it, 0-1, up and back down again as it goes.
fn twirl(grade: usize, style: Style, heavy: bool, combo: u8, lands: f64, f: f64) -> (f64, f64) {
    if heavy || grade < FLOURISH_GRADE || matches!(style, Style::Lash | Style::Whirl) {
        return (0.0, 0.0);
    }
    let last = combo == finisher(style);
    let turns = match (grade > FLOURISH_GRADE, last) {
        (true, true) => 2.0,
        (true, false) | (false, true) => 1.0,
        (false, false) => return (0.0, 0.0),
    };
    let from = lands + 0.04;
    let to = from + 0.55 * (0.97 - from);
    let u = ((f - from) / (to - from)).clamp(0.0, 1.0);
    (TAU * turns * u * u * u * (u * (u * 6.0 - 15.0) + 10.0), (PI * u).sin().powi(2))
}

/// When a swing in `style` starts coming down, and when it lands, as parts of the whole swing.
/// `combo` is which blow of a combo it is.
fn timing(style: Style, heavy: bool, combo: u8) -> (f64, f64) {
    // The third blow of a blade's chain, a new way again
    if combo == 2 && !heavy && finisher(style) > 2 {
        return match style {
            Style::Slash => (0.3, 0.46),
            Style::Stab => (0.28, 0.42),
            Style::Thrust => (0.3, 0.45),
            Style::Smash => (0.34, 0.5),
            _ => (0.32, 0.48),
        };
    }
    if combo == finisher(style) && !heavy {
        return match style {
            Style::Slash => (0.3, 0.45),
            Style::Smash => (0.45, 0.6),
            Style::Stab => (0.35, 0.5),
            Style::Thrust => (0.3, 0.47),
            // Time for the lash to go round in a loop overhead first
            Style::Lash => (0.5, 0.6),
            Style::Whirl => (0.3, 0.5),
            _ => (0.4, 0.52),
        };
    }
    match (style, heavy) {
        // A flail whirled round lands as its head comes round in front, or is brought down
        // overhead
        (Style::Whirl, false) => (0.3, 0.5),
        (Style::Whirl, true) => (0.45, 0.6),
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
    if style == Style::Whirl {
        return whirl_keys(heavy, combo);
    }
    let (comes, lands) = timing(style, heavy, combo);
    let flip = combo == 1;
    let third = combo == 2 && !heavy && finisher(style) > 2;
    let finisher = combo == finisher(style) && !heavy;
    // When the in-between key of a blow coming round in an arc is reached
    let midway = comes + 0.55 * (lands - comes);
    // A blow wound up at `wound`, struck through `strike` as it lands, carrying on to `end` and
    // slowing to `after`: an arc pushed out by `push` on the way in, so it sweeps rather than
    // cutting straight across
    let arc = |wound: Pose, push: [f64; 3], strike: Pose, end: Pose, after: Pose| {
        vec![(comes, wound.clone(), false), (midway, through(&wound, &strike, 0.5, push), false), (lands, strike, true), (lands + 0.12, end, false), (lands + 0.24, after, false)]
    };
    // Wound up only as far as the edge of the screen, so the weapon stays in sight.
    // Raised high and brought straight down: the overhead blow of a heavy sword or axe, gathered
    // a moment first, coming over the top and sinking after it lands
    let overhead = || {
        let raised = pose([0.22, 0.33, 0.0], [0.15, 0.1, 0.98], [0.0, 0.98, -0.12]);
        let strike = pose([0.1, 0.58, -0.22], [0.04, 0.99, -0.05], [0.0, -0.05, -1.0]);
        vec![
            (comes * 0.35, pose([0.31, 0.42, -0.31], [-0.02, 0.55, 0.83], [0.0, 1.0, 0.0]), false),
            (comes, raised.clone(), false),
            (midway, through(&raised, &strike, 0.5, [0.0, 0.03, 0.07]), false),
            (lands, strike, true),
            (lands + 0.1, pose([0.06, 0.55, -0.36], [0.0, 0.95, -0.3], [0.0, -0.3, -0.95]), false),
            (lands + 0.22, pose([0.07, 0.5, -0.4], [0.0, 0.85, -0.52], [0.0, -0.52, -0.85]), false),
        ]
    };
    // A sweep from the right across to the left, the edge leading
    let sweep = || {
        arc(
            pose([0.22, 0.36, -0.16], [0.75, 0.35, 0.55], [-0.3, 1.0, 0.0]),
            [0.04, 0.07, 0.0],
            pose([0.16, 0.55, -0.2], [0.3, 0.93, 0.15], [-1.0, 0.3, 0.0]),
            pose([-0.1, 0.5, -0.28], [-0.8, 0.55, 0.05], [-0.55, -0.8, 0.0]),
            pose([-0.15, 0.44, -0.33], [-0.9, 0.38, -0.12], [-0.38, -0.92, 0.0]),
        )
    };
    // A backhand sweep, from the left across to the right, the edge leading
    let backhand = || {
        arc(
            pose([-0.06, 0.36, -0.14], [-0.78, 0.32, 0.52], [0.3, 1.0, 0.0]),
            [-0.04, 0.07, 0.0],
            pose([0.14, 0.56, -0.2], [0.25, 0.94, 0.12], [1.0, 0.25, 0.0]),
            pose([0.38, 0.5, -0.27], [0.84, 0.52, 0.0], [0.55, -0.8, 0.0]),
            pose([0.42, 0.44, -0.33], [0.9, 0.36, -0.15], [0.38, -0.92, 0.0]),
        )
    };
    // Rising from low on the left up through the middle and out high to the right, the edge
    // turned up, then easing back down
    let uppercut = || {
        arc(
            pose([0.02, 0.38, -0.44], [-0.45, 0.55, -0.7], [0.25, 0.4, 0.88]),
            [0.0, 0.06, -0.04],
            pose([0.14, 0.6, -0.16], [0.2, 0.92, 0.35], [0.15, -0.35, 0.92]),
            pose([0.34, 0.5, 0.02], [0.55, 0.45, 0.7], [0.0, -0.84, 0.55]),
            pose([0.37, 0.46, -0.04], [0.6, 0.35, 0.72], [0.0, -0.9, 0.45]),
        )
    };
    // The third blow of a blade's chain: level across for a sword, rising for an axe or a scythe,
    // backhanded for a hammer, a quick cut across for a dagger, and a spear driven down from high
    if third {
        return match style {
            Style::Slash | Style::Stab => sweep(),
            Style::Smash => backhand(),
            Style::Thrust => {
                let back = pose([0.3, 0.14, -0.06], [-0.04, 0.95, 0.3], [-1.0, 0.0, 0.0]);
                let lunge = pose([0.12, 0.7, -0.07], [-0.1, 0.98, -0.14], [-1.0, 0.0, 0.0]);
                let towards = (back.hand - lunge.hand) * 0.1;
                let mut keys = vec![(comes * 0.5, pose([0.34, 0.24, -0.2], [0.02, 0.95, 0.3], [-1.0, 0.0, 0.0]), false), (comes, back, false)];
                keys.extend(lunge_keys(lands, lunge, 0.15, [towards.x, towards.y, towards.z]));
                keys
            }
            _ => uppercut(),
        };
    }
    if finisher {
        return match style {
            // Drawn back to the hip and driven straight in
            Style::Slash => {
                let lunge = pose([0.08, 0.72, -0.15], [-0.1, 0.99, 0.1], [-1.0, 0.0, 0.0]);
                let mut keys = vec![(comes, pose([0.3, 0.2, -0.3], [0.05, 0.95, 0.3], [-1.0, 0.0, 0.0]), false)];
                keys.extend(lunge_keys(lands, lunge, 0.13, [0.0, -0.04, 0.0]));
                keys
            }
            // Raised high and slammed down, as a heavy blow
            Style::Smash | Style::Stab => swing(style, true, 0),
            // A wide sweep of the spear
            Style::Thrust => sweep(),
            _ => overhead(),
        };
    }
    match (style, heavy) {
        // The backhand, from the upper left down to the right
        (Style::Slash, false) if flip => arc(
            pose([-0.02, 0.36, -0.06], [-0.45, 0.3, 0.84], [0.83, 0.2, 0.55]),
            [0.0, 0.06, 0.03],
            pose([0.18, 0.56, -0.22], [0.22, 0.95, 0.1], [0.72, -0.1, -0.68]),
            pose([0.36, 0.52, -0.36], [0.8, 0.5, -0.3], [0.35, -0.1, -0.93]),
            pose([0.35, 0.48, -0.38], [0.8, 0.42, -0.42], [0.35, -0.2, -0.92]),
        ),
        // The forehand, from the upper right down to the left, wound up where it can still be seen
        (Style::Slash, false) => arc(
            pose([0.26, 0.36, -0.07], [0.38, 0.28, 0.88], [-0.86, 0.2, 0.45]),
            [0.0, 0.06, 0.03],
            pose([0.1, 0.56, -0.22], [-0.2, 0.95, 0.12], [-0.72, -0.05, -0.7]),
            pose([-0.12, 0.52, -0.38], [-0.8, 0.5, -0.3], [-0.35, -0.1, -0.93]),
            pose([-0.16, 0.46, -0.42], [-0.86, 0.33, -0.4], [-0.3, -0.2, -0.93]),
        ),
        // Cleaving down at a slant
        (Style::Chop, false) if flip => arc(
            pose([0.24, 0.36, -0.04], [0.3, 0.25, 0.92], [0.0, 1.0, -0.3]),
            [0.0, 0.05, 0.06],
            pose([0.0, 0.6, -0.3], [-0.4, 0.75, -0.5], [-0.2, 0.2, -1.0]),
            pose([-0.05, 0.5, -0.4], [-0.5, 0.4, -0.75], [-0.2, 0.2, -1.0]),
            pose([-0.06, 0.46, -0.43], [-0.5, 0.33, -0.8], [-0.2, 0.25, -0.95]),
        ),
        (Style::Chop, false) => sweep(),
        // The blade kept pointing across to the left, the pole swept round from the right, then
        // back the other way, its point leading
        (Style::Reap, false) if flip => arc(
            pose([-0.05, 0.36, -0.12], [-0.6, 0.35, 0.72], [1.0, 0.2, 0.0]),
            [-0.03, 0.06, 0.0],
            pose([0.16, 0.55, -0.25], [0.0, 0.95, 0.3], [1.0, 0.1, 0.0]),
            pose([0.34, 0.45, -0.3], [0.7, 0.6, 0.2], [0.6, -0.8, 0.0]),
            pose([0.38, 0.4, -0.33], [0.82, 0.48, 0.1], [0.5, -0.86, 0.0]),
        ),
        (Style::Reap, false) => arc(
            pose([0.3, 0.32, -0.12], [0.6, 0.35, 0.72], [-1.0, 0.2, 0.0]),
            [0.03, 0.06, 0.0],
            pose([0.12, 0.55, -0.25], [0.1, 0.95, 0.3], [-1.0, 0.1, 0.0]),
            pose([-0.1, 0.48, -0.3], [-0.7, 0.6, 0.2], [-0.6, -0.8, 0.0]),
            pose([-0.14, 0.43, -0.33], [-0.82, 0.48, 0.1], [-0.5, -0.86, 0.0]),
        ),
        (Style::Smash, false) if flip => sweep(),
        (Style::Smash, _) => {
            // Gathered, heaved up over the shoulder and brought over the top and down hard,
            // driving on into what it hits, then easing back up a little and staying down a moment
            let (raised, struck, bitten, settled) = if heavy {
                (
                    pose([0.2, 0.32, -0.04], [0.1, 0.05, 0.99], [0.0, 1.0, -0.05]),
                    pose([0.06, 0.6, -0.3], [0.0, 0.85, -0.5], [0.0, -0.5, -0.85]),
                    pose([0.06, 0.61, -0.37], [0.0, 0.78, -0.62], [0.0, -0.62, -0.78]),
                    pose([0.06, 0.58, -0.35], [0.0, 0.81, -0.58], [0.0, -0.58, -0.81]),
                )
            } else {
                (
                    pose([0.22, 0.36, -0.1], [0.12, 0.2, 0.97], [0.0, 1.0, -0.2]),
                    pose([0.1, 0.6, -0.3], [0.0, 0.8, -0.6], [0.0, -0.6, -0.8]),
                    pose([0.1, 0.61, -0.37], [0.0, 0.72, -0.7], [0.0, -0.7, -0.72]),
                    pose([0.1, 0.58, -0.35], [0.0, 0.76, -0.65], [0.0, -0.65, -0.76]),
                )
            };
            vec![
                (comes * 0.4, pose([0.31, 0.43, -0.32], [-0.02, 0.58, 0.81], [0.0, 1.0, 0.0]), false),
                (comes, raised.clone(), false),
                (midway, through(&raised, &struck, 0.5, [0.0, 0.02, 0.09]), false),
                (lands, struck, true),
                (lands + 0.05, bitten, false),
                (lands + 0.2, settled, false),
            ]
        }
        // A dagger's second blow cut backhanded across
        (Style::Stab, false) if flip => backhand(),
        (Style::Stab, false) => {
            // Drawn back and driven in
            let lunge = if flip { pose([0.14, 0.7, -0.24], [-0.25, 0.95, 0.2], [0.0, 0.0, 1.0]) } else { pose([0.08, 0.72, -0.16], [-0.1, 0.98, 0.15], [0.0, 0.0, 1.0]) };
            let mut keys = vec![(comes, pose([0.22, 0.3, -0.26], [-0.1, 0.9, 0.4], [0.0, 0.0, 1.0]), false)];
            keys.extend(lunge_keys(lands, lunge, 0.13, [0.0, -0.03, 0.0]));
            keys
        }
        (Style::Stab, true) => {
            // Raised point down and plunged in, over the top
            let raised = pose([0.2, 0.32, 0.02], [-0.05, 0.6, 0.8], [0.0, 1.0, 0.0]);
            let plunge = pose([0.06, 0.62, -0.34], [-0.05, 0.7, -0.7], [0.0, -0.7, -0.7]);
            let mut keys = vec![(comes, raised.clone(), false), (midway, through(&raised, &plunge, 0.5, [0.0, 0.04, 0.06]), false)];
            keys.extend(lunge_keys(lands, plunge, 0.13, [0.0, 0.0, -0.03]));
            keys
        }
        (Style::Thrust, _) => {
            // Drawn back along the shaft and lunged on at the crosshair, the second time from low
            // at the hip, rising
            let (back, lunge) = if heavy {
                (pose([0.32, 0.05, -0.25], [0.05, 0.95, 0.3], [-1.0, 0.0, 0.0]), pose([0.12, 0.75, -0.2], [-0.1, 0.99, 0.0], [-1.0, 0.0, 0.0]))
            } else if flip {
                (pose([0.3, 0.1, -0.42], [0.02, 0.98, -0.12], [-1.0, 0.0, 0.0]), pose([0.17, 0.64, -0.32], [-0.14, 0.93, 0.34], [-1.0, 0.0, 0.0]))
            } else {
                (pose([0.3, 0.12, -0.3], [0.0, 0.97, 0.2], [-1.0, 0.0, 0.0]), pose([0.16, 0.62, -0.22], [-0.12, 0.99, 0.04], [-1.0, 0.0, 0.0]))
            };
            let towards = (back.hand - lunge.hand) * 0.1;
            let mut keys = vec![(comes, back, false)];
            keys.extend(lunge_keys(lands, lunge, 0.15, [towards.x, towards.y, towards.z]));
            keys
        }
        _ => overhead(),
    }
}

/// A flail's swing, for swing(). A light blow whirls it right round: wound back out to the right
/// and up, swept round through in front as it lands, on out to the left and round behind, then
/// brought back up from below. The second goes the other way round. A heavy one's raised and
/// brought down overhead, as a hammer's is.
fn whirl_keys(heavy: bool, combo: u8) -> Vec<(f64, Pose, bool)> {
    if heavy {
        return swing(Style::Smash, true, 0);
    }
    let (comes, lands) = timing(Style::Whirl, false, combo);
    let flip = |p: Pose| if combo == 1 { mirrored(&p) } else { p };
    let wound = flip(pose([0.42, 0.28, -0.12], [0.75, -0.25, 0.6], [0.3, 1.0, 0.0]));
    let right = flip(pose([0.5, 0.36, -0.22], [0.92, 0.3, 0.2], [-0.3, 1.0, 0.0]));
    let middle = flip(pose([0.05, 0.62, -0.22], [0.08, 0.99, 0.08], [-1.0, 0.1, 0.0]));
    let left = flip(pose([-0.55, 0.36, -0.28], [-0.92, 0.32, 0.1], [-0.3, -1.0, 0.0]));
    let behind = flip(pose([-0.7, 0.12, -0.34], [-0.85, -0.45, 0.1], [0.4, -1.0, 0.0]));
    let below = flip(pose([0.32, 0.3, -0.7], [0.0, 0.5, -0.85], [1.0, 0.0, 0.0]));
    vec![
        (comes * 0.45, wound, false),
        (comes, right, false),
        (lands, middle, true),
        (lands + 0.13, left, false),
        (lands + 0.22, behind, false),
        // Gone round behind, out of sight, and back up from below
        (lands + 0.23, below, false),
    ]
}

/// The poses of a heavy blow that goes its own way, `move_`, for a weapon swung in `style`, for
/// animate(): each gathered a moment before it goes, so it reads as heavy, and carried on through
/// after it lands rather than stopping dead
fn heavy_keys(style: Style, move_: Heavy) -> Vec<(f64, Pose, bool)> {
    let (comes, lands) = blow_timing(style, move_, Finish::Style, true, 0);
    let midway = comes + 0.55 * (lands - comes);
    match move_ {
        // Drawn right back past the hip, the point low, held there a moment as the weight
        // settles, then driven out on a long step, the whole body behind it
        Heavy::Lunge => {
            let back = pose([0.36, 0.0, -0.36], [0.06, 0.95, 0.3], [-1.0, 0.0, 0.0]);
            let mut held = back.clone();
            held.hand += Vector3::new(0.01, -0.02, -0.01);
            let lunge = pose([0.1, 0.8, -0.18], [-0.1, 0.99, 0.04], [-1.0, 0.0, 0.0]);
            let towards = (held.hand - lunge.hand) * 0.1;
            let mut keys = vec![(comes * 0.4, pose([0.36, 0.2, -0.32], [0.04, 0.92, 0.38], [-1.0, 0.0, 0.0]), false), (comes * 0.75, back, false), (comes, held, false)];
            keys.extend(lunge_keys(lands, lunge, 0.2, [towards.x, towards.y, towards.z]));
            keys
        }
        // Drawn low across the left hip, the blade pointing back, held, then cut out level across
        // to the right in one fast stroke, a flick of the blade after
        Heavy::Draw => {
            let sheathed = pose([-0.02, 0.38, -0.36], [-0.7, -0.35, -0.2], [0.0, 0.25, 0.95]);
            let drawn = pose([0.0, 0.46, -0.3], [-0.55, 0.8, -0.1], [0.2, 0.15, 0.95]);
            let cut = pose([0.16, 0.6, -0.22], [0.35, 0.93, 0.0], [0.95, -0.3, 0.0]);
            vec![
                (comes * 0.45, sheathed.clone(), false),
                (comes, through(&sheathed, &drawn, 0.2, [0.0, -0.01, -0.01]), false),
                (midway, drawn, false),
                (lands, cut, true),
                (lands + 0.09, pose([0.4, 0.5, -0.22], [0.92, 0.3, 0.05], [0.3, -0.95, 0.0]), false),
                (lands + 0.2, pose([0.43, 0.46, -0.28], [0.88, 0.28, -0.3], [0.28, -0.9, 0.2]), false),
                // The blood flicked off
                (lands + 0.3, pose([0.38, 0.44, -0.36], [0.6, 0.45, -0.66], [0.35, -0.85, 0.3]), true),
            ]
        }
        // Heaved round on a full turn: wound far back to the right, swept round through in front,
        // on out to the left and round behind, and back in from the right
        Heavy::Spin => {
            let wound = pose([0.46, 0.22, -0.18], [0.85, -0.35, 0.4], [0.35, 1.0, 0.0]);
            let right = pose([0.52, 0.38, -0.24], [0.94, 0.3, 0.15], [-0.3, 1.0, 0.0]);
            let middle = pose([0.04, 0.64, -0.24], [0.08, 0.99, 0.06], [-1.0, 0.1, 0.0]);
            let left = pose([-0.56, 0.36, -0.28], [-0.92, 0.32, 0.08], [-0.3, -1.0, 0.0]);
            vec![
                (comes * 0.4, pose([0.4, 0.3, -0.3], [0.6, 0.1, 0.78], [0.2, 1.0, 0.0]), false),
                (comes * 0.85, wound, false),
                (comes, right, false),
                (lands, middle, true),
                (lands + 0.12, left, false),
                (lands + 0.22, pose([-0.68, 0.12, -0.34], [-0.85, -0.45, 0.1], [0.4, -1.0, 0.0]), false),
                (lands + 0.23, pose([0.36, 0.3, -0.66], [0.0, 0.5, -0.85], [1.0, 0.0, 0.0]), false),
            ]
        }
        // Hauled back over the right shoulder, then hewn down at a slant to the lower left, biting
        // in and settling there
        Heavy::Hew => {
            let raised = pose([0.3, 0.26, 0.06], [0.42, -0.25, 0.87], [-0.2, 0.95, 0.25]);
            let strike = pose([0.02, 0.6, -0.3], [-0.5, 0.72, -0.48], [-0.45, 0.25, -0.86]);
            vec![
                (comes * 0.35, pose([0.32, 0.42, -0.32], [0.1, 0.6, 0.79], [-0.2, 0.95, 0.2]), false),
                (comes, raised.clone(), false),
                (midway, through(&raised, &strike, 0.5, [0.04, 0.04, 0.07]), false),
                (lands, strike, true),
                (lands + 0.06, pose([-0.04, 0.6, -0.38], [-0.55, 0.62, -0.56], [-0.45, 0.2, -0.87]), false),
                (lands + 0.22, pose([-0.03, 0.56, -0.38], [-0.52, 0.6, -0.6], [-0.45, 0.25, -0.86]), false),
            ]
        }
        // Wound far out to the right and low, then swept right round in front at knee height
        Heavy::Sweep => {
            let mut keys = swing(Style::Reap, false, 0);
            for (k, key) in keys.iter_mut().enumerate() {
                key.0 = [comes * 0.8, comes + 0.55 * (lands - comes), lands, lands + 0.13, lands + 0.27].get(k).copied().unwrap_or(key.0);
                key.1.hand.x *= 1.25;
                key.1.hand.z -= 0.1;
                key.1.turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.25) * key.1.turn;
            }
            keys.insert(0, (comes * 0.35, pose([0.4, 0.34, -0.38], [0.75, 0.3, 0.58], [-1.0, 0.2, 0.0]), false));
            keys
        }
        // Twirled round in front of the face a turn and a half, then brought down on its blade
        Heavy::Twirl => {
            let mut keys = Vec::new();
            let turns = 10;
            for k in 0..turns {
                let a = k as f64 / 8.0 * TAU + 0.6;
                let at = 0.05 + (comes * 0.72 - 0.05) * k as f64 / (turns - 1) as f64;
                keys.push((at, pose([0.12, 0.5, -0.14], [a.cos() * 0.95, 0.3, a.sin() * 0.95], [0.0, 1.0, 0.0]), false));
            }
            let raised = pose([0.2, 0.32, 0.04], [0.1, 0.05, 0.99], [0.0, 1.0, -0.05]);
            let struck = pose([0.06, 0.6, -0.3], [0.0, 0.85, -0.5], [0.0, -0.5, -0.85]);
            // Caught at the top of the last turn and gathered a moment before it comes down
            let mut gathered = raised.clone();
            gathered.hand += Vector3::new(0.02, 0.02, -0.04);
            keys.push((comes * 0.86, gathered, false));
            keys.push((comes, raised.clone(), false));
            keys.push((midway, through(&raised, &struck, 0.5, [0.0, 0.02, 0.09]), false));
            keys.push((lands, struck, true));
            // Driving on into what it hits, then easing back up a little
            keys.push((lands + 0.05, pose([0.06, 0.61, -0.37], [0.0, 0.78, -0.62], [0.0, -0.62, -0.78]), false));
            keys.push((lands + 0.2, pose([0.06, 0.58, -0.35], [0.0, 0.81, -0.58], [0.0, -0.58, -0.81]), false));
            keys
        }
        // Raised high and brought down on its axe blade, then wrenched back by its hook
        Heavy::Cleave => {
            let raised = pose([0.22, 0.3, 0.08], [0.1, -0.1, 0.99], [0.0, 1.0, 0.1]);
            let struck = pose([0.08, 0.66, -0.3], [0.0, 0.8, -0.6], [0.0, -0.6, -0.8]);
            vec![
                (comes * 0.4, pose([0.32, 0.36, -0.3], [0.0, 0.6, 0.8], [0.0, 1.0, -0.2]), false),
                (comes, raised.clone(), false),
                (midway, through(&raised, &struck, 0.5, [0.0, 0.03, 0.08]), false),
                (lands, struck, true),
                (lands + 0.05, pose([0.08, 0.67, -0.38], [0.0, 0.74, -0.67], [0.0, -0.67, -0.74]), false),
                // Hooked and hauled back in
                (lands + 0.22, pose([0.16, 0.36, -0.32], [0.0, 0.96, -0.15], [0.0, -0.1, -1.0]), true),
                (lands + 0.32, pose([0.2, 0.36, -0.32], [0.0, 0.94, 0.1], [0.0, 0.1, -1.0]), false),
            ]
        }
        // Set low, heaved right up over the head and held there a moment, then brought down with
        // everything behind it, driving on into the ground and staying down
        Heavy::Heave => {
            let top = pose([0.16, 0.22, 0.14], [0.05, -0.5, 0.86], [0.0, 0.86, 0.5]);
            let mut held = top.clone();
            held.hand.z += 0.025;
            held.hand.y -= 0.02;
            let struck = pose([0.05, 0.62, -0.32], [0.0, 0.84, -0.54], [0.0, -0.54, -0.84]);
            vec![
                (comes * 0.25, pose([0.3, 0.42, -0.44], [0.05, 0.85, -0.52], [0.0, 0.5, 0.85]), false),
                (comes * 0.7, top, false),
                (comes, held.clone(), false),
                (midway, through(&held, &struck, 0.5, [0.0, 0.02, 0.08]), false),
                (lands, struck, true),
                (lands + 0.05, pose([0.05, 0.64, -0.46], [0.0, 0.7, -0.71], [0.0, -0.71, -0.7]), false),
                (lands + 0.22, pose([0.05, 0.6, -0.42], [0.0, 0.76, -0.65], [0.0, -0.65, -0.76]), false),
            ]
        }
        // Whirled round over the head a turn and a bit, then brought down in front
        Heavy::Overhead => {
            let mut keys = Vec::new();
            for k in 0..10 {
                let a = k as f64 / 8.0 * TAU - 0.4;
                let at = 0.06 + (comes - 0.08) * k as f64 / 9.0;
                keys.push((at, pose([0.2, 0.4, 0.06], [a.cos() * 0.85, a.sin() * 0.85, 0.5], [-a.sin(), a.cos(), 0.0]), false));
            }
            let raised = pose([0.2, 0.32, 0.06], [0.1, 0.0, 0.99], [0.0, 1.0, 0.0]);
            let struck = pose([0.06, 0.6, -0.32], [0.0, 0.82, -0.57], [0.0, -0.57, -0.82]);
            keys.push((comes, raised.clone(), false));
            keys.push((midway, through(&raised, &struck, 0.5, [0.0, 0.03, 0.1]), false));
            keys.push((lands, struck, true));
            keys.push((lands + 0.05, pose([0.06, 0.61, -0.4], [0.0, 0.76, -0.65], [0.0, -0.65, -0.76]), false));
            keys.push((lands + 0.22, pose([0.07, 0.56, -0.38], [0.0, 0.78, -0.62], [0.0, -0.62, -0.78]), false));
            keys
        }
        Heavy::Style => swing(style, true, 0),
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
const LASH_SECOND: f64 = 0.78;
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

/// A whip's handle held in the hand at `hand`, pointing `along`, for lash_keys(). It's round, so
/// which way round it's turned hardly shows: it's kept turned the same way as near as can be.
fn grip(hand: [f64; 3], along: [f64; 3]) -> Pose {
    let a = Vector3::from(along).normalize();
    let edge = a.cross(&Vector3::x()).try_normalize(0.2).unwrap_or_else(|| a.cross(&Vector3::y()));
    pose(hand, along, [edge.x, edge.y, edge.z])
}

/// The poses a whip's handle goes through in a swing, for animate(), by `combo` as for swing(),
/// and by its `grade` as what it can do grows with it, see LASH_LINE and the rest. The lash itself
/// is a rope that follows the hand, steered as lash_cracks() says, see Combat::swing_lash().
///
/// A light blow flicks it out sideways: wound back out to the side, swept round in front and
/// snapped across, the hand checking as it cracks, then following through across the body. The
/// second goes back the other way. A heavy blow dips, raises the whip high so the lash falls back
/// behind, and brings it over the top to crack forward and down; from epic it's cracked out level,
/// held a moment as it catches, and yanked back in. A finisher whirls the lash round in a loop
/// over the head and cracks it out of the loop, and from rare flicks it up and cracks it again.
fn lash_keys(heavy: bool, combo: u8, grade: usize) -> Vec<(f64, Pose, bool)> {
    let (comes, lands) = timing(Style::Lash, heavy, combo);
    let finisher = combo == FINISHER && !heavy;
    // Cracked out at the crosshair, the hand checked
    let cracked = grip([0.16, 0.65, -0.15], [-0.1, 0.97, 0.15]);
    if heavy {
        let mut keys = vec![
            // Dipping a moment before it rises
            (comes * 0.3, grip([0.3, 0.5, -0.26], [-0.05, 0.95, -0.25]), false),
            (comes * 0.7, grip([0.25, 0.36, 0.04], [0.05, -0.1, 1.0]), false),
            // Raised high, the lash falling back behind
            (comes, grip([0.21, 0.3, 0.13], [0.05, -0.7, 0.72]), false),
            (comes + 0.55 * (lands - comes), grip([0.18, 0.48, 0.07], [-0.02, 0.45, 0.9]), false),
        ];
        if grade >= LASH_SNARE {
            // Cracked out level, caught, and yanked back in
            keys.extend([
                (lands, grip([0.14, 0.68, -0.15], [-0.12, 0.97, 0.1]), true),
                (lands + 0.07, grip([0.14, 0.66, -0.16], [-0.12, 0.97, 0.06]), false),
                (lands + 0.2, grip([0.3, 0.36, -0.22], [0.0, 0.5, 0.86]), true),
                (lands + 0.34, grip([0.3, 0.42, -0.24], [-0.05, 0.85, 0.3]), false),
            ]);
        } else {
            // Cracked forward and down, following through low
            keys.extend([
                (lands, grip([0.15, 0.67, -0.13], [-0.1, 0.96, -0.15]), true),
                (lands + 0.12, grip([0.15, 0.6, -0.3], [-0.05, 0.72, -0.68]), false),
                (lands + 0.28, grip([0.24, 0.5, -0.26], [-0.06, 0.9, -0.4]), false),
            ]);
        }
        return keys;
    }
    if finisher {
        // The hand circling over the head, the lash swinging round after it, then cracked out of
        // the loop
        // The handle leans out the way the hand's going round, rather than straight up its arm
        let mut keys = vec![
            (0.1, grip([0.36, 0.48, 0.02], [0.85, 0.15, 0.5]), false),
            (0.2, grip([0.29, 0.56, 0.08], [0.15, 0.85, 0.5]), false),
            (0.3, grip([0.22, 0.48, 0.1], [-0.8, 0.25, 0.5]), false),
            (0.4, grip([0.29, 0.41, 0.1], [0.1, -0.8, 0.55]), false),
            (comes, grip([0.36, 0.47, 0.06], [0.85, -0.1, 0.5]), false),
            (lands, cracked.clone(), true),
        ];
        if grade >= LASH_DOUBLE {
            // Flicked up and cracked again, forward and down
            keys.extend([
                (lands + 0.09, grip([0.16, 0.61, -0.07], [-0.06, 0.6, 0.8]), false),
                (LASH_SECOND, grip([0.13, 0.67, -0.16], [-0.1, 0.96, -0.1]), true),
                (LASH_SECOND + 0.11, grip([0.14, 0.58, -0.3], [-0.05, 0.7, -0.7]), false),
            ]);
        } else {
            keys.extend([(lands + 0.12, grip([0.02, 0.57, -0.25], [-0.55, 0.75, -0.3]), false), (lands + 0.26, grip([0.14, 0.5, -0.26], [-0.25, 0.92, -0.25]), false)]);
        }
        return keys;
    }
    // A light blow, flicked across from out at one side, the forehand from the right and then the
    // backhand from the left
    let side = if combo == 1 { -1.0 } else { 1.0 };
    // Where the hand is across at each key: the forehand winds out to the right and follows
    // through to the left, the backhand winds across the body and follows through out right
    let x = if combo == 1 { [0.16, 0.06, 0.1, 0.22, 0.38, 0.32] } else { [0.36, 0.38, 0.33, 0.17, 0.04, 0.14] };
    vec![
        (comes * 0.45, grip([x[0], 0.42, -0.12], [side * 0.6, 0.35, 0.72]), false),
        // Wound back out to the side, still in sight
        (comes, grip([x[1], 0.38, -0.07], [side * 0.85, -0.25, 0.46]), false),
        // Swept round in front
        (comes + 0.6 * (lands - comes), grip([x[2], 0.56, -0.09], [side * 0.5, 0.82, 0.25]), false),
        // Snapped across, the hand checking
        (lands, grip([x[3], 0.64, -0.14], [side * -0.08, 0.97, 0.18]), true),
        // Following through across the body
        (lands + 0.1, grip([x[4], 0.56, -0.24], [side * -0.6, 0.72, -0.3]), false),
        (lands + 0.28, grip([x[5], 0.5, -0.25], [side * -0.25, 0.92, -0.25]), false),
    ]
}

/// How a whip's lash is wound back before a crack, as lash_cracks() has it
#[derive(Clone, Copy, PartialEq, Debug)]
enum Wind {
    /// Trailing back that way from the hand, in the camera's right, forward and up
    Back([f64; 3]),
    /// Whirled round over the head in a loop, coming round to trail back out to the right
    Whirl,
    /// Not a crack: the lash twirled round over the head so many times after the last one, from
    /// `from` to `at` of the way through the swing, see lash_swing()
    Twirl(f64),
    /// Swept right round the player low over the ground, from straight ahead, by the left, as a
    /// trip sweep has it
    Low,
}

/// One crack of a whip's lash in a swing, see Combat::swing_lash(): wound back as `wind` from
/// `from` of the way through the swing, thrown out from the hand so it straightens out, the tip
/// last, at the crosshair at `at`, held out there `hold` seconds, and let go. With `sweep` its
/// aim sweeps across, from the right if it's positive, through a fan.
#[derive(Clone, Copy, Debug)]
struct Crack {
    from: f64,
    at: f64,
    wind: Wind,
    hold: f64,
    sweep: f64,
}

/// How a whip's lash is steered through a swing, crack by crack, to go with lash_keys()
fn lash_cracks(heavy: bool, combo: u8, grade: usize) -> Vec<Crack> {
    let (_, lands) = timing(Style::Lash, heavy, combo);
    let finisher = combo == FINISHER && !heavy;
    // Held out straight just long enough to be seen, or a moment longer by a snare as it catches
    let hold = 0.035;
    if heavy {
        let hold = if grade >= LASH_SNARE { 0.1 } else { hold };
        return vec![Crack { from: 0.18, at: lands, wind: Wind::Back([0.1, -0.55, 0.85]), hold, sweep: 0.0 }];
    }
    if finisher {
        let mut cracks = vec![Crack { from: 0.06, at: lands, wind: Wind::Whirl, hold, sweep: 0.0 }];
        if grade >= LASH_DOUBLE {
            cracks.push(Crack { from: lands + 0.04, at: LASH_SECOND, wind: Wind::Back([0.0, 0.2, 1.0]), hold, sweep: 0.0 });
        }
        return cracks;
    }
    let side = if combo == 1 { -1.0 } else { 1.0 };
    // From legendary a light blow sweeps through a wide fan
    let sweep = if grade >= LASH_FAN { side } else { 0.0 };
    // Wound out to the side and up, where it can be seen, rather than straight back
    vec![Crack { from: 0.06, at: lands, wind: Wind::Back([side * 0.65, 0.4, 0.6]), hold, sweep }]
}

/// How a whip's lash is steered through a special attack, crack by crack, to go with
/// special_move(): flung out at what it's thrown round, cracked down onto the ground, swept round
/// low, or caught on something to swing from. The rest leave the lash to follow the hand.
fn special_cracks(special: Special) -> Vec<Crack> {
    let crack = |from: f64, at: f64, wind: Wind, hold: f64| Crack { from, at, wind, hold, sweep: 0.0 };
    match special {
        Special::LassoPull => vec![crack(0.04, 0.42, Wind::Whirl, 0.22)],
        Special::CoilBind => vec![crack(0.1, 0.36, Wind::Back([0.1, -0.5, 0.86]), 0.3)],
        Special::ThunderCrack => vec![crack(0.14, 0.42, Wind::Back([0.1, -0.55, 0.85]), 0.1)],
        Special::SerpentLash => vec![crack(0.06, 0.36, Wind::Back([0.0, -0.3, 1.0]), 0.2)],
        Special::TripSweep => vec![crack(0.28, 0.7, Wind::Low, 0.0)],
        Special::BurningBrand => vec![crack(0.1, 0.38, Wind::Back([0.05, -0.5, 0.86]), 0.25)],
        Special::ChainSwing => vec![crack(0.02, 0.22, Wind::Back([0.2, -0.4, 0.9]), 0.55), crack(0.7, 0.82, Wind::Back([0.0, -0.5, 0.86]), 0.05)],
        _ => Vec::new(),
    }
}

/// How long a special attack takes with a weapon of `kind` and `style`, and the poses the weapon
/// goes through, for animate()
fn special_move(special: Special, kind: Kind, style: Style) -> (f64, Vec<(f64, Pose, bool)>) {
    // Shackle Chain, Blood Moon and Cluster Volley have their own, see own_move()
    if let Some(own) = own_move(special, kind, style) {
        return own;
    }
    // A bow drawn and loosed, kicking a little each time
    let drawn = pose([-0.1, 0.66, -0.08], [-0.33, 0.02, 0.94], [0.0, 1.0, 0.0]);
    let kicked = |side: f64| {
        let mut p = drawn.clone();
        p.hand.y -= 0.04;
        p.hand.x += 0.03 * side;
        p
    };
    let staff = kind == Kind::Magic;
    // Some go through the moves of others: slamming down to send ice out, whirling as knives fly,
    // darting in to strike, or thrust out and held as a beam pours or lightning spreads
    let special = match special {
        Special::FrostNova | Special::Earthquake | Special::Avalanche => Special::Skyfall,
        Special::BladeStorm | Special::Cyclone => Special::Whirlwind,
        Special::Shatterwave => Special::Riptide,
        Special::BlinkStrike => Special::Flurry,
        Special::SoulSiphon | Special::ThunderWeb | Special::Prism => Special::Stormchain,
        Special::Harvest => Special::Riptide,
        other => other,
    };
    // A crossbow or gun levelled, swung from one to the next and shot at each, kicking each time
    if matches!(style, Style::Crank | Style::Powder) && special == Special::Reckoning {
        let level = pose([0.12, 0.5, -0.17], [0.03, -0.08, 0.99], [-0.1, 1.0, 0.05]);
        let length = 1.45;
        let mut keys = vec![(0.12 / length, level.clone(), false)];
        for k in 0..8 {
            // Shots go at 0.18 + 0.14 k seconds, see unleash()
            let at = (0.18 + 0.14 * k as f64) / length;
            let side = [0.0, -0.05, 0.05, -0.03, 0.06, -0.06, 0.03, 0.0][k];
            let mut aimed = level.clone();
            aimed.hand.x += side;
            aimed.turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), -side * 2.0) * aimed.turn;
            let mut kicked = aimed.clone();
            kicked.hand.y -= 0.035;
            kicked.turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 0.18) * kicked.turn;
            keys.push((at - 0.03 / length, aimed, false));
            keys.push((at, kicked, true));
        }
        return (length, keys);
    }
    // Brought up to the hip and let go point-blank, kicking right back and up, then lowered
    if matches!(style, Style::Crank | Style::Powder) && special == Special::Grapeshot {
        let braced = pose([0.1, 0.42, -0.24], [0.02, -0.05, 1.0], [-0.08, 1.0, 0.05]);
        let mut kicked = braced.clone();
        kicked.hand += Vector3::new(0.03, -0.1, 0.06);
        kicked.turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 0.55) * kicked.turn;
        let mut settled = braced.clone();
        settled.hand.z -= 0.02;
        return (0.9, vec![(0.04, braced.clone(), false), (0.1, kicked, true), (0.4, settled, false), (0.7, braced, false)]);
    }
    // A crossbow or gun levelled at what's ahead, or raised to the sky, and shot, kicking
    if matches!(style, Style::Crank | Style::Powder) {
        let (length, _) = special_move(special, kind, Style::Draw);
        let ahead = matches!(
            special,
            Special::Volley
                | Special::Stormchain
                | Special::Breath
                | Special::Riptide
                | Special::Fissure
                | Special::Ricochet
                | Special::SplitBolt
                | Special::NetBolt
                | Special::Shackle
                | Special::ChainBlast
                | Special::Wildfire
                | Special::RuneMines
                | Special::BallLightning
                | Special::Swarm
                | Special::Thornburst
        );
        // Raised off to the side rather than right across the view, so what's coming down can be seen
        let raised = if ahead { pose([0.14, 0.4, -0.18], [0.04, -0.12, 0.99], [-0.1, 1.0, 0.08]) } else { pose([0.24, 0.4, -0.16], [0.08, -0.55, 0.83], [-0.08, 0.83, 0.55]) };
        let mut fired = raised.clone();
        fired.hand.y -= 0.05;
        fired.turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 0.3) * fired.turn;
        return (length, vec![(0.25, raised.clone(), false), (0.3, fired, true), (0.55, raised, false)]);
    }
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
        // Five quick slashes, whirled round in loops in front rather than hacked back and forth:
        // cutting down to the left through the middle, round beneath, back up to the right
        // through the middle and round over the top, each cut landing as unleash() has it
        Special::Flurry => {
            let length = 0.55;
            let high_right = pose([0.32, 0.42, -0.06], [0.55, 0.45, 0.7], [-0.6, 0.3, 0.3]);
            let cut_down = pose([0.08, 0.58, -0.18], [-0.25, 0.95, 0.15], [-0.7, 0.0, -0.7]);
            let low_left = pose([-0.08, 0.48, -0.34], [-0.75, 0.45, -0.45], [0.55, 0.3, 0.5]);
            let cut_up = pose([0.16, 0.57, -0.28], [0.25, 0.95, -0.1], [0.7, 0.0, 0.7]);
            let mut keys = vec![(0.04 / length, high_right.clone(), false)];
            for k in 1..=5 {
                let (cut, round) = if k % 2 == 1 { (cut_down.clone(), low_left.clone()) } else { (cut_up.clone(), high_right.clone()) };
                keys.push((0.08 * k as f64 / length, cut, true));
                keys.push(((0.08 * k as f64 + 0.04) / length, round, false));
            }
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
        // Raised high overhead and shaken as the roar goes up
        Special::BattleCry => {
            let raised = pose([0.2, 0.36, 0.1], [0.08, 0.2, 0.98], [-1.0, 0.0, 0.0]);
            let mut shaken = raised.clone();
            shaken.hand.z += 0.04;
            shaken.hand.x -= 0.02;
            (1.0, vec![(0.25, raised.clone(), false), (0.45, shaken, false), (0.65, raised, false)])
        }
        // Drawn back over the shoulder and hurled, the empty hand held out till it comes back
        Special::HurledBlade => {
            let back = pose([0.34, 0.22, 0.0], [0.3, -0.35, 0.89], [0.0, 1.0, 0.0]);
            let thrown = pose([0.16, 0.66, -0.16], [-0.1, 0.99, 0.1], [-1.0, 0.0, 0.0]);
            let caught = pose([0.22, 0.52, -0.2], [-0.05, 0.75, 0.66], [-1.0, 0.0, 0.0]);
            (1.4, vec![(0.12, back, false), (0.2, thrown.clone(), true), (0.8, thrown, false), (0.92, caught, false)])
        }
        // Ten arrows, drawn and loosed as fast as they'll go, the bow kicking a little off true
        // each time
        Special::Quickdraw => {
            let length = 1.05;
            let mut keys = vec![(0.06 / length, drawn.clone(), false)];
            for k in 0..10 {
                let at = (0.1 + 0.085 * k as f64) / length;
                keys.push((at, kicked([1.0, -1.0, 0.5, -0.5, 0.0][k % 5]), true));
                keys.push((at + 0.045 / length, drawn.clone(), false));
            }
            (length, keys)
        }
        // The bow raised and swept across what's round about, the mark going on each
        Special::HuntersMark if kind == Kind::Ranged => {
            let left = pose([-0.2, 0.62, 0.0], [-0.45, 0.25, 0.86], [0.35, 0.94, 0.0]);
            let right = pose([0.04, 0.64, 0.0], [-0.2, 0.25, 0.95], [-0.25, 0.97, 0.0]);
            (1.0, vec![(0.2, left, false), (0.55, right, false), (0.75, drawn.clone(), false)])
        }
        // A whip's lash whirled round over the head, three times, cracking out at the end of each
        Special::CoilingLash => {
            let mut keys = Vec::new();
            for k in 0..30 {
                let a = k as f64 / 10.0 * TAU;
                let t = 0.04 + 0.86 * k as f64 / 29.0;
                keys.push((t, grip([0.24 + 0.08 * a.cos(), 0.46 + 0.08 * a.sin(), 0.06], [a.cos() * 0.8, a.sin() * 0.8, 0.6]), k % 10 == 9));
            }
            (1.0, keys)
        }
        // Flicked out again and again, one way then the other, faster and faster
        Special::Flay => {
            let length = 1.2;
            let mut keys = Vec::new();
            // The cracks land as unleash() has them, each wound back halfway from the last
            let (mut at, mut before): (f64, f64) = (0.15, 0.0);
            for k in 0..8 {
                // Flicked less far back the quicker they come
                let side = if k % 2 == 0 { 1.0 } else { -1.0 } * ((at - before) / 0.17).min(1.0).powf(1.5);
                keys.push(((at + before) * 0.5 / length, grip([0.18 + side * 0.08, 0.5 - 0.06 * side.abs(), -0.08 + 0.04 * side.abs()], [side * 0.7, 0.55 - 0.65 * side.abs(), 0.7]), false));
                keys.push((at / length, grip([0.16, 0.65, -0.14], [-side * 0.12, 0.97, 0.15]), k < 2));
                before = at;
                at += 0.17 * 0.88f64.powi(k + 1);
            }
            (length, keys)
        }
        // Cut across from high on the right to low on the left, hard, the crescent flying off the
        // edge as it comes through, and carried on round
        Special::CrescentWave => {
            let wound = pose([0.3, 0.3, 0.02], [0.55, 0.05, 0.83], [-0.7, 0.3, 0.6]);
            let cut = pose([0.06, 0.62, -0.16], [-0.35, 0.92, 0.1], [-0.75, -0.2, -0.6]);
            (
                0.85,
                vec![
                    (0.12, pose([0.34, 0.4, -0.26], [0.2, 0.5, 0.84], [-0.5, 0.6, 0.6]), false),
                    (0.26, wound.clone(), false),
                    (0.3, through(&wound, &cut, 0.5, [0.0, 0.06, 0.05]), false),
                    (0.36, cut, true),
                    (0.48, pose([-0.2, 0.5, -0.36], [-0.85, 0.45, -0.25], [-0.3, -0.2, -0.93]), false),
                    (0.62, pose([-0.22, 0.45, -0.4], [-0.86, 0.33, -0.4], [-0.3, -0.2, -0.93]), false),
                ],
            )
        }
        // Levelled low at the hip and driven on ahead the whole way, then heaved up and round as
        // what's on it is flung off
        Special::ImpalingCharge => {
            let levelled = pose([0.14, 0.62, -0.28], [-0.08, 0.99, 0.02], [-1.0, 0.0, 0.0]);
            let mut driving = levelled.clone();
            driving.hand.y += 0.04;
            (
                1.0,
                vec![
                    (0.05, pose([0.3, 0.22, -0.32], [0.05, 0.95, 0.25], [-1.0, 0.0, 0.0]), false),
                    (0.12, levelled.clone(), true),
                    (0.5, driving, false),
                    (0.62, pose([0.0, 0.58, -0.08], [-0.6, 0.6, 0.55], [-0.6, -0.5, 0.6]), true),
                    (0.8, pose([-0.14, 0.5, -0.2], [-0.85, 0.35, 0.3], [-0.3, -0.9, 0.2]), false),
                ],
            )
        }
        // Raised high overhead and held a moment, then slammed down into the ground, the pillars
        // bursting up as it lands
        Special::StonePillars => {
            let raised = pose([0.16, 0.26, 0.12], [0.05, -0.4, 0.92], [0.0, 0.92, 0.4]);
            let mut held = raised.clone();
            held.hand.z += 0.02;
            let slammed = pose([0.05, 0.62, -0.42], [0.0, 0.62, -0.78], [0.0, -0.78, -0.62]);
            let mut settled = slammed.clone();
            settled.hand.z -= 0.04;
            (1.0, vec![(0.1, pose([0.3, 0.42, -0.4], [0.05, 0.85, -0.5], [0.0, 0.5, 0.85]), false), (0.22, raised, false), (0.28, held, false), (0.34, slammed, true), (0.55, settled, false)])
        }
        // Whirled round overhead faster and faster, then flung out as the chain's let run
        Special::WreckingOrbit => {
            let mut keys = vec![(0.04, pose([0.3, 0.42, -0.2], [0.2, 0.6, 0.77], [1.0, 0.0, 0.0]), false)];
            for k in 0..14 {
                let a = k as f64 / 8.0 * TAU;
                let t = 0.08 + 0.75 * (k as f64 / 13.0).powf(0.8);
                keys.push((t, pose([0.18, 0.42, 0.04], [a.cos() * 0.85, a.sin() * 0.85, 0.5], [-a.sin(), a.cos(), 0.0]), false));
            }
            keys.push((0.9, pose([0.2, 0.55, -0.15], [0.0, 0.8, 0.6], [1.0, 0.0, 0.0]), true));
            (0.95, keys)
        }
        // A rocket's fuse lit off the bomb in hand and the bomb flung straight up
        Special::Skyburst => {
            let low = pose([0.28, 0.4, -0.32], [0.1, -0.2, 0.97], [1.0, 0.0, 0.0]);
            let up = pose([0.22, 0.5, 0.12], [0.0, 0.3, 0.95], [1.0, 0.0, 0.0]);
            (0.8, vec![(0.2, low, false), (0.32, up, true), (0.55, pose([0.24, 0.5, -0.05], [0.0, 0.2, 0.98], [1.0, 0.0, 0.0]), false)])
        }
        // Whirled round overhead twice, flung out at what's aimed at, held as the loop drops round
        // it, then yanked back hard
        Special::LassoPull => {
            // Over the head, up out of the way, so the loop shows going round above
            let mut keys = Vec::new();
            for k in 0..12 {
                let a = k as f64 / 6.0 * TAU;
                keys.push((0.04 + 0.3 * k as f64 / 11.0, grip([0.3 + 0.06 * a.cos(), 0.24 + 0.06 * a.sin(), 0.22], [a.cos() * 0.55, a.sin() * 0.55 - 0.2, 0.8]), false));
            }
            keys.extend([
                (0.42, grip([0.15, 0.68, -0.12], [-0.1, 0.97, 0.15]), true),
                (0.58, grip([0.16, 0.66, -0.14], [-0.1, 0.96, 0.12]), false),
                (0.66, grip([0.3, 0.3, -0.2], [0.0, 0.5, 0.86]), true),
                (0.86, grip([0.3, 0.38, -0.22], [-0.05, 0.8, 0.6]), false),
            ]);
            (1.0, keys)
        }
        // Drawn back and flung out at what's aimed at, then twisted and drawn tight
        Special::CoilBind => (
            0.9,
            vec![
                (0.14, grip([0.28, 0.36, 0.0], [0.1, -0.4, 0.9]), false),
                (0.36, grip([0.15, 0.68, -0.13], [-0.1, 0.97, 0.12]), true),
                (0.5, grip([0.2, 0.56, -0.16], [-0.08, 0.9, 0.42]), false),
                (0.58, grip([0.24, 0.46, -0.18], [-0.02, 0.8, 0.6]), true),
                (0.78, grip([0.24, 0.46, -0.2], [-0.03, 0.82, 0.57]), false),
            ],
        ),
        // Dipped, raised high so the lash falls back behind, then brought over the top as hard as
        // it'll go, cracked forward and down
        Special::ThunderCrack => (
            0.95,
            vec![
                (0.1, grip([0.3, 0.5, -0.26], [-0.05, 0.95, -0.25]), false),
                (0.28, grip([0.22, 0.28, 0.16], [0.05, -0.7, 0.72]), false),
                (0.36, grip([0.18, 0.48, 0.07], [-0.02, 0.45, 0.9]), false),
                (0.42, grip([0.13, 0.7, -0.14], [-0.1, 0.97, -0.05]), true),
                (0.55, grip([0.14, 0.6, -0.32], [-0.05, 0.7, -0.7]), false),
                (0.75, grip([0.24, 0.5, -0.28], [-0.06, 0.9, -0.4]), false),
            ],
        ),
        // Raised and cracked down onto the ground ahead, held there as it splits
        Special::SerpentLash => (
            0.85,
            vec![
                (0.15, grip([0.26, 0.38, 0.02], [0.1, -0.3, 0.95]), false),
                (0.36, grip([0.12, 0.66, -0.3], [-0.05, 0.75, -0.65]), true),
                (0.6, grip([0.13, 0.62, -0.34], [-0.05, 0.7, -0.7]), false),
            ],
        ),
        // Crouched low and swept right round, the hand going round low in front
        Special::TripSweep => {
            let mut keys = vec![(0.15, grip([0.36, 0.42, -0.42], [0.6, 0.5, -0.6]), false)];
            for k in 0..=8 {
                let a = FRAC_PI_2 + k as f64 / 8.0 * TAU;
                keys.push((0.28 + 0.42 * k as f64 / 8.0, grip([0.16 + 0.16 * a.cos(), 0.46 + 0.1 * a.sin(), -0.42], [a.cos() * 0.9, a.sin() * 0.9, -0.4]), k == 1));
            }
            keys.push((0.86, grip([0.3, 0.45, -0.25], [-0.1, 0.9, 0.3]), false));
            (1.0, keys)
        }
        // Raised high and cracked down onto the ground ahead, dragged along it as the line catches
        Special::BurningBrand => (
            0.9,
            vec![
                (0.18, grip([0.24, 0.3, 0.12], [0.05, -0.65, 0.76]), false),
                (0.3, grip([0.18, 0.48, 0.05], [-0.02, 0.5, 0.86]), false),
                (0.38, grip([0.12, 0.66, -0.26], [-0.06, 0.8, -0.6]), true),
                (0.62, grip([0.14, 0.6, -0.34], [-0.04, 0.7, -0.7]), false),
            ],
        ),
        // Flung up ahead, hung from overhead as the player swings, then let go of and cracked
        // down as they land
        Special::ChainSwing => (
            1.25,
            vec![
                (0.22, grip([0.18, 0.56, 0.02], [-0.1, 0.6, 0.8]), true),
                (0.35, grip([0.22, 0.32, 0.2], [-0.15, 0.3, 0.94]), false),
                (0.68, grip([0.23, 0.33, 0.18], [-0.13, 0.35, 0.93]), false),
                (0.82, grip([0.14, 0.65, -0.24], [-0.06, 0.8, -0.6]), true),
                (0.95, grip([0.2, 0.55, -0.28], [-0.06, 0.85, -0.5]), false),
            ],
        ),
        // A string of charges swung back low and flung out underarm across the ground
        Special::Firecrackers => {
            let low = pose([0.3, 0.3, -0.36], [0.1, -0.4, 0.9], [1.0, 0.0, 0.0]);
            let flung = pose([0.18, 0.62, -0.18], [0.0, 0.8, 0.6], [1.0, 0.0, 0.0]);
            (0.8, vec![(0.2, low, false), (0.36, flung, true), (0.6, pose([0.2, 0.55, -0.28], [0.0, 0.7, -0.7], [1.0, 0.0, 0.0]), false)])
        }
        // Drawn and loosed level at the crosshair
        Special::Thornburst if kind == Kind::Ranged => (0.7, vec![(0.16, drawn.clone(), false), (0.24, kicked(0.0), true), (0.48, drawn.clone(), false)]),
        // Bent down to set the keg on the ground ahead, then straightening up and back away
        Special::PowderKeg => {
            let set = pose([0.12, 0.62, -0.62], [0.0, 0.3, 0.95], [1.0, 0.0, 0.0]);
            let mut down = set.clone();
            down.hand.z -= 0.03;
            (0.9, vec![(0.3, set, false), (0.42, down, true), (0.65, pose([0.3, 0.3, -0.35], [0.1, 0.0, 0.99], [1.0, 0.0, 0.0]), false)])
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

/// How long a bell's ring follows through, in seconds
const CAST_TIME: f64 = 0.45;
/// How much of the time from one cast to the next casting takes, and the longest and shortest a
/// cast can be, in seconds
const CAST_SHARE: f64 = 0.9;
const CAST_LONGEST: f64 = 0.85;
const CAST_SHORTEST: f64 = 0.16;

/// How long casting a spell takes with a weapon casting `rate` times a second: most of the time
/// from one cast to the next, so a quick wand's flicked and a slow staff's spell gathered at
/// length, whatever its parts and their rarity made of it
fn cast_length(rate: f64) -> f64 {
    (CAST_SHARE / rate.max(0.1)).clamp(CAST_SHORTEST, CAST_LONGEST)
}

/// When a cast `casting` with a weapon of `style` has gathered its spell, and when it lets it go,
/// as parts of the whole cast. The spell leaves just as the move drawn lets it go.
fn cast_times(casting: Casting, style: Style) -> (f64, f64) {
    match style {
        Style::Summon => (0.08, 0.15),
        Style::Hex => (0.16, 0.3),
        Style::Arc => (0.09, 0.22),
        Style::Toll => (0.0, 0.0),
        _ => match casting {
            Casting::Thrust => (0.14, 0.3),
            Casting::Flick => (0.2, 0.4),
            Casting::Gather => (0.36, 0.62),
            Casting::Overhead => (0.3, 0.54),
            Casting::Sweep => (0.2, 0.38),
            Casting::Scoop => (0.22, 0.4),
            Casting::Present => (0.26, 0.46),
        },
    }
}

/// The poses a staff, scepter or wand goes through casting `casting`, for animate(): raised, the
/// spell gathered, and let go on the quick key at cast_times()'s release, then following through
fn cast_keys(casting: Casting) -> Vec<(f64, Pose, bool)> {
    let (gathers, release) = cast_times(casting, Style::Cast);
    let point = pose([0.2, 0.62, -0.3], [-0.15, 0.85, 0.5], [1.0, 0.0, 0.0]);
    let after = pose([0.21, 0.6, -0.32], [-0.13, 0.8, 0.58], [1.0, 0.0, 0.0]);
    let follow = |keys: &mut Vec<(f64, Pose, bool)>| keys.extend([(release, point.clone(), true), (release + 0.22, after.clone(), false)]);
    let mut keys = Vec::new();
    match casting {
        // Drawn back a little and up, then thrust out
        Casting::Thrust => {
            keys.push((gathers, pose([0.31, 0.38, -0.25], [0.02, 0.3, 0.95], [1.0, 0.0, 0.0]), false));
            follow(&mut keys);
        }
        // Drawn back from the wrist, then flicked out at it, checked and held a moment
        Casting::Flick => {
            keys.push((gathers, pose([0.24, 0.4, -0.19], [0.0, 0.1, 1.0], [1.0, 0.0, 0.0]), false));
            keys.push((release, pose([0.16, 0.62, -0.2], [-0.12, 0.95, 0.25], [1.0, 0.0, 0.0]), true));
            keys.push((release + 0.2, pose([0.17, 0.61, -0.22], [-0.12, 0.92, 0.2], [1.0, 0.0, 0.0]), false));
        }
        // Brought in upright before the chest in both hands and held there, trembling a little
        // as it gathers, then pushed out at the crosshair
        Casting::Gather => {
            let held = pose([0.15, 0.42, -0.27], [-0.18, 0.22, 0.96], [1.0, 0.0, 0.0]);
            let mut tight = held.clone();
            tight.hand += Vector3::new(-0.01, -0.03, 0.02);
            keys.push((gathers * 0.5, held, false));
            keys.push((gathers, tight.clone(), false));
            let mut swelling = tight;
            swelling.hand += Vector3::new(0.0, -0.015, 0.02);
            keys.push((release - 0.1, swelling, false));
            keys.push((release, pose([0.14, 0.68, -0.24], [-0.15, 0.85, 0.5], [1.0, 0.0, 0.0]), true));
            keys.push((release + 0.22, pose([0.16, 0.64, -0.27], [-0.13, 0.82, 0.56], [1.0, 0.0, 0.0]), false));
        }
        // Raised high overhead, out to the right and leaning back over the head so it's out of the
        // way, held up a moment, then brought down to point at where it falls
        Casting::Overhead => {
            let high = pose([0.34, 0.38, 0.1], [0.12, -0.22, 0.97], [1.0, 0.0, 0.0]);
            let mut held = high.clone();
            held.hand.z += 0.03;
            keys.push((gathers, high, false));
            keys.push((release - 0.1, held, false));
            follow(&mut keys);
        }
        // Swept round in from the right
        Casting::Sweep => {
            keys.push((gathers * 0.5, pose([0.34, 0.4, -0.3], [0.2, 0.4, 0.9], [0.5, 0.86, 0.0]), false));
            keys.push((gathers, pose([0.36, 0.42, -0.24], [0.4, 0.45, 0.8], [0.0, 1.0, 0.0]), false));
            follow(&mut keys);
        }
        // Dipped low, then scooped up and out
        Casting::Scoop => {
            keys.push((gathers, pose([0.26, 0.48, -0.44], [0.0, 0.9, -0.25], [1.0, 0.0, 0.0]), false));
            follow(&mut keys);
        }
        // Held up and shown, upright, then levelled at the crosshair
        Casting::Present => {
            let shown = pose([0.22, 0.56, -0.08], [-0.02, 0.08, 1.0], [1.0, 0.0, 0.0]);
            let mut held = shown.clone();
            held.hand.z += 0.02;
            keys.push((gathers, shown, false));
            keys.push((release - 0.08, held, false));
            keys.push((release, pose([0.17, 0.66, -0.24], [-0.1, 0.9, 0.42], [1.0, 0.0, 0.0]), true));
            keys.push((release + 0.22, pose([0.19, 0.63, -0.27], [-0.1, 0.86, 0.5], [1.0, 0.0, 0.0]), false));
        }
    }
    keys
}

/// How many times a weapon of `grade` is twirled after a flourish-worthy move, `last` being the
/// end of a chain: from legendary the last once, from mythic every one, the last twice
fn flourish_turns(grade: usize, last: bool) -> f64 {
    match (grade > FLOURISH_GRADE, grade >= FLOURISH_GRADE, last) {
        (true, _, true) => 2.0,
        (true, _, false) | (_, true, true) => 1.0,
        _ => 0.0,
    }
}

/// How far round a staff or wand of `grade` is twirled `f` of the way through its cast, in
/// radians, a whole number of turns from just after it lets go to before the cast's over, and
/// how far it's lifted for it, 0-1: every third cast from legendary, every one from mythic, the
/// third twice. `third` is whether it's the third.
fn cast_twirl(grade: usize, third: bool, release: f64, f: f64) -> (f64, f64) {
    let turns = flourish_turns(grade, third);
    if turns <= 0.0 {
        return (0.0, 0.0);
    }
    let from = release + 0.05;
    let to = from + 0.8 * (0.97 - from);
    let u = ((f - from) / (to - from)).clamp(0.0, 1.0);
    (TAU * turns * u * u * u * (u * (u * 6.0 - 15.0) + 10.0), (PI * u).sin().powi(2))
}

/// Most sparks flying about at once
const MOST_SPARKS: usize = 140;

/// Seconds into a powder keg special before the keg's been set down on the ground
const KEG_SET: f64 = 0.38;

/// Meters walked from one footfall to the next, for bobbing what's in hand
const STEP_LENGTH: f64 = 0.8;

/// How stiff the spring is that brings the view back down after a shot kicks it up, per second
const KICK_SPRING: f64 = 16.0;
/// Seconds a gun's flash shows at its muzzle
const FLARE: f64 = 0.06;
/// How much of a gun's damage a spread of shot does all told, if every pellet hits
const PELLETS_SHARE: f64 = 1.25;
/// Degrees a spread of shot spreads if nothing says how far
const PELLET_SPREAD: f64 = 4.5;
/// How much of a crossbow's damage a twin-prod's pair of bolts does all told, and a
/// bolt-thrower's five, if all of them hit
const TWIN_SHARE: f64 = 1.4;
const FAN_SHARE: f64 = 1.6;
/// How far apart a bolt-thrower's grooves are turned, in radians, see weapons::crossbow(): its
/// bolts fan out as far apart as its spread says
const FAN_YAW: f64 = 0.12;
/// Seconds a siege crossbow takes to brace on its leg
const BRACE_TIME: f64 = 0.45;
/// How far a harpoon's line reaches, in meters
const HARPOON_REACH: f64 = 32.0;
/// What a harpoon can reel in: up to this big round, and no boss
const HARPOON_HAUL: f64 = 1.1;
/// How fast a harpoon reels in what it caught, in meters a second, and how near it brings it
const REEL_SPEED: f64 = 13.0;
const REEL_NEAR: f64 = 2.2;

/// How hard a crossbow or gun built `build` kicks as it goes off, 1 for a musket
fn kick_of(build: Build) -> f64 {
    match build {
        Build::Pistol => 0.75,
        Build::Musket => 1.0,
        Build::Blunderbuss => 1.25,
        Build::Cannon => 1.5,
        Build::Repeater => 0.15,
        Build::Hand => 0.25,
        Build::Twin => 0.5,
        Build::Fan => 0.8,
        Build::Harpoon => 0.6,
        Build::Siege => 1.3,
        _ => 0.35,
    }
}

/// How long a crossbow or gun takes to settle again after a shot, in seconds: a repeater's lever
/// spanning it again, as long as its cycle
fn fire_length(profile: &Profile) -> f64 {
    match profile.build {
        Build::Repeater => profile.cycle,
        Build::Lever | Build::Crank | Build::Twin | Build::Fan | Build::Harpoon => 0.3,
        Build::Hand => 0.22,
        Build::Siege => 0.55,
        Build::Cannon => 0.6,
        _ => 0.45,
    }
}

/// The poses a crossbow or gun goes through being loaded, for animate(): a gun tipped up
/// muzzle first for the powder and the ramrod, then brought down level and cocked; a light
/// crossbow dipped while its lever swings over; a heavy one stood on its nose while it's wound
/// back; a repeater rolled over to drop bolts into its magazine
fn reload_keys(build: Build) -> Vec<(f64, Pose, bool)> {
    match build {
        Build::Lever => {
            let dipped = pose([0.15, 0.42, -0.3], [-0.05, 0.25, 0.97], [-0.1, 0.95, -0.25]);
            let mut braced = dipped.clone();
            braced.hand.y -= 0.03;
            vec![(0.12, dipped.clone(), false), (0.5, braced, false), (0.85, dipped, false)]
        }
        Build::Crank => {
            let down = pose([0.12, 0.62, -0.3], [0.0, 0.35, 0.94], [-0.25, 0.9, -0.35]);
            let mut low = down.clone();
            low.hand.z -= 0.02;
            vec![(0.1, down.clone(), false), (0.5, low, false), (0.88, down, false)]
        }
        Build::Repeater => {
            let rolled = pose([0.12, 0.4, -0.26], [-0.55, 0.0, 0.84], [-0.05, 1.0, 0.0]);
            let mut shaken = rolled.clone();
            shaken.hand.z -= 0.025;
            vec![(0.15, rolled.clone(), false), (0.35, shaken.clone(), false), (0.55, rolled.clone(), false), (0.7, shaken, false), (0.85, rolled, false)]
        }
        // Brought in across the body and tipped up so the other hand can pull its lever
        Build::Hand => {
            let tipped = pose([-0.15, 0.4, -0.3], [0.15, -0.3, 0.94], [0.15, 0.95, 0.28]);
            let mut pulled = tipped.clone();
            pulled.hand.z -= 0.02;
            pulled.hand.x += 0.01;
            vec![(0.12, tipped.clone(), false), (0.45, pulled, false), (0.85, tipped, false)]
        }
        // Held level, rolled so its crank side faces up to be turned
        Build::Twin | Build::Harpoon => {
            let rolled = pose([0.17, 0.42, -0.31], [0.4, -0.1, 0.9], [-0.1, 1.0, 0.02]);
            let mut low = rolled.clone();
            low.hand.z -= 0.02;
            vec![(0.1, rolled.clone(), false), (0.5, low, false), (0.88, rolled, false)]
        }
        // Lowered, braced, and its screw spun round from behind
        Build::Siege => {
            let low = pose([0.12, 0.5, -0.32], [0.02, -0.05, 1.0], [-0.08, 1.0, 0.04]);
            let mut lower = low.clone();
            lower.hand.z -= 0.015;
            vec![(0.1, low.clone(), false), (0.5, lower, false), (0.88, low, false)]
        }
        // Swung round level, its side lever out away from the body
        Build::Fan => {
            let turned = pose([0.08, 0.44, -0.27], [-0.2, 0.0, 0.98], [-0.35, 0.94, 0.0]);
            let mut hauled = turned.clone();
            hauled.hand.y -= 0.03;
            vec![(0.12, turned.clone(), false), (0.55, hauled, false), (0.86, turned, false)]
        }
        // Muzzle up for the powder, jolting down a little as the ramrod goes home twice, as ramrod()
        // has it, then each its own way: a pistol brought in close and turned to look down its
        // pan, its hammer thumbed back with a flick; a blunderbuss tipped right up and shaken to
        // settle its shot; a hand cannon heaved up onto the shoulder, heavily; a musket as it is
        _ => {
            let (tipped, powder) = match build {
                Build::Pistol => (pose([0.04, 0.36, -0.28], [0.35, -0.45, 0.82], [-0.2, 0.85, 0.5]), pose([0.0, 0.34, -0.22], [0.5, -0.3, 0.81], [-0.4, 0.85, 0.35])),
                Build::Blunderbuss => (pose([0.08, 0.38, -0.38], [0.15, -0.7, 0.7], [-0.1, 0.7, 0.7]), pose([0.12, 0.36, -0.36], [0.25, -0.65, 0.72], [-0.15, 0.72, 0.68])),
                Build::Cannon => (pose([0.24, 0.3, -0.24], [0.1, -0.75, 0.66], [-0.08, 0.66, 0.75]), pose([0.26, 0.28, -0.22], [0.12, -0.78, 0.62], [-0.08, 0.62, 0.78])),
                _ => (pose([0.1, 0.38, -0.36], [0.2, -0.55, 0.81], [-0.12, 0.81, 0.57]), pose([0.1, 0.38, -0.36], [0.2, -0.55, 0.81], [-0.12, 0.81, 0.57])),
            };
            let mut rammed = tipped.clone();
            rammed.hand.z -= if build == Build::Cannon { 0.035 } else { 0.025 };
            let level = pose([0.16, 0.36, -0.24], [0.04, -0.15, 0.99], [-0.1, 1.0, 0.12]);
            let mut keys = vec![(0.1, powder.clone(), false)];
            // Shaken, side to side, as what's poured in settles
            if build == Build::Blunderbuss {
                let mut shaken = powder.clone();
                shaken.hand.x += 0.025;
                keys.push((0.17, shaken.clone(), false));
                shaken.hand.x -= 0.05;
                keys.push((0.23, shaken, false));
            }
            keys.extend([(0.3, tipped.clone(), false), (0.45, rammed.clone(), true), (0.52, tipped.clone(), false), (0.63, rammed, true), (0.76, tipped, false)]);
            if build == Build::Pistol {
                // Brought level, then the hammer thumbed back: a little flick up
                let mut cocked = level.clone();
                cocked.turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 0.25) * cocked.turn;
                keys.extend([(0.86, level.clone(), false), (0.92, cocked, true)]);
            } else {
                keys.push((0.9, level, false));
            }
            keys
        }
    }
}

/// The poses a crossbow or gun built `build` goes through being loaded with `knack`, see
/// weapons::knack_of(): as reload_keys() has them, or held in close and high and snapped up at the
/// end, a gun's cartridge bitten open first; braced low and leant over, as a heavy one is; or
/// rolled right round in the hand as it's done, with a flourish
fn knack_keys(build: Build, knack: Knack) -> Vec<(f64, Pose, bool)> {
    let mut keys = reload_keys(build);
    let gun = !build.crossbow();
    match knack {
        Knack::Plain => {}
        Knack::Quick => {
            for k in &mut keys {
                k.1.hand += Vector3::new(-0.025, -0.02, 0.03);
            }
            // A paper cartridge brought up to the mouth and bitten open, before it's poured
            if gun && build != Build::Blunderbuss
                && let Some(first) = keys.first().map(|k| k.1.clone())
            {
                let mut bitten = first;
                bitten.hand += Vector3::new(-0.08, -0.04, 0.1);
                keys.insert(0, (0.05, bitten, false));
                keys[1].0 = keys[1].0.max(0.14);
            }
            let end = keys.iter().rposition(|k| k.0 <= 0.88).unwrap_or(0);
            keys.truncate(end + 1);
            let mut snapped = keys[end].1.clone();
            snapped.hand.z += 0.03;
            snapped.turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 0.18) * snapped.turn;
            keys.push((0.94, snapped, true));
        }
        Knack::Heavy => {
            for k in &mut keys {
                k.1.hand += Vector3::new(0.02, -0.03, -0.05);
                k.1.turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.12) * k.1.turn;
            }
        }
        // Brought back to how it's held at the end a little sooner, then rolled right round
        Knack::Showy => {
            let Some(last) = keys.last().map(|k| k.1.clone()) else {
                return keys;
            };
            keys.retain(|k| k.0 <= 0.78);
            keys.push((0.82, last.clone(), false));
            for (j, at) in [0.87, 0.92, 0.97].into_iter().enumerate() {
                let mut rolled = last.clone();
                rolled.turn *= UnitQuaternion::from_axis_angle(&Vector3::z_axis(), (j + 1) as f64 * TAU / 3.0);
                rolled.hand.z += 0.04 * (PI * (j + 1) as f64 / 3.0).sin();
                keys.push((at, rolled, false));
            }
        }
    }
    keys
}

/// Seconds the weapon in hand has to be at rest, standing about, before it's given a flourish,
/// and how long one takes
const IDLE_AFTER: f64 = 6.0;
const FLOURISH_SECONDS: f64 = 1.7;

/// A flourish given the weapon in hand, standing about with it, see flourish_at()
#[derive(Clone, Copy, PartialEq, Debug)]
enum Flourish {
    /// Spun round its own length in the hand, a blade catching the light on its flats
    Twirl,
    /// Flipped end over end and caught, a dagger's
    Flip,
    /// Rested back on the shoulder a moment
    Shoulder,
    /// Turned over to be looked at, and back
    Inspect,
    /// Spun round twice on a finger, a pistol's or a wand's
    Spin,
    /// Tossed up and caught, a bomb or an orb
    Toss,
    /// Its foot tapped on the ground twice
    Tap,
    /// A bow turned to sight along it, as if checking its string
    Check,
}

/// Which flourish the weapon in hand gets, the one that began at `began` seconds: one of two
/// for each kind of weapon, by turns as it happens
fn flourish_of(weapon: &Weapon, began: f64) -> Flourish {
    let profile = &weapon.profile;
    let pair = match (profile.kind, profile.style) {
        (Kind::Melee, Style::Stab) => [Flourish::Flip, Flourish::Twirl],
        (Kind::Melee, Style::Chop | Style::Smash | Style::Quake | Style::Whirl) => [Flourish::Shoulder, Flourish::Inspect],
        (Kind::Melee, Style::Thrust) | (Kind::Magic, Style::Glaive | Style::Cast | Style::Hex) => [Flourish::Tap, Flourish::Twirl],
        (Kind::Melee, Style::Lash) => [Flourish::Twirl, Flourish::Twirl],
        (Kind::Melee, _) => [Flourish::Twirl, Flourish::Inspect],
        (Kind::Ranged, Style::Lob) | (Kind::Magic, Style::Channel) => [Flourish::Toss, Flourish::Toss],
        (Kind::Ranged, _) if matches!(profile.build, Build::Pistol | Build::Hand) => [Flourish::Spin, Flourish::Inspect],
        (Kind::Ranged, _) if profile.build == Build::None => [Flourish::Check, Flourish::Inspect],
        (Kind::Ranged, _) => [Flourish::Inspect, Flourish::Check],
        (Kind::Magic, Style::Flick | Style::Arc) => [Flourish::Spin, Flourish::Twirl],
        (Kind::Magic, _) => [Flourish::Inspect, Flourish::Toss],
    };
    pair[(hash(weapon.seed ^ (began * 0.25) as u32) & 1) as usize]
}

/// How flourish `kind` moves the weapon `f` of the way through, 0-1: how far the hand moves, in
/// the camera's right, forward and up, and how it's turned, before the pose (in the camera's
/// frame) and after it (in the weapon's own). Each eases out of rest and back into it, a turn
/// winding back a little first and carrying on a little past before it settles.
fn flourish_at(kind: Flourish, f: f64) -> (Vector3<f64>, UnitQuaternion<f64>, UnitQuaternion<f64>) {
    let f = f.clamp(0.0, 1.0);
    let bump = (PI * f).sin().powi(2);
    let plateau = smooth(f / 0.3) * (1.0 - smooth((f - 0.7) / 0.3));
    // A bump from nothing up and back to nothing between `from` and `to`, leaving and arriving still
    let hump = |from: f64, to: f64| (PI * ((f - from) / (to - from)).clamp(0.0, 1.0)).sin().powi(2);
    let spin = |turns: f64| {
        let u = ((f - 0.15) / 0.7).clamp(0.0, 1.0);
        let eased = u * u * u * (u * (u * 6.0 - 15.0) + 10.0);
        TAU * turns * eased - 0.2 * hump(0.0, 0.3) + 0.12 * hump(0.75, 1.0)
    };
    let about = |axis: Unit<Vector3<f64>>, angle: f64| UnitQuaternion::from_axis_angle(&axis, angle);
    let none = UnitQuaternion::identity();
    match kind {
        Flourish::Twirl => (Vector3::new(-0.03, 0.03, 0.05) * bump, none, about(Vector3::z_axis(), spin(1.0))),
        Flourish::Flip => (Vector3::new(0.0, 0.0, 0.12 * hump(0.12, 0.88)), none, about(Vector3::y_axis(), spin(1.0))),
        Flourish::Shoulder => (Vector3::new(0.04, -0.12, 0.1) * plateau, about(Vector3::x_axis(), 0.9 * plateau), none),
        Flourish::Inspect => (Vector3::new(-0.07, -0.05, 0.07) * plateau, about(Vector3::y_axis(), 0.75 * plateau) * about(Vector3::z_axis(), -0.4 * plateau), none),
        Flourish::Spin => (Vector3::new(0.0, 0.0, 0.03) * bump, none, about(Vector3::y_axis(), spin(2.0))),
        // Tossed up, turning over, and caught with a little give
        Flourish::Toss => (Vector3::new(0.0, 0.0, 0.2 * hump(0.12, 0.75) - 0.03 * hump(0.65, 1.0)), none, about(Vector3::x_axis(), spin(1.0))),
        Flourish::Tap => (Vector3::new(0.0, 0.0, -0.06 * (hump(0.2, 0.42) + hump(0.48, 0.7)) + 0.03 * bump), about(Vector3::x_axis(), -0.15 * bump), none),
        Flourish::Check => (Vector3::new(0.08, 0.05, 0.06) * plateau, about(Vector3::z_axis(), 0.5 * plateau) * about(Vector3::y_axis(), -0.4 * plateau), none),
    }
}

/// Where a gun's ramrod is `f` of the way through loading it, in meters out of the barrel, if
/// it's out: pulled out, rammed down twice, and put away
fn ramrod(f: f64) -> Option<f64> {
    let keys = [(0.3, 0.6), (0.34, 0.45), (0.45, 0.04), (0.53, 0.32), (0.63, 0.04), (0.72, 0.6)];
    if f < keys[0].0 || f > keys[keys.len() - 1].0 {
        return None;
    }
    let i = keys.windows(2).position(|w| f <= w[1].0).unwrap_or(keys.len() - 2);
    let ((t0, a), (t1, b)) = (keys[i], keys[i + 1]);
    Some(a + (b - a) * smooth((f - t0) / (t1 - t0)))
}

/// How far a crossbow's string is spanned back, 0-1, and its lever or cranks turned, in radians,
/// `f` of the way through spanning it
fn spanning(build: Build, f: f64) -> (f64, f64) {
    match build {
        // The lever swung up and over onto the string, then pulled back, the string with it
        Build::Lever => {
            let over = smooth(f / 0.45) * (1.0 - smooth((f - 0.45) / 0.45));
            (smooth((f - 0.45) / 0.45), 2.5 * over)
        }
        // Wound back turn after turn
        Build::Crank => {
            let wound = ((f - 0.08) / 0.8).clamp(0.0, 1.0);
            (wound, wound * 5.0 * TAU)
        }
        // The lever swung down from under the tiller, drawing the string back, and folded up again
        Build::Hand => {
            let down = smooth(f / 0.42) * (1.0 - smooth((f - 0.5) / 0.38));
            (smooth((f - 0.08) / 0.38), -1.25 * down)
        }
        // The pinion's crank turned quickly, the rack drawing both strings back
        Build::Twin => {
            let wound = ((f - 0.06) / 0.8).clamp(0.0, 1.0);
            (wound, wound * 7.0 * TAU)
        }
        // The jack's screw spun round and round
        Build::Siege => {
            let wound = ((f - 0.1) / 0.75).clamp(0.0, 1.0);
            (wound, wound * 10.0 * TAU)
        }
        // The reel wound in, the prod with it
        Build::Harpoon => {
            let wound = ((f - 0.05) / 0.8).clamp(0.0, 1.0);
            (wound, -wound * 8.0 * TAU)
        }
        // The side lever swung out, then hauled back in, bringing the yoke back with it
        Build::Fan => {
            let out = smooth((f - 0.35) / 0.25) * (1.0 - smooth((f - 0.62) / 0.26));
            (smooth((f - 0.6) / 0.28), -1.1 * out)
        }
        // A repeater's magazine is filled, then its lever worked once to span it
        _ => (smooth((f - 0.82) / 0.12), -0.45 * (PI * ((f - 0.8) / 0.18).clamp(0.0, 1.0)).sin()),
    }
}

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
            drawn: None,
            drawn_velocity: (Vector3::zeros(), Vector3::zeros()),
            drawn_action: Action::Idle,
            blending: None,
            lash: None,
            lash_aim: None,
            binds: Vec::new(),
            serpents: Vec::new(),
            casts: 0,
            shake: 0.0,
            kick: 0.0,
            kick_speed: 0.0,
            loads: HashMap::new(),
            flare: 0.0,
            flare_roll: 0.0,
            nocked: None,
            spent: Vec::new(),
            beam_tick: 0.0,
            beam_hum: 0.0,
            familiars: Vec::new(),
            orbit: None,
            stases: Vec::new(),
            siphon: None,
            tether: None,
            line_from: None,
            brace: 0.0,
            snared: Vec::new(),
            mines: Vec::new(),
            sweep: None,
            thrown: None,
            shackle: None,
            blood_moon: None,
            lobs: Vec::new(),
            ripples: Vec::new(),
            zones: Vec::new(),
            pool_timer: 0.0,
            puff: 0.0,
            scar_timer: 0.0,
            scarless: false,
            rays: Vec::new(),
            speed: 0.0,
            stride: 0.0,
            player_hits: Vec::new(),
            trail: None,
            ribbon: None,
            glow: 0.0,
            glints: Vec::new(),
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
            close: false,
            blood_timer: 0.0,
            charge: 0.0,
            heal: 0.0,
            dash: Vector3::zeros(),
            hunt: 0.0,
            marked: Vec::new(),
            marked_at: Vec::new(),
            skewered: Vec::new(),
            charging: Vector3::zeros(),
            idle: 0.0,
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
            blast_floors: Vec::new(),
            remnants: Vec::new(),
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
                Act::Shell(at) => *at -= shift,
                Act::Blast { from, .. } | Act::Kindle { from, .. } => *from -= shift,
                Act::Spike(at, _) | Act::Geyser(at) | Act::Pillar(at) => *at -= shift,
                Act::Brand { from, to, .. } => {
                    *from -= shift;
                    *to -= shift;
                }
                Act::Lasso { .. } | Act::Bind(_) | Act::Boom(_) | Act::Serpents | Act::Trip(_) | Act::Swing { .. } => {}
                Act::Bolt(_)
                | Act::Spin
                | Act::Slash
                | Act::Dash(_)
                | Act::Land { .. }
                | Act::Volley(_)
                | Act::Gout(_)
                | Act::Web(_)
                | Act::Knives(_)
                | Act::Plunge(_)
                | Act::Reap
                | Act::Quake(_)
                | Act::Erupt(_)
                | Act::Avalanche
                | Act::Shatter
                | Act::Skewer(_)
                | Act::Fling
                | Act::Coil(_)
                | Act::Flay(_)
                | Act::Quick(_)
                | Act::Reckon(_) => {}
            }
        }
        for p in &mut self.projectiles {
            if let Trick::Net(at) | Trick::Comet(at) | Trick::Rocket(at) = &mut p.trick {
                *at -= shift;
            }
        }
        for r in &mut self.ripples {
            r.center -= shift;
        }
        for z in &mut self.zones {
            z.position -= shift;
            for at in &mut z.path {
                *at -= shift;
            }
        }
        if let Some(t) = &mut self.tether {
            t.at -= shift;
        }
        if let Some(at) = &mut self.line_from {
            *at -= shift;
        }
        for (at, _) in &mut self.snared {
            *at -= shift;
        }
        for m in &mut self.mines {
            m.position -= shift;
        }
        if let Some(t) = &mut self.thrown {
            t.from -= shift;
            t.position -= shift;
        }
        if let Some(s) = &mut self.shackle {
            s.center -= shift;
            s.from -= shift;
            for c in &mut s.chains {
                c.at -= shift;
            }
        }
        if let Some(m) = &mut self.blood_moon {
            m.moon -= shift;
            for s in &mut m.shots {
                s.from -= shift;
                s.to -= shift;
                s.position -= shift;
            }
            for s in &mut m.streams {
                s.from -= shift;
            }
            for b in &mut m.beads {
                b.position -= shift;
            }
        }
        for l in &mut self.lobs {
            l.to -= shift;
        }
        for f in &mut self.familiars {
            f.position -= shift;
        }
        for s in &mut self.stases {
            s.position -= shift;
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
        if let Some(r) = &mut self.ribbon {
            *r -= shift;
        }
        for g in &mut self.glints {
            g.position -= shift;
        }
        if let Some(lash) = &mut self.lash {
            lash.recenter(shift);
        }
        if let Some(at) = &mut self.lash_aim {
            *at -= shift;
        }
        for s in &mut self.serpents {
            s.position -= shift;
        }
        for b in &mut self.binds {
            b.at -= shift;
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
        for r in &mut self.remnants {
            r.position -= shift;
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
        self.glints.clear();
        self.stuck.clear();
        self.flashes.clear();
        self.blasts.clear();
        self.blast_floors.clear();
        self.remnants.clear();
        self.echoes.clear();
        self.popups.clear();
        self.craters.clear();
        self.lash = None;
        self.familiars.clear();
        self.orbit = None;
        self.stases.clear();
        self.siphon = None;
        self.tether = None;
        self.line_from = None;
        self.snared.clear();
        self.mines.clear();
        self.sweep = None;
        self.thrown = None;
        self.shackle = None;
        self.blood_moon = None;
        self.lobs.clear();
        self.ripples.clear();
        self.zones.clear();
        self.hunt = 0.0;
        self.marked.clear();
        self.skewered.clear();
        self.lash_aim = None;
        self.binds.clear();
        self.serpents.clear();
    }

    /// How fast the player can walk, next to as they would: a siege crossbow braced holds them
    /// all but still
    pub fn pace(&self) -> f64 {
        1.0 - 0.75 * self.brace
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

    /// Advances the fight by `dt` seconds: the weapon in hand, `weapon` with its index and the
    /// special ammo `loaded` in it if any, attacking as `controls` say, things flying, and what's
    /// been done to the animals wearing off
    pub fn update(&mut self, arena: &mut Arena, weapon: Option<(usize, &Weapon)>, loaded: Option<&Loaded>, controls: Controls, dt: f64) {
        self.marks.clear();
        self.rays.clear();
        self.vpm = arena.view.vpm;
        self.speed = arena.view.speed;
        self.stride = arena.view.stride;
        self.time += dt;
        self.blood_timer -= dt;
        self.mana = (self.mana + MANA_REGEN * dt).min(MAX_MANA);
        self.cooldown -= dt;
        self.frenzy -= dt;
        // A hunter's mark wearing off, and coming off what's died
        self.hunt -= dt;
        if self.hunt <= 0.0 {
            self.marked.clear();
        } else if !self.marked.is_empty() {
            let animals = &arena.animals;
            self.marked.retain(|&id| animals.iter().any(|a| a.id == id && a.health > 0.0));
        }
        self.marked_at.clear();
        {
            // Those a hunter's mark is on, and those a weapon's mark is on, the first dozen
            let vpm = arena.view.vpm;
            let marked = &self.marked;
            let over = |a: &Animal| a.center(vpm as u32) + Vector3::new(0.0, 0.0, (a.look.body().1 + 0.5) * vpm);
            self.marked_at.extend(arena.animals.iter().filter(|a| a.health > 0.0 && (marked.contains(&a.id) || a.status.marked > 0.0)).take(12).map(over));
        }
        self.mana_warning -= dt;
        self.hitstop -= dt;
        self.recoil = (self.recoil - dt * 5.0).max(0.0);
        self.shake *= (-7.0 * dt).exp();
        self.flare -= dt;
        self.glow -= dt;
        for g in &mut self.glints {
            g.life -= dt;
        }
        self.glints.retain(|g| g.life > 0.0);
        self.nocked = loaded.map(|l| (l.head, l.profile.element));
        // The view springing back down after a shot kicked it up: a critically damped spring,
        // stepped exactly so it's the same however long the frames are
        let (x, v, w) = (self.kick, self.kick_speed, KICK_SPRING);
        let decay = (-w * dt).exp();
        self.kick = (x + (v + w * x) * dt) * decay;
        self.kick_speed = (v - w * (v + w * x) * dt) * decay;

        let index = weapon.map(|(i, _)| i);
        if index != self.held {
            self.held = index;
            self.raise = 0.0;
            self.action = Action::Idle;
            self.trail = None;
            self.ribbon = None;
            self.glow = 0.0;
            self.drawn = None;
            self.blending = None;
            self.lash = None;
            self.lash_aim = None;
            self.casts = 0;
            self.brace = 0.0;
            // A weapon thrown is back in the pack, a beam swept stops, and a harpoon's line is let go of
            self.thrown = None;
            self.sweep = None;
            self.tether = None;
            for p in &mut self.projectiles {
                if p.trick == Trick::Harpoon {
                    p.trick = Trick::None;
                }
            }
            if let Some((_, weapon)) = weapon {
                let pitch = match weapon.kind() {
                    Kind::Melee => 1.0,
                    Kind::Ranged if weapon.profile.build != Build::None => 0.55,
                    Kind::Ranged => 0.7,
                    Kind::Magic => 1.3,
                };
                self.cues.push(Cue::here(Sound::Equip, 0.35).pitched(pitch));
            }
        }
        self.raise = (self.raise + dt / 0.3).min(1.0);

        if let Some((index, weapon)) = weapon {
            self.wield(arena, index, weapon, loaded, controls, dt);
            // At rest a while, standing about, it's given a flourish now and then; one started
            // is let finish even if the player moves off, but anything done with it stops it
            let flourishing = self.idle > IDLE_AFTER;
            if self.action != Action::Idle || self.raise < 1.0 || self.thrown.is_some() || (!flourishing && self.speed > 1.5) {
                self.idle = 0.0;
            } else {
                self.idle += dt;
                if self.idle > IDLE_AFTER + FLOURISH_SECONDS {
                    // A while longer till the next, not always as long
                    self.idle = -4.0 * random(&mut self.rng);
                }
            }
            self.settle(weapon, dt);
            self.swing_lash(arena, weapon, dt);
            self.flourish(arena.view, weapon, dt);
        }
        // Where a harpoon's line leaves the crossbow in hand, for it to be drawn from
        self.line_from = weapon.filter(|(_, w)| w.profile.build == Build::Harpoon).map(|(_, w)| self.held_point(w, arena.view, false));
        if weapon.is_none_or(|(_, w)| w.profile.build != Build::Siege) {
            self.brace = 0.0;
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
        self.pool_timer -= dt;
        self.scar_timer -= dt;
        self.follow_up(arena, dt);
        self.conjured(arena, weapon.map(|(_, w)| w), dt);
        self.fly(arena, dt);
        self.fly_hostiles(arena, dt);
        self.linger(arena, dt);
        // Shockwaves and the special attacks that go on a while hit a lot at once, so what they
        // set off blows no holes and leaves no stains, as changing the world takes a while
        self.scarless = true;
        self.roll_ripples(arena, dt);
        self.zones(arena, dt);
        self.scarless = false;
        if !self.binds.is_empty() || !self.serpents.is_empty() {
            self.writhe(arena, dt);
        }
        self.afflict(arena, dt);

        for s in &mut self.sparks {
            s.velocity.z -= 9.8 * arena.view.vpm * s.gravity * dt;
            // Smoke hangs in the air, soon slowed to a drift
            s.velocity *= (-if s.slot == SLOT_SMOKE || s.slot == SLOT_CLOUD { 4.0 } else { 1.5 } * dt).exp();
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
        self.age_blasts(arena, dt);
        let rise = 0.9 * arena.view.vpm;
        for p in &mut self.popups {
            p.age += dt;
            p.position.z += rise * dt;
        }
        self.popups.retain(|p| p.age < 1.2);
    }

    /// Explosions going on: how high each new one is over the ground worked out once, for the ring
    /// it throws up there; those done leaving their smoke drifting up after them, the bigger ones
    fn age_blasts(&mut self, arena: &Arena, dt: f64) {
        let vpm = arena.view.vpm;
        self.blast_floors.truncate(self.blasts.len());
        while self.blast_floors.len() < self.blasts.len() {
            let at = self.blasts[self.blast_floors.len()].position;
            let top = at + Vector3::new(0.0, 0.0, 0.3 * vpm);
            let down = Vector3::new(0.0, 0.0, -5.0 * vpm);
            let height = solid_along(arena.world, top, top + down).map_or(f64::INFINITY, |t| (at.z - (top.z + down.z * t)).max(0.0));
            self.blast_floors.push(height);
        }
        for b in &mut self.blasts {
            b.life -= dt;
        }
        let mut k = 0;
        while k < self.blasts.len() {
            if self.blasts[k].life > 0.0 {
                k += 1;
                continue;
            }
            let b = self.blasts.remove(k);
            self.blast_floors.remove(k);
            if b.radius >= 0.9 && leaves_smoke(b.look) {
                self.remnants.push(Remnant { position: b.position, look: b.look, radius: b.radius, yaw: blast_yaw(b.position), life: REMNANT_SECONDS, lifetime: REMNANT_SECONDS });
            }
        }
        for r in &mut self.remnants {
            r.life -= dt;
            r.position.z += 0.6 * vpm * dt;
        }
        self.remnants.retain(|r| r.life > 0.0);
        if self.remnants.len() > MOST_REMNANTS {
            let extra = self.remnants.len() - MOST_REMNANTS;
            self.remnants.drain(..extra);
        }
    }

    /// Attacking with the weapon in hand, weapon `index` among Game::weapons, loaded with special
    /// ammo `loaded` if it shoots and there is any
    fn wield(&mut self, arena: &mut Arena, index: usize, weapon: &Weapon, loaded: Option<&Loaded>, controls: Controls, dt: f64) {
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
        // Nothing else till a special attack's done, or the weapon's back in hand
        if let Action::Special { t, length, special } = self.action {
            self.action = if t + dt >= length { Action::Idle } else { Action::Special { t: t + dt, length, special } };
            if self.action == Action::Idle {
                self.lash_aim = None;
            }
            return;
        }
        if self.thrown.is_some() {
            return;
        }
        let style = moves_like(profile.style);
        // A battle staff's swung like a melee weapon
        let kind = if profile.style == Style::Glaive { Kind::Melee } else { weapon.kind() };
        match kind {
            Kind::Magic if profile.style == Style::Channel => self.channel(arena, weapon, controls, dt),
            Kind::Magic if profile.style == Style::Toll => self.toll(arena, weapon, controls, dt),
            Kind::Magic if profile.style == Style::Censer => self.swing_censer(arena, weapon, controls, dt),
            Kind::Ranged if profile.style == Style::Lob => self.lob(arena, weapon, controls, dt),
            Kind::Melee => {
                if let Action::Swing { t, length, heavy, struck, combo } = self.action {
                    // A heavy weapon gathers longer and comes down harder, a light one snaps out,
                    // landing as the move it's drawn going through does, see target_pose()
                    let (comes, lands) = blow_times(&profile, heavy, combo);
                    let finisher = combo == finisher(style) && !heavy;
                    // In heavy slow motion a moment as a blow lands
                    let dt = if self.hitstop > 0.0 { dt * HITSTOP_PACE } else { dt };
                    // Whooshing as it comes, and landing partway through the swing
                    if t < length * comes && t + dt >= length * comes {
                        let (sound, pitch) = match style {
                            Style::Stab => (Sound::Stab, if heavy || finisher { 0.85 } else { 1.1 }),
                            Style::Thrust => (Sound::Stab, if heavy { 0.7 } else { 0.85 }),
                            Style::Smash => (Sound::HeavySwing, if heavy || finisher { 0.7 } else { 0.85 }),
                            Style::Chop => (if heavy || finisher { Sound::HeavySwing } else { Sound::Swing }, 0.9),
                            Style::Reap => (if heavy || finisher { Sound::HeavySwing } else { Sound::Swing }, 0.75),
                            // A thin, high whistle as the lash comes round. It cracks in lash().
                            Style::Lash => (Sound::Swing, if heavy || finisher { 1.25 } else { 1.45 }),
                            // A flail's chain whirring round, its head humming through the air
                            Style::Whirl if heavy => (Sound::HeavySwing, 0.75),
                            Style::Whirl => (Sound::Whirr, if finisher { 0.85 } else { 1.0 }),
                            _ => (if heavy || finisher { Sound::HeavySwing } else { Sound::Swing }, if combo == 1 { 1.08 } else { 1.0 }),
                        };
                        // Lower the heavier it swings
                        let deeper = 1.0 - 0.08 * weight(&profile) as f32;
                        self.cues.push(Cue::here(sound, if finisher { 0.75 } else { 0.6 }).pitched(pitch * deeper * if finisher { 0.9 } else { 1.0 }));
                    }
                    let t = t + dt;
                    let land = !struck && t >= length * lands;
                    self.action = if t >= length { Action::Idle } else { Action::Swing { t, length, heavy, struck: struck || land, combo } };
                    if land {
                        self.melee(arena, &profile, heavy, if finisher { FINISHER_POWER } else { 1.0 }, finisher);
                        // A battle staff's finishing blow, or its heavy one, looses a spell too
                        let cost = profile.stat("manaCost");
                        if profile.style == Style::Glaive && (finisher || heavy) && self.mana >= cost {
                            self.mana -= cost;
                            self.cast(arena, weapon);
                        }
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
                        let last = finisher(style);
                        let finisher = combo == last && !heavy;
                        // A heavy blow that winds up longer takes a little longer all told, and so
                        // does a rare whip's finisher, twirling its lash round after it cracks
                        let gather = if heavy && matches!(profile.heavy, Heavy::Heave | Heavy::Twirl | Heavy::Overhead | Heavy::Lunge) { 1.12 } else { 1.0 };
                        let stretch = if profile.style == Style::Lash { lash_swing(heavy, combo, profile.grade, weight(&profile)).stretch } else { 1.0 };
                        let length = (if heavy { 1.6 * gather } else if finisher { 1.25 } else { 1.0 }) * stretch / (profile.stat("attackSpeed") * speedup).max(0.1);
                        // Light blows go one way, then another, then finish with a harder one. A
                        // maul's all come down overhead: a light one, then a crushing one.
                        let next = if profile.style == Style::Quake {
                            if combo == 0 { last } else { 0 }
                        } else {
                            (combo + 1) % (last + 1)
                        };
                        self.combo = if heavy { 0 } else { next };
                        self.combo_timer = 0.5;
                        self.action = Action::Swing { t: 0.0, length, heavy, struck: false, combo };
                        self.cooldown = length;
                    }
                }
            }
            Kind::Ranged if profile.build != Build::None => self.shoulder(arena, index, weapon, loaded, controls, dt),
            // The right button draws the bow and holds it drawn, the left lets the arrow fly.
            // Letting go of the right lowers it again.
            Kind::Ranged => match self.action {
                Action::Draw { t } => {
                    let draw = self.drawn(&profile);
                    if controls.attack && draw >= 0.15 {
                        self.loose(arena, weapon, loaded, draw);
                        self.action = Action::Loose { t: 0.0 };
                        self.cooldown = 0.25;
                    } else if controls.aiming {
                        self.action = Action::Draw { t: t + dt };
                        // The bow creaks as it comes to full draw
                        if draw < 1.0 && self.drawn(&profile) >= 1.0 {
                            self.cues.push(Cue::here(Sound::Creak, 0.25).pitched(1.5));
                        }
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
                // Raised, the spell gathered, and let go just as the move drawn lets it go, the
                // whole cast as quick as the weapon casts, see cast_length()
                if let Action::Cast { t, length, cast } = self.action {
                    let t = t + dt;
                    let (_, release) = cast_times(profile.casting, profile.style);
                    let let_go = !cast && t >= release * length;
                    if let_go {
                        self.cast(arena, weapon);
                        self.cast_flourish(arena.view, weapon);
                    }
                    self.action = if t >= length { Action::Idle } else { Action::Cast { t, length, cast: cast || let_go } };
                }
                if (controls.attack || controls.holding) && self.cooldown <= 0.0 && self.raise >= 1.0 {
                    let cost = profile.stat("manaCost");
                    if self.mana >= cost {
                        self.mana -= cost;
                        let rate = (profile.stat("castSpeed") * speedup).max(0.1);
                        self.action = Action::Cast { t: 0.0, length: cast_length(rate), cast: false };
                        self.casts = self.casts.wrapping_add(1);
                        self.cooldown = 1.0 / rate;
                    } else if self.mana_warning <= 0.0 {
                        self.messages.push("Not enough mana.".into());
                        self.mana_warning = 2.0;
                    }
                }
            }
        }
    }

    /// Shots loaded in crossbow or gun `index`, `profile`'s
    fn rounds_in(&self, index: usize, profile: &Profile) -> u32 {
        let full = profile.magazine();
        self.loads.get(&index).copied().unwrap_or(full).min(full)
    }

    /// Shooting the crossbow or gun in hand, weapon `index`: the left button shoots straight away
    /// if it's loaded, and once it's empty it's loaded again by itself, which takes a while. The
    /// right button loads it early. A repeater keeps shooting while the button's held, working its
    /// lever between shots.
    fn shoulder(&mut self, arena: &mut Arena, index: usize, weapon: &Weapon, loaded: Option<&Loaded>, controls: Controls, dt: f64) {
        let profile = weapon.profile.clone();
        let full = profile.magazine();
        let left = self.rounds_in(index, &profile);
        let speedup = self.speedup();
        let trigger = controls.attack || (profile.build == Build::Repeater && controls.holding);
        let siege = profile.build == Build::Siege;
        // A harpoon's line has to come back before it can be loaded again
        let line_out = profile.build == Build::Harpoon && (self.tether.is_some() || self.projectiles.iter().any(|p| p.trick == Trick::Harpoon));
        // A siege crossbow let down onto its leg while it's braced and shot, and while it's spanned
        // again with right click held, and folded up otherwise
        if siege {
            let braced = matches!(self.action, Action::Brace { .. } | Action::Fire { .. }) || (matches!(self.action, Action::Reload { .. }) && self.brace > 0.5 && controls.aiming);
            let before = self.brace;
            self.brace = if braced { (self.brace + dt * speedup / BRACE_TIME).min(1.0) } else { (self.brace - dt / 0.35).max(0.0) };
            if before < 1.0 && self.brace >= 1.0 {
                self.cues.push(Cue::here(Sound::Brace, 0.8));
                self.shake = self.shake.max(0.12);
            }
        }
        match self.action {
            Action::Reload { t, length } => {
                let now = t + dt;
                self.reload_sounds(profile.build, t / length, now / length);
                if now >= length {
                    self.loads.insert(index, full);
                    self.action = Action::Idle;
                } else if trigger && left > 0 && !siege {
                    // Loading early, stopped to shoot what's in it
                    self.fire(arena, index, weapon, loaded);
                } else {
                    self.action = Action::Reload { t: now, length };
                }
            }
            // Braced, it looses once it's steady on its leg. Spent, it's spanned again where it
            // stands. Let go of before it's shot, it's lifted off its leg again.
            Action::Brace { t, loose } => {
                let loose = loose || trigger;
                if left == 0 {
                    let length = 1.0 / (profile.stat("reloadSpeed") * speedup).max(0.1);
                    self.action = Action::Reload { t: 0.0, length };
                } else if self.brace >= 1.0 && loose && self.cooldown <= 0.0 {
                    self.fire(arena, index, weapon, loaded);
                } else if controls.aiming || loose {
                    self.action = Action::Brace { t: t + dt, loose };
                } else {
                    self.action = Action::Idle;
                }
            }
            Action::Fire { t, length } => {
                let now = t + dt;
                // A repeater's lever drawn back, catching the string, and pushed home again
                if profile.build == Build::Repeater && left > 0 {
                    for (at, sound, pitch) in [(0.25, Sound::Ratchet, 0.8), (0.7, Sound::Latch, 1.3)] {
                        if t < at * length && now >= at * length {
                            self.cues.push(Cue::here(sound, 0.4).pitched(pitch));
                        }
                    }
                }
                self.action = if now >= length { Action::Idle } else { Action::Fire { t: now, length } };
            }
            _ if self.raise < 1.0 => {}
            _ => {
                if siege && left > 0 && (trigger || controls.aiming) {
                    self.action = Action::Brace { t: 0.0, loose: trigger };
                    self.cues.push(Cue::here(Sound::Creak, 0.5).pitched(0.8));
                } else if trigger && left > 0 && self.cooldown <= 0.0 {
                    self.fire(arena, index, weapon, loaded);
                } else if line_out {
                    // Waiting on the harpoon, or letting go of its line with right click
                    if controls.heavy {
                        self.tether = None;
                        for p in &mut self.projectiles {
                            if p.trick == Trick::Harpoon {
                                p.trick = Trick::None;
                            }
                        }
                        self.cues.push(Cue::here(Sound::Taut, 0.5).pitched(1.4));
                    }
                } else if left == 0 || (controls.heavy && left < full && !siege) {
                    let length = 1.0 / (profile.stat("reloadSpeed") * speedup).max(0.1);
                    self.action = Action::Reload { t: 0.0, length };
                }
            }
        }
    }

    /// The sounds of a crossbow or gun built `build` being loaded, from `from` to `to` of the way
    /// through: a lever creaking over, a windlass's ratchet clicking turn after turn, bolts dropped
    /// into a magazine, or powder poured, the ball rammed home and the hammer cocked, then the
    /// string catching on its latch
    fn reload_sounds(&mut self, build: Build, from: f64, to: f64) {
        let mut at = |f: f64, sound: Sound, volume: f32, pitch: f32| {
            if from < f && to >= f {
                self.cues.push(Cue::here(sound, volume).pitched(pitch));
            }
        };
        match build {
            Build::Lever => {
                at(0.05, Sound::Draw, 0.35, 1.3);
                at(0.45, Sound::Draw, 0.45, 0.9);
                at(0.9, Sound::Latch, 0.6, 1.0);
            }
            Build::Crank => {
                for k in 0..40 {
                    at(0.08 + 0.8 * k as f64 / 40.0, Sound::Ratchet, 0.35, 0.95 + 0.1 * (k % 2) as f32);
                }
                at(0.9, Sound::Latch, 0.7, 0.8);
            }
            Build::Repeater => {
                for k in 0..5 {
                    at(0.2 + 0.12 * k as f64, Sound::Ratchet, 0.4, 0.6);
                }
                at(0.9, Sound::Latch, 0.55, 1.15);
            }
            // A light lever unclipped, a quick creak and a snap onto the nut
            Build::Hand => {
                at(0.04, Sound::Ratchet, 0.35, 1.6);
                at(0.2, Sound::Draw, 0.35, 1.6);
                at(0.48, Sound::Latch, 0.5, 1.5);
                at(0.85, Sound::Ratchet, 0.3, 1.4);
            }
            // The pinion's gears chattering as the rack comes back, both strings catching
            Build::Twin => {
                for k in 0..30 {
                    at(0.06 + 0.8 * k as f64 / 30.0, Sound::Ratchet, 0.28, 1.3 + 0.1 * (k % 3) as f32);
                }
                at(0.88, Sound::Latch, 0.55, 1.05);
                at(0.91, Sound::Latch, 0.5, 0.95);
            }
            // The great screw turned and turned, groaning, then the latch dropping heavily
            Build::Siege => {
                at(0.05, Sound::Creak, 0.6, 0.7);
                for k in 0..18 {
                    at(0.1 + 0.75 * k as f64 / 18.0, Sound::Ratchet, 0.4, 0.7 + 0.05 * (k % 2) as f32);
                }
                at(0.5, Sound::Creak, 0.5, 0.6);
                at(0.9, Sound::Latch, 0.8, 0.6);
            }
            // The line wound back onto its reel, the prod spanned with it
            Build::Harpoon => {
                at(0.05, Sound::Reel, 0.6, 1.0);
                at(0.35, Sound::Reel, 0.55, 0.95);
                at(0.62, Sound::Reel, 0.5, 0.9);
                at(0.9, Sound::Latch, 0.65, 0.85);
            }
            // Five bolts dropped into their grooves, the side lever swung out and hauled back
            Build::Fan => {
                for k in 0..5 {
                    at(0.04 + 0.07 * k as f64, Sound::Ratchet, 0.4, 0.75);
                }
                at(0.45, Sound::Draw, 0.5, 0.7);
                at(0.7, Sound::Creak, 0.45, 0.8);
                at(0.88, Sound::Latch, 0.7, 0.8);
            }
            _ => {
                at(0.12, Sound::Powder, 0.6, 1.0);
                at(0.3, Sound::Ram, 0.3, 1.3);
                at(0.45, Sound::Ram, 0.6, 1.0);
                at(0.63, Sound::Ram, 0.6, 0.9);
                at(0.92, Sound::Cock, 0.6, 1.0);
            }
        }
    }

    /// Shoots the crossbow or gun in hand, weapon `index`, loaded with special ammo `loaded` if
    /// there is any: a bolt loosed with a thwack, or a ball, a spread of shot or a shell fired with
    /// a bang, a flash and a cloud of smoke
    fn fire(&mut self, arena: &mut Arena, index: usize, weapon: &Weapon, loaded: Option<&Loaded>) {
        let view = arena.view;
        let vpm = view.vpm;
        let base = weapon.profile.clone();
        let profile = loaded.map_or_else(|| base.clone(), |l| l.profile.clone());
        let left = self.rounds_in(index, &base);
        self.loads.insert(index, left.saturating_sub(1));
        self.action = Action::Fire { t: 0.0, length: fire_length(&base) / self.speedup() };
        self.cooldown = 0.08;
        if let Some(l) = loaded {
            self.spent.push(l.ammo);
        }

        let muzzle = self.held_point(weapon, view, false);
        let target = self.aim(arena);
        let shot = base.build.shot();
        let speed = profile.stat("projectileSpeed") * vpm;
        let damage = profile.stat("physicalDamage") + profile.stat("elementalDamage");
        let pellets = profile.stat("pellets").round().max(1.0) as usize;
        // A harpoon's always its own barbed head, whatever's loaded
        let harpoon = base.build == Build::Harpoon;
        let head = if harpoon { QUARREL_HARPOON } else { loaded.and_then(|l| l.head).unwrap_or(0) };
        let ahead = (target - muzzle).normalize();
        // Balls drawn as big as the bore, pellets smaller, and bolts as big as the crossbow's
        let size = match shot {
            Shot::Ball => base.caliber / 0.009 * if pellets > 1 { 0.55 } else { 1.0 },
            Shot::Quarrel => base.bolt,
            _ => 1.0,
        };
        let missile = Missile { shot, head, power: pellets == 1 && shot == Shot::Ball || base.build == Build::Siege, size };
        let before = self.projectiles.len();
        let rails = &weapon.look.rails;
        if shot == Shot::Quarrel && rails.len() > 1 {
            // A twin-prod's two bolts from either prod, meeting where it's aimed, or a
            // bolt-thrower's fanned out across what's ahead, as wide as its spread. Only one of
            // them bursts, so a volley of bursting bolts doesn't blow the ground away.
            let n = rails.len();
            let share = if base.build == Build::Fan { FAN_SHARE } else { TWIN_SHARE };
            let fan = profile.stat("spread").max(0.0).to_radians() / FAN_YAW;
            let quiet = Rc::new(profile.without(Effect::Explosive));
            let up = view.rotation * Vector3::z_axis();
            for (k, &(offset, yaw)) in rails.iter().enumerate() {
                let from = self.held_at(weapon, view, weapon.look.tip + offset);
                let direction = UnitQuaternion::from_axis_angle(&up, yaw * fan) * (target - from).normalize();
                let profile = if k == 0 { &profile } else { &quiet };
                self.shoot(view, profile, Some(missile), Form::Bolt, from, direction, speed * (1.0 - 0.02 * k as f64), damage * share / n as f64);
            }
        } else if pellets > 1 && shot != Shot::Quarrel {
            // A spread of shot, each pellet off a little way at random. Only one of them bursts,
            // so a spread of bursting shot doesn't blow the ground away.
            let spread = if profile.stat("spread") > 0.0 { profile.stat("spread") } else { PELLET_SPREAD }.to_radians();
            let quiet = Rc::new(profile.without(Effect::Explosive));
            let right = view.rotation * Vector3::x();
            let up = view.rotation * Vector3::z();
            for k in 0..pellets {
                let a = random(&mut self.rng) * TAU;
                let r = random(&mut self.rng).sqrt() * spread;
                let direction = (ahead + (right * a.cos() + up * a.sin()) * r.tan()).normalize();
                let speed = speed * (0.9 + 0.2 * random(&mut self.rng));
                let profile = if k == 0 { &profile } else { &quiet };
                self.shoot(view, profile, Some(missile), Form::Bolt, muzzle, direction, speed, damage * PELLETS_SHARE / pellets as f64);
            }
        } else {
            self.shoot(view, &profile, Some(missile), Form::Bolt, muzzle, ahead, speed, damage);
        }
        // A blunderbuss's shot only carries so far
        let range = profile.stat("range");
        if range > 0.0 {
            for p in &mut self.projectiles[before..] {
                p.life = p.life.min(range * vpm / p.velocity.norm().max(1.0));
            }
        }

        // Bursting ammo bursts out of the shot that carries its charge, the first: in the air most
        // of the way to what it's aimed at, if it bursts in the air, or else where it strikes
        if let Some(burst) = loaded.and_then(|l| l.burst)
            && !harpoon
            && let Some(first) = self.projectiles.get_mut(before)
        {
            let fuse = if burst.airburst {
                let reach = (target - muzzle).norm().min(AIRBURST_REACH * vpm);
                (reach * AIRBURST_AT / first.velocity.norm().max(1.0)).max(AIRBURST_SOONEST)
            } else {
                f64::MAX
            };
            first.trick = Trick::Burst { fuse, burst };
        }
        // A harpoon trails its line, a siege crossbow's great bolt pins what it goes through
        if let Some(first) = self.projectiles.get_mut(before)
            && harpoon
        {
            first.trick = Trick::Harpoon;
            first.life = HARPOON_REACH * vpm / speed.max(1.0);
        }
        if base.build == Build::Siege {
            for p in &mut self.projectiles[before..] {
                p.pin = p.pin.max(0.9);
            }
        }

        let kick = kick_of(base.build);
        self.kick_speed += 3.4 * kick * if shot == Shot::Quarrel { 0.3 } else { 1.0 };
        if shot == Shot::Quarrel {
            match base.build {
                Build::Hand => self.cues.push(Cue::here(Sound::Snap, 0.7)),
                Build::Twin => {
                    self.cues.push(Cue::here(Sound::Release, 0.75).pitched(0.95));
                    self.cues.push(Cue::here(Sound::Release, 0.6).pitched(1.08));
                }
                Build::Fan => self.cues.push(Cue::here(Sound::Volley, 0.9)),
                Build::Harpoon => self.cues.push(Cue::here(Sound::Harpoon, 0.9)),
                Build::Siege => {
                    self.cues.push(Cue::here(Sound::Release, 1.0).pitched(0.5));
                    self.cues.push(Cue::here(Sound::Taut, 0.7).pitched(0.6));
                    self.shake = self.shake.max(0.35);
                }
                build => self.cues.push(Cue::here(Sound::Release, 0.8).pitched(match build {
                    Build::Crank => 0.8,
                    Build::Repeater => 1.25,
                    _ => 1.0,
                })),
            }
            if profile.element.is_some() {
                self.sparks(muzzle, 4, 1.0, Element::look(profile.element), 0.0, 0.3);
            }
            return;
        }
        // A bang, the bigger the gun the deeper, a flash lighting everything round, the flame at
        // the muzzle, sparks and a cloud of smoke rolling out ahead and hanging there
        let bore = base.caliber / 0.009;
        let pitch = match base.build {
            Build::Pistol => 1.2,
            Build::Blunderbuss => 0.85,
            Build::Cannon => 0.62,
            _ => 1.0,
        };
        self.cues.push(Cue::here(Sound::Gunshot, 1.0).pitched(pitch));
        self.shake = self.shake.max(0.18 * kick);
        self.flare = FLARE;
        self.flare_roll = random(&mut self.rng) * TAU;
        self.flashes.push(Flash { position: muzzle, color: [1.0, 0.72, 0.38], reach: 7.0 + 3.0 * bore.min(3.0), life: 0.09, lifetime: 0.09 });
        let look = Element::look(profile.element);
        let puffs = (6.0 + 2.0 * bore.min(4.0)) as usize;
        for k in 0..puffs {
            let out = (ahead * (0.6 + 2.6 * random(&mut self.rng)) + self.any_direction() * 0.7) * (1.0 + 0.3 * bore.min(3.0));
            let life = 1.1 + 0.9 * random(&mut self.rng);
            let at = muzzle + ahead * (0.04 * k as f64) * vpm;
            self.sparks.push(Spark { position: at, velocity: out * vpm, look, life, lifetime: life, gravity: -0.03, slot: SLOT_SMOKE });
        }
        for _ in 0..6 {
            let v = (ahead * 4.0 + self.any_direction() * 1.5) * vpm;
            self.fine(muzzle, v, Element::look(Some(Element::Fire)), 0.12, 0.3);
        }
        // A puff from the pan as the priming goes off
        let pan = self.held_at(weapon, view, weapon.look.grip + Vector3::new(0.06, 0.03, 0.03));
        self.sparks.push(Spark { position: pan, velocity: Vector3::new(0.0, 0.0, 0.3) * vpm, look, life: 0.8, lifetime: 0.8, gravity: -0.03, slot: SLOT_SMOKE });
        self.cap_sparks();
    }

    /// Pouring the beam of the orb in hand out at the crosshair for as long as the button's held,
    /// its mana cost going each second
    fn channel(&mut self, arena: &mut Arena, weapon: &Weapon, controls: Controls, dt: f64) {
        let profile = weapon.profile.clone();
        let cost = profile.stat("manaCost") * dt;
        let held = controls.attack || controls.holding;
        if !held || self.raise < 1.0 || self.mana < cost {
            if held && self.mana < cost && self.mana_warning <= 0.0 {
                self.messages.push("Not enough mana.".into());
                self.mana_warning = 2.0;
            }
            if matches!(self.action, Action::Channel { .. }) {
                self.action = Action::Idle;
            }
            return;
        }
        self.mana -= cost;
        let t = if let Action::Channel { t } = self.action { t + dt } else { 0.0 };
        self.action = Action::Channel { t };
        let vpm = arena.view.vpm;
        let from = self.held_point(weapon, arena.view, false);
        let target = self.aim(arena);
        let reach = profile.stat("range").max(4.0) * vpm;
        let to = from + (target - from).normalize() * (target - from).norm().min(reach);
        let dps = (profile.stat("spellDamage") + profile.stat("elementalDamage")) * profile.stat("castSpeed").max(0.5) * BEAM_POWER * self.speedup();
        self.pour(arena, &profile, from, to, dps, dt, 0.0);
    }

    /// A beam from `from` to `to` this frame: sparks streaming along it as far as the first thing
    /// in its way, a glow where it lands, and every BEAM_TICK seconds that thing hurt by `dps` a
    /// second's worth. With `drain`, that much of what it does comes back as health.
    #[allow(clippy::too_many_arguments)]
    fn pour(&mut self, arena: &mut Arena, profile: &Rc<Profile>, from: Vector3<f64>, to: Vector3<f64>, dps: f64, dt: f64, drain: f64) -> Option<usize> {
        let vpm = arena.view.vpm;
        let mut end = 1.0;
        let mut hit = None;
        for (i, a) in arena.animals.iter().enumerate().filter(|(_, a)| a.health > 0.0 && !a.look.is_pet()) {
            if let Some(t) = sphere_hit(from, to, a.center(vpm as u32), a.look.body().1 * vpm).filter(|&t| t < end) {
                end = t;
                hit = Some(i);
            }
        }
        let end = from + (to - from) * end;
        let look = Element::look(profile.element);
        // A stream of sparks, wavering, a fresh lot each frame: fine ones up close, ones the size
        // of the world's voxels further out, where fine ones would be too small to see
        let steps = ((end - from).norm() / (0.45 * vpm)).clamp(2.0, 20.0) as usize;
        let wave = self.time * 23.0;
        for k in 0..=steps {
            let f = k as f64 / steps as f64;
            let wobble = Vector3::new((wave + f * 9.0).sin(), (wave * 1.3 + f * 7.0).cos(), (wave * 0.7 + f * 11.0).sin()) * 0.04 * vpm * (f * (1.0 - f) * 4.0);
            let at = from.lerp(&end, f) + wobble;
            if (at - from).norm() < 1.5 * vpm {
                self.fine(at, Vector3::zeros(), look, 0.05, 0.0);
            } else {
                self.sparks.push(Spark { position: at, velocity: Vector3::zeros(), look, life: 0.04, lifetime: 0.04, gravity: 0.0, slot: SLOT_SPARK });
            }
        }
        self.cap_sparks();
        if random(&mut self.rng) < dt * 20.0 {
            self.mote(end, profile.element, 1.0);
        }
        self.flashes.push(Flash { position: end, color: profile.element.map_or([0.6, 0.85, 1.0], Element::light), reach: 3.0, life: dt * 1.5, lifetime: dt * 1.5 });
        self.beam_hum -= dt;
        if self.beam_hum <= 0.0 {
            self.beam_hum = 0.3;
            self.cues.push(Cue::here(Sound::Beam, 0.35).pitched(if drain > 0.0 { 0.7 } else { 1.0 }));
        }
        self.beam_tick -= dt;
        if self.beam_tick > 0.0 {
            return None;
        }
        self.beam_tick += BEAM_TICK;
        if let Some(i) = hit {
            let crit = self.roll_crit(profile);
            let damage = dps * BEAM_TICK * if crit { profile.stat("critDamage") } else { 1.0 };
            self.element_hit(end, profile.element, 0.6);
            self.strike(arena, profile, i, Blow { damage, crit, heavy: false, from, depth: 0, element: profile.element });
            self.heal += damage * drain;
        }
        hit.filter(|&i| arena.animals[i].health > 0.0)
    }

    /// Ringing the bell in hand: held, it's raised to ring harder, up to a full toll in a second or
    /// so; let go, it rings out in a shockwave all round, as wide and hard as it was rung
    fn toll(&mut self, arena: &mut Arena, weapon: &Weapon, controls: Controls, dt: f64) {
        let profile = weapon.profile.clone();
        let full = 1.0 / (profile.stat("castSpeed") * self.speedup()).max(0.2);
        match self.action {
            Action::Charge { t } if controls.holding || controls.attack => {
                let t = t + dt;
                // Light gathering in to the bell as it's raised
                if random(&mut self.rng) < dt * 30.0 {
                    let mouth = self.held_point(weapon, arena.view, false);
                    let out = self.any_direction() * 0.5 * arena.view.vpm;
                    self.fine(mouth + out, -out * 3.0, Element::look(profile.element), 0.3, 0.0);
                }
                if t < full && t + dt >= full {
                    self.cues.push(Cue::here(Sound::Pickup, 0.4).pitched(0.7));
                }
                self.action = Action::Charge { t };
            }
            Action::Charge { t } => {
                let rung = (t / full).clamp(0.25, 1.0);
                let cost = profile.stat("manaCost") * rung;
                if self.mana >= cost {
                    self.mana -= cost;
                    self.ring_out(arena, &profile, rung);
                } else if self.mana_warning <= 0.0 {
                    self.messages.push("Not enough mana.".into());
                    self.mana_warning = 2.0;
                }
                self.action = Action::Cast { t: 0.0, length: CAST_TIME, cast: true };
                self.cooldown = 0.35;
            }
            Action::Cast { t, length, cast } => self.action = if t + dt >= length { Action::Idle } else { Action::Cast { t: t + dt, length, cast } },
            _ => {
                if controls.attack && self.cooldown <= 0.0 && self.raise >= 1.0 {
                    self.action = Action::Charge { t: 0.0 };
                }
            }
        }
    }

    /// A bell's toll, rung `rung` of the way, 0-1: a ring of light and a shockwave sweeping out
    /// round the player, hurting and throwing back everything it reaches
    fn ring_out(&mut self, arena: &mut Arena, profile: &Rc<Profile>, rung: f64) {
        let view = arena.view;
        let vpm = view.vpm;
        let feet = arena.player.unwrap_or(view.eye - Vector3::new(0.0, 0.0, 1.6 * vpm));
        let middle = feet + Vector3::new(0.0, 0.0, 1.0 * vpm);
        let radius = (3.0 + 4.5 * rung) * profile.stat("area").max(0.5);
        let look = Element::look(profile.element);
        for k in 0..36 {
            let a = k as f64 / 36.0 * TAU;
            let out = Vector3::new(a.cos(), a.sin(), 0.0);
            self.sparks.push(Spark { position: middle + out * 0.6 * vpm, velocity: out * radius * 2.2 * vpm, look, life: 0.45, lifetime: 0.45, gravity: 0.0, slot: SLOT_SPARK });
        }
        self.cap_sparks();
        self.flashes.push(Flash { position: middle, color: profile.element.map_or([0.9, 0.85, 0.7], Element::light), reach: radius + 6.0, life: 0.3, lifetime: 0.3 });
        self.cues.push(Cue::here(Sound::Toll, (0.6 + 0.4 * rung) as f32).pitched((1.2 - 0.35 * rung) as f32));
        self.shake = self.shake.max(0.25 + 0.4 * rung);
        let damage = (profile.stat("spellDamage") + profile.stat("elementalDamage")) * rung;
        let push = profile.stat("knockback").max(3.0) * (0.5 + rung);
        let near: Vec<(usize, f64)> = (0..arena.animals.len())
            .filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet())
            .map(|i| (i, ((arena.animals[i].position - feet).norm() / vpm - arena.animals[i].look.body().1).max(0.0)))
            .filter(|&(_, d)| d < radius)
            .collect();
        for (i, d) in near {
            let away = arena.animals[i].position - feet;
            let at = arena.animals[i].center(vpm as u32);
            self.element_hit(at, profile.element, 0.8 + rung);
            self.strike(arena, profile, i, Blow { damage: damage * (1.0 - 0.5 * d / radius), crit: false, heavy: rung > 0.8, from: feet, depth: 0, element: profile.element });
            arena.animals[i].status.push = Vector3::new(away.x, away.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x()) * push * vpm;
        }
    }

    /// The bombs in hand: one drawn back and lobbed in an arc to come down on the crosshair, as
    /// fast as they can be while the button's held. With the right button held a ring shows where
    /// it'll come down.
    fn lob(&mut self, arena: &mut Arena, weapon: &Weapon, controls: Controls, dt: f64) {
        let profile = weapon.profile.clone();
        if controls.aiming && self.raise >= 1.0 {
            let (at, ..) = self.lob_aim(arena, weapon);
            self.marks.push((at, BOMB_BLAST * profile.stat("area").max(0.5)));
        }
        match self.action {
            Action::Throw { t, length, thrown } => {
                let t = t + dt;
                let release = BOMB_RELEASE * length;
                if !thrown && t >= release {
                    self.throw_bomb(arena, weapon);
                }
                if t >= length {
                    // A fresh one comes up into the hand
                    self.action = Action::Idle;
                    self.raise = 0.35;
                } else {
                    self.action = Action::Throw { t, length, thrown: thrown || t >= release };
                }
            }
            _ => {
                if (controls.attack || controls.holding) && self.cooldown <= 0.0 && self.raise >= 1.0 {
                    let speedup = self.speedup();
                    // Thrown quicker the quicker its throws come, a heavy one heaved slower, and it
                    // leaves the hand as the arm comes over, see target_pose()
                    let length = (0.6 / profile.stat("attackSpeed").max(0.1).sqrt()).clamp(0.45, 0.8) / speedup;
                    self.action = Action::Throw { t: 0.0, length, thrown: false };
                    self.cooldown = (1.0 / (profile.stat("attackSpeed") * speedup).max(0.2)).max(length);
                    // Its fuse lit with a hiss as it's drawn back
                    self.cues.push(Cue::here(Sound::Fuse, 0.5));
                }
            }
        }
    }

    /// Where a bomb lobbed now would come down, where it leaves the hand, how fast it's thrown, in
    /// voxels a second, and how long it's in the air: onto the crosshair, or as far as it can be
    /// thrown that way
    fn lob_aim(&self, arena: &Arena, weapon: &Weapon) -> (Vector3<f64>, Vector3<f64>, Vector3<f64>, f64) {
        let view = arena.view;
        let vpm = view.vpm;
        let from = view.eye + view.rotation * Vector3::new(0.25, 0.45, 0.05) * vpm;
        let mut target = self.aim(arena);
        let reach = weapon.profile.stat("range").max(6.0) * vpm;
        if (target - from).norm() > reach {
            let out = from + (target - from).normalize() * reach;
            target = ground_under(arena.world, out, vpm);
        }
        let (velocity, time) = lobbed(from, target, vpm);
        (target, from, velocity, time)
    }

    /// The bomb in hand lobbed, its fuse burning down to go off a moment after it lands
    fn throw_bomb(&mut self, arena: &mut Arena, weapon: &Weapon) {
        let profile = weapon.profile.clone();
        let (_, from, velocity, time) = self.lob_aim(arena, weapon);
        let damage = profile.stat("physicalDamage") + profile.stat("elementalDamage");
        let cluster = if profile.has(Effect::Cluster) { (profile.param(Effect::Cluster, "count", 4.0) * profile.potency(Effect::Cluster).max(1.0)).round().min(8.0) as u8 } else { 0 };
        self.launch(&profile, Some(Missile { shot: Shot::Shell, head: 0, power: false, size: 1.0 }), Form::Bolt, from, velocity, damage, BOMB_GRAVITY);
        if let Some(bomb) = self.projectiles.last_mut() {
            bomb.trick = Trick::Bomb { fuse: time + BOMB_FUSE, cluster, hole: true };
            bomb.life = time + 3.0;
        }
        self.cues.push(Cue::here(Sound::Swing, 0.55).pitched(0.75));
    }

    /// A bomb going off at `at`: a blast as wide as its area, everything in it taking the bomb's
    /// effects, a small hole if it was thrown by hand, and bomblets flying if it carries them.
    /// What it leaves on the ground is drawn, never painted onto it.
    fn bomb_burst(&mut self, arena: &mut Arena, p: &Projectile, at: Vector3<f64>) {
        let Trick::Bomb { cluster, hole, .. } = p.trick else {
            return;
        };
        let profile = p.profile.clone();
        let bomblet = p.size < 0.9;
        let radius = BOMB_BLAST * profile.stat("area").max(0.8) * p.size.min(1.3) * if bomblet { 0.9 } else { 1.0 };
        self.scarless = true;
        self.burst(arena, &profile, at, radius, p.damage, bomblet as u32, false, profile.element.or(Some(Element::Fire)), None);
        self.scarless = false;
        // A bomblet blows its own small hole, and a bomb packed with them a bigger one
        if hole {
            self.craters.push((at, if bomblet { BOMBLET_HOLE } else if cluster > 0 { BOMB_HOLE * CLUSTER_BLAST } else { BOMB_HOLE }));
        }
        if !bomblet {
            self.landing(arena, &profile, at, p.damage, None, false);
        }
        if cluster > 0 {
            let power = profile.param(Effect::Cluster, "power", 0.35);
            self.scatter(&profile, at, cluster as usize, p.damage * power);
        }
    }

    /// `n` bomblets flung out from `at`, each going off a moment later where it lands for `damage`
    /// and blowing a small hole there
    fn scatter(&mut self, profile: &Rc<Profile>, at: Vector3<f64>, n: usize, damage: f64) {
        let vpm = self.vpm;
        let quiet = Rc::new(profile.without(Effect::Cluster));
        for k in 0..n.min(8) {
            let a = (k as f64 + 0.6 * random(&mut self.rng)) / n as f64 * TAU;
            let out = 3.5 + 3.5 * random(&mut self.rng);
            let v = Vector3::new(a.cos() * out, a.sin() * out, 5.0 + 3.0 * random(&mut self.rng)) * vpm;
            let fuse = 0.5 + 0.4 * random(&mut self.rng);
            self.launch(&quiet, Some(Missile { shot: Shot::Shell, head: 0, power: false, size: BOMBLET_SIZE }), Form::Bolt, at + Vector3::new(0.0, 0.0, 0.5 * vpm), v, damage, BOMB_GRAVITY);
            if let Some(b) = self.projectiles.last_mut() {
                b.trick = Trick::Bomb { fuse, cluster: 0, hole: true };
                b.life = 2.5;
            }
        }
        self.cues.push(Cue::at(Sound::Chain, at, 0.5).pitched(0.6));
    }

    /// Bursting ammo `p` bursting in the air at `at` as its fuse runs out: a crack and a flash, a
    /// puff of fire hurting what's right round it, and a spray of burning shot flying on the way
    /// it was going, spreading as `burst` has it, each bursting again where it lands. It leaves
    /// nothing on the ground itself, being up in the air.
    fn airburst(&mut self, arena: &mut Arena, p: &Projectile, at: Vector3<f64>, burst: Burst) {
        let vpm = arena.view.vpm;
        let element = p.profile.element.or(Some(Element::Fire));
        let look = Element::look(element);
        // A quick white-hot crack rather than a rolling fireball, so it doesn't hide its shards
        // flying on behind it, and a starburst of sparks
        self.blasts.push(Blast { position: at, look, radius: 0.55, life: 0.18, lifetime: 0.18 });
        self.flashes.push(Flash { position: at, color: element.map_or([1.0, 0.8, 0.5], Element::light), reach: 9.0, life: 0.2, lifetime: 0.2 });
        self.sparks(at, 12, 7.0, look, 0.3, 0.3);
        self.cues.push(Cue::at(Sound::Burst, at, 0.8).pitched(1.15));
        let near = (1.0 - (at - arena.view.eye).norm() / vpm / 30.0).max(0.0);
        self.shake = self.shake.max(near * 0.3);
        let r = AIRBURST_BLAST * vpm;
        let inside: Vec<(usize, f64)> = arena
            .animals
            .iter()
            .enumerate()
            .filter(|(_, a)| a.health > 0.0 && !a.look.is_pet())
            .map(|(i, a)| (i, ((a.center(vpm as u32) - at).norm() - a.look.body().1 * vpm).max(0.0)))
            .filter(|&(_, d)| d < r)
            .collect();
        let profile = p.profile.clone();
        for (i, d) in inside {
            self.strike(arena, &profile, i, Blow { damage: p.damage * AIRBURST_POWER * (1.0 - 0.5 * d / r), crit: false, heavy: false, from: at, depth: 0, element });
        }
        let speed = (p.velocity.norm() * AIRBURST_SHARDS).clamp(22.0 * vpm, 50.0 * vpm);
        self.fling_fragments(p, at, p.velocity, burst.spread, speed, BALL_GRAVITY, burst);
        self.cap_sparks();
    }

    /// Bursting ammo `p` bursting where it strikes at `at`: the blast of its charge, as any
    /// explosive shot's, and its iron thrown back out and up off what it hit, roughly `off`, in
    /// burning shards fanning out wide, each bursting again where it lands
    fn shatter(&mut self, arena: &mut Arena, p: &Projectile, at: Vector3<f64>, off: Vector3<f64>, burst: Burst) {
        let vpm = arena.view.vpm;
        self.impact(arena, p, at);
        let off = off.try_normalize(1e-6).unwrap_or_else(Vector3::z);
        let way = (off + Vector3::new(0.0, 0.0, 0.8)).normalize();
        self.fling_fragments(p, at + off * 0.3 * vpm, way, burst.spread.max(50.0), STRIKE_SHARDS * vpm, 1.0, burst);
        self.cues.push(Cue::at(Sound::Ricochet, at, 0.5).pitched(0.7));
    }

    /// The shards bursting ammo `p` flies apart into at `at`, as many as `burst` has it, spread
    /// evenly over a cone `spread` degrees round `way`, flying at about `speed`, in voxels a
    /// second, and falling `gravity` as fast as things do, each a Trick::Fragment doing `burst`'s
    /// share of `p`'s damage and bursting again where it lands, see pop(). None of them hits what
    /// `p` hit already.
    #[allow(clippy::too_many_arguments)]
    fn fling_fragments(&mut self, p: &Projectile, at: Vector3<f64>, way: Vector3<f64>, spread: f64, speed: f64, gravity: f64, burst: Burst) {
        let quiet = Rc::new(p.profile.without(Effect::Explosive).without(Effect::Cluster));
        let n = burst.fragments.clamp(1, 12);
        for k in 0..n {
            let d = shard_way(way, spread, k, n, random(&mut self.rng) * 0.5);
            let velocity = d * speed * (0.85 + 0.3 * random(&mut self.rng));
            let mut shard = self.offshoot(p, at + d * 0.15 * self.vpm, velocity, p.damage * burst.power, Trick::Fragment);
            shard.profile = quiet.clone();
            shard.shot = Shot::Ball;
            shard.gravity = gravity;
            shard.size = FRAGMENT_SIZE;
            shard.life = 1.4;
            shard.power = false;
            self.projectiles.push(shard);
        }
    }

    /// A shard of bursting ammo `p` going off in a little blast where it lands at `at`, or in
    /// animal `struck`: a puff of fire and a crack, what's right round hurt a little, and on the
    /// ground a small hole, if no other blast has blown one just now
    fn pop(&mut self, arena: &mut Arena, p: &Projectile, at: Vector3<f64>, struck: Option<usize>) {
        let vpm = arena.view.vpm;
        let element = p.profile.element.or(Some(Element::Fire));
        let look = Element::look(element);
        let back = p.velocity.try_normalize(1e-9).unwrap_or_else(Vector3::z);
        let puff = at - back * 0.25 * vpm;
        self.blasts.push(Blast { position: puff, look, radius: FRAGMENT_BLAST * 0.55, life: 0.22, lifetime: 0.22 });
        self.flashes.push(Flash { position: puff, color: element.map_or([1.0, 0.8, 0.5], Element::light), reach: 3.0, life: 0.12, lifetime: 0.12 });
        self.sparks(puff, 3, 3.0, look, 0.4, 0.25);
        self.cues.push(Cue::at(Sound::Burst, at, 0.3).pitched(1.45 + 0.3 * random(&mut self.rng) as f32));
        let r = FRAGMENT_BLAST * vpm;
        let near: Vec<usize> = (0..arena.animals.len())
            .filter(|&i| {
                let a = &arena.animals[i];
                Some(i) != struck && a.health > 0.0 && !a.look.is_pet() && (a.center(vpm as u32) - at).norm() - a.look.body().1 * vpm < r
            })
            .collect();
        let profile = p.profile.clone();
        for i in near {
            self.strike(arena, &profile, i, Blow { damage: p.damage * POP_SHARE, crit: false, heavy: false, from: at, depth: 1, element });
        }
        if struck.is_none() && !self.scarless && self.scar_timer <= 0.0 {
            self.craters.push((at, FRAGMENT_HOLE));
            self.scar_timer = SCAR_GAP;
        }
        self.cap_sparks();
    }

    /// What `profile`'s effects do round where something of it lands at `at`, doing `damage`, or
    /// strikes animal `struck`: splashing onto what's round, dragging it in, and leaving a pool.
    /// With `splash` it splashes as a shot does.
    fn landing(&mut self, arena: &mut Arena, profile: &Rc<Profile>, at: Vector3<f64>, damage: f64, struck: Option<usize>, splash: bool) {
        if splash && profile.has(Effect::Splash) {
            self.splash_hit(arena, profile, at, damage, struck);
        }
        if profile.has(Effect::Vortex) {
            self.vortex(arena, profile, at);
        }
        if profile.has(Effect::Pool) {
            self.leave_pool(arena, profile, at, damage);
        }
    }

    /// Everything round `at` dragged in towards it, as far as `profile`'s vortex reaches
    fn vortex(&mut self, arena: &mut Arena, profile: &Rc<Profile>, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let r = profile.param(Effect::Vortex, "radius", 3.5) * profile.stat("area").max(1.0) * profile.potency(Effect::Vortex).max(1.0).sqrt();
        for a in arena.animals.iter_mut().filter(|a| a.health > 0.0 && !a.look.is_pet() && a.boss.is_none()) {
            let to = at - a.position;
            let d = to.xy().norm() / vpm - a.look.body().1;
            if d < r && d > 0.4 {
                // Thrown in so it comes to rest about where it was drawn to
                a.status.push = Vector3::new(to.x, to.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros) * (d * PUSH_DECAY).min(20.0) * vpm;
            }
        }
        self.sparks(at, 14, -2.5 * r / 3.5, Element::look(Some(Element::Void)), 0.0, 0.45);
        self.cues.push(Cue::at(Sound::Burst, at, 0.35).pitched(1.45));
    }

    /// A pool of `profile`'s element left on the ground under `at`, hurting what stands in it,
    /// though not more often than every so often
    fn leave_pool(&mut self, arena: &mut Arena, profile: &Rc<Profile>, at: Vector3<f64>, damage: f64) {
        if self.pool_timer > 0.0 {
            return;
        }
        let vpm = arena.view.vpm;
        let top = at + Vector3::new(0.0, 0.0, 0.5 * vpm);
        let down = Vector3::new(0.0, 0.0, -4.0 * vpm);
        let Some(t) = solid_along(arena.world, top, top + down) else {
            return;
        };
        self.pool_timer = 0.45;
        let ground = top + down * t;
        let element = profile.element.unwrap_or(Element::Fire);
        let radius = profile.param(Effect::Pool, "radius", 1.5) * profile.stat("area").max(1.0);
        let life = profile.param(Effect::Pool, "duration", 4.0);
        let tick = damage * profile.param(Effect::Pool, "power", 0.2) * profile.potency(Effect::Pool).max(1.0);
        self.keep_pools_few();
        self.lingering.push(Lingering { pool: true, cloud: false, tall: false, position: ground, radius, life, element, profile: profile.clone(), damage: tick, tick: 0.2, effects: false, collapse: 0.0 });
        self.element_hit(ground + Vector3::new(0.0, 0.0, 0.2 * vpm), Some(element), 0.7);
    }

    /// The oldest pool or cloud gone if there are as many as there can be
    fn keep_pools_few(&mut self) {
        let left = |l: &Lingering| l.pool || l.cloud;
        if self.lingering.iter().filter(|l| left(l)).count() >= MOST_POOLS
            && let Some(k) = self.lingering.iter().position(left)
        {
            self.lingering.remove(k);
        }
    }

    /// A shot splashing out from `at` onto everything within its splash but `skip`, for a share of
    /// `damage`: a ring of its element thrown out, blowing no hole
    fn splash_hit(&mut self, arena: &mut Arena, profile: &Rc<Profile>, at: Vector3<f64>, damage: f64, skip: Option<usize>) {
        let vpm = arena.view.vpm;
        let r = profile.param(Effect::Splash, "radius", 1.8) * profile.stat("area").max(1.0);
        let share = profile.param(Effect::Splash, "power", 0.4) * profile.potency(Effect::Splash).max(1.0);
        let look = Element::look(profile.element);
        for k in 0..14 {
            let a = k as f64 / 14.0 * TAU;
            self.sparks.push(Spark { position: at, velocity: Vector3::new(a.cos(), a.sin(), 0.25) * r * 2.2 * vpm, look, life: 0.28, lifetime: 0.28, gravity: 0.6, slot: SLOT_SPARK });
        }
        self.cap_sparks();
        self.flashes.push(Flash { position: at, color: profile.element.map_or([1.0, 0.85, 0.6], Element::light), reach: r + 2.5, life: 0.12, lifetime: 0.12 });
        self.cues.push(Cue::at(Sound::Burst, at, 0.3).pitched(1.7));
        let inside: Vec<usize> = (0..arena.animals.len())
            .filter(|&i| Some(i) != skip && arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet())
            .filter(|&i| (arena.animals[i].center(vpm as u32) - at).norm() < (r + arena.animals[i].look.body().1) * vpm)
            .collect();
        for i in inside {
            self.strike(arena, profile, i, Blow { damage: damage * share, crit: false, heavy: false, from: at, depth: 1, element: profile.element });
        }
    }

    /// Animal `i`, killed by `profile`, bursting into a cloud of its element that hangs there
    fn miasma(&mut self, arena: &Arena, profile: &Rc<Profile>, i: usize) {
        let vpm = arena.view.vpm;
        let a = &arena.animals[i];
        let at = a.position + Vector3::new(0.0, 0.0, 0.2 * vpm);
        let element = profile.element.unwrap_or(Element::Poison);
        let radius = profile.param(Effect::Miasma, "radius", 2.5) * profile.stat("area").max(1.0);
        let life = profile.param(Effect::Miasma, "duration", 4.0);
        let damage = (profile.stat("physicalDamage") + profile.stat("spellDamage") + profile.stat("elementalDamage")) * profile.param(Effect::Miasma, "power", 0.25) * profile.potency(Effect::Miasma).max(1.0);
        self.keep_pools_few();
        self.lingering.push(Lingering { pool: false, cloud: true, tall: false, position: at, radius, life, element, profile: profile.clone(), damage, tick: 0.3, effects: false, collapse: 0.0 });
        let center = a.center(vpm as u32);
        self.sparks(center, 16, 2.2, Element::look(Some(element)), -0.1, 0.8);
        for _ in 0..4 {
            let v = (self.any_direction() * 0.8 + Vector3::new(0.0, 0.0, 0.4)) * vpm;
            let life = 1.4 + 0.6 * random(&mut self.rng);
            self.sparks.push(Spark { position: center, velocity: v, look: Element::look(Some(element)), life, lifetime: life, gravity: -0.02, slot: SLOT_CLOUD });
        }
        self.cap_sparks();
        self.cues.push(Cue::at(Sound::Hiss, at, 0.55).pitched(0.75));
    }

    /// Lightning from the rod in hand, from `from`: to whatever's nearest the crosshair within its
    /// reach, then leaping on through what's near it, each leap a little weaker; or into the
    /// ground where it's aimed, if there's nothing to strike
    fn zap(&mut self, arena: &mut Arena, profile: &Rc<Profile>, from: Vector3<f64>, target: Vector3<f64>, damage: f64) {
        let view = arena.view;
        let vpm = view.vpm;
        let forward = view.forward();
        let reach = profile.stat("range").max(6.0) * vpm;
        let look = Element::look(profile.element.or(Some(Element::Storm)));
        let color = profile.element.map_or(Element::Storm.light(), Element::light);
        // Whatever's nearest the line of the crosshair, not through walls
        let first = (0..arena.animals.len())
            .filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet())
            .filter_map(|i| {
                let a = &arena.animals[i];
                let to = a.center(vpm as u32) - view.eye;
                let along = to.dot(&forward);
                let off = (to - forward * along).norm();
                let body = a.look.body().1 * vpm;
                (along > 0.0 && to.norm() < reach + body && off < 1.2 * vpm + body + along * 0.1).then_some((off / along.max(vpm), i))
            })
            .filter(|&(_, i)| solid_along(arena.world, from, arena.animals[i].center(vpm as u32)).is_none_or(|t| t > 0.9))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, i)| i);
        self.cues.push(Cue::here(Sound::Zap, 0.6));
        let Some(first) = first else {
            self.zap_line(from, target, look);
            self.flashes.push(Flash { position: target, color, reach: 5.0, life: 0.12, lifetime: 0.12 });
            self.scarless = true;
            self.burst(arena, profile, target, 1.3, damage * 0.6, 0, false, profile.element.or(Some(Element::Storm)), None);
            self.scarless = false;
            return;
        };
        let hops = 3 + profile.grade / 2 + (profile.potency(Effect::Chain).round() as usize).min(3);
        let range = 7.0 * profile.stat("area").max(0.7) * vpm;
        let mut struck: Vec<u32> = Vec::new();
        let (mut at, mut i, mut power) = (from, first, 1.0);
        for hop in 0..=hops {
            let center = arena.animals[i].center(vpm as u32);
            self.zap_line(at, center, look);
            self.flashes.push(Flash { position: center, color, reach: 4.0, life: 0.12, lifetime: 0.12 });
            let crit = self.roll_crit(profile);
            let hit = damage * power * if crit { profile.stat("critDamage") } else { 1.0 };
            struck.push(arena.animals[i].id);
            self.element_hit(center, profile.element.or(Some(Element::Storm)), 0.7);
            // The first leaps bring the rod's effects, those after only hurt
            self.strike(arena, profile, i, Blow { damage: hit, crit, heavy: false, from: at, depth: (hop >= 2) as u32, element: profile.element.or(Some(Element::Storm)) });
            arena.animals[i].status.shocked = arena.animals[i].status.shocked.max(0.4);
            let next = (0..arena.animals.len())
                .filter(|&j| arena.animals[j].health > 0.0 && !arena.animals[j].look.is_pet() && !struck.contains(&arena.animals[j].id))
                .map(|j| (j, (arena.animals[j].center(vpm as u32) - center).norm()))
                .filter(|&(_, d)| d < range)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            match next {
                Some((j, _)) => {
                    at = center;
                    i = j;
                    power *= 0.85;
                }
                None => break,
            }
        }
        if struck.len() > 2 {
            self.cues.push(Cue::at(Sound::Crackle, at, 0.6).pitched(1.25));
        }
    }

    /// A jagged thread of lightning, or of `look`'s element, from `from` to `to`, there a moment
    fn zap_line(&mut self, from: Vector3<f64>, to: Vector3<f64>, look: usize) {
        let vpm = self.vpm;
        let steps = ((to - from).norm() / (0.35 * vpm)).clamp(4.0, 26.0) as usize;
        for k in 0..=steps {
            let jitter = if k == 0 || k == steps { Vector3::zeros() } else { self.any_direction() * 0.22 * vpm };
            let at = from.lerp(&to, k as f64 / steps as f64) + jitter;
            if (at - from).norm() < 1.2 * vpm && k < 3 {
                self.fine(at, Vector3::zeros(), look, 0.12, 0.0);
            } else {
                self.sparks.push(Spark { position: at, velocity: Vector3::zeros(), look, life: 0.16, lifetime: 0.16, gravity: 0.0, slot: SLOT_SPARK });
            }
        }
        self.cap_sparks();
    }

    /// Swinging the censer in hand back and forth on its chains while the button's held: each
    /// time it swings out forward it billows a cloud that drifts off ahead and hangs where it
    /// settles, for its mana
    fn swing_censer(&mut self, arena: &mut Arena, weapon: &Weapon, controls: Controls, dt: f64) {
        let profile = weapon.profile.clone();
        let held = controls.attack || controls.holding;
        if !held || self.raise < 1.0 {
            if matches!(self.action, Action::Channel { .. }) {
                self.action = Action::Idle;
            }
            return;
        }
        let t = match self.action {
            Action::Channel { t } => t + dt,
            _ => {
                self.puff = 0.3;
                0.0
            }
        };
        self.action = Action::Channel { t };
        // Wisps of smoke trailing off it as it swings
        if random(&mut self.rng) < dt * 7.0 {
            let mouth = self.held_point(weapon, arena.view, false);
            let life = 0.7 + 0.3 * random(&mut self.rng);
            let look = Element::look(Some(profile.element.unwrap_or(Element::Holy)));
            self.sparks.push(Spark { position: mouth, velocity: Vector3::new(0.0, 0.0, 0.25 * arena.view.vpm), look, life, lifetime: life, gravity: -0.02, slot: SLOT_SMOKE });
            self.cap_sparks();
        }
        self.puff -= dt;
        if self.puff > 0.0 {
            return;
        }
        self.puff += censer_period(&profile, self.speedup());
        let cost = profile.stat("manaCost");
        if self.mana < cost {
            if self.mana_warning <= 0.0 {
                self.messages.push("Not enough mana.".into());
                self.mana_warning = 2.0;
            }
            return;
        }
        self.mana -= cost;
        self.billow(arena, weapon);
    }

    /// The censer in hand billowing out a cloud that settles on the crosshair, or as far off as it
    /// drifts, and hangs there, hurting what's in it and bringing the censer's effects
    fn billow(&mut self, arena: &mut Arena, weapon: &Weapon) {
        let profile = weapon.profile.clone();
        let view = arena.view;
        let vpm = view.vpm;
        let mouth = self.held_point(weapon, view, false);
        let reach = profile.stat("range").max(3.0) * vpm;
        let mut at = self.aim(arena);
        if (at - view.eye).norm() > reach {
            at = view.eye + (at - view.eye).normalize() * reach;
        }
        let position = ground_under(arena.world, at, vpm) + Vector3::new(0.0, 0.0, 0.3 * vpm);
        let element = profile.element.unwrap_or(Element::Holy);
        let radius = 2.4 * profile.stat("area").max(0.5);
        let damage = (profile.stat("spellDamage") + profile.stat("elementalDamage")) * 0.5;
        self.keep_pools_few();
        self.lingering.push(Lingering { pool: false, cloud: true, tall: false, position, radius, life: 4.5, element, profile: profile.clone(), damage, tick: 0.35, effects: true, collapse: 0.0 });
        // A stream of smoke pouring out of it to where it settles
        let look = Element::look(Some(element));
        for k in 0..5 {
            let f = k as f64 / 5.0;
            let life = 0.6 + 0.4 * f;
            let v = (position - mouth) * (2.5 + 2.5 * f) + self.any_direction() * 0.3 * vpm;
            self.sparks.push(Spark { position: mouth, velocity: v, look, life, lifetime: life, gravity: -0.02, slot: SLOT_SMOKE });
        }
        self.cap_sparks();
        self.cues.push(Cue::at(Sound::Billow, mouth, 0.55));
        self.cues.push(Cue::here(Sound::Chain, 0.2).pitched(1.5));
    }

    /// A ring rolling out along the ground from `center` as far as `reach` meters, hitting each
    /// thing once for `damage`, though not what's in `hit` already. With `effects` it brings the
    /// weapon's effects, and with `rocks` boulders heave up along it.
    #[allow(clippy::too_many_arguments)]
    fn ripple(&mut self, profile: &Rc<Profile>, center: Vector3<f64>, reach: f64, damage: f64, hit: Vec<u32>, effects: bool, rocks: bool) {
        self.ripples.push(Ripple { center, radius: 0.4, reach, speed: 13.0, hit, profile: profile.clone(), damage, push: 5.0, stagger: 0.5, effects, rocks, owed: 0.0 });
        if self.ripples.len() > 8 {
            self.ripples.remove(0);
        }
    }

    /// Rings rolling out along the ground, hurting and throwing back what they pass, earth and
    /// sparks thrown up along their fronts
    fn roll_ripples(&mut self, arena: &mut Arena, dt: f64) {
        let vpm = arena.view.vpm;
        for mut r in std::mem::take(&mut self.ripples) {
            r.radius = (r.radius + r.speed * dt).min(r.reach);
            let look = Element::look(r.profile.element);
            r.owed += dt * (36.0 + 5.0 * r.radius).min(80.0);
            while r.owed >= 1.0 {
                r.owed -= 1.0;
                let a = random(&mut self.rng) * TAU;
                let out = Vector3::new(a.cos(), a.sin(), 0.0);
                let at = r.center + out * r.radius * vpm + Vector3::new(0.0, 0.0, 0.15 * vpm);
                if random(&mut self.rng) < 0.55 {
                    let up = 2.5 + 2.5 * random(&mut self.rng);
                    self.drop(at, (out * 1.5 + Vector3::new(0.0, 0.0, up)) * vpm, DROP_MUD, 0.7);
                } else {
                    let life = 0.25 + 0.1 * random(&mut self.rng);
                    self.sparks.push(Spark { position: at, velocity: (out * r.speed * 0.35 + Vector3::new(0.0, 0.0, 1.5)) * vpm, look, life, lifetime: life, gravity: 0.5, slot: SLOT_SPARK });
                }
            }
            if r.rocks && random(&mut self.rng) < dt * 12.0 {
                let a = random(&mut self.rng) * TAU;
                let at = r.center + Vector3::new(a.cos(), a.sin(), 0.0) * r.radius * vpm;
                self.swells.push(Swell { model: BOULDER, position: at, radius: 0.8 + 0.4 * random(&mut self.rng), life: 0.55, lifetime: 0.55 });
            }
            self.cap_sparks();
            let passed: Vec<usize> = (0..arena.animals.len())
                .filter(|&i| {
                    let a = &arena.animals[i];
                    a.health > 0.0 && !a.look.is_pet() && !r.hit.contains(&a.id) && (a.position - r.center).xy().norm() / vpm - a.look.body().1 < r.radius && (a.position.z - r.center.z).abs() < 3.0 * vpm
                })
                .collect();
            for i in passed {
                r.hit.push(arena.animals[i].id);
                let away = arena.animals[i].position - r.center;
                let d = away.xy().norm() / vpm;
                let damage = r.damage * (1.0 - 0.4 * (d / r.reach).min(1.0));
                let at = arena.animals[i].center(vpm as u32);
                let profile = r.profile.clone();
                self.element_hit(at, profile.element, 1.0);
                self.strike(arena, &profile, i, Blow { damage, crit: false, heavy: true, from: r.center, depth: if r.effects { 0 } else { 1 }, element: profile.element });
                if arena.animals[i].health > 0.0 {
                    let s = &mut arena.animals[i].status;
                    s.staggered = s.staggered.max(r.stagger);
                    s.push = Vector3::new(away.x, away.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x()) * r.push * vpm;
                }
            }
            if r.radius < r.reach {
                self.ripples.push(r);
            }
        }
    }

    /// A zone of `kind` set going at `position`, `radius` meters round, for `life` seconds, doing
    /// `damage` each time it hurts
    #[allow(clippy::too_many_arguments)]
    fn zone(&mut self, kind: ZoneKind, profile: &Rc<Profile>, position: Vector3<f64>, velocity: Vector3<f64>, radius: f64, life: f64, damage: f64) {
        self.zones.push(Zone { kind, position, velocity, radius, life, lifetime: life, tick: 0.0, profile: profile.clone(), damage, hit: Vec::new(), target: None, dwell: 0.0, owed: 0.0, path: Vec::new() });
    }

    /// Special attacks going on a while: storms, bogs, suns, whirlwinds, balls of lightning, swarms
    /// and boulders, see Zone
    fn zones(&mut self, arena: &mut Arena, dt: f64) {
        for mut z in std::mem::take(&mut self.zones) {
            z.life -= dt;
            z.tick -= dt;
            match z.kind {
                ZoneKind::Storm => self.storm(arena, &mut z, dt),
                ZoneKind::Mire => self.mire(arena, &mut z, dt),
                ZoneKind::Sun => self.sun(arena, &mut z, dt),
                ZoneKind::Cyclone => self.cyclone(arena, &mut z, dt),
                ZoneKind::Ball => self.ball(arena, &mut z, dt),
                ZoneKind::Swarm => self.swarm(arena, &mut z, dt),
                ZoneKind::Boulder => self.boulder(arena, &mut z, dt),
                ZoneKind::Crescent => self.crescent(arena, &mut z, dt),
                ZoneKind::Keg => self.keg(arena, &mut z, dt),
                ZoneKind::Orbit => self.orbit(arena, &mut z, dt),
                ZoneKind::Brand => self.brand_burn(arena, &mut z, dt),
            }
            if z.life > 0.0 {
                self.zones.push(z);
            } else {
                self.zone_ends(arena, &z);
            }
        }
        self.cap_sparks();
    }

    /// The living animals, by index, within `radius` meters of `at` across the ground and `height`
    /// meters up or down
    fn under(arena: &Arena, at: Vector3<f64>, radius: f64, height: f64) -> Vec<usize> {
        let vpm = arena.view.vpm;
        (0..arena.animals.len())
            .filter(|&i| {
                let a = &arena.animals[i];
                a.health > 0.0 && !a.look.is_pet() && (a.position - at).xy().norm() < (radius + a.look.body().1) * vpm && (a.position.z - at.z).abs() < height * vpm
            })
            .collect()
    }

    /// A thunderstorm: thunderheads churning over the ground, a bolt striking down every so often
    /// onto something under it, or the ground if there's nothing
    fn storm(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let vpm = arena.view.vpm;
        self.marks.push((z.position + Vector3::new(0.0, 0.0, 0.1 * vpm), z.radius));
        z.owed += dt * 7.0;
        while z.owed >= 1.0 {
            z.owed -= 1.0;
            let a = random(&mut self.rng) * TAU;
            let r = random(&mut self.rng).sqrt() * z.radius * vpm;
            let at = z.position + Vector3::new(a.cos() * r, a.sin() * r, (5.2 + 1.3 * random(&mut self.rng)) * vpm);
            let life = 1.6 + 0.6 * random(&mut self.rng);
            let drift = Vector3::new(random(&mut self.rng) - 0.5, random(&mut self.rng) - 0.5, 0.0) * 0.8 * vpm;
            self.sparks.push(Spark { position: at, velocity: drift, look: Element::look(Some(Element::Void)), life, lifetime: life, gravity: 0.0, slot: SLOT_CLOUD });
        }
        if z.tick > 0.0 {
            return;
        }
        z.tick = 0.3;
        let under = Combat::under(arena, z.position, z.radius, 8.0);
        let (to, struck) = if under.is_empty() {
            let a = random(&mut self.rng) * TAU;
            let r = random(&mut self.rng).sqrt() * z.radius * vpm;
            (ground_under(arena.world, z.position + Vector3::new(a.cos() * r, a.sin() * r, 0.0), vpm), None)
        } else {
            let i = under[(random(&mut self.rng) * under.len() as f64) as usize % under.len()];
            (arena.animals[i].center(vpm as u32), Some(i))
        };
        let top = Vector3::new(to.x, to.y, z.position.z + 5.8 * vpm);
        self.bolt_line(top, to, Element::look(Some(Element::Storm)));
        self.flashes.push(Flash { position: to + Vector3::new(0.0, 0.0, 1.0 * vpm), color: Element::Storm.light(), reach: 10.0, life: 0.16, lifetime: 0.16 });
        self.sparks(to, 8, 4.0, Element::look(Some(Element::Storm)), 0.4, 0.25);
        if random(&mut self.rng) < 0.55 {
            self.cues.push(Cue::at(Sound::Thunder, to, 0.8).pitched(0.85 + 0.3 * random(&mut self.rng) as f32));
        }
        if let Some(i) = struck {
            let profile = z.profile.clone();
            let first = !z.hit.contains(&arena.animals[i].id);
            z.hit.push(arena.animals[i].id);
            self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: true, from: top, depth: (!first) as u32, element: Some(Element::Storm) });
            arena.animals[i].status.shocked = 0.5;
        }
    }

    /// A bog: bubbles of venom welling up and bursting, mud spattering and a haze over it, and
    /// everything in it mired, poisoned and drawn slowly in
    fn mire(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let vpm = arena.view.vpm;
        z.owed += dt * 22.0;
        while z.owed >= 1.0 {
            z.owed -= 1.0;
            let a = random(&mut self.rng) * TAU;
            let r = random(&mut self.rng).sqrt() * z.radius * vpm;
            let at = z.position + Vector3::new(a.cos() * r, a.sin() * r, 0.1 * vpm);
            match (random(&mut self.rng) * 10.0) as u32 {
                0 => {
                    let life = 1.6 + 0.6 * random(&mut self.rng);
                    self.sparks.push(Spark { position: at + Vector3::new(0.0, 0.0, 0.5 * vpm), velocity: Vector3::new(0.0, 0.0, 0.15 * vpm), look: Element::look(Some(Element::Poison)), life, lifetime: life, gravity: 0.0, slot: SLOT_CLOUD });
                }
                1..=3 => {
                    let v = Vector3::new(random(&mut self.rng) - 0.5, random(&mut self.rng) - 0.5, 2.5) * vpm;
                    self.drop(at, v, DROP_MUD, 0.6);
                }
                _ => {
                    let life = 0.5 + 0.4 * random(&mut self.rng);
                    self.sparks.push(Spark { position: at, velocity: Vector3::new(0.0, 0.0, (0.4 + 0.5 * random(&mut self.rng)) * vpm), look: Element::look(Some(Element::Poison)), life, lifetime: life, gravity: -0.05, slot: SLOT_SPARK });
                }
            }
        }
        if z.tick > 0.0 {
            return;
        }
        z.tick = 0.5;
        if random(&mut self.rng) < 0.6 {
            self.cues.push(Cue::at(Sound::Bubble, z.position, 0.6).pitched(0.8 + 0.4 * random(&mut self.rng) as f32));
        }
        let profile = z.profile.clone();
        for i in Combat::under(arena, z.position, z.radius, 2.5) {
            self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: false, from: z.position, depth: 1, element: Some(Element::Poison) });
            let a = &mut arena.animals[i];
            if a.health <= 0.0 {
                continue;
            }
            let to = z.position - a.position;
            let s = &mut a.status;
            s.slowed = s.slowed.max(0.8);
            s.slow = s.slow.max(0.7);
            s.poison = s.poison.max(3.0);
            s.poison_stacks = (s.poison_stacks + 1).min(5);
            s.poison_dps = s.poison_dps.max(z.damage * 0.25);
            s.shown = 4.0;
            if a.boss.is_none() && to.xy().norm() > 1.0 * vpm {
                s.push = Vector3::new(to.x, to.y, 0.0).normalize() * 1.6 * vpm;
            }
        }
    }

    /// A sun hanging over the ground: light pouring off it, and every so often rays searing the
    /// nearest few of what's under it
    fn sun(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let vpm = arena.view.vpm;
        self.flashes.push(Flash { position: z.position, color: Element::Holy.light(), reach: 16.0, life: dt * 1.5, lifetime: dt * 1.5 });
        z.owed += dt * 24.0;
        while z.owed >= 1.0 {
            z.owed -= 1.0;
            let v = (self.any_direction() + Vector3::new(0.0, 0.0, -0.6)) * 2.5 * vpm;
            let at = z.position + self.any_direction() * 0.6 * vpm;
            self.fine(at, v, Element::look(Some(Element::Holy)), 0.7, 0.0);
        }
        let ground = z.position - Vector3::new(0.0, 0.0, SUN_HEIGHT * vpm);
        if z.tick <= 0.0 {
            z.tick = 0.3;
            let mut near: Vec<(f64, usize)> = Combat::under(arena, ground, z.radius, 6.0).into_iter().map(|i| ((arena.animals[i].position - ground).xy().norm(), i)).collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0));
            near.truncate(5);
            let profile = z.profile.clone();
            let first = z.hit.is_empty();
            z.hit = near.iter().map(|&(_, i)| arena.animals[i].id).collect();
            for (_, i) in near {
                let at = arena.animals[i].center(vpm as u32);
                self.element_hit(at, Some(Element::Holy), 0.6);
                self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: false, from: z.position, depth: (!first) as u32, element: Some(Element::Holy) });
            }
            if !z.hit.is_empty() {
                self.cues.push(Cue::at(Sound::Beam, z.position, 0.5).pitched(1.5));
            }
        }
        // The rays, drawn solid
        for &id in &z.hit {
            if let Some(a) = arena.animals.iter().find(|a| a.id == id && a.health > 0.0) {
                self.rays.push((z.position, a.center(vpm as u32), Element::look(Some(Element::Holy))));
            }
        }
    }

    /// A whirlwind wandering on along the ground: dragging in and spinning round what's near it,
    /// battering what it's caught, dust and leaves flung off it
    fn cyclone(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let vpm = arena.view.vpm;
        let next = z.position + z.velocity * dt;
        if solid(arena.world, next + Vector3::new(0.0, 0.0, 1.2 * vpm)) {
            z.velocity = Vector3::zeros();
        } else {
            z.position = ground_under(arena.world, next, vpm);
        }
        for a in arena.animals.iter_mut().filter(|a| a.health > 0.0 && !a.look.is_pet() && a.boss.is_none()) {
            let to = z.position - a.position;
            let d = to.xy().norm() / vpm;
            if d < z.radius && (to.z / vpm).abs() < 3.0 {
                let inward = Vector3::new(to.x, to.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros);
                let round = Vector3::new(-inward.y, inward.x, 0.0);
                a.status.push = (inward * (d * 2.5).min(7.0) + round * 5.0) * vpm + z.velocity;
            }
        }
        z.owed += dt * 30.0;
        while z.owed >= 1.0 {
            z.owed -= 1.0;
            let a = random(&mut self.rng) * TAU;
            let h = random(&mut self.rng);
            let r = (0.3 + 1.1 * h * h) * vpm;
            let out = Vector3::new(a.cos(), a.sin(), 0.0);
            let at = z.position + out * r + Vector3::new(0.0, 0.0, h * 3.0 * vpm);
            let v = (Vector3::new(-out.y, out.x, 0.0) * 7.0 + out * 1.5 + Vector3::new(0.0, 0.0, 2.0)) * vpm;
            if random(&mut self.rng) < 0.5 {
                let kind = if random(&mut self.rng) < 0.5 { DROP_ASH } else { DROP_MUD };
                self.drop(at, v, kind, 0.8);
            } else {
                self.fine(at, v, 0, 0.4, 0.0);
            }
        }
        if z.tick <= 0.0 {
            z.tick = 0.3;
            let profile = z.profile.clone();
            for i in Combat::under(arena, z.position, 2.4, 3.5) {
                let first = !z.hit.contains(&arena.animals[i].id);
                if first {
                    z.hit.push(arena.animals[i].id);
                }
                self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: false, from: z.position, depth: (!first) as u32, element: profile.element });
                let s = &mut arena.animals[i].status;
                s.staggered = s.staggered.max(0.35);
            }
            if (z.lifetime - z.life) % 1.4 < 0.3 {
                self.cues.push(Cue::at(Sound::Gale, z.position, 0.8));
            }
        }
    }

    /// A ball of lightning drifting on over the ground, crackling, lashing out at what's near
    fn ball(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let vpm = arena.view.vpm;
        let next = z.position + z.velocity * dt;
        if solid(arena.world, next) {
            z.velocity = Vector3::zeros();
        } else {
            // Kept floating a little over the ground
            let ground = ground_under(arena.world, next, vpm);
            let height = 1.5 * vpm;
            z.position = Vector3::new(next.x, next.y, next.z + ((ground.z + height) - next.z) * (dt * 4.0).min(1.0));
        }
        self.flashes.push(Flash { position: z.position, color: Element::Storm.light(), reach: 7.0, life: dt * 1.5, lifetime: dt * 1.5 });
        z.owed += dt * 28.0;
        while z.owed >= 1.0 {
            z.owed -= 1.0;
            let out = self.any_direction();
            self.fine(z.position + out * 0.35 * vpm, out * 1.8 * vpm, Element::look(Some(Element::Storm)), 0.15, 0.0);
        }
        if z.tick > 0.0 {
            return;
        }
        z.tick = 0.3;
        let profile = z.profile.clone();
        let mut near: Vec<(f64, usize)> = (0..arena.animals.len())
            .filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet())
            .map(|i| ((arena.animals[i].center(vpm as u32) - z.position).norm(), i))
            .filter(|&(d, _)| d < z.radius * vpm)
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        near.truncate(6);
        for &(_, i) in &near {
            let at = arena.animals[i].center(vpm as u32);
            self.arc(z.position, at);
            let first = !z.hit.contains(&arena.animals[i].id);
            if first {
                z.hit.push(arena.animals[i].id);
            }
            self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: false, from: z.position, depth: (!first) as u32, element: Some(Element::Storm) });
            arena.animals[i].status.shocked = 0.5;
        }
        if !near.is_empty() {
            self.cues.push(Cue::at(Sound::Zap, z.position, 0.55).pitched(0.8 + 0.3 * random(&mut self.rng) as f32));
        } else if random(&mut self.rng) < 0.4 {
            self.cues.push(Cue::at(Sound::Crackle, z.position, 0.4));
        }
    }

    /// A swarm going after one enemy, then the next, stinging and poisoning everything round it
    fn swarm(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let vpm = arena.view.vpm;
        let alive = |id: u32| arena.animals.iter().position(|a| a.id == id && a.health > 0.0);
        let current = z.target.and_then(alive);
        if current.is_none() || z.dwell > 1.3 {
            // The nearest it hasn't been at lately, or failing that the nearest
            let visited = z.hit.clone();
            let mut near: Vec<(f64, usize, bool)> = (0..arena.animals.len())
                .filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet())
                .map(|i| ((arena.animals[i].position - z.position).norm(), i, visited.contains(&arena.animals[i].id)))
                .filter(|&(d, _, _)| d < 22.0 * vpm)
                .collect();
            near.sort_by(|a, b| (a.2, a.0).partial_cmp(&(b.2, b.0)).unwrap_or(std::cmp::Ordering::Equal));
            z.target = near.first().map(|&(_, i, _)| arena.animals[i].id);
            z.dwell = 0.0;
            if let Some(id) = z.target {
                z.hit.retain(|&h| h != id);
                z.hit.push(id);
                if z.hit.len() > 3 {
                    z.hit.remove(0);
                }
            }
        }
        match z.target.and_then(alive) {
            Some(i) => {
                let goal = arena.animals[i].center(vpm as u32);
                let to = goal - z.position;
                if to.norm() > 1.0 * vpm {
                    let want = to.normalize() * 10.0 * vpm;
                    z.velocity += (want - z.velocity) * (dt * 6.0).min(1.0);
                } else {
                    z.dwell += dt;
                    z.velocity *= (-4.0 * dt).exp();
                }
            }
            None => z.velocity *= (-1.0 * dt).exp(),
        }
        z.position += z.velocity * dt;
        // The swarm itself: specks darting round it
        z.owed += dt * 90.0;
        while z.owed >= 1.0 {
            z.owed -= 1.0;
            let out = self.any_direction();
            let at = z.position + out * (0.4 + 0.9 * random(&mut self.rng)) * vpm;
            let v = Vector3::new(-out.y, out.x, out.z * 0.5) * 3.5 * vpm + z.velocity * 0.7;
            let life = 0.35 + 0.2 * random(&mut self.rng);
            if random(&mut self.rng) < 0.03 {
                // A shadow of them, a dark haze round the swarm
                self.sparks.push(Spark { position: at, velocity: z.velocity * 0.8, look: Element::look(Some(Element::Void)), life: 0.7, lifetime: 0.7, gravity: 0.0, slot: SLOT_CLOUD });
            } else if random(&mut self.rng) < 0.82 {
                self.sparks.push(Spark { position: at, velocity: v, look: DROP_ASH, life, lifetime: life, gravity: 0.0, slot: SLOT_DROP });
            } else {
                self.fine(at, v, Element::look(Some(Element::Poison)), life, 0.0);
            }
        }
        if z.tick > 0.0 {
            return;
        }
        z.tick = 0.25;
        if random(&mut self.rng) < 0.3 {
            self.cues.push(Cue::at(Sound::Buzz, z.position, 0.6).pitched(0.9 + 0.2 * random(&mut self.rng) as f32));
        }
        let profile = z.profile.clone();
        let stung: Vec<usize> = (0..arena.animals.len())
            .filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet() && (arena.animals[i].center(vpm as u32) - z.position).norm() < (z.radius + arena.animals[i].look.body().1) * vpm)
            .collect();
        for i in stung {
            self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: false, from: z.position, depth: 1, element: Some(Element::Poison) });
            let s = &mut arena.animals[i].status;
            s.poison = s.poison.max(4.0);
            s.poison_stacks = (s.poison_stacks + 1).min(5);
            s.poison_dps = s.poison_dps.max(z.damage * 0.3);
        }
    }

    /// A boulder rolling on along the ground, bowling over and battering what's in its way, once
    /// each, and breaking up on what it can't roll over
    fn boulder(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let vpm = arena.view.vpm;
        let next = z.position + z.velocity * dt;
        let ahead = z.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::zeros);
        if solid(arena.world, next + ahead * z.radius * vpm + Vector3::new(0.0, 0.0, 0.5 * vpm)) {
            z.life = 0.0;
            return;
        }
        let ground = ground_under(arena.world, next, vpm);
        z.position = Vector3::new(next.x, next.y, ground.z + z.radius * vpm);
        z.owed += dt * 18.0;
        while z.owed >= 1.0 {
            z.owed -= 1.0;
            let v = (self.any_direction() * 1.5 + Vector3::new(0.0, 0.0, 2.5)) * vpm - z.velocity * 0.15;
            let at = ground + self.any_direction() * 0.5 * vpm;
            self.drop(at, v, DROP_MUD, 0.6);
        }
        let profile = z.profile.clone();
        for i in Combat::under(arena, z.position, z.radius + 0.3, 2.5) {
            let id = arena.animals[i].id;
            if z.hit.contains(&id) {
                continue;
            }
            z.hit.push(id);
            let at = arena.animals[i].center(vpm as u32);
            self.sparks(at, 8, 3.0, Element::look(None), 0.8, 0.3);
            self.cues.push(Cue::at(Sound::Hit, at, 0.9).pitched(0.6));
            self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: true, from: z.position - z.velocity * 0.1, depth: 0, element: profile.element });
            let s = &mut arena.animals[i].status;
            s.staggered = s.staggered.max(0.8);
            s.push = Vector3::new(ahead.x, ahead.y, 0.0) * 11.0 * vpm;
        }
    }

    /// A crescent of light flying on ahead, cutting through each thing it meets once, till it runs
    /// into something solid or fades. Until its `tick` runs out it's still being cut loose from
    /// the blade, and waits.
    fn crescent(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        if z.tick > 0.0 {
            return;
        }
        let vpm = arena.view.vpm;
        let next = z.position + z.velocity * dt;
        if solid(arena.world, next + Vector3::new(0.0, 0.0, 0.3 * vpm)) {
            z.life = 0.0;
            return;
        }
        z.position = next;
        let ahead = z.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let side = ahead.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::y);
        let look = Element::look(z.profile.element.or(Some(Element::Holy)));
        // Light streaming off its edge behind it
        z.owed += dt * 40.0;
        while z.owed >= 1.0 {
            z.owed -= 1.0;
            let s = random(&mut self.rng) * 2.0 - 1.0;
            let at = z.position + side * s * z.radius * vpm - ahead * s * s * 0.8 * vpm;
            self.fine(at, -ahead * 3.0 * vpm, look, 0.25, 0.0);
        }
        let profile = z.profile.clone();
        for i in Combat::under(arena, z.position, z.radius, 2.2) {
            let id = arena.animals[i].id;
            let to = arena.animals[i].position - z.position;
            // Only what the blade of it has reached, not what's just beside it
            if z.hit.contains(&id) || to.dot(&ahead).abs() > (0.9 + arena.animals[i].look.body().1) * vpm {
                continue;
            }
            z.hit.push(id);
            let at = arena.animals[i].center(vpm as u32);
            self.element_hit(at, profile.element.or(Some(Element::Holy)), 1.5);
            self.cues.push(Cue::at(Sound::Hit, at, 0.8).pitched(1.3));
            self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: true, from: z.position - ahead * vpm, depth: 0, element: profile.element.or(Some(Element::Holy)) });
            if arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
                let s = &mut arena.animals[i].status;
                s.push = ahead * 6.0 * vpm;
                s.staggered = s.staggered.max(0.4);
            }
        }
    }

    /// A keg set down, its fuse sputtering, a ring round it showing how far it'll throw things
    fn keg(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let vpm = arena.view.vpm;
        // Not set down yet, see special_move()
        if z.lifetime - z.life < KEG_SET {
            return;
        }
        self.marks.push((z.position + Vector3::new(0.0, 0.0, 0.1 * vpm), z.radius));
        z.owed += dt * 22.0;
        let top = z.position + Vector3::new(0.0, 0.0, 0.85 * vpm);
        while z.owed >= 1.0 {
            z.owed -= 1.0;
            let v = (self.any_direction() * 0.8 + Vector3::new(0.0, 0.0, 1.6)) * 1.5 * vpm;
            self.fine(top, v, Element::look(Some(Element::Fire)), 0.35, 0.6);
        }
        // Hissing faster as it burns down
        if z.tick <= 0.0 {
            z.tick = 0.25 + 0.3 * (z.life / z.lifetime);
            self.cues.push(Cue::at(Sound::Fuse, top, 0.5).pitched(1.0 + 0.4 * (1.0 - z.life / z.lifetime) as f32));
        }
    }

    /// A weapon's head going round the player on its chain, wider and wider, battering what it
    /// meets, each thing at most every 0.35 s. Its `dwell` is how far round it's gone, in radians.
    fn orbit(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let vpm = arena.view.vpm;
        let feet = arena.player.unwrap_or(arena.view.eye - Vector3::new(0.0, 0.0, 1.6 * vpm));
        let age = z.lifetime - z.life;
        z.dwell += dt * TAU * 1.5;
        z.radius = 1.8 + 2.6 * smooth(age / 2.5);
        z.position = feet + Vector3::new(z.dwell.cos(), z.dwell.sin(), 0.0) * z.radius * vpm + Vector3::new(0.0, 0.0, 1.0 * vpm);
        let look = Element::look(z.profile.element);
        self.fine(z.position, Vector3::zeros(), look, 0.3, 0.0);
        if z.tick <= 0.0 {
            z.tick = 0.35;
            z.hit.clear();
        }
        z.owed += dt;
        if z.owed > 0.66 {
            z.owed -= 0.66;
            self.cues.push(Cue::here(Sound::Whirr, 0.55).pitched(0.75));
        }
        let profile = z.profile.clone();
        let along = Vector3::new(-z.dwell.sin(), z.dwell.cos(), 0.0);
        for i in Combat::under(arena, z.position - Vector3::new(0.0, 0.0, 1.0 * vpm), 0.9, 2.2) {
            let id = arena.animals[i].id;
            if z.hit.contains(&id) {
                continue;
            }
            z.hit.push(id);
            let at = arena.animals[i].center(vpm as u32);
            self.sparks(at, 8, 3.0, Element::look(None), 0.8, 0.3);
            self.element_hit(at, profile.element, 1.0);
            self.cues.push(Cue::at(Sound::Hit, at, 0.9).pitched(0.65));
            self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: true, from: feet, depth: 0, element: profile.element });
            if arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
                let away = (arena.animals[i].position - feet).xy().try_normalize(1e-6).map_or(along, |v| Vector3::new(v.x, v.y, 0.0));
                let s = &mut arena.animals[i].status;
                s.push = (along * 0.6 + away) * 9.0 * vpm;
                s.staggered = s.staggered.max(0.5);
            }
        }
    }

    /// A rocket bursting at `at`: a starburst of sparks and a flash, fire raining down onto what's
    /// under it. It blows no hole.
    fn skyburst(&mut self, arena: &mut Arena, p: &Projectile, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let profile = p.profile.clone();
        let element = profile.element.or(Some(Element::Fire));
        let (look, gold) = (Element::look(element), Element::look(Some(Element::Holy)));
        for (k, d) in sphere_directions(26).into_iter().enumerate() {
            let look = if k % 3 == 0 { gold } else { look };
            self.sparks.push(Spark { position: at, velocity: d * (6.0 + 1.5 * random(&mut self.rng)) * vpm, look, life: 0.8, lifetime: 0.8, gravity: 0.35, slot: SLOT_SPARK });
        }
        for _ in 0..6 {
            let v = self.any_direction() * 2.0 * vpm;
            self.drop(at, v, DROP_EMBER, 1.4);
        }
        self.cap_sparks();
        self.flashes.push(Flash { position: at, color: element.map_or([1.0, 0.8, 0.5], Element::light), reach: 14.0, life: 0.45, lifetime: 0.45 });
        self.cues.push(Cue::at(Sound::Burst, at, 0.7).pitched(1.7));
        self.cues.push(Cue::at(Sound::Crackle, at, 0.6).pitched(1.3));
        let ground = ground_under(arena.world, at, vpm);
        for i in Combat::under(arena, ground, 3.2, 4.0) {
            let center = arena.animals[i].center(vpm as u32);
            self.element_hit(center, element, 1.2);
            self.strike(arena, &profile, i, Blow { damage: p.damage, crit: false, heavy: false, from: at, depth: 0, element });
            if arena.animals[i].health > 0.0 && element == Some(Element::Fire) {
                let s = &mut arena.animals[i].status;
                s.burn = s.burn.max(3.0);
                s.burn_dps = s.burn_dps.max(p.damage * 0.2);
            }
        }
    }

    /// A zone ending: a sun bursting, a whirlwind flinging out what it caught, a ball of lightning
    /// going off, a boulder breaking up
    fn zone_ends(&mut self, arena: &mut Arena, z: &Zone) {
        let vpm = arena.view.vpm;
        let profile = z.profile.clone();
        match z.kind {
            // A keg going off: a great blast, everything round it thrown, bomblets flung out
            ZoneKind::Keg => {
                let at = z.position + Vector3::new(0.0, 0.0, 0.5 * vpm);
                self.scarless = true;
                self.burst(arena, &profile, at, z.radius * 0.8, z.damage, 0, false, profile.element.or(Some(Element::Fire)), None);
                self.scarless = false;
                for i in Combat::under(arena, z.position, z.radius, 4.0) {
                    if arena.animals[i].health <= 0.0 || arena.animals[i].boss.is_some() {
                        continue;
                    }
                    let away = arena.animals[i].position - z.position;
                    let near = 1.0 - (away.xy().norm() / vpm / z.radius).min(0.7);
                    let s = &mut arena.animals[i].status;
                    s.push = Vector3::new(away.x, away.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x()) * 18.0 * near * vpm;
                    s.staggered = s.staggered.max(1.2);
                }
                self.scatter(&profile, at, 5, z.damage * 0.25);
                self.swells.push(Swell { model: PILLAR, position: z.position, radius: 1.6, life: 0.5, lifetime: 0.5 });
                self.cues.push(Cue::at(Sound::Slam, at, 1.0).pitched(0.7));
                self.shake = self.shake.max(1.0);
            }
            ZoneKind::Crescent | ZoneKind::Orbit => {
                let look = Element::look(profile.element.or(Some(Element::Holy)));
                self.sparks(z.position, 10, 2.0, look, 0.0, 0.35);
            }
            ZoneKind::Sun => {
                let ground = ground_under(arena.world, z.position, vpm);
                self.swells.push(Swell { model: PILLAR, position: ground, radius: 2.5, life: 0.8, lifetime: 0.8 });
                self.scarless = true;
                self.burst(arena, &profile, ground + Vector3::new(0.0, 0.0, 0.8 * vpm), z.radius * 0.7, z.damage * 3.0, 0, false, Some(Element::Holy), None);
                self.scarless = false;
                for i in Combat::under(arena, ground, z.radius * 0.7, 6.0) {
                    let away = arena.animals[i].position - ground;
                    arena.animals[i].status.push = Vector3::new(away.x, away.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x()) * 8.0 * vpm;
                }
                self.cues.push(Cue::at(Sound::Toll, ground, 0.8).pitched(1.4));
                self.shake = self.shake.max(0.6);
            }
            ZoneKind::Cyclone => {
                for i in Combat::under(arena, z.position, 3.5, 3.5) {
                    let away = arena.animals[i].position - z.position;
                    let s = &mut arena.animals[i].status;
                    s.push = Vector3::new(away.x, away.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x()) * 11.0 * vpm;
                    s.staggered = s.staggered.max(0.7);
                }
                for _ in 0..14 {
                    let v = (self.any_direction() * 5.0 + Vector3::new(0.0, 0.0, 3.0)) * vpm;
                    self.drop(z.position + Vector3::new(0.0, 0.0, 1.0 * vpm), v, DROP_ASH, 1.0);
                }
                self.cues.push(Cue::at(Sound::HeavySwing, z.position, 0.8).pitched(0.6));
            }
            ZoneKind::Ball => {
                self.scarless = true;
                self.burst(arena, &profile, z.position, 2.5, z.damage * 1.5, 1, false, Some(Element::Storm), None);
                self.scarless = false;
            }
            ZoneKind::Boulder => {
                for _ in 0..12 {
                    let v = (self.any_direction() * 3.0 + Vector3::new(0.0, 0.0, 2.5)) * vpm;
                    self.drop(z.position, v, DROP_MUD, 0.9);
                }
                self.swells.push(Swell { model: BOULDER, position: z.position, radius: z.radius * 1.4, life: 0.35, lifetime: 0.35 });
                self.cues.push(Cue::at(Sound::Crack, z.position, 0.7).pitched(0.5));
            }
            ZoneKind::Storm | ZoneKind::Mire | ZoneKind::Swarm | ZoneKind::Brand => {}
        }
    }

    /// A burning brand's line of fire along the ground, from z's position along its velocity:
    /// embers rising off it here and there, and every half second what's standing in it burnt
    fn brand_burn(&mut self, arena: &mut Arena, z: &mut Zone, dt: f64) {
        let vpm = arena.view.vpm;
        if z.path.len() < 2 || (z.tick > 0.0 && z.lifetime - z.life < 0.45) {
            // Not alight yet
            return;
        }
        z.owed += dt * 9.0;
        while z.owed >= 1.0 {
            z.owed -= 1.0;
            let k = (random(&mut self.rng) * (z.path.len() - 1) as f64) as usize;
            let at = z.path[k].lerp(&z.path[k + 1], random(&mut self.rng)) + Vector3::new(0.0, 0.0, 0.15 * vpm);
            self.mote(at, Some(Element::Fire), 2.0);
        }
        // Tongues of flame licking up here and there along it, a few at a time, dying down
        // as it burns out
        z.dwell += dt * 7.0 * (z.life / 1.5).min(1.0);
        while z.dwell >= 1.0 {
            z.dwell -= 1.0;
            let k = (random(&mut self.rng) * (z.path.len() - 1) as f64) as usize;
            let at = z.path[k].lerp(&z.path[k + 1], random(&mut self.rng)) + Vector3::new(0.0, 0.0, (0.25 + 0.2 * random(&mut self.rng)) * vpm);
            let radius = 0.25 + 0.15 * random(&mut self.rng);
            self.blasts.push(Blast { position: at, look: Element::look(Some(Element::Fire)), radius, life: 0.45, lifetime: 0.45 });
        }
        if z.tick > 0.0 {
            return;
        }
        z.tick = 0.5;
        // How far `p` is from the line, across the ground, and whether it's near it up and down
        let near = |p: Vector3<f64>| {
            z.path
                .windows(2)
                .map(|w| {
                    let line = w[1] - w[0];
                    let along = ((p - w[0]).dot(&line) / line.norm_squared().max(1e-6)).clamp(0.0, 1.0);
                    let on = w[0] + line * along;
                    if (p.z - on.z).abs() < 3.0 * vpm { (p - on).xy().norm() } else { f64::MAX }
                })
                .fold(f64::MAX, f64::min)
        };
        let inside: Vec<usize> = (0..arena.animals.len())
            .filter(|&i| {
                let a = &arena.animals[i];
                a.health > 0.0 && !a.look.is_pet() && near(a.position) / vpm - a.look.body().1 < BRAND_WIDTH
            })
            .collect();
        let profile = z.profile.clone();
        let middle = z.path[z.path.len() / 2];
        for i in inside {
            self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: false, from: middle, depth: 1, element: Some(Element::Fire) });
            if arena.animals[i].health > 0.0 {
                let s = &mut arena.animals[i].status;
                s.burn = s.burn.max(2.0);
                s.burn_dps = s.burn_dps.max(z.damage * 0.6);
            }
        }
    }

    /// A comet of ice coming down at `at`: ice bursting up, everything near frozen and hurt, and
    /// shards flung out all round. It blows no hole.
    fn comet_impact(&mut self, arena: &mut Arena, p: &Projectile, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let profile = p.profile.clone();
        let radius = 6.0;
        let frost = Element::look(Some(Element::Frost));
        self.swells.push(Swell { model: ICE_BURST, position: at, radius: 3.2, life: 1.5, lifetime: 1.5 });
        for k in 0..7 {
            let a = k as f64 / 7.0 * TAU + 0.3 * random(&mut self.rng);
            self.swells.push(Swell { model: ICE_BURST, position: at + Vector3::new(a.cos(), a.sin(), 0.0) * 3.8 * vpm, radius: 1.4, life: 1.1, lifetime: 1.1 });
        }
        for k in 0..40 {
            let a = k as f64 / 40.0 * TAU;
            let out = Vector3::new(a.cos(), a.sin(), 0.1);
            self.sparks.push(Spark { position: at + Vector3::new(0.0, 0.0, 0.4 * vpm), velocity: out * 15.0 * vpm, look: frost, life: 0.5, lifetime: 0.5, gravity: 0.3, slot: SLOT_SPARK });
        }
        for _ in 0..16 {
            let v = (self.any_direction() + Vector3::new(0.0, 0.0, 1.4)) * 4.0 * vpm;
            self.drop(at, v, DROP_ICE, 1.2);
        }
        self.flashes.push(Flash { position: at + Vector3::new(0.0, 0.0, 1.0 * vpm), color: Element::Frost.light(), reach: 18.0, life: 0.5, lifetime: 0.5 });
        self.cues.push(Cue::at(Sound::Crack, at, 1.0).pitched(0.55));
        self.cues.push(Cue::at(Sound::Burst, at, 1.0).pitched(0.6));
        let near = (1.0 - (at - arena.view.eye).norm() / vpm / 40.0).max(0.0);
        self.shake = self.shake.max(0.3 + 0.7 * near);
        self.scarless = true;
        for i in Combat::under(arena, at, radius, 5.0) {
            let d = (arena.animals[i].position - at).xy().norm() / vpm;
            let center = arena.animals[i].center(vpm as u32);
            self.element_hit(center, Some(Element::Frost), 1.2);
            self.strike(arena, &profile, i, Blow { damage: p.damage * (1.0 - 0.5 * (d / radius).min(1.0)), crit: false, heavy: true, from: at, depth: 0, element: Some(Element::Frost) });
            let s = &mut arena.animals[i].status;
            s.frozen = s.frozen.max(3.0);
            s.chill = 0.0;
        }
        self.scarless = false;
        // Shards flung out all round, going off quietly where they land
        for k in 0..12 {
            let a = k as f64 / 12.0 * TAU;
            let out = Vector3::new(a.cos(), a.sin(), 0.12);
            self.launch(&profile, None, Form::Shard, at + Vector3::new(0.0, 0.0, 0.7 * vpm) + out * 0.8 * vpm, out * 22.0 * vpm, p.damage * 0.3, 0.05);
            if let Some(shard) = self.projectiles.last_mut() {
                shard.life = 0.55;
                shard.trick = Trick::Shard;
            }
        }
        self.cap_sparks();
    }

    /// The ground bursting into a column of fire at `at`: what's on it hurt for `damage`, set
    /// alight and knocked off its feet, and fire left burning a while. It blows no hole.
    fn geyser(&mut self, arena: &mut Arena, profile: &Rc<Profile>, at: Vector3<f64>, damage: f64) {
        let vpm = arena.view.vpm;
        let fire = Element::look(Some(Element::Fire));
        for (k, (h, r)) in [(0.35, 0.95), (1.0, 0.85), (1.65, 0.72), (2.3, 0.6), (2.95, 0.48), (3.6, 0.36)].into_iter().enumerate() {
            let life = 0.4 + 0.07 * k as f64;
            self.blasts.push(Blast { position: at + Vector3::new(0.0, 0.0, h * vpm), look: fire, radius: r, life, lifetime: life });
        }
        for k in 0..14 {
            let v = (self.any_direction() * 0.45 + Vector3::new(0.0, 0.0, 1.0)) * (5.0 + 3.0 * random(&mut self.rng)) * vpm;
            self.drop(at + Vector3::new(0.0, 0.0, 0.3 * vpm), v, if k % 3 == 0 { DROP_ASH } else { DROP_EMBER }, 1.3);
        }
        self.flashes.push(Flash { position: at + Vector3::new(0.0, 0.0, 1.5 * vpm), color: Element::Fire.light(), reach: 10.0, life: 0.4, lifetime: 0.4 });
        self.cues.push(Cue::at(Sound::Burst, at, 0.85).pitched(0.6));
        self.cues.push(Cue::at(Sound::Breath, at, 0.45).pitched(1.8));
        let near = (1.0 - (at - arena.view.eye).norm() / vpm / 30.0).max(0.0);
        self.shake = self.shake.max(0.5 * near);
        for i in Combat::under(arena, at, 1.8, 3.0) {
            self.strike(arena, profile, i, Blow { damage, crit: false, heavy: true, from: at, depth: 0, element: Some(Element::Fire) });
            let s = &mut arena.animals[i].status;
            s.burn = s.burn.max(4.0);
            s.burn_dps = s.burn_dps.max(damage * 0.2);
            s.staggered = s.staggered.max(0.8);
        }
        self.keep_pools_few();
        self.lingering.push(Lingering { pool: true, cloud: false, tall: false, position: at, radius: 1.3, life: 3.0, element: Element::Fire, profile: profile.clone(), damage: damage * 0.15, tick: 0.5, effects: false, collapse: 0.0 });
    }

    /// A tome's familiar conjured a little way ahead, to hang there casting at whatever's near,
    /// the oldest of them going if there are too many
    fn conjure(&mut self, profile: &Rc<Profile>, phantom: bool, at: Vector3<f64>) {
        let damage = profile.stat("spellDamage") + profile.stat("elementalDamage");
        if !phantom && self.familiars.iter().filter(|f| !f.phantom).count() >= FAMILIARS {
            if let Some(k) = self.familiars.iter().position(|f| !f.phantom) {
                self.familiars.remove(k);
            }
        }
        self.familiars.push(Familiar { position: at, life: FAMILIAR_SECONDS, wait: 0.3, profile: profile.clone(), damage: if phantom { damage * 0.5 } else { damage }, phantom });
        self.sparks(at, 14, 2.0, Element::look(if phantom { Some(Element::Holy) } else { profile.element }), -0.1, 0.5);
        self.cues.push(Cue::at(Sound::Summon, at, 0.6).pitched(if phantom { 0.8 } else { 1.0 }));
    }

    /// Familiars and phantoms casting at what's nearest, and wearing off; spectral blades going
    /// round; stilled time holding what's in it; a soul siphon draining
    fn conjured(&mut self, arena: &mut Arena, weapon: Option<&Weapon>, dt: f64) {
        let vpm = arena.view.vpm;
        let feet = arena.player.unwrap_or(arena.view.eye - Vector3::new(0.0, 0.0, 1.6 * vpm));
        for mut f in std::mem::take(&mut self.familiars) {
            f.life -= dt;
            f.wait -= dt;
            if f.wait <= 0.0 {
                f.wait = if f.phantom { 0.5 } else { FAMILIAR_PACE };
                let near = arena
                    .animals
                    .iter()
                    .filter(|a| a.health > 0.0 && !a.look.is_pet())
                    .map(|a| a.center(vpm as u32))
                    .filter(|c| (c - f.position).norm() < 20.0 * vpm)
                    .min_by(|a, b| (a - f.position).norm().total_cmp(&(b - f.position).norm()));
                if let Some(to) = near {
                    let speed = f.profile.stat("projectileSpeed").max(20.0) * vpm;
                    self.launch(&f.profile, None, Form::Missile, f.position, (to - f.position).normalize() * speed, f.damage, 0.0);
                    self.fine(f.position, Vector3::zeros(), Element::look(f.profile.element), 0.2, 0.0);
                }
            }
            if f.life > 0.0 {
                self.familiars.push(f);
            } else {
                self.sparks(f.position, 8, 1.5, Element::look(f.profile.element), 0.0, 0.4);
            }
        }
        if let Some(mut o) = self.orbit.take() {
            o.left -= dt;
            o.angle += dt * 4.5;
            o.tick -= dt;
            if o.tick <= 0.0 {
                o.tick = 0.25;
                let near: Vec<usize> = (0..arena.animals.len())
                    .filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet() && (arena.animals[i].position - feet).xy().norm() < (3.2 + arena.animals[i].look.body().1) * vpm)
                    .collect();
                for i in near {
                    let at = arena.animals[i].center(vpm as u32);
                    self.sparks(at, 4, 2.5, Element::look(Some(Element::Arcane)), 0.4, 0.3);
                    self.strike(arena, &o.profile, i, Blow { damage: o.damage, crit: false, heavy: false, from: feet, depth: 1, element: o.profile.element });
                }
            }
            if o.left > 0.0 {
                self.orbit = Some(o);
            }
        }
        for mut s in std::mem::take(&mut self.stases) {
            s.life -= dt;
            s.tick -= dt;
            let r = s.radius * vpm;
            for a in arena.animals.iter_mut().filter(|a| a.health > 0.0 && !a.look.is_pet()) {
                if (a.center(vpm as u32) - s.position).norm() < r + a.look.body().1 * vpm {
                    a.status.slowed = a.status.slowed.max(0.15);
                    a.status.slow = a.status.slow.max(0.85);
                    a.status.push *= 0.5;
                }
            }
            // What bosses threw hangs in it too
            for h in &mut self.hostiles {
                if (h.position - s.position).norm() < r {
                    h.velocity *= (-4.0 * dt).exp();
                    h.life += dt * 0.8;
                }
            }
            // Motes drifting round its skin
            if random(&mut self.rng) < dt * 70.0 {
                let d = self.any_direction();
                let round = d.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                self.sparks.push(Spark { position: s.position + d * r, velocity: round * 1.2 * vpm, look: Element::look(Some(Element::Arcane)), life: 0.5, lifetime: 0.5, gravity: 0.0, slot: SLOT_SPARK });
                self.cap_sparks();
            }
            if s.tick <= 0.0 {
                s.tick = 0.5;
                let inside: Vec<usize> = (0..arena.animals.len()).filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet() && (arena.animals[i].center(vpm as u32) - s.position).norm() < r).collect();
                for i in inside {
                    self.strike(arena, &s.profile, i, Blow { damage: s.damage, crit: false, heavy: false, from: s.position, depth: 1, element: Some(Element::Arcane) });
                }
            }
            if s.life > 0.0 {
                self.stases.push(s);
            }
        }
        if let Some(mut s) = self.siphon.take() {
            s.left -= dt;
            if let Some(weapon) = weapon {
                let from = self.held_point(weapon, arena.view, false);
                let forward = arena.view.forward();
                let near = arena
                    .animals
                    .iter()
                    .filter(|a| a.health > 0.0 && !a.look.is_pet())
                    .map(|a| a.center(vpm as u32))
                    .filter(|c| (c - from).norm() < 20.0 * vpm && (c - from).normalize().dot(&forward) > 0.5)
                    .min_by(|a, b| (a - from).norm().total_cmp(&(b - from).norm()));
                let to = near.map_or(from + forward * 12.0 * vpm, |c| from + (c - from) * 1.05);
                let profile = s.profile.clone();
                self.pour(arena, &profile, from, to, s.damage, dt, 0.35);
                self.heal = self.heal.min(40.0);
            }
            if s.left > 0.0 {
                self.siphon = Some(s);
            }
        }
        self.reel(arena, dt);
        // Runes waiting on the ground, going off as anything comes near
        for mut m in std::mem::take(&mut self.mines) {
            m.life -= dt;
            m.arming -= dt;
            let r = 2.2 * vpm;
            let tripped = m.arming <= 0.0
                && arena.animals.iter().any(|a| a.health > 0.0 && !a.look.is_pet() && (a.position - m.position).xy().norm() < r + a.look.body().1 * vpm && (a.position.z - m.position.z).abs() < 4.0 * vpm);
            if tripped {
                let profile = m.profile.clone();
                self.scarless = true;
                self.burst(arena, &profile, m.position + Vector3::new(0.0, 0.0, 0.5 * vpm), 2.6, m.damage, 0, false, Some(Element::Arcane), None);
                self.scarless = false;
                self.swells.push(Swell { model: PILLAR, position: m.position, radius: 0.5, life: 0.45, lifetime: 0.45 });
            } else if m.life > 0.0 {
                if random(&mut self.rng) < dt * 2.5 {
                    self.mote(m.position + Vector3::new(0.0, 0.0, 0.15 * vpm), Some(Element::Arcane), 1.3);
                }
                self.mines.push(m);
            }
        }
        // A prismatic beam swept across from the left to the right, its colors running through
        // fire, frost, lightning, arcane and holy, doing what each does to what it crosses
        if let Some(mut s) = self.sweep.take() {
            s.t += dt;
            if let Some(weapon) = weapon
                && s.t < s.length
            {
                if s.t > 0.0 {
                    let view = arena.view;
                    let k = s.t / s.length;
                    let up = view.rotation * Vector3::z_axis();
                    let direction = UnitQuaternion::from_axis_angle(&up, 0.95 - 1.9 * smooth(k)) * view.forward();
                    let from = self.held_point(weapon, view, false);
                    let element = PRISM[((k * PRISM.len() as f64) as usize).min(PRISM.len() - 1)];
                    // Out to whatever it crosses first, or the ground
                    let to = from + direction * 20.0 * vpm;
                    let mut end = solid_along(arena.world, from, to).unwrap_or(1.0);
                    let mut struck = None;
                    for (i, a) in arena.animals.iter().enumerate().filter(|(_, a)| a.health > 0.0 && !a.look.is_pet()) {
                        if let Some(t) = sphere_hit(from, to, a.center(vpm as u32), a.look.body().1 * vpm).filter(|&t| t < end) {
                            end = t;
                            struck = Some(i);
                        }
                    }
                    let end = from + (to - from) * end;
                    let look = Element::look(Some(element));
                    // Drawn solid, so it shows right across the view
                    self.rays.push((from, end, look));
                    self.flashes.push(Flash { position: end, color: element.light(), reach: 3.0, life: dt * 1.5, lifetime: dt * 1.5 });
                    if random(&mut self.rng) < dt * 25.0 {
                        self.mote(end, Some(element), 1.5);
                    }
                    self.beam_hum -= dt;
                    if self.beam_hum <= 0.0 {
                        self.beam_hum = 0.3;
                        self.cues.push(Cue::here(Sound::Beam, 0.45).pitched(1.2));
                    }
                    // Each thing it crosses is struck once, by whatever color it is there
                    if let Some(i) = struck.filter(|&i| !s.hit.contains(&arena.animals[i].id)) {
                        s.hit.push(arena.animals[i].id);
                        let mut tinted = (*s.profile).clone();
                        tinted.element = Some(element);
                        let tinted = Rc::new(tinted);
                        self.element_hit(end, Some(element), 1.4);
                        self.strike(arena, &tinted, i, Blow { damage: s.damage, crit: false, heavy: false, from, depth: 0, element: Some(element) });
                    }
                    if let Some(i) = struck.filter(|&i| arena.animals[i].health > 0.0) {
                        let st = &mut arena.animals[i].status;
                        match element {
                            Element::Fire => {
                                st.burn = st.burn.max(3.0);
                                st.burn_dps = st.burn_dps.max(s.damage * 0.12);
                            }
                            Element::Frost => {
                                st.slowed = st.slowed.max(2.5);
                                st.slow = st.slow.max(0.5);
                                st.chill += 0.3;
                                if st.chill >= 1.0 {
                                    st.chill = 0.0;
                                    st.frozen = st.frozen.max(2.0);
                                }
                            }
                            Element::Storm => st.shocked = 0.5,
                            _ => {}
                        }
                    }
                }
                self.sweep = Some(s);
            }
        }
        self.throw_weapon(arena, dt);
        self.drag(arena, weapon, dt);
        self.wax(arena, weapon, dt);
        self.fire_lobs(arena, weapon, dt);
    }

    /// A harpoon's line pulling: what it caught reeled in to the player, or the player hauled out
    /// to what's too big to reel in or to where it bit into the ground, with one heave
    fn reel(&mut self, arena: &mut Arena, dt: f64) {
        let Some(mut t) = self.tether.take() else {
            return;
        };
        let vpm = arena.view.vpm;
        let feet = arena.player.unwrap_or(arena.view.eye - Vector3::new(0.0, 0.0, 1.6 * vpm));
        let first = t.t == 0.0;
        t.t += dt;
        let caught = t.caught.and_then(|id| arena.animals.iter().position(|a| a.id == id && a.health > 0.0));
        if let Some(i) = caught {
            t.at = arena.animals[i].center(vpm as u32);
        }
        let holding = match (t.caught, caught) {
            // Dead, or gone: the line goes slack
            (Some(_), None) => false,
            (Some(_), Some(i)) if t.reel => {
                let a = &mut arena.animals[i];
                let to = feet - a.position;
                let gap = to.xy().norm() / vpm - a.look.body().1;
                if gap > REEL_NEAR && t.t < 1.8 {
                    a.status.push = Vector3::new(to.x, to.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros) * REEL_SPEED * vpm;
                    a.status.staggered = a.status.staggered.max(0.15);
                    true
                } else {
                    a.status.staggered = a.status.staggered.max(0.6);
                    false
                }
            }
            // Hauled out to it in one heave, up and over whatever's between, held a moment
            _ => {
                if first {
                    let to = t.at - feet;
                    // Stopping short of what it caught, as that's coming on too
                    let stop = caught.map_or(1.0, |i| arena.animals[i].look.body().1 * 1.5 + 2.5);
                    let distance = (to.xy().norm() / vpm - stop).max(0.0);
                    let rise = (to.z / vpm).max(0.0);
                    let up = if rise > 0.5 { (2.0 * 9.8 * (rise + 0.5)).sqrt().min(14.0) } else { 3.0 };
                    let flat = Vector3::new(to.x, to.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros);
                    self.dash += flat * (distance / 0.36).min(36.0) * vpm + Vector3::new(0.0, 0.0, up * vpm);
                    self.cues.push(Cue::here(Sound::Dodge, 0.8).pitched(0.8));
                }
                t.t < 0.45
            }
        };
        if holding {
            self.tether = Some(t);
        } else {
            self.cues.push(Cue::here(Sound::Taut, 0.4).pitched(1.3));
        }
    }

    /// The weapon thrown by Hurled Blade, out to its reach easing off, then back to the hand
    /// speeding up, cutting through whatever it passes each way
    fn throw_weapon(&mut self, arena: &mut Arena, dt: f64) {
        let Some(mut t) = self.thrown.take() else {
            return;
        };
        let vpm = arena.view.vpm;
        let before = t.t;
        t.t += dt;
        if t.t > 0.0 {
            let k = (t.t / t.length).min(1.0);
            let hand = arena.view.eye + arena.view.rotation * Vector3::new(0.22, 0.5, -0.2) * vpm;
            let far = t.from + t.direction * t.reach * vpm;
            let position = if k < 0.5 { t.from.lerp(&far, 1.0 - (1.0 - k * 2.0).powi(2)) } else { far.lerp(&hand, ((k - 0.5) * 2.0).powi(2)) };
            // Stopped short by a wall, it turns back from there
            if k < 0.5 && solid(arena.world, position) {
                t.reach = ((t.position - t.from).norm() / vpm).max(1.0);
                t.t = t.length * 0.5;
            } else {
                t.position = position;
            }
            if !t.back && k >= 0.5 {
                t.back = true;
                t.hit.clear();
            }
            let caught: Vec<usize> = (0..arena.animals.len())
                .filter(|&i| {
                    let a = &arena.animals[i];
                    a.health > 0.0 && !a.look.is_pet() && !t.hit.contains(&a.id) && (a.center(vpm as u32) - t.position).norm() < (1.3 + a.look.body().1) * vpm
                })
                .collect();
            let profile = t.profile.clone();
            for i in caught {
                t.hit.push(arena.animals[i].id);
                let at = arena.animals[i].center(vpm as u32);
                self.element_hit(at, profile.element, 1.2);
                self.sparks(at, 6, 3.0, Element::look(None), 0.8, 0.3);
                self.cues.push(Cue::at(Sound::Hit, at, 0.8));
                self.strike(arena, &profile, i, Blow { damage: t.damage, crit: false, heavy: true, from: t.position - t.direction * vpm, depth: 0, element: profile.element });
            }
            // Whirring as it spins
            if (t.t / 0.16).floor() != (before / 0.16).floor() {
                self.cues.push(Cue::at(Sound::Swing, t.position, 0.6).pitched(1.3));
            }
            self.fine(t.position, Vector3::zeros(), Element::look(profile.element), 0.2, 0.0);
        }
        if t.t < t.length {
            self.thrown = Some(t);
        } else {
            self.cues.push(Cue::here(Sound::Equip, 0.5).pitched(1.2));
        }
    }

    /// Shackle Chain going on: its chains shot out of the crossbow in hand as it kicks, each hooking
    /// into what it's flung at, then hauled on, dragging all they caught in onto the biggest, then
    /// crashing them into each other in a heap, a ring rolling out along the ground from it. With
    /// nothing caught, the one chain bites into the ground and is drawn back.
    fn drag(&mut self, arena: &mut Arena, weapon: Option<&Weapon>, dt: f64) {
        let Some(mut s) = self.shackle.take() else {
            return;
        };
        let view = arena.view;
        let vpm = view.vpm;
        let before = s.t;
        s.t += dt;
        let passed = |at: f64| before < at && s.t >= at;
        let (released, hauled, crashed) = (passed(SHACKLE_RELEASE), passed(SHACKLE_HAUL), passed(SHACKLE_CRASH));
        s.from = weapon.map_or(view.eye + view.rotation * Vector3::new(0.15, 0.6, -0.15) * vpm, |w| self.held_point(w, view, false));
        // Where each is now. What's died lets its chain go.
        s.chains.retain_mut(|c| match c.id {
            Some(id) => arena.animals.iter().find(|a| a.id == id && a.health > 0.0).map(|a| c.at = a.center(vpm as u32)).is_some(),
            None => true,
        });
        let look = Element::look(s.profile.element);
        if released {
            self.flashes.push(Flash { position: s.from, color: s.profile.element.map_or([1.0, 0.85, 0.6], Element::light), reach: 3.5, life: 0.15, lifetime: 0.15 });
            self.sparks(s.from, 5, 3.0, look, 0.3, 0.25);
            self.cues.push(Cue::here(Sound::Harpoon, 1.0).pitched(0.85));
            self.cues.push(Cue::here(Sound::Chain, 0.8).pitched(1.1));
        }
        // Each chain hooking in as it gets there
        let fly = s.t - SHACKLE_RELEASE;
        let mut hooks = Vec::new();
        for c in s.chains.iter_mut().filter(|c| !c.hooked && fly >= c.reach) {
            c.hooked = true;
            hooks.push((c.id, c.at));
        }
        for (id, at) in hooks {
            self.sparks(at, 6, 2.5, Element::look(None), 0.6, 0.3);
            match id.and_then(|id| arena.animals.iter().position(|a| a.id == id)) {
                // Bitten into and held, not hurt yet: whatever's caught lives to be dragged in and
                // crashed into the rest, which is where the damage is
                Some(i) => {
                    self.cues.push(Cue::at(Sound::Chain, at, 0.7).pitched(1.15));
                    self.flashes.push(Flash { position: at, color: s.profile.element.map_or([1.0, 0.85, 0.6], Element::light), reach: 3.0, life: 0.15, lifetime: 0.15 });
                    let st = &mut arena.animals[i].status;
                    st.flash = st.flash.max(0.09);
                    st.rooted = st.rooted.max(1.0);
                    st.staggered = st.staggered.max(0.4);
                }
                // Biting into the ground, dirt flying
                None => {
                    for _ in 0..6 {
                        let v = (self.any_direction() + Vector3::new(0.0, 0.0, 1.3)) * 3.0 * vpm;
                        self.drop(at, v, DROP_MUD, 0.8);
                    }
                    self.cues.push(Cue::at(Sound::Clang, at, 0.7).pitched(0.85));
                }
            }
        }
        let caught = s.chains.iter().any(|c| c.id.is_some());
        if !caught {
            // Drawn back in once it's bitten, or all it caught have died
            let back = s.chains.first().map_or(0.0, |c| c.reach) + 0.35;
            if fly < back && !s.chains.is_empty() {
                self.shackle = Some(s);
            }
            return;
        }
        if hauled {
            self.cues.push(Cue::here(Sound::Taut, 1.0).pitched(0.7));
            self.cues.push(Cue::at(Sound::Chain, s.center, 0.9).pitched(0.8));
            self.shake = self.shake.max(0.25);
        }
        // Hauled in onto the biggest, quicker the further each has to come, arriving together.
        // Bosses are too big to be moved.
        if s.t >= SHACKLE_HAUL && s.t < SHACKLE_CRASH {
            let left = (SHACKLE_CRASH - s.t).max(0.05);
            for c in &s.chains {
                let Some(i) = c.id.and_then(|id| arena.animals.iter().position(|a| a.id == id)) else {
                    continue;
                };
                let a = &mut arena.animals[i];
                let to = s.center - a.position;
                let gap = to.xy().norm() / vpm - a.look.body().1 - s.girth * 0.6;
                a.status.rooted = a.status.rooted.max(0.2);
                if a.boss.is_some() || Some(a.id) == s.anchor || gap <= 0.2 {
                    continue;
                }
                a.status.push = Vector3::new(to.x, to.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros) * (gap / left).min(24.0) * vpm;
                // Scraping up dirt as it's dragged
                if random(&mut self.rng) < dt * 7.0 {
                    let feet = a.position + Vector3::new(0.0, 0.0, 0.1 * vpm);
                    let v = (self.any_direction() * 0.6 + Vector3::new(0.0, 0.0, 1.0)) * 2.5 * vpm;
                    self.drop(feet, v, DROP_MUD, 0.6);
                }
            }
        }
        if !crashed {
            self.shackle = Some(s);
            return;
        }
        // Crashed into each other in a heap: each struck hard and knocked off its feet, bouncing
        // back apart, a blast and a ring rolling out from where they land, dirt and sparks thrown
        // up, and the ground dented
        let profile = s.profile.clone();
        let ground = ground_under(arena.world, s.center, vpm);
        let ids: Vec<u32> = s.chains.iter().filter_map(|c| c.id).collect();
        for &id in &ids {
            let Some(i) = arena.animals.iter().position(|a| a.id == id && a.health > 0.0) else {
                continue;
            };
            let at = arena.animals[i].center(vpm as u32);
            self.sparks(at, 4, 3.5, Element::look(None), 0.8, 0.35);
            self.strike(arena, &profile, i, Blow { damage: s.damage, crit: false, heavy: true, from: s.center, depth: 0, element: profile.element });
            let a = &mut arena.animals[i];
            let away = a.position - s.center;
            a.status.staggered = a.status.staggered.max(1.5);
            if a.boss.is_none() && Some(a.id) != s.anchor {
                a.status.push = Vector3::new(away.x, away.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x) * 4.0 * vpm;
            }
        }
        self.blasts.push(Blast { position: ground + Vector3::new(0.0, 0.0, 0.6 * vpm), look, radius: 1.6, life: 0.45, lifetime: 0.45 });
        self.flashes.push(Flash { position: ground + Vector3::new(0.0, 0.0, 1.2 * vpm), color: profile.element.map_or([1.0, 0.85, 0.6], Element::light), reach: 12.0, life: 0.4, lifetime: 0.4 });
        self.ripples.push(Ripple { center: ground, radius: 0.5, reach: 5.5, speed: 13.0, hit: ids, profile: profile.clone(), damage: s.damage * 0.35, push: 7.0, stagger: 0.8, effects: false, rocks: true, owed: 0.0 });
        for k in 0..18 {
            let a = k as f64 / 18.0 * TAU;
            let out = Vector3::new(a.cos(), a.sin(), 0.15);
            self.sparks.push(Spark { position: ground + Vector3::new(0.0, 0.0, 0.3 * vpm), velocity: out * 11.0 * vpm, look: Element::look(None), life: 0.45, lifetime: 0.45, gravity: 0.2, slot: SLOT_SPARK });
        }
        for _ in 0..10 {
            let v = (self.any_direction() * 0.7 + Vector3::new(0.0, 0.0, 1.2)) * 4.0 * vpm;
            self.drop(ground + Vector3::new(0.0, 0.0, 0.2 * vpm), v, DROP_MUD, 1.0);
        }
        // The chains snapping apart as they go slack, bits of them flying off all along them
        for c in &s.chains {
            for k in 1..4 {
                let at = s.from.lerp(&c.at, k as f64 / 4.0);
                let v = (self.any_direction() + Vector3::new(0.0, 0.0, 0.7)) * 2.5 * vpm;
                self.sparks.push(Spark { position: at, velocity: v, look, life: 0.45, lifetime: 0.45, gravity: 0.8, slot: SLOT_SPARK });
            }
        }
        self.cap_sparks();
        if !self.scarless && self.scar_timer <= 0.0 {
            self.craters.push((ground, 0.8));
            self.scar_timer = SCAR_GAP;
        }
        let near = (1.0 - (ground - view.eye).norm() / vpm / 30.0).max(0.0);
        self.shake = self.shake.max(0.3 + 0.5 * near);
        self.say_at(ground + Vector3::new(0.0, 0.0, 2.2 * vpm), "Shackled!", [205, 215, 230], 20.0);
        self.cues.push(Cue::at(Sound::Slam, s.center, 1.0).pitched(0.85));
        self.cues.push(Cue::at(Sound::Clang, s.center, 0.9).pitched(0.7));
        self.cues.push(Cue::at(Sound::Chain, s.center, 0.8).pitched(1.25));
    }

    /// Blood Moon going on, see BloodMoon: the shots let go as the weapon kicks, flying at what's
    /// near and tearing the blood out of each as they hit, or up at the moon if there's nothing,
    /// or a blade coming down and tearing it out of all of them at once; the blood streaming back
    /// to the player and healing them as it comes; and the moon hanging till it's all done, then
    /// sinking away
    fn wax(&mut self, arena: &mut Arena, weapon: Option<&Weapon>, dt: f64) {
        let Some(mut m) = self.blood_moon.take() else {
            return;
        };
        let view = arena.view;
        let vpm = view.vpm;
        m.t += dt;
        let blood = Element::look(Some(Element::Blood));
        // Where the blood comes back to, pooling a little way ahead where it's seen gathering, and
        // where the player drinks it from, just under the view
        let pool = blood_pool(view);
        let lips = view.eye + view.rotation * Vector3::new(0.0, 0.3, -0.5) * vpm;
        // Red light gathering on the weapon as it's raised to the moon
        if !m.released && m.t > 0.15 && random(&mut self.rng) < dt * 30.0 {
            let at = weapon.map_or(view.eye + view.rotation * Vector3::new(0.15, 0.6, -0.15) * vpm, |w| self.held_point(w, view, false));
            let v = self.any_direction() * 0.3 * vpm;
            self.fine(at + v * 0.4, -v, blood, 0.35, 0.0);
        }
        if !m.released && m.t >= m.release {
            m.released = true;
            let from = weapon.map_or(view.eye + view.rotation * Vector3::new(0.15, 0.6, -0.15) * vpm, |w| self.held_point(w, view, false));
            if m.blade {
                for id in m.targets.clone() {
                    if let Some(i) = arena.animals.iter().position(|a| a.id == id && a.health > 0.0) {
                        self.tear(arena, &mut m, i);
                    }
                }
                // A crescent of blood thrown out ahead along the cut, from high on the right
                // down across to the left
                for k in 0..14 {
                    let f = k as f64 / 13.0;
                    let a = 0.9 - 1.9 * f;
                    let out = view.rotation * Vector3::new(a.sin() * 1.1, 1.6, a.cos() * 0.7 - 0.3) * vpm;
                    let ahead = view.rotation * Vector3::new(0.0, 6.0, 0.0) * vpm;
                    self.sparks.push(Spark { position: view.eye + out, velocity: ahead + out * 2.0, look: blood, life: 0.35, lifetime: 0.35, gravity: 0.4, slot: SLOT_SPARK });
                }
                self.cap_sparks();
                self.cues.push(Cue::here(Sound::HeavySwing, 0.9).pitched(0.8));
            } else {
                for (k, &id) in m.targets.iter().enumerate() {
                    if let Some(a) = arena.animals.iter().find(|a| a.id == id && a.health > 0.0) {
                        let to = a.center(vpm as u32);
                        let arrive = 0.05 * k as f64 + (to - from).norm() / vpm / BLOODSHOT_PACE;
                        m.shots.push(Bloodshot { target: Some(id), from, to, position: from, t: 0.0, arrive });
                    }
                }
                // Nothing near: shot up at the moon
                if m.shots.is_empty() {
                    for k in 0..3 {
                        let to = m.moon + self.any_direction() * 2.0 * vpm;
                        m.shots.push(Bloodshot { target: None, from, to, position: from, t: 0.0, arrive: 0.55 + 0.06 * k as f64 });
                    }
                }
                self.flashes.push(Flash { position: from, color: Element::Blood.light(), reach: 4.0, life: 0.15, lifetime: 0.15 });
                let sound = match m.shot {
                    Some(Shot::Arrow) => Sound::Twang,
                    Some(Shot::Quarrel) => Sound::Release,
                    Some(_) => Sound::Gunshot,
                    None => Sound::Cast(blood as u8),
                };
                self.cues.push(Cue::here(sound, 0.9).pitched(0.75));
            }
        }
        // The shots flying, trailing blood, each tearing the blood out of what it reaches
        for mut shot in std::mem::take(&mut m.shots) {
            shot.t += dt;
            if let Some(a) = shot.target.and_then(|id| arena.animals.iter().find(|a| a.id == id && a.health > 0.0)) {
                shot.to = a.center(vpm as u32);
            }
            let k = (shot.t / shot.arrive).min(1.0);
            shot.position = shot.from.lerp(&shot.to, k) + Vector3::new(0.0, 0.0, 4.0 * k * (1.0 - k) * 0.4 * vpm);
            if random(&mut self.rng) < dt * 50.0 {
                self.mote(shot.position, Some(Element::Blood), 1.6);
            }
            if k < 1.0 {
                m.shots.push(shot);
                continue;
            }
            match shot.target.and_then(|id| arena.animals.iter().position(|a| a.id == id && a.health > 0.0)) {
                Some(i) => self.tear(arena, &mut m, i),
                // Gone into the moon, or what it was at is already dead
                None => {
                    self.sparks(shot.position, 5, 2.0, blood, 0.2, 0.4);
                    self.flashes.push(Flash { position: shot.position, color: Element::Blood.light(), reach: 6.0, life: 0.2, lifetime: 0.2 });
                }
            }
        }
        // Blood streaming back, arcing over into the pool ahead, each stream's health coming as its
        // blood gets there, and some of it running on from the pool into the player
        for b in &mut m.beads {
            b.velocity.z -= 9.8 * STREAM_GRAVITY * vpm * dt;
            b.velocity *= (-SPARK_DRAG * dt).exp();
            b.position += b.velocity * dt;
            b.life -= dt;
        }
        m.beads.retain(|b| b.life > 0.0);
        let pouring = m.streams.iter().filter(|s| s.t < STREAM_SECONDS).count().max(1);
        let rate = BEAD_RATE.min(BEADS as f64 / STREAM_FLIGHT / pouring as f64);
        for k in 0..m.streams.len() {
            m.streams[k].t += dt;
            if m.streams[k].t < STREAM_SECONDS {
                m.streams[k].owed += dt * rate;
                while m.streams[k].owed >= 1.0 {
                    m.streams[k].owed -= 1.0;
                    let start = m.streams[k].from + self.any_direction() * 0.2 * vpm;
                    let flight = STREAM_FLIGHT * (0.9 + 0.2 * random(&mut self.rng));
                    let velocity = streamed(start, pool, flight, STREAM_GRAVITY, vpm);
                    if m.beads.len() >= BEADS {
                        m.beads.remove(0);
                    }
                    m.beads.push(Bead { position: start, velocity, life: flight });
                }
            }
            if !m.streams[k].arrived && m.streams[k].t >= STREAM_FLIGHT {
                m.streams[k].arrived = true;
                m.gathered += 1;
                let gain = m.heal.min(MOON_HEAL - m.healed).max(0.0);
                if gain >= 0.5 {
                    m.healed += gain;
                    self.heal += gain;
                    self.say_at(pool, &format!("+{gain:.0}"), [255, 60, 75], 26.0);
                    self.flashes.push(Flash { position: pool, color: Element::Blood.light(), reach: 3.5, life: 0.3, lifetime: 0.3 });
                    self.cues.push(Cue::here(Sound::Squelch, 0.4).pitched(1.3));
                }
                self.drink(pool, lips, 8);
            }
        }
        self.cap_sparks();
        let done = m.released && m.shots.is_empty() && m.streams.iter().all(|s| s.t >= STREAM_SECONDS + STREAM_FLIGHT);
        // What's pooled drunk down all at once as soon as the last of it's there, a red flash over
        // the view
        if done && m.drunk.is_none() {
            m.drunk = Some(m.t);
            if m.gathered > 0 {
                self.drink(pool, lips, 18);
                self.flashes.push(Flash { position: lips, color: Element::Blood.light(), reach: 4.0, life: 0.45, lifetime: 0.45 });
                self.cues.push(Cue::here(Sound::Squelch, 0.6).pitched(0.7));
            }
            if m.healed > 0.0 {
                self.messages.push(format!("Blood Moon drank from {}: +{:.0} health.", m.streams.len(), m.healed));
            }
        }
        if done && m.t >= MOON_SECONDS && m.ending.is_none() {
            m.ending = Some(m.t);
        }
        if m.t >= m.end() + MOON_SINK {
            return;
        }
        self.blood_moon = Some(m);
    }

    /// Blood Moon tearing the blood out of animal `i`: struck hard, a gout of blood thrown out of
    /// it, and a stream of it starting back to the player
    fn tear(&mut self, arena: &mut Arena, m: &mut BloodMoon, i: usize) {
        let vpm = arena.view.vpm;
        let at = arena.animals[i].center(vpm as u32);
        let profile = m.profile.clone();
        self.strike(arena, &profile, i, Blow { damage: m.damage, crit: false, heavy: true, from: at, depth: 0, element: Some(Element::Blood) });
        for _ in 0..4 {
            let v = (self.any_direction() * 1.2 + Vector3::new(0.0, 0.0, 1.5)) * 2.5 * vpm;
            self.drop(at, v, DROP_BLOOD, 1.0);
        }
        self.sparks(at, 5, 3.0, Element::look(Some(Element::Blood)), 0.3, 0.35);
        self.flashes.push(Flash { position: at, color: Element::Blood.light(), reach: 4.0, life: 0.25, lifetime: 0.25 });
        self.cues.push(Cue::at(Sound::Squelch, at, 0.8).pitched(0.8));
        m.streams.push(Stream { from: at, t: 0.0, owed: 0.0, arrived: false });
    }

    /// `n` fine drops of blood running from Blood Moon's pool at `pool` into the player at `lips`
    fn drink(&mut self, pool: Vector3<f64>, lips: Vector3<f64>, n: usize) {
        let vpm = self.vpm;
        let blood = Element::look(Some(Element::Blood));
        for _ in 0..n {
            let start = pool + self.any_direction() * 0.12 * vpm;
            let flight = 0.22 + 0.1 * random(&mut self.rng);
            let velocity = streamed(start, lips, flight, 0.0, vpm);
            self.sparks.push(Spark { position: start, velocity, look: blood, life: flight, lifetime: flight, gravity: 0.0, slot: SLOT_FINE });
        }
        self.cap_sparks();
    }

    /// How strong Blood Moon's red is now, 0-1: rising as the moon does, sinking away with it, for
    /// the light it casts
    fn blood_moon(&self) -> f64 {
        self.blood_moon.as_ref().map_or(0.0, |m| smooth(m.t / 0.45) * (1.0 - smooth((m.t - m.end()) / MOON_SINK)))
    }

    /// Cluster Volley's bombs fired as their time comes, from the weapon's muzzle or the hand, each
    /// lobbed to come down where it's aimed, a flash, a puff of smoke and a thump as each goes
    fn fire_lobs(&mut self, arena: &mut Arena, weapon: Option<&Weapon>, dt: f64) {
        if self.lobs.is_empty() {
            return;
        }
        let view = arena.view;
        let vpm = view.vpm;
        let mut k = 0;
        while k < self.lobs.len() {
            self.lobs[k].delay -= dt;
            if self.lobs[k].delay > 0.0 {
                k += 1;
                continue;
            }
            let l = self.lobs.remove(k);
            let from = weapon.map_or(view.eye + view.rotation * Vector3::new(0.15, 0.5, 0.1) * vpm, |w| self.held_point(w, view, false));
            let (velocity, time) = lobbed(from, l.to, vpm);
            self.launch(&l.profile, Some(Missile { shot: Shot::Shell, head: 0, power: false, size: 1.25 }), Form::Bolt, from, velocity, l.damage, BOMB_GRAVITY);
            if let Some(bomb) = self.projectiles.last_mut() {
                bomb.trick = Trick::Bomb { fuse: time + 0.25 + 0.08 * l.k as f64, cluster: 4, hole: true };
                bomb.life = time + 3.0;
            }
            if l.profile.style == Style::Lob {
                if l.k == 0 {
                    self.cues.push(Cue::here(Sound::HeavySwing, 0.8).pitched(0.8));
                }
                continue;
            }
            self.flashes.push(Flash { position: from, color: [1.0, 0.75, 0.4], reach: 4.0, life: 0.12, lifetime: 0.12 });
            let ahead = velocity.try_normalize(1e-6).unwrap_or_else(|| view.forward());
            for _ in 0..2 {
                let life = 0.6 + 0.3 * random(&mut self.rng);
                let velocity = (ahead * 1.5 + self.any_direction() * 0.4) * vpm;
                self.sparks.push(Spark { position: from, velocity, look: 0, life, lifetime: life, gravity: -0.03, slot: SLOT_SMOKE });
            }
            self.sparks(from, 4, 3.0, Element::look(Some(Element::Fire)), 0.2, 0.2);
            let sound = if l.profile.build.crossbow() { Sound::Release } else { Sound::Gunshot };
            self.cues.push(Cue::here(sound, 0.8).pitched(0.6 + 0.04 * l.k as f32));
            self.kick_speed += 1.6;
        }
    }

    /// What to draw for a shackle's chains, see drag(): bursting out of the crossbow in a fan, one
    /// to each of what it catches, a glowing iron hook on the end of each; hanging from the
    /// crossbow to what they hooked; then hauled taut, drawing together as what they caught is
    /// dragged in onto the biggest. Thirty instances at most, while they're flying out.
    fn shackle_instances(&self, s: &Shackle, view: &View) -> Vec<AnimalInstance> {
        let vpm = view.vpm;
        let mut out = Vec::new();
        let fly = s.t - SHACKLE_RELEASE;
        if fly <= 0.0 {
            return out;
        }
        // About two dozen instances in all, however many chains, so the rest of what's flying
        // about still gets drawn
        let most = (18 / s.chains.len().max(1)).clamp(2, CHAIN_STRETCHES);
        let (hook_model, hook) = self.model_at(LASH_CRACKERS + CRACKER_HOOK);
        let (glow_model, glow) = self.effect_model(SLOT_FIREBALL, Element::look(s.profile.element));
        // A hook as big as it needs to be to be seen as far off as it is, its bend on `at`
        let hook_at = |out: &mut Vec<AnimalInstance>, at: Vector3<f64>, way: Vector3<f64>| {
            let way = way.try_normalize(1e-6).unwrap_or_else(Vector3::z);
            let rotation = UnitQuaternion::rotation_between(&Vector3::z(), &way).unwrap_or_else(|| UnitQuaternion::from_axis_angle(&Vector3::x_axis(), PI));
            let size = ((at - view.eye).norm() / vpm * 0.045).clamp(0.35, 1.2);
            out.push(place(hook_model, hook, LASH_RES, Vector3::new(0.0, 0.0, 0.8), at, rotation, vpm, size, MOVING));
        };
        // Hanging a little once they've bitten, taut once they're hauled on
        let sag = if s.t >= SHACKLE_HAUL { 0.0 } else { 0.05 };
        for c in &s.chains {
            if fly < c.reach {
                // Flying out, arcing a little, its hook glowing with the crossbow's element
                let k = fly / c.reach;
                let e = 1.0 - (1.0 - k) * (1.0 - k);
                let head = s.from.lerp(&c.at, e) + Vector3::new(0.0, 0.0, 2.0 * e * (1.0 - e) * vpm);
                out.extend(self.chain_instances(view.eye, s.from, head, 0.02, most));
                hook_at(&mut out, head, head - s.from);
                let size = ((head - view.eye).norm() / vpm * 0.012).clamp(0.1, 0.3);
                out.push(place(glow_model, glow, MODEL_RES, Vector3::zeros(), head, UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 9.0), vpm, size, MOVING));
            } else if c.id.is_none() {
                // Bitten into the ground, then drawn back in
                let r = ((fly - c.reach) / 0.35).min(1.0);
                let head = c.at.lerp(&s.from, r * r);
                out.extend(self.chain_instances(view.eye, s.from, head, 0.05, most));
                hook_at(&mut out, head, c.at - s.from);
            } else {
                out.extend(self.chain_instances(view.eye, s.from, c.at, sag, most));
                hook_at(&mut out, c.at, c.at - s.from);
            }
        }
        out
    }

    /// A shackle's heavy chain from `from` to `to`, sagging by `sag` of how long it is in the
    /// middle, as at most `most` straight stretches of links. Seen from `eye`, each stretch is as
    /// long and as thick as it is far off, so the links look much the same size all along it: the
    /// chain still reads as a chain twenty meters off, and the links by the crossbow don't fill
    /// the view.
    fn chain_instances(&self, eye: Vector3<f64>, from: Vector3<f64>, to: Vector3<f64>, sag: f64, most: usize) -> Vec<AnimalInstance> {
        let vpm = self.vpm;
        let length = (to - from).norm() / vpm;
        if length < 1e-3 {
            return Vec::new();
        }
        let n = chain_stretches(length, (from - eye).norm() / vpm, (to - eye).norm() / vpm, most);
        let point = |t: f64| from.lerp(&to, t) - Vector3::new(0.0, 0.0, 4.0 * t * (1.0 - t) * sag * length * vpm);
        let mut out = Vec::with_capacity(n.len());
        for w in n.windows(2) {
            let (a, b) = (point(w[0]), point(w[1]));
            let meters = (b - a).norm() / vpm;
            if meters < 1e-3 {
                continue;
            }
            let rotation = UnitQuaternion::rotation_between(&Vector3::z(), &((b - a) / (meters * vpm))).unwrap_or_else(|| UnitQuaternion::from_axis_angle(&Vector3::x_axis(), PI));
            let far = ((a + b) * 0.5 - eye).norm() / vpm;
            let ratio = (CHAIN_THICK * far).clamp(0.025, 0.18) / meters;
            let off = |k: usize| (LASH_TAPERS[k] / ratio).ln().abs();
            let taper = (0..LASH_TAPERS.len()).min_by(|&x, &y| off(x).total_cmp(&off(y))).unwrap_or(0);
            let (model, shape) = self.model_at(LASH_SEGMENTS + LASH_CHAIN * LASH_TAPERS.len() + taper);
            out.push(place(model, shape, LASH_RES, Vector3::zeros(), a, rotation, vpm, meters, MOVING));
        }
        out
    }

    /// What to draw for Blood Moon: the moon, rising up the sky, hanging and sinking away, and its
    /// shots in flight, each the weapon's own made of blood, or an orb of blood
    fn blood_moon_instances(&self, m: &BloodMoon, view: &View) -> Vec<AnimalInstance> {
        let vpm = view.vpm;
        let blood = Element::look(Some(Element::Blood));
        let mut out = Vec::new();
        let rise = smooth(m.t / 0.6);
        let sink = smooth((m.t - m.end()) / MOON_SINK);
        let size = MOON_SIZE * rise * (1.0 - sink);
        if size > 0.05 {
            let at = m.moon - Vector3::new(0.0, 0.0, (1.0 - rise + sink) * 8.0 * vpm);
            // Its face to the eye, so its rim glows all round, turning slowly, see effects::moon()
            let face = (view.eye - at).try_normalize(1e-6).unwrap_or_else(Vector3::z);
            let turn = UnitQuaternion::from_axis_angle(&Unit::new_normalize(face), self.time * 0.08) * UnitQuaternion::rotation_between(&Vector3::z(), &face).unwrap_or_else(UnitQuaternion::identity);
            let (model, moon) = self.model_at(MOON);
            out.push(place(model, moon, MOON_RES, Vector3::zeros(), at, turn, vpm, size, MOVING));
        }
        for s in &m.shots {
            let way = (s.to - s.from).try_normalize(1e-6).unwrap_or_else(Vector3::x);
            let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &way).unwrap_or_else(UnitQuaternion::identity);
            // Big enough to be seen on their way, but not so big as they leave the weapon that
            // they reach back past the eye and fill the view
            let off = (s.position - view.eye).norm() / vpm;
            if off < 1.0 {
                continue;
            }
            let grow = ((off - 0.6) / 5.0).clamp(0.2, 1.0);
            out.push(match m.shot {
                Some(Shot::Arrow) => {
                    let (model, arrow) = self.effect_model(SLOT_ARROW, blood);
                    place(model, arrow, ARROW_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), s.position, rotation, vpm, 2.0 * grow, MOVING)
                }
                Some(Shot::Quarrel) => {
                    let (model, quarrel) = self.effect_model(SLOT_QUARREL, blood);
                    place(model, quarrel, ARROW_RES, Vector3::new(QUARREL_HEAD, 0.0, 0.0), s.position, rotation, vpm, 2.4 * grow, MOVING)
                }
                Some(_) => {
                    let (model, ball) = self.effect_model(SLOT_SHOT, blood);
                    place(model, ball, SHOT_RES, Vector3::zeros(), s.position, rotation, vpm, 4.5 * grow, MOVING)
                }
                None => {
                    let spin = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.time * 6.0) * rotation;
                    let (model, ball) = self.effect_model(SLOT_FIREBALL, blood);
                    place(model, ball, MODEL_RES, Vector3::zeros(), s.position, spin, vpm, 0.28 * grow, MOVING)
                }
            });
        }
        // The drops of blood streaming back, each a streak along the way it's going
        let (model, streak) = self.effect_model(SLOT_BOLT, blood);
        for b in &m.beads {
            // Dwindling as they come in to the lips, never slabs of red across the view
            let off = (b.position - view.eye).norm() / vpm;
            let near = ((off - 0.55) / 4.5).clamp(0.0, 1.0);
            if near < 0.05 {
                continue;
            }
            let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &b.velocity).unwrap_or_else(UnitQuaternion::identity);
            out.push(place(model, streak, BOLT_RES, Vector3::zeros(), b.position, rotation, vpm, 1.3 * near, MOVING));
        }
        // The blood pooling ahead, swelling as each stream gets there, churning, then drunk down
        if m.gathered > 0 {
            let full = (m.gathered as f64 / 4.0).min(1.0);
            let drunk = m.drunk.map_or(1.0, |at| 1.0 - smooth((m.t - at) / 0.25));
            let size = (0.06 + 0.07 * full) * (1.0 + 0.12 * (self.time * 14.0).sin()) * drunk;
            if size > 0.02 {
                let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 5.0) * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.time * 3.0);
                let (model, ball) = self.effect_model(SLOT_FIREBALL, blood);
                out.push(place(model, ball, MODEL_RES, Vector3::zeros(), blood_pool(view), spin, vpm, size, MOVING));
            }
        }
        out
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
        let (length, _) = special_move(special, moves_kind(&profile), profile.style);
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
                self.lingering.push(Lingering { pool: false, cloud: false, tall: false, position: target, radius: 4.5, life: 2.2, element: Element::Void, profile: profile.clone(), damage: base * 0.2, tick: 0.0, effects: false, collapse: base * 2.2 });
            }
            Special::Blizzard => {
                self.lingering.push(Lingering { pool: false, cloud: false, tall: false, position: target, radius: 5.0, life: 4.0, element: Element::Frost, profile: profile.clone(), damage: base * 0.3, tick: 0.0, effects: true, collapse: 0.0 });
                self.stains.push((target, 5.0, SNOW, false));
            }
            Special::Plague => {
                self.lingering.push(Lingering { pool: false, cloud: false, tall: false, position: target, radius: 5.5, life: 7.0, element: Element::Poison, profile: profile.clone(), damage: base * 0.25, tick: 0.0, effects: true, collapse: 0.0 });
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
            // A red moon rising, then shots let go at what's near, or a blade cutting down, and the
            // blood torn out of each streaming back, see wax()
            Special::Bloodmoon => {
                let (length, keys) = special_move(special, moves_kind(&profile), profile.style);
                let mut near: Vec<(f64, u32)> = arena
                    .animals
                    .iter()
                    .filter(|a| a.health > 0.0 && !a.look.is_pet())
                    .map(|a| ((a.position - feet).norm() / vpm - a.look.body().1, a.id))
                    .filter(|&(d, _)| d < MOON_REACH)
                    .collect();
                near.sort_by(|a, b| a.0.total_cmp(&b.0));
                let blade = moves_kind(&profile) == Kind::Melee && profile.style != Style::Lob;
                // Bombs flung as orbs of blood, the rest shooting what they shoot
                let shot = weapon.shot().filter(|&s| s != Shot::Shell);
                let moon = moon_rise(view.eye, forward, view.tan[1], vpm);
                self.blood_moon = Some(BloodMoon {
                    t: 0.0,
                    release: first_release(length, &keys),
                    moon,
                    targets: near.iter().take(8).map(|&(_, id)| id).collect(),
                    shots: Vec::new(),
                    streams: Vec::new(),
                    beads: Vec::new(),
                    drunk: None,
                    released: false,
                    blade,
                    shot,
                    healed: 0.0,
                    heal: base * 0.3,
                    gathered: 0,
                    profile: Rc::new(Profile { element: Some(Element::Blood), ..(*profile).clone() }),
                    damage: base * 1.2,
                    ending: None,
                });
                // Blood spurting up out of the ground in a ring round the player as it rises, never
                // as high as the eye, the light going red
                let blood = Element::look(Some(Element::Blood));
                for k in 0..16 {
                    let a = k as f64 / 16.0 * TAU;
                    let out = Vector3::new(a.cos(), a.sin(), 0.0);
                    self.sparks.push(Spark { position: feet + out * 2.4 * vpm + Vector3::new(0.0, 0.0, 0.2 * vpm), velocity: (out * 2.0 + Vector3::new(0.0, 0.0, 3.0)) * vpm, look: blood, life: 0.6, lifetime: 0.6, gravity: 1.0, slot: SLOT_SPARK });
                }
                self.cap_sparks();
                self.flashes.push(Flash { position: feet + Vector3::new(0.0, 0.0, 2.0 * vpm), color: Element::Blood.light(), reach: 16.0, life: 0.7, lifetime: 0.7 });
                self.cues.push(Cue::here(Sound::Toll, 0.9).pitched(0.5));
                self.cues.push(Cue::here(Sound::Wail, 0.45).pitched(0.6));
            }
            Special::Starfall => {
                for k in 0..12 {
                    let from = feet + Vector3::new(0.0, 0.0, 3.0 * vpm) + disc(&mut self.rng, 1.5);
                    later(&mut self.pending, k as f64 * 0.05, Act::Star(from), base * 0.6);
                }
            }
            // A ring of ice bursting out round the player, freezing everything it reaches
            Special::FrostNova => {
                let frost = Element::look(Some(Element::Frost));
                for k in 0..40 {
                    let a = k as f64 / 40.0 * TAU;
                    let out = Vector3::new(a.cos(), a.sin(), 0.05);
                    self.sparks.push(Spark { position: feet + out * 0.8 * vpm + Vector3::new(0.0, 0.0, 0.4 * vpm), velocity: out * 14.0 * vpm, look: frost, life: 0.6, lifetime: 0.6, gravity: 0.3, slot: SLOT_SPARK });
                }
                self.cap_sparks();
                for k in 0..6 {
                    let a = k as f64 / 6.0 * TAU + 0.3;
                    let at = feet + Vector3::new(a.cos(), a.sin(), 0.0) * 4.5 * vpm;
                    self.swells.push(Swell { model: ICE_BURST, position: at, radius: 1.6, life: 1.2, lifetime: 1.2 });
                }
                self.cues.push(Cue::here(Sound::Crack, 1.0).pitched(0.7));
                let near: Vec<usize> = (0..arena.animals.len())
                    .filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet() && (arena.animals[i].position - feet).norm() < (9.0 + arena.animals[i].look.body().1) * vpm)
                    .collect();
                for i in near {
                    let at = arena.animals[i].center(vpm as u32);
                    self.element_hit(at, Some(Element::Frost), 1.5);
                    self.strike(arena, &profile, i, Blow { damage: base * 1.2, crit: false, heavy: true, from: feet, depth: 0, element: Some(Element::Frost) });
                    let s = &mut arena.animals[i].status;
                    s.frozen = s.frozen.max(3.0);
                    s.chill = 0.0;
                }
            }
            // Lightning strung round everything near, again and again
            Special::ThunderWeb => {
                for k in 0..4 {
                    later(&mut self.pending, 0.1 + 0.35 * k as f64, Act::Web(k), base * 0.6);
                }
            }
            // Gone in a flash and there in front of it, three cuts, then back out of it
            Special::BlinkStrike => {
                let aimed = arena
                    .animals
                    .iter()
                    .filter(|a| a.health > 0.0 && !a.look.is_pet())
                    .map(|a| (a.position, a.look.body().1, (a.center(vpm as u32) - target).norm()))
                    .filter(|&(p, _, d)| d < 5.0 * vpm && (p - feet).norm() < 25.0 * vpm)
                    .min_by(|a, b| a.2.total_cmp(&b.2));
                let (towards, distance) = match aimed {
                    Some((p, body, _)) => ((p - feet).xy().try_normalize(1e-6).map_or(flat, |v| Vector3::new(v.x, v.y, 0.0)), ((p - feet).xy().norm() / vpm - body - 1.2).max(0.0)),
                    None => (flat, room.min(10.0)),
                };
                self.sparks(view.eye, 20, -3.0, Element::look(Some(Element::Void)), 0.0, 0.4);
                self.flashes.push(Flash { position: view.eye, color: Element::Void.light(), reach: 6.0, life: 0.3, lifetime: 0.3 });
                self.dash = towards * (distance / 0.31).min(40.0) * vpm + Vector3::new(0.0, 0.0, 2.0 * vpm);
                self.cues.push(Cue::here(Sound::Dodge, 1.0).pitched(0.7));
                for k in 0..3 {
                    later(&mut self.pending, 0.24 + 0.1 * k as f64, Act::Slash, base * 1.3);
                }
            }
            Special::SpectralBlades => {
                self.orbit = Some(Orbit { left: 8.0, angle: 0.0, tick: 0.0, profile: profile.clone(), damage: base * 0.4 });
                self.cues.push(Cue::here(Sound::Summon, 0.8).pitched(1.3));
            }
            // Three phantoms standing round, ahead and to either side
            Special::PhantomHost => {
                self.familiars.retain(|f| !f.phantom);
                for a in [-1.1f64, 0.0, 1.1] {
                    let out = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), a) * flat;
                    let at = feet + out * 2.6 * vpm + Vector3::new(0.0, 0.0, 1.6 * vpm);
                    self.conjure(&profile, true, at);
                }
            }
            Special::SoulSiphon => self.siphon = Some(Siphon { left: 3.0, profile: profile.clone(), damage: base * 0.9 }),
            Special::Stasis => {
                self.stases.push(Stasis { position: target + Vector3::new(0.0, 0.0, 1.0 * vpm), radius: 6.0, life: 6.0, tick: 0.0, profile: profile.clone(), damage: base * 0.15 });
                self.cues.push(Cue::at(Sound::Toll, target, 1.0).pitched(0.5));
            }
            // One great bolt, through everything in a line, pinning it
            Special::Ballista => {
                let from = view.eye + view.rotation * Vector3::new(0.1, 0.6, -0.15) * vpm;
                let missile = Missile { shot: Shot::Quarrel, head: 0, power: true, size: 3.0 };
                self.launch(&profile, Some(missile), Form::Bolt, from, (target - from).normalize() * 120.0 * vpm, base * 3.0, QUARREL_GRAVITY * 0.3);
                if let Some(bolt) = self.projectiles.last_mut() {
                    bolt.pierce = 12;
                    bolt.pin = 2.0;
                }
                self.cues.push(Cue::here(Sound::Release, 1.0).pitched(0.55));
                self.shake = self.shake.max(0.8);
            }
            // The weapon's own shots fanning out, then each turning after what's near
            Special::SeekerSwarm => {
                let shot = weapon.shot().unwrap_or(Shot::Arrow);
                let from = view.eye + view.rotation * Vector3::new(0.1, 0.6, -0.1) * vpm;
                let up = view.rotation * Vector3::z();
                let right = view.rotation * Vector3::x();
                for k in 0..14 {
                    let a = (k as f64 / 13.0 - 0.5) * 2.2;
                    let direction = (forward + right * a.sin() * 0.9 + up * (0.25 + 0.2 * random(&mut self.rng))).normalize();
                    let size = if shot == Shot::Ball { 1.2 } else { 1.0 };
                    let speed = if shot == Shot::Arrow { 40.0 } else { 55.0 };
                    let missile = Missile { shot: if shot == Shot::Shell { Shot::Ball } else { shot }, head: k % 2, power: true, size };
                    self.launch(&profile, Some(missile), Form::Bolt, from, direction * speed * vpm, base * 0.5, 0.05);
                    if let Some(p) = self.projectiles.last_mut() {
                        p.seek = 4.0;
                        p.life = 3.0;
                    }
                }
                self.cues.push(Cue::here(if shot == Shot::Arrow { Sound::Twang } else { Sound::Release }, 0.9).pitched(1.2));
            }
            Special::BladeStorm => {
                for k in 0..3 {
                    later(&mut self.pending, 0.08 + 0.15 * k as f64, Act::Knives(k), base * 0.55);
                }
            }
            // Shells marching on ahead, one after another
            Special::Barrage => {
                for k in 0..7 {
                    let ahead = feet + flat * (4.0 + k as f64 * 2.2) * vpm + Vector3::new(0.0, 0.0, 3.0 * vpm);
                    let down = Vector3::new(0.0, 0.0, -8.0 * vpm);
                    let at = solid_along(arena.world, ahead, ahead + down).map_or(ahead + down, |t| ahead + down * t);
                    let side = (random(&mut self.rng) - 0.5) * 1.6 * vpm;
                    later(&mut self.pending, 0.2 + 0.18 * k as f64, Act::Shell(at + flat.cross(&Vector3::z()) * side), base * 1.4);
                }
            }
            // A line of fire across the way ahead
            Special::Firewall => {
                let across = flat.cross(&Vector3::z());
                for k in 0..5 {
                    let ahead = feet + flat * 6.0 * vpm + across * (k as f64 - 2.0) * 2.0 * vpm + Vector3::new(0.0, 0.0, 2.0 * vpm);
                    let down = Vector3::new(0.0, 0.0, -5.0 * vpm);
                    let at = solid_along(arena.world, ahead, ahead + down).map_or(ahead + down, |t| ahead + down * t);
                    self.lingering.push(Lingering { pool: false, cloud: false, tall: true, position: at, radius: 1.1, life: 6.0, element: Element::Fire, profile: profile.clone(), damage: base * 0.3, tick: 0.0, effects: true, collapse: 0.0 });
                    self.blasts.push(Blast { position: at + Vector3::new(0.0, 0.0, 0.6 * vpm), look: Element::look(Some(Element::Fire)), radius: 1.0, life: 0.5, lifetime: 0.5 });
                }
                self.cues.push(Cue::here(Sound::Breath, 0.8).pitched(1.2));
            }
            // Three bolts plunging down onto each of the nearest, one after another
            Special::BoltRain => {
                let mut near: Vec<(f64, u32)> = arena
                    .animals
                    .iter()
                    .filter(|a| a.health > 0.0 && !a.look.is_pet())
                    .map(|a| ((a.position - feet).norm(), a.id))
                    .filter(|&(d, _)| d < 22.0 * vpm)
                    .collect();
                near.sort_by(|a, b| a.0.total_cmp(&b.0));
                for (k, &(_, id)) in near.iter().take(8).enumerate() {
                    for j in 0..3 {
                        later(&mut self.pending, 0.15 + 0.06 * k as f64 + 0.22 * j as f64, Act::Plunge(id), base * 0.8);
                    }
                }
                if near.is_empty() {
                    for k in 0..9 {
                        let at = target + disc(&mut self.rng, 2.5);
                        later(&mut self.pending, 0.15 + k as f64 * 0.06, Act::Arrow(at), base * 0.6);
                    }
                }
            }
            // One bolt, glancing on from each thing it hits to the next
            Special::Ricochet => {
                let from = view.eye + view.rotation * Vector3::new(0.1, 0.6, -0.15) * vpm;
                let missile = Missile { shot: Shot::Quarrel, head: 0, power: true, size: 1.2 };
                self.launch(&profile, Some(missile), Form::Bolt, from, (target - from).normalize() * 90.0 * vpm, base * 1.3, QUARREL_GRAVITY * 0.2);
                if let Some(bolt) = self.projectiles.last_mut() {
                    bolt.trick = Trick::Ricochet(8);
                    bolt.seek = 2.5;
                }
                self.cues.push(Cue::here(Sound::Release, 1.0).pitched(1.1));
            }
            // A bolt that bursts apart in flight
            Special::SplitBolt => {
                let from = view.eye + view.rotation * Vector3::new(0.1, 0.6, -0.15) * vpm;
                let missile = Missile { shot: Shot::Quarrel, head: 1, power: true, size: 1.4 };
                self.launch(&profile, Some(missile), Form::Bolt, from, (target - from).normalize() * 70.0 * vpm, base * 1.2, QUARREL_GRAVITY * 0.3);
                if let Some(bolt) = self.projectiles.last_mut() {
                    bolt.trick = Trick::Split(0.3);
                }
                self.cues.push(Cue::here(Sound::Release, 1.0).pitched(0.9));
            }
            // A weighted net lobbed out to burst open over where it's aimed
            Special::NetBolt => {
                let from = view.eye + view.rotation * Vector3::new(0.1, 0.6, -0.15) * vpm;
                let ahead = target - from;
                let missile = Missile { shot: Shot::Quarrel, head: 3, power: false, size: 1.3 };
                self.launch(&profile, Some(missile), Form::Bolt, from, (ahead + Vector3::z() * ahead.norm() * 0.08).normalize() * 45.0 * vpm, base * 0.4, 0.5);
                if let Some(bolt) = self.projectiles.last_mut() {
                    bolt.trick = Trick::Net(target);
                    bolt.life = 2.5;
                }
                self.cues.push(Cue::here(Sound::Release, 0.9).pitched(0.75));
            }
            // Chains shot out of the crossbow at up to six near where it's aimed, within reach, the
            // biggest an anchor the rest are dragged to, see drag()
            Special::Shackle => {
                let from = self.held_point(weapon, view, false);
                let mut caught: Vec<(f64, usize)> = (0..arena.animals.len())
                    .filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet() && (arena.animals[i].position - view.eye).norm() < SHACKLE_REACH * vpm)
                    .map(|i| ((arena.animals[i].position - target).xy().norm(), i))
                    .filter(|&(d, _)| d < 8.0 * vpm)
                    .collect();
                caught.sort_by(|a, b| a.0.total_cmp(&b.0));
                caught.truncate(6);
                let anchor = caught.iter().map(|&(_, i)| i).max_by(|&a, &b| arena.animals[a].look.body().1.total_cmp(&arena.animals[b].look.body().1));
                let ground = ground_under(arena.world, target, vpm);
                let center = anchor.map_or(ground, |i| arena.animals[i].position);
                let mut chains: Vec<ShackleChain> = caught
                    .iter()
                    .enumerate()
                    .map(|(k, &(_, i))| {
                        let at = arena.animals[i].center(vpm as u32);
                        ShackleChain { id: Some(arena.animals[i].id), at, reach: chain_flight(k, (at - from).norm() / vpm), hooked: false }
                    })
                    .collect();
                // Nothing near: flung at the ground where it's aimed, as far as it reaches
                if chains.is_empty() {
                    let at = if (ground - from).norm() > CHAIN_FLING * vpm { ground_under(arena.world, from + (ground - from).normalize() * CHAIN_FLING * vpm, vpm) } else { ground };
                    chains.push(ShackleChain { id: None, at, reach: chain_flight(0, (at - from).norm() / vpm), hooked: false });
                }
                let girth = anchor.map_or(0.0, |i| arena.animals[i].look.body().1);
                let mut anchor = anchor.map(|i| arena.animals[i].id);
                // Only the one caught, and not a boss: reeled a good way in towards the player and
                // slammed down there instead
                let (center, girth) = match caught.as_slice() {
                    &[(_, i)] if arena.animals[i].boss.is_none() => {
                        anchor = None;
                        let a = &arena.animals[i];
                        let to = feet - a.position;
                        let pull = (to.xy().norm() * 0.4).min(SHACKLE_REEL * vpm).min((to.xy().norm() - (a.look.body().1 + 2.5) * vpm).max(0.0));
                        (ground_under(arena.world, a.position + Vector3::new(to.x, to.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros) * pull, vpm), 0.0)
                    }
                    _ => (center, girth),
                };
                self.shackle = Some(Shackle { center, chains, anchor, girth, from, t: 0.0, profile: profile.clone(), damage: base * 1.6 });
                self.cues.push(Cue::here(Sound::Chain, 0.6).pitched(0.85));
            }
            // A bolt that blows up in what it hits, the blast leaping on
            Special::ChainBlast => {
                let from = view.eye + view.rotation * Vector3::new(0.1, 0.6, -0.15) * vpm;
                let missile = Missile { shot: Shot::Quarrel, head: 0, power: true, size: 1.3 };
                self.launch(&profile, Some(missile), Form::Bolt, from, (target - from).normalize() * 80.0 * vpm, base * 1.1, QUARREL_GRAVITY * 0.3);
                if let Some(bolt) = self.projectiles.last_mut() {
                    bolt.trick = Trick::Detonate(7);
                    bolt.seek = 5.0;
                }
                self.cues.push(Cue::here(Sound::Release, 1.0).pitched(0.85));
            }
            // Everything in front hooked and dragged in to the player, then cut through
            Special::Harvest => {
                let hooked: Vec<usize> = (0..arena.animals.len())
                    .filter(|&i| {
                        let a = &arena.animals[i];
                        let to = a.position - feet;
                        a.health > 0.0 && !a.look.is_pet() && to.xy().norm() < 10.0 * vpm && to.xy().try_normalize(1e-6).is_some_and(|d| d.dot(&flat.xy()) > 0.45)
                    })
                    .collect();
                let chest = feet + Vector3::new(0.0, 0.0, 1.1 * vpm);
                let void = Element::look(Some(Element::Void));
                for i in hooked {
                    let a = &mut arena.animals[i];
                    let to = feet - a.position;
                    let gap = (to.xy().norm() / vpm - a.look.body().1 - 1.6).max(0.0);
                    // Thrown in so it comes to rest just in front, as a push dies away
                    if a.boss.is_none() {
                        a.status.push = Vector3::new(to.x, to.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros) * (gap * PUSH_DECAY).min(45.0) * vpm;
                    }
                    a.status.staggered = a.status.staggered.max(0.7);
                    let center = a.center(vpm as u32);
                    self.bolt_line(center, chest, void);
                }
                later(&mut self.pending, 0.42, Act::Reap, base * 2.2);
                self.cues.push(Cue::here(Sound::Chain, 0.9).pitched(0.8));
            }
            // A roar, everything near staggered, and a frenzy
            Special::BattleCry => {
                self.frenzy = 8.0;
                self.frenzy_bonus = 0.5;
                self.heal += 10.0;
                let blood = Element::look(Some(Element::Blood));
                for k in 0..36 {
                    let a = k as f64 / 36.0 * TAU;
                    let out = Vector3::new(a.cos(), a.sin(), 0.0);
                    self.sparks.push(Spark { position: feet + out * 0.8 * vpm + Vector3::new(0.0, 0.0, 1.0 * vpm), velocity: out * 11.0 * vpm, look: blood, life: 0.5, lifetime: 0.5, gravity: 0.0, slot: SLOT_SPARK });
                }
                self.cap_sparks();
                self.flashes.push(Flash { position: feet + Vector3::new(0.0, 0.0, 1.0 * vpm), color: Element::Blood.light(), reach: 10.0, life: 0.45, lifetime: 0.45 });
                self.cues.push(Cue::here(Sound::Roar, 1.0).pitched(1.35));
                self.cues.push(Cue::here(Sound::Quake, 0.6).pitched(1.3));
                let near: Vec<usize> = (0..arena.animals.len())
                    .filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet() && (arena.animals[i].position - feet).norm() < (12.0 + arena.animals[i].look.body().1) * vpm)
                    .collect();
                for i in near {
                    let away = arena.animals[i].position - feet;
                    let s = &mut arena.animals[i].status;
                    s.staggered = s.staggered.max(1.4);
                    s.push = Vector3::new(away.x, away.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x()) * 4.0 * vpm;
                    s.shown = 4.0;
                }
                self.messages.push("Frenzy: attacks 50% faster for 8 s.".into());
            }
            // The weapon thrown spinning out ahead
            Special::HurledBlade => {
                let from = view.eye + view.rotation * Vector3::new(0.25, 0.5, -0.1) * vpm;
                let reach = ((target - view.eye).norm() / vpm).clamp(6.0, 18.0);
                let direction = (target - from).try_normalize(1e-6).unwrap_or(forward);
                self.thrown = Some(Thrown { t: -0.28, length: 1.15, from, direction, reach, position: from, hit: Vec::new(), back: false, profile: profile.clone(), damage: base * 1.6 });
            }
            // A beam swept across from the left to the right, cycling through the elements
            Special::Prism => self.sweep = Some(Sweep { t: -0.12, length: 1.5, profile: profile.clone(), damage: base * 2.2, hit: Vec::new() }),
            // Six runes scattered round where it's aimed
            Special::RuneMines => {
                for k in 0..6 {
                    let a = k as f64 / 6.0 * TAU + random(&mut self.rng) * 0.5;
                    let out = target + Vector3::new(a.cos(), a.sin(), 0.0) * (2.0 + 2.0 * random(&mut self.rng)) * vpm + Vector3::new(0.0, 0.0, 2.0 * vpm);
                    let down = Vector3::new(0.0, 0.0, -6.0 * vpm);
                    let at = solid_along(arena.world, out, out + down).map_or(out + down, |t| out + down * t) + Vector3::new(0.0, 0.0, 0.08 * vpm);
                    self.mines.push(Mine { position: at, life: 20.0, arming: 0.5 + 0.1 * k as f64, profile: profile.clone(), damage: base * 1.5 });
                    self.sparks(at, 5, 1.5, Element::look(Some(Element::Arcane)), -0.2, 0.4);
                }
                if self.mines.len() > 12 {
                    let extra = self.mines.len() - 12;
                    self.mines.drain(..extra);
                }
                self.cues.push(Cue::at(Sound::Hex, target, 0.8).pitched(1.3));
            }
            // At what's nearest where it's aimed, or else whatever's ahead
            Special::Wildfire => {
                let tip = self.held_point(weapon, view, false);
                let aimed = arena
                    .animals
                    .iter()
                    .filter(|a| a.health > 0.0 && !a.look.is_pet())
                    .map(|a| ((a.center(vpm as u32) - target).norm(), a.id))
                    .filter(|&(d, _)| d < 5.0 * vpm)
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                match aimed {
                    Some((_, id)) => later(&mut self.pending, 0.1, Act::Kindle { from: tip, to: id, hop: 0 }, base * 1.4),
                    None => {
                        self.scarless = true;
                        self.burst(arena, &profile, target, 1.8, base, 0, false, Some(Element::Fire), None);
                        self.scarless = false;
                    }
                }
                self.cues.push(Cue::here(Sound::Breath, 0.7).pitched(1.4));
            }
            // Three rings of upheaval as the weapon slams down, each wider
            Special::Earthquake => {
                for k in 0..3 {
                    later(&mut self.pending, 0.45 + 0.3 * k as f64, Act::Quake(k), base * 1.2);
                }
            }
            // A comet of ice coming down at a slant out of the sky onto where it's aimed
            Special::Comet => {
                let ground = ground_under(arena.world, target, vpm);
                let across = flat.cross(&Vector3::z());
                let from = ground + (flat * -8.0 + across * 4.0 + Vector3::new(0.0, 0.0, 30.0)) * vpm;
                let frosty = Rc::new(Profile { element: Some(Element::Frost), ..(*profile).clone() });
                self.launch(&frosty, None, Form::Fireball, from, (ground - from).normalize() * 26.0 * vpm, base * 2.6, 0.0);
                if let Some(comet) = self.projectiles.last_mut() {
                    comet.trick = Trick::Comet(ground);
                    comet.size = 4.5;
                    comet.life = 4.0;
                }
                self.cues.push(Cue::at(Sound::Whistle, ground, 1.0).pitched(0.55));
            }
            // Five bombs lobbed out round where it's aimed, bomblets bursting out of each: fired one
            // after another as the weapon kicks, or flung out together, see own_move() and
            // fire_lobs()
            Special::ClusterBomb => {
                let ground = ground_under(arena.world, target, vpm);
                let flung = profile.style == Style::Lob;
                for k in 0..5 {
                    let to = ground_under(arena.world, ground + disc(&mut self.rng, 4.0), vpm);
                    let delay = if flung { VOLLEY_FLUNG } else { VOLLEY_FIRST + VOLLEY_GAP * k as f64 };
                    self.lobs.push(Lob { delay, to, k, profile: profile.clone(), damage: base * 1.05 });
                }
                self.cues.push(Cue::here(Sound::Fuse, 0.6));
            }
            Special::Thunderstorm => {
                let ground = ground_under(arena.world, target, vpm);
                self.zone(ZoneKind::Storm, &profile, ground, Vector3::zeros(), 7.0, 6.0, base * 1.1);
                if let Some(z) = self.zones.last_mut() {
                    z.tick = 0.6;
                }
                self.cues.push(Cue::at(Sound::Thunder, ground, 1.0).pitched(0.7));
            }
            Special::Mire => {
                let ground = ground_under(arena.world, target, vpm);
                self.zone(ZoneKind::Mire, &profile, ground, Vector3::zeros(), 6.0, 7.0, base * 0.3);
                for _ in 0..30 {
                    let v = self.any_direction() * 0.8 + Vector3::new(0.0, 0.0, 1.0 + random(&mut self.rng));
                    let kind = if random(&mut self.rng) < 0.5 { DROP_VENOM } else { DROP_MUD };
                    self.drop(ground, v * 4.0 * vpm, kind, 1.4);
                }
                self.cues.push(Cue::at(Sound::Squelch, ground, 1.0).pitched(0.6));
                self.cues.push(Cue::at(Sound::Bubble, ground, 0.8));
            }
            Special::Sunburst => {
                let ground = ground_under(arena.world, target, vpm);
                self.zone(ZoneKind::Sun, &profile, ground + Vector3::new(0.0, 0.0, SUN_HEIGHT * vpm), Vector3::zeros(), 9.0, 2.5, base * 0.6);
                if let Some(z) = self.zones.last_mut() {
                    z.tick = 0.35;
                }
                self.cues.push(Cue::at(Sound::Summon, ground, 1.0).pitched(0.6));
                self.cues.push(Cue::at(Sound::Toll, ground, 0.5).pitched(1.6));
            }
            Special::Shatterwave => later(&mut self.pending, 0.38, Act::Shatter, base * 2.0),
            Special::Cyclone => {
                let at = ground_under(arena.world, feet + flat * 2.5 * vpm, vpm);
                self.zone(ZoneKind::Cyclone, &profile, at, flat * 4.0 * vpm, 5.5, 4.0, base * 0.45);
                self.cues.push(Cue::at(Sound::Gale, at, 1.0));
            }
            // The ground bursting into flame under each of the nearest, one after another
            Special::Eruption => {
                let mut near: Vec<(f64, u32)> = arena
                    .animals
                    .iter()
                    .filter(|a| a.health > 0.0 && !a.look.is_pet())
                    .map(|a| ((a.position - feet).norm(), a.id))
                    .filter(|&(d, _)| d < 16.0 * vpm)
                    .collect();
                near.sort_by(|a, b| a.0.total_cmp(&b.0));
                for (k, &(_, id)) in near.iter().take(10).enumerate() {
                    later(&mut self.pending, 0.15 + 0.13 * k as f64, Act::Erupt(id), base * 1.4);
                }
                if near.is_empty() {
                    let ground = ground_under(arena.world, target, vpm);
                    for k in 0..5 {
                        let a = k as f64 / 5.0 * TAU;
                        let at = ground_under(arena.world, ground + Vector3::new(a.cos(), a.sin(), 0.0) * 3.0 * vpm, vpm);
                        later(&mut self.pending, 0.15 + 0.13 * k as f64, Act::Geyser(at), base);
                    }
                }
                self.cues.push(Cue::here(Sound::Rumble, 1.0).pitched(0.8));
                self.shake = self.shake.max(0.7);
            }
            Special::BallLightning => {
                let from = view.eye + forward * 1.5 * vpm - Vector3::new(0.0, 0.0, 0.2 * vpm);
                self.zone(ZoneKind::Ball, &profile, from, flat * 5.0 * vpm, 6.0, 5.0, base * 0.7);
                if let Some(z) = self.zones.last_mut() {
                    z.tick = 0.2;
                }
                self.cues.push(Cue::here(Sound::Zap, 1.0).pitched(0.7));
                self.cues.push(Cue::here(Sound::Beam, 0.6).pitched(0.8));
            }
            Special::Swarm => {
                let from = feet + flat * 1.0 * vpm + Vector3::new(0.0, 0.0, 1.4 * vpm);
                self.zone(ZoneKind::Swarm, &profile, from, flat * 6.0 * vpm, 2.6, 7.0, base * 0.35);
                self.cues.push(Cue::here(Sound::Buzz, 1.0));
            }
            Special::Avalanche => {
                later(&mut self.pending, 0.42, Act::Avalanche, base * 1.6);
                self.cues.push(Cue::here(Sound::Rumble, 0.8).pitched(0.7));
            }
            // Cut loose as the blade comes across, see special_move()
            Special::CrescentWave => {
                // Out ahead of the eye, never at it: its middle a couple of meters on and below
                // the eye, its horns, trailing back and up, still a meter and a half ahead and well
                // out to the sides. Not through a wall in the way.
                let ahead = view.eye + flat * 2.4 * vpm;
                let mut from = Vector3::new(ahead.x, ahead.y, (feet.z + 1.1 * vpm).min(view.eye.z - 0.45 * vpm));
                if let Some(t) = solid_along(arena.world, view.eye, from) {
                    from = view.eye + (from - view.eye) * t * 0.9;
                }
                self.zone(ZoneKind::Crescent, &profile, from, flat * 22.0 * vpm, 2.4, 1.25, base * 2.2);
                if let Some(z) = self.zones.last_mut() {
                    // Held a moment till the blade's come round
                    z.tick = 0.3;
                }
                self.cues.push(Cue::here(Sound::Shing, 1.0).pitched(0.9));
            }
            // Levelled and away, what's in the way skewered as the player goes
            Special::ImpalingCharge => {
                self.skewered.clear();
                self.charging = flat;
                self.dash = flat * (room.max(4.0) / 0.31).min(28.0) * vpm + Vector3::new(0.0, 0.0, 1.5 * vpm);
                for k in 0..6 {
                    later(&mut self.pending, 0.12 + 0.07 * k as f64, Act::Skewer(k), base * 0.5);
                }
                later(&mut self.pending, 0.62, Act::Fling, base * 2.0);
                self.cues.push(Cue::here(Sound::Rush, 1.0));
                self.cues.push(Cue::here(Sound::Roar, 0.5).pitched(1.5));
            }
            Special::CoilingLash => {
                // As the lash comes round the end of each turn overhead, see special_move()
                for k in 0..3 {
                    later(&mut self.pending, 0.31 + 0.3 * k as f64, Act::Coil(k), base * 1.1);
                }
            }
            // Faster and faster: each crack comes a little sooner after the last
            Special::Flay => {
                let mut at = 0.15;
                later(&mut self.pending, at, Act::Flay(0), base * 0.9);
                for k in 1..8 {
                    at += 0.17 * 0.88f64.powi(k as i32);
                    later(&mut self.pending, at, Act::Flay(k), base * 0.9);
                }
            }
            Special::Quickdraw => {
                for k in 0..10 {
                    later(&mut self.pending, 0.1 + 0.085 * k as f64, Act::Quick(k), base * 0.75);
                }
            }
            // Everything within 30 meters marked, the nearest first
            Special::HuntersMark => {
                let mut near: Vec<(f64, u32)> = arena
                    .animals
                    .iter()
                    .filter(|a| a.health > 0.0 && !a.look.is_pet())
                    .map(|a| ((a.position - feet).norm(), a.id))
                    .filter(|&(d, _)| d < 30.0 * vpm)
                    .collect();
                near.sort_by(|a, b| a.0.total_cmp(&b.0));
                self.marked = near.iter().take(12).map(|&(_, id)| id).collect();
                self.hunt = 10.0;
                let holy = Element::look(Some(Element::Holy));
                for a in arena.animals.iter().filter(|a| self.marked.contains(&a.id)) {
                    let over = a.center(vpm as u32) + Vector3::new(0.0, 0.0, (a.look.body().1 + 0.6) * vpm);
                    for k in 0..10 {
                        let t = k as f64 / 10.0 * TAU;
                        self.sparks.push(Spark { position: over, velocity: Vector3::new(t.cos(), t.sin(), 0.0) * 2.5 * vpm, look: holy, life: 0.35, lifetime: 0.35, gravity: 0.0, slot: SLOT_SPARK });
                    }
                }
                self.cap_sparks();
                self.cues.push(Cue::here(Sound::Chime, 1.0));
                self.messages.push(format!("{} marked: your shots seek them out and hit 50% harder for 10 s.", self.marked.len()));
            }
            // One at a time, the nearest to the crosshair first, each a moment after the last
            Special::Reckoning => {
                let mut seen: Vec<(f64, u32)> = arena
                    .animals
                    .iter()
                    .filter(|a| a.health > 0.0 && !a.look.is_pet())
                    .filter_map(|a| {
                        let to = a.center(vpm as u32) - view.eye;
                        let d = to.norm();
                        let clear = solid_along(arena.world, view.eye, a.center(vpm as u32) - to / d * a.look.body().1 * vpm).is_none();
                        (d < 45.0 * vpm && to.dot(&forward) > 0.35 * d && clear).then_some((1.0 - to.dot(&forward) / d + d / (200.0 * vpm), a.id))
                    })
                    .collect();
                seen.sort_by(|a, b| a.0.total_cmp(&b.0));
                for (k, &(_, id)) in seen.iter().take(8).enumerate() {
                    later(&mut self.pending, 0.18 + 0.14 * k as f64, Act::Reckon(id), base * profile.stat("critDamage").max(1.5) * 1.2);
                }
                if seen.is_empty() {
                    self.messages.push("Nothing in sight to reckon with.".into());
                }
                self.cues.push(Cue::here(Sound::Cock, 0.9).pitched(0.8));
            }
            // Point-blank: a cone of shot, a great flash and smoke, everything in front thrown back
            Special::Grapeshot => {
                let from = self.held_point(weapon, view, false);
                let up = view.rotation * Vector3::z();
                let right = view.rotation * Vector3::x();
                for _ in 0..26 {
                    let (a, r) = (random(&mut self.rng) * TAU, random(&mut self.rng).sqrt() * 0.42);
                    let direction = (forward + right * a.cos() * r + up * a.sin() * r * 0.6).normalize();
                    let speed = (55.0 + 15.0 * random(&mut self.rng)) * vpm;
                    self.launch(&profile, Some(Missile { shot: Shot::Ball, head: 0, power: true, size: 1.3 }), Form::Bolt, from, direction * speed, base * 0.35, BALL_GRAVITY);
                }
                for _ in 0..14 {
                    let v = (forward * (2.0 + 4.0 * random(&mut self.rng)) + self.any_direction() * 0.8) * vpm;
                    self.sparks.push(Spark { position: from, velocity: v, look: Element::look(None), life: 1.2, lifetime: 1.2, gravity: -0.05, slot: SLOT_SMOKE });
                }
                self.flare = FLARE * 2.0;
                self.flare_roll = random(&mut self.rng) * TAU;
                self.kick += 0.09;
                self.shake = self.shake.max(0.9);
                self.flashes.push(Flash { position: from, color: [1.0, 0.75, 0.4], reach: 10.0, life: 0.25, lifetime: 0.25 });
                self.cues.push(Cue::here(Sound::Gunshot, 1.0).pitched(0.55));
                self.cues.push(Cue::here(Sound::Slam, 0.7).pitched(1.2));
                let caught: Vec<usize> = (0..arena.animals.len())
                    .filter(|&i| {
                        let a = &arena.animals[i];
                        let to = a.center(vpm as u32) - view.eye;
                        a.health > 0.0 && !a.look.is_pet() && to.norm() < (9.0 + a.look.body().1) * vpm && to.normalize().dot(&forward) > 0.75
                    })
                    .collect();
                for i in caught {
                    let away = arena.animals[i].position - feet;
                    let near = 1.0 - ((away.norm() / vpm) / 10.0).min(0.8);
                    let boss = arena.animals[i].boss.is_some();
                    let s = &mut arena.animals[i].status;
                    s.staggered = s.staggered.max(0.9);
                    if !boss {
                        s.push = Vector3::new(away.x, away.y, 0.0).try_normalize(1e-6).unwrap_or(flat) * 16.0 * near * vpm;
                    }
                }
            }
            // A rocket sent up over each of the nearest, one after another
            Special::Skyburst => {
                let mut near: Vec<(f64, Vector3<f64>)> = arena
                    .animals
                    .iter()
                    .filter(|a| a.health > 0.0 && !a.look.is_pet())
                    .map(|a| ((a.position - feet).norm(), a.position))
                    .filter(|&(d, _)| d < 20.0 * vpm)
                    .collect();
                near.sort_by(|a, b| a.0.total_cmp(&b.0));
                let mut aims: Vec<Vector3<f64>> = near.iter().take(8).map(|&(_, at)| at).collect();
                if aims.is_empty() {
                    let ground = ground_under(arena.world, target, vpm);
                    aims = (0..5).map(|_| ground + disc(&mut self.rng, 4.0)).collect();
                }
                let from = view.eye + view.rotation * Vector3::new(0.2, 0.4, -0.3) * vpm;
                for (k, at) in aims.into_iter().enumerate() {
                    let burst = at + Vector3::new(0.0, 0.0, (6.5 + random(&mut self.rng)) * vpm);
                    // Up first, curving over onto it, so they go up in a fan
                    let rise = (burst - from).normalize() + Vector3::new(0.0, 0.0, 0.8);
                    self.launch(&profile, None, Form::Missile, from, rise.normalize() * 24.0 * vpm, base * 1.3, 0.0);
                    if let Some(rocket) = self.projectiles.last_mut() {
                        rocket.trick = Trick::Rocket(burst);
                        rocket.life = 1.6 + 0.12 * k as f64;
                        rocket.seek = 0.0;
                        rocket.hit = arena.animals.iter().map(|a| a.id).collect();
                    }
                }
                self.cues.push(Cue::here(Sound::Rocket, 1.0));
                self.cues.push(Cue::here(Sound::Fuse, 0.5).pitched(1.3));
            }
            // Set down a few meters ahead, its fuse burning
            Special::PowderKeg => {
                let at = ground_under(arena.world, feet + flat * (room.min(4.0).max(2.0)) * vpm + Vector3::new(0.0, 0.0, 1.0 * vpm), vpm);
                self.zone(ZoneKind::Keg, &profile, at, Vector3::zeros(), 5.0, 2.4 + KEG_SET, base * 3.2);
                self.cues.push(Cue::at(Sound::Fuse, at, 1.0));
                self.cues.push(Cue::at(Sound::Clang, at, 0.5).pitched(0.6));
            }
            // Pillars up under everything in front, the nearest first, or in a line ahead
            Special::StonePillars => {
                let mut ahead: Vec<(f64, Vector3<f64>)> = arena
                    .animals
                    .iter()
                    .filter(|a| a.health > 0.0 && !a.look.is_pet())
                    .filter_map(|a| {
                        let to = a.position - feet;
                        let d = to.xy().norm();
                        (d < 15.0 * vpm && to.xy().try_normalize(1e-6).is_some_and(|v| v.dot(&flat.xy()) > 0.45)).then_some((d, a.position))
                    })
                    .collect();
                ahead.sort_by(|a, b| a.0.total_cmp(&b.0));
                ahead.truncate(9);
                if ahead.len() < 4 {
                    for k in 0..(6 - ahead.len()) {
                        ahead.push((0.0, feet + flat * (3.0 + 2.2 * k as f64) * vpm));
                    }
                }
                for (k, &(_, at)) in ahead.iter().enumerate() {
                    let ground = ground_under(arena.world, at + Vector3::new(0.0, 0.0, 0.5 * vpm), vpm);
                    later(&mut self.pending, 0.36 + 0.09 * k as f64, Act::Pillar(ground), base * 1.5);
                }
            }
            Special::WreckingOrbit => {
                self.zones.retain(|z| z.kind != ZoneKind::Orbit);
                self.zone(ZoneKind::Orbit, &profile, feet, Vector3::zeros(), 1.8, 5.0, base * 0.8);
                self.cues.push(Cue::here(Sound::Whirr, 1.0).pitched(0.7));
                self.cues.push(Cue::here(Sound::Chain, 0.7));
            }
            // Whirled overhead, then flung round what's aimed at as special_move() throws it, and
            // yanked in as it's hauled back
            Special::LassoPull => {
                match Combat::aimed_foe(arena, target, 18.0) {
                    Some(i) => {
                        let id = arena.animals[i].id;
                        self.lash_aim = Some(arena.animals[i].center(vpm as u32));
                        later(&mut self.pending, 0.42, Act::Lasso { id, yank: false }, base * 0.5);
                        later(&mut self.pending, 0.64, Act::Lasso { id, yank: true }, base * 1.6);
                    }
                    None => {
                        self.lash_aim = Some(view.eye + forward * 12.0 * vpm);
                        self.messages.push("Nothing in reach to lasso.".into());
                    }
                }
                self.cues.push(Cue::here(Sound::Lasso, 0.9));
            }
            Special::CoilBind => {
                match Combat::aimed_foe(arena, target, 16.0) {
                    Some(i) => {
                        let id = arena.animals[i].id;
                        self.lash_aim = Some(arena.animals[i].center(vpm as u32));
                        later(&mut self.pending, 0.34, Act::Bind(id), base * 0.45);
                    }
                    None => {
                        self.lash_aim = Some(view.eye + forward * 10.0 * vpm);
                        self.messages.push("Nothing in reach to bind.".into());
                    }
                }
                self.cues.push(Cue::here(Sound::HeavySwing, 0.7).pitched(1.2));
            }
            // As the lash comes over the top and cracks, the cone rolling out from it
            Special::ThunderCrack => {
                for k in 0..4 {
                    later(&mut self.pending, 0.4 + 0.05 * k as f64, Act::Boom(k), base * 1.7);
                }
                self.cues.push(Cue::here(Sound::HeavySwing, 0.9).pitched(0.8));
            }
            // Cracked down onto the ground ahead, where it splits
            Special::SerpentLash => {
                let at = ground_under(arena.world, feet + flat * 4.0 * vpm + Vector3::new(0.0, 0.0, 1.0 * vpm), vpm);
                self.lash_aim = Some(at);
                later(&mut self.pending, 0.31, Act::Serpents, base * 1.1);
            }
            // Each eighth of the way round as the lash sweeps it, see special_cracks()
            Special::TripSweep => {
                for k in 0..8 {
                    later(&mut self.pending, 0.31 + 0.052 * k as f64, Act::Trip(k), base * 0.9);
                }
                self.cues.push(Cue::here(Sound::Swoop, 1.0).pitched(0.85));
            }
            // The line catching from near to far as the lash comes down along it
            Special::BurningBrand => {
                // From a couple of meters ahead on along the ground, each stretch lying on it
                // where it is, over what's in the way
                let reach = room.max(4.0).min(10.0) + 2.0;
                let along = |f: f64| ground_under(arena.world, feet + flat * (2.0 + (reach - 2.0) * f) * vpm + Vector3::new(0.0, 0.0, 1.2 * vpm), vpm);
                let path: Vec<Vector3<f64>> = (0..=BRAND_STRETCHES).map(|k| along(k as f64 / BRAND_STRETCHES as f64)).collect();
                self.lash_aim = Some(path[BRAND_STRETCHES as usize / 2]);
                let fiery = Rc::new(Profile { element: Some(Element::Fire), ..(*profile).clone() });
                self.zones.retain(|z| z.kind != ZoneKind::Brand);
                self.zone(ZoneKind::Brand, &fiery, path[0], path[BRAND_STRETCHES as usize] - path[0], 1.0, 5.4, base * 0.3);
                if let Some(z) = self.zones.last_mut() {
                    // Not alight till the lash comes down on it
                    z.tick = 0.4;
                    z.path = path.clone();
                }
                for k in 0..BRAND_STRETCHES {
                    later(&mut self.pending, 0.36 + 0.045 * k as f64, Act::Brand { from: path[k as usize], to: path[k as usize + 1], k }, base * 0.8);
                }
                self.cues.push(Cue::here(Sound::Breath, 0.5).pitched(1.5));
            }
            // Caught on the first thing high up ahead, or on nothing at all, and swung from
            Special::ChainSwing => {
                let up = (flat * 0.72 + Vector3::new(0.0, 0.0, 0.7)).normalize();
                let reach = 14.0 * vpm;
                let anchor = [up, (flat * 0.9 + Vector3::new(0.0, 0.0, 0.42)).normalize(), forward]
                    .iter()
                    .find_map(|&d| solid_along(arena.world, view.eye, view.eye + d * reach).filter(|&t| t * 14.0 > 2.5).map(|t| view.eye + d * reach * t))
                    .unwrap_or(view.eye + flat * 8.0 * vpm + Vector3::new(0.0, 0.0, 4.5 * vpm));
                self.lash_aim = Some(anchor);
                later(&mut self.pending, 0.28, Act::Swing { land: false }, 0.0);
                later(&mut self.pending, 1.0, Act::Swing { land: true }, base * 1.4);
                self.cues.push(Cue::here(Sound::Lasso, 0.7).pitched(1.2));
            }
            // A string of little charges flung out over the ground ahead, each going off a moment
            // after the one before
            Special::Firecrackers => {
                let from = view.eye + view.rotation * Vector3::new(0.2, 0.5, -0.25) * vpm;
                let side = flat.cross(&Vector3::z());
                for k in 0..8 {
                    let along = 2.5 + 1.25 * k as f64;
                    let at = ground_under(arena.world, feet + (flat * along + side * if k % 2 == 0 { 0.6 } else { -0.6 }) * vpm + Vector3::new(0.0, 0.0, 1.5 * vpm), vpm);
                    let (velocity, time) = lobbed(from, at, vpm);
                    self.launch(&profile, Some(Missile { shot: Shot::Shell, head: 0, power: false, size: 0.55 }), Form::Bolt, from, velocity, base * 0.75, BOMB_GRAVITY);
                    if let Some(b) = self.projectiles.last_mut() {
                        b.trick = Trick::Bomb { fuse: time + 0.3 + 0.13 * k as f64, cluster: 0, hole: false };
                        b.life = time + 3.0;
                    }
                }
                self.cues.push(Cue::here(Sound::Fuse, 0.8).pitched(1.3));
                self.cues.push(Cue::here(Sound::Chain, 0.5).pitched(1.4));
            }
            // An arrow or a bolt, bursting into thorns where it strikes
            Special::Thornburst => {
                let from = if weapon.profile.build == Build::None { self.held_point(weapon, view, true) } else { self.held_point(weapon, view, false) };
                let missile = if profile.build.crossbow() { Missile { shot: Shot::Quarrel, head: 2, power: true, size: 1.1 } } else { Missile::arrow(weapon.arrowhead(), true) };
                self.launch(&profile, Some(missile), Form::Bolt, from, (target - from).normalize() * 55.0 * vpm, base * 1.2, ARROW_GRAVITY * 0.5);
                if let Some(arrow) = self.projectiles.last_mut() {
                    arrow.trick = Trick::Thornburst;
                }
                self.cues.push(Cue::here(Sound::Twang, 0.9).pitched(0.85));
            }
        }
    }

    /// What a special attack's flung at: what's nearest where it's aimed, or else the nearest
    /// thing ahead, within `within` meters
    fn aimed_foe(arena: &Arena, target: Vector3<f64>, within: f64) -> Option<usize> {
        let view = arena.view;
        let vpm = view.vpm;
        let forward = view.forward();
        let alive = |a: &Animal| a.health > 0.0 && !a.look.is_pet() && (a.center(vpm as u32) - view.eye).norm() < within * vpm;
        let near_aim = (0..arena.animals.len())
            .filter(|&i| alive(&arena.animals[i]))
            .map(|i| ((arena.animals[i].center(vpm as u32) - target).norm(), i))
            .filter(|&(d, i)| d < (2.5 + arena.animals[i].look.body().1) * vpm)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        near_aim
            .or_else(|| {
                (0..arena.animals.len())
                    .filter(|&i| alive(&arena.animals[i]))
                    .filter_map(|i| {
                        let to = arena.animals[i].center(vpm as u32) - view.eye;
                        (to.normalize().dot(&forward) > 0.8).then_some((to.norm(), i))
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
            })
            .map(|(_, i)| i)
    }

    /// A projectile set off as it is, without multishot or echo
    #[allow(clippy::too_many_arguments)]
    fn launch(&mut self, profile: &Rc<Profile>, arrow: Option<Missile>, form: Form, from: Vector3<f64>, velocity: Vector3<f64>, damage: f64, gravity: f64) {
        let (head, power) = arrow.map_or((0, false), |m| (m.head, m.power));
        let (_, _, size, _) = form.flight();
        self.projectiles.push(Projectile {
            arrow: arrow.is_some(),
            shot: arrow.map_or(Shot::Arrow, |m| m.shot),
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
            size: arrow.map_or(size, |m| m.size),
            pin: 0.0,
            seek: 0.0,
            blast: 0.0,
            trick: Trick::None,
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
            // What the newer special attacks set off blows no holes, see update()
            self.scarless = matches!(
                p.act,
                Act::Quake(_) | Act::Spike(..) | Act::Erupt(_) | Act::Geyser(_) | Act::Avalanche | Act::Shatter | Act::Skewer(_) | Act::Fling | Act::Coil(_) | Act::Flay(_) | Act::Reckon(_) | Act::Pillar(_)
            );
            match p.act {
                Act::Meteor(at) => {
                    let from = at + Vector3::new((random(&mut self.rng) - 0.5) * 8.0, (random(&mut self.rng) - 0.5) * 8.0, 28.0) * vpm;
                    self.launch(&profile, None, Form::Fireball, from, (at - from).normalize() * 32.0 * vpm, p.damage, 0.0);
                }
                Act::Arrow(at) => {
                    let from = at + Vector3::new((random(&mut self.rng) - 0.5) * 3.0, (random(&mut self.rng) - 0.5) * 3.0, 30.0) * vpm;
                    let head = (random(&mut self.rng) * 3.0) as usize % 3;
                    self.launch(&profile, Some(Missile::arrow(head, true)), Form::Bolt, from, (at - from).normalize() * 45.0 * vpm, p.damage, ARROW_GRAVITY);
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
                // Everything within 18 meters, gone round in order and strung together
                Act::Web(k) => {
                    let mut near: Vec<(f64, usize)> = (0..arena.animals.len())
                        .filter(|&i| arena.animals[i].health > 0.0 && !arena.animals[i].look.is_pet() && (arena.animals[i].position - feet).norm() < 18.0 * vpm)
                        .map(|i| {
                            let to = arena.animals[i].position - feet;
                            (to.y.atan2(to.x), i)
                        })
                        .collect();
                    near.sort_by(|a, b| a.0.total_cmp(&b.0));
                    near.truncate(8);
                    let chest = feet + Vector3::new(0.0, 0.0, 1.2 * vpm);
                    let points: Vec<Vector3<f64>> = near.iter().map(|&(_, i)| arena.animals[i].center(vpm as u32)).collect();
                    match points.len() {
                        0 => {}
                        1 => self.arc(chest, points[0]),
                        n => {
                            for k in 0..n {
                                if n > 2 || k + 1 < n {
                                    self.arc(points[k], points[(k + 1) % n]);
                                }
                            }
                        }
                    }
                    for &(_, i) in &near {
                        arena.animals[i].status.shocked = 0.5;
                        self.strike(arena, &profile, i, Blow { damage: p.damage, crit: false, heavy: false, from: chest, depth: 1, element: Some(Element::Storm) });
                    }
                    // Crackling higher each time it's strung
                    self.cues.push(Cue::here(Sound::Crackle, 0.9).pitched(0.7 + 0.12 * k as f32));
                }
                // A ring of knives thrown out, turned a little further round each time, a spiral
                Act::Knives(k) => {
                    let from = feet + Vector3::new(0.0, 0.0, 1.1 * vpm);
                    for j in 0..10 {
                        let a = (j as f64 + 0.33 * k as f64) / 10.0 * TAU;
                        let out = Vector3::new(a.cos(), a.sin(), 0.02);
                        let swirl = Vector3::new(-a.sin(), a.cos(), 0.0) * 0.35;
                        let knife = Missile { shot: Shot::Quarrel, head: 0, power: true, size: 0.8 };
                        self.launch(&profile, Some(knife), Form::Bolt, from + out * 0.5 * vpm, (out + swirl).normalize() * 22.0 * vpm, p.damage, 0.05);
                        if let Some(knife) = self.projectiles.last_mut() {
                            knife.life = 0.7;
                            knife.pierce = 3;
                        }
                    }
                    self.cues.push(Cue::here(Sound::Swing, 0.8).pitched(1.3 + 0.1 * k as f32));
                }
                // A shell whistling down from high up onto it
                Act::Shell(at) => {
                    let from = at + Vector3::new((random(&mut self.rng) - 0.5) * 4.0, (random(&mut self.rng) - 0.5) * 4.0, 26.0) * vpm;
                    let missile = Missile { shot: Shot::Shell, head: 0, power: false, size: 1.4 };
                    self.launch(&profile, Some(missile), Form::Bolt, from, (at - from).normalize() * 34.0 * vpm, p.damage, 0.0);
                    if let Some(shell) = self.projectiles.last_mut() {
                        shell.blast = 2.4;
                    }
                    self.cues.push(Cue::at(Sound::Whistle, at, 0.6));
                }
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
                    self.launch(&profile, Some(Missile::arrow(head, true)), Form::Bolt, from, direction * speed, p.damage, ARROW_GRAVITY * 0.5);
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
                        self.lingering.push(Lingering { pool: false, cloud: false, tall: false, position: at, radius: 1.5, life: 3.0, element: lingers, profile: profile.clone(), damage: p.damage * 0.1, tick: 0.0, effects: false, collapse: 0.0 });
                    }
                }
                // A bolt plunging down out of the sky onto it
                Act::Plunge(id) => {
                    let Some(i) = arena.animals.iter().position(|a| a.id == id && a.health > 0.0) else {
                        continue;
                    };
                    let at = arena.animals[i].center(vpm as u32);
                    let from = at + Vector3::new((random(&mut self.rng) - 0.5) * 4.0, (random(&mut self.rng) - 0.5) * 4.0, 24.0) * vpm;
                    let missile = Missile { shot: Shot::Quarrel, head: (random(&mut self.rng) * 2.0) as usize % 2, power: true, size: 1.2 };
                    self.launch(&profile, Some(missile), Form::Bolt, from, (at - from).normalize() * 60.0 * vpm, p.damage, 0.0);
                    if random(&mut self.rng) < 0.3 {
                        self.cues.push(Cue::at(Sound::Whistle, at, 0.35).pitched(1.5));
                    }
                }
                // The blast leaping on: a streak of fire to the next, which goes up too
                Act::Blast { from, to, last, hop } => {
                    let Some(i) = arena.animals.iter().position(|a| a.id == to && a.health > 0.0) else {
                        continue;
                    };
                    let center = arena.animals[i].center(vpm as u32);
                    self.streak(from, center, Element::look(Some(Element::Fire)));
                    self.scarless = true;
                    self.burst(arena, &profile, center, 1.6, p.damage, 1, false, Some(Element::Fire), None);
                    self.scarless = false;
                    if hop < 7
                        && let Some(next) = self.nearest_other(arena, center, 9.0, &[to, last])
                    {
                        self.pending.push(Pending { delay: 0.16, act: Act::Blast { from: center, to: next, last: to, hop: hop + 1 }, profile, damage: p.damage * 0.95 });
                    }
                }
                // Fire leaping on to it, setting it alight, then on to what's near it that isn't
                Act::Kindle { from, to, hop } => {
                    let Some(i) = arena.animals.iter().position(|a| a.id == to && a.health > 0.0) else {
                        continue;
                    };
                    let center = arena.animals[i].center(vpm as u32);
                    self.streak(from, center, Element::look(Some(Element::Fire)));
                    self.element_hit(center, Some(Element::Fire), 1.2);
                    self.cues.push(Cue::at(Sound::Breath, center, 0.4).pitched(1.6 + 0.05 * hop as f32));
                    self.strike(arena, &profile, i, Blow { damage: p.damage, crit: false, heavy: false, from, depth: if hop == 0 { 0 } else { 1 }, element: Some(Element::Fire) });
                    if arena.animals[i].health > 0.0 {
                        let s = &mut arena.animals[i].status;
                        s.burn = s.burn.max(5.0);
                        s.burn_dps = s.burn_dps.max(p.damage * 0.3);
                    }
                    // Spreading to two at first, then one at a time, only to what isn't burning
                    let spread = if hop < 2 { 2 } else { 1 };
                    let mut caught = vec![to];
                    for _ in 0..spread {
                        let next = (0..arena.animals.len())
                            .filter(|&j| {
                                let a = &arena.animals[j];
                                a.health > 0.0 && !a.look.is_pet() && a.status.burn <= 0.0 && !caught.contains(&a.id)
                            })
                            .map(|j| (j, (arena.animals[j].center(vpm as u32) - center).norm()))
                            .filter(|&(_, d)| d < 7.0 * vpm)
                            .min_by(|a, b| a.1.total_cmp(&b.1));
                        if let Some((j, _)) = next.filter(|_| hop < 7) {
                            let id = arena.animals[j].id;
                            caught.push(id);
                            self.pending.push(Pending { delay: 0.3, act: Act::Kindle { from: center, to: id, hop: hop + 1 }, profile: profile.clone(), damage: p.damage * 0.9 });
                        }
                    }
                }
                // Cutting through everything dragged in in front
                Act::Reap => {
                    let view = arena.view;
                    let forward = view.forward();
                    let reach = (profile.stat("range").max(2.5) + 1.8) * vpm;
                    let caught: Vec<usize> = (0..arena.animals.len())
                        .filter(|&i| {
                            let a = &arena.animals[i];
                            let to = a.center(vpm as u32) - view.eye;
                            a.health > 0.0 && !a.look.is_pet() && to.norm() < reach + a.look.body().1 * vpm && to.normalize().dot(&forward) > 0.15
                        })
                        .collect();
                    let look = Element::look(profile.element.or(Some(Element::Void)));
                    for k in 0..30 {
                        let a = -1.2 + 2.4 * k as f64 / 29.0;
                        let out = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), a) * Vector3::new(forward.x, forward.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x());
                        self.fine(view.eye + out * 2.2 * vpm - Vector3::new(0.0, 0.0, 0.5 * vpm), out * 2.0 * vpm, look, 0.35, 0.0);
                    }
                    self.cues.push(Cue::here(Sound::HeavySwing, 1.0).pitched(0.7));
                    self.shake = self.shake.max(0.45);
                    for i in caught {
                        let at = arena.animals[i].center(vpm as u32);
                        self.element_hit(at, profile.element, 1.4);
                        self.strike(arena, &profile, i, Blow { damage: p.damage, crit: false, heavy: true, from: view.eye, depth: 0, element: profile.element });
                    }
                }
                // A ring of upheaval rolling out from the player's feet, boulders heaving up along
                // it, everything it passes staggered and thrown back
                Act::Quake(k) => {
                    let reach = [6.0, 9.0, 12.0][(k as usize).min(2)];
                    self.ripples.push(Ripple { center: feet, radius: 0.6, reach, speed: 12.0, hit: Vec::new(), profile: profile.clone(), damage: p.damage, push: 6.0, stagger: 0.9, effects: k == 0, rocks: true, owed: 0.0 });
                    let look = Element::look(profile.element);
                    for j in 0..36 {
                        let a = j as f64 / 36.0 * TAU;
                        let out = Vector3::new(a.cos(), a.sin(), 0.0);
                        self.sparks.push(Spark { position: feet + out * 0.8 * vpm + Vector3::new(0.0, 0.0, 0.3 * vpm), velocity: out * 9.0 * vpm, look, life: 0.4, lifetime: 0.4, gravity: 0.0, slot: SLOT_SPARK });
                    }
                    self.cap_sparks();
                    self.cues.push(Cue::at(Sound::Quake, feet, 1.0).pitched(1.1 - 0.15 * k as f32));
                    self.shake = self.shake.max(0.9 - 0.15 * k as f64);
                }
                Act::Spike(at, radius) => {
                    self.swells.push(Swell { model: ICE_BURST, position: at, radius, life: 0.9, lifetime: 0.9 });
                    for _ in 0..5 {
                        let v = (self.any_direction() + Vector3::new(0.0, 0.0, 1.3)) * 3.5 * vpm;
                        self.drop(at, v, DROP_ICE, 1.0);
                    }
                    self.cues.push(Cue::at(Sound::Crack, at, 0.6).pitched(1.1 + 0.1 * random(&mut self.rng) as f32));
                }
                Act::Erupt(id) => {
                    if let Some(i) = arena.animals.iter().position(|a| a.id == id && a.health > 0.0) {
                        let at = ground_under(arena.world, arena.animals[i].position + Vector3::new(0.0, 0.0, 0.5 * vpm), vpm);
                        self.geyser(arena, &profile, at, p.damage);
                    }
                }
                Act::Geyser(at) => self.geyser(arena, &profile, at, p.damage),
                // Boulders let loose in a ragged front, tumbling on ahead
                Act::Avalanche => {
                    let forward = arena.view.forward();
                    let flat = Vector3::new(forward.x, forward.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                    let across = flat.cross(&Vector3::z());
                    for k in 0..5 {
                        let side = (k as f64 - 2.0) * 1.8;
                        let start = ground_under(arena.world, feet + (flat * (2.0 + 0.8 * random(&mut self.rng)) + across * side) * vpm, vpm);
                        let speed = 10.0 + 2.5 * random(&mut self.rng);
                        self.zone(ZoneKind::Boulder, &profile, start + Vector3::new(0.0, 0.0, 0.9 * vpm), flat * speed * vpm, 0.9, 2.0, p.damage);
                    }
                    self.cues.push(Cue::at(Sound::Quake, feet, 1.0).pitched(1.25));
                    self.shake = self.shake.max(1.0);
                }
                // A wave of force tearing out across what's in front: everything in it hurt and
                // thrown back, and what's frozen shattered
                Act::Shatter => {
                    let view = arena.view;
                    let forward = view.forward();
                    let flat = Vector3::new(forward.x, forward.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                    let frost = Element::look(Some(Element::Frost));
                    for k in 0..36 {
                        let a = (k as f64 / 35.0 - 0.5) * 1.4;
                        let out = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), a) * flat;
                        self.sparks.push(Spark { position: feet + out * 1.0 * vpm + Vector3::new(0.0, 0.0, 0.9 * vpm), velocity: out * 17.0 * vpm, look: frost, life: 0.6, lifetime: 0.6, gravity: 0.15, slot: SLOT_SPARK });
                    }
                    self.cap_sparks();
                    for k in 1..=5 {
                        let at = ground_under(arena.world, feet + flat * (2.0 * k as f64) * vpm, vpm);
                        self.pending.push(Pending { delay: 0.05 * k as f64, act: Act::Spike(at, 0.9 + 0.25 * k as f64), profile: profile.clone(), damage: 0.0 });
                    }
                    self.cues.push(Cue::here(Sound::Crack, 1.0).pitched(0.6));
                    self.cues.push(Cue::here(Sound::HeavySwing, 0.9).pitched(0.6));
                    self.shake = self.shake.max(0.8);
                    let caught: Vec<usize> = (0..arena.animals.len())
                        .filter(|&i| {
                            let a = &arena.animals[i];
                            let to = a.position - feet;
                            a.health > 0.0 && !a.look.is_pet() && to.xy().norm() < (11.0 + a.look.body().1) * vpm && to.xy().try_normalize(1e-6).is_some_and(|d| d.dot(&flat.xy()) > 0.76)
                        })
                        .collect();
                    for i in caught {
                        let frozen = arena.animals[i].status.frozen > 0.0;
                        let center = arena.animals[i].center(vpm as u32);
                        if frozen {
                            arena.animals[i].status.frozen = 0.0;
                            self.sparks(center, 24, 5.0, frost, 0.8, 0.7);
                            self.say_at(center, "Shatter!", Element::Frost.color(), 22.0);
                            for other in arena.animals.iter_mut().filter(|o| o.health > 0.0 && !o.look.is_pet() && (o.center(vpm as u32) - center).norm() < 2.5 * vpm) {
                                other.status.chill += 0.5;
                                other.status.slowed = other.status.slowed.max(3.0);
                                other.status.slow = other.status.slow.max(0.45);
                            }
                        }
                        self.element_hit(center, Some(Element::Frost), 1.3);
                        self.strike(arena, &profile, i, Blow { damage: p.damage * if frozen { 2.5 } else { 1.0 }, crit: frozen, heavy: true, from: feet, depth: 0, element: Some(Element::Frost) });
                        if arena.animals[i].health > 0.0 {
                            let away = arena.animals[i].position - feet;
                            let s = &mut arena.animals[i].status;
                            s.push = Vector3::new(away.x, away.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x()) * 9.0 * vpm;
                            s.staggered = s.staggered.max(0.6);
                            s.chill += 0.5;
                            if !frozen && s.chill >= 1.0 {
                                s.chill = 0.0;
                                s.frozen = s.frozen.max(2.0);
                            }
                        }
                    }
                }
                // What's just in front of the charge skewered, struck the first time, and carried
                // along on ahead of it
                Act::Skewer(k) => {
                    let dir = self.charging;
                    let look = Element::look(profile.element);
                    // Air streaming past
                    for _ in 0..6 {
                        let side = Vector3::new(-dir.y, dir.x, 0.0) * (random(&mut self.rng) - 0.5) * 2.4 * vpm;
                        let at = feet + dir * 2.2 * vpm + side + Vector3::new(0.0, 0.0, (0.4 + 1.2 * random(&mut self.rng)) * vpm);
                        self.fine(at, -dir * 14.0 * vpm, look, 0.18, 0.0);
                    }
                    let caught: Vec<usize> = (0..arena.animals.len())
                        .filter(|&i| {
                            let a = &arena.animals[i];
                            let to = a.position - feet;
                            let along = to.dot(&dir) / vpm;
                            let across = (to - dir * to.dot(&dir)).xy().norm() / vpm;
                            let body = a.look.body().1;
                            a.health > 0.0 && !a.look.is_pet() && along > -0.5 && along < 2.6 + body && across < 1.2 + body
                        })
                        .collect();
                    for i in caught {
                        let id = arena.animals[i].id;
                        if !self.skewered.contains(&id) {
                            self.skewered.push(id);
                            let at = arena.animals[i].center(vpm as u32);
                            self.sparks(at, 10, 3.0, Element::look(None), 0.8, 0.35);
                            self.element_hit(at, profile.element, 1.2);
                            self.cues.push(Cue::at(Sound::Hit, at, 1.0).pitched(0.8 + 0.05 * k as f32));
                            self.strike(arena, &profile, i, Blow { damage: p.damage, crit: false, heavy: true, from: feet, depth: 0, element: profile.element });
                        }
                        if arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
                            let s = &mut arena.animals[i].status;
                            s.push = dir * 22.0 * vpm;
                            s.staggered = s.staggered.max(0.5);
                        }
                    }
                }
                // Everything skewered flung off to either side, hard
                Act::Fling => {
                    let dir = self.charging;
                    let side = Vector3::new(-dir.y, dir.x, 0.0);
                    let look = Element::look(profile.element);
                    let front = feet + dir * 1.5 * vpm + Vector3::new(0.0, 0.0, 1.0 * vpm);
                    for k in 0..24 {
                        let a = k as f64 / 24.0 * TAU;
                        let out = side * a.cos() + Vector3::new(0.0, 0.0, a.sin());
                        self.sparks.push(Spark { position: front, velocity: (out * 7.0 + dir * 3.0) * vpm, look, life: 0.4, lifetime: 0.4, gravity: 0.0, slot: SLOT_SPARK });
                    }
                    self.cap_sparks();
                    for (n, id) in std::mem::take(&mut self.skewered).into_iter().enumerate() {
                        let Some(i) = arena.animals.iter().position(|a| a.id == id && a.health > 0.0) else {
                            continue;
                        };
                        let at = arena.animals[i].center(vpm as u32);
                        self.element_hit(at, profile.element, 1.6);
                        self.strike(arena, &profile, i, Blow { damage: p.damage, crit: false, heavy: true, from: feet, depth: 0, element: profile.element });
                        if arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
                            let s = &mut arena.animals[i].status;
                            let way = if n % 2 == 0 { 1.0 } else { -1.0 };
                            s.push = (side * way * 10.0 + dir * 5.0) * vpm;
                            s.staggered = s.staggered.max(1.1);
                        }
                    }
                    self.cues.push(Cue::here(Sound::Slam, 0.9).pitched(1.1));
                    self.cues.push(Cue::here(Sound::HeavySwing, 0.9).pitched(0.7));
                    self.shake = self.shake.max(0.8);
                }
                // A ring cracked out round the player, everything it catches dragged in
                Act::Coil(k) => {
                    let r = (profile.stat("range").max(3.5) * [0.75, 1.05, 1.35][(k as usize).min(2)]).min(10.0);
                    let look = Element::look(profile.element);
                    for j in 0..40 {
                        let a = j as f64 / 40.0 * TAU + k as f64 * 0.4;
                        let at = feet + Vector3::new(a.cos() * r, a.sin() * r, 0.9 + 0.15 * (a * 3.0).sin()) * vpm;
                        if j % 2 == 0 {
                            self.sparks.push(Spark { position: at, velocity: Vector3::new(-a.sin(), a.cos(), 0.0) * 4.0 * vpm, look, life: 0.3, lifetime: 0.3, gravity: 0.0, slot: SLOT_SPARK });
                        } else {
                            self.fine(at, Vector3::new(-a.sin(), a.cos(), 0.0) * 4.0 * vpm, look, 0.3, 0.0);
                        }
                    }
                    self.cap_sparks();
                    self.cues.push(Cue::here(Sound::Crack, 1.0).pitched(0.9 + 0.12 * k as f32));
                    self.shake = self.shake.max(0.3);
                    for i in Combat::under(arena, feet, r, 3.0) {
                        let a = &arena.animals[i];
                        let to = feet - a.position;
                        let gap = (to.xy().norm() / vpm - a.look.body().1 - 1.4).max(0.0);
                        if gap <= 0.0 && k > 0 {
                            continue;
                        }
                        let at = a.center(vpm as u32);
                        self.element_hit(at, profile.element, 1.0);
                        self.strike(arena, &profile, i, Blow { damage: p.damage, crit: false, heavy: false, from: feet, depth: if k == 0 { 0 } else { 1 }, element: profile.element });
                        if arena.animals[i].health > 0.0 {
                            let boss = arena.animals[i].boss.is_some();
                            let s = &mut arena.animals[i].status;
                            if !boss {
                                s.push = Vector3::new(to.x, to.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros) * (gap * PUSH_DECAY).min(32.0) * vpm;
                            }
                            s.staggered = s.staggered.max(0.6);
                        }
                    }
                }
                // A crack at one of what's within reach in front, round them in turn
                Act::Flay(k) => {
                    let view = arena.view;
                    let forward = view.forward();
                    let reach = (profile.stat("range").max(3.0) + 3.0) * vpm;
                    let mut within: Vec<(f64, usize)> = (0..arena.animals.len())
                        .filter_map(|i| {
                            let a = &arena.animals[i];
                            let to = a.center(vpm as u32) - view.eye;
                            let d = to.norm();
                            (a.health > 0.0 && !a.look.is_pet() && d < reach + a.look.body().1 * vpm && to.dot(&forward) > 0.2 * d).then_some((to.dot(&forward) / d, i))
                        })
                        .collect();
                    within.sort_by(|a, b| b.0.total_cmp(&a.0));
                    within.truncate(4);
                    let hand = view.eye + view.rotation * Vector3::new(0.25, 0.5, -0.15) * vpm;
                    let look = Element::look(profile.element.or(Some(Element::Blood)));
                    self.cues.push(Cue::here(Sound::Crack, 0.9).pitched(1.05 + 0.06 * k as f32));
                    match within.get(k as usize % within.len().max(1)) {
                        Some(&(_, i)) => {
                            let at = arena.animals[i].center(vpm as u32);
                            self.lash_line(hand, at, look);
                            self.sparks(at, 6, 2.5, Element::look(Some(Element::Blood)), 0.9, 0.3);
                            self.strike(arena, &profile, i, Blow { damage: p.damage * (1.0 + 0.08 * k as f64), crit: k == 7, heavy: k == 7, from: view.eye, depth: 0, element: profile.element });
                            if arena.animals[i].health > 0.0 {
                                let s = &mut arena.animals[i].status;
                                s.bleed_stacks = (s.bleed_stacks + 1).min(5);
                                s.bleed = s.bleed.max(4.0);
                                s.bleed_dps = s.bleed_dps.max(p.damage * 0.15);
                                s.staggered = s.staggered.max(0.25);
                            }
                        }
                        None => self.lash_line(hand, view.eye + forward * reach, look),
                    }
                }
                // An arrow loosed straight off the string, a little off true
                Act::Quick(k) => {
                    let view = arena.view;
                    let jitter = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), (random(&mut self.rng) - 0.5) * 0.05) * UnitQuaternion::from_axis_angle(&(view.rotation * Vector3::x_axis()), (random(&mut self.rng) - 0.5) * 0.03);
                    let direction = jitter * view.forward();
                    let from = view.eye + view.rotation * Vector3::new(-0.1, 0.6, -0.08) * vpm;
                    let speed = profile.stat("projectileSpeed").max(40.0) * vpm;
                    let head = (random(&mut self.rng) * 3.0) as usize % 3;
                    self.launch(&profile, Some(Missile::arrow(head, true)), Form::Bolt, from, direction * speed, p.damage, ARROW_GRAVITY * 0.6);
                    self.cues.push(Cue::here(Sound::Twang, 0.7).pitched(1.0 + 0.03 * k as f32));
                }
                // A shot straight at it, sure to strike true: a line of light out to it
                Act::Reckon(id) => {
                    let Some(i) = arena.animals.iter().position(|a| a.id == id && a.health > 0.0) else {
                        continue;
                    };
                    let view = arena.view;
                    let center = arena.animals[i].center(vpm as u32);
                    let from = view.eye + view.rotation * Vector3::new(0.15, 0.75, -0.15) * vpm;
                    self.lash_line(from, center, Element::look(Some(Element::Holy)));
                    self.sparks(center, 10, 3.0, Element::look(None), 0.8, 0.3);
                    self.element_hit(center, profile.element, 1.4);
                    self.say_at(center + Vector3::new(0.0, 0.0, 0.6 * vpm), "Reckoned!", [255, 230, 140], 16.0);
                    self.strike(arena, &profile, i, Blow { damage: p.damage, crit: true, heavy: false, from: view.eye, depth: 0, element: profile.element });
                    let gun = matches!(profile.build.shot(), Shot::Ball | Shot::Shell);
                    if gun {
                        self.flare = FLARE;
                        self.flare_roll = random(&mut self.rng) * TAU;
                        for _ in 0..3 {
                            let v = (view.forward() * 2.0 + self.any_direction() * 0.6) * vpm;
                            self.sparks.push(Spark { position: from, velocity: v, look: Element::look(None), life: 0.8, lifetime: 0.8, gravity: -0.05, slot: SLOT_SMOKE });
                        }
                    }
                    self.cues.push(Cue::here(if gun { Sound::Gunshot } else { Sound::Release }, 0.85).pitched(1.1));
                    self.kick += 0.025;
                    self.shake = self.shake.max(0.2);
                }
                // A pillar of stone bursting up, throwing what's on it up and back
                Act::Pillar(at) => {
                    self.swells.push(Swell { model: STONE_PILLAR, position: at - Vector3::new(0.0, 0.0, 0.25 * vpm), radius: 1.0, life: 1.5, lifetime: 1.5 });
                    for _ in 0..10 {
                        let v = (self.any_direction() + Vector3::new(0.0, 0.0, 1.6)) * 3.5 * vpm;
                        self.drop(at, v, DROP_MUD, 1.0);
                    }
                    self.sparks(at + Vector3::new(0.0, 0.0, 0.3 * vpm), 6, 2.0, Element::look(profile.element), 0.8, 0.4);
                    self.cues.push(Cue::at(Sound::Slam, at, 0.8).pitched(1.15 + 0.1 * random(&mut self.rng) as f32));
                    self.shake = self.shake.max(0.35);
                    for i in Combat::under(arena, at, 1.5, 3.0) {
                        let center = arena.animals[i].center(vpm as u32);
                        self.element_hit(center, profile.element, 1.3);
                        self.strike(arena, &profile, i, Blow { damage: p.damage, crit: false, heavy: true, from: at, depth: 0, element: profile.element });
                        if arena.animals[i].health > 0.0 {
                            let away = arena.animals[i].position - feet;
                            let s = &mut arena.animals[i].status;
                            s.staggered = s.staggered.max(1.0);
                            s.push = Vector3::new(away.x, away.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x()) * 5.0 * vpm;
                        }
                    }
                }
                Act::Lasso { id, yank } => self.lasso(arena, &profile, id, yank, p.damage),
                Act::Bind(id) => self.bind(arena, &profile, id, p.damage),
                Act::Boom(k) => self.boom(arena, &profile, k, p.damage),
                Act::Serpents => self.loose_serpents(arena, &profile, p.damage),
                Act::Trip(k) => self.trip(arena, &profile, k, p.damage),
                Act::Brand { from, to, k } => self.brand(arena, &profile, from, to, k, p.damage),
                Act::Swing { land } => self.chain_swing(arena, &profile, land, p.damage),
            }
        }
        self.scarless = false;
    }

    /// A lasso's loop dropping round animal `id`, holding it a moment, or it yanked in to the
    /// player's feet, reeling: or, too big to be moved, the player hauled out to it
    fn lasso(&mut self, arena: &mut Arena, profile: &Rc<Profile>, id: u32, yank: bool, damage: f64) {
        let view = arena.view;
        let vpm = view.vpm;
        let feet = arena.player.unwrap_or(view.eye - Vector3::new(0.0, 0.0, 1.6 * vpm));
        let Some(i) = arena.animals.iter().position(|a| a.id == id && a.health > 0.0) else {
            self.lash_aim = None;
            return;
        };
        let center = arena.animals[i].center(vpm as u32);
        let body = arena.animals[i].look.body().1;
        self.lash_aim = Some(center);
        let look = Element::look(profile.element.or(Some(Element::Holy)));
        if !yank {
            // A loop of light dropping round it, drawing in
            for k in 0..18 {
                let a = k as f64 / 18.0 * TAU;
                let out = Vector3::new(a.cos(), a.sin(), 0.0) * (body + 0.5) * vpm;
                self.fine(center + out + Vector3::new(0.0, 0.0, 0.4 * vpm), -out * 2.0 - Vector3::new(0.0, 0.0, 1.5 * vpm), look, 0.25, 0.0);
            }
            arena.animals[i].status.rooted = arena.animals[i].status.rooted.max(0.35);
            self.strike(arena, profile, i, Blow { damage, crit: false, heavy: false, from: view.eye, depth: 1, element: profile.element });
            self.say_at(center, "Lassoed!", [235, 205, 150], 18.0);
            self.cues.push(Cue::at(Sound::Taut, center, 0.9).pitched(1.2));
            return;
        }
        let a = &mut arena.animals[i];
        let mut toward = feet - a.position;
        toward.z = 0.0;
        let gap = (toward.norm() / vpm - body - 1.6).max(0.0);
        let boss = a.boss.is_some() || body > 1.6;
        a.status.staggered = a.status.staggered.max(1.2);
        if let Some(toward) = toward.try_normalize(1e-6) {
            if boss {
                // Too big to be moved: the player's hauled out to it instead
                self.dash -= toward * (gap / 0.31).min(22.0) * vpm;
                self.dash.z += 2.0 * vpm;
            } else {
                a.status.push = toward * (gap * PUSH_DECAY).min(52.0) * vpm;
            }
        }
        self.strike(arena, profile, i, Blow { damage, crit: false, heavy: true, from: view.eye, depth: 0, element: profile.element });
        self.lash_line(center, feet + Vector3::new(0.0, 0.0, 1.1 * vpm), look);
        self.sparks(center, 8, 2.5, Element::look(None), 0.8, 0.3);
        self.cues.push(Cue::at(Sound::Crack, center, 1.0).pitched(1.1));
        self.cues.push(Cue::here(Sound::Chain, 0.6).pitched(0.9));
        self.shake = self.shake.max(0.45);
        self.lash_aim = None;
    }

    /// A coil bind's lash drawn tight round animal `id` and what's right beside it, holding them
    /// fast, see writhe()
    fn bind(&mut self, arena: &mut Arena, profile: &Rc<Profile>, id: u32, damage: f64) {
        let vpm = arena.view.vpm;
        let Some(first) = arena.animals.iter().position(|a| a.id == id && a.health > 0.0) else {
            self.lash_aim = None;
            return;
        };
        let at = arena.animals[first].position;
        let mut caught: Vec<(f64, usize)> = Combat::under(arena, at, 2.2, 3.0).into_iter().map(|i| ((arena.animals[i].position - at).norm(), i)).collect();
        caught.sort_by(|a, b| a.0.total_cmp(&b.0));
        caught.truncate(3);
        for &(_, i) in &caught {
            let a = &mut arena.animals[i];
            a.status.rooted = a.status.rooted.max(BIND_SECONDS);
            a.status.shown = 4.0;
            let center = a.center(vpm as u32);
            let girth = a.look.body().1;
            let id = a.id;
            self.binds.retain(|b| b.id != id);
            self.binds.push(Bind { id, life: BIND_SECONDS, tick: BIND_SQUEEZE, profile: profile.clone(), damage, at: center, girth });
            self.say_at(center, "Bound!", [220, 190, 140], 18.0);
        }
        if self.binds.len() > 6 {
            let extra = self.binds.len() - 6;
            self.binds.drain(..extra);
        }
        let center = arena.animals[first].center(vpm as u32);
        self.sparks(center, 10, 2.0, Element::look(profile.element), 0.4, 0.3);
        self.cues.push(Cue::at(Sound::Squeeze, center, 1.0));
        self.cues.push(Cue::at(Sound::Taut, center, 0.6).pitched(0.8));
        self.lash_aim = None;
    }

    /// The `k`th ring of a thunder crack's cone of sound rolling out ahead, everything in its
    /// stretch of the cone hit once, staggered and thrown back
    fn boom(&mut self, arena: &mut Arena, profile: &Rc<Profile>, k: u32, damage: f64) {
        let view = arena.view;
        let vpm = view.vpm;
        let feet = arena.player.unwrap_or(view.eye - Vector3::new(0.0, 0.0, 1.6 * vpm));
        let forward = view.forward();
        let flat = Vector3::new(forward.x, forward.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let (near, far) = (k as f64 * THUNDER_REACH / 4.0, (k + 1) as f64 * THUNDER_REACH / 4.0);
        if k == 0 {
            self.flashes.push(Flash { position: view.eye + flat * 2.0 * vpm, color: profile.element.map_or([1.0, 0.97, 0.9], Element::light), reach: 12.0, life: 0.25, lifetime: 0.25 });
            self.cues.push(Cue::here(Sound::Boom, 1.0));
            self.cues.push(Cue::here(Sound::Crack, 1.0).pitched(0.75));
            self.shake = self.shake.max(1.0);
        }
        // An arc of streaks across the cone, rushing on out, two rows of them, fewer the further
        // it's gone
        let look = Element::look(profile.element.or(Some(Element::Storm)));
        let n = 8 - k as usize;
        for row in [0.75, 1.45] {
            for j in 0..n {
                let a = (j as f64 / (n - 1) as f64 - 0.5) * 2.0 * THUNDER_CONE;
                let out = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), a) * flat;
                let at = feet + out * far.max(1.5) * vpm + Vector3::new(0.0, 0.0, (row + 0.2 * (a * 5.0).cos()) * vpm);
                let life = 0.24;
                self.sparks.push(Spark { position: at, velocity: out * 16.0 * vpm, look, life, lifetime: life, gravity: 0.0, slot: SLOT_BOLT });
            }
        }
        self.cap_sparks();
        let caught: Vec<usize> = (0..arena.animals.len())
            .filter(|&i| {
                let a = &arena.animals[i];
                let to = (a.position - feet).xy();
                let d = to.norm() / vpm - a.look.body().1;
                a.health > 0.0 && !a.look.is_pet() && (k == 0 || d >= near) && d < far && to.try_normalize(1e-6).is_some_and(|v| v.dot(&flat.xy()) > THUNDER_CONE.cos())
            })
            .collect();
        for i in caught {
            let at = arena.animals[i].center(vpm as u32);
            self.element_hit(at, profile.element, 1.4);
            self.strike(arena, profile, i, Blow { damage: damage * (1.0 - 0.15 * k as f64), crit: false, heavy: true, from: feet, depth: 0, element: profile.element });
            if arena.animals[i].health > 0.0 {
                let boss = arena.animals[i].boss.is_some();
                let away = arena.animals[i].position - feet;
                let s = &mut arena.animals[i].status;
                s.staggered = s.staggered.max(1.5);
                if !boss {
                    s.push = Vector3::new(away.x, away.y, 0.0).try_normalize(1e-6).unwrap_or(flat) * (12.0 - 2.0 * k as f64) * vpm;
                }
            }
        }
    }

    /// A serpent lash splitting into its strands where it struck the ground, each slithering off
    /// after one of the nearest, or fanning out ahead if there's nothing to go after
    fn loose_serpents(&mut self, arena: &mut Arena, profile: &Rc<Profile>, damage: f64) {
        let view = arena.view;
        let vpm = view.vpm;
        let feet = arena.player.unwrap_or(view.eye - Vector3::new(0.0, 0.0, 1.6 * vpm));
        let flat = Vector3::new(view.forward().x, view.forward().y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let from = self.lash_aim.take().unwrap_or(feet + flat * 4.0 * vpm);
        let mut near: Vec<(f64, u32)> = arena
            .animals
            .iter()
            .filter(|a| a.health > 0.0 && !a.look.is_pet())
            .map(|a| ((a.position - from).norm(), a.id))
            .filter(|&(d, _)| d < 16.0 * vpm)
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        let venom = Rc::new(Profile { element: profile.element.or(Some(Element::Poison)), ..(**profile).clone() });
        for k in 0..SERPENTS {
            let target = near.get(k).map(|&(_, id)| id);
            let fan = (k as f64 / (SERPENTS - 1) as f64 - 0.5) * 1.6;
            let heading = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), fan) * flat;
            self.serpents.push(Serpent { position: from, heading, target, life: SERPENT_SECONDS, phase: k as f64 * 1.3, trail: 0.0, profile: venom.clone(), damage });
        }
        self.sparks(from, 12, 2.5, Element::look(venom.element), 0.6, 0.35);
        for _ in 0..6 {
            let v = (self.any_direction() + Vector3::new(0.0, 0.0, 1.4)) * 2.5 * vpm;
            self.drop(from, v, DROP_MUD, 0.8);
        }
        self.cues.push(Cue::at(Sound::Slither, from, 1.0));
        self.cues.push(Cue::at(Sound::Crack, from, 0.8).pitched(1.3));
    }

    /// The `k`th eighth of a trip sweep going round low over the ground: everything within reach
    /// that way taken off its feet
    fn trip(&mut self, arena: &mut Arena, profile: &Rc<Profile>, k: u32, damage: f64) {
        let view = arena.view;
        let vpm = view.vpm;
        let feet = arena.player.unwrap_or(view.eye - Vector3::new(0.0, 0.0, 1.6 * vpm));
        let flat = Vector3::new(view.forward().x, view.forward().y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let right = flat.cross(&Vector3::z());
        let reach = profile.stat("range").max(3.5) + 0.5;
        let (from, to) = (FRAC_PI_2 + k as f64 * TAU / 8.0, FRAC_PI_2 + (k + 1) as f64 * TAU / 8.0);
        let way = |a: f64| right * a.cos() + flat * a.sin();
        // Sparks streaking round low along the arc, dust kicked up
        let look = Element::look(profile.element);
        for j in 0..4 {
            let a = from + (to - from) * j as f64 / 3.0;
            let out = way(a);
            let at = feet + out * reach * 0.85 * vpm + Vector3::new(0.0, 0.0, 0.25 * vpm);
            let along = way(a + FRAC_PI_2);
            self.sparks.push(Spark { position: at, velocity: along * 9.0 * vpm, look, life: 0.22, lifetime: 0.22, gravity: 0.0, slot: SLOT_BOLT });
        }
        let dust = feet + way((from + to) * 0.5) * reach * 0.8 * vpm;
        self.drop(dust, (way((from + to) * 0.5) * 1.5 + Vector3::new(0.0, 0.0, 2.5)) * vpm, DROP_MUD, 0.7);
        self.cap_sparks();
        let caught: Vec<usize> = Combat::under(arena, feet, reach, 2.5)
            .into_iter()
            .filter(|&i| {
                // Which way round it is, measured as the sweep goes, from the right past straight
                // ahead, and whether that's in this eighth
                let to_it = arena.animals[i].position - feet;
                let a = to_it.dot(&flat).atan2(to_it.dot(&right));
                (a - from).rem_euclid(TAU) < to - from || to_it.xy().norm() < 1e-6
            })
            .collect();
        for i in caught {
            let at = arena.animals[i].center(vpm as u32);
            self.element_hit(at, profile.element, 1.0);
            self.strike(arena, profile, i, Blow { damage, crit: false, heavy: true, from: feet, depth: 0, element: profile.element });
            if arena.animals[i].health > 0.0 {
                let s = &mut arena.animals[i].status;
                s.staggered = s.staggered.max(1.6);
                s.push = Vector3::zeros();
                self.say_at(at, "Tripped!", [235, 215, 170], 16.0);
                self.cues.push(Cue::at(Sound::Hit, at, 0.7).pitched(0.8));
            }
        }
    }

    /// The `k`th stretch of a burning brand's line catching, from `from` to `to`: what's on it
    /// branded, burning hard. The line itself burns on as a zone, see brand_burn().
    fn brand(&mut self, arena: &mut Arena, profile: &Rc<Profile>, from: Vector3<f64>, to: Vector3<f64>, k: u32, damage: f64) {
        let vpm = arena.view.vpm;
        let middle = from.lerp(&to, 0.5);
        let fire = Element::look(Some(Element::Fire));
        for j in 0..3 {
            let at = from.lerp(&to, j as f64 / 2.0) + Vector3::new(0.0, 0.0, 0.2 * vpm);
            self.sparks.push(Spark { position: at, velocity: Vector3::new(0.0, 0.0, 2.5 * vpm), look: fire, life: 0.35, lifetime: 0.35, gravity: -0.1, slot: SLOT_SPARK });
        }
        if k == 0 {
            self.cues.push(Cue::at(Sound::Crack, middle, 1.0).pitched(1.15));
            self.shake = self.shake.max(0.35);
        }
        if k % 2 == 0 {
            self.cues.push(Cue::at(Sound::Sear, middle, 0.6));
        }
        self.flashes.push(Flash { position: middle + Vector3::new(0.0, 0.0, 0.4 * vpm), color: Element::Fire.light(), reach: 3.5, life: 0.2, lifetime: 0.2 });
        self.cap_sparks();
        for i in Combat::under(arena, middle, 1.4 + (to - from).norm() / vpm * 0.5, 2.5) {
            let at = arena.animals[i].center(vpm as u32);
            self.strike(arena, profile, i, Blow { damage, crit: false, heavy: false, from: middle, depth: 0, element: Some(Element::Fire) });
            if arena.animals[i].health > 0.0 {
                let s = &mut arena.animals[i].status;
                s.burn = s.burn.max(6.0);
                s.burn_dps = s.burn_dps.max(damage * 0.35);
                if s.flash < 0.2 {
                    self.say_at(at, "Branded!", Element::Fire.color(), 17.0);
                }
            }
        }
    }

    /// A chain swing: the player swung off on ahead from where the lash caught, or come down,
    /// a crack all round throwing back what's near
    fn chain_swing(&mut self, arena: &mut Arena, profile: &Rc<Profile>, land: bool, damage: f64) {
        let view = arena.view;
        let vpm = view.vpm;
        let feet = arena.player.unwrap_or(view.eye - Vector3::new(0.0, 0.0, 1.6 * vpm));
        if !land {
            let anchor = self.lash_aim.unwrap_or(view.eye + view.forward() * 8.0 * vpm);
            let to = anchor - feet;
            let flat = Vector3::new(to.x, to.y, 0.0);
            // On past where it caught a little way, a short swing, and up off the ground: a hop
            // goes about a third of a meter for each meter a second, see Flurry
            let go = (flat.norm() / vpm + 1.5).clamp(3.0, 7.5);
            let ahead = flat.try_normalize(1e-6).unwrap_or_else(|| Vector3::new(view.forward().x, view.forward().y, 0.0).normalize());
            self.dash += ahead * (go / 0.31).min(36.0) * vpm + Vector3::new(0.0, 0.0, 7.0 * vpm);
            self.cues.push(Cue::here(Sound::Swoop, 1.0));
            self.cues.push(Cue::here(Sound::Taut, 0.7).pitched(1.1));
            self.sparks(anchor, 8, 2.0, Element::look(None), 0.8, 0.3);
            return;
        }
        // Let go of, and cracked all round on landing
        self.lash_aim = None;
        let look = Element::look(profile.element);
        for k in 0..28 {
            let a = k as f64 / 28.0 * TAU;
            let out = Vector3::new(a.cos(), a.sin(), 0.0);
            self.sparks.push(Spark { position: feet + out * 1.0 * vpm + Vector3::new(0.0, 0.0, 0.5 * vpm), velocity: out * 9.0 * vpm, look, life: 0.3, lifetime: 0.3, gravity: 0.0, slot: SLOT_SPARK });
        }
        self.cap_sparks();
        self.cues.push(Cue::here(Sound::Crack, 1.0).pitched(1.0));
        self.cues.push(Cue::here(Sound::Slam, 0.6).pitched(1.3));
        self.shake = self.shake.max(0.6);
        for i in Combat::under(arena, feet, 3.5, 3.0) {
            let at = arena.animals[i].center(vpm as u32);
            self.element_hit(at, profile.element, 1.3);
            self.strike(arena, profile, i, Blow { damage, crit: false, heavy: true, from: feet, depth: 0, element: profile.element });
            if arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
                let away = arena.animals[i].position - feet;
                let s = &mut arena.animals[i].status;
                s.staggered = s.staggered.max(0.9);
                s.push = Vector3::new(away.x, away.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x()) * 8.0 * vpm;
            }
        }
    }

    /// What coil binds hold, held and crushed, and serpent lash strands slithering on, biting what
    /// they reach
    fn writhe(&mut self, arena: &mut Arena, dt: f64) {
        let vpm = arena.view.vpm;
        for mut b in std::mem::take(&mut self.binds) {
            b.life -= dt;
            b.tick -= dt;
            let Some(i) = arena.animals.iter().position(|a| a.id == b.id && a.health > 0.0) else {
                continue;
            };
            b.at = arena.animals[i].center(vpm as u32);
            arena.animals[i].status.rooted = arena.animals[i].status.rooted.max(b.life.min(0.2));
            if b.tick <= 0.0 {
                b.tick += BIND_SQUEEZE;
                self.strike(arena, &b.profile, i, Blow { damage: b.damage, crit: false, heavy: false, from: b.at, depth: 1, element: b.profile.element });
                self.fine(b.at + Vector3::new(b.girth * vpm, 0.0, 0.0), Vector3::new(0.0, 0.0, 0.8 * vpm), Element::look(b.profile.element), 0.25, 0.0);
                self.cues.push(Cue::at(Sound::Squeeze, b.at, 0.35).pitched(1.0 + 0.1 * random(&mut self.rng) as f32));
            }
            if b.life > 0.0 {
                self.binds.push(b);
            }
        }
        for mut s in std::mem::take(&mut self.serpents) {
            s.life -= dt;
            s.phase += dt * 13.0;
            // Turning towards what it's after, if that's still about
            let goal = s.target.and_then(|id| arena.animals.iter().position(|a| a.id == id && a.health > 0.0));
            if let Some(i) = goal {
                let to = arena.animals[i].position - s.position;
                if let Some(way) = Vector3::new(to.x, to.y, 0.0).try_normalize(1e-6) {
                    s.heading = s.heading.lerp(&way, (5.0 * dt).min(1.0)).try_normalize(1e-6).unwrap_or(way);
                }
                // Reached it: a bite, venom in the wound, and the strand's gone
                if to.xy().norm() < (arena.animals[i].look.body().1 + 0.6) * vpm {
                    let at = arena.animals[i].center(vpm as u32);
                    self.strike(arena, &s.profile, i, Blow { damage: s.damage, crit: false, heavy: false, from: s.position, depth: 0, element: s.profile.element });
                    if arena.animals[i].health > 0.0 {
                        let st = &mut arena.animals[i].status;
                        st.poison_stacks = (st.poison_stacks + 2).min(6);
                        st.poison = st.poison.max(5.0);
                        st.poison_dps = st.poison_dps.max(s.damage * 0.15);
                        st.staggered = st.staggered.max(0.4);
                    }
                    self.sparks(at, 8, 2.5, Element::look(s.profile.element), 0.6, 0.3);
                    self.cues.push(Cue::at(Sound::Bite, at, 0.6).pitched(1.3));
                    continue;
                }
            }
            s.position += s.heading * SERPENT_SPEED * vpm * dt;
            // Along the ground, over what's in the way
            s.position = ground_under(arena.world, s.position + Vector3::new(0.0, 0.0, 0.8 * vpm), vpm) + Vector3::new(0.0, 0.0, 0.15 * vpm);
            s.trail -= dt;
            if s.trail <= 0.0 {
                s.trail = 0.05;
                let at = Combat::serpent_at(&s, vpm);
                self.sparks.push(Spark { position: at, velocity: Vector3::zeros(), look: Element::look(s.profile.element), life: 0.3, lifetime: 0.3, gravity: 0.0, slot: SLOT_SPARK });
            }
            if s.life > 0.0 {
                self.serpents.push(s);
            } else {
                self.sparks(s.position, 4, 1.0, Element::look(s.profile.element), 0.3, 0.25);
            }
        }
        if !self.serpents.is_empty() {
            self.cap_sparks();
        }
    }

    /// Where a serpent lash's strand is drawn: weaving from side to side of its way
    fn serpent_at(s: &Serpent, vpm: f64) -> Vector3<f64> {
        let side = Vector3::new(-s.heading.y, s.heading.x, 0.0);
        s.position + side * s.phase.sin() * 0.3 * vpm
    }

    /// A streak of sparks from `from` to `to`, there a moment, like a fuse burning along
    fn streak(&mut self, from: Vector3<f64>, to: Vector3<f64>, look: usize) {
        let vpm = self.vpm;
        let steps = ((to - from).norm() / (0.5 * vpm)).clamp(3.0, 16.0) as usize;
        for k in 0..=steps {
            let at = from.lerp(&to, k as f64 / steps as f64) + Vector3::new(0.0, 0.0, (k as f64 / steps as f64 * PI).sin() * 0.6 * vpm);
            self.sparks.push(Spark { position: at, velocity: Vector3::zeros(), look, life: 0.22, lifetime: 0.22, gravity: 0.0, slot: SLOT_SPARK });
        }
        self.cap_sparks();
    }

    /// The id of the living animal nearest `at`, within `within` meters, that isn't one of `not`
    fn nearest_other(&self, arena: &Arena, at: Vector3<f64>, within: f64, not: &[u32]) -> Option<u32> {
        let vpm = arena.view.vpm;
        arena
            .animals
            .iter()
            .filter(|a| a.health > 0.0 && !a.look.is_pet() && !not.contains(&a.id))
            .map(|a| ((a.center(vpm as u32) - at).norm(), a.id))
            .filter(|&(d, _)| d < within * vpm)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id)
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

    /// A blow struck in hand landing, a whip's or any other weapon's. What flies off it goes off
    /// right in front of the eye, so it's kept small.
    fn melee(&mut self, arena: &mut Arena, profile: &Rc<Profile>, heavy: bool, power: f64, finisher: bool) {
        self.close = true;
        if profile.style == Style::Lash {
            self.lash(arena, profile, heavy, power, finisher);
        } else {
            self.blade(arena, profile, heavy, power, finisher);
        }
        self.close = false;
    }

    /// A sword swing landing: hits what's in front within reach. Echoes hit for `power` of a swing.
    /// `finisher` is the last blow of a combo.
    fn blade(&mut self, arena: &mut Arena, profile: &Rc<Profile>, heavy: bool, power: f64, finisher: bool) {
        let view = arena.view;
        let vpm = view.vpm;
        // A lunge, as a heavy blow or a finisher, reaches further on its long step
        let lunge = (heavy && profile.heavy == Heavy::Lunge) || (finisher && profile.finish == Finish::Lunge);
        let reach = (profile.stat("range") + if heavy || finisher { 0.3 } else { 0.0 } + if lunge { 0.6 } else { 0.0 }) * vpm;
        // Heavy blows come down on less, but cleave through one more. A spear's finishing sweep
        // takes in a wide arc, and a finisher turned right round everything round about.
        let sweep = finisher && profile.style == Style::Thrust && profile.finish == Finish::Style;
        let spun = finisher && profile.finish == Finish::Spin;
        // A flail whirled round hits all round, but brought down overhead only what's in front
        let cone = if spun {
            -1.0
        } else if lunge {
            0.85
        } else if sweep {
            profile.cone - 0.45
        } else if heavy && profile.style == Style::Whirl {
            0.5
        } else {
            (profile.cone + if heavy { 0.15 } else { 0.0 }).min(0.95)
        };
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
            Style::Whirl => 0.9,
            Style::Quake => 0.6,
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
            // A finisher brought down hard or swept up from below knocks what it hits off its feet
            if finisher && matches!(profile.finish, Finish::Slam | Finish::Rising) && arena.animals[i].health > 0.0 {
                let s = &mut arena.animals[i].status;
                s.staggered = s.staggered.max(0.6);
            }
        }
        let feet = arena.player.unwrap_or(view.eye - Vector3::new(0.0, 0.0, 1.6 * vpm));
        // A flail whirled round leaves a ring of its trail all round
        if profile.style == Style::Whirl && !heavy && power >= 1.0 {
            let look = Element::look(profile.element);
            let r = profile.stat("range").max(1.5) * 0.8;
            for k in 0..30 {
                let a = k as f64 / 30.0 * TAU;
                let at = feet + Vector3::new(a.cos() * r * vpm, a.sin() * r * vpm, 1.1 * vpm);
                self.fine(at, Vector3::new(-a.sin(), a.cos(), 0.0) * 3.0 * vpm, look, 0.25, 0.0);
            }
        }
        // Cleaving on into what's next to the first thing it hit
        if power >= 1.0
            && profile.has(Effect::Cleave)
            && let Some(&(_, first)) = targets.first()
        {
            let center = arena.animals[first].center(vpm as u32);
            let r = profile.param(Effect::Cleave, "radius", 2.0) * profile.stat("area").max(1.0);
            let share = profile.param(Effect::Cleave, "power", 0.5) * profile.potency(Effect::Cleave).max(1.0);
            let others: Vec<usize> = (0..arena.animals.len())
                .filter(|&j| !targets.iter().any(|&(_, t)| t == j) && arena.animals[j].health > 0.0 && !arena.animals[j].look.is_pet())
                .filter(|&j| (arena.animals[j].center(vpm as u32) - center).norm() < (r + arena.animals[j].look.body().1) * vpm)
                .collect();
            let look = Element::look(profile.element);
            for j in others {
                let there = arena.animals[j].center(vpm as u32);
                for k in 1..8 {
                    self.fine(center.lerp(&there, k as f64 / 8.0), Vector3::zeros(), look, 0.2, 0.0);
                }
                self.sparks(there, 5, 2.5, Element::look(None), 0.8, 0.3);
                self.strike(arena, profile, j, Blow { damage: base * share, crit: false, heavy, from: center, depth: 1, element: profile.element });
            }
        }
        // Heavy blows drag what's round in, and leave a pool where they land
        if power >= 1.0
            && let Some(&(_, first)) = targets.first()
        {
            let at = arena.animals[first].center(vpm as u32);
            if (heavy || finisher) && profile.has(Effect::Vortex) {
                self.vortex(arena, profile, at);
            }
            if profile.has(Effect::Pool) {
                self.leave_pool(arena, profile, at, base);
            }
        }
        // The blade stops dead a moment as it bites, jolts back, and a big one shakes the view: a
        // heavy weapon's longer and harder, a light one's hardly at all
        if !targets.is_empty() && power >= 1.0 {
            let big = heavy || finisher || any_crit;
            let w = weight(profile);
            let weighty = 1.0 + 0.6 * w.max(0.0) + 0.25 * w.min(0.0);
            self.hitstop = if big { 0.085 } else { 0.045 } * weighty;
            self.recoil = 1.0;
            self.shake = self.shake.max(if big { 0.6 } else { 0.25 } * (1.0 + 0.35 * w.max(0.0)));
        }
        let flat = Vector3::new(forward.x, forward.y, 0.0);
        // A maul's every blow, a flail brought down overhead, and with the shockwave effect a
        // heavy blow or a finisher, send a ring rolling out along the ground from where they
        // come down, passing over what the blow itself hit
        let quake = profile.style == Style::Quake;
        let slam = heavy && profile.style == Style::Whirl;
        let shockwave = (heavy || finisher) && profile.has(Effect::Shockwave);
        if power >= 1.0 && (quake || slam || shockwave) && flat.norm() > 0.2 {
            let ahead = view.eye + flat.normalize() * reach * 0.6;
            let down = Vector3::new(0.0, 0.0, -2.8 * vpm);
            if let Some(t) = solid_along(arena.world, ahead, ahead + down) {
                let at = ahead + down * t;
                let area = profile.stat("area").max(0.7) * (1.0 + 0.3 * profile.potency(Effect::Shockwave)).sqrt();
                let (reach, share) = if quake {
                    (if heavy { 5.0 } else if finisher { 4.0 } else { 2.8 }, if heavy { 0.8 } else { 0.6 })
                } else if slam {
                    (3.2, 0.6)
                } else {
                    (profile.param(Effect::Shockwave, "radius", 4.0), profile.param(Effect::Shockwave, "power", 0.5))
                };
                let hit: Vec<u32> = targets.iter().map(|&(_, i)| arena.animals[i].id).collect();
                self.ripple(profile, at, reach * area, base * share, hit, true, quake && (heavy || finisher));
                let big = heavy || finisher;
                self.cues.push(Cue::at(Sound::Slam, at, if big { 1.0 } else { 0.75 }).pitched(if big { 0.8 } else { 1.0 }));
                if quake && heavy {
                    self.cues.push(Cue::at(Sound::Quake, at, 0.7).pitched(1.2));
                    self.craters.push((at, 0.35));
                }
                self.sparks(at, 12, 2.5, Element::look(profile.element), 1.0, 0.4);
                for _ in 0..10 {
                    let v = (self.any_direction() + Vector3::new(0.0, 0.0, 1.5)) * 3.0 * vpm;
                    self.drop(at, v, DROP_MUD, 0.9);
                }
                self.shake = self.shake.max(if big { 0.9 } else { 0.55 });
            }
        }
        // A heavy hammer blow dents the ground in front, knocking back what's around it, and a
        // finishing one a little
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
        // Where the tip of the lash is as it cracks
        let tip = self.lash.as_ref().map_or_else(|| view.eye + view.rotation * Vector3::new(0.12, 1.5, -0.15) * vpm, Lash::tip);
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
        // A swing made longer, coiled back by a heavy whip or twirled by a rare one, cracks as
        // much harder, a light one's quicker flick as much softer, so a whip does as much a
        // second whatever it's made of. Not an echo or a second crack.
        let w = weight(profile);
        let longer = if power >= 1.0 { lash_swing(heavy, if finisher { FINISHER } else { 0 }, grade, w).stretch } else { 1.0 };
        let base = (profile.stat("physicalDamage") + profile.stat("elementalDamage")) * if heavy { HEAVY } else { 1.0 } * power * spreading * longer;
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

        // The crack, at the tip of the lash: sparks and a flash, then a streak on to where it
        // landed if that's further than the tip got, sweeping a fan, or going right round
        for _ in 0..if circle { 20 } else { 8 } {
            let v = self.any_direction() * (1.0 + random(&mut self.rng)) * vpm;
            self.fine(tip, v, look, 0.16, 0.3);
        }
        // Out at the end of the lash it's not right in front of the eye, so they're full size. A
        // heavy whip's crack is bigger and deeper, a light one's a sharp little snap.
        let close = std::mem::replace(&mut self.close, (tip - view.eye).norm() < 1.5 * vpm);
        self.sparks(tip, (5.0 + 3.0 * w.max(0.0)) as usize, 2.5 + 1.0 * w.max(0.0), look, 0.4, 0.2);
        self.close = close;
        self.flashes.push(Flash { position: tip, color: profile.element.map_or([1.0, 0.92, 0.75], Element::light), reach: 2.5 + 1.2 * w.max(0.0), life: 0.07, lifetime: 0.07 });
        let short = |to: Vector3<f64>| (to - tip).norm() > 0.75 * vpm;
        let loud = if heavy || finisher { 0.9 } else { 0.7 } * (1.0 + 0.15 * w.max(0.0));
        self.cues.push(Cue::at(Sound::Crack, tip, loud as f32).pitched(1.55 - 0.28 * w as f32 + 0.2 * random(&mut self.rng) as f32));
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
            if short(stretch) {
                self.lash_line(tip, stretch, look);
            }
        } else if short(end) {
            self.lash_line(tip, end, look);
        }
        self.cap_sparks();

        // A heavy whip's crack bites longer and shakes the view more, as a heavy blade's does
        if !hits.is_empty() && power >= 1.0 {
            let big = heavy || finisher || any_crit;
            let weighty = 1.0 + 0.6 * w.max(0.0) + 0.25 * w.min(0.0);
            self.hitstop = if big { 0.085 } else { 0.045 } * weighty;
            self.recoil = 1.0;
            self.shake = self.shake.max(if big { 0.5 } else { 0.2 } * (1.0 + 0.35 * w.max(0.0)));
        }
        // A double crack: flicked up and cracked again as the swing comes round, when lash_swing()
        // has it land. The swing all but stands still while it's in slow motion, so that's added on.
        if finisher && power >= 1.0 && grade >= LASH_DOUBLE {
            let length = if let Action::Swing { length, .. } = self.action { length } else { 0.6 };
            let swing = lash_swing(false, FINISHER, grade, w);
            let delay = (swing.second.unwrap_or(LASH_SECOND) - swing.lands) * length + self.hitstop.max(0.0) * (1.0 - HITSTOP_PACE);
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

    /// Lets an arrow fly from the bow in hand, drawn `draw` of the way, one of special ammo
    /// `loaded` if there is any
    fn loose(&mut self, arena: &mut Arena, weapon: &Weapon, loaded: Option<&Loaded>, draw: f64) {
        let profile = loaded.map_or_else(|| weapon.profile.clone(), |l| l.profile.clone());
        let vpm = arena.view.vpm;
        let from = self.held_point(weapon, arena.view, true);
        let target = self.aim(arena);
        // Drawn all the way, it flies a little harder, streaking
        let power = draw >= 0.98;
        let speed = profile.stat("projectileSpeed") * (0.45 + 0.55 * draw) * if power { 1.1 } else { 1.0 } * vpm;
        let damage = (profile.stat("physicalDamage") + profile.stat("elementalDamage")) * (0.3 + 0.7 * draw * draw) * if power { 1.1 } else { 1.0 };
        let head = loaded.and_then(|l| l.head).unwrap_or_else(|| weapon.arrowhead());
        self.shoot(arena.view, &profile, Some(Missile::arrow(head, power)), Form::Bolt, from, (target - from).normalize(), speed, damage);
        if let Some(l) = loaded {
            self.spent.push(l.ammo);
        }
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
        match profile.style {
            // A familiar conjured a little way ahead, each beside the last
            Style::Summon => {
                let n = self.familiars.iter().filter(|f| !f.phantom).count() % FAMILIARS;
                let (x, z) = [(-0.9, 0.4), (0.9, 0.4), (0.0, 1.0)][n];
                let at = arena.view.eye + arena.view.rotation * Vector3::new(x, 2.2, z) * vpm;
                self.conjure(&profile, false, at);
                return;
            }
            // A hex drifting off after what's near
            Style::Hex => {
                self.shoot(arena.view, &profile, None, Form::Hex, from, (target - from).normalize(), speed, damage);
                self.sparks(from, 6, 0.8, Element::look(Some(Element::Void)), -0.1, 0.5);
                self.cues.push(Cue::here(Sound::Hex, 0.5));
                return;
            }
            // Lightning leaping from the rod's prongs through the pack
            Style::Arc => {
                for _ in 0..8 {
                    let v = self.any_direction() * 1.2 * vpm;
                    self.fine(from, v, Element::look(profile.element.or(Some(Element::Storm))), 0.2, 0.0);
                }
                self.zap(arena, &profile, from, target, damage);
                self.kick_speed += 0.6;
                return;
            }
            _ => {}
        }
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

    /// Arrows, bolts, shot or spells leaving `from` towards `direction`: more than one with
    /// multishot, and maybe again a moment later with echo. `arrow` is what a bow, crossbow or gun
    /// shoots, None for spells, of `form`.
    #[allow(clippy::too_many_arguments)]
    fn shoot(&mut self, view: &View, profile: &Rc<Profile>, arrow: Option<Missile>, form: Form, from: Vector3<f64>, direction: Vector3<f64>, speed: f64, damage: f64) {
        let pierce = if profile.has(Effect::Pierce) { (profile.param(Effect::Pierce, "count", 2.0) * profile.potency(Effect::Pierce).max(1.0)).round() as u32 } else { 0 };
        let (head, power) = arrow.map_or((0, false), |m| (m.head, m.power));
        let shot = arrow.map_or(Shot::Arrow, |m| m.shot);
        let (pace, gravity, size, _) = match arrow {
            Some(m) => match m.shot {
                Shot::Arrow => (1.0, ARROW_GRAVITY, 1.0, 1.0),
                Shot::Quarrel => (1.0, QUARREL_GRAVITY, 1.0, 1.0),
                Shot::Ball => (1.0, BALL_GRAVITY, m.size, 1.0),
                Shot::Shell => (1.0, SHELL_GRAVITY, m.size, 1.0),
            },
            None => form.flight(),
        };
        let life = match (arrow.map(|m| m.shot), form) {
            (Some(Shot::Ball), _) => 1.5,
            (Some(_), _) => 4.0,
            (None, Form::Orb) => 6.0,
            (None, _) => 3.0,
        };
        let make = |direction: Vector3<f64>, damage: f64| Projectile {
            arrow: arrow.is_some(),
            shot,
            position: from,
            velocity: direction * speed * pace,
            damage,
            profile: profile.clone(),
            pierce,
            hit: Vec::new(),
            life,
            head,
            power,
            trail: 0.0,
            form,
            gravity,
            size,
            pin: 0.0,
            seek: 0.0,
            blast: 0.0,
            trick: Trick::None,
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
            // A splitting bolt bursts apart partway, and a net bursts open nearing where it's
            // aimed, or as it's about to fall out of the air
            if let Trick::Split(left) = p.trick {
                if left <= dt {
                    self.split(&p, p.position, 7, 0.45, Trick::Splinter);
                    continue;
                }
                p.trick = Trick::Split(left - dt);
            }
            if let Trick::Net(aim) = p.trick
                && ((p.position - aim).norm() < 2.5 * vpm || p.life <= dt)
            {
                self.net(arena, &p, p.position);
                continue;
            }
            // A rocket bursting once it's up over what it was sent up over
            if let Trick::Rocket(aim) = p.trick
                && (p.position.z >= aim.z || p.life <= dt)
            {
                self.skyburst(arena, &p, p.position);
                continue;
            }
            // A bomb's fuse burning down, wherever it's got to
            if let Trick::Bomb { fuse, cluster, hole } = p.trick {
                if fuse <= dt {
                    self.bomb_burst(arena, &p, p.position);
                    continue;
                }
                p.trick = Trick::Bomb { fuse: fuse - dt, cluster, hole };
            }
            // Bursting ammo's fuse burning down, bursting apart in the air when it's out
            if let Trick::Burst { fuse, burst } = p.trick {
                if fuse <= dt {
                    self.airburst(arena, &p, p.position, burst);
                    continue;
                }
                p.trick = Trick::Burst { fuse: fuse - dt, burst };
            }
            // A comet's shadow, a ring where it's coming down
            if let Trick::Comet(aim) = p.trick {
                self.marks.push((aim + Vector3::new(0.0, 0.0, 0.1 * vpm), 6.0));
            }
            // Homing ones turn towards the animal most nearly ahead of them, not just the nearest,
            // so they keep on at what they were shot at rather than veering off after a squirrel
            let ahead = |to: &Vector3<f64>, direction: &Vector3<f64>| to.normalize().dot(direction) - 0.1 * to.norm() / (25.0 * vpm);
            if profile.has(Effect::Homing) {
                let direction = p.velocity.normalize();
                let range = profile.param(Effect::Homing, "range", 18.0) * vpm;
                let target = arena
                    .animals
                    .iter()
                    .filter(|a| a.health > 0.0 && !a.look.is_pet() && !p.hit.contains(&a.id))
                    .map(|a| a.center(vpm as u32) - p.position)
                    .filter(|to| to.norm() < range && to.normalize().dot(&direction) > 0.5)
                    .max_by(|a, b| ahead(a, &direction).total_cmp(&ahead(b, &direction)));
                if let Some(to) = target {
                    let turn = (profile.param(Effect::Homing, "turn", 4.0) * dt).min(1.0);
                    p.velocity = direction.lerp(&to.normalize(), turn).normalize() * p.velocity.norm();
                }
            }
            // Shots seeking out what a hunter's mark is on
            if self.hunt > 0.0 && p.arrow && !self.marked.is_empty() && !matches!(p.trick, Trick::Bomb { .. } | Trick::Harpoon) {
                let direction = p.velocity.normalize();
                let marked = &self.marked;
                let target = arena
                    .animals
                    .iter()
                    .filter(|a| a.health > 0.0 && marked.contains(&a.id) && !p.hit.contains(&a.id))
                    .map(|a| a.center(vpm as u32) - p.position)
                    .filter(|to| to.norm() < 35.0 * vpm && to.normalize().dot(&direction) > 0.2)
                    .max_by(|a, b| ahead(a, &direction).total_cmp(&ahead(b, &direction)));
                if let Some(to) = target {
                    p.velocity = direction.lerp(&to.normalize(), (6.0 * dt).min(1.0)).normalize() * p.velocity.norm();
                }
            }
            // Missiles, hexes and a swarm of seekers home in on the nearest animal ahead
            let seek = if p.seek > 0.0 {
                p.seek
            } else if !p.arrow && matches!(p.form, Form::Missile | Form::Hex) {
                if p.form == Form::Hex { 2.5 } else { 5.0 }
            } else {
                0.0
            };
            if seek > 0.0 {
                let direction = p.velocity.normalize();
                let target = arena
                    .animals
                    .iter()
                    .filter(|a| a.health > 0.0 && !a.look.is_pet() && !p.hit.contains(&a.id))
                    .map(|a| a.center(vpm as u32) - p.position)
                    .filter(|to| to.norm() < 25.0 * vpm && to.normalize().dot(&direction) > 0.3)
                    .max_by(|a, b| ahead(a, &direction).total_cmp(&ahead(b, &direction)));
                if let Some(to) = target {
                    p.velocity = direction.lerp(&to.normalize(), (seek * dt).min(1.0)).normalize() * p.velocity.norm();
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
            // full draw and a gun's balls a streak. A fireball sheds more, venom and blood drip,
            // and a shell's fuse sputters, smoking.
            let shell = p.arrow && p.shot == Shot::Shell;
            if (!p.arrow && p.form != Form::Lightning) || profile.element.is_some() || p.power || shell {
                // A burst's shards trail less, as there are several at once
                p.trail += dt * if !p.arrow && p.form == Form::Fireball {
                    150.0
                } else if p.trick == Trick::Fragment {
                    30.0
                } else {
                    80.0
                };
                while p.trail >= 1.0 {
                    p.trail -= 1.0;
                    let at = start.lerp(&end, random(&mut self.rng));
                    if !p.arrow && p.form == Form::Hex {
                        // A hex trails darkness, whatever its totem's element
                        self.mote(at, Some(Element::Void), 1.6);
                    } else if shell {
                        let v = self.any_direction() * 0.6 * vpm;
                        self.fine(at, v, Element::look(Some(Element::Fire)), 0.25, -0.1);
                        if random(&mut self.rng) < 0.12 {
                            self.sparks.push(Spark { position: at, velocity: Vector3::new(0.0, 0.0, 0.2) * vpm, look: 0, life: 0.7, lifetime: 0.7, gravity: -0.03, slot: SLOT_SMOKE });
                        }
                    } else if profile.element.is_none() && p.arrow {
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
            let mut bounced = false;
            let wall = solid_along(arena.world, start, end).unwrap_or(f64::MAX);
            // Splashing into water: a spell goes off on it, an arrow carries on, slowed
            let (high, low) = ((arena.water)(start.x, start.y), (arena.water)(end.x, end.y));
            if start.z > high && end.z <= low && low.is_finite() {
                let t = ((start.z - low) / (start.z - end.z)).clamp(0.0, 1.0);
                if t < wall {
                    let at = start + (end - start) * t;
                    self.splash(at, if p.arrow { 0.6 } else { 1.0 });
                    if let Trick::Comet(_) = p.trick {
                        self.comet_impact(arena, &p, at);
                        continue;
                    }
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
                // A bomb goes off on what it hits, a comet comes down on it
                if let Trick::Bomb { .. } = p.trick {
                    self.bomb_burst(arena, &p, at);
                    gone = true;
                    break;
                }
                if let Trick::Comet(_) = p.trick {
                    self.comet_impact(arena, &p, at);
                    gone = true;
                    break;
                }
                let crit = self.roll_crit(&profile);
                let damage = p.damage * if crit { profile.stat("critDamage") } else { 1.0 };
                self.sparks(at, if crit { 10 } else { 6 }, 3.0, Element::look(None), 0.8, 0.35);
                self.element_hit(at, profile.element, if crit || p.power { 1.5 } else { 1.0 });
                self.cues.push(Cue::at(Sound::Hit, at, 0.7));
                p.hit.push(arena.animals[i].id);
                self.strike(arena, &profile, i, Blow { damage, crit, heavy: false, from: start, depth: 0, element: profile.element });
                // Splashing onto what's round it, dragging it in, leaving a pool
                if matches!(p.trick, Trick::None | Trick::Glanced | Trick::Splinter) {
                    self.landing(arena, &profile, at, damage, Some(i), true);
                }
                // A hex clings on, eating away at it
                if !p.arrow && p.form == Form::Hex {
                    let s = &mut arena.animals[i].status;
                    s.hex = HEX_SECONDS;
                    s.hex_dps = s.hex_dps.max(p.damage * HEX_DAMAGE * profile.stat("statusPotency").max(0.5));
                }
                // Pinned where it stands
                if p.pin > 0.0 {
                    let s = &mut arena.animals[i].status;
                    s.staggered = s.staggered.max(p.pin);
                    s.push = Vector3::zeros();
                }
                // Glancing off it to the nearest other, once, with the ricochet effect
                if p.arrow && p.trick == Trick::None && profile.has(Effect::Ricochet) {
                    let range = profile.param(Effect::Ricochet, "range", 9.0);
                    let struck = arena.animals[i].id;
                    if let Some(next) = self.nearest_other(arena, at, range, &[struck])
                        && let Some(to) = arena.animals.iter().find(|a| a.id == next).map(|a| a.center(vpm as u32))
                    {
                        let mut glanced = self.offshoot(&p, at, (to - at).normalize() * p.velocity.norm() * 0.85, p.damage * profile.param(Effect::Ricochet, "power", 0.7), Trick::Glanced);
                        glanced.hit.push(struck);
                        self.projectiles.push(glanced);
                        self.cues.push(Cue::at(Sound::Ricochet, at, 0.45).pitched(1.2));
                    }
                }
                let struck = arena.animals[i].id;
                match p.trick {
                    // A harpoon bites in and holds, its line going taut
                    Trick::Harpoon => {
                        let a = &arena.animals[i];
                        let reel = a.boss.is_none() && a.look.body().1 <= HARPOON_HAUL;
                        self.tether = Some(Tether { caught: Some(struck), at, t: 0.0, reel });
                        self.cues.push(Cue::at(Sound::Taut, at, 0.9));
                        gone = true;
                        break;
                    }
                    // Glancing on to the nearest other it hasn't hit yet, harder each time
                    Trick::Ricochet(n) if n > 0 => {
                        let next = arena
                            .animals
                            .iter()
                            .filter(|a| a.health > 0.0 && !a.look.is_pet() && !p.hit.contains(&a.id))
                            .map(|a| a.center(vpm as u32))
                            .filter(|c| (c - at).norm() < 16.0 * vpm)
                            .min_by(|a, b| (a - at).norm().total_cmp(&(b - at).norm()));
                        if let Some(to) = next {
                            p.velocity = (to - at).normalize() * p.velocity.norm();
                            p.position = at;
                            p.damage *= 1.15;
                            p.trick = Trick::Ricochet(n - 1);
                            p.life = p.life.max(1.0);
                            self.sparks(at, 6, 3.0, Element::look(None), 0.4, 0.25);
                            self.cues.push(Cue::at(Sound::Ricochet, at, 0.6).pitched(1.0 + 0.06 * (8 - n) as f32));
                            bounced = true;
                            break;
                        }
                    }
                    Trick::Split(_) => {
                        self.split(&p, at, 7, 0.45, Trick::Splinter);
                        gone = true;
                        break;
                    }
                    Trick::Splinter => {
                        self.split(&p, at, 3, 0.4, Trick::Glanced);
                        gone = true;
                        break;
                    }
                    Trick::Net(_) => {
                        self.net(arena, &p, at);
                        gone = true;
                        break;
                    }
                    Trick::Detonate(n) => {
                        self.detonate(arena, &p, at, Some(i), n);
                        gone = true;
                        break;
                    }
                    Trick::Thornburst => {
                        self.thornburst(&p, at);
                        gone = true;
                        break;
                    }
                    // Bursting ammo bursting in what it strikes, its shards thrown back out off it
                    Trick::Burst { burst, .. } => {
                        let off = at - arena.animals[i].center(vpm as u32);
                        self.shatter(arena, &p, at, off, burst);
                        gone = true;
                        break;
                    }
                    // One of its shards going off in what it strikes
                    Trick::Fragment => {
                        self.pop(arena, &p, at, Some(i));
                        gone = true;
                        break;
                    }
                    _ => {}
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
            if bounced {
                keep.push(p);
                continue;
            }
            if wall <= 1.0 {
                let at = start + (end - start) * wall;
                match p.trick {
                    // A harpoon biting into the ground or a wall: the player's hauled out to it
                    Trick::Harpoon => {
                        self.tether = Some(Tether { caught: None, at, t: 0.0, reel: false });
                        let direction = p.velocity.normalize();
                        let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &direction).unwrap_or_else(UnitQuaternion::identity);
                        self.stuck.push(Stuck { position: at + direction * 0.15 * vpm, rotation, look: Element::look(profile.element), head: Some(p.head), quarrel: true, element: profile.element, life: 4.0, glowing: 0.0 });
                        self.sparks(at, 6, 2.0, Element::look(None), 1.0, 0.3);
                        self.cues.push(Cue::at(Sound::ArrowHit, at, 0.9).pitched(0.7));
                        self.cues.push(Cue::here(Sound::Taut, 0.8));
                        continue;
                    }
                    // Glancing off the wall, then turning after what's near
                    Trick::Ricochet(n) if n > 0 => {
                        let v = glance(arena.world, at, p.velocity);
                        p.position = at - p.velocity.normalize() * 0.6;
                        p.velocity = v;
                        p.trick = Trick::Ricochet(n - 1);
                        p.seek = 3.5;
                        self.sparks(at, 5, 2.5, Element::look(None), 0.6, 0.25);
                        self.cues.push(Cue::at(Sound::Ricochet, at, 0.6));
                        keep.push(p);
                        continue;
                    }
                    Trick::Net(_) => {
                        self.net(arena, &p, at);
                        continue;
                    }
                    Trick::Detonate(n) => {
                        self.detonate(arena, &p, at, None, n);
                        continue;
                    }
                    Trick::Thornburst => {
                        self.thornburst(&p, at - p.velocity.normalize() * 0.3 * vpm);
                        continue;
                    }
                    // Bursting ammo bursting where it strikes the ground or a wall, its shards
                    // thrown back out off it, and one of its shards going off where it lands
                    Trick::Burst { burst, .. } => {
                        let off = glance(arena.world, at, p.velocity);
                        self.shatter(arena, &p, at - p.velocity.normalize() * 0.3 * vpm, off, burst);
                        continue;
                    }
                    Trick::Fragment => {
                        self.pop(arena, &p, at, None);
                        continue;
                    }
                    // A thorn sticks in where it lands, as a boss's do
                    Trick::Thorn => {
                        let direction = p.velocity.normalize();
                        let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &direction).unwrap_or_else(UnitQuaternion::identity);
                        self.stuck.push(Stuck { position: at + direction * 0.1 * vpm, rotation, look: 0, head: None, quarrel: false, element: None, life: 2.5, glowing: 0.0 });
                        if self.stuck.len() > 40 {
                            self.stuck.remove(0);
                        }
                        continue;
                    }
                    // A bomb bouncing, losing most of its speed each time, till it comes to rest
                    // and lies there, its fuse burning down
                    Trick::Bomb { .. } => {
                        let Some(back) = p.velocity.try_normalize(1e-9) else {
                            keep.push(p);
                            continue;
                        };
                        let v = glance(arena.world, at, p.velocity) * 0.3;
                        p.position = at - back * 0.6;
                        if v.norm() < 1.5 * vpm {
                            p.velocity = Vector3::zeros();
                            p.gravity = 0.0;
                        } else {
                            if v.norm() > 2.5 * vpm {
                                self.cues.push(Cue::at(Sound::ArrowHit, at, 0.45).pitched(0.55));
                            }
                            p.velocity = v;
                        }
                        keep.push(p);
                        continue;
                    }
                    Trick::Comet(_) => {
                        self.comet_impact(arena, &p, at);
                        continue;
                    }
                    Trick::None | Trick::Glanced | Trick::Splinter => self.landing(arena, &profile, at, p.damage, None, true),
                    _ => {}
                }
                self.impact(arena, &p, at);
                // A ball kicks up dirt and whines off
                if p.arrow && p.shot == Shot::Ball && !profile.has(Effect::Explosive) {
                    for _ in 0..3 {
                        let v = (self.any_direction() + Vector3::new(0.0, 0.0, 1.3)) * 2.5 * vpm;
                        self.drop(at, v, DROP_MUD, 0.8);
                    }
                    if random(&mut self.rng) < 0.5 {
                        self.cues.push(Cue::at(Sound::Ricochet, at, 0.5).pitched(0.85 + 0.3 * random(&mut self.rng) as f32));
                    }
                }
                if p.arrow && matches!(p.shot, Shot::Arrow | Shot::Quarrel) && !profile.has(Effect::Explosive) {
                    // Stuck in where it landed, its head buried
                    let direction = p.velocity.normalize();
                    let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &direction).unwrap_or_else(UnitQuaternion::identity);
                    self.stuck.push(Stuck {
                        position: at + direction * 0.12 * vpm,
                        rotation,
                        look: Element::look(profile.element),
                        head: Some(p.head),
                        quarrel: p.shot == Shot::Quarrel,
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
        // A rocket that runs into something on its way up bursts there
        if let Trick::Rocket(_) = p.trick {
            self.skyburst(arena, p, at);
            return;
        }
        if p.blast > 0.0 {
            self.burst(arena, profile, at, p.blast, p.damage * 0.8, 1, false, profile.element.or(Some(Element::Fire)), None);
        } else if p.trick == Trick::Shard {
            // A comet's shard shattering quietly
            self.sparks(at, 6, 2.5, look, 1.2, 0.35);
            self.element_hit(at, Some(Element::Frost), 0.6);
        } else if !p.arrow && p.form == Form::Hex {
            // A hex puffs out where it lands, darkly, blowing no hole
            self.sparks(at, 10, -2.0, Element::look(Some(Element::Void)), 0.0, 0.4);
            self.cues.push(Cue::at(Sound::Hex, at, 0.4).pitched(0.8));
        } else if profile.has(Effect::Explosive) {
            // A shell packed with bomblets bursts bigger, but blows no bigger a hole: blowing one
            // takes as much longer as there's more in it, and it's the bomblets' own holes that
            // tear the ground up round about
            let packed = if profile.has(Effect::Cluster) { CLUSTER_BLAST } else { 1.0 };
            let radius = profile.param(Effect::Explosive, "radius", 2.2) * profile.stat("area").max(1.0) * packed;
            let damage = p.damage * profile.param(Effect::Explosive, "power", 0.7) * profile.potency(Effect::Explosive).max(1.0);
            let holes = self.craters.len();
            self.burst(arena, profile, at, radius, damage, 1, false, profile.element.or(Some(Element::Fire)), None);
            for hole in &mut self.craters[holes..] {
                hole.1 /= packed;
            }
            // Bomblets scattering out of it
            if profile.has(Effect::Cluster) {
                let n = (profile.param(Effect::Cluster, "count", 4.0) * profile.potency(Effect::Cluster).max(1.0)).round() as usize;
                self.scatter(profile, at, n, damage * profile.param(Effect::Cluster, "power", 0.35) / profile.param(Effect::Explosive, "power", 0.7));
            }
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
            // A ball's a sharper smack than an arrow's thud
            let pitch = if p.shot == Shot::Ball { 1.6 } else { 1.0 };
            self.cues.push(Cue::at(Sound::ArrowHit, at, 0.8).pitched(pitch));
        }
    }

    /// A bolt like `p` setting off from `at` at `velocity`, doing `damage`, with `trick`
    fn offshoot(&self, p: &Projectile, at: Vector3<f64>, velocity: Vector3<f64>, damage: f64, trick: Trick) -> Projectile {
        Projectile {
            arrow: p.arrow,
            shot: p.shot,
            position: at,
            velocity,
            damage,
            profile: p.profile.clone(),
            pierce: 0,
            hit: p.hit.clone(),
            life: if trick == Trick::Glanced { 0.8 } else { 1.6 },
            head: p.head,
            power: true,
            trail: 0.0,
            form: p.form,
            gravity: p.gravity,
            size: p.size * 0.85,
            pin: 0.0,
            seek: 0.0,
            blast: 0.0,
            trick,
        }
    }

    /// A thornburst arrow `p` bursting at `at`: a ring of thorns flying out all round, a little
    /// up, through what they meet
    fn thornburst(&mut self, p: &Projectile, at: Vector3<f64>) {
        let vpm = self.vpm;
        let n = 14;
        let turn = random(&mut self.rng) * TAU;
        for k in 0..n {
            let a = turn + k as f64 / n as f64 * TAU;
            let out = Vector3::new(a.cos(), a.sin(), 0.12 + 0.1 * (k % 3) as f64).normalize();
            let mut thorn = self.offshoot(p, at + out * 0.3 * vpm, out * 24.0 * vpm, p.damage * 0.55, Trick::Thorn);
            thorn.pierce = 2;
            thorn.life = 0.55;
            thorn.gravity = 0.25;
            thorn.size = 1.0;
            self.projectiles.push(thorn);
        }
        let look = Element::look(p.profile.element.or(Some(Element::Poison)));
        self.sparks(at, 10, 3.5, look, 0.4, 0.35);
        for _ in 0..6 {
            let v = (self.any_direction() + Vector3::new(0.0, 0.0, 1.2)) * 3.0 * vpm;
            self.drop(at, v, DROP_MUD, 0.8);
        }
        self.flashes.push(Flash { position: at, color: [0.75, 1.0, 0.55], reach: 5.0, life: 0.15, lifetime: 0.15 });
        self.cues.push(Cue::at(Sound::Thorns, at, 1.0));
    }

    /// Bolt `p` bursting apart at `at` into `n` fanned out across the way it was going, each doing
    /// `share` of its damage, with `trick`
    fn split(&mut self, p: &Projectile, at: Vector3<f64>, n: usize, share: f64, trick: Trick) {
        let vpm = self.vpm;
        let speed = p.velocity.norm();
        let ahead = p.velocity / speed.max(1e-6);
        let spread = if n > 3 { 0.45 } else { 0.3 };
        for k in 0..n {
            let a = (k as f64 / (n - 1).max(1) as f64 - 0.5) * 2.0 * spread;
            let d = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), a) * ahead;
            let shard = self.offshoot(p, at + d * 0.25 * vpm, d * speed * 0.9, p.damage * share, trick);
            self.projectiles.push(shard);
        }
        self.sparks(at, 8, 3.0, Element::look(p.profile.element), 0.3, 0.3);
        self.flashes.push(Flash { position: at, color: p.profile.element.map_or([1.0, 0.9, 0.7], Element::light), reach: 4.0, life: 0.15, lifetime: 0.15 });
        self.cues.push(Cue::at(Sound::Release, at, 0.6).pitched(1.5));
    }

    /// A net bursting open over `at`, falling over everything under it and rooting it to the spot
    fn net(&mut self, arena: &mut Arena, p: &Projectile, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let down = Vector3::new(0.0, 0.0, -8.0 * vpm);
        let ground = solid_along(arena.world, at, at + down).map_or(at + down, |t| at + down * t);
        let r = 4.5 * vpm;
        let inside: Vec<usize> = (0..arena.animals.len())
            .filter(|&i| {
                let a = &arena.animals[i];
                a.health > 0.0 && !a.look.is_pet() && (a.position - ground).xy().norm() < r + a.look.body().1 * vpm && (a.position.z - ground.z).abs() < 4.0 * vpm
            })
            .collect();
        let profile = p.profile.clone();
        for i in inside {
            arena.animals[i].status.rooted = arena.animals[i].status.rooted.max(5.0);
            self.strike(arena, &profile, i, Blow { damage: p.damage, crit: false, heavy: false, from: ground, depth: 0, element: profile.element });
        }
        // Flung open over them all, settling
        self.swells.push(Swell { model: NET, position: ground + Vector3::new(0.0, 0.0, 0.6 * vpm), radius: 10.0, life: 0.6, lifetime: 0.6 });
        for _ in 0..10 {
            let v = self.any_direction() + Vector3::new(0.0, 0.0, 0.6);
            self.drop(at, v * 3.0 * vpm, DROP_ASH, 0.8);
        }
        self.cues.push(Cue::at(Sound::Net, ground, 1.0));
    }

    /// A bolt blowing up at `at` in animal `i`, or where it struck, the blast leaping on from it
    /// `n` more times. It blows no holes: there are too many at once.
    fn detonate(&mut self, arena: &mut Arena, p: &Projectile, at: Vector3<f64>, i: Option<usize>, n: u32) {
        let struck = i.map_or(u32::MAX, |i| arena.animals[i].id);
        let profile = p.profile.clone();
        self.scarless = true;
        self.burst(arena, &profile, at, 1.8, p.damage * 0.9, 1, false, Some(Element::Fire), i);
        self.scarless = false;
        if n > 0
            && let Some(next) = self.nearest_other(arena, at, 9.0, &[struck])
        {
            self.pending.push(Pending { delay: 0.16, act: Act::Blast { from: at, to: next, last: struck, hop: 1 }, profile, damage: p.damage * 0.9 });
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
        // A special attack's many small blasts going off together leave nothing behind, as
        // blowing and painting the world takes a while
        let (craters, stains, lingering) = (self.craters.len(), self.stains.len(), self.lingering.len());
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
                    self.lingering.push(Lingering { pool: false, cloud: false, tall: false, position: at, radius: radius * 0.6, life: 3.0, element: Element::Fire, profile: profile.clone(), damage: damage * 0.15, tick: 0.0, effects: false, collapse: 0.0 });
                }
                for k in 0..count(14.0) {
                    let v = self.any_direction() + Vector3::new(0.0, 0.0, 1.2);
                    self.drop(at, v * 3.0 * vpm, if k % 3 == 0 { DROP_EMBER } else { DROP_ASH }, 1.2);
                }
            }
            // Spikes of ice bursting up, shards falling, frost spreading over the ground
            Some(Element::Frost) => {
                self.swells.push(Swell { model: ICE_BURST, position: at, radius, life: 0.9, lifetime: 0.9 });
                // A burst of cold light, shards and mist rolling out round them
                self.blasts.push(Blast { position: at, look, radius: radius * 0.8, life: 0.55, lifetime: 0.55 });
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
                // A green cloud bubbling up and sinking
                self.blasts.push(Blast { position: at, look, radius: radius * 0.85, life: 0.6, lifetime: 0.6 });
                for _ in 0..count(28.0) {
                    let v = self.any_direction() * 0.8 + Vector3::new(0.0, 0.0, 0.9 + random(&mut self.rng));
                    self.drop(at, v * (2.5 + radius) * vpm, DROP_VENOM, 1.4);
                }
                self.element_hit(at, element, 1.5);
                self.stains.push((at, radius, MOSS, false));
                self.lingering.push(Lingering { pool: false, cloud: false, tall: false, position: at, radius: radius * 0.9, life: 4.0, element: Element::Poison, profile: profile.clone(), damage: damage * 0.12, tick: 0.0, effects: false, collapse: 0.0 });
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
                // A burst of it spattering out in drops
                self.blasts.push(Blast { position: at, look, radius: radius * 0.8, life: 0.45, lifetime: 0.45 });
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
        if self.scarless {
            self.craters.truncate(craters);
            self.stains.truncate(stains);
            self.lingering.truncate(lingering);
        } else if self.scar_timer > 0.0 {
            // However many go off at once, as when a blow sets off a synergy on a whole pack, only
            // one blows a hole and stains the ground in any moment, as each takes a while
            self.craters.truncate(craters);
            self.stains.truncate(stains);
        } else if self.craters.len() > craters || self.stains.len() > stains {
            self.scar_timer = SCAR_GAP;
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
            // Flames flickering up, or bubbles and a haze of venom. A wall of fire's flames leap
            // high, tongues of fire swelling and dying away, so it shows over the grass: a few
            // at a time, as each is a single model however big.
            // A cloud billows, a few great puffs of it at a time, and a pool shimmers a little
            let mut owed = dt * if l.tall {
                4.0
            } else if l.cloud {
                3.0 + l.radius
            } else if l.pool {
                (6.0 * l.radius).min(14.0)
            } else {
                25.0 * l.radius
            };
            while owed > 0.0 {
                if random(&mut self.rng) < owed {
                    let a = random(&mut self.rng) * TAU;
                    let r = random(&mut self.rng).sqrt() * l.radius * vpm;
                    if l.cloud {
                        let at = l.position + Vector3::new(a.cos() * r * 0.7, a.sin() * r * 0.7, (0.3 + 0.9 * random(&mut self.rng)) * vpm);
                        let drift = Vector3::new(random(&mut self.rng) - 0.5, random(&mut self.rng) - 0.5, 0.4) * 0.6 * vpm;
                        let life = 1.7 + 0.5 * random(&mut self.rng);
                        let look = Element::look(Some(l.element));
                        self.sparks.push(Spark { position: at, velocity: drift, look, life, lifetime: life, gravity: -0.01, slot: SLOT_CLOUD });
                        self.mote(at, Some(l.element), 1.5);
                    } else if l.tall {
                        let at = l.position + Vector3::new(a.cos() * r * 0.6, a.sin() * r * 0.6, (0.3 + 1.1 * random(&mut self.rng)) * vpm);
                        let radius = 0.3 + 0.2 * random(&mut self.rng);
                        self.blasts.push(Blast { position: at, look: Element::look(Some(Element::Fire)), radius, life: 0.5, lifetime: 0.5 });
                    } else {
                        let at = l.position + Vector3::new(a.cos() * r, a.sin() * r, 0.2 * vpm);
                        self.mote(at, Some(l.element), 2.0);
                    }
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
                    // What stands in a pool or a cloud takes on its element: set alight, chilled
                    // till it freezes, poisoned, shocked or bleeding
                    if (l.pool || l.cloud) && arena.animals[i].health > 0.0 {
                        let s = &mut arena.animals[i].status;
                        match l.element {
                            Element::Fire => {
                                s.burn = s.burn.max(1.0);
                                s.burn_dps = s.burn_dps.max(l.damage * 0.5);
                            }
                            Element::Frost => {
                                s.slowed = s.slowed.max(0.8);
                                s.slow = s.slow.max(0.4);
                                s.chill += 0.2;
                                if s.chill >= 1.0 {
                                    s.chill = 0.0;
                                    s.frozen = s.frozen.max(1.5);
                                }
                            }
                            Element::Poison => {
                                s.poison = s.poison.max(2.5);
                                s.poison_stacks = s.poison_stacks.max(1);
                                s.poison_dps = s.poison_dps.max(l.damage * 0.4);
                            }
                            Element::Storm => s.shocked = s.shocked.max(0.4),
                            Element::Blood => {
                                s.bleed = s.bleed.max(2.0);
                                s.bleed_stacks = s.bleed_stacks.max(1);
                                s.bleed_dps = s.bleed_dps.max(l.damage * 0.4);
                            }
                            _ => {}
                        }
                    }
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

    /// A thread of lightning from `from` to `to`, a few sparks along it, cheaper than a bolt_line
    /// for stringing a lot of them at once
    fn arc(&mut self, from: Vector3<f64>, to: Vector3<f64>) {
        let vpm = self.vpm;
        let look = Element::look(Some(Element::Storm));
        let steps = ((to - from).norm() / (0.45 * vpm)).clamp(3.0, 18.0) as usize;
        for k in 0..=steps {
            let jitter = Vector3::new(random(&mut self.rng) - 0.5, random(&mut self.rng) - 0.5, random(&mut self.rng) - 0.5) * 0.3 * vpm;
            let at = from.lerp(&to, k as f64 / steps as f64) + if k == 0 || k == steps { Vector3::zeros() } else { jitter };
            self.sparks.push(Spark { position: at, velocity: Vector3::zeros(), look, life: 0.2, lifetime: 0.2, gravity: 0.0, slot: SLOT_SPARK });
        }
        self.cap_sparks();
        self.flashes.push(Flash { position: to, color: Element::Storm.light(), reach: 4.0, life: 0.12, lifetime: 0.12 });
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

        // What's marked by a hunter's mark takes half as much again, and what a weapon's mark is
        // on what that mark says
        if self.hunt > 0.0 && self.marked.contains(&arena.animals[i].id) {
            damage *= 1.5;
        }
        if arena.animals[i].status.marked > 0.0 {
            damage *= 1.0 + arena.animals[i].status.mark_bonus;
        }
        // Rupture: a hit on something bleeding hard tears its wounds open, all the bleeding still
        // to come landing at once
        if blow.depth == 0 && profile.has(Effect::Rupture) {
            let s = &mut arena.animals[i].status;
            if s.bleed > 0.0 && s.bleed_stacks >= profile.param(Effect::Rupture, "stacks", 2.0) as u32 {
                let torn = s.bleed_dps * s.bleed_stacks as f64 * s.bleed * profile.param(Effect::Rupture, "share", 0.8) * profile.potency(Effect::Rupture).max(1.0);
                s.bleed = 0.0;
                s.bleed_stacks = 0;
                damage += torn;
                self.say_at(center, "Ruptured!", Element::Blood.color(), 19.0);
                for _ in 0..8 {
                    let v = (self.any_direction() + Vector3::new(0.0, 0.0, 0.8)) * 3.0 * vpm;
                    self.drop(center, v, DROP_BLOOD, 1.0);
                }
            }
        }

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

        // A shieldbearer facing the blow takes most of it on its shield, and a burrower dug in
        // most of it in the sand over it
        let guard = arena.animals[i].guard(blow.from);
        if guard < 1.0 {
            damage *= guard;
            if blow.depth == 0 {
                self.guarded(arena, i, blow.from);
            }
        }
        let died = arena.animals[i].damage(damage, blow.from);
        // Blood spraying out of it, or what it has for blood, and a hard blow staining the ground
        self.bleed(arena, i, damage, blow.from, blow.crit || blow.heavy || died);
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
        // Lifesteal gives health back, a little of what each blow or shot does, its own and not
        // what it sets off
        if blow.depth == 0 && profile.has(Effect::Lifesteal) {
            let share = (profile.param(Effect::Lifesteal, "share", 0.05) * profile.potency(Effect::Lifesteal).max(1.0)).min(0.2);
            self.heal += damage * share;
            let to = (arena.view.eye - Vector3::new(0.0, 0.0, 0.5 * vpm) - center) * 1.6;
            self.fine(center, to, Element::look(Some(Element::Blood)), 0.5, 0.0);
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
            if profile.has(Effect::Miasma) {
                self.miasma(arena, profile, i);
            }
            self.died(arena, i);
        }
    }

    /// The weapon's effects taking hold of animal `i`, hit for `damage`
    fn afflict_hit(&mut self, arena: &mut Arena, profile: &Rc<Profile>, i: usize, damage: f64, blow: Blow) {
        let vpm = arena.view.vpm;
        let center = arena.animals[i].center(vpm as u32);
        let mut froze = false;
        let mut implode = false;
        let mut snared = false;
        let mut marked = false;
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
            // A bigger weapon throws what it hits further, see weapons::SIZES
            let knock = profile.param(Effect::Knockback, "speed", 0.0) * profile.potency(Effect::Knockback).max(1.0) * profile.heft + profile.stat("knockback");
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
            if profile.has(Effect::Snare) && s.rooted <= 0.0 && random(&mut self.rng) < profile.param(Effect::Snare, "chance", 0.4) {
                s.rooted = profile.param(Effect::Snare, "duration", 1.6) * profile.potency(Effect::Snare).max(1.0);
                snared = true;
            }
            if profile.has(Effect::Mark) {
                if s.marked <= 0.0 {
                    marked = true;
                }
                s.marked = profile.param(Effect::Mark, "duration", 6.0);
                s.mark_bonus = s.mark_bonus.max((profile.param(Effect::Mark, "bonus", 0.15) * profile.potency(Effect::Mark).max(1.0)).min(0.4));
            }
        }
        if marked {
            self.say_at(center, "Marked!", [255, 210, 120], 16.0);
        }
        if froze {
            self.say_at(center, "Frozen!", Element::Frost.color(), 19.0);
        }
        if snared {
            self.say_at(center, "Snared!", [220, 200, 150], 17.0);
            self.cues.push(Cue::at(Sound::Net, center, 0.5).pitched(1.3));
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
        // A hex outlives what it caught, passing on to whatever's nearest
        let s = &arena.animals[i].status;
        if s.hex > 0.5 {
            let (left, dps) = (s.hex, s.hex_dps);
            let next = (0..arena.animals.len())
                .filter(|&j| j != i && arena.animals[j].health > 0.0 && !arena.animals[j].look.is_pet())
                .map(|j| (j, (arena.animals[j].center(vpm as u32) - center).norm()))
                .filter(|&(_, d)| d < 8.0 * vpm)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((j, _)) = next {
                let there = arena.animals[j].center(vpm as u32);
                let o = &mut arena.animals[j].status;
                o.hex = o.hex.max(left);
                o.hex_dps = o.hex_dps.max(dps);
                o.shown = 4.0;
                self.bolt_line(center, there, Element::look(Some(Element::Void)));
                self.say_at(there, "Hex passes on!", Element::Void.color(), 18.0);
            }
        }
    }

    /// Animal `i` struck for `damage` by a blow from `from`: its blood, or what it has for blood,
    /// spraying out of it away from the blow, more the harder it is, and with `hard` a splash of it
    /// left on the ground under it
    fn bleed(&mut self, arena: &Arena, i: usize, damage: f64, from: Vector3<f64>, hard: bool) {
        let vpm = self.vpm;
        let a = &arena.animals[i];
        let (kind, stain) = ichor(a.look);
        let (_, size) = a.look.body();
        let center = a.center(vpm as u32);
        // Out of the side the blow came from, and on away from it
        let toward = (from - center).try_normalize(1e-6).unwrap_or_else(Vector3::z);
        let at = center + toward * size * 0.7 * vpm;
        let away = -Vector3::new(toward.x, toward.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let count = ((damage / 6.0).sqrt() * 4.0).clamp(3.0, 16.0) as usize;
        for _ in 0..count {
            let v = away * (0.4 + random(&mut self.rng)) + self.any_direction() * 0.6 + Vector3::new(0.0, 0.0, 0.7 + 0.6 * random(&mut self.rng));
            let speed = 1.5 + 2.0 * random(&mut self.rng);
            self.drop(at, v * speed * vpm, kind, 1.1);
        }
        let Some(material) = stain.filter(|_| hard && self.blood_timer <= 0.0) else {
            return;
        };
        let foot = a.position + Vector3::new(0.0, 0.0, 0.3 * vpm);
        let down = Vector3::new(0.0, 0.0, -4.0 * vpm);
        if let Some(t) = solid_along(arena.world, foot, foot + down) {
            let splat = foot + down * t + away * (0.2 + 0.6 * random(&mut self.rng)) * vpm;
            self.stains.push((splat, (0.3 + 0.15 * size).min(0.8), material, false));
            self.blood_timer = 0.35;
        }
    }

    /// Damage over time, and everything wearing off
    fn afflict(&mut self, arena: &mut Arena, dt: f64) {
        let vpm = arena.view.vpm;
        // A net over each of what's rooted, the nearest few, but for what's bound by a lash
        self.snared.clear();
        let bound: Vec<u32> = self.binds.iter().map(|b| b.id).collect();
        for a in arena.animals.iter().filter(|a| a.health > 0.0 && a.status.rooted > 0.0 && !bound.contains(&a.id)) {
            if self.snared.len() < 16 {
                self.snared.push((a.center(vpm as u32), a.look.body().1 * 2.6));
            }
        }
        // Bleeding, drops of blood falling off it now and then
        for a in arena.animals.iter().filter(|a| a.health > 0.0 && a.status.bleed > 0.0) {
            if random(&mut self.rng) < dt * 5.0 * a.status.bleed_stacks.max(1) as f64 {
                let (height, size) = a.look.body();
                let off = self.any_direction() * size * 0.6;
                let at = a.position + Vector3::new(off.x, off.y, height * 0.8) * vpm;
                let v = Vector3::new(off.x, off.y, 0.2) * vpm;
                self.drop(at, v, ichor(a.look).0, 1.2);
            }
        }
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
            let hexed = s.hex > 0.0;
            if hexed {
                dot += s.hex_dps * dt;
                s.hex -= dt;
                s.slowed = s.slowed.max(0.1);
                s.slow = s.slow.max(HEX_SLOW);
                color = Element::Void.color();
            } else {
                s.hex_dps = 0.0;
            }
            s.frozen -= dt;
            s.staggered -= dt;
            s.rooted -= dt;
            s.marked -= dt;
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
            // Dark motes drawn in to what's hexed
            if hexed && random(&mut self.rng) < dt * 8.0 {
                self.sparks(center, 1, -1.2, Element::look(Some(Element::Void)), 0.0, 0.5);
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
            // Off a blow struck in hand, right in front of the eye, they're fine ones
            let slot = if self.close { SLOT_FINE } else { SLOT_SPARK };
            self.sparks.push(Spark { position, velocity, look, life, lifetime: life, gravity, slot });
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
            // Embers flying up, and a puff of flame, a little one off a blow struck in hand
            Some(Element::Fire) => {
                self.sparks(at, n(8.0), 2.0, look, -0.25, 0.7);
                let puff = if self.close { 0.1 } else { 0.3 };
                self.blasts.push(Blast { position: at, look, radius: puff * strength, life: 0.25, lifetime: 0.25 });
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

    /// A blow from `from` caught on animal `i`'s shield, sparks flying off it and steel ringing,
    /// or on the sand over a burrower, kicking it up
    fn guarded(&mut self, arena: &Arena, i: usize, from: Vector3<f64>) {
        let vpm = self.vpm;
        let a = &arena.animals[i];
        if a.look == Look::Shieldbearer {
            let toward = Vector3::new(from.x - a.position.x, from.y - a.position.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
            let at = a.position + toward * 0.5 * vpm + Vector3::new(0.0, 0.0, 1.1 * vpm);
            self.sparks(at, 10, 3.0, Element::look(None), 0.8, 0.35);
            self.cues.push(Cue::at(Sound::Clang, at, 0.8).pitched(0.85 + 0.2 * random(&mut self.rng) as f32));
            self.popups.push(Popup { position: at + Vector3::new(0.0, 0.0, 0.4 * vpm), text: "Blocked".into(), color: [200, 210, 230], size: 15.0, age: 0.0 });
        } else {
            self.stir(a.position, 0.7, 0.3, DROP_MUD);
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
            // A skeleton archer's bow clattering down with its bones
            Look::BoneArcher => {
                spray(self, 14, DROP_WATER, 3.6, 1.5);
                self.cues.push(Cue::at(Sound::Rattle, at, 0.9).pitched(1.3));
                self.cues.push(Cue::at(Sound::Creak, at, 0.5).pitched(1.6));
            }
            // The void going out of a necromancer's eyes and its orb in a burst, its bones and robe
            // crumpling with a last wail
            Look::Necromancer => {
                spray(self, 12, DROP_WATER, 3.2, 1.4);
                spray(self, 8, DROP_ASH, 2.4, 1.2);
                let void = Element::look(Some(Element::Void));
                self.sparks(at + Vector3::new(0.0, 0.0, 0.6 * vpm), 28, 3.0, void, 0.4, 0.7);
                self.element_hit(at, Some(Element::Void), 1.6);
                self.cues.push(Cue::at(Sound::Wail, at, 0.8).pitched(0.6));
                self.cues.push(Cue::at(Sound::Rattle, at, 0.7).pitched(0.9));
            }
            // A bloater bursting: venom everywhere, see Game::reap() for the cloud it leaves
            Look::Bloater => {
                spray(self, 30, DROP_VENOM, 4.2, 1.4);
                spray(self, 10, DROP_BLOOD, 3.0, 1.1);
                self.element_hit(at, Some(Element::Poison), 2.2);
                self.cues.push(Cue::at(Sound::Burst, at, 0.9).pitched(0.8));
                self.cues.push(Cue::at(Sound::Squelch, at, 1.0).pitched(0.55));
                self.shake = self.shake.max(0.35);
            }
            // A ghoul going down with a shriek
            Look::Ghoul => {
                spray(self, 12, DROP_BLOOD, 3.0, 1.1);
                spray(self, 6, DROP_ASH, 2.4, 1.0);
                self.cues.push(Cue::at(Sound::Screech, at, 0.8).pitched(0.7));
            }
            // A brute toppling like a felled tree, its gravestone thudding down
            Look::GraveBrute => {
                spray(self, 16, DROP_VENOM, 3.0, 1.3);
                spray(self, 10, DROP_BLOOD, 2.6, 1.2);
                spray(self, 8, DROP_MUD, 2.0, 0.9);
                self.cues.push(Cue::at(Sound::Squelch, at, 0.9).pitched(0.6));
                self.cues.push(Cue::at(Sound::Quake, at, 0.5).pitched(1.4));
                self.shake = self.shake.max(0.35);
            }
            Look::Zombie | Look::ArmoredZombie => {
                spray(self, 12, DROP_VENOM, 3.0, 1.2);
                spray(self, 8, DROP_BLOOD, 2.5, 1.2);
                self.cues.push(Cue::at(Sound::Squelch, at, 0.9).pitched(0.8));
            }
            // A bandit going down with a cry, a crossbowman's crossbow clattering down with them
            Look::Bandit | Look::BanditBrute | Look::Crossbowman | Look::Shieldbearer => {
                spray(self, 14, DROP_BLOOD, 2.8, 1.1);
                self.cues.push(Cue::at(Sound::Hurt, at, 0.8).pitched(if a.look == Look::Crossbowman { 1.0 } else { 0.85 }));
                if a.look == Look::Crossbowman {
                    self.cues.push(Cue::at(Sound::Latch, at, 0.6).pitched(0.8));
                }
            }
            // Its plates cracking open, ichor and sand spilling out, its legs clicking as they curl
            Look::DuneBurrower => {
                spray(self, 12, DROP_VENOM, 3.0, 1.1);
                spray(self, 14, DROP_MUD, 2.6, 1.0);
                self.cues.push(Cue::at(Sound::Crack, at, 0.8).pitched(0.9));
                self.cues.push(Cue::at(Sound::Chitter, at, 0.7).pitched(0.7));
            }
            // Coming apart into a swirl of frost rising and settling, its light going out with a
            // last wail
            Look::RimeWraith => {
                spray(self, 18, DROP_ICE, 3.2, 1.3);
                let frost = Element::look(Some(Element::Frost));
                for _ in 0..24 {
                    let v = self.any_direction() * 0.6 + Vector3::new(0.0, 0.0, 0.5);
                    self.fine(at + Vector3::new(0.0, 0.0, 0.4 * vpm), v * vpm, frost, 1.2, -0.05);
                }
                self.element_hit(at, Some(Element::Frost), 1.5);
                self.cues.push(Cue::at(Sound::Wail, at, 0.8).pitched(1.3));
                self.cues.push(Cue::at(Sound::Crack, at, 0.6).pitched(1.4));
            }
            // Bursting wetly, a last croak gurgling out of it
            Look::MireLurker => {
                spray(self, 14, DROP_WATER, 3.0, 1.1);
                spray(self, 10, DROP_VENOM, 2.4, 1.1);
                self.cues.push(Cue::at(Sound::Squelch, at, 0.9).pitched(0.7));
                self.cues.push(Cue::at(Sound::Croak, at, 0.7).pitched(0.6));
            }
            // A thin shriek as it drops out of the air
            Look::Gloomwing => {
                spray(self, 8, DROP_BLOOD, 2.4, 0.9);
                self.cues.push(Cue::at(Sound::Screech, at, 0.6).pitched(1.5));
            }
            // A great beast going down with a squeal and a heavy thud
            Look::Tusker => {
                spray(self, 18, DROP_BLOOD, 3.0, 1.2);
                spray(self, 8, DROP_MUD, 2.0, 0.9);
                self.cues.push(Cue::at(Sound::Snort, at, 0.9).pitched(1.4));
                self.cues.push(Cue::at(Sound::Quake, at, 0.4).pitched(1.6));
                self.shake = self.shake.max(0.3);
            }
            _ => {
                spray(self, 10, DROP_ASH, 3.0, 1.0);
                spray(self, 8, DROP_VENOM, 3.0, 1.0);
                self.cues.push(Cue::at(Sound::Hiss, at, 0.6).pitched(1.8));
                self.cues.push(Cue::at(Sound::Crack, at, 0.5).pitched(1.8));
            }
        }
        // Armor ringing as it falls, and a shield
        if matches!(a.look, Look::ArmoredSkeleton | Look::ArmoredZombie | Look::BanditBrute | Look::Shieldbearer) {
            self.sparks(at, 8, 3.0, Element::look(None), 0.8, 0.4);
            self.cues.push(Cue::at(Sound::Clang, at, 0.6).pitched(0.7));
        }
        self.flashes.push(Flash { position: at, color: [1.0, 1.0, 1.0], reach: 4.0, life: 0.12, lifetime: 0.12 });
        self.cues.push(Cue::at(Sound::Hit, at, 0.8).pitched(0.6));
        self.shake = self.shake.max(0.2);
        self.cap_sparks();
    }

    /// For this frame, a cloud of venom hanging over the ground at `at`, `radius` meters round:
    /// puffs of it billowing up and drifting, as a censer's or a miasma's do, without hurting what's
    /// in it, see Game::reek() for what it does to the player
    pub fn reek(&mut self, at: Vector3<f64>, radius: f64, dt: f64) {
        let vpm = self.vpm;
        let mut owed = dt * (3.0 + radius);
        while owed > 0.0 {
            if random(&mut self.rng) < owed {
                let a = random(&mut self.rng) * TAU;
                let r = random(&mut self.rng).sqrt() * radius * vpm;
                let p = at + Vector3::new(a.cos() * r * 0.7, a.sin() * r * 0.7, (0.3 + 0.9 * random(&mut self.rng)) * vpm);
                let drift = Vector3::new(random(&mut self.rng) - 0.5, random(&mut self.rng) - 0.5, 0.4) * 0.6 * vpm;
                let life = 1.7 + 0.5 * random(&mut self.rng);
                let look = Element::look(Some(Element::Poison));
                self.sparks.push(Spark { position: p, velocity: drift, look, life, lifetime: life, gravity: -0.01, slot: SLOT_CLOUD });
                self.mote(p, Some(Element::Poison), 1.5);
            }
            owed -= 1.0;
        }
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

    /// Something bursting up out of the water at `at`, on its surface, `size` 1 for a spell's splash
    pub fn splash_up(&mut self, at: Vector3<f64>, size: f64) {
        self.splash(at, size);
    }

    /// Something bursting up out of the ground at `at`: a fountain of `kind`, one of
    /// weapons::DROP_MUD and the rest, thrown up and falling round, and more stirred up after
    pub fn burst_up(&mut self, at: Vector3<f64>, kind: usize) {
        let vpm = self.vpm;
        for _ in 0..34 {
            let out = self.any_direction();
            let v = Vector3::new(out.x * 0.8, out.y * 0.8, 1.0 + 0.8 * random(&mut self.rng)) * (2.5 + 2.5 * random(&mut self.rng));
            self.drop(at + Vector3::new(0.0, 0.0, 0.2 * vpm), v * vpm, kind, 1.3);
        }
        self.stir(at, 1.3, 0.5, kind);
    }

    /// A wraith gone from `from` in a swirl of frost drawn in on itself, and back at `to` in a
    /// burst of it
    pub fn blink(&mut self, from: Vector3<f64>, to: Vector3<f64>) {
        let frost = Element::look(Some(Element::Frost));
        self.sparks(from, 26, -2.5, frost, 0.0, 0.45);
        self.sparks(to, 22, 2.2, frost, 0.1, 0.5);
        self.element_hit(to, Some(Element::Frost), 1.2);
        self.flashes.push(Flash { position: to, color: Element::Frost.light(), reach: 5.0, life: 0.2, lifetime: 0.2 });
    }

    /// A lurker's tongue lashing out from `from` to `to` and back, there for a moment: a string of
    /// wet drops along it
    pub fn tongue(&mut self, from: Vector3<f64>, to: Vector3<f64>) {
        let vpm = self.vpm;
        let steps = ((to - from).norm() / (0.12 * vpm)).clamp(4.0, 80.0) as usize;
        let back = (from - to).try_normalize(1e-6).unwrap_or_else(Vector3::z);
        for k in 0..=steps {
            let f = k as f64 / steps as f64;
            // Pulled back in along itself, the far end first
            let velocity = back * (6.0 * f) * vpm;
            self.sparks.push(Spark { position: from.lerp(&to, f), velocity, look: DROP_BLOOD, life: 0.22, lifetime: 0.22, gravity: 0.1, slot: SLOT_DROP });
        }
        for _ in 0..8 {
            let v = self.any_direction() + Vector3::new(0.0, 0.0, 0.6);
            self.drop(to, v * 1.8 * vpm, DROP_WATER, 0.6);
        }
        self.cap_sparks();
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
            // A bolt thudding in, a little dirt kicked up round it
            HostileKind::Bolt => {
                for _ in 0..4 {
                    let v = self.any_direction() * 0.6 + Vector3::new(0.0, 0.0, 1.0);
                    self.drop(at, v * 1.6 * vpm, DROP_MUD, 0.5);
                }
                self.cues.push(Cue::at(Sound::ArrowHit, at, 0.6).pitched(0.9));
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
                None if h.kind != HostileKind::Bolt && random(&mut self.rng) < dt * 40.0 => {
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
            // A thorn or a bolt only hurts if it hits; the rest go off, hurting what's near
            let hit = direct.is_some_and(|d| d <= t);
            if let Some(b) = body {
                let d = (b - at).norm() / vpm;
                if h.kind.pierces() {
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
            if !h.kind.pierces() {
                let near = (1.0 - (at - arena.view.eye).norm() / vpm / 30.0).max(0.0);
                self.shake = self.shake.max(near * 0.5);
            }
            if water.is_some_and(|w| w <= t) && !hit {
                self.splash(at, if h.kind.pierces() { 0.5 } else { 1.4 });
                continue;
            }
            self.land(h.kind, at);
            if h.kind.pierces() && !hit && wall.is_some_and(|w| w <= t) {
                // Left standing in the ground: a thorn, or a bolt
                let direction = h.velocity.normalize();
                let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &direction).unwrap_or_else(UnitQuaternion::identity);
                let (head, quarrel) = if h.kind == HostileKind::Bolt { (Some(0), true) } else { (None, false) };
                self.stuck.push(Stuck { position: at + direction * 0.12 * vpm, rotation, look: Element::look(None), head, quarrel, element: None, life: 12.0, glowing: 0.0 });
                if self.stuck.len() > 40 {
                    self.stuck.remove(0);
                }
            }
        }
    }

    /// The lash of the whip in hand, a rope hanging from the top of its handle: swaying and
    /// trailing as the player walks and turns, and through a swing steered as lash_cracks() has it,
    /// winding back, rolling out from the hand and snapping straight at the crosshair as the blow
    /// lands, then let go to fall and follow through
    fn swing_lash(&mut self, arena: &Arena, weapon: &Weapon, dt: f64) {
        let Some(look) = &weapon.look.lash else {
            self.lash = None;
            return;
        };
        let view = arena.view;
        let vpm = view.vpm;
        let segment = (weapon.profile.stat("range").max(1.0) - LASH_SHORT).max(1.5) * vpm / (LASH_POINTS - 1) as f64;
        let root = self.held_at(weapon, view, look.root);
        // Laid out afresh when first held, if it's grown, or if the hand jumped far
        let fresh = self.lash.as_ref().is_none_or(|l| (l.segment - segment).abs() > 1e-6 || (l.root - root).norm() > 2.5 * vpm);
        if fresh {
            self.lash = Some(Lash::hanging(arena, root, segment));
        }
        // In slow motion with the swing as a blow lands
        let frame = dt.min(0.05);
        let slow = matches!(self.action, Action::Swing { .. }) && self.hitstop > 0.0;
        let dt = if slow { frame * HITSTOP_PACE } else { frame };
        if dt <= 0.0 {
            return;
        }
        let guide = self.lash_guide(arena, weapon, root, segment);
        if let Some(lash) = &mut self.lash {
            lash.step(arena, root, view.eye, guide.as_deref(), dt, frame);
        }
    }

    /// Where the swing in hand steers the whip's lash now, if it does: for each point, how far from
    /// the root at `root` it's pulled to, in voxels, and how hard, 0-1.
    ///
    /// Through a crack, each stretch of the lash is pulled to point back the way it's wound, then
    /// turned to point at the crosshair, the stretches nearest the hand first and the tip last,
    /// which rolls a loop out along it that straightens at the tip just as the blow lands, out to
    /// whatever's there or full stretch. It's held out a moment, then let go.
    fn lash_guide(&self, arena: &Arena, weapon: &Weapon, root: Vector3<f64>, segment: f64) -> Option<Vec<(Vector3<f64>, f64)>> {
        let (t, length, cracks, special) = match self.action {
            Action::Swing { t, length, heavy, combo, .. } => (t, length, lash_swing(heavy, combo, weapon.grade, weight(&weapon.profile)).cracks, false),
            Action::Special { t, length, special } => (t, length, special_cracks(special), true),
            _ => return None,
        };
        let view = arena.view;
        let crack = cracks.into_iter().rev().find(|c| t >= c.from * length)?;
        // Twirled round over the head, or swept round low over the ground: a loose curve going
        // round the hand, each stretch trailing the one before, eased in and let go at the end
        if let Wind::Twirl(_) | Wind::Low = crack.wind {
            let (start, end) = (crack.from * length, crack.at * length);
            if t > end + 0.06 {
                return None;
            }
            let u = ((t - start) / (end - start).max(1e-3)).clamp(0.0, 1.0);
            let ease = smooth(u / 0.15) * (1.0 - smooth((u - 0.85) / 0.15).max(smooth((t - end) / 0.06)));
            return Some(match crack.wind {
                Wind::Twirl(turns) => self.round_guide(view, FRAC_PI_2 + turns * TAU * smooth(u), 0.42, 0.9, 0.45 * ease, segment),
                _ => self.round_guide(view, FRAC_PI_2 + TAU * smooth(u), -0.28, 0.45, 0.75 * ease, segment),
            });
        }
        let at = crack.at * length;
        let release = at + crack.hold;
        if t > release + 0.08 {
            return None;
        }
        // How long the throw takes to roll out from the hand to the tip, and each stretch's turn:
        // a little longer the heavier it cracks
        let lead = (0.3 * length).min(0.11 * (1.0 + 0.25 * weight(&weapon.profile).max(0.0)));
        let turn = lead * 0.45;
        // Where it's thrown, once it is: at the crosshair, as far as what's there, or for a
        // special attack at what it's flung round or caught on
        let (mut aim, distance) = if t < at - lead {
            (view.forward(), f64::MAX)
        } else {
            let target = match self.lash_aim {
                Some(p) if special => p,
                _ => self.crack_point(arena, weapon.profile.stat("range").max(1.0) * view.vpm),
            };
            ((target - root).try_normalize(1e-9)?, (target - root).norm())
        };
        // A fan sweeps across from one side to the other as it cracks
        if crack.sweep != 0.0 {
            let across = ((at + 0.1 - t) / 0.2).clamp(0.0, 1.0) * 2.0 - 1.0;
            let up = view.rotation * Vector3::z_axis();
            aim = UnitQuaternion::from_axis_angle(&up, crack.sweep * across * 0.6) * aim;
        }
        // Which way each stretch is wound back at `time`, `u` of the way along the lash
        let start = crack.from * length;
        let wound = |time: f64, u: f64| -> Vector3<f64> {
            match crack.wind {
                Wind::Back(d) => view.rotation * Vector3::from(d).normalize(),
                // A loop through the hand, going round once over the head like a lasso's, each
                // stretch along it, coming round so it leaves the hand out back to the right as
                // it's thrown
                Wind::Whirl => {
                    let k = ((time - start) / (at - lead - start).max(1e-3)).clamp(0.0, 1.0);
                    let angle = 3.67 - (1.0 - k) * TAU - u * TAU;
                    view.rotation * Vector3::new(-angle.sin(), angle.cos(), 0.3).normalize()
                }
                // Taken care of above
                Wind::Twirl(_) | Wind::Low => view.rotation * Vector3::new(0.3, -0.5, 0.8).normalize(),
            }
        };
        // Wound back gently, so it still sags and swings rather than standing out stiff, but
        // whirled harder, to keep up with the loop
        let firm = if matches!(crack.wind, Wind::Whirl) { 0.5 } else { 0.2 };
        let winding = firm * smooth((t - start) / (0.25 * length).max(0.04));
        let letting_go = 1.0 - smooth((t - release) / 0.08);
        let mut out = Vec::with_capacity(LASH_POINTS);
        let mut offset = Vector3::zeros();
        out.push((offset, 0.0));
        for i in 1..LASH_POINTS {
            let u = i as f64 / (LASH_POINTS - 1) as f64;
            let from = at - lead + (lead - turn) * u;
            let k = smooth((t - from) / turn);
            let dir = if k <= 0.0 { wound(t, u) } else { swing_toward(wound(from, u), aim, k) };
            offset += dir * segment;
            // Not on past what it cracks against
            let ahead = offset.dot(&aim);
            let pulled = if ahead > distance { offset - aim * (ahead - distance) } else { offset };
            out.push((pulled, (winding + (1.0 - winding) * k) * letting_go));
        }
        Some(out)
    }

    /// A whip's lash steered round the hand, for lash_guide(): each stretch pointing `angle`
    /// radians round from the right, past straight ahead, less `lag` of it by the tip, so the lash
    /// curves round after the hand, tilted `up` out of level, and pulled there `firm` hard
    fn round_guide(&self, view: &View, angle: f64, up: f64, lag: f64, firm: f64, segment: f64) -> Vec<(Vector3<f64>, f64)> {
        let forward = view.forward();
        let flat = Vector3::new(forward.x, forward.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::y);
        let right = flat.cross(&Vector3::z());
        let level = (1.0 - up * up).max(0.0).sqrt();
        let mut out = Vec::with_capacity(LASH_POINTS);
        let mut offset = Vector3::zeros();
        out.push((offset, 0.0));
        for i in 1..LASH_POINTS {
            let u = i as f64 / (LASH_POINTS - 1) as f64;
            let a = angle - lag * u;
            offset += ((right * a.cos() + flat * a.sin()) * level + Vector3::z() * up) * segment;
            out.push((offset, firm));
        }
        out
    }

    /// Where a whip reaching `reach` voxels cracks: at the first thing along the crosshair, or out
    /// at full stretch
    fn crack_point(&self, arena: &Arena, reach: f64) -> Vector3<f64> {
        let view = arena.view;
        let far = view.eye + view.forward() * reach;
        let mut t = solid_along(arena.world, view.eye, far).unwrap_or(1.0);
        for a in arena.animals.iter().filter(|a| a.health > 0.0 && !a.look.is_pet()) {
            if let Some(hit) = sphere_hit(view.eye, far, a.center(view.vpm as u32), a.look.body().1 * view.vpm) {
                t = t.min(hit);
            }
        }
        view.eye + (far - view.eye) * t
    }

    /// A spell let go from an epic or rarer staff, scepter or wand: a ring of its rarity's color
    /// bursting out round the tip, wider from legendary, and a flash of it
    fn cast_flourish(&mut self, view: &View, weapon: &Weapon) {
        if weapon.grade < TRAIL_GRADE {
            return;
        }
        let vpm = view.vpm;
        let tip = self.held_point(weapon, view, false);
        let (first, second, share) = rarity_looks(weapon.profile.color);
        let forward = view.forward();
        let right = view.rotation * Vector3::x();
        let up = view.rotation * Vector3::z();
        let n = if weapon.grade > TRAIL_GRADE { 12 } else { 8 };
        for k in 0..n {
            let a = k as f64 / n as f64 * TAU;
            let out = right * a.cos() + up * a.sin();
            let look = if random(&mut self.rng) < share { first } else { second };
            self.fine(tip + out * 0.03 * vpm, (out * 1.4 + forward * 0.6) * vpm, look, 0.22, 0.0);
        }
        self.glow = self.glow.max(0.15);
    }

    /// The weapon in hand leaving a trail off its tip as it comes down on something, and sparks of
    /// its element drifting off it
    fn flourish(&mut self, view: &View, weapon: &Weapon, dt: f64) {
        let vpm = view.vpm;
        let profile = &weapon.profile;
        let look = Element::look(profile.element);
        let swinging = match self.action {
            // A whip's lash from as it's thrown till it's let go, each crack, and its twirl
            Action::Swing { t, length, heavy, combo, .. } if profile.style == Style::Lash => {
                let f = t / length;
                lash_swing(heavy, combo, weapon.grade, weight(profile)).cracks.iter().any(|c| match c.wind {
                    Wind::Twirl(_) => (c.from..c.at).contains(&f),
                    _ => (c.at - 0.12..c.at + 0.08).contains(&f),
                })
            }
            Action::Swing { t, length, heavy, combo, .. } => {
                let (comes, lands) = blow_times(profile, heavy, combo);
                (comes..lands + 0.1).contains(&(t / length))
            }
            // A spell's cast from as it's gathered till just after it's let go: a wand's flick and
            // a rod's thrust trailing, and an epic or rarer staff's arc
            Action::Cast { t, length, .. } => {
                let (gathers, release) = cast_times(profile.casting, profile.style);
                (gathers * 0.6..release + 0.15).contains(&(t / length)) && (matches!(profile.style, Style::Flick | Style::Arc) || weapon.grade >= TRAIL_GRADE)
            }
            // Not while it's thrown, out of the hand
            Action::Special { .. } => weapon.kind() == Kind::Melee && self.thrown.is_none(),
            _ => false,
        };
        // From epic on, a blow or a cast trails a glowing arc of its rarity's color off the blade,
        // the lash or the staff, from its tip and from further down it, see rarity_looks(), and
        // lights what's round it
        let glowing = swinging && weapon.grade >= TRAIL_GRADE && matches!(self.action, Action::Swing { .. } | Action::Special { .. } | Action::Cast { .. });
        if swinging {
            let tip = self.held_point(weapon, view, false);
            // An element's own trail off the tip, or a plain one if it has no glow of its own
            if let Some(last) = self.trail
                && (profile.element.is_some() || !glowing)
            {
                let steps = ((tip - last).norm() / (0.012 * vpm)).ceil().min(24.0) as usize;
                for k in 1..=steps {
                    let at = last.lerp(&tip, k as f64 / steps as f64);
                    self.fine(at, Vector3::zeros(), look, if profile.element.is_some() { 0.22 } else { 0.12 }, 0.0);
                }
            }
            if glowing {
                let (first, second, share) = rarity_looks(profile.color);
                // Two thirds of the way along a whip's lash, or of a blade or a staff
                let edge = match &self.lash {
                    Some(lash) if weapon.look.lash.is_some() => lash.points[LASH_POINTS * 2 / 3],
                    _ => self.held_at(weapon, view, weapon.look.grip.lerp(&weapon.look.tip, 0.7)),
                };
                for (from, to) in [(self.trail, tip), (self.ribbon, edge)] {
                    let Some(from) = from else {
                        continue;
                    };
                    let steps = ((to - from).norm() / (0.025 * vpm)).ceil().min(8.0) as usize;
                    for k in 1..=steps {
                        let look = if random(&mut self.rng) < share { first } else { second };
                        self.glints.push(Glint { position: from.lerp(&to, k as f64 / steps as f64), look, life: GLINT_LIFE });
                    }
                }
                if self.glints.len() > MOST_GLINTS {
                    let extra = self.glints.len() - MOST_GLINTS;
                    self.glints.drain(..extra);
                }
                self.ribbon = Some(edge);
                self.glow = 0.2;
            }
            self.trail = Some(tip);
        } else {
            self.trail = None;
        }
        if !glowing {
            self.ribbon = None;
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
            // From the crystal of a staff, anywhere along a blade or a whip's lash
            let along = if weapon.kind() == Kind::Magic { 1.0 } else { 0.35 + 0.65 * random(&mut self.rng) };
            let at = match &self.lash {
                Some(lash) => lash.points[((along * (LASH_POINTS - 1) as f64).round() as usize).min(LASH_POINTS - 1)],
                None => self.held_at(weapon, view, weapon.look.grip.lerp(&weapon.look.tip, along)),
            };
            let v = Vector3::new(random(&mut self.rng), random(&mut self.rng), random(&mut self.rng)) * 2.0 - Vector3::repeat(1.0);
            self.fine(at, v * speed * vpm, look, life, gravity);
        }
    }

    /// The pose of the weapon in hand now, in the camera's frame: as it was drawn this frame, see
    /// settle(), or where its move has it if it hasn't been yet
    fn pose(&self, weapon: &Weapon) -> Pose {
        self.drawn.clone().unwrap_or_else(|| self.target_pose(weapon))
    }

    /// Works out the pose to draw the weapon in hand in this frame: where its move has it, blended
    /// in from wherever it really was when a new move started, keeping how fast it was going
    fn settle(&mut self, weapon: &Weapon, dt: f64) {
        let target = self.target_pose(weapon);
        if let Some(drawn) = &self.drawn
            && new_move(self.drawn_action, self.action)
        {
            self.blending = Some(Blending::new(drawn, self.drawn_velocity, &target));
        }
        self.drawn_action = self.action;
        let now = match &mut self.blending {
            Some(b) => {
                b.age += dt;
                b.apply(&target)
            }
            None => target,
        };
        if self.blending.as_ref().is_some_and(Blending::done) {
            self.blending = None;
        }
        if let Some(before) = &self.drawn
            && dt > 1e-6
        {
            self.drawn_velocity = ((now.hand - before.hand) / dt, (now.turn * before.turn.inverse()).scaled_axis() / dt);
        }
        self.drawn = Some(now);
    }

    /// Where the weapon in hand's move has it now, in the camera's frame
    fn target_pose(&self, weapon: &Weapon) -> Pose {
        // Held like in Skyrim: a sword or staff in the right hand down at the side, pointing on
        // ahead, a bow in the left
        let style = moves_like(weapon.profile.style);
        // A battle staff's held and swung like a scythe
        let kind = if weapon.profile.style == Style::Glaive { Kind::Melee } else { weapon.kind() };
        let mut p = match kind {
            Kind::Melee => {
                // Out to the right, clear of the crosshair
                let rest = match style {
                    // Daggers up a little to be seen, spears levelled on ahead
                    Style::Stab => pose([0.28, 0.42, -0.23], [-0.08, 0.75, 0.65], [0.0, 1.0, 0.0]),
                    Style::Thrust => pose([0.34, 0.3, -0.33], [0.05, 0.97, 0.2], [-1.0, 0.0, 0.0]),
                    // A scythe's pole leant across, its blade out to the left at the top
                    Style::Reap => pose([0.31, 0.4, -0.4], [-0.2, 0.45, 0.86], [-1.0, 0.0, 0.25]),
                    // A whip held out a little raised, its lash hanging from the end of the handle
                    Style::Lash => grip(LASH_REST.0, LASH_REST.1),
                    // A flail held low, its head out ahead on its chain
                    Style::Whirl => pose([0.3, 0.42, -0.32], [-0.1, 0.88, 0.46], [1.0, 0.0, 0.0]),
                    _ => pose([0.3, 0.46, -0.27], [-0.05, 0.62, 0.78], [0.0, 1.0, 0.0]),
                };
                match self.action {
                    // A whip's handle as heavy as it cracks, see lash_swing()
                    Action::Swing { t, length, heavy, combo, .. } if style == Style::Lash => animate(&rest, &blow_poses(&weapon.profile, heavy, combo), t / length),
                    // As heavy as it swings, and a rare one twirled in the hand as it follows
                    // through, see twirl()
                    Action::Swing { t, length, heavy, combo, .. } => {
                        let mut p = animate(&rest, &blow_poses(&weapon.profile, heavy, combo), t / length);
                        let (spin, lift) = twirl(weapon.grade, style, heavy, combo, blow_times(&weapon.profile, heavy, combo).1, t / length);
                        p.hand += Vector3::new(-0.02, 0.02, 0.04) * lift;
                        p.turn *= UnitQuaternion::from_axis_angle(&Vector3::z_axis(), spin);
                        p
                    }
                    Action::Special { t, length, special } => animate(&rest, &special_move(special, Kind::Melee, style).1, t / length),
                    _ => rest,
                }
            }
            // A bomb held up on the palm at the right, drawn back over the shoulder and lobbed,
            // the arm following through
            Kind::Ranged if weapon.profile.style == Style::Lob => {
                let rest = pose([0.27, 0.48, -0.26], [0.05, 0.1, 0.99], [1.0, 0.0, 0.0]);
                match self.action {
                    Action::Throw { t, length, .. } => {
                        let keys = [
                            (0.22, pose([0.3, 0.3, -0.12], [0.15, -0.35, 0.92], [1.0, 0.0, 0.0]), false),
                            (BOMB_RELEASE - 0.05, pose([0.33, 0.14, 0.02], [0.1, -0.55, 0.83], [1.0, 0.0, 0.0]), false),
                            (BOMB_RELEASE, pose([0.22, 0.62, -0.02], [0.0, 0.85, 0.53], [1.0, 0.0, 0.0]), true),
                            (0.78, pose([0.12, 0.6, -0.38], [0.0, 0.6, -0.8], [1.0, 0.0, 0.0]), false),
                        ];
                        animate(&rest, &keys, t / length)
                    }
                    Action::Special { t, length, special } => animate(&rest, &special_move(special, Kind::Melee, style).1, t / length),
                    _ => rest,
                }
            }
            Kind::Ranged if weapon.profile.build != Build::None => self.shouldered(weapon),
            Kind::Ranged => {
                // Held low at the left, then raised and drawn back to the cheek
                let rest = pose([-0.25, 0.48, -0.28], [-0.35, 0.35, 0.87], [0.1, 1.0, 0.0]);
                let drawn = pose([-0.1, 0.66, -0.08], [-0.33, 0.02, 0.94], [0.0, 1.0, 0.0]);
                match self.action {
                    Action::Draw { t } => {
                        // Raised and canted in, then pushed out against the string's pull as it
                        // comes back, the bow leaning in to the cheek. Held at full draw it
                        // trembles, more the longer it's held, and sways as the archer breathes.
                        // A short bow's snapped up quick and close, a heavy one leant into and
                        // trembling more, a fine one canted right over and held so.
                        let knack = weapon.profile.knack;
                        let (pace, cant, push, tremble) = match knack {
                            Knack::Quick => (6.5, 0.12, 0.015, 0.7),
                            Knack::Heavy => (2.8, 0.3, 0.045, 1.6),
                            _ => (4.0, 0.22, 0.025, 1.0),
                        };
                        let raise = smooth(t * pace);
                        let draw = self.drawn(&weapon.profile);
                        let mut p = blend(&rest, &drawn, raise);
                        p.turn = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), -cant * raise * (1.0 - raise) * 4.0 * 0.5) * p.turn;
                        p.hand.y += push * draw * draw;
                        p.turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), -0.06 * draw) * p.turn;
                        match knack {
                            // Leant over into the draw, the bow lower
                            Knack::Heavy => {
                                p.hand.z -= 0.025 * draw;
                                p.turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.06 * draw) * p.turn;
                            }
                            // Canted well over, held flat across, the hand in a little
                            Knack::Showy => {
                                p.turn = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), -0.55 * raise) * p.turn;
                                p.hand.x += 0.03 * raise;
                                p.hand.z -= 0.02 * raise;
                            }
                            _ => {}
                        }
                        let full = 1.0 / (weapon.profile.stat("drawSpeed") * self.speedup()).max(0.1);
                        let held = smooth((t - full) / 3.0);
                        let shake = (0.0008 + 0.0035 * held) * draw * tremble;
                        p.hand.x += shake * ((self.time * 23.0).sin() + 0.6 * (self.time * 37.0).sin());
                        p.hand.z += shake * ((self.time * 29.0).cos() + 0.5 * (self.time * 41.0).sin()) + 0.004 * draw * (self.time * 1.6).sin();
                        p
                    }
                    Action::Loose { t } => {
                        // The string snapping forward kicks the bow, which rolls forward over the
                        // hand and drops as the arm follows through, then comes back to rest
                        let f = (t / 0.2).min(1.0);
                        let mut p = blend(&drawn, &rest, smooth((f - 0.25) / 0.75));
                        let snap = (PI * f).sin() * (1.0 - f);
                        p.hand.y -= 0.03 * (1.0 - f) - 0.02 * snap;
                        p.hand.z -= 0.02 * snap;
                        p.turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 0.35 * snap) * p.turn;
                        p
                    }
                    Action::Special { t, length, special } => animate(&rest, &special_move(special, Kind::Ranged, style).1, t / length),
                    _ => rest,
                }
            }
            // An orb held up on the palm at the right, pushed out at the crosshair as its beam
            // pours, trembling with it
            Kind::Magic if style == Style::Channel => {
                let rest = pose([0.24, 0.52, -0.24], [-0.05, 0.1, 0.99], [1.0, 0.0, 0.0]);
                match self.action {
                    Action::Channel { t } => {
                        let mut p = blend(&rest, &pose([0.15, 0.62, -0.19], [-0.12, 0.42, 0.9], [1.0, 0.0, 0.0]), smooth(t * 6.0));
                        p.hand.x += 0.003 * (self.time * 37.0).sin();
                        p.hand.z += 0.003 * (self.time * 43.0).cos();
                        p
                    }
                    Action::Special { t, length, special } => animate(&rest, &special_move(special, Kind::Magic, style).1, t / length),
                    _ => rest,
                }
            }
            // A tome held open in the left hand, raised and thrust out as it conjures
            Kind::Magic if style == Style::Summon => {
                let rest = pose([-0.24, 0.52, -0.3], [0.1, 0.3, 0.95], [1.0, 0.0, -0.1]);
                let keys = [(0.15, pose([-0.17, 0.6, -0.2], [0.05, 0.45, 0.89], [1.0, 0.0, -0.05]), true), (0.5, pose([-0.19, 0.58, -0.23], [0.07, 0.4, 0.91], [1.0, 0.0, -0.05]), false)];
                match self.action {
                    Action::Cast { t, length, .. } => animate(&rest, &keys, t / length),
                    Action::Special { t, length, special } => animate(&rest, &special_move(special, Kind::Magic, style).1, t / length),
                    _ => rest,
                }
            }
            // A totem held upright, shaken and pointed as a hex goes
            Kind::Magic if style == Style::Hex => {
                // Its carved faces turned to look back at whoever holds it
                let rest = pose([0.3, 0.46, -0.34], [-0.03, 0.4, 0.92], [-0.2, -0.92, 0.4]);
                let keys = [
                    (0.08, pose([0.31, 0.45, -0.3], [0.1, 0.35, 0.93], [-0.2, -0.92, 0.4]), false),
                    (0.16, pose([0.28, 0.47, -0.32], [-0.12, 0.38, 0.92], [-0.2, -0.92, 0.4]), false),
                    (0.3, pose([0.2, 0.6, -0.28], [-0.12, 0.8, 0.58], [-0.2, -0.6, 0.8]), true),
                ];
                match self.action {
                    Action::Cast { t, length, .. } => animate(&rest, &keys, t / length),
                    Action::Special { t, length, special } => animate(&rest, &special_move(special, Kind::Magic, style).1, t / length),
                    _ => rest,
                }
            }
            // A bell hanging from the hand, raised and trembling as it's held, swung down and out
            // as it rings
            Kind::Magic if style == Style::Toll => {
                let rest = pose([0.24, 0.55, -0.1], [0.0, 0.05, 1.0], [1.0, 0.0, 0.0]);
                let raised = pose([0.2, 0.5, 0.02], [0.0, -0.35, 0.94], [1.0, 0.0, 0.0]);
                match self.action {
                    Action::Charge { t } => {
                        let k = smooth(t * 2.5);
                        let mut p = blend(&rest, &raised, k);
                        p.hand.x += 0.006 * k * (self.time * 31.0).sin();
                        p
                    }
                    Action::Cast { t, length, .. } => {
                        let keys = [(0.18, pose([0.16, 0.58, -0.1], [0.0, 0.75, 0.66], [1.0, 0.0, 0.0]), true), (0.5, pose([0.2, 0.52, -0.14], [0.0, 0.4, 0.92], [1.0, 0.0, 0.0]), false)];
                        animate(&raised, &keys, t / length)
                    }
                    Action::Special { t, length, special } => animate(&rest, &special_move(special, Kind::Magic, style).1, t / length),
                    _ => rest,
                }
            }
            // A censer hanging from the hand on its chains, swung back and forth on them as it's
            // held, out forward as each cloud billows from it
            Kind::Magic if style == Style::Censer => {
                let rest = pose([0.24, 0.55, -0.08], [0.0, 0.05, 1.0], [1.0, 0.0, 0.0]);
                match self.action {
                    Action::Channel { t } => {
                        let period = censer_period(&weapon.profile, self.speedup());
                        let swing = 0.8 * smooth(t * 2.5) * (TAU * (1.0 - self.puff / period)).cos();
                        let mut p = rest.clone();
                        p.hand.y += 0.04 * smooth(t * 2.5);
                        p.hand.z += 0.03 * smooth(t * 2.5);
                        p.turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), swing) * p.turn;
                        p
                    }
                    Action::Special { t, length, special } => animate(&rest, &special_move(special, Kind::Magic, style).1, t / length),
                    _ => rest,
                }
            }
            // A lightning rod held upright, thrust out at the crosshair as lightning leaps from it,
            // and held there a moment, trembling
            Kind::Magic if style == Style::Arc => {
                let rest = pose([0.3, 0.44, -0.33], [-0.03, 0.42, 0.9], [1.0, 0.0, 0.0]);
                let keys = [
                    (0.09, pose([0.32, 0.4, -0.27], [0.05, 0.25, 0.97], [1.0, 0.0, 0.0]), false),
                    (0.22, pose([0.17, 0.62, -0.27], [-0.12, 0.9, 0.42], [1.0, 0.0, 0.0]), true),
                    (0.55, pose([0.18, 0.6, -0.28], [-0.12, 0.88, 0.45], [1.0, 0.0, 0.0]), false),
                ];
                match self.action {
                    Action::Cast { t, length, .. } => {
                        let mut p = animate(&rest, &keys, t / length);
                        p.hand.x += 0.003 * (self.time * 53.0).sin() * (1.0 - t / length);
                        p
                    }
                    Action::Special { t, length, special } => animate(&rest, &special_move(special, Kind::Magic, style).1, t / length),
                    _ => rest,
                }
            }
            // A staff, scepter or wand: raised, the spell gathered and let go as its focus, element
            // and pace have it, see cast_keys(), and from legendary twirled after it lets go
            Kind::Magic => {
                let rest = if style == Style::Flick { pose([0.29, 0.44, -0.24], [-0.04, 0.55, 0.83], [1.0, 0.0, 0.0]) } else { pose([0.31, 0.45, -0.3], [-0.02, 0.5, 0.86], [1.0, 0.0, 0.0]) };
                match self.action {
                    Action::Cast { t, length, .. } => {
                        let casting = weapon.profile.casting;
                        let f = t / length;
                        let mut p = animate(&rest, &cast_keys(casting), f);
                        let (gathers, release) = cast_times(casting, style);
                        // Trembling with the spell as it gathers
                        if casting == Casting::Gather || casting == Casting::Overhead {
                            let swell = smooth((f - gathers * 0.6) / 0.1) * (1.0 - smooth((f - release) / 0.04));
                            p.hand.x += 0.004 * swell * (self.time * 47.0).sin();
                            p.hand.z += 0.003 * swell * (self.time * 39.0).cos();
                        }
                        // A wand flipped end over end, a staff spun round its length
                        let (spin, lift) = cast_twirl(weapon.grade, self.casts % 3 == 0, release, f);
                        p.hand += Vector3::new(-0.02, 0.02, 0.04) * lift;
                        let axis = if style == Style::Flick { Vector3::x_axis() } else { Vector3::z_axis() };
                        p.turn *= UnitQuaternion::from_axis_angle(&axis, spin);
                        p
                    }
                    Action::Special { t, length, special } => animate(&rest, &special_move(special, Kind::Magic, style).1, t / length),
                    _ => rest,
                }
            }
        };
        // A flourish now and then, standing about with it
        if self.action == Action::Idle && self.idle > IDLE_AFTER {
            let f = ((self.idle - IDLE_AFTER) / FLOURISH_SECONDS).min(1.0);
            let (hand, before, after) = flourish_at(flourish_of(weapon, self.time - self.idle), f);
            p.hand += hand;
            p.turn = before * p.turn * after;
        }
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
        // Jolting back from a blow: knocked back at once, then easing forward again, so it never
        // jumps
        let k = 1.0 - self.recoil;
        p.hand.y -= 0.035 * 6.75 * k * (1.0 - k) * (1.0 - k);
        p
    }

    /// Where a crossbow or gun in hand is held: low at the right hip, pointing on ahead, thrown
    /// back with its muzzle up as it goes off, and tipped about to be loaded, see reload_keys()
    fn shouldered(&self, weapon: &Weapon) -> Pose {
        let profile = &weapon.profile;
        let rest = match profile.build {
            // A hand crossbow held up in the left hand, pointing in a little towards the middle
            Build::Hand => pose([-0.21, 0.5, -0.2], [-0.03, -0.1, 0.99], [0.12, 1.0, 0.08]),
            // A bolt-thrower's broad frame held lower and nearer the middle
            Build::Fan => pose([0.15, 0.5, -0.26], [0.03, -0.1, 0.99], [-0.1, 1.0, 0.05]),
            // A siege crossbow carried by its grips, then lowered onto its leg as it's braced,
            // level and steady
            Build::Siege => blend(&pose([0.26, 0.4, -0.33], [0.06, -0.08, 0.99], [-0.06, 1.0, 0.06]), &pose([0.12, 0.5, -0.32], [0.02, -0.04, 1.0], [-0.07, 1.0, 0.03]), smooth(self.brace)),
            _ => pose([0.18, 0.47, -0.21], [0.04, -0.12, 0.99], [-0.12, 1.0, 0.06]),
        };
        match self.action {
            Action::Fire { t, .. } => {
                // Kicked back at once, then easing back to rest: a hand crossbow flicking up, a
                // braced siege crossbow hardly moving but for its nose. The kick carries on a
                // little past rest and back, a damped wobble, and twists it a little to one side,
                // the other side the next shot.
                let attack = 1.0 - (-t / 0.012).exp();
                let kick = kick_of(profile.build);
                let k = attack * (-t / 0.11).exp() * kick;
                let wobble = attack * (-t / 0.14).exp() * (t * 26.0).sin() * 0.3 * kick;
                let (back, lift, twist) = match profile.build {
                    Build::Hand => (0.5, 2.0, 0.6),
                    Build::Siege => (0.35, 0.6, 0.2),
                    Build::Pistol => (0.8, 1.5, 1.2),
                    Build::Cannon | Build::Blunderbuss => (1.2, 1.1, 0.8),
                    _ => (1.0, 1.0, 0.5),
                };
                let side = if self.rounds_held(weapon) % 2 == 0 { 1.0 } else { -1.0 };
                let mut p = rest;
                p.hand.y -= 0.06 * k * back - 0.008 * wobble;
                p.hand.z += 0.02 * k + 0.004 * wobble;
                p.hand.x += 0.008 * k * side * twist;
                p.turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 0.3 * k * lift - 0.04 * wobble) * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), 0.12 * k * side * twist) * p.turn;
                p
            }
            Action::Reload { t, length } => animate(&rest, &knack_keys(profile.build, profile.knack), t / length),
            // A hand crossbow goes through its moves on the left, as they would on the right
            Action::Special { t, length, special } => {
                let mut keys = special_move(special, Kind::Ranged, profile.style).1;
                if profile.build == Build::Hand {
                    for key in &mut keys {
                        key.1 = mirrored_shouldered(&key.1);
                    }
                }
                animate(&rest, &keys, t / length)
            }
            _ => rest,
        }
    }

    /// Shots loaded in the crossbow or gun in hand
    fn rounds_held(&self, weapon: &Weapon) -> u32 {
        self.held.map_or(0, |i| self.rounds_in(i, &weapon.profile))
    }

    /// How far the crossbow in hand is spanned, 0-1: spanned while it's loaded, the string snapping
    /// forward as it shoots, and drawn back again by the lever or windlass, or a repeater's lever
    /// between shots
    fn span(&self, weapon: &Weapon) -> f64 {
        let profile = &weapon.profile;
        let loaded = self.rounds_held(weapon) > 0;
        match self.action {
            Action::Reload { t, length } => spanning(profile.build, t / length).0,
            Action::Fire { t, length } if profile.build == Build::Repeater && loaded => smooth((t / length - 0.2) / 0.5),
            Action::Fire { .. } => 0.0,
            _ if loaded => 1.0,
            _ => 0.0,
        }
    }

    /// How the part that loads the crossbow or gun in hand is placed now, if it's to be drawn:
    /// turned so far round its axis, in radians, slid so far along it, in meters, and whether the
    /// other hand's working it
    fn mover_state(&self, weapon: &Weapon) -> Option<(f64, f64, bool)> {
        let build = weapon.profile.build;
        let loaded = self.rounds_held(weapon) > 0;
        match (build, self.action) {
            (b, Action::Reload { t, length }) if b.crossbow() && b != Build::Repeater => Some((spanning(build, t / length).1, 0.0, true)),
            (b, _) if b.crossbow() && b != Build::Repeater => Some((0.0, 0.0, false)),
            // Pulled back and pushed home between shots
            (Build::Repeater, Action::Fire { t, length }) if loaded => Some((-0.45 * (PI * (t / length).clamp(0.0, 1.0)).sin(), 0.0, true)),
            (Build::Repeater, Action::Reload { t, length }) => Some((spanning(build, t / length).1, 0.0, true)),
            (Build::Repeater, _) => Some((0.0, 0.0, true)),
            (_, Action::Reload { t, length }) => ramrod(t / length).map(|out| (0.0, out, true)),
            _ => None,
        }
    }

    /// The head and element of what's nocked on the bow or lying in the crossbow in hand: its
    /// special ammo's, or its own
    fn nocked_look(&self, weapon: &Weapon, heads: usize) -> (usize, usize) {
        let own = if weapon.shot() == Some(Shot::Arrow) { weapon.arrowhead() } else { 0 };
        let (head, element) = match self.nocked {
            Some((head, element)) => (head.unwrap_or(own), element),
            None => (own, weapon.profile.element),
        };
        (head.min(heads - 1), Element::look(element))
    }

    /// Where a point of the weapon in hand is in the world: the arrow's nock on a bow with
    /// `nock`, otherwise its tip, wherever a whip's lash has it
    fn held_point(&self, weapon: &Weapon, view: &View, nock: bool) -> Vector3<f64> {
        if !nock
            && weapon.look.lash.is_some()
            && let Some(lash) = &self.lash
        {
            return lash.tip();
        }
        self.held_at(weapon, view, if nock { self.nock(weapon) } else { weapon.look.tip })
    }

    /// Which of the weapon in hand's models to draw: how far a bow's drawn or a crossbow spanned,
    /// or whether a gun's hammer is cocked
    fn held_frame(&self, weapon: &Weapon) -> usize {
        let frames = weapon.look.frames.len();
        if frames <= 1 {
            return 0;
        }
        match weapon.profile.build {
            // A tome's open as it conjures
            Build::None if weapon.profile.style == Style::Summon => matches!(self.action, Action::Cast { .. } | Action::Special { .. }) as usize,
            Build::None => ((self.drawn(&weapon.profile) * (frames - 1) as f64).round() as usize).min(frames - 1),
            b if b.crossbow() => (self.span(weapon) * CROSSBOW_SPANNED as f64).round() as usize,
            _ => match self.action {
                Action::Reload { t, length } if t / length > 0.92 => GUN_COCKED,
                Action::Reload { .. } | Action::Fire { .. } => GUN_FIRED,
                _ if self.rounds_held(weapon) > 0 => GUN_COCKED,
                _ => GUN_FIRED,
            },
        }
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

    /// A line from `from` to `to`, sagging by `sag` of how long it is in the middle, `thick`
    /// meters thick, drawn as straight stretches of set `set` of the whip's lash models, rope or
    /// chain, each with where it starts
    fn line_instances(&self, from: Vector3<f64>, to: Vector3<f64>, sag: f64, set: usize, thick: f64) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = self.vpm;
        let length = (to - from).norm() / vpm;
        let n = ((length / 0.7).ceil() as usize).clamp(2, 40);
        let point = |k: usize| {
            let t = k as f64 / n as f64;
            from.lerp(&to, t) - Vector3::new(0.0, 0.0, 4.0 * t * (1.0 - t) * sag * length * vpm)
        };
        let mut out = Vec::with_capacity(n);
        for k in 0..n {
            let (a, b) = (point(k), point(k + 1));
            let meters = (b - a).norm() / vpm;
            if meters < 1e-4 {
                continue;
            }
            let rotation = UnitQuaternion::rotation_between(&Vector3::z(), &((b - a) / (meters * vpm))).unwrap_or_else(|| UnitQuaternion::from_axis_angle(&Vector3::x_axis(), PI));
            // The segment model nearest as thick as it should be, for how long the stretch is
            let ratio = thick / meters;
            let off = |k: usize| (LASH_TAPERS[k] / ratio).ln().abs();
            let taper = (0..LASH_TAPERS.len()).min_by(|&x, &y| off(x).total_cmp(&off(y))).unwrap_or(0);
            let (model, shape) = self.model_at(LASH_SEGMENTS + set * LASH_TAPERS.len() + taper);
            out.push((a, place(model, shape, LASH_RES, Vector3::zeros(), a, rotation, vpm, meters, MOVING)));
        }
        out
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

    /// What else to draw for the crossbow or gun in hand, its models from `first_model`, held at
    /// `hand` turned by `rotation`: the lever, cranks or ramrod that load it, the other hand
    /// working them or steadying it, the bolt lying in a crossbow's groove, and a gun's flash
    fn shouldered_instances(&self, weapon: &Weapon, first_model: u32, hand: Vector3<f64>, rotation: UnitQuaternion<f64>, view: &View, out: &mut Vec<AnimalInstance>) {
        let vpm = view.vpm;
        let look = &weapon.look;
        let at = |point: Vector3<f64>| hand + rotation * (point - look.grip) * vpm;
        let mut other = None;
        if let (Some(m), Some((turn, slide, held))) = (&look.mover, self.mover_state(weapon)) {
            let r = UnitQuaternion::from_axis_angle(&Unit::new_normalize(m.axis), turn);
            out.push(place(first_model + m.frame as u32, &look.frames[m.frame], look.res, m.pivot, at(m.pivot + m.axis * slide), rotation * r, vpm, 1.0, HELD));
            if held {
                other = Some((at(m.pivot + r * (m.handle - m.pivot) + m.axis * slide), rotation * r * m.handle_axis));
            }
        }
        // The other hand: the left for most, the right working a hand crossbow's lever
        let profile = &weapon.profile;
        if let Some((point, axis)) = other.or_else(|| look.support.map(|(point, axis)| (at(point), rotation * axis))) {
            let (which, elbow) = if profile.build == Build::Hand { (0, Vector3::new(0.25, 0.15, -0.65)) } else { (1, Vector3::new(-0.12, 0.15, -0.65)) };
            out.push(self.arm(which, point, axis, view.eye + view.rotation * elbow * vpm, vpm));
        }
        // A siege crossbow's leg, let down as it's braced
        if let Some((leg, down)) = &look.leg {
            let r = UnitQuaternion::from_axis_angle(&Unit::new_normalize(leg.axis), down * smooth(self.brace));
            out.push(place(first_model + leg.frame as u32, &look.frames[leg.frame], look.res, leg.pivot, at(leg.pivot), rotation * r, vpm, 1.0, HELD));
        }
        if profile.shot() == Some(Shot::Quarrel) {
            // The bolts, once it's spanned and loaded: one in each groove, a twin-prod's two and
            // a bolt-thrower's five, a harpoon its harpoon
            let placing = matches!(self.action, Action::Reload { t, length } if t / length > 0.9);
            if self.span(weapon) >= 0.99 && (self.rounds_held(weapon) > 0 || placing)
                && let Some(&nock) = look.nocks.get(CROSSBOW_SPANNED)
            {
                let (head, element) = self.nocked_look(weapon, QUARREL_HEADS);
                let head = if profile.build == Build::Harpoon { QUARREL_HARPOON } else { head };
                let (model, quarrel) = self.effect_model(SLOT_HELD_QUARREL + head, element);
                for &(offset, yaw) in &look.rails {
                    let turn = rotation * UnitQuaternion::from_axis_angle(&Vector3::z_axis(), yaw);
                    out.push(place(model, quarrel, HELD_ARROW_RES, Vector3::zeros(), at(nock + offset), turn, vpm, profile.bolt, HELD));
                }
            }
        } else if self.flare > 0.0 {
            // Flame bursting out of the muzzle, turned a different way each shot, bigger the
            // bigger the bore
            let roll = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.flare_roll);
            let size = (profile.caliber / 0.009).sqrt() * (0.6 + 0.5 * self.flare / FLARE);
            let (model, flash) = self.model_at(MUZZLE);
            out.push(place(model, flash, MUZZLE_RES, Vector3::zeros(), at(look.tip), rotation * roll, vpm, size, HELD));
        }
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
            // Where the elbows are, in the camera's frame
            let elbow = |x: f64, y: f64, z: f64| view.eye + view.rotation * Vector3::new(x, y, z) * vpm;
            let axis = rotation * look.hold;
            // Thrown, it spins end over end out there, the hand empty till it's back
            if let Some(t) = &self.thrown
                && t.t > 0.0
            {
                let across = t.direction.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                let lying = UnitQuaternion::rotation_between(&Vector3::z(), &t.direction).unwrap_or_else(UnitQuaternion::identity);
                let spin = UnitQuaternion::from_axis_angle(&Unit::new_normalize(across), self.time * 20.0) * lying;
                let middle = (look.grip + look.tip) * 0.5;
                out.push(place(first_model, &look.frames[0], look.res, middle, t.position, spin, vpm, 1.0, MOVING));
                out.push(self.arm(0, hand, axis, elbow(0.37, 0.12, -0.7), vpm));
                return out;
            }
            let frame = self.held_frame(weapon);
            // A bomb's gone from the hand once it's thrown, till the next comes up
            let thrown = matches!(self.action, Action::Throw { thrown: true, .. });
            if !thrown {
                out.push(place(first_model + frame as u32, &look.frames[frame], look.res, look.grip, hand, rotation, vpm, 1.0, HELD));
            }
            let build = weapon.profile.build;
            if build != Build::None {
                // A hand crossbow's held in the left hand, the rest in the right
                let off = build == Build::Hand;
                out.push(self.arm(off as usize, hand, axis, if off { elbow(-0.32, 0.12, -0.7) } else { elbow(0.37, 0.12, -0.7) }, vpm));
                self.shouldered_instances(weapon, first_model, hand, rotation, view, &mut out);
                return out;
            }
            // A bow or a tome in the left hand
            let left = (weapon.kind() == Kind::Ranged && weapon.profile.style != Style::Lob) || weapon.profile.style == Style::Summon;
            out.push(self.arm(left as usize, hand, axis, if left { elbow(-0.3, 0.12, -0.7) } else { elbow(0.37, 0.12, -0.7) }, vpm));
            // Gathering a big spell, the other hand takes the staff a little higher up, till it's
            // pushed out
            if let Action::Cast { t, length, .. } = self.action
                && weapon.profile.casting == Casting::Gather
                && matches!(weapon.profile.style, Style::Cast | Style::Flick)
            {
                let (gathers, release) = cast_times(Casting::Gather, weapon.profile.style);
                let f = t / length;
                if f > gathers * 0.45 && f < release + 0.05 {
                    let higher = self.held_at(weapon, view, look.grip.lerp(&look.tip, 0.12));
                    out.push(self.arm(1, higher, axis, elbow(-0.06, 0.1, -0.66), vpm));
                }
            }
            // The arrow on the string while drawing. The hand pulling it back is by the cheek, out
            // of sight.
            if matches!(self.action, Action::Draw { .. }) {
                let nock = hand + rotation * (self.nock(weapon) - look.grip) * vpm;
                let (head, element) = self.nocked_look(weapon, ARROW_HEADS);
                let (model, arrow) = self.effect_model(SLOT_HELD_ARROW + head, element);
                out.push(place(model, arrow, HELD_ARROW_RES, Vector3::zeros(), nock, rotation, vpm, 1.0, HELD));
            }
            // A whip's lash, a stretch at a time from one of its points to the next, each drawn
            // with the segment model as near as thick as it should be there for how long it is,
            // tapering to the cracker at its tip
            if let (Some(lash), Some(look)) = (&self.lash, &look.lash) {
                // A little thicker than it really is, so it still shows out at full stretch
                let thick = look.thickness.max(0.009) * 1.4;
                for i in 0..LASH_POINTS - 1 {
                    let (a, b) = (lash.points[i], lash.points[i + 1]);
                    let length = (b - a).norm();
                    if length < 1e-6 {
                        continue;
                    }
                    let rotation = UnitQuaternion::rotation_between(&Vector3::z(), &((b - a) / length)).unwrap_or_else(|| UnitQuaternion::from_axis_angle(&Vector3::x_axis(), PI));
                    let meters = length / vpm;
                    let index = if i == LASH_POINTS - 2 {
                        LASH_CRACKERS + look.cracker
                    } else {
                        let u = (i as f64 + 0.5) / (LASH_POINTS - 1) as f64;
                        let ratio = thick * (1.0 - (1.0 - LASH_THIN) * u) / meters;
                        let off = |k: usize| (LASH_TAPERS[k] / ratio).ln().abs();
                        let taper = (0..LASH_TAPERS.len()).min_by(|&x, &y| off(x).total_cmp(&off(y))).unwrap_or(0);
                        LASH_SEGMENTS + look.set * LASH_TAPERS.len() + taper
                    };
                    let (model, shape) = self.model_at(index);
                    out.push(place(model, shape, LASH_RES, Vector3::zeros(), a, rotation, vpm, meters, HELD));
                }
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
                let instance = match p.shot {
                    // A thornburst's thorns, point first
                    _ if p.trick == Trick::Thorn => {
                        let (model, thorn) = self.model_at(THORN);
                        place(model, thorn, MODEL_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), p.position, rotation, vpm, THORN_SIZE * 0.5, MOVING)
                    }
                    // A burst's shards, each a glowing streak along the way it's flying, smaller up
                    // close so they don't fill the view
                    _ if p.trick == Trick::Fragment => {
                        let (model, streak) = self.effect_model(SLOT_BOLT, look);
                        let near = ((p.position - view.eye).norm() / vpm / 4.0).clamp(0.3, 1.0);
                        place(model, streak, BOLT_RES, Vector3::zeros(), p.position, rotation, vpm, p.size * near, MOVING)
                    }
                    // A bursting grenade, an iron ball with its fuse burning, tumbling
                    Shot::Ball if matches!(p.trick, Trick::Burst { .. }) => {
                        let spin = rotation * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.time * 9.0);
                        let (model, shell) = self.model_at(SHELL);
                        place(model, shell, SHOT_RES, Vector3::zeros(), p.position, spin, vpm, p.size.max(0.8), MOVING)
                    }
                    Shot::Arrow => {
                        let (model, arrow) = self.effect_model(SLOT_ARROW + p.head, look);
                        place(model, arrow, ARROW_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), p.position, rotation, vpm, 1.0, MOVING)
                    }
                    Shot::Quarrel => {
                        let (model, quarrel) = self.effect_model(SLOT_QUARREL + p.head.min(QUARREL_HEADS - 1), look);
                        place(model, quarrel, ARROW_RES, Vector3::new(QUARREL_HEAD, 0.0, 0.0), p.position, rotation, vpm, p.size, MOVING)
                    }
                    Shot::Ball => {
                        let (model, ball) = self.effect_model(SLOT_SHOT, look);
                        place(model, ball, SHOT_RES, Vector3::zeros(), p.position, rotation, vpm, p.size, MOVING)
                    }
                    // A bomb tumbling as it flies, lying still once it's come to rest
                    Shot::Shell if matches!(p.trick, Trick::Bomb { .. }) => {
                        let rolling = if p.velocity.norm() > 1e-6 { self.time * 7.0 } else { 0.0 };
                        let spin = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), rolling) * UnitQuaternion::from_axis_angle(&Vector3::z_axis(), p.position.x * 0.3);
                        let (model, bomb) = self.effect_model(SLOT_BOMB, look);
                        place(model, bomb, BOMB_RES, Vector3::zeros(), p.position, spin, vpm, p.size, MOVING)
                    }
                    // Tumbling as it goes
                    Shot::Shell => {
                        let spin = rotation * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.time * 9.0);
                        let (model, shell) = self.model_at(SHELL);
                        place(model, shell, SHOT_RES, Vector3::zeros(), p.position, spin, vpm, p.size, MOVING)
                    }
                };
                add(p.position, instance);
            } else if matches!(p.form, Form::Fireball | Form::Orb | Form::Hex) {
                // A ball of fire or darkness churning as it goes, or a hex, a knot of it
                let spin = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.time * 6.0) * rotation;
                let (look, size) = match p.form {
                    Form::Hex => (Element::look(Some(Element::Void)), 0.07),
                    Form::Orb => (look, 0.13),
                    // A comet's far bigger than a fireball
                    _ => (look, 0.15 * p.size),
                };
                let (model, ball) = self.effect_model(SLOT_FIREBALL, look);
                add(p.position, place(model, ball, MODEL_RES, Vector3::zeros(), p.position, spin, vpm, size, MOVING));
            } else if p.form != Form::Lightning {
                let (model, bolt) = self.effect_model(SLOT_BOLT, look);
                add(p.position, place(model, bolt, BOLT_RES, Vector3::zeros(), p.position, rotation, vpm, p.size, MOVING));
            }
        }
        for s in &self.stuck {
            let instance = match s.head {
                Some(head) if s.quarrel => {
                    let (model, quarrel) = self.effect_model(SLOT_QUARREL + head.min(QUARREL_HEADS - 1), s.look);
                    place(model, quarrel, ARROW_RES, Vector3::new(QUARREL_HEAD, 0.0, 0.0), s.position, s.rotation, vpm, 1.0, STILL)
                }
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
                HostileKind::Bolt => {
                    let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &h.velocity.normalize()).unwrap_or_else(UnitQuaternion::identity);
                    let (model, quarrel) = self.effect_model(SLOT_QUARREL, Element::look(None));
                    place(model, quarrel, ARROW_RES, Vector3::new(QUARREL_HEAD, 0.0, 0.0), h.position, rotation, vpm, 1.0, MOVING)
                }
            };
            add(h.position, instance);
        }
        // Familiars, glowing orbs bobbing where they hang, and phantoms, pillars of light
        for f in &self.familiars {
            let bob = Vector3::new(0.0, 0.0, (self.time * 2.5 + f.position.x).sin() * 0.08 * vpm);
            let fade = (f.life / 0.4).min(1.0) * ((FAMILIAR_SECONDS - f.life) / 0.25).min(1.0);
            let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 3.0);
            let look = Element::look(if f.phantom { Some(Element::Holy) } else { f.profile.element.or(Some(Element::Arcane)) });
            let (model, ball) = self.effect_model(SLOT_FIREBALL, look);
            add(f.position, place(model, ball, MODEL_RES, Vector3::zeros(), f.position + bob, spin, vpm, (if f.phantom { 0.16 } else { 0.11 }) * fade.max(0.05), MOVING));
            if f.phantom {
                let (model, pillar) = self.model_at(PILLAR);
                let foot = f.position - Vector3::new(0.0, 0.0, 1.6 * vpm);
                add(foot, place(model, pillar, MODEL_RES, Vector3::zeros(), foot, UnitQuaternion::identity(), vpm, 0.3 * fade.max(0.05), MOVING));
            }
        }
        // Spectral blades going round, edge first
        if let Some(o) = &self.orbit {
            let feet = view.eye - Vector3::new(0.0, 0.0, 0.5 * vpm);
            let look = Element::look(o.profile.element.or(Some(Element::Arcane)));
            let (model, wisp) = self.effect_model(SLOT_BOLT, look);
            for k in 0..6 {
                let a = o.angle + k as f64 / 6.0 * TAU;
                let at = feet + Vector3::new(a.cos(), a.sin(), 0.0) * 2.2 * vpm;
                let rotation = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), a + PI * 0.5);
                add(at, place(model, wisp, BOLT_RES, Vector3::zeros(), at, rotation, vpm, 0.9, MOVING));
            }
        }
        // A harpoon's line, paying out after it sagging a little, pulled taut once it bites
        if let Some(from) = self.line_from {
            let end = self.projectiles.iter().find(|p| p.trick == Trick::Harpoon).map(|p| (p.position, 0.06)).or_else(|| self.tether.as_ref().map(|t| (t.at, 0.01)));
            if let Some((to, sag)) = end {
                for (at, instance) in self.line_instances(from, to, sag, LASH_HEMP, 0.028) {
                    add(at, instance);
                }
            }
        }
        // A shackle's chains and Blood Moon's moon and shots, ahead of everything else flying about,
        // as there are only ever so many of them, so they're never what's left out
        if let Some(s) = &self.shackle {
            for instance in self.shackle_instances(s, view) {
                add(view.eye, instance);
            }
        }
        if let Some(m) = &self.blood_moon {
            for instance in self.blood_moon_instances(m, view) {
                add(view.eye, instance);
            }
        }
        // Nets over what's rooted, and a net bolt's net folded up as it flies
        let (model, net) = self.model_at(NET);
        for &(at, size) in &self.snared {
            add(at, place(model, net, RING_RES, Vector3::zeros(), at + Vector3::new(0.0, 0.0, size * 0.25 * vpm), UnitQuaternion::identity(), vpm, size, MOVING));
        }
        for p in self.projectiles.iter().filter(|p| matches!(p.trick, Trick::Net(_))) {
            let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 8.0);
            add(p.position, place(model, net, RING_RES, Vector3::zeros(), p.position, spin, vpm, 0.3, MOVING));
        }
        // Beams drawn solid, glowing bolts end to end along them
        for &(from, to, look) in &self.rays {
            let length = (to - from).norm();
            let n = ((length / (0.4 * vpm)).ceil() as usize).clamp(1, 50);
            let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &((to - from) / length.max(1e-6))).unwrap_or_else(UnitQuaternion::identity);
            let (model, bolt) = self.effect_model(SLOT_BOLT, look);
            for k in 1..=n {
                let at = from.lerp(&to, k as f64 / n as f64);
                add(at, place(model, bolt, BOLT_RES, Vector3::zeros(), at, rotation, vpm, 0.55, MOVING));
            }
        }
        // Runes waiting on the ground, pulsing once they're armed
        let (model, rune) = self.model_at(RUNE);
        for m in &self.mines {
            let size = if m.arming > 0.0 { 0.6 } else { 1.1 + 0.08 * (self.time * 5.0 + m.position.x).sin() };
            let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 0.6 + m.position.y);
            add(m.position, place(model, rune, RING_RES, Vector3::zeros(), m.position, turn, vpm, size, MOVING));
        }
        // Pools lying on the ground, spreading out as they're left and drying up as they go
        for l in self.lingering.iter().filter(|l| l.pool) {
            let grow = (l.life / 0.8).min(1.0).sqrt();
            let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), l.position.x * 0.13 + l.position.y * 0.07);
            let (model, puddle) = self.effect_model(SLOT_PUDDLE, Element::look(Some(l.element)));
            let at = l.position + Vector3::new(0.0, 0.0, 0.06 * vpm);
            add(at, place(model, puddle, PUDDLE_RES, Vector3::zeros(), at, turn, vpm, l.radius * grow, MOVING));
        }
        // Shockwaves rolling out, rubble heaved up along their fronts
        let (model, wave) = self.model_at(WAVE);
        for r in self.ripples.iter().filter(|r| r.radius > 0.6) {
            let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), r.center.x * 0.11 + r.radius * 0.05);
            let at = r.center + Vector3::new(0.0, 0.0, 0.05 * vpm);
            add(at, place(model, wave, RING_RES, Vector3::zeros(), at, turn, vpm, r.radius, MOVING));
        }
        // Special attacks going on a while
        for z in &self.zones {
            let age = z.lifetime - z.life;
            // Swelling up as it starts and dying away as it ends
            let grow = (age / 0.35).min(1.0) * (z.life / 0.35).min(1.0);
            match z.kind {
                // A bog's mud lying over the ground, venom welling up in the middle of it, turning
                // slowly
                ZoneKind::Mire => {
                    for (k, look, s) in [(0.0, Element::look(None), 1.0), (1.3, Element::look(Some(Element::Poison)), 0.4)] {
                        let (model, puddle) = self.effect_model(SLOT_PUDDLE, look);
                        let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), k + self.time * 0.1);
                        let at = z.position + Vector3::new(0.0, 0.0, (0.06 + 0.03 * k) * vpm);
                        add(at, place(model, puddle, PUDDLE_RES, Vector3::zeros(), at, turn, vpm, z.radius * s * grow.max(0.05), MOVING));
                    }
                }
                // A sun, white-hot, churning and pulsing
                ZoneKind::Sun => {
                    let pulse = 1.0 + 0.08 * (self.time * 9.0).sin();
                    let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 1.5) * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.time * 0.7);
                    let (model, ball) = self.effect_model(SLOT_FIREBALL, Element::look(Some(Element::Holy)));
                    add(z.position, place(model, ball, MODEL_RES, Vector3::zeros(), z.position, spin, vpm, 0.6 * pulse * grow.max(0.05), MOVING));
                }
                // A whirlwind turning fast, a little wider as it goes on
                ZoneKind::Cyclone => {
                    let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 7.0);
                    let (model, twister) = self.model_at(TWISTER);
                    add(z.position, place(model, twister, MODEL_RES, Vector3::zeros(), z.position, turn, vpm, (0.8 + 0.2 * (age / z.lifetime)) * grow.max(0.05), MOVING));
                }
                // A crackling ball, jittering
                ZoneKind::Ball => {
                    let jitter = Vector3::new((self.time * 41.0).sin(), (self.time * 37.0).cos(), (self.time * 29.0).sin()) * 0.05 * vpm;
                    let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 11.0);
                    let (model, ball) = self.effect_model(SLOT_FIREBALL, Element::look(Some(Element::Storm)));
                    add(z.position, place(model, ball, MODEL_RES, Vector3::zeros(), z.position + jitter, spin, vpm, 0.32 * grow.max(0.05), MOVING));
                }
                // A boulder rolling along the way it's going
                ZoneKind::Boulder => {
                    let ahead = z.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
                    let across = Unit::new_normalize(ahead.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::y));
                    let rolled = (z.velocity.norm() / vpm) * age / z.radius;
                    let spin = UnitQuaternion::from_axis_angle(&across, -rolled);
                    let (model, boulder) = self.model_at(BOULDER);
                    add(z.position, place(model, boulder, MODEL_RES, Vector3::zeros(), z.position, spin, vpm, z.radius * 2.3, MOVING));
                }
                // A crescent of light, its horns trailing, once it's been cut loose: glowing
                // wisps end to end round its curve
                ZoneKind::Crescent if z.tick <= 0.0 => {
                    let ahead = z.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
                    let side = ahead.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::y);
                    let look = Element::look(z.profile.element.or(Some(Element::Holy)));
                    let (model, bolt) = self.effect_model(SLOT_BOLT, look);
                    let fade = (z.life / 0.25).min(1.0);
                    let r = z.radius * vpm;
                    // A crescent moon on its back, its horns swept up and back, thick in the
                    // middle and thinning to points
                    for j in 0..31 {
                        let s = j as f64 / 30.0 * 2.0 - 1.0;
                        let at = z.position + side * s * r - ahead * s * s * 0.35 * r + Vector3::new(0.0, 0.0, (s * s * 0.4 - 0.1) * r);
                        // Never drawn up against the eye, filling the view, whatever's happened
                        // to the player since it was cut loose
                        let clear = (((at - view.eye).norm() / vpm - 0.7) / 0.8).clamp(0.0, 1.0);
                        if clear <= 0.0 {
                            continue;
                        }
                        let tangent = side - ahead * 0.7 * s + Vector3::z() * 0.8 * s;
                        let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &tangent.normalize()).unwrap_or_else(UnitQuaternion::identity);
                        add(at, place(model, bolt, BOLT_RES, Vector3::zeros(), at, rotation, vpm, (0.8 - 0.5 * s * s) * fade.max(0.1) * clear, MOVING));
                    }
                }
                // A keg on the ground, a hoop of iron round it, swelling a little as it's about to go
                ZoneKind::Keg if age >= KEG_SET => {
                    let late = (1.0 - z.life / 0.5).max(0.0);
                    let swell = 1.0 + 0.15 * late * (self.time * 30.0).sin().abs();
                    let (model, bomb) = self.effect_model(SLOT_BOMB, Element::look(z.profile.element));
                    let at = z.position + Vector3::new(0.0, 0.0, 0.42 * vpm);
                    add(at, place(model, bomb, BOMB_RES, Vector3::zeros(), at, UnitQuaternion::from_axis_angle(&Vector3::z_axis(), 0.6), vpm, 3.2 * swell, MOVING));
                }
                // Not cut loose or set down yet
                ZoneKind::Crescent | ZoneKind::Keg => {}
                // The head going round on its chain, from the hand out to it
                ZoneKind::Orbit => {
                    let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 9.0) * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.time * 5.0);
                    let (model, bomb) = self.effect_model(SLOT_BOMB, Element::look(z.profile.element));
                    add(z.position, place(model, bomb, BOMB_RES, Vector3::zeros(), z.position, spin, vpm, 2.6 * grow.max(0.05), MOVING));
                    let hand = view.eye + view.rotation * Vector3::new(0.2, 0.45, -0.3) * vpm;
                    for (at, instance) in self.line_instances(hand, z.position, 0.02, LASH_CHAIN, 0.04) {
                        add(at, instance);
                    }
                }
                // A burning brand's line: a puddle of fire on each stretch once it's caught,
                // flickering, dying down as it burns out
                ZoneKind::Brand => {
                    let (model, puddle) = self.effect_model(SLOT_PUDDLE, Element::look(Some(Element::Fire)));
                    let fade = (z.life / 0.6).min(1.0);
                    for (k, w) in z.path.windows(2).enumerate() {
                        if age < 0.38 + 0.045 * k as f64 {
                            continue;
                        }
                        let at = w[0].lerp(&w[1], 0.5) + Vector3::new(0.0, 0.0, 0.06 * vpm);
                        let flicker = 1.0 + 0.12 * (self.time * 11.0 + k as f64 * 1.7).sin();
                        let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), k as f64 * 0.9);
                        add(at, place(model, puddle, PUDDLE_RES, Vector3::zeros(), at, turn, vpm, 0.75 * BRAND_WIDTH * flicker * fade.max(0.1), MOVING));
                    }
                }
                ZoneKind::Storm | ZoneKind::Swarm => {}
            }
        }
        // Coils of lash wound round what a coil bind holds, turning slowly and squeezing in
        if !self.binds.is_empty() {
            let (model, coil) = self.model_at(COIL);
            for b in &self.binds {
                let squeeze = 1.0 - 0.08 * (1.0 - (b.tick / BIND_SQUEEZE).clamp(0.0, 1.0)).powi(3);
                let grow = ((BIND_SECONDS - b.life) / 0.2).min(1.0) * (b.life / 0.3).min(1.0);
                let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 0.8 + b.at.x * 0.1);
                let at = b.at - Vector3::new(0.0, 0.0, b.girth * 0.5 * vpm);
                add(at, place(model, coil, RING_RES, Vector3::zeros(), at, turn, vpm, 2.0 * (b.girth + 0.12) * squeeze * grow.max(0.05), MOVING));
            }
        }
        // A serpent lash's strands, a snake's head at the front of each, weaving along
        if !self.serpents.is_empty() {
            let (model, head) = self.model_at(SERPENT);
            for s in &self.serpents {
                let at = Combat::serpent_at(s, vpm);
                let way = (s.heading + Vector3::new(-s.heading.y, s.heading.x, 0.0) * s.phase.cos() * 0.35).normalize();
                let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &way).unwrap_or_else(UnitQuaternion::identity);
                add(at, place(model, head, MODEL_RES, Vector3::zeros(), at, rotation, vpm, 1.5 * (s.life / 0.3).min(1.0).max(0.1), MOVING));
            }
        }
        // Marks hanging over what a hunter's mark is on, turning
        if !self.marked_at.is_empty() {
            let (model, ring) = self.model_at(RING);
            let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 2.0);
            let bob = Vector3::new(0.0, 0.0, (self.time * 3.0).sin() * 0.08 * vpm);
            for &at in &self.marked_at {
                add(at, place(model, ring, RING_RES, Vector3::zeros(), at + bob, turn, vpm, 0.45, MOVING));
            }
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
            let (index, res, size, turn) = match i.model {
                // Thrusting out past full size and back, holding, then cracking across and
                // shattering, see effects::ice()
                ICE_BURST => {
                    let thrust = if t < 0.1 {
                        let k = t / 0.1;
                        1.0 + 2.7 * (k - 1.0).powi(3) + 1.7 * (k - 1.0).powi(2)
                    } else {
                        1.0 - 0.12 * ((t - 0.6) / 0.4).max(0.0)
                    };
                    let stage = if t < 0.6 { 0 } else if t < 0.8 { 1 } else { 2 };
                    (ICE_STAGES + stage, ICE_RES, thrust, blast_yaw(i.position))
                }
                // Blazing, flickering, then thinning away, see effects::pillar()
                PILLAR => {
                    let flicker = 1.0 + 0.07 * (self.time * 41.0 + i.position.x).sin();
                    (PILLAR_STAGES + (t > 0.55) as usize, PILLAR_RES, grow.sqrt() * flicker, self.time * 1.5)
                }
                _ => (i.model, MODEL_RES, grow, 0.0),
            };
            let (model, shape) = self.model_at(index);
            let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), turn);
            add(i.position, place(model, shape, res, Vector3::zeros(), i.position, spin, vpm, i.radius * 0.9 * size.max(0.05), MOVING));
        }
        // What goes off right in front of the eye is drawn smaller, so it doesn't fill the view:
        // full size `reach` meters off, down to `least` of it up close
        let near = |at: Vector3<f64>, reach: f64, least: f64| ((at - view.eye).norm() / (reach * vpm)).clamp(least, 1.0);
        for s in self.sparks.iter().filter(|s| s.slot != SLOT_FINE) {
            // A spark flying fast is a streak along the way it's going, its head white-hot, see
            // effects::streak(); one hanging still, as along a bolt of lightning, a speck
            if s.slot == SLOT_SPARK && s.look < ELEMENT_LOOKS && s.velocity.norm() > 0.8 * vpm {
                let (model, streak) = self.model_at(STREAKS + s.look);
                let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &s.velocity).unwrap_or_else(UnitQuaternion::identity);
                let size = (s.life / s.lifetime).clamp(0.3, 1.0) * near(s.position, 2.5, 0.12);
                add(s.position, place(model, streak, STREAK_RES, Vector3::zeros(), s.position, rotation, vpm, size, MOVING));
                continue;
            }
            let (model, spark) = self.effect_model(s.slot, s.look);
            // Drops stay whole till they land, sparks dwindle, smoke billows out and thins away
            let age = 1.0 - s.life / s.lifetime;
            let (size, res) = match s.slot {
                SLOT_DROP => (1.0, SPARK_RES),
                SLOT_SMOKE => ((0.4 + 1.3 * age.sqrt()) * (1.0 - age.powi(3)).max(0.15), SMOKE_RES),
                // Billowing out, then thinning away
                SLOT_CLOUD => ((0.45 + 0.65 * age.sqrt()) * (1.0 - age.powi(3)).max(0.1), CLOUD_RES),
                // A streak of light, a good deal bigger than a spark, dwindling as it goes
                SLOT_BOLT => (2.2 * (s.life / s.lifetime).clamp(0.35, 1.0), BOLT_RES),
                _ => ((s.life / s.lifetime).clamp(0.2, 1.0), SPARK_RES),
            };
            // A streak points the way it's going, its tail trailing
            let rotation = if s.slot == SLOT_BOLT { UnitQuaternion::rotation_between(&Vector3::x(), &s.velocity).unwrap_or_else(UnitQuaternion::identity) } else { UnitQuaternion::identity() };
            // Sparks and drops a voxel across shrink hard right in front of the eye, rather than
            // showing as flat squares of color across the view
            let shrink = if matches!(s.slot, SLOT_SPARK | SLOT_DROP) { near(s.position, 2.5, 0.12) } else { near(s.position, 1.5, 0.3) };
            add(s.position, place(model, spark, res, Vector3::zeros(), s.position, rotation, vpm, size * shrink, MOVING));
        }
        // Explosions, each drawn in stages baked ahead of time, see effects::blast(): a white-hot
        // core, a fireball billowing out, breaking up and dissolving into smoke, each element its
        // own way, swelling fast to its full size and on a little as it breaks up, turning slowly.
        // Never so big up close that it reaches the eye and fills the view: the model reaches
        // BLAST_REACH meters from its middle at size 1, so it's kept half a meter short of the eye.
        let blast_size = |at: Vector3<f64>, radius: f64, swell: f64| (radius * 0.8 * swell * near(at, 4.0, 0.35)).min((((at - view.eye).norm() / vpm - 0.5) / BLAST_REACH).max(0.03));
        let storm = Element::look(Some(Element::Storm));
        let mut rings = 0;
        let mut extras = Vec::new();
        for (k, b) in self.blasts.iter().enumerate().rev() {
            let t = 1.0 - b.life / b.lifetime;
            let (model, shape) = self.model_at(BLASTS + b.look.min(ELEMENT_LOOKS - 1) * BLAST_STAGES + blast_stage(b.look, t));
            let swell = 0.62 + 0.38 * smooth(t / 0.22) + 0.1 * t;
            // Lightning flickers, turning every which way each frame
            let yaw = blast_yaw(b.position) + if b.look == storm { (self.time * 60.0).floor() * 2.4 } else { t * 0.7 };
            let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), yaw);
            add(b.position, place(model, shape, BLAST_RES, Vector3::zeros(), b.position, spin, vpm, blast_size(b.position, b.radius, swell), MOVING));
            // Going off by the ground, a ring of what it throws up rolls out over it, settling
            if let Some(&height) = self.blast_floors.get(k)
                && b.radius >= 1.0
                && t < 0.85
                && rings < MOST_RINGS
                && height < (b.radius * 0.8 + 0.6) * vpm
            {
                rings += 1;
                let spread = b.radius * (0.35 + 1.25 * (1.0 - (1.0 - t / 0.85).powi(3)));
                let sink = ((t - 0.5) / 0.35).max(0.0) * 0.06 * spread * vpm;
                let at = b.position - Vector3::new(0.0, 0.0, height + sink);
                let (model, ring) = self.model_at(BLAST_RINGS + b.look.min(ELEMENT_LOOKS - 1));
                extras.push((at, place(model, ring, BLAST_RING_RES, Vector3::zeros(), at, spin, vpm, spread, MOVING)));
            }
        }
        // Their smoke drifting up and thinning away after them, the newest first
        for r in self.remnants.iter().rev() {
            let t = 1.0 - r.life / r.lifetime;
            let (model, shape) = self.model_at(BLASTS + r.look.min(ELEMENT_LOOKS - 1) * BLAST_STAGES + BLAST_LIVE + (t > 0.5) as usize);
            let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), r.yaw + 0.7 + 0.3 * t);
            extras.push((r.position, place(model, shape, BLAST_RES, Vector3::zeros(), r.position, spin, vpm, blast_size(r.position, r.radius, 1.1 + 0.12 * t), MOVING)));
        }
        // Rings and smoke are only extra, so they go after everything else: the first left out
        // when there's too much to draw, and the last to cast shadows
        out.extend(extras.into_iter().map(|(at, instance)| (1e9 + (at - view.eye).norm(), instance)));
        out
    }

    /// What to draw for the fine sparks trailing off the weapon in hand. They don't need to cast
    /// shadows, so they can go last with the leaves.
    pub fn fine_instances(&self, view: &View) -> Vec<AnimalInstance> {
        // The glints of a rare blade's arc first, newest first, as there are few and they're what
        // the eye follows
        let glints = self.glints.iter().rev().map(|g| {
            let (model, spark) = self.effect_model(SLOT_FINE, g.look);
            let size = GLINT_SIZE * (g.life / GLINT_LIFE).clamp(0.35, 1.0);
            place(model, spark, FINE_SPARK_RES, Vector3::zeros(), g.position, UnitQuaternion::identity(), view.vpm, size, MOVING)
        });
        let sparks = self.sparks.iter().filter(|s| s.slot == SLOT_FINE).map(|s| {
            let (model, spark) = self.effect_model(SLOT_FINE, s.look);
            let size = (s.life / s.lifetime).clamp(0.3, 1.0);
            place(model, spark, FINE_SPARK_RES, Vector3::zeros(), s.position, UnitQuaternion::identity(), view.vpm, size, MOVING)
        });
        glints.chain(sparks).collect()
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
        // A flash bursts white-hot, settles into its own color and dies away fast, then lingers a
        // little; lightning's flickers
        let flicker = 0.55 + 0.45 * (hash((self.time * 60.0) as u32) >> 8) as f64 / 16777216.0;
        for f in &self.flashes {
            let k = (f.life / f.lifetime).clamp(0.0, 1.0);
            let white = 0.55 * smooth((k - 0.7) / 0.3) as f32;
            let color = f.color.map(|c| c + (1.0 - c) * white);
            let strength = (3.4 * k.powf(1.6) + 0.25 * k) * if f.color == Element::Storm.light() { flicker } else { 1.0 };
            lights.push(light(f.position, f.reach * (0.85 + 0.15 * k), color, strength as f32));
        }
        // Blood Moon's red light over everything round the player
        let moon = self.blood_moon();
        if moon > 0.0 {
            lights.push(light(view.eye + Vector3::new(0.0, 0.0, 4.0 * vpm), 30.0, Element::Blood.light(), 1.6 * moon as f32));
        }
        for p in &self.projectiles {
            if !p.arrow {
                lights.push(light(p.position, 6.0, p.profile.element.map_or([0.5, 0.9, 1.0], Element::light), 1.2));
            } else if p.shot == Shot::Shell {
                lights.push(light(p.position, 3.0, Element::Fire.light(), 0.6));
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
        for m in self.mines.iter().filter(|m| m.arming <= 0.0) {
            lights.push(light(m.position + Vector3::new(0.0, 0.0, 0.3 * vpm), 2.5, Element::Arcane.light(), 0.35));
        }
        for f in &self.familiars {
            let color = if f.phantom { Element::Holy.light() } else { f.profile.element.map_or([0.7, 0.5, 1.0], Element::light) };
            lights.push(light(f.position, 3.5, color, 0.6));
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
        // An epic or rarer blade's glow lighting what's round it as it's swung, in its rarity's
        // own color
        if let Some(weapon) = weapon
            && self.glow > 0.0
            && weapon.kind() != Kind::Magic
        {
            let tip = self.held_point(weapon, view, false);
            let color = weapon.profile.color.map(|c| c as f32 / 255.0);
            lights.push(light(tip, 2.5, color, 0.45 * (self.glow / 0.2).min(1.0) as f32));
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

    /// Shots loaded in crossbow or gun `index`, and how many it holds, for the HUD
    pub fn rounds(&self, index: usize, weapon: &Weapon) -> (u32, u32) {
        (self.rounds_in(index, &weapon.profile), weapon.profile.magazine())
    }

    /// How far through loading the crossbow or gun in hand is, 0-1, if it's being loaded
    pub fn reloading(&self) -> Option<f64> {
        match self.action {
            Action::Reload { t, length } => Some((t / length).min(1.0)),
            _ => None,
        }
    }

    /// The weapons were numbered again: what's loaded in each crossbow and gun moves along with
    /// it to where `to` says it is now, or is forgotten with it
    pub fn renumber(&mut self, to: impl Fn(usize) -> Option<usize>) {
        self.loads = self.loads.drain().filter_map(|(i, n)| Some((to(i)?, n))).collect();
    }

    /// A bundle of special ammo lying on the ground at `at`, in voxels, facing `heading`: a few
    /// arrows or bolts side by side, or a little pile of shot, of `element`
    pub fn bundle_instances(&self, shot: Shot, head: usize, element: Option<Element>, at: Vector3<f64>, heading: f64) -> Vec<AnimalInstance> {
        let vpm = self.vpm;
        let look = Element::look(element);
        let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), heading);
        let side = turn * Vector3::y();
        let lift = Vector3::new(0.0, 0.0, 0.04 * vpm);
        match shot {
            Shot::Arrow | Shot::Quarrel => (0..3)
                .map(|k| {
                    let at = at + lift + side * (k as f64 - 1.0) * 0.05 * vpm;
                    let (slot, point) = if shot == Shot::Arrow { (SLOT_ARROW + head.min(ARROW_HEADS - 1), 0.34) } else { (SLOT_QUARREL + head.min(QUARREL_HEADS - 1), QUARREL_HEAD * 0.5) };
                    let (model, arrow) = self.effect_model(slot, look);
                    place(model, arrow, ARROW_RES, Vector3::new(point, 0.0, 0.0), at, turn, vpm, 1.0, MOVING)
                })
                .collect(),
            _ => [(0.0, 0.0, 0.0), (0.03, 0.0, 0.0), (-0.015, 0.026, 0.0), (-0.015, -0.026, 0.0), (0.005, 0.009, 0.026)]
                .into_iter()
                .map(|(x, y, z)| {
                    let (model, ball) = self.effect_model(SLOT_SHOT, look);
                    place(model, ball, SHOT_RES, Vector3::zeros(), at + lift + turn * Vector3::new(x, y, z) * vpm, turn, vpm, 1.0, MOVING)
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STYLES: [Style; 8] = [Style::Slash, Style::Chop, Style::Reap, Style::Smash, Style::Stab, Style::Thrust, Style::Lash, Style::Whirl];
    const FINISHES: [Finish; 4] = [Finish::Lunge, Finish::Rising, Finish::Spin, Finish::Slam];
    const CASTINGS: [Casting; 7] = [Casting::Thrust, Casting::Flick, Casting::Gather, Casting::Overhead, Casting::Sweep, Casting::Scoop, Casting::Present];
    /// Each family's own heavy blow, with the style it's swung in
    const HEAVIES: [(Style, Heavy); 9] = [
        (Style::Thrust, Heavy::Lunge),
        (Style::Slash, Heavy::Draw),
        (Style::Slash, Heavy::Spin),
        (Style::Chop, Heavy::Hew),
        (Style::Reap, Heavy::Sweep),
        (Style::Reap, Heavy::Twirl),
        (Style::Thrust, Heavy::Cleave),
        (Style::Smash, Heavy::Heave),
        (Style::Whirl, Heavy::Overhead),
    ];
    const SPECIALS: [Special; 75] = [
        Special::LassoPull,
        Special::CoilBind,
        Special::ThunderCrack,
        Special::SerpentLash,
        Special::TripSweep,
        Special::BurningBrand,
        Special::ChainSwing,
        Special::Firecrackers,
        Special::Thornburst,
        Special::CrescentWave,
        Special::ImpalingCharge,
        Special::CoilingLash,
        Special::Flay,
        Special::Quickdraw,
        Special::HuntersMark,
        Special::Reckoning,
        Special::Grapeshot,
        Special::Skyburst,
        Special::PowderKeg,
        Special::StonePillars,
        Special::WreckingOrbit,
        Special::Earthquake,
        Special::Comet,
        Special::ClusterBomb,
        Special::Thunderstorm,
        Special::Mire,
        Special::Sunburst,
        Special::Shatterwave,
        Special::Cyclone,
        Special::Eruption,
        Special::BallLightning,
        Special::Swarm,
        Special::Avalanche,
        Special::BoltRain,
        Special::Ricochet,
        Special::SplitBolt,
        Special::NetBolt,
        Special::Shackle,
        Special::ChainBlast,
        Special::Harvest,
        Special::BattleCry,
        Special::HurledBlade,
        Special::Prism,
        Special::RuneMines,
        Special::Wildfire,
        Special::FrostNova,
        Special::ThunderWeb,
        Special::BlinkStrike,
        Special::SpectralBlades,
        Special::PhantomHost,
        Special::SoulSiphon,
        Special::Stasis,
        Special::Ballista,
        Special::SeekerSwarm,
        Special::BladeStorm,
        Special::Barrage,
        Special::Firewall,
        Special::Meteors,
        Special::Blizzard,
        Special::Tempest,
        Special::Singularity,
        Special::Fissure,
        Special::Whirlwind,
        Special::Flurry,
        Special::Nova,
        Special::Rain,
        Special::Bloodmoon,
        Special::Plague,
        Special::Starfall,
        Special::Skyfall,
        Special::Riposte,
        Special::Volley,
        Special::Stormchain,
        Special::Riptide,
        Special::Breath,
    ];

    /// Where the tip of a blade 0.8 m long is, in the pose
    fn tip(p: &Pose) -> Vector3<f64> {
        p.hand + p.turn * Vector3::new(0.0, 0.0, 0.8)
    }

    /// The most the tip's velocity jumps over a move `seconds` long, in meters a second, and where,
    /// sampled every quarter millisecond, leaving out the moments of cuts, where it's meant to jump
    /// out of sight. However quickly a smooth move speeds up, from one sample to the next that's a
    /// small change, and smaller the finer it's sampled; a jump stays as big.
    fn biggest_jolt(rest: &Pose, keys: &[(f64, Pose, bool)], seconds: f64, animate: fn(&Pose, &[(f64, Pose, bool)], f64) -> Pose) -> (f64, f64) {
        let steps = (seconds * 4000.0) as usize;
        let dt = seconds / steps as f64;
        let mut times: Vec<f64> = keys.iter().map(|k| k.0).collect();
        times.insert(0, 0.0);
        times.push(1.0);
        let cut = |a: f64, b: f64| times.windows(2).any(|w| w[1] - w[0] < CUT && b > w[0] - 0.003 && a < w[1] + 0.003);
        let tips: Vec<Vector3<f64>> = (0..=steps).map(|k| tip(&animate(rest, keys, k as f64 / steps as f64))).collect();
        let jolts: Vec<f64> = (2..=steps).map(|k| ((tips[k] - tips[k - 1]) - (tips[k - 1] - tips[k - 2])).norm() / dt).collect();
        // A jump shows as one millisecond's change standing out from those just before and after
        // it. Even a very quick turn spreads over several.
        let (mut most, mut when) = (0.0, 0.0);
        for k in 3..jolts.len().saturating_sub(3) {
            let spike = jolts[k] - jolts[k - 3].max(jolts[k + 3]);
            if spike > most && !cut(k as f64 / steps as f64, (k + 2) as f64 / steps as f64) {
                (most, when) = (spike, (k + 2) as f64 / steps as f64);
            }
        }
        (most, when)
    }

    #[test]
    fn moves_never_jump() {
        let rest = pose([0.3, 0.46, -0.27], [-0.05, 0.62, 0.78], [0.0, 1.0, 0.0]);
        let mut moves: Vec<(String, Vec<(f64, Pose, bool)>, f64)> = Vec::new();
        for style in STYLES {
            for heavy in [false, true] {
                for combo in 0..=finisher(style) {
                    let seconds = if heavy { 1.6 } else if combo == finisher(style) { 1.25 } else { 1.0 } / 1.9;
                    moves.push((format!("{style:?} heavy {heavy} combo {combo}"), swing(style, heavy, combo), seconds));
                    if style == Style::Lash {
                        // A whip's every grade, cracking light, even or heavy, twirls and all, at
                        // one pace, as weighted_moves_never_jump() has blades
                        for grade in 0..6 {
                            for w in [-1.0, 0.0, 1.0] {
                                let swing = lash_swing(heavy, combo, grade, w);
                                moves.push((format!("lash grade {grade} weight {w} heavy {heavy} combo {combo}"), swing.keys, seconds * swing.stretch));
                            }
                        }
                    }
                }
            }
        }
        // Each family's own heavy blow, and every finisher a weapon's head can give it
        for (style, move_) in HEAVIES {
            moves.push((format!("{move_:?} heavy blow"), heavy_keys(style, move_), 1.6 * 1.12 / 1.9));
        }
        for finish in FINISHES {
            moves.push((format!("{finish:?} finisher"), finish_keys(finish), 1.25 / 1.9));
        }
        // Every way a spell's cast, as quick as a quick wand casts and as slow as a slow staff
        for casting in CASTINGS {
            for rate in [1.2, 2.0, 5.0] {
                moves.push((format!("{casting:?} cast at {rate}"), cast_keys(casting), cast_length(rate)));
            }
        }
        for special in SPECIALS {
            for (kind, style) in [
                (Kind::Melee, Style::Slash),
                (Kind::Melee, Style::Thrust),
                (Kind::Melee, Style::Lash),
                (Kind::Melee, Style::Whirl),
                (Kind::Melee, Style::Lob),
                (Kind::Ranged, Style::Draw),
                (Kind::Ranged, Style::Powder),
                (Kind::Ranged, Style::Crank),
                (Kind::Magic, Style::Cast),
            ] {
                let (seconds, keys) = special_move(special, kind, style);
                moves.push((format!("{special:?} with {kind:?} {style:?}"), keys, seconds));
            }
        }
        // Crossbows spanned and guns loaded, each way they can be
        for build in [Build::Lever, Build::Crank, Build::Repeater, Build::Musket, Build::Pistol, Build::Blunderbuss, Build::Cannon, Build::Hand, Build::Twin, Build::Siege, Build::Harpoon, Build::Fan] {
            for knack in [Knack::Plain, Knack::Quick, Knack::Heavy, Knack::Showy] {
                moves.push((format!("loading a {build:?} {knack:?}"), knack_keys(build, knack), 1.0));
            }
        }
        // Easing in and out of each key on its own, as moves used to, stops dead at every one,
        // which this would catch
        let stepped: fn(&Pose, &[(f64, Pose, bool)], f64) -> Pose = |rest, keys, f| {
            let mut from = (0.0, rest);
            for (at, pose, quick) in keys {
                if f < *at {
                    let k = ((f - from.0) / (at - from.0)).clamp(0.0, 1.0);
                    return blend(from.1, pose, if *quick { k * k } else { smooth(k) });
                }
                from = (*at, pose);
            }
            blend(from.1, rest, smooth((f - from.0) / (1.0 - from.0)))
        };
        let (old, _) = biggest_jolt(&rest, &swing(Style::Slash, false, 0), 1.0 / 1.9, stepped);
        assert!(old > 5.0, "the old way only jumps {old:.2} m/s");
        let mut worst: Vec<(f64, f64, String)> = moves
            .iter()
            .map(|(name, keys, seconds)| {
                let (most, at) = biggest_jolt(&rest, keys, *seconds, animate);
                (most, at, name.clone())
            })
            .collect();
        worst.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (most, at, name) in worst.iter().take(8) {
            println!("{name}: velocity jumps {most:.2} m/s at {at:.3}");
        }
        // The old way, stopping dead at each key, jumped by 10-30 m/s at once
        let (most, at, name) = &worst[0];
        assert!(*most < 0.5, "{name} velocity jumps {most:.2} m/s at {at:.3}");
    }

    /// A hand crossbow goes through every special attack's moves in the left hand still pointing
    /// on ahead, never turned round to point back at the player, as Shackle Chain used to
    #[test]
    fn a_hand_crossbow_never_turns_round() {
        for special in SPECIALS {
            let (_, keys) = special_move(special, Kind::Ranged, Style::Crank);
            for (at, key, _) in &keys {
                let ahead = (key.turn * Vector3::x()).y;
                let left = mirrored_shouldered(key);
                let mirrored_ahead = (left.turn * Vector3::x()).y;
                assert!(ahead > 0.3 && mirrored_ahead > 0.3, "{special:?} at {at:.2}: points {ahead:.2} ahead, {mirrored_ahead:.2} in the left hand");
                assert!((left.hand.x + key.hand.x).abs() < 1e-9, "{special:?}: on the other side");
            }
        }
    }

    /// Shackle Chain, Blood Moon and Cluster Volley let go on the move, the weapon kicking, and
    /// before they're over
    #[test]
    fn own_specials_let_go_on_a_quick_key() {
        // Each with every kind of weapon it's on: Shackle Chain on crossbows, Cluster Volley on
        // crossbows, guns and bombs, Blood Moon on all of those, bows and blades, and a staff
        let shouldered = [(Kind::Ranged, Style::Crank), (Kind::Ranged, Style::Powder)];
        let all = [(Kind::Ranged, Style::Crank), (Kind::Ranged, Style::Powder), (Kind::Ranged, Style::Draw), (Kind::Melee, Style::Lob), (Kind::Melee, Style::Slash), (Kind::Melee, Style::Reap), (Kind::Magic, Style::Cast)];
        let cases: [(Special, &[(Kind, Style)]); 3] = [(Special::Shackle, &shouldered[..1]), (Special::ClusterBomb, &[shouldered[0], shouldered[1], (Kind::Melee, Style::Lob)]), (Special::Bloodmoon, &all)];
        for (special, ways) in cases {
            for &(kind, style) in ways {
                let (length, keys) = special_move(special, kind, style);
                let release = first_release(length, &keys);
                assert!(keys.iter().any(|k| k.2), "{special:?} with {kind:?} {style:?} never lets go");
                assert!(release > 0.0 && release < length * 0.8, "{special:?} with {kind:?} {style:?} lets go at {release:.2} of {length:.2}");
            }
        }
        // The chains leave the crossbow as it kicks, and even the furthest has hooked in by the
        // time they're hauled on, before they crash
        let (length, keys) = special_move(Special::Shackle, Kind::Ranged, Style::Crank);
        assert!((first_release(length, &keys) - SHACKLE_RELEASE).abs() < 1e-9);
        assert!(SHACKLE_RELEASE + chain_flight(5, SHACKLE_REACH) <= SHACKLE_HAUL + 1e-9 && SHACKLE_HAUL < SHACKLE_CRASH && SHACKLE_CRASH < length);
    }

    /// A shackle's chain is drawn in stretches that start at the crossbow and end at what it caught,
    /// longer further off, never more than it may have
    #[test]
    fn chain_stretches_grow_further_off() {
        for (length, near, far, most) in [(20.0, 0.6, 20.0, 5), (6.0, 15.0, 16.0, 4), (0.4, 0.5, 0.8, 5), (30.0, 25.0, 1.0, 3)] {
            let cuts = chain_stretches(length, near, far, most);
            assert!(cuts.len() >= 2 && cuts.len() <= most + 1, "{cuts:?}");
            assert_eq!(cuts[0], 0.0);
            assert!((cuts[cuts.len() - 1] - 1.0).abs() < 1e-9, "{cuts:?}");
            assert!(cuts.windows(2).all(|w| w[1] > w[0]), "{cuts:?}");
            let stretches: Vec<f64> = cuts.windows(2).map(|w| w[1] - w[0]).collect();
            if far > near * 2.0 {
                assert!(stretches[0] < stretches[stretches.len() - 1], "nearest {stretches:?}");
            }
        }
        // One from the crossbow twenty meters out has as many stretches as it may
        assert_eq!(chain_stretches(20.0, 0.6, 20.0, 5).len(), 6);
    }

    /// Something sent off with streamed() gets where it's sent when it's meant to, slowed by the
    /// air and falling as sparks do
    #[test]
    fn streams_arrive_where_theyre_sent() {
        let vpm = 16.0;
        let (from, to) = (Vector3::new(10.0, 40.0, 30.0) * vpm, Vector3::new(1.0, 2.0, 25.0) * vpm);
        for (flight, gravity) in [(0.42, 0.5), (0.25, 0.0), (0.6, 1.0)] {
            let mut at = from;
            let mut v = streamed(from, to, flight, gravity, vpm);
            let steps = 2000;
            let dt = flight / steps as f64;
            for _ in 0..steps {
                v.z -= 9.8 * vpm * gravity * dt;
                v *= (-SPARK_DRAG * dt).exp();
                at += v * dt;
            }
            assert!((at - to).norm() < 0.05 * vpm, "{flight} {gravity}: {:.3} m off", (at - to).norm() / vpm);
        }
    }

    /// Blood Moon rises up in the view however it's looked, but never under the horizon
    #[test]
    fn the_blood_moon_rises_in_view() {
        let vpm = 16.0;
        let tan_up = 35f64.to_radians().tan();
        for pitch in [-0.3f64, -0.15, 0.0, 0.2] {
            let forward = Vector3::new(pitch.cos(), 0.0, pitch.sin());
            let moon = moon_rise(Vector3::zeros(), forward, tan_up, vpm);
            let to = moon.normalize();
            assert!(to.z > MOON_LOWEST.sin() - 1e-9, "under the horizon looking {pitch}");
            let above = to.z.asin() - pitch;
            assert!(above > 0.0 && above.tan() < tan_up * 0.85, "out of view looking {pitch}: {above:.2} up");
            assert!((moon.norm() / vpm - MOON_OFF).abs() < 1e-6);
        }
    }

    /// A burst's shards spread out over its cone, none of them outside it and no two the same way
    #[test]
    fn shards_spread_over_the_cone() {
        let way = Vector3::new(0.3, 1.0, -0.1).normalize();
        for (spread, n) in [(18.0, 8), (65.0, 6), (12.0, 1)] {
            let ways: Vec<Vector3<f64>> = (0..n).map(|k| shard_way(way, spread, k, n, 0.3)).collect();
            for d in &ways {
                assert!((d.norm() - 1.0).abs() < 1e-9);
                assert!(d.dot(&way).acos().to_degrees() <= spread + 1e-6, "{spread}");
            }
            for i in 0..n {
                for j in i + 1..n {
                    assert!(ways[i].dot(&ways[j]) < 0.9999, "{spread}: {i} and {j} go the same way");
                }
            }
            // Spread right round, not all off to one side
            if n > 4 {
                let middle = ways.iter().sum::<Vector3<f64>>() / n as f64;
                assert!(middle.normalize().dot(&way) > (spread.to_radians() * 0.4).cos(), "{spread}: lopsided");
            }
        }
    }

    #[test]
    fn swings_keep_moving_into_the_blow() {
        let rest = pose([0.3, 0.46, -0.27], [-0.05, 0.62, 0.78], [0.0, 1.0, 0.0]);
        // Not a whip: its hand checks as it cracks, which is what snaps the lash out
        let mut blows: Vec<(String, Vec<(f64, Pose, bool)>, (f64, f64))> = Vec::new();
        for style in STYLES.into_iter().filter(|&s| s != Style::Lash) {
            for heavy in [false, true] {
                for combo in 0..=finisher(style) {
                    blows.push((format!("{style:?} heavy {heavy} combo {combo}"), swing(style, heavy, combo), timing(style, heavy, combo)));
                }
            }
        }
        for (style, move_) in HEAVIES {
            blows.push((format!("{move_:?}"), blow_keys(style, move_, Finish::Style, true, 0), blow_timing(style, move_, Finish::Style, true, 0)));
        }
        for finish in FINISHES {
            blows.push((format!("{finish:?} finisher"), finish_keys(finish), finish_timing(finish)));
        }
        for (name, keys, (comes, lands)) in blows {
            // Once the blow's under way after the wind-up, it doesn't slow down again before it
            // lands
            let speed = |f: f64| (tip(&animate(&rest, &keys, f + 1e-4)) - tip(&animate(&rest, &keys, f))).norm() / 1e-4;
            let mut fastest: f64 = 0.0;
            for k in 0..=60 {
                let f = comes + (lands - comes) * k as f64 / 60.0;
                let v = speed(f);
                assert!(fastest < 1.0 || v > 0.5 * fastest, "{name} slows to {v:.2} m/s after {fastest:.2} at {f:.3}");
                fastest = fastest.max(v);
            }
            // And it's going its fastest about when it lands, not well before
            let at_land = speed(lands - 0.01);
            assert!(at_land > 0.4 * fastest, "{name} has slowed to {at_land:.2} m/s by the time it lands, after {fastest:.2}");
        }
    }

    /// A blade's light blows go four different ways before the chain starts again
    #[test]
    fn a_blades_chain_goes_a_different_way_each_blow() {
        let rest = pose([0.3, 0.46, -0.27], [-0.05, 0.62, 0.78], [0.0, 1.0, 0.0]);
        for style in [Style::Slash, Style::Chop, Style::Smash, Style::Stab, Style::Thrust, Style::Reap] {
            assert_eq!(finisher(style), 3, "{style:?}");
            // Which way the tip's going as each lands
            let ways: Vec<Vector3<f64>> = (0..=3)
                .map(|combo| {
                    let keys = swing(style, false, combo);
                    let (_, lands) = timing(style, false, combo);
                    (tip(&animate(&rest, &keys, lands)) - tip(&animate(&rest, &keys, lands - 0.02))).normalize()
                })
                .collect();
            // The three light blows each go their own way, and the finisher another way from the
            // one before it: a hammer's may come down overhead again, as hard as it can
            for (a, b) in [(0, 1), (0, 2), (1, 2), (2, 3)] {
                assert!(ways[a].dot(&ways[b]) < 0.97, "{style:?} blows {a} and {b} both go {:?}", ways[a]);
            }
        }
        // Whips, flails and mauls keep their three
        for style in [Style::Lash, Style::Whirl, Style::Quake] {
            assert_eq!(finisher(style), FINISHER);
        }
    }

    /// Flourishes start and end where the weapon was, and move smoothly between
    #[test]
    fn flourishes_start_and_end_at_rest() {
        for kind in [Flourish::Twirl, Flourish::Flip, Flourish::Shoulder, Flourish::Inspect, Flourish::Spin, Flourish::Toss, Flourish::Tap, Flourish::Check] {
            for f in [0.0, 1.0] {
                let (hand, before, after) = flourish_at(kind, f);
                assert!(hand.norm() < 1e-9 && before.angle() < 1e-6 && after.angle() < 1e-6, "{kind:?} at {f}: {hand:?} {} {}", before.angle(), after.angle());
            }
            let rest = pose([0.3, 0.46, -0.27], [-0.05, 0.62, 0.78], [0.0, 1.0, 0.0]);
            let at = |f: f64| {
                let (hand, before, after) = flourish_at(kind, f);
                tip(&Pose { hand: rest.hand + hand, turn: before * rest.turn * after })
            };
            // A jump shows as one sample's change in velocity standing out from those round it,
            // as biggest_jolt() looks for
            let steps = 4000;
            let dt = FLOURISH_SECONDS / steps as f64;
            let tips: Vec<Vector3<f64>> = (0..=steps).map(|k| at(k as f64 / steps as f64)).collect();
            let jolts: Vec<f64> = (2..=steps).map(|k| ((tips[k] - tips[k - 1]) - (tips[k - 1] - tips[k - 2])).norm() / dt).collect();
            for k in 3..jolts.len() - 3 {
                let spike = jolts[k] - jolts[k - 3].max(jolts[k + 3]);
                assert!(spike < 0.3, "{kind:?} jumps {spike:.3} m/s at {:.3}", k as f64 / steps as f64);
            }
        }
    }

    /// How fast the tip goes through a move, to look at: cargo test --release speeds -- --ignored --nocapture
    #[test]
    #[ignore]
    fn speeds() {
        let rest = pose([0.3, 0.46, -0.27], [-0.05, 0.62, 0.78], [0.0, 1.0, 0.0]);
        for (name, keys) in [("smash heavy", swing(Style::Smash, true, 0)), ("twirl", heavy_keys(Style::Reap, Heavy::Twirl)), ("flurry", special_move(Special::Flurry, Kind::Melee, Style::Slash).1)] {
            println!("{name}: keys at {:?}", keys.iter().map(|k| (k.0 * 1000.0).round() / 1000.0).collect::<Vec<_>>());
            let line: Vec<String> = (0..=50)
                .map(|k| {
                    let f = k as f64 / 50.0;
                    let v = (tip(&animate(&rest, &keys, (f + 1e-4).min(1.0))) - tip(&animate(&rest, &keys, f))).norm() / 1e-4;
                    format!("{f:.2}:{v:.1}")
                })
                .collect();
            println!("{}", line.join(" "));
        }
    }

    #[test]
    fn moves_start_and_end_at_rest() {
        let rest = pose([0.3, 0.46, -0.27], [-0.05, 0.62, 0.78], [0.0, 1.0, 0.0]);
        for style in STYLES {
            let keys = swing(style, false, 0);
            for f in [0.0, 1.0] {
                let p = animate(&rest, &keys, f);
                assert!((p.hand - rest.hand).norm() < 1e-9 && p.turn.angle_to(&rest.turn) < 1e-6, "{style:?} at {f}");
            }
        }
    }

    #[test]
    fn bombs_come_down_where_theyre_aimed() {
        let vpm = 8.0;
        let from = Vector3::new(0.0, 0.0, 10.0) * vpm;
        let mut arcs = Vec::new();
        for to in [Vector3::new(6.0, 0.0, 9.0), Vector3::new(15.0, 7.0, 8.0), Vector3::new(-20.0, 3.0, 13.0), Vector3::new(0.0, 6.0, 10.0), Vector3::new(0.0, 22.0, 10.0)] {
            let to = to * vpm;
            let (mut v, time) = lobbed(from, to, vpm);
            // Flown as fly() flies it
            let (mut p, mut t, mut top) = (from, 0.0, from.z);
            let dt = 1.0 / 144.0;
            while t + dt <= time {
                v.z -= 9.8 * BOMB_GRAVITY * vpm * dt;
                p += v * dt;
                t += dt;
                top = top.max(p.z);
            }
            p += v * (time - t);
            assert!((p - to).norm() / vpm < 0.6, "a bomb lobbed at {to:?} came down at {p:?}");
            // Thrown up in an arc, higher the further
            let arc = (top - from.z.max(to.z)) / vpm;
            assert!(arc > 0.3, "a bomb lobbed {:.0} m only rises {arc:.1} m", (to - from).norm() / vpm);
            arcs.push(arc);
        }
        // Over level ground, the further it's thrown the higher it goes
        assert!(arcs[4] > arcs[3] * 2.0, "{arcs:?}");
    }

    #[test]
    fn a_flail_whirls_round_through_in_front_as_it_lands() {
        let rest = pose([0.3, 0.42, -0.32], [-0.1, 0.88, 0.46], [1.0, 0.0, 0.0]);
        for combo in 0..=FINISHER {
            let keys = swing(Style::Whirl, false, combo);
            let (comes, lands) = timing(Style::Whirl, false, combo);
            let along = |f: f64| animate(&rest, &keys, f).turn * Vector3::z();
            // Out to one side as it comes round, straight ahead as it lands, out the other side after
            let (before, at, after) = (along(comes), along(lands), along(lands + 0.13));
            assert!(at.y > 0.9, "combo {combo}: pointing {at:?} as it lands");
            assert!(before.x * after.x < -0.5, "combo {combo}: from {before:?} round to {after:?}");
        }
        // The second goes round the other way
        let first = animate(&rest, &swing(Style::Whirl, false, 0), timing(Style::Whirl, false, 0).0).turn * Vector3::z();
        let second = animate(&rest, &swing(Style::Whirl, false, 1), timing(Style::Whirl, false, 1).0).turn * Vector3::z();
        assert!(first.x * second.x < 0.0);
    }

    #[test]
    fn blending_starts_where_it_was_and_ends_on_the_move() {
        let drawn = pose([0.1, 0.5, -0.2], [0.3, 0.9, 0.1], [1.0, 0.0, 0.0]);
        let target = pose([0.3, 0.46, -0.27], [-0.05, 0.62, 0.78], [0.0, 1.0, 0.0]);
        let mut b = Blending::new(&drawn, (Vector3::new(-1.0, 0.0, 0.0), Vector3::new(0.0, 0.0, 2.0)), &target);
        let p = b.apply(&target);
        assert!((p.hand - drawn.hand).norm() < 1e-9 && p.turn.angle_to(&drawn.turn) < 1e-9);
        b.age = BLEND;
        let p = b.apply(&target);
        assert!((p.hand - target.hand).norm() < 1e-9 && p.turn.angle_to(&target.turn) < 1e-9);
        assert!(b.done());
    }

    /// Every blow but a whip's, light and heavy, with its timing and its keys
    fn all_blows() -> Vec<(String, (f64, f64), Vec<(f64, Pose, bool)>)> {
        let mut blows = Vec::new();
        for style in STYLES.into_iter().filter(|&s| s != Style::Lash) {
            for heavy in [false, true] {
                for combo in 0..=finisher(style) {
                    blows.push((format!("{style:?} heavy {heavy} combo {combo}"), timing(style, heavy, combo), swing(style, heavy, combo)));
                }
            }
        }
        for (style, move_) in HEAVIES {
            blows.push((format!("{move_:?}"), blow_timing(style, move_, Finish::Style, true, 0), blow_keys(style, move_, Finish::Style, true, 0)));
        }
        for finish in FINISHES {
            blows.push((format!("{finish:?} finisher"), finish_timing(finish), finish_keys(finish)));
        }
        blows
    }

    /// A heavy weapon gathers longer and comes down faster, a light one snaps out sooner, and
    /// either way the blow lands just as the move drawn lands it
    #[test]
    fn heavy_weapons_wind_up_longer_and_land_as_drawn() {
        for (name, timing, keys) in all_blows() {
            let (heavy, even, light) = (weighted(timing, 1.0), weighted(timing, 0.0), weighted(timing, -1.0));
            assert_eq!(even, timing, "{name}");
            assert!(heavy.0 > even.0 + 0.04 && even.0 > light.0 + 0.04, "{name}: winds up till {heavy:?} {even:?} {light:?}");
            assert!(heavy.1 - heavy.0 < even.1 - even.0 && even.1 - even.0 < light.1 - light.0, "{name}: {heavy:?} {even:?} {light:?}");
            assert!(heavy.1 < 0.9 && light.0 > 0.0, "{name}");
            let cuts = |keys: &[(f64, Pose, bool)]| {
                let mut at = vec![0.0];
                at.extend(keys.iter().map(|k| k.0));
                at.windows(2).filter(|w| w[1] - w[0] < CUT).count()
            };
            for w in [-1.0, -0.4, 0.0, 0.5, 1.0] {
                let (comes, lands) = weighted(timing, w);
                let moved = weighted_keys(keys.clone(), timing, w);
                // The key it's struck on, moving fastest, is where it lands
                assert!(moved.iter().any(|k| k.2 && (k.0 - lands).abs() < 1e-9), "{name} at weight {w}: nothing lands at {lands:.3}");
                assert!(moved.iter().all(|k| k.0 <= 1.0) && moved.windows(2).all(|p| p[1].0 >= p[0].0), "{name} at weight {w}: keys out of order");
                // What was out of sight still is, and nothing else jumps
                assert_eq!(cuts(&moved), cuts(&keys), "{name} at weight {w}");
                // Wound up before it comes
                assert!(moved.iter().filter(|k| k.0 <= comes + 1e-9).count() == keys.iter().filter(|k| k.0 <= timing.0 + 1e-9).count(), "{name}");
                // Unweighted, it's just as it was
                if w == 0.0 {
                    for (a, b) in moved.iter().zip(&keys) {
                        assert!((a.0 - b.0).abs() < 1e-9 && (a.1.hand - b.1.hand).norm() < 1e-9, "{name}");
                    }
                }
            }
            // Carried on further through after it lands the heavier it is
            let rest = pose([0.3, 0.46, -0.27], [-0.05, 0.62, 0.78], [0.0, 1.0, 0.0]);
            let after = |w: f64| {
                let (_, lands) = weighted(timing, w);
                let keys = weighted_keys(keys.clone(), timing, w);
                (tip(&animate(&rest, &keys, lands + 0.08)) - tip(&animate(&rest, &keys, lands))).norm()
            };
            assert!(after(1.0) > after(-1.0), "{name}: heavy follows through {:.3}, light {:.3}", after(1.0), after(-1.0));
        }
    }

    #[test]
    fn weighted_moves_never_jump() {
        let rest = pose([0.3, 0.46, -0.27], [-0.05, 0.62, 0.78], [0.0, 1.0, 0.0]);
        let mut worst = (0.0, 0.0, String::new());
        for (name, timing, keys) in all_blows() {
            for w in [-1.0, 1.0] {
                let moved = weighted_keys(keys.clone(), timing, w);
                let (most, at) = biggest_jolt(&rest, &moved, 1.25 / 1.9, animate);
                if most > worst.0 {
                    worst = (most, at, format!("{name} at weight {w}"));
                }
            }
        }
        let (most, at, name) = worst;
        assert!(most < 0.5, "{name} velocity jumps {most:.2} m/s at {at:.3}");
    }

    /// A legendary weapon's finisher, and a mythic one's every blow, is twirled whole turns round
    /// in the hand after it lands, and back at rest before the blow's over
    #[test]
    fn rare_blows_twirl_after_they_land() {
        for style in [Style::Slash, Style::Chop, Style::Reap, Style::Smash, Style::Stab, Style::Thrust] {
            for combo in 0..=finisher(style) {
                let (_, lands) = timing(style, false, combo);
                let last = combo == finisher(style);
                for grade in 0..6 {
                    let (end, lifted) = twirl(grade, style, false, combo, lands, 0.99);
                    assert!(lifted.abs() < 1e-9, "{style:?} {combo} {grade}");
                    let turns = end / TAU;
                    assert!((turns - turns.round()).abs() < 1e-9, "{style:?} {combo} grade {grade} ends {turns} turns round");
                    let want = match grade {
                        5 if last => 2.0,
                        5 => 1.0,
                        4 if last => 1.0,
                        _ => 0.0,
                    };
                    assert_eq!(turns.round(), want, "{style:?} combo {combo} grade {grade}");
                    assert_eq!(twirl(grade, style, false, combo, lands, lands).0, 0.0, "not before it lands");
                    assert_eq!(twirl(grade, style, true, combo, lands, 0.8).0, 0.0, "not on a heavy blow");
                }
            }
        }
        assert_eq!(twirl(5, Style::Lash, false, FINISHER, 0.6, 0.8).0, 0.0);
    }

    /// A whip's swing follows how heavy it cracks: a heavy one coils back longer, its throw comes
    /// faster once it goes, and its crack lands just as its handle's checked, on the very frame
    /// the blow's dealt; a light one flicks out sooner. Everything stays within the swing.
    #[test]
    fn whips_crack_with_their_weight_and_hit_on_the_crack() {
        for grade in 0..6 {
            for heavy in [false, true] {
                for combo in 0..=FINISHER {
                    if heavy && combo > 0 {
                        continue;
                    }
                    let mut comes_by_weight = Vec::new();
                    for w in [-1.0, -0.5, 0.0, 0.5, 1.0] {
                        let name = format!("grade {grade} heavy {heavy} combo {combo} weight {w}");
                        let s = lash_swing(heavy, combo, grade, w);
                        assert!(s.comes > 0.0 && s.comes < s.lands && s.lands < 0.9, "{name}: comes {:.3} lands {:.3}", s.comes, s.lands);
                        // The handle's checked, its key moving fastest, just as the first crack lands
                        assert!(s.keys.iter().any(|k| k.2 && (k.0 - s.lands).abs() < 1e-9), "{name}: no key lands at {:.3}", s.lands);
                        let first = s.cracks.iter().find(|c| !matches!(c.wind, Wind::Twirl(_))).expect("a crack");
                        assert!((first.at - s.lands).abs() < 1e-9 && first.from < first.at, "{name}: cracks at {:.3}, lands at {:.3}", first.at, s.lands);
                        assert!(s.keys.iter().all(|k| (0.0..=1.0).contains(&k.0)) && s.keys.windows(2).all(|p| p[1].0 >= p[0].0), "{name}: keys out of order");
                        assert!(s.cracks.iter().all(|c| (0.0..=1.0).contains(&c.from) && c.at <= 1.0 && c.from <= c.at), "{name}: {:?}", s.cracks);
                        // A double crack's second lands after the first, with time to be seen
                        if let Some(second) = s.second {
                            assert!(second > s.lands + 0.08 && second < 0.95, "{name}: second crack at {second:.3}");
                            assert!(s.keys.iter().any(|k| k.2 && (k.0 - second).abs() < 1e-9), "{name}: nothing cracks at {second:.3}");
                        }
                        // Longer by as much as it coils back longer, and by a rare finisher's twirl
                        let turns = lash_turns(grade, heavy, combo);
                        let coiled = 1.0 + timing(Style::Lash, heavy, combo).0 * WIND * w;
                        let twirled = if combo == FINISHER { LASH_TWIRL * turns } else { 0.0 };
                        assert!((s.stretch - coiled - twirled).abs() < 1e-9, "{name}: {} long", s.stretch);
                        assert!((0.75..=2.0).contains(&s.stretch), "{name}");
                        assert_eq!(s.cracks.iter().any(|c| matches!(c.wind, Wind::Twirl(_))), turns > 0.0, "{name}");
                        // How long, as parts of a plain swing, it coils back, and throws
                        comes_by_weight.push((s.comes * s.stretch, (s.lands - s.comes) * s.stretch, first.hold));
                    }
                    // The heavier, the longer it coils back, its throw as quick whatever it is
                    for pair in comes_by_weight.windows(2) {
                        assert!(pair[1].0 > pair[0].0 && (pair[1].1 - pair[0].1).abs() < 1e-9, "grade {grade} heavy {heavy} combo {combo}: {comes_by_weight:?}");
                        assert!(pair[1].2 >= pair[0].2, "a heavier crack's held out as long: {comes_by_weight:?}");
                    }
                }
            }
        }
        // A legendary whip's finisher twirls once, a mythic one's every blow, its finisher twice
        assert_eq!((lash_turns(4, false, FINISHER), lash_turns(4, false, 0), lash_turns(5, false, 0), lash_turns(5, false, FINISHER), lash_turns(5, true, 0)), (1.0, 0.0, 1.0, 2.0, 0.0));
    }

    /// The whips there are crack as heavy as they're quick: a chain or a scourge weighted with iron
    /// slower and heavier than one of silk, and every one of them within the limits, its hit
    /// landing on its first crack
    #[test]
    fn whips_weigh_what_theyre_made_of() {
        let library = crate::weapons::Library::load();
        let whip = library.family("whip").unwrap();
        let mut weights = Vec::new();
        for seed in 0..600 {
            let weapon = library.generate(seed, Some(whip), 2.0);
            let w = weight(&weapon.profile);
            assert!((-1.0..=1.0).contains(&w));
            let (_, lands) = blow_times(&weapon.profile, false, 0);
            assert!((lands - lash_swing(false, 0, weapon.grade, w).lands).abs() < 1e-12);
            weights.push(w);
        }
        let (least, most) = weights.iter().fold((f64::MAX, f64::MIN), |(a, b), &w| (a.min(w), b.max(w)));
        assert!(least < -0.3 && most > 0.3, "whips only weigh {least:.2} to {most:.2}");
        let part = |id: &str| library.components.iter().position(|c| c.id == id).unwrap_or_else(|| panic!("no {id}"));
        let made = |lash: &str, fall: &str| {
            let mut parts = library.generate(3, Some(whip), 0.0).parts;
            parts[0] = part(lash);
            parts[2] = part(fall);
            weight(&crate::weapons::Weapon::assemble(&library, whip, parts, 3).profile)
        };
        let (chain, silk) = (made("chain_lash", "iron_star_fall"), made("silk_scourge_lash", "feather_fall"));
        assert!(chain > silk + 0.4, "a chain whip weighs {chain:.2}, a silk one {silk:.2}");
    }

    /// Casting takes as long as the weapon casts quickly or slowly, within limits, and the spell
    /// leaves just as the move drawn lets it go
    #[test]
    fn casts_follow_their_pace_and_let_go_on_the_move() {
        let mut last = f64::MAX;
        for rate in [0.25, 0.8, 1.2, 2.0, 2.8, 3.6, 5.0, 6.0] {
            let length = cast_length(rate);
            assert!((CAST_SHORTEST..=CAST_LONGEST).contains(&length), "{rate}: {length}");
            assert!(length <= last, "{rate}");
            // Never so long the next cast has to cut it short, but where it's as short as it goes
            assert!(length <= 1.0 / rate + 1e-9 || length == CAST_SHORTEST, "{rate}: {length}");
            last = length;
        }
        assert!((cast_length(2.0) - 0.45).abs() < 1e-9, "a plain staff casts as it did");
        for casting in CASTINGS {
            let (gathers, release) = cast_times(casting, Style::Cast);
            assert!(0.0 < gathers && gathers < release && release < 0.8, "{casting:?}");
            let keys = cast_keys(casting);
            assert!(keys.iter().any(|k| k.2 && (k.0 - release).abs() < 1e-9), "{casting:?} lets go at {release}, but no key does");
            assert!(keys.iter().all(|k| k.0 < 1.0) && keys.windows(2).all(|p| p[1].0 - p[0].0 >= CUT), "{casting:?}");
            // Slow casts gather their spells longer, in seconds as well
            assert!(release * cast_length(1.2) > release * cast_length(4.0));
        }
        // A big spell gathered takes longer to let go than a wand's flick
        assert!(cast_times(Casting::Gather, Style::Cast).1 * cast_length(1.5) > 2.0 * cast_times(Casting::Flick, Style::Flick).1 * cast_length(3.6));
        // Twirled whole turns after it's let go: every third cast from legendary, every cast from
        // mythic, the third twice
        for (grade, third, turns) in [(3, true, 0.0), (4, false, 0.0), (4, true, 1.0), (5, false, 1.0), (5, true, 2.0)] {
            let (end, lifted) = cast_twirl(grade, third, 0.4, 0.995);
            assert!((end / TAU - turns).abs() < 1e-9 && lifted.abs() < 1e-9, "grade {grade} third {third}: {end}");
            assert_eq!(cast_twirl(grade, third, 0.4, 0.4).0, 0.0, "not before it's let go");
        }
    }

    /// Every way a weapon's parts can make it finish, and its heavy lunge, lands as drawn and
    /// carries on into the blow
    #[test]
    fn part_finishers_land_as_drawn() {
        for finish in FINISHES {
            let (comes, lands) = finish_timing(finish);
            let keys = finish_keys(finish);
            assert!(keys.iter().any(|k| k.2 && (k.0 - lands).abs() < 1e-9), "{finish:?}");
            assert!(comes < lands && lands < 0.9);
            for style in [Style::Slash, Style::Thrust, Style::Smash, Style::Stab] {
                assert_eq!(blow_timing(style, Heavy::Style, finish, false, finisher(style)), (comes, lands));
                assert_eq!(blow_timing(style, Heavy::Style, finish, false, 0), timing(style, false, 0), "only the finisher");
            }
        }
    }

    /// The trail of a rare weapon's blows comes out about its rarity's color
    #[test]
    fn rarity_trails_are_about_their_color() {
        let library = crate::weapons::Library::load();
        let epic = rarity_looks(library.rarities[3].color);
        assert_eq!(epic, (Element::look(Some(Element::Void)), Element::look(Some(Element::Void)), 1.0));
        let legendary = rarity_looks(library.rarities[4].color);
        assert_eq!(legendary.0, Element::look(Some(Element::Fire)));
        // Mythic's pink is between two of them
        let (a, b, share) = rarity_looks(library.rarities[5].color);
        assert!(a != b && (0.3..0.7).contains(&share), "{a} {b} {share}");
    }
}
