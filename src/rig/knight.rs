//! The Grave Knight, a fallen king of the dead, and the Crypt Titan made after it: a gaunt skeleton
//! with void burning in its eye sockets and in the heart behind its bare ribs, a crown with one point
//! snapped off on a broken helm, spiked pauldrons, a tattered cape and a tabard, armored forearms and
//! shins, and a huge notched greatsword with void glowing down its fuller. Standing about it drags
//! the sword's point along the ground; fighting, it holds it up in both hands. It raises it overhead
//! and cleaves down, crouches with it drawn back low and sweeps it round, raises it point down and
//! drives it into the ground so bone spikes burst up all round, kneels with it planted to raise the
//! dead, and leaps to cleave down from the air. Dying it drops to its knees and comes apart into a
//! heap of bones. The Titan wears black armor chased with gold, and a black cape.

use super::biped::{self, ARM, CAPE, CHEST, EXTRA, FOOT, FORE, Frame, Gait, HEAD, PELVIS, SHIN, Stance, THIGH, TOOL, bone};
use super::*;
use crate::animals::{ball, ellipsoid, limb};
use crate::worldgen::material::{DARK_STEEL, DIRT, LEATHER, OBSIDIAN, STEEL, TEMPLE_RED};

/// The three rings of bone spikes that burst up round it
const SPIKES: usize = EXTRA;

/// How far its blade reaches past its crossguard, and the crossguard past its hand
const BLADE: f64 = 1.25;
const GUARD: f64 = 0.06;
/// How far out its bone spikes burst up, in meters of its build
pub const SPIKE_REACH: f64 = 1.45;

pub const DEATH: f64 = 3.0;
const CRUMBLE: f64 = 1.55;
pub const THUDS: [f64; 2] = [0.75, 1.4];

const SNAP: f64 = 0.13;
const ANKLE: f64 = 0.06;

/// Its own number in its Stance: how far its cape streams out behind it
const FLARE: usize = 0;

fn frame() -> Frame {
    Frame {
        hip: 1.0,
        hip_w: 0.11,
        waist: 1.08,
        neck: V::new(0.01, 0.0, 0.61),
        shoulder: V::new(0.0, -0.26, 0.48),
        arm: [0.36, 0.34],
        leg: [0.53, 0.5],
        ankle: ANKLE,
        mouth: V::new(0.11, 0.0, 0.06),
        jaw: V::zeros(),
        tool: GUARD + BLADE,
        grip: 0.12,
        elbow: V::new(-0.35, -0.85, -0.4),
        knee: 0.05,
    }
}

const GAIT: Gait = Gait { lift: 0.13, bob: 0.025, sway: 0.04, roll: 0.04, yaw: 0.12, swing: 0.14, lean: 0.15, stomp: 0.02 };

/// Standing, its sword hanging from its right hand with its point on the ground ahead
pub fn rest(_look: Look) -> Stance {
    let mut s = frame().rest([[0.3, -0.42, 1.02], [0.17, 0.35, 0.92]]);
    s.tool = [0.45, -0.05, -0.89];
    s
}

