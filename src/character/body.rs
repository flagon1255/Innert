//! The player's own body, made of jointed pieces as the bosses are, see src\rig.rs: a head with
//! its hair and face, a torso, upper arms, forearms with the hands, and legs. Its pieces are made
//! afresh for how the player looks, see models(), and drawn where the player stands, walking as
//! they walk, looking where they look, see instances().
//!
//! The eye never sees it, as it'd be right round the camera: the shader passes it by for rays from
//! the camera, see BODY in shaders\traverse.comp, so it's only there in the shadow the player casts
//! and in water and what shines. Its pieces are drawn in two colours each, chosen by the player:
//! what's leather on a piece in one, what's cloth in the other, see AnimalInstance::outfit(). So
//! the head's leather is its skin and its cloth its hair, an arm's leather its glove and its cloth
//! its sleeve. What the player's painted on it, voxel by voxel, goes over that, see paint.rs.
//!
//! Its pieces are written to assets\models\player\ for the editor, in the look a new character has,
//! see export(); edited there, they're read back for every look, see assets::player().
//!
//! In the window a new character's made in, it's drawn on the processor turning round, see
//! picture().

use std::f64::consts::{FRAC_PI_2, TAU};
use std::sync::OnceLock;

use nalgebra::{UnitQuaternion, Vector3};

use super::Looks;
use crate::animals::{self, AnimalInstance, Model, Part, ball, cuboid, ellipsoid, limb};
use crate::worldgen::material::*;

type V = Vector3<f64>;
type Q = UnitQuaternion<f64>;

/// Voxels per meter of its pieces: twice the world's, so a face shows
pub const BODY_RES: f64 = 32.0;

/// Its pieces, in the order models() makes them
pub const PIECES: usize = 8;
pub const HEAD: usize = 0;
pub const TORSO: usize = 1;
pub const UPPER_ARM: [usize; 2] = [2, 3];
pub const FOREARM: [usize; 2] = [4, 5];
pub const LEG: [usize; 2] = [6, 7];
/// What each piece's file is called, see assets::player()
pub const PIECE_NAMES: [&str; PIECES] = ["head", "torso", "upper_arm_right", "upper_arm_left", "forearm_right", "forearm_left", "leg_right", "leg_left"];

/// Where the joints are, in meters from the middle of the feet, +x ahead and +y to the left, for
/// an average build: the hips, the small of the back, the neck and the shoulders, and how long an
/// upper arm is
const HIP: [f64; 3] = [0.0, 0.1, 0.92];
const WAIST: [f64; 3] = [0.0, 0.0, 0.92];
const NECK: [f64; 3] = [0.0, 0.0, 1.45];
const SHOULDER: [f64; 3] = [0.0, 0.2, 1.39];
const UPPER: f64 = 0.28;
/// How far a step takes the player, in meters, for how fast their legs go
const STRIDE: f64 = 1.5;

/// A belt and boots are leather of their own, not the gloves'
pub const BELT: [u8; 3] = [88, 56, 34];
pub const BOOTS: [u8; 3] = [70, 46, 30];

/// How much wider than average each build is
fn width(looks: &Looks) -> f64 {
    [0.86, 1.0, 1.16][(looks.build as usize).min(2)]
}

fn rx(a: f64) -> Q {
    Q::from_axis_angle(&V::x_axis(), a)
}

/// Turned forward by `a`, its nose going down, a limb hanging down swinging back
fn ry(a: f64) -> Q {
    Q::from_axis_angle(&V::y_axis(), a)
}

fn rz(a: f64) -> Q {
    Q::from_axis_angle(&V::z_axis(), a)
}

