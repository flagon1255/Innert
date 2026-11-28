//! What crawls on eight legs: the Broodmother and the Rimeweaver. A giant spider, its head with a
//! cluster of glowing eyes and fangs, a great abdomen behind with markings down its back, and eight
//! legs bent up high at the knee, each reaching for where its foot's planted. It scuttles four legs
//! at a time, its body gliding level between; it rears up with its forelegs raised and lunges down
//! to bite, crouches to spring, raises its abdomen to spin web, and dying curls its legs up under it.
//!
//! And the Sunstinger of the desert ruins, see src\bosses\roster.rs: a scorpion plated like the
//! sand and banded gold, low to the ground, its forelegs great pincers held up before it, and a tail
//! of five plates curled up over its back, venom glowing in its sting, which it stabs down at the
//! player again and again, raises to spit venom from, and snaps its pincers shut on whoever's near.

use bytemuck::{Pod, Zeroable};

use super::*;
use crate::animals::{ball, ellipsoid, limb};

const BODY: usize = 0;
const ABDOMEN: usize = 1;
/// Upper and lower legs: the right ones front to back, then the left
const FEMUR: [usize; 8] = [2, 3, 4, 5, 6, 7, 8, 9];
const TIBIA: [usize; 8] = [10, 11, 12, 13, 14, 15, 16, 17];
/// A scorpion's tail, joint by joint from its abdomen, and its sting
const TAIL: [usize; 5] = [18, 19, 20, 21, 22];
const STING: usize = 23;
/// How long each joint of its tail is, and where the first joins its abdomen
const TAIL_JOINT: f64 = 0.42;
const TAIL_ROOT: [f64; 3] = [-1.0, 0.0, 0.12];
/// Its pincers, its forelegs: the right and the left
const PINCERS: [usize; 2] = [0, 4];

pub const DEATH: f64 = 2.4;
pub const THUDS: [f64; 2] = [0.5, 0.95];
const SNAP: f64 = 0.12;

const HIGH: f64 = 1.3;
const LEG: [f64; 2] = [1.3, 1.5];
/// The middle of its body, where its head and abdomen join, from where its body is
const WAIST: [f64; 3] = [-0.25, 0.0, 0.05];

/// Where leg `k` joins its body, from the middle of it, and where its foot stands
fn hip(k: usize) -> [f64; 3] {
    let along = 0.95 - (k % 4) as f64 * 0.45;
    let side = if k < 4 { -1.0 } else { 1.0 };
    [along * 0.6 + 0.3 - 0.6, side * 0.3, 0.0]
}

fn stand(k: usize) -> [f64; 3] {
    let along = 0.95 - (k % 4) as f64 * 0.45;
    let side = if k < 4 { -1.0 } else { 1.0 };
    let reach = 1.6 + 0.2 * (1.0 - ((k % 4) as f64 - 1.5).abs() / 1.5);
    [along * 1.4 - 0.3, side * reach, 0.0]
}

/// How a crawler holds itself: its body moved and tipped forward, left side up and round to the
/// left; its abdomen raised and swung round; its fangs spread; and where each of its feet is
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Stance {
    pub body: [f64; 3],
    pub pitch: f64,
    pub roll: f64,
    pub yaw: f64,
    pub abdomen: [f64; 2],
    pub fangs: f64,
    pub feet: [[f64; 3]; 8],
    /// A scorpion's tail: how far it's struck out, 0 curled over its back to 1 stabbed down ahead
    /// of it, less than 0 drawn back tighter, and how far it's swung round to the left
    pub tail: [f64; 2],
}

/// Whether `look` is the scorpion, with pincers and a tail
fn scorpion(look: Look) -> bool {
    look == Look::Sunstinger
}

pub fn rest(look: Look) -> Stance {
    let mut s = Stance::default();
    for k in 0..8 {
        s.feet[k] = stand(k);
    }
    // Low to the ground, its pincers held up before it
    if scorpion(look) {
        s.body = [0.0, 0.0, -0.45];
        for (k, side) in [(PINCERS[0], -1.0), (PINCERS[1], 1.0)] {
            s.feet[k] = [1.55, side * 0.6, 0.75];
        }
    }
    s
}