pub fn pieces(look: Look) -> Vec<Vec<Part>> {
    let skin = look.skin();
    let (bone_, dark, armor, glow, trim) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let cape = if look == Look::CryptTitan { OBSIDIAN } else { TEMPLE_RED };
    let mut out = vec![Vec::new(); PIECES];
    let at = |v: [f64; 3], o: [f64; 3]| [v[0] - o[0], v[1] - o[1], v[2] - o[2]];
    // Its head: void in its sockets, a crown askew with one point snapped off, on a helm broken
    // open at one side
    let h = [0.01, 0.0, 1.69];
    let head = &mut out[HEAD];
    for side in [-1.0, 1.0] {
        head.push(ball(at([0.105, side * 0.045, 1.8], h), 0.022, glow));
        head.push(ball(at([0.094, side * 0.045, 1.8], h), 0.034, dark));
    }
    head.push(ball(at([0.118, 0.0, 1.755], h), 0.016, dark));
    head.push(ball(at([0.112, 0.0, 1.875], h), 0.02, glow));
    for k in 0..7 {
        let a = k as f64 / 7.0 * TAU;
        let (c, sn) = (a.cos(), a.sin());
        let foot = [0.005 + 0.105 * c, 0.11 * sn, 1.865 + 0.25 * 0.11 * sn];
        let (tall, out_) = if k == 2 { (0.035, 0.03) } else { (0.095, 0.012) };
        head.push(limb(at(foot, h), at([foot[0] + out_ * c, foot[1] + out_ * sn, foot[2] + tall], h), 0.022, 0.006, trim));
    }
    head.push(ellipsoid(at([0.005, 0.0, 1.865], h), [0.116, 0.122, 0.022], trim));
    head.push(ball(at([0.02, 0.0, 1.79], h), 0.112, bone_));
    head.push(ellipsoid(at([0.07, 0.0, 1.715], h), [0.065, 0.072, 0.035], bone_));
    head.push(ball(at([-0.05, 0.1, 1.9], h), 0.055, AIR));
    head.push(ellipsoid(at([-0.035, 0.0, 1.82], h), [0.12, 0.128, 0.115], armor));
    // Its chest: the void in its heart between its bare ribs, its breastbone and spine, collarbones,
    // a gorget, spiked pauldrons rimmed in trim, the clasps of its cape and the mantle over its
    // shoulders
    let c = [0.0, 0.0, 1.08];
    let chest = &mut out[CHEST];
    chest.push(ball(at([0.01, 0.0, 1.33], c), 0.06, glow));
    chest.push(limb(at([0.118, 0.0, 1.47], c), at([0.126, 0.0, 1.25], c), 0.02, 0.016, bone_));
    for k in 0..4 {
        let z = 1.21 + 0.078 * k as f64;
        let wider = (k as f64).min(2.0);
        let (rx_, ry_) = (0.112 + 0.01 * wider, 0.128 + 0.014 * wider);
        for side in [-1.0, 1.0] {
            let rib = |u: f64| -> [f64; 3] {
                let f = (PI - u) / (PI - 0.35);
                at([rx_ * u.cos() - 0.01, side * ry_ * u.sin(), z - 0.05 * f], c)
            };
            for j in 0..4 {
                let (u0, u1) = (PI - (PI - 0.35) * j as f64 / 4.0, PI - (PI - 0.35) * (j + 1) as f64 / 4.0);
                chest.push(limb(rib(u0), rib(u1), 0.017, 0.017, bone_));
            }
        }
    }
    for k in 0..8 {
        chest.push(ball(at([-0.105, 0.0, 1.04 + 0.065 * k as f64], c), 0.034, bone_));
    }
    chest.push(limb(at([-0.1, 0.0, 1.06], c), at([-0.11, 0.0, 1.54], c), 0.024, 0.028, bone_));
    chest.push(limb(at([0.0, 0.0, 1.54], c), at([0.01, 0.0, 1.7], c), 0.03, 0.028, bone_));
    for side in [-1.0, 1.0] {
        chest.push(limb(at([0.1, 0.0, 1.5], c), at([0.0, side * 0.24, 1.55], c), 0.02, 0.02, bone_));
    }
    chest.push(ellipsoid(at([0.0, 0.0, 1.56], c), [0.09, 0.13, 0.035], armor));
    for side in [-1.0, 1.0] {
        chest.push(limb(at([-0.02, side * 0.3, 1.64], c), at([-0.07, side * 0.43, 1.79], c), 0.035, 0.005, armor));
        chest.push(ellipsoid(at([0.0, side * 0.28, 1.6], c), [0.15, 0.13, 0.085], armor));
        chest.push(ellipsoid(at([0.0, side * 0.28, 1.585], c), [0.165, 0.145, 0.07], trim));
        chest.push(ellipsoid(at([0.0, side * 0.31, 1.5], c), [0.12, 0.1, 0.05], armor));
        chest.push(ball(at([0.03, side * 0.19, 1.555], c), 0.026, trim));
    }
    chest.push(ellipsoid(at([-0.09, 0.0, 1.56], c), [0.07, 0.21, 0.045], cape));
    // Its hips: its belt, and a tabard hanging in strips from it, trimmed at the top
    let p = [0.0, 0.0, 1.0];
    let hips = &mut out[PELVIS];
    hips.push(ball(at([0.112, 0.0, 1.05], p), 0.025, trim));
    hips.push(ellipsoid(at([0.0, 0.0, 1.05], p), [0.11, 0.17, 0.035], armor));
    hips.push(ellipsoid(at([0.106, 0.0, 1.0], p), [0.035, 0.12, 0.02], trim));
    for (y, long) in [(-0.07, 0.4), (0.0, 0.48), (0.07, 0.36)] {
        hips.push(limb(at([0.1, y, 1.0], p), at([0.14, y, 1.0 - long], p), 0.04, 0.046, cape).stretched([0.45, 1.0, 1.0]));
    }
    hips.push(ellipsoid(at([0.0, 0.0, 1.0], p), [0.09, 0.16, 0.07], bone_));
    // Its cape, strips torn to different lengths hanging from its shoulders
    for (i, hem) in [0.34, 0.47, 0.27, 0.52, 0.31, 0.44, 0.29].into_iter().enumerate() {
        let y = -0.21 + 0.07 * i as f64;
        out[CAPE].push(limb([-0.01, y, -0.01], [-0.09, y * 1.35, hem - 1.53], 0.04, 0.05, cape).stretched([0.5, 1.0, 1.0]));
    }
    // Bone above the elbow and the knee, armor below
    for k in 0..2 {
        out[ARM[k]] = vec![bone(0.36, 0.028, 0.026, bone_)];
        out[FORE[k]] = vec![ball([0.0, 0.0, -0.34], 0.05, armor), ball([0.0, 0.0, 0.0], 0.045, armor), bone(0.34, 0.042, 0.036, armor)];
        out[THIGH[k]] = vec![bone(0.53, 0.032, 0.03, bone_)];
        out[SHIN[k]] = vec![ball([0.0, 0.0, 0.0], 0.052, armor), bone(0.5, 0.048, 0.04, armor)];
        out[FOOT[k]] = vec![animals::cuboid([-0.3, -0.2, -0.5], [0.3, 0.2, -ANKLE], AIR), ellipsoid([0.05, 0.0, -0.02], [0.1, 0.05, 0.04], armor), ball([0.0, 0.0, 0.0], 0.035, armor)];
    }
    // The greatsword, its point down: pommel and grip, a crossguard swept towards the blade,
    // notches hacked out of its edges, void glowing down its fuller, and its bright edges
    let sword = &mut out[TOOL];
    sword.push(ball([0.0, 0.0, 0.25], 0.04, trim));
    sword.push(limb([0.0, 0.0, 0.24], [0.0, 0.0, -GUARD], 0.026, 0.026, LEATHER));
    for side in [-1.0, 1.0] {
        sword.push(limb([0.0, 0.0, -GUARD], [0.0, side * 0.17, -GUARD - 0.045], 0.03, 0.018, trim));
    }
    for (f, side) in [(0.33, 1.0), (0.47, -1.0), (0.62, 1.0)] {
        sword.push(ball([0.0, side * 0.068, -GUARD - BLADE * f], 0.028, AIR));
    }
    sword.push(limb([0.0, 0.0, -GUARD - 0.08], [0.0, 0.0, -GUARD - BLADE * 0.8], 0.03, 0.016, glow));
    for (o, metal) in [(-0.06, STEEL), (-0.02, DARK_STEEL), (0.02, DARK_STEEL), (0.06, STEEL)] {
        sword.push(limb([0.0, o, -GUARD], [0.0, 0.0, -GUARD - BLADE], 0.022, 0.006, metal));
    }
    // Three rings of bone spikes round its feet, the nearest first, each with the ground heaved up
    // round its foot
    let mut seed = 0x5b1e_u32;
    for (ring, (r, count, high)) in [(0.5, 7, 0.5), (0.85, 11, 0.62), (1.2, 15, 0.52)].into_iter().enumerate() {
        for k in 0..count {
            let a = (k as f64 + 0.5 * ring as f64) / count as f64 * TAU + 0.3 * random(&mut seed);
            let r = r + 0.08 * (random(&mut seed) - 0.5);
            let high = high * (0.7 + 0.6 * random(&mut seed));
            let lean = 0.12 + 0.1 * random(&mut seed);
            let (c, sn) = (a.cos(), a.sin());
            let foot = [r * c, r * sn, 0.0];
            out[SPIKES + ring].push(limb(foot, [(r + lean) * c, (r + lean) * sn, high], 0.055, 0.006, bone_));
            out[SPIKES + ring].push(ellipsoid(foot, [0.07, 0.07, 0.025], DIRT));
        }
    }
    // Made finer, see rig::res(), what only shows then, each before what it's on so it takes its
    // place on its surface: a stone of void set in the front of its crown, rivets round the rims
    // of its pauldrons, bands of trim round its armored forearms and shins, and its sword's grip
    // ridged with trim
    if super::finer(look) {
        let mut before = |piece: usize, parts: Vec<Part>| {
            let rest = std::mem::take(&mut out[piece]);
            out[piece] = parts.into_iter().chain(rest).collect();
        };
        before(HEAD, vec![ball(at([0.122, 0.0, 1.866], h), 0.016, glow)]);
        let mut rivets = Vec::new();
        for side in [-1.0, 1.0] {
            for k in 0..12 {
                let a = k as f64 / 12.0 * TAU;
                rivets.push(ball(at([0.165 * a.cos(), side * 0.28 + 0.145 * a.sin(), 1.585], c), 0.011, dark));
            }
        }
        before(CHEST, rivets);
        for k in 0..2 {
            before(FORE[k], [-0.1, -0.22].map(|z| ellipsoid([0.0, 0.0, z], [0.047, 0.047, 0.014], trim)).into());
            before(SHIN[k], [-0.14, -0.3].map(|z| ellipsoid([0.0, 0.0, z], [0.053, 0.053, 0.016], trim)).into());
        }
        before(TOOL, [0.0, 0.06, 0.12, 0.18].map(|z| ellipsoid([0.0, 0.0, z], [0.031, 0.031, 0.01], trim)).into());
    }
    out
}