/// The shapes of piece `p` for `looks`, about its joint, hanging straight down
pub fn shapes(p: usize, looks: &Looks) -> Vec<Part> {
    let w = width(looks);
    let mut s = Vec::new();
    match p {
        HEAD => {
            // What shows on the face first, as what comes first wins where shapes overlap
            let (brow, lips) = match looks.face {
                1 => (0.012, DARK_STEEL),
                _ => (0.008, MEAT),
            };
            for y in [-0.038, 0.038] {
                s.push(ball([0.088, y, 0.165], 0.016, EYE));
                let tilt = if looks.face == 1 { 0.012 } else { 0.0 };
                s.push(limb([0.1, y * 0.5, 0.193 - tilt], [0.095, y * 1.55, 0.198], brow, brow, CLOTH));
            }
            s.push(ball([0.108, 0.0, 0.142], 0.017, LEATHER));
            s.push(cuboid([0.096, -0.022, 0.1], [0.104, 0.022, 0.104], lips));
            match looks.face {
                // A beard along the jaw, in the hair's colour, the mouth showing through
                2 => s.push(ellipsoid([0.045, 0.0, 0.085], [0.075, 0.09, 0.06], CLOTH)),
                // A scar down the right cheek
                3 => {
                    for k in 0..3 {
                        let f = k as f64 / 2.0;
                        s.push(cuboid([0.094, -0.05 - 0.016 * f, 0.18 - 0.045 * f], [0.102, -0.046 - 0.016 * f, 0.186 - 0.045 * f], MEAT));
                    }
                }
                _ => {}
            }
            // The hair before the head, so it's hair where they overlap: a cap over the top and
            // back of the head, high enough in front to leave the brow, the eyes and the ears
            let cap = ellipsoid([-0.03, 0.0, 0.215], [0.118, 0.11, 0.092], CLOTH);
            match looks.hair {
                // Long, down the back to the shoulders
                1 => {
                    s.push(cap);
                    s.push(ellipsoid([-0.07, 0.0, 0.08], [0.06, 0.1, 0.15], CLOTH));
                }
                // Gathered up in a knot on top
                2 => {
                    s.push(cap);
                    s.push(ball([-0.035, 0.0, 0.3], 0.042, CLOTH));
                    s.push(limb([-0.03, 0.0, 0.26], [-0.035, 0.0, 0.3], 0.025, 0.025, LEATHER));
                }
                // Shaved: a shadow of it close to the scalp, on top
                3 => s.push(ellipsoid([-0.04, 0.0, 0.25], [0.09, 0.09, 0.05], CLOTH)),
                // A braid hanging down behind
                4 => {
                    s.push(cap);
                    for k in 0..5 {
                        let z = 0.1 - k as f64 * 0.055;
                        s.push(ball([-0.11 - k as f64 * 0.006, 0.0, z], 0.026 - k as f64 * 0.002, CLOTH));
                    }
                }
                _ => s.push(cap),
            }
            // Ears, the neck and the head itself, in the skin's colour
            for y in [-0.098, 0.098] {
                s.push(ellipsoid([0.0, y, 0.15], [0.02, 0.014, 0.03], LEATHER));
            }
            s.push(ellipsoid([0.0, 0.0, 0.15], [0.105, 0.095, 0.12], LEATHER));
            s.push(limb([0.0, 0.0, -0.05], [0.0, 0.0, 0.06], 0.046, 0.046, LEATHER));
        }
        TORSO => {
            // A belt, a tunic over the chest and shoulders, its skirt over the hips
            s.push(ellipsoid([0.0, 0.0, 0.07], [0.135 * w, 0.175 * w, 0.035], LEATHER));
            s.push(cuboid([0.12 * w, -0.025, 0.04], [0.15 * w, 0.025, 0.1], GOLD));
            s.push(ellipsoid([0.0, 0.0, 0.28], [0.125 * w, 0.19 * w, 0.27], CLOTH));
            for y in [-0.17, 0.17] {
                s.push(ellipsoid([0.0, y * w, 0.45], [0.075, 0.07 * w, 0.065], CLOTH));
            }
            s.push(limb([0.0, 0.0, -0.13], [0.0, 0.0, 0.06], 0.165 * w, 0.145 * w, CLOTH));
            s.push(limb([0.0, 0.0, 0.5], [0.0, 0.0, 0.55], 0.06, 0.05, CLOTH));
        }
        _ if UPPER_ARM.contains(&p) => {
            s.push(ellipsoid([0.0, 0.0, -0.02], [0.06 * w, 0.06 * w, 0.06], CLOTH));
            s.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, -UPPER], 0.052 * w, 0.044 * w, CLOTH));
        }
        _ if FOREARM.contains(&p) => {
            // A cuff of the glove's leather, then the hand
            s.push(limb([0.0, 0.0, -0.17], [0.0, 0.0, -0.21], 0.043 * w, 0.044 * w, LEATHER));
            s.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, -0.2], 0.044 * w, 0.038 * w, CLOTH));
            s.push(ellipsoid([0.012, 0.0, -0.265], [0.04, 0.03, 0.055], LEATHER));
        }
        _ if LEG.contains(&p) => {
            // Boots, the foot reaching forward, then the trousers
            s.push(ellipsoid([0.05, 0.0, -0.885], [0.11, 0.052, 0.04], LEATHER));
            s.push(limb([0.0, 0.0, -0.68], [0.0, 0.0, -0.87], 0.056 * w, 0.054, LEATHER));
            s.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, -0.45], 0.078 * w, 0.062 * w, CLOTH));
            s.push(limb([0.0, 0.0, -0.45], [0.0, 0.0, -0.7], 0.06 * w, 0.05 * w, CLOTH));
        }
        _ => {}
    }
    s
}

