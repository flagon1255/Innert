//! Dragons. Drakelings of four elements, the ashwing, the sleetwing, the squallwing and the
//! blightwing, fly in circles over the player, swoop down to rake them, strafe the ground with
//! their breath, hover in to bite, and come down to fight on foot before taking off again. Three
//! great dragons keep the lairs of the Dragon realm: the Pyre Matriarch on her peak, the Cindermaw
//! Tyrant in its caldera and the Gildscale Hoardwarden over its hoard among the bones. And the
//! Worldpyre Ancient, the first of them all, is the Dragon realm's finale, and keeps a far lair of
//! its own in Free Roam.
//!
//! A dragon's far too big, with wings far too wide, to be drawn in frames as the other animals
//! are: it's drawn out of jointed pieces instead, a torso, neck, head and jaw, a tail, two pieces
//! to each wing and the legs, each a little model of its own turned about its joint every frame,
//! see Dragon::instances(). So its wings beat smoothly, its neck turns its head to what it's
//! breathing at, and however big it is it takes little memory and little to draw.
//!
//! Dragons think for themselves rather than as the other bosses do, see act(): what they share
//! with them, the fight being on, the pack, the raid, they keep in the Boss beside them, and what
//! they do to the player goes out as Deeds like the others', their own as Deed::Dragon, carried
//! out in src\game\dragonfire.rs.

use std::f64::consts::{PI, TAU};
use std::sync::OnceLock;

use nalgebra::{Quaternion, UnitQuaternion, Vector3};

use crate::animals::{self, Animal, AnimalInstance, Look, MODEL_RES, MOVING, Model, Part, ball, ellipsoid, limb, voxelize};
use crate::bosses::{Boss, Deed};
use crate::player::WaterLevel;
use crate::weapons::Element;
use crate::worldgen::{World, is_passable, material::*};

/// Every dragon there is
pub const KINDS: [Look; 13] = [
    Look::Ashwing,
    Look::Sleetwing,
    Look::Squallwing,
    Look::Blightwing,
    Look::PyreMatriarch,
    Look::CindermawTyrant,
    Look::GildscaleHoardwarden,
    Look::WorldpyreAncient,
    Look::Drake,
    Look::VenomDrake,
    Look::FrostWyrm,
    Look::StormDrake,
    Look::ElderWyrm,
];

/// The small ones, monsters going about in pairs and threes, an element each
pub const DRAKELINGS: [Look; 4] = [Look::Ashwing, Look::Sleetwing, Look::Squallwing, Look::Blightwing];

/// The drakes, bosses keeping lairs of their own in the normal world and Free Roam, and guarding
/// and raiding in the realms: the Ember Drake and its kin of the cold, the jungle and the high
/// rock, and the Elder Cinder Wyrm, a raid boss of the normal world. Smaller than the great dragons
/// of the Cinder Crown, they fight mostly on the ground, and take to the air to swoop and strafe.
pub const DRAKES: [Look; 5] = [Look::Drake, Look::VenomDrake, Look::FrostWyrm, Look::StormDrake, Look::ElderWyrm];

pub fn is_drake(look: Look) -> bool {
    DRAKES.contains(&look)
}

/// One of the great dragons: the Cinder Crown's three raid bosses and its finale, each fought in
/// phases, as raid bosses are
pub fn is_great(look: Look) -> bool {
    RAID_DRAGONS.contains(&look) || look == ULTRA
}

/// The Dragon realm's three raid bosses, in the order of its lairs: the peak roost, the caldera,
/// and the hollow of bones and hoard
pub const RAID_DRAGONS: [Look; 3] = [Look::PyreMatriarch, Look::CindermawTyrant, Look::GildscaleHoardwarden];

/// The Dragon realm's finale, and the far lair's in Free Roam
pub const ULTRA: Look = Look::WorldpyreAncient;

pub fn is_drakeling(look: Look) -> bool {
    DRAKELINGS.contains(&look)
}

fn kind(look: Look) -> usize {
    KINDS.iter().position(|&k| k == look).unwrap_or(0)
}

/// What a dragon breathes
pub fn element(look: Look) -> Element {
    match look {
        Look::Sleetwing | Look::FrostWyrm => Element::Frost,
        Look::Squallwing | Look::GildscaleHoardwarden | Look::StormDrake => Element::Storm,
        Look::Blightwing | Look::VenomDrake => Element::Poison,
        _ => Element::Fire,
    }
}

pub fn name(look: Look) -> &'static str {
    match look {
        Look::Ashwing => "ashwing",
        Look::Sleetwing => "sleetwing",
        Look::Squallwing => "squallwing",
        Look::Blightwing => "blightwing",
        Look::PyreMatriarch => "Pyre Matriarch",
        Look::CindermawTyrant => "Cindermaw Tyrant",
        Look::GildscaleHoardwarden => "Gildscale Hoardwarden",
        Look::Drake => "Ember Drake",
        Look::VenomDrake => "Venom Drake",
        Look::FrostWyrm => "Frost Wyrm",
        Look::StormDrake => "Thunder Drake",
        Look::ElderWyrm => "Elder Cinder Wyrm",
        _ => "Worldpyre Ancient",
    }
}

/// What it's said of each, for the lore
#[allow(dead_code)]
pub fn lore(look: Look) -> &'static str {
    match look {
        Look::Ashwing => "Grey as cinders, with wings the red of embers. They hunt in pairs over the high rock, and they never stop breathing smoke.",
        Look::Sleetwing => "Pale as hoarfrost and quick as a gust off a glacier. What it breathes freezes as it falls.",
        Look::Squallwing => "It rides the storm-front down off the peaks, crackling. Its breath forks.",
        Look::Blightwing => "Moss grows on its scales, and nothing grows where it has breathed.",
        Look::PyreMatriarch => "Mother of the ashwings, she nests where the peak is hottest and drops on what climbs to her like a falling star.",
        Look::CindermawTyrant => "Too heavy to fly far, it lies in the caldera with its belly to the magma, and the ground cracks open where it stamps.",
        Look::GildscaleHoardwarden => "It has lain on its hoard so long the gold has grown into its scales. Lightning plays over it when it wakes.",
        Look::Drake => "Red as a forge and horned like a ram, it keeps to the old ruins, and it breathes fire on whatever comes up the stairs.",
        Look::VenomDrake => "It spreads its frill and spits, and the jungle round its lair has rotted black.",
        Look::FrostWyrm => "Long and pale, crowned with ice, it coils on the high snows and breathes the cold of the deep winter.",
        Look::StormDrake => "Two legs and wings far too long for them: it rides the thunderheads over the high rock and calls them down.",
        Look::ElderWyrm => "Old as the burn it lies in, black as soot with embers in its scales, its wings in tatters and its horns gone to gold.",
        _ => "The first fire took wing, and it is still burning. Every dragon there is was hatched from the embers it shed.",
    }
}

/// How much damage it takes to kill one
pub fn health(look: Look) -> f64 {
    match look {
        Look::Ashwing => 175.0,
        Look::Sleetwing => 175.0,
        Look::Squallwing => 175.0,
        Look::Blightwing => 175.0,
        Look::PyreMatriarch => 14500.0,
        Look::CindermawTyrant => 15500.0,
        Look::GildscaleHoardwarden => 14500.0,
        // The drakes are as hard to kill as they always were, see Look::health()
        Look::Drake | Look::VenomDrake | Look::FrostWyrm | Look::StormDrake | Look::ElderWyrm => look.health(),
        // The Worldpyre Ancient, a finale wherever it is, raised to 46000 as one, see
        // realms::FINALE_PROMOTION
        _ => 18400.0,
    }
}

/// How fast it walks and runs on the ground, in meters a second
pub fn speeds(look: Look) -> (f64, f64) {
    let s = spec(look).s;
    (1.4 * s.sqrt(), 4.8 * s.powf(0.35))
}

/// Meters covered by one stride of its walk
pub fn stride(look: Look) -> f64 {
    1.3 * spec(look).s
}

/// How much bigger than a drakeling it is
pub fn size(look: Look) -> f64 {
    spec(look).s
}

/// The drakelings a great dragon calls down to it with a roar
pub fn brood(look: Look) -> Option<Look> {
    match look {
        Look::PyreMatriarch | Look::CindermawTyrant | Look::WorldpyreAncient => Some(Look::Ashwing),
        Look::GildscaleHoardwarden => Some(Look::Squallwing),
        _ => None,
    }
}

/// The body to aim at, as Look::body(): how high its middle is over its feet standing, and how far
/// round each piece of it a blow lands, in meters. Where on it that is follows what the player's
/// looking at, see Dragon::aim().
pub fn body(look: Look) -> (f64, f64) {
    let sp = spec(look);
    (stand_height(&sp), 0.4 * sp.s * sp.girth.sqrt())
}

/// How far its pieces reach from the middle of it, in meters, wings spread
pub fn reach(look: Look) -> f64 {
    let sp = spec(look);
    sp.s * (2.6 + 0.6 * sp.span + 0.25 * sp.tails as f64)
}

/// What each dragon's hide's made of: its scales, what's darker, its belly, the skin of its wings,
/// its horns and claws, what glows in it, and its eyes
#[derive(Clone, Copy)]
struct Hide {
    scale: u8,
    dark: u8,
    belly: u8,
    wing: u8,
    horn: u8,
    glow: u8,
    eye: u8,
}

/// The horns and whatever else crowns its head, and runs down its back
#[derive(Clone, Copy, PartialEq)]
enum Crown {
    /// Two horns swept back, as the drakelings have
    Swept,
    /// A crest of spines down from the brow, and a mane of them
    Crest,
    /// Horns tipped and ringed with gold, the scales of its back set with it
    Gilded,
    /// A crown of four great horns and frills at the cheeks, the oldest of all
    Ancient,
    /// A horn on its snout and two swept back over its brow, spikes at its cheeks: the Ember
    /// Drake's
    Horned,
    /// A crown of ice, spikes of it fanned back from its brow
    Frost,
    /// A fan of skin on spines behind its head, spotted with venom, spread as it spits
    Frill,
    /// Horns forked like lightning, glowing at their tips
    Forked,
    /// Horns curled round like a ram's, gone to gold with age, embers in its scales
    Elder,
}

/// How the edge of a wing's skin runs between its fingers
#[derive(Clone, Copy, PartialEq)]
enum WingShape {
    /// Scalloped between long fingers, as most have
    Scalloped,
    /// Broad and rounded, the skin full between its fingers
    Broad,
    /// Long and narrow, swept back to a point, for riding the storm
    Long,
    /// Torn and holed with age
    Tattered,
    /// The fingers spiked out past the skin
    Barbed,
}

/// How a dragon's built, in units of a drakeling's meters
#[derive(Clone, Copy)]
struct Spec {
    /// Meters to a unit
    s: f64,
    /// Four legs and wings, rather than a wyvern's two legs and wings
    four: bool,
    necks: usize,
    tails: usize,
    /// How long each piece of its neck and tail is
    neck: f64,
    tail: f64,
    /// How bulky, and how long in the wing, next to a drakeling
    girth: f64,
    span: f64,
    hide: Hide,
    crown: Crown,
    wing: WingShape,
    /// A spiked club at the end of its tail rather than a spade
    club: bool,
}

fn spec(look: Look) -> Spec {
    let hide = |scale, dark, belly, wing, horn, glow, eye| Hide { scale, dark, belly, wing, horn, glow, eye };
    let small = |s: f64, hide: Hide| Spec { s, four: false, necks: 2, tails: 3, neck: 0.38, tail: 0.6, girth: 1.0, span: 1.0, hide, crown: Crown::Swept, wing: WingShape::Scalloped, club: false };
    let drake = |s: f64, hide: Hide, crown: Crown, wing: WingShape| Spec { s, four: true, necks: 3, tails: 4, neck: 0.3, tail: 0.5, girth: 1.1, span: 0.95, hide, crown, wing, club: false };
    match look {
        // Ash-grey, its wings the red of embers, fire glowing through its belly
        Look::Ashwing => small(1.0, hide(MORTAR, OBSIDIAN, FUR_TAN, CLOTH, BONE, GLOW_FIRE, GLOW_FIRE)),
        // Pale as ice with teal wings and frost in its eyes
        Look::Sleetwing => small(0.95, hide(ICE, DARK_STEEL, LINEN, TEMPLE_TEAL, STEEL, GLOW_FROST, GLOW_FROST)),
        // Dark as a thundercloud, grey wings, lightning in its seams
        Look::Squallwing => small(1.05, hide(DARK_STEEL, OBSIDIAN, STEEL, FUR_GRAY, BONE, GLOW_STORM, GLOW_STORM)),
        // Moss-green and black, its wings like old leather, venom glowing in it
        Look::Blightwing => small(1.0, hide(MOSS, FUR_DARK, FUR_CREAM, LEATHER, BONE, GLOW_POISON, GLOW_POISON)),
        // Crimson and black, crested, with burning eyes: a great wyvern of the peaks
        Look::PyreMatriarch => Spec {
            s: 3.3,
            four: false,
            necks: 3,
            tails: 4,
            neck: 0.4,
            tail: 0.56,
            girth: 1.05,
            span: 1.18,
            hide: hide(CLOTH, OBSIDIAN, BRONZE, TEMPLE_RED, BONE, GLOW_FIRE, FLAME),
            crown: Crown::Crest,
            wing: WingShape::Scalloped,
            club: false,
        },
        // Black as cooled lava, its belly molten, short in the wing and heavy, a club on its tail
        Look::CindermawTyrant => Spec {
            s: 3.4,
            four: true,
            necks: 2,
            tails: 4,
            neck: 0.42,
            tail: 0.55,
            girth: 1.38,
            span: 0.82,
            hide: hide(OBSIDIAN, BARK_DARK, GLOW_FIRE, LEATHER, STONE, GLOW_FIRE, FLAME),
            crown: Crown::Swept,
            wing: WingShape::Broad,
            club: true,
        },
        // Plated with the gold it's lain on, steel-grey wings, lightning in its eyes and seams
        Look::GildscaleHoardwarden => Spec {
            s: 3.0,
            four: true,
            necks: 3,
            tails: 4,
            neck: 0.4,
            tail: 0.55,
            girth: 1.12,
            span: 1.0,
            hide: hide(GOLD, BRONZE, LINEN, DARK_STEEL, BONE, GLOW_STORM, GLOW_STORM),
            crown: Crown::Gilded,
            wing: WingShape::Scalloped,
            club: false,
        },
        // The drakes. The Ember Drake brick-red, its wings dark leather, a horn on its snout, fire
        // in its eyes and its belly
        Look::Drake => drake(1.3, hide(TEMPLE_RED, OBSIDIAN, SANDSTONE, LEATHER, BONE, GLOW_FIRE, FLAME), Crown::Horned, WingShape::Scalloped),
        // Deep green, stocky and short in the wing, its wings barbed and a frill spotted with venom
        // round its head
        Look::VenomDrake => Spec { girth: 1.22, span: 0.84, ..drake(1.3, hide(PINE, OBSIDIAN, MOSS, CACTUS, BONE, GLOW_POISON, GLOW_POISON), Crown::Frill, WingShape::Barbed) },
        // Pale and long, a wyrm of the snows: a long neck and tail, broad teal wings, crowned with
        // ice
        Look::FrostWyrm => Spec { neck: 0.36, tail: 0.62, girth: 0.92, span: 1.08, ..drake(1.35, hide(ICE, STEEL, FUR_WHITE, TEMPLE_TEAL, CRYSTAL, GLOW_FROST, GLOW_FROST), Crown::Frost, WingShape::Broad) },
        // Steel-grey, a wyvern of two legs and long narrow wings, lightning in its fingers, its
        // horns forked
        Look::StormDrake => Spec { four: false, girth: 1.0, span: 1.3, tail: 0.56, ..drake(1.3, hide(STEEL, OBSIDIAN, LINEN, MORTAR, BONE, GLOW_STORM, GLOW_STORM), Crown::Forked, WingShape::Long) },
        // The Elder Cinder Wyrm, black as soot and huge, embers in its scales, its wings in tatters
        // and its curled horns gone to gold
        Look::ElderWyrm => Spec { girth: 1.3, span: 1.05, neck: 0.36, tail: 0.56, ..drake(2.4, hide(OBSIDIAN, BARK_DARK, TEMPLE_RED, LEATHER, GOLD, GLOW_FIRE, FLAME), Crown::Elder, WingShape::Tattered) },
        // Ash-grey as its brood, veined with fire, crowned with bone, wings of deep ember
        _ => Spec {
            s: 5.0,
            four: true,
            necks: 3,
            tails: 4,
            neck: 0.45,
            tail: 0.6,
            girth: 1.2,
            span: 1.2,
            hide: hide(MORTAR, OBSIDIAN, BRONZE, CLOTH, BONE, GLOW_FIRE, FLAME),
            crown: Crown::Ancient,
            wing: WingShape::Scalloped,
            club: false,
        },
    }
}

type Vec3 = [f64; 3];

/// Where the joints are on the torso, in units from its middle, facing +x with +y to its left:
/// the foot of the neck, the root of the tail, the left wing's shoulder, the left hind hip and the
/// left fore shoulder
fn neck_root(sp: &Spec) -> Vector3<f64> {
    Vector3::new(0.6, 0.0, 0.16 * sp.girth)
}

fn tail_root(sp: &Spec) -> Vector3<f64> {
    Vector3::new(-0.68, 0.0, 0.04 * sp.girth)
}

fn shoulder(sp: &Spec) -> Vector3<f64> {
    Vector3::new(0.3, 0.26 * sp.girth, 0.22 * sp.girth)
}

fn hip(sp: &Spec, hind: bool) -> Vector3<f64> {
    if hind { Vector3::new(-0.36, 0.25 * sp.girth, -0.14 * sp.girth) } else { Vector3::new(0.42, 0.24 * sp.girth, -0.16 * sp.girth) }
}

/// How long its legs are, thigh and shin, in units
fn leg_lengths(hind: bool) -> (f64, f64) {
    if hind { (0.42, 0.5) } else { (0.38, 0.45) }
}

/// Where the jaw's hinged on the head, and its mouth, in units of the head's own frame
const JAW_HINGE: Vec3 = [0.06, 0.0, -0.07];
const MOUTH: Vec3 = [0.5, 0.0, -0.08];
/// How long the head is to the end of its snout, in units
const HEAD_LENGTH: f64 = 0.56;
/// How long a wing's inner and outer pieces are, in units before its span, and where on the inner
/// the wrist is
const WING_IN: f64 = 1.05;
const WING_OUT: f64 = 1.45;

fn wrist(sp: &Spec) -> Vector3<f64> {
    Vector3::new(0.1, WING_IN * sp.span, 0.03)
}

/// The legs standing: hind and fore, how far forward the thigh's swung and how far the knee's
/// bent, in radians
const STAND_HIND: (f64, f64) = (0.5, 1.0);
const STAND_FORE: (f64, f64) = (0.2, 0.45);

/// How high the middle of its body stands over its feet, in meters, its legs as they stand
fn stand_height(sp: &Spec) -> f64 {
    let drop = |hind: bool, (swing, knee): (f64, f64)| {
        let (thigh, shin) = leg_lengths(hind);
        -hip(sp, hind).z + thigh * swing.cos() + shin * (swing - knee).cos() + ANKLE
    };
    let hind = drop(true, STAND_HIND);
    let h = if sp.four { (hind + drop(false, STAND_FORE)) * 0.5 } else { hind };
    h * sp.s
}

/// How high its ankle is over the ground, standing, in units
const ANKLE: f64 = 0.05;
/// How far forward of straight down from the hip its knee's set, in units
const KNEE_AHEAD: f64 = 0.04;

/// Where the ankle of a leg with its thigh swung `swing` forward and its knee bent `knee` is, from
/// its hip, in units of its body's frame: how far ahead, and up
fn ankle_at(hind: bool, swing: f64, knee: f64) -> (f64, f64) {
    let (lt, ls) = leg_lengths(hind);
    let (kx, kz) = (KNEE_AHEAD * swing.cos() + lt * swing.sin(), KNEE_AHEAD * swing.sin() - lt * swing.cos());
    (kx + ls * (swing - knee).sin(), kz - ls * (swing - knee).cos())
}

/// How far forward a leg's thigh has to swing and its knee bend for its ankle to be at `x` ahead of
/// its hip and `z` up, in units of its body's frame, its knee forward of the line down to it; as
/// near as it reaches, if it doesn't
fn reach_to(hind: bool, x: f64, z: f64) -> (f64, f64) {
    let (lt, ls) = leg_lengths(hind);
    let thigh = (lt * lt + KNEE_AHEAD * KNEE_AHEAD).sqrt();
    let offset = KNEE_AHEAD.atan2(lt);
    let d = (x * x + z * z).sqrt().clamp((thigh - ls).abs() + 1e-3, thigh + ls - 1e-3);
    let down = x.atan2(-z);
    let at_hip = ((thigh * thigh + d * d - ls * ls) / (2.0 * thigh * d)).clamp(-1.0, 1.0).acos();
    let at_knee = ((thigh * thigh + ls * ls - d * d) / (2.0 * thigh * ls)).clamp(-1.0, 1.0).acos();
    let toward_knee = down + at_hip;
    (toward_knee - offset, PI - at_knee - offset)
}

/// The pieces each dragon is made of, its models in this order, see model_of()
const PIECES: usize = 18;
/// What's drawn of a dragon, each piece where it is: torso, three pieces of neck, head, jaw, four
/// of tail, the left wing's inner and outer, the right's, then the thighs and shins of the hind
/// legs left and right and the fore legs
const BONES: usize = 22;
const TORSO: usize = 0;
const NECK: usize = 1;
const HEAD: usize = 4;
const JAW: usize = 5;
const TAIL: usize = 6;
const WING: usize = 10;
const THIGH: usize = 14;
const SHIN: usize = 18;

/// Which of a dragon's models bone `b` is drawn with
fn model_of(b: usize) -> usize {
    match b {
        _ if b < WING + 4 => b,
        // A thigh, hind ones then fore ones, left and right drawn alike
        _ if b < SHIN => if b - THIGH < 2 { 14 } else { 16 },
        _ => if b - SHIN < 2 { 15 } else { 17 },
    }
}

/// What dragons breathe and leave burning, after their pieces among their models: a billow of
/// breath of each element in two shapes, flames rising off the ground of each in two, and two
/// lengths of a crack glowing with what's under it
pub const FX_PUFF: usize = 0;
pub const FX_FLAME: usize = 8;
pub const FX_CRACK: usize = 16;
const FX: usize = 18;

/// How many models dragons have, after the frames of every look, see animals::DRAGON_MODELS_AT
pub const MODELS: usize = KINDS.len() * PIECES + FX;

/// The four elements dragons breathe, in the order their effects' models are in
pub fn element_slot(e: Element) -> usize {
    match e {
        Element::Frost => 1,
        Element::Storm => 2,
        Element::Poison => 3,
        _ => 0,
    }
}

/// The model of one of the effects, FX_PUFF and the rest, among all models
pub fn fx_model(fx: usize) -> u32 {
    (animals::DRAGON_MODELS_AT + KINDS.len() * PIECES + fx) as u32
}

/// Where each of the effects' models has the middle of its bottom, in its own meters, and how big
/// it is in voxels, see layout()
pub fn fx_shape(fx: usize) -> (Vector3<f64>, [u32; 3]) {
    layout()[KINDS.len() * PIECES + fx]
}

// The pieces, each built in its own frame with its joint at the middle, in units: the right wing's
// mirrored from the left's by `side`

fn torso(p: &mut Vec<Part>, sp: &Spec) {
    let (h, g) = (sp.hide, sp.girth);
    // Fire glowing along its belly between the plates, as through cracks
    for side in [-1.0, 1.0] {
        p.push(limb([0.5, side * 0.24 * g, -0.16 * g], [-0.55, side * 0.21 * g, -0.12 * g], 0.024, 0.018, h.glow));
    }
    match sp.crown {
        // Molten veins over the flanks of the oldest, and embers in an elder wyrm's scales
        Crown::Ancient | Crown::Elder => {
            for (k, side) in [(0, 1.0), (1, -1.0), (2, 1.0), (3, -1.0)] {
                let x = 0.45 - k as f64 * 0.3;
                p.push(limb([x, side * 0.3 * g, 0.12 * g], [x - 0.22, side * 0.33 * g, -0.06 * g], 0.022, 0.016, h.glow));
            }
            if sp.crown == Crown::Elder {
                for k in 0..10 {
                    let x = 0.55 - k as f64 * 0.12;
                    let side = if k % 2 == 0 { 1.0 } else { -1.0 };
                    p.push(ball([x, side * (0.2 + 0.04 * (k % 3) as f64) * g, (0.24 - 0.03 * (k % 2) as f64) * g], 0.028, h.glow));
                }
            }
        }
        Crown::Gilded => {
            // Coins and gems grown into the scales of its back
            for k in 0..9 {
                let x = 0.5 - k as f64 * 0.13;
                let side = if k % 2 == 0 { 1.0 } else { -1.0 };
                p.push(ellipsoid([x, side * 0.16 * g, 0.29 * g], [0.06, 0.06, 0.03], if k % 3 == 0 { CRYSTAL } else { GOLD }));
            }
        }
        // Lightning forking down its flanks
        Crown::Forked => {
            for side in [-1.0, 1.0] {
                let zig = [[0.5, 0.3, 0.14], [0.32, 0.33, 0.02], [0.18, 0.31, 0.12], [-0.02, 0.34, -0.02], [-0.2, 0.31, 0.1], [-0.42, 0.29, -0.03]];
                for pair in zig.windows(2) {
                    let (a, b) = (pair[0], pair[1]);
                    p.push(limb([a[0], side * a[1] * g, a[2] * g], [b[0], side * b[1] * g, b[2] * g], 0.02, 0.016, h.glow));
                }
            }
        }
        // Venom welling in spots along its back
        Crown::Frill => {
            for k in 0..8 {
                let x = 0.5 - k as f64 * 0.15;
                for side in [-1.0, 1.0] {
                    p.push(ball([x, side * 0.2 * g, 0.24 * g], 0.032, h.glow));
                }
            }
        }
        _ => {}
    }
    if sp.club {
        for k in 0..5 {
            let x = 0.4 - k as f64 * 0.22;
            p.push(limb([x, 0.0, 0.33 * g], [x - 0.12, 0.0, 0.2 * g], 0.03, 0.02, h.glow));
        }
    }
    // Belly plates
    for k in 0..7 {
        let x = 0.5 - k as f64 * 0.17;
        p.push(ellipsoid([x, 0.0, -0.25 * g], [0.075, 0.22 * g, 0.08 * g], h.belly));
    }
    // Spines down its back, tallest over the shoulders, a crest's taller still, and an old wyrm's
    let tall = match sp.crown {
        Crown::Crest => 1.5,
        Crown::Elder | Crown::Frost => 1.3,
        _ => 1.0,
    };
    for k in 0..8 {
        let x = 0.55 - k as f64 * 0.16;
        let up = (0.1 + 0.08 * (1.0 - ((x - 0.25) / 0.7).abs()).max(0.0)) * tall;
        p.push(limb([x, 0.0, 0.29 * g], [x - 0.09, 0.0, 0.29 * g + up], 0.045 * g.sqrt(), 0.008, h.horn));
    }
    // Where the wings and legs join it
    for side in [-1.0, 1.0] {
        p.push(ball([0.3, side * 0.25 * g, 0.2 * g], 0.11 * g, h.dark));
        if sp.four {
            p.push(ball([0.42, side * 0.22 * g, -0.1 * g], 0.15 * g, h.scale));
        }
        p.push(ball([-0.36, side * 0.22 * g, -0.08 * g], 0.15 * g, h.scale));
    }
    p.push(ellipsoid([0.34, 0.0, 0.03 * g], [0.4, 0.37 * g, 0.38 * g], h.scale));
    p.push(ellipsoid([0.0, 0.0, 0.0], [0.74, 0.34 * g, 0.35 * g], h.scale));
    p.push(ellipsoid([-0.46, 0.0, -0.02 * g], [0.34, 0.3 * g, 0.3 * g], h.scale));
}

/// How thick its neck is `f` of the way up it, 0 at the chest
fn neck_radius(sp: &Spec, f: f64) -> f64 {
    (0.2 - 0.085 * f) * sp.girth.sqrt()
}

