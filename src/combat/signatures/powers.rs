//! The right-click powers of legendary and mythic weapons, see weapons::Power: one for each
//! family, or a few families that fight alike, flavoured by the weapon's element, so a fire
//! greatsword's differs from a frost one's. Where right click already does something it carries
//! the power: a heavy blow lets it go as it lands and a bow drawn full looses it. A crossbow or gun
//! shoots it on right click, loading early moving to Q; bombs throw it when right click's let go
//! of after aiming; a weapon that casts casts it. Each has a cooldown, and one that casts costs
//! mana too, so it never outshines the special attack on R.
//!
//! Like the signature special attacks, what it shoots and leaves going is the fight's own
//! machinery: projectiles carrying Sig::Power and zones of SigZone::Power, done here.

use super::*;
use crate::combat::aim::{PowerWay, power_way};
use crate::weapons::Power;

/// How long till the power's ready again, how long right click's been held, for bombs, and
/// whether it's being aimed, right click pressed with it ready and held, to go off when let go
#[derive(Default)]
pub(in crate::combat) struct PowerState {
    pub(in crate::combat) cooldown: f64,
    held: f64,
    pub(in crate::combat) aiming: bool,
    /// How long it's been aimed, right click held, and how charged it was as it was let go, 0 to 1,
    /// for a priest's great spear, see faith.rs
    pub(in crate::combat) aimed: f64,
    pub(in crate::combat) charge: f64,
}

/// What one of their shots is
#[derive(Clone, Copy, PartialEq, Debug)]
pub(in crate::combat) enum Shape {
    /// A crescent flying on, `width` meters across, cutting through what's in its swath, or with
    /// `low` a whip's running on low over the ground, up slopes and down off ledges
    Wave { width: f32, low: bool },
    /// A spinning wheel hurled out, and once it's as far as it goes, coming back
    Wheel { back: bool },
    /// A spear of light, through everything in a line
    Javelin,
    /// An arrow like a comet, through everything, bursting where it comes down
    Arrow,
    /// A bolt that sticks where it strikes and bursts apart
    Bolt,
    /// A glowing slug glancing on this many more times
    Slug(u8),
    /// A lance, through everything, leaving a line that bursts after it
    Lance,
    /// One of a breath's embers, or a burst bolt's shards
    Ember,
    /// A mortar's shell
    Shell,
    /// A vortex charge
    Charge,
    /// A great slow orb
    Orb,
    /// A seeking mote
    Mote,
    /// A war fan's whirling funnel running on along the ground, carrying along what it's swept
    /// up, `hit`
    Gale,
    /// A katar's ghostly fist
    Fist,
    /// A hook on its chain flung out, then reeled back in, dragging what it caught, by id
    Hook { back: bool, caught: Option<u32> },
    /// An anvil coming down out of the sky
    Anvil,
    /// A spear hurled high, coming down to stake the ground
    Stake,
    /// An arrow loosed high, bursting into arrowlets over where it's aimed, `range` up into it
    Cloud,
    /// One of those arrowlets, coming down
    Arrowlet,
    /// Two weighted balls spinning on a line
    Bolas,
    /// A flare going up, bursting to hang over where it's aimed once it's `range` up into it
    Flare,
    /// A beacon drifting down onto where it's aimed
    Beacon,
    /// A pot of shards lobbed onto where it's aimed
    Pot,
    /// One of its shards, skimming out along the ground
    Shard,
    /// A spear of holy lightning from a talisman, through everything, lightning arcing off what it
    /// strikes, `big` for the great one of its power, see faith.rs
    Spear { big: bool },
    /// A skull focus's shade, drifting to the ring to raise the dead there, see faith.rs
    Shade,
    /// A throwing knife turning end over end, see thrown.rs
    Knife,
    /// A chakram spinning flat out to as far as it goes and back to the hand, glancing on from
    /// what it cuts to the next near this many more times first, see thrown.rs
    Chakram { back: bool, glances: u8 },
    /// The shade a Shadowstep steps through, flying at what it's aimed at, see arsenal.rs
    Step,
    /// One of a Spectrum's rays, through everything in a line
    Ray,
    /// A runestone's glyph cast down onto the ring, to write its circle there
    Glyph,
    /// A runestone's cast, a mote of its light flying to where it's aimed to write a rune there
    Sigil,
    /// A Vigil Spear, sticking fast where it strikes, see miracles.rs
    Vigil,
    /// Heavenfall's spear hurled up into the sky
    Ascent,
    /// Stormcall's orb going up over the ring to gather into a thunderhead
    Storm,
    /// Grasping Shadows' knot of shadow cast onto the ring
    Grasp,
    /// One of Helix's two orbs, wheeling round the other, `phase` which
    Helix { phase: u8 },
}

/// What they leave going a while
#[derive(Clone, Copy, PartialEq, Debug)]
pub(in crate::combat) enum Lasting {
    /// The ground splitting open from `position` on along `velocity`, a step of it at a time,
    /// `owed` steps so far
    Fissure,
    /// A burst bolt stuck fast, in `target` at `velocity` off its middle or in the ground,
    /// pointing `way`, bursting when its time's out
    Stuck { way: [f32; 3] },
    /// A vortex dragging everything in, bursting when its time's out
    Vortex,
    /// A lance's line hanging from `position` along `velocity`, bursting along when `tick`'s out
    Line,
    /// A ring rolling out from `position` to `radius` meters, `hit` what it's reached
    Nova,
    /// A ring of blades thrown out round `position` to `radius` meters and drawn back in, `hit`
    /// what it's cut on its way out and then, once `owed` is 1, on its way back
    BladeRing,
    /// A spear staked in the ground at `position`, pulsing every STAKE_PULSE seconds, `owed`
    /// pulses so far
    Stake,
    /// Arrowlets coming down over `position`, `owed` let fall so far
    Downpour,
    /// What a bolas wound round, `target`, held fast with what's by it, its balls spinning round
    /// it at `velocity` off its middle
    Bound,
    /// A flare hanging over `position`, sinking, raining sparks, `owed` let fall so far
    Flare,
    /// A beacon set down at `position`, tolling every BEACON_TOLL seconds, `owed` tolls so far,
    /// `hit` what this toll's ring has reached
    Beacon,
    /// A throwing knife stuck in the ground pointing `way`, sinking away when its time's out
    Knife { way: [f32; 3] },
    /// One of a Starbreak's cracks, a short fissure, see arsenal.rs
    Crack,
    /// A runestone's circle of runes being written round `position`, `owed` of them so far,
    /// blasting up once `dwell` is set, see arsenal.rs
    Runes,
    /// A runestone's one rune, written where its cast came down and blasting up a moment later
    Sigil,
    /// A Vigil Spear stuck fast, in `target` at `velocity` off its middle or in the ground, its
    /// way in `path`, erupting when its time's out, see miracles.rs
    Vigil,
    /// Heavenfall's spears coming down over the ring, `owed` so far
    Heavenfall,
    /// Stormcall's thunderhead over the ring, striking, `owed` times so far
    Storm,
    /// Glacier's ice bursting up step by step along `velocity`, the steps so far in `path`
    Glacier,
    /// Grasping Shadows' tendrils holding what's round
    Grasp,
}

/// How many steps a fissure takes, how far apart, in meters, and seconds between each, and how far
/// out its first is, in meters
pub(in crate::combat) const FISSURE_STEPS: f64 = 8.0;
pub(in crate::combat) const FISSURE_STEP: f64 = 1.5;
const FISSURE_GAP: f64 = 0.07;
pub(in crate::combat) const FISSURE_FROM: f64 = 2.6;
/// How far out a whip's wave starts, and how high it runs over the ground, in meters
pub(in crate::combat) const LASH_FROM: f64 = 2.0;
const LASH_HIGH: f64 = 0.8;
/// How far a javelin, a lance shot, a breath's embers and seeking motes go, in meters
pub(in crate::combat) const JAVELIN_REACH: f64 = 26.0;
pub(in crate::combat) const LANCE_REACH: f64 = 40.0;
pub(in crate::combat) const BREATH_REACH: f64 = 11.0;
pub(in crate::combat) const SEEKER_REACH: f64 = 30.0;
/// Seconds a burst bolt waits before it bursts, a vortex drags things in, and a lance's line hangs
const STUCK_SECONDS: f64 = 0.7;
const VORTEX_SECONDS: f64 = 1.4;
const VORTEX_REACH: f64 = 6.0;
const LINE_SECONDS: f64 = 0.45;
/// How fast a nova rolls out, in meters a second, and how far
const NOVA_SPEED: f64 = 12.0;
pub(in crate::combat) const NOVA_REACH: f64 = 9.0;
/// How fast a wheel flies, out and back, and how far out it goes, in meters
const WHEEL_SPEED: f64 = 18.0;
pub(in crate::combat) const WHEEL_REACH: f64 = 13.0;

/// A war fan's gale: how far out it starts, the rings along its way, in meters, and how fast it
/// runs, in meters a second
pub(in crate::combat) const GALE_FROM: f64 = 2.0;
pub(in crate::combat) const GALE_STEP: f64 = 2.0;
pub(in crate::combat) const GALE_STEPS: usize = 7;
const GALE_SPEED: f64 = 11.0;
/// How far a katar's fists and a hook fly, in meters, and how fast
pub(in crate::combat) const FISTS_REACH: f64 = 16.0;
pub(in crate::combat) const HOOK_REACH: f64 = 18.0;
const FIST_SPEED: f64 = 30.0;
const HOOK_SPEED: f64 = 32.0;
/// When a katar's five fists each leave it, as parts of its move
const FIST_RELEASES: [f64; 5] = [0.2, 0.33, 0.46, 0.59, 0.72];
/// How far out a naginata's ring of blades goes, in meters, and how long it takes out and back
pub(in crate::combat) const RING_REACH: f64 = 9.0;
const RING_SECONDS: f64 = 1.1;
/// How far an anvil and a stake reach, how far over the ring an anvil comes down from, and how
/// long it falls, in meters and seconds
pub(in crate::combat) const ANVIL_REACH: f64 = 24.0;
pub(in crate::combat) const STAKE_REACH: f64 = 26.0;
const ANVIL_HEIGHT: f64 = 14.0;
const ANVIL_FALL: f64 = 0.55;
/// A stake's pulses: how many, how far apart, in seconds, and how far round each reaches
const STAKE_PULSES: f64 = 3.0;
const STAKE_PULSE: f64 = 0.55;
const STAKE_ROUND: f64 = 3.0;
/// How high over the ring a cloudburst arrow and a starshell burst, in meters, how many
/// arrowlets come down, and over how long
const CLOUD_HEIGHT: f64 = 11.0;
const ARROWLETS: f64 = 20.0;
const DOWNPOUR_SECONDS: f64 = 0.9;
const FLARE_HEIGHT: f64 = 7.0;
const FLARE_SECONDS: f64 = 5.0;
/// How long a bolas holds, and how far round what it wound round it holds too, in meters
const BOUND_SECONDS: f64 = 3.0;
const BOUND_ROUND: f64 = 3.0;
/// A beacon's tolls: how many, how far apart, in seconds, how far its rings roll out, in meters,
/// and how fast
const BEACON_TOLLS: f64 = 3.0;
const BEACON_TOLL: f64 = 0.7;
pub(in crate::combat) const BEACON_REACH: f64 = 5.0;
const BEACON_SPEED: f64 = 14.0;
/// How far a shrapnel pot's shards skim out, in meters, and how many there are
pub(in crate::combat) const SHRAPNEL_REACH: f64 = 7.0;
const SHARDS: usize = 12;

/// A heavy blow's crescents: how many, how wide, how fast and far they fly, in meters and meters a
/// second, and how hard each hits next to a blow
pub(in crate::combat) fn wave_of(power: Power) -> (usize, f64, f64, f64, f64) {
    match power {
        Power::Rending => (1, 3.4, 16.0, 16.0, 2.0),
        Power::WindCut => (1, 1.4, 38.0, 22.0, 1.5),
        Power::BladeFan => (3, 1.1, 30.0, 11.0, 0.9),
        Power::LashWave => (1, 1.8, 30.0, 14.0, 1.4),
        _ => (1, 2.2, 26.0, 14.0, 1.6),
    }
}

/// How long a power's own move takes, for those shot, thrown or cast with a move of their own, and
/// when what it lets go leaves the weapon, as part of it
pub(in crate::combat) fn power_timing(power: Power) -> (f64, f64) {
    if let Some(timing) = super::arsenal::arsenal_timing(power).or_else(|| super::miracles::miracle_timing(power)) {
        return timing;
    }
    match power {
        Power::Mortar => (0.7, 0.4),
        Power::BurstBolt => (0.6, 0.34),
        Power::Vortex => (0.8, BOMB_RELEASE),
        Power::Comet | Power::Nova => (1.0, 0.42),
        Power::Seekers => (0.8, 0.34),
        Power::Gale => (0.75, 0.42),
        Power::Fists => (0.95, FIST_RELEASES[0]),
        Power::BladeRing => (1.0, 0.52),
        Power::Hook => (0.85, 0.34),
        Power::Anvil => (1.0, 0.56),
        Power::Stake => (0.85, 0.42),
        Power::Cloudburst => (0.95, 0.56),
        Power::Bolas => (0.75, 0.4),
        Power::Starshell => (0.85, 0.46),
        Power::Beacon => (1.0, 0.5),
        Power::Shrapnel => (0.8, 0.44),
        Power::Thunderspear => (0.9, 0.48),
        Power::RaiseDead => (1.0, 0.5),
        _ => (0.6, 0.32),
    }
}

/// Every moment, as a part of its move, a power's own move lets something go: a katar's five fists,
/// one each, everything else once
pub(in crate::combat) fn power_releases(power: Power) -> Vec<f64> {
    if let Some(releases) = super::miracles::miracle_releases(power) {
        return releases;
    }
    match power {
        Power::Fists => FIST_RELEASES.to_vec(),
        _ => vec![power_timing(power).1],
    }
}

/// How far out a ring of blades thrown out to `reach` meters is, `f` of the way through its time:
/// flung out fast, slowing at its widest, and drawn back in
fn ring_out(f: f64, reach: f64) -> f64 {
    0.6 + (reach - 0.6) * (PI * f.clamp(0.0, 1.0)).sin().powf(0.6)
}

/// How fast to shoot something from `from`, falling as things do, so it tops out right at `top`,
/// and how long it takes to get there
pub(super) fn topping_out(from: Vector3<f64>, top: Vector3<f64>, vpm: f64) -> (Vector3<f64>, f64) {
    let g = 9.8 * vpm;
    let rise = (top.z - from.z).max(vpm);
    let vz = (2.0 * g * rise).sqrt();
    let time = vz / g;
    let across = (top - from) / time;
    (Vector3::new(across.x, across.y, vz), time)
}

