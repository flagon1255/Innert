//! How the third batch's weapons look, see build_look(): a war pick's head on its haft, a chakram
//! held by its rim, a prism's great crystal held up in its cradle and a runestone's carved stone.
//! As everywhere in the builders, what's made first wins where shapes overlap, so the bright bits
//! go in before what they're set into.

use super::*;

/// A war pick's head across the top of a haft ending at `z_top`: a long beak along +x, curving
/// down to its point, and on the other side a squared hammer face, as `shape` has it: plain, a
/// crow's bill longer and hooked further down, a horseman's short and thick with a flanged face, or
/// a beak each way. Its fuller glows down the beak if it has one. Returns the beak's point.
pub(super) fn warpick_head(p: &mut Vec<Part>, v: &Visual, z_top: f64, infuse: Option<u8>, shape: &str) -> Vector3<f64> {
    let length = v.length.unwrap_or(0.24);
    let thick = v.width.unwrap_or(0.05);
    let body = mat(&v.material, STEEL);
    let edge = mat(&v.edge, body);
    let accent = mat(&v.accent, body);
    let glow = infuse.or_else(|| v.fuller.as_deref().and_then(material));
    let z = z_top - 0.035;
    // How far it reaches, how far down its point hooks, and how thick it is at the eye
    let (reach, droop, root) = match shape {
        "warpick_crow" => (length * 1.08, 0.085, thick * 0.42),
        "warpick_horseman" => (length * 0.78, 0.035, thick * 0.55),
        _ => (length, 0.06, thick * 0.48),
    };
    let beak = |s: f64| [[s * 0.022, 0.0, z], [s * reach * 0.42, 0.0, z + 0.012], [s * reach * 0.78, 0.0, z - droop * 0.35], [s * reach, 0.0, z - droop]];
    // A line of its glow along the top of the beak, inlaid
    if let Some(g) = glow {
        let points = beak(1.0);
        curve(p, &[[points[0][0] + 0.01, 0.0, z + root * 0.7], [points[1][0], 0.0, points[1][2] + root * 0.62], [points[2][0], 0.0, points[2][2] + root * 0.4]], root * 0.3, root * 0.2, g);
    }
    // Runes, engraving or veins pressed into its flats
    if matches!(v.pattern.as_deref(), Some("engraved" | "runes" | "veins")) {
        let mark = if v.pattern.as_deref() == Some("engraved") { accent } else { glow.unwrap_or(accent) };
        for k in 0..3 {
            let x = 0.05 + k as f64 * reach * 0.18;
            for s in [-1.0, 1.0] {
                p.push(cuboid([x, s * root * 0.95 - 0.002, z - root * 0.25], [x + reach * 0.08, s * root * 0.95 + 0.002, z + root * 0.25], mark));
            }
        }
    }
    curve(p, &beak(1.0), root, 0.004, edge);
    match shape {
        // A second beak the other way
        "warpick_double" => curve(p, &beak(-0.85), root, 0.004, edge),
        // A thick hammer face, flanged round
        "warpick_horseman" => {
            p.push(cuboid([-0.1, -thick * 0.62, z - thick * 0.62], [-0.03, thick * 0.62, z + thick * 0.62], body));
            for (y, zz) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                p.push(cuboid([-0.095, y * thick * 0.62 - 0.004, zz * thick * 0.62 + z - 0.004], [-0.04, y * thick * 0.62 + 0.004, zz * thick * 0.62 + z + 0.004], accent));
            }
            p.push(cuboid([-0.112, -thick * 0.5, z - thick * 0.5], [-0.1, thick * 0.5, z + thick * 0.5], edge));
        }
        // A squared hammer face, a little crowned
        _ => {
            p.push(limb([-0.02, 0.0, z], [-0.085, 0.0, z], thick * 0.48, thick * 0.55, body).stretched([1.0, 1.0, 1.0]));
            p.push(cuboid([-0.1, -thick * 0.45, z - thick * 0.45], [-0.085, thick * 0.45, z + thick * 0.45], edge));
        }
    }
    // The eye round the haft, a spike on top, and langets down the haft
    p.push(cuboid([-0.028, -0.024, z - 0.045], [0.028, 0.024, z + 0.03], accent));
    p.push(limb([0.0, 0.0, z + 0.03], [0.0, 0.0, z + 0.075], 0.012, 0.002, edge));
    for s in [-1.0, 1.0] {
        p.push(cuboid([s * 0.016 - 0.003, -0.012, z - 0.17], [s * 0.016 + 0.003, 0.012, z - 0.04], body));
    }
    v3(reach, 0.0, z - droop)
}

