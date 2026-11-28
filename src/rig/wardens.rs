//! The bosses of src\bosses\roster\wardens.rs, each on two legs its own way. The Winterglass
//! Duelist: tall and spare in steel rimed white, a fur cloak, frost in the slit of its visor, a
//! long blade of ice in its right hand and a parrying dagger in its left; it stands side on as a
//! fencer does, sidesteps and cuts back, lunges the length of itself and sweeps crescents off its
//! blade. The Hive Matriarch: black chitin banded gold, great glowing eyes, mandibles, scythed
//! forelimbs, four crystal wings and a swollen abdomen full of glowing brood behind it, which it
//! raises over itself to loose its swarm and its stingers and lowers to the ground to bring forth
//! its brood. The Gloamdelver: hunched and grey, a long snout and tiny eyes, crystals grown up out
//! of its back, huge spade claws it digs into the ground and tears up out of it. The Fenhulk:
//! a huge stooped hulk of peat and mud, moss over it, reeds growing out of its shoulders, the bog's
//! venom glowing in it; it stamps, spews and smashes. Dying, each staggers, drops to its knees and
//! falls on its face.

use super::biped::{self, ARM, CAPE, CHEST, EXTRA, FOOT, FORE, Frame, Gait, HEAD, JAW, PELVIS, SHIN, Stance, THIGH, TOOL, bone};
use super::*;
use crate::animals::{ball, cuboid, ellipsoid, limb};
use crate::worldgen::material::LEATHER;

pub const DEATH: f64 = 2.8;
pub const THUDS: [f64; 2] = [0.9, 1.6];
const SNAP: f64 = 0.14;
const ANKLE: f64 = 0.08;

/// The duelist's parrying dagger, the Matriarch's abdomen and wings
const DAGGER: usize = EXTRA;
const ABDOMEN: usize = EXTRA;
const WINGS: [usize; 4] = [EXTRA + 1, EXTRA + 2, EXTRA + 3, EXTRA + 4];
/// Its own numbers in its Stance: how far the Matriarch's abdomen is raised, 0 at rest, and how
/// hard her wings are buzzing, 0 to 1
const RAISE: usize = 0;
const BUZZ: usize = 1;

fn frame(look: Look) -> Frame {
    match look {
        Look::WinterglassDuelist => Frame {
            hip: 1.0,
            hip_w: 0.14,
            waist: 1.1,
            neck: V::new(0.04, 0.0, 0.55),
            shoulder: V::new(0.0, -0.24, 0.46),
            arm: [0.33, 0.31],
            leg: [0.5, 0.48],
            ankle: ANKLE,
            mouth: V::new(0.12, 0.0, 0.05),
            jaw: V::zeros(),
            tool: 1.15,
            grip: 0.0,
            elbow: V::new(-1.0, -0.4, -0.2),
            knee: 0.1,
        },
        Look::HiveMatriarch => Frame {
            hip: 0.95,
            hip_w: 0.2,
            waist: 1.05,
            neck: V::new(0.18, 0.0, 0.5),
            shoulder: V::new(0.1, -0.28, 0.42),
            arm: [0.45, 0.55],
            leg: [0.5, 0.5],
            ankle: ANKLE,
            mouth: V::new(0.2, 0.0, -0.02),
            jaw: V::new(0.12, 0.0, -0.03),
            tool: 0.3,
            grip: 0.0,
            elbow: V::new(-0.6, -0.7, 0.2),
            knee: 0.35,
        },
        Look::Gloamdelver => Frame {
            hip: 0.75,
            hip_w: 0.24,
            waist: 0.85,
            neck: V::new(0.34, 0.0, 0.4),
            shoulder: V::new(0.1, -0.42, 0.38),
            arm: [0.42, 0.42],
            leg: [0.42, 0.38],
            ankle: ANKLE,
            mouth: V::new(0.36, 0.0, -0.05),
            jaw: V::zeros(),
            tool: 0.3,
            grip: 0.0,
            elbow: V::new(-0.5, -1.0, -0.2),
            knee: 0.3,
        },
        _ => Frame {
            hip: 1.0,
            hip_w: 0.3,
            waist: 1.1,
            neck: V::new(0.22, 0.0, 0.85),
            shoulder: V::new(0.05, -0.55, 0.75),
            arm: [0.62, 0.62],
            leg: [0.5, 0.5],
            ankle: 0.1,
            mouth: V::new(0.24, 0.0, -0.02),
            jaw: V::new(0.14, 0.0, -0.06),
            tool: 0.3,
            grip: 0.0,
            elbow: V::new(-1.0, -0.6, -0.2),
            knee: 0.2,
        },
    }
}

fn gait(look: Look) -> Gait {
    match look {
        // Light on its feet
        Look::WinterglassDuelist => Gait { lift: 0.14, bob: 0.03, sway: 0.04, roll: 0.03, yaw: 0.12, swing: 0.18, lean: 0.2, stomp: 0.01 },
        Look::HiveMatriarch => Gait { lift: 0.16, bob: 0.04, sway: 0.06, roll: 0.05, yaw: 0.08, swing: 0.08, lean: 0.12, stomp: 0.03 },
        Look::Gloamdelver => Gait { lift: 0.14, bob: 0.06, sway: 0.1, roll: 0.07, yaw: 0.1, swing: 0.22, lean: 0.15, stomp: 0.05 },
        // Each step a heavy one
        _ => Gait { lift: 0.2, bob: 0.07, sway: 0.14, roll: 0.07, yaw: 0.08, swing: 0.2, lean: 0.08, stomp: 0.1 },
    }
}

