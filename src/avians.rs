//! Avians, and the winged rig they share with the gargoyles. Harpies, bird-women with talons for
//! feet, flock over the sky isles and roost on their edges; skyhunters, great hawks, circle high
//! and stoop on what they see. The sky isles' three raid bosses are theirs: the Cloudreaver Roc,
//! which dive-bombs and snatches; the Thunderbird, which calls the lightning down; and the Harpy
//! Queen, with her volleys of feathers and the gales she beats up to throw the player off the
//! islands. The Stormtalon and the Harridan guard them. Gargoyles, winged fiends of stone, are
//! drawn and flown the same way, see src\gargoyles.rs for their bodies and their stone.
//!
//! As a dragon is, see src\dragons.rs, each is drawn out of jointed pieces, each a little model of
//! its own voxelized once and turned about its joint every frame: a torso, head and lower jaw, a
//! tail, two pieces to each wing, the legs and, for the gargoyles, the arms. So its wings beat
//! with a hard downstroke and fold at the wrist on the way up, it glides, folds them tight to dive
//! and flares them to land, and it holds itself as it lands, perches, preens and strikes.
//!
//! Each thinks for itself, see act(): what it shares with the other bosses, the fight being on,
//! the pack, the raid, it keeps in the Boss beside it, and what it does to the player goes out as
//! Deed::Aerie, carried out in src\game\aerie.rs.

use std::f64::consts::{PI, TAU};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use nalgebra::{Quaternion, UnitQuaternion, Vector3};

use crate::animals::{self, Animal, AnimalInstance, Look, MODEL_RES, MOVING, Model, Part, ball, ellipsoid, limb, voxelize};
use crate::bosses::{Boss, Deed};
use crate::gargoyles;
use crate::player::WaterLevel;
use crate::weapons::Element;
use crate::worldgen::{Land, World, is_passable, material::*};

/// Everything drawn and flown here, the avians then the gargoyles
pub const KINDS: [Look; 14] = [
    Look::Harpy,
    Look::Skyhunter,
    Look::Stormtalon,
    Look::Harridan,
    Look::Roc,
    Look::Thunderbird,
    Look::HarpyQueen,
    Look::Gargoyle,
    Look::BloodGargoyle,
    Look::RimeGargoyle,
    Look::CinderGargoyle,
    Look::MossGargoyle,
    Look::Grotesque,
    Look::BloodwakeGargoyle,
];

/// The sky isles' three raid bosses, in the order of their lairs: the roc's eyrie, the thunder
/// peak and the harpies' court
pub const SKY_RAID_BOSSES: [Look; 3] = [Look::Roc, Look::Thunderbird, Look::HarpyQueen];

/// Drawn and flown here, see the top of this file
pub fn is_winged(look: Look) -> bool {
    KINDS.contains(&look)
}

/// The monsters among them, going about in flocks and perched in twos and threes
pub fn is_monster(look: Look) -> bool {
    matches!(look, Look::Harpy | Look::Skyhunter) || gargoyles::is_lesser(look)
}

/// The raid bosses among them, fought in three phases
pub fn is_raid(look: Look) -> bool {
    SKY_RAID_BOSSES.contains(&look) || look == Look::BloodwakeGargoyle
}

fn kind(look: Look) -> usize {
    KINDS.iter().position(|&k| k == look).unwrap_or(0)
}

pub fn name(look: Look) -> &'static str {
    match look {
        Look::Harpy => "harpy",
        Look::Skyhunter => "skyhunter",
        Look::Stormtalon => "Stormtalon",
        Look::Harridan => "Harridan",
        Look::Roc => "Cloudreaver Roc",
        Look::Thunderbird => "Thunderbird",
        Look::HarpyQueen => "Harpy Queen",
        _ => gargoyles::name(look),
    }
}

/// What it's said of each, for the lore
#[allow(dead_code)]
pub fn lore(look: Look) -> &'static str {
    match look {
        Look::Harpy => "Half a woman and half a bird of prey, and all of her hungry. They roost along the sky isles' edges and come down screaming on whatever walks out onto them.",
        Look::Skyhunter => "A hawk of the high air, broad as a door across the wings. It circles where the isles thin out, and stoops faster than an arrow flies.",
        Look::Stormtalon => "An eagle that nests in the thunderheads. Lightning runs in its feathers, and it spits it from its beak.",
        Look::Harridan => "The eldest of a harpy brood, grey and ragged and cunning, who keeps the young ones fed. Her shriek brings them all.",
        Look::Roc => "Its shadow passes over an island like a cloud. It takes oxen from the fields below, and men, and drops them from the height of the isles to break them.",
        Look::Thunderbird => "Every storm over the isles is its wingbeat. Where it looks, the lightning falls.",
        Look::HarpyQueen => "She holds court on the highest isle, crowned in the gold of the travellers her daughters brought down. She throws her feathers like knives and her gales throw the rest off the edge.",
        _ => gargoyles::lore(look),
    }
}

/// How much damage it takes to kill one
pub fn health(look: Look) -> f64 {
    match look {
        Look::Harpy => 175.0,
        Look::Skyhunter => 150.0,
        Look::Stormtalon => 2350.0,
        Look::Harridan => 2350.0,
        Look::Roc => 14500.0,
        Look::Thunderbird => 13500.0,
        Look::HarpyQueen => 14500.0,
        _ => gargoyles::health(look),
    }
}

/// What its attacks are made of
pub fn element(look: Look) -> Option<Element> {
    match look {
        Look::Stormtalon | Look::Thunderbird => Some(Element::Storm),
        _ => gargoyles::element(look),
    }
}

/// The monsters it shrieks down to it, if it does
pub fn brood(look: Look) -> Option<Look> {
    match look {
        Look::Harridan | Look::HarpyQueen => Some(Look::Harpy),
        Look::Roc | Look::Thunderbird => Some(Look::Skyhunter),
        Look::Grotesque => Some(Look::Gargoyle),
        Look::BloodwakeGargoyle => Some(Look::BloodGargoyle),
        _ => None,
    }
}

/// How much bigger than its smallest kind it is
pub fn size(look: Look) -> f64 {
    let sp = spec(look);
    sp.s / base_unit(sp.plan)
}

/// How fast it walks and runs on the ground, in meters a second
pub fn speeds(look: Look) -> (f64, f64) {
    let k = size(look);
    match spec(look).plan {
        Plan::Gargoyle => (1.4 * k.sqrt(), 5.2 * k.powf(0.35)),
        _ => (1.1 * k.sqrt(), 3.2 * k.powf(0.35)),
    }
}

/// Meters covered by one stride of its walk
pub fn stride(look: Look) -> f64 {
    let sp = spec(look);
    let (lt, ls) = leg_lengths(&sp);
    (lt + ls) * 1.4 * sp.s
}

/// The body to aim at, as Look::body(): how high its middle is over its feet standing, and how far
/// round each piece of it a blow lands, in meters. Where on it that is follows what the player's
/// looking at, see Flier::aim().
pub fn body(look: Look) -> (f64, f64) {
    let sp = spec(look);
    (stand_height(&sp), 0.3 * sp.s * sp.girth)
}

/// How far its pieces reach from the middle of it, in meters, wings spread
pub fn reach(look: Look) -> f64 {
    let sp = spec(look);
    let (wi, wo) = wing_lengths(&sp);
    sp.s * (0.4 + wi + wo + 0.3)
}

// How each is built

/// The body it's built on
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Plan {
    /// A bird of prey: a long body held upright perched, a hooked beak, broad wings, a tail fan
    Bird,
    /// A bird-woman: a woman's head and shoulders, wings for arms, feathered from the breast down,
    /// a bird's legs and talons
    Harpy,
    /// A fiend of stone: hunched, horned, arms long and clawed, bat's wings on its back, a tail
    Gargoyle,
}

/// Meters to a unit of the smallest of each plan, for how much bigger the rest are
fn base_unit(plan: Plan) -> f64 {
    match plan {
        Plan::Bird => 0.75,
        Plan::Harpy => 0.66,
        Plan::Gargoyle => 0.9,
    }
}

/// What's on its head and back
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Crown {
    Plain,
    /// A ruff of gold round the nape, the roc's
    Mantle,
    /// A crest of feathers swept back, lightning in their tips, the thunderbird's and stormtalon's
    Crest,
    /// A crown of gold set with a crystal, and long dark hair: the queen's
    Regal,
    /// Wild grey hair, and ragged feathers: the harridan's
    Hag,
    /// Two horns curving up and back, the gargoyles'
    Horns,
    /// Great horns curling round like a ram's, and a frill of stone: the grotesque's
    Ram,
    /// A crown of thorns of stone, and spines down its back: the bloodwake's
    Thorned,
}

/// Whether it has arms of its own besides its wings: one piece each, or upper arm and forearm
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Arms {
    None,
    Stiff,
    Jointed,
}

/// What it's made of: its feathers or stone, what's darker, its breast or belly, a harpy's skin
/// or its hair, its beak, talons and claws, its eyes, what glows in it and one thing more
#[derive(Clone, Copy)]
pub(crate) struct Hide {
    pub main: u8,
    pub dark: u8,
    pub pale: u8,
    pub skin: u8,
    pub beak: u8,
    pub eye: u8,
    pub glow: u8,
    pub extra: u8,
}

/// How it's built, in units of its plan's
#[derive(Clone, Copy)]
pub(crate) struct Spec {
    /// Meters to a unit
    pub s: f64,
    pub plan: Plan,
    pub hide: Hide,
    /// A lower jaw or beak of its own, that opens to shriek and bite
    pub jaw: bool,
    /// A tail of its own that moves, rather than part of its body
    pub tail: bool,
    /// Its legs each a single piece, bent as they stand
    pub stiff: bool,
    pub arms: Arms,
    /// How long in the wing and how bulky, next to its plan's
    pub span: f64,
    pub girth: f64,
    pub crown: Crown,
}

pub(crate) fn hide(main: u8, dark: u8, pale: u8, skin: u8, beak: u8, eye: u8, glow: u8, extra: u8) -> Hide {
    Hide { main, dark, pale, skin, beak, eye, glow, extra }
}

pub(crate) fn spec(look: Look) -> Spec {
    let small = |plan: Plan, s: f64, hide: Hide, crown: Crown| Spec { s, plan, hide, jaw: false, tail: false, stiff: true, arms: Arms::None, span: 1.0, girth: 1.0, crown };
    let great = |plan: Plan, s: f64, hide: Hide, crown: Crown, span: f64, girth: f64| Spec { s, plan, hide, jaw: true, tail: true, stiff: false, arms: Arms::None, span, girth, crown };
    match look {
        // Brown and buff, her face pale under dark hair, eyes red, talons black
        Look::Harpy => small(Plan::Harpy, 0.66, hide(FUR_BROWN, FUR_DARK, FUR_CREAM, FUR_TAN, OBSIDIAN, GLOW_BLOOD, FUR_GINGER, FUR_DARK), Crown::Plain),
        // A hawk: brown back, a cream breast barred darker, a yellow beak tipped black
        Look::Skyhunter => small(Plan::Bird, 0.75, hide(FUR_BROWN, FUR_DARK, FUR_CREAM, FUR_GINGER, GOLD, GOLD, FUR_GINGER, FUR_WHITE), Crown::Plain),
        // Slate and steel, a white head, lightning in its feathers
        Look::Stormtalon => great(Plan::Bird, 1.6, hide(DARK_STEEL, OBSIDIAN, STEEL, FUR_WHITE, GOLD, GLOW_STORM, GLOW_STORM, FUR_WHITE), Crown::Crest, 1.05, 1.0),
        // Grey and ragged, her hair wild and white, eyes burning
        Look::Harridan => Spec { tail: true, ..great(Plan::Harpy, 1.25, hide(FUR_GRAY, FUR_DARK, LINEN, FUR_TAN, OBSIDIAN, GLOW_FIRE, FUR_GINGER, FUR_WHITE), Crown::Hag, 1.05, 1.0) },
        // A great golden eagle the size of a house: dark brown, a gold mantle, a pale beak
        Look::Roc => great(Plan::Bird, 4.6, hide(FUR_BROWN, FUR_DARK, FUR_TAN, FUR_GINGER, BONE, GOLD, FUR_GINGER, GOLD), Crown::Mantle, 1.12, 1.1),
        // Black and teal, banded in red and gold, lightning running in its feathers
        Look::Thunderbird => great(Plan::Bird, 3.9, hide(OBSIDIAN, DARK_STEEL, TEMPLE_TEAL, TEMPLE_RED, GOLD, GLOW_STORM, GLOW_STORM, GOLD), Crown::Crest, 1.15, 1.05),
        // Pale as bone under black hair, crowned, her feathers black and crimson tipped with gold
        Look::HarpyQueen => great(Plan::Harpy, 2.2, hide(FUR_DARK, TEMPLE_RED, GOLD, LINEN, DARK_STEEL, GLOW_ARCANE, GOLD, FUR_DARK), Crown::Regal, 1.1, 1.05),
        _ => gargoyles::spec(look),
    }
}

type V3 = Vector3<f64>;
type Vec3 = [f64; 3];

// Where the joints are on the torso, in units from its middle, facing +x along its spine with +y
// to its left and +z its back: the top of its neck, the root of its tail, the left wing's
// shoulder, the left hip and, for the gargoyles, the left arm's shoulder

pub(crate) fn head_joint(sp: &Spec) -> V3 {
    match sp.plan {
        Plan::Bird => V3::new(0.5, 0.0, 0.14),
        Plan::Harpy => V3::new(0.56, 0.0, 0.03),
        Plan::Gargoyle => V3::new(0.5, 0.0, 0.12),
    }
}

pub(crate) fn tail_root(sp: &Spec) -> V3 {
    match sp.plan {
        Plan::Bird => V3::new(-0.46, 0.0, 0.03),
        Plan::Harpy => V3::new(-0.54, 0.0, -0.02),
        Plan::Gargoyle => V3::new(-0.42, 0.0, -0.06),
    }
}

pub(crate) fn shoulder(sp: &Spec) -> V3 {
    match sp.plan {
        Plan::Bird => V3::new(0.2, 0.17 * sp.girth, 0.15),
        Plan::Harpy => V3::new(0.4, 0.19 * sp.girth, 0.04),
        Plan::Gargoyle => V3::new(0.22, 0.12 * sp.girth, 0.24),
    }
}

pub(crate) fn hip(sp: &Spec) -> V3 {
    match sp.plan {
        Plan::Bird => V3::new(-0.12, 0.11 * sp.girth, -0.17),
        Plan::Harpy => V3::new(-0.44, 0.11 * sp.girth, -0.04),
        Plan::Gargoyle => V3::new(-0.38, 0.15 * sp.girth, -0.08),
    }
}

pub(crate) fn arm_root(sp: &Spec) -> V3 {
    V3::new(0.36, 0.25 * sp.girth, 0.02)
}

/// How long its legs are, thigh and shin, and its arms, upper arm and forearm, in units
pub(crate) fn leg_lengths(sp: &Spec) -> (f64, f64) {
    match sp.plan {
        Plan::Bird => (0.24, 0.3),
        Plan::Harpy => (0.5, 0.56),
        Plan::Gargoyle => (0.38, 0.44),
    }
}

pub(crate) fn arm_lengths(_sp: &Spec) -> (f64, f64) {
    (0.34, 0.38)
}

/// How long a wing's inner and outer pieces are, in units
pub(crate) fn wing_lengths(sp: &Spec) -> (f64, f64) {
    let (i, o) = match sp.plan {
        Plan::Bird => (0.78, 1.08),
        Plan::Harpy => (0.72, 0.98),
        Plan::Gargoyle => (0.62, 0.98),
    };
    (i * sp.span, o * sp.span)
}

/// Where the wrist is on the inner piece of the left wing
pub(crate) fn wrist(sp: &Spec) -> V3 {
    V3::new(0.06, wing_lengths(sp).0, 0.02)
}

/// Where the lower jaw's hinged on the head, and its mouth, in units of the head's own frame
pub(crate) fn jaw_hinge(sp: &Spec) -> Vec3 {
    match sp.plan {
        Plan::Bird => [0.12 * BIRD_HEAD, 0.0, -0.04 * BIRD_HEAD],
        Plan::Harpy => [0.08 * HARPY_HEAD, 0.0, -0.02 * HARPY_HEAD],
        Plan::Gargoyle => [0.12, 0.0, -0.07],
    }
}

/// How much bigger a harpy's head is built than its shapes say, a woman's head on a body built small,
/// and a bird's, so its glare and its beak can be made out
const HARPY_HEAD: f64 = 1.6;
const BIRD_HEAD: f64 = 1.3;

fn mouth_of(sp: &Spec) -> Vec3 {
    match sp.plan {
        Plan::Bird => [0.28 * BIRD_HEAD, 0.0, -0.04 * BIRD_HEAD],
        Plan::Harpy => [0.15 * HARPY_HEAD, 0.0, 0.0],
        Plan::Gargoyle => [0.3, 0.0, -0.05],
    }
}

/// How far its body's tipped back standing, nose up, in radians
pub(crate) fn stand_pitch(sp: &Spec) -> f64 {
    match sp.plan {
        Plan::Bird => 0.72,
        Plan::Harpy => 1.28,
        Plan::Gargoyle => 0.75,
    }
}

/// Its legs standing, as the torso's tipped back: the thigh forward a little and the shin back
pub(crate) fn stand_leg(sp: &Spec) -> LegPose {
    let p = stand_pitch(sp);
    match sp.plan {
        Plan::Bird => LegPose { swing: -p + 0.42, knee: 1.15 },
        Plan::Harpy => LegPose { swing: -p + 0.42, knee: 0.95 },
        Plan::Gargoyle => LegPose { swing: -p + 0.95, knee: 1.85 },
    }
}

/// A gargoyle's arms standing, hanging down a little forward, the elbows bent
pub(crate) fn stand_arm(sp: &Spec) -> ArmPose {
    ArmPose { swing: -stand_pitch(sp) + 0.3, spread: 0.18, elbow: 0.6 }
}

/// How high the middle of its body stands over its feet, in meters, as it stands
pub(crate) fn stand_height(sp: &Spec) -> f64 {
    let mut bones = [Bone::default(); BONES];
    let rig = Rig::standing(sp);
    pose(sp, V3::zeros(), 0.0, &rig, 1.0, &mut bones);
    -feet_of(sp, &bones, 1.0).iter().map(|f| f.z).fold(f64::MAX, f64::min)
}

/// The pieces each kind is made of, its models in this order, see model_of(): torso, head, lower
/// jaw, tail, the left wing's inner and outer and the right's from 4 on, a thigh, a shin, an
/// upper arm, a forearm, and a gargoyle's head with its eyes gone to stone, as a statue's
const PIECES: usize = 13;
pub(crate) const M_TORSO: usize = 0;
pub(crate) const M_HEAD: usize = 1;
pub(crate) const M_JAW: usize = 2;
pub(crate) const M_TAIL: usize = 3;
pub(crate) const M_THIGH: usize = 8;
pub(crate) const M_SHIN: usize = 9;
pub(crate) const M_UPPER: usize = 10;
pub(crate) const M_FORE: usize = 11;
pub(crate) const M_STONE_HEAD: usize = 12;

/// What's drawn of one, each piece where it is: torso, head, jaw, tail, the left wing's inner and
/// outer, the right's, the thighs left and right, the shins, the upper arms and the forearms
const BONES: usize = 16;
const TORSO: usize = 0;
const HEAD: usize = 1;
const JAW: usize = 2;
const TAIL: usize = 3;
const WING: usize = 4;
const THIGH: usize = 8;
const SHIN: usize = 10;
const UPPER: usize = 12;
const FORE: usize = 14;

/// Which of its models bone `b` is drawn with, left and right limbs alike
fn model_of(b: usize) -> usize {
    match b {
        _ if b < THIGH => b,
        _ if b < SHIN => M_THIGH,
        _ if b < UPPER => M_SHIN,
        _ if b < FORE => M_UPPER,
        _ => M_FORE,
    }
}

/// What they throw, call down and break into, after their pieces among their models: feathers of
/// three kinds, a stretch of a lightning bolt and a fork off it, and shards of four stones
pub const FX_FEATHER: usize = 0;
pub const FX_BOLT: usize = 3;
pub const FX_FORK: usize = 4;
pub const FX_SHARD: usize = 5;
const FX: usize = 9;

/// How many models they have, after the dragons', see animals::WINGED_MODELS_AT
pub const MODELS: usize = KINDS.len() * PIECES + FX;

/// The model of one of the effects among all models
pub fn fx_model(fx: usize) -> u32 {
    (animals::WINGED_MODELS_AT + KINDS.len() * PIECES + fx) as u32
}

/// Where each of the effects' models has the middle of its bottom, in its own meters, and how big
/// it is in voxels
pub fn fx_shape(fx: usize) -> (V3, [u32; 3]) {
    layout()[KINDS.len() * PIECES + fx]
}

/// Which feathers each throws, and the shards of which stone each breaks into
pub fn feather_of(look: Look) -> usize {
    match look {
        Look::HarpyQueen => FX_FEATHER + 1,
        Look::Stormtalon | Look::Thunderbird => FX_FEATHER + 2,
        _ => FX_FEATHER,
    }
}

pub fn shard_of(look: Look) -> usize {
    FX_SHARD
        + match look {
            Look::BloodGargoyle | Look::BloodwakeGargoyle => 1,
            Look::RimeGargoyle => 2,
            Look::CinderGargoyle => 3,
            _ => 0,
        }
}

// The pieces of the avians, each built in its own frame with its joint at the middle, in units.
// The right wing's mirrored from the left's by `side`.

/// A fan of tail feathers spreading back from `root`
fn tail_fan(p: &mut Vec<Part>, sp: &Spec, root: Vec3) {
    let h = sp.hide;
    let (long, spread, n) = match sp.plan {
        Plan::Bird => (0.62, 0.22, 7),
        _ => (0.5, 0.2, 7),
    };
    for k in 0..n {
        let f = k as f64 / (n - 1) as f64 - 0.5;
        let tip = [root[0] - long * (1.0 - 0.12 * f.abs()), f * 2.0 * spread, root[2] - 0.02];
        let mid = [root[0] - long * 0.8, tip[1] * 0.85, tip[2]];
        // Banded darker at the end, and a crest's light in the tips
        p.push(limb(mid, tip, 0.04, 0.03, if sp.crown == Crown::Crest { h.glow } else { h.dark }).stretched([1.0, 1.4, 0.55]));
        p.push(limb(root, mid, 0.05, 0.04, h.main).stretched([1.0, 1.4, 0.55]));
    }
}

fn bird_torso(p: &mut Vec<Part>, sp: &Spec) {
    let (h, g) = (sp.hide, sp.girth);
    if sp.crown == Crown::Mantle {
        // The gold ruff round its nape and shoulders
        p.push(ellipsoid([0.38, 0.0, 0.12], [0.18, 0.2 * g, 0.16], h.extra));
    }
    if sp.crown == Crown::Crest {
        // Lightning running in seams down its flanks and back
        for side in [-1.0, 1.0] {
            p.push(limb([0.3, side * 0.2 * g, 0.12], [-0.35, side * 0.17 * g, 0.12], 0.02, 0.015, h.glow));
        }
        p.push(limb([0.35, 0.0, 0.26], [-0.4, 0.0, 0.22], 0.025, 0.02, h.extra));
    }
    // Bars across its breast
    for k in 0..4 {
        let x = 0.36 - k as f64 * 0.1;
        p.push(limb([x, -0.17 * g, -0.2 + k as f64 * 0.02], [x, 0.17 * g, -0.2 + k as f64 * 0.02], 0.016, 0.016, h.dark).stretched([1.0, 1.0, 0.7]));
    }
    // Where the wings join it, and the feathered tops of its legs
    for side in [-1.0, 1.0] {
        p.push(ball([0.2, side * 0.16 * g, 0.13], 0.1 * g, h.dark));
    }
    // Its neck up to its head
    p.push(limb([0.26, 0.0, 0.05], [0.5, 0.0, 0.13], 0.17 * g, 0.12 * g, h.main));
    // Breast, body and rump
    p.push(ellipsoid([0.24, 0.0, -0.06], [0.3, 0.22 * g, 0.24 * g], h.pale));
    p.push(ellipsoid([0.0, 0.0, 0.02], [0.5, 0.25 * g, 0.25 * g], h.main));
    p.push(ellipsoid([-0.32, 0.0, 0.0], [0.24, 0.17 * g, 0.16 * g], h.main));
    if !sp.tail {
        tail_fan(p, sp, [-0.42, 0.0, 0.03]);
    }
}

