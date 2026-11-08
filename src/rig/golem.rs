//! The golems: the Stone Warden, the Frost Colossus, the Magma Golem and the Earthbreaker Colossus.
//! A hulking body of rock on short thick legs, long arms ending in fists of dark stone that hang
//! to its knees, its head sunk low in front of its shoulders, a light in its chest and crystals
//! growing out of its back. It lumbers, each foot coming down so the ground shakes and its whole
//! body settling onto it; it raises both fists high and brings them down on the ground; it lifts a
//! foot high and stamps so a shockwave runs out; it scoops up a boulder and hurls it. The Colossus
//! tears its boulders out of the ground with both hands, the stone plates over its chest forced
//! apart as it strains and the fire in it blazing out between them, and raises its arms to bring
//! pillars of stone up round the player. Dying it buckles to its knees, falls on its face and
//! crumbles into a heap of stones.

use super::biped::{self, ARM, CHEST, EXTRA, FOOT, FORE, Frame, Gait, HEAD, PELVIS, SHIN, Stance, THIGH, bone};
use super::*;
use crate::animals::{ball, cuboid, ellipsoid, limb};
use crate::bosses;

/// The plates over the Colossus's chest, the fire blazing out from between them, and the boulder a
/// golem scoops up to throw
const PLATES: usize = EXTRA;
const BLAZE: usize = EXTRA + 4;
const BOULDER: usize = EXTRA + 5;

/// Its own numbers in its Stance: how far the Colossus's plates are forced apart, and whether
/// it's holding a boulder
const OPEN: usize = 0;
const HOLDING: usize = 1;

/// Seconds it takes to fall, and how far into that it starts to crumble
pub const DEATH: f64 = 3.2;
const CRUMBLE: f64 = 1.7;
/// Seconds into dying its knees hit the ground and it falls on its face, for the ground to shake
pub const THUDS: [f64; 2] = [1.0, 1.62];

/// Seconds its blows take to come down, the last of winding them up
const SNAP: f64 = 0.17;

fn frame() -> Frame {
    Frame {
        hip: 1.15,
        hip_w: 0.42,
        waist: 1.35,
        neck: V::new(0.3, 0.0, 1.27),
        shoulder: V::new(0.05, -0.98, 1.1),
        arm: [0.85, 0.85],
        leg: [0.5, 0.5],
        ankle: 0.28,
        mouth: V::new(0.4, 0.0, 0.05),
        jaw: V::zeros(),
        tool: 0.0,
        grip: 0.0,
        elbow: V::new(-1.0, -0.4, -0.15),
        knee: 0.15,
    }
}

const GAIT: Gait = Gait { lift: 0.2, bob: 0.05, sway: 0.11, roll: 0.05, yaw: 0.09, swing: 0.32, lean: 0.12, stomp: 0.07 };

/// Where its feet are standing
const ANKLE: f64 = 0.28;

/// Standing with its fists hanging by its knees
pub fn rest(_look: Look) -> Stance {
    frame().rest([[0.45, -1.12, 0.9], [0.45, 1.12, 0.9]])
}

