//! What walks on four legs: the Elderhorn of the old woods, a great stag with mossy antlers of
//! bone, and the Bleeding Hart of the Dreaming Red, dark as old blood, its antlers crimson
//! brambles, blood glowing in its wounds. Its pose is a Stance: where its body is and how it's
//! tipped, how its neck is raised and turned and its head on that, its tail, and where each of its
//! four feet is; its legs reach for its feet, the forelegs bending forward at the knee and the hind
//! back at the hock. It walks a foot at a time, gallops when it charges, rears up to bring its
//! forehooves down, lowers its antlers to charge, sweeps them side to side, shakes thorns off them,
//! and bellows with its head thrown back. Dying, its legs fold and it falls over onto its side.
//!
//! And four more, see src\bosses\roster.rs: the Cinderback Tortoise, a shell of black glass with
//! vents smoking on it, which draws itself in to spit burning rocks out of them; the Dreadmare, a
//! nightmare of a horse, its ribs showing, blood burning in its mane and tail, rearing to bring its
//! hooves down; the Hoarback Mammoth, shaggy and white, ice grown on its back, tusks of old bone and a
//! trunk it sprays frost from, rearing and stamping again and again; and the Gilded Sphinx, a
//! lion of carved sandstone with folded wings, a gold-banded headdress and a face of stone, the sun
//! burning in its eyes, sitting up to wheel its jets of fire round it.

use bytemuck::{Pod, Zeroable};

use super::*;
use crate::animals::{ball, cuboid, ellipsoid, limb};
use crate::worldgen::material::{FUR_CREAM, FUR_DARK};

const BODY: usize = 0;
const NECK: usize = 1;
const HEAD: usize = 2;
const TAIL: usize = 3;
/// Upper and lower legs: the right foreleg, the left, the right hind leg, the left
const UPPER: [usize; 4] = [4, 5, 6, 7];
const LOWER: [usize; 4] = [8, 9, 10, 11];

pub const DEATH: f64 = 2.8;
pub const THUDS: [f64; 2] = [0.7, 1.3];
const SNAP: f64 = 0.14;

/// How a beast's put together, in meters at its build's size: how high its body is standing,
/// where each leg joins it from its body's middle, how long the upper and lower bones of fore and
/// hind legs are, which way the fore and hind legs bend, +1 forward, where its neck and tail join
/// it, how long its neck is, and where its mouth and what it flings things from are on its head
#[derive(Clone, Copy)]
pub struct Shape {
    pub high: f64,
    pub joints: [[f64; 3]; 4],
    pub legs: [[f64; 2]; 2],
    pub bends: [f64; 2],
    pub neck_at: [f64; 3],
    pub neck_long: [f64; 3],
    pub tail_at: [f64; 3],
    pub mouth: [f64; 3],
    pub cast: [f64; 3],
}

/// A stag's
const STAG: Shape = Shape {
    high: 1.35,
    joints: [[0.62, -0.2, -0.15], [0.62, 0.2, -0.15], [-0.65, -0.2, -0.13], [-0.65, 0.2, -0.13]],
    legs: [[0.62, 0.64], [0.64, 0.68]],
    bends: [1.0, -1.0],
    neck_at: [0.7, 0.0, 0.1],
    neck_long: [0.35, 0.0, 0.6],
    tail_at: [-0.8, 0.0, 0.12],
    mouth: [0.48, 0.0, -0.1],
    cast: [0.0, 0.0, 0.9],
};

/// A great bear's, bending at its elbows and knees as a man does
const BEAR: Shape = Shape {
    high: 1.6,
    joints: [[0.75, -0.45, -0.25], [0.75, 0.45, -0.25], [-0.75, -0.45, -0.25], [-0.75, 0.45, -0.25]],
    legs: [[0.7, 0.7], [0.72, 0.7]],
    bends: [-1.0, 1.0],
    neck_at: [0.8, 0.0, 0.2],
    neck_long: [0.6, 0.0, 0.05],
    tail_at: [-1.6, 0.0, 0.0],
    mouth: [0.8, 0.0, -0.15],
    cast: [0.8, 0.0, -0.15],
};

/// An alpha wolf's
const WOLF: Shape = Shape {
    high: 0.6,
    joints: [[0.3, -0.12, -0.08], [0.3, 0.12, -0.08], [-0.32, -0.12, -0.08], [-0.32, 0.12, -0.08]],
    legs: [[0.27, 0.27], [0.28, 0.28]],
    bends: [1.0, -1.0],
    neck_at: [0.42, 0.0, 0.02],
    neck_long: [0.3, 0.0, 0.16],
    tail_at: [-0.45, 0.0, 0.02],
    mouth: [0.24, 0.0, -0.06],
    cast: [0.24, 0.0, -0.06],
};

/// The Cinderback's: low and broad under its shell, its stubby legs splayed out
const TORTOISE: Shape = Shape {
    high: 0.75,
    joints: [[0.52, -0.42, -0.08], [0.52, 0.42, -0.08], [-0.48, -0.42, -0.08], [-0.48, 0.42, -0.08]],
    legs: [[0.4, 0.42], [0.4, 0.42]],
    bends: [-1.0, 1.0],
    neck_at: [0.78, 0.0, 0.02],
    neck_long: [0.36, 0.0, 0.12],
    tail_at: [-0.8, 0.0, -0.05],
    mouth: [0.3, 0.0, -0.04],
    cast: [0.3, 0.0, -0.02],
};

/// The Dreadmare's: a horse's, its neck longer and higher than a stag's
const HORSE: Shape = Shape {
    high: 1.4,
    joints: [[0.6, -0.18, -0.15], [0.6, 0.18, -0.15], [-0.62, -0.18, -0.13], [-0.62, 0.18, -0.13]],
    legs: [[0.64, 0.68], [0.66, 0.7]],
    bends: [1.0, -1.0],
    neck_at: [0.72, 0.0, 0.12],
    neck_long: [0.42, 0.0, 0.62],
    tail_at: [-0.82, 0.0, 0.15],
    mouth: [0.52, 0.0, -0.24],
    cast: [0.45, 0.0, -0.12],
};

/// The Hoarback's: huge and high-shouldered on pillars of legs, its head low on a short neck, its
/// trunk hanging nearly to the ground, which it sprays frost from
const MAMMOTH: Shape = Shape {
    high: 1.6,
    joints: [[0.72, -0.38, -0.3], [0.72, 0.38, -0.3], [-0.72, -0.38, -0.3], [-0.72, 0.38, -0.3]],
    legs: [[0.68, 0.72], [0.68, 0.72]],
    bends: [-1.0, 1.0],
    neck_at: [1.0, 0.0, 0.25],
    neck_long: [0.35, 0.0, 0.1],
    tail_at: [-1.2, 0.0, 0.15],
    mouth: [0.55, 0.0, -0.45],
    cast: [0.85, 0.0, -1.1],
};