pub fn pieces(look: Look) -> Vec<Vec<Part>> {
    if scorpion(look) {
        return sunstinger(look);
    }
    let skin = look.skin();
    let mut out = vec![Vec::new(); PIECES];
    // Its head: eight eyes and fangs, from the middle of its body
    let at = |v: [f64; 3]| [v[0] - 0.6, v[1], v[2] - HIGH];
    let b = &mut out[BODY];
    for (y, z, r) in [(0.1, 1.42, 0.06), (-0.1, 1.42, 0.06), (0.2, 1.36, 0.045), (-0.2, 1.36, 0.045), (0.06, 1.52, 0.04), (-0.06, 1.52, 0.04), (0.16, 1.5, 0.03), (-0.16, 1.5, 0.03)] {
        b.push(ball(at([1.18, y, z]), r, skin.glow));
    }
    for y in [-0.09, 0.09] {
        b.push(limb(at([1.22, y, 1.2]), at([1.38, y * 0.6, 0.92]), 0.05, 0.012, skin.extra));
    }
    b.push(ellipsoid(at([0.85, 0.0, 1.32]), [0.4, 0.38, 0.32], skin.body));
    b.push(ellipsoid(at([0.35, 0.0, 1.32]), [0.3, 0.32, 0.25], skin.body));
    // Its abdomen, from where it joins, glowing markings down its back
    let a = &mut out[ABDOMEN];
    for k in 0..3 {
        let x = -0.3 - k as f64 * 0.38;
        a.push(ellipsoid([x, 0.0, 0.5 - k as f64 * 0.05], [0.14, 0.32, 0.06], skin.accent));
    }
    a.push(ellipsoid([-0.6, 0.0, 0.1], [0.9, 0.75, 0.62], skin.body));
    for k in 0..8 {
        out[FEMUR[k]] = vec![limb([0.0, 0.0, 0.0], [0.0, 0.0, -LEG[0]], 0.1, 0.08, skin.dark)];
        out[TIBIA[k]] = vec![ball([0.0, 0.0, 0.0], 0.08, skin.dark), limb([0.0, 0.0, 0.0], [0.0, 0.0, -LEG[1]], 0.08, 0.03, skin.dark)];
    }
    out
}

