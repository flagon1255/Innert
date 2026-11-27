//! How a necromancer's skull focus and a priest's talisman look, see build_look(): each held up in
//! the fist, small enough for one hand, what its spell comes out of on top.

use super::*;

/// A skull on a short grip: a man's, a horned goat's, or one crowned, its eyes glowing with its
/// crystal, a binding round its neck. Returns its parts, where it's held, and between its eyes.
pub(super) fn skull(shaft: &Visual, focus: &Visual, crystal: &Visual, binding: &Visual) -> (Vec<Part>, Vector3<f64>, Vector3<f64>) {
    let mut p = Vec::new();
    let grip = mat(&shaft.material, BONE);
    let pommel = mat(&shaft.accent, LEATHER);
    let bone = mat(&focus.material, BONE);
    let accent = mat(&focus.accent, bone);
    let gem = mat(&crystal.gem, GLOW_VOID);
    let band = mat(&binding.material, LINEN);
    let trim = mat(&binding.accent, band);
    let top = 0.21;
    let c = [0.0, 0.0, top + 0.065];
    // Its eyes first, so they show through the bone, and the hole of its nose cut into it
    slot("crystal");
    let eye = crystal.size.unwrap_or(0.016).clamp(0.012, 0.03);
    for s in [-1.0, 1.0] {
        p.push(ball([0.048, s * 0.022, c[2] + 0.004], eye, gem));
    }
    slot("focus");
    p.push(cuboid([0.05, -0.006, c[2] - 0.03], [0.075, 0.006, c[2] - 0.014], AIR));
    // Teeth along the jaw, the jaw, the brow and the cranium
    for k in 0..5 {
        let y = (k as f64 - 2.0) * 0.011;
        p.push(cuboid([0.047, y - 0.004, c[2] - 0.05], [0.055, y + 0.004, c[2] - 0.038], if k % 2 == 0 { BONE } else { tooth(bone) }));
    }
    let shape = focus.shape.as_deref().unwrap_or("skull");
    // A beast's: a long snout reaching forward under its brow, fangs hooking down out of it
    if shape == "fanged" {
        p.push(limb([0.03, 0.0, c[2] - 0.022], [0.115, 0.0, c[2] - 0.034], 0.03, 0.017, bone).stretched([1.0, 0.95, 0.8]));
        p.push(limb([0.04, 0.0, c[2] - 0.05], [0.11, 0.0, c[2] - 0.052], 0.02, 0.012, bone).stretched([1.0, 0.95, 0.6]));
        for s in [-1.0, 1.0] {
            p.push(limb([0.1, s * 0.012, c[2] - 0.036], [0.106, s * 0.013, c[2] - 0.062], 0.006, 0.002, tooth(bone)));
            p.push(limb([0.07, s * 0.018, c[2] - 0.036], [0.072, s * 0.019, c[2] - 0.052], 0.004, 0.0015, tooth(bone)));
            // Its cheekbones flaring out
            p.push(limb([0.02, s * 0.05, c[2] - 0.01], [0.06, s * 0.034, c[2] - 0.024], 0.01, 0.007, accent));
        }
    } else {
        p.push(ellipsoid([0.024, 0.0, c[2] - 0.048], [0.034, 0.036, 0.018], bone));
    }
    p.push(limb([0.046, -0.03, c[2] + 0.024], [0.046, 0.03, c[2] + 0.024], 0.01, 0.01, bone));
    p.push(ellipsoid([0.0, 0.0, c[2]], [0.062, 0.052, 0.058], bone));
    match shape {
        // Horns curling back and out from its brow
        "horned" => {
            for s in [-1.0, 1.0] {
                curve(&mut p, &[[0.02, s * 0.04, c[2] + 0.04], [-0.02, s * 0.085, c[2] + 0.08], [-0.07, s * 0.1, c[2] + 0.05], [-0.075, s * 0.08, c[2] + 0.0]], 0.014, 0.005, accent);
            }
        }
        // A crown of spikes round its pate
        "crowned" => {
            ring(&mut p, [0.0, 0.0, c[2] + 0.04], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0.05, 0.008, accent);
            for k in 0..7 {
                let a = k as f64 / 7.0 * TAU;
                let (x, y) = (a.cos() * 0.05, a.sin() * 0.05);
                p.push(limb([x, y, c[2] + 0.04], [x * 1.1, y * 1.1, c[2] + 0.075], 0.008, 0.003, accent));
            }
        }
        // A stag's antlers branching up and out from its brow, three tines each
        "antlered" => {
            for s in [-1.0, 1.0] {
                // Kept short, so they don't fill the view held up before the eye
                let beam = [[0.01, s * 0.035, c[2] + 0.045], [-0.005, s * 0.06, c[2] + 0.08], [-0.02, s * 0.078, c[2] + 0.115], [-0.04, s * 0.085, c[2] + 0.145]];
                curve(&mut p, &beam, 0.009, 0.004, accent);
                for (k, t) in [(1usize, 0.035), (2, 0.04), (3, 0.028)] {
                    let b = beam[k];
                    p.push(limb(b, [b[0] + t, b[1] + s * 0.008, b[2] + 0.03], 0.006, 0.002, accent));
                }
            }
        }
        _ => {}
    }
    // Its binding round its neck, and what hangs from it
    slot("binding");
    match binding.shape.as_deref().unwrap_or("wraps") {
        "rings" => {
            for z in [top - 0.035, top - 0.012] {
                p.push(limb([0.0, 0.0, z], [0.0, 0.0, z + 0.01], 0.024, 0.024, band));
            }
            if trim != band {
                p.push(ball([0.024, 0.0, top - 0.024], 0.007, trim));
            }
        }
        // Stubs of grave candles standing on a collar round its neck, wax run down them, their
        // flames' wicks glowing
        "candles" => {
            p.push(limb([0.0, 0.0, top - 0.04], [0.0, 0.0, top - 0.012], 0.026, 0.028, trim));
            for k in 0..3 {
                let a = k as f64 / 3.0 * TAU + 0.5;
                let (x, y) = (a.cos() * 0.03, a.sin() * 0.03);
                let tall = 0.03 + 0.012 * k as f64;
                p.push(limb([x, y, top - 0.015], [x, y, top - 0.015 + tall], 0.008, 0.007, band));
                p.push(ball([x, y, top - 0.012 + tall], 0.004, GLOW_FIRE));
                p.push(limb([x + 0.006, y, top - 0.012 + tall * 0.8], [x + 0.008, y, top - 0.015 + tall * 0.3], 0.003, 0.002, band));
            }
        }
        "bones" => {
            for s in [-1.0, 1.0] {
                p.push(limb([0.0, s * 0.024, top - 0.02], [0.008, s * 0.045, top - 0.1], 0.006, 0.005, band));
                p.push(limb([0.012, s * 0.012, top - 0.02], [0.02, s * 0.02, top - 0.09], 0.005, 0.004, band));
            }
            p.push(limb([0.0, 0.0, top - 0.04], [0.0, 0.0, top - 0.01], 0.022, 0.022, trim));
        }
        _ => {
            p.push(limb([0.0, 0.0, top - 0.045], [0.0, 0.0, top - 0.005], 0.023, 0.025, band));
            p.push(limb([0.0, 0.02, top - 0.02], [0.01, 0.04, top - 0.11], 0.007, 0.004, band));
        }
    }
    // The grip: plain, or a spine of knuckled bones, with a knob at its foot
    slot("shaft");
    // A root, gnarled and knotted, twisting up to the skull
    if shaft.shape.as_deref() == Some("gnarled") {
        for k in 0..5 {
            let z = 0.02 + k as f64 * 0.04;
            let a = k as f64 * 1.7;
            p.push(ball([a.cos() * 0.012, a.sin() * 0.012, z], 0.013 + 0.003 * (k % 2) as f64, pommel));
        }
        curve(&mut p, &[[0.0, 0.0, -0.01], [0.012, 0.006, 0.06], [-0.01, -0.008, 0.12], [0.004, 0.01, top - 0.02]], 0.019, 0.016, grip);
    }
    if shaft.shape.as_deref() == Some("spine") {
        for k in 0..7 {
            let z = 0.01 + k as f64 * 0.026;
            p.push(ellipsoid([0.0, 0.0, z], [0.019, 0.019, 0.01], grip));
            p.push(limb([-0.012, 0.0, z], [-0.024, 0.0, z - 0.004], 0.006, 0.004, grip));
        }
    }
    p.push(ball([0.0, 0.0, -0.01], 0.021, pommel));
    p.push(limb([0.0, 0.0, -0.005], [0.0, 0.0, top - 0.02], 0.015, 0.018, grip));
    (p, v3(0.0, 0.0, 0.09), v3(0.04, 0.0, c[2] + 0.004))
}