/// Every piece of `look`, in its own frame about its joint
pub fn pieces(look: Look) -> Vec<Vec<Part>> {
    let skin = look.skin();
    let (rock, dark, glow) = (skin.body, skin.dark, skin.glow);
    let colossus = look == Look::Colossus;
    let f = frame();
    let mut out = vec![Vec::new(); PIECES];
    // Its hips, a dark mass joining its legs
    out[PELVIS] = vec![ball([0.0, 0.42, 0.0], 0.3, rock), ball([0.0, -0.42, 0.0], 0.3, rock), ellipsoid([0.0, 0.0, 0.05], [0.45, 0.6, 0.35], dark)];
    // Its body: the core glowing in its chest, light cracking out across it, crystals growing out
    // of its back, moss on its shoulders, and its shoulders themselves, round and dark
    let c = &mut out[CHEST];
    let z = |v: f64| v - f.waist;
    c.push(ball([0.6, 0.0, z(2.05)], 0.17, glow));
    c.push(limb([0.66, 0.0, z(2.05)], [0.62, 0.38, z(2.45)], 0.045, 0.02, glow));
    c.push(limb([0.66, 0.0, z(2.05)], [0.62, -0.32, z(1.7)], 0.045, 0.02, glow));
    for (y, h, high) in [(0.25, 2.5, 0.55), (-0.22, 2.3, 0.65), (0.05, 2.75, 0.5)] {
        c.push(limb([-0.45, y, z(h)], [-0.82, y * 1.3, z(h + high)], 0.1, 0.02, skin.extra));
    }
    for s in [-1.0, 1.0] {
        c.push(ellipsoid([0.0, s * 0.95, z(2.88)], [0.32, 0.32, 0.1], skin.accent));
        c.push(ball([0.05, s * 0.98, z(2.45)], 0.42, dark));
    }
    c.push(ball([0.24, 0.0, z(2.5)], 0.3, rock));
    c.push(ellipsoid([0.1, 0.0, z(2.05)], [0.62, 0.85, 0.75], rock));
    c.push(ellipsoid([0.0, 0.0, z(1.42)], [0.45, 0.62, 0.25], dark));
    // Its head, sunk low between its shoulders: eyes glowing under a heavy brow, moss on top
    let head = V::new(0.3, 0.0, 2.62);
    let h = |v: [f64; 3]| [v[0] - head.x, v[1], v[2] - head.z];
    out[HEAD] = vec![
        ball(h([0.64, 0.15, 2.86]), 0.065, glow),
        ball(h([0.64, -0.15, 2.86]), 0.065, glow),
        ellipsoid(h([0.6, 0.0, 2.98]), [0.12, 0.32, 0.06], dark),
        ellipsoid(h([0.35, 0.0, 3.1]), [0.26, 0.26, 0.08], skin.accent),
        ellipsoid(h([0.38, 0.0, 2.84]), [0.32, 0.32, 0.28], rock),
        ellipsoid(h([0.52, 0.0, 2.66]), [0.2, 0.25, 0.1], dark),
    ];
    // Its arms, lumpy stone, and its fists
    for k in 0..2 {
        let s = if k == 0 { -1.0 } else { 1.0 };
        out[ARM[k]] = vec![ball([0.06, s * 0.1, -0.35], 0.2, dark), bone(0.85, 0.27, 0.24, rock)];
        out[FORE[k]] = vec![
            ball([0.12, 0.0, -0.98], 0.13, dark),
            ball([0.0, 0.0, -0.85], 0.36, dark),
            ball([0.0, 0.0, 0.0], 0.25, rock),
            bone(0.85, 0.25, 0.3, rock),
        ];
        out[THIGH[k]] = vec![bone(0.5, 0.31, 0.29, rock)];
        out[SHIN[k]] = vec![ball([0.06, 0.0, 0.0], 0.27, dark), bone(0.5, 0.29, 0.27, rock)];
        // Its feet, flat on the ground
        out[FOOT[k]] = vec![cuboid([-0.8, -0.6, -0.9], [0.9, 0.6, -ANKLE], AIR), ellipsoid([0.1, 0.0, -0.1], [0.42, 0.33, 0.27], dark), ball([0.0, 0.0, 0.0], 0.24, rock)];
    }
    if colossus {
        // Its plates, and the fire behind them
        for k in 0..4 {
            out[PLATES + k] = vec![ellipsoid([0.0, 0.0, 0.0], [0.07, 0.2, 0.17], rock)];
        }
        out[BLAZE] = vec![ball([0.0, 0.0, 0.0], 0.42, glow)];
    } else {
        // A boulder of its own stone, cracked and mossy
        out[BOULDER] = vec![ellipsoid([0.2, 0.1, 0.25], [0.12, 0.2, 0.12], skin.accent), ball([0.0, 0.0, 0.0], 0.45, rock), ball([-0.15, -0.2, -0.15], 0.3, dark)];
    }
    out
}

