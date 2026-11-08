//! Serpents: the ordinary ones that roam every realm, a pack of two or three at a time, and the
//! great serpents that keep lairs of their own.
//!
//! An ordinary serpent is one model in many frames, like any monster, see animals.rs: its walk is
//! a slither, an S-curve travelling down its body as fast as it goes, so every part of it follows
//! the way its head went, and its poses rear it up coiled to strike, lunge it forward to bite, and
//! raise its head to spit. It's drawn with one instance, so a fight with a nest of them costs no
//! more than one with wolves.
//!
//! Where it's hit is the part of it nearest where the player's aiming, see focus(), so blows land
//! along the whole of its long body, not only round its middle.
//!
//! A great serpent is too long for one model, and has to bend all along itself: its head and jaw
//! are a model each, and its body a chain of pieces of one model, each drawn as an instance where
//! it is, see Coils. Its head leads, as an animal goes, and each piece follows the one in front at
//! its own distance, so the body slithers along where the head went, coils round where it circles,
//! pours into the hole it dives into and out of the one it bursts up from.

use std::f64::consts::{PI, TAU};
use std::sync::OnceLock;

use nalgebra::{UnitQuaternion, Vector2, Vector3};

use crate::animals::{Animal, AnimalInstance, Look, MOVING, Model, Part, ball, ellipsoid, limb, voxelize};
use crate::player::WaterLevel;
use crate::weapons::Element;
use crate::worldgen::{self, TerrainSettings, World, is_passable, material::*};

type Vec3 = [f64; 3];

/// What an ordinary serpent has besides its body and head
#[derive(Clone, Copy, PartialEq)]
enum Trim {
    /// A hood behind its head, flaring as it rears, glowing spots on it
    Hood,
    /// A ridge of fin down its back and a paddle of a tail, for swimming
    Fin,
    /// Two pairs of frilled fins spread like wings, to glide on
    Wings,
    /// Horns curving back over its eyes
    Horns,
    /// Spikes of ice down its back
    Spikes,
}

/// What an ordinary serpent's like to look at: its length and how thick it is at its thickest, in
/// meters, how long its head is, the colors of its back, the pattern on it, its belly, its eyes and
/// what it has besides, and whether it lies on the ground or glides over it
pub(crate) struct Kind {
    pub length: f64,
    pub girth: f64,
    head: f64,
    back: u8,
    pattern: u8,
    belly: u8,
    eye: u8,
    extra: u8,
    trim: Trim,
    /// Gliding through the air on its fins, drawn as high over the ground as it flies by
    /// Boss::sway(), its model low so it's no bigger than it has to be
    pub aloft: bool,
}

/// A reed viper: mossy green, dark diamonds down its back and a cream belly, a hood that flares
/// round two glowing spots as it rears, and green venom in its eyes. It lies hidden in long grass
/// and reeds till something comes close, and spits globs of venom.
const REED_VIPER: Kind = Kind { length: 3.0, girth: 0.145, head: 0.4, back: MOSS, pattern: PINE, belly: FUR_CREAM, eye: GLOW_POISON, extra: GLOW_POISON, trim: Trim::Hood, aloft: false };
/// A silt coiler: dark teal, blotched black, a pale belly and gold eyes, a fin down its back and a
/// paddle of a tail. It swims in rivers and lakes, only its back showing, and comes up out of them.
const SILT_COILER: Kind = Kind { length: 3.6, girth: 0.18, head: 0.46, back: TEMPLE_TEAL, pattern: OBSIDIAN, belly: LIMESTONE, eye: GOLD, extra: PINE, trim: Trim::Fin, aloft: false };
/// A gale serpent: steel grey banded darker, a linen-pale belly, storm light in its eyes, gliding
/// on two pairs of frilled fins. It comes down out of the air to bite, and spits crackling bolts.
const GALE_SERPENT: Kind = Kind { length: 3.4, girth: 0.14, head: 0.4, back: STEEL, pattern: DARK_STEEL, belly: LINEN, eye: GLOW_STORM, extra: LINEN, trim: Trim::Wings, aloft: true };
/// A bloodvein adder: black as obsidian, veined with glowing blood, bone horns curving back over
/// its eyes. It spits blood three globs at a time.
const BLOOD_ADDER: Kind = Kind { length: 2.8, girth: 0.15, head: 0.38, back: OBSIDIAN, pattern: GLOW_BLOOD, belly: BONE, eye: GLOW_BLOOD, extra: BONE, trim: Trim::Horns, aloft: false };
/// A drift serpent: grey banded dark, a white belly, frost in its eyes and spikes of ice down its
/// back. It swims through the snow under the surface and bursts up out of it, and spits frost.
const DRIFT_SERPENT: Kind = Kind { length: 3.4, girth: 0.16, head: 0.42, back: STEEL, pattern: DARK_STEEL, belly: FUR_WHITE, eye: GLOW_FROST, extra: ICE, trim: Trim::Spikes, aloft: false };

/// The ordinary serpent `look` is, if it is one
pub(crate) fn kind(look: Look) -> Option<&'static Kind> {
    Some(match look {
        Look::ReedViper => &REED_VIPER,
        Look::SiltCoiler => &SILT_COILER,
        Look::GaleSerpent => &GALE_SERPENT,
        Look::BloodAdder => &BLOOD_ADDER,
        Look::DriftSerpent => &DRIFT_SERPENT,
        _ => return None,
    })
}

/// The ordinary serpents, one for each realm's land and the normal world's waters
#[cfg(test)]
pub const SERPENTS: [Look; 5] = [Look::ReedViper, Look::SiltCoiler, Look::GaleSerpent, Look::BloodAdder, Look::DriftSerpent];

fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn times(a: Vec3, k: f64) -> Vec3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn norm(a: Vec3) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn unit(a: Vec3, or: Vec3) -> Vec3 {
    let n = norm(a);
    if n > 1e-9 { times(a, 1.0 / n) } else { or }
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// Meters along the ground its S-curve's waves are apart, which is how far it goes for each of its
/// walk's cycles, so every part of it slides along where the part in front of it was
pub(crate) fn stride(k: &Kind) -> f64 {
    0.55 * k.length
}

/// How far from its middle any of it reaches, however it's posed, in meters: its head lunged out
pub(crate) fn reach(k: &Kind) -> f64 {
    0.66 * k.length + k.head + 0.12
}

/// How far under the top of the water what swims floats, in meters: about its middle, so only its
/// back's out of it
pub const DRAFT: f64 = 0.16;

/// How far under the top of the water serpent `look` floats, in meters, see DRAFT: a great serpent
/// half its body's thickness, so its back and head are out of it
pub fn draft(look: Look) -> f64 {
    great_of(look).map_or(DRAFT, |g| 0.5 * g.girth)
}

/// How high over the ground a gale serpent glides, in meters
pub const GLIDE: f64 = 1.3;

/// Seconds a serpent thrashes, rolls over and lies still after it's killed, before it's gone
pub const DEATH_THROES: f64 = 1.25;
/// The same for a great serpent, rearing, crashing down and sinking away
pub const GREAT_THROES: f64 = 3.4;

/// How high a serpent's hiss and bite sound, 1 as recorded: higher for the small and quick, lower
/// for the big
pub fn voice(look: Look) -> f32 {
    match look {
        Look::ReedViper => 1.15,
        Look::SiltCoiler => 0.8,
        Look::GaleSerpent => 1.35,
        Look::BloodAdder => 1.0,
        Look::DriftSerpent => 0.9,
        Look::Ysveth => 0.36,
        _ if look.is_raid() => 0.45,
        _ => 0.58,
    }
}

/// Where what ordinary serpent `look` spits comes out of it, in meters of its model, +x ahead: its
/// snout, reared up to spit, up where it glides if it does
pub(crate) fn mouth(look: Look) -> Vec3 {
    let Some(k) = kind(look) else {
        return [1.0, 0.0, 0.5];
    };
    let neck = spine(k, 0.0, None, [0.0, 0.0, 1.0]);
    [neck[0] + 0.9 * k.head, neck[1], neck[2] + if k.aloft { GLIDE } else { 0.0 }]
}

/// How thick it is `u` of the way from its snout to the tip of its tail, in meters across from its
/// middle: thinner at its neck, thickest a third of the way back, tapering to the tip
fn radius(k: &Kind, u: f64) -> f64 {
    k.girth * (0.62 + 0.38 * smooth(u / 0.22)) * (1.0 - 0.84 * smooth((u - 0.4) / 0.6))
}

/// Where its spine is `u` of the way from its head (0) to the tip of its tail (1), in meters of its
/// model: facing +x with its middle over the origin and its belly on the ground. Slithering `phase`
/// of the way through its walk, or lying still with None, and as much in each of its poses as `w`
/// says: reared up coiled to strike, lunging out to bite, and reared higher to spit.
pub(crate) fn spine(k: &Kind, u: f64, phase: Option<f64>, w: [f64; 3]) -> Vec3 {
    let l = k.length;
    let r = radius(k, u);
    let along = u * l / stride(k);
    let ph = phase.unwrap_or(0.12);
    let still = if phase.is_some() { 1.0 } else { 0.8 };
    // Its head goes straight; the waves grow behind its neck
    let envelope = smooth(u / 0.28) * (0.75 + 0.25 * u);
    let mut rest = [0.5 * l - u * l, 0.1 * l * still * envelope * (TAU * (along - ph)).sin(), r];
    if k.aloft {
        // Rising and falling a little as well as weaving, as it glides
        rest[2] += 0.05 * l * envelope * (TAU * (0.5 * along - ph) + 1.0).sin();
    }
    // Coiled flat under its neck, a loop and a little more, curling in
    let f = 0.32;
    let ring = |v: f64| {
        let turn = v * 0.95 * TAU;
        let out = (0.13 - 0.045 * v) * l;
        [-0.08 * l + out * turn.cos(), out * turn.sin(), radius(k, f + v * (1.0 - f))]
    };
    // Reared up from the coil to `height` of its length, its head `ahead` of its middle, its neck
    // bowed back in an S to strike
    let reared = |height: f64, ahead: f64| {
        if u > f {
            return ring((u - f) / (1.0 - f));
        }
        let t = 1.0 - u / f;
        let base = ring(0.0);
        let top = [ahead * l, 0.0, height * l];
        let rise = t * t * (3.0 - 2.0 * t);
        [base[0] + (top[0] - base[0]) * t - 0.07 * l * (PI * t).sin(), 0.04 * l * (PI * t).sin(), base[2] + (top[2] - base[2]) * rise]
    };
    let windup = reared(0.26, 0.15);
    let special = reared(0.31, 0.21);
    // Lunged out straight, its head a little up, its tail still curving behind
    let strike = [0.5 * l + 0.14 * l - u * l * 0.97, 0.05 * l * smooth((u - 0.35) / 0.5) * (TAU * (1.3 * u - ph)).sin(), r + 0.07 * l * (1.0 - smooth(u / 0.35))];
    let mix = |i: usize| rest[i] + w[0] * (windup[i] - rest[i]) + w[1] * (strike[i] - rest[i]) + w[2] * (special[i] - rest[i]);
    [mix(0), mix(1), mix(2)]
}

/// How many pieces its body's made of, snout to tail
const PIECES: usize = 18;

/// The shapes making up ordinary serpent `k`, in meters, facing +x, its belly on the ground or, if
/// it glides, as low as it can be: slithering `phase` of the way through its walk or lying still,
/// and as much in its poses as `w` says, see spine()
pub(crate) fn parts(p: &mut Vec<Part>, k: &Kind, phase: Option<f64>, w: [f64; 3]) {
    let at = |u: f64| spine(k, u, phase, w);
    let angle = phase.map_or(0.0, |ph| ph * TAU);
    // Its head along its neck, tipped down a little to look at what it's about to strike
    let neck = at(0.0);
    let ahead = unit(add(unit(sub(neck, at(0.035)), [1.0, 0.0, 0.0]), [0.0, 0.0, -0.3 * (w[0] + w[2])]), [1.0, 0.0, 0.0]);
    let side = unit(cross([0.0, 0.0, 1.0], ahead), [0.0, 1.0, 0.0]);
    let up = cross(ahead, side);
    let gape = 0.75 * w[0] + 1.0 * w[1] + 0.95 * w[2];
    let (hl, hw) = (k.head, k.head * 0.36);
    let lift = 0.32 * gape;
    let drop = 0.7 * gape;
    let upper = unit(add(times(ahead, lift.cos()), times(up, lift.sin())), ahead);
    let lower = unit(sub(times(ahead, drop.cos()), times(up, drop.sin())), ahead);
    let upper_up = cross(upper, side);
    let on = |dir: Vec3, normal: Vec3, f: f64, s: f64, h: f64| add(add(add(neck, times(dir, f * hl)), times(side, s * hw)), times(normal, h * hw));
    // Eyes, then fangs bared as it gapes, its tongue flicking as it slithers or rears
    for s in [-1.0, 1.0] {
        p.push(ball(on(upper, upper_up, 0.42, s * 0.62, 0.32), 0.2 * hw, k.eye));
    }
    if gape > 0.25 {
        for s in [-1.0, 1.0] {
            let root = on(upper, upper_up, 0.86, s * 0.3, -0.15);
            p.push(limb(root, add(root, add(times(upper_up, -0.65 * hw * gape), times(upper, 0.04))), 0.06 * hw, 0.03 * hw, BONE));
        }
    }
    let flick = phase.is_some_and(|ph| (ph * 2.0 * TAU).sin() > 0.55) || w[0] > 0.6;
    if flick && gape < 0.6 {
        let root = on(ahead, up, 0.95, 0.0, -0.2);
        let tip = add(root, times(ahead, 0.3 * hl));
        p.push(limb(root, tip, 0.012, 0.01, FUR_RED));
        for s in [-1.0, 1.0] {
            p.push(limb(tip, add(add(tip, times(ahead, 0.08 * hl)), times(side, s * 0.06 * hl)), 0.009, 0.006, FUR_RED));
        }
    }
    // Horns curving back over its eyes
    if k.trim == Trim::Horns {
        for s in [-1.0, 1.0] {
            let root = on(upper, upper_up, 0.3, s * 0.5, 0.55);
            let bend = add(root, add(times(upper, -0.32 * hl), add(times(upper_up, 0.28 * hl), times(side, s * 0.1 * hl))));
            p.push(limb(root, bend, 0.24 * hw, 0.14 * hw, k.extra));
            p.push(limb(bend, add(bend, add(times(upper, -0.3 * hl), times(upper_up, 0.02))), 0.14 * hw, 0.03, k.extra));
        }
    }
    // Its jaws, the inside of its mouth showing between them as they open
    p.push(limb(sub(neck, times(ahead, 0.05 * hl)), add(neck, times(upper, hl)), 0.95 * hw, 0.42 * hw, k.back).stretched([1.0, 1.0, 0.72]));
    let jaw = sub(neck, times(up, 0.18 * hw));
    p.push(limb(jaw, add(jaw, times(lower, 0.9 * hl)), 0.75 * hw, 0.32 * hw, k.belly).stretched([1.0, 1.0, 0.6]));
    if gape > 0.2 {
        p.push(ball(add(neck, add(times(ahead, 0.5 * hl), times(up, -0.1 * hw))), 0.62 * hw * gape.min(1.0), MEAT));
    }
    // A hood flaring out behind its head as it rears, a glowing spot on either side of it
    let flare = (w[0] + w[2]).min(1.0);
    if k.trim == Trim::Hood {
        for (j, u) in [0.05, 0.1, 0.15].into_iter().enumerate() {
            let c = at(u);
            let t = unit(sub(at(u - 0.02), at(u + 0.02)), ahead);
            let s = unit(cross([0.0, 0.0, 1.0], t), side);
            let wide = radius(k, u) * (0.9 + 1.7 * flare * (1.0 - 0.25 * j as f64));
            if flare > 0.35 && j == 1 {
                for d in [-1.0, 1.0] {
                    p.push(ball(add(c, times(s, d * wide * 0.7)), 0.45 * radius(k, u), k.extra));
                }
            }
            p.push(limb(add(c, times(s, -wide)), add(c, times(s, wide)), 0.42 * radius(k, u), 0.42 * radius(k, u), k.pattern));
        }
    }
    // What runs down its back: fins, frilled wings, spikes of ice, or its pattern
    let dorsal_at = |u: f64| {
        let t = unit(sub(at((u - 0.02).max(0.0)), at((u + 0.02).min(1.0))), ahead);
        let o = unit(sub([0.0, 0.0, 1.0], times(t, t[2])), [-1.0, 0.0, 0.0]);
        (at(u), t, o)
    };
    match k.trim {
        Trim::Fin => {
            for j in 0..9 {
                let u = 0.16 + 0.085 * j as f64;
                let (c, t, o) = dorsal_at(u);
                let r = radius(k, u);
                p.push(limb(add(c, times(o, 0.7 * r)), add(add(c, times(o, 1.45 * r)), times(t, -0.35 * r)), 0.22 * r, 0.1 * r, k.extra));
            }
            let (c, t, o) = dorsal_at(0.96);
            p.push(limb(add(c, times(o, -0.9 * k.girth)), add(add(c, times(o, 1.1 * k.girth)), times(t, -0.45 * k.girth)), 0.12 * k.girth, 0.06, k.extra));
        }
        Trim::Spikes => {
            for j in 0..10 {
                let u = 0.1 + 0.075 * j as f64;
                let (c, t, o) = dorsal_at(u);
                let r = radius(k, u);
                p.push(limb(add(c, times(o, 0.75 * r)), add(add(c, times(o, 1.8 * r)), times(t, -0.5 * r)), 0.32 * r, 0.04, k.extra));
            }
        }
        Trim::Wings => {
            for (j, (u, span)) in [(0.2, 0.62), (0.43, 0.46)].into_iter().enumerate() {
                let (c, t, _) = dorsal_at(u);
                let s = unit(cross([0.0, 0.0, 1.0], t), side);
                // Beating slowly as it glides, swept up as it rears, flat out as it dives
                let beat = match phase {
                    Some(_) => 0.45 * (angle + j as f64 * 1.3).sin(),
                    None => 0.15,
                } * (1.0 - flare) + 0.7 * flare - 0.2 * w[1];
                for d in [-1.0, 1.0] {
                    let tip = add(add(c, add(times(s, d * span * beat.cos()), [0.0, 0.0, span * beat.sin()])), times(t, -0.22 * span));
                    let trail = add(add(c, times(t, -0.32 * span)), times(s, d * 0.35 * span * beat.cos()));
                    p.push(limb(c, tip, 0.03, 0.014, BONE));
                    p.push(ball(tip, 0.035, k.eye));
                    for f in [0.3, 0.55, 0.8] {
                        let edge = add(trail, times(sub(tip, trail), f));
                        p.push(limb(add(c, times(sub(tip, c), f * 0.6)), edge, 0.04, 0.03, k.extra));
                    }
                    p.push(limb(c, trail, 0.035, 0.03, k.extra));
                    p.push(limb(trail, tip, 0.03, 0.02, k.extra));
                }
            }
        }
        Trim::Hood | Trim::Horns => {}
    }
    // Its pattern: diamonds or blotches down its back, or glowing veins along its sides
    for j in 0..12 {
        let u = 0.07 + 0.072 * j as f64;
        let (c, t, o) = dorsal_at(u);
        let r = radius(k, u);
        if k.trim == Trim::Horns {
            let s = unit(cross(o, t), side);
            for d in [-1.0, 1.0] {
                p.push(limb(add(add(c, times(s, d * 0.72 * r)), times(t, 0.25 * r)), add(add(c, times(s, d * 0.72 * r)), times(t, -0.35 * r)), 0.32 * r, 0.32 * r, k.pattern));
            }
        } else if matches!(k.trim, Trim::Wings | Trim::Spikes) {
            if j % 2 == 0 {
                p.push(limb(add(c, times(t, 0.12 * r)), add(c, times(t, -0.12 * r)), 1.06 * r, 1.04 * r, k.pattern));
            }
        } else {
            p.push(ball(add(c, times(o, 0.62 * r)), 0.48 * r, k.pattern));
        }
    }
    // Its pale belly, then the rest of it
    for j in 0..PIECES {
        let (u0, u1) = (j as f64 / PIECES as f64, (j + 1) as f64 / PIECES as f64);
        let (a, ta, oa) = dorsal_at(u0.max(0.01));
        let (b, _, ob) = dorsal_at(u1);
        let _ = ta;
        let (ra, rb) = (radius(k, u0), radius(k, u1));
        p.push(limb(add(a, times(oa, -0.38 * ra)), add(b, times(ob, -0.38 * rb)), 0.74 * ra, 0.74 * rb, k.belly));
    }
    for j in 0..PIECES {
        let (u0, u1) = (j as f64 / PIECES as f64, (j + 1) as f64 / PIECES as f64);
        p.push(limb(at(u0), at(u1), radius(k, u0), radius(k, u1), k.back));
    }
    // The very tip of its tail
    let tip = at(1.0);
    p.push(ball(tip, 0.25 * k.girth, k.back));
    let _ = ellipsoid;
}

/// How far a point `p` is from the line through `from` along `along`, a unit vector, if it's ahead
/// of `from`
fn off_ray(p: Vector3<f64>, from: Vector3<f64>, along: Vector3<f64>) -> f64 {
    let to = p - from;
    let t = to.dot(&along);
    if t < 0.0 { f64::MAX } else { (to - along * t).norm() }
}

/// Of `points` along a serpent's body, in voxels, the one where a blow aimed from `eye` along
/// `along` lands: the nearest to that line, and of those about as near, the nearest to the eye
pub fn focus(points: &[Vector3<f64>], eye: Vector3<f64>, along: Vector3<f64>) -> Option<Vector3<f64>> {
    let along = along.try_normalize(1e-9)?;
    // Between each pair, so a long body's hit wherever the line crosses it
    let mut best: Option<(f64, Vector3<f64>)> = None;
    for pair in points.windows(2) {
        for f in [0.0, 0.25, 0.5, 0.75] {
            let p = pair[0].lerp(&pair[1], f);
            let score = off_ray(p, eye, along) + 0.02 * (p - eye).norm();
            if best.is_none_or(|(s, _)| score < s) {
                best = Some((score, p));
            }
        }
    }
    if let Some(&last) = points.last() {
        let score = off_ray(last, eye, along) + 0.02 * (last - eye).norm();
        if best.is_none_or(|(s, _)| score < s) {
            best = Some((score, last));
        }
    }
    best.map(|(_, p)| p)
}

/// Where a great serpent goes about: on the ground, along the top of the water and down into it,
/// or through the air
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Medium {
    Land,
    Water,
    Air,
}

/// What grows on a great serpent's head and along its back, which gives each its own look
#[derive(Clone, Copy, PartialEq, Debug)]
enum Crest {
    /// A fan of fin behind its head and down its back
    Fins,
    /// Sails of frill along its sides, spread like wings, to fly on
    Frills,
    /// Great curved horns, spurs down its back and veins glowing along its sides
    Horns,
    /// A crown of ice and spikes of it down its back
    Ice,
    /// Pale and eyeless, glowing pits where its eyes should be and lures hanging from its jaw
    Pale,
    /// Crusted with coral, knobs of it all over
    Coral,
    /// Spines down its back and lightning glowing in stripes across it
    Spines,
    /// A crown of gold-tipped horns, and plates of bone down its back
    Crown,
}

/// A great serpent: how thick its body is at its thickest, in meters across from its middle, how
/// long its head is, how many pieces its body's made of and how far apart they are, its colors,
/// what grows on it, where it goes about, and how high over the ground it flies if it does
pub struct Great {
    pub look: Look,
    pub girth: f64,
    pub head: f64,
    pub pieces: usize,
    pub spacing: f64,
    back: u8,
    pattern: u8,
    belly: u8,
    eye: u8,
    extra: u8,
    crest: Crest,
    pub medium: Medium,
    pub hover: f64,
}

const fn great(look: Look, girth: f64, head: f64, pieces: usize, spacing: f64, colors: [u8; 5], crest: Crest, medium: Medium, hover: f64) -> Great {
    Great { look, girth, head, pieces, spacing, back: colors[0], pattern: colors[1], belly: colors[2], eye: colors[3], extra: colors[4], crest, medium, hover }
}

/// The great serpents: the lesser bosses of each realm, the Water realm's three raid bosses, and the
/// ultra boss, the Coil Unending
pub const GREATS: [Great; 8] = [
    // The Meremaw: a lake serpent seventeen meters long, mossy green blotched darker, a pale belly, a
    // teal fin down its back, venom glowing in its eyes; it rises out of the water to spray venom
    great(Look::Meremaw, 0.85, 2.1, 16, 1.05, [MOSS, PINE, FUR_CREAM, GLOW_POISON, TEMPLE_TEAL], Crest::Fins, Medium::Water, 0.0),
    // The Galecoil: grey as a storm cloud, banded darker, sailing the sky on frills along its sides,
    // lightning in its eyes, coiled round its island
    great(Look::Galecoil, 0.75, 1.9, 18, 1.0, [STEEL, DARK_STEEL, LINEN, GLOW_STORM, LINEN], Crest::Frills, Medium::Air, 5.0),
    // The Veinbinder: a great constrictor black as obsidian, veined with glowing blood, bone horns
    // and spurs, crushing what it coils round
    great(Look::Veinbinder, 1.0, 2.2, 16, 1.15, [OBSIDIAN, GLOW_BLOOD, BONE, GLOW_BLOOD, BONE], Crest::Horns, Medium::Land, 0.0),
    // The Shatterfang: dark steel-grey under a crown and spikes of ice, frost in its eyes, swimming
    // under the ice and breaking up through it
    great(Look::Shatterfang, 0.95, 2.2, 16, 1.1, [DARK_STEEL, STEEL, LINEN, GLOW_FROST, ICE], Crest::Ice, Medium::Land, 0.0),
    // The Water realm's raid bosses: Hollowtide, pale and eyeless, of the sea caves, the void glowing
    // in pits where its eyes should be
    great(Look::Hollowtide, 1.25, 2.8, 20, 1.35, [LIMESTONE, TEMPLE_DARK, BONE, GLOW_VOID, GLOW_VOID], Crest::Pale, Medium::Water, 0.0),
    // Reefcoil, teal under a crust of coral, of the reefs
    great(Look::Reefcoil, 1.2, 2.6, 20, 1.3, [TEMPLE_TEAL, FLOWER_PINK, SANDSTONE, GLOW_POISON, FLOWER_PURPLE], Crest::Coral, Medium::Water, 0.0),
    // Gyremaw, black with lightning in stripes across it, steel spines down its back, of the
    // whirlpool
    great(Look::Gyremaw, 1.2, 2.7, 20, 1.3, [OBSIDIAN, GLOW_STORM, LINEN, GLOW_STORM, STEEL], Crest::Spines, Medium::Water, 0.0),
    // The Coil Unending: fifty meters long, a crown of gold-tipped horns and bone plates down its
    // back, its body taking on each realm's element in turn, see ELEMENT_SKINS: green and bone at
    // first, its venom stage, in the colors it's made in
    great(Look::Ysveth, 1.55, 3.6, 28, 1.7, [MOSS, PINE, BONE, GLOW_HOLY, GOLD], Crest::Crown, Medium::Water, 0.0),
];

/// The great serpent `look` is, if it is one
pub fn great_of(look: Look) -> Option<&'static Great> {
    GREATS.iter().find(|g| g.look == look)
}