/// The model of piece `p` for `looks`, and where it puts the middle of its bottom from its joint,
/// in meters
pub fn piece(p: usize, looks: &Looks) -> (Model, V) {
    animals::voxelize(&shapes(p, looks), BODY_RES)
}

/// Every piece's model for `looks`, with the files read in where they've been edited, see
/// assets::player()
pub fn models(looks: &Looks) -> Vec<(Model, V)> {
    let made: Vec<(Model, V)> = (0..PIECES).map(|p| piece(p, looks)).collect();
    crate::assets::player(made, looks)
}

/// The colours each piece is drawn in, packed as AnimalInstance::outfit() takes them: what's
/// leather on it, and what's cloth
pub fn colours(looks: &Looks) -> [u32; PIECES] {
    use super::outfit;
    std::array::from_fn(|p| match p {
        HEAD => outfit(looks.skin_rgb(), looks.hair_rgb()),
        TORSO => outfit(BELT, looks.tunic_rgb()),
        _ if LEG.contains(&p) => outfit(BOOTS, looks.trousers_rgb()),
        _ => outfit(looks.gloves_rgb(), looks.sleeves_rgb()),
    })
}

/// What the player's doing, for how the body's posed
#[derive(Clone, Copy, Debug, Default)]
pub struct Stance {
    /// Which way they face and look, in radians: round as the camera's yaw, and up
    pub yaw: f64,
    pub pitch: f64,
    /// How far they've walked in all, in meters, and how fast they're going, in meters a second
    pub walked: f64,
    pub speed: f64,
    /// Whether something's held in the right hand, raised before them
    pub holding: bool,
    /// Seconds since the game started, for breathing
    pub time: f64,
}

/// Where each piece goes for `stance`, in meters from the middle of the feet, +x the way they face:
/// each piece's joint, and how it's turned
pub fn pose(looks: &Looks, stance: &Stance) -> [(V, Q); PIECES] {
    let w = width(looks);
    let walking = (stance.speed / 3.0).clamp(0.0, 1.0);
    let running = ((stance.speed - 4.0) / 3.0).clamp(0.0, 1.0);
    let phase = stance.walked / STRIDE * TAU;
    let swing = (0.42 + 0.25 * running) * walking * phase.sin();
    let bob = 0.025 * walking * (2.0 * phase).cos().abs() - 0.02 * walking;
    let breath = 0.006 * (stance.time * 1.7).sin();
    let mut out = [(V::zeros(), Q::identity()); PIECES];
    let waist = V::from(WAIST) + V::new(0.0, 0.0, bob);
    let lean = ry(0.06 * running + 0.02 * walking);
    out[TORSO] = (waist, lean);
    let neck = waist + lean * (V::from(NECK) - V::from(WAIST)) + V::new(0.0, 0.0, breath);
    out[HEAD] = (neck, ry(-stance.pitch.clamp(-1.2, 1.2) * 0.7));
    for (k, side) in [-1.0, 1.0].into_iter().enumerate() {
        // The legs swinging opposite each other, the arms opposite the legs
        let hip = waist + V::new(HIP[0], side * HIP[1] * w, HIP[2] - WAIST[2]);
        let leg = if k == 0 { -swing } else { swing };
        out[LEG[k]] = (hip, ry(leg));
        let shoulder = waist + lean * V::new(SHOULDER[0], side * SHOULDER[1] * w, SHOULDER[2] - WAIST[2] + breath);
        let out_a = rx(side * 0.1);
        let (upper, bend) = if k == 0 && stance.holding {
            // Raised before them, as what's held in first person is
            (ry(-0.75 - 0.25 * stance.pitch.clamp(-0.8, 0.8)) * rx(0.25), 1.05)
        } else {
            (out_a * ry(-leg * 0.85), 0.25 + 0.35 * walking + 0.4 * running)
        };
        out[UPPER_ARM[k]] = (shoulder, lean * upper);
        let elbow = shoulder + lean * upper * V::new(0.0, 0.0, -UPPER);
        out[FOREARM[k]] = (elbow, lean * upper * ry(-bend));
    }
    out
}