/// The Gilded Sphinx's: a lion's, its elbows and hocks both bent back
const LION: Shape = Shape {
    high: 1.15,
    joints: [[0.62, -0.24, -0.12], [0.62, 0.24, -0.12], [-0.6, -0.24, -0.1], [-0.6, 0.24, -0.1]],
    legs: [[0.56, 0.58], [0.6, 0.62]],
    bends: [-1.0, -1.0],
    neck_at: [0.78, 0.0, 0.22],
    neck_long: [0.24, 0.0, 0.36],
    tail_at: [-0.82, 0.0, 0.12],
    mouth: [0.24, 0.0, -0.06],
    cast: [0.26, 0.0, 0.04],
};

fn shape(look: Look) -> Shape {
    match look {
        Look::Ironhide | Look::Frostfang => BEAR,
        Look::Direwolf | Look::WinterAlpha => WOLF,
        Look::Cinderback => TORTOISE,
        Look::Dreadmare => HORSE,
        Look::Hoarback => MAMMOTH,
        Look::GildedSphinx => LION,
        _ => STAG,
    }
}

/// A pose made for a stag's size, for a beast of `b`'s: its body moved and its feet moved from
/// where they stand as much less as it's lower
fn fitted(b: &Shape, s: &Stance) -> Stance {
    let k = b.high / STAG.high;
    let mut out = *s;
    out.body = s.body.map(|v| v * k);
    for f in 0..4 {
        let (stag, own) = (STAG.joints[f], b.joints[f]);
        out.feet[f] = [own[0] + (s.feet[f][0] - stag[0]) * k, own[1] * 1.05 + (s.feet[f][1] - stag[1] * 1.05) * k, s.feet[f][2] * k];
    }
    out
}

/// How a beast holds itself: its body moved from where it stands and tipped forward, left side up
/// and round to the left; its neck raised and turned, and its head on that nodded down and turned;
/// its jaw; its tail raised and turned; and where each foot is, in its own meters
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct Stance {
    pub body: [f64; 3],
    pub pitch: f64,
    pub roll: f64,
    pub yaw: f64,
    pub neck: [f64; 2],
    pub head: [f64; 2],
    pub jaw: f64,
    pub tail: [f64; 2],
    pub feet: [[f64; 3]; 4],
    pub more: [f64; 4],
}

pub fn rest(look: Look) -> Stance {
    let b = shape(look);
    let mut feet = [[0.0; 3]; 4];
    for k in 0..4 {
        feet[k] = [b.joints[k][0], b.joints[k][1] * 1.05, 0.0];
    }
    Stance { feet, tail: [-0.4, 0.0], ..Default::default() }
}

pub fn pieces(look: Look) -> Vec<Vec<Part>> {
    match look {
        Look::Ironhide | Look::Frostfang => return bear(look),
        Look::Direwolf | Look::WinterAlpha => return wolf(look),
        Look::Cinderback => return tortoise(look),
        Look::Dreadmare => return mare(look),
        Look::Hoarback => return mammoth(look),
        Look::GildedSphinx => return sphinx(look),
        _ => {}
    }
    let skin = look.skin();
    let (body, dark, accent, glow, extra) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let hart = look == Look::BleedingHart;
    let mut out = vec![Vec::new(); PIECES];
    let b = &mut out[BODY];
    if hart {
        // Brambles growing out along its back, and wounds glowing where they've torn through
        for k in 0..6 {
            let x = 0.5 - 0.2 * k as f64;
            let s = if k % 2 == 0 { 1.0 } else { -1.0 };
            b.push(limb([x, s * 0.08, 0.3], [x - 0.15, s * 0.25, 0.55], 0.04, 0.008, accent));
            b.push(ball([x + 0.05, s * 0.28, 0.05 - 0.04 * k as f64], 0.05, glow));
        }
    } else {
        b.push(ellipsoid([0.1, 0.0, -0.22], [0.55, 0.2, 0.12], FUR_CREAM));
    }
    b.push(ellipsoid([0.45, 0.0, -0.02], [0.4, 0.3, 0.36], body));
    b.push(ellipsoid([-0.5, 0.0, 0.02], [0.4, 0.31, 0.33], body));
    b.push(ellipsoid([0.0, 0.0, 0.0], [0.8, 0.29, 0.33], body));
    out[NECK] = vec![ellipsoid([0.12, 0.0, 0.25], [0.22, 0.12, 0.38], if hart { dark } else { FUR_DARK }), limb([0.0, 0.0, 0.0], STAG.neck_long, 0.2, 0.14, body)];
    // Its head: eyes, ears, a long muzzle, and its antlers branching up and out
    let h = &mut out[HEAD];
    for s in [-1.0, 1.0] {
        h.push(ball([0.2, s * 0.11, 0.06], 0.035, glow));
        h.push(limb([0.02, s * 0.1, 0.1], [-0.08, s * 0.3, 0.2], 0.05, 0.02, body));
        let beam = |f: f64| [-0.05 * f - 0.02, s * (0.08 + 0.42 * f), 0.12 + 0.7 * f - 0.15 * f * f];
        let antler = if hart { accent } else { extra };
        for k in 0..4 {
            let (f0, f1) = (k as f64 / 4.0, (k + 1) as f64 / 4.0);
            h.push(limb(beam(f0), beam(f1), 0.045 - 0.008 * k as f64, 0.035 - 0.007 * k as f64, antler));
            // A tine up off each length of it
            let at = beam(f1);
            let tine = [at[0] + 0.12, at[1] + s * 0.03, at[2] + 0.2 - 0.03 * k as f64];
            h.push(limb(at, tine, 0.03, 0.01, antler));
            if hart {
                h.push(ball(tine, 0.03, glow));
            } else if k % 2 == 1 {
                // Moss hanging off it
                h.push(limb(at, [at[0], at[1], at[2] - 0.15], 0.03, 0.02, accent));
            }
        }
    }
    h.push(ball([0.45, 0.0, -0.08], 0.06, if hart { glow } else { FUR_DARK }));
    h.push(limb([0.1, 0.0, 0.0], [0.45, 0.0, -0.08], 0.11, 0.07, body));
    h.push(ellipsoid([0.12, 0.0, 0.03], [0.2, 0.13, 0.13], body));
    out[TAIL] = vec![limb([0.0, 0.0, 0.0], [-0.12, 0.0, -0.22], 0.07, 0.04, if hart { dark } else { FUR_CREAM })];
    for k in 0..4 {
        let hind = k >= 2;
        let [upper, lower] = STAG.legs[hind as usize];
        out[UPPER[k]] = vec![limb([0.0, 0.0, 0.0], [0.0, 0.0, -upper], if hind { 0.17 } else { 0.13 }, 0.075, body)];
        out[LOWER[k]] = vec![cuboid([-0.2, -0.2, -lower - 0.3], [0.2, 0.2, -lower], AIR), ellipsoid([0.03, 0.0, -lower + 0.04], [0.08, 0.06, 0.07], dark), ball([0.0, 0.0, 0.0], 0.07, body), limb([0.0, 0.0, 0.0], [0.0, 0.0, -lower + 0.06], 0.06, 0.045, dark)];
    }
    out
}