/// Ready to fight: crouched a little, fists up
fn guard(rest: &Stance) -> Stance {
    Stance { hips: [0.0, 0.0, -0.1], bend: 0.12, lean: 0.04, hands: [[0.8, -1.0, 1.25], [0.75, 1.0, 1.3]], feet: [[0.15, -0.5, ANKLE], [-0.15, 0.5, ANKLE]], ..*rest }
}

/// What it wants to be doing with its body as it goes about `m`, and how quickly to get there, and
/// how free its legs are to walk
fn fight(look: Look, rest: &Stance, m: &Moment) -> (Stance, f64, f64) {
    let guard = if m.engaged { guard(rest) } else { *rest };
    let Some(d) = m.doing else {
        return (guard, 6.0, 1.0);
    };
    let pick = |(s, quick): (Stance, bool), slow: f64| (s, if quick { 42.0 } else { slow }, 0.0);
    let wide = [[0.3, -0.62, ANKLE], [-0.3, 0.6, ANKLE]];
    // Both fists raised high overhead, then drawn back further as it holds the blow, then brought
    // down on the ground in front of it, driven in, and pulled out
    let raised = Stance { hips: [-0.12, 0.0, 0.05], lean: -0.06, bend: -0.3, nod: -0.12, hands: [[0.05, -0.42, 3.95], [0.05, 0.42, 3.95]], feet: [[0.25, -0.55, ANKLE], [-0.3, 0.52, ANKLE]], toes: [0.0, 0.15], ..guard };
    let reared = Stance { hips: [-0.2, 0.0, 0.08], bend: -0.42, hands: [[-0.25, -0.4, 3.85], [-0.25, 0.4, 3.85]], ..raised };
    let smashed = Stance { hips: [0.35, 0.0, -0.38], lean: 0.32, bend: 0.5, nod: -0.35, hands: [[1.65, -0.36, 0.38], [1.65, 0.36, 0.38]], feet: [[0.55, -0.6, ANKLE], [-0.4, 0.56, ANKLE]], toes: [0.0, 0.25], ..guard };
    let sunk = Stance { hips: [0.32, 0.0, -0.46], bend: 0.56, hands: [[1.7, -0.4, 0.22], [1.7, 0.4, 0.22]], ..smashed };
    let pulled = Stance { hips: [0.15, 0.0, -0.25], bend: 0.3, lean: 0.15, hands: [[1.25, -0.6, 0.95], [1.25, 0.6, 0.95]], ..smashed };
    match (d.m, d.stage) {
        (Move::Slam, Stage::Wind) => {
            let lead = (d.len - SNAP).max(0.05);
            let usual = (lead - d.held).max(0.02);
            let hold = mix(&raised, &reared, (d.held / 0.4).min(1.0));
            let (mut s, quick) = along(d.t, &[(0.0, guard, Ease::Linear), (usual, raised, Ease::Gather), (lead, hold, Ease::Smooth), (d.len, smashed, Ease::Snap)]);
            if d.t > usual && d.t < lead {
                // Trembling with the weight of it, held back
                s.bend += 0.03 * (d.t * 38.0).sin();
                s.hips[2] += 0.012 * (d.t * 31.0).sin();
            }
            pick((s, quick), 10.0)
        }
        (Move::Slam, Stage::Act) => pick(along(d.t, &[(0.0, smashed, Ease::Linear), (0.12, sunk, Ease::Over), (d.len, sunk, Ease::Linear)]), 30.0),
        (Move::Slam, Stage::Recover) => pick(along(d.t, &[(0.0, sunk, Ease::Linear), (d.len * 0.4, pulled, Ease::Smooth), (d.len, guard, Ease::Smooth)]), 8.0),
        // One foot raised high, its arms flung wide for balance, its weight over the other; then
        // stamped down with a squat, its fists coming down to the ground either side as the
        // shockwave runs out, and slowly up again
        (Move::Quake, Stage::Wind) => {
            let lift = Stance { hips: [-0.05, 0.24, 0.06], roll: -0.12, tilt: 0.06, bend: -0.12, nod: -0.1, feet: [[0.3, -0.48, 1.05], [0.0, 0.5, ANKLE]], toes: [-0.25, 0.0], hands: [[0.35, -1.9, 2.75], [0.35, 1.9, 2.75]], ..guard };
            let stomp = Stance { hips: [0.1, 0.0, -0.44], lean: 0.2, bend: 0.38, nod: -0.25, feet: [[0.35, -0.66, ANKLE], [-0.1, 0.66, ANKLE]], hands: [[1.05, -1.05, 0.36], [1.05, 1.05, 0.36]], ..guard };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), lift, Ease::Gather), (d.len, stomp, Ease::Snap)]), 9.0)
        }
        (Move::Quake, Stage::Act) => {
            let stomp = Stance { hips: [0.1, 0.0, -0.44], lean: 0.2, bend: 0.38, nod: -0.25, feet: [[0.35, -0.66, ANKLE], [-0.1, 0.66, ANKLE]], hands: [[1.05, -1.05, 0.36], [1.05, 1.05, 0.36]], ..guard };
            let mut s = stomp;
            // Shuddering with the blow as it runs out
            s.hips[2] += 0.03 * (d.t * 24.0).sin() * (1.0 - d.t / d.len.max(0.1));
            pick((s, false), 30.0)
        }
        (Move::Quake, Stage::Recover) => pick(along(d.t, &[(0.0, pulled, Ease::Linear), (d.len, guard, Ease::Smooth)]), 6.0),
        (Move::Throw, _) if look == Look::Colossus => tear(&guard, &d, m),
        (Move::Throw, _) => throw(&guard, &d),
        // Its arms raised to the sky, its chest forced open and the fire in it blazing out, as it
        // calls the pillars up; then its fists thrown up one after the other as the rocks come down,
        // and at the last bent over, its hands on its knees, heaving
        (Move::Cataclysm, Stage::Wind) => {
            let gathered = Stance { hips: [0.0, 0.0, -0.35], lean: 0.15, bend: 0.4, nod: 0.1, hands: [[0.2, -1.3, 0.8], [0.2, 1.3, 0.8]], feet: wide, more: [0.5, 0.0, 0.0, 0.0, 0.0, 0.0], ..guard };
            let call = Stance { hips: [-0.1, 0.0, 0.0], lean: -0.08, bend: -0.35, nod: -0.5, hands: [[0.35, -1.3, 4.1], [0.35, 1.3, 4.1]], feet: wide, more: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0], ..guard };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len * 0.25, gathered, Ease::Gather), (d.len, call, Ease::Smooth)]), 7.0)
        }
        (Move::Cataclysm, Stage::Act) => {
            let mut s = Stance { hips: [-0.1, 0.0, 0.0], lean: -0.08, bend: -0.32, nod: -0.45, hands: [[0.35, -1.25, 3.7], [0.35, 1.25, 3.7]], feet: wide, more: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0], ..guard };
            let beat = d.t * 6.5;
            s.hands[0][2] += 0.45 * beat.sin().max(0.0);
            s.hands[1][2] += 0.45 * (beat + PI).sin().max(0.0);
            s.turn += 0.12 * (d.t * 3.0).sin();
            s.twist += 0.08 * beat.sin();
            pick((s, false), 16.0)
        }
        (Move::Cataclysm, Stage::Recover) => {
            let mut spent = Stance { hips: [0.05, 0.0, -0.3], lean: 0.25, bend: 0.55, nod: 0.25, hands: [[0.85, -0.6, 0.95], [0.85, 0.6, 0.95]], feet: wide, ..guard };
            spent.hips[2] += 0.05 * (d.t * 4.5).sin();
            spent.bend += 0.04 * (d.t * 4.5).sin();
            pick(along(d.t, &[(0.0, spent, Ease::Linear), (d.len * 0.75, spent, Ease::Linear), (d.len, guard, Ease::Smooth)]), 5.0)
        }
        (Move::Roar, Stage::Act) => roar(look, &guard, &d, m),
        (Move::Roar, Stage::Recover) => pick((guard, false), 5.0),
        (_, Stage::Reel) => {
            let reel = Stance { hips: [-0.38, 0.1, -0.12], lean: -0.22, bend: -0.28, tilt: 0.12, nod: -0.45, turn: 0.25, hands: [[0.1, -1.7, 2.2], [-0.25, 1.6, 1.8]], feet: [[-0.6, -0.6, ANKLE], [0.1, 0.5, ANKLE]], toes: [0.35, 0.0], ..guard };
            let caught = Stance { hips: [-0.25, 0.0, -0.3], lean: 0.1, bend: 0.25, nod: 0.15, hands: [[0.7, -1.2, 0.9], [0.6, 1.2, 1.0]], feet: [[-0.6, -0.6, ANKLE], [0.15, 0.55, ANKLE]], ..guard };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.16, reel, Ease::Gather), (d.len * 0.55, caught, Ease::Smooth), (d.len, guard, Ease::Smooth)]), 14.0)
        }
        (_, Stage::Stun) => {
            let mut dazed = Stance { hips: [0.0, 0.0, -0.2], lean: 0.1, bend: 0.3, nod: 0.4, hands: [[0.6, -1.15, 0.8], [0.6, 1.15, 0.8]], ..guard };
            dazed.turn += 0.3 * (d.t * 2.2).sin();
            dazed.roll += 0.06 * (d.t * 2.2).sin();
            pick((dazed, false), 5.0)
        }
        // Anything else: its arms drawn back, then thrust out at the player
        (_, Stage::Wind) => {
            let drawn = Stance { hips: [-0.1, 0.0, -0.05], bend: -0.15, hands: [[-0.2, -1.2, 2.0], [-0.2, 1.2, 2.0]], ..guard };
            let thrust = Stance { hips: [0.2, 0.0, -0.15], bend: 0.3, hands: [[1.6, -0.5, 1.9], [1.6, 0.5, 1.9]], feet: wide, ..guard };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), drawn, Ease::Gather), (d.len, thrust, Ease::Snap)]), 9.0)
        }
        (_, Stage::Act) => {
            let thrust = Stance { hips: [0.2, 0.0, -0.15], bend: 0.3, hands: [[1.6, -0.5, 1.9], [1.6, 0.5, 1.9]], feet: wide, ..guard };
            pick((thrust, false), 20.0)
        }
        (_, Stage::Recover) => pick((guard, false), 6.0),
    }
}