fn bird_head(p: &mut Vec<Part>, sp: &Spec) {
    let h = sp.hide;
    for side in [-1.0, 1.0] {
        p.push(ball([0.1, side * 0.085, 0.05], 0.032, h.eye));
        // The ridge over each eye that gives it its glare
        p.push(limb([0.04, side * 0.08, 0.1], [0.15, side * 0.07, 0.08], 0.028, 0.02, h.dark));
    }
    match sp.crown {
        Crown::Crest => {
            // Feathers swept back off its crown, lightning at their tips
            for k in 0..4 {
                let y = (k as f64 - 1.5) * 0.04;
                p.push(limb([-0.12 - 0.04 * k as f64, y * 1.6, 0.24 - 0.02 * k as f64], [-0.2 - 0.05 * k as f64, y * 1.8, 0.28 - 0.03 * k as f64], 0.02, 0.01, h.glow));
                p.push(limb([0.02, y, 0.1], [-0.12 - 0.04 * k as f64, y * 1.6, 0.24 - 0.02 * k as f64], 0.035, 0.02, h.dark));
            }
        }
        Crown::Mantle => {
            p.push(ellipsoid([-0.05, 0.0, -0.01], [0.12, 0.13, 0.12], h.extra));
        }
        _ => {}
    }
    // The hooked upper beak, the cere at its root, and the lower beak if it has none of its own
    p.push(limb([0.27, 0.0, -0.03], [0.3, 0.0, -0.1], 0.03, 0.01, OBSIDIAN));
    p.push(limb([0.15, 0.0, 0.0], [0.27, 0.0, -0.03], 0.055, 0.03, h.beak));
    p.push(ball([0.14, 0.0, 0.01], 0.055, h.skin));
    if !sp.jaw {
        p.push(limb([0.12, 0.0, -0.05], [0.23, 0.0, -0.07], 0.035, 0.015, h.beak));
    }
    // A white head on the stormtalon
    p.push(ellipsoid([0.04, 0.0, 0.03], [0.13, 0.1, 0.1], if sp.crown == Crown::Crest && sp.s < 2.0 { h.extra } else { h.main }));
}

fn bird_jaw(p: &mut Vec<Part>, sp: &Spec) {
    let h = sp.hide;
    p.push(ellipsoid([0.06, 0.0, 0.01], [0.06, 0.03, 0.012], FUR_RED));
    p.push(limb([0.0, 0.0, 0.0], [0.13, 0.0, -0.03], 0.037, 0.012, h.beak));
}

fn harpy_torso(p: &mut Vec<Part>, sp: &Spec) {
    let (h, g) = (sp.hide, sp.girth);
    if sp.crown == Crown::Regal {
        // A collar of gold
        p.push(limb([0.46, -0.11 * g, 0.0], [0.46, 0.11 * g, 0.0], 0.04, 0.04, h.pale).stretched([1.0, 1.0, 0.8]));
    }
    // Shoulder caps of feathers where her wings join her
    for side in [-1.0, 1.0] {
        p.push(ball([0.4, side * 0.17 * g, 0.03], 0.075 * g, h.main));
    }
    // Her neck, collarbones and shoulders, bare
    p.push(limb([0.38, 0.0, 0.0], [0.57, 0.0, 0.03], 0.06 * g, 0.05 * g, h.skin));
    p.push(ellipsoid([0.38, 0.0, 0.01], [0.09, 0.19 * g, 0.08 * g], h.skin));
    // Feathered from the breast down, pale in front and darker over the hips
    p.push(ellipsoid([0.24, 0.0, -0.035], [0.17, 0.16 * g, 0.11 * g], h.pale));
    p.push(ellipsoid([0.2, 0.0, 0.03], [0.22, 0.17 * g, 0.08 * g], h.skin));
    p.push(ellipsoid([-0.06, 0.0, -0.01], [0.26, 0.14 * g, 0.11 * g], h.main));
    p.push(ellipsoid([-0.36, 0.0, -0.02], [0.17, 0.17 * g, 0.12 * g], h.dark));
    if !sp.tail {
        tail_fan(p, sp, [-0.5, 0.0, -0.02]);
    }
}

fn harpy_head(p: &mut Vec<Part>, sp: &Spec) {
    let h = sp.hide;
    // Burning eyes, a sharp nose and fangs, then her face, and her hair over the back of it
    for side in [-1.0, 1.0] {
        p.push(ball([0.12, side * 0.04, 0.07], 0.018, h.eye));
        p.push(limb([0.12, side * 0.02, 0.0], [0.125, side * 0.02, -0.025], 0.009, 0.004, BONE));
    }
    p.push(limb([0.12, 0.0, 0.07], [0.145, 0.0, 0.035], 0.02, 0.012, h.skin));
    match sp.crown {
        Crown::Regal => {
            // A crown of gold spikes, a crystal at its front
            p.push(ball([0.09, 0.0, 0.2], 0.03, CRYSTAL));
            for k in 0..7 {
                let a = (k as f64 / 6.0 - 0.5) * PI * 1.1;
                let (x, y) = (0.02 + 0.08 * a.cos(), 0.085 * a.sin());
                p.push(limb([x, y, 0.15], [x * 1.1, y * 1.1, 0.24 + 0.02 * (k % 2) as f64], 0.016, 0.006, h.pale));
            }
            p.push(limb([-0.06, -0.09, 0.15], [-0.06, 0.09, 0.15], 0.02, 0.02, h.pale));
            p.push(limb([0.09, -0.07, 0.15], [0.09, 0.07, 0.15], 0.022, 0.022, h.pale));
        }
        Crown::Hag => {
            // Wild hair standing out every way
            for k in 0..8 {
                let a = k as f64 / 8.0 * TAU;
                p.push(limb([-0.02, 0.0, 0.1], [-0.08 + 0.1 * a.cos(), 0.18 * a.sin(), 0.14 + 0.08 * a.cos()], 0.03, 0.012, h.extra));
            }
        }
        _ => {}
    }
    p.push(ellipsoid([0.06, 0.0, 0.05], [0.085, 0.075, 0.1], h.skin));
    // Her hair, long down her back, streaming behind her as she flies
    let hair = if sp.crown == Crown::Hag { h.extra } else { h.dark };
    for y in [-0.05, 0.0, 0.05] {
        p.push(limb([-0.03, y, 0.06], [-0.3, y * 1.6, -0.12], 0.045, 0.02, hair));
    }
    p.push(ellipsoid([0.0, 0.0, 0.08], [0.1, 0.09, 0.1], hair));
}

fn harpy_jaw(p: &mut Vec<Part>, sp: &Spec) {
    // Her jaw, dropping open as she shrieks
    let h = sp.hide;
    for side in [-1.0, 1.0] {
        p.push(limb([0.05, side * 0.02, 0.0], [0.05, side * 0.02, 0.02], 0.008, 0.004, BONE));
    }
    p.push(ellipsoid([0.035, 0.0, -0.005], [0.05, 0.055, 0.028], h.skin));
}

/// A feather from `a` to `b`, flat in the wing
fn feather(a: Vec3, b: Vec3, r: f64, material: u8) -> Part {
    limb(a, b, r, r * 0.6, material).stretched([1.45, 1.45, 0.55])
}

/// The inner piece of a feathered wing, from the shoulder out to the wrist: the bone of its arm
/// along the leading edge, the coverts over it, and the long secondaries back from it, banded
/// darker at their ends
fn feathered_in(p: &mut Vec<Part>, sp: &Spec, side: f64) {
    let h = sp.hide;
    let w = wrist(sp);
    let w = [w.x, side * w.y, w.z];
    let reach = w[1].abs();
    let chord = if sp.plan == Plan::Bird { 0.52 } else { 0.44 };
    if sp.plan == Plan::Harpy {
        // The claw of her hand at her wrist
        p.push(limb(w, [w[0] + 0.1, w[1] + side * 0.01, w[2] - 0.06], 0.022, 0.005, h.beak));
    }
    p.push(ball(w, 0.045, h.dark));
    p.push(limb([0.0, 0.0, 0.0], w, 0.06 * sp.girth, 0.04, if sp.plan == Plan::Harpy { h.skin } else { h.main }));
    let strips = 7;
    for i in 0..strips {
        let f = (i as f64 + 0.5) / strips as f64;
        let y = side * (0.04 + (reach - 0.04) * f);
        let lead = [0.04 * f, y, 0.02 * f];
        let c = chord * (1.0 - 0.12 * f);
        let tip = [lead[0] - c, y + side * 0.03, -0.01];
        let band = [lead[0] - c * 0.72, y + side * 0.02, -0.005];
        let r = (reach - 0.04) / strips as f64 * 0.55;
        let dark = if sp.crown == Crown::Crest && i % 2 == 0 { h.glow } else { h.dark };
        p.push(feather(band, tip, r, dark));
        p.push(feather(lead, band, r, h.main));
    }
    // The coverts along its leading edge
    p.push(ellipsoid([-0.06, side * reach * 0.5, 0.025], [0.13, reach * 0.5, 0.04], h.main));
}

/// The outer piece, from the wrist to the tip: the bones of its hand and the long primaries
/// fanned out from them, spread like fingers at their tips
fn feathered_out(p: &mut Vec<Part>, sp: &Spec, side: f64) {
    let h = sp.hide;
    let l = wing_lengths(sp).1;
    // Each primary: where on the hand it starts, and where its tip is, as shares of its length
    // and of how far back it goes
    let fan: [(f64, f64, f64); 8] = [(0.42, -0.08, 1.0), (0.4, -0.17, 0.97), (0.37, -0.26, 0.91), (0.34, -0.34, 0.84), (0.3, -0.4, 0.75), (0.25, -0.45, 0.64), (0.18, -0.48, 0.52), (0.1, -0.5, 0.4)];
    let r = 0.045 * sp.span.sqrt();
    for (k, &(base, back, out)) in fan.iter().enumerate() {
        let from = [-0.02, side * base * l, 0.01];
        let tip = [back * if sp.plan == Plan::Bird { 1.0 } else { 0.85 }, side * out * l, -0.01];
        let band = [from[0] + (tip[0] - from[0]) * 0.7, from[1] + (tip[1] - from[1]) * 0.7, 0.0];
        let dark = if sp.crown == Crown::Crest && k % 2 == 1 { h.glow } else { h.dark };
        p.push(feather(band, tip, r, dark));
        p.push(feather(from, band, r, h.main));
    }
    // The coverts over the hand, and its bones
    p.push(ellipsoid([-0.1, side * 0.22 * l, 0.02], [0.15, 0.22 * l, 0.035], h.main));
    p.push(limb([0.0, 0.0, 0.0], [-0.03, side * 0.44 * l, 0.01], 0.04, 0.025, h.dark));
}

/// A bird's or a harpy's thigh, feathered, and for a monster's stiff legs the rest of the leg
/// too, bent as it stands
fn feathered_thigh(p: &mut Vec<Part>, sp: &Spec) {
    let (h, g) = (sp.hide, sp.girth);
    let (lt, _) = leg_lengths(sp);
    if sp.stiff {
        let leg = stand_leg(sp);
        talons(p, sp, [0.0, 0.0, -lt], leg.knee, stand_pitch(sp) + leg.swing);
    }
    // Feathered "trousers" down to the knee
    p.push(ellipsoid([0.02, 0.0, -lt * 0.45], [0.1 * g, 0.08 * g, lt * 0.55], h.main));
    p.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, -lt], 0.08 * g, 0.06 * g, h.main));
}

/// A scaly shin from the knee at `knee`, bent back `bend` radians from the thigh, its foot of
/// three talons forward and one back flat on the ground for a thigh `tipped` radians forward of
/// straight down as it stands
fn talons(p: &mut Vec<Part>, sp: &Spec, knee: Vec3, bend: f64, tipped: f64) {
    let h = sp.hide;
    let (_, ls) = leg_lengths(sp);
    let ankle = [knee[0] - ls * bend.sin(), 0.0, knee[2] - ls * bend.cos()];
    // Flat along the ground: the way that's forward in the world, in the thigh's frame
    let a = tipped;
    let (c, s) = (a.cos(), a.sin());
    let at = |x: f64, y: f64, z: f64| [ankle[0] + x * c + z * s, y, ankle[2] - x * s + z * c];
    for y in [-0.05, 0.0, 0.05] {
        p.push(limb(at(0.1, y * 1.6, -0.01), at(0.15, y * 1.8, -0.05), 0.016, 0.005, OBSIDIAN));
        p.push(limb(at(0.0, y * 0.4, 0.0), at(0.11, y * 1.6, -0.015), 0.022, 0.018, h.skin));
    }
    p.push(limb(at(-0.01, 0.0, 0.0), at(-0.09, 0.0, -0.03), 0.016, 0.005, OBSIDIAN));
    p.push(ball(knee, 0.045, h.skin));
    p.push(limb(knee, ankle, 0.035 * sp.girth, 0.028 * sp.girth, h.skin));
}

fn feathered_shin(p: &mut Vec<Part>, sp: &Spec) {
    if !sp.stiff {
        let leg = stand_leg(sp);
        // Built straight down from the knee, the foot turned so it's flat as the leg stands
        talons(p, sp, [0.0, 0.0, 0.0], 0.0, stand_pitch(sp) + leg.swing - leg.knee);
    }
}

/// The shapes of piece `piece` of kind `k`, in meters
fn piece_parts(k: usize, piece: usize) -> Vec<Part> {
    let look = KINDS[k];
    let sp = spec(look);
    let mut p = Vec::new();
    if sp.plan == Plan::Gargoyle {
        gargoyles::piece(&mut p, &sp, piece);
    } else {
        let harpy = sp.plan == Plan::Harpy;
        match piece {
            M_TORSO if harpy => harpy_torso(&mut p, &sp),
            M_TORSO => bird_torso(&mut p, &sp),
            M_HEAD if harpy => {
                harpy_head(&mut p, &sp);
                p = p.iter().map(|part| part.scaled(HARPY_HEAD)).collect();
            }
            M_HEAD => {
                bird_head(&mut p, &sp);
                p = p.iter().map(|part| part.scaled(BIRD_HEAD)).collect();
            }
            M_JAW if sp.jaw && harpy => {
                harpy_jaw(&mut p, &sp);
                p = p.iter().map(|part| part.scaled(HARPY_HEAD)).collect();
            }
            M_JAW if sp.jaw => {
                bird_jaw(&mut p, &sp);
                p = p.iter().map(|part| part.scaled(BIRD_HEAD)).collect();
            }
            M_TAIL if sp.tail => tail_fan(&mut p, &sp, [0.0, 0.0, 0.0]),
            4 => feathered_in(&mut p, &sp, 1.0),
            5 => feathered_out(&mut p, &sp, 1.0),
            6 => feathered_in(&mut p, &sp, -1.0),
            7 => feathered_out(&mut p, &sp, -1.0),
            M_THIGH => feathered_thigh(&mut p, &sp),
            M_SHIN => feathered_shin(&mut p, &sp),
            _ => {}
        }
    }
    p.iter().map(|part| part.scaled(sp.s)).collect()
}

/// The shapes of effect `fx`, in meters: a feather half a meter long, a stretch of a bolt a meter
/// long, a fork off it, and a shard of stone a third of a meter
fn fx_parts(fx: usize) -> Vec<Part> {
    let mut p = Vec::new();
    match fx {
        _ if fx < FX_BOLT => {
            let (vane, tip, quill) = [(FUR_BROWN, FUR_DARK, BONE), (FUR_DARK, GOLD, TEMPLE_RED), (DARK_STEEL, GLOW_STORM, GLOW_HOLY)][fx];
            // Along +x, its point foremost, the quill behind
            p.push(limb([0.18, 0.0, 0.0], [0.27, 0.0, 0.0], 0.03, 0.006, tip));
            p.push(limb([-0.2, 0.0, 0.0], [0.18, 0.0, 0.0], 0.045, 0.03, vane).stretched([1.0, 1.7, 0.5]));
            p.push(limb([-0.27, 0.0, 0.0], [0.27, 0.0, 0.0], 0.012, 0.008, quill));
        }
        FX_BOLT => {
            // A meter of it along +x, a white core in a blue glow
            p.push(limb([-0.5, 0.0, 0.0], [0.5, 0.0, 0.0], 0.05, 0.05, GLOW_HOLY));
            p.push(limb([-0.5, 0.0, 0.0], [0.5, 0.0, 0.0], 0.11, 0.11, GLOW_STORM));
        }
        FX_FORK => {
            p.push(limb([0.0, 0.0, 0.0], [0.35, 0.12, 0.0], 0.04, 0.015, GLOW_HOLY));
            p.push(limb([0.35, 0.12, 0.0], [0.6, 0.05, 0.0], 0.03, 0.01, GLOW_STORM));
            p.push(limb([0.0, 0.0, 0.0], [0.35, 0.12, 0.0], 0.07, 0.03, GLOW_STORM));
        }
        _ => {
            let (stone, dark) = [(STONE, ROCK), (TEMPLE_RED, GLOW_BLOOD), (LIMESTONE, ICE), (OBSIDIAN, GLOW_FIRE)][(fx - FX_SHARD).min(3)];
            // A jagged splinter, its point foremost
            p.push(limb([0.05, 0.0, 0.0], [0.2, 0.02, 0.01], 0.06, 0.012, dark));
            p.push(limb([-0.15, 0.0, 0.0], [0.08, 0.0, 0.0], 0.08, 0.06, stone).stretched([1.0, 1.2, 0.8]));
        }
    }
    p
}

/// Every kind's pieces and every effect's model, voxelized
fn build_models() -> Vec<(Model, V3)> {
    let jobs = KINDS.len() * PIECES + FX;
    let next = std::sync::atomic::AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 8);
    let mut made: Vec<(usize, (Model, V3))> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let job = next.fetch_add(1, Ordering::Relaxed);
                        if job >= jobs {
                            break out;
                        }
                        let parts = if job < KINDS.len() * PIECES { piece_parts(job / PIECES, job % PIECES) } else { fx_parts(job - KINDS.len() * PIECES) };
                        let made = if parts.is_empty() { (Model { size: [1, 1, 1], voxels: vec![AIR] }, V3::zeros()) } else { voxelize(&parts, MODEL_RES) };
                        out.push((job, made));
                    }
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("making a winged creature's model failed")).collect()
    });
    made.sort_unstable_by_key(|(job, _)| *job);
    let mut made: Vec<(Model, V3)> = made.into_iter().map(|(_, m)| m).collect();
    // Pieces drawn from files in assets\models, see src\assets.rs
    crate::assets::avians(&mut made);
    made
}

static LAYOUT: OnceLock<Vec<(V3, [u32; 3])>> = OnceLock::new();

/// Where each model has the middle of its bottom, in its own meters, and how big it is in voxels
fn layout() -> &'static [(V3, [u32; 3])] {
    LAYOUT.get_or_init(|| build_models().into_iter().map(|(m, anchor)| (anchor, m.size)).collect())
}

/// Their models, to go after the dragons' among all of them, see animals::animal_models()
pub fn models() -> Vec<Model> {
    let made = build_models();
    let _ = LAYOUT.set(made.iter().map(|(m, anchor)| (*anchor, m.size)).collect());
    made.into_iter().map(|(m, _)| m).collect()
}

// The rig: how each joint's turned, eased from where it is towards where what it's doing wants it

/// A wing's joints: how far it's raised at the shoulder, swept back, its leading edge tipped down,
/// how far its hand's folded back in the wing at the wrist, and bent down there
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct WingPose {
    pub flap: f64,
    pub sweep: f64,
    pub twist: f64,
    pub fold: f64,
    pub bend: f64,
}

/// A leg's joints: how far forward the thigh's swung from the body, and how far back the knee bends
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct LegPose {
    pub swing: f64,
    pub knee: f64,
}

/// An arm's: how far forward it's swung, out to the side, and how far forward the elbow bends
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct ArmPose {
    pub swing: f64,
    pub spread: f64,
    pub elbow: f64,
}

/// Every joint, in radians: its body's pitch, nose up, and roll, left wing up; its head turned to
/// its left and raised, its jaw opened, its tail the same; its wings, legs and arms, left then
/// right
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Rig {
    pub pitch: f64,
    pub roll: f64,
    pub head: [f64; 2],
    pub jaw: f64,
    pub tail: [f64; 2],
    pub wings: [WingPose; 2],
    pub legs: [LegPose; 2],
    pub arms: [ArmPose; 2],
}

/// Wings folded at its sides: the arm swept back along the body, the wing hanging down the flank
/// from it, the hand folded back along it
pub(crate) fn folded(sp: &Spec) -> WingPose {
    match sp.plan {
        Plan::Gargoyle => WingPose { flap: 0.35, sweep: 1.25, twist: -1.2, fold: 2.45, bend: 0.15 },
        Plan::Harpy => WingPose { flap: -0.15, sweep: 1.1, twist: -1.35, fold: 2.6, bend: 0.0 },
        Plan::Bird => WingPose { flap: 0.12, sweep: 1.4, twist: -1.4, fold: 2.65, bend: 0.0 },
    }
}

/// Wings spread, gliding, a little raised
pub(crate) const SPREAD: WingPose = WingPose { flap: 0.1, sweep: 0.04, twist: 0.0, fold: 0.04, bend: 0.0 };

impl Rig {
    /// Standing on the ground, perched, as when nothing's the matter
    pub(crate) fn standing(sp: &Spec) -> Rig {
        let p = stand_pitch(sp);
        let mut r = Rig { pitch: p, ..Default::default() };
        r.head = [0.0, -p + if sp.plan == Plan::Bird { 0.1 } else { 0.0 }];
        r.tail = [0.0, match sp.plan {
            Plan::Bird => -0.35,
            Plan::Harpy => -0.15,
            Plan::Gargoyle => -0.45,
        }];
        r.wings = [folded(sp); 2];
        r.legs = [stand_leg(sp); 2];
        r.arms = [stand_arm(sp); 2];
        r
    }

    /// Flying: level, head out ahead, tail straight behind, legs tucked up
    pub(crate) fn flying(sp: &Spec) -> Rig {
        let mut r = Rig { pitch: 0.0, ..Default::default() };
        r.head = [0.0, if sp.plan == Plan::Bird { -0.12 } else { 0.0 }];
        r.tail = [0.0, 0.04];
        r.wings = [SPREAD; 2];
        let leg = match sp.plan {
            Plan::Bird => LegPose { swing: -1.3, knee: 1.5 },
            Plan::Harpy => LegPose { swing: -1.0, knee: 1.2 },
            Plan::Gargoyle => LegPose { swing: -1.15, knee: 0.7 },
        };
        r.legs = [leg; 2];
        r.arms = [ArmPose { swing: -0.4, spread: 0.25, elbow: 1.4 }; 2];
        r
    }

    /// Eased towards `to`, as `rate` says, about the fraction of the way a second
    fn ease(&mut self, to: &Rig, rate: f64, dt: f64) {
        let k = 1.0 - (-rate * dt).exp();
        let e = |a: &mut f64, b: f64| *a += (b - *a) * k;
        e(&mut self.pitch, to.pitch);
        e(&mut self.roll, to.roll);
        for j in 0..2 {
            e(&mut self.head[j], to.head[j]);
            e(&mut self.tail[j], to.tail[j]);
        }
        e(&mut self.jaw, to.jaw);
        for s in 0..2 {
            let (a, b) = (&mut self.wings[s], to.wings[s]);
            e(&mut a.flap, b.flap);
            e(&mut a.sweep, b.sweep);
            e(&mut a.twist, b.twist);
            e(&mut a.fold, b.fold);
            e(&mut a.bend, b.bend);
            e(&mut self.legs[s].swing, to.legs[s].swing);
            e(&mut self.legs[s].knee, to.legs[s].knee);
            e(&mut self.arms[s].swing, to.arms[s].swing);
            e(&mut self.arms[s].spread, to.arms[s].spread);
            e(&mut self.arms[s].elbow, to.arms[s].elbow);
        }
    }
}

/// Where a piece's joint is, in voxels, and how it's turned, and whether it's drawn
#[derive(Clone, Copy)]
pub(crate) struct Bone {
    at: V3,
    turn: UnitQuaternion<f64>,
    shown: bool,
}

impl Default for Bone {
    fn default() -> Self {
        Bone { at: V3::zeros(), turn: UnitQuaternion::identity(), shown: false }
    }
}

fn rx(a: f64) -> UnitQuaternion<f64> {
    UnitQuaternion::from_axis_angle(&Vector3::x_axis(), a)
}

fn ry(a: f64) -> UnitQuaternion<f64> {
    UnitQuaternion::from_axis_angle(&Vector3::y_axis(), a)
}

fn rz(a: f64) -> UnitQuaternion<f64> {
    UnitQuaternion::from_axis_angle(&Vector3::z_axis(), a)
}

/// A turning seen in a mirror across the body's middle, for the right side from the left's
fn mirrored(q: UnitQuaternion<f64>) -> UnitQuaternion<f64> {
    let c = q.coords;
    UnitQuaternion::from_quaternion(Quaternion::new(c.w, -c.x, c.y, -c.z))
}

fn flip(v: V3) -> V3 {
    V3::new(v.x, -v.y, v.z)
}