/// Ironhide and Frostfang: a huge old bear in plates of armor down its back, spiked, tusked, its
/// eyes glowing, its shoulders humped high
fn bear(look: Look) -> Vec<Vec<Part>> {
    let skin = look.skin();
    let (fur, dark, armor, glow, extra) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let b = BEAR;
    let mut out = vec![Vec::new(); PIECES];
    let body = &mut out[BODY];
    for k in 0..4 {
        let x = 0.8 - k as f64 * 0.5;
        let z = 2.5 - k as f64 * 0.07 - b.high;
        body.push(limb([x, 0.0, z + 0.08], [x - 0.15, 0.0, z + 0.45], 0.08, 0.012, extra));
        body.push(ellipsoid([x, 0.0, z], [0.3, 0.55, 0.12], armor));
    }
    body.push(ball([0.4, 0.0, 0.45], 0.62, fur));
    body.push(limb([-0.85, 0.0, -0.05], [0.75, 0.0, 0.1], 0.8, 0.85, fur));
    out[NECK] = vec![limb([0.0, 0.0, 0.0], b.neck_long, 0.55, 0.45, fur)];
    let h0 = [b.neck_at[0] + b.neck_long[0], 0.0, b.high + b.neck_at[2] + b.neck_long[2]];
    let h = |v: [f64; 3]| [v[0] - h0[0], v[1], v[2] - h0[2]];
    let head = &mut out[HEAD];
    for s in [-1.0, 1.0] {
        head.push(ball(h([1.82, s * 0.22, 1.98]), 0.065, glow));
        head.push(limb(h([2.0, s * 0.14, 1.52]), h([2.3, s * 0.2, 1.22]), 0.05, 0.015, extra));
        head.push(ball(h([1.3, s * 0.33, 2.27]), 0.12, fur));
    }
    head.push(ball(h([2.2, 0.0, 1.7]), 0.07, crate::worldgen::material::EYE));
    head.push(ellipsoid(h([1.92, 0.0, 1.68]), [0.32, 0.22, 0.2], dark));
    head.push(ellipsoid(h([1.62, 0.0, 2.14]), [0.32, 0.3, 0.1], armor));
    head.push(ball(h([1.5, 0.0, 1.85]), 0.48, fur));
    out[TAIL] = vec![ball([0.0, 0.0, 0.0], 0.15, fur)];
    for k in 0..4 {
        let hind = k >= 2;
        let [upper, lower] = b.legs[hind as usize];
        let r = if hind { 0.3 } else { 0.28 };
        out[UPPER[k]] = vec![limb([0.0, 0.0, 0.0], [0.0, 0.0, -upper], r * 1.2, r, fur)];
        out[LOWER[k]] = vec![cuboid([-0.5, -0.5, -lower - 0.6], [0.5, 0.5, -lower], AIR), ball([0.08, 0.0, -lower + 0.2], r * 1.1, dark), ball([0.0, 0.0, 0.0], r, fur), limb([0.0, 0.0, 0.0], [0.0, 0.0, -lower + 0.2], r, r * 0.9, fur)];
    }
    out
}

/// The Alpha Direwolf and the Winter Alpha: a wolf twice a man's height, lean, its eyes glowing
fn wolf(look: Look) -> Vec<Vec<Part>> {
    let skin = look.skin();
    let (fur, dark, accent, glow) = (skin.body, skin.dark, skin.accent, skin.glow);
    let b = WOLF;
    let mut out = vec![Vec::new(); PIECES];
    out[BODY] = vec![ellipsoid([0.28, 0.0, -0.05], [0.22, 0.17, 0.18], accent), ellipsoid([0.0, 0.0, 0.0], [0.5, 0.18, 0.19], fur)];
    out[NECK] = vec![limb([0.0, 0.0, 0.0], b.neck_long, 0.12, 0.1, fur)];
    let head = &mut out[HEAD];
    for s in [-1.0, 1.0] {
        head.push(ball([0.09, s * 0.07, 0.05], 0.025, glow));
        head.push(limb([-0.03, s * 0.07, 0.1], [-0.05, s * 0.09, 0.24], 0.04, 0.01, dark));
    }
    head.push(ball([0.2, 0.0, -0.04], 0.03, crate::worldgen::material::EYE));
    head.push(limb([0.0, 0.0, 0.0], [0.2, 0.0, -0.04], 0.075, 0.05, fur));
    head.push(limb([0.05, 0.0, -0.08], [0.18, 0.0, -0.1], 0.04, 0.025, dark));
    head.push(ball([0.0, 0.0, 0.0], 0.13, fur));
    out[TAIL] = vec![limb([0.0, 0.0, 0.0], [-0.37, 0.0, -0.17], 0.06, 0.035, dark)];
    for k in 0..4 {
        let hind = k >= 2;
        let [upper, lower] = b.legs[hind as usize];
        out[UPPER[k]] = vec![limb([0.0, 0.0, 0.0], [0.0, 0.0, -upper], if hind { 0.075 } else { 0.065 }, 0.05, fur)];
        out[LOWER[k]] = vec![cuboid([-0.2, -0.2, -lower - 0.3], [0.2, 0.2, -lower], AIR), ball([0.03, 0.0, -lower + 0.04], 0.055, dark), limb([0.0, 0.0, 0.0], [0.0, 0.0, -lower + 0.04], 0.05, 0.04, fur)];
    }
    out
}

/// The Cinderback Tortoise: a domed shell of black glass rimmed with rock, cracks of fire across it
/// and four vents on it smoking with fire, a sandy hide under it, a beaked head with eyes of fire,
/// stubby legs with dark claws
fn tortoise(look: Look) -> Vec<Vec<Part>> {
    let skin = look.skin();
    let (hide, shell, rim, glow, claw) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let b = TORTOISE;
    let mut out = vec![Vec::new(); PIECES];
    let s = &mut out[BODY];
    for (x, y) in [(0.25, 0.2), (-0.22, -0.26), (-0.32, 0.22), (0.12, -0.3)] {
        s.push(ball([x, y, 0.84], 0.075, glow));
        s.push(limb([x, y, 0.6], [x * 1.05, y * 1.05, 0.84], 0.13, 0.09, rim));
    }
    for (from, to) in [([0.6, -0.1, 0.45], [0.2, 0.15, 0.68]), ([-0.1, 0.5, 0.42], [-0.45, 0.1, 0.62]), ([0.0, -0.55, 0.35], [-0.5, -0.3, 0.5])] {
        s.push(limb(from, to, 0.03, 0.03, glow));
    }
    s.push(ellipsoid([0.0, 0.0, 0.02], [0.92, 0.72, 0.12], rim));
    s.push(ellipsoid([0.0, 0.0, 0.2], [0.86, 0.66, 0.5], shell));
    s.push(ellipsoid([0.0, 0.0, -0.12], [0.76, 0.56, 0.2], hide));
    out[NECK] = vec![limb([0.0, 0.0, 0.0], b.neck_long, 0.15, 0.12, hide)];
    out[HEAD] = vec![
        ball([0.16, -0.09, 0.07], 0.03, glow),
        ball([0.16, 0.09, 0.07], 0.03, glow),
        limb([0.12, 0.0, -0.02], [0.32, 0.0, -0.09], 0.08, 0.03, claw),
        ellipsoid([0.06, 0.0, 0.02], [0.18, 0.13, 0.12], hide),
    ];
    out[TAIL] = vec![limb([0.0, 0.0, 0.0], [-0.2, 0.0, -0.08], 0.07, 0.02, hide)];
    for k in 0..4 {
        let hind = k >= 2;
        let [upper, lower] = b.legs[hind as usize];
        out[UPPER[k]] = vec![limb([0.0, 0.0, 0.0], [0.0, 0.0, -upper], 0.16, 0.14, hide)];
        let mut foot = vec![cuboid([-0.3, -0.3, -lower - 0.4], [0.3, 0.3, -lower], AIR)];
        for y in [-0.07, 0.0, 0.07] {
            foot.push(limb([0.1, y, -lower + 0.05], [0.2, y * 1.3, -lower + 0.02], 0.03, 0.015, claw));
        }
        foot.push(ellipsoid([0.04, 0.0, -lower + 0.07], [0.16, 0.15, 0.08], hide));
        foot.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, -lower + 0.08], 0.14, 0.15, hide));
        out[LOWER[k]] = foot;
    }
    out
}