/// The Sunstinger: plates the color of the sand banded darker and gold, small eyes glowing over its
/// mandibles, a flat segmented abdomen, thin legs, great pincers tipped black, and a tail of five
/// plates ending in a bulb of venom and its barb
fn sunstinger(look: Look) -> Vec<Vec<Part>> {
    let skin = look.skin();
    let (plate, dark, gold, glow, tip) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let mut out = vec![Vec::new(); PIECES];
    let b = &mut out[BODY];
    for (x, y) in [(0.62, 0.07), (0.62, -0.07), (0.55, 0.16), (0.55, -0.16)] {
        b.push(ball([x, y, 0.2], 0.035, glow));
    }
    for y in [-0.1, 0.1] {
        b.push(limb([0.68, y, 0.02], [0.82, y * 0.5, -0.06], 0.045, 0.015, tip));
    }
    b.push(ellipsoid([0.2, 0.0, 0.16], [0.45, 0.42, 0.06], gold));
    b.push(ellipsoid([0.25, 0.0, 0.05], [0.5, 0.46, 0.2], plate));
    let a = &mut out[ABDOMEN];
    for k in 0..4 {
        let x = -0.15 - k as f64 * 0.24;
        a.push(ellipsoid([x, 0.0, 0.18], [0.1, 0.44 - 0.04 * k as f64, 0.06], if k % 2 == 0 { gold } else { dark }));
    }
    a.push(ellipsoid([-0.5, 0.0, 0.05], [0.58, 0.45, 0.22], plate));
    for k in 0..8 {
        if PINCERS.contains(&k) {
            continue;
        }
        out[FEMUR[k]] = vec![limb([0.0, 0.0, 0.0], [0.0, 0.0, -LEG[0]], 0.08, 0.06, dark)];
        out[TIBIA[k]] = vec![ball([0.0, 0.0, 0.0], 0.065, plate), limb([0.0, 0.0, 0.0], [0.0, 0.0, -LEG[1]], 0.06, 0.025, dark)];
    }
    // Each pincer: a thick arm, a swollen claw and the two black-tipped jaws of it
    for k in PINCERS {
        out[FEMUR[k]] = vec![limb([0.0, 0.0, 0.0], [0.0, 0.0, -LEG[0]], 0.13, 0.1, plate).striped(gold)];
        let end = -LEG[1];
        out[TIBIA[k]] = vec![
            limb([0.06, 0.06, end + 0.25], [0.1, 0.12, end - 0.2], 0.07, 0.025, tip),
            limb([0.06, -0.06, end + 0.25], [0.1, -0.12, end - 0.2], 0.07, 0.025, tip),
            ellipsoid([0.02, 0.0, end + 0.38], [0.2, 0.16, 0.26], plate).striped(gold),
            ball([0.0, 0.0, 0.0], 0.1, plate),
            limb([0.0, 0.0, 0.0], [0.0, 0.0, end + 0.4], 0.1, 0.12, dark),
        ];
    }
    // The plates of its tail, each a little narrower than the last, from its joint along
    for (k, &p) in TAIL.iter().enumerate() {
        let r = 0.15 - 0.017 * k as f64;
        out[p] = vec![
            ellipsoid([0.0, 0.0, -TAIL_JOINT * 0.5], [r * 0.55, r * 1.05, TAIL_JOINT * 0.2], if k % 2 == 0 { gold } else { dark }),
            limb([0.0, 0.0, 0.0], [0.0, 0.0, -TAIL_JOINT], r, r * 0.85, plate),
        ];
    }
    out[STING] = vec![limb([0.0, 0.0, -0.28], [0.12, 0.0, -0.5], 0.04, 0.008, tip), ellipsoid([0.0, 0.0, -0.14], [0.11, 0.1, 0.16], glow), ball([0.0, 0.0, 0.0], 0.07, plate)];
    out
}

/// Where a scorpion's tail joints go along it, in its abdomen's frame: up off the end of it and
/// curled over its back, or struck out down ahead of it, `curl` from 0 to 1 between, and the way
/// each points, the last the sting's
fn tail_angles(curl: f64) -> [f64; 6] {
    let mut out = [0.0; 6];
    for (k, a) in out.iter_mut().enumerate().take(5) {
        let k = k as f64;
        let (rest, struck) = (0.5 + 0.55 * k, 1.5 + 0.5 * k);
        *a = rest + (struck - rest) * curl;
    }
    out[5] = 3.6 + 0.5 * curl;
    out
}

fn solve(look: Look, s: &Stance, bones: &mut [Bone; PIECES], sockets: &mut Sockets) {
    let at = V::new(0.0, 0.0, HIGH) + V::from(s.body);
    let turn = rz(s.yaw) * ry(s.pitch) * rx(s.roll);
    bones[BODY] = Bone::new(at, turn);
    let waist = at + turn * V::from(WAIST);
    bones[ABDOMEN] = Bone::new(waist, turn * rz(s.abdomen[1]) * ry(-s.abdomen[0]));
    // A scorpion's tail, joint after joint up off the end of its abdomen and over, and its sting
    if scorpion(look) {
        let abdomen = bones[ABDOMEN].turn * rz(s.tail[1]);
        let mut p = waist + bones[ABDOMEN].turn * V::from(TAIL_ROOT);
        let angles = tail_angles(s.tail[0]);
        for (k, &piece) in TAIL.iter().chain(std::iter::once(&STING)).enumerate() {
            let a = angles[k];
            let d = abdomen * V::new(-a.cos(), 0.0, a.sin());
            bones[piece] = Bone::new(p, aim(-V::z(), V::y(), d, abdomen * V::y()));
            p += d * TAIL_JOINT;
        }
        // Venom comes out of the end of its sting
        let sting = bones[STING];
        sockets.cast = sting.at + sting.turn * V::new(0.12, 0.0, -0.5);
    }
    for k in 0..8 {
        let joint = at + turn * V::from(hip(k));
        let side = if k < 4 { -1.0 } else { 1.0 };
        // Knees bent up high and out
        let pole = turn * V::new(0.0, side * 0.4, 1.0);
        let (knee, foot) = bend(joint, V::from(s.feet[k]), LEG[0], LEG[1], pole);
        bones[FEMUR[k]] = Bone::new(joint, aim(-V::z(), V::z(), knee - joint, pole));
        bones[TIBIA[k]] = Bone::new(knee, aim(-V::z(), V::z(), foot - knee, pole));
    }
    sockets.mouth = at + turn * V::new(0.8, 0.0, -0.3);
    if !scorpion(look) {
        sockets.cast = sockets.mouth;
    }
    sockets.hands = [V::from(s.feet[0]), V::from(s.feet[4])];
    sockets.tip = sockets.mouth;
    sockets.grip = sockets.mouth;
}