/// Standing: the duelist side on, its blade low before it; the queen upright, her scythes folded;
/// the digger and the hulk stooped, their claws and fists hanging low
pub fn rest(look: Look) -> Stance {
    let f = frame(look);
    let mut s = match look {
        Look::WinterglassDuelist => {
            let mut s = f.rest([[0.35, -0.28, 1.05], [0.05, 0.3, 1.0]]);
            s.tool = [0.6, 0.0, -0.8];
            s
        }
        Look::HiveMatriarch => {
            let mut s = f.rest([[0.45, -0.3, 1.4], [0.45, 0.3, 1.4]]);
            s.lean = -0.05;
            s
        }
        Look::Gloamdelver => {
            let mut s = f.rest([[0.55, -0.5, 0.45], [0.55, 0.5, 0.45]]);
            s.lean = 0.35;
            s.nod = -0.3;
            s
        }
        _ => {
            let mut s = f.rest([[0.45, -0.7, 0.8], [0.45, 0.7, 0.8]]);
            s.lean = 0.25;
            s.bend = 0.2;
            s.nod = -0.25;
            s
        }
    };
    s.feet = [[0.0, -f.hip_w, f.ankle], [0.0, f.hip_w, f.ankle]];
    s
}

pub fn pieces(look: Look) -> Vec<Vec<Part>> {
    match look {
        Look::WinterglassDuelist => duelist(look),
        Look::HiveMatriarch => matriarch(look),
        Look::Gloamdelver => delver(look),
        _ => fenhulk(look),
    }
}

/// The feet of all of them, cut off level with the ground under them
fn foot(length: f64, wide: f64, material: u8, joint: u8, ankle: f64) -> Vec<Part> {
    vec![cuboid([-0.5, -0.4, -0.6], [0.6, 0.4, -ankle], AIR), ellipsoid([length * 0.4, 0.0, -0.02], [length, wide, 0.06], material), ball([0.0, 0.0, 0.0], wide * 0.8, joint)]
}

/// The Winterglass Duelist: steel rimed white, a cloak of white fur, frost in its visor, a blade
/// of ice and a parrying dagger
fn duelist(look: Look) -> Vec<Vec<Part>> {
    let skin = look.skin();
    let (steel, dark, fur, frost, ice) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let mut out = vec![Vec::new(); PIECES];
    out[PELVIS] = vec![ellipsoid([0.0, 0.0, 0.02], [0.16, 0.19, 0.11], dark), ellipsoid([0.0, 0.0, -0.12], [0.19, 0.21, 0.15], steel)];
    out[CHEST] = vec![
        ball([0.17, 0.0, 0.32], 0.03, frost),
        ellipsoid([-0.03, 0.0, 0.5], [0.14, 0.26, 0.07], fur),
        ellipsoid([0.02, 0.0, 0.25], [0.16, 0.21, 0.27], steel),
        ellipsoid([0.0, 0.0, 0.05], [0.13, 0.17, 0.1], dark),
    ];
    out[HEAD] = vec![
        cuboid([0.095, -0.07, 0.11], [0.135, 0.07, 0.135], frost),
        limb([-0.02, 0.0, 0.24], [-0.24, 0.0, 0.32], 0.03, 0.01, fur),
        ellipsoid([0.02, 0.0, 0.12], [0.12, 0.11, 0.14], steel),
        ellipsoid([0.0, 0.0, 0.0], [0.07, 0.07, 0.06], dark),
    ];
    for k in 0..2 {
        out[ARM[k]] = vec![ellipsoid([0.0, 0.0, -0.03], [0.1, 0.1, 0.08], steel), bone(0.33, 0.055, 0.045, dark)];
        out[FORE[k]] = vec![ball([0.0, 0.0, -0.31], 0.05, steel), bone(0.31, 0.048, 0.042, steel)];
        out[THIGH[k]] = vec![bone(0.5, 0.075, 0.06, dark)];
        out[SHIN[k]] = vec![ellipsoid([0.02, 0.0, -0.05], [0.07, 0.07, 0.06], steel), bone(0.48, 0.06, 0.048, steel)];
        out[FOOT[k]] = foot(0.12, 0.06, dark, steel, ANKLE);
    }
    // A long blade of ice, a crossguard of dark steel, frost in its pommel
    out[TOOL] = vec![
        ball([0.0, 0.0, 0.09], 0.03, frost),
        cuboid([-0.02, -0.12, -0.12], [0.02, 0.12, -0.09], dark),
        limb([0.0, 0.0, 0.08], [0.0, 0.0, -0.1], 0.024, 0.024, LEATHER),
        limb([0.0, 0.0, -0.12], [0.0, 0.0, -1.15], 0.035, 0.008, ice),
    ];
    // Its cloak hung from its shoulders down its back
    out[CAPE] = vec![ellipsoid([-0.17, 0.0, 0.08], [0.045, 0.25, 0.44], fur)];
    out[DAGGER] = vec![cuboid([-0.015, -0.06, -0.06], [0.015, 0.06, -0.04], dark), limb([0.0, 0.0, -0.05], [0.0, 0.0, -0.42], 0.022, 0.006, ice)];
    out
}