/// A chakram, a ring sharpened round its rim, standing in the x-z plane with its foot at the hand:
/// its rim of the edge's metal round a band of its body, spikes round it, a second ring inside
/// joined to it by spokes, or points like a star's, as its shape has it, the grip wrapped round
/// its foot and its core set at the top. Returns its parts, where it's held, and its middle, which
/// it spins round flying.
pub(super) fn chakram(blade: &Visual, hilt: &Visual, core: &Visual) -> (Vec<Part>, Vector3<f64>, Vector3<f64>) {
    let mut p = Vec::new();
    let radius = blade.length.unwrap_or(0.3) * 0.5;
    let band = blade.width.unwrap_or(0.035);
    let body = mat(&blade.material, STEEL);
    let edge = mat(&blade.edge, body);
    let accent = mat(&blade.accent, body);
    let shape = blade.shape.as_deref().unwrap_or("chakram");
    let middle = [0.0, 0.0, radius];
    let (u, w) = ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    // Its core set in the rim at the top, and over the grip at its foot
    slot("core");
    if let Some(stone) = core.gem.as_deref().and_then(material) {
        gem(&mut p, [0.0, 0.0, radius * 2.0 - band * 0.5], core.size.unwrap_or(0.02).max(0.014), stone, core.shape.as_deref());
    }
    slot("hilt");
    // The grip, wrapped round the rim at its foot where it's held
    let grip = mat(&hilt.material, LEATHER);
    let wrap = hilt.wrap.as_deref().and_then(material).unwrap_or(grip);
    for k in 0..6 {
        let a = -0.42 + k as f64 * 0.17;
        let at = [a.sin() * radius, 0.0, radius - a.cos() * (radius - band * 0.5)];
        p.push(ball(at, band * 0.7, if k % 2 == 0 { wrap } else { grip }));
    }
    slot("blade");
    // Glow in its fuller, a ring of it round the band
    if let Some(g) = blade.fuller.as_deref().and_then(material) {
        ring(&mut p, middle, u, w, radius - band * 0.55, band * 0.22, g);
    }
    match shape {
        "chakram_spiked" => {
            for k in 0..10 {
                let a = (k as f64 + 0.5) / 10.0 * TAU;
                let (c, s) = (a.sin(), -a.cos());
                if s < -0.85 {
                    continue;
                }
                p.push(limb([c * radius, 0.0, radius + s * radius], [c * (radius + 0.04), 0.0, radius + s * (radius + 0.04)], 0.009, 0.002, edge));
            }
        }
        "chakram_star" => {
            for k in 0..8 {
                let a = k as f64 / 8.0 * TAU;
                let (c, s) = (a.sin(), -a.cos());
                if s < -0.9 {
                    continue;
                }
                let (left, right) = ((a - 0.18).sin(), (a + 0.18).sin());
                let (left_z, right_z) = (-(a - 0.18).cos(), -(a + 0.18).cos());
                p.push(limb([left * radius, 0.0, radius + left_z * radius], [c * (radius + 0.06), 0.0, radius + s * (radius + 0.06)], 0.007, 0.002, edge));
                p.push(limb([right * radius, 0.0, radius + right_z * radius], [c * (radius + 0.06), 0.0, radius + s * (radius + 0.06)], 0.007, 0.002, edge));
            }
        }
        // Sixteen teeth all round, raked back the way it spins, as a saw's
        "chakram_saw" => {
            for k in 0..16 {
                let a = (k as f64 + 0.5) / 16.0 * TAU;
                let b = a + 0.16;
                let (s, c) = (a.sin(), -a.cos());
                if c < -0.88 {
                    continue;
                }
                p.push(limb([s * radius, 0.0, radius + c * radius], [b.sin() * (radius + 0.03), 0.0, radius - b.cos() * (radius + 0.03)], 0.008, 0.002, edge));
            }
        }
        // Four hooked blades curving out off the rim the same way round, a pinwheel of them
        "chakram_hooked" => {
            for k in 0..4 {
                let a = k as f64 / 4.0 * TAU + TAU / 8.0;
                let at = |r: f64, t: f64| [t.sin() * r, 0.0, radius - t.cos() * r];
                curve(&mut p, &[at(radius, a), at(radius + 0.035, a + 0.2), at(radius + 0.05, a + 0.45), at(radius + 0.035, a + 0.7)], 0.011, 0.002, edge);
            }
        }
        // A cross of spokes inside the ring, a boss where they meet
        "chakram_crossed" => {
            for k in 0..4 {
                let a = k as f64 / 4.0 * TAU + TAU / 8.0;
                p.push(limb(middle, [a.sin() * (radius - band), 0.0, radius - a.cos() * (radius - band)], 0.008, 0.006, accent).stretched([1.0, 0.6, 1.0]));
            }
            p.push(ball(middle, band * 0.5, accent));
        }
        "chakram_twin" => {
            ring(&mut p, middle, u, w, radius * 0.55, band * 0.28, accent);
            for k in 0..6 {
                let a = k as f64 / 6.0 * TAU + 0.26;
                p.push(limb([a.cos() * radius * 0.55, 0.0, radius + a.sin() * radius * 0.55], [a.cos() * (radius - band), 0.0, radius + a.sin() * (radius - band)], 0.006, 0.006, accent));
            }
        }
        _ => {}
    }
    // Engraving round its band
    if v_pattern(blade) {
        ring(&mut p, middle, u, w, radius - band * 0.35, band * 0.16, accent);
    }
    // Its sharpened rim, then the flat band inside it, thin across
    let thin = |q: Part| q.stretched([1.0, 0.35, 1.0]);
    let n = 40;
    for k in 0..n {
        let (a, b) = (k as f64 / n as f64 * TAU, (k + 1) as f64 / n as f64 * TAU);
        let at = |r: f64, t: f64| [t.sin() * r, 0.0, radius - t.cos() * r];
        p.push(thin(limb(at(radius, a), at(radius, b), band * 0.3, band * 0.3, edge)));
        p.push(limb(at(radius - band * 0.55, a), at(radius - band * 0.55, b), band * 0.4, band * 0.4, body).stretched([1.0, 0.45, 1.0]));
    }
    (p, v3(0.0, 0.0, band * 0.4), Vector3::from(middle))
}