/// The move a power's shot, thrown or cast with, from where the weapon's held at rest
fn power_keys(power: Power, profile: &Profile) -> Vec<(f64, Pose, bool)> {
    if let Some(keys) = super::arsenal::arsenal_keys(power, profile).or_else(|| super::miracles::miracle_keys(power, profile)) {
        return keys;
    }
    let (_, release) = power_timing(power);
    match power {
        // Levelled and steadied, then kicking back hard as it goes off, a crossbow less so; a
        // mortar raised high to lob its shell, a breath held low and level
        Power::Ricochet | Power::LanceShot | Power::Breath | Power::Mortar | Power::BurstBolt => {
            let up = match power {
                Power::Mortar => 0.42,
                Power::Breath => -0.02,
                _ => 0.04,
            };
            let kick = if power == Power::BurstBolt { 1.0 } else { 1.8 };
            let aimed = levelled(up, 0.0);
            let mut lowered = levelled(0.02, 0.0);
            lowered.hand.z -= 0.02;
            vec![
                (0.12, levelled(up + 0.1, 0.12), false),
                (release - 0.1, aimed.clone(), false),
                (release - 0.03, steadied(&aimed, 1.0), false),
                (release, gun_kicked(&aimed, kick), true),
                (release + 0.2, rocked(&aimed, kick), false),
                (0.85, lowered, false),
            ]
        }
        // Drawn far back over the shoulder, held a moment, and hurled
        Power::Vortex => vec![
            (0.22, pose([0.3, 0.3, -0.12], [0.15, -0.35, 0.92], [1.0, 0.0, 0.0]), false),
            (release - 0.08, pose([0.36, 0.1, 0.06], [0.1, -0.62, 0.78], [1.0, 0.0, 0.0]), false),
            (release, pose([0.22, 0.64, -0.02], [0.0, 0.88, 0.48], [1.0, 0.0, 0.0]), true),
            (0.78, pose([0.12, 0.6, -0.38], [0.0, 0.6, -0.8], [1.0, 0.0, 0.0]), false),
        ],
        // Raised high and held trembling with what gathers in it, then brought down and out hard
        Power::Comet | Power::Nova => {
            let at = spell_at(profile.style);
            vec![
                (0.12, at([0.0, -0.05, 0.12], [0.0, -0.3, 0.95]), false),
                (0.3, at([0.01, -0.07, 0.15], [0.02, -0.42, 0.9]), false),
                (release - 0.05, at([0.0, -0.06, 0.14], [0.0, -0.38, 0.92]), false),
                (release, at([-0.07, 0.15, -0.01], [-0.06, 0.9, 0.42]), true),
                (0.62, at([-0.06, 0.13, 0.0], [-0.05, 0.85, 0.5]), false),
                (0.86, at([-0.01, 0.03, 0.01], [-0.02, 0.4, 0.92]), false),
            ]
        }
        // Drawn in to the chest, then flicked out to one side and the other as the motes go
        Power::Seekers => {
            let at = spell_at(profile.style);
            vec![
                (0.12, at([0.03, -0.06, 0.06], [0.05, 0.1, 1.0]), false),
                (release - 0.06, at([0.02, -0.08, 0.08], [0.0, 0.0, 1.0]), false),
                (release, at([-0.08, 0.16, 0.04], [-0.15, 0.88, 0.45]), true),
                (release + 0.14, at([0.04, 0.14, 0.05], [0.18, 0.86, 0.47]), false),
                (0.8, at([-0.01, 0.03, 0.01], [-0.02, 0.45, 0.9]), false),
            ]
        }
        // Wound right back out to the right, held a beat, then swept wide across, flat on, and
        // carried right on round to the left
        Power::Gale => vec![
            (0.14, pose([0.36, 0.32, -0.12], [0.8, 0.1, 0.6], [-0.6, 0.0, 0.8]), false),
            (0.3, pose([0.42, 0.28, -0.1], [0.88, 0.0, 0.48], [-0.48, 0.0, 0.88]), false),
            (release - 0.04, pose([0.4, 0.3, -0.11], [0.86, 0.05, 0.5], [-0.5, 0.0, 0.86]), false),
            (release, pose([0.02, 0.62, -0.18], [-0.1, 0.62, 0.78], [-1.0, 0.0, 0.1]), true),
            (release + 0.12, pose([-0.3, 0.5, -0.22], [-0.85, 0.3, 0.42], [-0.4, 0.0, -0.9]), false),
            (0.82, pose([-0.18, 0.48, -0.26], [-0.55, 0.45, 0.7], [-0.7, 0.0, -0.6]), false),
        ],
        // Drawn back to the hip and punched out five times, high, low, high, low, high, each fist
        // snapped back to the chest before the next
        Power::Fists => {
            let mut keys = vec![(0.1, pose([0.26, 0.2, -0.3], [0.02, 0.96, 0.28], [1.0, 0.0, 0.0]), false)];
            for (k, &at) in FIST_RELEASES.iter().enumerate() {
                let low = (k % 2) as f64;
                keys.push((at - 0.055, pose([0.25, 0.3, -0.27], [0.01, 0.97, 0.24], [1.0, 0.0, 0.0]), false));
                keys.push((at, pose([0.12 - 0.03 * low, 0.72, -0.18 - 0.05 * low], [-0.06, 0.99, 0.05 - 0.05 * low], [1.0, 0.0, 0.0]), true));
            }
            keys.push((0.86, pose([0.26, 0.36, -0.27], [0.0, 0.95, 0.3], [1.0, 0.0, 0.0]), false));
            keys
        }
        // Raised and spun right round overhead, once and a half, then swept out level as the ring
        // leaves it, the pole carried on round to the left after it
        Power::BladeRing => {
            let mut keys = Vec::new();
            for k in 0..6 {
                let a = k as f64 / 4.0 * TAU;
                keys.push((0.08 + 0.065 * k as f64, pose([0.12, 0.34, 0.06], [0.8 * a.cos(), 0.25 + 0.45 * a.sin(), 0.55], [-a.sin(), a.cos(), 0.0]), false));
            }
            keys.push((release, pose([0.04, 0.6, -0.24], [-0.2, 0.96, 0.12], [-1.0, 0.1, 0.0]), true));
            keys.push((release + 0.14, pose([-0.22, 0.5, -0.3], [-0.88, 0.42, 0.05], [-0.4, -0.9, 0.0]), false));
            keys.push((0.86, pose([-0.06, 0.46, -0.3], [-0.4, 0.5, 0.75], [-0.9, 0.2, 0.3]), false));
            keys
        }
        // Wound back over the shoulder, flung out hard, then hauled back in as the chain draws
        // tight
        Power::Hook => vec![
            (0.16, pose([0.32, 0.22, 0.0], [0.1, -0.35, 0.93], [0.0, 1.0, 0.3]), false),
            (release - 0.05, pose([0.34, 0.16, 0.04], [0.08, -0.5, 0.86], [0.0, 1.0, 0.5]), false),
            (release, pose([0.1, 0.72, -0.12], [0.0, 0.97, -0.12], [0.0, 0.12, 1.0]), true),
            (release + 0.12, pose([0.1, 0.7, -0.16], [0.0, 0.95, -0.25], [0.0, 0.25, 1.0]), false),
            (0.64, pose([0.3, 0.28, -0.18], [0.15, 0.55, 0.82], [0.0, 1.0, -0.2]), true),
            (0.86, pose([0.29, 0.4, -0.25], [-0.02, 0.6, 0.8], [0.0, 1.0, 0.0]), false),
        ],
        // Raised high overhead, held there trembling as it gathers, then slammed straight down
        // onto the ground in front, sinking after it as the anvil comes
        Power::Anvil => vec![
            (0.2, pose([0.22, 0.32, 0.02], [0.1, 0.05, 1.0], [0.0, 1.0, -0.05]), false),
            (0.34, pose([0.21, 0.3, 0.07], [0.12, -0.05, 0.99], [0.0, 1.0, 0.05]), false),
            (release - 0.06, pose([0.22, 0.29, 0.08], [0.1, -0.08, 0.99], [0.0, 1.0, 0.08]), false),
            (release, pose([0.06, 0.62, -0.36], [0.0, 0.8, -0.6], [0.0, -0.6, -0.8]), true),
            (release + 0.07, pose([0.06, 0.63, -0.42], [0.0, 0.72, -0.7], [0.0, -0.7, -0.72]), false),
            (0.88, pose([0.07, 0.56, -0.38], [0.0, 0.78, -0.62], [0.0, -0.62, -0.78]), false),
        ],
        // Drawn far back over the shoulder, its point up, held, and hurled high, the arm coming on
        // down after it
        Power::Stake => vec![
            (0.18, pose([0.36, 0.12, 0.0], [0.05, 0.4, 0.92], [-1.0, 0.0, 0.0]), false),
            (release - 0.06, pose([0.38, 0.06, 0.05], [0.04, 0.48, 0.88], [-1.0, 0.0, 0.0]), false),
            (release, pose([0.18, 0.7, 0.06], [-0.05, 0.75, 0.66], [-1.0, 0.0, 0.0]), true),
            (release + 0.16, pose([0.12, 0.6, -0.25], [0.0, 0.62, -0.78], [-1.0, 0.0, 0.0]), false),
            (0.86, pose([0.3, 0.36, -0.3], [0.04, 0.9, 0.42], [-1.0, 0.0, 0.0]), false),
        ],
        // The bow raised high, canted up at the sky, drawn and held, loosed with a snap
        Power::Cloudburst => vec![
            (0.18, bow_aimed(0.62, 0.0), false),
            (0.4, bow_held(0.62, 0.0), false),
            (release - 0.05, bow_held(0.64, 0.0), false),
            (release, bow_loosed(0.62, 0.0), true),
            (0.8, bow_aimed(0.2, 0.0), false),
        ],
        // Swung up and round to the right, levelled with a twist, settled and loosed, the line's
        // weights tugging it after
        Power::Bolas => {
            let aimed = levelled(0.03, 0.0);
            vec![
                (0.14, levelled(0.2, 0.25), false),
                (0.27, levelled(0.08, -0.08), false),
                (release - 0.05, steadied(&aimed, 1.0), false),
                (release, gun_kicked(&aimed, 1.1), true),
                (release + 0.16, rocked(&aimed, 1.1), false),
                (0.86, levelled(0.0, 0.0), false),
            ]
        }
        // Raised right up to the sky and held, the flare going up with a kick that throws it back
        Power::Starshell => {
            let aimed = levelled(0.78, 0.0);
            vec![(0.18, levelled(0.5, 0.1), false), (release - 0.12, aimed.clone(), false), (release - 0.03, steadied(&aimed, 1.0), false), (release, gun_kicked(&aimed, 1.5), true), (release + 0.2, rocked(&aimed, 1.5), false), (0.86, levelled(0.1, 0.0), false)]
        }
        // Lifted up and held as it gathers, then driven down point first at the ground ahead,
        // as a beacon's set down
        Power::Beacon => {
            let at = spell_at(profile.style);
            vec![
                (0.15, at([0.0, -0.04, 0.1], [0.0, -0.2, 0.98]), false),
                (0.34, at([0.0, -0.06, 0.16], [0.0, -0.3, 0.95]), false),
                (release - 0.05, at([0.01, -0.05, 0.17], [0.02, -0.28, 0.96]), false),
                (release, at([-0.04, 0.2, -0.08], [-0.05, 0.72, -0.69]), true),
                (release + 0.12, at([-0.04, 0.21, -0.11], [-0.05, 0.66, -0.75]), false),
                (0.86, at([-0.01, 0.03, 0.0], [-0.02, 0.4, 0.92]), false),
            ]
        }
        // Drawn far back over the shoulder as the lightning gathers into a spear in the hand, held
        // a beat trembling, and hurled, the arm coming on down after it
        Power::Thunderspear => vec![
            (0.16, pose([0.34, 0.1, 0.02], [0.05, -0.3, 0.95], [1.0, 0.0, 0.0]), false),
            (0.32, pose([0.4, 0.02, 0.1], [0.04, -0.45, 0.89], [1.0, 0.0, 0.0]), false),
            (release - 0.06, pose([0.41, 0.0, 0.11], [0.05, -0.5, 0.86], [1.0, 0.0, 0.0]), false),
            (release, pose([0.16, 0.72, 0.0], [-0.05, 0.85, 0.52], [1.0, 0.0, 0.0]), true),
            (release + 0.16, pose([0.12, 0.62, -0.26], [0.0, 0.7, -0.7], [1.0, 0.0, 0.0]), false),
            (0.86, pose([0.3, 0.38, -0.3], [0.04, 0.4, 0.92], [1.0, 0.0, 0.0]), false),
        ],
        // Raised high and held as the dead are called, then thrust down at the ground ahead
        Power::RaiseDead => {
            let at = spell_at(profile.style);
            vec![
                (0.15, at([0.0, -0.05, 0.12], [0.0, -0.25, 0.97]), false),
                (0.36, at([0.01, -0.07, 0.17], [0.02, -0.35, 0.94]), false),
                (release - 0.05, at([0.0, -0.06, 0.18], [0.0, -0.3, 0.95]), false),
                (release, at([-0.03, 0.22, -0.1], [-0.04, 0.7, -0.71]), true),
                (release + 0.14, at([-0.03, 0.23, -0.13], [-0.04, 0.64, -0.77]), false),
                (0.86, at([-0.01, 0.03, 0.0], [-0.02, 0.4, 0.92]), false),
            ]
        }
        // Swung back low behind and bowled underarm, the arm following through up high
        Power::Shrapnel => vec![
            (0.2, pose([0.3, 0.2, -0.42], [0.1, -0.35, 0.93], [1.0, 0.0, 0.0]), false),
            (release - 0.08, pose([0.28, 0.24, -0.46], [0.08, -0.15, 0.98], [1.0, 0.0, 0.0]), false),
            (release, pose([0.15, 0.66, -0.3], [0.0, 0.9, 0.42], [1.0, 0.0, 0.0]), true),
            (0.74, pose([0.13, 0.6, -0.12], [0.0, 0.55, 0.83], [1.0, 0.0, 0.0]), false),
        ],
        _ => Vec::new(),
    }
}

impl Combat {
    /// Seconds till the power on right click is ready again, for the HUD
    pub fn power_cooldown(&self) -> f64 {
        self.power.cooldown
    }

    /// What a player does with the mouse to use `weapon`'s power, `t` seconds after starting to,
    /// in frames `step` seconds long, for scripts and tests: a heavy blow; a bow drawn and loosed
    /// once it's full; bombs aimed a moment and let go; a siege crossbow braced and loosed; anything
    /// else, right click pressed, held a moment to aim and let go
    pub fn power_controls(&self, weapon: &Weapon, t: f64, step: f64) -> Controls {
        let profile = &weapon.profile;
        let mut c = Controls::default();
        if t < 0.0 {
            return c;
        }
        let used = self.power.cooldown > 0.0;
        let kind = if profile.style == Style::Glaive { Kind::Melee } else { profile.kind };
        let own = profile.power.is_some_and(|p| p.own_move());
        match kind {
            Kind::Ranged if profile.style == Style::Lob => c.aiming = t < 0.3,
            Kind::Ranged if profile.build == Build::None && !own => {
                c.aiming = !used;
                c.heavy = t < step;
                c.attack = !used && self.drawn(profile) >= 1.0;
            }
            Kind::Ranged if profile.build == Build::Siege => {
                c.aiming = !used && t < 2.5;
                c.attack = !used && self.brace >= 1.0;
            }
            _ => {
                c.heavy = t < step;
                c.aiming = t < 0.2;
            }
        }
        c
    }