/// Fighting: its knees bent, the sword up in both hands, angled up ahead of it
fn guard(rest: &Stance) -> Stance {
    Stance {
        hips: [0.0, 0.0, -0.05],
        lean: 0.08,
        hands: [[0.32, -0.16, 1.12], [0.25, 0.1, 1.05]],
        tool: [0.75, 0.05, 0.66],
        grip: 1.0,
        feet: [[0.12, -0.16, ANKLE], [-0.12, 0.16, ANKLE]],
        more: [0.1, 0.0, 0.0, 0.0, 0.0, 0.0],
        ..*rest
    }
}

/// Where it puts itself as it goes about `m`, how quickly, and how free its legs are to walk
fn fight(look: Look, rest: &Stance, m: &Moment) -> (Stance, f64, f64) {
    let guard = if m.engaged { guard(rest) } else { *rest };
    let Some(d) = m.doing else {
        return (guard, 7.0, 1.0);
    };
    let pick = |(s, quick): (Stance, bool), slow: f64| (s, if quick { 45.0 } else { slow }, 0.0);
    let wide = [[0.2, -0.2, ANKLE], [-0.18, 0.2, ANKLE]];
    // Raised overhead behind it in both hands, then cleaving down in front
    let raised = Stance { lean: -0.12, twist: -0.15, nod: -0.1, hips: [-0.04, 0.0, -0.04], hands: [[-0.06, -0.2, 2.12], [-0.06, 0.0, 2.1]], tool: [-0.75, 0.0, 0.66], grip: 1.0, feet: wide, more: [0.2, 0.0, 0.0, 0.0, 0.0, 0.0], ..guard };
    let reared = Stance { lean: -0.2, hands: [[-0.16, -0.18, 2.06], [-0.16, 0.0, 2.04]], tool: [-0.92, 0.0, 0.38], ..raised };
    let cleave = Stance { lean: 0.38, bend: 0.15, twist: 0.1, nod: 0.12, hips: [0.15, 0.0, -0.16], hands: [[0.85, -0.12, 1.0], [0.85, 0.0, 1.0]], tool: [0.91, 0.0, -0.41], grip: 1.0, feet: [[0.35, -0.2, ANKLE], [-0.3, 0.2, ANKLE]], toes: [0.0, 0.35], more: [0.5, 0.0, 0.0, 0.0, 0.0, 0.0], ..guard };
    let bitten = Stance { hips: [0.18, 0.0, -0.22], hands: [[0.88, -0.1, 0.82], [0.86, 0.0, 0.84]], tool: [0.8, 0.0, -0.6], ..cleave };
    // Crouched with it drawn back low behind it to its right, then swept out flat to its left
    let coil = Stance { lean: 0.22, twist: -0.75, nod: 0.0, hips: [-0.05, 0.0, -0.22], hands: [[-0.12, -0.6, 0.98], [0.35, 0.45, 1.2]], tool: [-0.35, -0.88, -0.31], flat: 0.0, grip: 0.0, feet: [[0.12, -0.24, ANKLE], [-0.12, 0.24, ANKLE]], more: [0.3, 0.0, 0.0, 0.0, 0.0, 0.0], ..guard };
    let swept = Stance { lean: 0.18, twist: 0.65, hips: [0.0, 0.0, -0.3], hands: [[0.38, 0.18, 1.0], [-0.25, 0.5, 1.25]], tool: [0.59, 0.75, -0.3], flat: 0.0, grip: 0.0, feet: coil.feet, more: [0.7, 0.0, 0.0, 0.0, 0.0, 0.0], ..guard };
    // Raised high point down, then driven into the ground as it drops onto a knee
    let raise = Stance { lean: -0.05, nod: -0.15, hips: [0.0, 0.0, 0.02], hands: [[0.28, -0.08, 2.08], [0.28, 0.06, 2.06]], tool: [0.02, 0.0, -1.0], grip: 1.0, feet: [[0.05, -0.18, ANKLE], [-0.05, 0.18, ANKLE]], more: [0.15, 0.0, 0.0, 0.0, 0.0, 0.0], ..guard };
    let driven = Stance { lean: 0.45, nod: 0.25, hips: [0.1, 0.0, -0.32], hands: [[0.75, 0.0, 0.85], [0.75, 0.06, 0.86]], tool: [0.45, 0.0, -0.89], grip: 1.0, feet: [[0.25, -0.16, ANKLE], [-0.42, 0.16, ANKLE]], toes: [0.0, 0.7], more: [0.4, 0.0, 0.0, 0.0, 0.0, 0.0], ..guard };
    match (d.m, d.stage) {
        (Move::Slam, Stage::Wind) => {
            let lead = (d.len - SNAP).max(0.05);
            let usual = (lead - d.held).max(0.02);
            let hold = mix(&raised, &reared, (d.held / 0.4).min(1.0));
            let (mut s, quick) = along(d.t, &[(0.0, guard, Ease::Linear), (usual, raised, Ease::Gather), (lead, hold, Ease::Smooth), (d.len, cleave, Ease::Snap)]);
            if d.t > usual && d.t < lead {
                s.lean += 0.025 * (d.t * 36.0).sin();
            }
            pick((s, quick), 11.0)
        }
        (Move::Slam, Stage::Act) => pick(along(d.t, &[(0.0, cleave, Ease::Linear), (0.1, bitten, Ease::Over), (d.len, bitten, Ease::Linear)]), 30.0),
        (Move::Slam | Move::Leap | Move::Quake | Move::Spikes, Stage::Recover) => {
            let wrench = Stance { hips: [0.05, 0.0, -0.12], lean: 0.1, hands: [[0.55, -0.15, 1.2], [0.5, 0.05, 1.15]], tool: [0.7, 0.0, 0.2], ..bitten };
            pick(along(d.t, &[(0.0, if d.m == Move::Spikes || d.m == Move::Quake { driven } else { bitten }, Ease::Linear), (d.len * 0.45, wrench, Ease::Smooth), (d.len, guard, Ease::Smooth)]), 8.0)
        }
        (Move::Sweep | Move::Spit, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), coil, Ease::Gather), (d.len, swept, Ease::Snap)]), 10.0),
        (Move::Sweep | Move::Spit, Stage::Act) => {
            let mut s = swept;
            // Its blade low and its cape out as it spins
            s.more[FLARE] = 0.9;
            pick((s, false), 25.0)
        }
        (Move::Sweep | Move::Spit, Stage::Recover) => {
            // Unsteady after spinning, its sword point down to catch itself
            let mut wobble = Stance { lean: 0.15, twist: 0.2, hips: [0.0, 0.0, -0.2], hands: [[0.5, -0.2, 0.95], [0.3, 0.4, 1.1]], tool: [0.4, 0.1, -0.9], grip: 0.0, feet: coil.feet, ..guard };
            wobble.roll += 0.06 * (d.t * 7.0).sin() * (1.0 - d.t / d.len.max(0.1));
            pick(along(d.t, &[(0.0, swept, Ease::Linear), (d.len * 0.3, wobble, Ease::Smooth), (d.len, guard, Ease::Smooth)]), 8.0)
        }
        (Move::Spikes | Move::Quake, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), raise, Ease::Gather), (d.len, driven, Ease::Snap)]), 9.0),
        (Move::Spikes | Move::Quake, Stage::Act) => {
            let mut s = driven;
            s.hips[2] += 0.02 * (d.t * 30.0).sin() * (1.0 - d.t / d.len.max(0.1));
            pick((s, false), 30.0)
        }
        // Gathering itself, sword raised; up through the air with its knees tucked, then cleaving
        // down as it lands
        (Move::Leap, Stage::Wind) => {
            let crouch = Stance { hips: [-0.05, 0.0, -0.28], lean: 0.2, ..raised };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len, crouch, Ease::Gather)]), 10.0)
        }
        (Move::Leap, Stage::Act) => {
            let mut flying = Stance { hips: [0.0, 0.0, 0.05], ..reared };
            for k in 0..2 {
                flying.feet[k] = [-0.15 + 0.2 * k as f64, if k == 0 { -0.18 } else { 0.18 }, 0.35 + 0.1 * k as f64];
            }
            flying.more[FLARE] = 1.0;
            let down = (d.len * 0.62).max(0.05);
            pick(along(d.t, &[(0.0, flying, Ease::Linear), (down, flying, Ease::Linear), (d.len, cleave, Ease::Snap)]), 18.0)
        }
        // Down on one knee, the sword planted point down in front of it, its other hand raised to
        // call the dead up
        (Move::Summon | Move::Cataclysm, Stage::Wind | Stage::Act) => {
            let kneel = Stance { hips: [0.0, 0.0, -0.36], lean: 0.12, twist: 0.2, nod: -0.4, hands: [[0.38, -0.18, 0.98], [0.13, 0.33, 2.2]], tool: [0.1, 0.0, -1.0], grip: 0.0, feet: [[0.3, -0.18, ANKLE], [-0.42, 0.15, ANKLE]], toes: [0.0, 0.75], more: [0.2, 0.0, 0.0, 0.0, 0.0, 0.0], ..guard };
            let mut s = kneel;
            if d.m == Move::Cataclysm {
                // The sword raised high, point up, both hands on it, its head back
                s = Stance { hips: [0.0, 0.0, -0.12], lean: -0.1, nod: -0.5, hands: [[0.12, -0.1, 2.22], [0.12, 0.05, 2.2]], tool: [0.1, 0.0, 1.0], grip: 1.0, feet: wide, ..kneel };
            }
            if d.stage == Stage::Act {
                s.hands[1][2] += 0.04 * (d.t * 20.0).sin();
                s.lean += 0.02 * (d.t * 17.0).sin();
            }
            pick(if d.stage == Stage::Wind { along(d.t, &[(0.0, guard, Ease::Linear), (d.len, s, Ease::Gather)]) } else { (s, false) }, 9.0)
        }
        (Move::Roar, Stage::Act) => {
            let mut roar = Stance { hips: [0.0, 0.0, -0.08], bend: -0.25, nod: -0.45, hands: [[0.25, -0.75, 1.25], [0.25, 0.75, 1.42]], tool: [0.3, -0.7, -0.65], grip: 0.0, feet: wide, more: [0.4, 0.0, 0.0, 0.0, 0.0, 0.0], ..guard };
            let shaking = (d.t > 0.25 && d.t < d.len - 0.3) as u8 as f64;
            roar.turn += 0.12 * (d.t * 10.0).sin() * shaking;
            if look.is_raid() && m.stage > 0 {
                // Raising its sword high, then driving it into the ground with a roar
                pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.35, raise, Ease::Gather), (0.48, driven, Ease::Snap), (0.8, driven, Ease::Linear), (1.1, roar, Ease::Smooth)]), 12.0)
            } else {
                pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.3, roar, Ease::Gather), (d.len, roar, Ease::Linear)]), 12.0)
            }
        }
        (_, Stage::Reel) => {
            let reel = Stance { hips: [-0.25, 0.05, -0.05], lean: -0.25, bend: -0.15, tilt: 0.1, nod: -0.4, turn: 0.2, hands: [[0.1, -0.55, 1.3], [-0.1, 0.6, 1.5]], tool: [0.2, -0.6, 0.4], grip: 0.0, feet: [[-0.35, -0.18, ANKLE], [0.05, 0.16, ANKLE]], toes: [0.4, 0.0], more: [0.6, 0.0, 0.0, 0.0, 0.0, 0.0], ..guard };
            let caught = Stance { hips: [-0.15, 0.0, -0.22], lean: 0.2, nod: 0.1, hands: [[0.55, -0.15, 0.95], [0.3, 0.3, 1.1]], tool: [0.4, 0.0, -0.9], ..reel };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.15, reel, Ease::Gather), (d.len * 0.55, caught, Ease::Smooth), (d.len, guard, Ease::Smooth)]), 14.0)
        }
        (_, Stage::Stun) => {
            let mut dazed = Stance { hips: [0.0, 0.0, -0.15], lean: 0.15, nod: 0.35, hands: [[0.5, -0.15, 0.95], [0.3, 0.3, 1.1]], tool: [0.4, 0.0, -0.9], grip: 0.0, ..guard };
            dazed.turn += 0.3 * (d.t * 2.2).sin();
            pick((dazed, false), 5.0)
        }
        (_, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), raised, Ease::Gather), (d.len, cleave, Ease::Snap)]), 10.0),
        (_, Stage::Act) => pick((bitten, false), 25.0),
        (_, Stage::Recover) => pick(along(d.t, &[(0.0, bitten, Ease::Linear), (d.len, guard, Ease::Smooth)]), 6.0),
    }
}

