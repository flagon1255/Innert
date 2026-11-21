//! Gargoyles: winged fiends of stone, perched as still as the statues they look like on the walls
//! of castles and ruins, before the temples of the fallen cities under the land, and on the high
//! rock of the realms. They wake when someone comes near, the stone cracking off them in flakes and
//! their eyes catching fire, and come down gliding and swooping, clawing and leaping. Hurt, one
//! wraps its wings round itself and turns back to stone, and blows glance off it till it cracks
//! out again. Each land has its own: grey stone in the normal world, bloodstone in the dreaming
//! realm, rimestone in the frozen one, verdigris in the sunken reach and obsidian in the Cinder
//! Crown. The Belfry Grotesque keeps the castles, and the Bloodwake Gargoyle, the greatest of them,
//! is one of the dreaming realm's raid bosses.
//!
//! They're built here, on the winged rig the avians share, and fly and fight as they do, see
//! src\avians.rs: what's their own, being stone, is seen to there too.

use std::f64::consts::{PI, TAU};

use crate::animals::{Look, Part, ball, ellipsoid, limb};
use crate::avians::{self, ArmPose, Arms, Crown, LegPose, Plan, Rig, Spec, WingPose, hide};
use crate::weapons::Element;
use crate::worldgen::{Land, material::*};

/// Every gargoyle, the lesser of each land's stone first, then the grotesque and the bloodwake
pub const GARGOYLES: [Look; 7] = [Look::Gargoyle, Look::BloodGargoyle, Look::RimeGargoyle, Look::CinderGargoyle, Look::MossGargoyle, Look::Grotesque, Look::BloodwakeGargoyle];

pub fn is_gargoyle(look: Look) -> bool {
    GARGOYLES.contains(&look)
}

/// One of the lesser, monsters perched in twos and threes
pub fn is_lesser(look: Look) -> bool {
    is_gargoyle(look) && !matches!(look, Look::Grotesque | Look::BloodwakeGargoyle)
}

/// The lesser gargoyle of `land`'s stone
pub fn of_land(land: Land) -> Look {
    match land {
        Land::Dream => Look::BloodGargoyle,
        Land::Ice => Look::RimeGargoyle,
        Land::Dragon => Look::CinderGargoyle,
        Land::Water => Look::MossGargoyle,
        _ => Look::Gargoyle,
    }
}

/// Seconds a gargoyle takes to wake, from the first crack in its stone to its wings spread
pub const WAKE: f64 = 1.9;

pub fn name(look: Look) -> &'static str {
    match look {
        Look::Gargoyle => "gargoyle",
        Look::BloodGargoyle => "bloodstone gargoyle",
        Look::RimeGargoyle => "rimestone gargoyle",
        Look::CinderGargoyle => "obsidian gargoyle",
        Look::MossGargoyle => "verdigris gargoyle",
        Look::Grotesque => "Belfry Grotesque",
        _ => "Bloodwake Gargoyle",
    }
}

/// What it's said of each, for the lore
pub fn lore(look: Look) -> &'static str {
    match look {
        Look::Gargoyle => "The masons carved them to keep the evil off the walls. Something got into the stone anyway, and now they keep everything off.",
        Look::BloodGargoyle => "Bloodstone that dreamed it had wings. It weeps red down its face as it sleeps, and wakes hungry.",
        Look::RimeGargoyle => "Frost has grown over it so long it is more ice than stone. It cracks like a frozen lake as it wakes.",
        Look::CinderGargoyle => "Glass from the mountain's heart, cooled in the shape of a fiend. Its cracks still glow.",
        Look::MossGargoyle => "A bronze-green thing grown over with weed, that keeps the drowned shrines of the reach. The tide has never moved it.",
        Look::Grotesque => "The great carving over the castle gate, that every gargoyle on the walls answers to. Its shriek wakes them all.",
        _ => "The dream's own gargoyle, a cathedral's worth of bloodstone, that holds the vigil over the red fields and has never once closed its eyes.",
    }
}

pub fn health(look: Look) -> f64 {
    match look {
        Look::Gargoyle => 210.0,
        Look::BloodGargoyle => 210.0,
        Look::RimeGargoyle => 210.0,
        Look::CinderGargoyle => 210.0,
        Look::MossGargoyle => 210.0,
        Look::Grotesque => 3000.0,
        _ => 15500.0,
    }
}

