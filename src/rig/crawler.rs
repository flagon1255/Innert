//! What crawls on eight legs: the Broodmother and the Rimeweaver. A giant spider, its head with a
//! cluster of glowing eyes and fangs, a great abdomen behind with markings down its back, and eight
//! legs bent up high at the knee, each reaching for where its foot's planted. It scuttles four legs
//! at a time, its body gliding level between; it rears up with its forelegs raised and lunges down
//! to bite, crouches to spring, raises its abdomen to spin web, and dying curls its legs up under it.

use bytemuck::{Pod, Zeroable};

use super::*;
use crate::animals::{ball, ellipsoid, limb};

const BODY: usize = 0;
const ABDOMEN: usize = 1;
/// Upper and lower legs: the right ones front to back, then the left
const FEMUR: [usize; 8] = [2, 3, 4, 5, 6, 7, 8, 9];
const TIBIA: [usize; 8] = [10, 11, 12, 13, 14, 15, 16, 17];

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
}

pub fn rest(_look: Look) -> Stance {
    let mut s = Stance::default();
    for k in 0..8 {
        s.feet[k] = stand(k);
    }
    s
}

pub fn pieces(look: Look) -> Vec<Vec<Part>> {
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

fn solve(s: &Stance, bones: &mut [Bone; PIECES], sockets: &mut Sockets) {
    let at = V::new(0.0, 0.0, HIGH) + V::from(s.body);
    let turn = rz(s.yaw) * ry(s.pitch) * rx(s.roll);
    bones[BODY] = Bone::new(at, turn);
    let waist = at + turn * V::from(WAIST);
    bones[ABDOMEN] = Bone::new(waist, turn * rz(s.abdomen[1]) * ry(-s.abdomen[0]));
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
    sockets.cast = sockets.mouth;
    sockets.hands = [V::from(s.feet[0]), V::from(s.feet[4])];
    sockets.tip = sockets.mouth;
    sockets.grip = sockets.mouth;
}

/// Scuttling four legs at a time, the rest planted, its body gliding level between
fn walk(s: &mut Stance, m: &Moment, free: f64) {
    let (walking, running) = m.pace();
    let amount = walking * free;
    if amount <= 0.0 {
        return;
    }
    let duty = 0.55;
    let step = m.stride * duty * 0.5 * amount;
    for k in 0..8 {
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

fn fight(rest: &Stance, m: &Moment) -> (Stance, f64, f64) {
    let guard = if m.engaged { Stance { body: [-0.05, 0.0, -0.1], pitch: 0.05, ..*rest } } else { *rest };
    let Some(d) = m.doing else {
        return (guard, 7.0, 1.0);
    };
    let pick = |(s, quick): (Stance, bool), slow: f64| (s, if quick { 40.0 } else { slow }, 0.0);
    let f = rest.feet;
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
        None => fight(&rest, m),
    };
    if !r.posed {
        *r.crawler() = want;
    }
    ease(r.crawler(), &want, rate, m.dt);
    let mut s = *r.crawler();
    if m.dying.is_none() {
        walk(&mut s, m, if m.doing.is_none() { 1.0 } else { free });
        let breath = (r.clock * TAU / 2.6).sin();
        s.abdomen[0] += 0.04 * breath;
        s.yaw += 0.3 * r.glance[0];
        let fl = r.flinched();
        s.pitch -= 0.4 * fl[0];
        s.body[0] -= 0.25 * fl[0];
        s.roll += 0.4 * fl[2];
        s.yaw += 0.3 * fl[1];
    }
    solve(&s, &mut r.bones, &mut r.sockets);
}