/// Where every piece of one built as `sp` is, its body's middle at `center` in voxels facing
/// `yaw`, its joints as `rig` has them: and where its mouth is
fn pose(sp: &Spec, center: V3, yaw: f64, rig: &Rig, vpm: f64, bones: &mut [Bone; BONES]) -> V3 {
    let m = sp.s * vpm;
    let body = rz(yaw) * ry(-rig.pitch) * rx(rig.roll);
    let world = |local: V3| center + body * local * m;
    bones[TORSO] = Bone { at: center, turn: body, shown: true };
    let head_at = world(head_joint(sp));
    let head = body * rz(rig.head[0]) * ry(-rig.head[1]);
    bones[HEAD] = Bone { at: head_at, turn: head, shown: true };
    let hinge = head_at + head * V3::from(jaw_hinge(sp)) * m;
    bones[JAW] = Bone { at: hinge, turn: head * ry(rig.jaw), shown: sp.jaw };
    let mouth = head_at + head * V3::from(mouth_of(sp)) * m;
    bones[TAIL] = Bone { at: world(tail_root(sp)), turn: body * rz(rig.tail[0]) * ry(rig.tail[1]), shown: sp.tail };
    // The wings, the left worked out and the right as it is in the mirror of it
    for side in 0..2 {
        let w = rig.wings[side];
        let inner = rz(w.sweep) * rx(w.flap) * ry(w.twist);
        let outer = inner * rz(w.fold) * rx(w.bend);
        let joint = shoulder(sp);
        let wrist_at = joint + inner * wrist(sp);
        let (inner, outer, joint, wrist_at) = if side == 0 { (inner, outer, joint, wrist_at) } else { (mirrored(inner), mirrored(outer), flip(joint), flip(wrist_at)) };
        bones[WING + side * 2] = Bone { at: world(joint), turn: body * inner, shown: true };
        bones[WING + side * 2 + 1] = Bone { at: world(wrist_at), turn: body * outer, shown: true };
    }
    let (lt, _) = leg_lengths(sp);
    for leg in 0..2 {
        let l = rig.legs[leg];
        let thigh = ry(-l.swing);
        let joint = hip(sp);
        let knee = joint + thigh * V3::new(0.0, 0.0, -lt);
        let shin = thigh * ry(l.knee);
        let (thigh, shin, joint, knee) = if leg == 0 { (thigh, shin, joint, knee) } else { (mirrored(thigh), mirrored(shin), flip(joint), flip(knee)) };
        bones[THIGH + leg] = Bone { at: world(joint), turn: body * thigh, shown: true };
        bones[SHIN + leg] = Bone { at: world(knee), turn: body * shin, shown: !sp.stiff };
    }
    let (la, _) = arm_lengths(sp);
    for arm in 0..2 {
        let a = rig.arms[arm];
        let upper = ry(-a.swing) * rx(a.spread);
        let joint = arm_root(sp);
        let elbow = joint + upper * V3::new(0.0, 0.0, -la);
        let fore = upper * ry(-a.elbow);
        let (upper, fore, joint, elbow) = if arm == 0 { (upper, fore, joint, elbow) } else { (mirrored(upper), mirrored(fore), flip(joint), flip(elbow)) };
        bones[UPPER + arm] = Bone { at: world(joint), turn: body * upper, shown: sp.arms != Arms::None };
        bones[FORE + arm] = Bone { at: world(elbow), turn: body * fore, shown: sp.arms == Arms::Jointed };
    }
    mouth
}

/// Where the soles of its feet are as it's posed, left and right, in voxels
fn feet_of(sp: &Spec, bones: &[Bone; BONES], vpm: f64) -> [V3; 2] {
    let m = sp.s * vpm;
    let (lt, ls) = leg_lengths(sp);
    std::array::from_fn(|leg| {
        let thigh = bones[THIGH + leg];
        let ankle = if sp.stiff {
            let bend = stand_leg(sp).knee;
            thigh.at + thigh.turn * V3::new(-ls * bend.sin(), 0.0, -lt - ls * bend.cos()) * m
        } else {
            let shin = bones[SHIN + leg];
            shin.at + shin.turn * V3::new(0.0, 0.0, -ls) * m
        };
        ankle - V3::new(0.0, 0.0, 0.03 * m)
    })
}

/// Where its claws or talons strike, in voxels: its feet's for the birds and harpies, its hands'
/// for the gargoyles
fn talons_of(sp: &Spec, bones: &[Bone; BONES], vpm: f64) -> V3 {
    if sp.arms != Arms::None {
        let (la, lf) = arm_lengths(sp);
        let m = sp.s * vpm;
        let hand = |k: usize| {
            let b = if sp.arms == Arms::Jointed { bones[FORE + k] } else { bones[UPPER + k] };
            let length = if sp.arms == Arms::Jointed { lf } else { la + lf * 0.8 };
            b.at + b.turn * V3::new(0.0, 0.0, -length) * m
        };
        return (hand(0) + hand(1)) * 0.5;
    }
    let [a, b] = feet_of(sp, bones, vpm);
    (a + b) * 0.5
}

/// Lines through each part of it, from one end to the other in voxels, with how thick round it is
/// in meters: for finding which part of it the player's hitting, see Flier::aim()
fn limbs(sp: &Spec, bones: &[Bone; BONES], vpm: f64) -> Vec<(V3, V3, f64)> {
    let m = sp.s * vpm;
    let mut out = Vec::new();
    let torso = bones[TORSO];
    out.push((torso.at + torso.turn * V3::new(0.5, 0.0, 0.0) * m, torso.at + torso.turn * V3::new(-0.45, 0.0, 0.0) * m, 0.26 * sp.girth * sp.s));
    let h = bones[HEAD];
    out.push((h.at, h.at + h.turn * V3::new(0.25, 0.0, 0.04) * m, 0.13 * sp.s));
    let (wi, wo) = wing_lengths(sp);
    for side in 0..2 {
        let y = if side == 0 { 1.0 } else { -1.0 };
        let (inner, outer) = (bones[WING + side * 2], bones[WING + side * 2 + 1]);
        out.push((inner.at + inner.turn * V3::new(-0.2, 0.1 * y, 0.0) * m, inner.at + inner.turn * V3::new(-0.2, y * wi, 0.0) * m, 0.24 * sp.s));
        out.push((outer.at + outer.turn * V3::new(-0.2, 0.0, 0.0) * m, outer.at + outer.turn * V3::new(-0.2, y * wo * 0.85, 0.0) * m, 0.2 * sp.s));
    }
    let feet = feet_of(sp, bones, vpm);
    for leg in 0..2 {
        out.push((bones[THIGH + leg].at, feet[leg], 0.08 * sp.girth * sp.s));
    }
    if sp.arms != Arms::None {
        let (la, lf) = arm_lengths(sp);
        for arm in 0..2 {
            let b = bones[UPPER + arm];
            out.push((b.at, b.at + b.turn * V3::new(0.0, 0.0, -(la + lf) * 0.9) * m, 0.08 * sp.s));
        }
    }
    if sp.tail {
        let t = bones[TAIL];
        out.push((t.at, t.at + t.turn * V3::new(-0.5, 0.0, 0.0) * m, 0.1 * sp.s));
    }
    out
}

/// The point of the segment from `a` to `b` nearest the line out from `origin` along `dir`, which
/// has length 1, ahead of it: how far it is from the line, and how far along it
fn nearest_to_line(a: V3, b: V3, origin: V3, dir: V3) -> (V3, f64, f64) {
    let ab = b - a;
    let mut best = (a, f64::MAX, 0.0);
    for k in 0..=8 {
        let p = a + ab * (k as f64 / 8.0);
        let along = (p - origin).dot(&dir);
        let off = (p - origin - dir * along).norm();
        if along > 0.0 && off < best.1 {
            best = (p, off, along);
        }
    }
    best
}

// What they do

/// Each thing they can do
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Move {
    /// Swooping down out of the air to rake the player with its talons as it passes low, and away
    Rake,
    /// Climbing high over the player and diving straight down on where they stood
    Stoop,
    /// The roc's: flying in low, seizing the player in its talons, carrying them up and letting go
    Snatch,
    /// A swipe of claws or talons at what's in front of it, on the ground or hovering over them
    Claw,
    /// Its head drawn back and its beak driven down at what's in front of it
    Peck,
    /// A gargoyle crouching and leaping onto the player
    Pounce,
    /// Its wings raised and beaten down hard, throwing back what's in front of it
    Gust,
    /// Feathers flung like knives in a fan at the player
    Volley,
    /// The thunderbird's: lightning called down out of the sky on the player and round them
    Lightning,
    /// Lightning spat from its beak at the player
    Arc,
    /// A shriek: on turning on the player, going into its next phase, or calling its kind down
    Shriek,
    /// A gargoyle wrapping its wings round itself and turning to stone, shrugging off blows
    Harden,
    /// A great gargoyle rearing up and bringing its fists down, stone bursting out all round
    Shatter,
}

const MOVES: usize = 13;

impl Move {
    const ALL: [Move; MOVES] = [Move::Rake, Move::Stoop, Move::Snatch, Move::Claw, Move::Peck, Move::Pounce, Move::Gust, Move::Volley, Move::Lightning, Move::Arc, Move::Shriek, Move::Harden, Move::Shatter];

    /// The move called `name`, for `--set boss_move=snatch`
    pub fn named(name: &str) -> Option<Move> {
        Move::ALL.into_iter().find(|m| format!("{m:?}").eq_ignore_ascii_case(name))
    }
}

/// For shots of a move: every one of them making only `--set boss_move=` this, as soon as it can,
/// see force()
static FORCED: AtomicU8 = AtomicU8::new(u8::MAX);

/// Every one of them made to make only move `m`, if it can, for shots of it
pub fn force(m: Option<Move>) {
    FORCED.store(m.map_or(u8::MAX, |m| m as u8), Ordering::Relaxed);
}

fn forced() -> Option<Move> {
    Move::ALL.get(FORCED.load(Ordering::Relaxed) as usize).copied()
}

/// Something it does: in the air or on the ground, from how near to how far in meters, how many
/// seconds till it can again, getting ready and getting over it, from which phase of a great
/// one's fight, and how likely next to the rest
#[derive(Clone, Copy)]
struct Skill {
    m: Move,
    air: bool,
    near: f64,
    far: f64,
    cool: f64,
    wind: f64,
    rest: f64,
    stage: u8,
    weight: f64,
}

#[allow(clippy::too_many_arguments)]
const fn skill(m: Move, air: bool, near: f64, far: f64, cool: f64, wind: f64, rest: f64, stage: u8, weight: f64) -> Skill {
    Skill { m, air, near, far, cool, wind, rest, stage, weight }
}

use Move::*;

/// A harpy's: clawing on the ground, raking, flinging feathers and hovering in to claw from the air
const HARPY: [Skill; 5] = [
    skill(Claw, false, 0.0, 2.6, 1.6, 0.45, 0.5, 0, 3.0),
    skill(Shriek, false, 0.0, 20.0, 14.0, 0.5, 0.3, 0, 0.5),
    skill(Rake, true, 4.0, 30.0, 4.5, 0.55, 0.3, 0, 3.0),
    skill(Volley, true, 6.0, 26.0, 7.0, 0.6, 0.4, 0, 1.4),
    skill(Claw, true, 0.0, 20.0, 6.0, 0.4, 0.4, 0, 1.0),
];

/// A skyhunter's: raking, stooping from on high, pecking and clawing on the ground
const SKYHUNTER: [Skill; 4] = [
    skill(Peck, false, 0.0, 2.4, 1.6, 0.4, 0.5, 0, 2.0),
    skill(Claw, false, 0.0, 2.2, 2.5, 0.45, 0.5, 0, 1.0),
    skill(Rake, true, 4.0, 34.0, 4.0, 0.5, 0.3, 0, 3.0),
    skill(Stoop, true, 3.0, 40.0, 8.0, 0.9, 0.3, 0, 1.8),
];

/// The Stormtalon: raking and stooping, lightning from its beak, its wings beating the player back
const STORMTALON: [Skill; 7] = [
    skill(Peck, false, 0.0, 4.0, 1.8, 0.55, 0.7, 0, 2.0),
    skill(Claw, false, 0.0, 3.5, 3.0, 0.6, 0.7, 0, 1.5),
    skill(Arc, false, 4.0, 26.0, 6.0, 0.8, 0.6, 0, 1.5),
    skill(Rake, true, 5.0, 44.0, 5.0, 0.7, 0.4, 0, 2.5),
    skill(Stoop, true, 4.0, 50.0, 9.0, 1.0, 0.4, 0, 1.5),
    skill(Arc, true, 6.0, 40.0, 5.5, 0.8, 0.4, 0, 2.0),
    skill(Gust, true, 3.0, 16.0, 10.0, 0.8, 0.6, 0, 1.0),
];

/// The Harridan: volleys of feathers, gusts and talons, and her shriek bringing the young ones
const HARRIDAN: [Skill; 7] = [
    skill(Claw, false, 0.0, 3.2, 1.8, 0.5, 0.6, 0, 2.5),
    skill(Volley, false, 5.0, 26.0, 6.0, 0.7, 0.6, 0, 2.0),
    skill(Shriek, false, 0.0, 40.0, 20.0, 0.7, 0.5, 0, 2.0),
    skill(Rake, true, 5.0, 36.0, 5.0, 0.6, 0.4, 0, 2.5),
    skill(Volley, true, 6.0, 34.0, 5.5, 0.7, 0.4, 0, 2.5),
    skill(Gust, true, 3.0, 16.0, 9.0, 0.8, 0.6, 0, 1.4),
    skill(Shriek, true, 0.0, 40.0, 20.0, 0.7, 0.4, 0, 1.0),
];

/// The Cloudreaver Roc: it lives in the air, raking, dive-bombing and snatching, its wings
/// battering the player, shrieking for skyhunters once hurt; on the ground its beak and talons
const ROC: [Skill; 9] = [
    skill(Peck, false, 0.0, 8.0, 2.2, 0.7, 0.9, 0, 2.5),
    skill(Claw, false, 0.0, 7.0, 4.0, 0.8, 0.9, 0, 1.5),
    skill(Gust, false, 0.0, 18.0, 9.0, 0.9, 0.8, 0, 1.5),
    skill(Rake, true, 8.0, 70.0, 6.0, 0.8, 0.4, 0, 2.5),
    skill(Stoop, true, 4.0, 70.0, 11.0, 1.3, 0.5, 0, 2.5),
    skill(Snatch, true, 8.0, 80.0, 16.0, 1.0, 0.5, 0, 2.5),
    skill(Gust, true, 6.0, 24.0, 11.0, 0.9, 0.6, 1, 1.2),
    skill(Shriek, true, 0.0, 90.0, 26.0, 0.7, 0.4, 1, 3.0),
    skill(Snatch, true, 8.0, 80.0, 11.0, 0.9, 0.4, 2, 2.0),
];

/// The Thunderbird: lightning called down and spat, a thunderclap of its wings, swoops and dives
const THUNDERBIRD: [Skill; 9] = [
    skill(Peck, false, 0.0, 7.5, 2.2, 0.7, 0.9, 0, 2.0),
    skill(Arc, false, 4.0, 40.0, 5.5, 0.9, 0.7, 0, 2.0),
    skill(Gust, false, 0.0, 16.0, 9.0, 0.9, 0.9, 0, 1.5),
    skill(Lightning, true, 0.0, 90.0, 9.0, 1.1, 0.5, 0, 3.0),
    skill(Arc, true, 6.0, 60.0, 5.0, 0.85, 0.4, 0, 2.0),
    skill(Rake, true, 8.0, 70.0, 6.5, 0.8, 0.4, 0, 1.5),
    skill(Stoop, true, 4.0, 70.0, 12.0, 1.2, 0.5, 1, 1.5),
    skill(Shriek, true, 0.0, 90.0, 26.0, 0.7, 0.4, 1, 2.5),
    skill(Lightning, true, 0.0, 90.0, 7.0, 1.0, 0.5, 2, 2.5),
];

/// The Harpy Queen: knives of feathers in fans, gales to throw the player off the edge, talons
/// from on high, and her daughters called down once she's hurt
const QUEEN: [Skill; 9] = [
    skill(Claw, false, 0.0, 5.0, 2.0, 0.65, 0.8, 0, 2.5),
    skill(Volley, false, 5.0, 40.0, 5.5, 0.8, 0.7, 0, 2.0),
    skill(Gust, false, 0.0, 22.0, 8.0, 0.9, 0.8, 0, 2.0),
    skill(Volley, true, 6.0, 50.0, 5.0, 0.8, 0.4, 0, 3.0),
    skill(Gust, true, 3.0, 26.0, 8.0, 0.9, 0.5, 0, 2.5),
    skill(Rake, true, 6.0, 60.0, 6.0, 0.8, 0.4, 0, 2.0),
    skill(Shriek, true, 0.0, 70.0, 24.0, 0.7, 0.4, 1, 3.0),
    skill(Stoop, true, 4.0, 60.0, 12.0, 1.1, 0.5, 2, 1.5),
    skill(Volley, true, 6.0, 50.0, 4.0, 0.7, 0.3, 2, 2.0),
];

/// A gargoyle's: clawing, leaping, gliding down on the player, and turning to stone when hurt
const GARGOYLE: [Skill; 5] = [
    skill(Claw, false, 0.0, 2.6, 1.7, 0.45, 0.55, 0, 3.0),
    skill(Pounce, false, 3.0, 11.0, 4.5, 0.55, 0.5, 0, 2.0),
    skill(Harden, false, 0.0, 30.0, 12.0, 0.4, 0.2, 0, 2.5),
    skill(Rake, true, 3.0, 26.0, 5.0, 0.5, 0.3, 0, 3.0),
    skill(Claw, true, 0.0, 14.0, 6.0, 0.45, 0.4, 0, 1.0),
];

/// The Belfry Grotesque: the same, harder, and stone bursting where it slams down, its shriek
/// waking its kind
const GROTESQUE: [Skill; 8] = [
    skill(Claw, false, 0.0, 5.0, 1.9, 0.6, 0.7, 0, 3.0),
    skill(Pounce, false, 4.0, 18.0, 5.0, 0.75, 0.6, 0, 2.0),
    skill(Harden, false, 0.0, 40.0, 14.0, 0.5, 0.3, 0, 2.5),
    skill(Shatter, false, 0.0, 9.0, 7.0, 0.9, 0.9, 0, 2.0),
    skill(Shriek, false, 0.0, 50.0, 24.0, 0.7, 0.5, 1, 2.5),
    skill(Rake, true, 4.0, 40.0, 5.5, 0.7, 0.4, 0, 2.5),
    skill(Stoop, true, 3.0, 40.0, 9.0, 1.0, 0.4, 0, 2.0),
    skill(Claw, true, 0.0, 16.0, 6.0, 0.55, 0.4, 0, 1.0),
];

/// The Bloodwake Gargoyle: all the grotesque does, in three phases, its slams and dives bursting
/// with bloodstone
const BLOODWAKE: [Skill; 9] = [
    skill(Claw, false, 0.0, 7.5, 2.0, 0.7, 0.8, 0, 3.0),
    skill(Pounce, false, 5.0, 24.0, 5.5, 0.85, 0.7, 0, 2.0),
    skill(Harden, false, 0.0, 50.0, 15.0, 0.6, 0.3, 0, 2.0),
    skill(Shatter, false, 0.0, 12.0, 6.0, 1.0, 1.0, 0, 2.5),
    skill(Shriek, false, 0.0, 70.0, 24.0, 0.7, 0.5, 1, 3.0),
    skill(Rake, true, 6.0, 60.0, 6.0, 0.8, 0.4, 0, 2.0),
    skill(Stoop, true, 4.0, 60.0, 9.0, 1.1, 0.5, 0, 2.5),
    skill(Shriek, true, 0.0, 70.0, 24.0, 0.7, 0.5, 1, 2.0),
    skill(Shatter, false, 0.0, 14.0, 5.0, 0.9, 0.9, 2, 2.0),
];

fn skills(look: Look) -> &'static [Skill] {
    match look {
        Look::Harpy => &HARPY,
        Look::Skyhunter => &SKYHUNTER,
        Look::Stormtalon => &STORMTALON,
        Look::Harridan => &HARRIDAN,
        Look::Roc => &ROC,
        Look::Thunderbird => &THUNDERBIRD,
        Look::HarpyQueen => &QUEEN,
        Look::Grotesque => &GROTESQUE,
        Look::BloodwakeGargoyle => &BLOODWAKE,
        _ => &GARGOYLE,
    }
}

/// How far through its health a great one goes into each next phase of its fight
pub fn phases(look: Look) -> &'static [f64] {
    match look {
        _ if is_raid(look) => &[0.66, 0.33],
        Look::Grotesque => &[0.5],
        _ => &[],
    }
}

/// How long it means to keep to the air, and then to the ground, in seconds at least and at most
fn budgets(look: Look, stage: u8) -> ([f64; 2], [f64; 2]) {
    match look {
        Look::Harpy => ([8.0, 12.0], [3.0, 5.0]),
        Look::Skyhunter => ([12.0, 18.0], [2.0, 4.0]),
        Look::Stormtalon => ([12.0, 16.0], [5.0, 7.0]),
        Look::Harridan => ([9.0, 12.0], [5.0, 7.0]),
        Look::Roc => ([16.0, 22.0], [5.0, 8.0]),
        Look::Thunderbird if stage >= 2 => ([20.0, 26.0], [4.0, 6.0]),
        Look::Thunderbird => ([14.0, 20.0], [6.0, 9.0]),
        Look::HarpyQueen => ([10.0, 14.0], [6.0, 9.0]),
        Look::Grotesque => ([4.0, 6.0], [10.0, 14.0]),
        Look::BloodwakeGargoyle => ([6.0, 9.0], [10.0, 14.0]),
        _ => ([2.5, 4.0], [8.0, 14.0]),
    }
}

/// How it flies: meters a second cruising and diving, in a circle how many meters round the player
/// and how high over them, how many radians a second it turns, how hard it speeds up, and how
/// many times a second its wings beat cruising
struct Flight {
    cruise: f64,
    dash: f64,
    orbit: f64,
    altitude: f64,
    turn: f64,
    accel: f64,
    beat: f64,
}

fn flight(look: Look) -> Flight {
    let f = |cruise, dash, orbit, altitude, turn, accel, beat| Flight { cruise, dash, orbit, altitude, turn, accel, beat };
    match look {
        Look::Harpy => f(8.0, 15.0, 9.0, 6.0, 2.6, 12.0, 2.4),
        Look::Skyhunter => f(10.0, 22.0, 12.0, 9.0, 2.4, 14.0, 2.0),
        Look::Stormtalon => f(12.0, 24.0, 16.0, 12.0, 1.6, 11.0, 1.4),
        Look::Harridan => f(9.0, 17.0, 12.0, 8.0, 2.0, 10.0, 1.7),
        Look::Roc => f(14.0, 30.0, 30.0, 20.0, 0.8, 8.0, 0.55),
        Look::Thunderbird => f(13.0, 26.0, 26.0, 17.0, 0.9, 8.0, 0.65),
        Look::HarpyQueen => f(11.0, 20.0, 18.0, 11.0, 1.2, 9.0, 0.95),
        Look::Grotesque => f(9.0, 18.0, 13.0, 8.0, 1.4, 9.0, 1.1),
        Look::BloodwakeGargoyle => f(11.0, 22.0, 20.0, 12.0, 1.0, 8.0, 0.8),
        _ => f(7.5, 15.0, 8.0, 4.5, 2.2, 10.0, 1.9),
    }
}

/// How hard each of its blows is: claws or talons, beak, a rake as it passes, a dive coming down,
/// a feather, lightning, a shard of stone, its wings beating and how fast in meters a second they
/// throw the player
struct Power {
    claw: f64,
    peck: f64,
    rake: f64,
    stoop: f64,
    feather: f64,
    bolt: f64,
    shard: f64,
    gust: f64,
    push: f64,
}

fn power(look: Look) -> Power {
    let p = |claw, peck, rake, stoop, feather, bolt, shard, gust, push| Power { claw, peck, rake, stoop, feather, bolt, shard, gust, push };
    match look {
        _ if is_monster(look) && gargoyles::is_gargoyle(look) => p(11.0, 10.0, 12.0, 14.0, 0.0, 0.0, 6.0, 0.0, 5.0),
        _ if is_monster(look) => p(10.0, 11.0, 12.0, 15.0, 7.0, 0.0, 0.0, 2.0, 5.0),
        _ if is_raid(look) => p(26.0, 28.0, 28.0, 34.0, 13.0, 22.0, 16.0, 10.0, 14.0),
        _ => p(18.0, 19.0, 21.0, 26.0, 10.0, 16.0, 12.0, 7.0, 10.0),
    }
}

/// How hard its everyday blows hit, its claws, beak and a rake as it passes, on the whole, for
/// bosses::blow()
pub fn blow(look: Look) -> f64 {
    let p = power(look);
    (p.claw + p.peck + p.rake) / 3.0
}

/// How near the player has to come before it turns on them, how far off they can get before it
/// gives up, and how far from home it'll follow them, in meters. A gargoyle asleep on its perch
/// wakes only for someone coming close.
fn senses(look: Look) -> (f64, f64, f64) {
    match look {
        _ if is_monster(look) && gargoyles::is_gargoyle(look) => (11.0, 40.0, 55.0),
        _ if is_monster(look) => (26.0, 50.0, 75.0),
        Look::Grotesque => (20.0, 70.0, 100.0),
        _ => (50.0, 110.0, 160.0),
    }
}