    /// The weapon in hand's power on right click, if it has one, before anything else it does:
    /// letting it go as the heavy blow lands or the bow's loosed drawn full, starting its own move
    /// on right click, and that move going on. Takes from `controls` what it uses; Q loading a
    /// crossbow or gun early is put in as right click for the rest. True while its own move's
    /// going on, when nothing else is done with the weapon.
    pub(in crate::combat) fn wield_power(&mut self, arena: &mut Arena, index: usize, weapon: &Weapon, controls: &mut Controls, dt: f64) -> bool {
        let profile = &weapon.profile;
        let shoulder = profile.kind == Kind::Ranged && profile.build != Build::None;
        let Some(power) = profile.power else {
            if shoulder && controls.reload {
                controls.heavy = true;
            }
            return false;
        };
        // Its own move going on, what it lets go leaving the weapon as the move lets it go: once,
        // or a katar's fists one after another
        if let Action::Power { t, length } = self.action {
            for (shot, at) in power_releases(power).into_iter().enumerate() {
                let at = at * length;
                if t <= at && t + dt > at {
                    self.unleash_power(arena, weapon, power, shot);
                }
            }
            self.action = if t + dt >= length { Action::Idle } else { Action::Power { t: t + dt, length } };
            return true;
        }
        if matches!(self.action, Action::Special { .. }) || self.thrown.is_some() || self.raise < 1.0 {
            self.power.aiming = false;
            return false;
        }
        let ready = self.power.cooldown <= 0.0;
        let kind = if profile.style == Style::Glaive { Kind::Melee } else { weapon.kind() };
        // Aimed first: right click pressed with it ready only starts aiming it, its marks showing
        // where it'll go while it's held, and it goes off as it's let go. Not bombs, which do so
        // their own way below, nor the heavy blow or the bow drawn full that carry it, nor a
        // braced siege crossbow, nor letting go of a harpoon's line.
        let own = power.own_move() && (kind == Kind::Melee || (kind == Kind::Ranged && profile.build == Build::None && profile.style != Style::Lob));
        let line_out = profile.build == Build::Harpoon && (self.tether.is_some() || self.projectiles.iter().any(|p| p.trick == Trick::Harpoon));
        let shouldered = kind == Kind::Ranged && profile.build != Build::None && profile.build != Build::Siege && profile.style != Style::Lob && !line_out;
        if own || shouldered || kind == Kind::Magic {
            if self.power.aiming {
                if controls.aiming {
                    // Held, aiming: right click does nothing else meanwhile, no heavy blow, no draw;
                    // a priest's spear forming in the hand meanwhile, greater the longer it's held
                    self.power.aimed += dt;
                    if power.spear() {
                        let hand = arena.view.eye + arena.view.rotation * Vector3::new(0.36, 0.1, 0.22) * arena.view.vpm;
                        let formed = (self.power.aimed / super::faith::SPEAR_FORMS).min(1.0);
                        self.spear_gathers(hand, formed, dt);
                    }
                    controls.heavy = false;
                    controls.aiming = false;
                    return false;
                }
                // Let go: off it goes, as a press used to set it off
                self.power.aiming = false;
                self.power.charge = self.spear_charge();
                controls.heavy = true;
            } else if controls.heavy && ready && controls.aiming {
                self.power.aiming = true;
                self.power.aimed = 0.0;
                self.power.charge = 0.0;
                controls.heavy = false;
                controls.aiming = false;
                return false;
            }
        } else {
            self.power.aiming = false;
        }
        // A power with a move of its own on a melee weapon or a bow: right click sets it going
        // when it's ready, and while it comes round right click swings heavy or draws as ever
        if own {
            if controls.heavy && ready && self.cooldown <= 0.0 && matches!(self.action, Action::Idle) {
                controls.heavy = false;
                self.power.cooldown = power.cooldown();
                self.action = Action::Power { t: 0.0, length: power_timing(power).0 };
                self.cooldown = power_timing(power).0;
                let pitch = if kind == Kind::Melee { 0.8 } else { 1.0 };
                self.cues.push(Cue::here(Sound::HeavySwing, 0.5).pitched(pitch));
                return true;
            }
            return false;
        }
        match kind {
            // The heavy blow carries it, letting it go just as the blow lands
            Kind::Melee => {
                if let Action::Swing { t, length, heavy: true, struck: false, combo } = self.action
                    && ready
                {
                    let lands = blow_times(profile, true, combo).1;
                    let step = if self.hitstop > 0.0 { dt * HITSTOP_PACE } else { dt };
                    if t + step >= length * lands {
                        self.power.cooldown = power.cooldown();
                        self.unleash_power(arena, weapon, power, 0);
                    }
                }
            }
            // Bombs: aimed with right click held, the ring showing where it'll come down, and
            // thrown as it's let go of
            Kind::Ranged if profile.style == Style::Lob => {
                if controls.aiming {
                    self.power.held += dt;
                } else {
                    if self.power.held > 0.15 && !matches!(self.action, Action::Throw { .. }) {
                        if ready {
                            self.power.held = 0.0;
                            self.power.cooldown = power.cooldown();
                            self.action = Action::Power { t: 0.0, length: power_timing(power).0 };
                            self.cues.push(Cue::here(Sound::Fuse, 0.7).pitched(0.8));
                            return true;
                        }
                        self.not_ready(power, profile);
                    }
                    self.power.held = 0.0;
                }
            }
            // A bow drawn full looses it in place of an arrow
            Kind::Ranged if profile.build == Build::None => {
                if matches!(self.action, Action::Draw { .. }) && controls.attack && ready && self.drawn(profile) >= 1.0 {
                    self.power.cooldown = power.cooldown();
                    self.unleash_power(arena, weapon, power, 0);
                    self.action = Action::Loose { t: 0.0 };
                    self.cooldown = 0.25;
                    controls.attack = false;
                }
            }
            Kind::Ranged => {
                let left = self.rounds_in(index, profile);
                if profile.build == Build::Siege {
                    // Braced, the first bolt loosed is it
                    let loosing = controls.attack || matches!(self.action, Action::Brace { loose: true, .. });
                    if matches!(self.action, Action::Brace { .. }) && self.brace >= 1.0 && self.cooldown <= 0.0 && ready && loosing && left > 0 {
                        self.power.cooldown = power.cooldown();
                        self.loads.insert(index, left - 1);
                        self.unleash_power(arena, weapon, power, 0);
                        self.action = Action::Fire { t: 0.0, length: 0.5 };
                        self.cooldown = 0.5;
                        controls.attack = false;
                    }
                } else {
                    // Right click shoots it, but for letting go of a harpoon's line
                    if controls.heavy && !line_out {
                        controls.heavy = false;
                        if ready && matches!(self.action, Action::Idle | Action::Fire { .. } | Action::Reload { .. }) {
                            self.power.cooldown = power.cooldown();
                            self.action = Action::Power { t: 0.0, length: power_timing(power).0 };
                            return true;
                        }
                        if !ready {
                            self.not_ready(power, profile);
                        }
                    }
                }
                if controls.reload {
                    controls.heavy = true;
                }
            }
            // Cast with a move of its own, for its mana
            Kind::Magic => {
                if controls.heavy && self.cooldown <= 0.0 && !matches!(self.action, Action::Channel { .. } | Action::Charge { .. }) {
                    let cost = profile.stat("manaCost") * power.mana();
                    if !ready {
                        self.not_ready(power, profile);
                    } else if self.mana < cost {
                        if self.mana_warning <= 0.0 {
                            self.messages.push("Not enough mana.".into());
                            self.mana_warning = 2.0;
                        }
                    } else {
                        self.mana -= cost;
                        self.power.cooldown = power.cooldown();
                        self.action = Action::Power { t: 0.0, length: power_timing(power).0 };
                        self.casts = self.casts.wrapping_add(1);
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Saying the power's not ready yet, once
    fn not_ready(&mut self, power: Power, profile: &Profile) {
        let said = format!("{} ready in {:.0} s.", power.name(profile.element), self.power.cooldown.ceil().max(1.0));
        if self.messages.last() != Some(&said) {
            self.messages.push(said);
        }
    }

    /// Where the weapon in hand is in its power's own move, `f` of the way through, from `rest`
    pub(in crate::combat) fn power_pose(&self, weapon: &Weapon, rest: &Pose, f: f64) -> Pose {
        let Some(power) = weapon.profile.power else {
            return rest.clone();
        };
        let mut keys = power_keys(power, &weapon.profile);
        // A hand crossbow goes through it on the left
        if weapon.profile.build == Build::Hand {
            for key in &mut keys {
                key.1 = mirrored_shouldered(&key.1);
            }
        }
        animate(rest, &keys, f)
    }

    /// The power let go: what it shoots, hurls or sets off leaving the weapon, the `shot`th of
    /// what its move lets go, see power_releases()
    fn unleash_power(&mut self, arena: &mut Arena, weapon: &Weapon, power: Power, shot: usize) {
        let view = arena.view;
        let vpm = view.vpm;
        let up = Vector3::z();
        let profile = weapon.profile.clone();
        let element = profile.element;
        let look = Element::look(element);
        let light = element.map_or([0.95, 0.9, 0.8], Element::light);
        let magic = profile.kind == Kind::Magic && profile.style != Style::Glaive;
        let base = if magic { profile.stat("spellDamage") } else { profile.stat("physicalDamage") } + profile.stat("elementalDamage");
        let feet = arena.player.unwrap_or(view.eye - up * 1.6 * vpm);
        let forward = view.forward();
        let flat = flat_of(forward);
        // Along where it's looked, never much up or down, so it runs on over the ground
        let pitch = forward.z.clamp(-0.18, 0.25);
        let level = (flat * (1.0 - pitch * pitch).sqrt() + up * pitch).normalize();
        // What the crosshair's on, or as far as the power reaches along it, see aim.rs
        let points = self.power_points(arena, power);
        let target = points.get(points.len() / 2).map_or_else(|| self.aim(arena), |p| p.0);
        let set = |c: &mut Combat, shape: Shape, range: f64, size: f64| {
            c.signed(Sig::Power(shape));
            if let Some(p) = c.projectiles.last_mut() {
                p.range = range * vpm;
                p.size = size;
            }
        };
        // The third batch's, see arsenal.rs
        if self.unleash_arsenal(arena, weapon, power, base) || self.unleash_miracle(arena, weapon, power, shot, base) {
            return;
        }
        match power {
            Power::Starbreak | Power::Shadowstep | Power::Ringstorm | Power::Spectrum | Power::Runeburst => {}
            Power::Vigil | Power::Heavenfall | Power::Triune | Power::Stormcall | Power::Glacier | Power::Shadowgrasp | Power::Helix => {}
            // Crescents flying on from the blow, out of the chest, or a whip's low over the ground
            // Each at what the crosshair, or its fan either side, first meets within its reach, or
            // as far as that, as its rings showed, see aim.rs; a whip's on along the ground
            Power::Crescent | Power::Rending | Power::WindCut | Power::BladeFan | Power::LashWave => {
                let (_, width, speed, reach, share) = wave_of(power);
                let low = power == Power::LashWave;
                let from = if low { ground_under(arena.world, feet + flat * LASH_FROM * vpm + up * vpm, vpm) + up * LASH_HIGH * vpm } else { view.eye + view.rotation * Vector3::new(0.05, 0.9, -0.35) * vpm };
                if low {
                    self.launch(&profile, None, Form::Bolt, from, flat * speed * vpm, base * share, 0.0);
                    set(self, Shape::Wave { width: width as f32, low: true }, reach, 1.0);
                    let line = match power_way(power) {
                        PowerWay::Line { from, step, steps } => Combat::power_line(arena, from, step, steps),
                        _ => Vec::new(),
                    };
                    self.keep_power_marks(power, &line, reach / speed);
                } else {
                    for (to, who) in self.power_points(arena, power) {
                        let along = (to - from).try_normalize(1e-6).unwrap_or(level);
                        // On through what it cuts, or as far as where it meets the ground
                        let range = (to - from).norm() / vpm + if who.is_some() { 3.0 } else { 0.3 };
                        self.launch(&profile, None, Form::Bolt, from, along * speed * vpm, base * share, 0.0);
                        set(self, Shape::Wave { width: width as f32, low: false }, range, 1.0);
                        if who.is_none() {
                            self.keep_power_marks(power, &[to], range / speed);
                        }
                    }
                }
                self.cues.push(Cue::here(Sound::Shing, 0.9).pitched(if power == Power::Rending { 0.7 } else { 1.1 }));
                self.cues.push(Cue::here(Sound::HeavySwing, 0.6).pitched(0.8));
                self.flashes.push(Flash { position: from, color: light, reach: 6.0, life: 0.2, lifetime: 0.2 });
                self.sparks(from, 12, 3.0, look, 0.0, 0.35);
            }
            // A wheel hurled out spinning, to come back
            Power::Wheel => {
                let from = view.eye + view.rotation * Vector3::new(0.1, 1.0, -0.3) * vpm;
                // Out to what the crosshair first meets, or as far as it goes, and back
                let (to, who) = self.power_points(arena, power)[0];
                let along = (to - from).try_normalize(1e-6).unwrap_or(level);
                let range = ((to - from).norm() / vpm + if who.is_some() { 1.5 } else { 0.0 }).min(WHEEL_REACH);
                self.launch(&profile, None, Form::Bolt, from, along * WHEEL_SPEED * vpm, base * 1.3, 0.0);
                set(self, Shape::Wheel { back: false }, range, 1.0);
                if who.is_none() {
                    self.keep_power_marks(power, &[to], range / WHEEL_SPEED);
                }
                self.cues.push(Cue::here(Sound::Whirr, 0.9).pitched(0.8));
                self.cues.push(Cue::here(Sound::Shing, 0.6).pitched(0.9));
                self.flashes.push(Flash { position: from, color: light, reach: 5.0, life: 0.2, lifetime: 0.2 });
            }
            // A spear of light hurled at the crosshair
            Power::Javelin => {
                let from = view.eye + view.rotation * Vector3::new(0.2, 0.7, -0.15) * vpm;
                // At what the crosshair first meets within its reach, or bursting as far as that
                let (to, who) = self.power_points(arena, power)[0];
                let way = lead(from, to, 34.0, 0.15, vpm);
                let range = (to - from).norm() / vpm * 1.03 + if who.is_some() { 3.0 } else { 0.3 };
                self.launch(&profile, None, Form::Bolt, from, way * 34.0 * vpm, base * 2.0, 0.15);
                set(self, Shape::Javelin, range, 1.0);
                if who.is_none() {
                    self.keep_power_marks(power, &[to], range / 34.0);
                }
                self.cues.push(Cue::here(Sound::Shing, 0.8).pitched(0.75));
                self.cues.push(Cue::here(Sound::Swing, 0.7).pitched(0.6));
                self.flashes.push(Flash { position: from, color: light, reach: 5.0, life: 0.2, lifetime: 0.2 });
            }
            // The ground split open in a line ahead, step by step
            Power::Fissure => {
                // Along the ground the way it's aimed, as its rings showed, see aim.rs
                let start = ground_under(arena.world, feet + flat * FISSURE_FROM * vpm + up * vpm, vpm);
                let line = Combat::power_line(arena, FISSURE_FROM, FISSURE_STEP, FISSURE_STEPS as usize);
                self.keep_power_marks(power, &line, FISSURE_STEPS * FISSURE_GAP + 0.2);
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Fissure)), &profile, start, flat * FISSURE_STEP * vpm, 1.6, FISSURE_STEPS * FISSURE_GAP + 0.3, base * 1.5);
                self.cues.push(Cue::here(Sound::Quake, 1.0).pitched(1.1));
                self.cues.push(Cue::here(Sound::Slam, 0.8).pitched(0.8));
                self.shake = self.shake.max(0.6);
            }
            // An arrow like a comet, from the nock at the crosshair
            Power::CometArrow => {
                let from = self.held_point(weapon, view, true);
                let way = lead(from, target, 50.0, 0.25, vpm);
                self.launch(&profile, Some(Missile { shot: Shot::Arrow, head: weapon.arrowhead(), power: true, size: 1.5 }), Form::Bolt, from, way * 50.0 * vpm, base * 2.2, 0.25);
                set(self, Shape::Arrow, f64::INFINITY, 1.5);
                self.cues.push(Cue::here(Sound::Twang, 1.0).pitched(0.8));
                self.cues.push(Cue::here(Sound::Shing, 0.5).pitched(1.4));
                self.flashes.push(Flash { position: from, color: light, reach: 6.0, life: 0.25, lifetime: 0.25 });
                self.shake = self.shake.max(0.3);
            }
            Power::BurstBolt => {
                let from = self.held_point(weapon, view, false);
                let way = lead(from, target, 55.0, 0.3, vpm);
                self.launch(&profile, Some(Missile { shot: Shot::Quarrel, head: 0, power: true, size: 1.7 }), Form::Bolt, from, way * 55.0 * vpm, base * 1.5, 0.3);
                set(self, Shape::Bolt, f64::INFINITY, 1.7);
                self.cues.push(Cue::here(Sound::Release, 1.0).pitched(0.8));
                self.cues.push(Cue::here(Sound::Zap, 0.4).pitched(0.7));
                self.flashes.push(Flash { position: from, color: light, reach: 5.0, life: 0.2, lifetime: 0.2 });
                self.kick += 0.04;
            }
            // A gun's: going off as its shots do, see gun_report(), harder
            Power::Ricochet | Power::LanceShot | Power::Breath | Power::Mortar => {
                let from = self.held_point(weapon, view, false);
                match power {
                    Power::Ricochet => {
                        let way = lead(from, target, 42.0, 0.1, vpm);
                        self.launch(&profile, Some(Missile { shot: Shot::Ball, head: 0, power: true, size: 1.4 }), Form::Bolt, from, way * 42.0 * vpm, base * 1.6, 0.1);
                        set(self, Shape::Slug(4), f64::INFINITY, 1.4);
                        self.gun_report(weapon, view, &profile, from, way, 1.3);
                    }
                    Power::LanceShot => {
                        let way = lead(from, target, 70.0, 0.0, vpm);
                        self.launch(&profile, Some(Missile { shot: Shot::Ball, head: 0, power: true, size: 1.4 }), Form::Bolt, from, way * 70.0 * vpm, base * 2.0, 0.0);
                        set(self, Shape::Lance, LANCE_REACH, 1.4);
                        self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Line)), &profile, from + way * 1.5 * vpm, way * (LANCE_REACH - 1.5) * vpm, 1.4, LINE_SECONDS + 0.2, base * 0.8);
                        if let Some(z) = self.zones.last_mut() {
                            z.tick = LINE_SECONDS;
                        }
                        self.gun_report(weapon, view, &profile, from, way, 1.5);
                    }
                    Power::Breath => {
                        let way = lead(from, target, 20.0, 0.15, vpm);
                        let across = way.cross(&up).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                        // Fanned out across the cone its rings showed, see aim.rs, the outer ones
                        // as far round as the rings either side
                        for k in 0..7 {
                            let s = (k as f64 - 3.0) / 3.0;
                            let along = (way + across * (s * crate::combat::aim::BREATH_FAN[2]).tan() + up * (0.04 * (k % 2) as f64)).normalize();
                            self.launch(&profile, Some(Missile { shot: Shot::Ball, head: 0, power: true, size: 1.0 }), Form::Bolt, from, along * (18.0 + 2.0 * (k % 3) as f64) * vpm, base * 0.7, 0.15);
                            set(self, Shape::Ember, BREATH_REACH, 1.0);
                        }
                        self.gun_report(weapon, view, &profile, from, way, 1.6);
                    }
                    // Onto the ring shown as it was aimed, on the ground, see aim.rs
                    _ => {
                        let aim = self.power_spot(arena, weapon).unwrap_or(target);
                        let time = 1.1 + 0.015 * (aim - from).norm() / vpm;
                        self.keep_power_ring(weapon, aim, time);
                        self.launch(&profile, Some(Missile { shot: Shot::Shell, head: 0, power: true, size: 1.4 }), Form::Bolt, from, thrown(from, aim, time, 1.0, vpm), base * 2.2, 1.0);
                        set(self, Shape::Shell, f64::INFINITY, 1.4);
                        self.gun_report(weapon, view, &profile, from, flat, 1.8);
                    }
                }
                self.kick += 0.06;
            }
            // A charge lobbed at where the ring was, the ring kept showing till it's there
            Power::Vortex => {
                let (aim, from, velocity, time) = self.lob_aim(arena, weapon);
                self.keep_power_ring(weapon, aim, time);
                self.launch(&profile, Some(Missile { shot: Shot::Shell, head: 0, power: true, size: 1.2 }), Form::Bolt, from, velocity, base * 2.0, BOMB_GRAVITY);
                set(self, Shape::Charge, f64::INFINITY, 1.2);
                self.cues.push(Cue::here(Sound::Swing, 0.7).pitched(0.65));
                self.cues.push(Cue::here(Sound::Hex, 0.5).pitched(0.7));
            }
            // Drifting down onto the ring shown as it was aimed, see aim.rs
            Power::Comet => {
                let from = self.held_point(weapon, view, false);
                let aim = self.power_spot(arena, weapon).unwrap_or(target);
                let time = ((aim - from).norm() / (15.0 * vpm)).max(0.35);
                self.keep_power_ring(weapon, aim, time);
                self.launch(&profile, None, Form::Bolt, from, thrown(from, aim, time, 0.12, vpm), base * 2.4, 0.12);
                set(self, Shape::Orb, f64::INFINITY, 1.0);
                self.cues.push(Cue::here(Sound::Summon, 0.9).pitched(0.6));
                self.flashes.push(Flash { position: from, color: light, reach: 8.0, life: 0.35, lifetime: 0.35 });
                self.shake = self.shake.max(0.3);
            }
            // Motes fanned out up and round, each then seeking what's near
            // At what's round what the crosshair's on, as the marks over them showed, each mote in
            // turn after the next of them; or with nothing there, onto the ring where it's on, see
            // aim.rs
            Power::Seekers => {
                let from = self.held_point(weapon, view, false);
                let (targets, at) = Combat::seek_targets(arena, SEEKER_REACH);
                let way = (at - from).try_normalize(1e-6).unwrap_or(forward);
                let across = way.cross(&up).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                for k in 0..5 {
                    let s = (k as f64 - 2.0) / 2.0;
                    let goal = targets.get(k % targets.len().max(1)).map_or(at, |&i| arena.animals[i].center(vpm as u32));
                    let toward = (goal - from).try_normalize(1e-6).unwrap_or(way);
                    // Fanned out wide after what they seek, or close round the way onto the ring
                    let spread = if targets.is_empty() { 0.3 } else { 1.0 };
                    let along = (toward + (across * s * 0.35 + up * (0.18 - 0.08 * s.abs())) * spread).normalize();
                    self.launch(&profile, None, Form::Bolt, from, along * 16.0 * vpm, base * 0.8, 0.0);
                    set(self, Shape::Mote, if targets.is_empty() { (at - from).norm() / vpm + 0.3 } else { SEEKER_REACH }, 1.0);
                    if let Some(p) = self.projectiles.last_mut() {
                        p.seek = if targets.is_empty() { 0.0 } else { 6.0 };
                    }
                }
                if targets.is_empty() {
                    self.keep_power_marks(power, &[at], (at - from).norm() / (16.0 * vpm));
                }
                self.cues.push(Cue::here(Sound::Summon, 0.7).pitched(1.4));
                self.flashes.push(Flash { position: from, color: light, reach: 5.0, life: 0.25, lifetime: 0.25 });
            }
            // A ring rolling out all round
            Power::Nova => {
                let center = feet + up * 0.3 * vpm;
                self.keep_power_marks(power, &[feet + up * 0.05 * vpm], NOVA_REACH / NOVA_SPEED);
                self.landed(center);
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Nova)), &profile, center, Vector3::zeros(), NOVA_REACH, NOVA_REACH / NOVA_SPEED + 0.35, base * 1.5);
                self.cues.push(Cue::here(Sound::Toll, 1.0).pitched(0.7));
                self.cues.push(Cue::here(Sound::Burst, 0.6).pitched(0.8));
                self.flashes.push(Flash { position: center + up * vpm, color: light, reach: 12.0, life: 0.4, lifetime: 0.4 });
                self.shake = self.shake.max(0.5);
            }
            // A funnel sent off along the ground the way it's aimed, as its rings showed
            Power::Gale => {
                let start = ground_under(arena.world, feet + flat * GALE_FROM * vpm + up * vpm, vpm) + up * 0.7 * vpm;
                let reach = GALE_STEP * (GALE_STEPS - 1) as f64;
                self.launch(&profile, None, Form::Bolt, start, flat * GALE_SPEED * vpm, base * 1.2, 0.0);
                set(self, Shape::Gale, reach, 1.0);
                let line = Combat::power_line(arena, GALE_FROM, GALE_STEP, GALE_STEPS);
                self.keep_power_marks(power, &line, reach / GALE_SPEED);
                self.cues.push(Cue::here(Sound::Whirr, 1.0).pitched(0.6));
                self.cues.push(Cue::here(Sound::HeavySwing, 0.7).pitched(0.7));
                self.sparks(start, 14, 3.5, look, 0.0, 0.4);
            }
            // A fist punched out from the hand at what the crosshair's on, as its ring showed
            Power::Fists => {
                let low = (shot % 2) as f64;
                let from = view.eye + view.rotation * Vector3::new(0.12 - 0.04 * low, 0.85, -0.2 - 0.05 * low) * vpm;
                let (to, who) = self.power_points(arena, power)[0];
                let along = (to - from).try_normalize(1e-6).unwrap_or(level);
                let range = (to - from).norm() / vpm + if who.is_some() { 1.0 } else { 0.3 };
                self.launch(&profile, None, Form::Bolt, from, along * FIST_SPEED * vpm, base * 0.55, 0.0);
                set(self, Shape::Fist, range, 1.0);
                if who.is_none() {
                    self.keep_power_marks(power, &[to], range / FIST_SPEED);
                }
                self.cues.push(Cue::here(Sound::Swing, 0.6).pitched(1.5 + 0.08 * shot as f32));
                self.flashes.push(Flash { position: from, color: light, reach: 3.0, life: 0.12, lifetime: 0.12 });
                self.sparks(from, 5, 2.5, look, 0.0, 0.25);
            }
            // A ring of blades thrown out all round the player, and drawn back in
            Power::BladeRing => {
                self.landed(feet + up * 0.9 * vpm);
                self.keep_power_marks(power, &[feet + up * 0.05 * vpm], RING_SECONDS * 0.5);
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::BladeRing)), &profile, feet + up * 0.9 * vpm, Vector3::zeros(), RING_REACH, RING_SECONDS, base * 1.1);
                self.cues.push(Cue::here(Sound::Whirr, 1.0).pitched(0.75));
                self.cues.push(Cue::here(Sound::Shing, 0.8).pitched(0.9));
                self.flashes.push(Flash { position: feet + up * vpm, color: light, reach: 8.0, life: 0.25, lifetime: 0.25 });
            }
            // A hook flung at what the crosshair first meets, or as far as it goes, and back
            Power::Hook => {
                let from = view.eye + view.rotation * Vector3::new(0.12, 0.9, -0.15) * vpm;
                let (to, who) = self.power_points(arena, power)[0];
                let along = (to - from).try_normalize(1e-6).unwrap_or(level);
                let range = ((to - from).norm() / vpm + if who.is_some() { 1.0 } else { 0.2 }).min(HOOK_REACH + 1.0);
                self.launch(&profile, None, Form::Bolt, from, along * HOOK_SPEED * vpm, base * 1.2, 0.0);
                set(self, Shape::Hook { back: false, caught: None }, range, 1.0);
                if who.is_none() {
                    self.keep_power_marks(power, &[to], range / HOOK_SPEED);
                }
                self.cues.push(Cue::here(Sound::Whirr, 0.8).pitched(1.3));
                self.cues.push(Cue::here(Sound::Clang, 0.4).pitched(1.2));
                self.flashes.push(Flash { position: from, color: light, reach: 4.0, life: 0.15, lifetime: 0.15 });
            }
            // An anvil coming down out of the sky onto the ring shown as it was aimed, see aim.rs
            Power::Anvil => {
                let aim = self.power_spot(arena, weapon).unwrap_or(target);
                let from = aim + up * ANVIL_HEIGHT * vpm;
                self.keep_power_ring(weapon, aim, ANVIL_FALL + 0.15);
                self.launch(&profile, None, Form::Bolt, from, thrown(from, aim, ANVIL_FALL, 3.0, vpm), base * 2.4, 3.0);
                set(self, Shape::Anvil, f64::INFINITY, 1.0);
                self.cues.push(Cue::here(Sound::Slam, 0.6).pitched(0.7));
                self.cues.push(Cue::at(Sound::Summon, from, 0.7).pitched(0.5));
                self.flashes.push(Flash { position: from, color: light, reach: 10.0, life: 0.3, lifetime: 0.3 });
                self.shake = self.shake.max(0.3);
            }
            // A spear hurled high to come down on the ring shown as it was aimed
            Power::Stake => {
                let from = view.eye + view.rotation * Vector3::new(0.2, 0.7, 0.1) * vpm;
                let aim = self.power_spot(arena, weapon).unwrap_or(target);
                let time = 0.75 + 0.012 * (aim - from).norm() / vpm;
                self.keep_power_ring(weapon, aim, time + 0.1);
                self.launch(&profile, None, Form::Bolt, from, thrown(from, aim, time, 1.0, vpm), base * 1.6, 1.0);
                set(self, Shape::Stake, f64::INFINITY, 1.0);
                self.cues.push(Cue::here(Sound::Swing, 0.8).pitched(0.6));
                self.cues.push(Cue::here(Sound::Shing, 0.6).pitched(0.8));
                self.flashes.push(Flash { position: from, color: light, reach: 5.0, life: 0.2, lifetime: 0.2 });
            }
            // An arrow loosed so it tops out over the ring shown as it was aimed, bursting there
            Power::Cloudburst => {
                let from = self.held_point(weapon, view, true);
                let aim = self.power_spot(arena, weapon).unwrap_or(target);
                let (velocity, time) = topping_out(from, aim + up * CLOUD_HEIGHT * vpm, vpm);
                self.keep_power_ring(weapon, aim, time + DOWNPOUR_SECONDS + 0.6);
                self.launch(&profile, Some(Missile { shot: Shot::Arrow, head: weapon.arrowhead(), power: true, size: 1.4 }), Form::Bolt, from, velocity, base * 1.1, 1.0);
                set(self, Shape::Cloud, f64::INFINITY, 1.4);
                self.cues.push(Cue::here(Sound::Twang, 1.0).pitched(0.7));
                self.cues.push(Cue::here(Sound::Whirr, 0.5).pitched(1.6));
                self.flashes.push(Flash { position: from, color: light, reach: 5.0, life: 0.2, lifetime: 0.2 });
            }
            Power::Bolas => {
                let from = self.held_point(weapon, view, false);
                let way = lead(from, target, 40.0, 0.2, vpm);
                self.launch(&profile, Some(Missile { shot: Shot::Ball, head: 0, power: true, size: 1.0 }), Form::Bolt, from, way * 40.0 * vpm, base * 1.3, 0.2);
                set(self, Shape::Bolas, f64::INFINITY, 1.0);
                self.cues.push(Cue::here(Sound::Release, 1.0).pitched(0.7));
                self.cues.push(Cue::here(Sound::Whirr, 0.6).pitched(1.4));
                self.flashes.push(Flash { position: from, color: light, reach: 4.0, life: 0.15, lifetime: 0.15 });
                self.kick += 0.04;
            }
            // A flare shot so it tops out over the ring shown as it was aimed, bursting to hang there
            Power::Starshell => {
                let from = self.held_point(weapon, view, false);
                let aim = self.power_spot(arena, weapon).unwrap_or(target);
                let (velocity, time) = topping_out(from, aim + up * FLARE_HEIGHT * vpm, vpm);
                self.keep_power_ring(weapon, aim, time + FLARE_SECONDS);
                self.launch(&profile, Some(Missile { shot: Shot::Ball, head: 0, power: true, size: 1.3 }), Form::Bolt, from, velocity, base * 1.0, 1.0);
                set(self, Shape::Flare, f64::INFINITY, 1.3);
                self.gun_report(weapon, view, &profile, from, velocity.try_normalize(1e-6).unwrap_or(up), 1.4);
                self.kick += 0.06;
            }
            // A beacon drifting down onto the ring shown as it was aimed
            Power::Beacon => {
                let from = self.held_point(weapon, view, false);
                let aim = self.power_spot(arena, weapon).unwrap_or(target);
                let time = ((aim - from).norm() / (18.0 * vpm)).max(0.35);
                self.keep_power_ring(weapon, aim, time + BEACON_TOLLS * BEACON_TOLL);
                self.launch(&profile, None, Form::Bolt, from, thrown(from, aim, time, 0.3, vpm), base * 1.4, 0.3);
                set(self, Shape::Beacon, f64::INFINITY, 1.0);
                self.cues.push(Cue::here(Sound::Summon, 0.9).pitched(0.9));
                self.flashes.push(Flash { position: from, color: light, reach: 7.0, life: 0.3, lifetime: 0.3 });
            }
            // A great spear of holy lightning hurled at what the crosshair first meets, as its
            // marks showed, or bursting as far as it goes, see faith.rs
            // Greater and brighter, and striking harder, the longer it was held, see faith.rs
            Power::Thunderspear => {
                let from = view.eye + view.rotation * Vector3::new(0.2, 0.75, 0.0) * vpm;
                let (to, who) = self.power_points(arena, power)[0];
                let range = (to - from).norm() / vpm * 1.03 + if who.is_some() { 4.0 } else { 0.3 };
                let charge = self.power.charge;
                self.hurl_spear(&profile, from, to, base * 2.2 * (0.8 + 0.7 * charge), range, true);
                if let Some(p) = self.projectiles.last_mut() {
                    p.size = 1.5 + 0.9 * charge;
                }
                if who.is_none() {
                    self.keep_power_marks(power, &[to], range / 32.0);
                }
                self.flashes.push(Flash { position: from, color: Element::Storm.light(), reach: 6.0 + 6.0 * charge, life: 0.2, lifetime: 0.2 });
                self.shake = self.shake.max(0.2 + 0.25 * charge);
            }
            // A shade sent down onto the ring shown as it was aimed, to raise the dead there
            Power::RaiseDead => {
                let from = self.held_point(weapon, view, false);
                let aim = self.power_spot(arena, weapon).unwrap_or(target);
                let time = ((aim - from).norm() / (16.0 * vpm)).max(0.35);
                self.keep_power_ring(weapon, aim, time + 0.3);
                self.send_shade(&profile, from, aim, time, base);
                self.flashes.push(Flash { position: from, color: [0.6, 0.1, 0.45], reach: 6.0, life: 0.3, lifetime: 0.3 });
            }
            // A pot lobbed at where the bombs' ring was, the ring kept showing till it's there
            Power::Shrapnel => {
                let (aim, from, velocity, time) = self.lob_aim(arena, weapon);
                self.keep_power_ring(weapon, aim, time);
                self.launch(&profile, Some(Missile { shot: Shot::Shell, head: 0, power: true, size: 1.1 }), Form::Bolt, from, velocity, base * 1.5, BOMB_GRAVITY);
                set(self, Shape::Pot, f64::INFINITY, 1.1);
                self.cues.push(Cue::here(Sound::Swing, 0.7).pitched(0.8));
                self.cues.push(Cue::here(Sound::Fuse, 0.5).pitched(1.1));
            }
        }
    }

    /// What it does to animal `i` it strikes at `at` for `damage`, as its element has it, from
    /// `from`
    pub(super) fn power_hit(&mut self, arena: &mut Arena, profile: &Rc<Profile>, i: usize, at: Vector3<f64>, damage: f64, from: Vector3<f64>) {
        let element = profile.element;
        self.element_hit(at, element, 1.3);
        self.sparks(at, 8, 3.0, Element::look(element), 0.6, 0.35);
        self.cues.push(Cue::at(Sound::Hit, at, 0.8));
        self.strike(arena, profile, i, Blow { damage, crit: false, heavy: true, from, depth: 0, element });
        self.empower(arena, profile, i, at, damage, from);
    }

    /// What its element does to animal `i` it struck at `at` for `damage`, from `from`: burning,
    /// freezing, lightning leaping on, poisoning, throwing back, dragging in, bleeding it and healing
    /// the player, or staggering it
    fn empower(&mut self, arena: &mut Arena, profile: &Rc<Profile>, i: usize, at: Vector3<f64>, damage: f64, from: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let element = profile.element;
        let alive = arena.animals[i].health > 0.0;
        let boss = arena.animals[i].boss.is_some();
        let away = (arena.animals[i].position - from).try_normalize(1e-6).map_or(Vector3::zeros(), |v| Vector3::new(v.x, v.y, 0.0));
        match element {
            Some(Element::Fire) if alive => {
                let s = &mut arena.animals[i].status;
                s.burn = s.burn.max(3.0);
                s.burn_dps = s.burn_dps.max(damage * 0.12);
            }
            Some(Element::Frost) if alive => {
                let s = &mut arena.animals[i].status;
                if s.chill >= 0.5 && !boss {
                    s.frozen = s.frozen.max(1.2);
                }
                s.chill = (s.chill + 0.5).min(1.0);
                s.slowed = s.slowed.max(2.0);
                s.slow = s.slow.max(0.4);
            }
            // Lightning leaping on to the nearest other
            Some(Element::Storm) => {
                if alive {
                    arena.animals[i].status.shocked = arena.animals[i].status.shocked.max(0.5);
                }
                let id = arena.animals[i].id;
                let next = Combat::foes_round(arena, at, 5.0).into_iter().find(|&j| arena.animals[j].id != id);
                if let Some(j) = next {
                    let to = arena.animals[j].center(vpm as u32);
                    self.arc(at, to);
                    self.strike(arena, profile, j, Blow { damage: damage * 0.4, crit: false, heavy: false, from: at, depth: 1, element: Some(Element::Storm) });
                    if arena.animals[j].health > 0.0 {
                        arena.animals[j].status.shocked = arena.animals[j].status.shocked.max(0.4);
                    }
                }
            }
            Some(Element::Poison) if alive => {
                let s = &mut arena.animals[i].status;
                s.poison = s.poison.max(4.0);
                s.poison_stacks = (s.poison_stacks + 1).min(5);
                s.poison_dps = s.poison_dps.max(damage * 0.1);
            }
            Some(Element::Arcane) if alive && !boss => {
                arena.animals[i].status.push = away * 9.0 * vpm;
                arena.animals[i].status.staggered = arena.animals[i].status.staggered.max(0.3);
            }
            // What's round it dragged in to it
            Some(Element::Void) => {
                for j in Combat::foes_round(arena, at, 4.0) {
                    let to = at - arena.animals[j].position;
                    if arena.animals[j].boss.is_none() && to.norm() > 0.5 * vpm {
                        arena.animals[j].status.push = Vector3::new(to.x, to.y, 0.0).normalize() * 5.0 * vpm;
                    }
                }
            }
            Some(Element::Blood) => {
                if alive {
                    let s = &mut arena.animals[i].status;
                    s.bleed = s.bleed.max(4.0);
                    s.bleed_stacks = (s.bleed_stacks + 1).min(5);
                    s.bleed_dps = s.bleed_dps.max(damage * 0.1);
                }
                self.heal += damage * 0.12;
                let to = (arena.view.eye - Vector3::new(0.0, 0.0, 0.5 * vpm) - at) * 1.6;
                self.fine(at, to, Element::look(Some(Element::Blood)), 0.5, 0.0);
            }
            Some(Element::Holy) => {
                if alive {
                    let s = &mut arena.animals[i].status;
                    s.staggered = s.staggered.max(if boss { 0.2 } else { 0.4 });
                }
                self.heal += damage * 0.05;
            }
            None if alive => {
                let s = &mut arena.animals[i].status;
                s.staggered = s.staggered.max(if boss { 0.2 } else { 0.5 });
                if !boss {
                    s.push = away * 4.0 * vpm;
                }
            }
            _ => {}
        }
        if boss && alive {
            arena.animals[i].status.limit_for_boss();
        }
    }

    /// A burst of the power's element `radius` meters round `at` for `damage`, leaving no mark, its
    /// element doing what it does to all it catches but `skip`
    pub(super) fn power_burst(&mut self, arena: &mut Arena, profile: &Rc<Profile>, at: Vector3<f64>, radius: f64, damage: f64, skip: Option<usize>) {
        let vpm = arena.view.vpm;
        let caught: Vec<usize> = Combat::foes_round(arena, at, radius).into_iter().filter(|&i| Some(i) != skip).collect();
        self.scarless = true;
        self.burst(arena, profile, at, radius, damage, 0, false, profile.element, skip);
        self.scarless = false;
        for i in caught {
            let center = arena.animals[i].center(vpm as u32);
            self.empower(arena, profile, i, center, damage * 0.6, at);
        }
    }

    /// A little pop of the power's element `radius` meters round `at` for `damage`, for the many
    /// small shots a power looses at once, embers, motes and shards: a puff and a spatter of sparks,
    /// what's in it struck and its element doing what it does, but none of an explosion's lightning
    /// out of the sky, ice, pools and the rest, which a handful going off together would cost too
    /// much to draw
    pub(super) fn power_pop(&mut self, arena: &mut Arena, profile: &Rc<Profile>, at: Vector3<f64>, radius: f64, damage: f64, skip: Option<usize>) {
        let vpm = arena.view.vpm;
        let look = Element::look(profile.element);
        self.blasts.push(Blast { position: at, look, radius: radius * 0.6, life: 0.25, lifetime: 0.25 });
        self.sparks(at, 6, 3.0, look, 0.4, 0.3);
        self.cues.push(Cue::at(Sound::Burst, at, 0.35).pitched(1.5));
        for i in Combat::foes_round(arena, at, radius).into_iter().filter(|&i| Some(i) != skip) {
            let center = arena.animals[i].center(vpm as u32);
            self.strike(arena, profile, i, Blow { damage, crit: false, heavy: false, from: at, depth: 1, element: profile.element });
            self.empower(arena, profile, i, center, damage * 0.6, at);
        }
    }

    /// One of their shots in flight: what's in a crescent's or a wheel's swath cut as it passes, a
    /// wheel turning back once it's as far as it goes and caught again at the hand, tracers, and
    /// coming down where it's flown as far as it goes. True if it's gone.
    pub(super) fn power_flight(&mut self, arena: &mut Arena, p: &mut Projectile, shape: Shape, dt: f64) -> bool {
        let vpm = arena.view.vpm;
        let look = Element::look(p.profile.element);
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        // Glowing tracers behind what's shot, never reaching back past the muzzle
        let tracer = match shape {
            Shape::Bolt | Shape::Slug(_) => Some(0.04),
            Shape::Lance | Shape::Flare => Some(0.06),
            Shape::Arrow | Shape::Cloud => Some(0.035),
            Shape::Stake => Some(0.05),
            Shape::Fist | Shape::Shard => Some(0.025),
            Shape::Spear { .. } | Shape::Vigil | Shape::Ascent => Some(0.07),
            Shape::Ray | Shape::Step => Some(0.04),
            _ => None,
        };
        if let Some(seconds) = tracer
            && p.flown > 0.4 * vpm
        {
            let back = (p.velocity.norm() * seconds).min(p.flown);
            self.rays.push((p.position - way * back, p.position, look));
        }
        // A whip's wave running on over the ground, up slopes and down off ledges
        if let Shape::Wave { low: true, .. } = shape {
            let ground = ground_under(arena.world, p.position - Vector3::new(0.0, 0.0, LASH_HIGH * vpm), vpm);
            p.position.z = ground.z + LASH_HIGH * vpm;
            p.velocity.z = 0.0;
        }
        // A gale or a shrapnel pot's shard running on over the ground the same way
        let skim = match shape {
            Shape::Gale => Some(0.7),
            Shape::Shard => Some(0.35),
            _ => None,
        };
        if let Some(high) = skim {
            let ground = ground_under(arena.world, p.position - Vector3::new(0.0, 0.0, high * vpm), vpm);
            p.position.z = ground.z + high * vpm;
            p.velocity.z = 0.0;
        }
        if shape == Shape::Gale {
            self.gale_sweeps(arena, p, way, dt);
        }
        // A talisman's spear crackling, a shade trailing darkness, see faith.rs
        if let Shape::Spear { big } = shape {
            self.spear_crackles(p, big, dt);
        }
        // A Vigil Spear crackling as a spear does, Helix's orbs wheeling round each other, a knot of
        // shadow trailing darkness, see miracles.rs
        if matches!(shape, Shape::Vigil | Shape::Ascent) {
            self.spear_crackles(p, false, dt);
        }
        if let Shape::Helix { phase } = shape {
            self.helix_flies(p, phase, dt);
        }
        if shape == Shape::Grasp {
            self.shade_trails(p, dt);
        }
        if shape == Shape::Shade {
            self.shade_trails(p, dt);
        }
        // A chakram out and back, a shade of shadow streaming off a Shadowstep, see thrown.rs and
        // arsenal.rs
        if let Shape::Chakram { back, glances } = shape {
            return self.chakram_flies(arena, p, back, glances, dt);
        }
        if shape == Shape::Step && random(&mut self.rng) < dt * 40.0 {
            let at = p.position + self.any_direction() * 0.25 * vpm;
            self.mote(at, Some(Element::Void), 1.3);
        }
        match shape {
            // Flung out as far as it goes, and turning back empty
            Shape::Hook { back: false, .. } if p.flown >= p.range => {
                p.trick = Trick::Signature(Sig::Power(Shape::Hook { back: true, caught: None }));
                p.hit.clear();
                self.cues.push(Cue::at(Sound::Clang, p.position, 0.4).pitched(1.5));
                return false;
            }
            // Reeled back in to the hand, what it caught dragged along behind it, and struck again
            // at the feet
            Shape::Hook { back: true, caught } => {
                let hand = arena.view.eye + arena.view.rotation * Vector3::new(0.25, 0.5, -0.3) * vpm;
                let to = hand - p.position;
                let held = caught.and_then(|id| arena.animals.iter().position(|a| a.id == id && a.health > 0.0));
                if to.norm() < 1.6 * vpm {
                    if let Some(i) = held {
                        let center = arena.animals[i].center(vpm as u32);
                        let profile = p.profile.clone();
                        arena.animals[i].status.push = Vector3::zeros();
                        self.power_hit(arena, &profile, i, center, p.damage * 0.6, hand);
                    }
                    self.sparks(p.position, 10, 2.5, look, 0.0, 0.3);
                    self.cues.push(Cue::here(Sound::Clang, 0.6).pitched(1.1));
                    return true;
                }
                p.velocity = to.normalize() * HOOK_SPEED * 0.75 * vpm;
                if let Some(i) = held
                    && arena.animals[i].boss.is_none()
                {
                    let pull = p.position - arena.animals[i].center(vpm as u32);
                    let s = &mut arena.animals[i].status;
                    s.push = Vector3::new(pull.x, pull.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros) * HOOK_SPEED * 0.75 * vpm;
                    s.staggered = s.staggered.max(0.25);
                }
            }
            // Bursting where it tops out, over the ring it was aimed at
            Shape::Cloud | Shape::Flare if p.velocity.z <= 0.0 => {
                let at = p.position;
                self.power_lands(arena, p, shape, at);
                return true;
            }
            Shape::Wheel { back: true } => {
                // Coming back to the hand, caught there
                let hand = arena.view.eye - Vector3::new(0.0, 0.0, 0.4 * vpm);
                let to = hand - p.position;
                if to.norm() < 1.4 * vpm {
                    self.sparks(p.position, 10, 2.5, look, 0.0, 0.3);
                    self.cues.push(Cue::here(Sound::Clang, 0.5).pitched(1.4));
                    return true;
                }
                p.velocity = to.normalize() * WHEEL_SPEED * 1.25 * vpm;
            }
            Shape::Wheel { back: false } if p.flown >= p.range => {
                self.landed(p.position);
                p.trick = Trick::Signature(Sig::Power(Shape::Wheel { back: true }));
                p.hit.clear();
                return false;
            }
            _ if p.flown >= p.range => {
                let at = p.position;
                self.power_lands(arena, p, shape, at);
                return true;
            }
            _ => {}
        }
        // What's in its swath, cut once each way
        let half = match shape {
            Shape::Wave { width, .. } => Some(width as f64 * 0.5),
            Shape::Wheel { .. } => Some(1.0),
            _ => None,
        };
        if let Some(half) = half {
            let step = p.velocity.norm() * dt;
            let caught: Vec<usize> = (0..arena.animals.len())
                .filter(|&i| {
                    let animal = &arena.animals[i];
                    if animal.health <= 0.0 || animal.look.is_pet() || p.hit.contains(&animal.id) {
                        return false;
                    }
                    let body = animal.look.body().1 * vpm;
                    let rel = animal.center(vpm as u32) - p.position;
                    let along = rel.dot(&way);
                    along > -body && along < step + body && (rel - way * along).norm() < half * vpm + body
                })
                .collect();
            let profile = p.profile.clone();
            for i in caught {
                let center = arena.animals[i].center(vpm as u32);
                self.landed(center);
                p.hit.push(arena.animals[i].id);
                self.power_hit(arena, &profile, i, center, p.damage, p.position - way * vpm);
                // A greatsword's rending wave throws what it cuts back
                if half > 1.5 && arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
                    arena.animals[i].status.push = Vector3::new(way.x, way.y, 0.0) * 8.0 * vpm;
                }
            }
            // Shedding sparks as it goes, so it's seen even with no element
            if random(&mut self.rng) < dt * 30.0 {
                let across = way.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                let at = p.position + across * (random(&mut self.rng) - 0.5) * 2.0 * half * vpm;
                let v = (-way * 2.0 + self.any_direction() * 0.5) * vpm;
                self.sparks.push(Spark { position: at, velocity: v, look, life: 0.3, lifetime: 0.3, gravity: 0.2, slot: SLOT_SPARK });
                self.cap_sparks();
            }
        }
        false
    }

    /// A gale running on, `way`: what it meets cut once and swept up, and what it's swept up carried
    /// along with it, dust and its element whirling up off it
    fn gale_sweeps(&mut self, arena: &mut Arena, p: &mut Projectile, way: Vector3<f64>, dt: f64) {
        let vpm = arena.view.vpm;
        let speed = p.velocity.norm();
        let profile = p.profile.clone();
        let mut met = Vec::new();
        for (i, a) in arena.animals.iter_mut().enumerate() {
            if a.health <= 0.0 || a.look.is_pet() {
                continue;
            }
            let center = a.center(vpm as u32);
            // Only what's down on the ground it runs along, never what flies over it
            if (center.z - p.position.z).abs() > (2.0 + a.look.body().1) * vpm {
                continue;
            }
            let off = (center - p.position).xy().norm() / vpm - a.look.body().1;
            if p.hit.contains(&a.id) {
                if off < 3.0 && a.boss.is_none() {
                    a.status.push = way * speed;
                    a.status.staggered = a.status.staggered.max(0.2);
                }
            } else if off < 1.6 {
                met.push(i);
            }
        }
        for i in met {
            let center = arena.animals[i].center(vpm as u32);
            p.hit.push(arena.animals[i].id);
            self.landed(center);
            self.power_hit(arena, &profile, i, center, p.damage, p.position - way * vpm);
        }
        let look = Element::look(profile.element);
        if random(&mut self.rng) < dt * 45.0 {
            let a = random(&mut self.rng) * TAU;
            let out = Vector3::new(a.cos(), a.sin(), 0.0);
            let at = p.position + out * 0.8 * vpm - Vector3::new(0.0, 0.0, 0.5 * vpm);
            let v = (Vector3::new(-out.y, out.x, 0.0) * 5.0 + Vector3::new(0.0, 0.0, 4.0)) * vpm + way * speed * 0.5;
            self.sparks.push(Spark { position: at, velocity: v, look, life: 0.45, lifetime: 0.45, gravity: 0.0, slot: SLOT_SPARK });
            self.cap_sparks();
        }
        if random(&mut self.rng) < dt * 12.0 {
            let v = (self.any_direction() * 1.5 + Vector3::new(0.0, 0.0, 2.5)) * vpm;
            self.drop(p.position - Vector3::new(0.0, 0.0, 0.6 * vpm), v, DROP_MUD, 0.7);
        }
    }

    /// What a bolas wound round, or where it came down: `on` held fast, and what's round `at`,
    /// for a while
    fn bolas_bind(&mut self, arena: &mut Arena, p: &Projectile, at: Vector3<f64>, on: Option<u32>) {
        let vpm = arena.view.vpm;
        let center = on.and_then(|id| arena.animals.iter().find(|a| a.id == id)).map_or(at, |a| a.center(vpm as u32));
        self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Bound)), &p.profile, center, Vector3::zeros(), BOUND_ROUND, BOUND_SECONDS, p.damage * 0.3);
        if let Some(z) = self.zones.last_mut() {
            z.target = on;
        }
        for i in Combat::foes_round(arena, center, BOUND_ROUND) {
            let s = &mut arena.animals[i].status;
            s.rooted = s.rooted.max(BOUND_SECONDS);
            if arena.animals[i].boss.is_some() {
                arena.animals[i].status.limit_for_boss();
            }
        }
        self.cues.push(Cue::at(Sound::Whirr, center, 0.6).pitched(1.8));
        self.cues.push(Cue::at(Sound::Clang, center, 0.4).pitched(1.3));
        self.sparks(center, 10, 2.5, Element::look(p.profile.element), 0.3, 0.35);
    }

    /// One of their shots striking animal `i` at `at`. True if it's gone, false if it goes on.
    pub(super) fn power_strikes(&mut self, arena: &mut Arena, p: &mut Projectile, shape: Shape, i: usize, at: Vector3<f64>) -> bool {
        self.landed(at);
        let vpm = arena.view.vpm;
        let profile = p.profile.clone();
        let id = arena.animals[i].id;
        let from = p.position - p.velocity * 0.02;
        match shape {
            // Its swath does it, as it flies, see power_flight()
            Shape::Wave { .. } | Shape::Wheel { .. } => false,
            // Through all
            Shape::Javelin | Shape::Arrow | Shape::Lance => {
                p.hit.push(id);
                self.power_hit(arena, &profile, i, at, p.damage, from);
                false
            }
            // Stuck fast in it, to burst a moment later
            Shape::Bolt => {
                self.power_hit(arena, &profile, i, at, p.damage, from);
                let center = arena.animals[i].center(vpm as u32);
                self.stick_bolt(p, at, Some((id, at - center)));
                true
            }
            // Glancing on to the next near, or bursting if there's none
            Shape::Slug(left) => {
                self.power_hit(arena, &profile, i, at, p.damage, from);
                let mut hit = p.hit.clone();
                hit.push(id);
                let next = Combat::foes_round(arena, at, 12.0).into_iter().find(|&j| !hit.contains(&arena.animals[j].id));
                match next.filter(|_| left > 0) {
                    Some(j) => {
                        let to = arena.animals[j].center(vpm as u32);
                        let way = (to - at).try_normalize(1e-6).unwrap_or_else(Vector3::z);
                        self.launch(&profile, Some(Missile { shot: Shot::Ball, head: 0, power: true, size: 1.4 }), Form::Bolt, at + way * 0.3 * vpm, way * 45.0 * vpm, p.damage * 1.1, 0.0);
                        self.signed(Sig::Power(Shape::Slug(left - 1)));
                        if let Some(n) = self.projectiles.last_mut() {
                            n.hit = hit;
                            n.size = 1.4;
                        }
                        self.cues.push(Cue::at(Sound::Clang, at, 0.6).pitched(1.6));
                        self.sparks(at, 10, 4.0, Element::look(None), 0.5, 0.3);
                    }
                    None => self.power_burst(arena, &profile, at, 1.4, p.damage * 0.5, Some(i)),
                }
                true
            }
            Shape::Ember | Shape::Mote => {
                self.power_hit(arena, &profile, i, at, p.damage, from);
                self.power_pop(arena, &profile, at, 1.1, p.damage * 0.4, Some(i));
                true
            }
            Shape::Shell | Shape::Charge | Shape::Orb => {
                self.power_lands(arena, p, shape, at);
                true
            }
            // Its swath does it, as it runs on, see gale_sweeps()
            Shape::Gale => false,
            // The first thing in its way struck, and a little pop round it
            Shape::Fist | Shape::Arrowlet | Shape::Shard => {
                self.power_hit(arena, &profile, i, at, p.damage, from);
                self.power_pop(arena, &profile, at, 0.9, p.damage * 0.35, Some(i));
                true
            }
            // Caught on the hook, to be dragged back in with it
            Shape::Hook { back: false, .. } => {
                self.power_hit(arena, &profile, i, at, p.damage, from);
                let caught = (arena.animals[i].boss.is_none() && arena.animals[i].health > 0.0).then_some(id);
                p.hit.push(id);
                p.trick = Trick::Signature(Sig::Power(Shape::Hook { back: true, caught }));
                self.cues.push(Cue::at(Sound::Clang, at, 0.7).pitched(0.9));
                false
            }
            Shape::Hook { back: true, .. } => false,
            // Wound round it
            Shape::Bolas => {
                self.power_hit(arena, &profile, i, at, p.damage, from);
                self.bolas_bind(arena, p, at, Some(id));
                true
            }
            Shape::Anvil | Shape::Stake | Shape::Cloud | Shape::Flare | Shape::Beacon | Shape::Pot | Shape::Shade => {
                self.power_lands(arena, p, shape, at);
                true
            }
            // Through all, lightning arcing off each, see faith.rs
            Shape::Spear { big } => {
                self.spear_strikes(arena, p, big, i, at);
                false
            }
            // A knife sunk in what it struck; a chakram's cut does it as it flies, see thrown.rs
            Shape::Knife => {
                self.knife_strikes(arena, p, i, at);
                true
            }
            Shape::Chakram { .. } => false,
            // Struck in the back out of the dark, see arsenal.rs
            Shape::Step => {
                self.step_strikes(arena, p, i, at);
                true
            }
            Shape::Ray => {
                p.hit.push(id);
                self.power_hit(arena, &profile, i, at, p.damage, from);
                false
            }
            Shape::Glyph | Shape::Sigil => {
                self.glyph_lands(arena, p, at);
                true
            }
            // Stuck fast in what it strikes, see miracles.rs
            Shape::Vigil => {
                self.vigil_strikes(arena, p, i, at);
                true
            }
            Shape::Ascent => false,
            Shape::Storm | Shape::Grasp => {
                self.power_lands(arena, p, shape, at);
                true
            }
            Shape::Helix { .. } => {
                self.power_hit(arena, &profile, i, at, p.damage, from);
                self.power_pop(arena, &profile, at, 1.2, p.damage * 0.4, Some(i));
                true
            }
        }
    }

    /// One of their shots coming down at `at`, on the ground, a wall or the water, or as far as
    /// it goes
    pub(super) fn power_lands(&mut self, arena: &mut Arena, p: &Projectile, shape: Shape, at: Vector3<f64>) {
        if !matches!(shape, Shape::Wheel { back: true } | Shape::Ascent) {
            self.landed(at);
        }
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let profile = p.profile.clone();
        let look = Element::look(profile.element);
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        match shape {
            // A bolt from the sky where a spear ends, and the dead raised where a shade comes down,
            // see faith.rs
            Shape::Spear { big } => self.spear_lands(arena, p, big, at),
            Shape::Shade => self.raise_dead(arena, p, at),
            // A knife stuck in the ground; a chakram glancing off and coming back, see thrown.rs
            Shape::Knife => self.knife_lands(p, at),
            Shape::Chakram { back: false, .. } => {
                self.sparks(at, 8, 3.0, Element::look(None), 0.6, 0.3);
                self.cues.push(Cue::at(Sound::Clang, at, 0.6).pitched(1.3));
                let damage = p.damage;
                self.launch(&profile, None, Form::Bolt, at - way * 0.3 * vpm, -way * p.velocity.norm(), damage, 0.0);
                self.signed(Sig::Power(Shape::Chakram { back: true, glances: 0 }));
            }
            Shape::Chakram { back: true, .. } => self.sparks(at, 8, 2.5, Element::look(None), 0.3, 0.3),
            Shape::Step => {
                self.step_out(at);
                self.power_pop(arena, &profile, at, 1.2, p.damage * 0.3, None);
            }
            Shape::Ray => self.power_pop(arena, &profile, at, 1.4, p.damage * 0.5, None),
            Shape::Glyph | Shape::Sigil => self.glyph_lands(arena, p, at),
            // A Vigil Spear stuck in the ground; Heavenfall's spear gone up into the sky; a
            // thunderhead gathering where Stormcall's orb tops out; tendrils reaching up where a
            // knot of shadow comes down, see miracles.rs
            Shape::Vigil => self.stick_vigil(p, at, None),
            Shape::Ascent => {
                self.sparks(at, 14, 4.0, Element::look(Some(Element::Holy)), 0.3, 0.4);
                self.flashes.push(Flash { position: at, color: Element::Holy.light(), reach: 10.0, life: 0.3, lifetime: 0.3 });
                self.cues.push(Cue::at(Sound::Thunder, at, 0.4).pitched(1.3));
            }
            Shape::Storm => {
                let ground = Combat::floor_at(arena, at);
                self.thunderhead(at, 3.5, 10);
                self.flashes.push(Flash { position: at, color: Element::Storm.light(), reach: 12.0, life: 0.3, lifetime: 0.3 });
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Storm)), &profile, ground, Vector3::zeros(), super::miracles::STORM_ROUND, 0.4 + 6.0 * 0.24 + 0.3, p.damage);
                self.cues.push(Cue::at(Sound::Rumble, at, 0.9).pitched(0.8));
            }
            Shape::Grasp => {
                let ground = Combat::floor_at(arena, at);
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Grasp)), &profile, ground, Vector3::zeros(), super::miracles::GRASP_ROUND, super::miracles::GRASP_SECONDS, p.damage);
                if (ground - arena.view.eye).norm() > 3.0 * vpm {
                    self.swells.push(Swell { model: crate::effects::TENDRILS, position: ground, radius: 2.4, life: 3.0, lifetime: 3.0 });
                }
                for i in Combat::foes_round(arena, ground, super::miracles::GRASP_ROUND).into_iter().take(4) {
                    let at = arena.animals[i].position;
                    self.swells.push(Swell { model: crate::effects::TENDRILS, position: at, radius: 1.3, life: 2.8, lifetime: 2.8 });
                }
                self.sparks(ground + up * 0.3 * vpm, 14, 2.5, look, -0.2, 0.6);
                self.cues.push(Cue::at(Sound::Wail, ground, 0.5).pitched(0.6));
                self.cues.push(Cue::at(Sound::Quake, ground, 0.4).pitched(1.6));
            }
            Shape::Helix { .. } => self.power_pop(arena, &profile, at, 1.2, p.damage * 0.5, None),
            // Breaking up into sparks and a flash where it ends
            Shape::Wave { .. } => {
                self.sparks(at, 16, 3.5, look, 0.3, 0.45);
                self.blasts.push(Blast { position: at, look, radius: 0.6, life: 0.3, lifetime: 0.3 });
                self.cues.push(Cue::at(Sound::Shing, at, 0.5).pitched(1.5));
            }
            // Glancing off what it struck and coming back
            Shape::Wheel { back: false } => {
                self.sparks(at, 10, 3.0, Element::look(None), 0.6, 0.3);
                self.cues.push(Cue::at(Sound::Clang, at, 0.6).pitched(1.1));
                self.launch(&profile, None, Form::Bolt, at - way * 0.3 * vpm, -way * WHEEL_SPEED * vpm, p.damage, 0.0);
                self.signed(Sig::Power(Shape::Wheel { back: true }));
            }
            Shape::Wheel { back: true } => self.sparks(at, 10, 2.5, look, 0.0, 0.3),
            Shape::Javelin => {
                self.power_burst(arena, &profile, at, 1.8, p.damage * 0.5, None);
                let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &way).unwrap_or_else(UnitQuaternion::identity);
                self.stuck.push(Stuck { position: at + way * 0.1 * vpm, rotation, look, head: Some(0), quarrel: true, element: profile.element, life: 2.0, glowing: 2.0 });
                self.keep_stuck_few();
            }
            Shape::Arrow => self.power_burst(arena, &profile, at, 2.2, p.damage * 0.45, None),
            Shape::Bolt => self.stick_bolt(p, at, None),
            // Bouncing on off the ground or a wall, or bursting at the last
            Shape::Slug(left) => {
                if left > 0 {
                    let mut v = p.velocity;
                    if v.z < 0.0 {
                        v.z = -v.z * 0.8;
                    } else {
                        v.x = -v.x;
                        v.y = -v.y;
                    }
                    self.launch(&profile, Some(Missile { shot: Shot::Ball, head: 0, power: true, size: 1.4 }), Form::Bolt, at + up * 0.15 * vpm - way * 0.1 * vpm, v * 0.85, p.damage, 0.1);
                    self.signed(Sig::Power(Shape::Slug(left - 1)));
                    if let Some(n) = self.projectiles.last_mut() {
                        n.size = 1.4;
                    }
                    self.sparks(at, 10, 4.0, Element::look(None), 0.5, 0.3);
                    self.cues.push(Cue::at(Sound::Clang, at, 0.5).pitched(1.7));
                } else {
                    self.power_burst(arena, &profile, at, 1.4, p.damage * 0.5, None);
                }
            }
            // The line it left shortened to where it stopped, and a burst there
            Shape::Lance => {
                if let Some(z) = self.zones.iter_mut().rev().find(|z| z.kind == ZoneKind::Signature(SigZone::Power(Lasting::Line)) && z.tick > 0.0) {
                    let along = z.velocity.try_normalize(1e-6).unwrap_or(way);
                    z.velocity = along * (at - z.position).dot(&along).max(0.5 * vpm);
                }
                self.power_burst(arena, &profile, at, 1.5, p.damage * 0.4, None);
            }
            Shape::Ember | Shape::Mote => self.power_pop(arena, &profile, at, 1.1, p.damage * 0.5, None),
            Shape::Shell => {
                self.power_burst(arena, &profile, at, 3.5, p.damage, None);
                for _ in 0..10 {
                    let v = (self.any_direction() + up * 1.4) * 4.0 * vpm;
                    self.drop(at, v, DROP_MUD, 1.0);
                }
                self.shake = self.shake.max(0.6);
            }
            Shape::Orb => {
                self.power_burst(arena, &profile, at, 3.5, p.damage, None);
                self.shake = self.shake.max(0.5);
            }
            // Dragging everything in where it lands
            Shape::Charge => {
                let ground = Combat::floor_at(arena, at) + up * 0.3 * vpm;
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Vortex)), &profile, ground, Vector3::zeros(), VORTEX_REACH, VORTEX_SECONDS, p.damage);
                self.cues.push(Cue::at(Sound::Hex, ground, 0.9).pitched(0.5));
                self.cues.push(Cue::at(Sound::Wail, ground, 0.3).pitched(1.2));
                self.flashes.push(Flash { position: ground + up * vpm, color: profile.element.map_or([0.9, 0.85, 0.8], Element::light), reach: 8.0, life: 0.3, lifetime: 0.3 });
            }
            // Blowing itself out at the end of its run, flinging off all it carried
            Shape::Gale => {
                for &id in &p.hit {
                    if let Some(a) = arena.animals.iter_mut().find(|a| a.id == id && a.health > 0.0 && a.boss.is_none()) {
                        let away = (a.position - at).xy().try_normalize(1e-6).unwrap_or_else(|| way.xy());
                        a.status.push = Vector3::new(away.x * 7.0, away.y * 7.0, 6.0) * vpm;
                        a.status.staggered = a.status.staggered.max(0.6);
                    }
                }
                self.power_burst(arena, &profile, at, 2.2, p.damage * 0.6, None);
                self.sparks(at, 16, 4.5, look, 0.2, 0.45);
                self.cues.push(Cue::at(Sound::Whirr, at, 0.7).pitched(0.5));
            }
            Shape::Fist | Shape::Arrowlet | Shape::Shard => self.power_pop(arena, &profile, at, 0.9, p.damage * 0.4, None),
            // Glancing off what it struck and coming back empty
            Shape::Hook { back: false, .. } => {
                self.sparks(at, 10, 3.0, Element::look(None), 0.6, 0.3);
                self.cues.push(Cue::at(Sound::Clang, at, 0.6).pitched(1.2));
                self.launch(&profile, None, Form::Bolt, at - way * 0.3 * vpm, -way * HOOK_SPEED * vpm, p.damage, 0.0);
                self.signed(Sig::Power(Shape::Hook { back: true, caught: None }));
            }
            Shape::Hook { back: true, .. } => self.sparks(at, 8, 2.5, look, 0.3, 0.3),
            // Crashing down: the ground shaking, everything round thrown up, dust and stone flying
            Shape::Anvil => {
                let ground = Combat::floor_at(arena, at);
                for i in Combat::foes_round(arena, ground, 3.5) {
                    if arena.animals[i].boss.is_none() {
                        let s = &mut arena.animals[i].status;
                        s.push = up * 7.0 * vpm;
                        s.staggered = s.staggered.max(0.8);
                    }
                }
                self.power_burst(arena, &profile, ground + up * 0.4 * vpm, 3.5, p.damage, None);
                if (ground - arena.view.eye).norm() > 4.5 * vpm {
                    self.swells.push(Swell { model: STONE_PILLAR, position: ground, radius: 1.1, life: 0.6, lifetime: 0.6 });
                }
                for _ in 0..14 {
                    let v = (self.any_direction() + up * 1.3) * 5.0 * vpm;
                    self.drop(ground + up * 0.3 * vpm, v, DROP_MUD, 1.1);
                }
                self.cues.push(Cue::at(Sound::Quake, ground, 1.0).pitched(0.7));
                self.cues.push(Cue::at(Sound::Slam, ground, 1.0).pitched(0.6));
                self.shake = self.shake.max(0.9);
            }
            // Staked in the ground, to pulse a while
            Shape::Stake => {
                let ground = Combat::floor_at(arena, at);
                self.power_burst(arena, &profile, ground + up * 0.4 * vpm, 1.6, p.damage * 0.6, None);
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Stake)), &profile, ground, way, STAKE_ROUND, STAKE_PULSES * STAKE_PULSE + 0.35, p.damage * 0.7);
                self.cues.push(Cue::at(Sound::ArrowHit, ground, 0.9).pitched(0.6));
                self.cues.push(Cue::at(Sound::Quake, ground, 0.5).pitched(1.4));
                self.shake = self.shake.max(0.3);
            }
            // Bursting into a rain of arrowlets
            Shape::Cloud => {
                self.blasts.push(Blast { position: at, look, radius: 1.4, life: 0.35, lifetime: 0.35 });
                self.sparks(at, 18, 5.0, look, 0.5, 0.5);
                self.cues.push(Cue::at(Sound::Burst, at, 0.8).pitched(1.3));
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Downpour)), &profile, at, Vector3::zeros(), 4.0, DOWNPOUR_SECONDS + 0.1, p.damage * 0.55);
            }
            // Bursting into a flare that hangs there
            Shape::Flare => {
                self.blasts.push(Blast { position: at, look, radius: 1.8, life: 0.4, lifetime: 0.4 });
                self.sparks(at, 24, 6.0, look, 0.6, 0.6);
                self.cues.push(Cue::at(Sound::Burst, at, 1.0).pitched(0.9));
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Flare)), &profile, at, Vector3::zeros(), 6.0, FLARE_SECONDS, p.damage * 0.4);
            }
            Shape::Bolas => self.bolas_bind(arena, p, at, None),
            // Set down, to toll
            Shape::Beacon => {
                let ground = Combat::floor_at(arena, at);
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Beacon)), &profile, ground, Vector3::zeros(), BEACON_REACH, BEACON_TOLLS * BEACON_TOLL + 0.45, p.damage);
                self.flashes.push(Flash { position: ground + up * vpm, color: profile.element.map_or([0.95, 0.9, 0.8], Element::light), reach: 8.0, life: 0.3, lifetime: 0.3 });
            }
            // Bursting, its shards wheeling out along the ground all round
            Shape::Pot => {
                let ground = Combat::floor_at(arena, at) + up * 0.35 * vpm;
                self.power_pop(arena, &profile, ground, 1.6, p.damage * 0.6, None);
                let turn = random(&mut self.rng) * TAU;
                for k in 0..SHARDS {
                    let a = turn + k as f64 / SHARDS as f64 * TAU;
                    let out = Vector3::new(a.cos(), a.sin(), 0.0);
                    // Turned a little off straight out, so the wheel of them spins
                    let along = (out + Vector3::new(-out.y, out.x, 0.0) * 0.35).normalize();
                    self.launch(&profile, Some(Missile { shot: Shot::Ball, head: 0, power: false, size: 0.8 }), Form::Bolt, ground + out * 0.3 * vpm, along * 14.0 * vpm, p.damage * 0.45, 0.0);
                    self.signed(Sig::Power(Shape::Shard));
                    if let Some(s) = self.projectiles.last_mut() {
                        s.range = SHRAPNEL_REACH * vpm;
                        s.size = 0.8;
                    }
                }
                self.cues.push(Cue::at(Sound::Burst, ground, 1.0).pitched(1.0));
                self.shake = self.shake.max(0.4);
            }
        }
    }

    /// A burst bolt stuck fast at `at`, in what it struck, `on` its id and where on it, or in the
    /// ground
    fn stick_bolt(&mut self, p: &Projectile, at: Vector3<f64>, on: Option<(u32, Vector3<f64>)>) {
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let kind = ZoneKind::Signature(SigZone::Power(Lasting::Stuck { way: [way.x as f32, way.y as f32, way.z as f32] }));
        self.zone(kind, &p.profile, at, on.map_or(Vector3::zeros(), |o| o.1), 2.4, STUCK_SECONDS, p.damage);
        if let Some(z) = self.zones.last_mut() {
            z.target = on.map(|o| o.0);
        }
        self.cues.push(Cue::at(Sound::ArrowHit, at, 0.7));
        self.cues.push(Cue::at(Sound::Fuse, at, 0.5).pitched(1.6));
    }
}