pub fn element(look: Look) -> Option<Element> {
    match look {
        Look::BloodGargoyle | Look::BloodwakeGargoyle => Some(Element::Blood),
        Look::RimeGargoyle => Some(Element::Frost),
        Look::CinderGargoyle => Some(Element::Fire),
        Look::MossGargoyle => Some(Element::Poison),
        _ => None,
    }
}

/// The color it's drawn turning towards as it turns to stone, its own stone's, dulled
pub fn stone_color(look: Look) -> [u8; 3] {
    match look {
        Look::BloodGargoyle | Look::BloodwakeGargoyle => [118, 62, 58],
        Look::RimeGargoyle => [178, 192, 202],
        Look::CinderGargoyle => [58, 54, 56],
        Look::MossGargoyle => [92, 118, 100],
        Look::Grotesque => [142, 136, 126],
        _ => [150, 150, 145],
    }
}

pub(crate) fn spec(look: Look) -> Spec {
    let lesser = |hide, crown| Spec { s: 0.9, plan: Plan::Gargoyle, hide, jaw: false, tail: false, stiff: true, arms: Arms::Stiff, span: 1.0, girth: 1.0, crown };
    let great = |s, hide, crown, span, girth| Spec { s, plan: Plan::Gargoyle, hide, jaw: true, tail: true, stiff: false, arms: Arms::Jointed, span, girth, crown };
    match look {
        // Grey stone, moss in its creases, fire in its eyes
        Look::Gargoyle => lesser(hide(STONE, ROCK, LIMESTONE, MOSS, ROCK, GLOW_FIRE, GLOW_FIRE, ROCK), Crown::Horns),
        // Red bloodstone, black in its joints, blood glowing in its cracks and eyes
        Look::BloodGargoyle => lesser(hide(TEMPLE_RED, OBSIDIAN, CLAY_BANK, OBSIDIAN, BONE, GLOW_BLOOD, GLOW_BLOOD, OBSIDIAN), Crown::Horns),
        // Pale stone furred with rime, ice for horns and claws, frost in its eyes
        Look::RimeGargoyle => lesser(hide(LIMESTONE, STEEL, SNOW, ICE, ICE, GLOW_FROST, GLOW_FROST, STEEL), Crown::Horns),
        // Black glass, its cracks glowing like coals
        Look::CinderGargoyle => lesser(hide(OBSIDIAN, BARK_DARK, MORTAR, BARK_DARK, OBSIDIAN, FLAME, GLOW_FIRE, BARK_DARK), Crown::Horns),
        // Bronze gone green, weed hanging off it, venom in its eyes
        Look::MossGargoyle => lesser(hide(TEMPLE_TEAL, BRONZE, MOSS, MOSS, BRONZE, GLOW_POISON, GLOW_POISON, BRONZE), Crown::Horns),
        // The castle's own brick and mortar, ram's horns, violet light in its eyes
        Look::Grotesque => great(2.2, hide(BRICK_STONE, MORTAR, LIMESTONE, MOSS, LIMESTONE, GLOW_ARCANE, GLOW_ARCANE, MORTAR), Crown::Ram, 1.1, 1.1),
        // Bloodstone the size of a chapel, crowned with thorns of it, bone horns, blood in every crack
        _ => great(3.4, hide(TEMPLE_RED, OBSIDIAN, BONE, OBSIDIAN, BONE, GLOW_BLOOD, GLOW_BLOOD, OBSIDIAN), Crown::Thorned, 1.15, 1.15),
    }
}

type Vec3 = [f64; 3];