/// What they do that no other boss does, for src\game\aerie.rs to see to. Positions are in voxels.
#[derive(Clone, Copy, Debug)]
pub enum Act {
    /// Getting ready to make move `m`, heard as it does, its head at `at`
    Warn { m: Move, at: V3 },
    /// A shriek: on `first` turning on the player, going into the next phase of its fight, or
    /// calling down `brood`, so many of them; `wake`s the gargoyles sleeping round it
    Shriek { at: V3, brood: Option<(Look, usize)>, first: bool, wake: bool },
    /// Talons, claws or a beak striking at `at` as move `m`, hurting what's within `radius`
    /// meters for `damage`
    Strike { m: Move, at: V3, radius: f64, damage: f64 },
    /// Its wings beating down hard at `at` along `dir`, throwing back what's within `reach`
    /// meters in front at `push` meters a second
    Gust { at: V3, dir: V3, reach: f64, push: f64, damage: f64 },
    /// `count` feathers flung from `from` at `to`, fanned out `spread` meters either side of it
    /// there, `speed` meters a second
    Feathers { from: V3, to: V3, count: u8, spread: f64, speed: f64, damage: f64 },
    /// Lightning to strike `to` out of the sky after `delay` seconds, `radius` meters round
    Lightning { to: V3, radius: f64, damage: f64, delay: f64 },
    /// Lightning from its beak at `from` to `to`
    Arc { from: V3, to: V3, damage: f64 },
    /// Coming down hard on `at` out of a dive or a leap, or its fists slamming down: hurting what's
    /// within `radius` meters, the dust thrown up and the ground shaking
    Impact { at: V3, radius: f64, damage: f64 },
    /// Shards of its stone bursting out of `at` all round, `count` of them
    Shards { at: V3, count: u8, damage: f64 },
    /// The roc seizing the player at `at`, carrying them where its talons are, at `to`, and
    /// letting go
    Grab { at: V3, damage: f64 },
    Carry { to: V3 },
    Drop { at: V3 },
    /// Taking off and landing, a downstroke of its wings, and crashing to the ground, `size` big
    TakeOff { at: V3, size: f64 },
    Land { at: V3, size: f64 },
    Wingbeat { at: V3, size: f64 },
    Crash { at: V3, size: f64 },
    /// Lightning crackling at its beak as it gets ready to loose it
    Spark { at: V3 },
    /// Stone cracking off it, `size` big: waking, coming out of its stone again, or breaking up
    /// as it dies
    Crumble { at: V3, size: f64 },
    /// Turning to stone, the grinding of it
    Petrify { at: V3, size: f64 },
    /// Its last cry as it falls
    Death { at: V3 },
}

/// What it's doing
#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
    /// At home, or going home: perched, soaring over it, or flying back to land there
    Rest,
    /// A gargoyle asleep on its perch, stone through and through
    Statue,
    /// A gargoyle waking, the stone cracking off it, for so many seconds
    Wake(f64),
    /// After the player: circling over them, or coming at them on foot, choosing what to do next
    Hunt,
    /// Taking off, for so many seconds
    Rise(f64),
    /// Coming down to land
    Alight(f64),
    /// Getting ready to make a move, so it can be seen coming
    Wind(Move, f64),
    /// Making it
    Do(Move, f64),
    /// Getting over it, for so many seconds more: the moment to hit it
    Spent(f64),
    /// A gargoyle turned to stone, for so many seconds
    Stone(f64),
    /// Knocked out of the air, or down on the ground dazed, for so many seconds more once down
    Reel(f64),
    /// Dying
    Fall,
}

/// How its wings are going: where they're held between beats, how far they beat either way, and
/// how many beats a second
#[derive(Clone, Copy, Debug)]
struct WingSet {
    base: WingPose,
    amp: f64,
    rate: f64,
}

/// Where it means to go this frame: what it's after, how fast, what to face, and whether it may
/// come right down to the ground
#[derive(Clone, Copy)]
struct Goal {
    to: V3,
    speed: f64,
    face: Option<V3>,
    low: bool,
}

/// A piece of a gargoyle broken off as it dies, tumbling: how fast it's going and turning
#[derive(Clone, Copy)]
struct Rubble {
    velocity: V3,
    spin: V3,
}

pub struct Flier {
    look: Look,
    /// The middle of its body, in voxels, how fast it's going in voxels a second, which way it
    /// faces, and whether it's flying
    center: V3,
    velocity: V3,
    yaw: f64,
    airborne: bool,
    state: State,
    clock: f64,
    cool: [f64; MOVES],
    /// Seconds till it'll make its next move, and the skill it's making
    next: f64,
    skill: Option<Skill>,
    /// Where its move's aimed, and the way it goes about it: where a swoop starts, comes lowest
    /// and ends, which way a run goes
    aim: V3,
    path: [V3; 3],
    way: V3,
    ready: bool,
    struck: bool,
    again: u8,
    /// Where round the player it's circling them, and which way round
    orbit: f64,
    round: f64,
    /// Seconds it's been in the air or on the ground, and how long it means to be
    aloft: f64,
    budget: f64,
    blocked: f64,
    /// How much health it had last frame, how much it's lost in the last second or so, and
    /// seconds till it can be knocked out of the air again
    health: f64,
    recent: f64,
    reeled: f64,
    /// Flinching from a blow, 1 just struck, and which way
    flinch: f64,
    flinch_side: f64,
    seen: bool,
    look_wait: f64,
    unseen: f64,
    /// Where it wanders to at home
    wander: V3,
    /// Perched: where it's looking, darting its head about as a bird does, how far its head's got
    /// there, seconds till it looks somewhere else, and seconds into rousing its feathers
    gaze: [f64; 2],
    gazed: [f64; 2],
    glance: f64,
    rouse: f64,
    rig: Rig,
    wings: WingSet,
    /// How far through a beat of its wings and a stride of its legs it is, 0 to 1
    beat: f64,
    stride: f64,
    bones: [Bone; BONES],
    mouth: V3,
    focus: V3,
    /// How high its middle stands over its feet standing, in meters
    stand: f64,
    /// Where the ground is under its feet, in voxels
    foot_z: f64,
    /// Seconds it's lain dead on the ground, falling still before
    dead: Option<f64>,
    rubble: Option<Box<[Rubble; BONES]>>,
    stage: u8,
    placed: bool,
    /// How much of it's stone, 0 to 1: a gargoyle asleep or hardened, or one dying
    stone: f64,
    /// A gargoyle hurt badly enough to want to turn to stone, as soon as it's down on the ground
    harden: bool,
    /// The roc has the player in its talons, so many seconds
    holding: Option<f64>,
    /// For shots of a move: the one move it makes, whenever it can, see force()
    pub only: Option<Move>,
    rng: u32,
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^ (x >> 16)
}

/// The ground under `x`, `y`, in voxels: the first solid voxel with room to stand over it, looking
/// down from a little over `z`, a quarter of a meter at a time and then to the voxel; None if
/// there's nothing within `depth` meters
fn ground_z(world: &World, x: f64, y: f64, z: f64, depth: f64) -> Option<f64> {
    let vpm = world.voxels_per_meter as f64;
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    if xi < 0 || yi < 0 || xi >= world.size[0] as i64 || yi >= world.size[1] as i64 {
        return None;
    }
    let step = (vpm / 4.0).max(1.0) as i64;
    let top = (z + 2.0 * vpm).min(world.size[2] as f64 - 1.0) as i64;
    let bottom = (z - depth * vpm).max(0.0) as i64;
    let mut k = top;
    while k >= bottom {
        if !is_passable(world.material_at(xi, yi, k)) {
            let mut top_solid = k;
            while top_solid < top && !is_passable(world.material_at(xi, yi, top_solid + 1)) {
                top_solid += 1;
            }
            return Some(top_solid as f64 + 1.0);
        }
        k -= step;
    }
    None
}

/// Whether nothing solid's between `from` and `to`, in voxels, looking half a meter at a time
fn clear_between(world: &World, from: V3, to: V3) -> bool {
    let vpm = world.voxels_per_meter as f64;
    let steps = ((to - from).norm() / (0.5 * vpm)).ceil() as usize;
    (1..steps).all(|s| {
        let p = from + (to - from) * (s as f64 / steps as f64);
        is_passable(world.material_at(p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64))
    })
}

/// The angle from `a` round to `b`, the short way, -PI to PI
fn turn_to(a: f64, b: f64) -> f64 {
    (b - a + PI).rem_euclid(TAU) - PI
}

fn flat(v: V3) -> V3 {
    V3::new(v.x, v.y, 0.0)
}

fn smooth(f: f64) -> f64 {
    let f = f.clamp(0.0, 1.0);
    f * f * (3.0 - 2.0 * f)
}

impl Flier {
    /// One standing with its feet at `at`, in voxels, facing `yaw`
    pub fn new(look: Look, at: V3, yaw: f64, seed: u32) -> Self {
        let sp = spec(look);
        let rig = Rig::standing(&sp);
        Flier {
            look,
            center: at,
            velocity: V3::zeros(),
            yaw,
            airborne: false,
            state: State::Rest,
            clock: 0.0,
            cool: [0.0; MOVES],
            next: 1.5,
            skill: None,
            aim: at,
            path: [at; 3],
            way: V3::x(),
            ready: false,
            struck: false,
            again: 0,
            orbit: (hash(seed) % 628) as f64 * 0.01,
            round: if seed % 2 == 0 { 1.0 } else { -1.0 },
            aloft: 0.0,
            budget: 8.0,
            blocked: 0.0,
            health: health(look),
            recent: 0.0,
            reeled: 0.0,
            flinch: 0.0,
            flinch_side: 1.0,
            seen: false,
            look_wait: 0.0,
            unseen: 0.0,
            wander: at,
            gaze: [0.0; 2],
            gazed: [0.0; 2],
            glance: 1.0,
            rouse: -1.0,
            rig,
            wings: WingSet { base: folded(&sp), amp: 0.0, rate: 0.0 },
            beat: 0.0,
            stride: 0.0,
            bones: [Bone::default(); BONES],
            mouth: at,
            focus: at,
            stand: stand_height(&sp),
            foot_z: at.z,
            dead: None,
            rubble: None,
            stage: 0,
            placed: false,
            stone: 0.0,
            harden: false,
            holding: None,
            only: forced(),
            rng: hash(seed ^ 0xa71a),
        }
    }

    fn roll(&mut self) -> f64 {
        self.rng = hash(self.rng.wrapping_add(0x9e37_79b9));
        (self.rng >> 8) as f64 / 16777216.0
    }

    /// The part of it the player's looking at, in voxels, which is what's hit, see aim()
    pub fn focus(&self) -> V3 {
        self.focus
    }

    /// Whether it's turned to stone, shrugging off blows: a gargoyle asleep, or hardened
    pub fn hardened(&self) -> bool {
        matches!(self.state, State::Statue | State::Wake(_)) || matches!(self.state, State::Stone(t) if t > 0.25)
    }

    /// The world moved over by `shift` voxels
    pub fn recenter(&mut self, shift: V3) {
        self.center -= shift;
        self.aim -= shift;
        for p in &mut self.path {
            *p -= shift;
        }
        self.wander -= shift;
        self.mouth -= shift;
        self.focus -= shift;
        self.foot_z -= shift.z;
        for b in &mut self.bones {
            b.at -= shift;
        }
    }

    /// Called down out of the sky by a shriek: in the air at `at`, in voxels, flying in towards
    /// `toward`, after the player
    pub fn swoop_in(&mut self, at: V3, toward: V3, vpm: f64) {
        let sp = spec(self.look);
        self.center = at;
        self.placed = true;
        self.airborne = true;
        self.state = State::Hunt;
        let way = flat(toward - at).try_normalize(1e-6).unwrap_or_else(V3::x);
        self.yaw = way.y.atan2(way.x);
        self.velocity = way * flight(self.look).cruise * vpm;
        self.wings = WingSet { base: SPREAD, amp: 0.6, rate: flight(self.look).beat };
        self.rig = Rig::flying(&sp);
        self.budget = 10.0;
        self.next = 1.5 + self.roll() * 2.0;
    }

    /// Picks the part of it the player at `eye`, looking along `dir`, is looking at, or if they're
    /// looking past it, the part nearest them, to be hit, see Animal::center()
    pub fn aim(&mut self, eye: V3, dir: V3, vpm: f64) {
        if self.rubble.is_some() {
            return;
        }
        let sp = spec(self.look);
        let parts = limbs(&sp, &self.bones, vpm);
        let dir = dir.try_normalize(1e-9).unwrap_or_else(V3::x);
        let body = body(self.look).1 * vpm;
        let mut best: Option<(f64, V3)> = None;
        for &(a, b, thick) in &parts {
            let (p, off, along) = nearest_to_line(a, b, eye, dir);
            let reach = thick.max(body / vpm) * vpm;
            if off < reach + 0.3 * vpm && best.is_none_or(|(s, _)| along < s) {
                best = Some((along, p));
            }
        }
        let nearest = || {
            parts
                .iter()
                .map(|&(a, b, _)| {
                    let ab = b - a;
                    let t = ((eye - a).dot(&ab) / ab.norm_squared().max(1e-9)).clamp(0.0, 1.0);
                    a + ab * t
                })
                .min_by(|p, q| (p - eye).norm().total_cmp(&(q - eye).norm()))
                .unwrap_or(self.center)
        };
        self.focus = best.map_or_else(nearest, |(_, p)| p);
    }
}

/// Whether one that's been killed is still falling out of the sky, lying where it fell, or
/// breaking up in pieces, so it's not to be taken away yet, see Game::reap()
pub fn dying(a: &Animal) -> bool {
    let Some(d) = &a.winged else {
        return false;
    };
    let lie = if d.rubble.is_some() { 3.2 } else if is_monster(a.look) { 1.4 } else { 2.6 };
    a.health <= 0.0 && d.dead.is_none_or(|t| t < lie)
}

/// What everything round it needs to know to act
struct Here<'a> {
    world: &'a World,
    water: WaterLevel<'a>,
    feet: Option<V3>,
    vpm: f64,
    dt: f64,
    sp: Spec,
    look: Look,
    /// Its moves come quicker, angry
    quick: f64,
    stand: f64,
}

/// For looking at them, see `--set aerie_show=1` in main.rs: with nobody about, they fight as if
/// someone stood a little way from where they were put
pub static SHOW: AtomicBool = AtomicBool::new(false);
/// `--set aerie_trace=1`: what each's doing, twice a second
pub static TRACE: AtomicBool = AtomicBool::new(false);

/// What one does this frame, see the top of this file
pub fn act(a: &mut Animal, d: &mut Flier, world: &World, water: WaterLevel, player: Option<V3>, dt: f64) {
    let vpm = world.voxels_per_meter as f64;
    let look = a.look;
    let sp = spec(look);
    let Some(mut boss) = a.boss.take() else {
        return;
    };
    let player = player.or_else(|| SHOW.load(Ordering::Relaxed).then_some(boss.home + V3::new(9.0 * vpm, 0.0, 0.0)));
    let pace = a.status.pace();
    let here = Here { world, water, feet: player, vpm, dt: dt * pace.max(0.0), sp, look, quick: if boss.enraged { 0.75 } else { 1.0 }, stand: d.stand * vpm };
    if !d.placed {
        place(a, d, &boss, &here);
    }
    d.clock += dt;
    for c in &mut d.cool {
        *c -= here.dt;
    }
    d.reeled -= dt;
    // How hard it's been hit lately, and a flinch from each blow
    let lost = (d.health - a.health).max(0.0);
    d.health = a.health;
    d.recent = d.recent * (-dt / 1.5).exp() + lost;
    if lost > a.max_health * 0.002 && a.health > 0.0 && !d.hardened() {
        let hard = (lost / a.max_health * if is_monster(look) { 6.0 } else { 40.0 }).clamp(0.35, 1.0);
        d.flinch = d.flinch.max(hard);
        d.flinch_side = if d.roll() < 0.5 { -1.0 } else { 1.0 };
    }
    d.flinch = (d.flinch - dt * 5.0).max(0.0);
    // Thrown back by a blow, or knocked about in the air
    let push = a.status.push * dt;
    if push.norm() > 1e-3 && !matches!(d.state, State::Statue | State::Stone(_)) {
        d.center += V3::new(push.x, push.y, 0.0);
    }
    a.status.push *= (-6.0 * dt).exp();

    let mut goal = Goal { to: d.center, speed: 0.0, face: None, low: false };
    let mut want = if d.airborne { Rig::flying(&sp) } else { Rig::standing(&sp) };
    let mut wings = if d.airborne { WingSet { base: SPREAD, amp: 0.55, rate: flight(look).beat } } else { WingSet { base: folded(&sp), amp: 0.0, rate: 0.0 } };

    if a.health > 0.0 && d.state == State::Fall {
        d.state = State::Rest;
        d.dead = None;
        d.rubble = None;
    }
    if a.health <= 0.0 {
        fall(d, &mut boss, &here, &mut goal, &mut want, &mut wings);
    } else {
        // Frozen solid, netted or knocked senseless in the air, a monster falls out of it
        if pace <= 0.0 && d.airborne && !matches!(d.state, State::Reel(_)) && is_monster(look) {
            d.state = State::Reel(2.0);
        }
        notice(a, d, &mut boss, &here);
        think(a, d, &mut boss, &here, &mut goal, &mut want, &mut wings);
    }
    if pace > 0.0 || a.health <= 0.0 || d.airborne {
        travel(d, &here, &goal, &mut want);
    }
    let beat = d.beat;
    animate(d, &mut boss, &here, &want, &wings, a.health <= 0.0);
    // Each downstroke heard as it starts
    if d.airborne && d.wings.amp > 0.3 && d.beat < beat {
        boss.deeds.push(Deed::Aerie(Act::Wingbeat { at: d.center, size: size(look) }));
    }
    // As an animal: its feet under the middle of its body, facing the way it faces
    a.position = V3::new(d.center.x, d.center.y, if d.airborne { d.center.z - here.stand } else { d.foot_z });
    a.heading = d.yaw;
    a.target_heading = d.yaw;
    a.speed = 0.0;
    a.mood = animals::Mood::Idle;
    if TRACE.load(Ordering::Relaxed) && (d.clock * 2.0).floor() != ((d.clock - dt) * 2.0).floor() {
        let feet = here.feet.unwrap_or(boss.home);
        println!(
            "aerie {look:?}: {:?} {} engaged {} stage {} {:.1} m off, {:.1} m over the ground, next {:.1}, aloft {:.1} of {:.1}, stone {:.2}, health {:.0}",
            d.state,
            if d.airborne { "flying" } else { "on foot" },
            boss.engaged,
            boss.stage,
            flat(feet - d.center).norm() / vpm,
            (d.center.z - ground_z(world, d.center.x, d.center.y, d.center.z, 60.0).unwrap_or(d.center.z)) / vpm,
            d.next,
            d.aloft,
            d.budget,
            d.stone,
            a.health
        );
    }
    a.boss = Some(boss);
}

/// Put where it was made, on the first frame: standing there, on its perch facing out over the
/// drop if it's on one, and a gargoyle asleep there in stone, unless it's come for the player
fn place(a: &Animal, d: &mut Flier, boss: &Boss, h: &Here) {
    d.placed = true;
    d.center = a.position + V3::new(0.0, 0.0, h.stand);
    d.foot_z = a.position.z;
    d.wander = a.position;
    if !boss.raider && !boss.engaged {
        if let Some(out) = drop_off(h.world, a.position, 3.0) {
            d.yaw = out;
        }
        if gargoyles::is_gargoyle(h.look) {
            d.state = State::Statue;
            d.stone = 1.0;
            d.rig = gargoyles::statue(&h.sp);
            d.wings.base = d.rig.wings[0];
        }
    }
}

/// Which way the ground falls away most within `within` meters of `feet`, in voxels, as from the
/// edge of a wall's top, a cliff or an island: None on level ground
fn drop_off(world: &World, feet: V3, within: f64) -> Option<f64> {
    let vpm = world.voxels_per_meter as f64;
    let mut best: Option<(f64, f64)> = None;
    for k in 0..16 {
        let a = k as f64 / 16.0 * TAU;
        let at = feet + V3::new(a.cos(), a.sin(), 0.0) * within * vpm;
        let fall = ground_z(world, at.x, at.y, feet.z + 0.5 * vpm, 40.0).map_or(40.0 * vpm, |z| feet.z - z);
        if fall > 1.2 * vpm && best.is_none_or(|(f, _)| fall > f) {
            best = Some((fall, a));
        }
    }
    best.map(|(_, a)| a)
}

/// Noticing the player, going after them, giving up on them, and going into the next phase of its
/// fight as it's hurt
fn notice(a: &mut Animal, d: &mut Flier, boss: &mut Boss, h: &Here) {
    let vpm = h.vpm;
    let (sense, give_up, leash) = senses(h.look);
    let sparing = boss.spare_for(h.dt);
    let distance = h.feet.map_or(f64::MAX, |p| flat(p - d.center).norm() / vpm);
    let from_home = flat(d.center - boss.home).norm() / vpm;
    d.look_wait -= h.dt;
    if d.look_wait <= 0.0 {
        d.look_wait = 0.25;
        d.seen = (boss.raider && h.feet.is_some()) || h.feet.is_some_and(|p| distance < give_up && clear_between(h.world, d.mouth, p + V3::new(0.0, 0.0, 1.3 * vpm)));
    }
    let alarmed = boss.flock.size > 1 && boss.flock.alarmed && distance < give_up && !sparing;
    if !boss.engaged && !sparing && h.feet.is_some() && (boss.raider || (distance < sense && d.seen) || alarmed) {
        boss.engaged = true;
    }
    // A gargoyle asleep woken by someone coming near, or struck, or the rest of its pack waking:
    // a pack wakes one after another
    if boss.engaged && d.state == State::Statue {
        let stagger = if boss.flock.size > 1 { -0.25 - 0.6 * d.roll() } else { 0.0 };
        d.state = State::Wake(stagger);
        return;
    }
    // Turning on the player, having noticed them, or been struck by them or sent after them
    if boss.engaged && d.state == State::Rest && h.feet.is_some() && (boss.raider || distance < give_up) {
        d.next = 0.8 + d.roll() * 1.2;
        let wake = h.look == Look::Grotesque || h.look == Look::BloodwakeGargoyle;
        boss.deeds.push(Deed::Aerie(Act::Shriek { at: d.mouth, brood: None, first: true, wake }));
        d.state = if d.airborne || (is_monster(h.look) && distance < 4.0) {
            State::Hunt
        } else if is_monster(h.look) {
            if gargoyles::is_gargoyle(h.look) { State::Hunt } else { State::Rise(0.0) }
        } else {
            begin(d, h, Shriek, true);
            d.state
        };
        d.aloft = 0.0;
        d.budget = pick_budget(d, h, !d.airborne);
    }
    boss.set_sighted(d.seen && boss.engaged);
    d.unseen = if d.seen || !boss.engaged { 0.0 } else { d.unseen + h.dt };
    if boss.engaged && !boss.raider && (h.feet.is_none() || distance > give_up || from_home > leash || d.unseen > 12.0) {
        boss.calm();
    }
    if !boss.engaged && !matches!(d.state, State::Rest | State::Statue | State::Reel(_) | State::Alight(_) | State::Fall | State::Stone(_)) {
        d.state = State::Rest;
        d.skill = None;
        d.holding = None;
    }
    // A great one going into the next phase of its fight: shrieking, calling its brood, and in the
    // last of them quicker about everything
    let marks = phases(h.look);
    if boss.engaged && (boss.stage as usize) < marks.len() && a.vitality() < marks[boss.stage as usize] && !matches!(d.state, State::Do(..) | State::Reel(_) | State::Fall | State::Stone(_)) && d.holding.is_none() {
        boss.stage += 1;
        boss.enraged = boss.stage as usize >= marks.len();
        d.stage = boss.stage;
        boss.deeds.push(Deed::Phase { at: d.center, stage: boss.stage });
        d.cool[Shriek as usize] = 0.0;
        d.cool[Lightning as usize] = d.cool[Lightning as usize].min(2.0);
        begin(d, h, Shriek, true);
        d.aloft = 0.0;
        d.budget = pick_budget(d, h, !d.airborne);
    }
}

/// How long it means to keep to the air, or the ground, from now
fn pick_budget(d: &mut Flier, h: &Here, air: bool) -> f64 {
    let (aloft, down) = budgets(h.look, d.stage);
    let [lo, hi] = if air { aloft } else { down };
    lo + (hi - lo) * d.roll()
}

/// What it can do from where it is, if anything's ready, picked by how likely each is: a gargoyle
/// turns to stone only once it's been hurt
fn choose(d: &mut Flier, h: &Here, stage: u8, distance: f64, hurt: bool) -> Option<Skill> {
    let only = d.only;
    // Hurt that badly, a gargoyle turns to stone as soon as it can
    if hurt && !d.airborne && only.is_none_or(|m| m == Harden)
        && let Some(stone) = skills(h.look).iter().find(|s| s.m == Harden && !s.air && d.cool[Harden as usize] <= 0.0)
    {
        return Some(*stone);
    }
    let ready: Vec<Skill> = skills(h.look)
        .iter()
        .copied()
        .filter(|s| s.air == d.airborne && s.stage <= stage && d.cool[s.m as usize] <= 0.0 && (s.near..s.far).contains(&distance))
        .filter(|s| s.m != Harden || hurt || only == Some(Harden))
        .filter(|s| only.is_none_or(|m| m == s.m))
        .collect();
    let total: f64 = ready.iter().map(|s| s.weight).sum();
    if total <= 0.0 {
        return None;
    }
    let mut pick = d.roll() * total;
    for s in &ready {
        if pick < s.weight {
            return Some(*s);
        }
        pick -= s.weight;
    }
    ready.last().copied()
}

