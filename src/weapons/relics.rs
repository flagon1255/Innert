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
    p.push(ellipsoid([0.024, 0.0, c[2] - 0.048], [0.034, 0.036, 0.018], bone));
    p.push(limb([0.046, -0.03, c[2] + 0.024], [0.046, 0.03, c[2] + 0.024], 0.01, 0.01, bone));
    p.push(ellipsoid([0.0, 0.0, c[2]], [0.062, 0.052, 0.058], bone));
    match focus.shape.as_deref().unwrap_or("skull") {
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
