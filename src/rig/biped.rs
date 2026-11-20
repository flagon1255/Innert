//! What walks on two legs: golems, knights, the warlord, giants. Its pose is a Stance: where its
//! hips are and how they're turned, how its chest is turned on them and its head on that, where its
//! hands and feet are, and which way what's in its right hand points. Its arms and legs reach for
//! where its hands and feet are, bending at the elbow and knee, see solve().

use bytemuck::{Pod, Zeroable};

use super::*;

// Its pieces, in this order among its models; a build leaves out those it hasn't got
pub const PELVIS: usize = 0;
pub const CHEST: usize = 1;
pub const HEAD: usize = 2;
pub const JAW: usize = 3;
/// Upper arms and forearms, the right then the left
pub const ARM: [usize; 2] = [4, 6];
pub const FORE: [usize; 2] = [5, 7];
/// Thighs, shins and feet, the right then the left
pub const THIGH: [usize; 2] = [8, 11];
pub const SHIN: [usize; 2] = [9, 12];
pub const FOOT: [usize; 2] = [10, 13];
/// What's in its right hand
pub const TOOL: usize = 14;
pub const CAPE: usize = 15;
/// Where a build's own pieces start
pub const EXTRA: usize = 16;

/// How a biped's put together, in meters at its build's size from the middle of its feet with +x
/// ahead: how high its hip joints are standing straight and how far apart either side, how high
/// the joint its chest turns on is, where its head's joint is from that and its right shoulder
/// (the left across from it), how long the bones of its arms and legs are, how high its ankles
/// are, where its mouth and the hinge of its jaw are from its head's joint, how far the tip of
/// what's in its right hand is from the hand and how far back along it the left hand takes it,
/// which way its right elbow points in its chest's frame, and how far out its knees point
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub hip: f64,
    pub hip_w: f64,
    pub waist: f64,
    pub neck: V,
    pub shoulder: V,
    pub arm: [f64; 2],
    pub leg: [f64; 2],
    pub ankle: f64,
    pub mouth: V,
    pub jaw: V,
    pub tool: f64,
    pub grip: f64,
    pub elbow: V,
    pub knee: f64,
}

/// How a biped holds itself: its hips moved from where they stand, ahead, left and up, and turned
/// forward, left side up and round to the left; its chest turned on them forward, round to the left
/// and left side up; its head turned on that down and round to the left; its jaw opened, 0 to 1;
/// where its right hand and its left are, and its feet, in its own meters; its toes pointed down;
/// which way what's in its right hand points, how far that's turned about its length, and how far
/// its left hand's on it; and a few numbers of its build's own
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Stance {
    pub hips: [f64; 3],
    pub lean: f64,
    pub roll: f64,
    pub yaw: f64,
    pub bend: f64,
    pub twist: f64,
    pub tilt: f64,
    pub nod: f64,
    pub turn: f64,
    pub jaw: f64,
    pub hands: [[f64; 3]; 2],
    pub feet: [[f64; 3]; 2],
    pub toes: [f64; 2],
    pub tool: [f64; 3],
    pub flat: f64,
    pub grip: f64,
    pub more: [f64; 6],
}

impl Frame {
    /// Standing straight, its feet under its hips and its hands where `hands` says
    pub fn rest(&self, hands: [[f64; 3]; 2]) -> Stance {
        Stance { hands, feet: [[0.0, -self.hip_w, self.ankle], [0.0, self.hip_w, self.ankle]], tool: [0.25, 0.0, -1.0], ..Default::default() }
    }
}

