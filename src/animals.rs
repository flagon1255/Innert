//! Animals wandering the landscape: bears, dogs, cats and squirrels. Each is a little model made
//! of voxels, built here from a few rounded shapes, with a few frames of walking. They're drawn by
//! shaders\traverse.comp, which traces rays through their voxels, so they cast shadows and show up
//! in reflections like everything else.

use std::f64::consts::{PI, TAU};
use std::sync::atomic::{AtomicU32, Ordering};

use nalgebra::{UnitQuaternion, Vector2, Vector3};
use vulkano::buffer::BufferContents;

use crate::combat::Status;
use crate::player::WaterLevel;
use crate::worldgen::{PropKind, World, is_passable, material::*};

/// What an animal, or something lying around to pick up, looks like. Each has its own model, in
/// several frames.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Look {
    Bear,
    Dog,
    GrayCat,
    GingerCat,
    Squirrel,
    Apple,
    Meat,
    Candle,
    Skull,
    Chest,
    OpenChest,
}

impl Look {
    pub const ALL: [Look; 11] = [
        Look::Bear,
        Look::Dog,
        Look::GrayCat,
        Look::GingerCat,
        Look::Squirrel,
        Look::Apple,
        Look::Meat,
        Look::Candle,
        Look::Skull,
        Look::Chest,
        Look::OpenChest,
    ];

    pub fn of_prop(kind: PropKind) -> Look {
        match kind {
            PropKind::Candle => Look::Candle,
            PropKind::Skull => Look::Skull,
            PropKind::Chest => Look::Chest,
        }
    }

    /// Voxels per meter of its model: the same size as the world's, like everything else
    fn resolution(self) -> f64 {
        MODEL_RES
    }

    /// Radius of a sphere around the whole model wherever it's turned, in meters
    fn reach(self) -> f64 {
        match self {
            Look::Chest | Look::OpenChest => 0.95,
            Look::Bear => 1.6,
            Look::Dog => 0.8,
            Look::GrayCat | Look::GingerCat => 0.45,
            Look::Squirrel => 0.3,
            Look::Apple => 0.14,
            Look::Meat => 0.22,
            Look::Candle => 0.2,
            Look::Skull => 0.2,
        }
    }

    /// The body to aim at: how high its middle is, and how big it is, in meters
    pub fn body(self) -> (f64, f64) {
        match self {
            Look::Bear => (0.85, 0.75),
            Look::Dog => (0.5, 0.35),
            Look::GrayCat | Look::GingerCat => (0.28, 0.22),
            Look::Squirrel => (0.14, 0.16),
            Look::Apple => (0.06, 0.2),
            Look::Meat => (0.05, 0.25),
            Look::Candle => (0.1, 0.1),
            Look::Skull => (0.1, 0.12),
            Look::Chest | Look::OpenChest => (0.22, 0.4),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Look::Bear => "bear",
            Look::Dog => "dog",
            Look::GrayCat | Look::GingerCat => "cat",
            Look::Squirrel => "squirrel",
            Look::Apple => "apple",
            Look::Meat => "meat",
            Look::Candle => "candle",
            Look::Skull => "skull",
            Look::Chest | Look::OpenChest => "chest",
        }
    }

    /// Pets get petted, the rest can be hunted for meat
    pub fn is_pet(self) -> bool {
        matches!(self, Look::Dog | Look::GrayCat | Look::GingerCat)
    }

    /// How much damage it takes to hunt. A bare-handed hit does PUNCH.
    pub fn health(self) -> f64 {
        match self {
            Look::Bear => 140.0,
            Look::Dog => 60.0,
            Look::GrayCat | Look::GingerCat => 30.0,
            _ => 20.0,
        }
    }

    /// How much meat it gives

    pub fn meat(self) -> u32 {
        match self {
            Look::Bear => 3,
            Look::Squirrel => 1,
            _ => 0,
        }
    }

    // Speeds in meters per second
    fn walk_speed(self) -> f64 {
        match self {
            Look::Bear => 1.1,
            Look::Dog => 1.6,
            Look::Squirrel => 1.4,
            _ => 0.9,
        }
    }

    fn run_speed(self) -> f64 {
        match self {
            Look::Bear => 3.2,
            Look::Dog => 4.5,
            Look::Squirrel => 4.0,
            _ => 3.0,
        }
    }

    /// Meters covered by one cycle of its walk
    fn stride(self) -> f64 {
        match self {
            Look::Bear => 1.5,
            Look::Dog => 0.9,
            Look::Squirrel => 0.35,
            _ => 0.55,
        }
    }

    /// Highest ledge it can step up, in meters
    fn climb(self) -> f64 {
        match self {
            Look::Dog => 0.4,
            _ => 0.5,
        }
    }
}

/// Frame 0 stands still, the rest are a walk cycle
const FRAMES: usize = 5;

/// Voxels per meter of everything drawn like an animal, weapons and leaves too: the same as the
/// world's, so every voxel is the same size
pub const MODEL_RES: f64 = 16.0;