/// Dying: thrown back, its sword falling from its hands, down onto its knees and slumping over
/// them, to come apart
fn death(rest: &Stance, t: f64) -> (Stance, f64, f64) {
    let hit = Stance { hips: [-0.15, 0.0, 0.0], lean: -0.2, bend: -0.2, nod: -0.55, hands: [[0.2, -0.5, 1.5], [0.1, 0.55, 1.6]], tool: [0.3, -0.5, -0.8], ..*rest };
    let knees = Stance { hips: [0.05, 0.0, -0.48], lean: 0.25, bend: 0.3, nod: 0.4, hands: [[0.45, -0.32, 0.55], [0.45, 0.32, 0.55]], tool: [0.6, -0.4, -0.7], feet: [[-0.2, -0.13, ANKLE], [-0.25, 0.13, ANKLE]], toes: [0.8, 0.8], ..*rest };
    let slumped = Stance { hips: [0.15, 0.0, -0.6], lean: 0.7, bend: 0.45, nod: 0.6, hands: [[0.7, -0.3, 0.08], [0.72, 0.32, 0.08]], ..knees };
    let (s, quick) = along(t, &[(0.0, *rest, Ease::Linear), (0.3, hit, Ease::Gather), (THUDS[0], knees, Ease::Snap), (1.05, knees, Ease::Linear), (THUDS[1], slumped, Ease::Snap), (DEATH, slumped, Ease::Linear)]);
    (s, if quick { 30.0 } else { 10.0 }, 0.0)
}

