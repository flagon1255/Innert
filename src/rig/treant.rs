//! The Ancient Treant: an old tree walking on its roots, moss on its bark, a crown of leaves and
//! eyes glowing deep in its trunk. Its trunk sways as it walks, its roots lifting and setting down
//! a few at a time, its crown swinging after it. It raises its branches wide and sweeps them down,
//! plunges them into the ground to call roots up, whips them forward to fling thorns, and dying it
//! creaks and topples over like a felled tree.

use super::biped::{self, ARM, CHEST, EXTRA, FORE, Frame, Gait, HEAD, PELVIS, Stance, bone};
use super::*;
use crate::animals::{ball, ellipsoid, limb};
use crate::worldgen::material::LEAVES_DARK;

/// Its five roots
const ROOTS: usize = EXTRA;
const ROOT_COUNT: usize = 5;

pub const DEATH: f64 = 3.0;
pub const THUDS: [f64; 2] = [0.6, 2.05];
const SNAP: f64 = 0.16;

fn frame() -> Frame {
    Frame {
        hip: 1.0,
        hip_w: 0.3,
        waist: 1.05,
        neck: V::new(0.0, 0.0, 2.55),
        shoulder: V::new(0.0, -0.45, 2.15),
        arm: [0.75, 1.0],
        leg: [0.5, 0.5],
        ankle: 0.0,
        mouth: V::new(0.5, 0.0, -0.6),
        jaw: V::zeros(),
        tool: 0.0,
        grip: 0.0,
        elbow: V::new(-0.4, -1.0, 0.3),
        knee: 0.0,
    }
}

const GAIT: Gait = Gait { lift: 0.0, bob: 0.05, sway: 0.0, roll: 0.06, yaw: 0.06, swing: 0.3, lean: 0.05, stomp: 0.05 };

pub fn rest(_look: Look) -> Stance {
    frame().rest([[0.45, -1.35, 1.6], [0.45, 1.35, 1.6]])
}

pub fn pieces(look: Look) -> Vec<Vec<Part>> {
    let skin = look.skin();
    let (bark, wood, moss, glow, leaves) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let mut out = vec![Vec::new(); PIECES];
    out[PELVIS] = vec![ellipsoid([0.0, 0.0, 0.0], [0.6, 0.6, 0.25], bark)];
    out[CHEST] = vec![
        ball([0.48, 0.17, 1.95], 0.08, glow),
        ball([0.48, -0.17, 1.95], 0.08, glow),
        ellipsoid([0.5, 0.0, 1.79], [0.06, 0.2, 0.05], crate::worldgen::material::BARK_DARK),
        ellipsoid([0.38, 0.0, 1.15], [0.12, 0.38, 0.5], moss),
        limb([0.0, 0.0, -0.05], [0.0, 0.0, 2.65], 0.58, 0.45, bark),
    ];
    out[HEAD] = [([0.0, 0.0, 0.8], 0.95), ([0.5, 0.6, 0.5], 0.6), ([-0.4, -0.6, 0.6], 0.65), ([0.3, -0.5, 1.0], 0.55), ([-0.3, 0.5, 1.1], 0.55)]
        .into_iter()
        .map(|(c, r)| ball(c, r, if r > 0.9 { LEAVES_DARK } else { leaves }))
        .collect();
    for k in 0..2 {
        out[ARM[k]] = vec![bone(0.75, 0.24, 0.17, bark)];
        out[FORE[k]] = vec![ball([0.0, 0.0, -1.0], 0.32, leaves), limb([0.0, 0.0, -1.0], [0.25, 0.0, -1.3], 0.08, 0.02, wood), ball([0.0, 0.0, 0.0], 0.17, bark), bone(1.0, 0.17, 0.09, wood)];
    }
    for k in 0..ROOT_COUNT {
        out[ROOTS + k] = vec![limb([0.0, 0.0, 0.0], [0.85, 0.0, -1.0], 0.3, 0.08, bark)];
    }
    out
}

fn guard(rest: &Stance) -> Stance {
    Stance { bend: 0.08, hands: [[0.9, -1.25, 2.0], [0.9, 1.25, 2.0]], ..*rest }
}