/// Starts getting ready to make move `m`, now
fn begin(d: &mut Flier, h: &Here, m: Move, forced: bool) {
    let found = skills(h.look).iter().copied().find(|s| s.m == m && (forced || s.air == d.airborne));
    d.skill = Some(found.unwrap_or(skill(m, d.airborne, 0.0, 1e9, 10.0, 0.6, 0.5, 0, 1.0)));
    d.state = State::Wind(m, 0.0);
    d.ready = false;
    d.struck = false;
    d.again = match m {
        Claw if gargoyles::is_gargoyle(h.look) && !d.airborne => 2,
        Volley if h.look == Look::HarpyQueen && d.stage >= 2 => 2,
        Gust if h.look == Look::HarpyQueen => 3,
        _ => 1,
    };
    let feet = h.feet.unwrap_or(d.center);
    d.aim = feet;
    d.way = flat(feet - d.center).try_normalize(1e-6).unwrap_or_else(|| V3::new(d.yaw.cos(), d.yaw.sin(), 0.0));
}

/// Starts coming down to land near the player, or at home without them
fn begin_landing(d: &mut Flier, h: &Here) {
    let vpm = h.vpm;
    let k = size(h.look);
    let around = h.feet.unwrap_or(d.center);
    let away = flat(d.center - around).try_normalize(1e-6).unwrap_or_else(V3::x);
    // A little way short of the player, clear of them, on open ground, trying round about
    let off = (2.5 + 2.0 * k) * vpm;
    let level = h.feet.and_then(|f| ground_z(h.world, f.x, f.y, f.z + 1.0 * vpm, 30.0));
    let mut spot = None;
    for j in 0..8 {
        let turn = [0.0, 0.6, -0.6, 1.2, -1.2, 1.9, -1.9, PI][j];
        let (c, s) = (turn.cos(), turn.sin());
        let way = V3::new(away.x * c - away.y * s, away.x * s + away.y * c, 0.0);
        let at = around + way * off;
        if let Some(z) = ground_z(h.world, at.x, at.y, at.z + 6.0 * vpm, 30.0)
            && (h.water)(at.x, at.y) < z + 0.2 * vpm
            && level.is_none_or(|l| (z - l).abs() < 3.0 * vpm)
        {
            spot = Some(V3::new(at.x, at.y, z));
            break;
        }
    }
    match spot {
        Some(spot) => {
            d.aim = spot;
            d.state = State::Alight(0.0);
        }
        None => {
            d.aloft = 0.0;
            d.budget = 5.0;
        }
    }
}

fn ground_of(h: &Here, at: V3) -> f64 {
    ground_z(h.world, at.x, at.y, at.z, 40.0).unwrap_or(at.z)
}

fn mark(boss: &mut Boss, at: V3, radius: f64, closing: f64) {
    boss.deeds.push(Deed::Mark { at, radius, closing: closing.clamp(0.0, 1.0) });
}

fn act_out(boss: &mut Boss, act: Act) {
    boss.deeds.push(Deed::Aerie(act));
}

/// Its head turned towards `at`, as far as it goes
fn look_at(d: &Flier, want: &mut Rig, at: V3) {
    let from = d.bones[HEAD].at;
    let body = rz(d.yaw);
    let local = body.inverse() * (at - from);
    let yaw = local.y.atan2(local.x).clamp(-1.4, 1.4);
    let pitch = local.z.atan2(flat(local).norm()).clamp(-1.0, 0.7);
    // Its head's held level as its body tips, see Rig::standing(), so it looks from there
    want.head[0] += yaw * 0.8;
    want.head[1] += pitch * 0.75 - (d.rig.pitch - want.pitch) * 0.0;
}

/// The size of its blows, in meters: how far round a claw, a peck and a rake hurt, how far round a
/// dive or a leap coming down does, and how far its wings throw back
fn reaches(h: &Here) -> (f64, f64, f64, f64, f64) {
    let s = h.sp.s;
    let k = size(h.look);
    (0.9 + 0.75 * k.powf(0.8), 0.8 + 0.6 * k.powf(0.8), 1.2 + 0.8 * k.powf(0.7), 1.8 + 1.3 * k.powf(0.75), 4.0 + 3.0 * s.powf(0.7))
}

/// How far in front of its middle a claw or peck on the ground lands, in meters
fn strike_reach(h: &Here) -> f64 {
    let sp = &h.sp;
    match sp.plan {
        Plan::Gargoyle => 0.75 * sp.s,
        _ => 0.7 * sp.s,
    }
}