/// The Hive Matriarch: black chitin banded gold, glowing eyes, mandibles, scythed forelimbs, an
/// abdomen swollen with glowing brood, and four wings of crystal
fn matriarch(look: Look) -> Vec<Vec<Part>> {
    let skin = look.skin();
    let (chitin, dark, gold, venom, wing) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let mut out = vec![Vec::new(); PIECES];
    out[PELVIS] = vec![ellipsoid([0.0, 0.0, 0.0], [0.18, 0.2, 0.14], chitin)];
    out[CHEST] = vec![
        ellipsoid([0.06, 0.0, 0.42], [0.14, 0.2, 0.05], gold),
        ellipsoid([0.06, 0.0, 0.25], [0.22, 0.22, 0.28], chitin),
        ellipsoid([0.0, 0.0, 0.05], [0.14, 0.16, 0.1], dark),
    ];
    let h = &mut out[HEAD];
    for s in [-1.0, 1.0] {
        h.push(ball([0.12, s * 0.1, 0.08], 0.06, venom));
        h.push(limb([0.12, s * 0.04, 0.16], [0.3, s * 0.22, 0.42], 0.015, 0.008, dark));
    }
    h.push(cuboid([0.0, -0.08, 0.15], [0.08, 0.08, 0.22], gold));
    h.push(ellipsoid([0.08, 0.0, 0.05], [0.14, 0.13, 0.11], chitin));
    let j = &mut out[JAW];
    for s in [-1.0, 1.0] {
        j.push(limb([0.0, s * 0.06, 0.0], [0.16, s * 0.015, -0.05], 0.03, 0.01, dark));
    }
    for k in 0..2 {
        out[ARM[k]] = vec![bone(0.45, 0.06, 0.045, chitin)];
        // A scythe of chitin, its edge gold
        out[FORE[k]] = vec![ellipsoid([0.05, 0.0, -0.32], [0.025, 0.02, 0.28], gold), limb([0.0, 0.0, 0.0], [0.0, 0.0, -0.58], 0.05, 0.015, chitin)];
        out[THIGH[k]] = vec![bone(0.5, 0.05, 0.04, chitin)];
        out[SHIN[k]] = vec![bone(0.5, 0.04, 0.025, dark)];
        out[FOOT[k]] = foot(0.1, 0.04, dark, chitin, ANKLE);
    }
    // Banded and swollen, brood glowing through it, a stinger at its end
    let mut abdomen = vec![limb([-1.0, 0.0, -0.05], [-1.22, 0.0, -0.15], 0.06, 0.01, dark)];
    for k in 0..3 {
        let x = -0.3 - 0.22 * k as f64;
        abdomen.push(ellipsoid([x, 0.0, 0.0], [0.05, 0.39 - 0.05 * k as f64, 0.37 - 0.05 * k as f64], gold));
    }
    for (x, y, z) in [(-0.45, 0.3, 0.12), (-0.7, -0.26, 0.15), (-0.55, 0.0, 0.32), (-0.85, 0.18, -0.12)] {
        abdomen.push(ball([x, y, z], 0.08, venom));
    }
    abdomen.push(ellipsoid([-0.55, 0.0, 0.0], [0.55, 0.36, 0.35], chitin));
    out[ABDOMEN] = abdomen;
    for (w, &p) in WINGS.iter().enumerate() {
        let long = if w < 2 { 0.62 } else { 0.46 };
        out[p] = vec![ellipsoid([-long * 0.5, 0.0, 0.0], [long * 0.5, 0.015, 0.12], wing)];
    }
    out
}

/// The Gloamdelver: grey fur and rock, crystals grown out of its back, a long snout, tiny eyes,
/// spade claws
fn delver(look: Look) -> Vec<Vec<Part>> {
    let skin = look.skin();
    let (fur, rock, crystal, glow, claw) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let mut out = vec![Vec::new(); PIECES];
    out[PELVIS] = vec![ball([0.0, 0.2, 0.0], 0.2, fur), ball([0.0, -0.2, 0.0], 0.2, fur), ellipsoid([-0.05, 0.0, 0.0], [0.28, 0.34, 0.24], fur)];
    let c = &mut out[CHEST];
    // Crystals grown up out of its back, glowing at their tips
    for (x, y, h, lean) in [(-0.2, 0.0, 0.55, -0.2), (-0.1, 0.22, 0.42, 0.25), (-0.12, -0.24, 0.45, -0.25), (-0.32, 0.12, 0.35, 0.1), (-0.3, -0.12, 0.3, -0.1), (0.05, 0.1, 0.3, 0.3)] {
        let base = [x, y, 0.55];
        let tip = [x - 0.12, y + lean * 0.4, 0.55 + h];
        c.push(ball(tip, 0.035, glow));
        c.push(limb(base, tip, 0.09, 0.02, crystal));
    }
    c.push(ellipsoid([-0.05, 0.0, 0.42], [0.36, 0.4, 0.14], rock));
    c.push(ellipsoid([0.05, 0.0, 0.25], [0.44, 0.47, 0.4], fur));
    for s in [-1.0, 1.0] {
        c.push(ball([0.08, s * 0.4, 0.38], 0.2, fur));
    }
    let h = &mut out[HEAD];
    for s in [-1.0, 1.0] {
        h.push(ball([0.18, s * 0.09, 0.08], 0.022, glow));
    }
    h.push(ball([0.42, 0.0, -0.03], 0.06, claw));
    h.push(ellipsoid([0.22, 0.0, -0.01], [0.22, 0.12, 0.1], fur));
    h.push(ellipsoid([0.04, 0.0, 0.02], [0.18, 0.18, 0.16], fur));
    for k in 0..2 {
        out[ARM[k]] = vec![bone(0.42, 0.13, 0.11, fur)];
        let mut fore = vec![ball([0.0, 0.0, -0.42], 0.13, fur), bone(0.42, 0.11, 0.11, fur)];
        // Three spade claws, curving down and forward out of its paw
        for c in [-1.0, 0.0, 1.0] {
            fore.push(limb([0.05, c * 0.07, -0.45], [0.18, c * 0.11, -0.78], 0.05, 0.015, claw));
        }
        out[FORE[k]] = fore;
        out[THIGH[k]] = vec![bone(0.42, 0.15, 0.12, fur)];
        out[SHIN[k]] = vec![bone(0.38, 0.12, 0.09, fur)];
        out[FOOT[k]] = foot(0.16, 0.1, claw, fur, ANKLE);
    }
    out
}