/// Where each of its pieces goes in `stance`, into `bones`, and where things come out of it into
/// `sockets`: its hips, chest and head turned one on the other, its arms and legs reaching for
/// where its hands and feet are, bending at the elbow and knee without twisting, what's in its
/// right hand pointing where it says, and its left hand on that if it says
pub fn solve(f: &Frame, s: &Stance, bones: &mut [Bone; PIECES], sockets: &mut Sockets) {
    let hips = V::new(0.0, 0.0, f.hip) + V::from(s.hips);
    let pelvis = rz(s.yaw) * ry(s.lean) * rx(s.roll);
    bones[PELVIS] = Bone::new(hips, pelvis);
    let chest_at = hips + pelvis * V::new(0.0, 0.0, f.waist - f.hip);
    let chest = pelvis * rz(s.twist) * ry(s.bend) * rx(s.tilt);
    bones[CHEST] = Bone::new(chest_at, chest);
    let head_at = chest_at + chest * f.neck;
    let head = chest * rz(s.turn) * ry(s.nod);
    bones[HEAD] = Bone::new(head_at, head);
    bones[JAW] = Bone::new(head_at + head * f.jaw, head * ry(0.6 * s.jaw));
    bones[CAPE] = Bone::new(chest_at, chest);
    sockets.mouth = head_at + head * f.mouth;
    sockets.cast = sockets.mouth;
    let side = |v: V, k: usize| if k == 0 { v } else { flip(v) };
    // An arm reaching for `target`, its elbow pointing back and out, and where its hand gets to
    let arm = |k: usize, target: V, bones: &mut [Bone; PIECES]| -> V {
        let shoulder = chest_at + chest * side(f.shoulder, k);
        let pole = chest * side(f.elbow, k);
        let (elbow, wrist) = bend(shoulder, target, f.arm[0], f.arm[1], pole);
        let rest = -V::x();
        bones[ARM[k]] = Bone::new(shoulder, aim(-V::z(), rest, elbow - shoulder, pole));
        bones[FORE[k]] = Bone::new(elbow, aim(-V::z(), rest, wrist - elbow, pole));
        wrist
    };
    let right = arm(0, V::from(s.hands[0]), bones);
    // What's in its right hand: built pointing down, its edges to either side, turned to point
    // where it says with its edges leading up and down or, as `flat` says, to the sides
    let d = V::from(s.tool).try_normalize(1e-9).unwrap_or(-V::z());
    let up = V::z();
    let edge_up = (up - d * d.dot(&up)).try_normalize(1e-6).unwrap_or_else(V::x);
    let edge_side = up.cross(&d).try_normalize(1e-6).unwrap_or_else(V::y);
    let e = (edge_up * (1.0 - s.flat) + edge_side * s.flat).try_normalize(1e-6).unwrap_or(edge_side);
    bones[TOOL] = Bone::new(right, aim(-V::z(), V::y(), d, e));
    sockets.tip = right + d * f.tool;
    let grip = right - d * f.grip;
    let left = arm(1, V::from(s.hands[1]).lerp(&grip, s.grip.clamp(0.0, 1.0)), bones);
    sockets.hands = [right, left];
    // Its legs, knees bending forward and a little out, its feet flat on the ground unless it
    // points its toes
    for k in 0..2 {
        let joint = hips + pelvis * side(V::new(0.0, -f.hip_w, 0.0), k);
        let target = V::from(s.feet[k]);
        let pole = rz(s.yaw) * side(V::new(1.0, -f.knee, 0.0), k);
        let (knee, ankle) = bend(joint, target, f.leg[0], f.leg[1], pole);
        bones[THIGH[k]] = Bone::new(joint, aim(-V::z(), V::x(), knee - joint, pole));
        bones[SHIN[k]] = Bone::new(knee, aim(-V::z(), V::x(), ankle - knee, pole));
        bones[FOOT[k]] = Bone::new(ankle, rz(s.yaw * 0.5) * ry(s.toes[k]));
    }
}

/// How a biped walks: how high it lifts each foot, how far its hips rise and fall, go over each foot
/// and drop on the other side, turn with each stride, how far its arms swing, how far it leans into
/// a run, and how hard it comes down on each foot, the heavier the harder
#[derive(Clone, Copy, Debug)]
pub struct Gait {
    pub lift: f64,
    pub bob: f64,
    pub sway: f64,
    pub roll: f64,
    pub yaw: f64,
    pub swing: f64,
    pub lean: f64,
    pub stomp: f64,
}

/// Its walk laid over `s`, as much as it's walking and as `free` says it's free to, 0 in the middle
/// of a move: each foot planted and going back under it as it goes on, then lifted and swung
/// forward to come down ahead, heel first; its hips rising over each foot, going over onto it and
/// dropping as the next comes down, turning with each stride and its shoulders against them; its
/// arms swinging against its legs. Turning on the spot, it steps round. `shuffle` is how far
/// through a step round it is.
pub fn walk(s: &mut Stance, g: &Gait, m: &Moment, shuffle: f64, free: f64) {
    let (walking, running) = m.pace();
    let amount = walking * free;
    let turning = (m.turning.abs() * 0.7).min(1.0) * (1.0 - walking) * free;
    let step = m.stride * 0.25 * amount;
    for k in 0..2 {
        let phi = (m.phase + k as f64 * 0.5).fract();
        let (x, lift, toe) = if phi < 0.5 {
            // Planted, going back under it, its heel coming up at the end
            let u = phi / 0.5;
            (step * (1.0 - 2.0 * u), 0.0, 0.35 * smooth((u - 0.6) / 0.4))
        } else {
            // Lifted and swung forward, toes first down then up to come down on its heel
            let u = (phi - 0.5) / 0.5;
            let toe = 0.35 * (1.0 - smooth(u / 0.35)) - 0.3 * smooth((u - 0.45) / 0.4) * (1.0 - smooth((u - 0.9) / 0.1));
            (step * (2.0 * smooth(u) - 1.0), g.lift * (1.0 + 0.7 * running) * (PI * u).sin().powf(0.8), toe)
        };
        s.feet[k][0] += x;
        s.feet[k][2] += lift * amount;
        s.toes[k] += toe * amount;
        let psi = (shuffle + k as f64 * 0.5).fract();
        if psi >= 0.5 {
            s.feet[k][2] += g.lift * 0.6 * turning * (PI * (psi - 0.5) / 0.5).sin();
        }
    }
    let w = TAU * m.phase;
    let bob = -g.bob * amount * (1.0 + 0.8 * running) * (2.0 * w).cos();
    let since = (2.0 * m.phase).fract();
    let dip = (since / 0.12) * (1.0 - since / 0.12).exp();
    let sway = -g.sway * amount * w.sin();
    s.hips[2] += bob - g.stomp * amount * dip;
    s.hips[1] += sway;
    s.roll -= g.roll * amount * w.sin();
    s.tilt += 0.7 * g.roll * amount * w.sin();
    s.yaw += g.yaw * amount * w.cos();
    s.twist -= 1.3 * g.yaw * amount * w.cos();
    s.lean += g.lean * running * free;
    s.nod -= 0.5 * g.lean * running * free;
    let arm = g.swing * amount * (1.0 + 0.4 * running);
    for k in 0..2 {
        let swing = if k == 0 { -w.cos() } else { w.cos() };
        s.hands[k][0] += arm * swing;
        s.hands[k][2] += bob * 0.9 - g.stomp * amount * dip * 0.6 + 0.3 * arm * swing.max(0.0);
        s.hands[k][1] += sway * 0.8;
    }
}