fn v_pattern(v: &Visual) -> bool {
    matches!(v.pattern.as_deref(), Some("engraved" | "runes"))
}

/// A prism: a great crystal held up in a cradle of prongs on a short grip, its glow burning in its
/// heart, as its focus's shape has it: one long point, two leaning apart, a cluster round a big
/// one, or one ringed round an eye. Little stones of its crystal set round the cradle, its binding
/// wound round the grip. Returns its parts, where it's held, and the crystal's point.
pub(super) fn prism(shaft: &Visual, focus: &Visual, crystal: &Visual, binding: &Visual) -> (Vec<Part>, Vector3<f64>, Vector3<f64>) {
    let mut p = Vec::new();
    let wood = mat(&shaft.material, WOOD);
    let glass = mat(&focus.material, CRYSTAL);
    let cradle = mat(&focus.accent, GOLD);
    let heart = focus.gem.as_deref().and_then(material);
    let size = focus.size.unwrap_or(0.09);
    let top = 0.16;
    let base = [0.0, 0.0, top + 0.03];
    let shape = focus.shape.as_deref().unwrap_or("prism_point");
    // A long six-sided crystal from `foot` to `point`, `r` round, its heart glowing inside
    let crystal_at = |p: &mut Vec<Part>, foot: [f64; 3], point: [f64; 3], r: f64| {
        let mid = [foot[0] + (point[0] - foot[0]) * 0.72, foot[1] + (point[1] - foot[1]) * 0.72, foot[2] + (point[2] - foot[2]) * 0.72];
        if let Some(h) = heart {
            p.push(limb(foot, mid, r * 0.4, r * 0.3, h));
        }
        p.push(limb(foot, mid, r * 0.8, r, glass));
        p.push(limb(mid, point, r, 0.003, glass));
    };
    slot("focus");
    let tip = match shape {
        "prism_twin" => {
            for s in [-1.0, 1.0] {
                crystal_at(&mut p, base, [s * size * 0.55, 0.0, base[2] + size * 1.8], size * 0.3);
            }
            v3(0.0, 0.0, base[2] + size * 1.6)
        }
        "prism_cluster" => {
            for k in 0..5 {
                let a = k as f64 / 5.0 * TAU;
                crystal_at(&mut p, [a.cos() * 0.016, a.sin() * 0.016, base[2]], [a.cos() * size * 0.75, a.sin() * size * 0.75, base[2] + size * 1.1], size * 0.18);
            }
            crystal_at(&mut p, base, [0.0, 0.0, base[2] + size * 2.1], size * 0.34);
            v3(0.0, 0.0, base[2] + size * 2.1)
        }
        "prism_eye" => {
            crystal_at(&mut p, base, [0.0, 0.0, base[2] + size * 2.0], size * 0.36);
            ring(&mut p, [0.0, 0.0, base[2] + size * 0.9], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], size * 0.62, 0.006, cradle);
            p.insert(0, ball([0.0, 0.0, base[2] + size * 0.9], size * 0.16, heart.unwrap_or(GLOW_HOLY)));
            v3(0.0, 0.0, base[2] + size * 2.0)
        }
        _ => {
            crystal_at(&mut p, base, [0.0, 0.0, base[2] + size * 2.2], size * 0.36);
            v3(0.0, 0.0, base[2] + size * 2.2)
        }
    };
    // The cradle's prongs, curling up round its foot
    for k in 0..4 {
        let a = k as f64 / 4.0 * TAU + 0.4;
        curve(&mut p, &[[a.cos() * 0.012, a.sin() * 0.012, top], [a.cos() * size * 0.42, a.sin() * size * 0.42, top + 0.035], [a.cos() * size * 0.36, a.sin() * size * 0.36, top + 0.075]], 0.006, 0.003, cradle);
    }
    p.push(limb([0.0, 0.0, top - 0.01], [0.0, 0.0, top + 0.03], 0.02, 0.026, cradle));
    // Little stones of its crystal round the cradle
    slot("crystal");
    let stone = crystal.gem.as_deref().and_then(material).unwrap_or(CRYSTAL);
    for k in 0..3 {
        let a = k as f64 / 3.0 * TAU + 1.2;
        p.insert(0, ball([a.cos() * 0.022, a.sin() * 0.022, top + 0.012], 0.008, stone));
    }
    slot("binding");
    let band = mat(&binding.material, LINEN);
    let trim = mat(&binding.accent, band);
    for k in 0..4 {
        let z = 0.03 + k as f64 * 0.028;
        p.push(limb([0.0, 0.0, z], [0.0, 0.0, z + 0.012], 0.017, 0.017, if k % 2 == 0 { band } else { trim }));
    }
    slot("shaft");
    p.push(ball([0.0, 0.0, -0.012], 0.018, cradle));
    p.push(limb([0.0, 0.0, -0.005], [0.0, 0.0, top], 0.014, 0.017, wood));
    (p, v3(0.0, 0.0, 0.07), tip)
}

