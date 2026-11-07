//! Leaves drifting down from the trees around the camera, a single voxel each, as in John Lin's
//! engine. They flutter as they fall, carried along by a slow wind, settle where they land, on the
//! ground or floating on water, and fade away after a while. Debris thrown out by explosions flies
//! and lands the same way. Positions are in voxels of the main world.

use std::f64::consts::TAU;

use nalgebra::{UnitQuaternion, Vector3};

use crate::animals::{AnimalInstance, MODEL_RES, MOVING, Model, STILL, cuboid, voxelize};
use crate::player::WaterLevel;
use crate::worldgen::{World, is_passable, material::*};

/// Leaves that fall: all the shades of broad-leaved crowns, and the blossom in them. Pines keep
/// their needles.
const KINDS: [u8; 14] = [
    LEAVES,
    LEAVES_DARK,
    LEAVES_YELLOW,
    LEAVES_ORANGE,
    LEAVES_RED,
    LEAVES_LIME,
    LEAVES_CRIMSON,
    FERN,
    WILLOW,
    BLOSSOM,
    FLOWER_RED,
    FLOWER_YELLOW,
    FLOWER_WHITE,
    FLOWER_PINK,
];
/// One model of a single voxel for each material, see Leaves::models: up to the last material
const MATERIALS: u8 = LEAVES_CRIMSON + 1;
/// Leaves only fall from this many meters up at the least, so not from ferns and flowers
const LOWEST: f64 = 1.2;
/// Voxels per meter of a leaf: a single voxel the size of the world's
const RES: f64 = MODEL_RES;
/// Leaves fall within this many meters of the camera
const RANGE: f64 = 24.0;
/// The most there are at once, and of debris
const MOST: usize = 90;
const MOST_DEBRIS: usize = 160;
/// How many spots a second are looked at for a tree to drop a leaf, and when the world's new
const TRIES: f64 = 80.0;
const WARM_TRIES: usize = 600;
/// Meters a second
const FALL_SPEED: f64 = 0.6;
const WIND: f64 = 0.35;
/// Seconds a leaf lies where it landed before it's gone
const LIE: f64 = 8.0;

struct Leaf {
    position: Vector3<f64>,
    material: u8,
    /// Where it is in its flutter, in radians
    phase: f64,
    /// How fast it flutters, and how far it swings
    flutter: f64,
    swing: f64,
    /// For debris, flying through the air, in voxels per second. None for leaves fluttering down.
    velocity: Option<Vector3<f64>>,
    /// Seconds left lying where it landed, None while falling
    landed: Option<f64>,
    /// For debris: how many voxels across the chunk is, which way it's turned and how fast it
    /// tumbles, about which way in radians a second, and how often it's bounced
    size: f64,
    turn: UnitQuaternion<f64>,
    spin: Vector3<f64>,
    bounces: u8,
}

/// Seconds a chunk thrown out by an explosion lies where it came to rest before it's gone, at the
/// least, and a bounce: how much of its speed down it keeps going back up, and of its speed along
/// the ground
const DEBRIS_LIE: f64 = 6.0;
const BOUNCE: f64 = 0.35;
const SKID: f64 = 0.55;

pub struct Leaves {
    leaves: Vec<Leaf>,
    /// One for each material, of a single voxel
    pub models: Vec<Model>,
    tries: f64,
    /// Set for a new world, to start it off with leaves already falling
    fresh: bool,
    time: f64,
    rng: u32,
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

impl Leaves {
    pub fn new(seed: u32) -> Self {
        let voxel = 1.0 / RES;
        let models = (0..MATERIALS).map(|m| voxelize(&[cuboid([0.0; 3], [voxel; 3], m)], RES).0).collect();
        Leaves { leaves: Vec::new(), models, tries: 0.0, fresh: true, time: 0.0, rng: hash(seed ^ 0x1eaf) }
    }

    pub fn clear(&mut self) {
        self.leaves.clear();
        self.fresh = true;
    }

    pub fn count(&self) -> usize {
        self.leaves.iter().filter(|l| l.velocity.is_none()).count()
    }

    /// The world moved over by `shift` voxels
    pub fn recenter(&mut self, shift: Vector3<f64>) {
        for leaf in &mut self.leaves {
            leaf.position -= shift;
        }
    }