/// How far its greatsword reaches as it sweeps it round, how far its bone spikes burst up, in
/// meters, and where on it, +x ahead, the middle of its blade is swept out, for what's flung off it
pub fn reach(look: Look) -> (f64, f64, [f64; 3]) {
    static SWEPT: OnceLock<(f64, [f64; 3])> = OnceLock::new();
    let (reach, blade) = *SWEPT.get_or_init(|| {
        let rest = rest(Look::GraveKnight);
        let swept = Stance { lean: 0.18, twist: 0.65, hips: [0.0, 0.0, -0.3], hands: [[0.38, 0.18, 1.0], [-0.25, 0.5, 1.25]], tool: [0.59, 0.75, -0.3], flat: 0.0, grip: 0.0, feet: [[0.12, -0.24, ANKLE], [-0.12, 0.24, ANKLE]], ..rest };
        let (mut bones, mut sockets) = ([Bone::default(); PIECES], Sockets::default());
        biped::solve(&frame(), &swept, &mut bones, &mut sockets);
        (sockets.tip.xy().norm(), sockets.hands[0].lerp(&sockets.tip, 0.5).into())
    });
    let s = look.scale();
    (reach * s, SPIKE_REACH * s, blade.map(|v| v * s))
}

pub(super) fn animate(r: &mut Rig, m: &Moment) {
    let look = r.look;
    let f = frame();
    let rest = rest(look);
    let want = match m.dying {
        Some(t) => death(&rest, t),
        None => fight(look, &rest, m),
    };
    let s = biped::pose(r, m, &f, &GAIT, want, 1.0);
    // Its cape hanging from its shoulders, straight down whichever way it leans, streaming out
    // behind it as it lunges, spins and walks
    let chest = r.bones[CHEST];
    let (walking, running) = m.pace();
    let flare = (s.more[FLARE] + 0.25 * walking + 0.3 * running).clamp(0.0, 1.2);
    let sway = 0.05 * walking * (TAU * m.phase).sin();
    let mantle = chest.at + chest.turn * V::new(-0.12, 0.0, 1.53 - f.waist);
    r.bones[CAPE] = Bone::new(mantle, rz(s.yaw + s.twist) * rx(sway) * ry(0.12 + 0.85 * flare));
    // Spells flung off the middle of its blade
    r.sockets.cast = r.sockets.hands[0].lerp(&r.sockets.tip, 0.5);
    // The bone spikes bursting up out of the ground round it as its sword goes in, the nearest
    // first, and sinking back as it works the sword out
    let burst = match m.doing {
        Some(d) if d.m == Move::Spikes && d.stage == Stage::Act => (d.t / 0.22).min(1.0),
        Some(d) if d.m == Move::Spikes && d.stage == Stage::Recover => 1.0 - (d.t / 0.5).min(1.0),
        _ => 0.0,
    };
    for ring in 0..3 {
        let grow = smooth((burst * 1.3 - 0.15 * ring as f64) / 0.85);
        r.bones[SPIKES + ring] = Bone { at: V::new(0.0, 0.0, -0.8 * (1.0 - grow)), turn: Q::identity(), shown: grow > 0.02 };
    }
    biped::fall_apart(r, m, CRUMBLE, DEATH);
}
