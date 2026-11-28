//! Giants: the Cragmaw Ogre of the hills and old castles, the Rime Jotun of the cold, and the
//! Thunderborn Giant of the high stone. Huge and heavy, a barrel of a body on thick legs, a club,
//! a frozen maul or a bronze-headed hammer in its right hand, resting on its shoulder as it walks.
//! It raises it in both hands and smashes it down; the ogre and the giant swat with it backhanded,
//! knocking whoever it catches flying; the jotun smashes it down so boulders of ice burst up out of
//! the ground and fly at the player; the giant claps its hands overhead so thunder runs out across
//! the ground and lightning comes down. Dying, it staggers, drops to its knees and falls on its face.
//!
//! And two more, see src\bosses\roster.rs: the Slagwright, a smith of black iron with the forge's
//! fire showing through the grate in its belly, a forge hammer in its right hand and a crucible of
//! slag in its left, which it strikes so the slag flies up and comes down all round the player; and
//! the Grovemaul, a great hunched ape of the jungle, mossy-backed and hung with vines, long-armed
//! and bare-fisted, which beats its chest, leaps high and comes down fists first.

use super::biped::{self, ARM, CHEST, EXTRA, FOOT, FORE, Frame, Gait, HEAD, PELVIS, SHIN, Stance, THIGH, TOOL, bone};
use super::*;
use crate::animals::{ball, cuboid, ellipsoid, limb};
use crate::worldgen::material::{BRONZE, DARK_STEEL, FUR_DARK, GLOW_STORM, ICE, LEATHER, MOSS, ROCK, STEEL, WOOD};

/// A boulder in its left hand, to throw
const BOULDER: usize = EXTRA;
/// Its own number in its Stance: whether it's holding the boulder
const HOLDING: usize = 0;

pub const DEATH: f64 = 3.0;
pub const THUDS: [f64; 2] = [0.95, 1.65];
const SNAP: f64 = 0.15;
const ANKLE: f64 = 0.1;

fn frame(look: Look) -> Frame {
    // The ape: short-legged, its head thrust forward, its arms long enough to go on its knuckles
    if look == Look::Grovemaul {
        return Frame {
            hip: 0.92,
            hip_w: 0.24,
            waist: 1.02,
            neck: V::new(0.22, 0.0, 0.78),
            shoulder: V::new(0.05, -0.5, 0.7),
            arm: [0.78, 0.72],
            leg: [0.5, 0.5],
            ankle: ANKLE,
            mouth: V::new(0.3, 0.0, 0.0),
            jaw: V::zeros(),
            tool: 0.3,
            grip: 0.0,
            elbow: V::new(-1.0, -0.6, -0.2),
            knee: 0.25,
        };
    }
    Frame {
        hip: 1.15,
        hip_w: 0.22,
        waist: 1.25,
        neck: V::new(0.1, 0.0, 0.95),
        shoulder: V::new(0.02, -0.45, 0.78),
        arm: [0.62, 0.6],
        leg: [0.6, 0.6],
        ankle: ANKLE,
        mouth: V::new(0.24, 0.0, 0.05),
        jaw: V::zeros(),
        tool: 1.55,
        grip: 0.28,
        elbow: V::new(-1.0, -0.5, -0.2),
        knee: 0.12,
    }
}

const GAIT: Gait = Gait { lift: 0.18, bob: 0.06, sway: 0.1, roll: 0.05, yaw: 0.1, swing: 0.28, lean: 0.15, stomp: 0.06 };

/// Standing, its weapon resting back on its right shoulder; the Slagwright with its crucible in its
/// left hand, the Grovemaul hunched forward, its fists low
pub fn rest(look: Look) -> Stance {
    let f = frame(look);
    match look {
        Look::Grovemaul => {
            let mut s = f.rest([[0.62, -0.62, 0.62], [0.62, 0.62, 0.62]]);
            s.hips = [-0.05, 0.0, -0.06];
            s.lean = 0.3;
            s.bend = 0.25;
            s.nod = -0.35;
            s.tool = [0.3, 0.0, -0.95];
            s
        }
        _ => {
            let mut s = f.rest([[0.3, -0.55, 1.35], [0.25, 0.62, 1.0]]);
            s.tool = [-0.45, 0.08, 0.89];
            if look == Look::Slagwright {
                s.hands[1] = [0.38, 0.6, 1.05];
                s.more[HOLDING] = 1.0;
            }
            s
        }
    }
}

