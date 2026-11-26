//! Life in the water: fish of five kinds swimming in schools, and sharks hunting the open sea.
//!
//! Fish keep together as a school does, each steering by the few of its own school about it:
//! away from any too close, along the way the rest are going, and towards the middle of them.
//! They keep under the top of the water and off its bottom, turn back where it gets too shallow
//! for them, scatter from the player and from anything splashing in, and now and then one leaps
//! clear of the water and splashes back. Any blow kills one, and it's caught: raw fish, to eat as
//! it is or cook over a fire.
//!
//! A shark cruises the deep water away from the shore. Once the player's in water over their
//! chest it comes round them, its dorsal fin cutting the top of the water, closing in; then it
//! charges, its jaws gaping wider the nearer it gets, snaps them shut on the player and breaks
//! off, thrashing round, to circle again. It never leaves the water, and turns back wherever it
//! gets too shallow for it. Its level is any monster's, see levels::foe_level().
//!
//! Each kind shares one model, a frame for each step of its tail's sweep, see parts(), so a
//! school costs no more to draw than as many squirrels; and only those near the player are about
//! at all, see game\sea.rs.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use nalgebra::{Vector2, Vector3};

use super::{Animal, Look, Mood, Part, ball, ellipsoid, hash, limb, random};
use crate::bosses::{Deed, Move};
use crate::player::WaterLevel;
use crate::worldgen::{Land, World, is_passable, material::*};

type V3 = Vector3<f64>;

/// How much finer a fish's model is than everything else's, see Look::resolution(): a fish is only
/// a hand long
pub const FINE: f64 = 2.0;

/// The fish, and the sharks
pub const FISH: [Look; 5] = [Look::Silverfin, Look::Reefling, Look::Brookback, Look::Gloamfin, Look::Frostfin];
pub const SHARKS: [Look; 2] = [Look::Saltjaw, Look::Ghostmaw];

pub fn is_fish(look: Look) -> bool {
    FISH.contains(&look)
}

pub fn is_shark(look: Look) -> bool {
    SHARKS.contains(&look)
}

pub fn is_sea(look: Look) -> bool {
    is_fish(look) || is_shark(look)
}

/// Drawn finer than the world, see FINE: the fish, and what's left of them lying about
pub fn is_small(look: Look) -> bool {
    is_fish(look) || matches!(look, Look::RawFish | Look::CookedFish | Look::SharkTooth)
}

/// What sets one kind of fish apart, in meters and meters a second
pub struct Fish {
    pub look: Look,
    pub name: &'static str,
    /// From its snout to the fork of its tail, how deep its body is and how broad
    pub length: f64,
    pub depth: f64,
    pub width: f64,
    /// Its back, its belly, its fins, and something of its own: a stripe along its side, spots
    pub back: u8,
    pub belly: u8,
    pub fin: u8,
    pub accent: u8,
    /// Bands across its body, AIR for none
    pub bands: u8,
    /// How fast it goes about, and darts off
    pub cruise: f64,
    pub dart: f64,
    /// How many swim together, at the least and the most
    pub school: (usize, usize),
    /// How deep the water has to be for it
    pub least: f64,
    /// Sweeps of its tail a second, cruising
    pub beat: f64,
    /// Whether it ever leaps out of the water
    pub leaps: bool,
}

pub const FISHES: [Fish; 5] = [
    // A slim fish of the open sea, blue-grey over silver, with a pale line along its side and a dark
    // spot by its tail, in big shoals
    Fish {
        look: Look::Silverfin,
        name: "silverfin",
        length: 0.3,
        depth: 0.075,
        width: 0.045,
        back: CLOTH_BLUE,
        belly: STEEL,
        fin: FUR_GRAY,
        accent: FUR_WHITE,
        bands: AIR,
        cruise: 0.9,
        dart: 3.4,
        school: (7, 11),
        least: 1.2,
        beat: 2.6,
        leaps: true,
    },
    // A deep little disc of a fish of the warm shallows, yellow banded with blue, its fins red
    Fish {
        look: Look::Reefling,
        name: "reefling",
        length: 0.22,
        depth: 0.14,
        width: 0.045,
        back: FLOWER_YELLOW,
        belly: FLOWER_YELLOW,
        fin: FUR_RED,
        accent: FLOWER_WHITE,
        bands: CLOTH_BLUE,
        cruise: 0.55,
        dart: 2.6,
        school: (4, 7),
        least: 0.5,
        beat: 3.2,
        leaps: false,
    },
    // A long olive fish of the rivers and lakes, speckled dark on its back, a pink stripe down its
    // side and a cream belly
    Fish {
        look: Look::Brookback,
        name: "brookback",
        length: 0.4,
        depth: 0.09,
        width: 0.06,
        back: MOSS,
        belly: FUR_CREAM,
        fin: FUR_TAN,
        accent: FLOWER_PINK,
        bands: AIR,
        cruise: 0.7,
        dart: 3.6,
        school: (3, 5),
        least: 0.6,
        beat: 2.2,
        leaps: true,
    },
    // A pale fish of the dim water under the rock, blind, with long trailing fins, feelers at its
    // snout and a row of faintly glowing spots down either side
    Fish {
        look: Look::Gloamfin,
        name: "gloamfin",
        length: 0.26,
        depth: 0.065,
        width: 0.045,
        back: BONE,
        belly: FUR_WHITE,
        fin: FUR_CREAM,
        accent: GLOW_FROST,
        bands: AIR,
        cruise: 0.4,
        dart: 2.2,
        school: (3, 6),
        least: 0.5,
        beat: 1.8,
        leaps: false,
    },
    // A fish of the cold water, deep blue over white, its fins and the crest down its back clear
    // as ice
    Fish {
        look: Look::Frostfin,
        name: "frostfin",
        length: 0.32,
        depth: 0.08,
        width: 0.05,
        back: TEMPLE_TEAL,
        belly: FUR_WHITE,
        fin: ICE,
        accent: ICE,
        bands: AIR,
        cruise: 0.75,
        dart: 3.2,
        school: (5, 8),
        least: 0.8,
        beat: 2.4,
        leaps: true,
    },
];

/// What sets one kind of shark apart, in meters and meters a second
pub struct Shark {
    pub look: Look,
    pub name: &'static str,
    pub length: f64,
    pub depth: f64,
    pub width: f64,
    /// How tall its dorsal fin stands
    pub fin: f64,
    pub back: u8,
    pub belly: u8,
    /// The tips of its fins, and its eyes
    pub tips: u8,
    pub eye: u8,
    pub cruise: f64,
    /// Going round the player, and charging
    pub circle: f64,
    pub dart: f64,
    /// How deep the water has to be for it
    pub least: f64,
    /// How far round the player it starts going, closing in to the second
    pub ring: (f64, f64),
    pub health: f64,
    /// Its bite, before its level's damage, see levels::damage()
    pub bite: f64,
}

pub const SHARK_KINDS: [Shark; 2] = [
    // The shark of the open sea: slate grey over white, its fins tipped black
    Shark {
        look: Look::Saltjaw,
        name: "saltjaw",
        length: 2.5,
        depth: 0.5,
        width: 0.46,
        fin: 0.4,
        back: SLATE,
        belly: FUR_WHITE,
        tips: OBSIDIAN,
        eye: OBSIDIAN,
        cruise: 1.3,
        circle: 2.4,
        dart: 6.5,
        least: 1.5,
        ring: (9.0, 5.0),
        health: 210.0,
        bite: 15.0,
    },
    // A great pale shark come up out of the deep by night, scarred, its eyes glowing cold
    Shark {
        look: Look::Ghostmaw,
        name: "ghostmaw",
        length: 3.7,
        depth: 0.78,
        width: 0.72,
        fin: 0.6,
        back: LIMESTONE,
        belly: FUR_WHITE,
        tips: FUR_GRAY,
        eye: GLOW_FROST,
        cruise: 1.1,
        circle: 2.2,
        dart: 7.2,
        least: 2.2,
        ring: (12.0, 6.5),
        health: 245.0,
        bite: 21.0,
    },
];