/// What it does, see State
fn think(a: &mut Animal, d: &mut Flier, boss: &mut Boss, h: &Here, goal: &mut Goal, want: &mut Rig, wings: &mut WingSet) {
    let vpm = h.vpm;
    let s = h.sp.s;
    let fl = flight(h.look);
    let pw = power(h.look);
    let (claw_r, peck_r, rake_r, impact_r, gust_r) = reaches(h);
    let feet = h.feet.unwrap_or(boss.home);
    let ground_here = ground_of(h, d.center);
    let distance = flat(feet - d.center).norm() / vpm;
    let forward = V3::new(d.yaw.cos(), d.yaw.sin(), 0.0);
    let k = size(h.look).sqrt();
    let gargoyle = gargoyles::is_gargoyle(h.look);
    let chest = feet + V3::new(0.0, 0.0, 1.1 * vpm);
    d.aloft += h.dt;
    match d.state {
        State::Rest => {
            if d.airborne {
                // Soaring round home, as a skyhunter does a while, then back down onto it
                goal.to = boss.home + V3::new(0.0, 0.0, (5.0 * k) * vpm + h.stand);
                goal.speed = fl.cruise * 0.7;
                let soaring = h.look == Look::Skyhunter && d.aloft < d.budget;
                if soaring {
                    d.orbit += d.round * fl.cruise * 0.6 / fl.orbit * h.dt;
                    let at = boss.home + V3::new(d.orbit.cos(), d.orbit.sin(), 0.0) * fl.orbit * vpm;
                    goal.to = V3::new(at.x, at.y, ground_of(h, boss.home) + fl.altitude * 1.4 * vpm);
                    *wings = WingSet { base: SPREAD, amp: 0.08, rate: fl.beat * 0.5 };
                } else if flat(d.center - boss.home).norm() < 3.0 * k * vpm {
                    d.aim = boss.home;
                    d.state = State::Alight(0.0);
                }
            } else {
                // Healing, perched at home and looking about, now and then rousing its feathers
                a.health = (a.health + a.max_health * 0.05 * h.dt).min(a.max_health);
                d.health = a.health;
                let home_off = flat(boss.home - d.center).norm() / vpm;
                if home_off > 1.5 && d.blocked < 3.0 {
                    goal.to = boss.home;
                    goal.speed = speeds(h.look).0;
                } else if d.blocked >= 3.0 {
                    d.state = State::Rise(0.0);
                }
                idle(d, h, want, wings);
                // A skyhunter takes to the air again before long, to soar over its perch; a
                // gargoyle back on its perch goes back to sleep in stone
                if h.look == Look::Skyhunter && d.aloft > d.budget {
                    d.state = State::Rise(0.0);
                    d.aloft = 0.0;
                    d.budget = 14.0 + 10.0 * d.roll();
                }
                if gargoyle && home_off <= 1.5 && d.aloft > 4.0 {
                    d.state = State::Stone(-1.0);
                }
            }
        }
        State::Statue => {
            // Still as the stone it's made of
            *want = gargoyles::statue(&h.sp);
            *wings = WingSet { base: want.wings[0], amp: 0.0, rate: 0.0 };
            d.stone = 1.0;
        }
        State::Wake(t) => {
            // Shuddering, the stone cracking off it in flakes; its eyes catching fire, its head
            // coming up to look at the player, its wings breaking out of their stone and spread
            let t = t + h.dt;
            let rest = gargoyles::statue(&h.sp);
            let f = (t / gargoyles::WAKE).clamp(0.0, 1.0);
            *want = rest;
            if t > 0.0 {
                let shake = (1.0 - f) * 0.06 * (d.clock * 47.0).sin();
                want.roll += shake;
                want.head[0] += shake * 2.0;
                for (j, at) in [(0, 0.08), (1, 0.38), (2, 0.66)] {
                    let when = at * gargoyles::WAKE;
                    if t >= when && t - h.dt < when {
                        act_out(boss, Act::Crumble { at: d.bones[[TORSO, HEAD, WING][j]].at, size: size(h.look) });
                    }
                }
                if f > 0.35 {
                    look_at(d, want, chest);
                    want.head[1] += 0.15;
                }
                if f > 0.55 {
                    let spread = smooth((f - 0.55) / 0.3);
                    let open = WingPose { flap: 0.9, sweep: -0.1, twist: 0.1, fold: 0.15, bend: 0.2 };
                    for w in &mut want.wings {
                        *w = lerp_wing(*w, open, spread);
                    }
                    want.jaw = 0.8 * spread;
                    want.pitch += 0.15 * spread;
                }
            }
            d.stone = 1.0 - smooth((t - 0.1 * gargoyles::WAKE) / (0.6 * gargoyles::WAKE));
            *wings = WingSet { base: want.wings[0], amp: if f > 0.85 { 0.15 } else { 0.0 }, rate: fl.beat };
            if t >= gargoyles::WAKE * 0.72 && t - h.dt < gargoyles::WAKE * 0.72 {
                act_out(boss, Act::Shriek { at: d.mouth, brood: None, first: true, wake: !is_monster(h.look) });
            }
            d.state = if t >= gargoyles::WAKE {
                d.stone = 0.0;
                d.aloft = 0.0;
                d.budget = pick_budget(d, h, true);
                d.next = 0.3 + 0.5 * d.roll();
                // Straight down off its perch at them, or at them on foot if they're near
                if distance > 7.0 && d.center.z > ground_of(h, feet) + 2.0 * vpm { State::Rise(0.3) } else { State::Hunt }
            } else {
                State::Wake(t)
            };
        }
        State::Hunt => {
            let stage = boss.stage;
            let hurt = d.recent > a.max_health * if is_monster(h.look) { 0.12 } else { 0.05 };
            // A gargoyle hurt that badly wants to turn to stone: down to the ground to, if it's flying
            if gargoyle && hurt && d.cool[Harden as usize] <= 0.0 {
                d.harden = true;
            }
            let hurt = hurt || d.harden;
            if d.airborne {
                // Circling over the player, a pack round them each its own way, high and low
                let slot = boss.flock.slot as f64;
                let orbit = fl.orbit * (0.85 + 0.3 * (slot * 0.7).sin().abs());
                d.orbit += d.round * fl.cruise / orbit * h.dt;
                let round = V3::new(d.orbit.cos(), d.orbit.sin(), 0.0) * orbit * vpm;
                let at = feet + round;
                let high = fl.altitude + 1.2 * (slot % 3.0);
                goal.to = V3::new(at.x, at.y, ground_of(h, feet).max(ground_here) + high * vpm);
                goal.speed = fl.cruise;
                if distance > fl.orbit * 2.0 {
                    goal.to = feet + V3::new(0.0, 0.0, fl.altitude * vpm);
                }
                d.next -= h.dt;
                if d.next <= 0.0 && boss.engaged
                    && let Some(skill) = choose(d, h, stage, distance, hurt)
                {
                    begin(d, h, skill.m, false);
                }
                if (d.aloft > d.budget || d.harden) && boss.engaged && matches!(d.state, State::Hunt) {
                    begin_landing(d, h);
                }
            } else {
                // On foot: up to the player, facing them, then whatever's in reach
                let close = strike_reach(h) + 0.4;
                goal.face = Some(feet);
                if distance > close {
                    goal.to = feet;
                    goal.speed = if distance > close + 3.0 * k { speeds(h.look).1 } else { speeds(h.look).0 };
                }
                look_at(d, want, chest);
                if gargoyle {
                    // Prowling low, its wings half open, arms out
                    want.pitch -= 0.15;
                    for w in &mut want.wings {
                        w.flap += 0.35;
                        w.sweep -= 0.35;
                        w.fold -= 0.8;
                    }
                }
                let facing = turn_to(d.yaw, flat(feet - d.center).y.atan2(flat(feet - d.center).x)).abs() < 0.7;
                d.next -= h.dt;
                if d.next <= 0.0 && facing && boss.engaged
                    && let Some(skill) = choose(d, h, stage, distance, hurt)
                {
                    begin(d, h, skill.m, false);
                }
                // Up again once it's had enough of the ground, or the player's out of reach
                let unreachable = (feet.z - ground_here).abs() > 3.5 * vpm || d.blocked > 1.5;
                if matches!(d.state, State::Hunt) && (d.aloft > d.budget || unreachable || distance > 16.0 * k) {
                    d.state = State::Rise(0.0);
                }
            }
        }
        State::Rise(t) => {
            let crouch = 0.25 * k;
            // Crouching, wings raised, then springing up into the air beating them hard
            *wings = WingSet { base: WingPose { flap: 1.0, sweep: -0.1, twist: 0.1, fold: 0.2, bend: 0.1 }, amp: 0.1, rate: 0.6 * fl.beat };
            if t < crouch && !d.airborne {
                for leg in &mut want.legs {
                    leg.knee += 0.45;
                    leg.swing += 0.25;
                }
                want.pitch += 0.1;
                goal.face = h.feet;
            } else {
                if !d.airborne {
                    d.airborne = true;
                    d.blocked = 0.0;
                    d.velocity = V3::new(forward.x, forward.y, 0.0) * 2.0 * vpm + V3::new(0.0, 0.0, 6.0 * k.powf(0.4) * vpm);
                    act_out(boss, Act::TakeOff { at: V3::new(d.center.x, d.center.y, d.foot_z), size: size(h.look) });
                }
                *wings = WingSet { base: WingPose { flap: 0.2, sweep: 0.0, twist: 0.1, fold: 0.05, bend: 0.0 }, amp: 0.9, rate: fl.beat * 1.6 };
                goal.to = d.center + forward * 3.0 * vpm + V3::new(0.0, 0.0, 7.0 * vpm);
                goal.speed = fl.cruise * 0.6;
                // Legs trailing as it climbs, tail spread
                want.tail[1] -= 0.25;
            }
            let height = (d.center.z - ground_here) / vpm;
            d.state = if d.airborne && (height > fl.altitude * 0.6 || t > 2.4 * k) {
                d.aloft = 0.0;
                d.budget = pick_budget(d, h, true);
                d.next = d.next.max(0.5);
                if boss.engaged { State::Hunt } else { State::Rest }
            } else {
                State::Rise(t + h.dt)
            };
        }
        State::Alight(t) => {
            // Down to the spot picked, flaring its wings to slow itself, talons out to land
            let spot = d.aim;
            let over = flat(spot - d.center).norm() / vpm;
            let height = (d.center.z - spot.z - h.stand) / vpm;
            let up = if over > 3.0 * k { height.min(over * 0.5).max(0.0) } else { 0.0 };
            goal.to = V3::new(spot.x, spot.y, spot.z + h.stand + up * vpm);
            goal.speed = if over > 3.0 * k { fl.cruise * 0.75 } else { 3.0 * k };
            goal.low = true;
            goal.face = Some(h.feet.filter(|_| boss.engaged).unwrap_or(spot + forward * vpm));
            if over < 5.0 * k {
                // Flaring: body up, wings forward and beating hard, tail fanned down, legs out
                *wings = WingSet { base: WingPose { flap: 0.4, sweep: -0.45, twist: -0.45, fold: 0.15, bend: 0.1 }, amp: 0.75, rate: fl.beat * 1.5 };
                want.pitch += 0.75;
                want.head[1] -= 0.55;
                want.tail[1] -= 0.6;
                for leg in &mut want.legs {
                    leg.swing += 1.0;
                    leg.knee = 0.4;
                }
            }
            if (over < 1.2 * k && height <= 0.15 * s) || t > 7.0 {
                d.airborne = false;
                d.velocity = V3::zeros();
                d.center.z = spot.z + h.stand;
                d.foot_z = spot.z;
                act_out(boss, Act::Land { at: spot, size: size(h.look) });
                d.aloft = 0.0;
                d.budget = pick_budget(d, h, false);
                d.state = if boss.engaged { State::Spent(0.3 * k) } else { State::Rest };
            } else {
                d.state = State::Alight(t + h.dt);
            }
        }
        State::Wind(m, t) => {
            let skill = d.skill.unwrap_or(skill(m, d.airborne, 0.0, 1.0, 1.0, 0.6, 0.5, 0, 1.0));
            let wind = skill.wind * k.powf(0.3) * h.quick.max(0.8);
            let toward = flat(feet - d.center).try_normalize(1e-6).unwrap_or(forward);
            let mut t = t;
            match m {
                Rake => {
                    // Rising a little and slowing, wings raised high, looking down at the player
                    if t == 0.0 {
                        let low = chest - toward * 0.5 * vpm - V3::new(0.0, 0.0, 0.3 * vpm);
                        let exit = feet + toward * (10.0 * k) * vpm + V3::new(0.0, 0.0, fl.altitude * vpm);
                        d.path = [d.center, V3::new(low.x, low.y, low.z.max(ground_of(h, low) + h.stand * 0.5)), exit];
                    }
                    goal.to = d.center + V3::new(0.0, 0.0, 1.2 * vpm);
                    goal.speed = 3.0;
                    goal.face = Some(feet);
                    *wings = WingSet { base: WingPose { flap: 1.0, sweep: 0.1, twist: 0.0, fold: 0.15, bend: 0.1 }, amp: 0.12, rate: fl.beat };
                    look_at(d, want, feet);
                    let low = d.path[1];
                    mark(boss, V3::new(low.x, low.y, ground_of(h, low)), rake_r, 1.0 - 0.6 * t / wind);
                }
                Stoop => {
                    // Climbing over where the player stands, which it'll fall on, the ring showing it
                    goal.to = V3::new(d.aim.x, d.aim.y, ground_of(h, d.aim) + (fl.altitude + 5.0 * k) * vpm);
                    goal.speed = fl.cruise * 0.85;
                    goal.face = Some(d.aim);
                    *wings = WingSet { base: WingPose { flap: 0.5, sweep: 0.0, twist: 0.0, fold: 0.1, bend: 0.1 }, amp: 0.8, rate: fl.beat * 1.3 };
                    if t < wind * 0.6 {
                        d.aim = feet;
                    }
                    look_at(d, want, d.aim);
                    mark(boss, V3::new(d.aim.x, d.aim.y, ground_of(h, d.aim)), impact_r, 1.0 - 0.7 * t / wind);
                }
                Snatch => {
                    // Out to where the run starts first, low on the far side, then lining up
                    let height = 1.6 + h.stand / vpm * 0.8;
                    let start = feet - d.way * 26.0 * vpm;
                    if !d.ready {
                        goal.to = V3::new(start.x, start.y, ground_of(h, start).max(feet.z) + height * vpm + 3.0 * vpm);
                        goal.speed = fl.cruise * 1.1;
                        t = 0.0;
                        if flat(d.center - start).norm() < 5.0 * vpm || d.aloft > d.budget + 6.0 {
                            d.ready = true;
                            d.way = toward;
                            d.aim = feet;
                        }
                    } else {
                        goal.to = d.center + d.way * 2.0 * vpm;
                        goal.speed = 4.0;
                        goal.face = Some(feet);
                        look_at(d, want, chest);
                        // Talons drawn up to strike, wings high
                        *wings = WingSet { base: WingPose { flap: 0.9, sweep: 0.2, twist: 0.0, fold: 0.2, bend: 0.1 }, amp: 0.2, rate: fl.beat };
                        for leg in &mut want.legs {
                            leg.swing = 0.4;
                            leg.knee = 1.4;
                        }
                    }
                }
                Claw if d.airborne => {
                    // In beside the player, just over their head, beating its wings hard
                    let side = V3::new(-toward.y, toward.x, 0.0) * d.round;
                    let at = feet - toward * 1.4 * vpm + side * 1.0 * vpm;
                    goal.to = V3::new(at.x, at.y, ground_of(h, feet) + h.stand + 1.3 * vpm);
                    goal.speed = fl.cruise * 0.6;
                    goal.face = Some(feet);
                    goal.low = true;
                    *wings = WingSet { base: WingPose { flap: 0.25, sweep: -0.25, twist: -0.3, fold: 0.1, bend: 0.1 }, amp: 0.85, rate: fl.beat * 1.5 };
                    want.pitch += 0.45;
                    if !d.ready {
                        t = 0.0;
                        if (d.center - goal.to).norm() < 1.2 * vpm || d.aloft > d.budget + 6.0 {
                            d.ready = true;
                        }
                    } else {
                        for leg in &mut want.legs {
                            leg.swing += 0.9 * t / wind;
                            leg.knee -= 0.5 * t / wind;
                        }
                        reach_back(want, &h.sp, t / wind);
                        mark(boss, V3::new(feet.x, feet.y, ground_of(h, feet)), claw_r, 1.0 - t / wind);
                    }
                }
                Claw => {
                    // Rearing back, claws or a foot raised
                    goal.face = Some(feet);
                    look_at(d, want, chest);
                    reach_back(want, &h.sp, t / wind);
                    want.pitch += 0.12 * t / wind;
                    let at = d.center + forward * strike_reach(h) * vpm;
                    mark(boss, V3::new(at.x, at.y, ground_of(h, at)), claw_r, 1.0 - t / wind);
                }
                Peck => {
                    goal.face = Some(feet);
                    look_at(d, want, chest);
                    // Its head drawn back and up, beak open
                    want.head[1] += 0.5 * t / wind;
                    want.pitch += 0.15 * t / wind;
                    want.jaw = 0.4 * t / wind;
                    let at = d.center + forward * strike_reach(h) * 1.1 * vpm;
                    mark(boss, V3::new(at.x, at.y, ground_of(h, at)), peck_r, 1.0 - t / wind);
                }
                Pounce => {
                    // Crouching low, wings half raised, its eyes on where it'll land
                    goal.face = Some(feet);
                    look_at(d, want, chest);
                    let f = t / wind;
                    want.pitch -= 0.25 * f;
                    for leg in &mut want.legs {
                        leg.knee += 0.5 * f;
                        leg.swing += 0.35 * f;
                    }
                    for w in &mut want.wings {
                        *w = lerp_wing(*w, WingPose { flap: 0.8, sweep: 0.3, twist: -0.3, fold: 0.8, bend: 0.2 }, f);
                    }
                    for arm in &mut want.arms {
                        arm.swing += 0.6 * f;
                        arm.spread += 0.3 * f;
                    }
                    if t < wind * 0.7 {
                        d.aim = feet;
                    }
                    mark(boss, V3::new(d.aim.x, d.aim.y, ground_of(h, d.aim)), claw_r * 1.1, 1.0 - f);
                }
                Gust => {
                    goal.face = Some(feet);
                    // Wings raised high and swept forward, rearing
                    *wings = WingSet { base: WingPose { flap: 1.25, sweep: -0.35, twist: 0.2, fold: 0.1, bend: 0.25 }, amp: 0.05, rate: 1.0 };
                    want.pitch += 0.3;
                    look_at(d, want, chest);
                    if d.airborne {
                        goal.to = d.center;
                        goal.speed = 2.0;
                    }
                    let ground = V3::new(d.center.x, d.center.y, ground_here);
                    for j in 1..4 {
                        let p = ground + forward * gust_r * (j as f64 / 3.5) * vpm;
                        mark(boss, V3::new(p.x, p.y, ground_of(h, p)), 0.7 + 0.45 * j as f64 * k.powf(0.5), 1.0 - t / wind);
                    }
                }
                Volley => {
                    goal.face = Some(feet);
                    look_at(d, want, chest);
                    // Wings drawn back to fling them, or its arms
                    let f = t / wind;
                    *wings = WingSet { base: WingPose { flap: 0.7, sweep: 0.75 * f, twist: -0.3, fold: 0.5 * f, bend: 0.1 }, amp: 0.1, rate: fl.beat };
                    if d.airborne {
                        goal.to = d.center + V3::new(0.0, 0.0, 0.5 * vpm);
                        goal.speed = 2.0;
                    }
                }
                Lightning => {
                    // Rearing up with its wings spread high, the storm gathering, crackling
                    *wings = WingSet { base: WingPose { flap: 1.15, sweep: -0.15, twist: 0.0, fold: 0.05, bend: 0.2 }, amp: 0.25, rate: fl.beat * 1.4 };
                    want.head[1] += 0.6;
                    want.jaw = 0.6 * t / wind;
                    if d.airborne {
                        goal.to = V3::new(feet.x, feet.y, ground_of(h, feet) + (fl.altitude + 4.0) * vpm) + V3::new(d.orbit.cos(), d.orbit.sin(), 0.0) * fl.orbit * 0.5 * vpm;
                        goal.speed = fl.cruise * 0.6;
                    }
                    if (d.clock * 9.0).fract() < h.dt * 9.0 {
                        act_out(boss, Act::Spark { at: d.mouth });
                    }
                }
                Arc => {
                    goal.face = Some(feet);
                    look_at(d, want, chest);
                    want.head[1] += 0.3 * (t / wind);
                    want.jaw = 0.7 * t / wind;
                    if (d.clock * 14.0).fract() < h.dt * 14.0 {
                        act_out(boss, Act::Spark { at: d.mouth });
                    }
                    if d.airborne {
                        goal.to = d.center + toward * vpm;
                        goal.speed = 3.0;
                    }
                    // Held where it was aimed for the last of it, so it can be stepped out of
                    if t < wind - 0.35 {
                        d.aim = chest;
                    }
                    mark(boss, V3::new(d.aim.x, d.aim.y, ground_of(h, d.aim)), 1.3, 1.0 - t / wind);
                }
                Shriek => {
                    want.head[1] += 0.55;
                    want.jaw = 0.9 * t / wind;
                    if d.airborne {
                        goal.to = d.center;
                        goal.speed = 3.0;
                    } else {
                        *wings = WingSet { base: WingPose { flap: 0.85, sweep: 0.05, twist: 0.0, fold: 0.5, bend: 0.15 }, amp: 0.05, rate: 1.0 };
                    }
                }
                Harden => {
                    // Down from the air first, to stand on
                    if d.airborne {
                        begin_landing(d, h);
                        return;
                    }
                    let f = t / wind;
                    *want = lerp_rig(want, &gargoyles::wrapped(&h.sp), f);
                }
                Shatter => {
                    goal.face = Some(feet);
                    // Rearing up, arms high overhead, wings spread
                    let f = t / wind;
                    want.pitch += 0.4 * f;
                    for arm in &mut want.arms {
                        arm.swing += 2.2 * f;
                        arm.spread += 0.2 * f;
                        arm.elbow = 0.3;
                    }
                    for w in &mut want.wings {
                        *w = lerp_wing(*w, WingPose { flap: 0.9, sweep: -0.1, twist: 0.0, fold: 0.1, bend: 0.2 }, f);
                    }
                    want.jaw = 0.7 * f;
                    let at = d.center + forward * strike_reach(h) * vpm;
                    mark(boss, V3::new(at.x, at.y, ground_of(h, at)), impact_r, 1.0 - f);
                    mark(boss, V3::new(at.x, at.y, ground_of(h, at)), impact_r * 3.0, 1.0 - f);
                }
            }
            if t == 0.0 && (d.ready || !matches!(m, Snatch) && !(m == Claw && d.airborne)) {
                act_out(boss, Act::Warn { m, at: d.mouth });
            }
            let t = t + h.dt;
            d.state = if t >= wind && (d.ready || !matches!(m, Snatch) && !(m == Claw && d.airborne)) {
                d.struck = false;
                if let Some(skill) = d.skill {
                    d.cool[skill.m as usize] = skill.cool * h.quick;
                }
                if m == Rake {
                    d.path[0] = d.center;
                }
                if m == Pounce {
                    d.path = [d.center, d.aim, d.aim];
                    d.airborne = true;
                    act_out(boss, Act::TakeOff { at: V3::new(d.center.x, d.center.y, d.foot_z), size: size(h.look) });
                }
                if m == Harden {
                    d.harden = false;
                    d.state = State::Stone(0.0);
                    act_out(boss, Act::Petrify { at: d.center, size: size(h.look) });
                    return;
                }
                State::Do(m, 0.0)
            } else {
                State::Wind(m, t)
            };
        }
        State::Do(m, t) => {
            let t = t + h.dt;
            let done: bool;
            let toward = flat(feet - d.center).try_normalize(1e-6).unwrap_or(forward);
            match m {
                Rake => {
                    // Down along the curve through where it comes lowest, and away up again
                    let [from, low, exit] = d.path;
                    let control = low * 2.0 - (from + exit) * 0.5;
                    let length = ((low - from).norm() + (exit - low).norm()) / (fl.dash * vpm);
                    let u = (t / length.max(0.3)).min(1.0);
                    let at = from * (1.0 - u) * (1.0 - u) + control * 2.0 * u * (1.0 - u) + exit * u * u;
                    let along = (control - from) * 2.0 * (1.0 - u) + (exit - control) * 2.0 * u;
                    d.velocity = along / length.max(0.3);
                    let floor = ground_of(h, at) + h.stand * 0.5;
                    d.center = V3::new(at.x, at.y, at.z.max(floor));
                    goal.to = d.center;
                    goal.speed = -1.0;
                    goal.low = true;
                    // Wings swept back for the dive, a gargoyle's held spread to glide, beating for
                    // the climb; talons thrown forward low down
                    *wings = if u < 0.45 {
                        if gargoyle { WingSet { base: WingPose { flap: 0.2, sweep: 0.3, twist: 0.0, fold: 0.2, bend: 0.0 }, amp: 0.0, rate: 1.0 } } else { WingSet { base: WingPose { flap: 0.35, sweep: 0.8, twist: 0.1, fold: 0.9, bend: 0.0 }, amp: 0.05, rate: 1.0 } }
                    } else {
                        WingSet { base: SPREAD, amp: 0.85, rate: fl.beat * 1.4 }
                    };
                    if (0.3..0.65).contains(&u) {
                        strike_pose(want, &h.sp);
                        want.jaw = 0.6;
                    }
                    if !d.struck {
                        mark(boss, V3::new(low.x, low.y, ground_of(h, low)), rake_r, 0.4 * (1.0 - u / 0.5));
                    }
                    if !d.struck && u >= 0.5 {
                        d.struck = true;
                        act_out(boss, Act::Strike { m: Rake, at: low - V3::new(0.0, 0.0, h.stand * 0.6), radius: rake_r, damage: pw.rake });
                    }
                    done = u >= 1.0;
                }
                Stoop => {
                    // Wings folded tight, falling on the ring head first
                    let to = V3::new(d.aim.x, d.aim.y, ground_of(h, d.aim) + h.stand);
                    let off = to - d.center;
                    d.velocity = off.try_normalize(1e-6).unwrap_or_else(|| -V3::z()) * fl.dash * vpm;
                    goal.to = to;
                    goal.speed = -1.0;
                    d.center += d.velocity * h.dt;
                    *wings = WingSet { base: WingPose { flap: 0.5, sweep: 1.05, twist: -0.2, fold: 1.6, bend: 0.0 }, amp: 0.0, rate: 1.0 };
                    want.pitch -= 0.9;
                    want.head[1] += 0.3;
                    let near = off.norm() / vpm;
                    if near < 4.0 * k {
                        strike_pose(want, &h.sp);
                    }
                    mark(boss, V3::new(d.aim.x, d.aim.y, ground_of(h, d.aim)), impact_r, (near / (fl.altitude * 2.0)).min(0.3));
                    if off.norm() < fl.dash * vpm * h.dt * 1.5 || d.center.z <= to.z || t > 4.0 {
                        let ground = V3::new(to.x, to.y, to.z - h.stand);
                        act_out(boss, Act::Impact { at: ground, radius: impact_r, damage: pw.stoop });
                        if h.look == Look::BloodwakeGargoyle {
                            act_out(boss, Act::Shards { at: ground, count: 10, damage: pw.shard });
                        }
                        let lands = matches!(h.look, Look::Roc | Look::Grotesque | Look::BloodwakeGargoyle) || gargoyle;
                        if lands {
                            // Coming down hard, and dazed a moment by it: the moment to get at it
                            d.center = to;
                            d.airborne = false;
                            d.velocity = V3::zeros();
                            d.foot_z = ground.z;
                            act_out(boss, Act::Land { at: ground, size: size(h.look) });
                            d.aloft = 0.0;
                            d.budget = pick_budget(d, h, false);
                            d.state = State::Spent(2.0 * k.powf(0.3));
                            d.skill = None;
                            return;
                        }
                        // Pulling out of it at the bottom, and away
                        d.center = V3::new(to.x, to.y, to.z + 0.5 * vpm);
                        d.velocity = V3::new(d.way.x, d.way.y, 0.0) * fl.dash * 0.7 * vpm + V3::new(0.0, 0.0, 4.0 * vpm);
                        done = true;
                    } else {
                        done = false;
                    }
                }
                Snatch => {
                    match d.holding {
                        None => {
                            // Low and fast through where the player is, talons out
                            let target = V3::new(chest.x, chest.y, chest.z + h.stand * 0.85);
                            let off = target - d.center;
                            goal.to = target + d.way * 6.0 * vpm;
                            goal.speed = fl.dash;
                            goal.low = true;
                            goal.face = Some(target + d.way * 10.0 * vpm);
                            *wings = WingSet { base: WingPose { flap: 0.3, sweep: 0.35, twist: 0.0, fold: 0.4, bend: 0.0 }, amp: 0.3, rate: fl.beat * 1.2 };
                            strike_pose(want, &h.sp);
                            let talons = talons_of(&h.sp, &d.bones, vpm);
                            let reach = (talons - chest).norm() / vpm;
                            if reach < 2.2 + 0.25 * k && h.feet.is_some() {
                                d.holding = Some(0.0);
                                act_out(boss, Act::Grab { at: chest, damage: pw.claw });
                                done = false;
                            } else if off.dot(&d.way) < -2.0 * vpm || t > 4.0 {
                                // Missed: raking past instead, and away
                                act_out(boss, Act::Strike { m: Rake, at: V3::new(talons.x, talons.y, ground_of(h, talons)), radius: rake_r, damage: pw.rake });
                                done = true;
                            } else {
                                done = false;
                            }
                        }
                        Some(held) => {
                            // Climbing with them, beating hard, then letting go high over the ground,
                            // or sooner if it's hit hard enough
                            let held = held + h.dt;
                            d.holding = Some(held);
                            goal.to = d.center + d.way * 4.0 * vpm + V3::new(0.0, 0.0, 6.0 * vpm);
                            goal.speed = fl.cruise * 0.8;
                            *wings = WingSet { base: WingPose { flap: 0.15, sweep: 0.0, twist: 0.1, fold: 0.05, bend: 0.0 }, amp: 0.95, rate: fl.beat * 1.7 };
                            for leg in &mut want.legs {
                                leg.swing = 0.1;
                                leg.knee = 0.9;
                            }
                            let talons = talons_of(&h.sp, &d.bones, vpm);
                            act_out(boss, Act::Carry { to: talons - V3::new(0.0, 0.0, 1.4 * vpm) });
                            let high = (d.center.z - ground_here) / vpm;
                            let shaken = d.recent > a.max_health * 0.04 && held > 0.6;
                            if held > 2.6 || high > 22.0 || shaken {
                                act_out(boss, Act::Drop { at: talons });
                                d.holding = None;
                                done = true;
                            } else {
                                done = false;
                            }
                        }
                    }
                }
                Claw if d.airborne => {
                    let length = 0.3;
                    goal.to = d.center;
                    goal.speed = 1.0;
                    goal.face = Some(feet);
                    goal.low = true;
                    *wings = WingSet { base: WingPose { flap: 0.25, sweep: -0.25, twist: -0.3, fold: 0.1, bend: 0.1 }, amp: 0.85, rate: fl.beat * 1.5 };
                    want.pitch += 0.4;
                    strike_pose(want, &h.sp);
                    if !d.struck && t >= length * 0.4 {
                        d.struck = true;
                        act_out(boss, Act::Strike { m: Claw, at: V3::new(feet.x, feet.y, ground_of(h, feet)), radius: claw_r, damage: pw.claw });
                    }
                    done = t >= length;
                }
                Claw => {
                    let length = 0.28 * k.powf(0.3);
                    goal.to = d.center + forward * 2.0 * vpm;
                    goal.speed = 1.5 * k;
                    goal.face = Some(feet);
                    swipe(want, &h.sp, (t / length).min(1.0), d.again);
                    if !d.struck && t >= length * 0.45 {
                        d.struck = true;
                        let at = d.center + forward * strike_reach(h) * vpm;
                        act_out(boss, Act::Strike { m: Claw, at: V3::new(at.x, at.y, ground_of(h, at)), radius: claw_r, damage: pw.claw });
                    }
                    done = t >= length;
                    if done && d.again > 1 {
                        d.again -= 1;
                        d.state = State::Wind(Claw, d.skill.map_or(0.0, |s| s.wind * 0.5));
                        d.struck = false;
                        return;
                    }
                }
                Peck => {
                    let length = 0.25 * k.powf(0.3);
                    goal.face = Some(feet);
                    // Its head driven down and forward, the body behind it
                    let f = (t / length).min(1.0);
                    want.head[1] -= 0.9 * (PI * f).sin();
                    want.pitch -= 0.35 * (PI * f).sin();
                    want.jaw = if f < 0.45 { 0.5 } else { 0.0 };
                    if !d.struck && t >= length * 0.45 {
                        d.struck = true;
                        let at = d.center + forward * strike_reach(h) * 1.15 * vpm;
                        act_out(boss, Act::Strike { m: Peck, at: V3::new(at.x, at.y, ground_of(h, at)), radius: peck_r, damage: pw.peck });
                    }
                    done = t >= length;
                }
                Pounce => {
                    // Leaping in an arc onto where it was aimed, wings spread, claws out
                    let [from, to, _] = d.path;
                    let span = flat(to - from).norm() / vpm;
                    let length = (span / (fl.cruise * 1.2)).clamp(0.45, 1.3);
                    let u = (t / length).min(1.0);
                    let high = (0.6 + span * 0.18) * vpm;
                    let target_z = ground_of(h, to) + h.stand;
                    let base = from + (V3::new(to.x, to.y, target_z) - from) * u;
                    d.center = base + V3::new(0.0, 0.0, high * 4.0 * u * (1.0 - u));
                    d.velocity = (to - from) / length;
                    goal.to = d.center;
                    goal.speed = -1.0;
                    goal.low = true;
                    goal.face = Some(to + toward * vpm);
                    *wings = WingSet { base: WingPose { flap: 0.6, sweep: 0.0, twist: 0.0, fold: 0.15, bend: 0.1 }, amp: 0.2, rate: fl.beat };
                    strike_pose(want, &h.sp);
                    if u >= 1.0 {
                        d.airborne = false;
                        d.velocity = V3::zeros();
                        d.foot_z = target_z - h.stand;
                        let ground = V3::new(d.center.x, d.center.y, d.foot_z);
                        act_out(boss, Act::Impact { at: ground, radius: claw_r * 1.2, damage: pw.claw * 1.2 });
                        act_out(boss, Act::Land { at: ground, size: size(h.look) });
                        d.aloft = 0.0;
                        done = true;
                    } else {
                        done = false;
                    }
                }
                Gust => {
                    let length = 0.45 * k.powf(0.3);
                    // Beating down hard, the ground blown clear before it
                    *wings = WingSet { base: WingPose { flap: -0.55, sweep: 0.05, twist: -0.1, fold: 0.05, bend: -0.2 }, amp: 0.05, rate: 1.0 };
                    if !d.struck && t >= length * 0.3 {
                        d.struck = true;
                        let at = V3::new(d.center.x, d.center.y, ground_here.max(feet.z - 2.0 * vpm));
                        act_out(boss, Act::Gust { at, dir: toward, reach: gust_r, push: pw.push, damage: pw.gust });
                    }
                    done = t >= length;
                    if done && d.again > 1 {
                        d.again -= 1;
                        d.state = State::Wind(Gust, d.skill.map_or(0.0, |s| s.wind * 0.55));
                        d.struck = false;
                        return;
                    }
                }
                Volley => {
                    let length = 0.3;
                    *wings = WingSet { base: WingPose { flap: 0.3, sweep: -0.4, twist: 0.1, fold: 0.0, bend: 0.0 }, amp: 0.1, rate: fl.beat };
                    if !d.struck {
                        d.struck = true;
                        let (count, spread) = match h.look {
                            Look::Harpy => (3, 1.6),
                            Look::Harridan => (5, 2.6),
                            _ => (7, 3.6),
                        };
                        let from = d.center + forward * 0.4 * s * vpm + V3::new(0.0, 0.0, 0.2 * s * vpm);
                        act_out(boss, Act::Feathers { from, to: chest, count, spread, speed: 24.0, damage: pw.feather });
                    }
                    done = t >= length;
                    if done && d.again > 1 {
                        d.again -= 1;
                        d.state = State::Wind(Volley, d.skill.map_or(0.0, |s| s.wind * 0.6));
                        d.struck = false;
                        return;
                    }
                }
                Lightning => {
                    let (length, every, spread) = if boss.stage >= 2 { (3.2, 0.2, 14.0) } else { (2.4, 0.3, 10.0) };
                    *wings = WingSet { base: WingPose { flap: 1.0, sweep: -0.1, twist: 0.0, fold: 0.05, bend: 0.2 }, amp: 0.4, rate: fl.beat * 1.2 };
                    want.head[1] += 0.5;
                    want.jaw = 0.8;
                    if d.airborne {
                        d.orbit += d.round * fl.cruise / fl.orbit * h.dt * 0.6;
                        goal.to = V3::new(feet.x, feet.y, ground_of(h, feet) + (fl.altitude + 4.0) * vpm) + V3::new(d.orbit.cos(), d.orbit.sin(), 0.0) * fl.orbit * 0.5 * vpm;
                        goal.speed = fl.cruise * 0.6;
                    }
                    let count = |t: f64| (t / every) as usize;
                    if count(t) != count(t - h.dt) {
                        let n = count(t);
                        // The first right where the player is, the rest round them, closing in
                        let at = if n % 3 == 0 {
                            feet
                        } else {
                            let a = d.roll() * TAU;
                            let r = (2.0 + (spread - 2.0) * d.roll().sqrt()) * vpm;
                            feet + V3::new(a.cos() * r, a.sin() * r, 0.0)
                        };
                        let at = V3::new(at.x, at.y, ground_of(h, at + V3::new(0.0, 0.0, 6.0 * vpm)));
                        act_out(boss, Act::Lightning { to: at, radius: 2.2, damage: pw.bolt, delay: 0.85 });
                    }
                    done = t >= length;
                }
                Arc => {
                    want.jaw = 0.9;
                    want.head[1] -= 0.1;
                    if !d.struck {
                        d.struck = true;
                        act_out(boss, Act::Arc { from: d.mouth, to: d.aim, damage: pw.bolt });
                        // The thunderbird's forks: two more either side
                        if h.look == Look::Thunderbird {
                            let side = V3::new(-toward.y, toward.x, 0.0) * 3.0 * vpm;
                            for sign in [-1.0, 1.0] {
                                act_out(boss, Act::Arc { from: d.mouth, to: d.aim + side * sign, damage: pw.bolt * 0.7 });
                            }
                        }
                    }
                    done = t >= 0.3;
                }
                Shriek => {
                    let length = 1.1;
                    want.head[1] += 0.6;
                    want.jaw = 1.0;
                    *wings = if d.airborne { *wings } else { WingSet { base: WingPose { flap: 0.95, sweep: 0.0, twist: 0.0, fold: 0.3, bend: 0.15 }, amp: 0.05, rate: 1.0 } };
                    if d.airborne {
                        goal.to = d.center;
                        goal.speed = 2.0;
                    }
                    if !d.struck {
                        d.struck = true;
                        let calls = brood(h.look).filter(|_| d.stage >= 1 || matches!(h.look, Look::Harridan)).map(|b| (b, if is_raid(h.look) { 3 } else { 2 }));
                        let wake = gargoyle && !is_monster(h.look);
                        act_out(boss, Act::Shriek { at: d.mouth, brood: calls, first: false, wake });
                    }
                    done = t >= length;
                }
                Harden => done = true,
                Shatter => {
                    let length = 0.35 * k.powf(0.3);
                    let f = (t / length).min(1.0);
                    want.pitch -= 0.35 * f;
                    for arm in &mut want.arms {
                        arm.swing += 1.2 - 1.6 * f;
                        arm.elbow = 0.2;
                    }
                    if !d.struck && t >= length * 0.4 {
                        d.struck = true;
                        let at = d.center + forward * strike_reach(h) * vpm;
                        let ground = V3::new(at.x, at.y, ground_of(h, at));
                        act_out(boss, Act::Impact { at: ground, radius: impact_r, damage: pw.stoop });
                        act_out(boss, Act::Shards { at: ground, count: if h.look == Look::BloodwakeGargoyle { 14 } else { 9 }, damage: pw.shard });
                    }
                    done = t >= length + 0.2;
                }
            }
            d.state = if done {
                let rest = d.skill.map_or(0.5, |s| s.rest) * h.quick * k.powf(0.3);
                d.skill = None;
                // A monster comes down after a rake now and then, to fight on foot
                if m == Rake && is_monster(h.look) && d.roll() < 0.3 && d.aloft > d.budget * 0.4 {
                    begin_landing(d, h);
                    if matches!(d.state, State::Alight(_)) {
                        return;
                    }
                }
                State::Spent(rest)
            } else {
                State::Do(m, t)
            };
        }
        State::Spent(t) => {
            if d.airborne {
                goal.to = d.center + forward * 2.0 * vpm + V3::new(0.0, 0.0, 0.6 * vpm);
                goal.speed = fl.cruise * 0.5;
            } else {
                goal.face = None;
                if gargoyle {
                    want.pitch -= 0.1;
                }
            }
            let t = t - h.dt;
            d.state = if t > 0.0 {
                State::Spent(t)
            } else {
                // A pack takes turns, so they don't all come at once
                let gap = if is_monster(h.look) { 1.8 + 1.4 * d.roll() + boss.flock.slot as f64 * 0.8 } else { 1.0 + 0.8 * d.roll() };
                d.next = gap * h.quick;
                if boss.engaged { State::Hunt } else { State::Rest }
            };
        }
        State::Stone(t) => {
            // Turned to stone, wrapped in its wings: blows glance off it till it cracks out again.
            // Back home and settled, it sleeps in its stone till someone comes near.
            let asleep = t < 0.0;
            let t = if asleep { 0.0 } else { t + h.dt };
            let wrap = if asleep { gargoyles::statue(&h.sp) } else { gargoyles::wrapped(&h.sp) };
            *want = wrap;
            *wings = WingSet { base: wrap.wings[0], amp: 0.0, rate: 0.0 };
            d.stone = (d.stone + h.dt / 0.35).min(1.0);
            let lasts = if is_monster(h.look) { 3.2 } else { 4.5 };
            if asleep {
                if d.stone >= 1.0 {
                    d.state = State::Statue;
                }
            } else if t >= lasts {
                // Cracking out of it, and straight at the player
                act_out(boss, Act::Crumble { at: d.center, size: size(h.look) });
                act_out(boss, Act::Shriek { at: d.mouth, brood: None, first: false, wake: false });
                d.stone = 0.0;
                d.recent = 0.0;
                d.next = 0.1;
                d.state = State::Spent(0.15);
            } else {
                d.state = State::Stone(t);
            }
        }
        State::Reel(t) => {
            // Falling out of the sky, wings flailing, then lying dazed where it came down
            *wings = WingSet { base: WingPose { flap: 0.4, sweep: 0.5, twist: 0.3, fold: 1.0, bend: 0.4 }, amp: 0.4, rate: fl.beat * 0.9 };
            if d.airborne {
                d.velocity.z -= 14.0 * vpm * h.dt;
                d.velocity.x *= (-1.5 * h.dt).exp();
                d.velocity.y *= (-1.5 * h.dt).exp();
                want.roll += 0.6;
                want.pitch -= 0.3;
                goal.speed = -1.0;
                d.center += d.velocity * h.dt;
                if d.center.z <= ground_here + h.stand * 0.8 {
                    d.center.z = ground_here + h.stand * 0.8;
                    d.airborne = false;
                    d.velocity = V3::zeros();
                    act_out(boss, Act::Crash { at: V3::new(d.center.x, d.center.y, ground_here), size: size(h.look) });
                }
                d.state = State::Reel(t);
            } else {
                *wings = WingSet { base: WingPose { flap: -0.1, sweep: 0.4, twist: 0.2, fold: 0.5, bend: -0.1 }, amp: 0.0, rate: 1.0 };
                want.pitch -= 0.4;
                want.roll += 0.25 * d.round;
                want.head[1] -= 0.4;
                want.jaw = 0.3;
                for leg in &mut want.legs {
                    leg.knee += 0.5;
                }
                let t = t - h.dt;
                d.state = if t > 0.0 {
                    State::Reel(t)
                } else if boss.engaged {
                    d.reeled = 12.0;
                    if gargoyle { State::Hunt } else { State::Rise(0.0) }
                } else {
                    State::Rest
                };
            }
        }
        State::Fall => {}
    }
    // Knocked out of the air by being hit hard low down: the moment for a sword
    let height = (d.center.z - ground_here) / vpm;
    let threshold = if is_monster(h.look) { 0.3 } else { 0.06 };
    if d.airborne && height < 8.0 * k && d.recent > a.max_health * threshold && d.reeled <= 0.0 && d.holding.is_none() && !matches!(d.state, State::Reel(_) | State::Do(Stoop | Pounce, _)) {
        d.recent = 0.0;
        d.reeled = if is_monster(h.look) { 6.0 } else { 16.0 };
        d.state = State::Reel(if is_monster(h.look) { 2.2 } else { 3.0 });
        d.skill = None;
    }
}

/// Between two poses of a wing, `f` of the way from `a` to `b`
fn lerp_wing(a: WingPose, b: WingPose, f: f64) -> WingPose {
    let m = |x: f64, y: f64| x + (y - x) * f;
    WingPose { flap: m(a.flap, b.flap), sweep: m(a.sweep, b.sweep), twist: m(a.twist, b.twist), fold: m(a.fold, b.fold), bend: m(a.bend, b.bend) }
}