/// What a great serpent's lair is called, in its key's third number, as no other lair's is, see
/// realms::Places::serpent_key()
pub const LAIR_KEY: i64 = -8800;

/// Each realm's great serpent, by the realm's id, see realms::REALMS, and about where its lair is
/// from the gate or the portal, in meters east and north: a lake serpent in the normal world, a sky
/// serpent coiled round an island of the sky isles, a constrictor in the dreaming realm and a frost
/// serpent under the ice of the frozen one
pub fn realm_serpent(realm: &str) -> Option<(Look, [f64; 2])> {
    match realm {
        "normal" => Some((Look::Meremaw, [-250.0, 60.0])),
        "sky" => Some((Look::Galecoil, [200.0, -220.0])),
        "dream" => Some((Look::Veinbinder, [250.0, 230.0])),
        "ice" => Some((Look::Shatterfang, [-240.0, -140.0])),
        _ => None,
    }
}

/// What each of the Water realm's raid bosses' lairs is, see realms::REALMS: a sea cave it lurks
/// deep in, a reef, or a whirlpool it circles in, the water spinning round it
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum LairKind {
    SeaCave,
    Reef,
    Whirlpool,
}

pub fn lair_kind(look: Look) -> Option<LairKind> {
    match look {
        Look::Hollowtide => Some(LairKind::SeaCave),
        Look::Reefcoil => Some(LairKind::Reef),
        Look::Gyremaw => Some(LairKind::Whirlpool),
        _ => None,
    }
}