/// The shapes of piece `piece` of a gargoyle built as `sp`, in units, see avians::M_TORSO
pub(crate) fn piece(p: &mut Vec<Part>, sp: &Spec, piece: usize) {
    match piece {
        avians::M_TORSO => torso(p, sp),
        avians::M_HEAD => head(p, sp, true),
        avians::M_STONE_HEAD => head(p, sp, false),
        avians::M_JAW if sp.jaw => jaw(p, sp),
        avians::M_TAIL if sp.tail => tail(p, sp, [0.0, 0.0, 0.0]),
        4 => wing_in(p, sp, 1.0),
        5 => wing_out(p, sp, 1.0),
        6 => wing_in(p, sp, -1.0),
        7 => wing_out(p, sp, -1.0),
        avians::M_THIGH => thigh(p, sp),
        avians::M_SHIN => shin(p, sp),
        avians::M_UPPER => upper_arm(p, sp),
        avians::M_FORE if sp.arms == Arms::Jointed => forearm(p, sp, [0.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
        _ => {}
    }
}

fn torso(p: &mut Vec<Part>, sp: &Spec) {
    let (h, g) = (sp.hide, sp.girth);
    let thorned = sp.crown == Crown::Thorned;
    // Spines of stone down its back, a bloodwake's taller
    for k in 0..6 {
        let x = 0.42 - k as f64 * 0.15;
        let tall = if thorned { 0.2 } else { 0.09 };
        p.push(limb([x, 0.0, 0.2 * g], [x - 0.09, 0.0, 0.2 * g + tall * (1.0 - k as f64 * 0.1)], 0.04, 0.008, h.dark));
    }
    // What glows in its cracks, running down its chest and flanks
    for side in [-1.0, 1.0] {
        p.push(limb([0.38, side * 0.2 * g, -0.12], [0.15, side * 0.24 * g, -0.05], 0.018, 0.012, h.glow));
        p.push(limb([0.05, side * 0.2 * g, 0.05], [-0.25, side * 0.17 * g, -0.08], 0.016, 0.012, h.glow));
    }
    // Ribs over its belly, its wings' roots on its back, its shoulders
    for k in 0..4 {
        let x = 0.12 - k as f64 * 0.09;
        p.push(limb([x, -0.17 * g, -0.13], [x, 0.17 * g, -0.13], 0.02, 0.02, h.dark).stretched([1.0, 1.0, 0.8]));
    }
    for side in [-1.0, 1.0] {
        p.push(ball([0.22, side * 0.11 * g, 0.2], 0.075 * g, h.dark));
        p.push(ball([0.36, side * 0.24 * g, 0.02], 0.1 * g, h.main));
        // Moss or weed in the crease of its hip
        p.push(ellipsoid([-0.3, side * 0.17 * g, 0.06], [0.08, 0.04, 0.05], h.skin));
    }
    p.push(limb([0.38, 0.0, 0.07], [0.5, 0.0, 0.12], 0.12 * g, 0.09 * g, h.main));
    p.push(ellipsoid([0.27, 0.0, 0.03], [0.24, 0.26 * g, 0.22 * g], h.main));
    p.push(ellipsoid([-0.04, 0.0, -0.02], [0.24, 0.19 * g, 0.18 * g], h.pale));
    p.push(ellipsoid([-0.33, 0.0, -0.04], [0.16, 0.19 * g, 0.15 * g], h.main));
    if !sp.tail {
        tail(p, sp, [-0.42, 0.0, -0.06]);
    }
}

/// Its head: with its eyes alight, or gone to stone as it sleeps
fn head(p: &mut Vec<Part>, sp: &Spec, lit: bool) {
    let h = sp.hide;
    let eye = if lit { h.eye } else { h.dark };
    for side in [-1.0, 1.0] {
        p.push(ball([0.2, side * 0.065, 0.05], 0.028, eye));
        p.push(limb([0.13, side * 0.08, 0.1], [0.24, side * 0.05, 0.075], 0.035, 0.025, h.dark));
        // Fangs hanging over its lip
        p.push(limb([0.27, side * 0.04, -0.06], [0.28, side * 0.04, -0.11], 0.016, 0.004, h.beak));
        match sp.crown {
            Crown::Ram => {
                // Curling round like a ram's, back and down past its ears
                let curl: [Vec3; 4] = [[0.02, side * 0.1, 0.12], [-0.12, side * 0.2, 0.18], [-0.2, side * 0.25, 0.04], [-0.1, side * 0.23, -0.06]];
                for (j, pair) in curl.windows(2).enumerate() {
                    p.push(limb(pair[0], pair[1], 0.06 - 0.014 * j as f64, 0.046 - 0.014 * j as f64, h.beak));
                }
            }
            Crown::Thorned => {
                p.push(limb([0.03, side * 0.09, 0.12], [-0.16, side * 0.18, 0.3], 0.05, 0.02, h.beak));
                p.push(limb([-0.16, side * 0.18, 0.3], [-0.3, side * 0.14, 0.4], 0.02, 0.006, h.beak));
            }
            _ => {
                p.push(limb([0.05, side * 0.08, 0.12], [-0.08, side * 0.14, 0.26], 0.045, 0.03, h.beak));
                p.push(limb([-0.08, side * 0.14, 0.26], [-0.2, side * 0.12, 0.34], 0.03, 0.008, h.beak));
            }
        }
        // Pointed ears
        p.push(limb([0.02, side * 0.11, 0.06], [-0.1, side * 0.21, 0.13], 0.035, 0.008, h.main));
    }
    if sp.crown == Crown::Thorned {
        // A crown of thorns of its stone
        for k in 0..6 {
            let a = k as f64 / 6.0 * TAU;
            let (x, y) = (0.04 + 0.08 * a.cos(), 0.09 * a.sin());
            p.push(limb([x, y, 0.13], [x * 1.3, y * 1.3, 0.26], 0.025, 0.006, h.dark));
        }
    }
    p.push(limb([0.16, 0.0, 0.0], [0.31, 0.0, -0.035], 0.085, 0.06, h.main));
    if !sp.jaw {
        p.push(limb([0.12, 0.0, -0.08], [0.28, 0.0, -0.09], 0.05, 0.035, h.main));
    }
    p.push(ellipsoid([0.06, 0.0, 0.04], [0.14, 0.12, 0.12], h.main));
}

fn jaw(p: &mut Vec<Part>, sp: &Spec) {
    let h = sp.hide;
    for side in [-1.0, 1.0] {
        p.push(limb([0.14, side * 0.035, 0.0], [0.145, side * 0.035, 0.04], 0.013, 0.004, h.beak));
    }
    // Its maw glowing with what's in it
    p.push(ellipsoid([0.06, 0.0, 0.01], [0.06, 0.035, 0.015], h.glow));
    p.push(limb([0.0, 0.0, 0.0], [0.17, 0.0, -0.02], 0.05, 0.035, h.main));
}

/// Its tail, curling down and up again to a spade of stone, from `root` back along -x
fn tail(p: &mut Vec<Part>, sp: &Spec, root: Vec3) {
    let h = sp.hide;
    let at = |x: f64, z: f64| [root[0] + x, root[1], root[2] + z];
    let line = [at(0.0, 0.0), at(-0.3, -0.06), at(-0.56, -0.04), at(-0.78, 0.05)];
    // The spade, its edges barbed
    for side in [-1.0, 1.0] {
        p.push(limb(at(-0.76, 0.05), [root[0] - 0.86, root[1] + side * 0.08, root[2] + 0.04], 0.018, 0.004, h.dark));
    }
    p.push(limb(at(-0.8, 0.06), at(-0.94, 0.06), 0.02, 0.004, h.dark));
    p.push(ellipsoid(at(-0.84, 0.06), [0.09, 0.07, 0.02], h.dark));
    for (j, pair) in line.windows(2).enumerate() {
        let r = 0.065 - 0.016 * j as f64;
        p.push(limb(pair[0], pair[1], r, r - 0.016, h.main));
    }
}

/// A strip of a wing's membrane from `a` on its leading edge to `b` on its trailing, `width` wide
fn skin(a: Vec3, b: Vec3, width: f64, material: u8) -> Part {
    let r = 0.025;
    limb(a, b, r, r, material).stretched([1.0, (width * 0.62 / r).max(1.0), 1.0])
}

/// The inner piece of a bat's wing of stone, from the shoulder out to the wrist: the bone of its
/// arm, the hooked claw at the wrist, and the membrane back from it
fn wing_in(p: &mut Vec<Part>, sp: &Spec, side: f64) {
    let h = sp.hide;
    let w = avians::wrist(sp);
    let w = [w.x, side * w.y, w.z];
    p.push(limb(w, [w[0] + 0.12, w[1] + side * 0.02, w[2] + 0.05], 0.03, 0.006, h.beak));
    p.push(ball(w, 0.05, h.dark));
    p.push(limb([0.0, 0.0, 0.0], w, 0.06, 0.042, h.main));
    let reach = w[1].abs();
    let strips = 8;
    for i in 0..strips {
        let f = (i as f64 + 0.5) / strips as f64;
        let y = side * (0.03 + (reach - 0.03) * f);
        let lead = [0.06 * f, y, 0.02 * f];
        let trail = [-0.6 + 0.25 * f - 0.06 * (PI * f).sin(), y, 0.0];
        p.push(skin(lead, trail, (reach - 0.03) / strips as f64, h.extra));
    }
}

/// Where the trailing edge of a wing's outer piece is, `y` out along it from the wrist, scalloped
/// in between the fingers
fn trailing(y: f64, l: f64) -> f64 {
    let points = [(0.0, -0.5), (0.2, -0.42), (0.42, -0.66), (0.6, -0.48), (0.78, -0.56), (0.9, -0.36), (1.0, -0.22)];
    let f = (y / l).clamp(0.0, 1.0);
    for pair in points.windows(2) {
        let ((f0, x0), (f1, x1)) = (pair[0], pair[1]);
        if f <= f1 {
            return x0 + (x1 - x0) * (f - f0) / (f1 - f0);
        }
    }
    -0.22
}

/// The outer piece, from the wrist to the tip: the long bones of its fingers and the membrane
/// between them
fn wing_out(p: &mut Vec<Part>, sp: &Spec, side: f64) {
    let h = sp.hide;
    let l = avians::wing_lengths(sp).1;
    let tip = [-0.22, side * l, 0.02];
    let mid = [tip[0] * 0.5, tip[1] * 0.5, tip[2] * 0.5];
    p.push(limb([0.0, 0.0, 0.0], mid, 0.042, 0.03, h.main));
    p.push(limb(mid, tip, 0.03, 0.008, h.main));
    for (f, x) in [(0.42, -0.66), (0.78, -0.56)] {
        p.push(limb([0.0, 0.0, 0.0], [x, side * f * l, 0.0], 0.025, 0.008, h.main));
    }
    let strips = 11;
    for i in 0..strips {
        let y = -0.03 + (l + 0.03) * (i as f64 + 0.5) / strips as f64;
        let lead = [-0.22 * (y / l).max(0.0), side * y, 0.02 * (y / l).max(0.0)];
        let trail = [trailing(y, l), side * y, 0.0];
        if lead[0] - trail[0] > 0.03 {
            p.push(skin(lead, trail, (l + 0.03) / strips as f64, h.extra));
        }
    }
}

/// A foot of three clawed toes, flat on the ground, at `ankle` with forward along `ahead`
fn foot(p: &mut Vec<Part>, sp: &Spec, ankle: Vec3, ahead: [f64; 2]) {
    let h = sp.hide;
    let [c, s] = ahead;
    let at = |x: f64, y: f64, z: f64| [ankle[0] + x * c + z * s, y, ankle[2] - x * s + z * c];
    for y in [-0.06, 0.0, 0.06] {
        p.push(limb(at(0.11, y * 1.4, -0.01), at(0.17, y * 1.6, -0.06), 0.022, 0.005, h.beak));
        p.push(limb(at(0.0, y * 0.5, 0.0), at(0.12, y * 1.4, -0.02), 0.035, 0.03, h.main));
    }
    p.push(ball(at(0.02, 0.0, 0.0), 0.05, h.dark));
}

/// A thigh, thick with stone, and for a lesser one's stiff legs the rest of the leg too, crouched
/// as it stands
fn thigh(p: &mut Vec<Part>, sp: &Spec) {
    let (h, g) = (sp.hide, sp.girth);
    let (lt, ls) = avians::leg_lengths(sp);
    if sp.stiff {
        let leg = avians::stand_leg(sp);
        let knee = [0.0, 0.0, -lt];
        let ankle = [-ls * leg.knee.sin(), 0.0, -lt - ls * leg.knee.cos()];
        let a = avians::stand_pitch(sp) + leg.swing;
        foot(p, sp, ankle, [a.cos(), a.sin()]);
        p.push(ball(knee, 0.075 * g, h.dark));
        p.push(limb(knee, ankle, 0.06 * g, 0.045 * g, h.main));
    }
    p.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, -lt], 0.1 * g, 0.075 * g, h.main));
}