/// The Dreadmare: a black horse gaunt enough its ribs show pale, blood burning in its mane, its
/// tail and its eyes, red tack hanging off it, hooves of black glass
fn mare(look: Look) -> Vec<Vec<Part>> {
    let skin = look.skin();
    let (coat, hoof, rib, glow, tack) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let b = HORSE;
    let mut out = vec![Vec::new(); PIECES];
    let body = &mut out[BODY];
    for k in 0..4 {
        let x = 0.25 - 0.17 * k as f64;
        for s in [-1.0, 1.0] {
            body.push(limb([x, s * 0.2, 0.24], [x + 0.06, s * 0.28, -0.12], 0.025, 0.02, rib));
        }
    }
    // A torn red saddlecloth, and the strap of a bridle long since broken
    body.push(ellipsoid([-0.05, 0.0, 0.28], [0.3, 0.3, 0.06], tack));
    body.push(ellipsoid([0.45, 0.0, -0.02], [0.4, 0.28, 0.36], coat));
    body.push(ellipsoid([-0.5, 0.0, 0.02], [0.4, 0.29, 0.33], coat));
    body.push(ellipsoid([0.0, 0.0, 0.0], [0.8, 0.26, 0.31], coat));
    // A mane of blood-fire along the top of its neck
    let mut neck = Vec::new();
    for k in 0..6 {
        let f = k as f64 / 5.0;
        let at = [b.neck_long[0] * f - 0.08, 0.0, b.neck_long[2] * f + 0.08];
        neck.push(limb(at, [at[0] - 0.22, (k % 2) as f64 * 0.06 - 0.03, at[2] + 0.12], 0.05, 0.01, glow));
    }
    neck.push(limb([0.0, 0.0, 0.0], b.neck_long, 0.2, 0.13, coat));
    out[NECK] = neck;
    let h = &mut out[HEAD];
    for s in [-1.0, 1.0] {
        h.push(ball([0.14, s * 0.1, 0.05], 0.035, glow));
        h.push(limb([0.0, s * 0.07, 0.1], [-0.05, s * 0.09, 0.26], 0.04, 0.012, coat));
        h.push(ball([0.5, s * 0.04, -0.21], 0.025, glow));
    }
    h.push(limb([0.12, 0.0, 0.12], [0.3, 0.0, 0.05], 0.03, 0.03, tack));
    h.push(limb([0.05, 0.0, 0.05], [0.52, 0.0, -0.22], 0.12, 0.075, coat));
    h.push(ellipsoid([0.05, 0.0, 0.05], [0.14, 0.11, 0.13], coat));
    out[TAIL] = vec![limb([0.0, 0.0, 0.0], [-0.25, 0.0, -0.35], 0.06, 0.03, glow), limb([0.0, 0.05, 0.0], [-0.3, 0.08, -0.25], 0.04, 0.01, glow)];
    for k in 0..4 {
        let hind = k >= 2;
        let [upper, lower] = b.legs[hind as usize];
        out[UPPER[k]] = vec![limb([0.0, 0.0, 0.0], [0.0, 0.0, -upper], if hind { 0.17 } else { 0.13 }, 0.075, coat)];
        out[LOWER[k]] = vec![
            cuboid([-0.2, -0.2, -lower - 0.3], [0.2, 0.2, -lower], AIR),
            ellipsoid([0.03, 0.0, -lower + 0.05], [0.09, 0.08, 0.07], hoof),
            ball([0.0, 0.0, -lower + 0.16], 0.05, glow),
            ball([0.0, 0.0, 0.0], 0.07, coat),
            limb([0.0, 0.0, 0.0], [0.0, 0.0, -lower + 0.08], 0.06, 0.045, coat),
        ];
    }
    out
}

/// The Hoarback Mammoth: a hill of white wool over grey, its fringe hanging to its knees, ice grown
/// in spikes along its back with frost glowing in them, a domed head with small eyes of frost,
/// flapping ears, a long trunk curling at its end, and great tusks of old bone sweeping out and up
fn mammoth(look: Look) -> Vec<Vec<Part>> {
    let skin = look.skin();
    let (wool, grey, ice, glow, tusk) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let b = MAMMOTH;
    let mut out = vec![Vec::new(); PIECES];
    let body = &mut out[BODY];
    for (x, y, h) in [(0.55, 0.0, 0.5), (0.25, 0.15, 0.45), (0.1, -0.18, 0.5), (-0.25, 0.1, 0.4), (-0.55, -0.08, 0.35), (0.4, -0.2, 0.35)] {
        body.push(ball([x + 0.05, y, 0.95 + h * 0.6], 0.05, glow));
        body.push(limb([x, y, 0.75], [x - 0.12, y * 1.4, 0.8 + h], 0.13, 0.02, ice));
    }
    // Its fringe of grey hanging from its flanks and belly
    for k in 0..7 {
        let x = 0.7 - 0.25 * k as f64;
        for s in [-1.0, 1.0] {
            body.push(limb([x, s * 0.6, -0.2], [x - 0.05, s * 0.66, -0.85], 0.12, 0.06, grey));
        }
    }
    body.push(ellipsoid([0.55, 0.0, 0.5], [0.6, 0.56, 0.5], wool));
    body.push(ellipsoid([0.1, 0.0, 0.05], [1.15, 0.7, 0.74], wool));
    out[NECK] = vec![limb([0.0, 0.0, 0.0], b.neck_long, 0.55, 0.5, wool)];
    let h = &mut out[HEAD];
    for s in [-1.0, 1.0] {
        h.push(ball([0.42, s * 0.3, 0.12], 0.05, glow));
        h.push(ellipsoid([0.0, s * 0.46, 0.0], [0.12, 0.2, 0.28], grey));
        // A tusk out of either side of its trunk, sweeping down, out and up
        h.push(limb([0.45, s * 0.2, -0.32], [1.05, s * 0.42, -0.55], 0.1, 0.08, tusk));
        h.push(limb([1.05, s * 0.42, -0.55], [1.45, s * 0.36, -0.15], 0.08, 0.055, tusk));
        h.push(limb([1.45, s * 0.36, -0.15], [1.48, s * 0.18, 0.25], 0.055, 0.02, tusk));
    }
    h.push(limb([0.48, 0.0, -0.1], [0.62, 0.0, -0.55], 0.19, 0.14, grey));
    h.push(limb([0.62, 0.0, -0.55], [0.72, 0.0, -0.95], 0.14, 0.11, grey));
    h.push(limb([0.72, 0.0, -0.95], [0.86, 0.0, -1.08], 0.11, 0.08, grey));
    h.push(ellipsoid([0.15, 0.0, 0.3], [0.38, 0.36, 0.32], wool));
    h.push(ellipsoid([0.2, 0.0, 0.05], [0.45, 0.42, 0.45], wool));
    out[TAIL] = vec![ball([-0.12, 0.0, -0.5], 0.08, grey), limb([0.0, 0.0, 0.0], [-0.1, 0.0, -0.45], 0.08, 0.05, wool)];
    for k in 0..4 {
        let hind = k >= 2;
        let [upper, lower] = b.legs[hind as usize];
        out[UPPER[k]] = vec![limb([0.0, 0.0, 0.0], [0.0, 0.0, -upper], 0.3, 0.26, wool)];
        out[LOWER[k]] = vec![
            cuboid([-0.5, -0.5, -lower - 0.6], [0.5, 0.5, -lower], AIR),
            ellipsoid([0.12, 0.0, -lower + 0.06], [0.1, 0.16, 0.06], tusk),
            ellipsoid([0.0, 0.0, -lower + 0.12], [0.28, 0.28, 0.13], grey),
            limb([0.0, 0.0, 0.0], [0.0, 0.0, -lower + 0.12], 0.26, 0.25, wool),
        ];
    }
    out
}