fn neck(p: &mut Vec<Part>, sp: &Spec, k: usize) {
    let h = sp.hide;
    let n = sp.necks as f64;
    let (r0, r1) = (neck_radius(sp, k as f64 / n), neck_radius(sp, (k + 1) as f64 / n));
    let l = sp.neck;
    let tall = match sp.crown {
        Crown::Crest => 1.6,
        Crown::Elder | Crown::Frost => 1.3,
        _ => 1.0,
    };
    for j in 0..2 {
        let x = l * (0.25 + 0.45 * j as f64);
        let r = r0 + (r1 - r0) * x / l;
        p.push(limb([x, 0.0, r * 0.8], [x - 0.07, 0.0, r * 0.8 + 0.09 * tall], 0.035, 0.007, h.horn));
    }
    // Lightning down a storm drake's neck, and venom spots down a venom drake's
    match sp.crown {
        Crown::Forked => {
            for side in [-1.0, 1.0] {
                p.push(limb([0.0, side * r0 * 0.8, 0.0], [l * 0.5, side * r0 * 0.85, r0 * 0.3], 0.018, 0.015, h.glow));
                p.push(limb([l * 0.5, side * r0 * 0.85, r0 * 0.3], [l, side * r1 * 0.8, -r1 * 0.1], 0.015, 0.014, h.glow));
            }
        }
        Crown::Frill => {
            for side in [-1.0, 1.0] {
                p.push(ball([l * 0.5, side * r0 * 0.6, r0 * 0.6], 0.03, h.glow));
            }
        }
        _ => {}
    }
    p.push(limb([0.0, 0.0, -r0 * 0.45], [l, 0.0, -r1 * 0.45], r0 * 0.7, r1 * 0.7, h.belly));
    p.push(limb([0.0, 0.0, 0.0], [l, 0.0, 0.0], r0, r1, h.scale));
}

fn head(p: &mut Vec<Part>, sp: &Spec) {
    let h = sp.hide;
    for side in [-1.0, 1.0] {
        p.push(ball([0.22, side * 0.105, 0.06], 0.036, h.eye));
        // Its nostrils, smoking with what it breathes
        p.push(ball([0.52, side * 0.04, 0.0], 0.02, h.glow));
        // Teeth down the edge of its upper jaw
        for k in 0..3 {
            let x = 0.27 + k as f64 * 0.08;
            p.push(limb([x, side * 0.055, -0.06], [x, side * 0.05, -0.105], 0.014, 0.005, BONE));
        }
        // A ridge over each eye
        p.push(limb([0.12, side * 0.1, 0.11], [0.27, side * 0.08, 0.085], 0.032, 0.02, h.dark));
        match sp.crown {
            Crown::Swept => {
                p.push(limb([0.04, side * 0.09, 0.1], [-0.24, side * 0.17, 0.2], 0.05, 0.026, h.horn));
                p.push(limb([-0.24, side * 0.17, 0.2], [-0.38, side * 0.15, 0.33], 0.026, 0.008, h.horn));
            }
            Crown::Crest => {
                p.push(limb([0.04, side * 0.09, 0.1], [-0.3, side * 0.12, 0.16], 0.045, 0.012, h.horn));
                for k in 0..3 {
                    let x = 0.14 - k as f64 * 0.1;
                    p.push(limb([x, side * 0.03, 0.12], [x - 0.12, side * 0.04, 0.26 - k as f64 * 0.03], 0.025, 0.006, h.horn));
                }
            }
            Crown::Gilded => {
                p.push(limb([0.04, side * 0.09, 0.1], [-0.22, side * 0.2, 0.18], 0.05, 0.03, h.horn));
                p.push(limb([-0.22, side * 0.2, 0.18], [-0.33, side * 0.18, 0.31], 0.03, 0.01, GOLD));
                p.push(ball([-0.1, side * 0.145, 0.14], 0.042, GOLD));
            }
            Crown::Ancient => {
                p.push(limb([0.04, side * 0.09, 0.11], [-0.26, side * 0.22, 0.24], 0.055, 0.03, h.horn));
                p.push(limb([-0.26, side * 0.22, 0.24], [-0.42, side * 0.18, 0.42], 0.03, 0.008, h.horn));
                p.push(limb([0.0, side * 0.12, 0.04], [-0.3, side * 0.27, 0.0], 0.04, 0.008, h.horn));
                // Frills at its cheeks, of the skin of its wings on bony spines
                p.push(limb([0.05, side * 0.13, -0.02], [-0.15, side * 0.24, 0.06], 0.02, 0.008, h.horn));
                p.push(ellipsoid([-0.04, side * 0.17, 0.02], [0.1, 0.03, 0.07], h.wing));
            }
            Crown::Horned => {
                // Swept back over its brow, heavier than a drakeling's, and spikes at its cheeks
                p.push(limb([0.06, side * 0.09, 0.1], [-0.22, side * 0.16, 0.18], 0.058, 0.032, h.horn));
                p.push(limb([-0.22, side * 0.16, 0.18], [-0.36, side * 0.13, 0.31], 0.032, 0.008, h.horn));
                for k in 0..2 {
                    let x = 0.14 - k as f64 * 0.12;
                    p.push(limb([x, side * 0.12, -0.03], [x - 0.15, side * 0.21, -0.07 - k as f64 * 0.02], 0.026, 0.006, h.horn));
                }
            }
            Crown::Frost => {
                // Spikes of ice fanned back from its brow, longer the further round
                for k in 0..4 {
                    let f = k as f64;
                    p.push(limb([0.08 - 0.06 * f, side * (0.07 + 0.025 * f), 0.11], [-0.2 - 0.07 * f, side * (0.12 + 0.07 * f), 0.24 - 0.04 * f], 0.03, 0.006, h.horn));
                }
                p.push(limb([0.1, side * 0.12, -0.03], [-0.12, side * 0.22, -0.06], 0.022, 0.005, h.horn));
            }
            Crown::Frill => {
                // A fan of skin on spines behind its head, venom spotted on it, and short horns
                p.push(limb([0.04, side * 0.08, 0.1], [-0.12, side * 0.12, 0.18], 0.04, 0.012, h.horn));
                for k in 0..4 {
                    let a = 0.35 + k as f64 * 0.38;
                    let tip = [-0.12 - 0.08 * a.cos().abs(), side * (0.1 + 0.26 * a.sin()), 0.04 + 0.26 * a.cos()];
                    p.push(limb([0.0, side * 0.09, 0.04], tip, 0.02, 0.006, h.horn));
                    p.push(ball([tip[0] * 0.75, tip[1] * 0.8, tip[2] * 0.8], 0.026, h.glow));
                }
                p.push(ellipsoid([-0.08, side * 0.2, 0.08], [0.05, 0.13, 0.15], h.wing));
            }
            Crown::Forked => {
                // Antlered like lightning, glowing at each tip
                p.push(limb([0.04, side * 0.09, 0.1], [-0.22, side * 0.15, 0.27], 0.048, 0.02, h.horn));
                p.push(limb([-0.22, side * 0.15, 0.27], [-0.36, side * 0.12, 0.42], 0.02, 0.008, h.horn));
                p.push(limb([-0.1, side * 0.12, 0.19], [-0.04, side * 0.22, 0.34], 0.02, 0.007, h.horn));
                p.push(ball([-0.36, side * 0.12, 0.42], 0.025, h.glow));
                p.push(ball([-0.04, side * 0.22, 0.34], 0.022, h.glow));
            }
            Crown::Elder => {
                // Curled right round like a ram's, thick at the root, and its brow heavy over them
                let curl = [[0.02, 0.1, 0.11], [-0.14, 0.17, 0.17], [-0.27, 0.23, 0.1], [-0.3, 0.27, -0.04], [-0.2, 0.29, -0.14], [-0.06, 0.26, -0.15]];
                let n = curl.len() - 1;
                for (k, pair) in curl.windows(2).enumerate() {
                    let (a, b) = (pair[0], pair[1]);
                    let r = 0.07 - 0.05 * k as f64 / n as f64;
                    p.push(limb([a[0], side * a[1], a[2]], [b[0], side * b[1], b[2]], r, r - 0.01, h.horn));
                }
                p.push(limb([0.1, side * 0.12, -0.04], [-0.1, side * 0.2, -0.1], 0.03, 0.008, h.dark));
            }
        }
    }
    match sp.crown {
        Crown::Ancient => p.push(limb([0.1, 0.0, 0.12], [-0.25, 0.0, 0.3], 0.04, 0.01, h.horn)),
        // The horn on an Ember Drake's snout
        Crown::Horned => p.push(limb([0.38, 0.0, 0.05], [0.45, 0.0, 0.2], 0.04, 0.008, h.horn)),
        // A spike of ice down the middle of the frost wyrm's crown
        Crown::Frost => p.push(limb([0.12, 0.0, 0.12], [-0.22, 0.0, 0.3], 0.035, 0.006, h.horn)),
        _ => {}
    }
    p.push(ellipsoid([0.1, 0.0, 0.02], [0.2, 0.15, 0.13], h.scale));
    p.push(limb([0.18, 0.0, 0.0], [0.5, 0.0, -0.03], 0.1, 0.066, h.scale));
}

fn jaw(p: &mut Vec<Part>, sp: &Spec) {
    let h = sp.hide;
    for side in [-1.0, 1.0] {
        for k in 0..3 {
            let x = 0.2 + k as f64 * 0.08;
            p.push(limb([x, side * 0.05, 0.02], [x, side * 0.047, 0.06], 0.012, 0.005, BONE));
        }
    }
    // What it breathes, glowing in its throat, seen as it opens its mouth
    p.push(ellipsoid([0.2, 0.0, 0.035], [0.16, 0.045, 0.03], h.glow));
    p.push(limb([0.0, 0.0, 0.0], [0.42, 0.0, -0.02], 0.08, 0.05, h.scale));
    p.push(limb([0.02, 0.0, -0.03], [0.36, 0.0, -0.04], 0.06, 0.035, h.belly));
}

fn tail_radius(sp: &Spec, f: f64) -> f64 {
    (0.24 - 0.19 * f) * sp.girth.sqrt()
}

fn tail(p: &mut Vec<Part>, sp: &Spec, k: usize) {
    let h = sp.hide;
    let n = sp.tails as f64;
    let (r0, r1) = (tail_radius(sp, k as f64 / n), tail_radius(sp, (k + 1) as f64 / n));
    let l = sp.tail;
    let last = k + 1 == sp.tails;
    if last {
        if sp.club {
            // A club of bone, spiked all round
            for j in 0..6 {
                let a = j as f64 / 6.0 * TAU;
                p.push(limb([-l - 0.08, 0.0, 0.0], [-l - 0.08 - 0.05, a.cos() * 0.26, a.sin() * 0.26], 0.05, 0.008, h.horn));
            }
            p.push(ellipsoid([-l - 0.1, 0.0, 0.0], [0.2, 0.15, 0.15], h.dark));
        } else {
            // A spade of the skin of its wings, ridged with bone
            for side in [-1.0, 1.0] {
                p.push(limb([-l + 0.05, 0.0, 0.0], [-l - 0.18, side * 0.17, 0.0], 0.02, 0.01, h.horn));
            }
            p.push(limb([-l, 0.0, 0.0], [-l - 0.32, 0.0, 0.0], 0.03, 0.01, h.horn));
            p.push(ellipsoid([-l - 0.13, 0.0, 0.0], [0.2, 0.15, 0.025], h.wing));
        }
    }
    for j in 0..2 {
        let x = -l * (0.3 + 0.45 * j as f64);
        let r = r0 + (r1 - r0) * (-x / l);
        p.push(limb([x, 0.0, r * 0.8], [x - 0.07, 0.0, r * 0.8 + 0.07], 0.03, 0.006, h.horn));
    }
    p.push(limb([0.0, 0.0, -r0 * 0.4], [-l, 0.0, -r1 * 0.4], r0 * 0.7, r1 * 0.7, h.belly));
    p.push(limb([0.0, 0.0, 0.0], [-l, 0.0, 0.0], r0, r1, h.scale));
}

/// A strip of a wing's skin from `a` on its leading edge to `b` on its trailing, the two at the
/// same distance out along the wing, `width` wide
fn skin(a: Vec3, b: Vec3, width: f64, material: u8) -> Part {
    let r = 0.032;
    limb(a, b, r, r, material).stretched([1.0, (width * 0.62 / r).max(1.0), 1.0])
}

/// The inner piece of a wing, from the shoulder out to the wrist: the bone of its arm, a claw at the
/// wrist, and the skin back from it to the body
fn wing_in(p: &mut Vec<Part>, sp: &Spec, side: f64) {
    let h = sp.hide;
    let w = wrist(sp);
    let w = [w.x, side * w.y, w.z];
    p.push(limb(w, [w[0] + 0.13, w[1] + side * 0.02, w[2] - 0.07], 0.028, 0.006, h.horn));
    p.push(ball(w, 0.06, h.dark));
    p.push(limb([0.0, 0.0, 0.0], w, 0.065, 0.045, h.dark));
    // A rib of the arm's tendons back across the skin
    let reach = w[1].abs();
    p.push(limb([0.05, side * reach * 0.55, 0.02], [-0.6, side * reach * 0.55, 0.0], 0.016, 0.012, h.dark));
    // How far back its skin runs to the body, a long wing's narrower
    let chord = match sp.wing {
        WingShape::Long => 0.68,
        WingShape::Broad => 0.9,
        _ => 0.82,
    };
    let strips = 9;
    for i in 0..strips {
        let f = (i as f64 + 0.5) / strips as f64;
        let y = side * (0.04 + (reach - 0.04) * f);
        let lead = [0.1 * f, y, 0.03 * f];
        // An old wing's torn at the edge
        let torn = if sp.wing == WingShape::Tattered && i % 3 == 1 { 0.16 } else { 0.0 };
        let trail = [-chord + 0.3 * f - 0.05 * (PI * f).sin() + torn, y, 0.0];
        p.push(skin(lead, trail, (reach - 0.04) / strips as f64, h.wing));
    }
}

/// The trailing edge of a wing's outer piece of `shape`, `y` out along it from the wrist, in units:
/// in between the fingers it's drawn in towards the leading edge
fn trailing(y: f64, l: f64, shape: WingShape) -> f64 {
    let points: &[(f64, f64)] = match shape {
        WingShape::Broad => &[(0.0, -0.76), (0.18, -0.7), (0.35, -0.86), (0.52, -0.78), (0.68, -0.84), (0.8, -0.7), (0.92, -0.6), (1.0, -0.36)],
        WingShape::Long => &[(0.0, -0.5), (0.2, -0.42), (0.38, -0.56), (0.55, -0.42), (0.7, -0.47), (0.84, -0.32), (0.93, -0.32), (1.0, -0.2)],
        WingShape::Tattered => &[(0.0, -0.6), (0.12, -0.36), (0.2, -0.66), (0.3, -0.44), (0.4, -0.8), (0.5, -0.4), (0.62, -0.7), (0.71, -0.34), (0.8, -0.62), (0.9, -0.3), (1.0, -0.3)],
        _ => &[(0.0, -0.62), (0.16, -0.52), (0.32, -0.78), (0.47, -0.6), (0.62, -0.72), (0.74, -0.5), (0.86, -0.6), (1.0, -0.32)],
    };
    let f = (y / l).clamp(0.0, 1.0);
    for pair in points.windows(2) {
        let ((f0, x0), (f1, x1)) = (pair[0], pair[1]);
        if f <= f1 {
            return x0 + (x1 - x0) * (f - f0) / (f1 - f0);
        }
    }
    points[points.len() - 1].1
}

/// Where a wing's fingers end, as far out along it and how far back, in units of its length and
/// units: at the points of its trailing edge
fn fingers(shape: WingShape) -> [(f64, f64); 3] {
    match shape {
        WingShape::Broad => [(0.35, -0.86), (0.68, -0.84), (0.92, -0.6)],
        WingShape::Long => [(0.38, -0.56), (0.7, -0.47), (0.93, -0.32)],
        WingShape::Tattered => [(0.4, -0.8), (0.62, -0.7), (0.8, -0.62)],
        _ => [(0.32, -0.78), (0.62, -0.72), (0.86, -0.6)],
    }
}

/// The outer piece, from the wrist out to the tip: the long bones of its fingers and the skin
/// between them
fn wing_out(p: &mut Vec<Part>, sp: &Spec, side: f64) {
    let h = sp.hide;
    let l = WING_OUT * sp.span;
    let tip = [-0.32, side * l, 0.03];
    let mid = [tip[0] * 0.5, tip[1] * 0.5, tip[2] * 0.5];
    // A storm drake's fingers crackle with lightning
    let bone = if sp.crown == Crown::Forked { h.glow } else { h.dark };
    p.push(limb([0.0, 0.0, 0.0], mid, 0.045, 0.032, h.dark));
    p.push(limb(mid, tip, 0.032, 0.008, h.dark));
    for (f, x) in fingers(sp.wing) {
        p.push(limb([0.0, 0.0, 0.0], [x, side * f * l, 0.0], 0.026, 0.008, bone));
        // Barbs of bone out past the skin
        if sp.wing == WingShape::Barbed {
            p.push(limb([x * 0.95, side * f * l, 0.0], [x - 0.14, side * (f * l + 0.04), -0.01], 0.02, 0.005, h.horn));
        }
    }
    let strips = 13;
    for i in 0..strips {
        // Starting a little inside the wrist, so there's no gap there as the wing bends
        let y = -0.04 + (l + 0.04) * (i as f64 + 0.5) / strips as f64;
        let lead = [-0.32 * (y / l).max(0.0), side * y, 0.03 * (y / l).max(0.0)];
        let trail = [trailing(y, l, sp.wing), side * y, 0.0];
        // Holed here and there with age, the skin between its fingers torn through
        if sp.wing == WingShape::Tattered && (i == 4 || i == 9) {
            let gap = (lead[0] - trail[0]) * 0.4;
            p.push(skin(lead, [lead[0] - gap, trail[1], 0.0], (l + 0.04) / strips as f64, h.wing));
            p.push(skin([trail[0] + gap * 0.5, trail[1], 0.0], trail, (l + 0.04) / strips as f64, h.wing));
        } else if lead[0] - trail[0] > 0.03 {
            p.push(skin(lead, trail, (l + 0.04) / strips as f64, h.wing));
        }
    }
}

/// A thigh, from the hip down to the knee
fn thigh(p: &mut Vec<Part>, sp: &Spec, hind: bool) {
    let (h, g) = (sp.hide, sp.girth);
    let (lt, _) = leg_lengths(hind);
    p.push(ellipsoid([0.03, 0.0, -lt * 0.35], [0.13 * g, 0.11 * g, 0.2], h.scale));
    p.push(limb([0.0, 0.0, 0.0], [KNEE_AHEAD, 0.0, -lt], 0.12 * g, 0.085 * g, h.scale));
}

/// A shin and its foot, from the knee at `knee` angled `lean` radians forward from straight down,
/// its foot turned `tilt` radians about the ankle from lying flat in this frame, so it's flat on
/// the ground as the leg stands
fn shin_and_foot(p: &mut Vec<Part>, sp: &Spec, hind: bool, knee: Vec3, lean: f64, tilt: f64) {
    let (h, g) = (sp.hide, sp.girth);
    let (_, ls) = leg_lengths(hind);
    let ankle = [knee[0] + ls * lean.sin(), 0.0, knee[2] - ls * lean.cos()];
    let (c, s) = (tilt.cos(), tilt.sin());
    let at = |x: f64, y: f64, z: f64| [ankle[0] + x * c + z * s, y, ankle[2] - x * s + z * c];
    for y in [-0.05, 0.0, 0.05] {
        p.push(limb(at(0.12, y * g, -0.01), at(0.23, y * 1.3 * g, -0.05), 0.025, 0.006, h.horn));
    }
    p.push(limb(at(-0.06, 0.0, -0.01), at(-0.13, 0.0, -0.05), 0.02, 0.006, h.horn));
    for y in [-0.04, 0.04] {
        p.push(limb(at(-0.07, y * g, -0.01), at(0.15, y * 1.2 * g, -0.015), 0.045, 0.04, h.dark));
    }
    p.push(ball(knee, 0.08 * g, h.dark));
    p.push(limb(knee, ankle, 0.075 * g, 0.06 * g, h.scale));
}

fn shin(p: &mut Vec<Part>, sp: &Spec, hind: bool) {
    // Built with the knee at its joint and the shin straight down, the foot turned so it's flat on
    // the ground as the leg stands
    let (swing, knee) = if hind { STAND_HIND } else { STAND_FORE };
    shin_and_foot(p, sp, hind, [0.0, 0.0, 0.0], 0.0, swing - knee);
}

/// The shapes of piece `piece` of a dragon of kind `k`, in meters
fn piece_parts(k: usize, piece: usize) -> Vec<Part> {
    let sp = spec(KINDS[k]);
    let mut p = Vec::new();
    match piece {
        TORSO => torso(&mut p, &sp),
        _ if (NECK..HEAD).contains(&piece) => {
            if piece - NECK < sp.necks {
                neck(&mut p, &sp, piece - NECK);
            }
        }
        HEAD => head(&mut p, &sp),
        JAW => jaw(&mut p, &sp),
        _ if (TAIL..WING).contains(&piece) => {
            if piece - TAIL < sp.tails {
                tail(&mut p, &sp, piece - TAIL);
            }
        }
        10 => wing_in(&mut p, &sp, 1.0),
        11 => wing_out(&mut p, &sp, 1.0),
        12 => wing_in(&mut p, &sp, -1.0),
        13 => wing_out(&mut p, &sp, -1.0),
        14 => thigh(&mut p, &sp, true),
        15 => shin(&mut p, &sp, true),
        16 => {
            if sp.four {
                thigh(&mut p, &sp, false);
            }
        }
        _ => {
            if sp.four {
                shin(&mut p, &sp, false);
            }
        }
    }
    p.iter().map(|part| part.scaled(sp.s)).collect()
}

/// The shapes of effect `fx`, in meters: a billow of breath a meter across, flames a meter and a
/// bit high, a crack a meter long
fn fx_parts(fx: usize) -> Vec<Part> {
    let mut p = Vec::new();
    let looks = [(FLAME, GLOW_FIRE, OBSIDIAN), (CRYSTAL, GLOW_FROST, ICE), (GLOW_HOLY, GLOW_STORM, DARK_STEEL), (GLOW_POISON, MOSS, FUR_DARK)];
    match fx {
        _ if fx < FX_FLAME => {
            let (core, glow, _) = looks[(fx - FX_PUFF) / 2];
            let second = (fx - FX_PUFF) % 2 == 1;
            // A hot heart in a lumpy billow, each shape different
            p.push(ball([0.0, 0.0, 0.5], 0.24, core));
            let lumps: &[(Vec3, f64)] = if second {
                &[([0.2, 0.12, 0.62], 0.26), ([-0.18, -0.1, 0.4], 0.28), ([0.05, -0.22, 0.6], 0.22), ([-0.1, 0.2, 0.38], 0.24)]
            } else {
                &[([0.18, -0.1, 0.4], 0.27), ([-0.2, 0.15, 0.6], 0.26), ([0.0, 0.22, 0.36], 0.22), ([0.06, -0.2, 0.66], 0.2)]
            };
            for &(c, r) in lumps {
                p.push(ball(c, r, glow));
            }
            p.push(ball([0.0, 0.0, 0.5], 0.36, glow));
        }
        _ if fx < FX_CRACK => {
            let (core, glow, _) = looks[(fx - FX_FLAME) / 2];
            let second = (fx - FX_FLAME) % 2 == 1;
            // Tongues of flame licking up, each tapering to its tip
            let tongues: &[(Vec3, f64, f64)] = if second {
                &[([0.0, 0.0, 1.25], 0.2, 0.0), ([0.25, 0.15, 0.85], 0.15, 0.1), ([-0.2, 0.2, 0.95], 0.15, -0.1), ([0.05, -0.28, 0.7], 0.13, 0.15)]
            } else {
                &[([0.05, 0.05, 1.1], 0.2, 0.0), ([-0.25, -0.1, 0.9], 0.15, 0.1), ([0.2, -0.22, 0.75], 0.14, -0.1), ([-0.05, 0.28, 0.8], 0.13, 0.15)]
            };
            for &(tip, r, lean) in tongues {
                let foot = [tip[0] * 0.4, tip[1] * 0.4, 0.05];
                p.push(limb(foot, [tip[0] * 0.7 + lean, tip[1] * 0.7, tip[2] * 0.55], r * 0.55, r * 0.25, core));
                p.push(limb(foot, [tip[0] + lean, tip[1], tip[2]], r, 0.02, glow));
            }
        }
        _ => {
            // A jagged split in the ground three meters long, glowing, its lips dark
            let second = fx == FX_CRACK + 1;
            let kinks: &[f64] = if second { &[0.0, 0.14, -0.1, 0.12, -0.08, 0.1, 0.0] } else { &[0.0, -0.12, 0.1, -0.06, 0.12, -0.1, 0.0] };
            for (i, pair) in kinks.windows(2).enumerate() {
                let x0 = -1.5 + i as f64 * 0.5;
                p.push(limb([x0, pair[0], 0.07], [x0 + 0.5, pair[1], 0.07], 0.08, 0.08, GLOW_FIRE));
                p.push(limb([x0, pair[0], 0.0], [x0 + 0.5, pair[1], 0.0], 0.17, 0.17, OBSIDIAN).stretched([1.0, 1.0, 0.45]));
            }
        }
    }
    p
}

/// Every dragon's pieces and every effect's model, voxelized
fn build_models() -> Vec<(Model, Vector3<f64>)> {
    let jobs = KINDS.len() * PIECES + FX;
    let next = std::sync::atomic::AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 8);
    let mut made: Vec<(usize, (Model, Vector3<f64>))> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let job = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if job >= jobs {
                            break out;
                        }
                        let parts = if job < KINDS.len() * PIECES { piece_parts(job / PIECES, job % PIECES) } else { fx_parts(job - KINDS.len() * PIECES) };
                        let made = if parts.is_empty() { (Model { size: [1, 1, 1], voxels: vec![AIR] }, Vector3::zeros()) } else { voxelize(&parts, MODEL_RES) };
                        out.push((job, made));
                    }
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("making a dragon's model failed")).collect()
    });
    made.sort_unstable_by_key(|(job, _)| *job);
    let mut made: Vec<(Model, Vector3<f64>)> = made.into_iter().map(|(_, m)| m).collect();
    // Pieces drawn from files in assets\models, see src\assets.rs
    crate::assets::dragons(&mut made);
    made
}

/// Where each model has the middle of its bottom, in its own meters, see layout(): for the model
/// files, see src\assets.rs
pub fn anchors() -> Vec<Vector3<f64>> {
    layout().iter().map(|(anchor, _)| *anchor).collect()
}

static LAYOUT: OnceLock<Vec<(Vector3<f64>, [u32; 3])>> = OnceLock::new();

/// Where each model has the middle of its bottom, in its own meters, and how big it is in voxels
fn layout() -> &'static [(Vector3<f64>, [u32; 3])] {
    LAYOUT.get_or_init(|| build_models().into_iter().map(|(m, anchor)| (anchor, m.size)).collect())
}

/// The dragons' models, to go after the frames of every look, see animals::animal_models()
pub fn models() -> Vec<Model> {
    let made = build_models();
    let _ = LAYOUT.set(made.iter().map(|(m, anchor)| (*anchor, m.size)).collect());
    made.into_iter().map(|(m, _)| m).collect()
}

// The rig: how each joint's turned. Its body, neck, head, tail and legs are sprung, each joint
// swinging after where its moves want it and carried on a little past, those out along its neck and
// tail looser and slower than those by its body, so they lag behind as it turns, bobs and lunges and
// whip round after; its wings beat and its feet step on top of that.

/// A wing's joints: how far it's raised at the shoulder, swept back, its leading edge tipped down,
/// how far its outer piece is folded back on the inner at the wrist, and bent up there
#[derive(Clone, Copy, Default, Debug)]
struct WingPose {
    flap: f64,
    sweep: f64,
    twist: f64,
    fold: f64,
    bend: f64,
}

/// A leg's joints: how far forward the thigh's swung and how far the knee's bent; and how far its
/// foot's planted on the ground rather than held as those say, 0 to 1, and how far from where it'd
/// stand it's planted, in units ahead and up
#[derive(Clone, Copy, Default, Debug)]
struct LegPose {
    swing: f64,
    knee: f64,
    plant: f64,
    foot: [f64; 2],
}