/// The Fenhulk: peat and mud, moss over it, reeds out of its shoulders, the bog's venom glowing in
/// it, a maw and a jaw that hangs
fn fenhulk(look: Look) -> Vec<Vec<Part>> {
    let skin = look.skin();
    let (peat, mud, moss, venom, reed) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let mut out = vec![Vec::new(); PIECES];
    out[PELVIS] = vec![ball([0.0, 0.25, 0.0], 0.27, mud), ball([0.0, -0.25, 0.0], 0.27, mud), ellipsoid([0.0, 0.0, 0.05], [0.38, 0.48, 0.28], peat)];
    let c = &mut out[CHEST];
    // Reeds growing up out of its shoulders and back
    for (x, y, h) in [(-0.1, -0.5, 0.6), (-0.2, -0.4, 0.75), (0.0, -0.6, 0.5), (-0.1, 0.52, 0.65), (-0.25, 0.4, 0.7), (-0.35, 0.0, 0.8), (-0.3, 0.15, 0.55)] {
        c.push(limb([x, y, 0.85], [x - 0.12, y * 1.15, 0.85 + h], 0.025, 0.012, reed));
    }
    // The bog's venom glowing through it in bubbles
    for (x, y, z) in [(0.42, 0.18, 0.4), (0.38, -0.25, 0.55), (0.3, 0.05, 0.2), (-0.1, 0.45, 0.35)] {
        c.push(ball([x, y, z], 0.07, venom));
    }
    for s in [-1.0, 1.0] {
        c.push(ellipsoid([0.0, s * 0.5, 0.9], [0.3, 0.26, 0.16], moss));
        c.push(ball([0.05, s * 0.52, 0.74], 0.26, peat));
    }
    c.push(ellipsoid([-0.15, 0.0, 0.85], [0.35, 0.45, 0.2], moss));
    c.push(ellipsoid([0.12, 0.0, 0.32], [0.46, 0.5, 0.42], peat));
    c.push(ellipsoid([0.02, 0.0, 0.65], [0.42, 0.6, 0.36], mud));
    let h = &mut out[HEAD];
    for s in [-1.0, 1.0] {
        h.push(ball([0.24, s * 0.1, 0.12], 0.04, venom));
    }
    h.push(ellipsoid([0.0, 0.0, 0.22], [0.2, 0.22, 0.08], moss));
    h.push(ellipsoid([0.1, 0.0, 0.08], [0.2, 0.22, 0.2], peat));
    out[JAW] = vec![ball([0.14, 0.0, -0.06], 0.04, venom), ellipsoid([0.1, 0.0, -0.06], [0.16, 0.18, 0.08], mud)];
    for k in 0..2 {
        out[ARM[k]] = vec![ellipsoid([0.0, 0.0, -0.08], [0.18, 0.18, 0.12], moss), bone(0.62, 0.15, 0.13, peat)];
        out[FORE[k]] = vec![ball([0.0, 0.0, -0.64], 0.17, mud), limb([0.05, 0.0, -0.4], [0.08, 0.0, -0.75], 0.03, 0.02, moss), bone(0.62, 0.13, 0.12, peat)];
        out[THIGH[k]] = vec![bone(0.5, 0.2, 0.17, peat)];
        out[SHIN[k]] = vec![ellipsoid([0.0, 0.0, -0.35], [0.17, 0.17, 0.12], moss), bone(0.5, 0.17, 0.15, mud)];
        out[FOOT[k]] = foot(0.2, 0.14, mud, peat, 0.1);
    }
    out
}

