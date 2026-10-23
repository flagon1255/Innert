//! Animals wandering the landscape: bears, dogs, cats and squirrels. Each is a little model made
//! of voxels, built here from a few rounded shapes, with a few frames of walking. They're drawn by
//! shaders\traverse.comp, which traces rays through their voxels, so they cast shadows and show up
//! in reflections like everything else.

use std::f64::consts::{PI, TAU};

use nalgebra::{Vector2, Vector3};
use vulkano::buffer::BufferContents;

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
}

impl Look {
    pub const ALL: [Look; 9] = [
        Look::Bear,
        Look::Dog,
        Look::GrayCat,
        Look::GingerCat,
        Look::Squirrel,
        Look::Apple,
        Look::Meat,
        Look::Candle,
        Look::Skull,
    ];

    pub fn of_prop(kind: PropKind) -> Look {
        match kind {
            PropKind::Candle => Look::Candle,
            PropKind::Skull => Look::Skull,
        }
    }

    /// Voxels per meter of its model. Smaller things get smaller voxels, so they aren't just a few.
    fn resolution(self) -> f64 {
        match self {
            Look::Bear => 20.0,
            Look::Dog => 28.0,
            Look::GrayCat | Look::GingerCat => 36.0,
            Look::Squirrel => 44.0,
            Look::Apple => 64.0,
            Look::Meat => 48.0,
            Look::Candle => 48.0,
            Look::Skull => 48.0,
        }
    }

    /// Radius of a sphere around the whole model wherever it's turned, in meters
    fn reach(self) -> f64 {
        match self {
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
        }
    }

    /// Pets get petted, the rest can be hunted for meat
    pub fn is_pet(self) -> bool {
        matches!(self, Look::Dog | Look::GrayCat | Look::GingerCat)
    }

    /// How many hits it takes to hunt, and how much meat it gives
    fn health(self) -> u32 {
        match self {
            Look::Bear => 4,
            _ => 1,
        }
    }

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

type Vec3 = [f64; 3];

/// A capsule whose radius goes from `ra` at `a` to `rb` at `b`, stretched by `stretch` around its
/// middle line so it can be an ellipsoid too. Earlier shapes win where they overlap.
struct Part {
    a: Vec3,
    b: Vec3,
    ra: f64,
    rb: f64,
    stretch: Vec3,
    material: u8,
    /// Darker bands across the body, every few voxels along x
    stripes: u8,
}

fn ball(c: Vec3, r: f64, material: u8) -> Part {
    Part { a: c, b: c, ra: r, rb: r, stretch: [1.0; 3], material, stripes: AIR }
}

fn ellipsoid(c: Vec3, radii: Vec3, material: u8) -> Part {
    let r = radii[0].min(radii[1]).min(radii[2]);
    Part { a: c, b: c, ra: r, rb: r, stretch: radii.map(|v| v / r), material, stripes: AIR }
}

fn limb(a: Vec3, b: Vec3, ra: f64, rb: f64, material: u8) -> Part {
    Part { a, b, ra, rb, stretch: [1.0; 3], material, stripes: AIR }
}

impl Part {
    fn striped(mut self, stripes: u8) -> Self {
        self.stripes = stripes;
        self
    }

    fn contains(&self, p: Vec3) -> bool {
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
    }
    p
}

/// A model rasterized into voxels: x + (y + z * size.y) * size.x
struct Model {
    size: [u32; 3],
    voxels: Vec<u8>,
}

fn rasterize(look: Look, phase: Option<f64>) -> Model {
    let parts = parts(look, phase);
    let res = look.resolution();
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

/// Every frame of every animal, packed for shaders\traverse.comp: two header words per model
/// (where its voxels start, and its size packed x | y << 8 | z << 16), then the voxels, four to a word.
pub fn build_models() -> Vec<u32> {
    let mut models = Vec::new();
    for look in Look::ALL {
        models.push(rasterize(look, None));
        for frame in 1..FRAMES {
            models.push(rasterize(look, Some((frame - 1) as f64 / (FRAMES - 1) as f64)));
        }
    }
    let mut words = vec![0u32; models.len() * 2];
    for (i, model) in models.iter().enumerate() {
        words[i * 2] = words.len() as u32;
        words[i * 2 + 1] = model.size[0] | model.size[1] << 8 | model.size[2] << 16;
        for chunk in model.voxels.chunks(4) {
            words.push(chunk.iter().enumerate().fold(0, |w, (j, &m)| w | (m as u32) << (j * 8)));
        }
    }
    words
}

/// One animal as traverse.comp reads it, see the Animal struct there
#[derive(BufferContents, Clone, Copy)]
#[repr(C)]
pub struct AnimalInstance {
    position: [f32; 4],
    rotation: [f32; 4],
    info: [u32; 4],
}

#[derive(Clone, Copy, PartialEq)]
enum Mood {
    Idle,
    Walk,
    Run,
}

/// Positions are in voxels of the main world
pub struct Animal {
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
    health: u32,
    rng: u32,
}

/// Something in the world drawn like an animal that doesn't move, like food lying on the ground.
/// `position` is in voxels of the main world.
pub fn still_instance(look: Look, position: Vector3<f64>, heading: f64, world_voxels_per_meter: u32) -> AnimalInstance {
    let vpm = world_voxels_per_meter as f64;
    AnimalInstance {
        position: [position.x as f32, position.y as f32, position.z as f32, (look.reach() * vpm) as f32],
        rotation: [heading.cos() as f32, heading.sin() as f32, (vpm / look.resolution()) as f32, 0.0],
        info: [model_index(look, 0), 0, 0, 0],
    }
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
        let mut rng = rng;
        let heading = random(&mut rng) * TAU;
        Animal {
            look,
            position,
            heading,
            target_heading: heading,
            speed: 0.0,
            mood: Mood::Idle,
            timer: random(&mut rng) * 3.0,
            phase: 0.0,
            health: look.health(),
            rng,
        }
    }

    pub fn heading(&self) -> f64 {
        self.heading
    }

    /// Hit by the player standing at `from`. Runs off, and returns whether that was the last hit.
    pub fn hurt(&mut self, from: Vector3<f64>) -> bool {
        self.health = self.health.saturating_sub(1);
        self.flee(from, 4.0);
        self.health == 0
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

        // Turn towards where it's headed, the short way round
        let turn = (self.target_heading - self.heading + PI).rem_euclid(TAU) - PI;
        self.heading += turn.clamp(-3.0 * dt, 3.0 * dt);

        let wish = match self.mood {
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
        AnimalInstance {
            position: [self.position.x as f32, self.position.y as f32, self.position.z as f32, radius as f32],
            rotation: [self.heading.cos() as f32, self.heading.sin() as f32, scale as f32, 0.0],
            info: [model_index(self.look, frame), 0, 0, 0],
        }
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
