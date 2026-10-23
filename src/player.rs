use nalgebra::{Vector2, Vector3};

use crate::worldgen::{World, is_passable, is_water};

// Sizes and speeds in meters, scaled by the world's voxels per meter
const WIDTH: f64 = 0.6;
const HEIGHT: f64 = 1.75;
const EYE_HEIGHT: f64 = 1.6;
const CHEST_HEIGHT: f64 = 1.1;
const STEP_HEIGHT: f64 = 0.6;
const WALK_SPEED: f64 = 4.5;
const SPRINT_SPEED: f64 = 9.0;
const SWIM_SPEED: f64 = 3.0;
const JUMP_SPEED: f64 = 6.5;
const GRAVITY: f64 = 22.0;

/// Whether a material stops the player. Plants and water don't.
fn blocks(material: u8) -> bool {
    !is_passable(material)
}

/// Height of the top of the ground in a column, or None if it's under water.
fn dry_ground(world: &World, x: f64, y: f64) -> Option<f64> {
    let (x, y) = (x.floor() as i64, y.floor() as i64);
    for z in (0..world.size[2] as i64).rev() {
        let material = world.material_at(x, y, z);
        if is_water(material) {
            return None;
        }
        if blocks(material) {
            return Some(z as f64 + 1.0);
        }
    }
    None
}

/// Walking on foot: gravity, jumping, swimming and collision with the voxels.
/// Positions and speeds are in voxels.
pub struct Player {
    /// Center of the soles of the feet
    pub position: Vector3<f64>,
    velocity: Vector3<f64>,
    on_ground: bool,
    /// Negative for a moment after stepping up a ledge, so the view glides up instead of jumping
    eye_offset: f64,
}

impl Player {
    /// Puts the player's eyes at `eye`, moved up out of anything solid.
    pub fn new(eye: Vector3<f64>, world: &World) -> Self {
        let vpm = world.voxels_per_meter as f64;
        let mut player = Player {
            position: eye - Vector3::new(0.0, 0.0, EYE_HEIGHT * vpm),
            velocity: Vector3::zeros(),
            on_ground: false,
            eye_offset: 0.0,
        };
        while player.collides(world, player.position) && player.position.z < world.size[2] as f64 {
            player.position.z += 1.0;
        }
        player
    }

    /// Starts on the dry ground nearest to `near`, searching outwards in rings a meter apart.
    pub fn on_dry_ground(near: Vector3<f64>, world: &World) -> Self {
        let vpm = world.voxels_per_meter as f64;
        for ring in 0..100 {
            let count = (ring * 6).max(1);
            for i in 0..count {
                let angle = i as f64 / count as f64 * std::f64::consts::TAU;
                let x = near.x + angle.cos() * ring as f64 * vpm;
                let y = near.y + angle.sin() * ring as f64 * vpm;
                if let Some(ground) = dry_ground(world, x, y) {
                    return Player::new(Vector3::new(x, y, ground + EYE_HEIGHT * vpm), world);
                }
            }
        }
        Player::new(near, world)
    }

    pub fn eye(&self, world: &World) -> Vector3<f64> {
        let vpm = world.voxels_per_meter as f64;
        self.position + Vector3::new(0.0, 0.0, EYE_HEIGHT * vpm + self.eye_offset)
    }

    /// `wish` is the direction to walk in on the ground, up to length 1.
    pub fn update(&mut self, world: &World, wish: Vector2<f64>, jump: bool, sprint: bool, dt: f64) {
        let vpm = world.voxels_per_meter as f64;
        let chest = self.position + Vector3::new(0.0, 0.0, CHEST_HEIGHT * vpm);
        let swimming = is_water(world.material_at(chest.x as i64, chest.y as i64, chest.z as i64));

        let speed = if swimming {
            SWIM_SPEED
        } else if sprint {
            SPRINT_SPEED
        } else {
            WALK_SPEED
        };
        self.velocity.x = wish.x * speed * vpm;
        self.velocity.y = wish.y * speed * vpm;

        if swimming {
            // Sink slowly, swim up with jump
            self.velocity.z -= GRAVITY * 0.15 * vpm * dt;
            self.velocity.z *= (-2.0 * dt).exp();
            if jump {
                self.velocity.z = SWIM_SPEED * vpm;
            }
        } else {
            self.velocity.z -= GRAVITY * vpm * dt;
            if jump && self.on_ground {
                self.velocity.z = JUMP_SPEED * vpm;
            }
        }

        let step_up = if self.on_ground || swimming { (STEP_HEIGHT * vpm).ceil() as i32 } else { 0 };
        self.move_horizontally(world, 0, self.velocity.x * dt, step_up);
        self.move_horizontally(world, 1, self.velocity.y * dt, step_up);

        let falling = self.velocity.z <= 0.0;
        let stopped = self.move_vertically(world, self.velocity.z * dt);
        if stopped {
            self.velocity.z = 0.0;
        }
        self.on_ground = stopped && falling;

        self.eye_offset *= (-12.0 * dt).exp();
        if self.eye_offset.abs() < 0.01 {
            self.eye_offset = 0.0;
        }
    }

    // Moves in steps of at most half a voxel so falling fast can't skip through thin floors.
    // Returns whether something was in the way.
    fn move_vertically(&mut self, world: &World, distance: f64) -> bool {
        let steps = (distance.abs() / 0.5).ceil().max(1.0) as usize;
        for _ in 0..steps {
            let mut next = self.position;
            next.z += distance / steps as f64;
            if self.collides(world, next) {
                return true;
            }
            self.position = next;
        }
        false
    }

    // Like move_vertically, but climbs ledges up to `step_up` voxels high
    fn move_horizontally(&mut self, world: &World, axis: usize, distance: f64, step_up: i32) {
        let steps = (distance.abs() / 0.5).ceil().max(1.0) as usize;
        for _ in 0..steps {
            let mut next = self.position;
            next[axis] += distance / steps as f64;
            if !self.collides(world, next) {
                self.position = next;
                continue;
            }

            let climb = (1..=step_up).map(f64::from).find(|&lift| {
                let mut raised = next;
                raised.z += lift;
                !self.collides(world, raised)
            });
            match climb {
                Some(lift) => {
                    next.z += lift;
                    self.position = next;
                    self.eye_offset -= lift;
                }
                None => {
                    self.velocity[axis] = 0.0;
                    return;
                }
            }
        }
    }

    fn collides(&self, world: &World, position: Vector3<f64>) -> bool {
        let vpm = world.voxels_per_meter as f64;
        let half_width = WIDTH * vpm / 2.0;
        let min = position - Vector3::new(half_width, half_width, 0.0);
        let max = position + Vector3::new(half_width, half_width, HEIGHT * vpm);

        // Invisible walls around the world and solid ground below it
        let [sx, sy, _] = world.size.map(f64::from);
        if min.x < 0.0 || min.y < 0.0 || max.x > sx || max.y > sy || min.z < 0.0 {
            return true;
        }

        let range = |lo: f64, hi: f64| (lo.floor() as i64)..=((hi - 1e-6).floor() as i64);
        for z in range(min.z, max.z) {
            for y in range(min.y, max.y) {
                for x in range(min.x, max.x) {
                    if blocks(world.material_at(x, y, z)) {
                        return true;
                    }
                }
            }
        }
        false
    }
}
