//! Leaves drifting down from the trees around the camera, a single voxel each, as in John Lin's
//! engine. They flutter as they fall, carried along by a slow wind, settle where they land, on the
//! ground or floating on water, and fade away after a while. Debris thrown out by explosions flies
//! and lands the same way. Positions are in voxels of the main world.

use std::f64::consts::TAU;

use nalgebra::{UnitQuaternion, Vector3};

use crate::animals::{AnimalInstance, MODEL_RES, MOVING, Model, STILL, cuboid, voxelize};
use crate::player::WaterLevel;
use crate::worldgen::{World, is_passable, material::*};

/// Leaves that fall
const KINDS: [u8; 7] = [LEAVES, LEAVES_DARK, LEAVES_YELLOW, LEAVES_ORANGE, LEAVES_RED, BLOSSOM, WILLOW];
/// One model of a single voxel for each material, see Leaves::models
const MATERIALS: u8 = GLOW_HOLY + 1;
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
}

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

    /// Bits of what an explosion at `at` blew out, `materials`, thrown out in all directions
    pub fn debris(&mut self, at: Vector3<f64>, materials: &[u8], vpm: f64) {
        for &material in materials {
            if material == AIR || material >= MATERIALS {
                continue;
            }
            let angle = random(&mut self.rng) * TAU;
            let out = 1.5 + 4.0 * random(&mut self.rng);
            let up = 3.0 + 5.0 * random(&mut self.rng);
            let velocity = Vector3::new(angle.cos() * out, angle.sin() * out, up) * vpm;
            let position = at + Vector3::new(angle.cos(), angle.sin(), 0.5) * 0.3 * vpm;
            self.leaves.push(Leaf { position, material, phase: 0.0, flutter: 0.0, swing: 0.0, velocity: Some(velocity), landed: None });
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
                // Debris flies, and falls
                Some(velocity) => {
                    velocity.z -= 9.8 * vpm * dt;
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
                // Landed on something solid, lying on top of it
                leaf.position = Vector3::new(leaf.position.x, leaf.position.y, (z + 1) as f64);
                leaf.landed = Some(LIE * (0.6 + 0.4 * (leaf.phase.sin() * 0.5 + 0.5)));
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
        let mut in_leaves = None;
        for z in (0..=top).rev() {
            let material = world.material_at(xi, yi, z);
            if KINDS.contains(&material) {
                in_leaves = Some(material);
                continue;
            }
            match in_leaves {
                Some(kind) => {
                    if !is_passable(material) {
                        // Leaves right on the ground, a bush, or the crown sits on its trunk
                        return;
                    }
                    let mut height = z as f64 + 0.9;
                    // Starting out, leaves are already on their way down, anywhere between the
                    // crown and the ground
                    if anywhere {
                        let ground = (0..z).rev().find(|&g| !is_passable(world.material_at(xi, yi, g))).unwrap_or(0) as f64 + 1.0;
                        height = ground + (height - ground) * random(&mut self.rng);
                    }
                    let position = Vector3::new(x, y, height);
                    let flutter = 2.0 + 2.5 * random(&mut self.rng);
                    let swing = 0.15 + 0.35 * random(&mut self.rng);
                    let phase = random(&mut self.rng) * TAU;
                    self.leaves.push(Leaf { position, material: kind, phase, flutter, swing, velocity: None, landed: None });
                    return;
                }
                None if !is_passable(material) => return,
                None => {}
            }
        }
    }

    /// What to draw, `first_model` being where the leaf models are in the buffer of all models
    pub fn instances(&self, first_model: u32, vpm: f64) -> Vec<AnimalInstance> {
        self.leaves
            .iter()
            .map(|leaf| {
                // Shrinking away at the end
                let size = leaf.landed.map_or(1.0, |left| left.clamp(0.0, 1.0));
                let kind = if leaf.landed.is_some() { STILL } else { MOVING };
                AnimalInstance::new(first_model + leaf.material as u32, leaf.position, UnitQuaternion::identity(), vpm / RES * size, vpm / RES * 1.5, kind)
            })
            .collect()
    }
}