/// A golem scooping a boulder up off the ground to its right, raising it over its shoulder and
/// back behind its head, its other arm pointing at the player, then hurling it with its whole body
/// and following through across itself
fn throw(guard: &Stance, d: &Doing) -> (Stance, f64, f64) {
    let ankle = ANKLE;
    let holding = [0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
    let reach = Stance { hips: [0.1, -0.1, -0.38], lean: 0.25, bend: 0.5, twist: -0.3, nod: -0.2, hands: [[0.95, -0.9, 0.45], [0.8, 0.8, 1.2]], feet: [[0.3, -0.62, ankle], [-0.2, 0.5, ankle]], ..*guard };
    let scooped = Stance { more: holding, ..reach };
    let cocked = Stance { hips: [-0.15, 0.05, 0.04], lean: -0.08, bend: -0.25, twist: -0.5, nod: -0.05, hands: [[-0.7, -0.75, 3.6], [1.3, 0.7, 2.6]], feet: [[-0.2, -0.55, ankle], [0.45, 0.5, ankle]], more: holding, ..*guard };
    let hurled = Stance { hips: [0.3, -0.05, -0.12], lean: 0.18, bend: 0.35, twist: 0.45, nod: -0.25, hands: [[1.75, -0.25, 2.4], [-0.2, 0.9, 1.4]], feet: [[-0.1, -0.55, ankle], [0.55, 0.5, ankle]], toes: [0.35, 0.0], more: holding, ..*guard };
    let follow = Stance { hips: [0.35, 0.0, -0.22], lean: 0.25, bend: 0.5, twist: 0.6, hands: [[1.2, 0.45, 1.1], [-0.3, 0.95, 1.3]], more: [0.0; 6], ..hurled };
    let (s, quick) = match d.stage {
        Stage::Wind => {
            let lead = (d.len - SNAP).max(0.1);
            along(d.t, &[(0.0, *guard, Ease::Linear), (lead * 0.33, reach, Ease::Smooth), (lead * 0.4, scooped, Ease::Linear), (lead, cocked, Ease::Gather), (d.len, hurled, Ease::Snap)])
        }
        Stage::Act => along(d.t, &[(0.0, Stance { more: [0.0; 6], ..hurled }, Ease::Linear), (d.len, follow, Ease::Smooth)]),
        _ => along(d.t, &[(0.0, follow, Ease::Linear), (d.len, *guard, Ease::Smooth)]),
    };
    (s, if quick { 42.0 } else { 10.0 }, 0.0)
}

/// The Colossus tearing a boulder out of the ground: both fists driven into the ground in front of
/// it, heaving at it, bent low, harder as it comes loose; then torn free and lifted high overhead,
/// leaning back under the weight; then hurled down at the player
fn tear(guard: &Stance, d: &Doing, m: &Moment) -> (Stance, f64, f64) {
    let ankle = ANKLE;
    let feet = [[0.35, -0.66, ankle], [-0.3, 0.62, ankle]];
    let dig = Stance { hips: [0.3, 0.0, -0.46], lean: 0.3, bend: 0.56, nod: -0.3, hands: [[1.6, -0.38, 0.2], [1.6, 0.38, 0.2]], feet, more: [0.3, 0.0, 0.0, 0.0, 0.0, 0.0], ..*guard };
    let heave = Stance { hips: [0.12, 0.0, -0.36], lean: 0.16, bend: 0.28, nod: -0.4, hands: [[1.38, -0.4, 0.7], [1.38, 0.4, 0.7]], more: [0.8, 0.0, 0.0, 0.0, 0.0, 0.0], ..dig };
    let overhead = Stance { hips: [-0.15, 0.0, 0.0], lean: -0.1, bend: -0.32, nod: -0.25, hands: [[0.12, -0.42, 3.95], [0.12, 0.42, 3.95]], feet, more: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0], ..*guard };
    let hurl = Stance { hips: [0.3, 0.0, -0.2], lean: 0.2, bend: 0.4, nod: -0.3, hands: [[1.45, -0.4, 3.0], [1.45, 0.4, 3.0]], feet: [[0.5, -0.66, ankle], [-0.35, 0.62, ankle]], more: [0.6, 0.0, 0.0, 0.0, 0.0, 0.0], ..*guard };
    let follow = Stance { hips: [0.35, 0.0, -0.35], lean: 0.3, bend: 0.5, hands: [[1.55, -0.45, 1.3], [1.55, 0.45, 1.3]], more: [0.2, 0.0, 0.0, 0.0, 0.0, 0.0], ..hurl };
    let (s, quick) = match (d.stage, m.carry) {
        (Stage::Wind, Some(Carry::Prying(loose))) => {
            let (dug, quick) = along(d.t, &[(0.0, *guard, Ease::Linear), (0.22, dig, Ease::Snap)]);
            let pull = (0.5 - 0.5 * (d.t * 9.0).cos()) * (0.3 + 0.7 * loose);
            (if d.t < 0.22 { dug } else { mix(&dig, &heave, pull * 0.6) }, quick)
        }
        (Stage::Wind, _) => {
            let tear_at = bosses::TEAR_PRY * if m.enraged { 0.8 } else { 1.0 };
            let lift = (d.len - tear_at - SNAP).max(0.1);
            along(d.t - tear_at, &[(0.0, heave, Ease::Linear), (lift, overhead, Ease::Gather), (lift + SNAP, hurl, Ease::Snap)])
        }
        (Stage::Act, _) => along(d.t, &[(0.0, hurl, Ease::Linear), (d.len, follow, Ease::Smooth)]),
        _ => along(d.t, &[(0.0, follow, Ease::Linear), (d.len, *guard, Ease::Smooth)]),
    };
    (s, if quick { 40.0 } else { 9.0 }, 0.0)
}