/// Every joint of a dragon, in radians: its body's pitch, nose up, its roll, left wing up, and its
/// twist, turned to its left, as its hips and shoulders roll; how far its body's moved from where
/// it'd be, in units ahead, to its left and up; each joint of its neck turned to its left and raised;
/// its head the same, and its jaw opened; each joint of its tail the same; its wings, left then
/// right; and its legs, the hind left and right, then the fore
#[derive(Clone, Copy, Default, Debug)]
struct Rig {
    pitch: f64,
    roll: f64,
    twist: f64,
    shift: [f64; 3],
    neck: [[f64; 2]; 3],
    head: [f64; 2],
    jaw: f64,
    tail: [[f64; 2]; 4],
    wings: [WingPose; 2],
    legs: [LegPose; 4],
}

/// How many of a rig's numbers are sprung, all but its wings', and where in Rig::flat() each lot
/// starts
const SPRUNG: usize = 43;
const AT_SHIFT: usize = 3;
const AT_NECK: usize = 6;
const AT_HEAD: usize = 12;
const AT_JAW: usize = 14;
const AT_TAIL: usize = 15;
const AT_LEGS: usize = 23;

/// Wings folded at its sides, standing: the arm swept back from the shoulder, the skin hanging
/// down the flank from it, the fingers folded over at the wrist like a book shut, back along it
/// towards the chest
const FOLDED: WingPose = WingPose { flap: 0.1, sweep: 1.5, twist: -1.35, fold: 0.1, bend: -2.9 };
/// Wings spread, gliding, held a little up
const SPREAD: WingPose = WingPose { flap: 0.14, sweep: 0.1, twist: 0.0, fold: 0.06, bend: 0.06 };

impl Rig {
    /// Standing on the ground as it does when nothing's the matter
    fn standing(sp: &Spec) -> Rig {
        let mut r = Rig { pitch: if sp.four { 0.0 } else { 0.12 }, ..Default::default() };
        // The neck rising steeply from the chest, curving over to hold the head out level at its end
        let n = sp.necks as f64;
        r.neck[0] = [0.0, 0.85];
        for k in 1..3 {
            r.neck[k] = [0.0, -0.68 / (n - 1.0).max(1.0)];
        }
        r.head = [0.0, -0.22];
        r.tail[0] = [0.0, -0.22];
        for k in 1..4 {
            r.tail[k] = [0.0, 0.07];
        }
        r.wings = [FOLDED; 2];
        let (hind, fore) = (LegPose { swing: STAND_HIND.0, knee: STAND_HIND.1, plant: 1.0, foot: [0.0; 2] }, LegPose { swing: STAND_FORE.0, knee: STAND_FORE.1, plant: 1.0, foot: [0.0; 2] });
        r.legs = [hind, hind, fore, fore];
        r
    }

    /// Flying: neck stretched out ahead, tail straight behind, legs tucked up
    fn flying() -> Rig {
        let mut r = Rig::default();
        r.neck[0] = [0.0, 0.2];
        r.neck[1] = [0.0, -0.04];
        r.head = [0.0, -0.18];
        r.tail[0] = [0.0, 0.06];
        r.wings = [SPREAD; 2];
        let (hind, fore) = (LegPose { swing: -0.95, knee: 1.5, ..Default::default() }, LegPose { swing: -0.7, knee: 1.3, ..Default::default() });
        r.legs = [hind, hind, fore, fore];
        r
    }

    /// All its sprung numbers in a row, see SPRUNG
    fn flat(&self) -> [f64; SPRUNG] {
        let mut v = [0.0; SPRUNG];
        v[0] = self.pitch;
        v[1] = self.roll;
        v[2] = self.twist;
        v[AT_SHIFT..AT_NECK].copy_from_slice(&self.shift);
        for k in 0..3 {
            v[AT_NECK + 2 * k..AT_NECK + 2 * k + 2].copy_from_slice(&self.neck[k]);
        }
        v[AT_HEAD..AT_JAW].copy_from_slice(&self.head);
        v[AT_JAW] = self.jaw;
        for k in 0..4 {
            v[AT_TAIL + 2 * k..AT_TAIL + 2 * k + 2].copy_from_slice(&self.tail[k]);
        }
        for (k, l) in self.legs.iter().enumerate() {
            v[AT_LEGS + 5 * k..AT_LEGS + 5 * k + 5].copy_from_slice(&[l.swing, l.knee, l.plant, l.foot[0], l.foot[1]]);
        }
        v
    }

    fn set_flat(&mut self, v: &[f64; SPRUNG]) {
        (self.pitch, self.roll, self.twist) = (v[0], v[1], v[2]);
        self.shift.copy_from_slice(&v[AT_SHIFT..AT_NECK]);
        for k in 0..3 {
            self.neck[k] = [v[AT_NECK + 2 * k], v[AT_NECK + 2 * k + 1]];
        }
        self.head = [v[AT_HEAD], v[AT_HEAD + 1]];
        self.jaw = v[AT_JAW];
        for k in 0..4 {
            self.tail[k] = [v[AT_TAIL + 2 * k], v[AT_TAIL + 2 * k + 1]];
        }
        for (k, l) in self.legs.iter_mut().enumerate() {
            let at = AT_LEGS + 5 * k;
            *l = LegPose { swing: v[at], knee: v[at + 1], plant: v[at + 2], foot: [v[at + 3], v[at + 4]] };
        }
    }
}

/// How quickly each of a rig's sprung numbers swings to where it's wanted, in radians a second at
/// a drakeling's size, and how much it's damped, 1 to come to it without going past: the joints out
/// along its neck and tail slower and looser than those near its body, so they swing after them
fn springiness(i: usize) -> (f64, f64) {
    match i {
        0 | 1 => (6.5, 0.85),
        2 => (8.0, 0.9),
        AT_SHIFT..AT_NECK => (9.0, 0.8),
        AT_NECK..AT_HEAD => (10.5 - 2.2 * ((i - AT_NECK) / 2) as f64, 0.62),
        AT_HEAD..AT_JAW => (11.5, 0.72),
        AT_JAW => (20.0, 1.0),
        AT_TAIL..AT_LEGS => (7.5 - 1.2 * ((i - AT_TAIL) / 2) as f64, 0.5),
        _ => (14.0, 1.0),
    }
}

/// How much of the turning and heaving of what it hangs from each joint of its neck and tail is
/// left behind by, swinging after it: none for the rest of it
fn inertia(i: usize) -> f64 {
    match i {
        AT_NECK..AT_HEAD => 0.3 + 0.12 * ((i - AT_NECK) / 2) as f64,
        AT_HEAD..AT_JAW => 0.35,
        AT_JAW => 0.0,
        AT_TAIL..AT_LEGS => 0.45 + 0.12 * ((i - AT_TAIL) / 2) as f64,
        _ => 0.0,
    }
}

/// Where a piece's joint is, in voxels, and how it's turned, and whether it's drawn
#[derive(Clone, Copy)]
struct Bone {
    at: Vector3<f64>,
    turn: UnitQuaternion<f64>,
    shown: bool,
}

impl Default for Bone {
    fn default() -> Self {
        Bone { at: Vector3::zeros(), turn: UnitQuaternion::identity(), shown: false }
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

fn flip(v: Vector3<f64>) -> Vector3<f64> {
    Vector3::new(v.x, -v.y, v.z)
}

/// Where the middle of a dragon's body is drawn, its middle at `center` in voxels facing `yaw`, its
/// body moved and turned as `rig` has it; and how it's turned
fn body_frame(sp: &Spec, center: Vector3<f64>, yaw: f64, rig: &Rig, vpm: f64) -> (Vector3<f64>, UnitQuaternion<f64>) {
    (center + rz(yaw) * Vector3::from(rig.shift) * sp.s * vpm, rz(yaw + rig.twist) * ry(-rig.pitch) * rx(rig.roll))
}

/// Where the hip of leg `leg` is on the body, in units: the hind left and right, then the fore
fn hip_of(sp: &Spec, leg: usize) -> Vector3<f64> {
    let joint = hip(sp, leg < 2);
    if leg % 2 == 0 { joint } else { flip(joint) }
}

/// Where every piece of a dragon built as `sp` is, its body's middle at `center` in voxels facing
/// `yaw`, its joints as `rig` has them; and where its mouth is and which way it breathes
fn pose(sp: &Spec, center: Vector3<f64>, yaw: f64, rig: &Rig, vpm: f64, bones: &mut [Bone; BONES]) -> (Vector3<f64>, Vector3<f64>) {
    let m = sp.s * vpm;
    let (center, body) = body_frame(sp, center, yaw, rig, vpm);
    let world = |local: Vector3<f64>| center + body * local * m;
    bones[TORSO] = Bone { at: center, turn: body, shown: true };
    // The neck, a joint at a time from the chest, then the head and its jaw: not turned with the
    // twist of its shoulders, so its head's held steady as it walks
    let mut at = world(neck_root(sp));
    let mut turn = body * rz(-rig.twist);
    for k in 0..3 {
        if k < sp.necks {
            turn *= rz(rig.neck[k][0]) * ry(-rig.neck[k][1]);
            bones[NECK + k] = Bone { at, turn, shown: true };
            at += turn * Vector3::new(sp.neck, 0.0, 0.0) * m;
        } else {
            bones[NECK + k].shown = false;
        }
    }
    let head = turn * rz(rig.head[0]) * ry(-rig.head[1]);
    bones[HEAD] = Bone { at, turn: head, shown: true };
    let hinge = at + head * Vector3::from(JAW_HINGE) * m;
    let jaw = head * ry(rig.jaw);
    bones[JAW] = Bone { at: hinge, turn: jaw, shown: true };
    let mouth = at + head * Vector3::from(MOUTH) * m;
    // Breathing out between its jaws, half way between where each points
    let breath = (head * ry(rig.jaw * 0.5) * Vector3::x()).normalize();
    // The tail, a joint at a time from the hips
    let mut at = world(tail_root(sp));
    let mut turn = body;
    for k in 0..4 {
        if k < sp.tails {
            turn *= rz(rig.tail[k][0]) * ry(rig.tail[k][1]);
            bones[TAIL + k] = Bone { at, turn, shown: true };
            at += turn * Vector3::new(-sp.tail, 0.0, 0.0) * m;
        } else {
            bones[TAIL + k].shown = false;
        }
    }
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
    // The legs, each a thigh from the hip and a shin from the knee
    for leg in 0..4 {
        let hind = leg < 2;
        let shown = hind || sp.four;
        let l = rig.legs[leg];
        let thigh = ry(-l.swing);
        let joint = hip(sp, hind);
        let (lt, _) = leg_lengths(hind);
        let knee = joint + thigh * Vector3::new(KNEE_AHEAD, 0.0, -lt);
        let shin = thigh * ry(l.knee);
        let (thigh, shin, joint, knee) = if leg % 2 == 0 { (thigh, shin, joint, knee) } else { (mirrored(thigh), mirrored(shin), flip(joint), flip(knee)) };
        bones[THIGH + leg] = Bone { at: world(joint), turn: body * thigh, shown };
        bones[SHIN + leg] = Bone { at: world(knee), turn: body * shin, shown };
    }
    (mouth, breath)
}

/// Lines through each part of a dragon, from one end to the other in voxels, with how thick round
/// it is in meters: for finding which part of it the player's hitting, see Dragon::aim()
fn limbs(sp: &Spec, bones: &[Bone; BONES], vpm: f64) -> Vec<(Vector3<f64>, Vector3<f64>, f64)> {
    let m = sp.s * vpm;
    let mut out = Vec::new();
    let torso = bones[TORSO];
    out.push((torso.at + torso.turn * Vector3::new(0.55, 0.0, 0.0) * m, torso.at + torso.turn * Vector3::new(-0.6, 0.0, 0.0) * m, 0.32 * sp.girth * sp.s));
    for k in 0..sp.necks {
        let b = bones[NECK + k];
        out.push((b.at, b.at + b.turn * Vector3::new(sp.neck, 0.0, 0.0) * m, neck_radius(sp, k as f64 / sp.necks as f64) * sp.s));
    }
    let h = bones[HEAD];
    out.push((h.at, h.at + h.turn * Vector3::new(HEAD_LENGTH, 0.0, 0.0) * m, 0.15 * sp.s));
    for k in 0..sp.tails {
        let b = bones[TAIL + k];
        out.push((b.at, b.at + b.turn * Vector3::new(-sp.tail, 0.0, 0.0) * m, tail_radius(sp, k as f64 / sp.tails as f64) * sp.s));
    }
    // Through the middle of the skin of each wing, rather than along its bones
    for side in 0..2 {
        let y = if side == 0 { 1.0 } else { -1.0 };
        let (inner, outer) = (bones[WING + side * 2], bones[WING + side * 2 + 1]);
        let reach = WING_IN * sp.span;
        out.push((inner.at + inner.turn * Vector3::new(-0.3, 0.1 * y, 0.0) * m, inner.at + inner.turn * Vector3::new(-0.2, y * reach, 0.0) * m, 0.35 * sp.s));
        out.push((outer.at + outer.turn * Vector3::new(-0.3, 0.0, 0.0) * m, outer.at + outer.turn * Vector3::new(-0.25, y * WING_OUT * sp.span * 0.85, 0.0) * m, 0.3 * sp.s));
    }
    for leg in 0..4 {
        if leg >= 2 && !sp.four {
            continue;
        }
        let (_, ls) = leg_lengths(leg < 2);
        let (b, shin) = (bones[THIGH + leg], bones[SHIN + leg]);
        let foot = shin.at + shin.turn * Vector3::new(0.0, 0.0, -ls) * m;
        out.push((b.at, foot, 0.12 * sp.girth * sp.s));
    }
    out
}

/// How far through a beat of its wings the downstroke takes, the rest bringing them back up
const DOWNSTROKE: f64 = 0.42;

/// How far up and down a wing beating is, 1 at the top of its beat and -1 at the bottom, `u` of
/// the way through it from the top: quickly down, more slowly up
fn stroke(u: f64) -> f64 {
    if u < DOWNSTROKE { (PI * u / DOWNSTROKE).cos() } else { -(PI * (u - DOWNSTROKE) / (1.0 - DOWNSTROKE)).cos() }
}

/// How far through the downstroke a wing is, 0 to 1 and back to 0, and how far folded through the
/// upstroke, the same, `u` of the way through its beat: folded most a little before half way up,
/// and open again a moment before the top
fn strokes(u: f64) -> (f64, f64) {
    if u < DOWNSTROKE { ((PI * u / DOWNSTROKE).sin(), 0.0) } else { (0.0, (PI * ((u - DOWNSTROKE) / (0.95 - DOWNSTROKE)).min(1.0)).sin()) }
}

/// A wing held at `base` beating `amp` hard, `u` of the way through a beat from the top of it:
/// driven down hard and spread wide, its leading edge tipped down into it and its tip lagging up,
/// then folded at the wrist to bring it back up, the tip trailing down behind, and opened again at
/// the top. The more `hover`, the further its beat swings forward and back, as it hangs in the air.
fn beating(base: WingPose, amp: f64, hover: f64, u: f64) -> WingPose {
    let c = stroke(u);
    let (down, up) = strokes(u);
    // Its upstroke's fold at its height in the middle of it, held a little longer than a sine
    let up = up.powf(0.75);
    WingPose {
        flap: base.flap + amp * (0.95 * c + 0.06),
        sweep: base.sweep + amp * (-0.14 * down + 0.45 * up + 0.5 * hover * c),
        twist: base.twist + amp * (0.3 * down - 0.35 * up),
        fold: base.fold + amp * 1.5 * up,
        bend: base.bend + amp * (0.38 * down - 1.1 * up),
    }
}

/// How high its body's carried by its wings, beating, -1 to 1, `u` of the way through a beat:
/// lifted all through the downstroke, sinking as they come up again, a little behind them
fn lift(u: f64) -> f64 {
    let u = (u - 0.08).rem_euclid(1.0);
    if u < DOWNSTROKE + 0.04 { -(PI * u / (DOWNSTROKE + 0.04)).cos() } else { (PI * (u - DOWNSTROKE - 0.04) / (0.96 - DOWNSTROKE)).cos() }
}

/// The point of the segment from `a` to `b` nearest the line out from `origin` along `dir`, which
/// has length 1, ahead of it: how far it is from the line, and how far along it
fn nearest_to_line(a: Vector3<f64>, b: Vector3<f64>, origin: Vector3<f64>, dir: Vector3<f64>) -> (Vector3<f64>, f64, f64) {
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

// What dragons do

/// Each thing a dragon can do
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Move {
    /// Rearing its head back and snapping at what's in front of it
    Bite,
    /// Coiling round and sweeping its tail out low all round it, to be jumped
    Tail,
    /// Rearing up with fire in its throat, then sweeping its breath across the ground ahead
    Breath,
    /// Raising its wings high and beating them down, throwing back whatever's in front of it
    Gust,
    /// A great one rearing up and coming down on its forefeet, the ground cracking open
    Stomp,
    /// Calling down the sky: a roar, and what it calls coming
    Roar,
    /// Diving out of the sky to rake the player with its claws as it passes low, and away
    Swoop,
    /// Lining up a run and flying it low, breathing a line of fire across the ground below
    Strafe,
    /// Hovering in beside the player, over their head, and biting down at them
    Hover,
    /// A ball of its element spat at the player
    Spit,
    /// A great one climbing, then falling on the ground where the player stood
    Dive,
    /// A great one climbing high and raining its element down all round the player
    Rain,
}

const MOVES: usize = 12;

/// Something a dragon does: in the air or on the ground, from how near to how far in meters, how
/// many seconds till it can again, getting ready and getting over it, from which phase of a great
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

/// A drakeling's: snapping and lashing its tail and breathing on the ground; swooping, strafing,
/// hovering in to bite and spitting from the air
const DRAKELING: [Skill; 8] = [
    skill(Bite, false, 0.0, 2.7, 1.8, 0.5, 0.6, 0, 3.0),
    skill(Tail, false, 0.0, 2.8, 6.0, 0.7, 0.8, 0, 1.2),
    skill(Breath, false, 2.5, 9.0, 8.0, 0.8, 0.8, 0, 1.5),
    skill(Gust, false, 0.0, 5.0, 10.0, 0.7, 0.6, 0, 0.8),
    skill(Swoop, true, 5.0, 34.0, 5.0, 0.6, 0.3, 0, 3.0),
    skill(Strafe, true, 6.0, 36.0, 10.0, 0.75, 0.4, 0, 1.6),
    skill(Hover, true, 0.0, 30.0, 9.0, 0.45, 0.4, 0, 1.4),
    skill(Spit, true, 7.0, 32.0, 4.5, 0.65, 0.3, 0, 1.5),
];

/// The Pyre Matriarch lives in the air: strafing runs, swoops and dives, beating the player back
/// with her wings, calling her brood once hurt, and raining fire at the last
const MATRIARCH: [Skill; 11] = [
    skill(Bite, false, 0.0, 7.0, 2.2, 0.65, 0.8, 0, 2.0),
    skill(Tail, false, 0.0, 9.0, 6.5, 0.85, 1.0, 0, 1.5),
    skill(Breath, false, 5.0, 20.0, 8.0, 0.9, 1.0, 0, 2.0),
    skill(Gust, false, 0.0, 13.0, 10.0, 0.85, 0.8, 0, 1.0),
    skill(Roar, true, 0.0, 70.0, 24.0, 0.6, 0.4, 1, 3.0),
    skill(Swoop, true, 8.0, 60.0, 6.0, 0.8, 0.4, 0, 2.0),
    skill(Strafe, true, 10.0, 70.0, 8.0, 0.85, 0.5, 0, 3.0),
    skill(Dive, true, 6.0, 55.0, 12.0, 1.2, 0.4, 0, 2.0),
    skill(Gust, true, 6.0, 22.0, 11.0, 0.9, 0.6, 1, 1.2),
    skill(Rain, true, 0.0, 70.0, 18.0, 1.0, 0.6, 2, 3.0),
    skill(Spit, true, 10.0, 60.0, 6.0, 0.8, 0.4, 0, 1.0),
];

/// The Cindermaw Tyrant keeps to the ground: snapping, lashing its club, stamping the ground open,
/// breathing and lobbing magma; it takes to the air only to fall on the player, and at the last to
/// rain fire
const TYRANT: [Skill; 9] = [
    skill(Bite, false, 0.0, 7.5, 2.2, 0.7, 0.8, 0, 2.0),
    skill(Tail, false, 0.0, 10.0, 5.5, 0.9, 1.0, 0, 2.0),
    skill(Breath, false, 5.0, 22.0, 7.0, 0.95, 1.0, 0, 2.0),
    skill(Stomp, false, 0.0, 9.0, 7.0, 1.0, 1.3, 0, 2.5),
    skill(Spit, false, 8.0, 45.0, 7.0, 0.8, 0.7, 0, 1.5),
    skill(Roar, false, 0.0, 70.0, 24.0, 0.6, 0.4, 1, 3.0),
    skill(Dive, true, 6.0, 55.0, 10.0, 1.2, 0.4, 0, 3.0),
    skill(Strafe, true, 10.0, 60.0, 12.0, 0.9, 0.5, 1, 1.0),
    skill(Rain, true, 0.0, 60.0, 18.0, 1.0, 0.6, 2, 2.0),
];

/// The Gildscale Hoardwarden: lightning breathed and called down out of the sky on the ground,
/// its wings beating, squallwings called to it; and short flights to swoop and strafe
const WARDEN: [Skill; 10] = [
    skill(Bite, false, 0.0, 6.5, 2.0, 0.65, 0.8, 0, 2.0),
    skill(Tail, false, 0.0, 9.0, 6.0, 0.85, 1.0, 0, 1.5),
    skill(Breath, false, 5.0, 20.0, 7.5, 0.9, 1.0, 0, 2.0),
    skill(Gust, false, 0.0, 12.0, 9.0, 0.8, 0.8, 0, 1.5),
    skill(Rain, false, 4.0, 50.0, 14.0, 1.0, 0.6, 1, 2.5),
    skill(Roar, false, 0.0, 70.0, 24.0, 0.6, 0.4, 1, 3.0),
    skill(Swoop, true, 8.0, 55.0, 6.0, 0.8, 0.4, 0, 2.0),
    skill(Strafe, true, 10.0, 60.0, 8.0, 0.85, 0.5, 0, 2.5),
    skill(Spit, true, 9.0, 55.0, 5.0, 0.75, 0.4, 0, 1.5),
    skill(Rain, true, 0.0, 60.0, 16.0, 1.0, 0.6, 2, 2.0),
];

/// The Worldpyre Ancient: everything the rest do and more, each in its phase
const ANCIENT: [Skill; 13] = [
    skill(Bite, false, 0.0, 11.0, 2.4, 0.75, 0.9, 0, 2.0),
    skill(Tail, false, 0.0, 16.0, 6.0, 1.0, 1.1, 0, 2.0),
    skill(Breath, false, 7.0, 34.0, 7.0, 1.0, 1.0, 0, 2.5),
    skill(Stomp, false, 0.0, 14.0, 6.5, 1.1, 1.4, 0, 2.5),
    skill(Gust, false, 0.0, 22.0, 10.0, 1.0, 0.8, 0, 1.2),
    skill(Roar, false, 0.0, 100.0, 26.0, 0.6, 0.4, 2, 3.0),
    skill(Roar, true, 0.0, 100.0, 26.0, 0.6, 0.4, 2, 3.0),
    skill(Strafe, true, 14.0, 100.0, 7.0, 1.0, 0.6, 0, 3.0),
    skill(Swoop, true, 10.0, 90.0, 9.0, 1.0, 0.5, 0, 1.5),
    skill(Dive, true, 8.0, 80.0, 11.0, 1.3, 0.4, 1, 2.0),
    skill(Rain, true, 0.0, 100.0, 14.0, 1.1, 0.6, 0, 2.0),
    skill(Spit, true, 14.0, 90.0, 6.0, 0.9, 0.4, 0, 1.0),
    skill(Gust, true, 8.0, 26.0, 12.0, 1.0, 0.6, 3, 1.0),
];

/// A drake's: what drakes always did, snapping at what's close, breathing, spitting its element and
/// calling it down out of the sky on whoever keeps their distance; lashing its tail and beating its
/// wings at whoever crowds it; and now and then taking to the air to swoop, strafe and spit from it
const DRAKE: [Skill; 10] = [
    skill(Bite, false, 0.0, 3.6, 2.4, 0.6, 0.8, 0, 3.0),
    skill(Tail, false, 0.0, 3.9, 6.0, 0.75, 0.9, 0, 1.2),
    skill(Breath, false, 2.0, 10.0, 6.5, 0.8, 1.0, 0, 2.2),
    skill(Gust, false, 0.0, 6.5, 10.0, 0.75, 0.6, 0, 0.8),
    skill(Spit, false, 5.0, 35.0, 3.0, 0.6, 0.5, 0, 1.6),
    skill(Rain, false, 10.0, 30.0, 9.0, 1.0, 0.9, 0, 1.3),
    skill(Swoop, true, 6.0, 40.0, 6.0, 0.7, 0.4, 0, 2.0),
    skill(Strafe, true, 7.0, 45.0, 9.0, 0.8, 0.5, 0, 2.2),
    skill(Spit, true, 8.0, 40.0, 4.0, 0.7, 0.4, 0, 1.4),
    skill(Rain, true, 0.0, 40.0, 12.0, 1.0, 0.6, 0, 0.8),
];

/// The Elder Cinder Wyrm's: a drake's, heavier, and stamping the ground open; once it's hurt
/// roaring, falling on the player out of the sky and bringing the sky down, its fire falling all
/// round them
const ELDER: [Skill; 13] = [
    skill(Bite, false, 0.0, 6.0, 2.4, 0.65, 0.8, 0, 2.5),
    skill(Tail, false, 0.0, 7.0, 6.0, 0.85, 1.0, 0, 1.5),
    skill(Breath, false, 3.0, 14.0, 6.5, 0.9, 1.0, 0, 2.2),
    skill(Stomp, false, 0.0, 7.5, 7.0, 1.0, 1.2, 0, 2.0),
    skill(Gust, false, 0.0, 10.0, 10.0, 0.85, 0.7, 0, 1.0),
    skill(Spit, false, 6.0, 35.0, 3.0, 0.65, 0.5, 0, 1.4),
    skill(Rain, false, 8.0, 35.0, 9.0, 1.0, 0.9, 0, 1.2),
    skill(Roar, false, 0.0, 60.0, 24.0, 0.6, 0.4, 1, 1.0),
    skill(Swoop, true, 8.0, 50.0, 6.0, 0.8, 0.4, 0, 2.0),
    skill(Strafe, true, 10.0, 55.0, 8.0, 0.85, 0.5, 0, 2.5),
    skill(Dive, true, 6.0, 50.0, 11.0, 1.2, 0.4, 1, 2.0),
    skill(Rain, true, 0.0, 55.0, 14.0, 1.0, 0.6, 1, 2.5),
    skill(Spit, true, 10.0, 50.0, 5.0, 0.75, 0.4, 0, 1.2),
];

fn skills(look: Look) -> &'static [Skill] {
    match look {
        Look::PyreMatriarch => &MATRIARCH,
        Look::CindermawTyrant => &TYRANT,
        Look::GildscaleHoardwarden => &WARDEN,
        Look::WorldpyreAncient => &ANCIENT,
        Look::ElderWyrm => &ELDER,
        _ if is_drake(look) => &DRAKE,
        _ => &DRAKELING,
    }
}

/// How far through its health a great one goes into each next phase of its fight
pub fn phases(look: Look) -> &'static [f64] {
    match look {
        Look::WorldpyreAncient => &[0.7, 0.4, 0.15],
        _ if look.is_raid() => &[0.66, 0.33],
        _ => &[],
    }
}

/// How long it means to keep to the air, and then to the ground, in seconds at least and at most,
/// in phase `stage` of its fight
fn budgets(look: Look, stage: u8) -> ([f64; 2], [f64; 2]) {
    match look {
        Look::PyreMatriarch => ([16.0, 22.0], [6.0, 9.0]),
        Look::CindermawTyrant => ([4.0, 6.0], [16.0, 22.0]),
        Look::GildscaleHoardwarden => ([8.0, 11.0], [10.0, 14.0]),
        Look::WorldpyreAncient => match stage {
            // Strafing the arena from on high, coming down only now and then
            0 => ([24.0, 30.0], [5.0, 6.0]),
            // Down on the ground with the player, the arena burning
            1 => ([5.0, 7.0], [20.0, 26.0]),
            2 => ([11.0, 14.0], [10.0, 12.0]),
            _ => ([9.0, 11.0], [8.0, 10.0]),
        },
        // The elder wyrm takes to the air longer and more often the more it's hurt
        Look::ElderWyrm if stage == 0 => ([6.0, 9.0], [12.0, 16.0]),
        Look::ElderWyrm => ([8.0, 11.0], [10.0, 14.0]),
        // A drake fights mostly on foot, and flies a while now and then
        _ if is_drake(look) => ([5.0, 8.0], [12.0, 18.0]),
        _ => ([9.0, 14.0], [5.0, 8.0]),
    }
}

/// How it flies: meters a second cruising and diving, in a circle how many meters round the player
/// and how high over them, and how many radians a second it turns, how hard it speeds up, and how
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
    match look {
        _ if is_drakeling(look) => Flight { cruise: 9.0, dash: 15.0, orbit: 11.0, altitude: 6.5, turn: 2.6, accel: 12.0, beat: 2.2 },
        Look::WorldpyreAncient => Flight { cruise: 15.0, dash: 26.0, orbit: 34.0, altitude: 20.0, turn: 0.8, accel: 8.0, beat: 0.7 },
        Look::ElderWyrm => Flight { cruise: 12.0, dash: 21.0, orbit: 19.0, altitude: 12.0, turn: 1.3, accel: 9.0, beat: 1.15 },
        // Long in the wing, quick through the air, its beats slow and deep
        Look::StormDrake => Flight { cruise: 12.0, dash: 20.0, orbit: 17.0, altitude: 10.0, turn: 2.0, accel: 12.0, beat: 1.35 },
        _ if is_drake(look) => Flight { cruise: 10.5, dash: 18.0, orbit: 15.0, altitude: 9.0, turn: 1.8, accel: 11.0, beat: 1.6 },
        _ => Flight { cruise: 12.0, dash: 22.0, orbit: 21.0, altitude: 13.0, turn: 1.2, accel: 9.0, beat: 1.1 },
    }
}