type Vec3 = [f64; 3];

/// A capsule whose radius goes from `ra` at `a` to `rb` at `b`, stretched by `stretch` around its
/// middle line so it can be an ellipsoid too. Or with `boxed`, the box with corners `a` and `b`.
/// Earlier shapes win where they overlap.
#[derive(Clone, Copy)]
pub struct Part {
    a: Vec3,
    b: Vec3,
    ra: f64,
    rb: f64,
    stretch: Vec3,
    boxed: bool,
    material: u8,
    /// Darker bands across the body, every few voxels along x
    stripes: u8,
}

pub fn ball(c: Vec3, r: f64, material: u8) -> Part {
    Part { a: c, b: c, ra: r, rb: r, stretch: [1.0; 3], boxed: false, material, stripes: AIR }
}

pub fn ellipsoid(c: Vec3, radii: Vec3, material: u8) -> Part {
    let r = radii[0].min(radii[1]).min(radii[2]);
    Part { a: c, b: c, ra: r, rb: r, stretch: radii.map(|v| v / r), boxed: false, material, stripes: AIR }
}

pub fn limb(a: Vec3, b: Vec3, ra: f64, rb: f64, material: u8) -> Part {
    Part { a, b, ra, rb, stretch: [1.0; 3], boxed: false, material, stripes: AIR }
}

pub fn cuboid(lo: Vec3, hi: Vec3, material: u8) -> Part {
    Part { a: lo, b: hi, ra: 0.0, rb: 0.0, stretch: [1.0; 3], boxed: true, material, stripes: AIR }
}

impl Part {
    fn striped(mut self, stripes: u8) -> Self {
        self.stripes = stripes;
        self
    }

    /// `s` times as big, and as far from the origin
    pub fn scaled(&self, s: f64) -> Part {
        Part { a: self.a.map(|v| v * s), b: self.b.map(|v| v * s), ra: self.ra * s, rb: self.rb * s, ..*self }
    }

    /// Squashed or stretched around its middle, `stretch` times as big along each axis
    pub fn stretched(mut self, stretch: Vec3) -> Self {
        self.stretch = [0, 1, 2].map(|i| self.stretch[i] * stretch[i]);
        self
    }

    /// At least a voxel thick at `res` voxels per meter whichever way it's turned, so nothing
    /// thin falls between the voxels
    fn thickened(&self, res: f64) -> Part {
        let (a, b) = (self.a, self.b);
        if self.boxed {
            let half = 0.51 / res;
            let mut lo = a;
            let mut hi = b;
            for i in 0..3 {
                if hi[i] - lo[i] < 2.0 * half {
                    let mid = (lo[i] + hi[i]) * 0.5;
                    lo[i] = mid - half;
                    hi[i] = mid + half;
                }
            }
            return Part { a: lo, b: hi, ..*self };
        }
        let least = 0.72 / res;
        let thinnest = self.ra.min(self.rb).max(1e-6);
        let (ra, rb) = (self.ra.max(least), self.rb.max(least));
        let k = ra.min(rb);
        let stretch = self.stretch.map(|s| (s * thinnest / k).max(least / k));
        Part { a, b, ra, rb, stretch, ..*self }
    }

    fn contains(&self, p: Vec3) -> bool {
        if self.boxed {
            return (0..3).all(|i| p[i] >= self.a[i] && p[i] <= self.b[i]);
        }
        // Squash the point towards the middle so stretched shapes become plain capsules
        let mid = [0, 1, 2].map(|i| (self.a[i] + self.b[i]) * 0.5);
        let q = [0, 1, 2].map(|i| mid[i] + (p[i] - mid[i]) / self.stretch[i]);
        let ab = [0, 1, 2].map(|i| self.b[i] - self.a[i]);
        let aq = [0, 1, 2].map(|i| q[i] - self.a[i]);
        let length2: f64 = ab.iter().map(|v| v * v).sum();
        let t = if length2 > 0.0 { (aq.iter().zip(&ab).map(|(a, b)| a * b).sum::<f64>() / length2).clamp(0.0, 1.0) } else { 0.0 };
        let r = self.ra + (self.rb - self.ra) * t;
        let d2: f64 = (0..3).map(|i| (aq[i] - ab[i] * t).powi(2)).sum();
        d2 <= r * r
    }

    fn reach(&self) -> (Vec3, Vec3) {
        if self.boxed {
            return (self.a, self.b);
        }
        let r = self.ra.max(self.rb);
        let s = self.stretch;
        let mid = [0, 1, 2].map(|i| (self.a[i] + self.b[i]) * 0.5);
        let lo = [0, 1, 2].map(|i| mid[i] + (self.a[i].min(self.b[i]) - r - mid[i]) * s[i]);
        let hi = [0, 1, 2].map(|i| mid[i] + (self.a[i].max(self.b[i]) + r - mid[i]) * s[i]);
        (lo, hi)
    }
}