fn fight(rest: &Stance, m: &Moment) -> (Stance, f64, f64) {
    let guard = if m.engaged { guard(rest) } else { *rest };
    let Some(d) = m.doing else {
        return (guard, 5.0, 1.0);
    };
    let pick = |(s, quick): (Stance, bool), slow: f64| (s, if quick { 35.0 } else { slow }, 0.0);
    // Branches raised wide and high, then swept down in front
    let wide = Stance { bend: -0.18, lean: -0.05, hands: [[0.3, -1.9, 4.2], [0.3, 1.9, 4.2]], ..guard };
    let swept = Stance { bend: 0.3, lean: 0.08, hands: [[1.8, -0.45, 0.9], [1.8, 0.45, 0.9]], ..guard };
    // Plunged down into the ground either side of it
    let plunged = Stance { bend: 0.22, hands: [[1.1, -1.0, 0.15], [1.1, 1.0, 0.15]], ..guard };
    // Drawn back, then whipped forward
    let back = Stance { bend: -0.25, hands: [[-0.5, -1.4, 3.3], [-0.5, 1.4, 3.3]], ..guard };
    let whipped = Stance { bend: 0.25, hands: [[1.6, -0.8, 2.6], [1.6, 0.8, 2.6]], ..guard };
    match (d.m, d.stage) {
        (Move::Slam, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), wide, Ease::Gather), (d.len, swept, Ease::Snap)]), 8.0),
        (Move::Slam, Stage::Act) => pick((swept, false), 25.0),
        (Move::Roots | Move::RootWave, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), wide, Ease::Gather), (d.len, plunged, Ease::Snap)]), 8.0),
        (Move::Roots | Move::RootWave, Stage::Act) => {
            let mut s = plunged;
            s.bend += 0.03 * (d.t * 20.0).sin();
            pick((s, false), 20.0)
        }
        (Move::Volley, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), back, Ease::Gather), (d.len, whipped, Ease::Snap)]), 8.0),
        (Move::Volley, Stage::Act) => pick((whipped, false), 20.0),
        (Move::Roar | Move::Summon, Stage::Act) => {
            let mut s = Stance { bend: -0.25, nod: -0.2, hands: [[0.4, -2.0, 3.6], [0.4, 2.0, 3.6]], ..guard };
            s.turn += 0.08 * (d.t * 6.0).sin();
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.35, s, Ease::Gather), (d.len, s, Ease::Linear)]), 8.0)
        }
        (_, Stage::Reel) => {
            let reel = Stance { lean: -0.12, bend: -0.2, tilt: 0.1, hands: [[0.0, -1.8, 3.0], [-0.3, 1.7, 2.6]], ..guard };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.18, reel, Ease::Gather), (d.len, guard, Ease::Smooth)]), 12.0)
        }
        (_, Stage::Recover) => pick(along(d.t, &[(0.0, swept, Ease::Linear), (d.len, guard, Ease::Smooth)]), 5.0),
        (_, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), wide, Ease::Gather), (d.len, swept, Ease::Snap)]), 8.0),
        _ => pick((swept, false), 15.0),
    }
}

/// Dying: a shudder, then leaning further and further over until it topples and crashes down
fn death(rest: &Stance, t: f64) -> (Stance, f64, f64) {
    let shudder = Stance { bend: -0.15, lean: -0.05, hands: [[0.3, -1.8, 3.6], [0.3, 1.8, 3.6]], ..*rest };
    let leaning = Stance { lean: 0.35, bend: 0.1, hands: [[1.2, -1.6, 2.2], [1.2, 1.6, 2.2]], ..*rest };
    let felled = Stance { lean: 1.42, bend: 0.12, hands: [[2.0, -1.6, 0.4], [2.0, 1.6, 0.4]], ..*rest };
    let (s, quick) = along(t, &[(0.0, *rest, Ease::Linear), (THUDS[0], shudder, Ease::Gather), (1.3, leaning, Ease::Smooth), (THUDS[1], felled, Ease::Snap), (DEATH, felled, Ease::Linear)]);
    (s, if quick { 40.0 } else { 8.0 }, 0.0)
}

pub(super) fn animate(r: &mut Rig, m: &Moment) {
    let look = r.look;
    let f = frame();
    let rest = rest(look);
    let want = match m.dying {
        Some(t) => death(&rest, t),
        None => fight(&rest, m),
    };
    let s = biped::pose(r, m, &f, &GAIT, want, 2.0);
    // Its crown swinging after its trunk
    let chest = r.bones[CHEST];
    let (walking, _) = m.pace();
    let swing = 0.05 * walking * (TAU * m.phase - 0.8).sin() + 0.03 * (r.clock * 0.7).sin();
    r.bones[HEAD] = Bone::new(chest.at + chest.turn * V::new(0.0, 0.0, 2.55), chest.turn * rx(swing) * ry(0.5 * swing));
    // Its roots spread round its foot, lifting a few at a time as it walks, gripping the ground
    // still as it stands
    let pelvis = r.bones[PELVIS];
    for k in 0..ROOT_COUNT {
        let a = k as f64 / ROOT_COUNT as f64 * TAU + 0.3;
        let lift = if m.dying.is_some() { 0.0 } else { 0.4 * walking * ((TAU * m.phase + if k % 2 == 0 { 0.0 } else { PI }).sin()).max(0.0) };
        let at = pelvis.at + pelvis.turn * V::new(a.cos() * 0.35, a.sin() * 0.35, 0.0);
        r.bones[ROOTS + k] = Bone::new(at, rz(s.yaw + a) * ry(-lift));
    }
    r.sockets.cast = r.sockets.mouth;
}