fn shin(p: &mut Vec<Part>, sp: &Spec) {
    if sp.stiff {
        return;
    }
    let (h, g) = (sp.hide, sp.girth);
    let (_, ls) = avians::leg_lengths(sp);
    let leg = avians::stand_leg(sp);
    let a = avians::stand_pitch(sp) + leg.swing - leg.knee;
    foot(p, sp, [0.0, 0.0, -ls], [a.cos(), a.sin()]);
    p.push(ball([0.0, 0.0, 0.0], 0.075 * g, h.dark));
    p.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, -ls], 0.065 * g, 0.048 * g, h.main));
}

/// A forearm from `elbow` along `along`, ending in a hand of four long claws curving forward
fn forearm(p: &mut Vec<Part>, sp: &Spec, elbow: Vec3, along: Vec3) {
    let (h, g) = (sp.hide, sp.girth);
    let (_, lf) = avians::arm_lengths(sp);
    let hand = [elbow[0] + along[0] * lf, elbow[1], elbow[2] + along[2] * lf];
    // Forward of the hand is the forearm's own forward, turned with it
    let fwd = [-along[2], along[0]];
    for y in [-0.06, -0.02, 0.02, 0.06] {
        let knuckle = [hand[0] + fwd[0] * 0.04 + along[0] * 0.05, y * 1.2, hand[2] + fwd[1] * 0.04 + along[2] * 0.05];
        let tip = [hand[0] + fwd[0] * 0.13 + along[0] * 0.15, y * 1.5, hand[2] + fwd[1] * 0.13 + along[2] * 0.15];
        p.push(limb(knuckle, tip, 0.018, 0.004, h.beak));
    }
    p.push(ball(hand, 0.06 * g, h.dark));
    p.push(limb(elbow, hand, 0.055 * g, 0.045 * g, h.main));
}