/// A leg hanging from `hip`, swung forwards by `swing` radians, ending in a paw
fn leg(parts: &mut Vec<Part>, hip: Vec3, length: f64, radius: f64, swing: f64, fur: u8, paw: u8) {
    let foot = [hip[0] + swing.sin() * length, hip[1], hip[2] - swing.cos() * length + radius * 0.6];
    parts.push(ball(foot, radius * 1.1, paw));
    parts.push(limb(hip, foot, radius * 1.15, radius, fur));
}

/// The shapes making up an animal, in meters: facing +x, with its feet on z = 0.
/// `phase` is how far through its walk it is, None standing still.
fn parts(look: Look, phase: Option<f64>) -> Vec<Part> {
    let angle = phase.map_or(0.0, |p| p * TAU);
    let moving = phase.is_some() as u8 as f64;
    // Diagonal pairs of legs swing together
    let swing = angle.sin() * moving;
    let mut p = Vec::new();
    match look {
        Look::Bear => {
            let (fur, dark) = (FUR_BROWN, FUR_DARK);
            p.push(ball([0.98, 0.12, 1.08], 0.035, EYE));
            p.push(ball([0.98, -0.12, 1.08], 0.035, EYE));
            p.push(ball([1.2, 0.0, 0.97], 0.05, EYE));
            p.push(ellipsoid([1.06, 0.0, 0.93], [0.16, 0.12, 0.1], FUR_CREAM));
            p.push(ball([0.78, 0.19, 1.24], 0.08, dark));
            p.push(ball([0.78, -0.19, 1.24], 0.08, dark));
            p.push(ball([0.84, 0.0, 1.02], 0.27, fur));
            p.push(ball([0.32, 0.0, 1.06], 0.32, fur));
            p.push(ellipsoid([0.0, 0.0, 0.88], [0.78, 0.42, 0.42], fur));
            p.push(ball([-0.78, 0.0, 0.96], 0.08, fur));
            for (x, y, s) in [(0.48, 0.24, 1.0), (0.48, -0.24, -1.0), (-0.48, 0.24, -1.0), (-0.48, -0.24, 1.0)] {
                leg(&mut p, [x, y, 0.66], 0.66, 0.13, 0.35 * swing * s, fur, dark);
            }
        }
        Look::Dog => {
            let (fur, white) = (FUR_TAN, FUR_WHITE);
            p.push(ball([0.51, 0.06, 0.75], 0.022, EYE));
            p.push(ball([0.51, -0.06, 0.75], 0.022, EYE));
            p.push(ball([0.635, 0.0, 0.685], 0.026, EYE));
            p.push(ellipsoid([0.55, 0.0, 0.66], [0.09, 0.06, 0.055], white));
            p.push(ellipsoid([0.39, 0.115, 0.68], [0.04, 0.025, 0.08], FUR_BROWN));
            p.push(ellipsoid([0.39, -0.115, 0.68], [0.04, 0.025, 0.08], FUR_BROWN));
            p.push(ball([0.42, 0.0, 0.71], 0.115, fur));
            p.push(limb([0.27, 0.0, 0.5], [0.38, 0.0, 0.66], 0.085, 0.07, fur));
            p.push(ellipsoid([0.2, 0.0, 0.41], [0.15, 0.12, 0.13], white));
            p.push(ellipsoid([0.0, 0.0, 0.47], [0.36, 0.15, 0.16], fur));
            // Wags while it walks
            let wag = 0.06 * (angle * 2.0).sin() * moving;
            p.push(limb([-0.32, 0.0, 0.53], [-0.5, wag, 0.7], 0.035, 0.025, fur));
            for (x, y, s) in [(0.22, 0.09, 1.0), (0.22, -0.09, -1.0), (-0.24, 0.09, -1.0), (-0.24, -0.09, 1.0)] {
                leg(&mut p, [x, y, 0.4], 0.4, 0.042, 0.45 * swing * s, fur, white);
            }
        }
        Look::GrayCat | Look::GingerCat => {
            let (fur, stripes) = if look == Look::GrayCat { (FUR_GRAY, FUR_DARK) } else { (FUR_GINGER, FUR_RED) };
            let white = FUR_WHITE;
            p.push(ball([0.3, 0.035, 0.38], 0.015, EYE));
            p.push(ball([0.3, -0.035, 0.38], 0.015, EYE));
            p.push(ball([0.327, 0.0, 0.345], 0.012, NOSE));
            p.push(ball([0.3, 0.0, 0.33], 0.038, white));
            p.push(limb([0.225, 0.045, 0.41], [0.22, 0.052, 0.47], 0.026, 0.006, fur));
            p.push(limb([0.225, -0.045, 0.41], [0.22, -0.052, 0.47], 0.026, 0.006, fur));
            p.push(ball([0.24, 0.0, 0.36], 0.08, fur));
            p.push(ellipsoid([0.03, 0.0, 0.21], [0.15, 0.06, 0.055], white));
            p.push(ellipsoid([0.0, 0.0, 0.25], [0.21, 0.085, 0.09], fur).striped(stripes));
            // The tail sways
            let sway = 0.04 * (angle + 1.0).sin() * moving;
            p.push(limb([-0.19, 0.0, 0.28], [-0.3, sway, 0.42], 0.022, 0.02, fur).striped(stripes));
            p.push(limb([-0.3, sway, 0.42], [-0.28, sway * 1.5, 0.53], 0.02, 0.016, fur).striped(stripes));
            for (x, y, s) in [(0.14, 0.05, 1.0), (0.14, -0.05, -1.0), (-0.13, 0.05, -1.0), (-0.13, -0.05, 1.0)] {
                leg(&mut p, [x, y, 0.21], 0.21, 0.024, 0.45 * swing * s, fur, white);
            }
        }
        Look::Squirrel => {
            // Squirrels bound along: both pairs of legs together, the body bobbing up between
            let fur = FUR_RED;
            let lift = 0.04 * angle.sin().max(0.0) * moving;
            let up = |v: Vec3| [v[0], v[1], v[2] + lift];
            p.push(ball(up([0.13, 0.027, 0.175]), 0.011, EYE));
            p.push(ball(up([0.13, -0.027, 0.175]), 0.011, EYE));
            p.push(ball(up([0.146, 0.0, 0.16]), 0.009, EYE));
            p.push(limb(up([0.09, 0.022, 0.2]), up([0.088, 0.025, 0.23]), 0.012, 0.004, fur));
            p.push(limb(up([0.09, -0.022, 0.2]), up([0.088, -0.025, 0.23]), 0.012, 0.004, fur));
            p.push(ball(up([0.1, 0.0, 0.16]), 0.045, fur));
            p.push(ellipsoid(up([0.03, 0.0, 0.085]), [0.07, 0.035, 0.04], FUR_CREAM));
            p.push(ellipsoid(up([0.0, 0.0, 0.11]), [0.1, 0.05, 0.06], fur));
            // A big bushy tail curling up over its back
            let flick = 0.02 * angle.cos() * moving;
            for (c, r) in [([-0.1, 0.0, 0.11], 0.045), ([-0.15, 0.0, 0.17], 0.055), ([-0.15 + flick, 0.0, 0.25], 0.06), ([-0.11 + flick, 0.0, 0.31], 0.055)] {
                p.push(ball(up(c), r, fur));
            }
            for (x, y, length) in [(0.06, 0.03, 0.07), (0.06, -0.03, 0.07), (-0.06, 0.04, 0.08), (-0.06, -0.04, 0.08)] {
                let s = if x > 0.0 { 1.0 } else { -1.0 };
                leg(&mut p, up([x, y, length]), length, 0.018, 0.5 * swing * s, fur, fur);
            }
        }
        Look::Apple => {
            p.push(limb([0.0, 0.0, 0.11], [0.01, 0.0, 0.145], 0.008, 0.006, WOOD));
            p.push(ellipsoid([0.028, 0.0, 0.138], [0.026, 0.012, 0.008], LEAVES));
            p.push(ellipsoid([0.0, 0.0, 0.062], [0.066, 0.066, 0.06], APPLE));
        }
        Look::Meat => {
            // A drumstick
            p.push(ball([-0.14, 0.022, 0.03], 0.024, BONE));
            p.push(ball([-0.14, -0.022, 0.03], 0.024, BONE));
            p.push(limb([-0.14, 0.0, 0.03], [-0.02, 0.0, 0.045], 0.016, 0.018, BONE));
            p.push(ellipsoid([0.05, 0.0, 0.055], [0.1, 0.07, 0.055], MEAT));
        }
        Look::Candle => {
            // A stubby candle with wax run down its side, and its flame
            p.push(ellipsoid([0.0, 0.0, 0.25], [0.028, 0.028, 0.05], FLAME));
            p.push(limb([0.0, 0.0, 0.18], [0.0, 0.0, 0.205], 0.008, 0.008, BARK_DARK));
            p.push(limb([0.045, 0.0, 0.06], [0.045, 0.0, 0.17], 0.018, 0.012, WAX));
            p.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, 0.18], 0.05, 0.045, WAX));
        }
        Look::Skull => {
            p.push(ball([0.085, 0.042, 0.115], 0.026, EYE));
            p.push(ball([0.085, -0.042, 0.115], 0.026, EYE));
            p.push(ball([0.105, 0.0, 0.075], 0.015, EYE));
            p.push(limb([0.075, -0.035, 0.045], [0.075, 0.035, 0.045], 0.008, 0.008, BARK_DARK));
            p.push(ellipsoid([0.055, 0.0, 0.045], [0.06, 0.06, 0.04], BONE));
            p.push(ellipsoid([0.0, 0.0, 0.115], [0.1, 0.085, 0.09], BONE));
        }
        Look::Chest | Look::OpenChest => {
            // A wooden chest bound with iron, its lock at the front. Open, its lid stands up
            // at the back and gold glints inside.
            let open = look == Look::OpenChest;
            if open {
                p.push(cuboid([-0.17, -0.3, 0.17], [0.17, 0.3, 0.21], GLOW_HOLY));
                p.push(cuboid([-0.18, -0.31, 0.04], [0.18, 0.31, 0.3], AIR));
            } else {
                p.push(cuboid([0.215, -0.04, 0.17], [0.245, 0.04, 0.26], GOLD));
            }
            for y in [-0.25, 0.25] {
                p.push(cuboid([-0.23, y - 0.025, 0.0], [0.23, y + 0.025, 0.29], DARK_STEEL));
            }
            p.push(cuboid([-0.22, -0.35, 0.0], [0.22, 0.35, 0.28], WOOD));
            let lid = |p: &mut Vec<Part>, lo: Vec3, hi: Vec3| {
                if open {
                    // Swung up and over on its hinge at the back
                    let turn = |q: Vec3| [-0.22 - (q[2] - 0.28), q[1], 0.28 + (q[0] + 0.22)];
                    let (a, b) = (turn(lo), turn(hi));
                    p.push(cuboid([a[0].min(b[0]), a[1], a[2].min(b[2])], [a[0].max(b[0]), b[1], a[2].max(b[2])], WOOD));
                } else {
                    p.push(cuboid(lo, hi, WOOD));
                }
            };
            if !open {
                for y in [-0.25, 0.25] {
                    p.push(cuboid([-0.23, y - 0.025, 0.28], [0.23, y + 0.025, 0.37], DARK_STEEL));
                    p.push(cuboid([-0.18, y - 0.025, 0.36], [0.18, y + 0.025, 0.41], DARK_STEEL));
                }
            }
            lid(&mut p, [-0.22, -0.35, 0.28], [0.22, 0.35, 0.36]);
            lid(&mut p, [-0.17, -0.35, 0.36], [0.17, 0.35, 0.4]);
        }
    }
    p
}