pub fn pieces(look: Look) -> Vec<Vec<Part>> {
    match look {
        Look::Slagwright => return slagwright(look),
        Look::Grovemaul => return grovemaul(look),
        _ => {}
    }
    let skin = look.skin();
    let (body, dark, accent, glow, extra) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let mut out = vec![Vec::new(); PIECES];
    // A kilt of fur or hide round its hips, belted
    out[PELVIS] = vec![
        ball([0.0, 0.22, 0.0], 0.2, body),
        ball([0.0, -0.22, 0.0], 0.2, body),
        ellipsoid([0.0, 0.0, 0.12], [0.35, 0.44, 0.07], dark),
        ellipsoid([0.02, 0.0, -0.05], [0.33, 0.42, 0.28], if look == Look::StormGiant { dark } else { accent }),
    ];
    // A barrel of a body: a great belly, a broad chest, heavy shoulders
    let c = &mut out[CHEST];
    match look {
        Look::Ogre => {
            // A strap of leather across it and a bone hung on it
            c.push(limb([0.38, -0.4, 0.75], [0.42, 0.4, 0.2], 0.05, 0.05, LEATHER));
            c.push(ellipsoid([0.48, 0.15, 0.42], [0.04, 0.08, 0.1], extra));
        }
        Look::RimeJotun => {
            // White furs over its shoulders, and ice grown out of its back
            for s in [-1.0, 1.0] {
                c.push(ellipsoid([0.0, s * 0.42, 0.88], [0.3, 0.26, 0.16], accent));
            }
            for (y, z, h) in [(0.2, 0.6, 0.45), (-0.18, 0.45, 0.5), (0.0, 0.75, 0.4)] {
                c.push(limb([-0.3, y, z], [-0.6, y * 1.4, z + h], 0.09, 0.02, extra));
            }
        }
        _ => {
            // Bands of bronze across its chest, and runes glowing between them
            for z in [0.45, 0.68] {
                c.push(ellipsoid([0.05, 0.0, z], [0.42, 0.57, 0.05], accent));
            }
            for y in [-0.2, 0.0, 0.2] {
                c.push(ball([0.42, y, 0.56], 0.04, glow));
            }
            for s in [-1.0, 1.0] {
                c.push(ellipsoid([0.0, s * 0.45, 0.88], [0.24, 0.22, 0.12], accent));
            }
        }
    }
    c.push(ellipsoid([0.12, 0.0, 0.3], [0.42, 0.46, 0.42], body));
    c.push(ellipsoid([0.02, 0.0, 0.62], [0.38, 0.55, 0.35], body));
    c.push(ellipsoid([-0.05, 0.0, 0.85], [0.25, 0.4, 0.14], body));
    for s in [-1.0, 1.0] {
        c.push(ball([0.02, s * 0.45, 0.78], 0.22, body));
    }
    // Its head, low on its shoulders: a heavy jaw, a brow over small eyes
    let h = &mut out[HEAD];
    for s in [-1.0, 1.0] {
        h.push(ball([0.24, s * 0.075, 0.19], 0.03, glow));
    }
    h.push(ellipsoid([0.2, 0.0, 0.25], [0.08, 0.17, 0.04], dark));
    h.push(ball([0.27, 0.0, 0.12], 0.05, body));
    match look {
        Look::Ogre => {
            // Tusks up out of its jaw, ears sticking out, and a knot of hair on top
            for s in [-1.0, 1.0] {
                h.push(limb([0.27, s * 0.09, -0.03], [0.31, s * 0.12, 0.12], 0.03, 0.008, extra));
                h.push(ellipsoid([0.0, s * 0.2, 0.18], [0.05, 0.06, 0.08], body));
            }
            h.push(ball([-0.02, 0.0, 0.38], 0.08, dark));
        }
        Look::RimeJotun => {
            // A beard of icicles, and a hood of white fur
            for (y, long) in [(-0.1, 0.35), (-0.04, 0.45), (0.03, 0.42), (0.1, 0.32)] {
                h.push(limb([0.22, y, -0.02], [0.22, y * 1.2, -long], 0.04, 0.008, ICE));
            }
            h.push(ellipsoid([0.0, 0.0, 0.22], [0.22, 0.22, 0.22], accent));
        }
        _ => {
            // Hair streaming back crackling with lightning, and a bronze circlet
            for (y, up) in [(-0.1, 0.25), (0.0, 0.32), (0.1, 0.25), (-0.05, 0.15), (0.05, 0.15)] {
                h.push(limb([0.0, y, 0.3], [-0.3, y * 1.5, 0.3 + up], 0.05, 0.01, GLOW_STORM));
            }
            h.push(ellipsoid([0.06, 0.0, 0.27], [0.2, 0.2, 0.04], accent));
            h.push(ellipsoid([0.18, 0.0, -0.02], [0.08, 0.15, 0.12], extra));
        }
    }
    h.push(ellipsoid([0.16, 0.0, 0.0], [0.17, 0.2, 0.1], body));
    h.push(ellipsoid([0.08, 0.0, 0.15], [0.2, 0.19, 0.22], body));
    // Arms like tree trunks, bracers on its forearms
    for k in 0..2 {
        out[ARM[k]] = vec![bone(0.62, 0.13, 0.11, body)];
        if look == Look::StormGiant {
            out[ARM[k]].insert(0, ellipsoid([0.0, 0.0, -0.08], [0.17, 0.17, 0.1], accent));
        }
        out[FORE[k]] = vec![ellipsoid([0.0, 0.0, -0.42], [0.12, 0.12, 0.13], dark), ball([0.0, 0.0, -0.62], 0.13, body), ball([0.0, 0.0, 0.0], 0.11, body), bone(0.6, 0.11, 0.1, body)];
        out[THIGH[k]] = vec![bone(0.6, 0.17, 0.15, body)];
        out[SHIN[k]] = vec![ball([0.0, 0.0, 0.0], 0.14, body), ellipsoid([0.0, 0.0, -0.4], [0.14, 0.14, 0.14], if look == Look::Ogre { LEATHER } else { dark }), bone(0.6, 0.14, 0.12, body)];
        out[FOOT[k]] = vec![cuboid([-0.4, -0.3, -0.5], [0.5, 0.3, -ANKLE], AIR), ellipsoid([0.09, 0.0, -0.03], [0.21, 0.12, 0.09], dark), ball([0.0, 0.0, 0.0], 0.1, body)];
    }
    // What's in its right hand, pointing down from it: a studded club, a maul with a head of ice,
    // or a bronze-headed hammer with lightning in it
    let t = &mut out[TOOL];
    match look {
        Look::Ogre => {
            for k in 0..5 {
                let a = k as f64 * 1.3;
                t.push(ball([0.15 * a.cos(), 0.15 * a.sin(), -1.05 - 0.09 * k as f64], 0.05, DARK_STEEL));
            }
            t.push(limb([0.0, 0.0, -0.55], [0.0, 0.0, -1.5], 0.11, 0.19, WOOD));
            t.push(limb([0.0, 0.0, 0.2], [0.0, 0.0, -0.6], 0.05, 0.06, LEATHER));
        }
        Look::RimeJotun => {
            for k in 0..4 {
                let a = k as f64 * TAU / 4.0 + 0.4;
                t.push(limb([0.0, 0.0, -1.3], [0.32 * a.cos(), 0.32 * a.sin(), -1.25], 0.07, 0.01, extra));
            }
            t.push(cuboid([-0.22, -0.26, -1.55], [0.22, 0.26, -1.05], ICE));
            t.push(limb([0.0, 0.0, 0.22], [0.0, 0.0, -1.1], 0.05, 0.05, STEEL));
        }
        _ => {
            t.push(ball([0.0, 0.0, -1.32], 0.12, glow));
            t.push(ellipsoid([0.0, 0.0, -1.32], [0.22, 0.32, 0.22], BRONZE));
            t.push(limb([0.0, 0.0, 0.25], [0.0, 0.0, -1.15], 0.045, 0.045, DARK_STEEL));
            t.push(ball([0.0, 0.0, 0.27], 0.07, extra));
        }
    }
    out[BOULDER] = match look {
        Look::RimeJotun => vec![ellipsoid([0.15, 0.1, 0.2], [0.1, 0.18, 0.1], extra), ball([0.0, 0.0, 0.0], 0.42, ICE)],
        _ => vec![ellipsoid([0.15, 0.1, 0.2], [0.12, 0.18, 0.1], FUR_DARK), ball([0.0, 0.0, 0.0], 0.4, ROCK)],
    };
    out
}