/// A runestone: a stone held up in the fist by a strap-wound grip under it, carved with runes that
/// glow on both its faces, as its focus's shape has it: a slab, a round disc, a ring with a hole
/// through it, or a tall shard of an obelisk. Its crystal is set at its top, its binding wound round
/// it. Returns its parts, where it's held, and the middle of its face.
pub(super) fn runestone(shaft: &Visual, focus: &Visual, crystal: &Visual, binding: &Visual) -> (Vec<Part>, Vector3<f64>, Vector3<f64>) {
    let mut p = Vec::new();
    let grip = mat(&shaft.material, LEATHER);
    let stone = mat(&focus.material, ROCK);
    let accent = mat(&focus.accent, stone);
    let rune = mat(&focus.gem, GLOW_ARCANE);
    let size = focus.size.unwrap_or(0.12);
    let top = 0.12;
    let shape = focus.shape.as_deref().unwrap_or("rune_slab");
    let (half_w, half_h, half_t) = match shape {
        "rune_disc" | "rune_ring" => (size * 0.5, size * 0.5, 0.018),
        "rune_obelisk" => (size * 0.28, size * 0.75, 0.024),
        _ => (size * 0.38, size * 0.55, 0.022),
    };
    let c = [0.0, 0.0, top + half_h + 0.01];
    slot("crystal");
    let gem_material = crystal.gem.as_deref().and_then(material).unwrap_or(rune);
    p.push(ball([0.0, 0.0, c[2] + half_h * 0.85], 0.011, gem_material));
    slot("focus");
    // Runes cut into both faces, glowing: a stroke down the middle, strokes branching off it, and
    // marks round it, so it reads as writing from either side
    for s in [-1.0, 1.0] {
        let y = s * half_t;
        let face = |x: f64, z: f64| [x, y, c[2] + z];
        let stroke = |p: &mut Vec<Part>, a: [f64; 3], b: [f64; 3]| p.push(limb(a, b, 0.0045, 0.0045, rune).stretched([1.0, 0.6, 1.0]));
        stroke(&mut p, face(0.0, -half_h * 0.6), face(0.0, half_h * 0.6));
        for k in 0..3 {
            let z = -half_h * 0.45 + k as f64 * half_h * 0.42;
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            stroke(&mut p, face(0.0, z), face(side * half_w * 0.5, z + half_h * 0.18));
        }
        for k in 0..4 {
            let a = (k as f64 + 0.5) / 4.0 * TAU;
            p.push(ball(face(a.cos() * half_w * 0.72, a.sin() * half_h * 0.72), 0.006, rune));
        }
    }
    match shape {
        "rune_disc" => p.push(ellipsoid(c, [half_w, half_t, half_h], stone)),
        "rune_ring" => {
            p.push(limb([0.0, -half_t * 2.0, c[2]], [0.0, half_t * 2.0, c[2]], half_w * 0.28, half_w * 0.28, AIR));
            p.push(ellipsoid(c, [half_w, half_t, half_h], stone));
            ring(&mut p, c, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], half_w * 0.98, 0.006, accent);
        }
        "rune_obelisk" => {
            p.push(limb([0.0, 0.0, c[2] - half_h], [0.0, 0.0, c[2] + half_h * 0.7], half_w, half_w * 0.75, stone).stretched([1.0, half_t / half_w, 1.0]));
            p.push(limb([0.0, 0.0, c[2] + half_h * 0.7], [0.0, 0.0, c[2] + half_h * 1.05], half_w * 0.75, 0.004, accent).stretched([1.0, half_t / half_w, 1.0]));
        }
        _ => {
            p.push(cuboid([-half_w, -half_t, c[2] - half_h], [half_w, half_t, c[2] + half_h * 0.8], stone));
            p.push(ellipsoid([0.0, 0.0, c[2] + half_h * 0.8], [half_w, half_t, half_h * 0.22], stone));
        }
    }
    // Its binding, wound round the stone's foot and hanging off it
    slot("binding");
    let band = mat(&binding.material, LINEN);
    let trim = mat(&binding.accent, band);
    p.insert(0, cuboid([-half_w * 1.02, -half_t * 1.15, c[2] - half_h * 0.88], [half_w * 1.02, half_t * 1.15, c[2] - half_h * 0.74], band));
    p.push(limb([half_w * 0.6, 0.0, c[2] - half_h * 0.85], [half_w * 0.85, 0.01, c[2] - half_h * 1.6], 0.006, 0.004, trim));
    slot("shaft");
    p.push(ball([0.0, 0.0, -0.01], 0.019, accent));
    p.push(limb([0.0, 0.0, -0.005], [0.0, 0.0, c[2] - half_h * 0.9], 0.015, 0.018, grip));
    (p, v3(0.0, 0.0, 0.07), Vector3::from(c))
}