/// A model rasterized into voxels: x + (y + z * size.y) * size.x
pub struct Model {
    pub size: [u32; 3],
    pub voxels: Vec<u8>,
}

/// Turns shapes in meters into a model with `res` voxels per meter, sized to fit them however
/// they're placed. Returns where the shader puts the model in the world, the middle of its bottom,
/// in the same meters as the shapes.
pub fn voxelize(parts: &[Part], res: f64) -> (Model, Vector3<f64>) {
    let parts: Vec<Part> = parts.iter().map(|p| p.thickened(res)).collect();
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for part in &parts {
        let (a, b) = part.reach();
        for i in 0..3 {
            lo[i] = lo[i].min(a[i]);
            hi[i] = hi[i].max(b[i]);
        }
    }
    let size = [0, 1, 2].map(|i| (((hi[i] - lo[i]) * res).ceil() as u32).clamp(1, 1023));
    let mut voxels = vec![AIR; (size[0] * size[1] * size[2]) as usize];
    // Each shape only looks at the voxels around it, and takes those no earlier shape took
    let mut taken = vec![false; voxels.len()];
    for part in &parts {
        let (a, b) = part.reach();
        let from = [0, 1, 2].map(|i| (((a[i] - lo[i]) * res - 0.5).floor().max(0.0) as u32).min(size[i]));
        let to = [0, 1, 2].map(|i| (((b[i] - lo[i]) * res + 0.5).ceil().max(0.0) as u32).min(size[i]));
        for z in from[2]..to[2] {
            for y in from[1]..to[1] {
                for x in from[0]..to[0] {
                    let i = (x + (y + z * size[1]) * size[0]) as usize;
                    let p = [lo[0] + (x as f64 + 0.5) / res, lo[1] + (y as f64 + 0.5) / res, lo[2] + (z as f64 + 0.5) / res];
                    if !taken[i] && part.contains(p) {
                        taken[i] = true;
                        voxels[i] = part.material;
                    }
                }
            }
        }
    }
    let anchor = Vector3::new(lo[0] + size[0] as f64 * 0.5 / res, lo[1] + size[1] as f64 * 0.5 / res, lo[2]);
    (Model { size, voxels }, anchor)
}