impl Combat {
    /// One of their zones going on
    pub(super) fn power_zone(&mut self, arena: &mut Arena, z: &mut Zone, kind: Lasting, dt: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let profile = z.profile.clone();
        let element = profile.element;
        let look = Element::look(element);
        let age = z.lifetime - z.life;
        match kind {
            // A step of it erupting each FISSURE_GAP, throwing up what stands on it
            Lasting::Fissure => {
                while z.owed < FISSURE_STEPS && age >= z.owed * FISSURE_GAP {
                    let at = ground_under(arena.world, z.position + z.velocity * z.owed + up * 1.5 * vpm, vpm);
                    self.landed(at);
                    z.owed += 1.0;
                    let model = match element {
                        Some(Element::Frost) => ICE_BURST,
                        Some(Element::Holy) => PILLAR,
                        Some(Element::Poison) => ROOTS,
                        _ => STONE_PILLAR,
                    };
                    // Not right in front of the eye, where it'd fill the view
                    if (at - arena.view.eye).norm() > 4.5 * vpm {
                        self.swells.push(Swell { model, position: at, radius: 0.85, life: 0.7, lifetime: 0.7 });
                    }
                    self.blasts.push(Blast { position: at + up * 0.3 * vpm, look, radius: 0.8, life: 0.35, lifetime: 0.35 });
                    for _ in 0..4 {
                        let v = (self.any_direction() * 0.6 + up * 1.4) * 4.0 * vpm;
                        self.drop(at, v, DROP_MUD, 0.9);
                    }
                    self.sparks(at + up * 0.3 * vpm, 6, 3.0, look, 0.4, 0.4);
                    self.cues.push(Cue::at(Sound::Quake, at, 0.5).pitched(1.3 + 0.05 * z.owed as f32));
                    for i in Combat::foes_round(arena, at, z.radius) {
                        let id = arena.animals[i].id;
                        if z.hit.contains(&id) {
                            continue;
                        }
                        z.hit.push(id);
                        let center = arena.animals[i].center(vpm as u32);
                        self.power_hit(arena, &profile, i, center, z.damage, at);
                        if arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
                            let s = &mut arena.animals[i].status;
                            s.push = (up * 6.0 + flat_of(z.velocity) * 2.0) * vpm;
                            s.staggered = s.staggered.max(0.6);
                        }
                    }
                }
                if z.owed >= FISSURE_STEPS {
                    z.life = z.life.min(0.3);
                }
            }
            // Stuck fast, blinking faster, riding along on what it's in
            Lasting::Stuck { .. } => {
                if let Some(id) = z.target {
                    match arena.animals.iter().position(|a| a.id == id && a.health > 0.0) {
                        Some(i) => z.position = arena.animals[i].center(vpm as u32) + z.velocity,
                        None => z.target = None,
                    }
                }
                if random(&mut self.rng) < dt * (10.0 + 30.0 * age / STUCK_SECONDS) {
                    let v = self.any_direction() * 1.5 * vpm;
                    self.fine(z.position, v, look, 0.25, 0.0);
                }
            }
            // Dragging everything near in, sparks spiralling down into it
            Lasting::Vortex => {
                for i in Combat::foes_round(arena, z.position, z.radius) {
                    let to = z.position - arena.animals[i].position;
                    if arena.animals[i].boss.is_none() && to.xy().norm() > 0.6 * vpm {
                        arena.animals[i].status.push = Vector3::new(to.x, to.y, 0.0).normalize() * 4.5 * vpm;
                    }
                }
                z.owed += dt * 40.0;
                while z.owed >= 1.0 {
                    z.owed -= 1.0;
                    let a = random(&mut self.rng) * TAU;
                    let out = Vector3::new(a.cos(), a.sin(), 0.0);
                    let at = z.position + out * (2.0 + 3.5 * random(&mut self.rng)) * vpm + up * 0.3 * vpm;
                    let v = (-out * 3.0 + Vector3::new(-out.y, out.x, 0.0) * 4.0) * vpm;
                    self.sparks.push(Spark { position: at, velocity: v, look, life: 0.5, lifetime: 0.5, gravity: 0.0, slot: SLOT_SPARK });
                }
                self.cap_sparks();
            }
            // A knife stuck in the ground, see thrown.rs; a circle of runes being written, see
            // arsenal.rs
            Lasting::Knife { .. } => {}
            Lasting::Vigil | Lasting::Heavenfall | Lasting::Storm | Lasting::Glacier | Lasting::Grasp => {
                self.miracle_zone(arena, z, kind, dt);
            }
            Lasting::Runes | Lasting::Sigil => self.runes_go_on(arena, z, kind == Lasting::Sigil),
            // A Starbreak's crack, a short fissure, see arsenal.rs
            Lasting::Crack => {
                while z.owed < super::arsenal::STAR_STEPS && age >= z.owed * FISSURE_GAP {
                    let at = ground_under(arena.world, z.position + z.velocity * z.owed + up * 1.5 * vpm, vpm);
                    self.landed(at);
                    z.owed += 1.0;
                    // A pillar thrown up every other step, not right in front of the eye
                    if z.owed as usize % 2 == 0 && (at - arena.view.eye).norm() > 4.5 * vpm {
                        let model = match element {
                            Some(Element::Frost) => ICE_BURST,
                            Some(Element::Holy) => PILLAR,
                            _ => STONE_PILLAR,
                        };
                        self.swells.push(Swell { model, position: at, radius: 0.6, life: 0.6, lifetime: 0.6 });
                    }
                    self.blasts.push(Blast { position: at + up * 0.3 * vpm, look, radius: 0.6, life: 0.3, lifetime: 0.3 });
                    for _ in 0..2 {
                        let v = (self.any_direction() * 0.6 + up * 1.4) * 4.0 * vpm;
                        self.drop(at, v, DROP_MUD, 0.9);
                    }
                    self.cues.push(Cue::at(Sound::Quake, at, 0.3).pitched(1.4 + 0.05 * z.owed as f32));
                    for i in Combat::foes_round(arena, at, z.radius) {
                        let id = arena.animals[i].id;
                        if z.hit.contains(&id) {
                            continue;
                        }
                        z.hit.push(id);
                        let center = arena.animals[i].center(vpm as u32);
                        self.power_hit(arena, &profile, i, center, z.damage, at);
                        if arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
                            let s = &mut arena.animals[i].status;
                            s.push = (up * 5.0 + flat_of(z.velocity) * 2.0) * vpm;
                            s.staggered = s.staggered.max(0.6);
                        }
                    }
                }
                if z.owed >= super::arsenal::STAR_STEPS {
                    z.life = z.life.min(0.3);
                }
            }
            // Hanging there crackling till it bursts along its length
            Lasting::Line => {
                if z.tick <= 0.0 && z.dwell <= 0.0 {
                    z.dwell = 1.0;
                    let length = z.velocity.norm() / vpm;
                    let mut on = 2.0;
                    while on < length {
                        let at = z.position + z.velocity * (on / length);
                        self.blasts.push(Blast { position: at, look, radius: 0.7, life: 0.3, lifetime: 0.3 });
                        self.sparks(at, 3, 2.5, look, 0.3, 0.3);
                        on += 2.5;
                    }
                    self.cap_sparks();
                    let (a, b) = (z.position, z.position + z.velocity);
                    let cut: Vec<usize> = (0..arena.animals.len())
                        .filter(|&i| {
                            let animal = &arena.animals[i];
                            animal.health > 0.0 && !animal.look.is_pet() && off_line(animal.center(vpm as u32), a, b) / vpm - animal.look.body().1 < z.radius
                        })
                        .collect();
                    for i in cut {
                        let center = arena.animals[i].center(vpm as u32);
                        self.power_hit(arena, &profile, i, center, z.damage, a);
                    }
                    self.cues.push(Cue::at(Sound::Burst, a.lerp(&b, 0.5), 0.9).pitched(1.1));
                    z.life = z.life.min(0.2);
                } else if z.dwell <= 0.0 && random(&mut self.rng) < dt * 30.0 {
                    let at = z.position + z.velocity * random(&mut self.rng);
                    let v = self.any_direction() * 0.4 * vpm;
                    self.fine(at, v, look, 0.3, 0.0);
                }
            }
            // Rolling out, what it reaches struck once and thrown back
            Lasting::Nova => {
                let reached = (0.6 + NOVA_SPEED * age).min(z.radius);
                let near: Vec<usize> = Combat::foes_round(arena, z.position, reached).into_iter().filter(|&i| !z.hit.contains(&arena.animals[i].id)).collect();
                for i in near {
                    z.hit.push(arena.animals[i].id);
                    let center = arena.animals[i].center(vpm as u32);
                    self.power_hit(arena, &profile, i, center, z.damage, z.position);
                    if arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
                        let away = arena.animals[i].position - z.position;
                        arena.animals[i].status.push = Vector3::new(away.x, away.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x) * 6.0 * vpm;
                    }
                }
                // Its element flaring up off it here and there as it goes
                if random(&mut self.rng) < dt * 40.0 && reached < z.radius {
                    let a = random(&mut self.rng) * TAU;
                    let at = z.position + Vector3::new(a.cos(), a.sin(), 0.0) * reached * vpm;
                    let v = (Vector3::new(a.cos(), a.sin(), 0.0) * 2.0 + up * 3.0) * vpm;
                    self.sparks.push(Spark { position: at, velocity: v, look, life: 0.4, lifetime: 0.4, gravity: 0.5, slot: SLOT_SPARK });
                    self.cap_sparks();
                }
            }
            // Out round the player and back in, cutting what it passes each way, dragging in what
            // it cuts coming back
            Lasting::BladeRing => {
                if let Some(feet) = arena.player {
                    z.position = feet + up * 0.9 * vpm;
                }
                let f = (age / RING_SECONDS).clamp(0.0, 1.0);
                if f >= 0.5 && z.owed < 1.0 {
                    z.owed = 1.0;
                    z.hit.clear();
                }
                let reached = ring_out(f, z.radius);
                let cut: Vec<usize> = (0..arena.animals.len())
                    .filter(|&i| {
                        let a = &arena.animals[i];
                        a.health > 0.0 && !a.look.is_pet() && !z.hit.contains(&a.id) && ((a.center(vpm as u32) - z.position).xy().norm() / vpm - reached).abs() < 1.2 + a.look.body().1
                    })
                    .collect();
                for i in cut {
                    z.hit.push(arena.animals[i].id);
                    let center = arena.animals[i].center(vpm as u32);
                    self.landed(center);
                    self.power_hit(arena, &profile, i, center, z.damage, z.position);
                    if z.owed >= 1.0 && arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
                        let to = z.position - arena.animals[i].position;
                        arena.animals[i].status.push = Vector3::new(to.x, to.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros) * 9.0 * vpm;
                    }
                }
                if random(&mut self.rng) < dt * 50.0 {
                    let a = random(&mut self.rng) * TAU;
                    let out = Vector3::new(a.cos(), a.sin(), 0.0);
                    let v = (Vector3::new(-out.y, out.x, 0.0) * 6.0 + up * 1.5) * vpm;
                    self.sparks.push(Spark { position: z.position + out * reached * vpm, velocity: v, look, life: 0.3, lifetime: 0.3, gravity: 0.3, slot: SLOT_SPARK });
                    self.cap_sparks();
                }
            }
            // Pulsing every so often, striking all round it each time
            Lasting::Stake => {
                while z.owed < STAKE_PULSES && age >= 0.15 + z.owed * STAKE_PULSE {
                    z.owed += 1.0;
                    let at = z.position + up * 0.3 * vpm;
                    self.landed(at);
                    self.power_burst(arena, &profile, at, z.radius, z.damage, None);
                    let model = match element {
                        Some(Element::Frost) => ICE_BURST,
                        Some(Element::Poison) => ROOTS,
                        Some(Element::Fire | Element::Storm | Element::Void) => STONE_PILLAR,
                        _ => PILLAR,
                    };
                    if (z.position - arena.view.eye).norm() > 4.5 * vpm {
                        self.swells.push(Swell { model, position: z.position, radius: 0.7, life: 0.45, lifetime: 0.45 });
                    }
                    self.sparks(at, 10, 3.5, look, 0.3, 0.4);
                    self.cues.push(Cue::at(Sound::Burst, at, 0.7).pitched(0.9 + 0.1 * z.owed as f32));
                    z.dwell = age;
                }
            }
            // Arrowlets let fall one after another, out over the ring
            Lasting::Downpour => {
                let due = ((age / DOWNPOUR_SECONDS).min(1.0) * ARROWLETS).floor();
                while z.owed < due {
                    z.owed += 1.0;
                    let a = random(&mut self.rng) * TAU;
                    let r = z.radius * random(&mut self.rng).sqrt();
                    let from = z.position + Vector3::new(a.cos() * r, a.sin() * r, -0.5) * vpm;
                    let v = Vector3::new(-a.cos() * 0.8, -a.sin() * 0.8, -22.0) * vpm;
                    self.launch(&profile, Some(Missile::arrow(0, true)), Form::Bolt, from, v, z.damage, 1.0);
                    self.signed(Sig::Power(Shape::Arrowlet));
                    if let Some(p) = self.projectiles.last_mut() {
                        p.range = f64::INFINITY;
                        p.size = 0.9;
                    }
                }
            }
            // Holding fast what it wound round, riding along on it, and what's by it
            Lasting::Bound => {
                if let Some(id) = z.target {
                    match arena.animals.iter().position(|a| a.id == id && a.health > 0.0) {
                        Some(i) => z.position = arena.animals[i].center(vpm as u32),
                        None => z.target = None,
                    }
                }
                for i in Combat::foes_round(arena, z.position, z.radius) {
                    if arena.animals[i].boss.is_none() {
                        let s = &mut arena.animals[i].status;
                        s.rooted = s.rooted.max(0.15);
                    }
                }
                if z.tick <= 0.0 {
                    z.tick = 1.0;
                    for i in Combat::foes_round(arena, z.position, z.radius * 0.6) {
                        let center = arena.animals[i].center(vpm as u32);
                        self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: false, from: z.position, depth: 1, element });
                        self.sparks(center, 4, 2.0, look, 0.3, 0.3);
                    }
                }
            }
            // Hanging there, sinking, burning sparks raining down off it on what's under it, and
            // marking what's under it
            Lasting::Flare => {
                let ground = Combat::floor_at(arena, z.position);
                if z.position.z > ground.z + 4.0 * vpm {
                    z.position.z -= 0.5 * vpm * dt;
                }
                for i in Combat::under(arena, ground, z.radius, 12.0) {
                    let s = &mut arena.animals[i].status;
                    s.marked = s.marked.max(0.4);
                    s.mark_bonus = s.mark_bonus.max(0.3);
                }
                z.owed += dt * 5.0;
                // Each spark at something under it, or anywhere under it if nothing is
                let under: Vec<usize> = Combat::under(arena, ground, z.radius, 12.0);
                while z.owed >= 1.0 {
                    z.owed -= 1.0;
                    let a = random(&mut self.rng) * TAU;
                    let r = z.radius * 0.85 * random(&mut self.rng).sqrt();
                    let pick = (random(&mut self.rng) * under.len() as f64) as usize;
                    let to = match under.get(pick) {
                        Some(&i) => arena.animals[i].center(vpm as u32),
                        None => ground + Vector3::new(a.cos() * r, a.sin() * r, 0.0) * vpm,
                    };
                    let time = 0.6;
                    self.launch(&profile, Some(Missile { shot: Shot::Ball, head: 0, power: false, size: 0.8 }), Form::Bolt, z.position, thrown(z.position, to, time, 0.6, vpm), z.damage, 0.6);
                    self.signed(Sig::Power(Shape::Ember));
                    if let Some(p) = self.projectiles.last_mut() {
                        p.range = 30.0 * vpm;
                        p.size = 0.8;
                    }
                }
                if random(&mut self.rng) < dt * 25.0 {
                    let v = self.any_direction() * 1.2 * vpm;
                    self.fine(z.position, v, look, 0.4, 0.3);
                }
            }
            // Tolling, a ring rolling out from it each time, through what it reaches
            Lasting::Beacon => {
                if z.owed < BEACON_TOLLS && age >= z.owed * BEACON_TOLL {
                    z.owed += 1.0;
                    z.dwell = age;
                    z.hit.clear();
                    if (z.position - arena.view.eye).norm() > 4.5 * vpm {
                        self.swells.push(Swell { model: PILLAR, position: z.position, radius: 0.9, life: 0.5, lifetime: 0.5 });
                    }
                    self.cues.push(Cue::at(Sound::Toll, z.position, 0.9).pitched(0.8 + 0.12 * z.owed as f32));
                    self.landed(z.position);
                }
                let reached = (0.5 + BEACON_SPEED * (age - z.dwell)).min(z.radius);
                let near: Vec<usize> = Combat::foes_round(arena, z.position, reached).into_iter().filter(|&i| !z.hit.contains(&arena.animals[i].id)).collect();
                for i in near {
                    z.hit.push(arena.animals[i].id);
                    let center = arena.animals[i].center(vpm as u32);
                    self.power_hit(arena, &profile, i, center, z.damage * 0.6, z.position);
                    if arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
                        let away = arena.animals[i].position - z.position;
                        let away = Vector3::new(away.x, away.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                        arena.animals[i].status.push = Vector3::new(away.x, away.y, 0.3) * 5.0 * vpm;
                    }
                }
            }
        }
    }

    /// One of their zones whose time is out
    pub(super) fn power_zone_ends(&mut self, arena: &mut Arena, z: &Zone, kind: Lasting) {
        let vpm = arena.view.vpm;
        let profile = z.profile.clone();
        match kind {
            // Bursting apart, its shards flying all round
            Lasting::Stuck { .. } => {
                self.power_burst(arena, &profile, z.position, z.radius, z.damage, None);
                for k in 0..8 {
                    let a = k as f64 / 8.0 * TAU;
                    let out = Vector3::new(a.cos(), a.sin(), 0.35).normalize();
                    self.launch(&profile, Some(Missile { shot: Shot::Ball, head: 0, power: false, size: 0.8 }), Form::Bolt, z.position + out * 0.4 * vpm, out * 16.0 * vpm, z.damage * 0.35, 0.5);
                    self.signed(Sig::Power(Shape::Ember));
                    if let Some(p) = self.projectiles.last_mut() {
                        p.range = 7.0 * vpm;
                        p.size = 0.8;
                    }
                }
            }
            Lasting::Vortex => {
                self.power_burst(arena, &profile, z.position, 3.0, z.damage, None);
                self.shake = self.shake.max(0.5);
            }
            // Its last pulse a big one as it crumbles
            Lasting::Stake => {
                self.power_burst(arena, &profile, z.position + Vector3::new(0.0, 0.0, 0.4 * vpm), z.radius * 1.2, z.damage * 0.8, None);
                self.sparks(z.position, 14, 4.0, Element::look(profile.element), 0.4, 0.4);
            }
            Lasting::Fissure | Lasting::Line | Lasting::Nova | Lasting::BladeRing | Lasting::Downpour | Lasting::Bound | Lasting::Flare | Lasting::Beacon => {}
            Lasting::Knife { .. } | Lasting::Crack | Lasting::Runes | Lasting::Sigil => {}
            Lasting::Vigil | Lasting::Heavenfall | Lasting::Storm | Lasting::Glacier | Lasting::Grasp => self.miracle_zone_ends(arena, z, kind),
        }
    }

    /// How one of their shots is drawn: a crescent as an arc of glowing streaks, a wheel as a ring
    /// of them spinning, a spear or lance with its streak behind it, arrows and bolts bigger than
    /// any, glowing orbs and shells. Shown however much else is about.
    pub(super) fn power_shot_instances(&self, p: &Projectile, shape: Shape, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        let look = Element::look(p.profile.element);
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &way).unwrap_or_else(UnitQuaternion::identity);
        let spin = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.time * 7.0) * rotation;
        // A little bigger the further off past 15 m, as other shots are
        let far = ((p.position - eye).norm() / vpm / 15.0).max(1.0).sqrt();
        let ball = |size: f64| {
            let (model, ball) = self.effect_model(SLOT_FIREBALL, look);
            (eye, place(model, ball, MODEL_RES, Vector3::zeros(), p.position, spin, vpm, size * far, MOVING))
        };
        let mut out = Vec::new();
        match shape {
            Shape::Wave { width, .. } => {
                let across = way.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                let w = width as f64 * vpm;
                for k in 0..7 {
                    let s = k as f64 / 3.0 - 1.0;
                    let at = p.position + across * s * 0.5 * w - way * s * s * 0.3 * w;
                    let tangent = across * 0.5 - way * s * 0.6;
                    out.extend(self.streak_at(eye, at, tangent, look, (1.3 + 0.25 * width as f64) * far * (1.0 - 0.3 * s.abs())));
                }
            }
            Shape::Wheel { .. } => out.extend(self.ring_instances(eye, p.position, Vector3::z(), 0.75, look, 1.2 * far, self.time * 16.0)),
            // A shaft of light, its head glowing
            Shape::Javelin => {
                out.extend(self.line_bolts(eye, p.position - way * 2.6 * vpm, p.position, look, 0.5, 2.2 * far, 0.0));
                out.extend(self.streak_at(eye, p.position + way * 0.2 * vpm, way, look, 3.2 * far));
                out.push(ball(0.12));
            }
            Shape::Arrow => {
                let (model, arrow) = self.effect_model(SLOT_ARROW + p.head.min(ARROW_HEADS - 1), look);
                out.push((eye, place(model, arrow, ARROW_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), p.position, rotation, vpm, 1.5 * far, MOVING)));
                out.extend(self.line_bolts(eye, p.position - way * 2.4 * vpm, p.position - way * 0.4 * vpm, look, 0.6, 1.2 * far, 0.0));
            }
            Shape::Bolt => {
                let (model, quarrel) = self.effect_model(SLOT_QUARREL, look);
                out.push((eye, place(model, quarrel, ARROW_RES, Vector3::new(QUARREL_HEAD, 0.0, 0.0), p.position, rotation, vpm, 1.7 * far, MOVING)));
                out.extend(self.line_bolts(eye, p.position - way * 1.6 * vpm, p.position - way * 0.3 * vpm, look, 0.55, 1.0 * far, 0.0));
            }
            Shape::Slug(_) => {
                out.push(ball(0.14));
                out.extend(self.line_bolts(eye, p.position - way * 1.4 * vpm, p.position, look, 0.5, 1.1 * far, 0.0));
            }
            Shape::Lance => {
                out.push(ball(0.11));
                out.extend(self.line_bolts(eye, p.position - way * 4.0 * vpm, p.position, look, 0.8, 1.4 * far, 0.0));
            }
            Shape::Ember => out.push(ball(0.11 * p.size.max(0.8))),
            Shape::Mote => {
                out.push(ball(0.09));
                out.extend(self.streak_at(eye, p.position - way * 0.3 * vpm, way, look, 0.9 * far));
            }
            Shape::Shell | Shape::Charge => {
                let (model, bomb) = self.effect_model(SLOT_BOMB, look);
                let tumble = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.time * 8.0) * UnitQuaternion::from_axis_angle(&Vector3::z_axis(), p.position.x * 0.3);
                out.push((eye, place(model, bomb, BOMB_RES, Vector3::zeros(), p.position, tumble, vpm, 1.4 * far, MOVING)));
                if shape == Shape::Charge {
                    out.extend(self.ring_instances(eye, p.position, eye - p.position, 0.5, look, 0.8 * far, self.time * 9.0));
                }
            }
            Shape::Orb => {
                out.push(ball(0.36));
                out.extend(self.ring_instances(eye, p.position, eye - p.position, 0.75 * far, look, 1.0 * far, self.time * 4.0));
            }
            // A whirling funnel, spinning, rings of its element turning round it as it goes
            Shape::Gale => {
                let (model, twister) = self.model_at(TWISTER);
                let foot = p.position - Vector3::new(0.0, 0.0, 0.7 * vpm);
                let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 9.0);
                out.push((eye, place(model, twister, MODEL_RES, Vector3::zeros(), foot, turn, vpm, 1.15, MOVING)));
                for (k, r) in [0.55, 0.85, 1.2].into_iter().enumerate() {
                    let at = foot + Vector3::new(0.0, 0.0, (0.3 + 0.75 * k as f64) * vpm);
                    out.extend(self.circle_instances(eye, at, r, look, 1.1, self.time * (7.0 + 2.0 * k as f64)));
                }
            }
            // A fist of light, its streak behind it
            Shape::Fist => {
                out.push(ball(0.1));
                out.extend(self.streak_at(eye, p.position - way * 0.2 * vpm, way, look, 1.8 * far));
            }
            // The hook's head, its chain run out behind it from the hand
            Shape::Hook { .. } => {
                let hand = eye + view.rotation * Vector3::new(0.25, 0.5, -0.3) * vpm;
                out.extend(self.line_bolts(eye, hand, p.position, look, 0.7, 0.8, 0.0));
                let head = if p.velocity.dot(&(p.position - hand)) < 0.0 { -way } else { way };
                out.extend(self.streak_at(eye, p.position, head, look, 1.6 * far));
                out.push(ball(0.09));
            }
            // An anvil of stone falling, glowing, its streak above it, a ring of its element round
            Shape::Anvil => {
                let (model, boulder) = self.model_at(BOULDER);
                let tumble = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.time * 2.0);
                out.push((eye, place(model, boulder, MODEL_RES, Vector3::zeros(), p.position, tumble, vpm, 1.6, MOVING)));
                out.push(ball(0.22));
                out.extend(self.line_bolts(eye, p.position + Vector3::new(0.0, 0.0, 3.0 * vpm), p.position, look, 0.7, 1.6, 0.0));
                out.extend(self.circle_instances(eye, p.position, 0.9, look, 1.2, self.time * 5.0));
            }
            Shape::Stake => {
                out.extend(self.line_bolts(eye, p.position - way * 2.4 * vpm, p.position, look, 0.5, 2.0 * far, 0.0));
                out.extend(self.streak_at(eye, p.position + way * 0.2 * vpm, way, look, 3.0 * far));
                out.push(ball(0.1));
            }
            Shape::Cloud => {
                let (model, arrow) = self.effect_model(SLOT_ARROW + p.head.min(ARROW_HEADS - 1), look);
                out.push((eye, place(model, arrow, ARROW_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), p.position, rotation, vpm, 1.4 * far, MOVING)));
                out.extend(self.line_bolts(eye, p.position - way * 2.0 * vpm, p.position - way * 0.4 * vpm, look, 0.6, 1.1 * far, 0.0));
            }
            Shape::Arrowlet => {
                let (model, arrow) = self.effect_model(SLOT_ARROW, look);
                out.push((eye, place(model, arrow, ARROW_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), p.position, rotation, vpm, 0.9 * far, MOVING)));
            }
            // Two balls spinning round each other on their line as they fly
            Shape::Bolas => {
                let across = way.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                let over = across.cross(&way);
                let a = self.time * 22.0;
                let arm = (across * a.cos() + over * a.sin()) * 0.45 * vpm;
                let (model, b) = self.effect_model(SLOT_FIREBALL, look);
                for s in [-1.0, 1.0] {
                    out.push((eye, place(model, b, MODEL_RES, Vector3::zeros(), p.position + arm * s, spin, vpm, 0.12 * far, MOVING)));
                }
                out.extend(self.line_bolts(eye, p.position - arm, p.position + arm, look, 0.3, 0.7 * far, 0.0));
            }
            Shape::Flare => {
                out.push(ball(0.16));
                out.extend(self.streak_at(eye, p.position - way * 0.3 * vpm, way, look, 1.4 * far));
            }
            Shape::Beacon => {
                out.push(ball(0.24));
                out.extend(self.ring_instances(eye, p.position, eye - p.position, 0.5 * far, look, 0.9 * far, self.time * 6.0));
            }
            Shape::Pot => {
                let (model, bomb) = self.effect_model(SLOT_BOMB, look);
                let tumble = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.time * 8.0) * UnitQuaternion::from_axis_angle(&Vector3::z_axis(), p.position.x * 0.3);
                out.push((eye, place(model, bomb, BOMB_RES, Vector3::zeros(), p.position, tumble, vpm, 1.2 * far, MOVING)));
            }
            Shape::Shard => {
                out.push(ball(0.07));
                out.extend(self.streak_at(eye, p.position - way * 0.25 * vpm, way, look, 0.8 * far));
            }
            Shape::Spear { big } => out.extend(self.spear_instances(p, big, view)),
            Shape::Shade => out.extend(self.shade_instances(p, view)),
            // The knife or chakram itself, as it's drawn in the hand, see thrown.rs, a little
            // bigger the further off it is
            Shape::Knife | Shape::Chakram { .. } => {
                if let Some(f) = self.flung() {
                    let turn = if shape == Shape::Knife { crate::combat::thrown::knife_turn(way, self.time) } else { crate::combat::thrown::chakram_turn(way, self.time) };
                    out.push((eye + (p.position - eye) * 0.1, f.instance(p.position, turn, vpm, far.min(1.6), MOVING)));
                } else {
                    out.extend(self.streak_at(eye, p.position, way, look, 1.0 * far));
                }
                if matches!(shape, Shape::Chakram { .. }) && p.profile.element.is_some() {
                    out.extend(self.circle_instances(eye, p.position, 0.32, look, 0.5 * far, self.time * 20.0));
                }
            }
            Shape::Step | Shape::Ray | Shape::Glyph | Shape::Sigil => out.extend(self.arsenal_shot_instances(p, shape, view)),
            Shape::Vigil | Shape::Ascent | Shape::Storm | Shape::Grasp | Shape::Helix { .. } => out.extend(self.miracle_shot_instances(p, shape, view)),
        }
        out
    }

    /// How one of their zones is drawn
    pub(super) fn power_zone_instances(&self, z: &Zone, kind: Lasting, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        let look = Element::look(z.profile.element);
        let age = z.lifetime - z.life;
        let up = Vector3::z();
        let mut out = Vec::new();
        match kind {
            // A knife stuck in the ground, sinking away at the last, see thrown.rs
            Lasting::Knife { way } => {
                if let Some(f) = self.flung() {
                    let way = Vector3::new(way[0] as f64, way[1] as f64, way[2] as f64);
                    let sunk = smooth(1.0 - z.life / 0.4) * 0.12 * vpm;
                    let lying = UnitQuaternion::rotation_between(&Vector3::z(), &way).unwrap_or_else(UnitQuaternion::identity);
                    out.push((z.position, f.instance(z.position - way * 0.05 * vpm + way * sunk, lying, vpm, 1.0, if z.life < 0.4 { MOVING } else { STILL })));
                }
            }
            Lasting::Runes | Lasting::Sigil => out.extend(self.runes_instances(z, view, kind == Lasting::Sigil)),
            Lasting::Vigil | Lasting::Heavenfall | Lasting::Storm | Lasting::Glacier | Lasting::Grasp => out.extend(self.miracle_zone_instances(z, kind, view)),
            // A glowing crack along the ground as far as it's split
            Lasting::Fissure | Lasting::Crack => {
                let to = z.position + z.velocity * z.owed.max(0.5) + up * 0.15 * vpm;
                out.extend(self.line_bolts(eye, z.position + up * 0.15 * vpm, to, look, 1.2, 1.3, 0.15));
            }
            // The bolt stuck there, a ring blinking faster round it
            Lasting::Stuck { way } => {
                let way = Vector3::new(way[0] as f64, way[1] as f64, way[2] as f64);
                let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &way).unwrap_or_else(UnitQuaternion::identity);
                let (model, quarrel) = self.effect_model(SLOT_QUARREL, look);
                out.push((eye, place(model, quarrel, ARROW_RES, Vector3::new(QUARREL_HEAD, 0.0, 0.0), z.position + way * 0.15 * vpm, rotation, vpm, 1.7, MOVING)));
                if (age * (6.0 + 20.0 * age / STUCK_SECONDS)).fract() < 0.5 {
                    out.extend(self.ring_instances(eye, z.position, eye - z.position, 0.45, look, 0.9, self.time * 5.0));
                }
            }
            // Two rings turning opposite ways, drawing in, the charge at the middle
            Lasting::Vortex => {
                let f = age / VORTEX_SECONDS;
                let (model, bomb) = self.effect_model(SLOT_BOMB, look);
                out.push((eye, place(model, bomb, BOMB_RES, Vector3::zeros(), z.position, UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 12.0), vpm, 1.3 * (1.0 + 0.15 * (self.time * 20.0).sin()), MOVING)));
                out.extend(self.circle_instances(eye, z.position, (z.radius * (1.0 - 0.75 * f)).max(0.8), look, 1.6, self.time * 3.0));
                out.extend(self.circle_instances(eye, z.position + up * 0.4 * vpm, (z.radius * 0.55 * (1.0 - 0.6 * f)).max(0.6), look, 1.3, -self.time * 4.5));
            }
            Lasting::Line if z.dwell <= 0.0 => {
                let grown = smooth(age / 0.1).max(0.05);
                out.extend(self.line_bolts(eye, z.position, z.position + z.velocity * grown, look, 1.6, 1.4, 0.05));
            }
            Lasting::Line => {}
            Lasting::Nova => {
                let fade = (z.life / 0.3).clamp(0.1, 1.0);
                let reached = (0.6 + NOVA_SPEED * age).min(z.radius);
                out.extend(self.circle_instances(eye, z.position, reached, look, 2.4 * fade, age * 0.6));
            }
            // Two rings of spinning blades, turning opposite ways
            Lasting::BladeRing => {
                let reached = ring_out(age / RING_SECONDS, z.radius);
                out.extend(self.circle_instances(eye, z.position, reached, look, 2.2, age * 9.0));
                out.extend(self.circle_instances(eye, z.position + up * 0.3 * vpm, reached * 0.94, look, 1.6, -age * 7.0));
            }
            // The spear standing in the ground, a ring rolling out from it at each pulse
            Lasting::Stake => {
                let way = z.velocity.try_normalize(1e-6).unwrap_or(-up);
                let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &way).unwrap_or_else(UnitQuaternion::identity);
                let (model, quarrel) = self.effect_model(SLOT_QUARREL, look);
                let head = z.position + up * 0.1 * vpm;
                out.push((eye, place(model, quarrel, ARROW_RES, Vector3::new(QUARREL_HEAD, 0.0, 0.0), head, rotation, vpm, 2.2, MOVING)));
                out.extend(self.line_bolts(eye, head - way * 2.2 * vpm, head, look, 0.55, 1.4, 0.0));
                let since = age - z.dwell;
                if z.owed > 0.0 && since < 0.35 {
                    out.extend(self.circle_instances(eye, z.position + up * 0.1 * vpm, (0.5 + 10.0 * since).min(z.radius), look, 1.8 * (1.0 - since / 0.35), age));
                }
            }
            // The arrow's burst, spreading and fading as the arrowlets come down
            Lasting::Downpour => {
                let fade = (z.life / z.lifetime).clamp(0.1, 1.0);
                out.extend(self.circle_instances(eye, z.position, 1.0 + 2.5 * age, look, 1.3 * fade, age * 2.0));
            }
            // Its cords wound round what it holds, the balls swinging round
            Lasting::Bound => {
                let fade = (z.life / 0.4).clamp(0.1, 1.0);
                out.extend(self.circle_instances(eye, z.position, 0.7, look, 0.9 * fade, age * 6.0));
                let (model, b) = self.effect_model(SLOT_FIREBALL, look);
                for s in [0.0, PI] {
                    let a = age * 8.0 + s;
                    let at = z.position + Vector3::new(a.cos() * 0.75, a.sin() * 0.75, -0.2) * vpm;
                    out.push((eye, place(model, b, MODEL_RES, Vector3::zeros(), at, UnitQuaternion::identity(), vpm, 0.1, MOVING)));
                }
            }
            // A burning star hanging there, flickering, a ring round it
            Lasting::Flare => {
                let (model, b) = self.effect_model(SLOT_FIREBALL, look);
                let flicker = 1.0 + 0.12 * (self.time * 31.0).sin();
                out.push((eye, place(model, b, MODEL_RES, Vector3::zeros(), z.position, UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 3.0), vpm, 0.34 * flicker, MOVING)));
                out.extend(self.ring_instances(eye, z.position, eye - z.position, 0.7 * flicker, look, 1.2, self.time * 2.0));
            }
            // A column of light standing over where it was set down, its orb bobbing at the top,
            // and the ring rolling out from its toll
            Lasting::Beacon => {
                let top = z.position + up * (1.6 + 0.1 * (age * 3.0).sin()) * vpm;
                out.extend(self.line_bolts(eye, z.position, top, look, 0.5, 1.5, 0.0));
                let (model, b) = self.effect_model(SLOT_FIREBALL, look);
                out.push((eye, place(model, b, MODEL_RES, Vector3::zeros(), top, UnitQuaternion::from_axis_angle(&Vector3::z_axis(), age * 4.0), vpm, 0.22, MOVING)));
                let since = age - z.dwell;
                let reached = 0.5 + BEACON_SPEED * since;
                if reached < z.radius + 0.5 {
                    out.extend(self.circle_instances(eye, z.position + up * 0.15 * vpm, reached.min(z.radius), look, 2.2, age));
                }
            }
        }
        out
    }

    /// What their zones light round them: where, how far, what color and how bright
    pub(super) fn power_light(&self, z: &Zone, kind: Lasting) -> Option<(Vector3<f64>, f64, [f32; 3], f32)> {
        let color = z.profile.element.map_or([0.95, 0.9, 0.8], Element::light);
        let fade = (z.life / 0.3).min(1.0) as f32;
        let age = z.lifetime - z.life;
        match kind {
            Lasting::Vortex => Some((z.position, 7.0, color, 0.9 * fade)),
            Lasting::Nova | Lasting::BladeRing => Some((z.position + Vector3::new(0.0, 0.0, self.vpm), 10.0, color, 0.8 * fade)),
            Lasting::Stuck { .. } => Some((z.position, 3.0, color, 0.6)),
            // Flaring up with each pulse or toll
            Lasting::Stake | Lasting::Beacon => Some((z.position + Vector3::new(0.0, 0.0, self.vpm), 7.0, color, (0.35 + 0.65 * (1.0 - (age - z.dwell) / 0.4).max(0.0)) as f32 * fade)),
            // A flare lighting up the ground all round under it
            Lasting::Flare => Some((z.position, 16.0, color, 1.2 * fade)),
            Lasting::Fissure | Lasting::Line | Lasting::Downpour | Lasting::Bound | Lasting::Knife { .. } | Lasting::Crack => None,
            Lasting::Vigil | Lasting::Heavenfall | Lasting::Storm | Lasting::Glacier | Lasting::Grasp => self.miracle_light(z, kind),
            // A circle of runes glowing as it's written, blazing as it blasts up
            Lasting::Runes | Lasting::Sigil => {
                let blaze = if z.dwell > 0.0 { (1.0 - (age - z.dwell) / 0.4).max(0.0) * 2.0 } else { 0.25 + 0.1 * z.owed };
                Some((z.position + Vector3::new(0.0, 0.0, self.vpm), 8.0, z.profile.element.map_or(Element::Arcane.light(), Element::light), blaze as f32 * fade))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapons::Library;

    /// Open ground, 48 m across, four meters of earth under four of sky
    fn field() -> World {
        let size = [384u32, 384, 64];
        let (tx, ty) = (size[0] as usize / 4, size[1] as usize / 4);
        let mut occupancy = vec![0u128; tx * ty * (size[2] as usize / 8)];
        occupancy[..tx * ty * 4].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, crate::worldgen::material::DIRT);
        world.voxels_per_meter = 8;
        world
    }

    /// Whether `family` is one of the gathering tools, see tools.rs: they're never legendary, never
    /// rolled as loot, and have no power
    fn is_tool(family: &str) -> bool {
        crate::tools::Tool::ALL.iter().any(|t| t.family() == family)
    }

    /// A legendary or mythic weapon of `family`, as one would drop, with its family's own power
    fn legendary(library: &Library, family: usize) -> Weapon {
        let power = Power::of(&library.families[family].id);
        (0..40_000u32)
            .map(|seed| library.generate(hash(seed ^ 0x90e7), Some(family), 8.0))
            .find(|w| w.profile.power.is_some() && w.profile.power == power)
            .unwrap_or_else(|| panic!("no legendary or mythic {} rolled", library.families[family].id))
    }

    /// A weapon of each part that brings a power of its own, see weapons::Component::power, with
    /// the part's id: rolled as one would drop with that part on it
    fn with_powers(library: &Library) -> Vec<(String, Weapon)> {
        library.components.iter().filter(|c| c.power().is_some()).map(|c| (c.id.clone(), library.with_part(&c.id, None).unwrap_or_else(|| panic!("no weapon with {}", c.id)))).collect()
    }

    /// What using a power looked like, a pack ahead or not
    struct Used {
        /// Seconds after right click that anything of it was first to be seen out away from the
        /// weapon, and seconds it was to be seen in all
        first_seen: Option<f64>,
        seen: f64,
        /// How far the weapon's tip moved through it, in meters
        travel: f64,
        /// Seconds after right click that the pack was first hurt by it, and whether it went
        first_hurt: Option<f64>,
        went: bool,
    }

    /// Right click with `weapon` as a player would to use its power, standing on open ground,
    /// looking a little down at it, with `foes` a pack of bandits 8.5 m ahead, out of reach of the
    /// weapon's own blows, a whip's crack too, watching three seconds of it
    fn use_power(world: &World, weapon: &Weapon, foes: bool) -> Used {
        let vpm = 8.0;
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let feet = Vector3::new(24.0, 10.0, 4.0) * vpm;
        let view = View { eye: feet + Vector3::new(0.0, 0.0, 1.6 * vpm), rotation: UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.12), tan: [1.2445, 0.7002], vpm, speed: 0.0, stride: 0.0 };
        let mut animals = if foes { crate::animals::spawn_pack(world, &water, &[Look::Bandit; 4], feet + Vector3::new(0.0, 8.5 * vpm, 0.0), crate::animals::PACK_SPREAD, 3) } else { Vec::new() };
        let health: Vec<f64> = animals.iter().map(|a| a.health).collect();
        let mut combat = Combat::new(1, 0);
        let dt = 1.0 / 120.0;
        let start = 0.4;
        let mut out = Used { first_seen: None, seen: 0.0, travel: 0.0, first_hurt: None, went: false };
        let mut last_tip: Option<Vector3<f64>> = None;
        for frame in 0..((start + 3.0) / dt) as usize {
            let t = frame as f64 * dt - start;
            // As a player would, see power_controls()
            let controls = combat.power_controls(weapon, t, dt);
            let mut arena = Arena { world, animals: &mut animals, view: &view, player: Some(feet), water: &water };
            combat.update(&mut arena, Some((0, weapon)), None, controls, dt);
            if t < 0.0 {
                continue;
            }
            out.went |= combat.power.cooldown > 0.0;
            let tip = combat.held_point(weapon, &view, false);
            if let Some(last) = last_tip {
                out.travel += (tip - last).norm() / vpm;
            }
            last_tip = Some(tip);
            if out.first_hurt.is_none() && animals.iter().zip(&health).any(|(a, &h)| a.health < h - 1e-6) {
                out.first_hurt = Some(t);
            }
            // What of the power's to be seen: on screen, not lost in the distance, out away
            // from the weapon
            let hand = view.eye + view.rotation * combat.pose(weapon).hand * vpm;
            let away = |at: &Vector3<f64>| (at - tip).norm() > 1.2 * vpm && (at - hand).norm() > 1.2 * vpm;
            let on = |instance: &AnimalInstance| {
                let (at, reach) = instance.sphere();
                let d = (at - view.eye).norm();
                away(&at) && d > 0.4 * vpm && reach / d > 0.004 && view.project(at).is_some_and(|[x, y]| (-0.02..=1.02).contains(&x) && (-0.02..=1.02).contains(&y))
            };
            // What it shoots and leaves going, and what it sets off: bursts, eruptions, sparks
            if combat.flying_instances(&view).iter().any(|(_, i)| on(i)) {
                out.seen += dt;
                out.first_seen.get_or_insert(t);
            }
        }
        out
    }

    /// Every family's legendary and mythic weapons have a power on right click, its own family's,
    /// named and described in every element, with a cooldown, those that cast costing mana; and
    /// a weapon short of legendary has none
    #[test]
    fn every_legendary_family_has_a_power() {
        let library = Library::load();
        let mut seen = Vec::new();
        for (family, f) in library.families.iter().enumerate().filter(|(_, f)| !is_tool(&f.id)) {
            let power = Power::of(&f.id).unwrap_or_else(|| panic!("{} has no power", f.id));
            let weapon = legendary(&library, family);
            assert_eq!(weapon.profile.power, Some(power), "{}", f.id);
            seen.push(power);
            let common = library.generate(7, Some(family), 0.0);
            // But for a skull focus and a talisman, whose powers are theirs at any rarity
            if Power::innate(&f.id) {
                assert_eq!(common.profile.power, Some(power), "a common {} has no power", f.id);
            } else {
                assert!(common.profile.power.is_none() || common.profile.special.is_some(), "a common {} has a power", f.id);
            }
        }
        // A part that brings a power of its own gives it to the weapon it's on, in place of its
        // family's
        for (id, weapon) in with_powers(&library) {
            let c = library.components.iter().find(|c| c.id == id).unwrap();
            assert_eq!(weapon.profile.power, c.power(), "{id}");
            seen.push(c.power().unwrap());
        }
        for power in Power::ALL {
            assert!(seen.contains(&power), "{power:?} isn't any family's");
            assert!(power.cooldown() >= 6.0, "{power:?} comes round too often");
            for element in std::iter::once(None).chain(Element::ALL.map(Some)) {
                assert!(power.name(element).split(' ').count() >= 2 && power.description(element).len() > 60, "{power:?} in {element:?}");
            }
        }
    }

    /// The rule for every family's legendary and mythic weapons' power: right click, as a player
    /// would, sets it going; the weapon moves through it; what it does flies, rolls or erupts out
    /// to be seen, alone or with a pack ahead; it never reaches the pack 8.5 m off the moment it's
    /// used; and it does reach it
    #[test]
    fn every_power_is_seen_and_animated() {
        let library = Library::load();
        let world = field();
        let mut broken = Vec::new();
        // Every family's own, and every one a part brings
        let mut weapons: Vec<(String, Weapon)> = library.families.iter().enumerate().filter(|(_, f)| !is_tool(&f.id)).map(|(family, f)| (f.id.clone(), legendary(&library, family))).collect();
        weapons.extend(with_powers(&library).into_iter().map(|(id, w)| (format!("{} with {id}", library.families[w.family].id), w)));
        for (what, weapon) in weapons {
            let name = format!("{} on a {what}", weapon.profile.power.unwrap().name(weapon.profile.element));
            for foes in [false, true] {
                let used = use_power(&world, &weapon, foes);
                let pack = if foes { "with a pack ahead" } else { "alone" };
                if !used.went {
                    broken.push(format!("{name}, {pack}: right click never set it going"));
                    continue;
                }
                if used.travel < 0.2 {
                    broken.push(format!("{name}, {pack}: the weapon only moves {:.2} m", used.travel));
                }
                if used.seen < 0.3 {
                    broken.push(format!("{name}, {pack}: it's only to be seen for {:.2} s", used.seen));
                }
                if foes {
                    match used.first_hurt {
                        Some(h) if used.first_seen.is_some_and(|s| h <= s + 1.5 / 120.0) => broken.push(format!("{name}: the pack's hurt {h:.3} s in, the moment it's first seen")),
                        Some(_) => {}
                        None => broken.push(format!("{name}: it never reaches the pack 8.5 m ahead")),
                    }
                }
            }
        }
        assert!(broken.is_empty(), "{} break the rule:\n{}", broken.len(), broken.join("\n"));
    }

    /// Where a family's power's shots and zones go, used as a player would, to look at:
    /// POWER=bombs cargo test --release power_paths -- --ignored --nocapture
    #[test]
    #[ignore]
    fn power_paths() {
        let library = Library::load();
        let world = field();
        let only = std::env::var("POWER").unwrap_or_default();
        let vpm = 8.0;
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let feet = Vector3::new(24.0, 10.0, 4.0) * vpm;
        let view = View { eye: feet + Vector3::new(0.0, 0.0, 1.6 * vpm), rotation: UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.12), tan: [1.2445, 0.7002], vpm, speed: 0.0, stride: 0.0 };
        for (family, f) in library.families.iter().enumerate().filter(|(_, f)| !is_tool(&f.id)) {
            if !only.is_empty() && f.id != only {
                continue;
            }
            let weapon = legendary(&library, family);
            println!("== {} on a {}", weapon.profile.power.unwrap().name(weapon.profile.element), f.id);
            let mut animals = crate::animals::spawn_pack(&world, &water, &[Look::Bandit; 4], feet + Vector3::new(0.0, 8.5 * vpm, 0.0), crate::animals::PACK_SPREAD, 3);
            let mut combat = Combat::new(1, 0);
            let dt = 1.0 / 120.0;
            for frame in 0..(3.4 / dt) as usize {
                let t = frame as f64 * dt - 0.4;
                let controls = combat.power_controls(&weapon, t, dt);
                let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet), water: &water };
                combat.update(&mut arena, Some((0, &weapon)), None, controls, dt);
                if frame % 12 == 0 && t >= 0.0 {
                    let at = |v: Vector3<f64>| format!("({:.1}, {:.1}, {:.1})", (v.x - feet.x) / vpm, (v.y - feet.y) / vpm, (v.z - feet.z) / vpm);
                    let shots: Vec<String> = combat.projectiles.iter().filter_map(|p| if let Trick::Signature(Sig::Power(s)) = p.trick { Some(format!("{s:?} {}", at(p.position))) } else { None }).collect();
                    let zones: Vec<String> = combat.zones.iter().filter_map(|z| if let ZoneKind::Signature(SigZone::Power(k)) = z.kind { Some(format!("{k:?} {}", at(z.position))) } else { None }).collect();
                    let hurt = animals.iter().filter(|a| a.health < a.max_health).count();
                    let doing = match combat.action {
                        Action::Power { .. } => "power",
                        Action::Swing { .. } => "swing",
                        Action::Draw { .. } => "draw",
                        Action::Brace { .. } => "brace",
                        Action::Throw { .. } => "throw",
                        Action::Idle => "idle",
                        _ => "other",
                    };
                    println!("{t:.2} s: {doing} shots {shots:?} zones {zones:?} hurt {hurt}");
                }
            }
        }
    }

    /// No basic function lost: a crossbow or gun with no power still loads early on right click,
    /// and one with a power shoots it on right click and loads early on Q
    #[test]
    fn right_click_still_loads_and_q_does_too() {
        let library = Library::load();
        let world = field();
        let vpm = 8.0;
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let feet = Vector3::new(24.0, 10.0, 4.0) * vpm;
        let view = View { eye: feet + Vector3::new(0.0, 0.0, 1.6 * vpm), rotation: UnitQuaternion::identity(), tan: [1.2445, 0.7002], vpm, speed: 0.0, stride: 0.0 };
        // A repeater, so one shot leaves it neither full nor empty
        let family = library.family("repeater").expect("repeaters");
        let plain = library.generate(7, Some(family), 0.0);
        let grand = legendary(&library, family);
        for (weapon, legendary) in [(&plain, false), (&grand, true)] {
            assert_eq!(weapon.profile.power.is_some(), legendary);
            let mut animals = Vec::new();
            let mut combat = Combat::new(1, 0);
            let mut step = |combat: &mut Combat, controls: Controls| {
                let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet), water: &water };
                combat.update(&mut arena, Some((0, weapon)), None, controls, 1.0 / 60.0);
            };
            for _ in 0..30 {
                step(&mut combat, Controls::default());
            }
            // One shot, so it's not full, then right click
            step(&mut combat, Controls { attack: true, holding: true, ..Default::default() });
            for _ in 0..60 {
                step(&mut combat, Controls::default());
            }
            step(&mut combat, Controls { heavy: true, aiming: true, ..Default::default() });
            if legendary {
                // Aimed while it's held, shot as it's let go
                assert!(!matches!(combat.action, Action::Power { .. }), "a legendary repeater's power waits to be let go of");
                step(&mut combat, Controls::default());
                assert!(matches!(combat.action, Action::Power { .. }), "letting go of right click shoots a legendary repeater's power");
                for _ in 0..60 {
                    step(&mut combat, Controls::default());
                }
                step(&mut combat, Controls { reload: true, ..Default::default() });
            }
            assert!(matches!(combat.action, Action::Reload { .. }), "a {} repeater loads early", if legendary { "legendary" } else { "plain" });
        }
    }
}