/// Scuttling four legs at a time, the rest planted, its body gliding level between
fn walk(look: Look, s: &mut Stance, m: &Moment, free: f64) {
    let (walking, running) = m.pace();
    let amount = walking * free;
    if amount <= 0.0 {
        return;
    }
    let duty = 0.55;
    let step = m.stride * duty * 0.5 * amount;
    for k in 0..8 {
        // A scorpion's pincers held up before it, bobbing a little as it goes, not walked on
        if scorpion(look) && PINCERS.contains(&k) {
            s.feet[k][2] += 0.05 * amount * (TAU * m.phase * 2.0).sin();
            continue;
        }
        // Alternate legs together: the right first and third with the left second and fourth
        let group = ((k % 4) + (k >= 4) as usize) % 2;
        let phi = (m.phase + group as f64 * 0.5).fract();
        let (x, lift) = if phi < duty {
            (step * (1.0 - 2.0 * phi / duty), 0.0)
        } else {
            let u = (phi - duty) / (1.0 - duty);
            (step * (2.0 * smooth(u) - 1.0), (0.25 + 0.15 * running) * (PI * u).sin())
        };
        s.feet[k][0] += x;
        s.feet[k][2] += lift * amount;
    }
    s.body[2] += 0.02 * amount * (4.0 * PI * m.phase).cos();
    s.abdomen[1] += 0.06 * amount * (TAU * m.phase).sin();
}