fn rasterize(look: Look, phase: Option<f64>) -> Model {
    let res = look.resolution();
    let parts: Vec<Part> = parts(look, phase).iter().map(|p| p.thickened(res)).collect();
    // Wide enough either side of the middle to hold everything, as the shader puts the middle of
    // the model where the animal stands
    let (mut half_x, mut half_y, mut top) = (0.0f64, 0.0f64, 0.0f64);
    for part in &parts {
        let (lo, hi) = part.reach();
        half_x = half_x.max(lo[0].abs()).max(hi[0].abs());
        half_y = half_y.max(lo[1].abs()).max(hi[1].abs());
        top = top.max(hi[2]);
    }
    let size = [
        ((2.0 * half_x * res).ceil() as u32).clamp(1, 255),
        ((2.0 * half_y * res).ceil() as u32).clamp(1, 255),
        ((top * res).ceil() as u32).clamp(1, 255),
    ];
    let mut voxels = vec![AIR; (size[0] * size[1] * size[2]) as usize];
    for z in 0..size[2] {
        for y in 0..size[1] {
            for x in 0..size[0] {
                let p = [
                    (x as f64 + 0.5 - size[0] as f64 * 0.5) / res,
                    (y as f64 + 0.5 - size[1] as f64 * 0.5) / res,
                    (z as f64 + 0.5) / res,
                ];
                if let Some(part) = parts.iter().find(|part| part.contains(p)) {
                    let band = part.stripes != AIR && (x / 2) % 3 == 0;
                    voxels[(x + (y + z * size[1]) * size[0]) as usize] = if band { part.stripes } else { part.material };
                }
            }
        }
    }
    Model { size, voxels }
}