/// Bone's paler kin for teeth, or bone itself for what isn't bone
fn tooth(m: u8) -> u8 {
    if m == BONE { FUR_CREAM } else { BONE }
}

/// A holy icon held up on a short grip: a disc, a sunwheel, a star, a halo or a pair of wings, its
/// crystal burning at its heart, ribbons, beads, seals or bells hanging under it. Returns its parts,
/// where it's held, and its heart.
pub(super) fn talisman(shaft: &Visual, focus: &Visual, crystal: &Visual, binding: &Visual) -> (Vec<Part>, Vector3<f64>, Vector3<f64>) {
    let mut p = Vec::new();
    let cord = mat(&shaft.material, LINEN);
    let link = mat(&shaft.accent, cord);
    let metal = mat(&focus.material, BRONZE);
    let accent = mat(&focus.accent, metal);
    let gem = mat(&crystal.gem, GLOW_HOLY);
    let band = mat(&binding.material, LINEN);
    let trim = mat(&binding.accent, band);
    let top = 0.11;
    let c = [0.0, 0.0, top + 0.06];
    let r = 0.046;
    // Its heart, burning on both faces
    slot("crystal");
    let heart = crystal.size.unwrap_or(0.016).clamp(0.012, 0.03);
    p.push(ellipsoid(c, [0.018, heart * 1.2, heart * 1.2], gem));
    slot("focus");
    let (y, z) = ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    match focus.shape.as_deref().unwrap_or("disc") {
        // Rays all round a disc
        "sunwheel" => {
            for k in 0..12 {
                let a = k as f64 / 12.0 * TAU;
                let long = if k % 2 == 0 { 1.75 } else { 1.4 };
                p.push(limb([0.0, a.cos() * r * 0.8, c[2] + a.sin() * r * 0.8], [0.0, a.cos() * r * long, c[2] + a.sin() * r * long], 0.009, 0.003, accent));
            }
            ring(&mut p, c, y, z, r, 0.009, metal);
            p.push(ellipsoid(c, [0.01, r, r], metal));
        }
        // Eight points, long and short
        "star" => {
            for k in 0..8 {
                let a = k as f64 / 8.0 * TAU + TAU / 16.0;
                let long = if k % 2 == 0 { 1.9 } else { 1.25 };
                p.push(limb([0.0, 0.0, c[2]], [0.0, a.cos() * r * long, c[2] + a.sin() * r * long], 0.02, 0.003, if k % 2 == 0 { metal } else { accent }));
            }
            p.push(ellipsoid(c, [0.012, r * 0.55, r * 0.55], metal));
        }
        // A ring of light round its heart, held on spokes
        "halo" => {
            ring(&mut p, c, y, z, r * 1.15, 0.011, metal);
            ring(&mut p, c, y, z, r * 0.75, 0.006, accent);
            for k in 0..4 {
                let a = k as f64 / 4.0 * TAU + TAU / 8.0;
                p.push(limb([0.0, a.cos() * r * 0.4, c[2] + a.sin() * r * 0.4], [0.0, a.cos() * r * 1.12, c[2] + a.sin() * r * 1.12], 0.005, 0.005, metal));
            }
        }
        // A disc with a pair of wings spread from it
        "wings" => {
            for s in [-1.0, 1.0] {
                for k in 0..4 {
                    let f = k as f64 / 3.0;
                    p.push(ellipsoid([-0.004, s * (r * 0.9 + f * 0.05), c[2] + 0.02 + f * 0.03 - f * f * 0.05], [0.008, 0.035, 0.014 + 0.006 * (1.0 - f)], if k % 2 == 0 { accent } else { metal }));
                }
            }
            ring(&mut p, c, y, z, r * 0.8, 0.008, metal);
            p.push(ellipsoid(c, [0.01, r * 0.8, r * 0.8], metal));
        }
        // A moon on its back, horns up, its heart burning between them
        "crescent" => {
            for k in 0..11 {
                let a = PI * (0.1 + 0.8 * k as f64 / 10.0) + PI;
                let thick = 0.012 + 0.01 * (1.0 - ((k as f64 - 5.0) / 5.0).abs());
                p.push(ball([0.0, a.cos() * r * 1.15, c[2] + 0.01 + a.sin() * r * 1.15], thick, if k % 5 == 0 { accent } else { metal }));
            }
            ring(&mut p, c, y, z, r * 0.42, 0.005, accent);
        }
        // An eye in a ring of rays, its heart its pupil
        "eye" => {
            for s in [-1.0, 1.0] {
                for k in 0..7 {
                    let t = k as f64 / 6.0 * 2.0 - 1.0;
                    p.push(ball([0.0, t * r * 1.1, c[2] + s * (1.0 - t * t) * r * 0.5], 0.008, metal));
                }
            }
            p.push(ellipsoid(c, [0.008, r * 0.95, r * 0.45], LINEN));
            for k in 0..8 {
                let a = k as f64 / 8.0 * TAU;
                p.push(limb([0.0, a.cos() * r * 1.2, c[2] + a.sin() * r * 0.8], [0.0, a.cos() * r * 1.55, c[2] + a.sin() * r * 1.15], 0.006, 0.002, accent));
            }
        }
        // A plain disc, rimmed
        _ => {
            ring(&mut p, c, y, z, r, 0.008, accent);
            p.push(ellipsoid(c, [0.01, r, r], metal));
        }
    }
    // What hangs from under it
    slot("binding");
    match binding.shape.as_deref().unwrap_or("ribbons") {
        "beads" => {
            for k in 0..9 {
                let a = k as f64 / 8.0 * PI;
                p.push(ball([0.012, a.cos() * 0.035, top - 0.01 - a.sin() * 0.07], 0.009, if k % 4 == 0 { trim } else { band }));
            }
        }
        "seals" => {
            for s in [-1.0, 1.0] {
                p.push(limb([0.004, s * 0.03, top + 0.01], [0.012, s * 0.04, top - 0.08], 0.008, 0.008, band).stretched([0.4, 1.0, 1.0]));
                p.push(ellipsoid([0.014, s * 0.041, top - 0.085], [0.006, 0.016, 0.016], trim));
            }
        }
        // Feathers hanging on cords, a bead at each one's quill
        "feathers" => {
            for k in 0..3 {
                let yk = (k as f64 - 1.0) * 0.028;
                let drop = 0.05 + 0.015 * (k % 2) as f64;
                p.push(limb([0.004, yk, top + 0.01], [0.006, yk * 1.2, top - drop * 0.5], 0.0025, 0.0025, LINEN));
                p.push(ball([0.006, yk * 1.2, top - drop * 0.5], 0.005, trim));
                p.push(limb([0.006, yk * 1.25, top - drop * 0.5], [0.01, yk * 1.4, top - drop - 0.04], 0.011, 0.004, band).stretched([0.3, 1.0, 1.0]));
            }
        }
        "bells" => {
            for s in [-1.0, 1.0] {
                p.push(limb([0.0, s * 0.03, top + 0.01], [0.004, s * 0.04, top - 0.05], 0.003, 0.003, DARK_STEEL));
                p.push(ball([0.004, s * 0.04, top - 0.065], 0.016, band));
            }
        }
        _ => {
            for s in [-1.0, 1.0] {
                p.push(limb([0.006, s * 0.028, top + 0.01], [0.016, s * 0.05, top - 0.1], 0.009, 0.007, band).stretched([0.35, 1.0, 1.0]));
                p.push(limb([0.016, s * 0.05, top - 0.1], [0.018, s * 0.055, top - 0.12], 0.007, 0.007, trim).stretched([0.35, 1.0, 1.0]));
            }
        }
    }
    // The grip it's held by: cord wound round it, or a chain
    slot("shaft");
    // Three cords braided round each other
    if shaft.shape.as_deref() == Some("braid") {
        for k in 0..9 {
            let zk = k as f64 * 0.014;
            for j in 0..3 {
                let a = j as f64 / 3.0 * TAU + k as f64 * 0.9;
                p.push(ball([a.cos() * 0.01, a.sin() * 0.01, zk], 0.008, if j == 0 { link } else { cord }));
            }
        }
    }
    if shaft.shape.as_deref() == Some("chain") {
        for k in 0..6 {
            let zk = 0.005 + k as f64 * 0.022;
            let round = if k % 2 == 0 { [0.016, 0.009, 0.012] } else { [0.009, 0.016, 0.012] };
            p.push(ellipsoid([0.0, 0.0, zk], round, link));
        }
    }
    p.push(ball([0.0, 0.0, -0.012], 0.018, link));
    p.push(limb([0.0, 0.0, -0.01], [0.0, 0.0, top + 0.01], 0.014, 0.016, cord));
    (p, v3(0.0, 0.0, 0.06), v3(0.02, 0.0, c[2]))
}