pub fn fish_of(look: Look) -> Option<&'static Fish> {
    FISHES.iter().find(|f| f.look == look)
}

pub fn shark_of(look: Look) -> Option<&'static Shark> {
    SHARK_KINDS.iter().find(|s| s.look == look)
}

/// How far a fish's fins hang under its body, from how deep the body is
const UNDER: f64 = 0.22;
/// How far a shark's pectoral fins hang under its body, in meters
const SHARK_UNDER: f64 = 0.16;

/// How high the middle of its body is over the bottom of its model, in meters
pub fn middle(look: Look) -> f64 {
    if let Some(f) = fish_of(look) {
        return f.depth * (0.5 + UNDER);
    }
    shark_of(look).map_or(0.1, |s| SHARK_UNDER + s.depth * 0.5)
}

pub fn reach(look: Look) -> f64 {
    match look {
        Look::RawFish | Look::CookedFish => 0.24,
        Look::SharkTooth => 0.1,
        _ => {
            if let Some(f) = fish_of(look) {
                return (f.length * 0.56).hypot(f.depth * 1.6);
            }
            shark_of(look).map_or(1.0, |s| (s.length * 0.53).hypot(SHARK_UNDER + s.depth + s.fin + 0.1))
        }
    }
}

/// The body to aim at, see Look::body(): a little bigger than a fish is, so it can be caught
pub fn body(look: Look) -> (f64, f64) {
    match look {
        Look::RawFish | Look::CookedFish => (0.04, 0.2),
        Look::SharkTooth => (0.02, 0.12),
        _ => match shark_of(look) {
            Some(s) => (middle(look), s.depth * 1.15),
            None => (middle(look), fish_of(look).map_or(0.15, |f| (f.length * 0.5).max(0.15))),
        },
    }
}

pub fn name(look: Look) -> &'static str {
    match look {
        Look::RawFish => "raw fish",
        Look::CookedFish => "cooked fish",
        Look::SharkTooth => "shark tooth",
        _ => fish_of(look).map(|f| f.name).or(shark_of(look).map(|s| s.name)).unwrap_or("fish"),
    }
}

pub fn health(look: Look) -> f64 {
    shark_of(look).map_or(1.0, |s| s.health)
}

/// How fast it cruises and charges
pub fn speeds(look: Look) -> (f64, f64) {
    shark_of(look).map(|s| (s.cruise, s.dart)).or(fish_of(look).map(|f| (f.cruise, f.dart))).unwrap_or((1.0, 3.0))
}

// ---------------------------------------------------------------------------------------------
// Their models