/// The Gilded Sphinx: a lion of carved sandstone, darker where it's carved, its wings folded along
/// its back banded in gold, a headdress striped teal and gold falling either side of a stern face,
/// a ceremonial beard of gold, the sun burning in its eyes and on its breast
fn sphinx(look: Look) -> Vec<Vec<Part>> {
    let skin = look.skin();
    let (stone, carved, gold, glow, teal) = (skin.body, skin.dark, skin.accent, skin.glow, skin.extra);
    let b = LION;
    let mut out = vec![Vec::new(); PIECES];
    let body = &mut out[BODY];
    body.push(ball([0.72, 0.0, 0.05], 0.09, glow));
    body.push(ellipsoid([0.7, 0.0, 0.05], [0.06, 0.18, 0.18], gold));
    for s in [-1.0, 1.0] {
        // Its folded wings, feathers carved in bands of gold
        body.push(ellipsoid([-0.08, s * 0.3, 0.36], [0.72, 0.07, 0.24], gold).striped(carved));
        body.push(limb([0.5, s * 0.28, 0.36], [0.2, s * 0.32, 0.6], 0.07, 0.05, gold));
    }
    body.push(ellipsoid([0.42, 0.0, 0.0], [0.42, 0.32, 0.38], stone));
    body.push(ellipsoid([-0.5, 0.0, 0.02], [0.36, 0.31, 0.32], stone));
    body.push(ellipsoid([0.0, 0.0, 0.0], [0.8, 0.28, 0.29], stone));
    // Its headdress falling either side of its neck
    let mut neck = Vec::new();
    for s in [-1.0, 1.0] {
        neck.push(limb([b.neck_long[0], s * 0.17, b.neck_long[2]], [0.02, s * 0.24, -0.02], 0.09, 0.07, teal).striped(gold));
    }
    neck.push(limb([0.0, 0.0, 0.0], b.neck_long, 0.24, 0.19, stone));
    out[NECK] = neck;
    let h = &mut out[HEAD];
    for s in [-1.0, 1.0] {
        h.push(ball([0.17, s * 0.065, 0.07], 0.03, glow));
    }
    h.push(limb([0.18, 0.0, -0.08], [0.21, 0.0, -0.3], 0.04, 0.03, gold));
    h.push(limb([0.12, 0.0, 0.24], [0.22, 0.0, 0.2], 0.03, 0.02, gold));
    h.push(ellipsoid([0.14, 0.0, 0.0], [0.1, 0.12, 0.14], stone));
    h.push(ellipsoid([0.02, 0.0, 0.08], [0.17, 0.24, 0.2], teal).striped(gold));
    out[TAIL] = vec![ball([-0.5, 0.0, -0.42], 0.06, carved), limb([0.0, 0.0, 0.0], [-0.5, 0.0, -0.38], 0.05, 0.035, stone)];
    for k in 0..4 {
        let hind = k >= 2;
        let [upper, lower] = b.legs[hind as usize];
        out[UPPER[k]] = vec![limb([0.0, 0.0, 0.0], [0.0, 0.0, -upper], if hind { 0.21 } else { 0.18 }, 0.13, stone)];
        let mut paw = vec![cuboid([-0.25, -0.25, -lower - 0.35], [0.25, 0.25, -lower], AIR)];
        for y in [-0.06, 0.0, 0.06] {
            paw.push(ball([0.18, y, -lower + 0.03], 0.03, gold));
        }
        paw.push(ellipsoid([0.07, 0.0, -lower + 0.07], [0.15, 0.12, 0.08], stone));
        paw.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, -lower + 0.06], 0.12, 0.1, stone));
        out[LOWER[k]] = paw;
    }
    out
}

/// Where each piece goes in `s`
fn solve(b: &Shape, s: &Stance, bones: &mut [Bone; PIECES], sockets: &mut Sockets) {
    let at = V::new(0.0, 0.0, b.high) + V::from(s.body);
    let turn = rz(s.yaw) * ry(s.pitch) * rx(s.roll);
    bones[BODY] = Bone::new(at, turn);
    for k in 0..4 {
        let hind = k >= 2;
        let [upper, lower] = b.legs[hind as usize];
        let joint = at + turn * V::from(b.joints[k]);
        // A stag's forelegs bend forward at the knee and hind legs back at the hock, a bear's the
        // other way about
        let rest_pole = V::x() * b.bends[hind as usize];
        let pole = rz(s.yaw) * rest_pole;
        let (knee, foot) = bend(joint, V::from(s.feet[k]), upper, lower, pole);
        bones[UPPER[k]] = Bone::new(joint, aim(-V::z(), rest_pole, knee - joint, pole));
        bones[LOWER[k]] = Bone::new(knee, aim(-V::z(), rest_pole, foot - knee, pole));
    }
    let neck_at = at + turn * V::from(b.neck_at);
    let neck = turn * rz(s.neck[1]) * ry(-s.neck[0]);
    bones[NECK] = Bone::new(neck_at, neck);
    let head_at = neck_at + neck * V::from(b.neck_long);
    let head = neck * rz(s.head[1]) * ry(s.head[0]);
    bones[HEAD] = Bone::new(head_at, head);
    bones[TAIL] = Bone::new(at + turn * V::from(b.tail_at), turn * rz(s.tail[1]) * ry(-s.tail[0]));
    sockets.mouth = head_at + head * V::from(b.mouth);
    // Thorns flung off the tips of its antlers
    sockets.cast = head_at + head * V::from(b.cast);
    sockets.hands = [head_at + head * V::new(0.0, -0.4, 0.7), head_at + head * V::new(0.0, 0.4, 0.7)];
    sockets.tip = sockets.mouth;
    sockets.grip = sockets.mouth;
}

