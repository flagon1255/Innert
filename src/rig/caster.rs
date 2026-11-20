//! Those that fight with spells: the Sunken Pharaoh of the desert ruins, wrapped in linen under a
//! gold mask and a striped headdress, a sceptre with the sun burning in it in its hand; the Mire
//! Crone of the jungle shrines, hunched and mossy under her rags, leaning on a gnarled staff with a
//! skull lantern glowing venom; and the Hollow Mother of the Dreaming Red, a gaunt pale giantess with
//! no face, four long arms and a crown of bone, floating over the ground in a long red veil, blood
//! weeping from where her eyes should be and glowing in her heart. They thrust their staffs out to
//! hurl what's in them, sweep them round to fling swarms, raise them to call down the sky or call up
//! the dead, and the Hollow Mother raises all four arms to weep blood down all round the player.

use super::biped::{self, ARM, CHEST, EXTRA, FOOT, FORE, Frame, Gait, HEAD, PELVIS, SHIN, Stance, THIGH, TOOL, bone};
use super::*;
use crate::animals::{ball, cuboid, ellipsoid, limb};
use crate::worldgen::material::{BONE, FUR_DARK};

/// The Hollow Mother's lower arms: upper and fore, right then left
const LOW_ARM: [usize; 2] = [EXTRA, EXTRA + 2];
const LOW_FORE: [usize; 2] = [EXTRA + 1, EXTRA + 3];

pub const DEATH: f64 = 2.8;
const CRUMBLE: f64 = 1.5;
pub const THUDS: [f64; 2] = [0.8, 1.35];
const SNAP: f64 = 0.12;
const ANKLE: f64 = 0.06;
/// How far its staff reaches up past its hand, to what burns in it
const TOP: f64 = 0.66;

fn frame(look: Look) -> Frame {
    let long = look == Look::HollowMother;
    Frame {
        hip: 0.95,
        hip_w: 0.11,
        waist: 1.05,
        neck: V::new(0.03, 0.0, if long { 0.62 } else { 0.55 }),
        shoulder: V::new(0.0, if long { -0.24 } else { -0.2 }, if long { 0.46 } else { 0.4 }),
        arm: if long { [0.46, 0.46] } else { [0.32, 0.3] },
        leg: [0.48, 0.45],
        ankle: ANKLE,
        mouth: V::new(0.1, 0.0, 0.05),
        jaw: V::zeros(),
        tool: 0.92,
        grip: 0.0,
        elbow: V::new(-1.0, -0.6, -0.2),
        knee: 0.05,
    }
}

const GAIT: Gait = Gait { lift: 0.1, bob: 0.02, sway: 0.03, roll: 0.03, yaw: 0.08, swing: 0.1, lean: 0.1, stomp: 0.0 };

pub fn rest(look: Look) -> Stance {
    let f = frame(look);
    let mut s = match look {
        Look::BogHag => f.rest([[0.38, -0.26, 0.82], [0.32, 0.25, 0.72]]),
        Look::HollowMother => f.rest([[0.25, -0.5, 1.15], [0.25, 0.5, 1.15]]),
        _ => f.rest([[0.25, -0.28, 1.0], [0.1, 0.3, 0.92]]),
    };
    s.tool = [0.05, 0.0, -1.0];
    match look {
        // Hunched over her staff
        Look::BogHag => {
            s.lean = 0.12;
            s.bend = 0.45;
            s.nod = -0.3;
            s.hips[2] = -0.08;
        }
        // Floating, her lower arms folded before her
        Look::HollowMother => {
            s.hips[2] = 0.3;
            s.more = [0.25, -0.12, 1.25, 0.25, 0.12, 1.25];
        }
        _ => {}
    }
    s
}