/// Roaring: chest thrown out and arms flung wide and down, its head up and shaking. A raid boss
/// going into its next phase raises its fists high first and brings them down on the ground.
fn roar(look: Look, guard: &Stance, d: &Doing, m: &Moment) -> (Stance, f64, f64) {
    let wide = [[0.3, -0.66, ANKLE], [-0.3, 0.62, ANKLE]];
    let mut roaring = Stance { hips: [-0.1, 0.0, -0.15], lean: -0.05, bend: -0.3, nod: -0.45, hands: [[0.5, -1.8, 1.9], [0.5, 1.8, 1.9]], feet: wide, more: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0], ..*guard };
    let shaking = (d.t > 0.25 && d.t < d.len - 0.3) as u8 as f64;
    roaring.turn += 0.14 * (d.t * 9.0).sin() * shaking;
    roaring.bend += 0.04 * (d.t * 13.0).sin() * shaking;
    let big = look.is_raid() && m.stage > 0;
    let (s, quick) = if big {
        let up = Stance { hips: [-0.15, 0.0, 0.05], bend: -0.4, nod: -0.5, hands: [[0.2, -1.1, 4.0], [0.2, 1.1, 4.0]], feet: wide, more: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0], ..*guard };
        let down = Stance { hips: [0.2, 0.0, -0.45], lean: 0.3, bend: 0.5, nod: -0.4, hands: [[1.4, -0.7, 0.35], [1.4, 0.7, 0.35]], feet: wide, more: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0], ..*guard };
        along(d.t, &[(0.0, *guard, Ease::Linear), (0.35, up, Ease::Gather), (0.5, down, Ease::Snap), (0.75, down, Ease::Linear), (1.0, roaring, Ease::Smooth), (d.len, roaring, Ease::Linear)])
    } else {
        along(d.t, &[(0.0, *guard, Ease::Linear), (0.3, roaring, Ease::Gather), (d.len, roaring, Ease::Linear)])
    };
    (s, if quick { 40.0 } else { 12.0 }, 0.0)
}