/// Breathing, shifting its weight from foot to foot as it stands, and looking about, laid over
/// `s`: `deep` is how slow and heavy its breathing is, 1 for a man's
pub fn idle(s: &mut Stance, m: &Moment, clock: f64, glance: [f64; 2], deep: f64) {
    let (walking, _) = m.pace();
    let still = 1.0 - walking;
    let breath = (clock * TAU / (3.0 * deep.sqrt())).sin();
    s.bend += 0.03 * breath;
    s.hips[2] += 0.01 * breath * deep;
    for k in 0..2 {
        s.hands[k][2] += 0.02 * breath * deep;
    }
    let shift = (clock * 0.31).sin() * 0.7 + (clock * 0.13 + 1.0).sin() * 0.3;
    s.hips[1] += 0.05 * deep * shift * still;
    s.roll -= 0.03 * shift * still;
    s.tilt += 0.025 * shift * still;
    s.turn += 0.75 * glance[0];
    s.twist += 0.2 * glance[0];
    s.nod += glance[1];
}

/// How far it's been knocked back, twisted and tipped by blows laid over `s`: its hips pushed
/// back, its chest and head thrown back further, its arms flung back
pub fn flinch(s: &mut Stance, f: [f64; 3]) {
    s.lean -= 0.4 * f[0];
    s.bend -= 0.6 * f[0];
    s.nod -= 0.8 * f[0];
    s.hips[0] -= 0.25 * f[0];
    s.twist += 0.7 * f[1];
    s.turn += 0.6 * f[1];
    s.tilt += 0.55 * f[2];
    s.roll += 0.25 * f[2];
    for k in 0..2 {
        s.hands[k][0] -= 0.4 * f[0];
        s.hands[k][2] += 0.3 * f[0].abs();
    }
}

/// Biped `r` posed: eased towards `want` as quickly as `rate` says, its walk laid over that as
/// `free` lets it, its breath, `deep` as idle() has it, and its flinching, unless it's dying; then
/// worked out into its pieces. Returns the pose it's in.
pub fn pose(r: &mut Rig, m: &Moment, f: &Frame, gait: &Gait, (want, rate, free): (Stance, f64, f64), deep: f64) -> Stance {
    if !r.posed {
        *r.biped() = want;
    }
    ease(r.biped(), &want, rate, m.dt);
    let mut s = *r.biped();
    if m.dying.is_none() {
        walk(&mut s, gait, m, r.shuffle, if m.doing.is_none() { 1.0 } else { free });
        idle(&mut s, m, r.clock, [r.glance[0], r.glance[1]], deep);
        flinch(&mut s, r.flinched());
    }
    solve(f, &s, &mut r.bones, &mut r.sockets);
    s
}

/// Its pieces falling apart into a heap once it's `from` seconds into dying, done by `until`
pub fn fall_apart(r: &mut Rig, m: &Moment, from: f64, until: f64) {
    match m.dying {
        Some(t) if t >= from => {
            if r.fallen.is_none() {
                r.fallen = Some(r.bones);
            }
            let was = r.fallen.unwrap();
            crumble(&mut r.bones, &was, (t - from) / (until - from), r.seed);
        }
        Some(_) => {}
        None => r.fallen = None,
    }
}

/// A limb's piece in its own frame: from its joint straight down `length`, `ra` round at the
/// joint to `rb` at its end
pub fn bone(length: f64, ra: f64, rb: f64, material: u8) -> Part {
    animals::limb([0.0, 0.0, 0.0], [0.0, 0.0, -length], ra, rb, material)
}