fn fight(look: Look, rest: &Stance, m: &Moment) -> (Stance, f64, f64) {
    let guard = if m.engaged { Stance { body: [rest.body[0] - 0.05, 0.0, rest.body[2] - 0.1], pitch: 0.05, ..*rest } } else { *rest };
    let Some(d) = m.doing else {
        return (guard, 7.0, 1.0);
    };
    let pick = |(s, quick): (Stance, bool), slow: f64| (s, if quick { 40.0 } else { slow }, 0.0);
    let f = rest.feet;
    if scorpion(look) {
        return sting_fight(&guard, &d);
    }
    // Reared up, its forelegs raised high, fangs spread; then lunging down to bite
    let mut reared = Stance { body: [-0.3, 0.0, 0.35], pitch: -0.45, abdomen: [-0.4, 0.0], fangs: 1.0, ..guard };
    for k in [0, 4] {
        reared.feet[k] = [f[k][0] + 0.2, f[k][1] * 0.7, 1.9];
    }
    for k in [1, 5] {
        reared.feet[k] = [f[k][0] + 0.3, f[k][1] * 0.8, 0.9];
    }
    let mut bitten = Stance { body: [0.45, 0.0, -0.35], pitch: 0.3, abdomen: [0.3, 0.0], fangs: 0.0, ..guard };
    for k in [0, 4] {
        bitten.feet[k] = [f[k][0] + 0.6, f[k][1] * 0.6, 0.0];
    }
    // Crouched low to spring or to spit, abdomen raised
    let crouched = Stance { body: [-0.2, 0.0, -0.55], pitch: 0.1, abdomen: [0.5, 0.0], fangs: 1.0, ..guard };
    let raised = Stance { body: [0.0, 0.0, -0.1], pitch: -0.15, abdomen: [1.0, 0.0], ..guard };
    match (d.m, d.stage) {
        (Move::Slam, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), reared, Ease::Gather), (d.len, bitten, Ease::Snap)]), 10.0),
        (Move::Slam, Stage::Act) => pick((bitten, false), 25.0),
        (Move::Leap, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len, crouched, Ease::Gather)]), 10.0),
        (Move::Leap, Stage::Act) => {
            let mut flying = Stance { body: [0.0, 0.0, 0.0], pitch: -0.1, fangs: 1.0, ..guard };
            for k in 0..8 {
                flying.feet[k] = [f[k][0] * 0.8, f[k][1] * 0.6, 0.7];
            }
            pick(along(d.t, &[(0.0, flying, Ease::Linear), (d.len * 0.75, flying, Ease::Linear), (d.len, crouched, Ease::Snap)]), 15.0)
        }
        (Move::Web | Move::Spit, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), crouched, Ease::Gather), (d.len, Stance { body: [0.15, 0.0, -0.2], pitch: -0.2, fangs: 1.0, ..raised }, Ease::Snap)]), 10.0),
        (Move::Web | Move::Spit, Stage::Act) => pick((Stance { body: [0.15, 0.0, -0.2], pitch: -0.2, fangs: 1.0, ..raised }, false), 20.0),
        (Move::Summon, Stage::Wind | Stage::Act) => {
            let mut s = raised;
            s.abdomen[0] += 0.25 * (d.t * 7.0).sin();
            pick((s, false), 10.0)
        }
        (Move::Roar, Stage::Act) => pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.3, reared, Ease::Gather), (d.len, reared, Ease::Linear)]), 12.0),
        (_, Stage::Reel) => {
            let reel = Stance { body: [-0.4, 0.1, 0.15], pitch: -0.25, roll: 0.15, ..guard };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.15, reel, Ease::Gather), (d.len, guard, Ease::Smooth)]), 14.0)
        }
        (_, Stage::Recover) => pick(along(d.t, &[(0.0, bitten, Ease::Linear), (d.len, guard, Ease::Smooth)]), 6.0),
        (_, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), reared, Ease::Gather), (d.len, bitten, Ease::Snap)]), 10.0),
        _ => pick((bitten, false), 20.0),
    }
}

/// How the Sunstinger goes about its moves: its tail drawn back tight and stabbed out down ahead of
/// it, three times, lunging after it; raised and aimed to spit venom; its pincers spread wide, then
/// snapped shut on what's in front of it
fn sting_fight(guard: &Stance, d: &Doing) -> (Stance, f64, f64) {
    let guard = *guard;
    let pick = |(s, quick): (Stance, bool), slow: f64| (s, if quick { 40.0 } else { slow }, 0.0);
    // Its pincers held out further, spread wider and raised higher than they rest
    let posed = |body: [f64; 2], pitch: f64, tail: f64, reach: f64, spread: f64, high: f64| {
        let mut s = Stance { body: [guard.body[0] + body[0], 0.0, guard.body[2] + body[1]], pitch, tail: [tail, 0.0], ..guard };
        for (k, side) in [(PINCERS[0], -1.0), (PINCERS[1], 1.0)] {
            s.feet[k] = [1.55 + reach, side * (0.6 + spread), 0.75 + high];
        }
        s
    };
    let cocked = posed([-0.15, 0.05], -0.08, -0.35, -0.15, 0.15, 0.2);
    let struck = posed([0.45, -0.05], 0.3, 1.0, -0.3, 0.35, 0.1);
    let spread = posed([-0.1, 0.1], -0.15, -0.2, 0.1, 0.5, 0.45);
    let snapped = posed([0.35, -0.05], 0.12, 0.2, 0.55, -0.45, -0.15);
    let aimed = posed([-0.05, 0.05], -0.12, 0.55, -0.1, 0.25, 0.1);
    match (d.m, d.stage) {
        (Move::Sting, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len, cocked, Ease::Gather)]), 10.0),
        (Move::Sting, Stage::Act) => pick(
            along(
                d.t,
                &[
                    (0.0, struck, Ease::Linear),
                    (0.08, struck, Ease::Linear),
                    (0.3, cocked, Ease::Gather),
                    (0.42, struck, Ease::Snap),
                    (0.5, struck, Ease::Linear),
                    (0.72, cocked, Ease::Gather),
                    (0.84, struck, Ease::Snap),
                    (d.len, struck, Ease::Linear),
                ],
            ),
            30.0,
        ),
        (Move::Sting, Stage::Recover) => pick(along(d.t, &[(0.0, struck, Ease::Linear), (d.len, guard, Ease::Smooth)]), 6.0),
        (Move::Spit, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), cocked, Ease::Gather), (d.len, aimed, Ease::Snap)]), 10.0),
        (Move::Spit, Stage::Act) => pick((aimed, false), 20.0),
        (Move::Roar, Stage::Act) => pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.3, spread, Ease::Gather), (d.len, spread, Ease::Linear)]), 12.0),
        (_, Stage::Reel) => {
            let mut reel = posed([-0.4, 0.1], -0.2, -0.3, -0.2, 0.3, 0.3);
            reel.roll = 0.15;
            reel.tail[1] = 0.3;
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.15, reel, Ease::Gather), (d.len, guard, Ease::Smooth)]), 14.0)
        }
        (_, Stage::Stun) => pick((posed([0.0, -0.15], 0.1, 0.3, 0.0, 0.0, -0.3), false), 5.0),
        (_, Stage::Recover) => pick(along(d.t, &[(0.0, snapped, Ease::Linear), (d.len, guard, Ease::Smooth)]), 6.0),
        (_, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), spread, Ease::Gather), (d.len, snapped, Ease::Snap)]), 10.0),
        _ => pick((snapped, false), 20.0),
    }
}