    /// Chunks of what an explosion at `at` blew out, `materials`, thrown out in all directions,
    /// tumbling: a few big lumps, more small bits
    pub fn debris(&mut self, at: Vector3<f64>, materials: &[u8], vpm: f64) {
        for &material in materials {
            if material == AIR || material >= MATERIALS {
                continue;
            }
            let angle = random(&mut self.rng) * TAU;
            let out = 1.5 + 5.0 * random(&mut self.rng);
            let up = 3.5 + 6.0 * random(&mut self.rng);
            let velocity = Vector3::new(angle.cos() * out, angle.sin() * out, up) * vpm;
            let position = at + Vector3::new(angle.cos(), angle.sin(), 0.5) * 0.3 * vpm;
            let size = 1.5 + 4.0 * random(&mut self.rng).powi(2);
            let axis = Vector3::new(random(&mut self.rng) - 0.5, random(&mut self.rng) - 0.5, random(&mut self.rng) - 0.5).try_normalize(1e-6).unwrap_or(Vector3::z());
            // Little bits spin faster than big lumps
            let spin = axis * (5.0 + 10.0 * random(&mut self.rng)) / size.sqrt();
            let turn = UnitQuaternion::from_axis_angle(&nalgebra::Unit::new_normalize(axis), random(&mut self.rng) * TAU);
            self.leaves.push(Leaf { position, material, phase: 0.0, flutter: 0.0, swing: 0.0, velocity: Some(velocity), landed: None, size, turn, spin, bounces: 0 });
        }
        let debris = self.leaves.iter().filter(|l| l.velocity.is_some()).count();
        if debris > MOST_DEBRIS {
            let mut extra = debris - MOST_DEBRIS;
            self.leaves.retain(|l| {
                let drop = extra > 0 && l.velocity.is_some();
                if drop {
                    extra -= 1;
                }
                !drop
            });
        }
    }

    /// Which way the wind blows now, slowly turning, in voxels per second
    fn wind(&self, vpm: f64) -> Vector3<f64> {
        let angle = 0.7 + 0.6 * (self.time * 0.05).sin();
        let strength = WIND * (0.7 + 0.3 * (self.time * 0.13).sin());
        Vector3::new(angle.cos(), angle.sin(), 0.0) * strength * vpm
    }

    pub fn update(&mut self, world: &World, water: WaterLevel, camera: Vector3<f64>, dt: f64) {
        let vpm = world.voxels_per_meter as f64;
        self.time += dt;

        // Trees near the camera let go of a leaf now and then. In a new world there are some
        // falling already.
        if self.fresh {
            self.fresh = false;
            for _ in 0..WARM_TRIES {
                if self.count() < MOST / 2 {
                    self.drop_leaf(world, camera, true);
                }
            }
        }
        self.tries += TRIES * dt;
        while self.tries >= 1.0 {
            self.tries -= 1.0;
            if self.count() < MOST {
                self.drop_leaf(world, camera, false);
            }
        }

        let wind = self.wind(vpm);
        let [sx, sy, _] = world.size.map(f64::from);
        for leaf in &mut self.leaves {
            if let Some(left) = &mut leaf.landed {
                *left -= dt;
                continue;
            }
            let next = match &mut leaf.velocity {
                // Debris flies, and falls, tumbling
                Some(velocity) => {
                    velocity.z -= 9.8 * vpm * dt;
                    let angle = leaf.spin.norm() * dt;
                    if angle > 1e-6 {
                        leaf.turn = UnitQuaternion::from_axis_angle(&nalgebra::Unit::new_normalize(leaf.spin), angle) * leaf.turn;
                    }
                    leaf.position + *velocity * dt
                }
                // Leaves flutter side to side as they fall, slower as they swing out
                None => {
                    leaf.phase += leaf.flutter * dt;
                    let swing = Vector3::new(leaf.phase.cos(), (leaf.phase * 0.7).sin(), 0.0) * leaf.swing * vpm;
                    let fall = FALL_SPEED * vpm * (0.75 + 0.35 * (leaf.phase * 2.0).cos().abs());
                    leaf.position + (wind + swing - Vector3::new(0.0, 0.0, fall)) * dt
                }
            };
            let (x, y, z) = (next.x.floor() as i64, next.y.floor() as i64, next.z.floor() as i64);
            let surface = water(next.x, next.y);
            if z < 0 || !is_passable(world.material_at(x, y, z)) {
                // A chunk falling fast bounces, skidding on, and spins less; once it's slow it
                // comes to rest, lying on top of what it hit
                if let Some(velocity) = &mut leaf.velocity
                    && velocity.z < -1.5 * vpm
                    && leaf.bounces < 2
                {
                    let solid_above = !is_passable(world.material_at(x, y, leaf.position.z.floor() as i64));
                    if !solid_above {
                        velocity.z *= -BOUNCE;
                        velocity.x *= SKID;
                        velocity.y *= SKID;
                        leaf.spin *= 0.5;
                        leaf.bounces += 1;
                        leaf.position.z = (z + 1) as f64;
                        continue;
                    }
                    // Hit the side of something: it stops going that way and drops
                    velocity.x *= -0.2;
                    velocity.y *= -0.2;
                    continue;
                }
                leaf.position = Vector3::new(leaf.position.x, leaf.position.y, (z + 1) as f64);
                let rest = if leaf.velocity.is_some() { DEBRIS_LIE } else { LIE };
                leaf.landed = Some(rest * (0.6 + 0.4 * (leaf.phase.sin() * 0.5 + 0.5)) + random(&mut self.rng) * 3.0 * leaf.velocity.is_some() as u8 as f64);
            } else if next.z < surface {
                leaf.position = Vector3::new(next.x, next.y, surface);
                leaf.landed = Some(LIE);
            } else {
                leaf.position = next;
            }
        }
        let range = RANGE * 1.5 * vpm;
        self.leaves.retain(|l| {
            l.landed.is_none_or(|left| left > 0.0)
                && (l.position - camera).xy().norm() < range
                && l.position.x >= 0.0
                && l.position.y >= 0.0
                && l.position.x < sx
                && l.position.y < sy
        });
    }