/// Its walk or gallop laid over `s`, as much as it's free to: a foot at a time walking, each
/// planted and going back under it then lifted and swung forward; at a run, its hind feet together
/// then its fore, its body pitching and its head reaching out with each bound
fn walk(s: &mut Stance, m: &Moment, free: f64) {
    let (walking, running) = m.pace();
    let amount = walking * free;
    if amount <= 0.0 {
        return;
    }
    let walk_off = [0.75, 0.25, 0.5, 0.0];
    let run_off = [0.55, 0.45, 0.08, 0.0];
    let duty = 0.62 - 0.22 * running;
    let step = m.stride * duty * 0.5 * amount;
    for k in 0..4 {
        let off = walk_off[k] + (run_off[k] - walk_off[k]) * running;
        let phi = (m.phase + off).fract();
        let (x, lift) = if phi < duty {
            (step * (1.0 - 2.0 * phi / duty), 0.0)
        } else {
            let u = (phi - duty) / (1.0 - duty);
            (step * (2.0 * smooth(u) - 1.0), (0.22 + 0.2 * running) * (PI * u).sin())
        };
        s.feet[k][0] += x;
        s.feet[k][2] += lift * amount;
    }
    let w = TAU * m.phase;
    s.body[2] += 0.035 * amount * (2.0 * w).cos() * (1.0 - running) + 0.12 * running * (w).sin();
    s.pitch += 0.09 * running * (w + 1.0).sin();
    s.roll += 0.03 * amount * w.sin() * (1.0 - running);
    s.neck[0] -= 0.12 * running;
    s.head[0] += 0.1 * amount * (2.0 * w).sin() * (1.0 - running) - 0.12 * running * w.cos();
    s.tail[1] += 0.15 * amount * w.sin();
}

fn guard(rest: &Stance) -> Stance {
    Stance { body: [-0.05, 0.0, -0.05], neck: [0.15, 0.0], head: [0.3, 0.0], tail: [0.1, 0.0], ..*rest }
}

fn fight(look: Look, rest: &Stance, m: &Moment) -> (Stance, f64, f64) {
    let guard = if m.engaged { guard(rest) } else { *rest };
    let Some(d) = m.doing else {
        return (guard, 6.0, 1.0);
    };
    let pick = |(s, quick): (Stance, bool), slow: f64| (s, if quick { 40.0 } else { slow }, 0.0);
    let f = rest.feet;
    // Reared up on its hind legs, forehooves high, then brought crashing down ahead
    let mut reared = Stance { body: [-0.35, 0.0, 0.45], pitch: -0.62, neck: [0.4, 0.0], head: [-0.2, 0.0], jaw: 1.0, tail: [0.4, 0.0], ..guard };
    reared.feet = [[0.75, -0.24, 1.25], [0.6, 0.24, 1.0], [f[2][0] + 0.1, -0.26, 0.0], [f[3][0] + 0.1, 0.26, 0.0]];
    let mut stamped = Stance { body: [0.3, 0.0, -0.2], pitch: 0.18, neck: [-0.1, 0.0], head: [0.5, 0.0], tail: [0.2, 0.0], ..guard };
    stamped.feet = [[1.25, -0.25, 0.0], [1.15, 0.25, 0.0], [f[2][0] + 0.2, -0.22, 0.0], [f[3][0] + 0.1, 0.22, 0.0]];
    // Head down, antlers lowered at the player
    let lowered = Stance { body: [-0.15, 0.0, -0.12], pitch: 0.12, neck: [-0.55, 0.0], head: [0.55, 0.0], tail: [0.5, 0.0], ..guard };
    // Head thrown back, bellowing
    let mut bellow = Stance { body: [-0.15, 0.0, 0.1], pitch: -0.18, neck: [0.75, 0.0], head: [-0.65, 0.0], jaw: 1.0, tail: [0.5, 0.0], ..guard };
    bellow.feet[0][0] += 0.15;
    let wolf = matches!(look, Look::Direwolf | Look::WinterAlpha);
    let bear = matches!(look, Look::Ironhide | Look::Frostfang);
    // Crouched low to spring, then lunging out with its head and jaws
    let mut crouched = Stance { body: [-0.3, 0.0, -0.35], pitch: 0.08, neck: [-0.35, 0.0], head: [0.4, 0.0], tail: [-0.2, 0.0], ..guard };
    crouched.feet[2][0] -= 0.15;
    crouched.feet[3][0] -= 0.15;
    let mut lunged = Stance { body: [0.55, 0.0, -0.1], pitch: 0.12, neck: [-0.15, 0.0], head: [0.2, 0.0], jaw: 1.0, tail: [0.3, 0.0], ..guard };
    lunged.feet[0][0] += 0.7;
    lunged.feet[1][0] += 0.6;
    if let Some(own) = own_fight(look, &d, f, &guard, &reared, &stamped, &lowered, &bellow) {
        return own;
    }
    match (d.m, d.stage) {
        (Move::Slam, Stage::Wind) if wolf => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), crouched, Ease::Gather), (d.len, lunged, Ease::Snap)]), 10.0),
        (Move::Slam, Stage::Act) if wolf => pick((lunged, false), 25.0),
        (Move::Leap, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len, crouched, Ease::Gather)]), 10.0),
        (Move::Leap, Stage::Act) => {
            // Stretched out through the air, forelegs reaching, then landing
            let mut flying = Stance { body: [0.0, 0.0, 0.2], pitch: -0.15, neck: [0.0, 0.0], head: [0.2, 0.0], jaw: 1.0, tail: [0.2, 0.0], ..guard };
            flying.feet[0] = [f[0][0] + 0.7, f[0][1], 0.9];
            flying.feet[1] = [f[1][0] + 0.6, f[1][1], 0.8];
            flying.feet[2] = [f[2][0] - 0.6, f[2][1], 0.7];
            flying.feet[3] = [f[3][0] - 0.5, f[3][1], 0.6];
            pick(along(d.t, &[(0.0, flying, Ease::Linear), (d.len * 0.75, flying, Ease::Linear), (d.len, stamped, Ease::Snap)]), 18.0)
        }
        // A bear swiping with one forepaw then the other, reared half up
        (Move::Maul, Stage::Wind | Stage::Act) if bear => {
            let k = if d.stage == Stage::Act { (d.t / 0.65) as usize } else { 0 };
            let local = if d.stage == Stage::Act { d.t % 0.65 } else { d.t };
            let side = k % 2;
            let mut up = Stance { body: [-0.1, 0.0, 0.2], pitch: -0.25, neck: [0.2, 0.0], head: [0.1, 0.0], jaw: 1.0, ..guard };
            up.feet[side] = [f[side][0] + 0.3, f[side][1] * 1.6, 1.3];
            let mut down = Stance { body: [0.3, 0.0, -0.05], pitch: 0.1, ..up };
            down.feet[side] = [f[side][0] + 1.0, -f[side][1] * 0.3, 0.1];
            let (s, quick) = if d.stage == Stage::Wind { along(d.t, &[(0.0, guard, Ease::Linear), (d.len, up, Ease::Gather)]) } else { along(local, &[(0.0, up, Ease::Linear), (0.38, up, Ease::Gather), (0.5, down, Ease::Snap), (0.65, down, Ease::Linear)]) };
            (s, if quick { 40.0 } else { 14.0 }, if d.stage == Stage::Act { 1.0 } else { 0.0 })
        }
        (Move::Slam, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), reared, Ease::Gather), (d.len, stamped, Ease::Snap)]), 9.0),
        (Move::Slam, Stage::Act) => pick((stamped, false), 25.0),
        (Move::Charge | Move::Trample, Stage::Wind) => {
            let mut s = lowered;
            // Pawing the ground with a forehoof
            let paw = (d.t * 8.0).sin();
            s.feet[0] = [f[0][0] + 0.15 * paw, f[0][1], 0.12 * paw.max(0.0)];
            pick((s, false), 10.0)
        }
        (Move::Charge | Move::Trample, Stage::Act) => (lowered, 10.0, 1.0),
        // Its antlers swept side to side, three times
        (Move::Maul, Stage::Wind | Stage::Act) => {
            let k = if d.stage == Stage::Act { (d.t / 0.65) as usize } else { 0 };
            let local = if d.stage == Stage::Act { d.t % 0.65 } else { d.t };
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            let from = Stance { neck: [-0.35, side * 0.5], head: [0.5, side * 0.4], ..lowered };
            let to = Stance { neck: [-0.4, -side * 0.55], head: [0.55, -side * 0.45], body: [0.25, 0.0, -0.12], ..lowered };
            let (s, quick) = if d.stage == Stage::Wind { along(d.t, &[(0.0, guard, Ease::Linear), (d.len, from, Ease::Gather)]) } else { along(local, &[(0.0, from, Ease::Linear), (0.4, from, Ease::Linear), (0.52, to, Ease::Snap), (0.65, to, Ease::Linear)]) };
            (s, if quick { 40.0 } else { 14.0 }, if d.stage == Stage::Act { 1.0 } else { 0.0 })
        }
        // Its head shaken hard, thorns flung off its antlers
        (Move::Volley | Move::RootWave, Stage::Wind | Stage::Act) => {
            let mut s = Stance { neck: [0.3, 0.0], head: [0.1, 0.0], ..guard };
            let shake = if d.stage == Stage::Act { 1.0 } else { (d.t / d.len.max(0.1)).min(1.0) };
            s.neck[1] = 0.35 * (d.t * 18.0).sin() * shake;
            s.head[1] = 0.45 * (d.t * 18.0 + 0.6).sin() * shake;
            if d.m == Move::RootWave {
                s = mix(&s, &stamped, 0.5);
            }
            pick((s, false), 25.0)
        }
        (Move::Bellow | Move::Roar | Move::Cataclysm | Move::Summon | Move::Storm, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len * 0.5, lowered, Ease::Gather), (d.len, bellow, Ease::Snap)]), 10.0),
        (Move::Bellow | Move::Roar | Move::Cataclysm | Move::Summon | Move::Storm, Stage::Act) => {
            let mut s = bellow;
            s.head[1] += 0.12 * (d.t * 11.0).sin();
            if look.is_raid() && d.m == Move::Roar && m.stage > 0 {
                s = mix(&reared, &bellow, 0.5);
            }
            pick((s, false), 14.0)
        }
        (_, Stage::Reel) => {
            let mut reel = Stance { body: [-0.35, 0.1, 0.05], pitch: -0.2, roll: 0.15, neck: [0.5, 0.4], head: [-0.4, 0.3], ..guard };
            reel.feet[0] = [f[0][0] - 0.3, -0.3, 0.0];
            pick(along(d.t, &[(0.0, guard, Ease::Linear), (0.15, reel, Ease::Gather), (d.len, guard, Ease::Smooth)]), 14.0)
        }
        (_, Stage::Stun) => {
            let mut dazed = Stance { body: [0.0, 0.0, -0.15], neck: [-0.4, 0.0], head: [0.6, 0.0], ..guard };
            dazed.neck[1] = 0.3 * (d.t * 2.2).sin();
            pick((dazed, false), 5.0)
        }
        (_, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), reared, Ease::Gather), (d.len, stamped, Ease::Snap)]), 9.0),
        (_, Stage::Act) => pick((stamped, false), 20.0),
        (_, Stage::Recover) => pick(along(d.t, &[(0.0, stamped, Ease::Linear), (d.len, guard, Ease::Smooth)]), 6.0),
    }
}