/// Fighting: how it stands up to the player
fn guard(look: Look, rest: &Stance) -> Stance {
    let f = frame(look);
    match look {
        // Side on as a fencer does, its blade raised before it, its dagger held back
        Look::WinterglassDuelist => Stance {
            hips: [0.0, 0.0, -0.08],
            yaw: -0.35,
            twist: 0.25,
            hands: [[0.45, -0.25, 1.35], [-0.05, 0.32, 1.25]],
            tool: [0.9, 0.05, 0.35],
            feet: [[0.25, -0.16, ANKLE], [-0.22, 0.16, ANKLE]],
            ..*rest
        },
        // Reared, her scythes raised
        Look::HiveMatriarch => Stance { lean: -0.12, bend: -0.08, hands: [[0.4, -0.35, 1.75], [0.4, 0.35, 1.75]], feet: [[0.15, -0.24, ANKLE], [-0.15, 0.24, ANKLE]], ..*rest },
        Look::Gloamdelver => Stance { hips: [0.0, 0.0, -0.06], lean: 0.4, nod: -0.35, hands: [[0.75, -0.45, 0.6], [0.75, 0.45, 0.6]], feet: [[0.15, -0.3, ANKLE], [-0.15, 0.3, ANKLE]], ..*rest },
        _ => Stance { hips: [0.0, 0.0, -0.06], lean: 0.25, bend: 0.2, nod: -0.25, hands: [[0.65, -0.6, 1.0], [0.65, 0.6, 1.0]], feet: [[0.15, -f.hip_w - 0.05, 0.1], [-0.15, f.hip_w + 0.05, 0.1]], ..*rest },
    }
}