/// The turn from the body's own frame, +x ahead, to the world's, facing as the camera's `yaw`
pub fn facing(yaw: f64) -> Q {
    rz(yaw + FRAC_PI_2)
}

/// What to draw of the body standing with the middle of its feet at `feet`, in voxels, its pieces'
/// models from `first_model`, each `models` big with its anchor, see models(), drawn as `how`
/// says: animals::BODY in play, or MOVING seen by a camera following behind
pub fn instances(models: &[(Model, V)], colours: &[u32; PIECES], first_model: u32, looks: &Looks, stance: &Stance, feet: V, vpm: f64, how: u32) -> Vec<AnimalInstance> {
    let turn = facing(stance.yaw);
    let posed = pose(looks, stance);
    let mut out = Vec::with_capacity(PIECES);
    for (p, &(joint, piece_turn)) in posed.iter().enumerate() {
        let Some((model, anchor)) = models.get(p) else { continue };
        if model.size == [1, 1, 1] {
            continue;
        }
        let [x, y, z] = model.size.map(|v| v as f64 / BODY_RES);
        let reach = (x * x * 0.25 + y * y * 0.25 + z * z).sqrt() * vpm;
        let whole = turn * piece_turn;
        let at = feet + turn * joint * vpm + whole * anchor * vpm;
        out.push(AnimalInstance::new(first_model + p as u32, at, whole, vpm / BODY_RES, reach, how).outfit(colours[p]));
    }
    out
}

/// The colours of every material, read out of the shader, see PALETTE in shaders\traverse.comp:
/// as they look, 0 to 1
pub(super) fn palette() -> &'static [[f64; 3]] {
    static PALETTE: OnceLock<Vec<[f64; 3]>> = OnceLock::new();
    PALETTE.get_or_init(|| {
        let text = include_str!("../../shaders/traverse.comp");
        let Some(start) = text.find("const vec3 PALETTE") else { return Vec::new() };
        let body = &text[start..];
        let (Some(open), Some(close)) = (body.find('('), body.find(");")) else { return Vec::new() };
        body[open + 1..close]
            .split("vec3(")
            .skip(1)
            .filter_map(|chunk| {
                let inside = &chunk[..chunk.find(')')?];
                let v: Vec<f64> = inside.split(',').filter_map(|n| n.trim().parse().ok()).collect();
                (v.len() == 3).then(|| [v[0], v[1], v[2]])
            })
            .collect()
    })
}