/// Water at least `deep` meters under the sea near `near`, in meters, within `within` of it, with
/// dry land no more than `shore` meters off it to fight from, looking in rings six meters apart
pub fn water_near(settings: &TerrainSettings, near: [f64; 2], within: f64, deep: f32, shore: f32) -> Option<[f64; 2]> {
    let height = |p: [f64; 2]| worldgen::seen_from_above(settings, p[0] as f32, p[1] as f32);
    for ring in 0..=(within / 6.0) as usize {
        let count = (ring * 6).max(1);
        for i in 0..count {
            let angle = i as f64 / count as f64 * TAU;
            let at = [near[0] + angle.cos() * ring as f64 * 6.0, near[1] + angle.sin() * ring as f64 * 6.0];
            if !height(at).is_some_and(|h| h < -deep) {
                continue;
            }
            let landed = (0..8).any(|k| {
                let a = k as f64 * TAU / 8.0;
                height([at[0] + a.cos() * shore as f64, at[1] + a.sin() * shore as f64]).is_some_and(|h| h > 1.5)
            });
            if landed {
                return Some(at);
            }
        }
    }
    None
}

/// Where great serpent `look`'s lair is about `near`, in meters of `settings`' landscape: in water
/// near a shore for what swims or lies under the ice, on dry land for the rest. None if there's
/// nowhere like that near.
pub fn lair_spot(look: Look, settings: &TerrainSettings, near: [f64; 2]) -> Option<[f64; 2]> {
    let g = great_of(look)?;
    let wet = g.medium == Medium::Water || look == Look::Shatterfang;
    if wet {
        water_near(settings, near, 220.0, 1.5, 24.0)
    } else {
        worldgen::dry_land_near(settings, [near[0] as f32, near[1] as f32], 160.0, 7.0, &|_| true).map(|p| [p[0] as f64, p[1] as f64])
    }
}

/// How far through the Coil Unending's health it goes into each of its later stages, taking on
/// each realm's element in turn, see ULTRA_ELEMENTS: circling half under the water spitting venom,
/// then coiling and striking under a storm, then calling up its brood and spraying blood, and at the
/// last bringing frost down out of the sky, enraged
pub const ULTRA_STAGES: [f64; 3] = [0.75, 0.5, 0.25];

/// Which of its stages the Coil Unending is in, `vitality` of its health left: 0 to 3
pub fn ultra_stage(vitality: f64) -> u8 {
    ULTRA_STAGES.iter().filter(|&&s| vitality < s).count() as u8
}

/// The serpents the Coil Unending calls up in each stage, of that stage's element
pub const ULTRA_BROOD: [Look; 4] = [Look::ReedViper, Look::GaleSerpent, Look::BloodAdder, Look::DriftSerpent];

/// What the Coil Unending's lair is called in Free Roam, in its key's third number; in the Water
/// realm it's the realm's finale's, see realms::Places::finale_key()
pub const ULTRA_KEY: i64 = -8801;

/// What the unique weapon the Coil Unending leaves is called, see Game::fang()
pub const FANG: &str = "Fang of Ysveth";

/// How far from where Free Roam starts the Coil Unending's lair is, in meters, at least
pub const ULTRA_FAR: f64 = 1500.0;

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^ (x >> 16)
}

/// Where the Coil Unending keeps its lair in Free Roam's landscape `settings`, about 1.85 km from
/// `start`, in meters, the way round the landscape's seed says: in deep water near a shore, or on
/// dry land if there's none that way. Worked out once for each landscape.
pub fn free_roam_ultra(settings: &TerrainSettings, start: [f64; 2]) -> Option<[f64; 2]> {
    static FOUND: std::sync::Mutex<Option<(u32, [i64; 2], Option<[f64; 2]>)>> = std::sync::Mutex::new(None);
    let key = [start[0].round() as i64, start[1].round() as i64];
    if let Some((seed, at, found)) = FOUND.lock().ok().and_then(|f| *f)
        && seed == settings.seed
        && at == key
    {
        return found;
    }
    let way = hash(settings.seed ^ 0x51e7_c011) as f64 / u32::MAX as f64 * TAU;
    let far = |spot: &[f64; 2]| (spot[0] - start[0]).hypot(spot[1] - start[1]) >= ULTRA_FAR;
    let found = (0..8)
        .find_map(|k| {
            let a = way + k as f64 * 0.8;
            let near = [start[0] + a.cos() * 1850.0, start[1] + a.sin() * 1850.0];
            water_near(settings, near, 300.0, 2.5, 40.0).filter(far)
        })
        .or_else(|| {
            let near = [start[0] + way.cos() * 1850.0, start[1] + way.sin() * 1850.0];
            worldgen::dry_land_near(settings, [near[0] as f32, near[1] as f32], 300.0, 9.0, &|_| true).map(|p| [p[0] as f64, p[1] as f64]).filter(far)
        });
    if let Ok(mut f) = FOUND.lock() {
        *f = Some((settings.seed, key, found));
    }
    found
}

/// The omens on the way to the Coil Unending's lair at `lair` from `start`, both in meters: shed
/// skins and the bones of what it ate lying about, more of them nearer it, with which way each lies
pub fn omens(seed: u32, start: [f64; 2], lair: [f64; 2]) -> Vec<(Look, [f64; 2], f64)> {
    let (dx, dy) = (lair[0] - start[0], lair[1] - start[1]);
    let length = dx.hypot(dy).max(1.0);
    let (ax, ay) = (dx / length, dy / length);
    let mut out = Vec::new();
    // From 1.1 km out from where it lies, closer together the nearer they are
    let mut along = (length - 1100.0).max(200.0);
    let mut k = 0u32;
    while along < length - 40.0 {
        let roll = hash(seed ^ hash(k ^ 0x0e3e));
        let side = ((roll & 0xff) as f64 / 255.0 - 0.5) * 50.0;
        let at = [start[0] + ax * along - ay * side, start[1] + ay * along + ax * side];
        let look = if roll >> 8 & 3 == 0 { Look::SerpentBones } else { Look::ShedSkin };
        out.push((look, at, (roll >> 16) as f64 / 65535.0 * TAU));
        let left = length - along;
        along += 25.0 + 70.0 * (left / 1100.0).min(1.0);
        k += 1;
    }
    out
}

/// What `a`'s attacks are made of now: the Coil Unending's element of the stage it's in, the rest
/// their own
pub fn element_now(a: &Animal) -> Option<Element> {
    match a.coils() {
        Some(coils) if a.look.is_ultra() => Some(ULTRA_ELEMENTS[coils.element.min(ULTRA_ELEMENTS.len() - 1)]),
        _ => a.look.element(),
    }
}

/// How dark the sky's getting over the Coil Unending's lair, 0 to 1, for someone `meters` from it:
/// from 600 m off, darkest within 150
pub fn gloom(meters: f64) -> f64 {
    smooth((600.0 - meters) / 450.0)
}

/// The elements the Coil Unending's body takes on in turn as its health drops, one for each of the
/// realms the Water realm comes after: venom, storm, blood and frost
pub const ULTRA_ELEMENTS: [Element; 4] = [Element::Poison, Element::Storm, Element::Blood, Element::Frost];
/// The skin it takes on for each of them, the color all of it's turned towards, light and dark as
/// it was, and how far, see AnimalInstance::skinned(): none for venom, the colors it's made in, and
/// slate blue, deep red and pale ice after. A tint of the same models rather than models of its own
/// in each, so a body fifty meters long in four elements costs no more of the graphics card's
/// memory than one.
const ELEMENT_SKINS: [[u8; 4]; 4] = [[0, 0, 0, 0], [96, 122, 178, 190], [150, 22, 30, 200], [168, 214, 240, 190]];

/// How many models the great serpents add after the animals' own: a head, a jaw and a piece of
/// body for each
pub const PART_MODELS: usize = GREATS.len() * 3;

/// The skin of element `element` of ULTRA_ELEMENTS, packed as traverse.comp reads it
fn skin_of(element: usize) -> u32 {
    u32::from_le_bytes(ELEMENT_SKINS[element.min(ELEMENT_SKINS.len() - 1)])
}

/// One of the great serpents' models: where it's put in the world from the point it's made round,
/// and how far from that point any of it reaches, in meters
struct Shape {
    anchor: Vector3<f64>,
    furthest: f64,
}