/// How deep a fish's body is `s` of the way from its snout to its tail, 0 to 1, as much of its
/// deepest: a rounded head, deepest a third of the way back, narrowing to the root of its tail
fn profile(s: f64) -> f64 {
    let ease = |t: f64| {
        let t = t.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    if s < 0.32 { 0.5 + 0.5 * ease(s / 0.32).sqrt() } else { 1.0 - 0.78 * ease((s - 0.32) / 0.5) }
}

/// How far to the side its body's swept `s` of the way back, as its tail beats: nothing at its
/// snout, most at its tail, a wave running back along it, `along` of the way through a beat
fn sweep(s: f64, along: f64, amplitude: f64) -> f64 {
    amplitude * s * s * (along * TAU - 2.7 * s).sin()
}

/// The shapes of `look` in a frame: `phase` of the way through a beat of its tail, None lying
/// still, and how far into each pose it is, getting ready to bite, biting and thrashing round
pub fn parts(p: &mut Vec<Part>, look: Look, phase: Option<f64>, pose: [f64; 3]) {
    match look {
        Look::RawFish => lying(p, &FISHES[0], false),
        Look::CookedFish => lying(p, &FISHES[2], true),
        Look::SharkTooth => tooth(p),
        _ => {
            if let Some(f) = fish_of(look) {
                fish(p, f, phase);
            } else if let Some(s) = shark_of(look) {
                shark(p, s, phase, pose);
            }
        }
    }
}

/// A fish swimming, its tail sweeping as `phase` has it
fn fish(p: &mut Vec<Part>, f: &Fish, phase: Option<f64>) {
    let (l, d, w) = (f.length, f.depth, f.width);
    let zc = d * (0.5 + UNDER);
    let along = phase.unwrap_or(0.0);
    let amplitude = if phase.is_some() { 0.16 * l } else { 0.0 };
    let x = |s: f64| l * (0.5 - s);
    let y = |s: f64| sweep(s, along, amplitude);
    let at = |s: f64, dz: f64| [x(s), y(s), zc + dz];
    let disc = f.look == Look::Reefling;
    // A reefling's body is deep nearly all along it
    let deep = |s: f64| if disc { profile(s * 0.8).max(if s < 0.7 { 0.85 } else { 0.0 }) } else { profile(s) };
    // Its eyes, but for the blind gloamfin, whose feelers trail from its snout instead
    if f.look == Look::Gloamfin {
        for side in [-1.0, 1.0] {
            p.push(limb(at(0.02, -0.15 * d), [x(0.0) + 0.04, y(0.0) + side * 0.03, zc - 0.3 * d], 0.004, 0.003, f.fin));
        }
    } else {
        for side in [-1.0, 1.0] {
            p.push(ball([x(0.12), y(0.12) + side * w * 0.42, zc + 0.12 * d], (0.13 * d).max(0.012), EYE));
        }
    }
    // What's its own: a line along its side, spots, a row of glowing dots or a crest of ice
    match f.look {
        Look::Silverfin => {
            for side in [-1.0, 1.0] {
                p.push(limb([x(0.25), y(0.25) + side * w * 0.47, zc], [x(0.72), y(0.72) + side * w * 0.3, zc], 0.006, 0.005, f.accent));
            }
            for side in [-1.0, 1.0] {
                p.push(ball([x(0.76), y(0.76) + side * w * 0.28, zc + 0.05 * d], 0.012, OBSIDIAN));
            }
        }
        Look::Brookback => {
            for side in [-1.0, 1.0] {
                p.push(limb([x(0.2), y(0.2) + side * w * 0.48, zc - 0.05 * d], [x(0.74), y(0.74) + side * w * 0.3, zc - 0.03 * d], 0.008, 0.006, f.accent));
                for k in 0..5 {
                    let s = 0.22 + 0.11 * k as f64;
                    p.push(ball([x(s), y(s) + side * w * 0.4 * deep(s), zc + d * 0.3 * deep(s)], 0.008, FUR_DARK));
                }
            }
        }
        Look::Gloamfin => {
            for side in [-1.0, 1.0] {
                for k in 0..4 {
                    let s = 0.22 + 0.13 * k as f64;
                    p.push(ball([x(s), y(s) + side * w * 0.47 * deep(s), zc], 0.008, f.accent));
                }
            }
        }
        _ => {}
    }
    // Its belly, the lower part of its body
    for k in 0..7 {
        let s = 0.06 + 0.11 * k as f64;
        let h = d * deep(s);
        p.push(ellipsoid([x(s), y(s), zc - 0.22 * h], [l * 0.075, w * 0.47 * deep(s).sqrt(), h * 0.3], f.belly).striped(f.bands));
    }
    // Its body, segment by segment down it, swept to the side the further back
    for k in 0..8 {
        let s = 0.04 + 0.1 * k as f64;
        let h = d * deep(s);
        p.push(ellipsoid(at(s, 0.0), [l * 0.075, w * 0.5 * deep(s).sqrt(), h * 0.5], f.back).striped(f.bands));
    }
    // Its tail, forked, swept furthest
    let root = at(0.82, 0.0);
    let spread = if disc { 0.42 } else { 0.55 };
    for up in [-1.0, 1.0] {
        let tip = [x(1.02), y(1.0) * 1.15, zc + up * d * spread];
        p.push(limb(root, tip, 0.012, 0.006, f.fin));
        p.push(limb(at(0.86, 0.0), [x(0.97), y(0.97), zc + up * d * spread * 0.5], 0.01, 0.006, f.fin));
    }
    // The fin along its back: a reefling's a crest nearly the length of it, a frostfin's spines
    // of ice
    let top = |s: f64| zc + 0.5 * d * deep(s);
    match f.look {
        Look::Reefling => {
            for k in 0..5 {
                let s = 0.25 + 0.1 * k as f64;
                p.push(limb([x(s), y(s), top(s) - 0.005], [x(s + 0.08), y(s + 0.08), top(s) + d * 0.32], 0.006, 0.004, f.fin));
                p.push(limb([x(s), y(s), zc - 0.5 * d * deep(s) + 0.005], [x(s + 0.1), y(s + 0.1), zc - 0.5 * d * deep(s) - d * 0.22], 0.006, 0.004, f.fin));
            }
        }
        Look::Frostfin => {
            for k in 0..4 {
                let s = 0.28 + 0.09 * k as f64;
                p.push(limb([x(s), y(s), top(s) - 0.005], [x(s + 0.05), y(s + 0.05), top(s) + d * (0.55 - 0.08 * k as f64)], 0.007, 0.003, f.fin));
            }
        }
        _ => {
            let long = if f.look == Look::Gloamfin { 1.6 } else { 1.0 };
            p.push(limb([x(0.36), y(0.36), top(0.36) - 0.005], [x(0.36 + 0.2 * long), y(0.5), top(0.36) + d * 0.32 * long], 0.008, 0.005, f.fin));
            p.push(limb([x(0.42), y(0.42), top(0.42) - 0.005], [x(0.36 + 0.2 * long), y(0.5), top(0.36) + d * 0.32 * long], 0.006, 0.005, f.fin));
        }
    }
    // The fins under it, and either side behind its head
    for (s, long) in [(0.42, 0.22), (0.62, 0.26)] {
        p.push(limb([x(s), y(s), zc - 0.4 * d * deep(s)], [x(s + 0.1), y(s + 0.1), zc - 0.5 * d * deep(s) - d * long], 0.006, 0.004, f.fin));
    }
    for side in [-1.0, 1.0] {
        p.push(limb([x(0.24), y(0.24) + side * w * 0.4, zc - 0.15 * d], [x(0.36), y(0.36) + side * w * 1.1, zc - 0.32 * d], 0.006, 0.004, f.fin));
    }
}

/// A fish lying on its side on the ground: raw and silver, or `cooked` brown, charred in bands
fn lying(p: &mut Vec<Part>, f: &Fish, cooked: bool) {
    let mut fish_parts = Vec::new();
    fish(&mut fish_parts, f, None);
    let zc = f.depth * (0.5 + UNDER);
    let (body, char) = if cooked { (FUR_GINGER, BARK_DARK) } else { (STEEL, CLOTH_BLUE) };
    for part in fish_parts {
        let material = match part.material {
            EYE if cooked => BONE,
            EYE => EYE,
            m if cooked && m == f.accent => char,
            m if m == f.back => {
                if cooked {
                    char
                } else {
                    CLOTH_BLUE
                }
            }
            m if m == f.fin => {
                if cooked {
                    BARK_DARK
                } else {
                    FUR_GRAY
                }
            }
            _ => body,
        };
        // Turned over onto its side: up is to the side, and its middle down on the ground
        let roll = |v: [f64; 3]| [v[0], v[2] - zc, f.width * 0.5 + v[1]];
        let s = part.stretch;
        let laid = Part { a: roll(part.a), b: roll(part.b), stretch: [s[0], s[2], s[1]], material, stripes: if cooked && material == body { char } else { AIR }, ..part };
        p.push(laid);
    }
}

/// A shark's tooth: a broad pale fang, lying on its side
fn tooth(p: &mut Vec<Part>) {
    p.push(limb([-0.03, 0.0, 0.02], [0.06, 0.0, 0.02], 0.025, 0.004, BONE));
    p.push(ellipsoid([-0.035, 0.0, 0.02], [0.012, 0.03, 0.018], FUR_CREAM));
}

/// A shark, its tail sweeping as `phase` has it, and `pose` how far it's into gaping to bite,
/// snapping its jaws shut, and thrashing round to break off
fn shark(p: &mut Vec<Part>, k: &Shark, phase: Option<f64>, pose: [f64; 3]) {
    let (l, d, w) = (k.length, k.depth, k.width);
    let [gape, snap, thrash] = pose;
    let zc = SHARK_UNDER + d * 0.5;
    let along = phase.unwrap_or(0.0);
    // Its tail sweeping less as it lunges in to bite, straight as it snaps its jaws
    let amplitude = (if phase.is_some() { 0.11 * l } else { 0.0 }) * (1.0 - snap);
    // Bent round into a curve as it thrashes round to break off
    let bend = |s: f64| thrash * 0.42 * l * (s - 0.45) * (s - 0.45) - thrash * 0.05 * l;
    // Its head thrust on as it snaps its jaws shut
    let thrust = 0.12 * l * snap;
    let x = |s: f64| l * (0.5 - s) + thrust * (1.0 - s).max(0.0);
    let y = |s: f64| sweep(s, along, amplitude) + bend(s);
    let at = |s: f64, side: f64, dz: f64| [x(s), y(s) + side, zc + dz];
    let ghost = k.look == Look::Ghostmaw;
    // How thick it is `s` of the way back: a pointed snout, deepest a third of the way back
    let girth = |s: f64| if s < 0.3 { 0.35 + 0.65 * (s / 0.3).sqrt() } else { 1.0 - 0.8 * ((s - 0.3) / 0.52).clamp(0.0, 1.0).powf(1.3) };
    // Its eyes, glowing cold for the ghostmaw
    for side in [-1.0, 1.0] {
        p.push(ball(at(0.115, side * w * 0.3, 0.1 * d), 0.035 * l / 2.5, k.eye));
    }
    // Its gill slits, dark down either side behind its head
    for side in [-1.0, 1.0] {
        for g in 0..4 {
            let s = 0.25 + 0.025 * g as f64;
            p.push(limb(at(s, side * w * 0.47 * girth(s).sqrt(), 0.18 * d), at(s + 0.01, side * w * 0.47 * girth(s).sqrt(), -0.22 * d), 0.02, 0.02, if ghost { FUR_GRAY } else { OBSIDIAN }));
        }
    }
    // Old scars raked down the ghostmaw's flanks
    if ghost {
        for side in [-1.0, 1.0] {
            for (s, dz) in [(0.38, 0.22), (0.44, 0.3), (0.5, 0.18)] {
                p.push(limb(at(s, side * w * 0.49, dz * d), at(s + 0.07, side * w * 0.47, (dz - 0.3) * d), 0.018, 0.015, FUR_GRAY));
            }
        }
    }
    // Its jaws: the snout over them, the lower jaw hinged under its eyes and dropping open, rows of
    // teeth along both, and its maw red inside
    let open = (gape.max(0.1 * f64::from(phase.is_some())) * (1.0 - snap)).clamp(0.0, 1.0);
    let lift = 0.12 * d * open;
    let hinge = [x(0.24), y(0.24), zc - 0.12 * d];
    let drop = 0.95 * open;
    let jaw_length = 0.2 * l;
    let jaw = |f: f64| [hinge[0] + jaw_length * f * drop.cos(), hinge[1] + (y(0.24 - 0.2 * f) - y(0.24)), hinge[2] - jaw_length * f * drop.sin()];
    let teeth = if open > 0.15 { 6 } else { 0 };
    for t in 0..teeth {
        let f = 0.3 + 0.12 * t as f64;
        let tip = jaw(f);
        p.push(ball([tip[0], tip[1], tip[2] + 0.06 * d], 0.025 * l / 2.5, BONE));
        let s = 0.24 - 0.2 * f;
        p.push(ball([x(s), y(s), zc - 0.05 * d + lift - 0.06 * d], 0.025 * l / 2.5, BONE));
    }
    // The snout
    p.push(ellipsoid(at(0.1, 0.0, 0.06 * d + lift), [0.13 * l, w * 0.34, d * 0.3], k.back));
    p.push(ellipsoid(at(0.13, 0.0, -0.02 * d + lift), [0.11 * l, w * 0.3, d * 0.2], k.belly));
    // The lower jaw
    p.push(limb(hinge, jaw(0.92), 0.17 * d, 0.1 * d, k.belly));
    if open > 0.15 {
        p.push(ellipsoid([x(0.16), y(0.16), zc - 0.04 * d + lift * 0.3], [0.1 * l, w * 0.27, d * 0.28 * open + 0.03], MEAT));
    }
    // A dark tip to the dorsal fin, the pectorals and the lower lobe of the tail
    let fin_root = |s: f64| at(s, 0.0, 0.45 * d * girth(s));
    let fin_tip = at(0.5, 0.0, 0.45 * d + k.fin);
    p.push(ball([fin_tip[0] + 0.02, fin_tip[1], fin_tip[2] - 0.04], 0.06 * l / 2.5, k.tips));
    // Its belly, pale under the body
    for g in 0..7 {
        let s = 0.16 + 0.095 * g as f64;
        let h = d * girth(s);
        p.push(ellipsoid(at(s, 0.0, -0.2 * h), [0.075 * l, w * 0.47 * girth(s).sqrt(), h * 0.31], k.belly));
    }
    // Its body, segment by segment, swept to the side the further back
    for g in 0..9 {
        let s = 0.14 + 0.08 * g as f64;
        let h = d * girth(s);
        p.push(ellipsoid(at(s, 0.0, 0.0), [0.07 * l, w * 0.5 * girth(s).sqrt(), h * 0.5], k.back));
    }
    // The dorsal fin, standing tall to cut the top of the water, its trailing edge curved back
    for g in 0..6 {
        let s = 0.36 + 0.035 * g as f64;
        p.push(limb(fin_root(s), fin_tip, 0.045, 0.02, k.back));
    }
    // A little second fin on its back near the tail, and one under it
    p.push(limb(fin_root(0.72), at(0.78, 0.0, 0.45 * d * girth(0.74) + 0.22 * k.fin), 0.03, 0.015, k.back));
    p.push(limb(at(0.72, 0.0, -0.45 * d * girth(0.72)), at(0.78, 0.0, -0.45 * d * girth(0.72) - 0.18 * k.fin), 0.03, 0.015, k.back));
    // Its pectoral fins, out either side and back like wings, low
    for side in [-1.0, 1.0] {
        let root = at(0.3, side * w * 0.35, -0.25 * d);
        let tip = [x(0.44), y(0.44) + side * (w * 0.5 + 0.5 * l / 2.5), zc - 0.5 * d - SHARK_UNDER * 0.6];
        p.push(ball(tip, 0.04 * l / 2.5, k.tips));
        p.push(limb(root, tip, 0.06 * l / 2.5, 0.025, k.back));
        p.push(limb(at(0.36, side * w * 0.3, -0.25 * d), tip, 0.05 * l / 2.5, 0.025, k.back));
    }
    // Its tail: the root of it, then the long upper lobe swept up and back, and a shorter lower one
    let root = at(0.84, 0.0, 0.0);
    p.push(limb(at(0.76, 0.0, 0.0), root, 0.09 * d * 2.0, 0.06 * d * 2.0, k.back));
    let upper = [x(1.04), y(1.0) * 1.1, zc + 0.95 * d];
    let lower = [x(0.98), y(0.97) * 1.1, zc - 0.62 * d];
    p.push(ball(lower, 0.04 * l / 2.5, k.tips));
    for g in 0..3 {
        let f = g as f64 / 2.0;
        p.push(limb(at(0.83 + 0.04 * f, 0.0, 0.05 * d), [upper[0] + 0.1 * f * l * 0.1, upper[1], upper[2] - 0.35 * d * f], 0.05 * l / 2.5, 0.02, k.back));
        p.push(limb(at(0.83 + 0.03 * f, 0.0, -0.05 * d), [lower[0], lower[1], lower[2] + 0.25 * d * f], 0.045 * l / 2.5, 0.02, k.back));
    }
}

// ---------------------------------------------------------------------------------------------
// How they swim

/// What a shark's doing
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hunt {
    /// Going about the deep water, with nobody in it to go for
    Cruise,
    /// Going round the player, `round` radians round them, `way` 1 or -1, closing in for
    /// `length` seconds, `t` so far
    Circle { round: f64, way: f64, t: f64, length: f64 },
    /// Coming straight at them, jaws gaping wider the nearer it gets
    Charge { t: f64 },
    /// Its jaws snapped shut, `t` seconds ago
    Bite { t: f64 },
    /// Thrashing round and off away from them, `t` seconds so far
    Break { t: f64, away: f64 },
}

/// How a fish or a shark swims, kept on its Animal, see Animal::swim
pub struct Swim {
    /// Which way it's going and how fast, in meters a second
    velocity: V3,
    /// Where the rest of its school would have it go, in meters a second a second, see school()
    steer: V3,
    /// Which school it's of
    pub school: u32,
    /// How far up from the bottom of the water to its top it keeps, 0 to 1
    level: f64,
    /// The bottom of the water under it, in voxels, looked for now and then, and seconds till next
    bottom: f64,
    check: f64,
    /// Seconds it goes on darting away from something
    pub scared: f64,
    /// Turning one way or the other as it wanders, radians a second
    wander: f64,
    /// Leaping out of the water: how fast it's going, in meters a second, while it's out
    leap: Option<V3>,
    /// Where it splashed into or out of the water this frame, and how big a splash, for game.rs
    pub splash: Option<(V3, f64)>,
    /// A school of reef fish in colours of their own, see AnimalInstance::skinned()
    pub skin: u32,
    /// A shark's
    pub hunt: Hunt,
    /// Where its fin's cutting the top of the water this frame, and how fast it's going, in voxels
    /// and voxels a second, for the spray and the wake it leaves, see game\sea.rs
    pub fin: Option<(V3, V3)>,
    /// Seconds the player's been out of the water, for a shark to give up on them
    lost: f64,
    /// Seconds stranded with no water under it, to go once it's been so a while
    dry: f64,
    /// Gone: swum off out of the way, to be taken out of the world without a word
    pub gone: bool,
    /// Leaving: a ghostmaw going back down into the deep as the day comes
    pub leaving: bool,
}

impl Swim {
    fn new(level: f64, school: u32, skin: u32, heading: f64, speed: f64) -> Self {
        Swim {
            velocity: Vector3::new(heading.cos(), heading.sin(), 0.0) * speed,
            steer: V3::zeros(),
            school,
            level,
            bottom: f64::NEG_INFINITY,
            check: 0.0,
            scared: 0.0,
            wander: 0.0,
            leap: None,
            splash: None,
            skin,
            hunt: Hunt::Cruise,
            fin: None,
            lost: 0.0,
            dry: 0.0,
            gone: false,
            leaving: false,
        }
    }

    /// Which way it's going, in meters a second
    pub fn velocity(&self) -> V3 {
        self.velocity
    }

    /// Leaping clear of the water
    pub fn leaping(&self) -> bool {
        self.leap.is_some()
    }
}

/// The bottom of the water at `x`, `y` under its top `top`, looking down at most `most` voxels: the
/// top of the first solid voxel under it
fn bottom_at(world: &World, top: f64, x: f64, y: f64, most: f64) -> f64 {
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    let from = top.floor() as i64;
    let to = (top - most).floor() as i64;
    (to..=from).rev().find(|&z| !is_passable(world.material_at(xi, yi, z))).map_or(to as f64, |z| z as f64 + 1.0)
}

/// Whether `p`, in voxels, is in the water, `under` voxels under its top at least, with nothing
/// solid there, and the water there at least `least` voxels deep
fn wet(world: &World, water: WaterLevel, p: V3, under: f64, least: f64) -> bool {
    let top = water(p.x, p.y);
    if !top.is_finite() || top < p.z + under {
        return false;
    }
    let (xi, yi) = (p.x.floor() as i64, p.y.floor() as i64);
    if !is_passable(world.material_at(xi, yi, p.z.floor() as i64)) {
        return false;
    }
    // Deep enough all the way down from the top
    let from = top.floor() as i64 - 1;
    let to = (top - least).floor() as i64;
    (to..=from).all(|z| is_passable(world.material_at(xi, yi, z)))
}

/// Eases `angle` towards `to` the short way round, at most `rate` radians
fn turn_towards(angle: f64, to: f64, rate: f64) -> f64 {
    let off = (to - angle + PI).rem_euclid(TAU) - PI;
    angle + off.clamp(-rate, rate)
}

/// What it does this frame, see Animal::act(): a fish swims with its school, a shark hunts
pub fn act(a: &mut Animal, s: &mut Swim, world: &World, water: WaterLevel, player: Option<V3>, dt: f64) {
    a.mood = Mood::Walk;
    if let Some(f) = fish_of(a.look) {
        swim_fish(a, s, f, world, water, dt);
    } else if let Some(k) = shark_of(a.look) {
        hunt(a, s, k, world, water, player, dt);
    }
}

/// Stranded, with no water where it is: it lies still, and goes after a while
fn stranded(a: &mut Animal, s: &mut Swim, dt: f64) {
    s.dry += dt;
    a.speed = 0.0;
    if s.dry > 4.0 {
        s.gone = true;
    }
}

fn swim_fish(a: &mut Animal, s: &mut Swim, f: &Fish, world: &World, water: WaterLevel, dt: f64) {
    let vpm = world.voxels_per_meter as f64;
    let half = middle(f.look) * vpm;
    let mut center = a.position + Vector3::new(0.0, 0.0, half);
    let pace = a.status.pace();
    // Leaping: flying through the air in an arc, its nose following it, till it's back under the
    // water with a splash
    if let Some(mut v) = s.leap {
        v.z -= 9.8 * dt;
        center += v * vpm * dt;
        let top = water(center.x, center.y);
        if v.z < 0.0 && (!top.is_finite() || center.z < top) {
            s.leap = None;
            s.splash = top.is_finite().then(|| (Vector3::new(center.x, center.y, top), 0.16));
            s.velocity = Vector3::new(v.x, v.y, -0.5) * 0.5;
            if !top.is_finite() {
                s.gone = true;
            }
        } else {
            s.leap = Some(v);
        }
        a.position = center - Vector3::new(0.0, 0.0, half);
        a.heading = v.y.atan2(v.x);
        a.target_heading = a.heading;
        a.lie = [-(v.z.atan2(v.xy().norm())).clamp(-1.2, 1.2), 0.0];
        a.phase = (a.phase + dt * f.beat * 2.0).fract();
        a.speed = v.norm();
        return;
    }
    let top = water(center.x, center.y);
    if !top.is_finite() || pace <= 0.0 {
        if !top.is_finite() {
            stranded(a, s, dt);
        }
        a.speed = 0.0;
        return;
    }
    s.dry = 0.0;
    s.check -= dt;
    if s.check <= 0.0 {
        s.bottom = bottom_at(world, top, center.x, center.y, 8.0 * vpm);
        s.check = 0.35 + 0.3 * random(&mut a.rng);
    }
    // Where it likes to be, between the bottom and the top of the water
    let low = s.bottom + 0.15 * vpm + half;
    let high = top - 0.3 * vpm;
    let want = if high > low { low + (high - low) * s.level } else { (low + high) * 0.5 };

    let mut v = s.velocity;
    v += s.steer * dt;
    // Wandering a little this way and that
    s.wander = (s.wander + (random(&mut a.rng) - 0.5) * 6.0 * dt).clamp(-0.8, 0.8);
    let turn = s.wander * dt;
    let (c, si) = (turn.cos(), turn.sin());
    v = Vector3::new(v.x * c - v.y * si, v.x * si + v.y * c, v.z);
    v.z += ((want - center.z) / vpm * 1.2 - v.z) * (1.0 - (-2.5 * dt).exp());
    // Never faster than it darts, nor slower than it drifts
    let fastest = (if s.scared > 0.0 { f.dart } else { f.cruise * 1.35 }) * pace;
    let slowest = f.cruise * 0.35 * pace;
    let flat = v.xy().norm();
    if flat > fastest {
        let k = fastest / flat;
        v.x *= k;
        v.y *= k;
    } else if flat < slowest {
        let dir = if flat > 1e-6 { v.xy() / flat } else { Vector2::new(a.heading.cos(), a.heading.sin()) };
        v.x = dir.x * slowest;
        v.y = dir.y * slowest;
    }
    v.z = v.z.clamp(-0.6 * fastest.max(0.5), 0.6 * fastest.max(0.5));
    // Feeling ahead: where it's going is too shallow, or the shore or a rock's in the way, it turns
    // aside to wherever isn't, or back
    let fits = |p: V3| wet(world, water, p, 0.12 * vpm, f.least * vpm);
    let heading = v.y.atan2(v.x);
    let feel = (0.35 + 0.25 * v.xy().norm()) * vpm;
    let probe = |h: f64| center + Vector3::new(h.cos(), h.sin(), 0.0) * feel;
    if !fits(probe(heading)) {
        let way = [0.9, -0.9, 1.8, -1.8].into_iter().find(|&t| fits(probe(heading + t))).unwrap_or(PI);
        let to = heading + way;
        let speed = v.xy().norm();
        let new = turn_towards(heading, to, 5.0 * dt);
        v.x = new.cos() * speed;
        v.y = new.sin() * speed;
        s.steer = V3::zeros();
    }
    let next = center + v * vpm * dt;
    if fits(next) || !fits(center) {
        center = next;
    } else {
        v.x *= -0.4;
        v.y *= -0.4;
    }
    center.z = center.z.min(top - 0.1 * vpm);
    // Leaping clear of the water now and then, up near the top of it
    if f.leaps && s.scared <= 0.0 && top - center.z < 0.8 * vpm && random(&mut a.rng) < dt / 90.0 {
        let dir = v.xy().try_normalize(1e-6).unwrap_or(Vector2::x());
        let leap = Vector3::new(dir.x * 2.2, dir.y * 2.2, 3.6 + random(&mut a.rng));
        s.leap = Some(leap);
        s.splash = Some((Vector3::new(center.x, center.y, top), 0.1));
        center.z = top + 0.02 * vpm;
    }
    a.position = center - Vector3::new(0.0, 0.0, half);
    s.velocity = v;
    s.scared = (s.scared - dt).max(0.0);
    a.speed = v.norm();
    a.heading = v.y.atan2(v.x);
    a.target_heading = a.heading;
    a.lie = [-(v.z.atan2(v.xy().norm().max(1e-3))).clamp(-0.6, 0.6), 0.0];
    a.phase = (a.phase + dt * f.beat * (0.5 + 0.8 * v.norm() / f.cruise)).fract();
}

/// Where the player's middle is if they're in water deep enough to be come at, over their chest,
/// see hunt(): their feet `feet`, in voxels
fn prey(water: WaterLevel, feet: V3, vpm: f64) -> Option<V3> {
    let top = water(feet.x, feet.y);
    (top.is_finite() && top > feet.z + 1.0 * vpm).then(|| feet + Vector3::new(0.0, 0.0, 1.0 * vpm))
}

fn hunt(a: &mut Animal, s: &mut Swim, k: &Shark, world: &World, water: WaterLevel, player: Option<V3>, dt: f64) {
    let vpm = world.voxels_per_meter as f64;
    let half = middle(k.look) * vpm;
    let mut center = a.position + Vector3::new(0.0, 0.0, half);
    s.fin = None;
    let top = water(center.x, center.y);
    let pace = a.status.pace();
    if let Some(boss) = &mut a.boss {
        boss.fixed_pose = None;
    }
    if !top.is_finite() {
        stranded(a, s, dt);
        return;
    }
    s.dry = 0.0;
    if pace <= 0.0 {
        a.speed = 0.0;
        return;
    }
    s.check -= dt;
    if s.check <= 0.0 {
        s.bottom = bottom_at(world, top, center.x, center.y, 30.0 * vpm);
        s.check = 0.25;
    }
    let fits = |p: V3| wet(world, water, p, 0.05 * vpm, k.least * vpm);
    let target = player.and_then(|feet| prey(water, feet, vpm));
    s.lost = if target.is_some() { 0.0 } else { s.lost + dt };
    let engaged = a.boss.as_ref().is_some_and(|b| b.engaged);
    // Riding high enough for its fin to cut the top of the water, or keeping lower down
    let fin_out = top + 0.22 * vpm - (k.depth * 0.5 + k.fin) * vpm;
    let cruising_depth = (s.bottom + half + 0.3 * vpm).max(top - (1.2 + k.depth) * vpm).min(fin_out);
    let mut speed = k.cruise;
    let mut turn = 1.4;
    let mut goal: Option<V3> = None;
    let mut want_z = cruising_depth;
    let mut frame: Option<usize> = None;
    let near = |t: V3| (t - center).norm() / vpm;
    match s.hunt {
        Hunt::Cruise => {
            // Going about where it is, now and then rising to show its fin, more so with the
            // player near
            s.wander = (s.wander + (random(&mut a.rng) - 0.5) * 2.0 * dt).clamp(-0.35, 0.35);
            a.heading += s.wander * dt;
            if player.is_some_and(|p| near(p) < 45.0) {
                want_z = fin_out;
            }
            if s.leaving {
                want_z = s.bottom + half + 0.3 * vpm;
            } else if let Some(t) = target.filter(|&t| near(t) < if engaged { 45.0 } else { 30.0 }) {
                let off = (center - t).xy();
                let way = if random(&mut a.rng) < 0.5 { 1.0 } else { -1.0 };
                let length = if engaged { 1.5 } else { 5.0 + 4.0 * random(&mut a.rng) };
                s.hunt = Hunt::Circle { round: off.y.atan2(off.x), way, t: 0.0, length };
            }
        }
        Hunt::Circle { round, way, t, length } => {
            let t = t + dt;
            if s.lost > 2.5 || s.leaving {
                s.hunt = Hunt::Cruise;
            } else if let Some(prey) = target {
                let ring = k.ring.0 + (k.ring.1 - k.ring.0) * (t / length).min(1.0);
                speed = k.circle;
                let round = round + way * speed / ring * dt;
                let ahead = round + way * 0.45;
                let point = prey + Vector3::new(ahead.cos(), ahead.sin(), 0.0) * ring * vpm;
                let mut way = way;
                if !fits(Vector3::new(point.x, point.y, fin_out)) {
                    way = -way;
                }
                goal = Some(point);
                want_z = fin_out;
                turn = 2.2;
                s.hunt = Hunt::Circle { round, way, t, length };
                // Closed in: it comes at them, if there's water deep enough for it all the way
                if t >= length && reachable(world, water, center, prey, k, vpm) {
                    s.hunt = Hunt::Charge { t: 0.0 };
                    if let Some(boss) = &mut a.boss {
                        boss.deeds.push(Deed::WindUp(Move::Slam, center));
                    }
                }
            } else {
                s.hunt = Hunt::Circle { round, way, t, length };
            }
        }
        Hunt::Charge { t } => {
            let t = t + dt;
            match target {
                Some(prey) => {
                    speed = k.dart;
                    turn = 3.2;
                    goal = Some(prey);
                    want_z = prey.z.min(top - (k.depth * 0.5) * vpm);
                    let d = near(prey);
                    // Its jaws opening wider the nearer it gets
                    let gape = ((6.5 - d) / 4.5).clamp(0.0, 1.0);
                    frame = Some(gaping(a.look, gape));
                    let snout = center + Vector3::new(a.heading.cos(), a.heading.sin(), 0.0) * k.length * 0.45 * vpm;
                    if (snout - prey).norm() < 1.1 * vpm || d < k.length * 0.45 + 0.6 {
                        s.hunt = Hunt::Bite { t: 0.0 };
                        if let Some(boss) = &mut a.boss {
                            boss.deeds.push(Deed::Slam { at: snout, radius: 1.5, damage: k.bite, crater: 0.0 });
                        }
                    } else if t > 4.0 {
                        s.hunt = Hunt::Break { t: 0.0, away: a.heading + PI };
                    } else {
                        s.hunt = Hunt::Charge { t };
                    }
                }
                None => s.hunt = Hunt::Break { t: 0.0, away: a.heading + FRAC_PI_2 },
            }
        }
        Hunt::Bite { t } => {
            let t = t + dt;
            // Snapped shut, carried on past by its lunge, then opening its jaws again
            speed = k.dart * (1.0 - t / 0.6).max(0.3);
            turn = 0.6;
            frame = if t < 0.22 { Some(super::POSE_STRIKE) } else { super::tween(a.look, super::POSE_STRIKE, 0, t - 0.22) };
            if t > 0.55 {
                let away = target.map_or(a.heading, |p| {
                    let off = (center - p).xy();
                    off.y.atan2(off.x)
                });
                let side = if random(&mut a.rng) < 0.5 { 0.7 } else { -0.7 };
                s.hunt = Hunt::Break { t: 0.0, away: away + side };
            } else {
                s.hunt = Hunt::Bite { t };
            }
        }
        Hunt::Break { t, away } => {
            let t = t + dt;
            speed = k.cruise * 2.0;
            turn = 2.8;
            a.heading = turn_towards(a.heading, away, turn * dt);
            // Thrashing round as it turns away, and straightening out
            frame = if t < 0.3 {
                super::tween(a.look, 0, super::POSE_SPECIAL, t).or(Some(super::POSE_SPECIAL))
            } else if t < 0.75 {
                Some(super::POSE_SPECIAL)
            } else {
                super::tween(a.look, super::POSE_SPECIAL, 0, t - 0.75)
            };
            if t > 2.6 {
                s.hunt = match target {
                    Some(prey) if !s.leaving => {
                        let off = (center - prey).xy();
                        Hunt::Circle { round: off.y.atan2(off.x), way: if random(&mut a.rng) < 0.5 { 1.0 } else { -1.0 }, t: 0.0, length: if engaged { 2.0 } else { 3.0 + 3.0 * random(&mut a.rng) } }
                    }
                    _ => Hunt::Cruise,
                };
            } else {
                s.hunt = Hunt::Break { t, away };
            }
        }
    }
    if let Some(boss) = &mut a.boss {
        boss.fixed_pose = frame;
    }
    // Turning towards where it's going
    if let Some(goal) = goal {
        let off = (goal - center).xy();
        if off.norm() > 1e-3 {
            a.heading = turn_towards(a.heading, off.y.atan2(off.x), turn * pace * dt);
        }
    }
    // Feeling ahead: too shallow there, or the shore, and it turns for deeper water; charging, it
    // gives up and breaks off
    let feel = (2.0 + 0.4 * a.speed) * vpm;
    let probe = |h: f64| Vector3::new(center.x + h.cos() * feel, center.y + h.sin() * feel, center.z.min(top - 0.2 * vpm));
    if !fits(probe(a.heading)) {
        if let Hunt::Charge { .. } = s.hunt {
            s.hunt = Hunt::Break { t: 0.0, away: a.heading + PI };
        }
        let way = [0.8, -0.8, 1.6, -1.6, 2.4, -2.4].into_iter().find(|&t| fits(probe(a.heading + t))).unwrap_or(PI);
        a.heading = turn_towards(a.heading, a.heading + way, 3.0 * dt);
        if let Hunt::Circle { round, way, t, length } = s.hunt {
            s.hunt = Hunt::Circle { round, way: -way, t, length };
        }
    }
    a.heading = a.heading.rem_euclid(TAU);
    a.target_heading = a.heading;
    a.speed += (speed * pace - a.speed) * (1.0 - (-3.0 * dt).exp());
    // Up or down to where it wants to be, never out of the water, never into the bottom of it
    let rise = ((want_z - center.z) / vpm * 1.5).clamp(-1.5, 1.5);
    let mut step = Vector3::new(a.heading.cos() * a.speed, a.heading.sin() * a.speed, rise) * vpm * dt;
    // Knocked back by a blow
    step += a.status.push * dt;
    a.status.push *= (-6.0 * dt).exp();
    let next = center + step;
    if fits(next) || !fits(center) {
        center = next;
    } else if fits(Vector3::new(center.x, center.y, next.z)) {
        center.z = next.z;
    }
    let top = water(center.x, center.y);
    if top.is_finite() {
        center.z = center.z.min(top - (k.depth * 0.5 - 0.06) * vpm).max(s.bottom + half * 0.8);
        // Its fin through the top of the water
        let fin_top = center.z + (k.depth * 0.5 + k.fin) * vpm;
        if fin_top > top {
            let back = Vector3::new(a.heading.cos(), a.heading.sin(), 0.0) * (-0.02 * k.length) * vpm;
            s.fin = Some((Vector3::new(center.x + back.x, center.y + back.y, top), Vector3::new(a.heading.cos(), a.heading.sin(), 0.0) * a.speed * vpm));
        }
    }
    s.velocity = Vector3::new(a.heading.cos(), a.heading.sin(), 0.0) * a.speed + Vector3::new(0.0, 0.0, rise);
    a.position = center - Vector3::new(0.0, 0.0, half);
    a.lie = [-(rise.atan2(a.speed.max(0.5))).clamp(-0.45, 0.45), 0.0];
    a.phase = (a.phase + dt * (0.45 + 0.35 * a.speed)).fract();
}

/// Which frame a shark's drawn in with its jaws `gape` of the way open, see animals::path_to()
fn gaping(look: Look, gape: f64) -> usize {
    let path = super::path_to(look, super::POSE_WINDUP);
    path.iter().min_by(|a, b| (a.0 - gape).abs().total_cmp(&(b.0 - gape).abs())).map_or(0, |p| p.1)
}

/// Whether there's water deep enough for shark `k` all the way from `from` to `to`, in voxels, a
/// meter at a time, but for the last meter and a half, where the player is
fn reachable(world: &World, water: WaterLevel, from: V3, to: V3, k: &Shark, vpm: f64) -> bool {
    let off = to - from;
    let steps = (off.norm() / vpm) as usize;
    (1..steps.saturating_sub(1)).all(|i| {
        let p = from + off * (i as f64 / steps as f64);
        let top = water(p.x, p.y);
        wet(world, water, Vector3::new(p.x, p.y, top - 0.5 * vpm), 0.05 * vpm, (k.least * 0.8) * vpm)
    })
}

/// Steers every fish by those of its school about it, see the module's comment: `player`'s middle,
/// in voxels, and `threats`, where something's splashed in or a shark's swimming, scatter them.
/// Only fish in the water, not those leaping. A few dozen at most, so each looks at every other.
pub fn school(animals: &mut [Animal], player: Option<V3>, threats: &[V3], vpm: f64) {
    let fish: Vec<usize> = (0..animals.len()).filter(|&i| is_fish(animals[i].look) && animals[i].swim.as_ref().is_some_and(|s| !s.leaping())).collect();
    let seen: Vec<(V3, V3, u32)> = fish
        .iter()
        .map(|&i| {
            let s = animals[i].swim.as_ref().unwrap();
            (animals[i].position, s.velocity, s.school)
        })
        .collect();
    for (n, &i) in fish.iter().enumerate() {
        let (p, v, school) = seen[n];
        let (mut apart, mut heading, mut middle, mut count) = (V3::zeros(), V3::zeros(), V3::zeros(), 0.0);
        for (m, &(q, w, other)) in seen.iter().enumerate() {
            if m == n || other != school {
                continue;
            }
            let off = (p - q) / vpm;
            let d = off.norm();
            if d > 2.2 {
                continue;
            }
            if d < 0.4 {
                apart += off / (d * d).max(0.01);
            }
            heading += w;
            middle += q;
            count += 1.0;
        }
        let mut steer = apart * 0.35;
        if count > 0.0 {
            steer += (heading / count - v) * 1.1;
            steer += (middle / count - p) / vpm * 0.7;
        }
        // Darting away from the player and from anything splashing in, a little up or down too
        let mut scared = false;
        for &t in player.iter().chain(threats) {
            let off = (p - t) / vpm;
            let d = off.norm();
            let near = if Some(t) == player { 3.2 } else { 5.0 };
            if d < near {
                steer += off.try_normalize(1e-6).unwrap_or(V3::x()) * 9.0 * (1.0 - d / near + 0.3);
                scared = true;
            }
        }
        if let Some(s) = animals[i].swim.as_mut() {
            s.steer = steer;
            if scared {
                s.scared = s.scared.max(1.4);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Where they're found

/// The kind of fish living in the water at `at`, in voxels, its bottom there `bottom` and its top
/// `top`: in the cold, frostfins; under the rock, gloamfins; in the open sea, silverfins and, in its
/// warm sandy shallows, reeflings; and in the rivers and lakes, brookbacks
pub fn fish_for(world: &World, land: Land, at: V3, top: f64, bottom: f64, roll: f64) -> Look {
    let vpm = world.voxels_per_meter as f64;
    let (x, y) = (at.x.floor() as i64, at.y.floor() as i64);
    let ground = world.material_at(x, y, bottom.floor() as i64 - 1);
    // Rock overhead, or walled in deep, the light hardly getting down to it
    let roofed = (1..(8.0 * vpm) as i64).any(|dz| !is_passable(world.material_at(x, y, top.floor() as i64 + dz)));
    let cold = land == Land::Ice || matches!(ground, SNOW | ICE) || near_material(world, x, y, top.floor() as i64, &[SNOW, ICE]);
    if cold {
        return Look::Frostfin;
    }
    if roofed {
        return Look::Gloamfin;
    }
    let sea = world.sea_level.is_some_and(|sea| (top - sea as f64).abs() < 0.8 * vpm) || land == Land::Water;
    if !sea {
        return Look::Brookback;
    }
    let shallow = top - bottom < 3.5 * vpm;
    let sandy = matches!(ground, SAND | CLAY | GRAVEL);
    let warm = land == Land::Water || near_material(world, x, y, top.floor() as i64, &[JUNGLE_GRASS, PALM_WOOD, PALM_LEAF, SAND]);
    if shallow && sandy && warm && roll < if land == Land::Water { 0.75 } else { 0.55 } { Look::Reefling } else { Look::Silverfin }
}

/// Whether any of `materials` is on the shore round `x`, `y`, a little over `z`, looking 6 meters
/// out a few ways
fn near_material(world: &World, x: i64, y: i64, z: i64, materials: &[u8]) -> bool {
    let reach = 6 * world.voxels_per_meter as i64;
    (0..8).any(|k| {
        let a = k as f64 * TAU / 8.0;
        let (px, py) = (x + (a.cos() * reach as f64) as i64, y + (a.sin() * reach as f64) as i64);
        (-2..6).any(|dz| materials.contains(&world.material_at(px, py, z + dz)))
    })
}

/// A school of `look` round `at`, in voxels, in the water there: None if it isn't deep enough
pub fn spawn_school(world: &World, water: WaterLevel, look: Look, at: V3, seed: u32) -> Vec<Animal> {
    let Some(f) = fish_of(look) else {
        return Vec::new();
    };
    let vpm = world.voxels_per_meter as f64;
    let mut rng = hash(seed ^ 0xf15e);
    let count = f.school.0 + (random(&mut rng) * (f.school.1 - f.school.0 + 1) as f64) as usize;
    let heading = random(&mut rng) * TAU;
    let level = 0.25 + 0.5 * random(&mut rng);
    // Reeflings come in colours of their own, each school its own: coral, violet or sea green, or
    // as they are
    let skin = if look == Look::Reefling {
        match (random(&mut rng) * 4.0) as u32 {
            0 => pack_skin([255, 120, 90], 0.55),
            1 => pack_skin([170, 110, 255], 0.5),
            2 => pack_skin([90, 230, 190], 0.5),
            _ => 0,
        }
    } else {
        0
    };
    let mut out = Vec::new();
    for _ in 0..count {
        let a = random(&mut rng) * TAU;
        let r = random(&mut rng).sqrt() * 1.2 * vpm;
        let top = water(at.x + a.cos() * r, at.y + a.sin() * r);
        let p = Vector3::new(at.x + a.cos() * r, at.y + a.sin() * r, at.z + (random(&mut rng) - 0.5) * 0.4 * vpm);
        if !top.is_finite() || !wet(world, water, p, 0.15 * vpm, f.least * vpm) {
            continue;
        }
        let mut fish = Animal::new(look, p - Vector3::new(0.0, 0.0, middle(look) * vpm), hash(rng ^ out.len() as u32));
        let h = heading + (random(&mut rng) - 0.5) * 0.4;
        fish.face(h);
        fish.phase = random(&mut rng);
        fish.swim = Some(Box::new(Swim::new(level + (random(&mut rng) - 0.5) * 0.15, seed, skin, h, f.cruise)));
        out.push(fish);
    }
    out
}

/// A colour taken on `how` far, as AnimalInstance::skinned() has it
fn pack_skin(rgb: [u8; 3], how: f64) -> u32 {
    rgb[0] as u32 | (rgb[1] as u32) << 8 | (rgb[2] as u32) << 16 | ((how * 255.0) as u32) << 24
}

/// A shark of `look` at `at`, in voxels, the middle of its body, cruising
pub fn spawn_shark(world: &World, look: Look, at: V3, seed: u32) -> Animal {
    let vpm = world.voxels_per_meter as f64;
    let mut shark = Animal::boss(look, at - Vector3::new(0.0, 0.0, middle(look) * vpm), [at.x as i64, at.y as i64, -2], seed);
    let heading = shark.heading();
    shark.swim = Some(Box::new(Swim::new(0.5, 0, 0, heading, speeds(look).0)));
    shark
}

/// Somewhere a shark of `look` can be, `from` to `to` meters from `near` in voxels: the middle of
/// its body at about the depth it cruises at, in water deep enough for it and well away from the
/// shore, in the sea
pub fn shark_water(world: &World, water: WaterLevel, look: Look, near: V3, from: f64, to: f64, seed: u32) -> Option<V3> {
    let k = shark_of(look)?;
    let vpm = world.voxels_per_meter as f64;
    let mut rng = hash(seed ^ 0x5a4c);
    for _ in 0..24 {
        let a = random(&mut rng) * TAU;
        let r = (from + (to - from) * random(&mut rng)) * vpm;
        let (x, y) = (near.x + a.cos() * r, near.y + a.sin() * r);
        if x < 2.0 || y < 2.0 || x >= world.size[0] as f64 - 2.0 || y >= world.size[1] as f64 - 2.0 {
            continue;
        }
        let top = water(x, y);
        if !top.is_finite() || world.sea_level.is_some_and(|sea| (top - sea as f64).abs() > 0.8 * vpm) {
            continue;
        }
        // Deep enough here, and a few meters round, so it's away from the shore
        let deep = |x: f64, y: f64| wet(world, water, Vector3::new(x, y, top - 0.5 * vpm), 0.05 * vpm, (k.least + 1.0) * vpm);
        if deep(x, y) && (0..4).all(|j| {
            let b = j as f64 * TAU / 4.0;
            deep(x + b.cos() * 4.0 * vpm, y + b.sin() * 4.0 * vpm)
        }) {
            return Some(Vector3::new(x, y, top - (k.depth * 0.5 + 0.6) * vpm));
        }
    }
    None
}

/// Somewhere in the water `from` to `to` meters from `near`, in voxels, for a school: the middle of
/// it, partway down, the water's top there and its bottom
pub fn school_water(world: &World, water: WaterLevel, near: V3, from: f64, to: f64, seed: u32) -> Option<(V3, f64, f64)> {
    let vpm = world.voxels_per_meter as f64;
    let mut rng = hash(seed ^ 0x5c401);
    for _ in 0..10 {
        let a = random(&mut rng) * TAU;
        let r = (from + (to - from) * random(&mut rng)) * vpm;
        let (x, y) = (near.x + a.cos() * r, near.y + a.sin() * r);
        if x < 2.0 || y < 2.0 || x >= world.size[0] as f64 - 2.0 || y >= world.size[1] as f64 - 2.0 {
            continue;
        }
        let top = water(x, y);
        if !top.is_finite() {
            continue;
        }
        let bottom = bottom_at(world, top, x, y, 12.0 * vpm);
        if top - bottom < 0.5 * vpm {
            continue;
        }
        let z = bottom + (top - bottom) * (0.3 + 0.4 * random(&mut rng));
        return Some((Vector3::new(x, y, z), top, bottom));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animals::{FRAMES, MODEL_RES, POSE_STRIKE, POSE_WINDUP, frame_model, shown};

    /// Every frame of every fish and shark: solid enough to see, small enough for the shader, and
    /// inside the reach it's culled by
    #[test]
    fn sea_life_has_whole_models_in_every_frame() {
        for look in FISH.into_iter().chain(SHARKS).chain([Look::RawFish, Look::CookedFish, Look::SharkTooth]) {
            let res = look.resolution();
            for frame in (0..FRAMES).filter(|&f| shown(look, f)) {
                let model = frame_model(look, frame);
                assert!(model.size.iter().all(|&s| s < 255), "{look:?} frame {frame} is {:?} voxels", model.size);
                let mut solid = 0;
                let mut furthest = 0.0f64;
                for z in 0..model.size[2] {
                    for y in 0..model.size[1] {
                        for x in 0..model.size[0] {
                            if model.voxels[(x + (y + z * model.size[1]) * model.size[0]) as usize] == AIR {
                                continue;
                            }
                            solid += 1;
                            let at = Vector3::new(x as f64 + 0.5 - model.size[0] as f64 * 0.5, y as f64 + 0.5 - model.size[1] as f64 * 0.5, z as f64 + 0.5) / res;
                            furthest = furthest.max(at.norm());
                        }
                    }
                }
                let least = if is_shark(look) { 2000 } else { 25 };
                assert!(solid > least, "{look:?} frame {frame} has only {solid} voxels");
                assert!(furthest <= look.reach(), "{look:?} frame {frame} reaches {furthest:.2} m, past its {:.2} m", look.reach());
            }
        }
        assert_eq!(Look::Silverfin.resolution(), FINE * MODEL_RES);
        assert_eq!(crate::assets::look_res(Look::Silverfin), Look::Silverfin.resolution());
    }

    /// A shark's jaws gape as it winds up to bite and snap shut as it bites: its frames differ
    #[test]
    fn a_shark_opens_its_jaws_to_bite() {
        for look in SHARKS {
            let rest = frame_model(look, 0);
            let open = frame_model(look, POSE_WINDUP);
            let shut = frame_model(look, POSE_STRIKE);
            let meat = |m: &crate::animals::Model| m.voxels.iter().filter(|&&v| v == MEAT).count();
            assert!(meat(&open) > 20, "{look:?} shows no mouth gaping");
            assert_eq!(meat(&shut), 0, "{look:?} snaps its jaws shut");
            assert!(meat(&rest) < meat(&open) / 3, "{look:?} at rest has its jaws open");
            let swept: Vec<_> = (1..=4).map(|f| frame_model(look, f).voxels).collect();
            assert!(swept.windows(2).any(|w| w[0] != w[1]), "{look:?}'s tail doesn't sweep");
        }
        for look in FISH {
            let a = frame_model(look, 1).voxels;
            let b = frame_model(look, 11).voxels;
            assert_ne!(a, b, "{look:?}'s tail doesn't sweep");
        }
    }
}