/// The Slagwright: black iron banded with bronze, the forge's fire glowing through a grate in its
/// belly and in the seams of its plates, a leather apron, a helm with a slit of fire for its eyes,
/// a forge hammer, and a crucible of slag hung from its left fist
fn slagwright(look: Look) -> Vec<Vec<Part>> {
    let skin = look.skin();
    let (body, dark, apron, glow, band) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let mut out = vec![Vec::new(); PIECES];
    out[PELVIS] = vec![
        ellipsoid([0.0, 0.0, 0.12], [0.36, 0.45, 0.07], band),
        cuboid([0.24, -0.3, -0.72], [0.38, 0.3, 0.08], apron),
        ball([0.0, 0.24, 0.0], 0.21, body),
        ball([0.0, -0.24, 0.0], 0.21, body),
        ellipsoid([0.0, 0.0, -0.02], [0.32, 0.42, 0.24], dark),
    ];
    let c = &mut out[CHEST];
    // The grate over the fire in its belly, and the fire behind it
    for z in [0.2, 0.31, 0.42] {
        c.push(cuboid([0.5, -0.2, z - 0.025], [0.6, 0.2, z + 0.025], dark));
    }
    for y in [-0.12, 0.0, 0.12] {
        c.push(cuboid([0.5, y - 0.022, 0.15], [0.6, y + 0.022, 0.47], dark));
    }
    c.push(ball([0.4, 0.0, 0.31], 0.21, glow));
    // Seams of fire between its plates, bronze bands over its shoulders, the apron's strap
    for (from, to) in [([0.3, -0.5, 0.75], [0.46, -0.25, 0.55]), ([0.3, 0.5, 0.75], [0.46, 0.25, 0.55]), ([-0.25, -0.35, 0.3], [-0.3, 0.35, 0.45])] {
        c.push(limb(from, to, 0.028, 0.028, glow));
    }
    for s in [-1.0, 1.0] {
        c.push(ellipsoid([0.0, s * 0.46, 0.9], [0.26, 0.24, 0.13], band));
        c.push(limb([0.42, s * 0.2, 0.62], [0.12, s * 0.38, 0.95], 0.04, 0.04, apron));
    }
    c.push(ellipsoid([0.12, 0.0, 0.3], [0.43, 0.47, 0.42], body));
    c.push(ellipsoid([0.02, 0.0, 0.62], [0.39, 0.56, 0.35], body));
    c.push(ellipsoid([-0.05, 0.0, 0.85], [0.26, 0.4, 0.14], body));
    for s in [-1.0, 1.0] {
        c.push(ball([0.02, s * 0.46, 0.78], 0.22, body));
    }
    // A helm of iron, a slit of fire for its eyes, a bronze crest and a beard of iron rods
    let h = &mut out[HEAD];
    h.push(cuboid([0.26, -0.13, 0.17], [0.31, 0.13, 0.215], glow));
    h.push(limb([-0.2, 0.0, 0.42], [0.22, 0.0, 0.4], 0.05, 0.035, band));
    for (y, long) in [(-0.1, 0.22), (-0.035, 0.3), (0.035, 0.3), (0.1, 0.22)] {
        h.push(limb([0.24, y, 0.02], [0.27, y * 1.1, -long], 0.03, 0.015, dark));
    }
    h.push(ellipsoid([0.08, 0.0, 0.18], [0.24, 0.22, 0.25], dark));
    h.push(ellipsoid([0.16, 0.0, 0.0], [0.16, 0.19, 0.1], body));
    for k in 0..2 {
        out[ARM[k]] = vec![ellipsoid([0.0, 0.0, -0.05], [0.18, 0.18, 0.1], band), bone(0.62, 0.14, 0.11, body)];
        out[FORE[k]] = vec![ellipsoid([0.0, 0.0, -0.42], [0.13, 0.13, 0.12], band), ball([0.0, 0.0, -0.62], 0.13, dark), ball([0.0, 0.0, 0.0], 0.11, body), bone(0.6, 0.11, 0.1, body)];
        out[THIGH[k]] = vec![bone(0.6, 0.17, 0.15, body)];
        out[SHIN[k]] = vec![ball([0.0, 0.0, 0.0], 0.14, body), ellipsoid([0.0, 0.0, -0.42], [0.15, 0.15, 0.15], dark), bone(0.6, 0.14, 0.12, body)];
        out[FOOT[k]] = vec![cuboid([-0.4, -0.3, -0.5], [0.5, 0.3, -ANKLE], AIR), ellipsoid([0.09, 0.0, -0.03], [0.22, 0.13, 0.09], dark), ball([0.0, 0.0, 0.0], 0.1, body)];
    }
    // A forge hammer, its iron head's striking faces glowing from the work
    out[TOOL] = vec![
        cuboid([-0.15, -0.34, -1.52], [0.15, -0.3, -1.2], glow),
        cuboid([-0.15, 0.3, -1.52], [0.15, 0.34, -1.2], glow),
        cuboid([-0.18, -0.3, -1.56], [0.18, 0.3, -1.16], dark),
        limb([0.0, 0.0, -1.16], [0.0, 0.0, -1.05], 0.07, 0.06, band),
        limb([0.0, 0.0, 0.22], [0.0, 0.0, -1.1], 0.05, 0.05, WOOD),
    ];
    // Its crucible, glowing slag brimming in it, hung from its fist on a chain
    out[BOULDER] = vec![
        ellipsoid([0.0, 0.0, 0.17], [0.22, 0.22, 0.06], glow),
        limb([0.0, 0.0, 0.45], [0.0, 0.0, 0.2], 0.03, 0.03, dark),
        ellipsoid([0.0, 0.0, 0.17], [0.3, 0.3, 0.06], band),
        ellipsoid([0.0, 0.0, 0.0], [0.29, 0.29, 0.24], dark),
    ];
    out
}