pub fn pieces(look: Look) -> Vec<Vec<Part>> {
    let skin = look.skin();
    let (body, dark, accent, glow, extra) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let mut out = vec![Vec::new(); PIECES];
    match look {
        Look::SandPharaoh => {
            // Wrapped from head to foot in linen, banded darker, a gold belt and a striped apron
            out[PELVIS] = vec![ellipsoid([0.0, 0.0, 0.08], [0.13, 0.17, 0.04], accent), limb([0.12, 0.0, 0.05], [0.15, 0.0, -0.45], 0.06, 0.08, extra).stretched([0.5, 1.0, 1.0]).striped(accent), ellipsoid([0.0, 0.0, 0.0], [0.12, 0.16, 0.11], body).striped(dark)];
            out[CHEST] = vec![
                ball([0.14, 0.0, 0.22], 0.035, glow),
                ellipsoid([0.02, 0.0, 0.37], [0.17, 0.25, 0.06], extra),
                ellipsoid([0.02, 0.0, 0.36], [0.19, 0.27, 0.05], accent),
                ellipsoid([0.0, 0.0, 0.2], [0.13, 0.19, 0.25], body).striped(dark),
            ];
            let h = &mut out[HEAD];
            for s in [-1.0, 1.0] {
                h.push(ball([0.15, s * 0.04, 0.13], 0.022, glow));
                h.push(limb([0.02, s * 0.13, 0.12], [0.05, s * 0.12, -0.16], 0.05, 0.055, extra).stretched([0.5, 1.0, 1.0]).striped(accent));
            }
            h.push(limb([0.15, 0.0, 0.22], [0.17, 0.0, 0.3], 0.02, 0.012, accent));
            h.push(limb([0.12, 0.0, 0.01], [0.13, 0.0, -0.1], 0.025, 0.018, accent));
            h.push(ellipsoid([0.07, 0.0, 0.11], [0.1, 0.1, 0.12], accent));
            h.push(ellipsoid([-0.01, 0.0, 0.16], [0.14, 0.17, 0.15], extra).striped(accent));
            // A sceptre of gold banded teal, the sun burning in a ring at its top
            out[TOOL] = vec![
                ball([0.0, 0.0, TOP], 0.08, glow),
                ellipsoid([0.0, 0.0, TOP], [0.03, 0.13, 0.13], accent),
                limb([0.0, 0.0, TOP - 0.1], [0.0, 0.0, -0.92], 0.025, 0.025, extra).striped(accent),
            ];
        }
        Look::BogHag => {
            // Rags to the ground, a shawl, a hump, a long crooked nose and wild hair under a hood
            out[PELVIS] = vec![limb([0.0, 0.0, 0.05], [0.04, 0.0, -0.82], 0.15, 0.3, dark).striped(accent)];
            out[CHEST] = vec![ball([-0.12, 0.0, 0.32], 0.13, body), ellipsoid([0.0, 0.0, 0.3], [0.17, 0.22, 0.1], dark), ellipsoid([0.0, 0.0, 0.18], [0.12, 0.17, 0.22], body), limb([0.0, 0.0, -0.05], [0.0, 0.0, 0.4], 0.14, 0.12, accent)];
            let h = &mut out[HEAD];
            for s in [-1.0, 1.0] {
                h.push(ball([0.12, s * 0.04, 0.1], 0.022, glow));
            }
            h.push(limb([0.12, 0.0, 0.07], [0.24, 0.0, -0.04], 0.035, 0.012, body));
            h.push(ball([0.2, 0.02, 0.0], 0.018, FUR_DARK));
            for (y, long) in [(-0.1, 0.3), (-0.05, 0.4), (0.05, 0.38), (0.1, 0.28)] {
                h.push(limb([-0.02, y, 0.1], [-0.08, y * 1.4, -long], 0.025, 0.01, extra));
            }
            h.push(ellipsoid([-0.03, 0.0, 0.11], [0.13, 0.14, 0.15], dark));
            h.push(ellipsoid([0.04, 0.0, 0.08], [0.1, 0.1, 0.11], body));
            // A gnarled staff, a skull hung at its top with venom glowing in it
            out[TOOL] = vec![
                ball([0.07, 0.0, TOP - 0.02], 0.045, glow),
                ball([0.08, 0.0, TOP - 0.04], 0.07, BONE),
                limb([0.0, 0.0, TOP + 0.08], [0.05, 0.0, TOP - 0.05], 0.025, 0.02, extra),
                limb([0.0, 0.0, TOP + 0.08], [0.02, 0.0, 0.2], 0.03, 0.035, extra),
                limb([0.02, 0.0, 0.2], [0.0, 0.0, -0.92], 0.035, 0.028, extra),
            ];
        }
        _ => {
            // The Hollow Mother: a long red veil from her waist trailing to the ground, ragged at
            // its hem; gaunt bare ribs over a heart of blood; long black hair down her back; a face
            // with nothing in it but the blood running from where her eyes should be, and a crown
            // of bone
            let veil = &mut out[PELVIS];
            for (k, (y, long)) in [(-0.22, 0.95), (-0.1, 1.05), (0.0, 0.98), (0.1, 1.08), (0.22, 0.92)].into_iter().enumerate() {
                veil.push(limb([0.0, y * 0.6, -0.4], [-0.06 + 0.04 * (k % 2) as f64, y, -long], 0.12, 0.08, accent));
            }
            veil.push(limb([0.0, 0.0, 0.05], [0.0, 0.0, -0.55], 0.15, 0.34, accent));
            let c = &mut out[CHEST];
            c.push(ball([0.03, 0.0, 0.25], 0.07, glow));
            for k in 0..4 {
                let z = 0.12 + 0.07 * k as f64;
                for s in [-1.0, 1.0] {
                    c.push(limb([-0.08, s * 0.03, z], [0.1, s * 0.15, z - 0.04], 0.018, 0.016, body));
                }
            }
            c.push(limb([-0.08, 0.0, -0.05], [-0.09, 0.0, 0.5], 0.03, 0.035, body));
            c.push(limb([-0.12, 0.0, 0.55], [-0.2, 0.0, -0.35], 0.09, 0.05, dark).stretched([0.5, 1.4, 1.0]));
            for s in [-1.0, 1.0] {
                c.push(ball([0.0, s * 0.24, 0.46], 0.06, body));
            }
            c.push(ellipsoid([0.0, 0.0, 0.47], [0.08, 0.24, 0.05], body));
            let h = &mut out[HEAD];
            for s in [-1.0, 1.0] {
                h.push(ball([0.1, s * 0.045, 0.12], 0.028, dark));
                h.push(limb([0.12, s * 0.045, 0.1], [0.13, s * 0.05, -0.04], 0.012, 0.01, glow));
            }
            for k in 0..7 {
                let a = (k as f64 / 6.0 - 0.5) * 2.6;
                h.push(limb([0.0, 0.11 * a.sin(), 0.2], [-0.05, 0.16 * a.sin(), 0.35 + 0.06 * a.cos()], 0.018, 0.004, extra));
            }
            h.push(ellipsoid([0.02, 0.0, 0.11], [0.11, 0.1, 0.14], body));
            h.push(ellipsoid([-0.05, 0.0, 0.1], [0.1, 0.12, 0.16], dark));
            for k in 0..2 {
                let claws = |p: &mut Vec<Part>, at: f64| {
                    for y in [-0.025, 0.0, 0.025] {
                        p.push(limb([0.0, y, -at], [0.03, y * 1.6, -at - 0.14], 0.01, 0.004, extra));
                    }
                };
                out[LOW_ARM[k]] = vec![bone(0.36, 0.03, 0.026, body)];
                let mut fore = vec![ball([0.0, 0.0, 0.0], 0.03, body), bone(0.36, 0.026, 0.02, body)];
                claws(&mut fore, 0.36);
                out[LOW_FORE[k]] = fore;
                let mut upper_fore = vec![ball([0.0, 0.0, 0.0], 0.035, body), bone(0.46, 0.03, 0.022, body)];
                claws(&mut upper_fore, 0.46);
                out[FORE[k]] = upper_fore;
                out[ARM[k]] = vec![bone(0.46, 0.035, 0.03, body)];
            }
            return out;
        }
    }
    for k in 0..2 {
        out[ARM[k]] = vec![bone(0.32, 0.045, 0.04, if look == Look::BogHag { dark } else { body }).striped(dark)];
        out[FORE[k]] = vec![ball([0.0, 0.0, -0.3], 0.04, body), ellipsoid([0.0, 0.0, -0.22], [0.045, 0.045, 0.03], accent), bone(0.3, 0.038, 0.03, body)];
        if look == Look::SandPharaoh {
            out[THIGH[k]] = vec![bone(0.48, 0.06, 0.05, body).striped(dark)];
            out[SHIN[k]] = vec![bone(0.45, 0.05, 0.04, body).striped(dark)];
        }
        out[FOOT[k]] = vec![cuboid([-0.2, -0.15, -0.3], [0.25, 0.15, -ANKLE], AIR), ellipsoid([0.05, 0.0, -0.02], [0.1, 0.05, 0.04], dark)];
    }
    out
}