    /// Tries a random spot near the camera: if there's a tree crown over it, a leaf falls from
    /// underneath it. With `anywhere`, from anywhere on the way down.
    fn drop_leaf(&mut self, world: &World, camera: Vector3<f64>, anywhere: bool) {
        let vpm = world.voxels_per_meter as f64;
        let angle = random(&mut self.rng) * TAU;
        let distance = random(&mut self.rng).sqrt() * RANGE * vpm;
        let (x, y) = (camera.x + angle.cos() * distance, camera.y + angle.sin() * distance);
        let (xi, yi) = (x.floor() as i64, y.floor() as i64);
        if xi < 0 || yi < 0 || xi >= world.size[0] as i64 || yi >= world.size[1] as i64 {
            return;
        }
        // Down from well above the camera to the first leaves, then on through them to the air below
        let top = ((camera.z + 40.0 * vpm) as i64).min(world.size[2] as i64 - 1);
        // The shades of leaves on the way down, as a crown's are lighter on top and darker inside
        let mut shades = [AIR; 4];
        let mut seen = 0;
        for z in (0..=top).rev() {
            let material = world.material_at(xi, yi, z);
            if KINDS.contains(&material) {
                if !shades[..seen.min(4)].contains(&material) {
                    shades[seen % 4] = material;
                    seen += 1;
                }
                continue;
            }
            match seen {
                0 if !is_passable(material) => return,
                0 => {}
                _ => {
                    if !is_passable(material) {
                        // Leaves right on the ground, a bush, or the crown sits on its trunk
                        return;
                    }
                    let ground = (0..z).rev().find(|&g| !is_passable(world.material_at(xi, yi, g))).unwrap_or(0) as f64 + 1.0;
                    if (z as f64 - ground) < LOWEST * vpm {
                        // Ferns and flowers
                        return;
                    }
                    let kind = shades[(random(&mut self.rng) * seen.min(4) as f64) as usize];
                    let mut height = z as f64 + 0.9;
                    // Starting out, leaves are already on their way down, anywhere between the
                    // crown and the ground
                    if anywhere {
                        height = ground + (height - ground) * random(&mut self.rng);
                    }
                    let position = Vector3::new(x, y, height);
                    let flutter = 2.0 + 2.5 * random(&mut self.rng);
                    let swing = 0.15 + 0.35 * random(&mut self.rng);
                    let phase = random(&mut self.rng) * TAU;
                    self.leaves.push(Leaf { position, material: kind, phase, flutter, swing, velocity: None, landed: None, size: 1.0, turn: UnitQuaternion::identity(), spin: Vector3::zeros(), bounces: 0 });
                    return;
                }
            }
        }
    }

    /// What to draw, `first_model` being where the leaf models are in the buffer of all models:
    /// what explosions blew out first, as there's less room for them than for the leaves
    pub fn instances(&self, first_model: u32, vpm: f64) -> Vec<AnimalInstance> {
        let debris = self.leaves.iter().filter(|l| l.velocity.is_some());
        let leaves = self.leaves.iter().filter(|l| l.velocity.is_none());
        debris
            .chain(leaves)
            .map(|leaf| {
                // Shrinking away at the end
                let shrink = leaf.landed.map_or(1.0, |left| left.clamp(0.0, 1.0));
                let kind = if leaf.landed.is_some() { STILL } else { MOVING };
                let scale = vpm / RES * leaf.size * shrink;
                // Turned about its middle rather than the middle of its bottom
                let middle = leaf.turn * Vector3::new(0.0, 0.0, 0.5 / RES * leaf.size * shrink * vpm);
                let at = leaf.position + Vector3::new(0.0, 0.0, 0.5 / RES * leaf.size * vpm) - middle;
                AnimalInstance::new(first_model + leaf.material as u32, at, leaf.turn, scale, vpm / RES * 1.5 * leaf.size, kind)
            })
            .collect()
    }
}