fn fight(look: Look, rest: &Stance, m: &Moment) -> (Stance, f64, f64) {
    let guard = if m.engaged { guard(look, rest) } else { *rest };
    let Some(d) = m.doing else {
        return (guard, 6.0, 1.0);
    };
    let f = frame(look);
    let pick = |(s, quick): (Stance, bool), slow: f64| (s, if quick { 42.0 } else { slow }, 0.0);
    let windup = |set: Stance, strike: Stance, slow: f64| {
        let lead = (d.len - SNAP).max(0.05);
        pick(along(d.t, &[(0.0, guard, Ease::Linear), (lead, set, Ease::Gather), (d.len, strike, Ease::Snap)]), slow)
    };
    let wide = [[0.3, -f.hip_w - 0.12, f.ankle], [-0.3, f.hip_w + 0.12, f.ankle]];
    let up = f.waist + f.shoulder.z;
    let reach = f.arm[0] + f.arm[1];
    // Raised over it, and brought down in front: a cut, the scythes, the claws, the fists
    let raised = Stance { hips: [-0.06, 0.0, 0.02], lean: -0.1, bend: -0.2, nod: -0.1, hands: [[0.1, -0.25, up + 0.8 * reach], [0.1, 0.25, up + 0.8 * reach]], tool: [-0.5, 0.0, 0.85], feet: wide, ..guard };
    let struck = Stance { hips: [0.15, 0.0, -0.2], lean: 0.3, bend: 0.3, nod: 0.05, hands: [[0.85 * reach, -0.2, up - 0.5 * reach], [0.8 * reach, 0.2, up - 0.5 * reach]], tool: [0.8, 0.0, -0.6], feet: wide, ..guard };
    let own = match (look, d.m, d.stage) {
        // The duelist drawn back low, its blade level, then flung out at full stretch
        (Look::WinterglassDuelist, Move::Lunge, Stage::Wind) => {
            let set = Stance { hips: [-0.15, 0.0, -0.22], lean: 0.3, yaw: -0.5, twist: 0.35, hands: [[-0.1, -0.3, 1.3], [-0.3, 0.32, 1.45]], tool: [1.0, 0.0, 0.1], feet: [[0.35, -0.16, ANKLE], [-0.4, 0.16, ANKLE]], ..guard };
            Some(pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len, set, Ease::Gather)]), 10.0))
        }
        (Look::WinterglassDuelist, Move::Lunge, Stage::Act) => {
            let out = Stance { hips: [0.3, 0.0, -0.28], lean: 0.45, yaw: -0.3, twist: 0.15, hands: [[0.95, -0.15, 1.2], [-0.45, 0.32, 1.35]], tool: [1.0, 0.0, -0.05], feet: [[0.6, -0.16, ANKLE], [-0.5, 0.16, ANKLE]], toes: [0.0, 0.4], ..guard };
            Some(pick((out, true), 42.0))
        }
        // Stepping aside, its blade raised high on the far side, then cut down across
        (Look::WinterglassDuelist, Move::Riposte, Stage::Act) => {
            let aside = Stance { hips: [0.0, 0.12, -0.12], roll: 0.15, lean: 0.1, hands: [[0.15, 0.15, 1.85], [-0.1, 0.35, 1.2]], tool: [0.2, 0.6, 0.8], feet: [[0.2, -0.3, ANKLE + 0.12], [-0.1, 0.2, ANKLE]], ..guard };
            let cut = Stance { hips: [0.15, -0.05, -0.18], lean: 0.25, twist: -0.4, hands: [[0.75, -0.45, 1.0], [-0.1, 0.35, 1.2]], tool: [0.6, -0.65, -0.45], feet: [[0.4, -0.18, ANKLE], [-0.25, 0.18, ANKLE]], ..guard };
            Some(pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.18, aside, Ease::Gather), (0.3, cut, Ease::Snap), (d.len, cut, Ease::Linear)]), 30.0))
        }
        // Its blade drawn back over its left shoulder, then swept across, and back, and across
        (Look::WinterglassDuelist, Move::Crescent | Move::Spit, Stage::Wind) => {
            let drawn = Stance { hips: [-0.05, 0.0, -0.1], twist: 0.6, hands: [[0.15, 0.25, 1.65], [-0.2, 0.4, 1.2]], tool: [-0.3, 0.8, 0.5], feet: wide, ..guard };
            Some(pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len, drawn, Ease::Gather)]), 10.0))
        }
        (Look::WinterglassDuelist, Move::Crescent | Move::Spit, Stage::Act) => {
            let left = Stance { hips: [-0.05, 0.0, -0.1], twist: 0.6, hands: [[0.3, 0.3, 1.55], [-0.2, 0.4, 1.2]], tool: [0.2, 0.9, 0.3], feet: wide, ..guard };
            let right = Stance { hips: [0.1, 0.0, -0.15], twist: -0.6, lean: 0.15, hands: [[0.6, -0.5, 1.25], [-0.2, 0.4, 1.2]], tool: [0.4, -0.9, 0.05], feet: wide, ..guard };
            let k = (d.t / 0.2) as usize;
            let local = d.t - k as f64 * 0.2;
            let (a, b) = if k % 2 == 0 { (left, right) } else { (right, left) };
            Some(pick(along(local, &[(0.0, a, Ease::Linear), (0.12, b, Ease::Snap), (0.2, b, Ease::Linear)]), 40.0))
        }
        // The queen's abdomen raised up over her, her wings a blur, her scythes spread
        (Look::HiveMatriarch, Move::Swarm, Stage::Wind | Stage::Act) => {
            let mut more = [0.0; 6];
            more[RAISE] = 1.0 + if d.stage == Stage::Act { 0.12 * (d.t * 14.0).sin() } else { 0.0 };
            more[BUZZ] = 1.0;
            let set = Stance { lean: -0.2, bend: -0.15, nod: -0.25, jaw: 1.0, hands: [[0.2, -0.75, 1.65], [0.2, 0.75, 1.65]], feet: wide, more, ..guard };
            Some(if d.stage == Stage::Wind { pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len, set, Ease::Gather)]), 9.0) } else { pick((set, false), 20.0) })
        }
        // Curled right up over her, then thrust forward to loose its stingers
        (Look::HiveMatriarch, Move::Stingrain, Stage::Wind | Stage::Act) => {
            let mut curled = [0.0; 6];
            curled[RAISE] = 1.6;
            curled[BUZZ] = 0.6;
            let mut thrust = curled;
            thrust[RAISE] = 0.9;
            let set = Stance { lean: -0.25, bend: -0.2, hands: [[0.3, -0.5, 1.8], [0.3, 0.5, 1.8]], feet: wide, more: curled, ..guard };
            let loosed = Stance { lean: 0.1, bend: 0.15, hands: [[0.55, -0.4, 1.4], [0.55, 0.4, 1.4]], feet: wide, more: thrust, ..guard };
            Some(if d.stage == Stage::Wind { windup(set, loosed, 9.0) } else { pick((loosed, false), 20.0) })
        }
        // Lowered to the ground behind her to bring forth her brood, her scythes raised
        (Look::HiveMatriarch, Move::Summon | Move::Roar, Stage::Wind | Stage::Act) => {
            let mut low = [0.0; 6];
            low[RAISE] = -0.5;
            low[BUZZ] = 0.8;
            let call = Stance { lean: -0.25, bend: -0.25, nod: -0.4, jaw: 1.0, hands: [[0.3, -0.6, 2.0], [0.3, 0.6, 2.0]], feet: wide, more: low, ..guard };
            Some(pick(along(d.t, &[(0.0, guard, Ease::Linear), (if d.stage == Stage::Wind { d.len } else { 0.25 }, call, Ease::Gather)]), 9.0))
        }
        // Its claws dug into the ground in front of it, then torn up and flung forward
        (Look::Gloamdelver, Move::Rockspray, Stage::Wind | Stage::Act) => {
            let dug = Stance { hips: [-0.05, 0.0, -0.2], lean: 0.75, bend: 0.35, nod: -0.2, hands: [[0.85, -0.35, 0.05], [0.85, 0.35, 0.05]], feet: wide, ..guard };
            let flung = Stance { hips: [0.1, 0.0, -0.05], lean: 0.1, bend: -0.1, nod: -0.4, hands: [[0.75, -0.45, 1.3], [0.75, 0.45, 1.3]], feet: wide, ..guard };
            Some(if d.stage == Stage::Wind { windup(dug, flung, 9.0) } else { pick((flung, false), 18.0) })
        }
        // Its claws raised high over its head, then driven down into the ground and held there
        (Look::Gloamdelver, Move::Undermine, Stage::Wind | Stage::Act) => {
            let high = Stance { hips: [-0.05, 0.0, 0.05], lean: -0.05, bend: -0.25, nod: -0.4, hands: [[0.2, -0.35, 1.6], [0.2, 0.35, 1.6]], feet: wide, ..guard };
            let driven = Stance { hips: [0.1, 0.0, -0.25], lean: 0.8, bend: 0.4, nod: 0.0, hands: [[0.95, -0.25, -0.05], [0.95, 0.25, -0.05]], feet: wide, ..guard };
            Some(if d.stage == Stage::Wind { windup(high, driven, 9.0) } else { pick((driven, false), 25.0) })
        }
        // Nosing down to dig in, then bursting up out of the ground claws first
        (Look::Gloamdelver, Move::Burrow, _) => {
            let nose = Stance { hips: [0.1, 0.0, -0.3], lean: 0.9, bend: 0.45, nod: 0.2, hands: [[0.9, -0.3, -0.1], [0.9, 0.3, -0.1]], feet: wide, ..guard };
            Some(pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len, nose, Ease::Gather)]), 10.0))
        }
        (Look::Gloamdelver, Move::Erupt, Stage::Act) => {
            let burst = Stance { hips: [0.0, 0.0, 0.05], lean: -0.15, bend: -0.25, nod: -0.5, hands: [[0.35, -0.55, 1.6], [0.35, 0.55, 1.6]], feet: wide, ..guard };
            Some(pick((burst, false), 20.0))
        }
        // Its right foot raised high, its arms flung out to keep its balance, then stamped down
        (Look::Fenhulk, Move::Mirestomp, Stage::Wind) => {
            let lifted = Stance { hips: [-0.05, 0.12, 0.05], roll: -0.12, lean: -0.05, bend: -0.1, hands: [[0.4, -1.3, 1.5], [0.4, 1.3, 1.5]], feet: [[0.35, -0.32, 0.85], [0.0, 0.32, 0.1]], toes: [-0.3, 0.0], ..guard };
            let stamped = Stance { hips: [0.1, 0.0, -0.32], lean: 0.25, bend: 0.3, hands: [[0.7, -0.75, 0.8], [0.7, 0.75, 0.8]], feet: [[0.45, -0.38, 0.1], [-0.1, 0.38, 0.1]], ..guard };
            Some(windup(lifted, stamped, 8.0))
        }
        (Look::Fenhulk, Move::Mirestomp, Stage::Act) => {
            let mut s = Stance { hips: [0.1, 0.0, -0.32], lean: 0.25, bend: 0.3, hands: [[0.7, -0.75, 0.8], [0.7, 0.75, 0.8]], feet: [[0.45, -0.38, 0.1], [-0.1, 0.38, 0.1]], ..guard };
            s.hips[2] += 0.02 * (d.t * 30.0).sin();
            Some(pick((s, false), 25.0))
        }
        // Its head thrown back, its gut heaving, then thrust forward with its jaw wide as it spews
        (Look::Fenhulk, Move::Bogspew, Stage::Wind | Stage::Act) => {
            let back = Stance { lean: -0.1, bend: -0.3, nod: -0.7, jaw: 0.4, hands: [[0.55, -0.35, 0.95], [0.55, 0.35, 0.95]], feet: wide, ..guard };
            let spew = Stance { lean: 0.3, bend: 0.4, nod: 0.25, jaw: 1.0, hands: [[0.3, -0.9, 1.1], [0.3, 0.9, 1.1]], feet: wide, ..guard };
            Some(if d.stage == Stage::Wind { windup(back, spew, 8.0) } else { pick((spew, false), 20.0) })
        }
        // A clod of the bog scooped up in its right hand, drawn back over its shoulder and hurled
        (Look::Fenhulk, Move::Throw, Stage::Wind | Stage::Act) => {
            let scoop = Stance { hips: [0.1, 0.0, -0.35], lean: 0.4, bend: 0.4, hands: [[0.9, -0.5, 0.3], [0.6, 0.6, 0.9]], feet: wide, ..guard };
            let cocked = Stance { hips: [-0.1, -0.05, 0.0], lean: -0.1, twist: 0.5, hands: [[-0.5, -0.7, 2.0], [0.9, 0.6, 1.4]], feet: wide, ..guard };
            let hurled = Stance { hips: [0.25, 0.05, -0.15], lean: 0.2, bend: 0.3, twist: -0.45, hands: [[1.3, -0.3, 1.6], [-0.2, 0.7, 1.1]], feet: wide, ..guard };
            Some(if d.stage == Stage::Wind {
                let lead = (d.len - SNAP).max(0.1);
                pick(along(d.t, &[(0.0, guard, Ease::Linear), (lead * 0.4, scoop, Ease::Smooth), (lead, cocked, Ease::Gather), (d.len, hurled, Ease::Snap)]), 9.0)
            } else {
                pick((hurled, false), 18.0)
            })
        }
        _ => None,
    };
    if let Some(pose) = own {
        return pose;
    }
    match (d.m, d.stage) {
        (Move::Slam, Stage::Wind) => windup(raised, struck, 9.0),
        (Move::Slam, Stage::Act) => pick((struck, false), 25.0),
        // Roaring, rearing up with its arms raised
        (Move::Summon | Move::Roar, Stage::Wind | Stage::Act) => {
            let call = Stance { hips: [0.0, 0.0, -0.05], lean: -0.15, bend: -0.3, nod: -0.5, jaw: 1.0, hands: [[0.35, -0.6, up + 0.7 * reach], [0.35, 0.6, up + 0.7 * reach]], tool: [0.1, -0.1, 0.99], feet: wide, ..guard };
            let mut s = along(d.t, &[(0.0, guard, Ease::Linear), (if d.stage == Stage::Wind { d.len } else { 0.25 }, call, Ease::Gather)]).0;
            s.hands[0][2] += 0.04 * (d.t * 12.0).sin();
            pick((s, false), 10.0)
        }
        (_, Stage::Reel) => {
            let reel = Stance { hips: [-0.25, 0.05, -0.06], lean: -0.22, bend: -0.25, tilt: 0.12, nod: -0.4, turn: 0.25, hands: [[0.0, -0.8, up + 0.3], [-0.1, 0.8, up + 0.2]], feet: [[-0.35, -f.hip_w, f.ankle], [0.1, f.hip_w, f.ankle]], ..guard };
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.16, reel, Ease::Gather), (d.len, guard, Ease::Smooth)]), 14.0)
        }
        (_, Stage::Stun) => {
            let mut dazed = Stance { hips: [0.0, 0.0, -0.15], lean: 0.12, bend: 0.25, nod: 0.35, ..guard };
            dazed.turn += 0.3 * (d.t * 2.2).sin();
            pick((dazed, false), 5.0)
        }
        (_, Stage::Wind) => windup(raised, struck, 9.0),
        (_, Stage::Act) => pick((struck, false), 20.0),
        (_, Stage::Recover) => pick(along(d.t, &[(0.0, struck, Ease::Linear), (d.len, guard, Ease::Smooth)]), 6.0),
    }
}