fn guard(look: Look, rest: &Stance) -> Stance {
    match look {
        Look::HollowMother => Stance { hands: [[0.35, -0.55, 1.45], [0.35, 0.55, 1.45]], more: [0.4, -0.3, 1.05, 0.4, 0.3, 1.05], ..*rest },
        Look::BogHag => Stance { hands: [[0.42, -0.24, 0.9], [0.4, 0.2, 0.95]], nod: -0.4, ..*rest },
        _ => Stance { hands: [[0.32, -0.26, 1.05], [0.25, 0.3, 1.1]], tool: [0.1, 0.0, -1.0], ..*rest },
    }
}

fn fight(look: Look, rest: &Stance, m: &Moment) -> (Stance, f64, f64) {
    let guard = if m.engaged { guard(look, rest) } else { *rest };
    let Some(d) = m.doing else {
        return (guard, 6.0, 1.0);
    };
    let pick = |(s, quick): (Stance, bool), slow: f64| (s, if quick { 40.0 } else { slow }, 0.0);
    let high = |s: &Stance| s.hips[2];
    let lift = high(&guard);
    // Its staff drawn back up behind it, then thrust out with what burns at its top at the player
    let drawn = Stance { hips: [-0.05, 0.0, lift], lean: -0.1, bend: if look == Look::BogHag { 0.25 } else { -0.15 }, hands: [[-0.08, -0.3, 1.6], [0.35, 0.35, 1.25]], tool: [0.35, 0.0, -0.94], more: [0.1, -0.5, 1.4, 0.1, 0.5, 1.4], ..guard };
    let thrust = Stance { hips: [0.1, 0.0, lift - 0.05], lean: 0.15, bend: if look == Look::BogHag { 0.55 } else { 0.15 }, hands: [[0.58, -0.15, 1.35], [0.2, 0.32, 1.1]], tool: [-0.95, 0.0, -0.25], more: [0.6, -0.25, 1.3, 0.6, 0.25, 1.3], ..guard };
    // Both arms raised high, the staff held up to the sky
    let raised = Stance { hips: [0.0, 0.0, lift + 0.05], lean: -0.1, bend: -0.15, nod: -0.45, hands: [[0.2, -0.32, 2.0], [0.2, 0.4, 1.95]], tool: [0.0, 0.0, -1.0], more: [0.15, -0.6, 1.85, 0.15, 0.6, 1.85], ..guard };
    // Swept low to its right, then round to its left, flinging what's in it
    let swept_from = Stance { twist: -0.7, lean: 0.1, hands: [[0.0, -0.6, 1.15], [0.3, 0.2, 1.25]], tool: [0.3, 0.9, -0.3], ..guard };
    let swept_to = Stance { twist: 0.6, lean: 0.12, hands: [[0.5, 0.35, 1.25], [-0.1, 0.45, 1.2]], tool: [0.3, -0.9, -0.3], ..guard };
    match (d.m, d.stage) {
        (Move::Spit | Move::Hex | Move::Lash, Stage::Wind) => {
            let mut wind = drawn;
            if d.m == Move::Hex {
                // Stirring something up between her hands, bent over it, then flinging it out
                wind = Stance { bend: 0.6, nod: 0.1, hands: [[0.32, -0.06, 0.95], [0.32, 0.06, 0.95]], tool: [0.2, -0.5, -0.85], ..guard };
                wind.hands[0][1] += 0.05 * (d.t * 12.0).sin();
                wind.hands[1][0] += 0.05 * (d.t * 12.0).cos();
            }
            let mut to = thrust;
            if d.m == Move::Hex {
                to.hands = [[0.55, -0.25, 1.2], [0.58, 0.25, 1.2]];
                to.tool = [0.2, -0.6, -0.8];
            }
            if d.m == Move::Lash {
                to.more = [1.0, -0.2, 1.1, 0.2, 0.3, 1.05];
            }
            pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), wind, Ease::Gather), (d.len, to, Ease::Snap)]), 10.0)
        }
        (Move::Spit | Move::Hex | Move::Lash, Stage::Act) => pick((thrust, false), 20.0),
        (Move::Scarabs | Move::Volley, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), swept_from, Ease::Gather), (d.len, swept_to, Ease::Snap)]), 10.0),
        (Move::Scarabs | Move::Volley, Stage::Act) => {
            // Sweeping back and forth as each swarm flies
            let f = 0.5 - 0.5 * (d.t * 7.8).cos();
            pick((mix(&swept_to, &swept_from, f), false), 18.0)
        }
        (Move::Summon | Move::Storm | Move::Weep | Move::Cataclysm | Move::Roar, Stage::Wind) => {
            let bowed = Stance { hips: [0.0, 0.0, lift - 0.12], bend: 0.45, nod: 0.5, hands: [[0.3, -0.12, 1.05], [0.3, 0.12, 1.05]], more: [0.3, -0.1, 0.95, 0.3, 0.1, 0.95], ..guard };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len * 0.4, bowed, Ease::Gather), (d.len, raised, Ease::Smooth)]), 9.0)
        }
        (Move::Summon | Move::Storm | Move::Weep | Move::Cataclysm | Move::Roar, Stage::Act) => {
            let mut s = raised;
            // Swaying, its arms trembling, its staff circling
            s.hands[0][0] += 0.12 * (d.t * 5.0).sin();
            s.hands[0][1] += 0.12 * (d.t * 5.0).cos();
            s.hands[1][2] += 0.04 * (d.t * 23.0).sin();
            s.more[2] += 0.05 * (d.t * 19.0).sin();
            s.more[5] += 0.05 * (d.t * 21.0).sin();
            s.turn += 0.15 * (d.t * 2.0).sin();
            s.hips[2] += 0.04 * (d.t * 3.0).sin();
            pick((s, false), 12.0)
        }
        // Its staff slammed down into the ground, roots or the dead coming up out of it
        (Move::Roots | Move::Slam | Move::RootWave, Stage::Wind) => {
            let up = Stance { hips: [-0.05, 0.0, lift + 0.03], hands: [[0.2, -0.25, 1.75], [0.25, -0.05, 1.7]], tool: [0.2, 0.0, -0.98], grip: 0.0, more: [0.3, -0.6, 1.6, 0.3, 0.6, 1.6], ..guard };
            let down = Stance { hips: [0.1, 0.0, lift - 0.12], lean: 0.2, bend: 0.3, hands: [[0.55, -0.2, 0.95], [0.5, 0.0, 0.95]], tool: [0.25, 0.0, -0.97], more: [0.7, -0.3, 0.6, 0.7, 0.3, 0.6], ..guard };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), up, Ease::Gather), (d.len, down, Ease::Snap)]), 10.0)
        }
        (Move::Roots | Move::Slam | Move::RootWave, Stage::Act) => pick((Stance { hips: [0.1, 0.0, lift - 0.12], lean: 0.2, bend: 0.3, hands: [[0.55, -0.2, 0.95], [0.5, 0.0, 0.95]], tool: [0.25, 0.0, -0.97], more: [0.7, -0.3, 0.6, 0.7, 0.3, 0.6], ..guard }, false), 20.0),
        (_, Stage::Reel) => {
            let reel = Stance { hips: [-0.25, 0.05, lift], lean: -0.2, bend: -0.2, nod: -0.45, turn: 0.3, hands: [[0.0, -0.5, 1.5], [-0.1, 0.5, 1.6]], tool: [-0.3, -0.3, -0.9], feet: [[-0.3, -0.12, ANKLE], [0.05, 0.12, ANKLE]], more: [-0.1, -0.6, 1.3, -0.1, 0.6, 1.3], ..guard };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.15, reel, Ease::Gather), (d.len, guard, Ease::Smooth)]), 14.0)
        }
        (_, Stage::Stun) => pick((Stance { nod: 0.4, bend: guard.bend + 0.2, ..guard }, false), 5.0),
        (_, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), drawn, Ease::Gather), (d.len, thrust, Ease::Snap)]), 10.0),
        (_, Stage::Act) => pick((thrust, false), 20.0),
        (_, Stage::Recover) => pick(along(d.t, &[(0.0, thrust, Ease::Linear), (d.len, guard, Ease::Smooth)]), 6.0),
    }
}