/// Between two whole poses, `f` of the way from `a` to `b`
fn lerp_rig(a: &Rig, b: &Rig, f: f64) -> Rig {
    let f = f.clamp(0.0, 1.0);
    let m = |x: f64, y: f64| x + (y - x) * f;
    let mut r = *a;
    r.pitch = m(a.pitch, b.pitch);
    r.roll = m(a.roll, b.roll);
    r.jaw = m(a.jaw, b.jaw);
    for j in 0..2 {
        r.head[j] = m(a.head[j], b.head[j]);
        r.tail[j] = m(a.tail[j], b.tail[j]);
        r.wings[j] = lerp_wing(a.wings[j], b.wings[j], f);
        r.legs[j] = LegPose { swing: m(a.legs[j].swing, b.legs[j].swing), knee: m(a.legs[j].knee, b.legs[j].knee) };
        r.arms[j] = ArmPose { swing: m(a.arms[j].swing, b.arms[j].swing), spread: m(a.arms[j].spread, b.arms[j].spread), elbow: m(a.arms[j].elbow, b.arms[j].elbow) };
    }
    r
}

/// Drawing back to strike, `f` of the way: a gargoyle's claws raised behind its shoulder, a bird's
/// or a harpy's foot raised, talons spread, and its wings up
fn reach_back(want: &mut Rig, sp: &Spec, f: f64) {
    let f = f.clamp(0.0, 1.0);
    match sp.arms {
        Arms::None => {
            want.legs[0].swing += 0.9 * f;
            want.legs[0].knee += 0.6 * f;
            for w in &mut want.wings {
                w.flap += 0.6 * f;
                w.sweep -= 0.2 * f;
                w.fold -= 1.2 * f;
            }
        }
        _ => {
            want.arms[0].swing += 1.9 * f;
            want.arms[0].spread += 0.5 * f;
            want.arms[0].elbow = 1.3;
            want.arms[1].swing += 0.6 * f;
        }
    }
}

/// A swipe coming through, `f` of the way, the `again`th of a combo: the other hand for the next
fn swipe(want: &mut Rig, sp: &Spec, f: f64, again: u8) {
    let arc = (PI * f).sin();
    match sp.arms {
        Arms::None => {
            want.legs[0].swing += 0.9 * (1.0 - f) + 0.4 * arc;
            want.legs[0].knee += 0.2;
            want.pitch += 0.2 * arc;
            for w in &mut want.wings {
                w.flap += 0.5;
                w.fold -= 1.2;
            }
        }
        _ => {
            let k = if again % 2 == 0 { 1 } else { 0 };
            want.arms[k].swing += 1.9 - 2.4 * f;
            want.arms[k].spread += 0.5 - 0.7 * f;
            want.arms[k].elbow = 0.3;
            want.pitch -= 0.25 * arc;
            want.roll += if k == 0 { -0.2 } else { 0.2 } * arc;
        }
    }
}

/// Talons or claws thrown forward to strike, as at the bottom of a rake or a dive
fn strike_pose(want: &mut Rig, sp: &Spec) {
    for leg in &mut want.legs {
        leg.swing = 0.9;
        leg.knee = 0.25;
    }
    if sp.arms != Arms::None {
        for arm in &mut want.arms {
            arm.swing = 1.4;
            arm.spread = 0.3;
            arm.elbow = 0.4;
        }
    }
    want.head[1] -= 0.3;
}

/// Perched with nothing the matter, never quite still: darting its head about as a bird does,
/// shifting its weight, now and then rousing its feathers or preening under a wing
fn idle(d: &mut Flier, h: &Here, want: &mut Rig, wings: &mut WingSet) {
    d.glance -= h.dt;
    if d.glance <= 0.0 {
        let preen = d.roll() < 0.15;
        d.glance = if preen { 1.8 } else { 0.5 + 1.8 * d.roll() };
        d.gaze = if preen {
            let side = if d.roll() < 0.5 { 1.0 } else { -1.0 };
            [1.9 * side, -0.55]
        } else {
            [(d.roll() - 0.5) * 2.0, (d.roll() - 0.5) * 0.6]
        };
        if d.roll() < 0.1 {
            d.rouse = 0.0;
        }
    }
    // Its weight from one foot to the other, and breathing
    let shift = (d.clock * 0.45 + d.rng as f64 * 1e-9).sin();
    want.roll += 0.03 * shift;
    want.pitch += 0.02 * (d.clock * 2.2).sin();
    // Rousing: half spreading its wings and shaking them, its feathers puffed out
    if d.rouse >= 0.0 {
        d.rouse += h.dt;
        let f = (d.rouse / 0.9).min(1.0);
        let env = (PI * f).sin();
        for w in &mut want.wings {
            w.flap += 0.5 * env;
            w.fold -= 1.4 * env;
            w.sweep -= 0.5 * env;
        }
        *wings = WingSet { base: want.wings[0], amp: 0.12 * env, rate: 7.0 };
        want.roll += 0.04 * env * (d.clock * 31.0).sin();
        if f >= 1.0 {
            d.rouse = -1.0;
        }
    }
}

/// Killed: falling out of the sky if it's flying, crashing down and lying still, or for a gargoyle
/// turning to stone and breaking apart
fn fall(d: &mut Flier, boss: &mut Boss, h: &Here, goal: &mut Goal, want: &mut Rig, wings: &mut WingSet) {
    let vpm = h.vpm;
    if d.state != State::Fall {
        d.state = State::Fall;
        d.holding = None;
        act_out(boss, Act::Death { at: d.mouth });
    }
    let gargoyle = gargoyles::is_gargoyle(h.look);
    goal.speed = -1.0;
    let ground = ground_of(h, d.center);
    if d.airborne {
        *wings = WingSet { base: WingPose { flap: 0.5, sweep: 0.6, twist: 0.3, fold: 1.2, bend: 0.4 }, amp: 0.25, rate: 1.2 };
        d.velocity.z -= 12.0 * vpm * h.dt.max(1e-3);
        d.velocity.x *= (-1.0 * h.dt).exp();
        d.velocity.y *= (-1.0 * h.dt).exp();
        d.center += d.velocity * h.dt.max(1.0 / 120.0);
        want.roll += 0.9 * d.round;
        want.pitch -= 0.5;
        if d.center.z <= ground + h.stand * 0.5 {
            d.center.z = ground + h.stand * 0.5;
            d.airborne = false;
            d.velocity = V3::zeros();
            act_out(boss, Act::Crash { at: V3::new(d.center.x, d.center.y, ground), size: size(h.look) });
        }
        if gargoyle {
            d.stone = (d.stone + h.dt.max(1.0 / 120.0) / 0.6).min(1.0);
        }
    } else if gargoyle {
        // Gone to stone, and breaking up into its pieces where it stood
        d.stone = (d.stone + h.dt.max(1.0 / 120.0) / 0.35).min(1.0);
        d.dead = Some(d.dead.unwrap_or(0.0) + h.dt.max(1.0 / 120.0));
        if d.stone >= 1.0 && d.rubble.is_none() {
            let mut pieces = [Rubble { velocity: V3::zeros(), spin: V3::zeros() }; BONES];
            for (b, piece) in pieces.iter_mut().enumerate() {
                let out = flat(d.bones[b].at - d.center).try_normalize(1e-6).unwrap_or_else(|| V3::new(d.yaw.cos(), d.yaw.sin(), 0.0));
                let speed = 1.5 + 2.5 * d.roll();
                piece.velocity = (out * speed + V3::new(0.0, 0.0, 1.5 + 2.5 * d.roll())) * vpm;
                piece.spin = V3::new(d.roll() - 0.5, d.roll() - 0.5, d.roll() - 0.5) * 9.0;
            }
            d.rubble = Some(Box::new(pieces));
            act_out(boss, Act::Crumble { at: d.center, size: size(h.look) * 1.6 });
        }
        *want = d.rig;
    } else {
        // Lying on its side, wings splayed out, head down
        d.dead = Some(d.dead.unwrap_or(0.0) + h.dt.max(1.0 / 120.0));
        *wings = WingSet { base: WingPose { flap: -0.2, sweep: 0.5, twist: 0.1, fold: 0.6, bend: -0.1 }, amp: 0.0, rate: 1.0 };
        want.pitch = 0.0;
        want.roll = 1.2 * d.round;
        want.head = [0.3, -0.5];
        want.jaw = 0.4;
        for leg in &mut want.legs {
            leg.swing = 0.6;
            leg.knee = 0.4;
        }
        let lie = ground + h.sp.s * 0.22 * vpm;
        d.center.z += (lie - d.center.z) * (1.0 - (-6.0 * h.dt.max(1.0 / 120.0)).exp());
    }
}

/// Moving it towards where it means to go: flying, steering and banking as it turns, never into
/// the ground; or walking on it
fn travel(d: &mut Flier, h: &Here, goal: &Goal, want: &mut Rig) {
    let vpm = h.vpm;
    let fl = flight(h.look);
    let k = size(h.look).sqrt();
    let dt = h.dt;
    let before = d.yaw;
    if matches!(d.state, State::Statue | State::Stone(_)) || d.rubble.is_some() {
        return;
    }
    if d.airborne {
        if goal.speed >= 0.0 {
            let off = goal.to - d.center;
            let distance = off.norm();
            let want_v = if distance > 1e-6 { off / distance * (goal.speed * vpm).min(distance * 2.6) } else { V3::zeros() };
            let change = want_v - d.velocity;
            let most = fl.accel * vpm * dt;
            d.velocity += if change.norm() > most { change.normalize() * most } else { change };
            d.center += d.velocity * dt;
        }
        // Never into the ground under it, nor what's just ahead
        let ahead = d.center + flat(d.velocity) * 0.6;
        let under = ground_z(h.world, d.center.x, d.center.y, d.center.z, 60.0).unwrap_or(f64::MIN);
        let before_it = ground_z(h.world, ahead.x, ahead.y, d.center.z, 60.0).unwrap_or(f64::MIN);
        let clearance = if goal.low { h.stand * 0.7 } else { h.stand * 1.3 + 1.0 * vpm };
        let floor = under.max(before_it) + clearance;
        if d.center.z < floor && !matches!(d.state, State::Alight(_) | State::Do(Stoop | Pounce, _) | State::Fall | State::Reel(_)) {
            d.center.z += (floor - d.center.z) * (1.0 - (-8.0 * dt).exp());
            d.velocity.z = d.velocity.z.max(0.0);
        }
        let roof = h.world.size[2] as f64 - 3.0 * k * vpm;
        if d.center.z > roof {
            d.center.z = roof;
            d.velocity.z = d.velocity.z.min(0.0);
        }
        // Facing the way it flies, or what it's looking at hovering
        let speed = flat(d.velocity).norm() / vpm;
        let heading = match goal.face {
            Some(at) if speed < fl.cruise * 0.8 || matches!(d.state, State::Wind(..)) => flat(at - d.center).y.atan2(flat(at - d.center).x),
            _ if speed > 1.0 => d.velocity.y.atan2(d.velocity.x),
            Some(at) => flat(at - d.center).y.atan2(flat(at - d.center).x),
            None => d.yaw,
        };
        let turn = turn_to(d.yaw, heading);
        d.yaw += turn.clamp(-fl.turn * 1.6 * dt, fl.turn * 1.6 * dt);
        // Banking into its turns, and climbing nose up
        let rate = turn_to(before, d.yaw) / dt.max(1e-6);
        want.roll += (-rate * speed * 0.07).clamp(-0.9, 0.9);
        let climb = d.velocity.z.atan2(flat(d.velocity).norm().max(1e-6));
        want.pitch += (climb * 0.6).clamp(-0.6, 0.5);
        // Its tail twisting to steer
        want.tail[0] += (rate * 0.15).clamp(-0.4, 0.4);
    } else if matches!(d.state, State::Fall) {
        // Lying still
    } else {
        // Walking: up to where it's going, turning towards it, over the ground
        let off = flat(goal.to - d.center);
        let distance = off.norm() / vpm;
        let heading = match goal.face {
            Some(at) if distance < 0.5 => flat(at - d.center).y.atan2(flat(at - d.center).x),
            _ if distance >= 0.5 => off.y.atan2(off.x),
            Some(at) => flat(at - d.center).y.atan2(flat(at - d.center).x),
            None => d.yaw,
        };
        let turning = turn_to(d.yaw, heading);
        let turn = 3.5 / k;
        d.yaw += turning.clamp(-turn * dt, turn * dt);
        let speed = if distance > 0.3 && turning.abs() < 1.2 { goal.speed.min(distance / dt.max(1e-6) / vpm) } else { 0.0 };
        let step = V3::new(d.yaw.cos(), d.yaw.sin(), 0.0) * speed * vpm * dt;
        let next = d.center + step;
        let feet_z = d.foot_z;
        let ground_next = ground_z(h.world, next.x, next.y, feet_z + 1.0 * vpm * k, 6.0 * k);
        let climb = 0.7 * k * vpm;
        match ground_next {
            Some(z) if speed > 0.0 && (z - feet_z).abs() < climb && (h.water)(next.x, next.y) < z + 0.4 * k * vpm => {
                d.center.x = next.x;
                d.center.y = next.y;
                d.blocked = 0.0;
                d.stride = (d.stride + speed * dt / stride(h.look)).fract();
            }
            _ if speed > 0.0 => d.blocked += dt,
            _ => {}
        }
        // Its legs going as it walks, a gargoyle's arms swinging
        if speed > 0.05 {
            let phase = d.stride * TAU;
            let swing = (0.5 * (speed / speeds(h.look).1).sqrt().min(1.0)).max(0.22);
            for (leg, offset) in [(0, 0.0), (1, PI)] {
                want.legs[leg].swing += swing * (phase + offset).sin();
                want.legs[leg].knee += 0.6 * swing * (-(phase + offset).cos()).max(0.0);
                want.arms[leg].swing -= 0.8 * swing * (phase + offset).sin();
            }
            want.roll += 0.04 * phase.sin();
            want.head[0] -= 0.06 * phase.sin();
        }
    }
}

/// Its joints eased towards where it wants them, its wings beating, every piece of it put where
/// they all are now, its feet planted on the ground standing, and a gargoyle's broken pieces
/// tumbling as it dies
fn animate(d: &mut Flier, boss: &mut Boss, h: &Here, want: &Rig, wings: &WingSet, dead: bool) {
    let dt = h.dt.max(if dead { 1.0 / 120.0 } else { 0.0 });
    let k = size(h.look).sqrt();
    if let Some(rubble) = &mut d.rubble {
        tumble(rubble, &mut d.bones, h, dt);
        d.focus = d.center;
        d.dead = Some(d.dead.unwrap_or(0.0));
        let _ = boss;
        return;
    }
    let still = matches!(d.state, State::Statue);
    let mut want = *want;
    // Its tail swaying a little, as it's never quite still
    if !still {
        want.tail[0] += 0.06 * (d.clock * 1.4 / k).sin();
    }
    let rate = if still { 30.0 } else { 7.0 / k.powf(0.5) };
    d.rig.ease(&want, rate, dt);
    // Its wings ease from one way of going to another, and beat on as they go
    let ease = 1.0 - (-5.0 / k.powf(0.3) * dt).exp();
    let (w, t) = (&mut d.wings, wings);
    let e = |a: &mut f64, b: f64| *a += (b - *a) * ease;
    e(&mut w.base.flap, t.base.flap);
    e(&mut w.base.sweep, t.base.sweep);
    e(&mut w.base.twist, t.base.twist);
    e(&mut w.base.fold, t.base.fold);
    e(&mut w.base.bend, t.base.bend);
    e(&mut w.amp, t.amp);
    e(&mut w.rate, t.rate);
    d.beat = (d.beat + d.wings.rate * dt).fract();
    let (beat, lift) = wingbeat(d.wings.base, d.wings.amp, d.beat, h.sp.plan);
    let mut drawn = d.rig;
    drawn.wings = [beat, beat];
    // Its head darting about perched, quicker than the rest of it moves
    let quick = 1.0 - (-16.0 * dt).exp();
    let gaze = if d.state == State::Rest && !d.airborne { d.gaze } else { [0.0; 2] };
    for j in 0..2 {
        d.gazed[j] += (gaze[j] - d.gazed[j]) * quick;
        drawn.head[j] += d.gazed[j];
    }
    // Flinching from a blow: its head snapping back, its body jolted, its wings jerked up
    if d.flinch > 0.0 {
        let f = d.flinch * d.flinch;
        let small = if is_monster(h.look) { 1.0 } else { 0.5 };
        drawn.head[1] += 0.45 * f * small;
        drawn.roll += 0.22 * f * d.flinch_side * small;
        drawn.pitch += 0.12 * f * small;
        drawn.jaw += 0.4 * f;
        for wing in &mut drawn.wings {
            wing.flap += 0.35 * f * small;
            wing.fold -= 0.4 * f * small;
        }
    }
    // Rising and falling with each beat of its wings, flying
    let bob = if d.airborne { lift * h.sp.s * h.vpm } else { 0.0 };
    let center = d.center + V3::new(0.0, 0.0, bob);
    d.mouth = pose(&h.sp, center, d.yaw, &drawn, h.vpm, &mut d.bones);
    // Animation files on top, see src\anim.rs
    edited(d, h.vpm);
    // On the ground, its feet on it however its legs are bent: the body over them moved up or
    // down to where they stand
    if !d.airborne {
        let feet = feet_of(&h.sp, &d.bones, h.vpm);
        let lowest = feet[0].z.min(feet[1].z);
        let ground = ground_z(h.world, d.center.x, d.center.y, d.foot_z + 1.0 * h.vpm * k, 4.0 * k).unwrap_or(d.foot_z);
        let lie = matches!(d.state, State::Fall);
        if !lie {
            let shift = ground - lowest;
            let settle = if still { 1.0 } else { 1.0 - (-14.0 * dt).exp() };
            let by = shift * settle;
            d.center.z += by;
            for b in &mut d.bones {
                b.at.z += by;
            }
            d.mouth.z += by;
        }
        d.foot_z = ground;
    }
    d.focus = d.center;
}

/// Where its wings are `beat` of the way through a stroke, held at `base`, beating `amp` either
/// way: the downstroke quick and hard with the wings spread wide and reaching forward, the upstroke
/// slower with the hand folded back at the wrist and the wings swept; and how high the beat lifts
/// its body, in units, up on the downstroke and sinking a little on the way up
fn wingbeat(base: WingPose, amp: f64, beat: f64, plan: Plan) -> (WingPose, f64) {
    const DOWN: f64 = 0.42;
    let fold = if plan == Plan::Gargoyle { 1.25 } else { 0.95 };
    if beat < DOWN {
        let f = beat / DOWN;
        let arc = (PI * f).sin();
        let pose = WingPose {
            flap: base.flap + amp * (1.0 - 2.0 * smooth(f)),
            sweep: base.sweep - 0.14 * amp * arc,
            twist: base.twist - 0.28 * amp * arc,
            fold: base.fold + 0.12 * amp * (1.0 - f),
            bend: base.bend + 0.18 * amp * arc,
        };
        (pose, 0.07 * amp * arc)
    } else {
        let f = (beat - DOWN) / (1.0 - DOWN);
        let arc = (PI * f).sin();
        let pose = WingPose {
            flap: base.flap + amp * (-1.0 + 2.0 * smooth(f)),
            sweep: base.sweep + 0.32 * amp * arc,
            twist: base.twist + 0.22 * amp * arc,
            fold: base.fold + fold * amp * arc,
            bend: base.bend - 0.3 * amp * arc,
        };
        (pose, -0.03 * amp * arc)
    }
}

/// A gargoyle's pieces broken apart as it dies, flying out, falling and coming to rest
fn tumble(rubble: &mut [Rubble; BONES], bones: &mut [Bone; BONES], h: &Here, dt: f64) {
    let vpm = h.vpm;
    for (b, piece) in rubble.iter_mut().enumerate() {
        if !bones[b].shown {
            continue;
        }
        piece.velocity.z -= 14.0 * vpm * dt;
        let next = bones[b].at + piece.velocity * dt;
        let ground = ground_z(h.world, next.x, next.y, bones[b].at.z + 0.5 * vpm, 20.0).unwrap_or(f64::MIN);
        let rest = ground + 0.12 * h.sp.s * vpm;
        if next.z < rest {
            bones[b].at = V3::new(next.x, next.y, rest);
            piece.velocity = V3::new(piece.velocity.x * 0.4, piece.velocity.y * 0.4, (-piece.velocity.z * 0.25).max(0.0));
            piece.spin *= 0.5;
        } else {
            bones[b].at = next;
        }
        bones[b].turn = UnitQuaternion::from_scaled_axis(piece.spin * dt) * bones[b].turn;
    }
}

/// What to draw for `a`: each of its pieces, into `out`, a gargoyle's turning to stone
pub fn instances(a: &Animal, d: &Flier, vpm: f64, out: &mut Vec<AnimalInstance>) {
    let layout = layout();
    let k = kind(a.look);
    let base = animals::WINGED_MODELS_AT + k * PIECES;
    let tint = a.status.tint();
    let stone = gargoyles::is_gargoyle(a.look) && d.stone > 0.0;
    let skin = if stone {
        let [r, g, b] = gargoyles::stone_color(a.look);
        let amount = (d.stone.clamp(0.0, 1.0) * 235.0) as u32;
        r as u32 | (g as u32) << 8 | (b as u32) << 16 | amount << 24
    } else {
        0
    };
    for (b, bone) in d.bones.iter().enumerate() {
        if !bone.shown {
            continue;
        }
        // A sleeping gargoyle's eyes are stone too, till they catch fire as it wakes
        let m = if b == HEAD && stone && d.stone > 0.5 { M_STONE_HEAD } else { model_of(b) };
        let (anchor, size) = layout[k * PIECES + m];
        if size == [1, 1, 1] {
            continue;
        }
        let [x, y, z] = size.map(|s| s as f64 / MODEL_RES);
        let reach = (x * x * 0.25 + y * y * 0.25 + z * z).sqrt() * vpm;
        let position = bone.at + bone.turn * anchor * vpm;
        out.push(AnimalInstance::new((base + m) as u32, position, bone.turn, vpm / MODEL_RES, reach, MOVING).tinted(tint).skinned(skin));
    }
}

/// One of `look` called down out of the sky by a shriek, at `at` in voxels, high up, flying in
/// towards `toward` and after the player, one of pack `pack`
pub fn called(look: Look, at: V3, toward: V3, pack: u32, vpm: f64, seed: u32) -> Animal {
    let mut animal = Animal::boss(look, at, [at.x as i64, at.y as i64, -2], seed);
    if let Some(d) = &mut animal.winged {
        d.swoop_in(at + V3::new(0.0, 0.0, stand_height(&spec(look)) * vpm), toward, vpm);
    }
    if let Some(boss) = &mut animal.boss {
        boss.provoke();
        boss.pack = pack;
    }
    animal
}

/// The highest dry ground at `x`, `y` with room to stand over it, in voxels, if it isn't under water
fn top_at(world: &World, water: WaterLevel, x: f64, y: f64) -> Option<f64> {
    let vpm = world.voxels_per_meter as f64;
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    let room = (2.0 * vpm) as i64;
    let mut clear = 0;
    for z in (0..world.size[2] as i64).rev() {
        let m = world.material_at(xi, yi, z);
        if is_passable(m) {
            clear += 1;
            continue;
        }
        if clear >= room && !matches!(m, LEAVES | LEAVES_DARK | WOOD) {
            let ground = z as f64 + 1.0;
            return (water(x, y) < ground + 0.2 * vpm).then_some(ground);
        }
        clear = 0;
    }
    None
}