fn model_index(look: Look, frame: usize) -> u32 {
    (Look::ALL.iter().position(|&l| l == look).unwrap() * FRAMES + frame) as u32
}

/// How many models animal_models() makes. Other models come after them.
pub const ANIMAL_MODELS: usize = Look::ALL.len() * FRAMES;

/// Every frame of every animal, and of everything else drawn like one
pub fn animal_models() -> Vec<Model> {
    let mut models = Vec::new();
    for look in Look::ALL {
        models.push(rasterize(look, None));
        for frame in 1..FRAMES {
            models.push(rasterize(look, Some((frame - 1) as f64 / (FRAMES - 1) as f64)));
        }
    }
    models
}

/// Models packed for shaders\traverse.comp: two header words per model (where its voxels start, and
/// its size packed x | y << 10 | z << 20), then the voxels, four to a word.
pub fn pack_models(models: &[&Model]) -> Vec<u32> {
    let mut words = vec![0u32; models.len() * 2];
    for (i, model) in models.iter().enumerate() {
        words[i * 2] = words.len() as u32;
        words[i * 2 + 1] = model.size[0] | model.size[1] << 10 | model.size[2] << 20;
        for chunk in model.voxels.chunks(4) {
            words.push(chunk.iter().enumerate().fold(0, |w, (j, &m)| w | (m as u32) << (j * 8)));
        }
    }
    words
}

// How an instance moves, has to match shaders\traverse.comp
pub const STILL: u32 = 0;
pub const MOVING: u32 = 1;
/// In the player's hand
pub const HELD: u32 = 2;

/// What weapons did to an animal, shown on it. Has to match TINT_* in shaders\traverse.comp.
pub const TINT_NONE: u32 = 0;
pub const TINT_BURNING: u32 = 1;
pub const TINT_FROZEN: u32 = 2;
pub const TINT_POISONED: u32 = 3;
pub const TINT_SHOCKED: u32 = 4;
pub const TINT_CHILLED: u32 = 5;

/// One animal as traverse.comp reads it, see the Animal struct there
#[derive(BufferContents, Clone, Copy)]
#[repr(C)]
pub struct AnimalInstance {
    position: [f32; 4],
    rotation: [f32; 4],
    scale: [f32; 4],
    info: [u32; 4],
}

impl AnimalInstance {
    /// Model `model` turned by `rotation`, with the middle of its bottom at `position` and its
    /// voxels `scale` voxels of the main world across. `reach` is how far it reaches from there,
    /// in voxels of the main world, and `kind` STILL, MOVING or HELD.
    pub fn new(model: u32, position: Vector3<f64>, rotation: UnitQuaternion<f64>, scale: f64, reach: f64, kind: u32) -> Self {
        let q = rotation.coords;
        AnimalInstance {
            position: [position.x as f32, position.y as f32, position.z as f32, reach as f32],
            rotation: [q.x as f32, q.y as f32, q.z as f32, q.w as f32],
            scale: [scale as f32, 0.0, 0.0, 0.0],
            info: [model, kind, TINT_NONE, 0],
        }
    }

    pub fn tinted(mut self, tint: u32) -> Self {
        self.info[2] = tint;
        self
    }
}

fn facing(heading: f64) -> UnitQuaternion<f64> {
    UnitQuaternion::from_axis_angle(&Vector3::z_axis(), heading)
}

#[derive(Clone, Copy, PartialEq)]
enum Mood {
    Idle,
    Walk,
    Run,
}

/// Positions are in voxels of the main world
pub struct Animal {
    /// Different for every animal
    pub id: u32,
    pub look: Look,
    /// The middle of its feet
    pub position: Vector3<f64>,
    heading: f64,
    target_heading: f64,
    speed: f64,
    mood: Mood,
    timer: f64,
    /// How far through its walk cycle, 0-1
    phase: f64,
    pub health: f64,
    /// Burning, poisoned, frozen and such, see src\combat.rs
    pub status: Status,
    rng: u32,
}