/// Dying: thrown back, sagging to its knees, then over; the Hollow Mother sinks down into her
/// veil
fn death(look: Look, rest: &Stance, t: f64) -> (Stance, f64, f64) {
    let hit = Stance { hips: [-0.15, 0.0, rest.hips[2] + 0.02], lean: -0.2, bend: -0.25, nod: -0.55, hands: [[0.1, -0.5, 1.5], [0.0, 0.5, 1.6]], more: [0.0, -0.6, 1.3, 0.0, 0.6, 1.3], ..*rest };
    let knees = Stance { hips: [0.05, 0.0, -0.45], lean: 0.25, bend: 0.35, nod: 0.45, hands: [[0.4, -0.3, 0.45], [0.4, 0.3, 0.45]], tool: [0.6, -0.4, -0.7], feet: [[-0.2, -0.12, ANKLE], [-0.25, 0.12, ANKLE]], toes: [0.8, 0.8], more: [0.4, -0.3, 0.3, 0.4, 0.3, 0.3], ..*rest };
    let fallen = Stance { hips: [0.3, 0.0, if look == Look::HollowMother { -0.75 } else { -0.6 }], lean: 0.85, bend: 0.4, nod: 0.5, hands: [[0.9, -0.4, 0.08], [0.9, 0.4, 0.08]], more: [0.8, -0.3, 0.05, 0.8, 0.3, 0.05], ..knees };
    let (s, quick) = along(t, &[(0.0, *rest, Ease::Linear), (0.3, hit, Ease::Gather), (THUDS[0], knees, Ease::Snap), (1.0, knees, Ease::Linear), (THUDS[1], fallen, Ease::Snap), (DEATH, fallen, Ease::Linear)]);
    (s, if quick { 30.0 } else { 10.0 }, 0.0)
}