/// The Grovemaul: a great ape, dark-furred and hunched, a hump of moss on its back and vines hung
/// off its shoulders, a leathery face under a heavy brow with gold eyes, huge fists, short legs
fn grovemaul(look: Look) -> Vec<Vec<Part>> {
    let skin = look.skin();
    let (fur, face, moss, eyes, vine) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let mut out = vec![Vec::new(); PIECES];
    out[PELVIS] = vec![ball([0.0, 0.25, 0.0], 0.22, fur), ball([0.0, -0.25, 0.0], 0.22, fur), ellipsoid([0.0, 0.0, 0.05], [0.33, 0.43, 0.26], fur)];
    let c = &mut out[CHEST];
    // Vines hanging off its shoulders, leaves on them, moss on its hump
    for (y, x, long) in [(-0.45, 0.1, 0.55), (-0.3, -0.1, 0.75), (0.35, 0.05, 0.65), (0.5, -0.12, 0.5), (0.0, -0.35, 0.7)] {
        c.push(limb([x, y, 0.95], [x - 0.12, y * 1.1, 0.95 - long], 0.03, 0.02, vine));
        c.push(ellipsoid([x - 0.06, y * 1.05, 0.95 - long * 0.6], [0.06, 0.06, 0.03], crate::worldgen::material::LEAVES));
    }
    c.push(ellipsoid([-0.22, 0.0, 0.8], [0.34, 0.5, 0.3], moss));
    c.push(ellipsoid([0.34, 0.0, 0.42], [0.18, 0.34, 0.3], face));
    c.push(ellipsoid([0.1, 0.0, 0.32], [0.42, 0.48, 0.42], fur));
    c.push(ellipsoid([0.0, 0.0, 0.62], [0.4, 0.6, 0.36], fur));
    for s in [-1.0, 1.0] {
        c.push(ball([0.05, s * 0.5, 0.72], 0.24, fur));
    }
    // Its head thrust forward: a heavy brow, gold eyes deep under it, a broad leathery muzzle, a
    // crest of fur
    let h = &mut out[HEAD];
    for s in [-1.0, 1.0] {
        h.push(ball([0.25, s * 0.08, 0.13], 0.03, eyes));
        h.push(ball([0.05, s * 0.2, 0.08], 0.05, face));
    }
    h.push(ellipsoid([0.23, 0.0, 0.2], [0.09, 0.19, 0.05], fur));
    h.push(ellipsoid([0.3, 0.0, -0.04], [0.13, 0.16, 0.12], face));
    h.push(ellipsoid([-0.02, 0.0, 0.32], [0.17, 0.08, 0.12], FUR_DARK));
    h.push(ellipsoid([0.1, 0.0, 0.1], [0.21, 0.2, 0.21], fur));
    let frame = frame(look);
    for k in 0..2 {
        let [arm, fore] = frame.arm;
        out[ARM[k]] = vec![ellipsoid([-0.05, 0.0, -0.3], [0.1, 0.12, 0.2], moss), bone(arm, 0.16, 0.13, fur)];
        out[FORE[k]] = vec![ball([0.04, 0.0, -fore], 0.18, face), ball([0.0, 0.0, 0.0], 0.13, fur), bone(fore, 0.13, 0.13, fur)];
        let [thigh, shin] = frame.leg;
        out[THIGH[k]] = vec![bone(thigh, 0.18, 0.15, fur)];
        out[SHIN[k]] = vec![ball([0.0, 0.0, 0.0], 0.15, fur), bone(shin, 0.14, 0.12, fur)];
        out[FOOT[k]] = vec![cuboid([-0.4, -0.3, -0.5], [0.5, 0.3, -ANKLE], AIR), ellipsoid([0.12, 0.0, -0.03], [0.25, 0.14, 0.08], face), ball([0.0, 0.0, 0.0], 0.11, fur)];
    }
    // A mossy rock it tears up to throw
    out[BOULDER] = vec![ellipsoid([0.1, 0.12, 0.25], [0.22, 0.2, 0.09], MOSS), ball([0.0, 0.0, 0.0], 0.42, ROCK)];
    out
}