/// Dying: staggering back, down onto its knees, then over onto its face
fn death(look: Look, rest: &Stance, t: f64) -> (Stance, f64, f64) {
    let f = frame(look);
    let legs = f.leg[0] + f.leg[1];
    let up = f.waist + f.shoulder.z;
    let hit = Stance { hips: [-0.2, 0.0, 0.02], lean: -0.18, bend: -0.3, nod: -0.5, hands: [[0.1, -0.8, up + 0.3], [0.0, 0.8, up + 0.2]], ..*rest };
    let knees = Stance { hips: [0.05, 0.0, -0.55 * legs], lean: 0.2, bend: 0.3, nod: 0.35, hands: [[0.5, -0.5, 0.35 * up], [0.55, 0.5, 0.35 * up]], feet: [[-0.3, -f.hip_w, f.ankle], [-0.35, f.hip_w, f.ankle]], toes: [0.8, 0.8], ..*rest };
    let fallen = Stance { hips: [0.5, 0.0, -0.7 * legs], lean: 1.15, bend: 0.35, nod: 0.25, turn: 0.5, hands: [[1.4, -0.6, 0.15], [1.5, 0.6, 0.15]], ..knees };
    let (s, quick) = along(t, &[(0.0, *rest, Ease::Linear), (0.35, hit, Ease::Gather), (THUDS[0], knees, Ease::Snap), (1.2, knees, Ease::Linear), (THUDS[1], fallen, Ease::Snap), (DEATH, fallen, Ease::Linear)]);
    (s, if quick { 30.0 } else { 9.0 }, 0.0)
}