/// An upper arm, and for a lesser one's stiff arms its forearm too, the elbow bent as it stands
fn upper_arm(p: &mut Vec<Part>, sp: &Spec) {
    let (h, g) = (sp.hide, sp.girth);
    let (la, _) = avians::arm_lengths(sp);
    if sp.arms == Arms::Stiff {
        let e = avians::stand_arm(sp).elbow;
        forearm(p, sp, [0.0, 0.0, -la], [e.sin(), 0.0, -e.cos()]);
    }
    p.push(ball([0.0, 0.0, -la], 0.065 * g, h.dark));
    p.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, -la], 0.07 * g, 0.056 * g, h.main));
}

/// Asleep on its perch, a statue: crouched low, hands on its knees, its wings mantled up over its
/// back, its head bowed to look down over the drop, its tail curled round its feet
pub(crate) fn statue(sp: &Spec) -> Rig {
    let p = 0.55;
    let leg = if sp.stiff { avians::stand_leg(sp) } else { LegPose { swing: -p + 1.2, knee: 2.2 } };
    Rig {
        pitch: p,
        roll: 0.0,
        head: [0.0, -p - 0.35],
        jaw: 0.15,
        tail: [0.55, -0.75],
        wings: [WingPose { flap: 0.85, sweep: 0.85, twist: -0.55, fold: 2.0, bend: 0.3 }; 2],
        legs: [leg; 2],
        arms: [ArmPose { swing: -p + 1.0, spread: 0.06, elbow: 0.9 }; 2],
    }
}