fn guard(look: Look, rest: &Stance) -> Stance {
    if look == Look::Grovemaul {
        return Stance { hips: [0.0, 0.0, -0.1], lean: 0.35, bend: 0.25, nod: -0.4, hands: [[0.75, -0.6, 0.75], [0.75, 0.6, 0.75]], feet: [[0.15, -0.32, ANKLE], [-0.15, 0.32, ANKLE]], ..*rest };
    }
    let mut s = Stance { hips: [0.0, 0.0, -0.08], lean: 0.06, bend: 0.08, hands: [[0.45, -0.55, 1.45], [0.5, 0.55, 1.3]], tool: [0.3, 0.05, 0.95], feet: [[0.15, -0.3, ANKLE], [-0.15, 0.3, ANKLE]], ..*rest };
    if look == Look::Slagwright {
        s.hands[1] = [0.55, 0.55, 1.15];
    }
    s
}

fn fight(look: Look, rest: &Stance, m: &Moment) -> (Stance, f64, f64) {
    let guard = if m.engaged { guard(look, rest) } else { *rest };
    let Some(d) = m.doing else {
        return (guard, 6.0, 1.0);
    };
    let pick = |(s, quick): (Stance, bool), slow: f64| (s, if quick { 42.0 } else { slow }, 0.0);
    let wide = [[0.3, -0.36, ANKLE], [-0.3, 0.36, ANKLE]];
    // The Slagwright's hammer raised high in its right hand, its crucible held out in its left, then
    // the hammer brought down on it, the slag flying, and held there shaking
    if d.m == Move::Slagrain && matches!(d.stage, Stage::Wind | Stage::Act) {
        let lifted = Stance { hips: [-0.08, 0.0, 0.0], lean: -0.08, bend: -0.2, twist: -0.25, nod: -0.15, hands: [[0.0, -0.45, 2.6], [0.85, 0.3, 1.35]], tool: [-0.6, -0.15, 0.78], feet: wide, ..guard };
        let struck = Stance { hips: [0.15, 0.0, -0.2], lean: 0.2, bend: 0.3, twist: 0.2, nod: 0.2, hands: [[0.35, -0.3, 2.1], [0.85, 0.3, 1.3]], tool: [0.4, 0.55, -0.75], feet: wide, toes: [0.0, 0.25], ..guard };
        if d.stage == Stage::Wind {
            let lead = (d.len - SNAP).max(0.05);
            return pick(along(d.t, &[(0.0, guard, Ease::Linear), (lead, lifted, Ease::Gather), (d.len, struck, Ease::Snap)]), 10.0);
        }
        let mut s = struck;
        s.hands[1][2] += 0.04 * (d.t * 40.0).sin();
        s.bend += 0.02 * (d.t * 33.0).sin();
        return pick((s, false), 25.0);
    }
    // The Grovemaul crouching low, its arms swung back, springing up with its fists raised high
    // over its head and coming down with them on the player
    if look == Look::Grovemaul && matches!(d.m, Move::Pounce | Move::Leap) && matches!(d.stage, Stage::Wind | Stage::Act) {
        let crouched = Stance { hips: [-0.15, 0.0, -0.32], lean: 0.55, bend: 0.3, nod: -0.5, hands: [[-0.2, -0.65, 0.65], [-0.2, 0.65, 0.65]], feet: [[0.25, -0.38, ANKLE], [-0.25, 0.38, ANKLE]], toes: [0.2, 0.2], ..guard };
        let flying = Stance { hips: [0.0, 0.0, 0.1], lean: -0.15, bend: -0.3, nod: -0.3, hands: [[0.25, -0.4, 2.75], [0.25, 0.4, 2.75]], feet: [[0.25, -0.3, 0.45], [-0.3, 0.3, 0.6]], toes: [0.5, 0.5], ..guard };
        let landed = Stance { hips: [0.2, 0.0, -0.4], lean: 0.6, bend: 0.45, nod: 0.1, hands: [[1.3, -0.3, 0.3], [1.3, 0.3, 0.3]], feet: [[0.35, -0.38, ANKLE], [-0.3, 0.38, ANKLE]], ..guard };
        return if d.stage == Stage::Wind {
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len, crouched, Ease::Gather)]), 10.0)
        } else {
            pick(along(d.t, &[(0.0, crouched, Ease::Linear), (d.len * 0.2, flying, Ease::Over), (d.len * 0.8, flying, Ease::Linear), (d.len, landed, Ease::Snap)]), 16.0)
        };
    }
    // The Grovemaul rearing up to beat its chest, one fist then the other
    if look == Look::Grovemaul && matches!(d.m, Move::Roar | Move::Summon) && d.stage == Stage::Act {
        let beat = (d.t * 9.0).sin();
        let up = Stance { hips: [-0.1, 0.0, 0.08], lean: -0.15, bend: -0.25, nod: -0.55, jaw: 1.0, hands: [[0.55 + 0.18 * beat.max(0.0), -0.22, 1.85], [0.55 + 0.18 * (-beat).max(0.0), 0.22, 1.85]], feet: wide, ..guard };
        return pick((up, false), 30.0);
    }
    // Raised behind its head in both hands, then smashed down on the ground in front of it
    let raised = Stance { hips: [-0.1, 0.0, 0.04], lean: -0.1, bend: -0.25, nod: -0.1, hands: [[0.05, -0.25, 2.55], [0.05, 0.1, 2.55]], tool: [-0.8, 0.0, 0.55], grip: 1.0, feet: wide, ..guard };
    let reared = Stance { lean: -0.16, bend: -0.35, hands: [[-0.1, -0.22, 2.5], [-0.1, 0.08, 2.5]], tool: [-0.95, 0.0, 0.2], ..raised };
    let smash = Stance { hips: [0.3, 0.0, -0.32], lean: 0.3, bend: 0.4, nod: -0.25, hands: [[1.15, -0.15, 0.95], [1.1, 0.05, 0.95]], tool: [0.85, 0.0, -0.5], grip: 1.0, feet: [[0.45, -0.36, ANKLE], [-0.35, 0.36, ANKLE]], toes: [0.0, 0.3], ..guard };
    let buried = Stance { hips: [0.32, 0.0, -0.38], hands: [[1.18, -0.15, 0.8], [1.12, 0.05, 0.82]], tool: [0.75, 0.0, -0.66], ..smash };
    match (d.m, d.stage) {
        (Move::Slam | Move::Avalanche, Stage::Wind) => {
            let lead = (d.len - SNAP).max(0.05);
            let usual = (lead - d.held).max(0.02);
            let hold = mix(&raised, &reared, (d.held / 0.4).min(1.0));
            let (mut s, quick) = along(d.t, &[(0.0, guard, Ease::Linear), (usual, raised, Ease::Gather), (lead, hold, Ease::Smooth), (d.len, smash, Ease::Snap)]);
            if d.t > usual && d.t < lead {
                s.bend += 0.03 * (d.t * 34.0).sin();
            }
            pick((s, quick), 10.0)
        }
        (Move::Slam | Move::Avalanche, Stage::Act) => pick(along(d.t, &[(0.0, smash, Ease::Linear), (0.12, buried, Ease::Over), (d.len, buried, Ease::Linear)]), 30.0),
        // Drawn back low to its right, twisted round, then swung across backhanded to its left
        (Move::Swat, Stage::Wind | Stage::Act) => {
            let drawn = Stance { hips: [-0.05, -0.05, -0.15], lean: 0.12, twist: -0.75, nod: 0.0, hands: [[-0.4, -1.0, 1.15], [0.55, 0.45, 1.4]], tool: [-0.7, -0.6, 0.2], flat: 1.0, grip: 0.0, feet: wide, ..guard };
            let swung = Stance { hips: [0.15, 0.08, -0.18], lean: 0.18, twist: 0.7, hands: [[0.75, 0.55, 1.25], [-0.25, 0.75, 1.3]], tool: [0.3, 0.92, 0.15], flat: 1.0, grip: 0.0, feet: wide, toes: [0.3, 0.0], ..guard };
            if d.stage == Stage::Wind {
                pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), drawn, Ease::Gather), (d.len, swung, Ease::Snap)]), 10.0)
            } else {
                let past = Stance { twist: 0.95, hands: [[0.3, 0.85, 1.15], [-0.4, 0.7, 1.25]], tool: [-0.3, 0.9, 0.1], ..swung };
                pick(along(d.t, &[(0.0, swung, Ease::Linear), (d.len, past, Ease::Gather)]), 25.0)
            }
        }
        // Throwing a boulder: scooped up in its left hand off the ground, drawn back over its
        // shoulder, then hurled
        (Move::Throw, _) => {
            let holding = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
            let reach = Stance { hips: [0.1, 0.1, -0.35], lean: 0.25, bend: 0.4, twist: 0.3, hands: [[0.4, -0.6, 1.3], [0.8, 0.6, 0.35]], ..guard };
            let scooped = Stance { more: holding, ..reach };
            let cocked = Stance { hips: [-0.12, -0.05, 0.02], lean: -0.08, bend: -0.2, twist: 0.5, hands: [[1.0, -0.6, 1.8], [-0.5, 0.6, 2.4]], feet: [[0.45, -0.32, ANKLE], [-0.25, 0.32, ANKLE]], more: holding, ..guard };
            let hurled = Stance { hips: [0.3, 0.05, -0.12], lean: 0.18, bend: 0.3, twist: -0.45, hands: [[-0.1, -0.7, 1.3], [1.35, 0.25, 2.0]], feet: [[0.5, -0.32, ANKLE], [-0.1, 0.32, ANKLE]], toes: [0.0, 0.35], more: holding, ..guard };
            let follow = Stance { hips: [0.35, 0.0, -0.2], lean: 0.25, bend: 0.45, twist: -0.6, hands: [[-0.2, -0.75, 1.2], [1.0, -0.4, 1.0]], more: [0.0; 6], ..hurled };
            match d.stage {
                Stage::Wind => {
                    let lead = (d.len - SNAP).max(0.1);
                    pick(along(d.t, &[(0.0, guard, Ease::Linear), (lead * 0.33, reach, Ease::Smooth), (lead * 0.4, scooped, Ease::Linear), (lead, cocked, Ease::Gather), (d.len, hurled, Ease::Snap)]), 10.0)
                }
                Stage::Act => pick(along(d.t, &[(0.0, Stance { more: [0.0; 6], ..hurled }, Ease::Linear), (d.len, follow, Ease::Smooth)]), 20.0),
                _ => pick(along(d.t, &[(0.0, follow, Ease::Linear), (d.len, guard, Ease::Smooth)]), 6.0),
            }
        }
        // Its hands spread wide and high, then clapped together over its head
        (Move::Thunderclap, Stage::Wind | Stage::Act) => {
            let spread = Stance { hips: [-0.05, 0.0, -0.05], lean: -0.1, bend: -0.25, nod: -0.25, hands: [[0.2, -1.4, 2.4], [0.2, 1.4, 2.4]], tool: [0.2, -0.6, 0.78], feet: wide, ..guard };
            let clap = Stance { hips: [0.05, 0.0, -0.25], lean: 0.05, bend: 0.1, nod: -0.1, hands: [[0.75, -0.12, 2.45], [0.75, 0.12, 2.45]], tool: [0.2, -0.2, 0.96], feet: wide, ..guard };
            if d.stage == Stage::Wind {
                pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - 0.12).max(0.05), spread, Ease::Gather), (d.len, clap, Ease::Snap)]), 9.0)
            } else {
                let mut s = clap;
                s.hands[0][2] += 0.03 * (d.t * 30.0).sin();
                pick((s, false), 25.0)
            }
        }
        (Move::Quake, Stage::Wind) => {
            let lift = Stance { hips: [-0.05, 0.2, 0.05], roll: -0.1, bend: -0.1, feet: [[0.3, -0.3, 0.8], [0.0, 0.3, ANKLE]], toes: [-0.25, 0.0], hands: [[0.35, -1.2, 2.2], [0.35, 1.2, 2.2]], ..guard };
            let stomp = Stance { hips: [0.1, 0.0, -0.4], lean: 0.18, bend: 0.3, feet: [[0.35, -0.4, ANKLE], [-0.1, 0.4, ANKLE]], ..guard };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), lift, Ease::Gather), (d.len, stomp, Ease::Snap)]), 9.0)
        }
        (Move::Quake, Stage::Act) => pick((Stance { hips: [0.1, 0.0, -0.4], lean: 0.18, bend: 0.3, feet: [[0.35, -0.4, ANKLE], [-0.1, 0.4, ANKLE]], ..guard }, false), 25.0),
        // Breathing frost, bent forward with its jaw thrust out, arms drawn back
        (Move::Breath, Stage::Wind | Stage::Act) => {
            let breathe = Stance { hips: [0.1, 0.0, -0.2], lean: 0.2, bend: 0.25, nod: -0.25, hands: [[-0.15, -0.8, 1.2], [-0.15, 0.8, 1.2]], tool: [-0.2, -0.3, -0.93], feet: wide, ..guard };
            let mut s = if d.stage == Stage::Wind { along(d.t, &[(0.0, guard, Ease::Linear), (d.len, Stance { lean: -0.1, bend: -0.25, nod: -0.4, ..breathe }, Ease::Gather)]).0 } else { breathe };
            if d.stage == Stage::Act {
                s.turn += 0.15 * (d.t * 1.7).sin();
            }
            pick((s, false), 10.0)
        }
        // Its weapon raised to the sky, howling or calling the storm down
        (Move::Summon | Move::Storm | Move::Roar, Stage::Wind | Stage::Act) => {
            let mut call = Stance { hips: [0.0, 0.0, -0.1], lean: -0.1, bend: -0.3, nod: -0.5, hands: [[0.4, -0.5, 2.7], [0.5, 0.9, 1.6]], tool: [0.1, -0.1, 0.99], feet: wide, ..guard };
            if d.stage == Stage::Act {
                call.turn += 0.12 * (d.t * 9.0).sin() * (d.t > 0.2) as u8 as f64;
                call.hands[0][2] += 0.05 * (d.t * 12.0).sin();
            }
            pick(if d.stage == Stage::Wind || d.t < 0.3 { along(d.t, &[(0.0, guard, Ease::Linear), (if d.stage == Stage::Wind { d.len } else { 0.3 }, call, Ease::Gather)]) } else { (call, false) }, 10.0)
        }
        (_, Stage::Reel) => {
            let reel = Stance { hips: [-0.35, 0.05, -0.08], lean: -0.22, bend: -0.25, tilt: 0.12, nod: -0.4, turn: 0.25, hands: [[0.0, -1.2, 1.9], [-0.2, 1.1, 1.7]], tool: [-0.3, -0.5, 0.8], feet: [[-0.5, -0.3, ANKLE], [0.1, 0.3, ANKLE]], toes: [0.35, 0.0], ..guard };
            let caught = Stance { hips: [-0.2, 0.0, -0.3], lean: 0.15, bend: 0.2, nod: 0.15, hands: [[0.6, -0.7, 0.95], [0.6, 0.7, 0.95]], ..reel };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.16, reel, Ease::Gather), (d.len * 0.55, caught, Ease::Smooth), (d.len, guard, Ease::Smooth)]), 14.0)
        }
        (_, Stage::Stun) => {
            let mut dazed = Stance { hips: [0.0, 0.0, -0.2], lean: 0.1, bend: 0.25, nod: 0.35, hands: [[0.5, -0.65, 0.95], [0.5, 0.65, 0.95]], tool: [0.4, 0.0, -0.9], ..guard };
            dazed.turn += 0.3 * (d.t * 2.2).sin();
            pick((dazed, false), 5.0)
        }
        (_, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), raised, Ease::Gather), (d.len, smash, Ease::Snap)]), 10.0),
        (_, Stage::Act) => pick((buried, false), 20.0),
        (_, Stage::Recover) => {
            let pulled = Stance { hips: [0.15, 0.0, -0.22], lean: 0.12, bend: 0.2, hands: [[0.9, -0.3, 1.2], [0.85, 0.2, 1.2]], tool: [0.6, 0.0, 0.2], ..guard };
            let _ = look;
            pick(along(d.t, &[(0.0, pulled, Ease::Linear), (d.len, guard, Ease::Smooth)]), 6.0)
        }
    }
}