/// How the beasts of src\bosses\roster.rs go about their own moves, and those they make otherwise
/// than a stag does, posed at a stag's size as the rest are: None for what they do as a stag does
#[allow(clippy::too_many_arguments)]
fn own_fight(look: Look, d: &Doing, f: [[f64; 3]; 4], guard: &Stance, reared: &Stance, stamped: &Stance, lowered: &Stance, bellow: &Stance) -> Option<(Stance, f64, f64)> {
    let pick = |(s, quick): (Stance, bool), slow: f64| Some((s, if quick { 40.0 } else { slow }, 0.0));
    let (guard, reared, stamped, lowered, bellow) = (*guard, *reared, *stamped, *lowered, *bellow);
    // Its head drawn back, then thrust out with its jaws open, to spit
    let drawn = Stance { body: [-0.12, 0.0, 0.0], neck: [0.3, 0.0], head: [-0.2, 0.0], jaw: 0.3, ..guard };
    let spat = Stance { body: [0.15, 0.0, -0.05], neck: [-0.1, 0.0], head: [0.3, 0.0], jaw: 1.0, ..guard };
    let spit = |stage: Stage, t: f64, len: f64| match stage {
        Stage::Wind => pick(along(t, &[(0.0, guard, Ease::Linear), ((len - SNAP).max(0.05), drawn, Ease::Gather), (len, spat, Ease::Snap)]), 10.0),
        _ => pick((spat, false), 20.0),
    };
    // Stamping again and again, rearing a little way between
    let half = mix(&stamped, &reared, 0.7);
    match look {
        Look::Cinderback => {
            // Drawn into its shell, down on the ground, its head, legs and tail pulled in
            let mut hunkered = Stance { body: [0.0, 0.0, -0.6], neck: [-0.9, 0.0], head: [0.9, 0.0], tail: [-0.8, 0.0], ..guard };
            for k in 0..4 {
                hunkered.feet[k] = [f[k][0] * 0.55, f[k][1] * 0.75, 0.0];
            }
            // Half up on its hind legs to stamp, as high as a tortoise rears
            let lifted = mix(&guard, &reared, 0.4);
            match (d.m, d.stage) {
                (Move::Vents, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len * 0.7, hunkered, Ease::Gather), (d.len, hunkered, Ease::Linear)]), 10.0),
                (Move::Vents, Stage::Act) => {
                    let mut s = hunkered;
                    s.body[2] += 0.025 * (d.t * 37.0).sin();
                    s.roll += 0.025 * (d.t * 29.0).sin();
                    pick((s, false), 20.0)
                }
                (Move::Vents, Stage::Recover) => pick(along(d.t, &[(0.0, hunkered, Ease::Linear), (d.len * 0.55, hunkered, Ease::Linear), (d.len, guard, Ease::Smooth)]), 6.0),
                (Move::Slam | Move::Quake, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), lifted, Ease::Gather), (d.len, stamped, Ease::Snap)]), 9.0),
                (Move::Spit, Stage::Wind | Stage::Act) => spit(d.stage, d.t, d.len),
                _ => None,
            }
        }
        // Rearing high, then bringing its hooves down, up again a little way and down again
        Look::Dreadmare => match (d.m, d.stage) {
            (Move::Hoofbrand, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), reared, Ease::Gather), (d.len, stamped, Ease::Snap)]), 10.0),
            (Move::Hoofbrand, Stage::Act) => pick(along(d.t, &[(0.0, stamped, Ease::Linear), (0.1, stamped, Ease::Linear), (0.32, half, Ease::Gather), (0.45, stamped, Ease::Snap), (d.len, stamped, Ease::Linear)]), 20.0),
            _ => None,
        },
        Look::Hoarback => {
            // Its trunk swung up and out ahead of it to spray frost, sweeping from side to side
            let spray = Stance { body: [0.05, 0.0, 0.0], neck: [0.3, 0.0], head: [-0.55, 0.0], ..guard };
            let tossed = Stance { body: [-0.05, 0.0, 0.05], neck: [0.6, 0.0], head: [-0.6, 0.0], ..guard };
            match (d.m, d.stage) {
                (Move::Rimestomp, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), reared, Ease::Gather), (d.len, stamped, Ease::Snap)]), 9.0),
                (Move::Rimestomp, Stage::Act) => pick(
                    along(d.t, &[(0.0, stamped, Ease::Linear), (0.3, half, Ease::Gather), (0.55, stamped, Ease::Snap), (0.85, half, Ease::Gather), (1.1, stamped, Ease::Snap), (d.len, stamped, Ease::Linear)]),
                    20.0,
                ),
                (Move::Breath, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len, spray, Ease::Gather)]), 10.0),
                (Move::Breath, Stage::Act) => {
                    let mut s = spray;
                    s.head[1] += 0.3 * (d.t * 1.7).sin();
                    pick((s, false), 12.0)
                }
                // Tusks lowered to the ground, then tossed up, flinging the ice it scooped up
                (Move::Throw, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), ((d.len - SNAP).max(0.05), lowered, Ease::Gather), (d.len, tossed, Ease::Snap)]), 10.0),
                (Move::Throw, Stage::Act) => pick((tossed, false), 18.0),
                _ => None,
            }
        }
        Look::GildedSphinx => {
            // Sitting up on its haunches, its head raised, forepaws planted before it
            let mut sat = Stance { body: [-0.35, 0.0, -0.05], pitch: -0.45, neck: [0.45, 0.0], head: [-0.15, 0.0], tail: [0.2, 0.0], ..guard };
            for k in 0..4 {
                let (x, y) = if k < 2 { (0.3, 1.0) } else { (0.25, 1.2) };
                sat.feet[k] = [f[k][0] + x, f[k][1] * y, 0.0];
            }
            let breathe = Stance { body: [0.1, 0.0, -0.1], pitch: 0.08, neck: [-0.15, 0.0], head: [0.25, 0.0], jaw: 1.0, ..guard };
            match (d.m, d.stage) {
                (Move::Sunwheel, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len * 0.6, sat, Ease::Gather), (d.len, mix(&sat, &bellow, 0.3), Ease::Smooth)]), 10.0),
                (Move::Sunwheel, Stage::Act) => {
                    let mut s = mix(&sat, &bellow, 0.3);
                    s.head[1] += 0.35 * (d.t * 1.1).sin();
                    pick((s, false), 10.0)
                }
                (Move::Sunwheel, Stage::Recover) => pick(along(d.t, &[(0.0, sat, Ease::Linear), (d.len, guard, Ease::Smooth)]), 5.0),
                (Move::Breath, Stage::Wind) => pick(along(d.t, &[(0.0, guard, Ease::Linear), (d.len, Stance { jaw: 0.4, neck: [0.2, 0.0], head: [-0.1, 0.0], ..breathe }, Ease::Gather)]), 10.0),
                (Move::Breath, Stage::Act) => {
                    let mut s = breathe;
                    s.neck[1] += 0.15 * (d.t * 1.7).sin();
                    pick((s, false), 12.0)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Dying: its head thrown up, its legs folding under it, then over onto its side
fn death(rest: &Stance, t: f64) -> (Stance, f64, f64) {
    let f = rest.feet;
    let hit = Stance { body: [-0.2, 0.0, 0.1], pitch: -0.2, neck: [0.6, 0.0], head: [-0.5, 0.0], jaw: 1.0, ..*rest };
    let mut folded = Stance { body: [0.0, 0.0, -0.7], pitch: 0.1, neck: [-0.1, 0.2], head: [0.4, 0.2], ..*rest };
    for k in 0..4 {
        folded.feet[k] = [f[k][0] + if k < 2 { 0.3 } else { -0.1 }, f[k][1] * 1.4, 0.0];
    }
    let mut fallen = Stance { body: [0.0, 0.25, -0.88], roll: 1.25, neck: [-0.3, 0.6], head: [0.6, 0.5], tail: [-0.2, 0.0], ..folded };
    for k in 0..4 {
        fallen.feet[k] = [f[k][0] + 0.2, -0.75, 0.25];
    }
    let (s, quick) = along(t, &[(0.0, *rest, Ease::Linear), (0.3, hit, Ease::Gather), (THUDS[0], folded, Ease::Snap), (0.95, folded, Ease::Linear), (THUDS[1], fallen, Ease::Snap), (DEATH, fallen, Ease::Linear)]);
    (s, if quick { 28.0 } else { 9.0 }, 0.0)
}

pub(super) fn animate(r: &mut Rig, m: &Moment) {
    let look = r.look;
    // Its poses are made at a stag's size, and fitted to its own
    let stag = rest(Look::Elderhorn);
    let b = shape(look);
    let (want, rate, free) = match m.dying {
        Some(t) => death(&stag, t),
        None => fight(look, &stag, m),
    };
    let want = fitted(&b, &want);
    if !r.posed {
        *r.beast() = want;
    }
    ease(r.beast(), &want, rate, m.dt);
    let mut s = *r.beast();
    if m.dying.is_none() {
        walk(&mut s, m, r.walking(m, free));
        // Breathing, its head turning to watch, and flinching from blows
        let breath = (r.clock * TAU / 3.2).sin();
        s.body[2] += 0.015 * breath;
        s.neck[1] += 0.6 * r.glance[0];
        s.head[0] += 0.5 * r.glance[1];
        let fl = r.flinched();
        s.pitch -= 0.4 * fl[0];
        s.body[0] -= 0.2 * fl[0];
        s.neck[0] += 0.6 * fl[0];
        s.neck[1] += 0.8 * fl[1];
        s.roll += 0.4 * fl[2];
    }
    solve(&shape(look), &s, &mut r.bones, &mut r.sockets);
    // What the Hoarback tosses comes off the ends of its tusks
    if look == Look::Hoarback {
        let head = r.bones[HEAD];
        r.sockets.grip = head.at + head.turn * V::new(1.45, 0.0, 0.05);
    }
}