/// `count` flocks and perches of what flies in the land they turn up in, well away from the
/// player at `near`: harpies roosting along the edges of the sky isles and skyhunters soaring
/// over them, gargoyles asleep on the high rock of the dreaming realm, the frozen one, the
/// sunken reach and the Cinder Crown, and now and then in the mountains of the normal world
pub fn spawn_wilds(world: &World, water: WaterLevel, rng: &mut u32, near: V3, land: Land, count: usize) -> Vec<Animal> {
    let vpm = world.voxels_per_meter as f64;
    let [sx, sy, sz] = world.size.map(f64::from);
    let mut out = Vec::new();
    let roll = |rng: &mut u32| {
        *rng = hash(rng.wrapping_add(0x9e37_79b9));
        (*rng >> 8) as f64 / 16777216.0
    };
    for _ in 0..count {
        for _ in 0..40 {
            let (x, y) = (sx * (0.1 + 0.8 * roll(rng)), sy * (0.1 + 0.8 * roll(rng)));
            if (x - near.x).hypot(y - near.y) < 40.0 * vpm {
                continue;
            }
            let Some(z) = top_at(world, water, x, y) else {
                continue;
            };
            let ground = world.material_at(x as i64, y as i64, z as i64 - 1);
            let high = z > sz * 0.5;
            let stony = matches!(ground, STONE | ROCK | GRAVEL | SNOW | ICE | OBSIDIAN | MORTAR);
            let r = roll(rng);
            let looks: Vec<Look> = match land {
                Land::Sky if r < 0.35 => vec![Look::Skyhunter; 2],
                Land::Sky => vec![Look::Harpy; 3 + (roll(rng) < 0.4) as usize],
                Land::Normal if high && stony && r < 0.3 => vec![Look::Gargoyle; 2],
                Land::Normal if high && stony && r < 0.6 => vec![Look::Skyhunter; 2],
                Land::Normal => continue,
                _ => vec![gargoyles::of_land(land); 2 + (r < 0.3) as usize],
            };
            // On the very edge if there's one near, to look out over it
            let mut at = V3::new(x, y, z);
            if looks[0] == Look::Harpy {
                for ring in 1..6 {
                    let a = roll(rng) * TAU;
                    let p = at + V3::new(a.cos(), a.sin(), 0.0) * ring as f64 * 1.5 * vpm;
                    if let Some(pz) = top_at(world, water, p.x, p.y)
                        && (pz - z).abs() < 2.0 * vpm
                        && drop_off(world, V3::new(p.x, p.y, pz), 2.5).is_some()
                    {
                        at = V3::new(p.x, p.y, pz);
                        break;
                    }
                }
            }
            out.extend(animals::spawn_pack(world, water, &looks, at, 2.2, hash(*rng)));
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flat open ground `meters` across at `vpm` voxels a meter, four meters deep
    fn open_ground(meters: u32, vpm: u32) -> World {
        let size = [meters * vpm, meters * vpm, 64 * vpm];
        let (tx, ty) = (size[0] as usize / 4, size[1] as usize / 4);
        let mut occupancy = vec![0u128; tx * ty * (size[2] as usize / 8)];
        occupancy[..tx * ty * (4 * vpm as usize / 8)].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, STONE);
        world.voxels_per_meter = vpm;
        world
    }

    /// Every piece of every kind that's drawn has a model, solid enough to see, under 255 voxels a
    /// side for the shader, and they're all together small next to the rest of the models
    #[test]
    fn every_piece_has_a_model() {
        let made = build_models();
        assert_eq!(made.len(), MODELS);
        let mut bytes = 0;
        for (k, &look) in KINDS.iter().enumerate() {
            let sp = spec(look);
            let mut bones = [Bone::default(); BONES];
            pose(&sp, V3::zeros(), 0.0, &Rig::standing(&sp), 16.0, &mut bones);
            for (b, bone) in bones.iter().enumerate() {
                let (model, _) = &made[k * PIECES + model_of(b)];
                if bone.shown {
                    let solid = model.voxels.iter().filter(|&&v| v != AIR).count();
                    assert!(solid > if b == JAW { 8 } else { 15 }, "{look:?} piece {b} has only {solid} voxels");
                }
                assert!(model.size.iter().all(|&s| s < 255), "{look:?} piece {b} is {:?}", model.size);
            }
            if gargoyles::is_gargoyle(look) {
                let stone = &made[k * PIECES + M_STONE_HEAD].0;
                assert!(stone.voxels.iter().filter(|&&v| v != AIR).count() > 30, "{look:?} has no head of stone");
                assert!(!stone.voxels.iter().any(|&v| matches!(v, GLOW_FIRE | GLOW_BLOOD | GLOW_FROST | GLOW_POISON | GLOW_ARCANE)), "{look:?}'s stone eyes glow");
            }
            let of: Vec<usize> = made[k * PIECES..(k + 1) * PIECES].iter().map(|(m, _)| m.voxels.len()).collect();
            println!("{look:?}: {:.2} MB, the biggest piece {:.2} MB", of.iter().sum::<usize>() as f64 / 1e6, *of.iter().max().unwrap() as f64 / 1e6);
            bytes += of.iter().sum::<usize>();
        }
        println!("winged pieces: {:.1} MB", bytes as f64 / 1e6);
        assert!(bytes < 12_000_000, "the winged take {bytes} bytes");
        for fx in 0..FX {
            assert!(made[KINDS.len() * PIECES + fx].0.voxels.iter().any(|&v| v != AIR), "effect {fx} is empty");
        }
    }

    /// Names, healths and what's what: each its own name, the raid bosses far tougher than the
    /// rest, the monsters monsters and the rest bosses
    #[test]
    fn each_is_what_it_is() {
        for (i, &a) in KINDS.iter().enumerate() {
            assert!(a.build().is_some() && is_winged(a));
            assert!(KINDS[i + 1..].iter().all(|&b| name(a) != name(b)), "{a:?} shares a name");
            assert_eq!(a.is_monster(), is_monster(a), "{a:?}");
            assert_eq!(a.is_boss(), !is_monster(a), "{a:?}");
            assert_eq!(a.is_raid(), is_raid(a), "{a:?}");
            if is_raid(a) {
                assert!(health(a) > 12000.0);
            } else if is_monster(a) {
                assert!(health(a) < 400.0);
            }
            assert!(a.flies());
            assert!(!skills(a).is_empty());
            assert!(lore(a).len() > 40);
        }
        for look in SKY_RAID_BOSSES {
            assert!(!gargoyles::is_gargoyle(look) && look.is_raid());
        }
    }

    /// However its joints are turned, its wings stay on at the wrist and its shins at the knee
    #[test]
    fn its_joints_hold_together() {
        let vpm = 16.0;
        let mut seed = 9u32;
        let mut roll = || {
            seed = hash(seed.wrapping_add(0x9e37_79b9));
            (seed >> 8) as f64 / 16777216.0 - 0.5
        };
        for look in KINDS {
            let sp = spec(look);
            for _ in 0..30 {
                let mut rig = Rig::flying(&sp);
                rig.pitch = roll();
                rig.roll = roll();
                for w in &mut rig.wings {
                    *w = WingPose { flap: roll() * 2.0, sweep: roll() * 2.0, twist: roll(), fold: roll() * 4.0, bend: roll() };
                }
                for l in &mut rig.legs {
                    *l = LegPose { swing: roll() * 3.0, knee: roll() * 3.0 };
                }
                let mut bones = [Bone::default(); BONES];
                let center = V3::new(100.0, 200.0, 300.0);
                pose(&sp, center, roll() * 6.0, &rig, vpm, &mut bones);
                let m = sp.s * vpm;
                for side in 0..2 {
                    let (inner, outer) = (bones[WING + side * 2], bones[WING + side * 2 + 1]);
                    let w = if side == 0 { wrist(&sp) } else { flip(wrist(&sp)) };
                    assert!((inner.at + inner.turn * w * m - outer.at).norm() < 1e-6 * m, "{look:?}'s wing {side} comes off at the wrist");
                    let (lt, _) = leg_lengths(&sp);
                    let (thigh, shin) = (bones[THIGH + side], bones[SHIN + side]);
                    assert!((thigh.at + thigh.turn * V3::new(0.0, 0.0, -lt) * m - shin.at).norm() < 1e-6 * m, "{look:?}'s leg {side} comes off at the knee");
                }
            }
        }
    }

    /// Standing, its feet are under its body and its folded wings lie along its sides, nothing of
    /// them standing up far over its back or out to the side
    #[test]
    fn it_stands_on_its_feet_with_its_wings_folded() {
        for look in KINDS {
            let sp = spec(look);
            let mut bones = [Bone::default(); BONES];
            pose(&sp, V3::zeros(), 0.0, &Rig::standing(&sp), 1.0, &mut bones);
            let feet = feet_of(&sp, &bones, 1.0);
            let stand = stand_height(&sp);
            assert!(stand > 0.25 * sp.s && stand < 1.6 * sp.s, "{look:?} stands {stand:.2} m high");
            for f in feet {
                assert!(f.xy().norm() < 0.6 * sp.s, "{look:?}'s foot is {:.2} m out from under it", f.xy().norm());
            }
            let (_, wo) = wing_lengths(&sp);
            for side in 0..2 {
                let outer = bones[WING + side * 2 + 1];
                let y = if side == 0 { 1.0 } else { -1.0 };
                let tip = outer.at + outer.turn * V3::new(-0.3, y * wo, 0.0) * sp.s;
                assert!(tip.y.abs() < 0.7 * sp.s, "{look:?}'s folded wingtip sticks {:.2} m out to the side", tip.y.abs());
            }
        }
    }

    /// What happens over `seconds` with one of `look` some way off a player standing still on
    /// open ground: what it did, and where it was, a frame at a time
    fn fight(look: Look, seconds: f64, hurt_to: Option<f64>) -> (Vec<(f64, Deed)>, Vec<(f64, V3, bool)>, Animal) {
        fight_as(look, seconds, hurt_to, None)
    }

    /// The same, it making only move `only`, as soon as it can
    fn fight_making(look: Look, seconds: f64, only: Option<Move>) -> (Vec<(f64, Deed)>, Vec<(f64, V3, bool)>, Animal) {
        fight_as(look, seconds, None, only)
    }

    fn fight_as(look: Look, seconds: f64, hurt_to: Option<f64>, only: Option<Move>) -> (Vec<(f64, Deed)>, Vec<(f64, V3, bool)>, Animal) {
        let vpm = 4u32;
        let world = open_ground(240, vpm);
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let v = vpm as f64;
        let feet = V3::new(120.0, 120.0, 4.0) * v;
        let mut a = Animal::boss(look, feet + V3::new(18.0, 6.0, 0.0) * v, [1, 2, -1], 11);
        a.winged.as_mut().unwrap().only = only;
        let dt = 1.0 / 30.0;
        let (mut deeds, mut track) = (Vec::new(), Vec::new());
        let mut t = 0.0;
        while t < seconds {
            if let Some(to) = hurt_to
                && t > 5.0
                && a.health > a.max_health * to
            {
                a.damage(a.max_health * 0.01, feet);
            }
            a.update(&world, &water, Some(feet), dt);
            let d = a.winged.as_ref().unwrap();
            track.push((t, d.center, d.airborne));
            for deed in a.boss.as_mut().unwrap().deeds.drain(..) {
                deeds.push((t, deed));
            }
            t += dt;
        }
        (deeds, track, a)
    }

    /// How long a flock of a dozen harpies and two raid bosses take to think and be posed a frame,
    /// all fighting: `cargo test --release avians::tests::cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn cost() {
        let vpm = 16u32;
        let world = open_ground(120, vpm);
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let v = vpm as f64;
        let feet = V3::new(60.0, 60.0, 4.0) * v;
        let mut flock: Vec<Animal> = (0..12).map(|k| Animal::boss(Look::Harpy, feet + V3::new(10.0 + k as f64, 5.0, 0.0) * v, [k, 2, -2], k as u32)).collect();
        flock.push(Animal::boss(Look::Roc, feet + V3::new(-20.0, 0.0, 0.0) * v, [1, 9, -1], 3));
        flock.push(Animal::boss(Look::BloodwakeGargoyle, feet + V3::new(0.0, -20.0, 0.0) * v, [2, 9, -1], 4));
        let start = std::time::Instant::now();
        let mut pieces = Vec::new();
        for _ in 0..600 {
            for a in &mut flock {
                a.update(&world, &water, Some(feet), 1.0 / 60.0);
                a.boss.as_mut().unwrap().deeds.clear();
                pieces.clear();
                instances(a, a.winged.as_ref().unwrap(), v, &mut pieces);
            }
        }
        println!("{:.3} ms a frame for 14", start.elapsed().as_secs_f64() * 1000.0 / 600.0);
    }

    /// What one of `AERIE_TRACE=Harpy` does, twice a second, to look at it
    #[test]
    #[ignore]
    fn trace() {
        let wanted = std::env::var("AERIE_TRACE").unwrap_or_default();
        let look = KINDS.into_iter().find(|l| format!("{l:?}") == wanted).unwrap_or(Look::Harpy);
        TRACE.store(true, Ordering::Relaxed);
        let hurt = std::env::var("AERIE_HURT").ok().and_then(|v| v.parse().ok());
        let (deeds, _, _) = fight(look, 20.0, hurt);
        for (t, d) in deeds {
            if let Deed::Aerie(act) = d {
                println!("{t:.2} {act:?}");
            }
        }
    }

    /// A harpy turns on the player, takes to the air, comes at them out of it, and lands to fight
    /// on foot before taking off again
    #[test]
    fn a_harpy_flies_fights_and_lands() {
        let (deeds, track, _) = fight(Look::Harpy, 40.0, None);
        assert!(track.iter().any(|&(_, _, up)| up), "it never flew");
        let strikes = deeds.iter().filter(|(_, d)| matches!(d, Deed::Aerie(Act::Strike { .. } | Act::Feathers { .. }))).count();
        assert!(strikes >= 3, "only {strikes} blows in 40 s");
        let landed = deeds.iter().any(|(_, d)| matches!(d, Deed::Aerie(Act::Land { .. })));
        let rose = deeds.iter().filter(|(_, d)| matches!(d, Deed::Aerie(Act::TakeOff { .. }))).count();
        assert!(landed && rose >= 2, "landed {landed}, took off {rose} times");
    }

    /// Every blow is seen coming: a warning, a mark on the ground or its wind-up first
    #[test]
    fn every_blow_is_seen_coming() {
        for look in [Look::Harpy, Look::Skyhunter, Look::Roc, Look::Thunderbird, Look::HarpyQueen, Look::Gargoyle, Look::BloodwakeGargoyle] {
            let (deeds, _, _) = fight(look, 45.0, Some(0.3));
            let blows: Vec<f64> = deeds.iter().filter(|(_, d)| matches!(d, Deed::Aerie(Act::Strike { .. } | Act::Impact { .. } | Act::Gust { .. } | Act::Arc { .. }))).map(|(t, _)| *t).collect();
            assert!(!blows.is_empty(), "{look:?} never struck");
            for t in blows {
                let warned = deeds.iter().any(|(w, d)| *w <= t && t - *w < 6.0 && matches!(d, Deed::Aerie(Act::Warn { .. }) | Deed::Mark { .. }));
                assert!(warned, "{look:?}'s blow at {t:.2} s came unseen");
            }
        }
    }

    /// The raid bosses fight in three phases, shrieking into each and calling their brood
    #[test]
    fn raid_bosses_fight_in_phases() {
        for look in [Look::Roc, Look::Thunderbird, Look::HarpyQueen, Look::BloodwakeGargoyle] {
            let (deeds, _, a) = fight(look, 70.0, Some(0.25));
            assert_eq!(a.boss.as_ref().unwrap().stage, 2, "{look:?}");
            let phases = deeds.iter().filter(|(_, d)| matches!(d, Deed::Phase { .. })).count();
            assert_eq!(phases, 2, "{look:?}");
            let called = deeds.iter().any(|(_, d)| matches!(d, Deed::Aerie(Act::Shriek { brood: Some(_), .. })));
            assert!(called, "{look:?} never called its brood");
        }
    }

    /// The roc snatches the player up and lets them go high over the ground, the thunderbird
    /// calls lightning down round them, and the queen throws feathers and gales
    #[test]
    fn the_sky_isles_bosses_do_their_own() {
        let (deeds, _, _) = fight_making(Look::Roc, 40.0, Some(Snatch));
        assert!(deeds.iter().any(|(_, d)| matches!(d, Deed::Aerie(Act::Grab { .. }))), "the roc never snatched");
        let carried = deeds.iter().filter(|(_, d)| matches!(d, Deed::Aerie(Act::Carry { .. }))).count();
        assert!(carried > 20 && deeds.iter().any(|(_, d)| matches!(d, Deed::Aerie(Act::Drop { .. }))), "carried {carried} frames");
        let (deeds, _, _) = fight_making(Look::Thunderbird, 30.0, Some(Lightning));
        assert!(deeds.iter().filter(|(_, d)| matches!(d, Deed::Aerie(Act::Lightning { .. }))).count() >= 6);
        let (deeds, _, _) = fight(Look::HarpyQueen, 60.0, None);
        assert!(deeds.iter().any(|(_, d)| matches!(d, Deed::Aerie(Act::Feathers { count: 7, .. }))));
        assert!(deeds.iter().any(|(_, d)| matches!(d, Deed::Aerie(Act::Gust { .. }))));
    }

    /// A gargoyle sleeps in stone till the player comes near, shrugging off blows, wakes with the
    /// stone cracking off it, turns to stone again when it's hurt, and breaks apart as it dies
    #[test]
    fn a_gargoyle_wakes_hardens_and_crumbles() {
        let vpm = 4u32;
        let world = open_ground(120, vpm);
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let v = vpm as f64;
        let home = V3::new(60.0, 60.0, 4.0) * v;
        let mut a = Animal::boss(Look::Gargoyle, home, [3, 4, -1], 5);
        let far = home + V3::new(30.0, 0.0, 0.0) * v;
        let dt = 1.0 / 30.0;
        for _ in 0..60 {
            a.update(&world, &water, Some(far), dt);
        }
        assert!(a.winged.as_ref().unwrap().hardened(), "it's not asleep in stone");
        assert!(a.guard(far) < 0.5, "blows don't glance off it asleep");
        // Coming near wakes it
        let near = home + V3::new(6.0, 0.0, 0.0) * v;
        let mut woke = false;
        let mut crumbles = 0;
        for _ in 0..120 {
            a.update(&world, &water, Some(near), dt);
            crumbles += a.boss.as_mut().unwrap().deeds.drain(..).filter(|d| matches!(d, Deed::Aerie(Act::Crumble { .. }))).count();
            woke |= !a.winged.as_ref().unwrap().hardened();
        }
        assert!(woke && crumbles >= 2, "woke {woke}, stone cracked off it {crumbles} times");
        // Hurt badly, it hardens
        let mut hardened = false;
        for _ in 0..400 {
            if a.health > a.max_health * 0.2 && !a.winged.as_ref().unwrap().hardened() {
                a.damage(a.max_health * 0.01, near);
            }
            a.update(&world, &water, Some(near), dt);
            a.boss.as_mut().unwrap().deeds.clear();
            hardened |= a.winged.as_ref().unwrap().hardened();
        }
        assert!(hardened, "it never turned to stone");
        // Killed, it breaks up and lies a while in pieces
        a.damage(1e6, near);
        let mut broke = false;
        for _ in 0..90 {
            a.update(&world, &water, Some(near), dt);
            broke |= a.winged.as_ref().unwrap().rubble.is_some();
        }
        assert!(broke && dying(&a), "it didn't break apart");
    }

    /// One killed in the air falls out of it and lies a moment where it came down
    #[test]
    fn one_killed_in_the_air_falls() {
        let (_, track, mut a) = fight(Look::Skyhunter, 8.0, None);
        assert!(track.last().unwrap().2, "it's not flying");
        let vpm = 4u32;
        let world = open_ground(240, vpm);
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        a.damage(1e6, V3::zeros());
        for _ in 0..240 {
            a.update(&world, &water, None, 1.0 / 30.0);
        }
        let d = a.winged.as_ref().unwrap();
        assert!(!d.airborne && d.center.z < (4.0 + 1.5) * vpm as f64, "it's still up at {:.1}", d.center.z / vpm as f64);
    }
}

// The rig as the model and animation editor sees it, see src\anim.rs and the editor: a few lines,
// so the animations edited there go on top of the code's poses here, see edited()

/// What each of its bones is called, in the order of Flier::bones
const BONE_NAMES: [&str; BONES] = [
    "torso",
    "head",
    "jaw",
    "tail",
    "wing_left_inner",
    "wing_left_outer",
    "wing_right_inner",
    "wing_right_outer",
    "thigh_left",
    "thigh_right",
    "shin_left",
    "shin_right",
    "upper_arm_left",
    "upper_arm_right",
    "forearm_left",
    "forearm_right",
];

/// What each of its pieces' models is called, by PIECES, for their files
pub const PIECE_NAMES: [&str; PIECES] = [
    "torso",
    "head",
    "jaw",
    "tail",
    "wing_left_inner",
    "wing_left_outer",
    "wing_right_inner",
    "wing_right_outer",
    "thigh",
    "shin",
    "upper_arm",
    "forearm",
    "stone_head",
];

/// The clips its animations are kept by: standing, walking and flying, then each move
const CLIPS: [&str; 3 + MOVES] = ["stand", "walk", "fly", "rake", "stoop", "snatch", "claw", "peck", "pounce", "gust", "volley", "lightning", "arc", "shriek", "harden", "shatter"];

/// Where each piece's model has the middle of its bottom from its joint, in meters, as the code
/// made it: for the model files, see src\assets.rs
pub fn anchors() -> Vec<V3> {
    layout().iter().map(|(a, _)| *a).collect()
}

/// An avian's or gargoyle's rig, for the editor and for animation files
pub struct EditorRig(pub Look);

impl EditorRig {
    fn id_of(look: Look) -> String {
        format!("avian/{}", crate::assets::snake(&format!("{look:?}")))
    }

    fn bone_defs() -> Vec<crate::anim::BoneDef> {
        let parent = |b: usize| -> Option<usize> {
            match b {
                TORSO => None,
                JAW => Some(HEAD),
                _ if b < WING => Some(TORSO),
                // An outer wing from its inner, an inner from the torso
                _ if b < THIGH => Some(if (b - WING) % 2 == 0 { TORSO } else { b - 1 }),
                _ if b < SHIN => Some(TORSO),
                _ if b < UPPER => Some(b - SHIN + THIGH),
                _ if b < FORE => Some(TORSO),
                _ => Some(b - FORE + UPPER),
            }
        };
        (0..BONES).map(|b| crate::anim::BoneDef { name: BONE_NAMES[b].into(), parent: parent(b) }).collect()
    }
}

/// The clip it's in, by CLIPS, and how many seconds into it
fn clip_now(d: &Flier) -> (&'static str, f64) {
    let move_clip = |m: Move| CLIPS[3 + m as usize % MOVES];
    match d.state {
        State::Wind(m, t) => (move_clip(m), t),
        State::Do(m, t) => (move_clip(m), d.skill.map_or(0.0, |s| s.wind) + t),
        _ if d.airborne => ("fly", d.clock),
        _ if (d.velocity.x * d.velocity.x + d.velocity.y * d.velocity.y).sqrt() > 0.3 => ("walk", d.clock),
        _ => ("stand", d.clock),
    }
}

/// Its bones as animation files have them on top of the code's pose, if there are any
fn edited(d: &mut Flier, vpm: f64) {
    if !crate::anim::active() {
        return;
    }
    let (clip, t) = clip_now(d);
    let center = d.bones[TORSO].at;
    let mut joints: Vec<crate::anim::Joint> = d.bones.iter().map(|b| crate::anim::Joint { at: b.at - center, turn: b.turn, shown: b.shown }).collect();
    crate::anim::adjust(&EditorRig::id_of(d.look), clip, t, &EditorRig::bone_defs, &mut joints, vpm);
    for (b, j) in d.bones.iter_mut().zip(&joints) {
        b.at = center + j.at;
        b.turn = j.turn;
    }
}

impl crate::anim::Rig for EditorRig {
    fn id(&self) -> String {
        EditorRig::id_of(self.0)
    }

    fn bones(&self) -> Vec<crate::anim::BoneDef> {
        EditorRig::bone_defs()
    }

    fn clips(&self) -> Vec<crate::anim::ClipDef> {
        let k = spec(self.0).s.sqrt();
        CLIPS
            .iter()
            .enumerate()
            .map(|(i, &name)| crate::anim::ClipDef {
                name: name.into(),
                length: match i {
                    0 => 4.0 * k,
                    1 => 1.0 * k,
                    2 => 1.0 / flight(self.0).beat,
                    _ => 1.4 * k,
                },
                looping: i < 3,
            })
            .collect()
    }

    /// The code's own poses: standing, walking its legs, flying with its wings beating, and for a
    /// move, standing, or flying for one made in the air
    fn pose(&self, clip: usize, t: f64) -> Vec<crate::anim::Joint> {
        let sp = spec(self.0);
        let air = matches!(CLIPS.get(clip), Some(&("fly" | "rake" | "stoop" | "snatch" | "gust" | "volley" | "lightning" | "arc")));
        let mut rig = if air { Rig::flying(&sp) } else { Rig::standing(&sp) };
        if air {
            let phase = (t * flight(self.0).beat).fract();
            for w in &mut rig.wings {
                *w = wingbeat(*w, 0.9, phase, sp.plan).0;
            }
        } else if clip == 1 {
            let phase = t / (1.0 * sp.s.sqrt()) * TAU;
            for (l, leg) in rig.legs.iter_mut().enumerate() {
                let s = if l == 0 { 1.0 } else { -1.0 };
                leg.swing += 0.4 * s * phase.sin();
            }
        }
        let mut bones = [Bone::default(); BONES];
        pose(&sp, V3::zeros(), 0.0, &rig, 1.0, &mut bones);
        bones.iter().map(|b| crate::anim::Joint { at: b.at, turn: b.turn, shown: b.shown }).collect()
    }

    fn draw(&self, joints: &[crate::anim::Joint], at: V3, yaw: f64, vpm: f64, lit: Option<usize>, out: &mut Vec<AnimalInstance>) {
        let layout = layout();
        let k = kind(self.0);
        let base = animals::WINGED_MODELS_AT + k * PIECES;
        let face = rz(yaw);
        for (b, j) in joints.iter().enumerate() {
            let m = model_of(b);
            let (anchor, size) = layout[k * PIECES + m];
            if !j.shown || size == [1, 1, 1] {
                continue;
            }
            let [x, y, z] = size.map(|s| s as f64 / MODEL_RES);
            let reach = (x * x * 0.25 + y * y * 0.25 + z * z).sqrt() * vpm;
            let turn = face * j.turn;
            let position = at + face * j.at * vpm + turn * anchor * vpm;
            let mut instance = AnimalInstance::new((base + m) as u32, position, turn, vpm / MODEL_RES, reach, MOVING);
            if lit == Some(b) {
                instance = instance.skinned(0xA0_30_D0_FF);
            }
            out.push(instance);
        }
    }
}