/// Dying: rearing back, then dropping flat, its legs curling up in under it
fn death(rest: &Stance, t: f64) -> (Stance, f64, f64) {
    let f = rest.feet;
    let mut reared = Stance { body: [-0.2, 0.0, 0.25], pitch: -0.35, abdomen: [-0.3, 0.0], fangs: 1.0, ..*rest };
    for k in [0, 4] {
        reared.feet[k] = [f[k][0], f[k][1] * 0.7, 1.4];
    }
    let mut dropped = Stance { body: [0.0, 0.0, -0.95], pitch: 0.05, abdomen: [0.2, 0.0], ..*rest };
    let mut curled = dropped;
    for k in 0..8 {
        dropped.feet[k] = [f[k][0] * 1.05, f[k][1] * 1.05, 0.0];
        curled.feet[k] = [f[k][0] * 0.35, f[k][1] * 0.45, 0.9];
    }
    let (s, quick) = along(t, &[(0.0, *rest, Ease::Linear), (0.3, reared, Ease::Gather), (THUDS[0], dropped, Ease::Snap), (THUDS[1], dropped, Ease::Linear), (DEATH, curled, Ease::Smooth)]);
    (s, if quick { 30.0 } else { 8.0 }, 0.0)
}

pub(super) fn animate(r: &mut Rig, m: &Moment) {
    let look = r.look;
    let rest = rest(look);
    let (want, rate, free) = match m.dying {
        Some(t) => death(&rest, t),
        None => fight(look, &rest, m),
    };
    if !r.posed {
        *r.crawler() = want;
    }
    ease(r.crawler(), &want, rate, m.dt);
    let mut s = *r.crawler();
    if m.dying.is_none() {
        walk(look, &mut s, m, r.walking(m, free));
        let breath = (r.clock * TAU / 2.6).sin();
        s.abdomen[0] += 0.04 * breath;
        // A scorpion's tail swaying over its back, following what it watches
        s.tail[0] += 0.06 * (r.clock * 1.9).sin();
        s.tail[1] += 0.12 * (r.clock * 0.7).sin() + 0.4 * r.glance[0];
        s.yaw += 0.3 * r.glance[0];
        let fl = r.flinched();
        s.pitch -= 0.4 * fl[0];
        s.body[0] -= 0.25 * fl[0];
        s.roll += 0.4 * fl[2];
        s.yaw += 0.3 * fl[1];
    }
    solve(look, &s, &mut r.bones, &mut r.sockets);
}