/// The shapes of a great serpent's head, about where its neck joins it, facing +x
fn skull(g: &Great, skin: [u8; 5]) -> Vec<Part> {
    let [back, pattern, _, eye, extra] = skin;
    let (h, r) = (g.head, g.girth);
    let mut p = Vec::new();
    // Eyes under heavy brows, or glowing pits where its eyes should be
    for s in [-1.0, 1.0] {
        p.push(ball([0.42 * h, s * 0.6 * r, 0.42 * r], 0.17 * r, eye));
        p.push(limb([0.3 * h, s * 0.62 * r, 0.6 * r], [0.55 * h, s * 0.5 * r, 0.55 * r], 0.14 * r, 0.1 * r, pattern));
        // Fangs, hidden in its jaw while it's shut
        p.push(limb([0.84 * h, s * 0.27 * r, -0.22 * r], [0.88 * h, s * 0.25 * r, -0.62 * r], 0.08 * r, 0.03 * r, BONE));
    }
    match g.crest {
        Crest::Fins => {
            for (k, a) in [-0.9, -0.45, 0.0, 0.45, 0.9].into_iter().enumerate() {
                let tip = [-0.25 * h - 0.1 * h * (1.0 - (a as f64).abs()), (a as f64) * 1.3 * r, 0.9 * r + 0.5 * r * (1.0 - (a as f64).abs())];
                p.push(limb([0.1 * h, a * 0.5 * r, 0.55 * r], tip, 0.12 * r, 0.05 * r, if k % 2 == 0 { extra } else { pattern }));
            }
        }
        Crest::Frills => {
            for s in [-1.0, 1.0] {
                for k in 0..4 {
                    let f = k as f64 / 3.0;
                    let tip = [-0.1 * h - 0.25 * h * f, s * (1.4 + 0.5 * f) * r, 0.3 * r + 0.6 * r * (1.0 - f)];
                    p.push(limb([0.25 * h - 0.2 * h * f, s * 0.55 * r, 0.2 * r], tip, 0.1 * r, 0.04 * r, if k == 0 { eye } else { extra }));
                }
            }
        }
        Crest::Horns | Crest::Crown => {
            for s in [-1.0, 1.0] {
                let root = [0.25 * h, s * 0.5 * r, 0.6 * r];
                let bend = [-0.15 * h, s * 0.95 * r, 1.25 * r];
                let tip = [-0.5 * h, s * 0.85 * r, 1.05 * r];
                p.push(limb(root, bend, 0.22 * r, 0.15 * r, BONE));
                p.push(limb(bend, tip, 0.15 * r, 0.04 * r, if g.crest == Crest::Crown { extra } else { BONE }));
                if g.crest == Crest::Crown {
                    p.push(limb([0.05 * h, s * 0.3 * r, 0.75 * r], [-0.25 * h, s * 0.35 * r, 1.45 * r], 0.12 * r, 0.03 * r, extra));
                }
            }
        }
        Crest::Ice => {
            for (k, s) in [-1.0, -0.5, 0.0, 0.5, 1.0].into_iter().enumerate() {
                let tall = 0.8 + 0.5 * (1.0 - (s as f64).abs());
                p.push(limb([0.2 * h - 0.05 * k as f64 * h, s * 0.45 * r, 0.6 * r], [-0.1 * h, s * 0.7 * r, (0.6 + tall) * r], 0.16 * r, 0.03, extra));
            }
        }
        Crest::Pale => {
            // Lures hanging from under its jaw, their tips glowing
            for s in [-1.0, 1.0] {
                p.push(limb([0.7 * h, s * 0.45 * r, -0.2 * r], [0.75 * h, s * 0.65 * r, -1.1 * r], 0.06 * r, 0.04 * r, pattern));
                p.push(ball([0.75 * h, s * 0.65 * r, -1.15 * r], 0.12 * r, extra));
            }
        }
        Crest::Coral => {
            for (x, y, z) in [(0.2, 0.3, 0.75), (0.0, -0.4, 0.7), (-0.15, 0.1, 0.8), (0.45, -0.15, 0.62), (0.1, 0.55, 0.5)] {
                p.push(ball([x * h, y * r, z * r], 0.24 * r, if y > 0.0 { pattern } else { extra }));
            }
        }
        Crest::Spines => {
            for k in 0..5 {
                let x = 0.35 * h - 0.17 * h * k as f64;
                p.push(limb([x, 0.0, 0.6 * r], [x - 0.25 * h, 0.0, (1.3 - 0.08 * k as f64) * r], 0.1 * r, 0.03, extra));
            }
        }
    }
    // The roof of its mouth, showing as its jaw drops
    p.push(ellipsoid([0.5 * h, 0.0, -0.3 * r], [0.38 * h, 0.5 * r, 0.1 * r], MEAT));
    // Its snout and the dome of its skull
    p.push(limb([0.55 * h, 0.0, 0.0], [h, 0.0, -0.08 * r], 0.62 * r, 0.36 * r, back).stretched([1.0, 1.05, 0.78]));
    p.push(limb([0.0, 0.0, 0.0], [0.62 * h, 0.0, 0.1 * r], 0.95 * r, 0.7 * r, back).stretched([1.0, 1.08, 0.82]));
    p
}

/// The shapes of a great serpent's lower jaw, hinged about the point it's made round, facing +x
fn jaw(g: &Great, skin: [u8; 5]) -> Vec<Part> {
    let [_, pattern, belly, _, _] = skin;
    let (h, r) = (g.head, g.girth);
    let mut p = Vec::new();
    for s in [-1.0, 1.0] {
        p.push(limb([0.78 * h, s * 0.3 * r, -0.3 * r], [0.8 * h, s * 0.3 * r, 0.08 * r], 0.07 * r, 0.03 * r, BONE));
    }
    p.push(limb([0.15 * h, 0.0, -0.25 * r], [0.65 * h, 0.0, -0.22 * r], 0.12 * r, 0.1 * r, FUR_RED));
    p.push(limb([0.0, 0.0, -0.38 * r], [0.9 * h, 0.0, -0.42 * r], 0.78 * r, 0.36 * r, belly).stretched([1.0, 1.0, 0.5]));
    p.push(limb([0.0, 0.0, -0.3 * r], [0.75 * h, 0.0, -0.32 * r], 0.7 * r, 0.3 * r, pattern).stretched([1.0, 1.02, 0.5]));
    p
}

/// The shapes of a piece of a great serpent's body, its middle at the point it's made round, along
/// x, long enough to meet the pieces either side of it however the body bends
fn piece(g: &Great, skin: [u8; 5]) -> Vec<Part> {
    let [back, pattern, belly, eye, extra] = skin;
    let r = g.girth;
    // Its rounded ends overlap the pieces either side, so it needn't be any longer than this
    let half = 0.5 * g.spacing;
    let mut p = Vec::new();
    match g.crest {
        Crest::Fins => {
            p.push(limb([0.3 * half, 0.0, 0.7 * r], [-0.6 * half, 0.0, 1.55 * r], 0.12 * r, 0.05 * r, extra));
            p.push(limb([-0.5 * half, 0.0, 0.7 * r], [-1.1 * half, 0.0, 1.3 * r], 0.1 * r, 0.04 * r, extra));
            for s in [-1.0, 1.0] {
                p.push(ball([0.2 * half, s * 0.45 * r, 0.55 * r], 0.32 * r, pattern));
            }
        }
        Crest::Frills => {
            for s in [-1.0, 1.0] {
                p.push(limb([0.4 * half, s * 0.6 * r, 0.15 * r], [-0.5 * half, s * 1.9 * r, 0.45 * r], 0.08 * r, 0.05 * r, eye));
                p.push(limb([-0.6 * half, s * 0.6 * r, 0.1 * r], [-0.5 * half, s * 1.85 * r, 0.42 * r], 0.12 * r, 0.1 * r, extra));
                p.push(limb([0.3 * half, s * 0.6 * r, 0.1 * r], [-0.55 * half, s * 1.3 * r, 0.3 * r], 0.14 * r, 0.12 * r, extra));
            }
            p.push(limb([0.12 * half, 0.0, 0.0], [-0.12 * half, 0.0, 0.0], 1.04 * r, 1.04 * r, pattern));
        }
        Crest::Horns | Crest::Crown => {
            p.push(limb([0.2 * half, 0.0, 0.75 * r], [-0.5 * half, 0.0, 1.35 * r], 0.2 * r, 0.04 * r, if g.crest == Crest::Crown { BONE } else { extra }));
            if g.crest == Crest::Crown {
                p.push(limb([0.7 * half, 0.0, 0.85 * r], [-0.7 * half, 0.0, 0.85 * r], 0.32 * r, 0.32 * r, BONE).stretched([1.0, 1.2, 0.5]));
            }
            for s in [-1.0, 1.0] {
                p.push(limb([0.9 * half, s * 0.78 * r, 0.1 * r], [-0.9 * half, s * 0.78 * r, -0.05 * r], 0.24 * r, 0.24 * r, pattern));
            }
        }
        Crest::Ice => {
            p.push(limb([0.3 * half, 0.15 * r, 0.7 * r], [-0.3 * half, 0.25 * r, 1.75 * r], 0.22 * r, 0.03, extra));
            p.push(limb([-0.4 * half, -0.2 * r, 0.7 * r], [-0.9 * half, -0.3 * r, 1.45 * r], 0.18 * r, 0.03, extra));
            p.push(limb([0.12 * half, 0.0, 0.0], [-0.12 * half, 0.0, 0.0], 1.04 * r, 1.04 * r, pattern));
            for s in [-1.0, 1.0] {
                p.push(ball([0.0, s * 0.85 * r, 0.2 * r], 0.14 * r, eye));
            }
        }
        Crest::Pale => {
            for (k, s) in [-1.0, 1.0].into_iter().enumerate() {
                p.push(ball([0.5 * half - k as f64 * half, s * 0.9 * r, 0.0], 0.13 * r, extra));
                p.push(ball([-0.3 * half, s * 0.5 * r, 0.6 * r], 0.36 * r, pattern));
            }
        }
        Crest::Coral => {
            for (x, y, z, big) in [(0.4, 0.3, 0.85, 0.3), (-0.3, -0.45, 0.75, 0.34), (-0.8, 0.5, 0.6, 0.26), (0.7, -0.75, 0.4, 0.24), (0.0, 0.9, 0.25, 0.22)] {
                p.push(ball([x * half, y * r, z * r], big * r, if x > 0.0 { pattern } else { extra }));
            }
        }
        Crest::Spines => {
            p.push(limb([0.2 * half, 0.0, 0.7 * r], [-0.55 * half, 0.0, 1.6 * r], 0.12 * r, 0.03, extra));
            p.push(limb([0.12 * half, 0.0, 0.0], [-0.12 * half, 0.0, 0.0], 1.04 * r, 1.04 * r, pattern));
            p.push(limb([0.82 * half, 0.0, 0.0], [0.7 * half, 0.0, 0.0], 1.03 * r, 1.03 * r, pattern));
        }
    }
    p.push(limb([-half, 0.0, -0.36 * r], [half, 0.0, -0.36 * r], 0.74 * r, 0.74 * r, belly));
    p.push(limb([-half, 0.0, 0.0], [half, 0.0, 0.0], r, r, back));
    p
}