pub(super) fn animate(r: &mut Rig, m: &Moment) {
    let look = r.look;
    let f = frame(look);
    let rest = rest(look);
    let (mut want, rate, free) = match m.dying {
        Some(t) => death(look, &rest, t),
        None => fight(look, &rest, m),
    };
    // The Hollow Mother floats, rising and falling, never touching the ground; and tall as she
    // is, her arms reach the higher for what's over her
    if look == Look::HollowMother && m.dying.is_none() {
        want.hips[2] += 0.06 * (r.clock * 1.3).sin();
        for k in 0..2 {
            want.hands[k][2] += 0.6 * (want.hands[k][2] - 1.4).max(0.0);
            want.more[k * 3 + 2] += 0.6 * (want.more[k * 3 + 2] - 1.3).max(0.0);
        }
    }
    let s = biped::pose(r, m, &f, &GAIT, (want, rate, free), 1.0);
    // What it casts comes out of the top of its staff; the Hollow Mother's out from between her hands
    let up = r.bones[TOOL].turn * V::z();
    r.sockets.cast = r.sockets.hands[0] + up * TOP;
    if look == Look::HollowMother {
        r.sockets.cast = (r.sockets.hands[0] + r.sockets.hands[1]) * 0.5;
        // Her lower arms, from just under her shoulders, reaching for where her stance says
        let chest = r.bones[CHEST];
        for k in 0..2 {
            let side = if k == 0 { 1.0 } else { -1.0 };
            let shoulder = chest.at + chest.turn * V::new(0.0, -side * 0.2, 0.2);
            let target = V::new(s.more[k * 3], s.more[k * 3 + 1], s.more[k * 3 + 2]) + V::new(0.0, 0.0, s.hips[2] - rest.hips[2]);
            let pole = chest.turn * V::new(-1.0, -side * 0.6, -0.3);
            let (elbow, wrist) = bend(shoulder, target, 0.36, 0.36, pole);
            r.bones[LOW_ARM[k]] = Bone::new(shoulder, aim(-V::z(), -V::x(), elbow - shoulder, pole));
            r.bones[LOW_FORE[k]] = Bone::new(elbow, aim(-V::z(), -V::x(), wrist - elbow, pole));
        }
        if m.dying.is_none() {
            for k in [THIGH[0], THIGH[1], SHIN[0], SHIN[1], FOOT[0], FOOT[1]] {
                r.bones[k].shown = false;
            }
        }
    }
    if look == Look::SandPharaoh {
        biped::fall_apart(r, m, CRUMBLE, DEATH);
    }
}