/// Dying: staggering back, down onto its knees, then over onto its face
fn death(rest: &Stance, t: f64) -> (Stance, f64, f64) {
    let hit = Stance { hips: [-0.25, 0.0, 0.02], lean: -0.18, bend: -0.3, nod: -0.5, hands: [[0.1, -1.0, 1.9], [0.0, 1.0, 1.8]], tool: [0.3, -0.6, -0.7], ..*rest };
    let knees = Stance { hips: [0.05, 0.0, -0.62], lean: 0.2, bend: 0.3, nod: 0.35, hands: [[0.5, -0.65, 0.45], [0.55, 0.65, 0.45]], tool: [0.6, -0.4, -0.7], feet: [[-0.3, -0.24, ANKLE], [-0.35, 0.24, ANKLE]], toes: [0.8, 0.8], ..*rest };
    let fallen = Stance { hips: [0.6, 0.0, -0.78], lean: 1.15, bend: 0.35, nod: 0.25, turn: 0.5, hands: [[1.6, -0.75, 0.25], [1.7, 0.75, 0.25]], tool: [0.2, -0.95, -0.1], ..knees };
    let (s, quick) = along(t, &[(0.0, *rest, Ease::Linear), (0.35, hit, Ease::Gather), (THUDS[0], knees, Ease::Snap), (1.25, knees, Ease::Linear), (THUDS[1], fallen, Ease::Snap), (DEATH, fallen, Ease::Linear)]);
    (s, if quick { 30.0 } else { 9.0 }, 0.0)
}