/// Turned to stone in a fight: hunched lower, its arms crossed and its wings wrapped round in
/// front of it like a cloak, its head tucked down behind them
pub(crate) fn wrapped(sp: &Spec) -> Rig {
    let p = 0.45;
    let leg = if sp.stiff { avians::stand_leg(sp) } else { LegPose { swing: -p + 1.25, knee: 2.3 } };
    Rig {
        pitch: p,
        roll: 0.0,
        head: [0.0, -p - 0.65],
        jaw: 0.0,
        tail: [0.8, -0.6],
        wings: [WingPose { flap: -0.15, sweep: -0.85, twist: -1.25, fold: 0.7, bend: 0.0 }; 2],
        legs: [leg; 2],
        arms: [ArmPose { swing: -p + 1.3, spread: -0.25, elbow: 1.6 }; 2],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each land has its own gargoyle, of its own stone, and the lesser ones are monsters
    #[test]
    fn each_land_has_its_own_stone() {
        let lands = [Land::Normal, Land::Dream, Land::Ice, Land::Dragon, Land::Water];
        for (i, &a) in lands.iter().enumerate() {
            let look = of_land(a);
            assert!(is_lesser(look) && look.is_monster(), "{look:?}");
            assert!(lands[i + 1..].iter().all(|&b| of_land(b) != look));
            assert!(lore(look).len() > 40);
        }
        assert!(!is_lesser(Look::Grotesque) && Look::Grotesque.is_boss() && !Look::Grotesque.is_raid());
        assert!(Look::BloodwakeGargoyle.is_raid());
        // Their stone colors told apart
        for (i, a) in GARGOYLES.iter().enumerate() {
            for b in &GARGOYLES[i + 1..] {
                assert!(stone_color(*a) != stone_color(*b) || matches!((a, b), (Look::BloodGargoyle, Look::BloodwakeGargoyle)), "{a:?} {b:?}");
            }
        }
    }
}
