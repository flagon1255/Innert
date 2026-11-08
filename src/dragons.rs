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
pub const KINDS: [Look; 8] = [
    Look::Ashwing,
    Look::Sleetwing,
    Look::Squallwing,
    Look::Blightwing,
    Look::PyreMatriarch,
    Look::CindermawTyrant,
    Look::GildscaleHoardwarden,
    Look::WorldpyreAncient,
];

/// The small ones, monsters going about in pairs and threes, an element each
pub const DRAKELINGS: [Look; 4] = [Look::Ashwing, Look::Sleetwing, Look::Squallwing, Look::Blightwing];

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
        Look::Sleetwing => Element::Frost,
        Look::Squallwing | Look::GildscaleHoardwarden => Element::Storm,
        Look::Blightwing => Element::Poison,
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
        _ => "The first fire took wing, and it is still burning. Every dragon there is was hatched from the embers it shed.",
    }
}

/// How much damage it takes to kill one
pub fn health(look: Look) -> f64 {
    match look {
        Look::Ashwing => 190.0,
        Look::Sleetwing => 170.0,
        Look::Squallwing => 175.0,
        Look::Blightwing => 200.0,
        Look::PyreMatriarch => 15500.0,
        Look::CindermawTyrant => 17500.0,
        Look::GildscaleHoardwarden => 16000.0,
        _ => 46000.0,
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
    /// Its legs each a single piece, bent as they always are
    stiff: bool,
    hide: Hide,
    crown: Crown,
    /// A spiked club at the end of its tail rather than a spade
    club: bool,
}

fn spec(look: Look) -> Spec {
    let hide = |scale, dark, belly, wing, horn, glow, eye| Hide { scale, dark, belly, wing, horn, glow, eye };
    let small = |s: f64, hide: Hide| Spec { s, four: false, necks: 1, tails: 2, neck: 0.75, tail: 0.9, girth: 1.0, span: 1.0, stiff: true, hide, crown: Crown::Swept, club: false };
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
            stiff: false,
            hide: hide(CLOTH, OBSIDIAN, BRONZE, TEMPLE_RED, BONE, GLOW_FIRE, FLAME),
            crown: Crown::Crest,
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
            stiff: false,
            hide: hide(OBSIDIAN, BARK_DARK, GLOW_FIRE, LEATHER, STONE, GLOW_FIRE, FLAME),
            crown: Crown::Swept,
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
            stiff: false,
            hide: hide(GOLD, BRONZE, LINEN, DARK_STEEL, BONE, GLOW_STORM, GLOW_STORM),
            crown: Crown::Gilded,
            club: false,
        },
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
            stiff: false,
            hide: hide(MORTAR, OBSIDIAN, BRONZE, CLOTH, BONE, GLOW_FIRE, FLAME),
            crown: Crown::Ancient,
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
        -hip(sp, hind).z + thigh * swing.cos() + shin * (swing - knee).cos() + 0.05
    };
    let hind = drop(true, STAND_HIND);
    let h = if sp.four { (hind + drop(false, STAND_FORE)) * 0.5 } else { hind };
    h * sp.s
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
        // Molten veins over the flanks of the oldest and of the tyrant
        Crown::Ancient => {
            for (k, side) in [(0, 1.0), (1, -1.0), (2, 1.0), (3, -1.0)] {
                let x = 0.45 - k as f64 * 0.3;
                p.push(limb([x, side * 0.3 * g, 0.12 * g], [x - 0.22, side * 0.33 * g, -0.06 * g], 0.022, 0.016, h.glow));
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
    // Spines down its back, tallest over the shoulders, a crest's taller still
    let tall = if sp.crown == Crown::Crest { 1.5 } else { 1.0 };
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
    let tall = if sp.crown == Crown::Crest { 1.6 } else { 1.0 };
    for j in 0..2 {
        let x = l * (0.25 + 0.45 * j as f64);
        let r = r0 + (r1 - r0) * x / l;
        p.push(limb([x, 0.0, r * 0.8], [x - 0.07, 0.0, r * 0.8 + 0.09 * tall], 0.035, 0.007, h.horn));
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
        }
    }
    if sp.crown == Crown::Ancient {
        p.push(limb([0.1, 0.0, 0.12], [-0.25, 0.0, 0.3], 0.04, 0.01, h.horn));
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
    let strips = 9;
    for i in 0..strips {
        let f = (i as f64 + 0.5) / strips as f64;
        let y = side * (0.04 + (reach - 0.04) * f);
        let lead = [0.1 * f, y, 0.03 * f];
        let trail = [-0.82 + 0.3 * f - 0.05 * (PI * f).sin(), y, 0.0];
        p.push(skin(lead, trail, (reach - 0.04) / strips as f64, h.wing));
    }
}

/// The trailing edge of a wing's outer piece, `y` out along it from the wrist, in units: in
/// between the fingers it's drawn in towards the leading edge
fn trailing(y: f64, l: f64) -> f64 {
    let points = [(0.0, -0.62), (0.16, -0.52), (0.32, -0.78), (0.47, -0.6), (0.62, -0.72), (0.74, -0.5), (0.86, -0.6), (1.0, -0.32)];
    let f = (y / l).clamp(0.0, 1.0);
    for pair in points.windows(2) {
        let ((f0, x0), (f1, x1)) = (pair[0], pair[1]);
        if f <= f1 {
            return x0 + (x1 - x0) * (f - f0) / (f1 - f0);
        }
    }
    -0.32
}

/// The outer piece, from the wrist out to the tip: the long bones of its fingers and the skin
/// between them
fn wing_out(p: &mut Vec<Part>, sp: &Spec, side: f64) {
    let h = sp.hide;
    let l = WING_OUT * sp.span;
    let tip = [-0.32, side * l, 0.03];
    let mid = [tip[0] * 0.5, tip[1] * 0.5, tip[2] * 0.5];
    p.push(limb([0.0, 0.0, 0.0], mid, 0.045, 0.032, h.dark));
    p.push(limb(mid, tip, 0.032, 0.008, h.dark));
    for (f, x) in [(0.32, -0.78), (0.62, -0.72), (0.86, -0.6)] {
        p.push(limb([0.0, 0.0, 0.0], [x, side * f * l, 0.0], 0.026, 0.008, h.dark));
    }
    let strips = 13;
    for i in 0..strips {
        // Starting a little inside the wrist, so there's no gap there as the wing bends
        let y = -0.04 + (l + 0.04) * (i as f64 + 0.5) / strips as f64;
        let lead = [-0.32 * (y / l).max(0.0), side * y, 0.03 * (y / l).max(0.0)];
        let trail = [trailing(y, l), side * y, 0.0];
        if lead[0] - trail[0] > 0.03 {
            p.push(skin(lead, trail, (l + 0.04) / strips as f64, h.wing));
        }
    }
}

/// A thigh, from the hip down to the knee, and for a drakeling's stiff legs the rest of the leg
/// too, bent as it stands
fn thigh(p: &mut Vec<Part>, sp: &Spec, hind: bool) {
    let (h, g) = (sp.hide, sp.girth);
    let (lt, _) = leg_lengths(hind);
    if sp.stiff {
        // The knee bent back as it stands, the shin and foot under the hip, the foot flat
        let (swing, knee) = if hind { STAND_HIND } else { STAND_FORE };
        let knee_at = [lt * swing.sin(), 0.0, -lt * swing.cos()];
        shin_and_foot(p, sp, hind, knee_at, swing - knee, 0.0);
    }
    p.push(ellipsoid([0.03, 0.0, -lt * 0.35], [0.13 * g, 0.11 * g, 0.2], h.scale));
    let end = if sp.stiff {
        let (swing, _) = if hind { STAND_HIND } else { STAND_FORE };
        [lt * swing.sin(), 0.0, -lt * swing.cos()]
    } else {
        [0.04, 0.0, -lt]
    };
    p.push(limb([0.0, 0.0, 0.0], end, 0.12 * g, 0.085 * g, h.scale));
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
    if !sp.stiff {
        // Built with the knee at its joint and the shin straight down, the foot turned so it's
        // flat on the ground as the leg stands
        let (swing, knee) = if hind { STAND_HIND } else { STAND_FORE };
        shin_and_foot(p, sp, hind, [0.0, 0.0, 0.0], 0.0, swing - knee);
    }
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
    made.into_iter().map(|(_, m)| m).collect()
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

// The rig: how each joint's turned, eased from where it is towards where the dragon's moves want
// it, so nothing ever snaps from one pose to the next

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

/// A leg's joints: how far forward the thigh's swung, and how far the knee's bent
#[derive(Clone, Copy, Default, Debug)]
struct LegPose {
    swing: f64,
    knee: f64,
}

/// Every joint of a dragon, in radians: its body's pitch, nose up, and roll, left wing up; each
/// joint of its neck turned to its left and raised; its head the same, and its jaw opened; each
/// joint of its tail the same; its wings, left then right; and its legs, the hind left and right,
/// then the fore
#[derive(Clone, Copy, Default, Debug)]
struct Rig {
    pitch: f64,
    roll: f64,
    neck: [[f64; 2]; 3],
    head: [f64; 2],
    jaw: f64,
    tail: [[f64; 2]; 4],
    wings: [WingPose; 2],
    legs: [LegPose; 4],
}

/// Wings folded at its sides, standing: the arm swept back from the shoulder, the skin hanging
/// down the flank from it, the fingers folded over at the wrist like a book shut, back along it
/// towards the chest
const FOLDED: WingPose = WingPose { flap: 0.1, sweep: 1.5, twist: -1.35, fold: 0.1, bend: -2.9 };
/// Wings spread, gliding
const SPREAD: WingPose = WingPose { flap: 0.1, sweep: 0.12, twist: 0.0, fold: 0.06, bend: 0.06 };

impl Rig {
    /// Standing on the ground as it does when nothing's the matter
    fn standing(sp: &Spec) -> Rig {
        let mut r = Rig { pitch: if sp.four { 0.0 } else { 0.12 }, ..Default::default() };
        // The neck rising in a curve, the head held level at its end
        let n = sp.necks as f64;
        r.neck[0] = [0.0, 0.62];
        for k in 1..3 {
            r.neck[k] = [0.0, -0.32 / n.max(1.0) * 2.0];
        }
        r.head = [0.0, -0.3];
        r.tail[0] = [0.0, -0.22];
        for k in 1..4 {
            r.tail[k] = [0.0, 0.07];
        }
        r.wings = [FOLDED; 2];
        let (hind, fore) = (LegPose { swing: STAND_HIND.0, knee: STAND_HIND.1 }, LegPose { swing: STAND_FORE.0, knee: STAND_FORE.1 });
        r.legs = [hind, hind, fore, fore];
        r
    }

    /// Flying: neck stretched out ahead, tail straight behind, legs tucked up
    fn flying() -> Rig {
        let mut r = Rig::default();
        r.neck[0] = [0.0, 0.18];
        r.head = [0.0, -0.15];
        r.tail[0] = [0.0, 0.04];
        r.wings = [SPREAD; 2];
        let (hind, fore) = (LegPose { swing: -0.95, knee: 1.5 }, LegPose { swing: -0.7, knee: 1.3 });
        r.legs = [hind, hind, fore, fore];
        r
    }

    /// Eased towards `to`, as `rate` says, about the fraction of the way a second
    fn ease(&mut self, to: &Rig, rate: f64, dt: f64) {
        let k = 1.0 - (-rate * dt).exp();
        let e = |a: &mut f64, b: f64| *a += (b - *a) * k;
        e(&mut self.pitch, to.pitch);
        e(&mut self.roll, to.roll);
        for j in 0..3 {
            e(&mut self.neck[j][0], to.neck[j][0]);
            e(&mut self.neck[j][1], to.neck[j][1]);
        }
        e(&mut self.head[0], to.head[0]);
        e(&mut self.head[1], to.head[1]);
        e(&mut self.jaw, to.jaw);
        for j in 0..4 {
            e(&mut self.tail[j][0], to.tail[j][0]);
            e(&mut self.tail[j][1], to.tail[j][1]);
        }
        for w in 0..2 {
            let (a, b) = (&mut self.wings[w], to.wings[w]);
            e(&mut a.flap, b.flap);
            e(&mut a.sweep, b.sweep);
            e(&mut a.twist, b.twist);
            e(&mut a.fold, b.fold);
            e(&mut a.bend, b.bend);
        }
        for l in 0..4 {
            e(&mut self.legs[l].swing, to.legs[l].swing);
            e(&mut self.legs[l].knee, to.legs[l].knee);
        }
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

/// Where every piece of a dragon built as `sp` is, its body's middle at `center` in voxels facing
/// `yaw`, its joints as `rig` has them; and where its mouth is and which way it breathes
fn pose(sp: &Spec, center: Vector3<f64>, yaw: f64, rig: &Rig, vpm: f64, bones: &mut [Bone; BONES]) -> (Vector3<f64>, Vector3<f64>) {
    let m = sp.s * vpm;
    let body = rz(yaw) * ry(-rig.pitch) * rx(rig.roll);
    let world = |local: Vector3<f64>| center + body * local * m;
    bones[TORSO] = Bone { at: center, turn: body, shown: true };
    // The neck, a joint at a time from the chest, then the head and its jaw
    let mut at = world(neck_root(sp));
    let mut turn = body;
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
    // The legs: a stiff one's piece is the whole leg, made standing, so it turns from there
    for leg in 0..4 {
        let hind = leg < 2;
        let shown = hind || sp.four;
        let l = rig.legs[leg];
        let (stand, _) = if hind { STAND_HIND } else { STAND_FORE };
        let thigh = if sp.stiff { ry(-(l.swing - stand)) } else { ry(-l.swing) };
        let joint = hip(sp, hind);
        let (lt, _) = leg_lengths(hind);
        let knee = joint + thigh * Vector3::new(0.04, 0.0, -lt);
        let shin = thigh * ry(l.knee);
        let (thigh, shin, joint, knee) = if leg % 2 == 0 { (thigh, shin, joint, knee) } else { (mirrored(thigh), mirrored(shin), flip(joint), flip(knee)) };
        bones[THIGH + leg] = Bone { at: world(joint), turn: body * thigh, shown };
        bones[SHIN + leg] = Bone { at: world(knee), turn: body * shin, shown: shown && !sp.stiff };
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
        let (lt, ls) = leg_lengths(leg < 2);
        let b = bones[THIGH + leg];
        let foot = if sp.stiff { b.at + b.turn * Vector3::new(0.0, 0.0, -(lt + ls) * 0.8) * m } else { bones[SHIN + leg].at + bones[SHIN + leg].turn * Vector3::new(0.0, 0.0, -ls) * m };
        out.push((b.at, foot, 0.12 * sp.girth * sp.s));
    }
    out
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

fn skills(look: Look) -> &'static [Skill] {
    match look {
        Look::PyreMatriarch => &MATRIARCH,
        Look::CindermawTyrant => &TYRANT,
        Look::GildscaleHoardwarden => &WARDEN,
        Look::WorldpyreAncient => &ANCIENT,
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
        _ if is_drakeling(look) => Power { bite: 9.0, claw: 11.0, tail: 10.0, stomp: 0.0, crack: 0.0, breath: 16.0, ball: 10.0, rain: 0.0, gust: 4.0, push: 7.0 },
        Look::WorldpyreAncient => Power { bite: 34.0, claw: 32.0, tail: 36.0, stomp: 38.0, crack: 22.0, breath: 28.0, ball: 22.0, rain: 20.0, gust: 11.0, push: 12.0 },
        _ => Power { bite: 26.0, claw: 28.0, tail: 30.0, stomp: 32.0, crack: 18.0, breath: 24.0, ball: 18.0, rain: 17.0, gust: 9.0, push: 10.0 },
    }
}

/// How near the player has to come before it turns on them, how far off they can get before it
/// gives up, and how far from home it'll follow them, in meters
fn senses(look: Look) -> (f64, f64, f64) {
    match look {
        _ if is_drakeling(look) => (30.0, 60.0, 90.0),
        Look::WorldpyreAncient => (80.0, 180.0, 260.0),
        _ => (55.0, 110.0, 160.0),
    }
}

/// What a dragon does that no other boss does, for src\game\dragonfire.rs to see to. Positions are
/// in voxels.
#[derive(Clone, Copy, Debug)]
pub enum Act {
    /// Getting ready to make move `m`, heard as it does, its head at `at`
    Warn { m: Move, at: Vector3<f64> },
    /// A roar: on `first` turning on the player, going into the next phase of its fight, or calling
    /// down `brood`, so many of them
    Roar { at: Vector3<f64>, brood: Option<(Look, usize)>, first: bool },
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
    to: Vector3<f64>,
    speed: f64,
    face: Option<Vector3<f64>>,
    low: bool,
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
    wings: WingSet,
    /// How far through a beat of its wings and a stride of its legs it is, 0 to 1
    beat: f64,
    stride: f64,
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
            wings: WingSet { base: FOLDED, amp: 0.0, rate: 0.0 },
            beat: 0.0,
            stride: 0.0,
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
        self.wings = WingSet { base: SPREAD, amp: 0.6, rate: flight(self.look).beat };
        self.rig = Rig::flying();
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

/// Whether a dragon that's been killed is still falling out of the sky, or lying where it fell, so
/// it's not to be taken away yet, see Game::reap()
pub fn dying(a: &Animal) -> bool {
    let Some(d) = &a.dragon else {
        return false;
    };
    let lie = if is_drakeling(a.look) { 1.4 } else { 2.6 };
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
    }
    d.clock += dt;
    for c in &mut d.cool {
        *c -= here.dt;
    }
    d.reeled -= dt;
    // How hard it's been hit lately
    let lost = (d.health - a.health).max(0.0);
    d.health = a.health;
    d.recent = d.recent * (-dt / 1.5).exp() + lost;
    // Thrown back by a blow, sliding to a stop, or knocked about in the air
    let push = a.status.push * dt;
    if push.norm() > 1e-3 {
        d.center += Vector3::new(push.x, push.y, 0.0);
    }
    a.status.push *= (-6.0 * dt).exp();

    let mut goal = Goal { to: d.center, speed: 0.0, face: None, low: false };
    let mut want = if d.airborne { Rig::flying() } else { Rig::standing(&sp) };
    let mut wings = if d.airborne { WingSet { base: SPREAD, amp: 0.55, rate: flight(look).beat } } else { WingSet { base: FOLDED, amp: 0.0, rate: 0.0 } };

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
        travel(d, &here, &goal, &mut want);
    }
    let beat = d.beat;
    animate(d, &here, &want, &wings, a.health <= 0.0);
    // Each downstroke of its wings heard as it starts
    if d.airborne && d.wings.amp > 0.3 && beat < 0.25 && d.beat >= 0.25 {
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
        let roar = Act::Roar { at: d.mouth, brood: None, first: true };
        boss.deeds.push(Deed::Dragon(roar));
        // A drakeling on the ground takes off unless they're right by it; a great one roars first
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
    if boss.engaged && !boss.raider && (h.feet.is_none() || distance > give_up || from_home > leash || d.unseen > 12.0) {
        boss.calm();
    }
    if !boss.engaged && !matches!(d.state, State::Rest | State::Reel(_) | State::Alight(_)) {
        d.state = State::Rest;
        d.skill = None;
    }
    // A great one going into the next phase of its fight: roaring, calling its brood, and in the
    // last of them quicker about everything
    let marks = phases(h.look);
    if boss.engaged && (boss.stage as usize) < marks.len() && a.vitality() < marks[boss.stage as usize] && !matches!(d.state, State::Do(..) | State::Reel(_) | State::Fall) {
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
}

/// How long it means to keep to the air, or the ground, from now
fn pick_budget(d: &mut Dragon, h: &Here, air: bool) -> f64 {
    let (aloft, down) = budgets(h.look, d.stage);
    let [lo, hi] = if air { aloft } else { down };
    lo + (hi - lo) * d.roll()
}

/// What it can do from where it is, if anything's ready, picked by how likely each is
fn choose(d: &mut Dragon, h: &Here, stage: u8, distance: f64) -> Option<Skill> {
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
    let mut spot = None;
    for k in 0..8 {
        let turn = [0.0, 0.6, -0.6, 1.2, -1.2, 1.9, -1.9, PI][k];
        let (c, si) = (turn.cos(), turn.sin());
        let way = Vector3::new(away.x * c - away.y * si, away.x * si + away.y * c, 0.0);
        let at = around + way * off;
        if let Some(z) = ground_z(h.world, at.x, at.y, at.z + 6.0 * vpm, 30.0)
            && (h.water)(at.x, at.y) < z + 0.2 * vpm
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

/// Its head turned towards `at`, its neck sharing the turn, as far as they go
fn look_at(d: &Dragon, h: &Here, want: &mut Rig, at: Vector3<f64>) {
    let neck = d.bones[NECK].at;
    let body = rz(d.yaw);
    let local = body.inverse() * (at - neck);
    let yaw = local.y.atan2(local.x).clamp(-1.3, 1.3);
    let pitch = (local.z.atan2(flat(local).norm()) - 0.1).clamp(-0.9, 0.6);
    let n = h.sp.necks as f64;
    for k in 0..h.sp.necks {
        want.neck[k][0] += yaw * 0.55 / n;
    }
    want.head[0] += yaw * 0.45;
    want.head[1] += pitch * 0.7;
    want.neck[0][1] += pitch * 0.3;
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
    let fl = flight(h.look);
    let pw = power(h.look);
    let (bite_r, claw_r, tail_r, stomp_r, gust_r) = reaches(&h.sp);
    let feet = h.feet.unwrap_or(boss.home);
    let ground_here = ground_of(h, d.center);
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
                if d.blocked > 3.0 && flat(d.center - boss.home).norm() > 6.0 * k * vpm {
                    d.state = State::Rise(0.0);
                }
                want.head[0] += 0.5 * (d.clock * 0.37 + a.id as f64).sin();
                want.neck[0][1] += 0.06 * (d.clock * 1.1).sin();
            }
        }
        State::Hunt => {
            let stage = boss.stage;
            if d.airborne {
                // Circling over the player, round and round, near enough to see them always
                d.orbit += d.round * fl.cruise / fl.orbit * h.dt;
                let round = Vector3::new(d.orbit.cos(), d.orbit.sin(), 0.0) * fl.orbit * vpm;
                let at = feet + round;
                goal.to = Vector3::new(at.x, at.y, ground_of(h, feet).max(ground_here) + fl.altitude * vpm);
                goal.speed = fl.cruise;
                if distance > fl.orbit * 2.0 {
                    goal.to = feet + Vector3::new(0.0, 0.0, fl.altitude * vpm);
                }
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
                // On foot: up to the player, facing them, then whatever's in reach
                let close = bite_reach(&h.sp) * 0.85;
                goal.face = Some(feet);
                if distance > close {
                    goal.to = feet;
                    goal.speed = if distance > close + 4.0 * k { speeds(h.look).1 } else { speeds(h.look).0 };
                }
                look_at(d, h, want, feet + Vector3::new(0.0, 0.0, 1.2 * vpm));
                let facing = turn_to(d.yaw, flat(feet - d.center).y.atan2(flat(feet - d.center).x)).abs() < 0.6;
                d.next -= h.dt;
                if d.next <= 0.0 && facing && boss.engaged {
                    if let Some(skill) = choose(d, h, stage, distance) {
                        begin(d, h, skill.m, false);
                    }
                }
                // Up again once it's had enough of the ground, or the player's out of reach
                let unreachable = (feet.z - ground_here).abs() > 3.5 * vpm || d.blocked > 1.5;
                if matches!(d.state, State::Hunt) && (d.aloft > d.budget || unreachable || distance > 18.0 * k) {
                    d.state = State::Rise(0.0);
                }
            }
        }
        State::Rise(t) => {
            let crouch = 0.3 * k;
            // Crouching, wings spread and raised, then springing up into the air beating them hard
            *wings = WingSet { base: WingPose { flap: 0.9, sweep: -0.1, twist: 0.1, fold: 0.15, bend: 0.2 }, amp: 0.2, rate: 0.6 * fl.beat };
            if t < crouch {
                for leg in &mut want.legs {
                    leg.knee += 0.5;
                    leg.swing += 0.2;
                }
                want.pitch += 0.15;
                goal.face = h.feet;
            } else {
                if !d.airborne {
                    d.airborne = true;
                    d.blocked = 0.0;
                    d.velocity = Vector3::new(forward.x, forward.y, 0.0) * 2.0 * vpm + Vector3::new(0.0, 0.0, 6.5 * k.powf(0.4) * vpm);
                    act_out(boss, Act::TakeOff { at: d.center - Vector3::new(0.0, 0.0, h.stand), size: s });
                }
                *wings = WingSet { base: WingPose { flap: 0.25, sweep: 0.05, twist: 0.1, fold: 0.05, bend: 0.1 }, amp: 0.85, rate: fl.beat * 1.6 };
                goal.to = d.center + forward * 3.0 * vpm + Vector3::new(0.0, 0.0, 8.0 * vpm);
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
            // Down to the spot picked, flaring its wings to slow itself, legs out to land
            let spot = d.aim;
            let over = flat(spot - d.center).norm() / vpm;
            let height = (d.center.z - spot.z - h.stand) / vpm;
            // Gliding down as it comes in, then straight down onto it
            let up = if over > 3.0 * k { height.min(over * 0.5).max(0.0) } else { 0.0 };
            goal.to = Vector3::new(spot.x, spot.y, spot.z + h.stand + up * vpm);
            goal.speed = if over > 3.0 * k { fl.cruise * 0.75 } else { 3.5 * k };
            goal.low = true;
            goal.face = Some(h.feet.unwrap_or(spot + forward * vpm));
            if over < 6.0 * k {
                // Flaring
                *wings = WingSet { base: WingPose { flap: 0.35, sweep: -0.35, twist: -0.4, fold: 0.2, bend: 0.25 }, amp: 0.7, rate: fl.beat * 1.4 };
                want.pitch += 0.35;
                for leg in &mut want.legs {
                    leg.swing = 0.6;
                    leg.knee = 0.6;
                }
            }
            if (over < 1.5 * k && height <= 0.15 * s) || t > 7.0 {
                d.airborne = false;
                d.velocity = Vector3::zeros();
                d.center.z = spot.z + h.stand;
                act_out(boss, Act::Land { at: Vector3::new(d.center.x, d.center.y, spot.z), size: s });
                d.aloft = 0.0;
                d.budget = pick_budget(d, h, false);
                d.state = if boss.engaged { State::Spent(0.35 * k) } else { State::Rest };
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
                Bite => {
                    goal.face = Some(feet);
                    // Rearing its head back to snap
                    want.neck[0][1] += 0.3;
                    want.head[1] += 0.25;
                    want.jaw = 0.25 * t / wind;
                    let at = d.center + forward * bite_reach(&h.sp) * vpm;
                    mark(boss, Vector3::new(at.x, at.y, ground_of(h, at)), bite_r, 1.0 - t / wind);
                }
                Tail => {
                    // Coiling away the way it'll spin, its tail raised, looking back over its
                    // shoulder; the ring round it where the tail will sweep
                    let away = toward.y.atan2(toward.x) - d.sweep * 0.6;
                    goal.face = Some(d.center + Vector3::new(away.cos(), away.sin(), 0.0) * vpm);
                    want.tail[0][1] += 0.5;
                    for j in 1..4 {
                        want.tail[j][1] += 0.12;
                        want.tail[j][0] += d.sweep * 0.25;
                    }
                    want.head[0] -= d.sweep * 0.8;
                    let ground = Vector3::new(d.center.x, d.center.y, ground_here);
                    mark(boss, ground, tail_r + 0.4, 1.0 - t / wind);
                }
                Breath => {
                    goal.face = Some(feet);
                    // Rearing up, fire gathering in its throat
                    want.neck[0][1] += 0.35;
                    want.head[1] += 0.35;
                    want.jaw = 0.45 * t / wind;
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
                    // Wings raised high and spread forward, rearing
                    *wings = WingSet { base: WingPose { flap: 1.25, sweep: -0.3, twist: 0.2, fold: 0.15, bend: 0.35 }, amp: 0.05, rate: 1.0 };
                    want.pitch += 0.25;
                    if d.airborne {
                        goal.to = d.center;
                        goal.speed = 2.0;
                    }
                    let ground = Vector3::new(d.center.x, d.center.y, ground_here);
                    for j in 1..4 {
                        let p = ground + forward * gust_r * (j as f64 / 3.5) * vpm;
                        mark(boss, Vector3::new(p.x, p.y, ground_of(h, p)), 0.8 + 0.5 * j as f64 * s.powf(0.4), 1.0 - t / wind);
                    }
                }
                Stomp => {
                    goal.face = Some(feet);
                    // Rearing up on its hind legs, its forefeet raised
                    want.pitch += 0.45;
                    want.legs[2] = LegPose { swing: 1.1, knee: 0.4 };
                    want.legs[3] = LegPose { swing: 1.1, knee: 0.4 };
                    want.neck[0][1] += 0.2;
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
                    want.neck[0][1] += 0.5;
                    want.head[1] += 0.5;
                    want.jaw = 0.4 * t / wind;
                    if d.airborne {
                        goal.to = d.center;
                        goal.speed = 3.0;
                    }
                }
                Spit => {
                    goal.face = Some(feet);
                    want.neck[0][1] += 0.25;
                    want.head[1] += 0.3;
                    want.jaw = 0.5 * t / wind;
                    if (d.clock * 12.0).fract() < h.dt * 12.0 {
                        act_out(boss, Act::Kindle { at: d.mouth, size: s });
                    }
                    if d.airborne {
                        goal.to = d.center + toward * vpm;
                        goal.speed = 3.0;
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
                    *wings = WingSet { base: WingPose { flap: 0.95, sweep: 0.1, twist: 0.0, fold: 0.1, bend: 0.2 }, amp: 0.15, rate: fl.beat };
                    look_at(d, h, want, feet);
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
                    // In beside the player, just over their head, beating its wings hard
                    let side = Vector3::new(-toward.y, toward.x, 0.0) * d.sweep;
                    let at = feet - toward * 1.6 * vpm + side * 1.2 * vpm;
                    goal.to = Vector3::new(at.x, at.y, ground_of(h, feet) + h.stand + 1.4 * vpm);
                    goal.speed = fl.cruise * 0.6;
                    goal.face = Some(feet);
                    goal.low = true;
                    *wings = WingSet { base: WingPose { flap: 0.3, sweep: -0.2, twist: -0.25, fold: 0.1, bend: 0.2 }, amp: 0.8, rate: fl.beat * 1.5 };
                    want.pitch += 0.3;
                    if !d.ready {
                        t = 0.0;
                        if (d.center - goal.to).norm() < 1.2 * vpm || d.aloft > d.budget + 6.0 {
                            d.ready = true;
                        }
                    } else {
                        want.neck[0][1] += 0.4;
                        want.head[1] += 0.2;
                        want.jaw = 0.3 * t / wind;
                        mark(boss, Vector3::new(feet.x, feet.y, ground_of(h, feet)), bite_r, 1.0 - t / wind);
                    }
                }
                Dive => {
                    // Climbing over where the player stands, which it'll fall on, the ring showing it
                    goal.to = Vector3::new(d.aim.x, d.aim.y, ground_of(h, d.aim) + (fl.altitude + 6.0 * k) * vpm);
                    goal.speed = fl.cruise * 0.8;
                    goal.face = Some(d.aim);
                    *wings = WingSet { base: WingPose { flap: 0.6, sweep: 0.0, twist: 0.0, fold: 0.1, bend: 0.2 }, amp: 0.7, rate: fl.beat * 1.3 };
                    if t < wind * 0.5 {
                        d.aim = feet;
                    }
                    mark(boss, Vector3::new(d.aim.x, d.aim.y, ground_of(h, d.aim)), stomp_r * 1.15, 1.0 - 0.7 * t / wind);
                }
                Rain => {
                    // Climbing high, roaring
                    goal.to = Vector3::new(feet.x, feet.y, ground_of(h, feet) + (fl.altitude + 8.0 * k) * vpm) + Vector3::new(d.orbit.cos(), d.orbit.sin(), 0.0) * fl.orbit * 0.6 * vpm;
                    goal.speed = fl.cruise * 0.8;
                    want.neck[0][1] += 0.5;
                    want.head[1] += 0.4;
                    want.jaw = 0.5 * t / wind;
                    if !d.airborne {
                        goal.to = d.center;
                        goal.face = Some(feet);
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
                    // Lunging, jaws wide, then snapping shut
                    goal.to = d.center + forward * 3.0 * vpm;
                    goal.speed = 2.0 * k;
                    want.neck[0][1] -= 0.25;
                    want.head[1] -= 0.25;
                    want.jaw = if t < length * 0.45 { 0.9 } else { 0.0 };
                    if !d.struck && t >= length * 0.45 {
                        d.struck = true;
                        let at = d.center + forward * bite_reach(&h.sp) * vpm;
                        act_out(boss, Act::Bite { at: Vector3::new(at.x, at.y, ground_of(h, at)), radius: bite_r, damage: pw.bite });
                    }
                    done = t >= length;
                }
                Tail => {
                    let length = 0.55 * k.powf(0.4);
                    // Round on the spot, its tail sweeping out low
                    d.yaw += d.sweep * TAU * 0.85 / length * h.dt;
                    want.tail[0][1] -= 0.05;
                    for j in 0..4 {
                        want.tail[j][0] -= d.sweep * 0.3;
                    }
                    if !d.struck && t >= length * 0.5 {
                        d.struck = true;
                        act_out(boss, Act::Sweep { at: Vector3::new(d.center.x, d.center.y, ground_here), radius: tail_r, damage: pw.tail });
                    }
                    done = t >= length;
                }
                Breath => {
                    let length = if is_drakeling(h.look) { 1.3 } else if h.look == ULTRA { 2.6 } else { 2.1 } * if boss.enraged { 1.2 } else { 1.0 };
                    // Sweeping across the ground ahead, the body turning slowly after the player
                    goal.face = Some(feet);
                    let across = d.sweep * (t / length - 0.5) * 1.1;
                    want.neck[0][0] += across * 0.4;
                    want.head[0] += across * 0.6;
                    want.neck[0][1] -= 0.1;
                    want.head[1] -= 0.45;
                    want.jaw = 0.8;
                    let size = 1.2 * s.powf(0.6);
                    act_out(boss, Act::Breath { from: d.mouth, dir: d.breath, dt: h.dt, dps: pw.breath, size, speed: 10.0 * s.powf(0.3), spread: 0.0 });
                    done = t >= length;
                }
                Gust => {
                    let length = 0.45 * k.powf(0.3);
                    // Beating down hard, the ground blown clear before it
                    *wings = WingSet { base: WingPose { flap: -0.55, sweep: 0.05, twist: -0.1, fold: 0.05, bend: -0.3 }, amp: 0.05, rate: 1.0 };
                    if !d.struck && t >= length * 0.3 {
                        d.struck = true;
                        act_out(boss, Act::Gust { at: Vector3::new(d.center.x, d.center.y, ground_here), dir: forward, reach: gust_r, push: pw.push, damage: pw.gust });
                    }
                    done = t >= length;
                }
                Stomp => {
                    let length = 0.3 * k.powf(0.3);
                    want.pitch -= 0.1;
                    if !d.struck && t >= length * 0.4 {
                        d.struck = true;
                        let at = d.center + forward * 0.7 * s * vpm;
                        act_out(boss, Act::Stomp { at: Vector3::new(at.x, at.y, ground_of(h, at)), radius: stomp_r, damage: pw.stomp, cracks: d.cracks, length: 8.0 * s.powf(0.5), crack: pw.crack });
                    }
                    done = t >= length;
                }
                Roar => {
                    let length = 1.3;
                    want.neck[0][1] += 0.55;
                    want.head[1] += 0.55;
                    want.jaw = 1.0;
                    *wings = if d.airborne { *wings } else { WingSet { base: WingPose { flap: 0.8, sweep: 0.2, twist: 0.0, fold: 0.6, bend: 0.2 }, amp: 0.05, rate: 1.0 } };
                    if d.airborne {
                        goal.to = d.center;
                        goal.speed = 2.0;
                    }
                    if !d.struck {
                        d.struck = true;
                        let brood = brood(h.look).filter(|_| d.stage >= if h.look == ULTRA { 2 } else { 1 }).map(|b| (b, if h.look == ULTRA { 3 } else { 2 }));
                        act_out(boss, Act::Roar { at: d.mouth, brood, first: false });
                    }
                    done = t >= length;
                }
                Spit => {
                    if !d.struck {
                        d.struck = true;
                        let lob = h.look == Look::CindermawTyrant;
                        let count = if is_drakeling(h.look) { 1 } else { 3 };
                        let target = d.aim + Vector3::new(0.0, 0.0, 0.8 * vpm);
                        let across = Vector3::new(-toward.y, toward.x, 0.0);
                        for j in 0..count {
                            let off = (j as f64 - (count - 1) as f64 * 0.5) * 3.2 * vpm;
                            let speed = if lob { 14.0 } else { 18.0 * s.powf(0.15) };
                            act_out(boss, Act::Fireball { from: d.mouth, to: target + across * off, speed, damage: pw.ball, radius: 1.6 * s.powf(0.35), lob });
                        }
                    }
                    want.jaw = 0.7;
                    want.head[1] -= 0.2;
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
                    // Wings swept back for the dive, beating for the climb, claws out low down
                    *wings = if u < 0.45 { WingSet { base: WingPose { flap: 0.35, sweep: 0.75, twist: 0.1, fold: 0.6, bend: 0.0 }, amp: 0.05, rate: 1.0 } } else { WingSet { base: SPREAD, amp: 0.85, rate: fl.beat * 1.4 } };
                    if (0.3..0.65).contains(&u) {
                        for leg in &mut want.legs {
                            leg.swing = 0.9;
                            leg.knee = 0.3;
                        }
                        want.head[1] -= 0.3;
                        want.jaw = 0.6;
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
                    want.neck[0][1] -= 0.25;
                    want.head[1] -= 0.55;
                    want.jaw = 0.85;
                    *wings = WingSet { base: SPREAD, amp: 0.4, rate: fl.beat };
                    // Breathing down and ahead, so it falls along the line below
                    let down = (d.way * 0.55 + Vector3::new(0.0, 0.0, -1.0)).normalize();
                    let spread = if h.look == ULTRA { 3.0 } else if is_drakeling(h.look) { 0.0 } else { 1.5 };
                    act_out(boss, Act::Breath { from: d.mouth, dir: down, dt: h.dt, dps: pw.breath, size: 1.3 * s.powf(0.6), speed: 12.0 * s.powf(0.25), spread });
                    done = t >= length;
                }
                Hover => {
                    let length = 0.3;
                    goal.to = d.center;
                    goal.speed = 1.0;
                    goal.face = Some(feet);
                    goal.low = true;
                    *wings = WingSet { base: WingPose { flap: 0.3, sweep: -0.2, twist: -0.25, fold: 0.1, bend: 0.2 }, amp: 0.8, rate: fl.beat * 1.5 };
                    want.neck[0][1] -= 0.3;
                    want.head[1] -= 0.5;
                    want.jaw = if t < length * 0.4 { 0.9 } else { 0.0 };
                    if !d.struck && t >= length * 0.4 {
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
                    // Falling on the ring, wings swept tight, and down hard
                    let to = Vector3::new(d.aim.x, d.aim.y, ground_of(h, d.aim) + h.stand);
                    let off = to - d.center;
                    d.velocity = off.try_normalize(1e-6).unwrap_or_else(|| -Vector3::z()) * fl.dash * vpm;
                    goal.to = to;
                    goal.speed = -1.0;
                    d.center += d.velocity * h.dt;
                    *wings = WingSet { base: WingPose { flap: 0.6, sweep: 0.9, twist: 0.0, fold: 0.9, bend: 0.0 }, amp: 0.0, rate: 1.0 };
                    want.pitch -= 0.5;
                    mark(boss, Vector3::new(d.aim.x, d.aim.y, ground_of(h, d.aim)), stomp_r * 1.15, (off.norm() / (fl.altitude * vpm * 2.0)).min(0.3));
                    if off.norm() < fl.dash * vpm * h.dt * 1.5 || d.center.z <= to.z || t > 4.0 {
                        d.center = to;
                        d.airborne = false;
                        d.velocity = Vector3::zeros();
                        act_out(boss, Act::Stomp { at: Vector3::new(to.x, to.y, to.z - h.stand), radius: stomp_r * 1.15, damage: pw.stomp * 1.1, cracks: d.cracks, length: 8.0 * s.powf(0.5), crack: pw.crack });
                        act_out(boss, Act::Land { at: Vector3::new(to.x, to.y, to.z - h.stand), size: s });
                        d.aloft = 0.0;
                        d.budget = pick_budget(d, h, false);
                        // Dazed a moment from coming down so hard: the moment to get at it
                        d.state = State::Spent(2.4 * k.powf(0.3));
                        d.skill = None;
                        return;
                    }
                    done = false;
                }
                Rain => {
                    let length = 3.0;
                    let every = if h.look == ULTRA { 0.13 } else { 0.18 };
                    let spread = if h.look == ULTRA { 18.0 } else { 13.0 };
                    if d.airborne {
                        d.orbit += d.round * fl.cruise / fl.orbit * h.dt * 0.7;
                        goal.to = Vector3::new(feet.x, feet.y, ground_of(h, feet) + (fl.altitude + 8.0 * k) * vpm) + Vector3::new(d.orbit.cos(), d.orbit.sin(), 0.0) * fl.orbit * 0.6 * vpm;
                        goal.speed = fl.cruise * 0.7;
                    } else {
                        goal.face = Some(feet);
                        want.neck[0][1] += 0.5;
                        want.head[1] += 0.5;
                        want.jaw = 0.8;
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
            if d.airborne {
                goal.to = d.center + forward * 2.0 * vpm + Vector3::new(0.0, 0.0, 0.5 * vpm);
                goal.speed = fl.cruise * 0.5;
            } else {
                goal.face = None;
            }
            let t = t - h.dt;
            d.state = if t > 0.0 {
                State::Spent(t)
            } else {
                // A pack takes turns, so they don't all come at once
                let gap = if is_drakeling(h.look) { 2.4 + 1.6 * d.roll() + boss.flock.slot as f64 * 0.9 } else if h.look == ULTRA { 0.9 + 0.7 * d.roll() } else { 1.2 + 0.8 * d.roll() };
                d.next = gap * h.quick;
                if boss.engaged { State::Hunt } else { State::Rest }
            };
        }
        State::Reel(t) => {
            // Falling out of the sky, wings flailing, then lying dazed where it came down
            *wings = WingSet { base: WingPose { flap: 0.4, sweep: 0.5, twist: 0.3, fold: 1.0, bend: 0.5 }, amp: 0.35, rate: fl.beat * 0.8 };
            if d.airborne {
                d.velocity.z -= 14.0 * vpm * h.dt;
                d.velocity.x *= (-1.5 * h.dt).exp();
                d.velocity.y *= (-1.5 * h.dt).exp();
                want.roll += 0.5;
                want.pitch -= 0.3;
                goal.speed = -1.0;
                d.center += d.velocity * h.dt;
                if d.center.z <= ground_here + h.stand * 0.8 {
                    d.center.z = ground_here + h.stand * 0.8;
                    d.airborne = false;
                    d.velocity = Vector3::zeros();
                    act_out(boss, Act::Crash { at: Vector3::new(d.center.x, d.center.y, ground_here), size: s });
                }
                d.state = State::Reel(t);
            } else {
                *wings = WingSet { base: WingPose { flap: 0.0, sweep: 0.35, twist: 0.2, fold: 0.4, bend: -0.2 }, amp: 0.0, rate: 1.0 };
                want.neck[0][1] -= 0.6;
                want.head[1] += 0.2;
                want.jaw = 0.25;
                for leg in &mut want.legs {
                    leg.swing += 0.5;
                    leg.knee -= 0.3;
                }
                let t = t - h.dt;
                d.state = if t > 0.0 {
                    State::Reel(t)
                } else if boss.engaged {
                    d.reeled = 12.0;
                    State::Rise(0.0)
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

/// Killed: falling out of the sky if it's flying, crashing down, and lying still
fn fall(_a: &mut Animal, d: &mut Dragon, boss: &mut Boss, h: &Here, goal: &mut Goal, want: &mut Rig, wings: &mut WingSet) {
    let vpm = h.vpm;
    if d.state != State::Fall {
        d.state = State::Fall;
        act_out(boss, Act::Death { at: d.mouth });
    }
    goal.speed = -1.0;
    let ground = ground_of(h, d.center);
    if d.airborne {
        *wings = WingSet { base: WingPose { flap: 0.5, sweep: 0.6, twist: 0.3, fold: 1.2, bend: 0.4 }, amp: 0.2, rate: 1.0 };
        d.velocity.z -= 12.0 * vpm * h.dt.max(1e-3);
        d.velocity.x *= (-1.0 * h.dt).exp();
        d.velocity.y *= (-1.0 * h.dt).exp();
        d.center += d.velocity * h.dt;
        want.roll += 0.7;
        want.pitch -= 0.4;
        if d.center.z <= ground + h.stand * 0.5 {
            d.center.z = ground + h.stand * 0.5;
            d.airborne = false;
            d.velocity = Vector3::zeros();
            act_out(boss, Act::Crash { at: Vector3::new(d.center.x, d.center.y, ground), size: h.sp.s });
        }
    } else {
        d.dead = Some(d.dead.unwrap_or(0.0) + h.dt.max(1.0 / 120.0));
        d.center.z += (ground + h.stand * 0.45 - d.center.z) * (1.0 - (-4.0 * h.dt).exp());
        *wings = WingSet { base: WingPose { flap: -0.15, sweep: 0.35, twist: 0.0, fold: 0.35, bend: -0.1 }, amp: 0.0, rate: 1.0 };
        want.neck[0][1] = -0.15;
        for j in 1..3 {
            want.neck[j][1] = 0.05;
        }
        want.head[1] = 0.15;
        want.jaw = 0.35;
        want.pitch = 0.0;
        for leg in &mut want.legs {
            leg.swing = 1.3;
            leg.knee = 0.2;
        }
        for j in 0..4 {
            want.tail[j] = [0.12, -0.02];
        }
    }
}

/// Moving it towards where it means to go: flying, steering and banking as it turns, never into
/// the ground; or walking on it
fn travel(d: &mut Dragon, h: &Here, goal: &Goal, want: &mut Rig) {
    let vpm = h.vpm;
    let fl = flight(h.look);
    let k = h.sp.s.sqrt();
    let dt = h.dt;
    let before = d.yaw;
    if d.airborne {
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
        // Banking into its turns, and climbing nose up
        let rate = turn_to(before, d.yaw) / dt.max(1e-6);
        want.roll += (-rate * speed * 0.06).clamp(-0.8, 0.8);
        let climb = d.velocity.z.atan2(flat(d.velocity).norm().max(1e-6));
        want.pitch += (climb * 0.6).clamp(-0.5, 0.5);
    } else if matches!(d.state, State::Fall) {
        // Lying still
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
        match (ground_now, ground_next) {
            (_, Some(z)) if speed > 0.0 && (z - (d.center.z - stand)).abs() < climb && (h.water)(next.x, next.y) < z + 0.4 * k * vpm => {
                d.center.x = next.x;
                d.center.y = next.y;
                d.blocked = 0.0;
                d.stride = (d.stride + speed * dt / stride(h.look)).fract();
            }
            _ if speed > 0.0 => d.blocked += dt,
            _ => {}
        }
        let ground = ground_z(h.world, d.center.x, d.center.y, d.center.z - stand + 1.0 * vpm * k, 8.0 * k).unwrap_or(d.center.z - stand);
        let z = ground + stand;
        d.center.z += (z - d.center.z) * (1.0 - (-12.0 * dt).exp());
        // Its legs going as it walks
        if speed > 0.05 {
            let phase = d.stride * TAU;
            let swing = (0.45 * (speed / speeds(h.look).1).sqrt().min(1.0)).max(0.2);
            for (leg, offset) in [(0, 0.0), (1, PI), (2, PI), (3, 0.0)] {
                want.legs[leg].swing += swing * (phase + offset).sin();
                want.legs[leg].knee += 0.5 * swing * (-(phase + offset).cos()).max(0.0);
            }
            want.tail[0][0] += 0.12 * (phase).sin();
            want.tail[1][0] += 0.12 * (phase - 0.6).sin();
        }
    }
}

/// Its joints eased towards where it wants them, its wings beating, and every piece of it put
/// where they all are now
fn animate(d: &mut Dragon, h: &Here, want: &Rig, wings: &WingSet, dead: bool) {
    let dt = h.dt.max(if dead { 1.0 / 120.0 } else { 0.0 });
    let k = h.sp.s.sqrt();
    // Its tail swaying, and its breath coming and going standing
    let mut want = *want;
    let sway = (d.clock * 1.3 / k).sin();
    for j in 0..h.sp.tails {
        want.tail[j][0] += 0.08 * sway * (j as f64 + 1.0) / h.sp.tails as f64;
    }
    want.neck[0][1] += 0.03 * (d.clock * 1.7 / k).sin();
    d.rig.ease(&want, 7.0 / k.powf(0.5), dt);
    // Its wings ease from one way of going to another, and beat on as they go
    let ease = 1.0 - (-4.5 / k.powf(0.3) * dt).exp();
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
    let phase = d.beat * TAU;
    let wing = |base: WingPose, amp: f64| WingPose {
        flap: base.flap + amp * phase.sin(),
        sweep: base.sweep + 0.12 * amp * phase.cos(),
        twist: base.twist + 0.18 * amp * phase.cos(),
        fold: base.fold,
        bend: base.bend + 0.45 * amp * (phase - 0.9).sin(),
    };
    d.rig.wings = [wing(d.wings.base, d.wings.amp), wing(d.wings.base, d.wings.amp)];
    // Rising and falling with each beat of its wings, flying
    let bob = if d.airborne { -0.05 * h.sp.s * d.wings.amp * phase.sin() * h.vpm } else { 0.0 };
    let center = d.center + Vector3::new(0.0, 0.0, bob);
    let (mouth, breath) = pose(&h.sp, center, d.yaw, &d.rig, h.vpm, &mut d.bones);
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

// Where the great ones keep their lairs

/// The Dragon realm's monsters, for its entry in realms::REALMS: ashwings above all, and the rest
/// now and then. Once it's been come to, they come in the night raids of the others too.
#[allow(dead_code)]
pub const REALM_MONSTERS: &[crate::realms::Pack] = &[
    crate::realms::Pack { looks: &[Look::Ashwing, Look::Ashwing, Look::Ashwing], most: 3 },
    crate::realms::Pack { looks: &[Look::Ashwing, Look::Squallwing], most: 2 },
    crate::realms::Pack { looks: &[Look::Sleetwing, Look::Sleetwing], most: 1 },
    crate::realms::Pack { looks: &[Look::Blightwing, Look::Ashwing], most: 1 },
];

/// Its three raid bosses, and about where their lairs are from where it's come to, in meters east
/// and north, for its entry in realms::REALMS
#[allow(dead_code)]
pub const REALM_RAIDS: [(Look, [f32; 2]); 3] = [(Look::PyreMatriarch, [260.0, 130.0]), (Look::CindermawTyrant, [-240.0, 200.0]), (Look::GildscaleHoardwarden, [-40.0, -300.0])];

/// The kind of lair each great dragon keeps, see worldgen\dragonlair.rs
pub fn lair_kind(look: Look) -> Option<crate::worldgen::LairKind> {
    use crate::worldgen::LairKind;
    match look {
        Look::PyreMatriarch => Some(LairKind::Peak),
        Look::CindermawTyrant => Some(LairKind::Caldera),
        Look::GildscaleHoardwarden => Some(LairKind::Hoard),
        _ => None,
    }
}

/// Whether ground of `material` at `z` voxels up a world `height` voxels high is the high bare rock
/// drakelings roost on
pub fn roost(material: u8, z: f64, height: f64) -> bool {
    matches!(material, STONE | ROCK | GRAVEL | SNOW) && z > height * 0.55
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
        for look in [Look::Ashwing, Look::PyreMatriarch, Look::CindermawTyrant, Look::GildscaleHoardwarden, ULTRA] {
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
        for look in [Look::PyreMatriarch, Look::CindermawTyrant, Look::GildscaleHoardwarden, ULTRA] {
            let (deeds, track, a) = fight(look, 120.0, Some(0.1));
            let stages: Vec<u8> = deeds.iter().filter_map(|(_, d)| if let Deed::Phase { stage, .. } = d { Some(*stage) } else { None }).collect();
            let all = phases(look).len() as u8;
            assert_eq!(stages, (1..=all).collect::<Vec<_>>(), "{look:?} went through phases {stages:?}");
            assert!(a.boss.as_ref().unwrap().enraged, "{look:?} isn't enraged at the last");
            if look == ULTRA {
                // Mostly in the air at first, and on the ground once it's come down
                let at = deeds.iter().find(|(_, d)| matches!(d, Deed::Phase { stage: 1, .. })).unwrap().0;
                let down = track.iter().filter(|(t, ..)| *t > at + 6.0 && *t < at + 18.0).filter(|p| !p.2).count();
                assert!(down > 200, "{look:?} kept to the air after its first phase");
            }
        }
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
}