pub(super) fn animate(r: &mut Rig, m: &Moment) {
    let look = r.look;
    let f = frame(look);
    let rest = rest(look);
    let mut want = match m.dying {
        Some(t) => death(&rest, t),
        None => fight(look, &rest, m),
    };
    // The Slagwright never lets go of its crucible
    if look == Look::Slagwright {
        want.0.more[HOLDING] = 1.0;
    }
    let s = biped::pose(r, m, &f, &GAIT, want, 1.4);
    // The boulder in its left hand, beyond its fist; the crucible hung lower on its chain
    let fore = r.bones[FORE[1]];
    if s.more[HOLDING] > 0.5 {
        let hang = if look == Look::Slagwright { 0.62 } else { 0.45 };
        r.bones[BOULDER] = Bone::new(r.sockets.hands[1] + fore.turn * V::new(0.0, 0.0, -hang), fore.turn);
        r.sockets.grip = r.bones[BOULDER].at;
    } else {
        r.bones[BOULDER].shown = false;
        r.sockets.grip = r.sockets.tip;
    }
    // The jotun breathes frost out of its mouth; the giant's lightning comes off its hammer's head;
    // the Slagwright's slag flies up out of its crucible
    if look == Look::StormGiant {
        r.sockets.cast = r.sockets.tip;
    }
    if look == Look::Slagwright {
        r.sockets.cast = r.bones[BOULDER].at + V::new(0.0, 0.0, 0.3);
    }
    // The ape has nothing in its hands but its fists
    if look == Look::Grovemaul {
        r.bones[TOOL].shown = false;
    }
}