/// The great serpents' models and shapes, made once
fn parts_made() -> &'static (Vec<Model>, Vec<Shape>) {
    static MADE: OnceLock<(Vec<Model>, Vec<Shape>)> = OnceLock::new();
    MADE.get_or_init(|| {
        let mut sets: Vec<Vec<Part>> = Vec::new();
        for g in &GREATS {
            let skin = [g.back, g.pattern, g.belly, g.eye, g.extra];
            sets.push(skull(g, skin));
            sets.push(jaw(g, skin));
            sets.push(piece(g, skin));
        }
        let made: Vec<(Model, Vector3<f64>)> = std::thread::scope(|scope| {
            let handles: Vec<_> = sets.iter().map(|parts| scope.spawn(move || voxelize(parts, crate::animals::MODEL_RES))).collect();
            handles.into_iter().map(|h| h.join().expect("making a serpent's model failed")).collect()
        });
        let shapes = made
            .iter()
            .map(|(model, anchor)| {
                let size = Vector3::new(model.size[0] as f64, model.size[1] as f64, model.size[2] as f64) / crate::animals::MODEL_RES;
                let lo = anchor - Vector3::new(size.x * 0.5, size.y * 0.5, 0.0);
                let corners = (0..8).map(|k| Vector3::new(if k & 1 == 0 { lo.x } else { lo.x + size.x }, if k & 2 == 0 { lo.y } else { lo.y + size.y }, if k & 4 == 0 { lo.z } else { lo.z + size.z }));
                Shape { anchor: *anchor, furthest: corners.map(|c| c.norm()).fold(0.0, f64::max) }
            })
            .collect();
        (made.into_iter().map(|(model, _)| model).collect(), shapes)
    })
}

/// The great serpents' models, to go after the animals' own, see animals::animal_models()
pub fn part_models() -> Vec<Model> {
    parts_made().0.iter().map(|m| Model { size: m.size, voxels: m.voxels.clone() }).collect()
}

/// Which of the great serpents' models is a part of great serpent `g`: 0 its head, 1 its jaw, 2 a
/// piece of its body
fn part_index(g: usize, part: usize) -> usize {
    g * 3 + part
}

/// The instance drawing part `part` of great serpent `g` with the point it's made round at `at`, in
/// voxels, turned by `rotation` and `size` times as big
fn place(g: usize, part: usize, at: Vector3<f64>, rotation: UnitQuaternion<f64>, size: f64, vpm: f64) -> AnimalInstance {
    let index = part_index(g, part);
    let shape = &parts_made().1[index];
    let position = at + rotation * shape.anchor * size * vpm;
    let model = (crate::animals::ANIMAL_MODELS - PART_MODELS + index) as u32;
    AnimalInstance::new(model, position, rotation, vpm / crate::animals::MODEL_RES * size, (shape.furthest + shape.anchor.norm()) * size * vpm, MOVING)
}

/// Turned to face along `way`, round and up, never rolled over
fn along(way: Vector3<f64>) -> UnitQuaternion<f64> {
    let yaw = way.y.atan2(way.x);
    let pitch = -(way.z / way.norm().max(1e-9)).clamp(-1.0, 1.0).asin();
    UnitQuaternion::from_axis_angle(&Vector3::z_axis(), yaw) * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), pitch)
}

/// How much thinner than its thickest a great serpent's body is `k` pieces back of `n`: a little
/// thinner at its neck, thickest a third of the way back, tapering to its tail
fn taper(k: usize, n: usize) -> f64 {
    let u = k as f64 / n as f64;
    (0.86 + 0.14 * smooth(u / 0.2)) * (1.0 - 0.72 * smooth((u - 0.35) / 0.65))
}

/// How high the middle of a piece of a great serpent's body `r` voxels thick, near height `z`,
/// rests at `x`, `y`, in voxels: on the ground, or for what swims, half in the water, its back out
/// of it
#[allow(clippy::too_many_arguments)]
fn rest_height(world: &World, water: WaterLevel, medium: Medium, x: f64, y: f64, z: f64, r: f64) -> f64 {
    let vpm = world.voxels_per_meter as f64;
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    let top = (z + 3.0 * vpm) as i64;
    let ground = (top - (12.0 * vpm) as i64..=top).rev().find(|&k| !is_passable(world.material_at(xi, yi, k))).map_or(z - 12.0 * vpm, |k| k as f64 + 1.0);
    if medium == Medium::Water { (ground + r).max(water(x, y)) } else { ground + r }
}

/// A great serpent's long body: its head, where its neck joins it, and each piece of its body
/// behind it, following the one in front of it, in voxels. What it's up to is said by how high it
/// rears its head, how wide its jaw gapes, how far under the ground or the water its head's gone,
/// and its tail raised to slam down, eased towards each, see Boss
pub struct Coils {
    /// Which of GREATS
    pub great: usize,
    pub head: Vector3<f64>,
    /// Which way its head faces
    pub forward: Vector3<f64>,
    pub points: Vec<Vector3<f64>>,
    /// Meters its head's reared up off the ground, and where it's going
    pub lift: f64,
    pub lift_to: f64,
    /// How wide its jaw gapes, 0 to 1
    pub gape: f64,
    pub gape_to: f64,
    /// Meters its head's gone under the ground or the water
    pub dive: f64,
    pub dive_to: f64,
    /// Where its tail's coming down, and how far it's got: rising from 0 to 1, slamming from 1 to 2
    pub tail: Option<(Vector3<f64>, f64)>,
    /// What its head looks down at, if anything
    pub look_at: Option<Vector3<f64>>,
    /// For the Coil Unending, the element its body's taking on, of ULTRA_ELEMENTS, the one it had,
    /// and how many pieces back from its head it's taken it on so far, rippling down to its tail
    pub element: usize,
    pub was: usize,
    pub shifted: f64,
    /// Meters the whole of it's sunk down into the ground or the water, as it lies in wait under
    /// it or dies, and where it's going
    pub sunk: f64,
    pub sunk_to: f64,
    pitch: f64,
    laid: bool,
    /// Which of its pieces are under the ground or the water
    under: Vec<bool>,
}

/// How many pieces back from its head a great serpent's neck bends up as it rears
const NECK: f64 = 5.0;

impl Coils {
    pub fn new(look: Look) -> Option<Coils> {
        let great = GREATS.iter().position(|g| g.look == look)?;
        let n = GREATS[great].pieces;
        Some(Coils {
            great,
            head: Vector3::zeros(),
            forward: Vector3::x(),
            points: vec![Vector3::zeros(); n],
            lift: 0.0,
            lift_to: 0.0,
            gape: 0.0,
            gape_to: 0.0,
            dive: 0.0,
            dive_to: 0.0,
            tail: None,
            look_at: None,
            element: 0,
            was: 0,
            shifted: n as f64,
            sunk: 0.0,
            sunk_to: 0.0,
            pitch: 0.0,
            laid: false,
            under: vec![false; n],
        })
    }