/// How hard each of its blows is
struct Power {
    bite: f64,
    claw: f64,
    tail: f64,
    stomp: f64,
    crack: f64,
    breath: f64,
    ball: f64,
    rain: f64,
    gust: f64,
    /// How fast its wings throw the player back, in meters a second
    push: f64,
}

fn power(look: Look) -> Power {
    match look {
        _ if is_drakeling(look) => Power { bite: 11.0, claw: 13.0, tail: 12.0, stomp: 0.0, crack: 0.0, breath: 16.0, ball: 12.0, rain: 0.0, gust: 4.0, push: 7.0 },
        Look::WorldpyreAncient => Power { bite: 34.0, claw: 32.0, tail: 36.0, stomp: 38.0, crack: 22.0, breath: 28.0, ball: 22.0, rain: 20.0, gust: 11.0, push: 12.0 },
        // A raid boss's blows, a third again as hard as a drake's, as they always were
        Look::ElderWyrm => Power { bite: 29.0, claw: 29.0, tail: 28.0, stomp: 30.0, crack: 16.0, breath: 24.0, ball: 24.0, rain: 18.0, gust: 9.0, push: 10.0 },
        // As hard as a drake always hit: its jaws, its breath, frost worst of all, what it spits,
        // venom the least, and what it calls down
        _ if is_drake(look) => {
            let e = element(look);
            let breath = if e == Element::Frost { 28.0 } else { 22.0 };
            let ball = if e == Element::Poison { 16.0 } else { 24.0 };
            let rain = if e == Element::Frost { 13.0 } else { 16.0 };
            Power { bite: 22.0, claw: 22.0, tail: 20.0, stomp: 0.0, crack: 0.0, breath, ball, rain, gust: 6.0, push: 8.0 }
        }
        _ => Power { bite: 26.0, claw: 28.0, tail: 30.0, stomp: 32.0, crack: 18.0, breath: 24.0, ball: 18.0, rain: 17.0, gust: 9.0, push: 10.0 },
    }
}

/// How hard its everyday blows hit, its jaws, its claws and what it spits, on the whole, for
/// bosses::blow()
pub fn blow(look: Look) -> f64 {
    let p = power(look);
    (p.bite + p.claw + p.ball) / 3.0
}

/// How near the player has to come before it turns on them, how far off they can get before it
/// gives up, and how far from home it'll follow them, in meters
fn senses(look: Look) -> (f64, f64, f64) {
    match look {
        _ if is_drakeling(look) => (30.0, 60.0, 90.0),
        Look::WorldpyreAncient => (80.0, 180.0, 260.0),
        Look::ElderWyrm => (34.0, 70.0, 90.0),
        // A little further than the bosses on foot, as it can fly after them
        _ if is_drake(look) => (28.0, 58.0, 72.0),
        _ => (55.0, 110.0, 160.0),
    }
}

/// How long it goes on breathing, and how long it rains its element down and how often one falls,
/// in seconds: the elder wyrm's rain once it's hurt is the sky brought down, a cataclysm
fn breath_length(look: Look) -> f64 {
    match look {
        _ if is_drakeling(look) => 1.3,
        Look::WorldpyreAncient => 2.6,
        Look::ElderWyrm => 2.2,
        _ if is_drake(look) => 1.8,
        _ => 2.1,
    }
}

fn rain_of(look: Look, stage: u8) -> (f64, f64) {
    match look {
        Look::WorldpyreAncient => (3.0, 0.13),
        Look::ElderWyrm if stage >= 1 => (2.8, 0.15),
        // A drake's storm: a few falling on and round the player
        _ if is_drake(look) => (1.5, 0.22),
        _ => (3.0, 0.18),
    }
}

/// What a dragon does that no other boss does, for src\game\dragonfire.rs to see to. Positions are
/// in voxels.
#[derive(Clone, Copy, Debug)]
pub enum Act {
    /// Getting ready to make move `m`, heard as it does, its head at `at`
    Warn { m: Move, at: Vector3<f64> },
    /// A roar: on `first` turning on the player, going into the next phase of its fight, or calling
    /// down `brood`, so many of them; or a drake's as it's `angry`, hurt badly enough
    Roar { at: Vector3<f64>, brood: Option<(Look, usize)>, first: bool, angry: bool },
    /// Jaws snapping shut on `at`, claws raking it as it swoops low, or its tail sweeping round it,
    /// hurting what's within `radius` meters for `damage`
    Bite { at: Vector3<f64>, radius: f64, damage: f64 },
    Claw { at: Vector3<f64>, radius: f64, damage: f64 },
    Sweep { at: Vector3<f64>, radius: f64, damage: f64 },
    /// Its breath going out of `from` along `dir` for `dt` seconds, this hot a second, billowing
    /// out to `size` meters across at `speed` meters a second, `spread` meters to either side
    Breath { from: Vector3<f64>, dir: Vector3<f64>, dt: f64, dps: f64, size: f64, speed: f64, spread: f64 },
    /// A ball of its element spat from `from` to land at `to`, `speed` meters a second, lobbed to
    /// fall on it or flown straight
    Fireball { from: Vector3<f64>, to: Vector3<f64>, speed: f64, damage: f64, radius: f64, lob: bool },
    /// Its element falling out of the sky on `to`
    Rain { to: Vector3<f64>, damage: f64, radius: f64 },
    /// Its wings beating down hard at `at` along `dir`, throwing back what's within `reach` meters
    /// in front at `push` meters a second
    Gust { at: Vector3<f64>, dir: Vector3<f64>, reach: f64, push: f64, damage: f64 },
    /// Coming down hard on `at`, hurting what's within `radius`, and the ground cracking open
    /// along each of `cracks`, radians round from east, NaN for none, `length` meters out
    Stomp { at: Vector3<f64>, radius: f64, damage: f64, cracks: [f64; 6], length: f64, crack: f64 },
    /// Taking off and landing, a downstroke of its wings, and crashing to the ground, `size` big
    TakeOff { at: Vector3<f64>, size: f64 },
    Land { at: Vector3<f64>, size: f64 },
    Wingbeat { at: Vector3<f64>, size: f64 },
    Crash { at: Vector3<f64>, size: f64 },
    /// Its element smouldering at its mouth as it gets ready to breathe it
    Kindle { at: Vector3<f64>, size: f64 },
    /// Its last roar as it falls
    Death { at: Vector3<f64> },
}

/// What a dragon's doing
#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
    /// At home, or going home: perched, or flying back to land there
    Rest,
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
    /// Knocked out of the air, or down on the ground dazed, for so many seconds more once down
    Reel(f64),
    /// Dying
    Fall,
}

/// How its wings are going: where they're held between beats, how far they beat either way, how
/// many beats a second, how far their beat swings forward and back as it hangs in the air, and how
/// much quicker than they would they go from one way of being held to the next
#[derive(Clone, Copy, Debug)]
struct WingSet {
    base: WingPose,
    amp: f64,
    rate: f64,
    hover: f64,
    quick: f64,
}

impl WingSet {
    fn held(base: WingPose) -> WingSet {
        WingSet { base, amp: 0.0, rate: 0.0, hover: 0.0, quick: 1.0 }
    }

    fn beating(base: WingPose, amp: f64, rate: f64) -> WingSet {
        WingSet { base, amp, rate, hover: 0.0, quick: 1.0 }
    }
}

/// Wings half raised off its back, mantled over it: as it rears to breathe, braced
const MANTLED: WingPose = WingPose { flap: 0.42, sweep: 0.95, twist: -0.7, fold: 0.32, bend: -1.7 };
/// Spread wide and raised high, as it roars or crouches to leap
const RAISED: WingPose = WingPose { flap: 0.85, sweep: -0.05, twist: 0.05, fold: 0.05, bend: 0.18 };
/// Swept back tight along its body, diving
const TUCKED: WingPose = WingPose { flap: 0.45, sweep: 0.95, twist: 0.1, fold: 0.95, bend: -0.2 };

/// Where it means to go this frame: what it's after, how fast, what to face, whether it may come
/// right down to the ground, and whether it's only going about and may glide between beats
#[derive(Clone, Copy)]
struct Goal {
    to: Vector3<f64>,
    speed: f64,
    face: Option<Vector3<f64>>,
    low: bool,
    cruise: bool,
}

pub struct Dragon {
    look: Look,
    /// The middle of its body, in voxels, how fast it's going in voxels a second, which way it
    /// faces, and whether it's flying
    center: Vector3<f64>,
    velocity: Vector3<f64>,
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
    aim: Vector3<f64>,
    path: [Vector3<f64>; 3],
    way: Vector3<f64>,
    /// Lined up for its move, as a run once it's got to where it starts from
    ready: bool,
    /// Done what its move does already
    struck: bool,
    /// How many more times it'll do it before it's done, as bites hovering in
    again: u8,
    /// Which way its breath sweeps, and its cracks run, radians round from east
    sweep: f64,
    cracks: [f64; 6],
    /// Where round the player it is circling them, and which way it goes round
    orbit: f64,
    round: f64,
    /// Seconds it's been in the air or on the ground, and how long it means to be
    aloft: f64,
    budget: f64,
    /// Seconds it's been held up walking, and seconds it's been at what it's doing at most
    blocked: f64,
    /// How much health it had last frame, how much it's lost in the last second or so, and
    /// seconds till it can be knocked out of the air again
    health: f64,
    recent: f64,
    reeled: f64,
    /// Whether it can see the player, worked out now and then, and seconds since it last did
    seen: bool,
    look_wait: f64,
    unseen: f64,
    /// Where it wanders to at home, and seconds till it picks somewhere else
    wander: Vector3<f64>,
    wander_wait: f64,
    rig: Rig,
    /// How fast each of its sprung joints is swinging, see Rig::flat()
    spin: [f64; SPRUNG],
    wings: WingSet,
    /// How far through a beat of its wings and a stride of its legs it is, 0 to 1, and how many
    /// meters a stride takes it
    beat: f64,
    stride: f64,
    stride_length: f64,
    /// How much it's walking, 0 standing to 1, and how fast it's going on the ground, in meters a
    /// second
    gait: f64,
    pace: f64,
    /// How far through a breath it is, standing about, 0 to 1
    breathing: f64,
    /// Where it's looking about, idle, turned to its left and up, and seconds till it looks
    /// somewhere else
    gaze: [f64; 2],
    gaze_wait: f64,
    /// Seconds it's still flinching from a heavy blow, and till it can again
    flinch: f64,
    flinch_wait: f64,
    /// The move it made last, and how long it's getting over it all told, for how it does
    last: Option<Move>,
    spent_for: f64,
    /// Seconds it's gliding on for before it beats its wings again, and beats it'll make then
    glide: f64,
    beats: f64,
    /// Which side it falls onto as it dies, 1 or -1
    side: f64,
    /// The ground under the middle of it, in voxels
    ground: f64,
    /// How high its body was drawn last frame in meters, which way it was turned and how far
    /// pitched, and how fast each was going, a little smoothed: for its neck and tail to swing
    /// after it, see animate(). None till it's been drawn once where it is.
    track: Option<([f64; 3], [f64; 3])>,
    bones: [Bone; BONES],
    mouth: Vector3<f64>,
    breath: Vector3<f64>,
    /// The part of it the player's looking at, which is what's hit, see aim()
    focus: Vector3<f64>,
    /// Seconds it's lain dead on the ground, falling still before
    dead: Option<f64>,
    /// Its phase last frame, to see it go into the next
    stage: u8,
    /// Whether its body's been put over where it was made standing, which takes knowing how many
    /// voxels there are to a meter
    placed: bool,
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
            // Up to the top of what's solid here
            let mut top_solid = k;
            while top_solid + 1 <= top && !is_passable(world.material_at(xi, yi, top_solid + 1)) {
                top_solid += 1;
            }
            return Some(top_solid as f64 + 1.0);
        }
        k -= step;
    }
    None
}