/// Dying: thrown back, then buckling onto its knees, then falling on its face, to crumble
fn death(rest: &Stance, t: f64) -> (Stance, f64, f64) {
    let hit = Stance { hips: [-0.2, 0.0, 0.05], lean: -0.15, bend: -0.3, nod: -0.5, hands: [[0.2, -1.6, 2.0], [0.0, 1.5, 1.8]], ..*rest };
    let kneel = Stance { hips: [0.1, 0.0, -0.62], lean: 0.12, bend: 0.4, nod: 0.35, hands: [[0.7, -1.05, 0.5], [0.75, 1.05, 0.5]], feet: [[-0.25, -0.45, ANKLE], [-0.3, 0.45, ANKLE]], toes: [0.6, 0.6], ..*rest };
    let fallen = Stance { hips: [0.55, 0.0, -0.72], lean: 0.95, bend: 0.5, nod: 0.3, hands: [[2.1, -0.9, 0.35], [2.0, 0.95, 0.35]], feet: kneel.feet, toes: [0.8, 0.8], ..*rest };
    let (s, quick) = along(t, &[(0.0, *rest, Ease::Linear), (0.35, hit, Ease::Gather), (THUDS[0], kneel, Ease::Snap), (1.25, kneel, Ease::Linear), (THUDS[1], fallen, Ease::Snap), (DEATH, fallen, Ease::Linear)]);
    (s, if quick { 30.0 } else { 9.0 }, 0.0)
}