/// How hard a bare-handed hit is
pub const PUNCH: f64 = 35.0;

/// Something in the world drawn like an animal that doesn't move, like food lying on the ground.
/// `position` is in voxels of the main world.
pub fn still_instance(look: Look, position: Vector3<f64>, heading: f64, world_voxels_per_meter: u32) -> AnimalInstance {
    let vpm = world_voxels_per_meter as f64;
    AnimalInstance::new(model_index(look, 0), position, facing(heading), vpm / look.resolution(), look.reach() * vpm, STILL)
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^ (x >> 16)
}

fn random(seed: &mut u32) -> f64 {
    *seed = hash(seed.wrapping_add(0x9e3779b9));
    (*seed >> 8) as f64 / 16777216.0
}

/// Height of the ground an animal standing at height `z` could step onto at x, y: up at most
/// `up` voxels, down at most `down`. None for walls, cliffs, water and outside the world.
fn ground_at(world: &World, water: WaterLevel, x: f64, y: f64, z: f64, up: f64, down: f64) -> Option<f64> {
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    let top = (z + up).floor() as i64;
    if !is_passable(world.material_at(xi, yi, top)) {
        return None;
    }
    let ground = ((z - down).floor() as i64..top)
        .rev()
        .find(|&zi| !is_passable(world.material_at(xi, yi, zi)))
        .map(|zi| zi as f64 + 1.0)?;
    // Paddling in the shallows is fine
    (water(x, y) < ground + 0.2 * world.voxels_per_meter as f64).then_some(ground)
}

/// The highest dry ground in a column, if it isn't under water
fn surface_at(world: &World, water: WaterLevel, x: f64, y: f64) -> Option<f64> {
    ground_at(world, water, x, y, 0.0, world.size[2] as f64 - 1.0, 1.0)
}

impl Animal {
    fn new(look: Look, position: Vector3<f64>, rng: u32) -> Self {
        static NEXT_ID: AtomicU32 = AtomicU32::new(1);
        let mut rng = rng;
        let heading = random(&mut rng) * TAU;
        Animal {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            look,
            position,
            heading,
            target_heading: heading,
            speed: 0.0,
            mood: Mood::Idle,
            timer: random(&mut rng) * 3.0,
            phase: 0.0,
            health: look.health(),
            status: Status::default(),
            rng,
        }
    }

    pub fn heading(&self) -> f64 {
        self.heading
    }

    /// The middle of its body, to aim at
    pub fn center(&self, world_voxels_per_meter: u32) -> Vector3<f64> {
        self.position + Vector3::new(0.0, 0.0, self.look.body().0 * world_voxels_per_meter as f64)
    }

    /// How much health it has left, 0-1
    pub fn vitality(&self) -> f64 {
        self.health / self.look.health()
    }

    /// Hit bare-handed by the player standing at `from`. Runs off, and returns whether that was
    /// the last hit.
    pub fn hurt(&mut self, from: Vector3<f64>) -> bool {
        self.damage(PUNCH, from)
    }

    /// Takes `amount` of damage from something at `from`, and runs off. Returns whether it died.
    pub fn damage(&mut self, amount: f64, from: Vector3<f64>) -> bool {
        self.health = (self.health - amount).max(0.0);
        self.flee(from, 4.0);
        self.health <= 0.0
    }

    pub fn flee(&mut self, from: Vector3<f64>, seconds: f64) {
        let away = (self.position - from).xy();
        self.mood = Mood::Run;
        self.timer = seconds;
        self.target_heading = away.y.atan2(away.x);
    }

    /// Petted by the player at `from`: stops to enjoy it, facing them
    pub fn pet(&mut self, from: Vector3<f64>) {
        let towards = (from - self.position).xy();
        self.mood = Mood::Idle;
        self.timer = 4.0;
        self.target_heading = towards.y.atan2(towards.x);
    }