/// A picture of the body made of `models`, `size` pixels across and 1.4 times as tall, turned
/// `turn` radians round from facing the eye, standing on a disc of ground with its shadow under it:
/// drawn on the processor for the window a new character's made in. RGBA, a row at a time.
pub fn picture(models: &[(Model, V)], looks: &Looks, turn: f64, size: usize) -> (Vec<u8>, [usize; 2]) {
    let (wide, high) = (size, size * 7 / 5);
    let mut color = vec![[0.0f64; 3]; wide * high];
    let mut depth = vec![f64::MAX; wide * high];
    // A dark backdrop, lighter round where the body stands
    for y in 0..high {
        for x in 0..wide {
            let (dx, dy) = (x as f64 / wide as f64 - 0.5, y as f64 / high as f64 - 0.45);
            let glow = (1.0 - (dx * dx * 2.0 + dy * dy).sqrt() * 1.6).max(0.0);
            color[x + y * wide] = [0.03 + 0.06 * glow, 0.035 + 0.06 * glow, 0.05 + 0.07 * glow];
        }
    }
    let stance = Stance { yaw: 0.0, pitch: 0.0, walked: 0.0, speed: 0.0, holding: false, time: 1.0 };
    let posed = pose(looks, &stance);
    // Seen from in front and a little above, the body turned round before the eye
    let pitch = 0.12f64;
    let spin = rz(turn);
    let forward = V::new(-pitch.cos(), 0.0, -pitch.sin());
    let right = V::new(0.0, 1.0, 0.0);
    let up = right.cross(&forward);
    let scale = high as f64 / 2.15;
    let middle = V::new(0.0, 0.0, 0.95);
    let project = |w: V| -> (f64, f64, f64) {
        let r = w - middle;
        (wide as f64 * 0.5 + r.dot(&right) * scale, high as f64 * 0.5 - r.dot(&up) * scale, r.dot(&forward))
    };
    // The ground: a disc under it
    let ground = [0.16, 0.2, 0.12];
    for y in 0..high {
        for x in 0..wide {
            let sx = (x as f64 - wide as f64 * 0.5) / scale;
            let sy = -(y as f64 - high as f64 * 0.5) / scale;
            let o = middle + right * sx + up * sy;
            let t = -o.z / forward.z;
            let g = o + forward * t;
            if t > 0.0 && g.xy().norm() < 0.62 {
                let edge = ((0.62 - g.xy().norm()) / 0.08).min(1.0);
                let c = &mut color[x + y * wide];
                *c = [0, 1, 2].map(|i| c[i] + (ground[i] - c[i]) * edge);
                depth[x + y * wide] = t;
            }
        }
    }
    let palette = palette();
    let paint = |rgb: [u8; 3]| rgb.map(|c| (c as f64 / 255.0).powf(2.2));
    let tints = |p: usize| -> ([f64; 3], [f64; 3]) {
        match p {
            HEAD => (paint(looks.skin_rgb()), paint(looks.hair_rgb())),
            TORSO => (paint(BELT), paint(looks.tunic_rgb())),
            _ if LEG.contains(&p) => (paint(BOOTS), paint(looks.trousers_rgb())),
            _ => (paint(looks.gloves_rgb()), paint(looks.sleeves_rgb())),
        }
    };
    let light = V::new(0.45, -0.55, 0.75).normalize();
    let splat = (scale / BODY_RES * 1.3).ceil().max(1.0) as i64;
    let mut points: Vec<(V, V, [f64; 3])> = Vec::new();
    for (p, &(joint, piece_turn)) in posed.iter().enumerate() {
        let Some((model, anchor)) = models.get(p) else { continue };
        let [sx, sy, sz] = model.size.map(|v| v as i64);
        let at = |x: i64, y: i64, z: i64| -> u8 {
            if x < 0 || y < 0 || z < 0 || x >= sx || y >= sy || z >= sz { AIR } else { model.voxels[(x + (y + z * sy) * sx) as usize] }
        };
        let (leather, cloth) = tints(p);
        for z in 0..sz {
            for y in 0..sy {
                for x in 0..sx {
                    let m = at(x, y, z);
                    if m == AIR {
                        continue;
                    }
                    let mut normal = V::zeros();
                    for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                        if at(x + dx, y + dy, z + dz) == AIR {
                            normal += V::new(dx as f64, dy as f64, dz as f64);
                        }
                    }
                    if normal == V::zeros() {
                        continue;
                    }
                    let local = V::new((x as f64 + 0.5) / BODY_RES - sx as f64 * 0.5 / BODY_RES, (y as f64 + 0.5) / BODY_RES - sy as f64 * 0.5 / BODY_RES, (z as f64 + 0.5) / BODY_RES) + anchor;
                    let world = spin * (joint + piece_turn * local);
                    let base = match m {
                        LEATHER => leather,
                        CLOTH => cloth,
                        m if m >= super::paint::PAINT_FIRST => paint(super::paint::material_rgb(m, None, looks)),
                        m => palette.get(m as usize).copied().unwrap_or([1.0, 0.0, 1.0]).map(|v| v.powf(2.2)),
                    };
                    points.push((world, spin * piece_turn * normal.normalize(), base));
                }
            }
        }
    }
    // Its shadow on the ground first
    let mut shadow = vec![false; wide * high];
    for &(w, _, _) in &points {
        let (px, py, _) = project(V::new(w.x - light.x * w.z / light.z, w.y - light.y * w.z / light.z, 0.0));
        for oy in 0..splat {
            for ox in 0..splat {
                let (x, y) = (px as i64 + ox - splat / 2, py as i64 + oy - splat / 2);
                if x >= 0 && y >= 0 && (x as usize) < wide && (y as usize) < high {
                    shadow[x as usize + y as usize * wide] = true;
                }
            }
        }
    }
    for (c, s) in color.iter_mut().zip(&shadow) {
        if *s {
            *c = c.map(|v| v * 0.5);
        }
    }
    for &(w, n, base) in &points {
        let (px, py, d) = project(w);
        let shade = 0.32 + 0.8 * n.dot(&light).max(0.0) + 0.15 * n.z.max(0.0) + 0.12 * (-n.dot(&forward)).max(0.0);
        let c = base.map(|v| v * shade);
        for oy in 0..splat {
            for ox in 0..splat {
                let (x, y) = (px as i64 + ox - splat / 2, py as i64 + oy - splat / 2);
                if x >= 0 && y >= 0 && (x as usize) < wide && (y as usize) < high {
                    let i = x as usize + y as usize * wide;
                    if d < depth[i] {
                        depth[i] = d;
                        color[i] = c;
                    }
                }
            }
        }
    }
    let to = |v: f64| (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0) as u8;
    let rgba = color.iter().flat_map(|c| [to(c[0]), to(c[1]), to(c[2]), 255]).collect();
    (rgba, [wide, high])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every piece of every look is there and small enough for the shader, and posed walking or
    /// standing, it all stays within a couple of meters of the feet, the head at the height of the
    /// eye, so its pieces' reaches cover it
    #[test]
    fn the_body_is_whole_for_every_look() {
        for build in 0..3 {
            for hair in 0..super::super::HAIRS.len() as u8 {
                let looks = Looks { build, hair, face: hair % 4, ..Default::default() };
                let made: Vec<(Model, V)> = (0..PIECES).map(|p| piece(p, &looks)).collect();
                for (p, (m, _)) in made.iter().enumerate() {
                    assert!(m.size != [1, 1, 1] && m.size.iter().all(|&s| s < 255), "{}: {:?}", PIECE_NAMES[p], m.size);
                }
                for speed in [0.0, 2.0, 6.0] {
                    let stance = Stance { speed, walked: 0.4, holding: speed > 1.0, ..Default::default() };
                    let posed = pose(&looks, &stance);
                    let eye = posed[HEAD].0 + posed[HEAD].1 * V::new(0.1, 0.0, 0.165);
                    assert!((eye.z - 1.6).abs() < 0.12, "the eyes at {:.2} m", eye.z);
                    for (p, (m, anchor)) in made.iter().enumerate() {
                        let (joint, turn) = posed[p];
                        let top = joint + turn * (anchor + V::new(0.0, 0.0, m.size[2] as f64 / BODY_RES));
                        assert!(top.norm() < 2.2 && (joint + turn * anchor).z > -0.2, "{} at {top:?}", PIECE_NAMES[p]);
                    }
                }
            }
        }
    }

    /// The feet are on the ground standing still, and the colours of each piece are the looks'
    #[test]
    fn the_body_stands_on_its_feet_in_its_colours() {
        let looks = Looks::default();
        let posed = pose(&looks, &Stance::default());
        let (m, anchor) = piece(LEG[0], &looks);
        let bottom = posed[LEG[0]].0 + posed[LEG[0]].1 * anchor;
        assert!(bottom.z.abs() < 0.05, "the feet at {:.3} m", bottom.z);
        assert!(m.voxels.contains(&LEATHER) && m.voxels.contains(&CLOTH));
        let c = colours(&looks);
        assert_eq!(c[HEAD] & 0xFFFF, super::super::pack565(looks.skin_rgb()));
        assert_eq!(c[HEAD] >> 16, super::super::pack565(looks.hair_rgb()));
        assert_eq!(c[FOREARM[0]] & 0xFFFF, super::super::pack565(looks.gloves_rgb()));
    }

    /// Drawn for the window, the body's there in the middle of the picture, and turned round
    /// the picture changes
    #[test]
    fn the_preview_shows_the_body() {
        let looks = Looks { tunic: 2, ..Default::default() };
        let made: Vec<(Model, V)> = (0..PIECES).map(|p| piece(p, &looks)).collect();
        let (front, [w, h]) = picture(&made, &looks, 0.0, 120);
        assert_eq!(front.len(), w * h * 4);
        let red = |px: &[u8]| px[0] > 90 && px[0] > px[1] * 2;
        let tunic = front.chunks(4).filter(|px| red(px)).count();
        assert!(tunic > 200, "{tunic} pixels of the crimson tunic");
        let (side, _) = picture(&made, &looks, 1.4, 120);
        assert_ne!(front, side);
    }
}