pub(super) fn animate(r: &mut Rig, m: &Moment) {
    let look = r.look;
    let f = frame(look);
    let rest = rest(look);
    let want = match m.dying {
        Some(t) => death(look, &rest, t),
        None => fight(look, &rest, m),
    };
    let s = biped::pose(r, m, &f, &gait(look), want, if look == Look::Fenhulk { 1.6 } else { 1.0 });
    match look {
        Look::WinterglassDuelist => {
            // Its dagger in its left hand, pointing on along its forearm
            let fore = r.bones[FORE[1]];
            r.bones[DAGGER] = Bone::new(r.sockets.hands[1], fore.turn);
            r.sockets.cast = r.sockets.tip;
            r.sockets.grip = r.sockets.tip;
        }
        Look::HiveMatriarch => {
            r.bones[TOOL].shown = false;
            // Her abdomen hung off the back of her hips, raised over her as she says
            let pelvis = r.bones[PELVIS];
            let raise = s.more[RAISE];
            let turn = pelvis.turn * ry(-0.2 + 0.9 * raise);
            let at = pelvis.at + pelvis.turn * V::new(-0.14, 0.0, 0.06);
            r.bones[ABDOMEN] = Bone::new(at, turn);
            r.sockets.cast = at + turn * V::new(-1.2, 0.0, -0.12);
            r.sockets.grip = r.sockets.cast;
            // Her wings folded along her back, spread and a blur as she buzzes
            let chest = r.bones[CHEST];
            let buzz = s.more[BUZZ].clamp(0.0, 1.0);
            let beat = (r.clock * 55.0).sin() * 0.35 * buzz;
            for (w, &p) in WINGS.iter().enumerate() {
                let side = if w % 2 == 0 { 1.0 } else { -1.0 };
                let low = if w < 2 { 0.0 } else { 0.28 };
                let spread = 0.25 + 0.75 * buzz + beat;
                let joint = chest.at + chest.turn * V::new(-0.15, side * 0.08, 0.42 - 0.12 * (w / 2) as f64);
                r.bones[p] = Bone::new(joint, chest.turn * rz(side * spread) * rx(side * (0.1 + low)));
            }
        }
        Look::Gloamdelver => {
            r.bones[TOOL].shown = false;
            r.sockets.cast = (r.sockets.hands[0] + r.sockets.hands[1]) * 0.5;
            r.sockets.grip = r.sockets.cast;
        }
        _ => {
            r.bones[TOOL].shown = false;
            r.sockets.grip = r.sockets.hands[0];
        }
    }
}