    /// `player` is where the player is, in voxels, so some animals can come over or run off
    pub fn update(&mut self, world: &World, water: WaterLevel, player: Option<Vector3<f64>>, dt: f64) {
        let vpm = world.voxels_per_meter as f64;
        let look = self.look;

        self.timer -= dt;
        if self.timer <= 0.0 {
            let r = random(&mut self.rng);
            let (mood, time) = if r < 0.4 {
                (Mood::Idle, 2.0 + 4.0 * random(&mut self.rng))
            } else if r < 0.9 {
                (Mood::Walk, 3.0 + 5.0 * random(&mut self.rng))
            } else {
                (Mood::Run, 1.5 + 2.0 * random(&mut self.rng))
            };
            self.mood = mood;
            self.timer = time;
            self.target_heading = self.heading + (random(&mut self.rng) - 0.5) * 3.0;

            // Dogs like to come over to the player now and then
            if let Some(player) = player
                && look == Look::Dog
                && random(&mut self.rng) < 0.5
            {
                let offset = (player - self.position).xy() / vpm;
                if offset.norm() > 3.0 && offset.norm() < 25.0 {
                    self.mood = Mood::Walk;
                    self.target_heading = offset.y.atan2(offset.x);
                }
            }
        }

        // Squirrels dart away when the player gets close
        if let Some(player) = player
            && look == Look::Squirrel
        {
            let offset = (self.position - player).xy() / vpm;
            if offset.norm() < 4.0 && self.mood != Mood::Run {
                self.mood = Mood::Run;
                self.timer = 1.5;
                self.target_heading = offset.y.atan2(offset.x);
            }
        }

        // Thrown back by a blow, sliding to a stop
        let push = self.status.push * dt;
        if push.norm() > 1e-3 {
            let (x, y) = (self.position.x + push.x, self.position.y + push.y);
            let reach = look.climb() * vpm * 2.0;
            if let Some(z) = ground_at(world, water, x, y, self.position.z, reach, reach * 2.0) {
                self.position = Vector3::new(x, y, z);
            } else {
                self.status.push = Vector3::zeros();
            }
        }
        self.status.push *= (-6.0 * dt).exp();

        // Frozen solid or knocked off its feet it can't move, slowed it crawls
        let pace = self.status.pace();
        if pace <= 0.0 {
            self.speed = 0.0;
            return;
        }

        // Turn towards where it's headed, the short way round
        let turn = (self.target_heading - self.heading + PI).rem_euclid(TAU) - PI;
        self.heading += turn.clamp(-3.0 * dt * pace, 3.0 * dt * pace);

        let wish = pace
            * match self.mood {
                Mood::Idle => 0.0,
                Mood::Walk => look.walk_speed(),
                Mood::Run => look.run_speed(),
            };
        self.speed += (wish - self.speed) * (1.0 - (-4.0 * dt).exp());

        if self.speed > 0.01 {
            let step = Vector2::new(self.heading.cos(), self.heading.sin()) * self.speed * vpm * dt;
            // Feel ahead a little, so it turns before its nose is in a wall
            let ahead = Vector2::new(self.heading.cos(), self.heading.sin()) * 0.3 * vpm;
            let (x, y) = (self.position.x + step.x, self.position.y + step.y);
            let climb = look.climb() * vpm;
            let ground = ground_at(world, water, x, y, self.position.z, climb, climb * 2.0);
            let clear = ground_at(world, water, x + ahead.x, y + ahead.y, self.position.z, climb, climb * 2.0).is_some();
            match ground {
                Some(z) if clear => {
                    self.position.x = x;
                    self.position.y = y;
                    // Settle onto the new height quickly but not instantly
                    self.position.z += (z - self.position.z) * (1.0 - (-20.0 * dt).exp());
                }
                _ => {
                    // Something's in the way: turn around and try elsewhere
                    self.target_heading = self.heading + PI + (random(&mut self.rng) - 0.5) * 1.5;
                    self.speed = 0.0;
                }
            }
            self.phase = (self.phase + self.speed * dt / look.stride()).fract();
        }
    }

    pub fn instance(&self, world_voxels_per_meter: u32) -> AnimalInstance {
        let frame = if self.speed > 0.15 { 1 + (self.phase * (FRAMES - 1) as f64) as usize % (FRAMES - 1) } else { 0 };
        let scale = world_voxels_per_meter as f64 / self.look.resolution();
        let radius = self.look.reach() * world_voxels_per_meter as f64;
        AnimalInstance::new(model_index(self.look, frame), self.position, facing(self.heading), scale, radius, MOVING).tinted(self.status.tint())
    }
}

/// Scatters animals over dry land: about half of them close to `near` so there are some to see
/// straight away, the rest anywhere.
pub fn spawn(world: &World, water: WaterLevel, seed: u32, near: Vector3<f64>) -> Vec<Animal> {
    let vpm = world.voxels_per_meter as f64;
    let [sx, sy, _] = world.size.map(f64::from);
    let mut rng = hash(seed ^ 0xa11a);
    let mut animals = Vec::new();
    for (look, count) in [(Look::Bear, 6), (Look::Dog, 8), (Look::GrayCat, 5), (Look::GingerCat, 5), (Look::Squirrel, 14)] {
        for i in 0..count {
            for _ in 0..64 {
                let (x, y) = if i % 2 == 0 {
                    let angle = random(&mut rng) * TAU;
                    let distance = (4.0 + 40.0 * random(&mut rng)) * vpm;
                    (near.x + angle.cos() * distance, near.y + angle.sin() * distance)
                } else {
                    (sx * (0.1 + 0.8 * random(&mut rng)), sy * (0.1 + 0.8 * random(&mut rng)))
                };
                if x < 1.0 || y < 1.0 || x >= sx - 1.0 || y >= sy - 1.0 {
                    continue;
                }
                if let Some(z) = surface_at(world, water, x, y) {
                    animals.push(Animal::new(look, Vector3::new(x, y, z), hash(rng)));
                    break;
                }
            }
        }
    }
    animals
}