/// Its pose for what it's doing, its walk, breath and flinching laid over it, worked out into its
/// pieces; with the Colossus's plates forced apart as it strains, the boulder in its hand, and
/// dying, its pieces crumbling
pub(super) fn animate(r: &mut Rig, m: &Moment) {
    let look = r.look;
    let f = frame();
    let rest = rest(look);
    let (want, rate, free) = match m.dying {
        Some(t) => death(&rest, t),
        None => fight(look, &rest, m),
    };
    if !r.posed {
        *r.biped() = want;
    }
    ease(r.biped(), &want, rate, m.dt);
    let mut s = *r.biped();
    if m.dying.is_none() {
        biped::walk(&mut s, &GAIT, m, r.shuffle, if m.doing.is_none() { 1.0 } else { free });
        biped::idle(&mut s, m, r.clock, [r.glance[0], r.glance[1]], 1.6 * look.size().sqrt());
        biped::flinch(&mut s, r.flinched());
    }
    biped::solve(&f, &s, &mut r.bones, &mut r.sockets);
    let chest = r.bones[CHEST];
    if look == Look::Colossus {
        // Its plates grinding as it walks, and forced apart as it strains, the fire blazing out
        let open = s.more[OPEN].clamp(0.0, 1.2);
        let grind = m.pace().0 * (TAU * m.phase).sin();
        for (i, (y, z)) in [(0.2, 2.3), (-0.2, 2.3), (0.22, 1.8), (-0.22, 1.8)].into_iter().enumerate() {
            let (side, high) = (f64::signum(y), f64::signum(z - 2.05));
            let local = V::new(0.7 + open * 0.12, y * (1.0 + open * 0.9), z - f.waist + high * open * 0.3 + 0.025 * grind * side * high);
            r.bones[PLATES + i] = Bone::new(chest.at + chest.turn * local, chest.turn * rx(-side * high * open * 0.35));
        }
        r.bones[BLAZE] = Bone::new(chest.at + chest.turn * V::new(0.32 + 0.3 * open, 0.0, 0.7), chest.turn);
    }
    // The boulder in its right hand, beyond its fist along its forearm, and where it's held
    let holding = !matches!(look, Look::Colossus) && s.more[HOLDING] > 0.5;
    let fore = r.bones[FORE[0]];
    let along_arm = fore.turn * -V::z();
    if holding {
        r.bones[BOULDER] = Bone::new(r.sockets.hands[0] + along_arm * 0.62, fore.turn);
        r.sockets.grip = r.bones[BOULDER].at;
    } else {
        r.bones[BOULDER].shown = false;
        let mid = (r.sockets.hands[0] + r.sockets.hands[1]) * 0.5;
        let round = bosses::TORN_BOULDER / look.scale();
        let high = ((mid.z - 0.6) / 2.8).clamp(0.0, 1.0);
        let ahead = rz(s.yaw) * V::x();
        r.sockets.grip = mid + V::new(0.0, 0.0, round * (0.25 + 0.4 * high)) + ahead * (0.22 + round * 0.55) * (1.0 - high);
    }
    if let Some(t) = m.dying {
        if t >= CRUMBLE {
            if r.fallen.is_none() {
                r.fallen = Some(r.bones);
            }
            let from = r.fallen.unwrap();
            crumble(&mut r.bones, &from, (t - CRUMBLE) / (DEATH - CRUMBLE), r.seed);
        }
    } else {
        r.fallen = None;
    }
}