/// Whether nothing solid's between `from` and `to`, in voxels, looking half a meter at a time
fn clear_between(world: &World, from: Vector3<f64>, to: Vector3<f64>) -> bool {
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

fn flat(v: Vector3<f64>) -> Vector3<f64> {
    Vector3::new(v.x, v.y, 0.0)
}

/// Smoothly from 0 to 1 as `t` goes from 0 to 1, and quickly at first then slowing into it
fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn ease_out(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

/// A wing pose `f` of the way from `a` to `b`
fn blend(a: WingPose, b: WingPose, f: f64) -> WingPose {
    let m = |x: f64, y: f64| x + (y - x) * f;
    WingPose { flap: m(a.flap, b.flap), sweep: m(a.sweep, b.sweep), twist: m(a.twist, b.twist), fold: m(a.fold, b.fold), bend: m(a.bend, b.bend) }
}

impl Dragon {
    /// A dragon standing with its feet at `at`, in voxels, facing `yaw`
    pub fn new(look: Look, at: Vector3<f64>, yaw: f64, seed: u32) -> Self {
        let sp = spec(look);
        let rig = Rig::standing(&sp);
        Dragon {
            look,
            center: at,
            velocity: Vector3::zeros(),
            yaw,
            airborne: false,
            state: State::Rest,
            clock: 0.0,
            cool: [0.0; MOVES],
            next: 1.5,
            skill: None,
            aim: at,
            path: [at; 3],
            way: Vector3::x(),
            ready: false,
            struck: false,
            again: 0,
            sweep: 0.0,
            cracks: [f64::NAN; 6],
            orbit: 0.0,
            round: if seed % 2 == 0 { 1.0 } else { -1.0 },
            aloft: 0.0,
            budget: 8.0,
            blocked: 0.0,
            health: health(look),
            recent: 0.0,
            reeled: 0.0,
            seen: false,
            look_wait: 0.0,
            unseen: 0.0,
            wander: at,
            wander_wait: 2.0,
            rig,
            spin: [0.0; SPRUNG],
            wings: WingSet::held(FOLDED),
            beat: 0.0,
            stride: 0.0,
            stride_length: stride(look),
            gait: 0.0,
            pace: 0.0,
            breathing: (seed % 100) as f64 / 100.0,
            gaze: [0.0; 2],
            gaze_wait: 1.0,
            flinch: 0.0,
            flinch_wait: 0.0,
            last: None,
            spent_for: 1.0,
            glide: 0.0,
            beats: 2.0,
            side: if seed % 3 == 0 { 1.0 } else { -1.0 },
            ground: at.z,
            track: None,
            bones: [Bone::default(); BONES],
            mouth: at,
            breath: Vector3::x(),
            focus: at,
            dead: None,
            stage: 0,
            placed: false,
            rng: hash(seed ^ 0x51a7),
        }
    }

    fn roll(&mut self) -> f64 {
        self.rng = hash(self.rng.wrapping_add(0x9e37_79b9));
        (self.rng >> 8) as f64 / 16777216.0
    }

    /// The part of it the player's looking at, in voxels, which is what's hit, see aim()
    pub fn focus(&self) -> Vector3<f64> {
        self.focus
    }

    /// The world moved over by `shift` voxels
    pub fn recenter(&mut self, shift: Vector3<f64>) {
        self.center -= shift;
        self.aim -= shift;
        for p in &mut self.path {
            *p -= shift;
        }
        self.wander -= shift;
        self.mouth -= shift;
        self.focus -= shift;
        self.ground -= shift.z;
        // Not seen as a leap up or down by its neck and tail
        self.track = None;
        for b in &mut self.bones {
            b.at -= shift;
        }
    }

    /// Called down out of the sky by a great one's roar: in the air at `at`, in voxels, flying in
    /// towards `toward`, after the player
    pub fn swoop_in(&mut self, at: Vector3<f64>, toward: Vector3<f64>, vpm: f64) {
        self.center = at;
        self.placed = true;
        self.airborne = true;
        self.state = State::Hunt;
        let way = flat(toward - at).try_normalize(1e-6).unwrap_or_else(Vector3::x);
        self.yaw = way.y.atan2(way.x);
        self.velocity = way * flight(self.look).cruise * vpm;
        self.wings = WingSet::beating(SPREAD, 0.72, flight(self.look).beat);
        self.rig = Rig::flying();
        self.spin = [0.0; SPRUNG];
        self.track = None;
        self.budget = 10.0;
        self.next = 2.0 + self.roll() * 2.0;
    }

    /// Picks the part of it the player at `eye`, looking along `dir`, is looking at, or if they're
    /// looking past it, the part nearest them, to be hit: a blow lands on whatever part of a
    /// dragon it's struck at, see Animal::center()
    pub fn aim(&mut self, eye: Vector3<f64>, dir: Vector3<f64>, vpm: f64) {
        let sp = spec(self.look);
        let parts = limbs(&sp, &self.bones, vpm);
        let dir = dir.try_normalize(1e-9).unwrap_or_else(Vector3::x);
        let body = body(self.look).1 * vpm;
        let mut best: Option<(f64, Vector3<f64>)> = None;
        for &(a, b, thick) in &parts {
            let (p, off, along) = nearest_to_line(a, b, eye, dir);
            // Looked straight at, within its thickness: by how near it is along the look
            let reach = thick.max(body / vpm) * vpm;
            if off < reach + 0.3 * vpm {
                let score = along;
                if best.is_none_or(|(s, _)| score < s) {
                    best = Some((score, p));
                }
            }
        }
        // Not looked at: the part nearest the eye, for blows all round the player
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

/// Whether a dragon that's been killed is still falling out of the sky, or crumpling and lying
/// where it fell, so it's not to be taken away yet, see Game::reap()
pub fn dying(a: &Animal) -> bool {
    let Some(d) = &a.dragon else {
        return false;
    };
    let lie = if is_drakeling(a.look) { 2.2 } else { 3.6 };
    a.health <= 0.0 && d.dead.is_none_or(|t| t < lie)
}

/// What everything round it needs to know to act
struct Here<'a> {
    world: &'a World,
    water: WaterLevel<'a>,
    feet: Option<Vector3<f64>>,
    vpm: f64,
    dt: f64,
    sp: Spec,
    look: Look,
    /// Its moves come quicker, angry
    quick: f64,
    stand: f64,
}

/// For looking at them, see `--set dragon_show=1` in main.rs: with nobody about, dragons fight as if
/// someone stood where they were put, to be seen flying and fighting
pub static SHOW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// For looking at them too, see `--set dragon_move=breath` in main.rs: the one move every dragon
/// makes, as soon as it's ready, from wherever it is; its number among Move::ALL and one, or none
pub static FORCED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl Move {
    /// Every move, in the order they're numbered
    pub const ALL: [Move; MOVES] = [Bite, Tail, Breath, Gust, Stomp, Roar, Swoop, Strafe, Hover, Spit, Dive, Rain];

    /// The move called `name`, as it's written here, `breath` or `Swoop`
    pub fn named(name: &str) -> Option<Move> {
        Move::ALL.into_iter().find(|m| format!("{m:?}").eq_ignore_ascii_case(name))
    }
}

fn forced() -> Option<Move> {
    let n = FORCED.load(std::sync::atomic::Ordering::Relaxed);
    (n > 0).then(|| Move::ALL[(n - 1).min(MOVES - 1)])
}

/// What a dragon does this frame, see the top of this file
pub fn act(a: &mut Animal, d: &mut Dragon, world: &World, water: WaterLevel, player: Option<Vector3<f64>>, dt: f64) {
    let vpm = world.voxels_per_meter as f64;
    let look = a.look;
    let sp = spec(look);
    let Some(mut boss) = a.boss.take() else {
        return;
    };
    let player = player.or_else(|| SHOW.load(std::sync::atomic::Ordering::Relaxed).then_some(boss.home + Vector3::new(16.0 * vpm, 0.0, 0.0)));
    let pace = a.status.pace();
    let enraged = boss.enraged;
    let here = Here { world, water, feet: player, vpm, dt: dt * pace.max(0.0), sp, look, quick: if enraged { 0.75 } else { 1.0 }, stand: stand_height(&sp) * vpm };
    if !d.placed {
        d.placed = true;
        d.center = a.position + Vector3::new(0.0, 0.0, here.stand);
        d.wander = a.position;
        d.ground = a.position.z;
    }
    d.clock += dt;
    for c in &mut d.cool {
        *c -= here.dt;
    }
    d.reeled -= dt;
    // How hard it's been hit lately, and whether this blow was hard enough to make it flinch
    let lost = (d.health - a.health).max(0.0);
    d.health = a.health;
    d.recent = d.recent * (-dt / 1.5).exp() + lost;
    d.flinch = (d.flinch - here.dt).max(0.0);
    d.flinch_wait -= here.dt;
    let heavy = a.max_health * if is_drakeling(look) { 0.07 } else if is_drake(look) { 0.018 } else { 0.006 };
    if lost > heavy && a.health > 0.0 && d.flinch_wait <= 0.0 {
        flinch(d, (lost / heavy).min(3.0));
    }
    // Thrown back by a blow, sliding to a stop, or knocked about in the air
    let push = a.status.push * dt;
    if push.norm() > 1e-3 {
        d.center += Vector3::new(push.x, push.y, 0.0);
    }
    a.status.push *= (-6.0 * dt).exp();

    let mut goal = Goal { to: d.center, speed: 0.0, face: None, low: false, cruise: false };
    let mut want = if d.airborne { Rig::flying() } else { Rig::standing(&sp) };
    let mut wings = if d.airborne { WingSet::beating(SPREAD, 0.72, flight(look).beat) } else { WingSet::held(FOLDED) };

    // Healed whole again as it was falling, as when the player dies: up again
    if a.health > 0.0 && d.state == State::Fall {
        d.state = State::Rest;
        d.dead = None;
    }
    if a.health <= 0.0 {
        fall(a, d, &mut boss, &here, &mut goal, &mut want, &mut wings);
    } else {
        // Frozen solid, netted or knocked senseless in the air, a drakeling falls out of it
        if pace <= 0.0 && d.airborne && !matches!(d.state, State::Reel(_)) && is_drakeling(look) {
            d.state = State::Reel(2.0);
        }
        notice(a, d, &mut boss, &here);
        think(a, d, &mut boss, &here, &mut goal, &mut want, &mut wings);
    }
    if pace > 0.0 || a.health <= 0.0 || d.airborne {
        travel(d, &here, &goal, &mut want, &mut wings);
    }
    let beat = d.beat;
    animate(d, &here, &want, &wings, a.health <= 0.0);
    // Each downstroke of its wings heard as it starts
    if d.airborne && d.wings.amp > 0.3 && beat < 0.06 && d.beat >= 0.06 {
        act_out(&mut boss, Act::Wingbeat { at: d.center, size: sp.s });
    }
    // As an animal: its feet under the middle of its body, facing the way it faces
    a.position = d.center - Vector3::new(0.0, 0.0, here.stand);
    a.heading = d.yaw;
    a.target_heading = d.yaw;
    a.speed = 0.0;
    a.mood = animals::Mood::Idle;
    if TRACE.load(std::sync::atomic::Ordering::Relaxed) && (d.clock * 2.0).floor() != ((d.clock - dt) * 2.0).floor() {
        let feet = here.feet.unwrap_or(boss.home);
        println!(
            "dragon {look:?}: {:?} {} engaged {} stage {} {:.1} m off, feet {:.1} m over its ground, next {:.1}, aloft {:.1} of {:.1}, blocked {:.1}, health {:.0}",
            d.state,
            if d.airborne { "flying" } else { "on foot" },
            boss.engaged,
            boss.stage,
            flat(feet - d.center).norm() / vpm,
            (feet.z - ground_z(world, d.center.x, d.center.y, d.center.z, 40.0).unwrap_or(d.center.z)) / vpm,
            d.next,
            d.aloft,
            d.budget,
            d.blocked,
            a.health
        );
    }
    a.boss = Some(boss);
}

/// `--set dragon_trace=1`: what each dragon's doing, twice a second
pub static TRACE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Struck hard, `how` many times harder than it takes: its head and neck thrown back and aside, its
/// body jolted, its jaw thrown open with a cry and its wings jerking half open, for a moment
fn flinch(d: &mut Dragon, how: f64) {
    let side = if d.roll() < 0.5 { 1.0 } else { -1.0 };
    let k = how.sqrt().min(1.6);
    d.flinch = 0.45;
    d.flinch_wait = 0.7;
    d.spin[0] += 1.4 * k;
    d.spin[1] += side * 1.2 * k;
    d.spin[AT_SHIFT] -= 0.9 * k;
    d.spin[AT_NECK] += side * 2.5 * k;
    d.spin[AT_NECK + 1] += 3.5 * k;
    d.spin[AT_HEAD] -= side * 3.0 * k;
    d.spin[AT_HEAD + 1] += 4.0 * k;
    for j in 0..4 {
        d.spin[AT_TAIL + 2 * j] += side * (1.5 + j as f64) * k;
    }
}

/// Noticing the player, going after them, giving up on them, and going into the next phase of its
/// fight as it's hurt
fn notice(a: &mut Animal, d: &mut Dragon, boss: &mut Boss, h: &Here) {
    let vpm = h.vpm;
    let (sense, give_up, leash) = senses(h.look);
    let sparing = boss.spare_for(h.dt);
    let distance = h.feet.map_or(f64::MAX, |p| flat(p - d.center).norm() / vpm);
    let from_home = flat(d.center - boss.home).norm() / vpm;
    d.look_wait -= h.dt;
    if d.look_wait <= 0.0 {
        d.look_wait = 0.25;
        let eye = d.mouth;
        d.seen = (boss.raider && h.feet.is_some()) || h.feet.is_some_and(|p| distance < give_up && clear_between(h.world, eye, p + Vector3::new(0.0, 0.0, 1.3 * vpm)));
    }
    let alarmed = boss.flock.size > 1 && boss.flock.alarmed && distance < give_up && !sparing;
    if !boss.engaged && !sparing && h.feet.is_some() && (boss.raider || (distance < sense && d.seen) || alarmed) {
        boss.engaged = true;
    }
    // Turning on the player, having noticed them, or been struck by them or sent after them as it
    // rested, see Boss::provoke()
    if boss.engaged && d.state == State::Rest && h.feet.is_some() && (boss.raider || distance < give_up) {
        d.next = 0.8 + d.roll() * 1.2;
        let roar = Act::Roar { at: d.mouth, brood: None, first: true, angry: false };
        boss.deeds.push(Deed::Dragon(roar));
        // A drakeling on the ground takes off unless they're right by it; a great one or a drake
        // rears up and roars first
        d.state = if d.airborne || (is_drakeling(h.look) && distance < 5.0) {
            State::Hunt
        } else if is_drakeling(h.look) {
            State::Rise(0.0)
        } else {
            State::Wind(Roar, 0.0)
        };
        d.aloft = 0.0;
        d.budget = pick_budget(d, h, !d.airborne);
    }
    boss.set_sighted(d.seen && boss.engaged);
    d.unseen = if d.seen || !boss.engaged { 0.0 } else { d.unseen + h.dt };
    // Not losing sight of them in its own arena, behind a pillar or under its rim: it knows they're
    // there, see in_arena()
    let unseen = d.unseen > 12.0 && !h.feet.is_some_and(|f| in_arena(h, boss, f));
    if boss.engaged && !boss.raider && (h.feet.is_none() || distance > give_up || from_home > leash || unseen) {
        boss.calm();
    }
    if !boss.engaged && !matches!(d.state, State::Rest | State::Reel(_) | State::Alight(_)) {
        d.state = State::Rest;
        d.skill = None;
    }
    let busy = matches!(d.state, State::Do(..) | State::Reel(_) | State::Fall);
    // A great one going into the next phase of its fight: roaring, calling its brood, and in the
    // last of them quicker about everything, and still so if it's turned on the player again
    let marks = phases(h.look);
    if boss.engaged && !marks.is_empty() && boss.stage as usize >= marks.len() {
        boss.enraged = true;
    }
    if boss.engaged && (boss.stage as usize) < marks.len() && a.vitality() < marks[boss.stage as usize] && !busy {
        boss.stage += 1;
        boss.enraged = boss.stage as usize >= marks.len();
        d.stage = boss.stage;
        boss.deeds.push(Deed::Phase { at: d.center, stage: boss.stage });
        d.cool[Roar as usize] = 0.0;
        d.cool[Rain as usize] = d.cool[Rain as usize].min(3.0);
        // The oldest comes down out of the sky into its second phase, and fights on the ground
        if h.look == ULTRA && boss.stage == 1 && d.airborne {
            begin_landing(d, h);
        } else {
            begin(d, h, Roar, true);
        }
        d.aloft = 0.0;
        d.budget = pick_budget(d, h, !d.airborne);
    }
    // A drake hurt badly enough rears up and roars, enraged, and is quicker about everything after,
    // as every boss of its kind is
    if boss.engaged && is_drake(h.look) && marks.is_empty() && !boss.enraged && a.vitality() < 0.5 && !busy {
        boss.enraged = true;
        begin(d, h, Roar, true);
    }
}

/// How long it means to keep to the air, or the ground, from now
fn pick_budget(d: &mut Dragon, h: &Here, air: bool) -> f64 {
    let (aloft, down) = budgets(h.look, d.stage);
    let [lo, hi] = if air { aloft } else { down };
    lo + (hi - lo) * d.roll()
}

/// What it can do from where it is, if anything's ready, picked by how likely each is
fn choose(d: &mut Dragon, h: &Here, stage: u8, distance: f64) -> Option<Skill> {
    // Made to make the one move, for looking at it, wherever the player is
    if let Some(m) = forced() {
        return skills(h.look).iter().copied().find(|s| s.m == m && s.air == d.airborne && d.cool[m as usize] <= 0.0);
    }
    let ready: Vec<Skill> = skills(h.look).iter().copied().filter(|s| s.air == d.airborne && s.stage <= stage && d.cool[s.m as usize] <= 0.0 && (s.near..s.far).contains(&distance)).collect();
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
fn begin(d: &mut Dragon, h: &Here, m: Move, forced: bool) {
    let skill = skills(h.look).iter().copied().find(|s| s.m == m && (forced || s.air == d.airborne)).unwrap_or(skill(m, d.airborne, 0.0, 1e9, 10.0, 0.6, 0.5, 0, 1.0));
    d.skill = Some(skill);
    d.state = State::Wind(m, 0.0);
    d.ready = false;
    d.struck = false;
    d.again = 0;
    let feet = h.feet.unwrap_or(d.center);
    d.aim = feet;
    let toward = flat(feet - d.center).try_normalize(1e-6).unwrap_or_else(|| Vector3::new(d.yaw.cos(), d.yaw.sin(), 0.0));
    d.way = toward;
    d.sweep = if d.roll() < 0.5 { 1.0 } else { -1.0 };
    match m {
        Hover => d.again = 2,
        Stomp | Dive => {
            // The ground's to crack open straight at the player and fanned out either side
            let at = toward.y.atan2(toward.x);
            let n = if h.look == ULTRA { 6 } else { 4 };
            for k in 0..6 {
                d.cracks[k] = if k < n { at + (k as f64 - (n - 1) as f64 * 0.5) * 0.55 + (d.roll() - 0.5) * 0.2 } else { f64::NAN };
            }
        }
        _ => {}
    }
}

/// Starts coming down to land near the player, or at home without them
fn begin_landing(d: &mut Dragon, h: &Here) {
    let vpm = h.vpm;
    let s = h.sp.s;
    let around = h.feet.unwrap_or(d.center);
    let away = flat(d.center - around).try_normalize(1e-6).unwrap_or_else(Vector3::x);
    // A little way short of the player, far enough its body's clear of them, on open ground,
    // trying round about if it's not there
    let off = if is_drakeling(h.look) { 3.5 + 1.8 * s } else { 4.0 + 2.6 * s } * vpm;
    // On the ground they're on, not up on a rock or a pillar over it
    let level = h.feet.and_then(|f| ground_z(h.world, f.x, f.y, f.z + 1.0 * vpm, 30.0));
    let mut spot = None;
    for k in 0..8 {
        let turn = [0.0, 0.6, -0.6, 1.2, -1.2, 1.9, -1.9, PI][k];
        let (c, si) = (turn.cos(), turn.sin());
        let way = Vector3::new(away.x * c - away.y * si, away.x * si + away.y * c, 0.0);
        let at = around + way * off;
        if let Some(z) = ground_z(h.world, at.x, at.y, at.z + 6.0 * vpm, 30.0)
            && (h.water)(at.x, at.y) < z + 0.2 * vpm
            && level.is_none_or(|l| (z - l).abs() < 3.0 * vpm)
        {
            spot = Some(Vector3::new(at.x, at.y, z));
            break;
        }
    }
    if let Some(spot) = spot {
        d.aim = spot;
        d.state = State::Alight(0.0);
    } else {
        // Nowhere to come down: it stays up a while longer
        d.aloft = 0.0;
        d.budget = 5.0;
    }
}

/// Whether there's open sky over it to take off into, high enough to fly about in: not under a
/// roof, a cave's or a ruin's
fn open_sky(d: &Dragon, h: &Here) -> bool {
    let up = flight(h.look).altitude * 0.8 * h.vpm;
    clear_between(h.world, d.center, d.center + Vector3::new(0.0, 0.0, up))
}

/// Whether it's the Worldpyre Ancient fighting the player in its arena, see
/// worldgen\dragonlair.rs: its lair's a finale's, and the player's in it, `feet` being where
fn in_arena(h: &Here, boss: &Boss, feet: Vector3<f64>) -> bool {
    h.look == ULTRA && crate::realms::is_finale_key(&boss.key) && flat(feet - boss.home).norm() < (crate::worldgen::DRAGON_ARENA_FLOOR as f64 + 6.0) * h.vpm
}

/// Where the ground is under the player, or under it, in voxels
fn ground_of(h: &Here, at: Vector3<f64>) -> f64 {
    ground_z(h.world, at.x, at.y, at.z, 40.0).unwrap_or(at.z)
}

fn mark(boss: &mut Boss, at: Vector3<f64>, radius: f64, closing: f64) {
    boss.deeds.push(Deed::Mark { at, radius, closing: closing.clamp(0.0, 1.0) });
}

fn act_out(boss: &mut Boss, act: Act) {
    boss.deeds.push(Deed::Dragon(act));
}

/// Its head turned `how` far towards `at`, its neck sharing the turn, as far as they go
fn look_at(d: &Dragon, h: &Here, want: &mut Rig, at: Vector3<f64>, how: f64) {
    let neck = d.bones[NECK].at;
    let body = rz(d.yaw);
    let local = body.inverse() * (at - neck);
    let yaw = local.y.atan2(local.x).clamp(-1.3, 1.3) * how;
    let pitch = (local.z.atan2(flat(local).norm()) - 0.1).clamp(-0.9, 0.6) * how;
    let n = h.sp.necks as f64;
    for k in 0..h.sp.necks {
        want.neck[k][0] += yaw * 0.55 / n;
    }
    want.head[0] += yaw * 0.45;
    want.head[1] += pitch * 0.7;
    want.neck[0][1] += pitch * 0.3;
}

/// Its neck drawn back into an S, `back` of the way, to strike: its root raised, the rest bent back
/// down over it, the head drawn in and tipped down at what it'll strike
fn coil(want: &mut Rig, sp: &Spec, back: f64) {
    let n = sp.necks;
    want.neck[0][1] += 0.6 * back;
    for k in 1..n {
        want.neck[k][1] -= 1.1 * back / (n - 1) as f64;
    }
    if n == 1 {
        want.head[1] -= 0.4 * back;
    }
    want.head[1] -= 0.05 * back;
    want.shift[0] -= 0.12 * back;
    want.pitch += 0.06 * back;
}

/// Its neck thrust straight out ahead and a little down, `out` of the way, as it strikes, its body
/// lurching after it
fn thrust(want: &mut Rig, sp: &Spec, out: f64) {
    let n = sp.necks;
    want.neck[0][1] -= 0.6 * out;
    for k in 1..n {
        want.neck[k][1] += 0.5 * out / (n - 1) as f64;
    }
    want.head[1] += 0.18 * out;
    want.shift[0] += 0.2 * out;
    want.pitch -= 0.05 * out;
}

/// Reared up, `up` of the way, as to breathe or roar: its front lifted, its weight back, its neck
/// raised high and its head tipped back
fn rear(want: &mut Rig, sp: &Spec, up: f64) {
    want.pitch += 0.22 * up;
    want.shift[0] -= 0.08 * up;
    want.shift[2] += 0.04 * up;
    want.neck[0][1] += 0.35 * up;
    for k in 1..sp.necks {
        want.neck[k][1] += 0.08 * up;
    }
    want.head[1] += 0.4 * up;
    // Its tail lifted as its front comes up, so it's not driven into the ground
    want.tail[0][1] += 0.2 * up;
}

/// Its legs as they're held: all four planted, or `fore` lifted off the ground, held as `pose`
fn lift_fore(want: &mut Rig, how: f64, pose: LegPose) {
    for leg in 2..4 {
        let l = &mut want.legs[leg];
        l.plant *= 1.0 - how;
        l.swing += (pose.swing - l.swing) * how;
        l.knee += (pose.knee - l.knee) * how;
    }
}

/// The size of its blows, in meters: how far round a bite and a claw hurt, how far its tail
/// reaches, how far round a stamp hurts, and how far its wings throw back
fn reaches(sp: &Spec) -> (f64, f64, f64, f64, f64) {
    let s = sp.s;
    let tail = (0.68 + sp.tails as f64 * sp.tail) * s;
    (1.2 * s.powf(0.6), 1.7 * s.powf(0.6), tail * 0.92, 2.3 * s.powf(0.75), 6.0 * s.powf(0.6))
}

/// How far in front of its middle its bite lands, in meters
fn bite_reach(sp: &Spec) -> f64 {
    (0.6 + sp.necks as f64 * sp.neck * 0.75 + 0.45) * sp.s
}

/// What a dragon does, see State
fn think(a: &mut Animal, d: &mut Dragon, boss: &mut Boss, h: &Here, goal: &mut Goal, want: &mut Rig, wings: &mut WingSet) {
    let vpm = h.vpm;
    let s = h.sp.s;
    let sp = h.sp;
    let fl = flight(h.look);
    let pw = power(h.look);
    let (bite_r, claw_r, tail_r, stomp_r, gust_r) = reaches(&h.sp);
    let feet = h.feet.unwrap_or(boss.home);
    let ground_here = ground_of(h, d.center);
    d.ground = ground_here;
    let distance = flat(feet - d.center).norm() / vpm;
    let forward = Vector3::new(d.yaw.cos(), d.yaw.sin(), 0.0);
    let k = s.sqrt();
    d.aloft += h.dt;
    match d.state {
        State::Rest => {
            if d.airborne {
                // Back home, and down
                goal.to = boss.home + Vector3::new(0.0, 0.0, (6.0 * k) * vpm + h.stand);
                goal.speed = fl.cruise * 0.7;
                goal.cruise = true;
                if flat(d.center - boss.home).norm() < 4.0 * k * vpm {
                    d.aim = boss.home;
                    d.state = State::Alight(0.0);
                }
            } else {
                // Healing, wandering about home a little, looking round
                a.health = (a.health + a.max_health * 0.05 * h.dt).min(a.max_health);
                d.health = a.health;
                d.wander_wait -= h.dt;
                if d.wander_wait <= 0.0 {
                    d.wander_wait = 4.0 + 5.0 * d.roll();
                    let r = d.roll() * 5.0 * k * vpm;
                    let t = d.roll() * TAU;
                    d.wander = if d.roll() < 0.4 { d.center } else { boss.home + Vector3::new(t.cos() * r, t.sin() * r, 0.0) };
                }
                if flat(d.wander - d.center).norm() > 0.6 * vpm {
                    goal.to = d.wander;
                    goal.speed = speeds(h.look).0;
                }
                // Left somewhere it can't walk home from, in water or hemmed in: it flies home
                if d.blocked > 3.0 && flat(d.center - boss.home).norm() > 6.0 * k * vpm && open_sky(d, h) {
                    d.state = State::Rise(0.0);
                }
                // Looking about, now here, now there; and warily at the player, if they're near
                d.gaze_wait -= h.dt;
                if d.gaze_wait <= 0.0 {
                    d.gaze_wait = 1.8 + 3.5 * d.roll();
                    d.gaze = [(d.roll() - 0.5) * 1.8, (d.roll() - 0.4) * 0.5];
                }
                let near = h.feet.filter(|f| flat(f - d.center).norm() < senses(h.look).0 * 1.5 * vpm);
                match near {
                    Some(f) => look_at(d, h, want, f + Vector3::new(0.0, 0.0, 1.2 * vpm), 0.8),
                    None => {
                        for j in 0..sp.necks {
                            want.neck[j][0] += d.gaze[0] * 0.5 / sp.necks as f64;
                        }
                        want.head[0] += d.gaze[0] * 0.5;
                        want.head[1] += d.gaze[1];
                    }
                }
            }
        }
        State::Hunt => {
            let stage = boss.stage;
            if d.airborne {
                // Circling over the player, round and round, near enough to see them always; or
                // round its arena, over the player in it, see worldgen\dragonlair.rs
                let arena = crate::worldgen::DRAGON_ARENA_FLOOR as f64;
                let (middle, orbit) = if in_arena(h, boss, feet) { (Vector3::new(boss.home.x, boss.home.y, feet.z), arena * 0.75) } else { (feet, fl.orbit) };
                // Making for a point a little way on round the circle from where it is, so it flies
                // round it smoothly at full speed, banked into the turn
                let from = flat(d.center - middle);
                d.orbit = from.y.atan2(from.x) + d.round * 0.6;
                let round = Vector3::new(d.orbit.cos(), d.orbit.sin(), 0.0) * orbit * vpm;
                let at = middle + round;
                goal.to = Vector3::new(at.x, at.y, ground_of(h, feet).max(ground_here) + fl.altitude * vpm);
                goal.speed = fl.cruise;
                goal.cruise = true;
                if distance > fl.orbit * 2.0 {
                    goal.to = feet + Vector3::new(0.0, 0.0, fl.altitude * vpm);
                }
                // Its head turned to keep the player in sight as it goes round
                look_at(d, h, want, feet, 0.45);
                d.next -= h.dt;
                if d.next <= 0.0 && boss.engaged {
                    if let Some(skill) = choose(d, h, stage, distance) {
                        begin(d, h, skill.m, false);
                    }
                }
                if d.aloft > d.budget && boss.engaged && matches!(d.state, State::Hunt) {
                    begin_landing(d, h);
                }
            } else {
                // On foot: up to the player, facing them, low and stalking, then whatever's in reach
                let close = bite_reach(&h.sp) * 0.85;
                goal.face = Some(feet);
                if distance > close {
                    goal.to = feet;
                    goal.speed = if distance > close + 4.0 * k { speeds(h.look).1 } else { speeds(h.look).0 };
                }
                look_at(d, h, want, feet + Vector3::new(0.0, 0.0, 1.2 * vpm), 1.0);
                // Head held low and forward, its wings a little open, menacing
                want.neck[0][1] -= 0.12;
                want.head[1] += 0.05;
                want.shift[2] -= 0.03;
                *wings = WingSet::held(blend(FOLDED, MANTLED, 0.25));
                let facing = turn_to(d.yaw, flat(feet - d.center).y.atan2(flat(feet - d.center).x)).abs() < 0.6;
                d.next -= h.dt;
                if d.next <= 0.0 && (facing || forced().is_some()) && boss.engaged {
                    if let Some(skill) = choose(d, h, stage, distance) {
                        begin(d, h, skill.m, false);
                    }
                }
                // Up again once it's had enough of the ground, or the player's out of reach, if
                // there's open sky over it
                let unreachable = (feet.z - ground_here).abs() > 3.5 * vpm || d.blocked > 1.5;
                if matches!(d.state, State::Hunt) && (d.aloft > d.budget || unreachable || distance > 18.0 * k) {
                    if open_sky(d, h) {
                        d.state = State::Rise(0.0);
                    } else {
                        d.aloft = 0.0;
                        d.budget = pick_budget(d, h, false);
                    }
                }
            }
        }
        State::Rise(t) => {
            // Crouching low, its wings raised high and spread, looking up; then springing up into
            // the air with a great downstroke, its legs pushing off, and climbing hard
            let crouch = 0.65 * k.sqrt();
            if t < crouch {
                // Down into the crouch, and held there a moment, gathering itself
                let p = ease(t / (crouch * 0.6));
                want.shift[2] -= 0.24 * p;
                want.shift[0] -= 0.05 * p;
                want.pitch += 0.1 * p;
                want.neck[0][1] -= 0.15 * p;
                want.head[1] += 0.35 * p;
                want.tail[0][1] += 0.2 * p;
                *wings = WingSet { quick: 2.2, ..WingSet::held(WingPose { flap: 1.15, sweep: -0.15, twist: 0.15, fold: 0.05, bend: 0.2 }) };
                goal.face = h.feet;
            } else {
                if !d.airborne {
                    d.airborne = true;
                    d.blocked = 0.0;
                    d.velocity = Vector3::new(forward.x, forward.y, 0.0) * 2.0 * vpm + Vector3::new(0.0, 0.0, 6.5 * k.powf(0.4) * vpm);
                    // Its first downstroke as it leaves the ground, its body flung up out of the
                    // crouch
                    d.beat = 0.0;
                    d.spin[AT_SHIFT + 2] += 3.0;
                    act_out(boss, Act::TakeOff { at: d.center - Vector3::new(0.0, 0.0, h.stand), size: s });
                }
                *wings = WingSet { base: WingPose { flap: 0.25, sweep: 0.05, twist: 0.1, fold: 0.05, bend: 0.1 }, amp: 0.95, rate: fl.beat * 1.5, hover: 0.25, quick: 2.0 };
                want.pitch += 0.18;
                // Its legs straight down, pushing off, then drawn up
                if t < crouch + 0.35 * k.sqrt() {
                    for leg in &mut want.legs {
                        *leg = LegPose { swing: 0.15, knee: 0.2, ..Default::default() };
                    }
                }
                goal.to = d.center + forward * 7.0 * vpm + Vector3::new(0.0, 0.0, 6.0 * vpm);
                goal.speed = fl.cruise * 0.6;
            }
            let height = (d.center.z - ground_here) / vpm;
            d.state = if d.airborne && (height > fl.altitude * 0.6 || t > 2.4 * k) {
                d.aloft = 0.0;
                d.budget = pick_budget(d, h, true);
                d.next = d.next.max(0.6);
                if boss.engaged { State::Hunt } else { State::Rest }
            } else {
                State::Rise(t + h.dt)
            };
        }
        State::Alight(t) => {
            // Gliding in down to the spot picked, then flaring, its body tipped up and its wings
            // beating forward to stop it; hanging over the spot a moment, its legs reaching down for
            // the ground, its tail low; then down
            let spot = d.aim;
            let over = flat(spot - d.center).norm() / vpm;
            let height = (d.center.z - spot.z - h.stand) / vpm;
            let up = if over > 3.0 * k { height.min(over * 0.5).max(0.0) } else { 0.0 };
            goal.to = Vector3::new(spot.x, spot.y, spot.z + h.stand + up * vpm);
            goal.speed = if over > 3.0 * k { fl.cruise * 0.75 } else { (1.2 + 1.2 * height.min(3.0)) * k };
            goal.low = true;
            goal.cruise = over > 9.0 * k;
            goal.face = Some(h.feet.unwrap_or(spot + forward * vpm));
            if over < 7.0 * k {
                let flare = ease((7.0 * k - over) / (4.0 * k));
                *wings = WingSet { base: blend(SPREAD, WingPose { flap: 0.35, sweep: -0.3, twist: -0.4, fold: 0.15, bend: 0.25 }, flare), amp: 0.55 + 0.3 * flare, rate: fl.beat * (1.0 + 0.35 * flare), hover: 0.8 * flare, quick: 1.6 };
                want.pitch += 0.5 * flare;
                want.tail[0][1] -= 0.35 * flare;
                want.neck[0][1] += 0.2 * flare;
                // Its feet reaching for the ground, planting as they find it
                let reach = ease(1.0 - (height / (1.3 * s)).clamp(0.0, 1.0));
                for leg in &mut want.legs {
                    *leg = LegPose { swing: 0.45, knee: 0.35, plant: reach * flare, foot: [0.08, 0.0] };
                }
            }
            if (over < 1.5 * k && height <= 0.15 * s) || t > 7.0 {
                d.airborne = false;
                d.velocity = Vector3::zeros();
                d.center.z = spot.z + h.stand;
                act_out(boss, Act::Land { at: Vector3::new(d.center.x, d.center.y, spot.z), size: s });
                // Coming down on its legs, its knees giving under it
                d.spin[AT_SHIFT + 2] -= 1.6;
                d.aloft = 0.0;
                d.budget = pick_budget(d, h, false);
                d.last = None;
                d.spent_for = 0.45 * k;
                d.state = if boss.engaged { State::Spent(0.45 * k) } else { State::Rest };
            } else {
                d.state = State::Alight(t + h.dt);
            }
        }
        State::Wind(m, t) => {
            let skill = d.skill.unwrap_or(skill(m, d.airborne, 0.0, 1.0, 1.0, 0.6, 0.5, 0, 1.0));
            let wind = skill.wind * k.powf(0.3) * h.quick.max(0.8);
            let toward = flat(feet - d.center).try_normalize(1e-6).unwrap_or(forward);
            let mut t = t;
            let p = (t / wind).min(1.0);
            match m {
                Bite => {
                    goal.face = Some(feet);
                    // Its neck drawn back into an S, crouching a little, its jaws parting
                    look_at(d, h, want, feet + Vector3::new(0.0, 0.0, 0.8 * vpm), 1.0);
                    coil(want, &sp, ease(p));
                    want.shift[2] -= 0.04 * p;
                    want.pitch += 0.04 * p;
                    want.jaw = 0.15 + 0.2 * p;
                    let at = d.center + forward * bite_reach(&h.sp) * vpm;
                    mark(boss, Vector3::new(at.x, at.y, ground_of(h, at)), bite_r, 1.0 - t / wind);
                }
                Tail => {
                    // Coiling away the way it'll spin, crouched, its tail raised and curled round
                    // the other way, looking back over its shoulder; the ring round it where the
                    // tail will sweep
                    let away = toward.y.atan2(toward.x) - d.sweep * 0.6;
                    goal.face = Some(d.center + Vector3::new(away.cos(), away.sin(), 0.0) * vpm);
                    let e = ease(p);
                    want.shift[2] -= 0.06 * e;
                    want.roll -= d.sweep * 0.06 * e;
                    want.tail[0][1] += 0.4 * e;
                    for j in 0..4 {
                        want.tail[j][1] += 0.08 * e;
                        want.tail[j][0] += d.sweep * 0.32 * e;
                    }
                    for j in 0..sp.necks {
                        want.neck[j][0] -= d.sweep * 0.25 * e;
                    }
                    want.head[0] -= d.sweep * 0.7 * e;
                    let ground = Vector3::new(d.center.x, d.center.y, ground_here);
                    mark(boss, ground, tail_r + 0.4, 1.0 - t / wind);
                }
                Breath => {
                    goal.face = Some(feet);
                    // Rearing up, its weight back and its wings mantled over it, fire gathering in
                    // its throat as its jaws part
                    let e = ease(p);
                    rear(want, &sp, e);
                    want.jaw = 0.12 + 0.38 * p;
                    *wings = WingSet::held(blend(FOLDED, MANTLED, e));
                    if (d.clock * 12.0).fract() < h.dt * 12.0 {
                        act_out(boss, Act::Kindle { at: d.mouth, size: s });
                    }
                    // Where it'll sweep, shown on the ground
                    let at = toward.y.atan2(toward.x);
                    let reach = (7.0 * s.powf(0.6)).min(distance.max(3.0));
                    for j in 0..5 {
                        let a = at + d.sweep * (j as f64 / 4.0 - 0.5) * 1.1;
                        let p = d.center + Vector3::new(a.cos(), a.sin(), 0.0) * reach * vpm;
                        mark(boss, Vector3::new(p.x, p.y, ground_of(h, p)), 1.0 * s.powf(0.6), 1.0 - t / wind);
                    }
                }
                Gust => {
                    goal.face = Some(feet);
                    // Rearing up off its forefeet, its wings raised high and spread forward, its
                    // neck drawn back
                    let e = ease(p);
                    *wings = WingSet { base: blend(MANTLED, WingPose { flap: 1.3, sweep: -0.35, twist: 0.25, fold: 0.08, bend: 0.4 }, e), amp: 0.04, rate: 1.0, hover: 0.0, quick: 1.8 };
                    if d.airborne {
                        goal.to = d.center;
                        goal.speed = 2.0;
                        want.pitch += 0.35 * e;
                    } else {
                        want.pitch += (if sp.four { 0.32 } else { 0.18 }) * e;
                        want.shift[0] -= 0.06 * e;
                        lift_fore(want, e, LegPose { swing: 0.9, knee: 1.0, ..Default::default() });
                    }
                    want.neck[0][1] += 0.25 * e;
                    want.head[1] -= 0.25 * e;
                    let ground = Vector3::new(d.center.x, d.center.y, ground_here);
                    for j in 1..4 {
                        let p = ground + forward * gust_r * (j as f64 / 3.5) * vpm;
                        mark(boss, Vector3::new(p.x, p.y, ground_of(h, p)), 0.8 + 0.5 * j as f64 * s.powf(0.4), 1.0 - t / wind);
                    }
                }
                Stomp => {
                    goal.face = Some(feet);
                    // Rearing up high on its hind legs, its forefeet raised and clawing, its wings
                    // half spread to balance it, its tail down behind, shaking with the strain of
                    // it at the top
                    let e = ease(p);
                    want.pitch += 0.52 * e + 0.02 * (t * 30.0).sin() * p;
                    want.shift[0] -= 0.12 * e;
                    want.shift[2] += 0.05 * e;
                    lift_fore(want, e, LegPose { swing: 1.15 + 0.15 * (t * 7.0).sin(), knee: 0.6, ..Default::default() });
                    want.neck[0][1] += 0.2 * e;
                    want.head[1] -= 0.35 * e;
                    want.tail[0][1] += 0.4 * e;
                    want.jaw = 0.3 * e;
                    *wings = WingSet::beating(blend(FOLDED, WingPose { flap: 0.6, sweep: 0.45, twist: 0.0, fold: 0.35, bend: -0.6 }, e), 0.12 * e, 0.9);
                    let at = d.center + forward * 0.7 * s * vpm;
                    let ground = Vector3::new(at.x, at.y, ground_of(h, at));
                    mark(boss, ground, stomp_r, 1.0 - t / wind);
                    // And where the ground will crack open
                    let length = 8.0 * s.powf(0.5);
                    for &c in d.cracks.iter().filter(|c| c.is_finite()) {
                        for j in 1..4 {
                            let p = ground + Vector3::new(c.cos(), c.sin(), 0.0) * (stomp_r + length * j as f64 / 3.5) * vpm;
                            mark(boss, Vector3::new(p.x, p.y, ground_of(h, p)), 0.7 * s.powf(0.3), 1.0 - t / wind);
                        }
                    }
                }
                Roar => {
                    // Rearing up, its neck raised, drawing breath, its wings opening
                    let e = ease(p);
                    rear(want, &sp, 1.1 * e);
                    want.jaw = 0.3 * p;
                    *wings = if d.airborne { WingSet { hover: 0.7, ..WingSet::beating(SPREAD, 0.7, fl.beat * 1.2) } } else { WingSet::held(blend(MANTLED, RAISED, e)) };
                    if d.airborne {
                        goal.to = d.center;
                        goal.speed = 3.0;
                        want.pitch += 0.3 * e;
                    }
                }
                Spit => {
                    goal.face = Some(feet);
                    // Its head drawn back and up, its element gathering in its throat
                    let e = ease(p);
                    coil(want, &sp, 0.6 * e);
                    want.head[1] += 0.3 * e;
                    want.jaw = 0.5 * p;
                    if (d.clock * 12.0).fract() < h.dt * 12.0 {
                        act_out(boss, Act::Kindle { at: d.mouth, size: s });
                    }
                    if d.airborne {
                        goal.to = d.center + toward * vpm;
                        goal.speed = 3.0;
                        *wings = WingSet { hover: 0.6, ..WingSet::beating(SPREAD, 0.7, fl.beat * 1.2) };
                        want.pitch += 0.3 * e;
                    }
                    // Held where it was aimed for the last of it, so it can be stepped out of
                    if t < wind - 0.3 {
                        d.aim = feet;
                    }
                    mark(boss, d.aim, 1.6 * s.powf(0.4), 1.0 - t / wind);
                }
                Swoop => {
                    // Rising a little and slowing, wings raised high, looking down at them
                    if t == 0.0 {
                        let low = feet - toward * 1.0 * s * vpm;
                        let exit = feet + toward * 12.0 * k * vpm + Vector3::new(0.0, 0.0, fl.altitude * vpm);
                        d.path = [d.center, low, exit];
                    }
                    goal.to = d.center + Vector3::new(0.0, 0.0, 1.5 * vpm);
                    goal.speed = 4.0;
                    goal.face = Some(feet);
                    *wings = WingSet::beating(WingPose { flap: 0.95, sweep: 0.1, twist: 0.0, fold: 0.1, bend: 0.2 }, 0.15 * (1.0 - p), fl.beat);
                    look_at(d, h, want, feet, 0.7);
                    want.pitch -= 0.2 * p;
                    let low = d.path[1];
                    mark(boss, Vector3::new(low.x, low.y, ground_of(h, low)), claw_r, 1.0 - 0.6 * t / wind);
                }
                Strafe => {
                    // To where the run starts first, the far side of the player, then lining up
                    let height = fl.altitude * 0.55;
                    let start = feet - d.way * 14.0 * k * vpm + Vector3::new(0.0, 0.0, height * vpm);
                    if !d.ready {
                        goal.to = Vector3::new(start.x, start.y, ground_of(h, start).max(feet.z) + height * vpm);
                        goal.speed = fl.cruise;
                        goal.cruise = true;
                        t = 0.0;
                        if flat(d.center - start).norm() < 3.0 * k * vpm || d.aloft > d.budget + 6.0 {
                            d.ready = true;
                            d.way = toward;
                            d.aim = feet;
                        }
                    } else {
                        goal.to = d.center + d.way * 2.0 * vpm;
                        goal.speed = 3.0;
                        goal.face = Some(d.center + d.way * 10.0 * vpm);
                        want.jaw = 0.4 * t / wind;
                        coil(want, &sp, 0.4 * p);
                        if (d.clock * 12.0).fract() < h.dt * 12.0 {
                            act_out(boss, Act::Kindle { at: d.mouth, size: s });
                        }
                        // The line it'll burn across the ground
                        for j in 0..6 {
                            let p = d.aim + d.way * (-3.0 + 2.2 * j as f64) * k * vpm;
                            mark(boss, Vector3::new(p.x, p.y, ground_of(h, p)), 1.1 * s.powf(0.5), 1.0 - t / wind);
                        }
                    }
                }
                Hover => {
                    // In beside the player, just over their head, hanging in the air on its beating
                    // wings, its legs dangling and its tail hanging down
                    let side = Vector3::new(-toward.y, toward.x, 0.0) * d.sweep;
                    let at = feet - toward * 1.6 * vpm + side * 1.2 * vpm;
                    goal.to = Vector3::new(at.x, at.y, ground_of(h, feet) + h.stand + 1.4 * vpm);
                    goal.speed = fl.cruise * 0.6;
                    goal.face = Some(feet);
                    goal.low = true;
                    *wings = WingSet { hover: 1.0, ..WingSet::beating(WingPose { flap: 0.3, sweep: -0.2, twist: -0.25, fold: 0.1, bend: 0.2 }, 0.85, fl.beat * 1.5) };
                    want.pitch += 0.5;
                    want.tail[0][1] -= 0.35;
                    for leg in &mut want.legs {
                        *leg = LegPose { swing: 0.25, knee: 0.8, ..Default::default() };
                    }
                    if !d.ready {
                        t = 0.0;
                        if (d.center - goal.to).norm() < 1.2 * vpm || d.aloft > d.budget + 6.0 {
                            d.ready = true;
                        }
                    } else {
                        coil(want, &sp, ease(p));
                        want.jaw = 0.3 * t / wind;
                        mark(boss, Vector3::new(feet.x, feet.y, ground_of(h, feet)), bite_r, 1.0 - t / wind);
                    }
                }
                Dive => {
                    // Climbing over where the player stands, which it'll fall on, the ring showing
                    // it; tipping over at the top, its wings drawn in
                    goal.to = Vector3::new(d.aim.x, d.aim.y, ground_of(h, d.aim) + (fl.altitude + 6.0 * k) * vpm);
                    goal.speed = fl.cruise * 0.8;
                    goal.face = Some(d.aim);
                    let tuck = ease((p - 0.6) / 0.4);
                    *wings = WingSet::beating(blend(WingPose { flap: 0.6, sweep: 0.0, twist: 0.0, fold: 0.1, bend: 0.2 }, TUCKED, tuck), 0.75 * (1.0 - tuck), fl.beat * 1.3);
                    want.pitch -= 0.6 * tuck;
                    want.neck[0][1] -= 0.25 * tuck;
                    if t < wind * 0.5 {
                        d.aim = feet;
                    }
                    mark(boss, Vector3::new(d.aim.x, d.aim.y, ground_of(h, d.aim)), stomp_r * 1.15, 1.0 - 0.7 * t / wind);
                }
                Rain => {
                    // Climbing high, rearing back and roaring at the sky
                    let e = ease(p);
                    goal.to = Vector3::new(feet.x, feet.y, ground_of(h, feet) + (fl.altitude + 8.0 * k) * vpm) + Vector3::new(d.orbit.cos(), d.orbit.sin(), 0.0) * fl.orbit * 0.6 * vpm;
                    goal.speed = fl.cruise * 0.8;
                    want.neck[0][1] += 0.5 * e;
                    want.head[1] += 0.5 * e;
                    want.jaw = 0.55 * p;
                    if !d.airborne {
                        goal.to = d.center;
                        goal.face = Some(feet);
                        rear(want, &sp, e);
                        *wings = WingSet::held(blend(MANTLED, RAISED, e));
                    }
                }
            }
            if t == 0.0 && (d.ready || !matches!(m, Strafe | Hover)) {
                act_out(boss, Act::Warn { m, at: d.mouth });
            }
            let t = t + h.dt;
            d.state = if t >= wind && (d.ready || !matches!(m, Strafe | Hover)) {
                d.struck = false;
                if let Some(skill) = d.skill {
                    d.cool[skill.m as usize] = skill.cool * h.quick;
                }
                // Down from where it's got to as it got ready
                if m == Swoop {
                    d.path[0] = d.center;
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
                Bite => {
                    let length = 0.3 * k.powf(0.3);
                    let q = t / length;
                    // Lunging, its neck shooting out straight, jaws wide, then snapping shut on what
                    // it reached, its body lurching in after
                    goal.to = d.center + forward * 3.0 * vpm;
                    goal.speed = 2.0 * k;
                    look_at(d, h, want, feet + Vector3::new(0.0, 0.0, 0.6 * vpm), 1.0);
                    thrust(want, &sp, if q < 0.45 { ease_out(q / 0.3) } else { 1.0 - 0.4 * (q - 0.45) / 0.55 });
                    want.shift[2] -= 0.04;
                    want.jaw = if q < 0.45 { 0.95 } else { 0.0 };
                    if !d.struck && q >= 0.45 {
                        d.struck = true;
                        let at = d.center + forward * bite_reach(&h.sp) * vpm;
                        act_out(boss, Act::Bite { at: Vector3::new(at.x, at.y, ground_of(h, at)), radius: bite_r, damage: pw.bite });
                    }
                    done = t >= length;
                }
                Tail => {
                    let length = 0.55 * k.powf(0.4);
                    // Round on the spot, crouched low, its tail sweeping out after it
                    d.yaw += d.sweep * TAU * 0.85 / length * h.dt;
                    want.shift[2] -= 0.07;
                    want.tail[0][1] -= 0.05;
                    for j in 0..4 {
                        want.tail[j][0] -= d.sweep * 0.12;
                    }
                    want.head[0] += d.sweep * 0.3;
                    if !d.struck && t >= length * 0.5 {
                        d.struck = true;
                        act_out(boss, Act::Sweep { at: Vector3::new(d.center.x, d.center.y, ground_here), radius: tail_r, damage: pw.tail });
                    }
                    done = t >= length;
                }
                Breath => {
                    let length = breath_length(h.look) * if boss.enraged { 1.2 } else { 1.0 };
                    // Lunging forward into it, braced, its neck thrust out and its jaws wide,
                    // sweeping its breath across the ground ahead as it heaves it out, the body
                    // turning slowly after the player
                    goal.face = Some(feet);
                    let across = d.sweep * (t / length - 0.5) * 1.1;
                    thrust(want, &sp, 0.7);
                    want.pitch -= 0.05;
                    want.shift[2] -= 0.03;
                    want.neck[0][0] += across * 0.4;
                    want.head[0] += across * 0.6;
                    want.head[1] -= 0.4 + 0.05 * (t * 17.0).sin();
                    want.jaw = 0.85 + 0.05 * (t * 23.0).sin();
                    *wings = WingSet::held(MANTLED);
                    for leg in 2..4 {
                        want.legs[leg].foot[0] += 0.06;
                    }
                    let size = 1.2 * s.powf(0.6);
                    act_out(boss, Act::Breath { from: d.mouth, dir: d.breath, dt: h.dt, dps: pw.breath, size, speed: 10.0 * s.powf(0.3), spread: 0.0 });
                    done = t >= length;
                }
                Gust => {
                    let length = 0.45 * k.powf(0.3);
                    let q = t / length;
                    // Beating down hard, the whole of it thrown into it, coming down on its forefeet
                    // as the ground's blown clear before it
                    let down = ease(q / 0.35);
                    let top = WingPose { flap: 1.3, sweep: -0.35, twist: 0.25, fold: 0.08, bend: 0.4 };
                    let bottom = WingPose { flap: -0.6, sweep: 0.05, twist: -0.1, fold: 0.05, bend: -0.35 };
                    *wings = WingSet { base: blend(top, bottom, down), amp: 0.0, rate: 1.0, hover: 0.0, quick: 6.0 };
                    if !d.airborne {
                        want.pitch += (if sp.four { 0.32 } else { 0.18 }) * (1.0 - down) - 0.06 * down;
                        want.shift[0] += 0.06 * down;
                        want.shift[2] -= 0.05 * down;
                        lift_fore(want, 1.0 - down, LegPose { swing: 0.9, knee: 1.0, ..Default::default() });
                    }
                    want.head[1] -= 0.2;
                    if !d.struck && q >= 0.3 {
                        d.struck = true;
                        act_out(boss, Act::Gust { at: Vector3::new(d.center.x, d.center.y, ground_here), dir: forward, reach: gust_r, push: pw.push, damage: pw.gust });
                    }
                    done = t >= length;
                }
                Stomp => {
                    let length = 0.3 * k.powf(0.3);
                    let q = t / length;
                    // Coming down on its forefeet with all its weight, roaring, its wings snapping out
                    let down = ease_out(q / 0.4);
                    want.pitch += 0.52 * (1.0 - down) - 0.12 * down;
                    want.shift[0] += -0.12 * (1.0 - down) + 0.06 * down;
                    want.shift[2] -= 0.06 * down;
                    lift_fore(want, 1.0 - down, LegPose { swing: 1.0, knee: 0.5, ..Default::default() });
                    want.neck[0][1] -= 0.2 * down;
                    want.head[1] -= 0.2;
                    want.jaw = 0.7;
                    *wings = WingSet { quick: 3.0, ..WingSet::held(blend(WingPose { flap: 0.6, sweep: 0.45, twist: 0.0, fold: 0.35, bend: -0.6 }, MANTLED, down)) };
                    if !d.struck && q >= 0.4 {
                        d.struck = true;
                        d.spin[AT_SHIFT + 2] -= 1.2;
                        let at = d.center + forward * 0.7 * s * vpm;
                        act_out(boss, Act::Stomp { at: Vector3::new(at.x, at.y, ground_of(h, at)), radius: stomp_r, damage: pw.stomp, cracks: d.cracks, length: 8.0 * s.powf(0.5), crack: pw.crack });
                    }
                    done = t >= length;
                }
                Roar => {
                    let length = 1.3;
                    let q = t / length;
                    // Its neck thrust out and up, jaws wide, wings spread wide over it, its head
                    // shaking with the roar
                    rear(want, &sp, 0.7);
                    thrust(want, &sp, 0.35);
                    want.neck[0][1] += 0.25;
                    want.head[1] += 0.25;
                    want.head[0] += 0.09 * (t * 21.0).sin() * (1.0 - q);
                    want.neck[1][0] += 0.04 * (t * 17.0).sin() * (1.0 - q);
                    want.jaw = 1.0;
                    *wings = if d.airborne { WingSet { hover: 0.7, ..WingSet::beating(SPREAD, 0.7, fl.beat * 1.2) } } else { WingSet::held(RAISED) };
                    if d.airborne {
                        goal.to = d.center;
                        goal.speed = 2.0;
                        want.pitch += 0.3;
                    }
                    if !d.struck {
                        d.struck = true;
                        let brood = brood(h.look).filter(|_| d.stage >= if h.look == ULTRA { 2 } else { 1 }).map(|b| (b, if h.look == ULTRA { 3 } else { 2 }));
                        let angry = is_drake(h.look) && phases(h.look).is_empty() && boss.enraged;
                        act_out(boss, Act::Roar { at: d.mouth, brood, first: false, angry });
                    }
                    done = t >= length;
                }
                Spit => {
                    if !d.struck {
                        d.struck = true;
                        let lob = matches!(h.look, Look::CindermawTyrant | Look::VenomDrake);
                        let count = if is_drakeling(h.look) || (is_drake(h.look) && h.look != Look::ElderWyrm) { 1 } else { 3 };
                        let target = d.aim + Vector3::new(0.0, 0.0, 0.8 * vpm);
                        let across = Vector3::new(-toward.y, toward.x, 0.0);
                        for j in 0..count {
                            let off = (j as f64 - (count - 1) as f64 * 0.5) * 3.2 * vpm;
                            let speed = if lob { 14.0 } else { 18.0 * s.powf(0.15) };
                            act_out(boss, Act::Fireball { from: d.mouth, to: target + across * off, speed, damage: pw.ball, radius: 1.6 * s.powf(0.35), lob });
                        }
                    }
                    // Its head snapping forward and down as it spits
                    thrust(want, &sp, 0.6);
                    want.jaw = 0.75;
                    want.head[1] -= 0.25;
                    if d.airborne {
                        *wings = WingSet { hover: 0.6, ..WingSet::beating(SPREAD, 0.7, fl.beat * 1.2) };
                        want.pitch += 0.2;
                    }
                    done = t >= 0.25;
                }
                Swoop => {
                    // Down along the curve through where it comes lowest, and away up again
                    let [from, low, exit] = d.path;
                    let control = low * 2.0 - (from + exit) * 0.5;
                    let length = ((low - from).norm() + (exit - low).norm()) / (fl.dash * vpm);
                    let u = (t / length.max(0.3)).min(1.0);
                    let at = from * (1.0 - u) * (1.0 - u) + control * 2.0 * u * (1.0 - u) + exit * u * u;
                    let along = (control - from) * 2.0 * (1.0 - u) + (exit - control) * 2.0 * u;
                    d.velocity = along / length.max(0.3);
                    let floor = ground_of(h, at) + h.stand * 0.8;
                    d.center = Vector3::new(at.x, at.y, at.z.max(floor));
                    goal.to = d.center;
                    goal.speed = -1.0;
                    goal.low = true;
                    // Wings swept back for the dive, its head down at them, then flared and
                    // beating for the climb; claws thrown forward low down
                    *wings = if u < 0.45 { WingSet { quick: 2.5, ..WingSet::beating(WingPose { flap: 0.35, sweep: 0.75, twist: 0.1, fold: 0.6, bend: 0.0 }, 0.05, 1.0) } } else { WingSet { quick: 2.0, ..WingSet::beating(SPREAD, 0.95, fl.beat * 1.45) } };
                    if (0.28..0.62).contains(&u) {
                        for leg in &mut want.legs {
                            *leg = LegPose { swing: 0.95, knee: 0.3, ..Default::default() };
                        }
                        want.head[1] -= 0.3;
                        want.jaw = 0.6;
                    } else if u < 0.28 {
                        look_at(d, h, want, low, 1.0);
                    }
                    // Where its claws will come down, closing in as it does
                    if !d.struck {
                        mark(boss, Vector3::new(low.x, low.y, ground_of(h, low)), claw_r, 0.4 * (1.0 - u / 0.5));
                    }
                    if !d.struck && u >= 0.5 {
                        d.struck = true;
                        act_out(boss, Act::Claw { at: Vector3::new(low.x, low.y, ground_of(h, low)), radius: claw_r, damage: pw.claw });
                    }
                    done = u >= 1.0;
                }
                Strafe => {
                    let length = if is_drakeling(h.look) { 1.8 } else if h.look == ULTRA { 3.0 } else { 2.4 };
                    let height = fl.altitude * 0.55;
                    let ahead = d.center + d.way * 20.0 * vpm;
                    goal.to = Vector3::new(ahead.x, ahead.y, ground_of(h, d.center).max(feet.z) + height * vpm);
                    goal.speed = fl.cruise * 1.15;
                    goal.face = Some(ahead);
                    // Its head down, breathing on the ground below as it flies over it
                    want.neck[0][1] -= 0.3;
                    want.head[1] -= 0.6;
                    want.jaw = 0.85;
                    *wings = WingSet::beating(SPREAD, 0.35, fl.beat);
                    // Breathing down and ahead, so it falls along the line below
                    let down = (d.way * 0.55 + Vector3::new(0.0, 0.0, -1.0)).normalize();
                    let spread = if h.look == ULTRA { 3.0 } else if is_drakeling(h.look) { 0.0 } else { 1.5 };
                    act_out(boss, Act::Breath { from: d.mouth, dir: down, dt: h.dt, dps: pw.breath, size: 1.3 * s.powf(0.6), speed: 12.0 * s.powf(0.25), spread });
                    done = t >= length;
                }
                Hover => {
                    let length = 0.3;
                    let q = t / length;
                    goal.to = d.center;
                    goal.speed = 1.0;
                    goal.face = Some(feet);
                    goal.low = true;
                    *wings = WingSet { hover: 1.0, ..WingSet::beating(WingPose { flap: 0.3, sweep: -0.2, twist: -0.25, fold: 0.1, bend: 0.2 }, 0.85, fl.beat * 1.5) };
                    want.pitch += 0.5;
                    want.tail[0][1] -= 0.35;
                    for leg in &mut want.legs {
                        *leg = LegPose { swing: 0.25, knee: 0.8, ..Default::default() };
                    }
                    // Striking down at them
                    thrust(want, &sp, ease_out(q / 0.4));
                    want.neck[0][1] -= 0.4;
                    want.head[1] -= 0.5;
                    want.jaw = if q < 0.4 { 0.9 } else { 0.0 };
                    if !d.struck && q >= 0.4 {
                        d.struck = true;
                        act_out(boss, Act::Bite { at: Vector3::new(feet.x, feet.y, ground_of(h, feet)), radius: bite_r, damage: pw.bite });
                    }
                    done = t >= length;
                    if done && d.again > 1 {
                        d.again -= 1;
                        d.state = State::Wind(Hover, 0.0);
                        d.ready = true;
                        d.struck = false;
                        return;
                    }
                }
                Dive => {
                    // Falling on the ring, wings swept tight, nose down, its legs thrown out ahead
                    // of it as it comes in, and down hard
                    let to = Vector3::new(d.aim.x, d.aim.y, ground_of(h, d.aim) + h.stand);
                    let off = to - d.center;
                    d.velocity = off.try_normalize(1e-6).unwrap_or_else(|| -Vector3::z()) * fl.dash * vpm;
                    goal.to = to;
                    goal.speed = -1.0;
                    d.center += d.velocity * h.dt;
                    let near = off.norm() < 5.0 * k * vpm;
                    *wings = if near { WingSet { quick: 3.0, ..WingSet::beating(WingPose { flap: 0.6, sweep: -0.2, twist: -0.3, fold: 0.1, bend: 0.3 }, 0.4, fl.beat * 1.4) } } else { WingSet { quick: 2.0, ..WingSet::held(TUCKED) } };
                    want.pitch -= if near { 0.0 } else { 0.5 };
                    if near {
                        for leg in &mut want.legs {
                            *leg = LegPose { swing: 0.7, knee: 0.35, ..Default::default() };
                        }
                        want.jaw = 0.8;
                    }
                    mark(boss, Vector3::new(d.aim.x, d.aim.y, ground_of(h, d.aim)), stomp_r * 1.15, (off.norm() / (fl.altitude * vpm * 2.0)).min(0.3));
                    if off.norm() < fl.dash * vpm * h.dt * 1.5 || d.center.z <= to.z || t > 4.0 {
                        d.center = to;
                        d.airborne = false;
                        d.velocity = Vector3::zeros();
                        act_out(boss, Act::Stomp { at: Vector3::new(to.x, to.y, to.z - h.stand), radius: stomp_r * 1.15, damage: pw.stomp * 1.1, cracks: d.cracks, length: 8.0 * s.powf(0.5), crack: pw.crack });
                        act_out(boss, Act::Land { at: Vector3::new(to.x, to.y, to.z - h.stand), size: s });
                        d.spin[AT_SHIFT + 2] -= 3.0;
                        d.aloft = 0.0;
                        d.budget = pick_budget(d, h, false);
                        // Dazed a moment from coming down so hard: the moment to get at it
                        d.last = Some(Dive);
                        d.spent_for = 2.4 * k.powf(0.3);
                        d.state = State::Spent(d.spent_for);
                        d.skill = None;
                        return;
                    }
                    done = false;
                }
                Rain => {
                    let (length, every) = rain_of(h.look, boss.stage);
                    let spread = if h.look == ULTRA { 18.0 } else if is_drake(h.look) && boss.stage == 0 { 6.0 } else { 13.0 };
                    if d.airborne {
                        d.orbit += d.round * fl.cruise / fl.orbit * h.dt * 0.7;
                        goal.to = Vector3::new(feet.x, feet.y, ground_of(h, feet) + (fl.altitude + 8.0 * k) * vpm) + Vector3::new(d.orbit.cos(), d.orbit.sin(), 0.0) * fl.orbit * 0.6 * vpm;
                        goal.speed = fl.cruise * 0.7;
                        want.neck[0][1] += 0.4;
                        want.head[1] += 0.4;
                        want.jaw = 0.7;
                    } else {
                        goal.face = Some(feet);
                        rear(want, &sp, 1.0);
                        want.neck[0][1] += 0.25;
                        want.head[1] += 0.35 + 0.05 * (t * 19.0).sin();
                        want.jaw = 0.9;
                        *wings = WingSet::held(RAISED);
                    }
                    let count = |t: f64| (t / every) as usize;
                    if count(t) != count(t - h.dt) {
                        let n = count(t);
                        // The first right where the player is, the rest round them
                        let at = if n < 3 {
                            feet
                        } else {
                            let a = d.roll() * TAU;
                            let r = (2.0 + (spread - 2.0) * d.roll().sqrt()) * vpm;
                            feet + Vector3::new(a.cos() * r, a.sin() * r, 0.0)
                        };
                        let at = Vector3::new(at.x, at.y, ground_of(h, at + Vector3::new(0.0, 0.0, 6.0 * vpm)));
                        act_out(boss, Act::Rain { to: at, damage: pw.rain, radius: 2.0 * s.powf(0.2) });
                    }
                    done = t >= length;
                }
            }
            d.state = if done {
                let rest = d.skill.map_or(0.5, |s| s.rest) * h.quick * k.powf(0.3);
                d.skill = None;
                d.last = Some(m);
                d.spent_for = rest.max(0.05);
                // A drakeling comes down after a swoop now and then, to fight on foot
                if m == Swoop && is_drakeling(h.look) && d.roll() < 0.35 && d.aloft > d.budget * 0.4 {
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
            // Getting over what it did: how far it's through it, from 1 down to 0
            let left = (t / d.spent_for.max(1e-3)).clamp(0.0, 1.0);
            if d.airborne {
                goal.to = d.center + forward * 2.0 * vpm + Vector3::new(0.0, 0.0, 0.5 * vpm);
                goal.speed = fl.cruise * 0.5;
                goal.cruise = true;
            } else {
                goal.face = None;
                match d.last {
                    // Its head thrown back and shaken after breathing, smoke coughed up, its
                    // jaws closing slowly
                    Some(Breath) => {
                        want.head[1] += 0.35 * left;
                        want.neck[0][1] += 0.15 * left;
                        want.head[0] += 0.14 * (t * 15.0).sin() * left;
                        want.jaw = 0.35 * left;
                        *wings = WingSet::held(blend(FOLDED, MANTLED, left));
                        if left > 0.6 && (d.clock * 8.0).fract() < h.dt * 8.0 {
                            act_out(boss, Act::Kindle { at: d.mouth, size: s });
                        }
                    }
                    // Settling back after it snapped, its neck drawing in
                    Some(Bite) | Some(Spit) => coil(want, &sp, 0.25 * left),
                    // Off balance after spinning, leaning into it
                    Some(Tail) => {
                        want.roll -= d.sweep * 0.09 * left;
                        want.shift[2] -= 0.05 * left;
                    }
                    // Low and heaving after stamping, its head down
                    Some(Stomp) => {
                        want.pitch -= 0.08 * left;
                        want.shift[2] -= 0.05 * left;
                        want.neck[0][1] -= 0.2 * left;
                        want.jaw = 0.3 * left;
                    }
                    // Dazed from coming down so hard: sprawled, its head low and shaking, its
                    // wings splayed
                    Some(Dive) => {
                        want.shift[2] -= 0.12 * left;
                        want.neck[0][1] -= 0.45 * left;
                        want.head[0] += 0.2 * (t * 6.0).sin() * left;
                        want.roll += 0.05 * (t * 4.0).sin() * left;
                        want.jaw = 0.3 * left;
                        *wings = WingSet::held(blend(FOLDED, WingPose { flap: 0.0, sweep: 0.6, twist: 0.1, fold: 0.4, bend: -0.5 }, left));
                        for (j, leg) in want.legs.iter_mut().enumerate() {
                            leg.foot[0] += if j < 2 { -0.06 } else { 0.08 } * left;
                        }
                    }
                    // Wings folding back slowly after beating them
                    Some(Gust) => *wings = WingSet::held(blend(FOLDED, MANTLED, left)),
                    _ => {}
                }
                look_at(d, h, want, feet + Vector3::new(0.0, 0.0, 1.0 * vpm), 1.0);
            }
            let t = t - h.dt;
            d.state = if t > 0.0 {
                State::Spent(t)
            } else {
                // A pack takes turns, so they don't all come at once
                let gap = if is_drakeling(h.look) { 2.4 + 1.6 * d.roll() + boss.flock.slot as f64 * 0.9 } else if h.look == ULTRA { 0.9 + 0.7 * d.roll() } else { 1.2 + 0.8 * d.roll() };
                d.next = gap * h.quick;
                d.last = None;
                if boss.engaged { State::Hunt } else { State::Rest }
            };
        }
        State::Reel(t) => {
            if d.airborne {
                // Falling out of the sky, tumbling, wings flailing
                *wings = WingSet { base: WingPose { flap: 0.4, sweep: 0.5, twist: 0.3, fold: 0.9, bend: 0.4 }, amp: 0.5, rate: fl.beat * 1.7, hover: 0.3, quick: 2.0 };
                d.velocity.z -= 14.0 * vpm * h.dt;
                d.velocity.x *= (-1.5 * h.dt).exp();
                d.velocity.y *= (-1.5 * h.dt).exp();
                want.roll += 0.7 * (d.clock * 2.3).sin();
                want.pitch -= 0.4;
                goal.speed = -1.0;
                d.center += d.velocity * h.dt;
                if d.center.z <= ground_here + h.stand * 0.8 {
                    d.center.z = ground_here + h.stand * 0.8;
                    d.airborne = false;
                    d.velocity = Vector3::zeros();
                    d.spin[AT_SHIFT + 2] -= 3.0;
                    d.spin[1] += 2.0 * d.side;
                    act_out(boss, Act::Crash { at: Vector3::new(d.center.x, d.center.y, ground_here), size: s });
                }
                d.state = State::Reel(t);
            } else {
                // Down on the ground dazed, sprawled on its side a little, its head low and
                // shaking, trying to get up
                *wings = WingSet::held(WingPose { flap: 0.0, sweep: 0.35, twist: 0.2, fold: 0.4, bend: -0.2 });
                want.roll += 0.25 * d.side;
                want.shift[2] -= 0.15;
                want.neck[0][1] -= 0.6;
                want.head[1] += 0.2;
                want.head[0] += 0.25 * (t * 5.0).sin();
                want.jaw = 0.25;
                for (j, leg) in want.legs.iter_mut().enumerate() {
                    leg.foot[0] += if j < 2 { -0.08 } else { 0.1 };
                }
                let t = t - h.dt;
                d.state = if t > 0.0 {
                    State::Reel(t)
                } else if boss.engaged && open_sky(d, h) {
                    d.reeled = 12.0;
                    State::Rise(0.0)
                } else if boss.engaged {
                    d.reeled = 12.0;
                    State::Hunt
                } else {
                    State::Rest
                };
            }
        }
        State::Fall => {}
    }
    // Knocked out of the air by being hit hard low down: the moment for a sword
    let height = (d.center.z - ground_here) / vpm;
    let threshold = if is_drakeling(h.look) { 0.22 } else if h.look == ULTRA { 0.05 } else { 0.07 };
    if d.airborne && height < 9.0 * k && d.recent > a.max_health * threshold && d.reeled <= 0.0 && !matches!(d.state, State::Reel(_) | State::Do(Dive, _)) {
        d.recent = 0.0;
        d.reeled = if is_drakeling(h.look) { 6.0 } else { 16.0 };
        d.state = State::Reel(if is_drakeling(h.look) { 2.4 } else { 3.2 });
        d.skill = None;
    }
}

/// How long a dragon killed on the ground rears up with its last roar before it goes down, in
/// seconds, and how long it takes to come down after
const LAST_ROAR: f64 = 0.55;
const COLLAPSE: f64 = 0.8;

/// Killed: falling out of the sky if it's flying, tumbling, its wings crumpled, and crashing down;
/// or rearing up with a last roar on the ground. Then its legs give under it, it rolls over onto its
/// side, its neck and head come down after it, and it lies still.
fn fall(_a: &mut Animal, d: &mut Dragon, boss: &mut Boss, h: &Here, goal: &mut Goal, want: &mut Rig, wings: &mut WingSet) {
    let vpm = h.vpm;
    let sp = h.sp;
    if d.state != State::Fall {
        d.state = State::Fall;
        d.dead = None;
        act_out(boss, Act::Death { at: d.mouth });
    }
    goal.speed = -1.0;
    let ground = ground_of(h, d.center);
    d.ground = ground;
    let dt = h.dt.max(1.0 / 120.0);
    if d.airborne {
        // Its wings crumpled and flailing, tumbling over and over, its head lolling
        *wings = WingSet { base: WingPose { flap: 0.5, sweep: 0.6, twist: 0.3, fold: 1.1, bend: 0.4 }, amp: 0.3, rate: 1.3, hover: 0.0, quick: 2.0 };
        d.velocity.z -= 12.0 * vpm * dt;
        d.velocity.x *= (-1.0 * dt).exp();
        d.velocity.y *= (-1.0 * dt).exp();
        d.center += d.velocity * dt;
        want.roll += d.side * (0.7 + 0.5 * (d.clock * 1.9).sin());
        want.pitch -= 0.55;
        want.neck[0][1] -= 0.35;
        want.head[1] -= 0.3;
        want.jaw = 0.55;
        for leg in &mut want.legs {
            *leg = LegPose { swing: 0.3, knee: 0.7, ..Default::default() };
        }
        if d.center.z <= ground + h.stand * 0.5 {
            d.center.z = ground + h.stand * 0.5;
            d.airborne = false;
            d.velocity = Vector3::zeros();
            act_out(boss, Act::Crash { at: Vector3::new(d.center.x, d.center.y, ground), size: sp.s });
            // Slammed into the ground: everything jolting with it. No last roar now.
            d.spin[AT_SHIFT + 2] -= 3.5;
            d.spin[1] += 2.5 * d.side;
            d.dead = Some(LAST_ROAR);
        }
        return;
    }
    let t = d.dead.unwrap_or(0.0) + dt;
    d.dead = Some(t);
    if t < LAST_ROAR {
        // Rearing up, its jaws wide, its wings thrown open
        let e = ease(t / (LAST_ROAR * 0.5));
        rear(want, &sp, e);
        want.neck[0][1] += 0.2 * e;
        want.head[1] += 0.2 * e + 0.06 * (t * 24.0).sin();
        want.jaw = 0.95;
        *wings = WingSet::held(blend(FOLDED, RAISED, 0.8 * e));
        return;
    }
    // Going down: its legs folding under it, rolling onto its side, its neck dropping and its head
    // laid on the ground, its tail limp, its wings fallen open across the ground
    let c = ease((t - LAST_ROAR) / COLLAPSE);
    let low = ground + h.stand * 0.36 * sp.girth.sqrt();
    let level = d.center.z + (low - d.center.z) * c;
    d.center.z += (level - d.center.z) * (1.0 - (-6.0 * dt).exp());
    want.roll += d.side * 1.0 * c;
    want.pitch = -0.06 * c;
    want.shift[2] -= 0.04 * c;
    for (j, leg) in want.legs.iter_mut().enumerate() {
        let folded = if j < 2 { LegPose { swing: 1.15, knee: 2.5, ..Default::default() } } else { LegPose { swing: 1.25, knee: 2.3, ..Default::default() } };
        leg.plant *= 1.0 - c;
        leg.swing += (folded.swing - leg.swing) * c;
        leg.knee += (folded.knee - leg.knee) * c;
    }
    want.neck[0][1] -= 0.75 * c;
    for j in 1..3 {
        want.neck[j][1] += 0.12 * c;
        want.neck[j][0] += d.side * 0.15 * c;
    }
    want.head[1] += 0.35 * c;
    want.head[0] += d.side * 0.2 * c;
    want.jaw = 0.4 * c;
    want.tail[0][1] -= 0.2 * c;
    for j in 0..4 {
        want.tail[j][1] -= 0.08 * c;
        want.tail[j][0] -= d.side * 0.16 * c;
    }
    *wings = WingSet::held(blend(FOLDED, WingPose { flap: -0.15, sweep: 0.55, twist: 0.2, fold: 0.55, bend: -0.7 }, c));
}

/// Moving it towards where it means to go: flying, steering and banking as it turns, never into
/// the ground, gliding between beats as it goes about; or walking on it
fn travel(d: &mut Dragon, h: &Here, goal: &Goal, want: &mut Rig, wings: &mut WingSet) {
    let vpm = h.vpm;
    let fl = flight(h.look);
    let k = h.sp.s.sqrt();
    let dt = h.dt;
    let before = d.yaw;
    if d.airborne {
        d.pace = 0.0;
        // goal.speed below 0: where it is has been set already, as it follows a curve
        if goal.speed >= 0.0 {
            let off = goal.to - d.center;
            let distance = off.norm();
            let want_v = if distance > 1e-6 { off / distance * (goal.speed * vpm).min(distance * 1.8) } else { Vector3::zeros() };
            let change = want_v - d.velocity;
            let most = fl.accel * vpm * dt;
            d.velocity += if change.norm() > most { change.normalize() * most } else { change };
            d.center += d.velocity * dt;
        }
        // Never into the ground under it, nor what's just ahead
        let ahead = d.center + flat(d.velocity) * 0.6;
        let under = ground_z(h.world, d.center.x, d.center.y, d.center.z, 60.0).unwrap_or(f64::MIN);
        let before_it = ground_z(h.world, ahead.x, ahead.y, d.center.z, 60.0).unwrap_or(f64::MIN);
        let clearance = if goal.low { h.stand * 0.8 } else { h.stand * 1.3 + 1.0 * vpm };
        let floor = under.max(before_it) + clearance;
        if d.center.z < floor && !matches!(d.state, State::Alight(_) | State::Do(Dive, _) | State::Fall | State::Reel(_)) {
            d.center.z += (floor - d.center.z) * (1.0 - (-8.0 * dt).exp());
            d.velocity.z = d.velocity.z.max(0.0);
        }
        // Nor up out of the top of the world, or the roof of a cave
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
        // Banking into its turns, its tail swung out after it and its head leading into them; and
        // climbing nose up
        let rate = turn_to(before, d.yaw) / dt.max(1e-6);
        want.roll += (-rate * speed * 0.05).clamp(-0.7, 0.7);
        for j in 0..4 {
            want.tail[j][0] += (rate * 0.07).clamp(-0.22, 0.22);
        }
        want.neck[0][0] += (rate * 0.15).clamp(-0.35, 0.35);
        let climb = d.velocity.z.atan2(flat(d.velocity).norm().max(1e-6));
        want.pitch += (climb * 0.6).clamp(-0.5, 0.5);
        // Going about, it glides between a few beats of its wings, beating again whenever it has to
        // climb or speed up
        if goal.cruise && matches!(d.state, State::Hunt | State::Rest | State::Spent(_) | State::Alight(_) | State::Wind(Strafe, _)) {
            let rising = (goal.to.z - d.center.z) / vpm > 1.5 && d.velocity.z / vpm < 1.5;
            let slow = speed < goal.speed.max(0.0) * 0.75;
            if rising || slow {
                d.glide = 0.0;
                d.beats = d.beats.max(1.0);
            }
            if d.glide > 0.0 {
                d.glide -= dt;
                *wings = WingSet { base: WingPose { flap: 0.24, sweep: 0.06, twist: -0.04, fold: 0.04, bend: 0.1 }, amp: 0.04, ..*wings };
            } else {
                d.beats -= wings.rate * dt;
                if d.beats <= 0.0 && !(rising || slow) {
                    d.glide = 1.2 + 1.8 * d.roll();
                    d.beats = 2.0 + (d.roll() * 2.0).floor();
                }
            }
        } else {
            d.glide = 0.0;
        }
    } else if matches!(d.state, State::Fall) {
        // Lying still
        d.pace = 0.0;
    } else {
        // Walking: up to where it's going, turning towards it, up and down the ground
        let off = flat(goal.to - d.center);
        let distance = off.norm() / vpm;
        let heading = match goal.face {
            Some(at) if distance < 0.5 => flat(at - d.center).y.atan2(flat(at - d.center).x),
            _ if distance >= 0.5 => off.y.atan2(off.x),
            Some(at) => flat(at - d.center).y.atan2(flat(at - d.center).x),
            None => d.yaw,
        };
        let turning = turn_to(d.yaw, heading);
        let turn = 3.0 / k;
        d.yaw += turning.clamp(-turn * dt, turn * dt);
        let speed = if distance > 0.3 && turning.abs() < 1.2 { goal.speed.min(distance / dt.max(1e-6) / vpm) } else { 0.0 };
        let step = Vector3::new(d.yaw.cos(), d.yaw.sin(), 0.0) * speed * vpm * dt;
        let next = d.center + step;
        let stand = h.stand;
        let ground_now = ground_z(h.world, d.center.x, d.center.y, d.center.z - stand + 1.0 * vpm, 6.0 * k);
        let ground_next = ground_z(h.world, next.x, next.y, d.center.z - stand + 1.0 * vpm * k, 6.0 * k);
        let climb = 0.7 * k * vpm;
        let (walk, run) = speeds(h.look);
        let mut moved = 0.0;
        match (ground_now, ground_next) {
            (_, Some(z)) if speed > 0.0 && (z - (d.center.z - stand)).abs() < climb && (h.water)(next.x, next.y) < z + 0.4 * k * vpm => {
                d.center.x = next.x;
                d.center.y = next.y;
                d.blocked = 0.0;
                moved = speed;
            }
            _ if speed > 0.0 => d.blocked += dt,
            _ => {}
        }
        // Its strides lengthening as it runs, and stepping round as it turns on the spot
        let quick = ((moved - walk) / (run - walk).max(1e-6)).clamp(0.0, 1.0);
        d.stride_length = stride(h.look) * (1.0 + 0.55 * quick);
        let rate = turn_to(before, d.yaw) / dt.max(1e-6);
        let stepping = if moved < 0.3 && rate.abs() > 0.2 { 0.4 * walk } else { 0.0 };
        d.pace = moved.max(stepping);
        d.stride = (d.stride + d.pace * dt / d.stride_length).fract();
        let ground = ground_z(h.world, d.center.x, d.center.y, d.center.z - stand + 1.0 * vpm * k, 8.0 * k).unwrap_or(d.center.z - stand);
        d.ground = ground;
        let z = ground + stand;
        d.center.z += (z - d.center.z) * (1.0 - (-12.0 * dt).exp());
    }
}

/// How it carries itself on top of how its joints are sprung: rising and falling with the beat of
/// its wings, walking with its hips and shoulders rolling, breathing, standing about; how high its
/// body's moved, how far it's pitched, rolled and twisted
fn carriage(d: &mut Dragon, h: &Here, dt: f64, alive: bool) -> [f64; 4] {
    let k = h.sp.s.sqrt();
    let mut o = [0.0; 4];
    // Carried up by each downstroke and sinking between, flying, its body nodding with it
    if d.airborne {
        let a = d.wings.amp * (1.0 - 0.4 * d.wings.hover);
        o[0] += 0.3 * a * lift(d.beat);
        o[1] += 0.05 * a * stroke(d.beat);
    }
    // Walking: its body rising and falling twice a stride, lowest as each foot comes down, its
    // hips and shoulders rolling with its weight, and it turning a little after each step
    let moving = if !d.airborne && alive && d.pace > 0.05 { 1.0 } else { 0.0 };
    d.gait += (moving - d.gait) * (1.0 - (-5.0 * dt).exp());
    if d.gait > 1e-3 {
        let (walk, run) = speeds(h.look);
        let quick = ((d.pace - walk) / (run - walk).max(1e-6)).clamp(0.0, 1.0);
        let (g, phase) = (d.gait, d.stride * TAU);
        o[0] -= g * (0.022 + 0.035 * quick) * (2.0 * phase).cos();
        o[1] += g * (0.012 + 0.02 * quick) * (2.0 * phase + 0.8).sin();
        o[2] += g * (0.035 + 0.025 * quick) * phase.sin();
        o[3] += g * 0.045 * (phase + 0.5).sin();
    }
    // Breathing slowly, standing about
    if !d.airborne && alive {
        d.breathing = (d.breathing + dt / (3.4 * k.sqrt())).fract();
        let b = (d.breathing * TAU).sin();
        o[0] += 0.012 * b;
        o[1] += 0.01 * b;
    }
    o
}

/// Its joints swung after where it wants them, its wings beating, its feet set on the ground, and
/// every piece of it put where they all are now
fn animate(d: &mut Dragon, h: &Here, want: &Rig, wings: &WingSet, dead: bool) {
    let dt = h.dt.max(if dead { 1.0 / 120.0 } else { 0.0 });
    let sp = h.sp;
    let k = sp.s.sqrt();
    let vpm = h.vpm;
    let m = sp.s * vpm;
    let mut want = *want;
    // Its tail swaying, a wave running down it, gently in the air
    let sway = if d.airborne { 0.035 } else if dead { 0.0 } else { 0.07 };
    for j in 0..sp.tails {
        want.tail[j][0] += sway * (j as f64 + 1.0) / sp.tails as f64 * (d.clock * 1.15 / k - j as f64 * 0.75).sin();
    }
    // Crying out as it flinches
    if d.flinch > 0.0 {
        want.jaw += 0.45 * d.flinch / 0.45;
    }
    // Its wings ease from one way of being held to another, beating on as they go: unfurling, its
    // wrist opens after its arm's out
    let ease_by = 1.0 - (-4.5 / k.powf(0.3) * wings.quick * dt).exp();
    {
        let (w, t) = (&mut d.wings, wings);
        let e = |a: &mut f64, b: f64, by: f64| *a += (b - *a) * by;
        let unfurl = if t.base.bend > w.base.bend { 0.55 } else { 1.0 };
        e(&mut w.base.flap, t.base.flap, ease_by);
        e(&mut w.base.sweep, t.base.sweep, ease_by);
        e(&mut w.base.twist, t.base.twist, ease_by);
        e(&mut w.base.fold, t.base.fold, ease_by);
        e(&mut w.base.bend, t.base.bend, ease_by * unfurl);
        e(&mut w.amp, t.amp, ease_by);
        e(&mut w.rate, t.rate, ease_by);
        e(&mut w.hover, t.hover, ease_by);
        w.quick = t.quick;
    }
    d.beat = (d.beat + d.wings.rate * dt).fract();
    let carried = carriage(d, h, dt, !dead);
    if dt > 0.0 {
        // How its body's moving as it's drawn: up and down, turning and pitching, for its neck and
        // tail to swing after it
        let now = [d.center.z / vpm + (d.rig.shift[2] + carried[0]) * sp.s, d.yaw + d.rig.twist + carried[3], d.rig.pitch + carried[1]];
        let mut push = [0.0; 3];
        d.track = Some(match d.track {
            Some((last, was)) => {
                let moving = [(now[0] - last[0]) / dt, turn_to(last[1], now[1]) / dt, (now[2] - last[2]) / dt];
                let by = 1.0 - (-dt / 0.06).exp();
                let v: [f64; 3] = std::array::from_fn(|i| was[i] + (moving[i] - was[i]) * by);
                push = [((v[0] - was[0]) / dt).clamp(-40.0, 40.0), ((v[1] - was[1]) / dt).clamp(-14.0, 14.0), ((v[2] - was[2]) / dt).clamp(-14.0, 14.0)];
                (now, v)
            }
            None => (now, [0.0; 3]),
        });
        let [heave, yawing, pitching] = push;
        // Each sprung joint swinging after where it's wanted, a few steps a frame so the quick
        // ones don't fly off; those along its neck and tail swung after the turning and heaving of
        // the joint before
        let target = want.flat();
        let mut x = d.rig.flat();
        let steps = (dt * 120.0).ceil().max(1.0) as usize;
        let step = dt / steps as f64;
        let slower = k.powf(0.5);
        for _ in 0..steps {
            let mut a = [0.0; SPRUNG];
            for i in 0..SPRUNG {
                let (w, z) = springiness(i);
                let w = w / slower;
                a[i] = w * w * (target[i] - x[i]) - 2.0 * z * w * d.spin[i];
            }
            // Down the neck to the head: turning its body leaves each joint behind it, and so does
            // tipping it up, and heaving it up
            let (mut turning, mut tipping) = (yawing, pitching);
            for j in 0..=sp.necks {
                let i = if j < sp.necks { AT_NECK + 2 * j } else { AT_HEAD };
                let l = inertia(i);
                a[i] -= l * turning;
                a[i + 1] -= l * (tipping + heave / (0.7 * sp.s));
                turning += a[i];
                tipping += a[i + 1];
            }
            // And down the tail, which tipping its body up swings the other way
            let (mut turning, mut tipping) = (yawing, -pitching);
            for j in 0..sp.tails {
                let i = AT_TAIL + 2 * j;
                let l = inertia(i);
                a[i] -= l * turning;
                a[i + 1] -= l * (tipping + heave / (0.9 * sp.s));
                turning += a[i];
                tipping += a[i + 1];
            }
            for i in 0..SPRUNG {
                d.spin[i] += a[i] * step;
                x[i] += d.spin[i] * step;
            }
        }
        for v in &mut x[AT_NECK..AT_LEGS] {
            *v = v.clamp(-1.8, 1.8);
        }
        x[AT_JAW] = x[AT_JAW].clamp(0.0, 1.1);
        for leg in 0..4 {
            x[AT_LEGS + 5 * leg + 2] = x[AT_LEGS + 5 * leg + 2].clamp(0.0, 1.0);
        }
        d.rig.set_flat(&x);
    }
    // Its wings beating about how they're held, half opened a moment as it flinches
    let mut base = d.wings.base;
    if d.flinch > 0.0 && !d.airborne && !dead {
        base = blend(base, MANTLED, 0.6 * d.flinch / 0.45);
    }
    let beat = beating(base, d.wings.amp, d.wings.hover, d.beat);
    d.rig.wings = [beat, beat];
    // Lying dead on its side, the wing under it lies flat along the ground rather than through
    // it, and the one over it falls across its body
    if dead && !d.airborne {
        let r = d.rig.roll;
        let (under, over) = if r > 0.0 { (1, 0) } else { (0, 1) };
        d.rig.wings[under].flap += r.abs() * 0.95;
        d.rig.wings[over].flap -= r.abs() * 0.3;
    }
    // As it's drawn: as it's sprung, carried as it carries itself
    let mut drawn = d.rig;
    drawn.shift[2] += carried[0];
    drawn.pitch += carried[1];
    drawn.roll += carried[2];
    drawn.twist += carried[3];
    if !d.airborne && !dead {
        let b = (d.breathing * TAU).sin();
        drawn.neck[0][1] += 0.03 * (d.breathing * TAU - 0.6).sin();
        for w in &mut drawn.wings {
            w.flap += 0.025 * b;
        }
    }
    // Its feet: those planted set down on the ground under them, stepping as it walks, the knees
    // bending to reach them
    let (center, body) = body_frame(&sp, d.center, d.yaw, &drawn, vpm);
    let ahead = Vector3::new(d.yaw.cos(), d.yaw.sin(), 0.0);
    let order = if sp.four { [0.0, 0.5, 0.25, 0.75] } else { [0.0, 0.5, 0.0, 0.5] };
    let (walk, run) = speeds(h.look);
    let quick = ((d.pace - walk) / (run - walk).max(1e-6)).clamp(0.0, 1.0);
    let duty = 0.68 - 0.2 * quick;
    let length = d.stride_length / sp.s;
    let raise = 0.1 + 0.08 * quick;
    for leg in 0..4 {
        let hind = leg < 2;
        let l = drawn.legs[leg];
        if (!hind && !sp.four) || l.plant < 0.01 {
            continue;
        }
        let (swing, knee) = if hind { STAND_HIND } else { STAND_FORE };
        let (under, _) = ankle_at(hind, swing, knee);
        let psi = (d.stride - order[leg]).rem_euclid(1.0);
        let (dx, dz) = if psi < duty {
            (length * (duty * 0.5 - psi), 0.0)
        } else {
            let u = (psi - duty) / (1.0 - duty);
            (length * duty * (ease(u) - 0.5), raise * (PI * u).sin())
        };
        let hip_at = center + body * hip_of(&sp, leg) * m;
        let foot = hip_at + ahead * (under + dx * d.gait + l.foot[0]) * m;
        let ground = ground_z(h.world, foot.x, foot.y, hip_at.z - 2.0 * vpm, 3.0 * sp.s + 2.0).unwrap_or(d.ground);
        let foot = Vector3::new(foot.x, foot.y, ground + (ANKLE + dz * d.gait + l.foot[1]) * m);
        let local = body.inverse() * (foot - hip_at) / m;
        let (s, kn) = reach_to(hind, local.x, local.z);
        drawn.legs[leg].swing = l.swing + (s - l.swing) * l.plant;
        drawn.legs[leg].knee = l.knee + (kn - l.knee) * l.plant;
    }
    let (mouth, breath) = pose(&sp, d.center, d.yaw, &drawn, vpm, &mut d.bones);
    // Animation files on top, see src\anim.rs
    edited(d, vpm);
    d.mouth = mouth;
    d.breath = breath;
    // Hit in the body, till the player's seen looking at some other part of it, see Dragon::aim()
    d.focus = d.center;
}

/// What to draw for dragon `a`: each of its pieces, into `out`
pub fn instances(a: &Animal, d: &Dragon, vpm: f64, out: &mut Vec<AnimalInstance>) {
    let layout = layout();
    let k = kind(a.look);
    let base = animals::DRAGON_MODELS_AT + k * PIECES;
    let tint = a.status.tint();
    for (b, bone) in d.bones.iter().enumerate() {
        if !bone.shown {
            continue;
        }
        let m = model_of(b);
        let (anchor, size) = layout[k * PIECES + m];
        if size == [1, 1, 1] {
            continue;
        }
        let [x, y, z] = size.map(|s| s as f64 / MODEL_RES);
        let reach = (x * x * 0.25 + y * y * 0.25 + z * z).sqrt() * vpm;
        let position = bone.at + bone.turn * anchor * vpm;
        out.push(AnimalInstance::new((base + m) as u32, position, bone.turn, vpm / MODEL_RES, reach, MOVING).tinted(tint));
    }
}

/// A drakeling called down out of the sky by a great one's roar, at `at` in voxels, high up, flying
/// in towards `toward` and after the player, one of pack `pack`
pub fn called(look: Look, at: Vector3<f64>, toward: Vector3<f64>, pack: u32, vpm: f64, seed: u32) -> Animal {
    let mut animal = Animal::boss(look, at, [at.x as i64, at.y as i64, -2], seed);
    if let Some(d) = &mut animal.dragon {
        d.swoop_in(at + Vector3::new(0.0, 0.0, stand_height(&spec(look)) * vpm), toward, vpm);
    }
    if let Some(boss) = &mut animal.boss {
        boss.provoke();
        boss.pack = pack;
    }
    animal
}

/// A pair or three of drakelings perched on high bare rock round `near`, in voxels, as there are
/// now and then in the mountains of Free Roam: what's breathed up there goes by the cold or the
/// jungle round about, and is mostly fire
pub fn roosting(world: &World, water: WaterLevel, near: Vector3<f64>, ground: u8, roll: f64, seed: u32) -> Vec<Animal> {
    let look = match ground {
        SNOW | ICE => {
            if roll < 0.6 {
                Look::Sleetwing
            } else {
                Look::Ashwing
            }
        }
        JUNGLE_GRASS | MOSS => Look::Blightwing,
        _ if roll < 0.15 => Look::Squallwing,
        _ => Look::Ashwing,
    };
    let count = 2 + (roll * 7.0) as usize % 2;
    animals::spawn_pack(world, water, &vec![look; count], near, 3.0, seed)
}

// Where the great ones keep their lairs: the Cinder Crown's, see realms.rs, and in Free Roam the
// Worldpyre Ancient's far lair, see worldgen\dragonlair.rs

/// The kind of lair each great dragon keeps, see worldgen\dragonlair.rs
pub fn lair_kind(look: Look) -> Option<crate::worldgen::LairKind> {
    use crate::worldgen::LairKind;
    match look {
        Look::PyreMatriarch => Some(LairKind::Peak),
        Look::CindermawTyrant => Some(LairKind::Caldera),
        Look::GildscaleHoardwarden => Some(LairKind::Hoard),
        Look::WorldpyreAncient => Some(LairKind::Ancient),
        _ => None,
    }
}

/// What's kept of the dragons in a world's save, see game\dragonfire.rs: whether the far lair's
/// been found, to show on the map
pub const WORLD_SECTION: &str = "dragons";

/// Whether ground of `material` at `z` voxels up a world `height` voxels high is the high bare rock
/// drakelings roost on
pub fn roost(material: u8, z: f64, height: f64) -> bool {
    matches!(material, STONE | ROCK | GRAVEL | SNOW) && z > height * 0.55
}

#[cfg(test)]
mod sheets;

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

    /// Every piece of every dragon that's drawn has a model, solid enough to see, small enough for
    /// the shader to walk through, and they're all together small next to the rest
    #[test]
    fn every_piece_has_a_model() {
        let made = build_models();
        assert_eq!(made.len(), MODELS - 0);
        let mut bytes = 0;
        for (k, &look) in KINDS.iter().enumerate() {
            let sp = spec(look);
            let mut bones = [Bone::default(); BONES];
            pose(&sp, Vector3::zeros(), 0.0, &Rig::standing(&sp), 16.0, &mut bones);
            for (b, bone) in bones.iter().enumerate() {
                let (model, _) = &made[k * PIECES + model_of(b)];
                if bone.shown {
                    let solid = model.voxels.iter().filter(|&&v| v != AIR).count();
                    assert!(solid > 30, "{look:?} piece {b} has only {solid} voxels");
                    assert!(model.size.iter().all(|&s| s < 1023), "{look:?} piece {b} is {:?}", model.size);
                }
            }
            let of: Vec<usize> = made[k * PIECES..(k + 1) * PIECES].iter().map(|(m, _)| m.voxels.len()).collect();
            println!("{look:?}: {:.2} MB, the biggest piece {:.2} MB", of.iter().sum::<usize>() as f64 / 1e6, *of.iter().max().unwrap() as f64 / 1e6);
            bytes += of.iter().sum::<usize>();
        }
        println!("dragons' pieces: {:.1} MB", bytes as f64 / 1e6);
        assert!(bytes < 24_000_000, "the dragons take {bytes} bytes");
        for fx in 0..FX {
            assert!(made[KINDS.len() * PIECES + fx].0.voxels.iter().any(|&v| v != AIR), "effect {fx} is empty");
        }
    }

    /// However its joints are turned, a dragon's wings stay on at the wrist and its neck and tail
    /// run on from each piece to the next
    #[test]
    fn its_joints_hold_together() {
        let vpm = 16.0;
        let mut seed = 7u32;
        let mut roll = || {
            seed = hash(seed.wrapping_add(0x9e37_79b9));
            (seed >> 8) as f64 / 16777216.0 - 0.5
        };
        for look in KINDS {
            let sp = spec(look);
            for _ in 0..40 {
                let mut rig = Rig::flying();
                rig.pitch = roll();
                rig.roll = roll();
                for w in &mut rig.wings {
                    *w = WingPose { flap: roll() * 2.0, sweep: roll() * 2.0, twist: roll(), fold: roll() * 4.0, bend: roll() };
                }
                for n in &mut rig.neck {
                    *n = [roll(), roll()];
                }
                let mut bones = [Bone::default(); BONES];
                let center = Vector3::new(100.0, 200.0, 300.0);
                pose(&sp, center, roll() * 6.0, &rig, vpm, &mut bones);
                let m = sp.s * vpm;
                for side in 0..2 {
                    let (inner, outer) = (bones[WING + side * 2], bones[WING + side * 2 + 1]);
                    let w = wrist(&sp);
                    let w = if side == 0 { w } else { flip(w) };
                    let at = inner.at + inner.turn * w * m;
                    assert!((at - outer.at).norm() < 1e-6 * m, "{look:?}'s wing {side} comes off at the wrist");
                }
                for k in 1..sp.necks {
                    let (a, b) = (bones[NECK + k - 1], bones[NECK + k]);
                    assert!((a.at + a.turn * Vector3::new(sp.neck, 0.0, 0.0) * m - b.at).norm() < 1e-6 * m);
                }
                let last = bones[NECK + sp.necks - 1];
                assert!((last.at + last.turn * Vector3::new(sp.neck, 0.0, 0.0) * m - bones[HEAD].at).norm() < 1e-6 * m);
            }
        }
    }

    /// Folded, a dragon's wings lie along its sides, their skin hanging down its flanks, nothing of
    /// them standing up over its back
    #[test]
    fn folded_wings_lie_along_its_sides() {
        for look in KINDS {
            let sp = spec(look);
            let mut bones = [Bone::default(); BONES];
            pose(&sp, Vector3::zeros(), 0.0, &Rig::standing(&sp), 1.0, &mut bones);
            let m = sp.s;
            for side in 0..2 {
                let (inner, outer) = (bones[WING + side * 2], bones[WING + side * 2 + 1]);
                let y = if side == 0 { 1.0 } else { -1.0 };
                let top = 0.7 * sp.girth * m;
                for (b, p) in [(outer, Vector3::new(-0.32, y * WING_OUT * sp.span, 0.03)), (outer, Vector3::new(-0.78, y * 0.32 * WING_OUT * sp.span, 0.0)), (inner, Vector3::new(-0.82, y * 0.04, 0.0))] {
                    let at = b.at + b.turn * p * m;
                    assert!(at.z < top, "{look:?}'s folded wing stands {:.2} m up", at.z);
                }
            }
        }
    }

    /// What happens over `seconds` with a dragon of `look` some way off a player standing still on
    /// open ground: what it did, and where it was, a frame at a time
    fn fight(look: Look, seconds: f64, hurt_to: Option<f64>) -> (Vec<(f64, Deed)>, Vec<(f64, Vector3<f64>, bool)>, Animal) {
        let vpm = 4u32;
        let world = open_ground(240, vpm);
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let v = vpm as f64;
        let feet = Vector3::new(120.0, 120.0, 4.0) * v;
        let mut a = Animal::boss(look, feet + Vector3::new(26.0, 6.0, 0.0) * v, [1, 2, -1], 11);
        let dt = 1.0 / 30.0;
        let (mut deeds, mut track) = (Vec::new(), Vec::new());
        let mut t = 0.0;
        while t < seconds {
            if let Some(to) = hurt_to
                && t > 5.0
                && a.health > a.max_health * to
            {
                // Worn down steadily from five seconds in
                a.damage(a.max_health * 0.01, feet);
            }
            a.update(&world, &water, Some(feet), dt);
            let d = a.dragon.as_ref().unwrap();
            track.push((t, d.center, d.airborne));
            for deed in a.boss.as_mut().unwrap().deeds.drain(..) {
                deeds.push((t, deed));
            }
            t += dt;
        }
        (deeds, track, a)
    }

    /// A drakeling turns on the player, takes off, circles and comes down at them, swooping or
    /// biting or breathing, then lands to fight on foot and takes off again; never into the ground
    /// and never anywhere it can't be
    #[test]
    fn a_drakeling_flies_fights_lands_and_rises() {
        let (deeds, track, _) = fight(Look::Ashwing, 90.0, None);
        let count = |f: &dyn Fn(&Act) -> bool| deeds.iter().filter(|(_, d)| matches!(d, Deed::Dragon(a) if f(a))).count();
        let blows = count(&|a| matches!(a, Act::Bite { .. } | Act::Claw { .. } | Act::Sweep { .. } | Act::Gust { .. } | Act::Breath { .. } | Act::Fireball { .. }));
        let takeoffs = count(&|a| matches!(a, Act::TakeOff { .. }));
        let landings = count(&|a| matches!(a, Act::Land { .. }));
        assert!(blows > 4, "it only struck {blows} times in a minute and a half");
        assert!(takeoffs >= 2 && landings >= 1, "took off {takeoffs} times, landed {landings}");
        let ground = 4.0 * 4.0;
        for &(t, at, flying) in &track {
            assert!(at.iter().all(|v| v.is_finite()), "lost at {t:.1} s");
            assert!(at.z > ground + 0.3 * 4.0, "in the ground at {t:.1} s: {:.2} m up", (at.z - ground) / 4.0);
            if flying {
                assert!(at.z < ground + 40.0 * 4.0, "flown off out of sight at {t:.1} s");
            }
        }
        // Up in the air some of the time and on the ground some of it
        let up = track.iter().filter(|t| t.2).count() as f64 / track.len() as f64;
        assert!((0.25..0.95).contains(&up), "in the air {:.0}% of the time", up * 100.0);
    }

    /// Everything a dragon does that hurts is seen coming: it's got ready for it, heard, a moment
    /// before, and where it'll land's been shown on the ground
    #[test]
    fn every_blow_is_seen_coming() {
        for look in [Look::Ashwing, Look::PyreMatriarch, Look::CindermawTyrant, Look::GildscaleHoardwarden, ULTRA, Look::Drake, Look::VenomDrake, Look::FrostWyrm, Look::StormDrake, Look::ElderWyrm] {
            let (deeds, ..) = fight(look, 70.0, Some(0.2));
            let mut warned: Option<f64> = None;
            let mut marked: Option<f64> = None;
            let mut blows = 0;
            for (t, deed) in &deeds {
                match deed {
                    Deed::Dragon(Act::Warn { .. }) => warned = Some(*t),
                    Deed::Mark { .. } => marked = Some(*t),
                    Deed::Dragon(act @ (Act::Bite { .. } | Act::Claw { .. } | Act::Sweep { .. } | Act::Stomp { .. } | Act::Gust { .. } | Act::Fireball { .. } | Act::Rain { .. })) => {
                        blows += 1;
                        let since = warned.map_or(f64::MAX, |w| t - w);
                        assert!(since < 6.0, "{look:?} struck at {t:.1} s without warning, {since:.1} s since the last");
                        // What it spits and rains down shows where it'll land all the way down, see
                        // game\dragonfire.rs; the rest's shown as it gets ready
                        if !matches!(act, Act::Fireball { .. } | Act::Rain { .. }) {
                            assert!(marked.is_some_and(|m| t - m < 0.5), "{look:?} struck at {t:.1} s without showing where: {act:?}");
                        }
                    }
                    _ => {}
                }
            }
            assert!(blows > 3, "{look:?} only struck {blows} times");
        }
    }

    /// The great ones go through each phase of their fight as they're worn down, roaring into
    /// each, and the oldest comes down to the ground into its second
    #[test]
    fn great_dragons_fight_in_phases() {
        for look in [Look::PyreMatriarch, Look::CindermawTyrant, Look::GildscaleHoardwarden, ULTRA, Look::ElderWyrm] {
            let (deeds, track, a) = fight(look, 120.0, Some(0.1));
            let stages: Vec<u8> = deeds.iter().filter_map(|(_, d)| if let Deed::Phase { stage, .. } = d { Some(*stage) } else { None }).collect();
            let all = phases(look).len() as u8;
            assert_eq!(stages, (1..=all).collect::<Vec<_>>(), "{look:?} went through phases {stages:?}");
            let d = a.dragon.as_ref().unwrap();
            assert!(a.boss.as_ref().unwrap().enraged, "{look:?} isn't enraged at the last: engaged {}, {:?}, {:.0} m off", a.boss.as_ref().unwrap().engaged, d.state, (d.center - Vector3::new(120.0, 120.0, 4.0) * 4.0).norm() / 4.0);
            if look == ULTRA {
                // Mostly in the air at first, and on the ground once it's come down
                let at = deeds.iter().find(|(_, d)| matches!(d, Deed::Phase { stage: 1, .. })).unwrap().0;
                let down = track.iter().filter(|(t, ..)| *t > at + 6.0 && *t < at + 18.0).filter(|p| !p.2).count();
                assert!(down > 200, "{look:?} kept to the air after its first phase");
            }
        }
    }

    /// The Cinder Crown is the dragons' realm: its three raid bosses the great dragons, by the lairs
    /// they keep, its finale the Worldpyre Ancient, as hard as it is in its far lair, and its
    /// monsters drakelings, flying in its night raids into the other realms
    #[test]
    fn the_cinder_crown_is_the_dragons_realm() {
        use crate::realms::{self, REALMS};
        let realm = &REALMS[realms::by_id("dragon").expect("the Cinder Crown")];
        assert_eq!(realm.raid_bosses.map(|(look, _)| look), RAID_DRAGONS);
        for (look, _) in realm.raid_bosses {
            assert!(lair_kind(look).is_some() && look.is_raid(), "{look:?}");
        }
        assert_eq!(realm.finale.map(|(look, ..)| look), Some(ULTRA));
        assert!(realm.monsters.iter().all(|p| p.looks.iter().all(|&l| is_drakeling(l))));
        let ancient = Animal::boss(ULTRA, Vector3::zeros(), [0, 0, -1], 1);
        assert_eq!(ancient.max_health * realms::FINALE_PROMOTION, 46000.0);
    }

    /// Struck as it rests, as by the player rising again beside it where they fell, it turns on them
    /// rather than lying there healing whatever they do to it
    #[test]
    fn a_resting_dragon_struck_turns_on_the_player() {
        let vpm = 4u32;
        let world = open_ground(240, vpm);
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let v = vpm as f64;
        let feet = Vector3::new(120.0, 120.0, 4.0) * v;
        let mut a = Animal::boss(Look::PyreMatriarch, feet + Vector3::new(6.0, 0.0, 0.0) * v, [1, 2, -1], 11);
        let dt = 1.0 / 30.0;
        for _ in 0..30 {
            a.update(&world, &water, None, dt);
        }
        // Sparing them, as after beating them, and resting beside them
        a.boss.as_mut().unwrap().spare();
        for _ in 0..30 {
            a.update(&world, &water, Some(feet), dt);
        }
        assert_eq!(a.dragon.as_ref().unwrap().state, State::Rest);
        a.boss.as_mut().unwrap().provoke();
        let mut struck = false;
        for _ in 0..(15.0 / dt) as usize {
            a.update(&world, &water, Some(feet), dt);
            let blows = a.boss.as_mut().unwrap().deeds.drain(..).filter(|d| matches!(d, Deed::Dragon(Act::Bite { .. } | Act::Claw { .. } | Act::Sweep { .. } | Act::Gust { .. } | Act::Breath { .. } | Act::Stomp { .. } | Act::Fireball { .. } | Act::Rain { .. }))).count();
            struck |= blows > 0;
        }
        assert!(struck, "it lay there struck, {:?}", a.dragon.as_ref().unwrap().state);
    }

    /// Killed in the air, a drakeling falls out of it to the ground and lies there a moment before
    /// it's taken away, rather than vanishing where it was
    #[test]
    fn a_dragon_killed_in_the_air_falls() {
        let vpm = 4u32;
        let world = open_ground(240, vpm);
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let v = vpm as f64;
        let feet = Vector3::new(120.0, 120.0, 4.0) * v;
        let mut a = called(Look::Ashwing, feet + Vector3::new(10.0, 0.0, 14.0) * v, feet, 1, v, 3);
        let dt = 1.0 / 30.0;
        for _ in 0..30 {
            a.update(&world, &water, Some(feet), dt);
        }
        let high = a.dragon.as_ref().unwrap().center.z;
        assert!(a.dragon.as_ref().unwrap().airborne && high > feet.z + 5.0 * v);
        a.damage(1e6, feet);
        let mut fell = 0.0;
        while dying(&a) && fell < 10.0 {
            a.update(&world, &water, Some(feet), dt);
            fell += dt;
        }
        let d = a.dragon.as_ref().unwrap();
        assert!(fell > 1.0 && fell < 10.0, "took {fell:.1} s to be done with");
        assert!(!d.airborne && d.center.z < feet.z + 2.0 * v, "still {:.1} m up", (d.center.z - feet.z) / v);
    }

    /// The drakes are dragons now, drawn on the jointed rig with no frames of their own, each its
    /// own look, keeping the names, elements, health and places in the realms they always had: the
    /// Elder Cinder Wyrm a raid boss in three phases, the rest bosses, none of them a drakeling
    #[test]
    fn the_drakes_are_dragons_and_keep_their_roles() {
        use crate::animals::Build;
        let elements = [Element::Fire, Element::Poison, Element::Frost, Element::Storm, Element::Fire];
        let healths = [2600.0, 2600.0, 2600.0, 2600.0, 13500.0];
        for (k, look) in DRAKES.into_iter().enumerate() {
            assert_eq!(look.build(), Some(Build::Dragon), "{look:?}");
            assert!(look.is_dragon() && look.is_boss() && look.flies() && !is_drakeling(look) && !is_great(look));
            assert_eq!(look.element(), Some(elements[k]), "{look:?}");
            assert_eq!(look.health(), healths[k], "{look:?}");
            assert_eq!(name(look), look.name());
            assert_eq!(look.is_raid(), look == Look::ElderWyrm, "{look:?}");
            assert_eq!(phases(look).len(), if look == Look::ElderWyrm { 2 } else { 0 });
            let a = Animal::boss(look, Vector3::zeros(), [0, 0, -1], 1);
            assert!(a.dragon.is_some(), "{look:?} has no rig");
        }
        // Each looks its own: no two the same in hide, crown and wings together
        for (i, a) in DRAKES.iter().enumerate() {
            for b in &DRAKES[i + 1..] {
                let (x, y) = (spec(*a), spec(*b));
                assert!(x.hide.scale != y.hide.scale || x.crown != y.crown || x.wing != y.wing, "{a:?} and {b:?} look alike");
            }
        }
    }

    /// A leg's ankle put anywhere it reaches is put there by the angles worked out for it, its
    /// knee forward of it
    #[test]
    fn legs_reach_where_they_are_set_down() {
        for hind in [true, false] {
            for (x, z) in [(0.0, -0.8), (0.3, -0.7), (-0.25, -0.6), (0.1, -0.45), (0.4, -0.3)] {
                let (swing, knee) = reach_to(hind, x, z);
                let (ax, az) = ankle_at(hind, swing, knee);
                assert!((ax - x).abs() < 1e-6 && (az - z).abs() < 1e-6, "{hind} {x} {z}: got {ax:.3} {az:.3}");
                assert!(knee > 0.0, "the knee bends backwards");
            }
        }
        // Standing, as the legs stand
        let (s, k) = reach_to(true, ankle_at(true, STAND_HIND.0, STAND_HIND.1).0, ankle_at(true, STAND_HIND.0, STAND_HIND.1).1);
        assert!((s - STAND_HIND.0).abs() < 1e-6 && (k - STAND_HIND.1).abs() < 1e-6);
    }

    /// A wing beats down quickly, spread wide, and comes back up more slowly folded at the wrist,
    /// opening again at the top, and the body's carried up by the downstroke
    #[test]
    fn wings_beat_down_hard_and_fold_coming_up() {
        assert!(DOWNSTROKE < 0.5);
        let at = |u: f64| beating(SPREAD, 0.8, 0.0, u);
        let (top, bottom) = (at(0.0), at(DOWNSTROKE));
        assert!(top.flap > SPREAD.flap + 0.6 && bottom.flap < SPREAD.flap - 0.5, "{top:?} {bottom:?}");
        let down = at(DOWNSTROKE * 0.5);
        let up = at(DOWNSTROKE + (0.95 - DOWNSTROKE) * 0.5);
        assert!(down.fold < 0.1 && up.fold > 0.9, "folds {:.2} going down and {:.2} coming up", down.fold, up.fold);
        assert!(up.bend < down.bend - 0.8, "its tip doesn't trail coming up");
        assert!(at(0.99).fold < 0.15, "it's not open again at the top");
        assert!(lift(DOWNSTROKE) > lift(0.0) && lift(DOWNSTROKE) > lift(0.9));
    }

    /// Struck dead on the ground, a drake rears with a last roar, then its legs fold, it rolls
    /// onto its side and its body comes down low, and it lies there before it's taken away
    #[test]
    fn a_dragon_killed_on_the_ground_crumples() {
        let vpm = 4u32;
        let world = open_ground(240, vpm);
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let v = vpm as f64;
        let feet = Vector3::new(120.0, 120.0, 4.0) * v;
        let mut a = Animal::boss(Look::Drake, feet + Vector3::new(6.0, 0.0, 0.0) * v, [1, 2, -1], 5);
        let dt = 1.0 / 30.0;
        for _ in 0..30 {
            a.update(&world, &water, Some(feet), dt);
        }
        let standing = a.dragon.as_ref().unwrap().center.z;
        a.damage(1e7, feet);
        let mut t = 0.0;
        let mut reared = 0.0f64;
        while dying(&a) && t < 10.0 {
            a.update(&world, &water, Some(feet), dt);
            reared = reared.max(a.dragon.as_ref().unwrap().rig.pitch);
            t += dt;
        }
        let d = a.dragon.as_ref().unwrap();
        assert!(reared > 0.15, "it never reared with a last roar");
        assert!(d.rig.roll.abs() > 0.6, "it didn't roll onto its side: {:.2}", d.rig.roll);
        assert!(d.center.z < standing - 0.3 * stand_height(&spec(Look::Drake)) * v, "its body didn't come down");
        assert!(t > 2.5 && t < 10.0, "lay {t:.1} s");
    }

    /// Hit hard, it flinches: its neck thrown about at once
    #[test]
    fn a_heavy_blow_makes_it_flinch() {
        let vpm = 4u32;
        let world = open_ground(240, vpm);
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let v = vpm as f64;
        let feet = Vector3::new(120.0, 120.0, 4.0) * v;
        let mut a = Animal::boss(Look::FrostWyrm, feet + Vector3::new(30.0, 0.0, 0.0) * v, [1, 2, -1], 5);
        let dt = 1.0 / 30.0;
        for _ in 0..20 {
            a.update(&world, &water, None, dt);
        }
        let before = a.dragon.as_ref().unwrap().rig.neck;
        a.damage(a.max_health * 0.05, feet);
        for _ in 0..4 {
            a.update(&world, &water, None, dt);
        }
        let d = a.dragon.as_ref().unwrap();
        let moved: f64 = (0..3).map(|k| (d.rig.neck[k][0] - before[k][0]).abs() + (d.rig.neck[k][1] - before[k][1]).abs()).sum();
        assert!(d.flinch > 0.0 && moved > 0.15, "its neck moved {moved:.2}");
    }
}



// The rig as the model and animation editor sees it, see src\anim.rs and src\editor.rs: a few
// lines, so the animations edited there go on top of the code's poses here, see edited()

/// What each of a dragon's bones is called, in the order of Dragon::bones
const BONE_NAMES: [&str; BONES] = [
    "torso",
    "neck_1",
    "neck_2",
    "neck_3",
    "head",
    "jaw",
    "tail_1",
    "tail_2",
    "tail_3",
    "tail_4",
    "wing_left_inner",
    "wing_left_outer",
    "wing_right_inner",
    "wing_right_outer",
    "thigh_hind_left",
    "thigh_hind_right",
    "thigh_fore_left",
    "thigh_fore_right",
    "shin_hind_left",
    "shin_hind_right",
    "shin_fore_left",
    "shin_fore_right",
];

/// The clips a dragon's animations are kept by: standing, walking and flying, then each move
const CLIPS: [&str; 15] = ["stand", "walk", "fly", "bite", "tail", "breath", "gust", "stomp", "roar", "swoop", "strafe", "hover", "spit", "dive", "rain"];

/// A dragon's rig, for the editor and for animation files
pub struct EditorRig(pub Look);

impl EditorRig {
    fn id_of(look: Look) -> String {
        format!("dragon/{}", crate::assets::snake(&format!("{look:?}")))
    }

    fn bone_defs(look: Look) -> Vec<crate::anim::BoneDef> {
        let sp = spec(look);
        let parent = |b: usize| -> Option<usize> {
            match b {
                TORSO => None,
                HEAD => Some(NECK + sp.necks.max(1) - 1),
                JAW => Some(HEAD),
                _ if b == NECK || b == TAIL => Some(TORSO),
                _ if b < WING => Some(b - 1),
                // An outer wing from its inner, an inner from the torso
                _ if b < THIGH => Some(if (b - WING) % 2 == 0 { TORSO } else { b - 1 }),
                _ if b < SHIN => Some(TORSO),
                _ => Some(b - SHIN + THIGH),
            }
        };
        (0..BONES).map(|b| crate::anim::BoneDef { name: BONE_NAMES[b].into(), parent: parent(b) }).collect()
    }
}

/// The clip a dragon's in, by CLIPS, and how many seconds into it: its move, getting ready and
/// making it, or flying, walking or standing
fn clip_now(d: &Dragon) -> (&'static str, f64) {
    let move_clip = |m: Move| CLIPS[3 + m as usize % (CLIPS.len() - 3)];
    match d.state {
        State::Wind(m, t) => (move_clip(m), t),
        State::Do(m, t) => (move_clip(m), d.skill.map_or(0.0, |s| s.wind) + t),
        _ if d.airborne => ("fly", d.clock),
        _ if flat(d.velocity).norm() > 0.3 => ("walk", d.clock),
        _ => ("stand", d.clock),
    }
}

/// Its bones as animation files have them on top of the code's pose, if there are any, see
/// src\anim.rs
fn edited(d: &mut Dragon, vpm: f64) {
    if !crate::anim::active() {
        return;
    }
    let (clip, t) = clip_now(d);
    let center = d.bones[TORSO].at;
    let mut joints: Vec<crate::anim::Joint> = d.bones.iter().map(|b| crate::anim::Joint { at: b.at - center, turn: b.turn, shown: b.shown }).collect();
    let look = d.look;
    crate::anim::adjust(&EditorRig::id_of(look), clip, t, &|| EditorRig::bone_defs(look), &mut joints, vpm);
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
        EditorRig::bone_defs(self.0)
    }

    fn clips(&self) -> Vec<crate::anim::ClipDef> {
        let sp = spec(self.0);
        let k = sp.s.sqrt();
        CLIPS
            .iter()
            .enumerate()
            .map(|(i, &name)| crate::anim::ClipDef {
                name: name.into(),
                length: match i {
                    0 => 4.0 * k,
                    1 => 1.2 * k,
                    2 => 1.0 / flight(self.0).beat,
                    _ => 1.6 * k,
                },
                looping: i < 3,
            })
            .collect()
    }

    /// The code's own poses: standing, swaying and breathing; walking, its legs striding; flying,
    /// its wings beating; and for a move, standing, or flying for a move made in the air
    fn pose(&self, clip: usize, t: f64) -> Vec<crate::anim::Joint> {
        let sp = spec(self.0);
        let k = sp.s.sqrt();
        let air = matches!(CLIPS.get(clip), Some(&("fly" | "swoop" | "strafe" | "hover" | "dive" | "rain")));
        let mut rig = if air { Rig::flying() } else { Rig::standing(&sp) };
        let sway = (t * 1.3 / k).sin();
        for j in 0..sp.tails {
            rig.tail[j][0] += 0.08 * sway * (j as f64 + 1.0) / sp.tails as f64;
        }
        rig.neck[0][1] += 0.03 * (t * 1.7 / k).sin();
        if air {
            let phase = t * flight(self.0).beat * TAU;
            for w in &mut rig.wings {
                w.flap += 0.9 * phase.sin();
                w.bend += 0.45 * 0.9 * (phase - 0.9).sin();
            }
        } else if clip == 1 {
            let phase = t / (1.2 * k) * TAU;
            for (l, leg) in rig.legs.iter_mut().enumerate() {
                let s = if l % 2 == 0 { 1.0 } else { -1.0 } * if l < 2 { 1.0 } else { -1.0 };
                leg.swing += 0.35 * s * phase.sin();
                leg.knee += 0.25 * (s * phase.cos()).max(0.0);
            }
        }
        let mut bones = [Bone::default(); BONES];
        pose(&sp, Vector3::zeros(), 0.0, &rig, 1.0, &mut bones);
        bones.iter().map(|b| crate::anim::Joint { at: b.at, turn: b.turn, shown: b.shown }).collect()
    }

    fn draw(&self, joints: &[crate::anim::Joint], at: Vector3<f64>, yaw: f64, vpm: f64, lit: Option<usize>, out: &mut Vec<AnimalInstance>) {
        let layout = layout();
        let k = kind(self.0);
        let base = animals::DRAGON_MODELS_AT + k * PIECES;
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