    pub fn kind(&self) -> &'static Great {
        &GREATS[self.great]
    }

    /// The world moved over by `shift` voxels
    pub fn recenter(&mut self, shift: Vector3<f64>) {
        self.head -= shift;
        for p in &mut self.points {
            *p -= shift;
        }
        if let Some((at, _)) = &mut self.tail {
            *at -= shift;
        }
        self.look_at = self.look_at.map(|p| p - shift);
    }

    /// Where its snout is, in voxels: what it spits and sprays from
    pub fn snout(&self, vpm: f64) -> Vector3<f64> {
        let g = self.kind();
        self.head + self.forward * 0.95 * g.head * vpm - Vector3::new(0.0, 0.0, 0.2 * g.girth * vpm)
    }

    /// Which way and how far whoever's standing `tall` meters tall with their middle at `at`, in
    /// voxels, has to be shoved out sideways to be out of its head and body, in voxels; nothing if
    /// they're not in it. Out to its side, if they're right under the middle of it.
    pub fn shove(&self, at: Vector3<f64>, tall: f64, vpm: f64) -> Vector3<f64> {
        let g = self.kind();
        let down = Vector3::new(0.0, 0.0, self.sunk * vpm);
        let n = self.points.len();
        // Kept further from its head, so its face doesn't fill the whole view
        let head = [(self.head + self.forward * 0.45 * g.head * vpm, 1.1 * g.girth, 1.0), (self.head, g.girth, 1.0)];
        let body = self.points.iter().enumerate().map(|(k, p)| (*p, g.girth * taper(k, n), 0.35));
        let side = Vector2::new(-self.forward.y, self.forward.x).try_normalize(1e-6).unwrap_or_else(Vector2::x);
        let mut push = Vector3::zeros();
        for (p, r, spare) in head.into_iter().chain(body) {
            let off = at - (p - down);
            let reach = (r + spare) * vpm;
            if off.z.abs() > r * vpm + 0.5 * tall * vpm {
                continue;
            }
            let d = off.xy().norm();
            if d < reach {
                let away = off.xy().try_normalize(1e-6).unwrap_or(side);
                push += Vector3::new(away.x, away.y, 0.0) * (reach - d);
            }
        }
        push
    }

    /// Where along it to try hitting, in voxels: its head, and every piece of its body
    pub fn hit_points(&self, vpm: f64) -> Vec<Vector3<f64>> {
        let g = self.kind();
        let down = Vector3::new(0.0, 0.0, self.sunk * vpm);
        let mut out = vec![self.head + self.forward * 0.55 * g.head * vpm - down, self.head - down];
        out.extend(self.points.iter().map(|p| p - down));
        out
    }

    /// Follows its head, its neck joint over `feet`, in voxels, the head facing `heading`, its
    /// whole head moved on by `on` meters and up by `up`, over `dt` seconds
    pub fn update(&mut self, feet: Vector3<f64>, heading: f64, on: f64, up: f64, world: &World, water: WaterLevel, dt: f64) {
        let g = self.kind();
        let vpm = world.voxels_per_meter as f64;
        let ease = |v: &mut f64, to: f64, rate: f64| *v += (to - *v) * (1.0 - (-rate * dt).exp());
        ease(&mut self.lift, self.lift_to, 5.0);
        ease(&mut self.gape, self.gape_to, 9.0);
        ease(&mut self.dive, self.dive_to, 2.6);
        let rising = self.sunk_to < self.sunk;
        ease(&mut self.sunk, self.sunk_to, if rising { 2.2 } else { 1.2 });
        // The Coil Unending's new element rippling down its body
        self.shifted = (self.shifted + 9.0 * dt).min(g.pieces as f64 + 1.0);
        let flat = Vector3::new(heading.cos(), heading.sin(), 0.0);
        self.head = feet + flat * on * vpm + Vector3::new(0.0, 0.0, (g.girth * 0.95 + g.hover + self.lift - self.dive + up) * vpm);
        // Looking down at what it's after, or along the way it's going
        let want = self.look_at.map_or(-0.08 - 0.03 * self.lift, |p| {
            let off = p - self.head;
            (-off.z).atan2(off.xy().norm().max(1.0)).clamp(-0.5, 0.75)
        });
        ease(&mut self.pitch, want, 4.0);
        self.forward = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), heading) * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.pitch) * Vector3::x();
        let n = self.points.len();
        // Laid out straight behind its head on the ground the first time, or level with its head in
        // the air for what flies, so it trails behind it rather than hanging down where there's no
        // ground under it, between the Sky Isles
        if !self.laid {
            self.laid = true;
            for k in 0..n {
                let mut p = self.head - flat * (k as f64 + 0.8) * g.spacing * vpm;
                let rest = rest_height(world, water, g.medium, p.x, p.y, feet.z, g.girth * taper(k, n) * vpm);
                p.z = if g.medium == Medium::Air { self.head.z.max(rest) } else { rest + g.hover * vpm };
                self.points[k] = p;
            }
        }
        let mut prev = self.head;
        let mut under_ahead = self.dive > g.girth * 0.8;
        for k in 0..n {
            let r = g.girth * taper(k, n);
            let gap = if k == 0 { 0.8 } else { 1.0 } * g.spacing * taper(k, n).max(0.4) * vpm;
            let way = (self.points[k] - prev).try_normalize(1e-6).unwrap_or(-flat);
            let mut p = prev + way * gap;
            let rest = rest_height(world, water, g.medium, p.x, p.y, p.z, r * vpm);
            // What's under the ground follows where its head went down; what flies follows its head
            // through the air; the rest lies on the ground or the water, its neck rearing up
            if g.medium == Medium::Air {
                p.z = p.z.max(rest);
            } else if !under_ahead {
                let neck = (1.0 - (k as f64 + 1.0) / NECK).max(0.0).powi(2);
                let target = rest + (self.lift.max(0.0) * neck) * vpm;
                p.z += (target - p.z) * (1.0 - (-10.0 * dt).exp());
                p.z = p.z.max(rest - 0.3 * r * vpm);
                p = prev + (p - prev).try_normalize(1e-6).unwrap_or(-flat) * gap;
            }
            self.under[k] = p.z < rest - 0.7 * r * vpm;
            under_ahead = self.under[k];
            self.points[k] = p;
            prev = p;
        }
        // Its tail raised high in an arc and brought down on what it's after
        if let Some((target, k)) = self.tail {
            let tail = (n / 3).max(4).min(n - 1);
            let base = self.points[n - 1 - tail];
            let rise = smooth(k.min(1.0));
            let fall = smooth((k - 1.0).clamp(0.0, 1.0));
            let high = (3.0 + 2.0 * g.girth) * vpm;
            let over = base.lerp(&target, 0.6) + Vector3::new(0.0, 0.0, high);
            let down = target + Vector3::new(0.0, 0.0, g.girth * 0.5 * vpm);
            let tip = self.points[n - 1].lerp(&over, rise).lerp(&down, fall);
            let control = base.lerp(&tip, 0.5) + Vector3::new(0.0, 0.0, high * 0.45 * (1.0 - fall * 0.7));
            for j in 1..=tail {
                let s = j as f64 / tail as f64;
                let curve = base * (1.0 - s) * (1.0 - s) + control * 2.0 * s * (1.0 - s) + tip * s * s;
                let i = n - 1 - tail + j;
                self.points[i] = self.points[i].lerp(&curve, rise);
            }
        }
    }

    /// Its head, its jaw and every piece of its body, to draw, tinted as weapons have left it, see
    /// animals::TINT_NONE
    pub fn instances(&self, vpm: f64, tint: u32, out: &mut Vec<AnimalInstance>) {
        let from = out.len();
        self.pieces(vpm, out);
        for i in &mut out[from..] {
            *i = i.tinted(tint);
        }
    }

    fn pieces(&self, vpm: f64, out: &mut Vec<AnimalInstance>) {
        let g = self.kind();
        let turn = along(self.forward);
        let down = Vector3::new(0.0, 0.0, self.sunk * vpm);
        // The Coil Unending's skin, its head taking on its new element first and the rest of it
        // after, rippling down to its tail
        let ultra = g.look == Look::Ysveth;
        let skin = |k: f64| if !ultra { 0 } else if k < self.shifted { skin_of(self.element) } else { skin_of(self.was) };
        out.push(place(self.great, 0, self.head - down, turn, 1.0, vpm).skinned(skin(-1.0)));
        let hinge = self.head - down + turn * Vector3::new(0.0, 0.0, -0.18 * g.girth) * vpm;
        out.push(place(self.great, 1, hinge, turn * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), 0.75 * self.gape), 1.0, vpm).skinned(skin(-1.0)));
        let n = self.points.len();
        for k in 0..n {
            let ahead = if k == 0 { self.head } else { self.points[k - 1] };
            let behind = if k + 1 < n { self.points[k + 1] } else { self.points[k] * 2.0 - ahead };
            out.push(place(self.great, 2, self.points[k] - down, along(ahead - behind), taper(k, n), vpm).skinned(skin(k as f64)));
        }
    }

    /// The Coil Unending taking on element `element` of ULTRA_ELEMENTS, rippling down its body from
    /// its head
    pub fn shift(&mut self, element: usize) {
        if element != self.element {
            self.was = self.element;
            self.element = element;
            self.shifted = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Slithering, every part of it follows the way its head went: moved on by its stride's worth
    /// of its walk for each stride it goes, the body stays put on the ground, rather than waving
    /// about over it
    #[test]
    fn a_slither_leaves_its_body_where_its_head_went() {
        for look in SERPENTS {
            let k = kind(look).unwrap();
            let step = 0.05;
            for u in [0.3, 0.5, 0.7, 0.9] {
                let here = spine(k, u, Some(0.2), [0.0; 3]);
                // Gone `step` of a stride further on, the part that's now where this was is a little
                // further back along it
                let on = spine(k, u + step * stride(k) / k.length, Some(0.2 + step), [0.0; 3]);
                let moved = [on[0] + step * stride(k), on[1]];
                let sideways = (moved[1] - here[1]).abs();
                assert!(sideways < 0.03 * k.length, "{look:?} at {u}: it slid {sideways:.3} m sideways");
            }
        }
    }

    /// Rearing to strike, its head's well up off the ground, high over its coils, and its body's
    /// gathered in under it; lunging, its head's out past where it was
    #[test]
    fn it_rears_up_coiled_and_lunges_out() {
        for look in SERPENTS {
            let k = kind(look).unwrap();
            let head = |w: [f64; 3]| spine(k, 0.0, None, w);
            let reared = head([1.0, 0.0, 0.0]);
            assert!(reared[2] > 0.2 * k.length, "{look:?} rears {:.2} m", reared[2]);
            let tail = spine(k, 1.0, None, [1.0, 0.0, 0.0]);
            assert!(tail[0].abs() < 0.3 * k.length && tail[1].abs() < 0.3 * k.length, "{look:?}'s tail's at {tail:?}, not coiled");
            assert!(head([0.0, 1.0, 0.0])[0] > head([0.0; 3])[0] + 0.1 * k.length);
            assert!(head([0.0, 0.0, 1.0])[2] > reared[2]);
        }
    }

    /// A flat field of dirt 192 m across at 8 voxels a meter, its ground 4 m up, as combat's tests
    /// have
    fn field() -> World {
        let size = [1536u32, 1536, 128];
        let (tx, ty) = (size[0] as usize / 4, size[1] as usize / 4);
        let mut occupancy = vec![0u128; tx * ty * (size[2] as usize / 8)];
        occupancy[..tx * ty * 4].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, DIRT);
        world.voxels_per_meter = 8;
        world
    }

    /// Hurt past each of its stages, the Coil Unending goes into each in turn: rearing up with a
    /// roar to say so, its body taking on that stage's element from its head to its tail, and at
    /// the last enraged. Its moves are its stage's.
    #[test]
    fn the_coil_unending_goes_through_its_stages() {
        assert_eq!([1.0, 0.76, 0.74, 0.49, 0.26, 0.24, 0.01].map(ultra_stage), [0, 0, 1, 2, 2, 3, 3]);
        let world = field();
        let vpm = 8.0;
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let home = Vector3::new(96.0, 96.0, 4.0) * vpm;
        let mut ultra = Animal::boss(Look::Ysveth, home, [0, 0, ULTRA_KEY], 7);
        let player = Some(home + Vector3::new(12.0, 0.0, 0.0) * vpm);
        let dt = 1.0 / 30.0;
        for _ in 0..90 {
            ultra.update(&world, &water, player, dt);
            ultra.boss.as_mut().unwrap().deeds.clear();
        }
        assert!(ultra.boss.as_ref().unwrap().engaged, "it turns on the player");
        assert_eq!(element_now(&ultra), Some(Element::Poison));
        for (fraction, stage) in [(0.7, 1u8), (0.45, 2), (0.2, 3)] {
            ultra.health = ultra.max_health * fraction;
            let mut said = false;
            for _ in 0..240 {
                ultra.update(&world, &water, player, dt);
                let boss = ultra.boss.as_mut().unwrap();
                said |= boss.deeds.iter().any(|d| matches!(d, crate::bosses::Deed::Phase { stage: s, .. } if *s == stage));
                boss.deeds.clear();
                // It doesn't heal while it's fighting
                ultra.health = ultra.health.min(ultra.max_health * fraction);
            }
            let boss = ultra.boss.as_ref().unwrap();
            assert!(said, "it roars into stage {stage}");
            assert_eq!(boss.stage, stage);
            assert_eq!(element_now(&ultra), Some(ULTRA_ELEMENTS[stage as usize]));
            let coils = ultra.coils().unwrap();
            assert!(coils.shifted >= coils.points.len() as f64, "its whole body's taken on its element by now");
            assert_eq!(boss.enraged, stage == 3);
        }
    }

    /// A great serpent's body follows its head: each piece about as far from the one in front as
    /// it's meant to be, on the ground, and where the head went
    #[test]
    fn a_great_serpents_body_follows_its_head() {
        let world = field();
        let vpm = 8.0;
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let mut coils = Coils::new(Look::Veinbinder).unwrap();
        let g = coils.kind();
        let start = Vector3::new(40.0, 96.0, 4.0) * vpm;
        // Slithering east 30 m
        for k in 0..300 {
            let feet = start + Vector3::new(0.1 * k as f64, 0.0, 0.0) * vpm;
            coils.update(feet, 0.0, 0.0, 0.0, &world, &water, 1.0 / 30.0);
        }
        let mut ahead = coils.head;
        for (k, p) in coils.points.iter().enumerate() {
            let gap = (p - ahead).norm() / vpm;
            assert!(gap > 0.3 && gap < 1.3 * g.spacing, "piece {k} is {gap:.2} m from the one in front");
            assert!(p.x < ahead.x + 0.01, "piece {k} is behind the one in front");
            assert!((p.y - start.y).abs() < 0.5 * vpm, "piece {k} is off the way the head went");
            assert!((p.z / vpm - 4.0 - g.girth * taper(k, g.pieces)).abs() < 0.3, "piece {k} isn't lying on the ground: {:.2} m", p.z / vpm);
            ahead = *p;
        }
        // Its tail raised and brought down where it's aimed
        let target = coils.points[coils.points.len() - 1] + Vector3::new(0.0, 3.0, 0.0) * vpm;
        coils.tail = Some((target, 1.0));
        coils.update(start + Vector3::new(29.9, 0.0, 0.0) * vpm, 0.0, 0.0, 0.0, &world, &water, 1.0 / 30.0);
        assert!(coils.points[coils.points.len() - 1].z > target.z + 3.0 * vpm, "its tail's raised high");
        coils.tail = Some((target, 2.0));
        coils.update(start + Vector3::new(29.9, 0.0, 0.0) * vpm, 0.0, 0.0, 0.0, &world, &water, 1.0 / 30.0);
        assert!((coils.points[coils.points.len() - 1] - target).xy().norm() < 1.0 * vpm, "its tail's come down where it was aimed");
        // Whoever's stood in its body is shoved out of it, to the side they're on, and whoever's
        // clear of it isn't
        let piece = coils.points[4];
        let beside = piece + Vector3::new(0.0, 0.3, 0.0) * vpm;
        let shove = coils.shove(beside, 1.8, vpm);
        assert!(shove.y > 0.5 * vpm && shove.z == 0.0, "shoved out to the side it's on: {:.2} m", shove.y / vpm);
        assert!(coils.shove(piece + Vector3::new(0.0, 4.0, 0.0) * vpm, 1.8, vpm) == Vector3::zeros(), "clear of it, left be");
        assert!(coils.shove(piece + Vector3::new(0.0, 0.3, 4.0) * vpm, 1.8, vpm) == Vector3::zeros(), "high over it, left be");
    }

    /// What swims goes on out over deep water, the open sea of the Sunken Reach, rather than being
    /// stopped where it can't feel the bottom: a raid serpent comes across ten meters of water
    /// after the player, along the top of it
    #[test]
    fn a_great_serpent_swims_across_deep_water() {
        let world = field();
        let vpm = 8.0;
        let water = |_: f64, _: f64| 14.0 * vpm;
        let afloat = 14.0 - draft(Look::Reefcoil);
        let start = Vector3::new(60.0, 96.0, afloat) * vpm;
        let mut serpent = Animal::boss(Look::Reefcoil, start, [0, 0, -1], 3);
        let boss = serpent.boss.as_mut().unwrap();
        boss.provoke();
        boss.only = Some(crate::bosses::Move::Coil);
        let player = Some(start + Vector3::new(25.0, 0.0, 0.0) * vpm);
        for _ in 0..150 {
            serpent.update(&world, &water, player, 1.0 / 30.0);
            serpent.boss.as_mut().unwrap().deeds.clear();
        }
        let gone = (serpent.position - start).xy().norm() / vpm;
        assert!(gone > 6.0, "it's swum only {gone:.1} m");
        assert!((serpent.position.z / vpm - afloat).abs() < 0.5, "along the top of the water: {:.2} m", serpent.position.z / vpm);
    }

    /// A great serpent of the sky over a gap between islands, with no ground under it, trails its
    /// body behind it through the air rather than hanging down from its head
    #[test]
    fn a_sky_serpents_body_trails_through_the_air() {
        let world = field();
        let vpm = 8.0;
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let mut coils = Coils::new(Look::Galecoil).unwrap();
        // High over the ground, well out of reach of it
        let feet = Vector3::new(60.0, 96.0, 40.0) * vpm;
        for k in 0..60 {
            coils.update(feet + Vector3::new(0.1 * k as f64, 0.0, 0.0) * vpm, 0.0, 0.0, 0.0, &world, &water, 1.0 / 30.0);
        }
        let tail = coils.points[coils.points.len() - 1];
        assert!((tail.z - coils.head.z).abs() < 1.5 * vpm, "its tail's {:.1} m below its head", (coils.head.z - tail.z) / vpm);
        assert!(coils.head.x - tail.x > 10.0 * vpm, "its body trails out behind it");
    }

    /// Serpents turn up in their own land: vipers on the normal world's grass and dirt, gale
    /// serpents in the sky isles, adders in the dreaming realm and drift serpents in the frozen one,
    /// a nest of two or three of them together, well away from the player
    #[test]
    fn serpents_turn_up_in_their_own_land() {
        let world = field();
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let near = Vector3::new(96.0, 96.0, 4.0) * 8.0;
        for (land, look) in [(worldgen::Land::Normal, Look::ReedViper), (worldgen::Land::Sky, Look::GaleSerpent), (worldgen::Land::Dream, Look::BloodAdder), (worldgen::Land::Ice, Look::DriftSerpent)] {
            let mut rng = 11;
            let nests = crate::animals::spawn_serpents(&world, &water, &mut rng, near, land, 2);
            assert!(nests.len() >= 4, "{land:?}: {} serpents", nests.len());
            assert!(nests.iter().all(|a| a.look == look), "{land:?}: {:?}", nests.iter().map(|a| a.look).collect::<Vec<_>>());
            assert!(nests.iter().all(|a| (a.position - near).xy().norm() > 30.0 * 8.0));
        }
    }

    /// In Free Roam the Coil Unending's lair is far from where it starts, and the way there's
    /// strewn with omens, more of them nearer it
    #[test]
    fn the_coil_unendings_lair_is_far_off_with_omens_on_the_way() {
        let start = [200.0, 180.0];
        for seed in [1, 2, 77] {
            let settings = TerrainSettings { width_m: 224, height_m: 64, voxels_per_meter: 16, seed, grass_tufts: true };
            let lair = free_roam_ultra(&settings, start).expect("a lair");
            let far = (lair[0] - start[0]).hypot(lair[1] - start[1]);
            assert!(far >= ULTRA_FAR, "seed {seed}: only {far:.0} m off");
            let omens = omens(seed, start, lair);
            assert!(omens.len() >= 10, "seed {seed}: {} omens", omens.len());
            let off = |p: [f64; 2]| (p[0] - lair[0]).hypot(p[1] - lair[1]);
            let near = omens.iter().filter(|(_, p, _)| off(*p) < 400.0).count();
            let further = omens.iter().filter(|(_, p, _)| (400.0..800.0).contains(&off(*p))).count();
            assert!(near > further, "seed {seed}: {near} near it, {further} further off");
            assert!(omens.iter().any(|(l, ..)| *l == Look::ShedSkin) && omens.iter().any(|(l, ..)| *l == Look::SerpentBones));
        }
        assert!(gloom(1000.0) == 0.0 && gloom(100.0) > 0.99 && gloom(400.0) > 0.0);
    }

    /// A blow lands on the part of a long body it's aimed at, not on its middle
    #[test]
    fn the_part_aimed_at_is_hit() {
        let body: Vec<Vector3<f64>> = (0..10).map(|k| Vector3::new(k as f64 * 10.0, 0.0, 2.0)).collect();
        let eye = Vector3::new(70.0, -40.0, 2.0);
        let hit = focus(&body, eye, Vector3::new(0.0, 1.0, 0.0)).unwrap();
        assert!((hit - Vector3::new(70.0, 0.0, 2.0)).norm() < 1.0, "{hit:?}");
        // Aimed at its tail end, past the last of it, it's the tail
        let hit = focus(&body, Vector3::new(100.0, -40.0, 2.0), Vector3::new(0.0, 1.0, 0.0)).unwrap();
        assert!((hit - Vector3::new(90.0, 0.0, 2.0)).norm() < 1.0, "{hit:?}");
    }
}
