use nalgebra::{Vector2, Vector3};

use crate::worldgen::{World, is_passable};

/// The top of the water at a point in voxels, NEG_INFINITY where it's dry, see water::Water::surface_at
pub type WaterLevel<'a> = &'a dyn Fn(f64, f64) -> f64;

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
/// Gliding on wings, unless they say otherwise, see Player::glide_feel: how fast the player goes on
/// ahead and sinks at most, in meters a second, and how quickly they come round to where they steer
const GLIDE: [f64; 3] = [11.0, 2.2, 4.0];

/// Flying on wings that truly fly, see Player::fly(): a little either side of level, in radians,
/// the way flown is level; the steepest it climbs and dives, however far up or down they look
const LEVEL: f64 = 0.08;
const STEEPEST_CLIMB: f64 = 0.6;
const STEEPEST_DIVE: f64 = 1.05;
/// How much of gravity's pull along the way flown speeds a dive or slows a climb, and how quickly
/// flying level comes to the wings' cruising speed, the share of the way a second
const PULL: f64 = 0.75;
const SPEED_UP: f64 = 0.6;
/// The slowest they fly, beating hard, in meters a second, and how fast they're going at the first
/// beat off the ground at least
const SLOWEST: f64 = 3.0;
const TAKE_OFF: f64 = 5.0;
/// How much of their glide's sinking they sink at their cruising speed at the least, sinking the
/// more the slower they go, to half again as much
const SINK_AT_CRUISE: f64 = 0.6;
/// How quickly a beat's lift dies away, the share of it a second, as e^-x
pub const LIFT_FADE: f64 = 3.0;
/// How far over the ground, in meters, coming down to it they flare, slowing and easing down onto
/// it, and how fast they touch down at most
const FLARE: f64 = 4.0;
const TOUCH_DOWN: f64 = 0.8;
/// How far under the top of the world, in meters, the feet go at most: the sky's limit, as high as
/// the world's built
pub const CEILING: f64 = 5.0;

/// How the wings worn fly, while the player's up on them, see wings::Lift: how fast flying level
/// comes to and the fastest a dive goes, how fast they sink at most gliding, all in meters a
/// second, and how quickly they come round to where the player looks
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Soaring {
    pub cruise: f64,
    pub dive: f64,
    pub sink: f64,
    pub turn: f64,
}

/// Whether a material stops the player. Plants and water don't.
fn blocks(material: u8) -> bool {
    !is_passable(material)
}

/// Height of the top of the ground in a column, or None if it's under water.
fn dry_ground(world: &World, water: WaterLevel, x: f64, y: f64) -> Option<f64> {
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    for z in (0..world.size[2] as i64).rev() {
        if blocks(world.material_at(xi, yi, z)) {
            let top = z as f64 + 1.0;
            return (water(x, y) <= top).then_some(top);
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
    /// How fast blows and dashes are throwing the player sideways, in voxels a second, on top of
    /// walking, dying away quickly on the ground and slowly in the air
    shove: Vector2<f64>,
    /// Pulled along a grappling hook's rope this frame, this fast in voxels a second, see haul()
    hauled: Option<Vector3<f64>>,
    /// Gliding on wings, falling slowly and going on ahead, while it's set and they're in the air
    pub gliding: bool,
    /// How the wings worn glide: how fast on ahead and how fast they sink at most, in meters a
    /// second, and how quickly they come round to where the player steers, see wings::WingKind
    pub glide_feel: [f64; 3],
    /// How fast they're going along while gliding, in voxels a second, for coming round gradually
    glide_velocity: Vector2<f64>,
    /// Flying on wings that truly fly while it's set and they're in the air, steering by where
    /// they look, `look`, see fly()
    pub flight: Option<Soaring>,
    pub look: Vector3<f64>,
    /// How fast they're going along the way they fly, in voxels a second, 0 till they're flying
    airspeed: f64,
    /// How fast the last beats of the wings are lifting them, in voxels a second, dying away
    lift: f64,
    /// How hard they're flaring to touch down, 0 to 1, and whether their chest is under water
    flare: f64,
    swimming: bool,
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
            shove: Vector2::zeros(),
            hauled: None,
            gliding: false,
            glide_feel: GLIDE,
            glide_velocity: Vector2::zeros(),
            flight: None,
            look: Vector3::y(),
            airspeed: 0.0,
            lift: 0.0,
            flare: 0.0,
            swimming: false,
        };
        while player.collides(world, player.position) && player.position.z < world.size[2] as f64 {
            player.position.z += 1.0;
        }
        player
    }

    /// Starts on the dry ground nearest to `near`, searching outwards in rings a meter apart.
    pub fn on_dry_ground(near: Vector3<f64>, world: &World, water: WaterLevel) -> Self {
        let vpm = world.voxels_per_meter as f64;
        for ring in 0..100 {
            let count = (ring * 6).max(1);
            for i in 0..count {
                let angle = i as f64 / count as f64 * std::f64::consts::TAU;
                let x = near.x + angle.cos() * ring as f64 * vpm;
                let y = near.y + angle.sin() * ring as f64 * vpm;
                if let Some(ground) = dry_ground(world, water, x, y) {
                    return Player::new(Vector3::new(x, y, ground + EYE_HEIGHT * vpm), world);
                }
            }
        }
        Player::new(near, world)
    }

    pub fn on_ground(&self) -> bool {
        self.on_ground
    }

    /// Thrown by a blow, `velocity` voxels a second more
    pub fn push(&mut self, velocity: Vector3<f64>) {
        self.shove += velocity.xy();
        self.velocity.z += velocity.z;
        if velocity.z > 0.0 {
            self.on_ground = false;
        }
    }

    /// Pulled along by a rope this frame at `velocity` voxels a second, gravity let be: what's in
    /// the way still stops them
    pub fn haul(&mut self, velocity: Vector3<f64>) {
        self.hauled = Some(velocity);
    }

    /// How fast they're going, in voxels a second
    pub fn velocity(&self) -> Vector3<f64> {
        self.velocity
    }

    /// A beat of the wings in the air, lifting the player at lift voxels a second
    pub fn flap(&mut self, lift: f64) {
        self.velocity.z = self.velocity.z.max(lift);
        self.on_ground = false;
    }

    /// A beat of wings that truly fly, see fly(): lift that dies away, so that beating every
    /// `every` seconds climbs `climb` voxels a second more than gliding would, about
    pub fn beat(&mut self, climb: f64, every: f64) {
        self.lift = (self.lift + climb * every * LIFT_FADE).min(climb * 1.5);
        self.on_ground = false;
    }

    /// Whether their chest was under water last they moved
    pub fn swimming(&self) -> bool {
        self.swimming
    }

    /// How hard they're flaring to touch down, flying, 0 to 1
    pub fn flaring(&self) -> f64 {
        self.flare
    }

    /// How fast they're going along the way they fly, in voxels a second
    pub fn airspeed(&self) -> f64 {
        self.airspeed
    }

    pub fn eye(&self, world: &World) -> Vector3<f64> {
        let vpm = world.voxels_per_meter as f64;
        self.position + Vector3::new(0.0, 0.0, EYE_HEIGHT * vpm + self.eye_offset)
    }

    /// `wish` is the direction to walk in on the ground, up to length 1.
    pub fn update(&mut self, world: &World, water: WaterLevel, wish: Vector2<f64>, jump: bool, sprint: bool, dt: f64) {
        let vpm = world.voxels_per_meter as f64;
        // Hauled along a rope, as fast as it pulls, going nowhere else
        if let Some(velocity) = self.hauled.take() {
            self.velocity = velocity;
            self.shove = Vector2::zeros();
            self.move_horizontally(world, 0, velocity.x * dt, (STEP_HEIGHT * vpm).ceil() as i32);
            self.move_horizontally(world, 1, velocity.y * dt, (STEP_HEIGHT * vpm).ceil() as i32);
            if self.move_vertically(world, velocity.z * dt) {
                self.velocity.z = 0.0;
            }
            self.on_ground = false;
            return;
        }
        let chest = self.position + Vector3::new(0.0, 0.0, CHEST_HEIGHT * vpm);
        let swimming = chest.z < water(chest.x, chest.y);
        self.swimming = swimming;
        let flight = self.flight.filter(|_| !swimming && !self.on_ground);
        if flight.is_none() {
            self.airspeed = 0.0;
            self.lift = 0.0;
            self.flare = 0.0;
        }

        let speed = if swimming {
            SWIM_SPEED
        } else if sprint {
            SPRINT_SPEED
        } else {
            WALK_SPEED
        };
        self.velocity.x = wish.x * speed * vpm + self.shove.x;
        self.velocity.y = wish.y * speed * vpm + self.shove.y;
        self.shove *= (-(if self.on_ground { 10.0 } else { 1.0 }) * dt).exp();

        if swimming {
            // Sink slowly, swim up with jump
            self.velocity.z -= GRAVITY * 0.15 * vpm * dt;
            self.velocity.z *= (-2.0 * dt).exp();
            if jump {
                self.velocity.z = SWIM_SPEED * vpm;
            }
        } else if let Some(feel) = flight {
            self.fly(world, water, feel, wish, dt);
        } else {
            self.velocity.z -= GRAVITY * vpm * dt;
            if jump && self.on_ground {
                self.velocity.z = JUMP_SPEED * vpm;
            }
            // Wings spread, falling slowly and sailing on ahead, faster than running, from the top
            // of a jump on, or a flap, coming round to where they steer as quickly as the wings
            // let them
            let [glide_speed, sink, turn] = self.glide_feel;
            let sailing = self.glide_velocity != Vector2::zeros();
            if self.gliding && !self.on_ground && (self.velocity.z < 0.0 || sailing) {
                if self.velocity.z < 0.0 {
                    self.velocity.z = self.velocity.z.max(-sink * vpm);
                }
                if !sailing {
                    self.glide_velocity = self.velocity.xy() - self.shove;
                }
                let ahead = if wish.norm() > 0.1 { wish.normalize() } else { wish };
                self.glide_velocity += (ahead * glide_speed * vpm - self.glide_velocity) * (1.0 - (-turn * dt).exp());
                if self.glide_velocity == Vector2::zeros() {
                    self.glide_velocity.x = 1e-9;
                }
                self.velocity.x = self.glide_velocity.x + self.shove.x;
                self.velocity.y = self.glide_velocity.y + self.shove.y;
            } else {
                self.glide_velocity = Vector2::zeros();
            }
        }

        let step_up = if self.on_ground || swimming { (STEP_HEIGHT * vpm).ceil() as i32 } else { 0 };
        let from = self.position;
        self.move_horizontally(world, 0, self.velocity.x * dt, step_up);
        self.move_horizontally(world, 1, self.velocity.y * dt, step_up);
        // Flying into a wall, a tree or under a roof, they're stopped rather than pressing on
        if flight.is_some() {
            let made = (self.position - from).xy() / dt.max(1e-6);
            if made.norm() < self.glide_velocity.norm() * 0.5 {
                self.glide_velocity = made;
                self.airspeed = self.airspeed.min(made.norm()).max(SLOWEST * vpm);
            }
        }

        let falling = self.velocity.z <= 0.0;
        let stopped = self.move_vertically(world, self.velocity.z * dt);
        if stopped {
            self.velocity.z = 0.0;
        }
        self.on_ground = stopped && falling;
        // Touching down from flying they run on a step or two, rather than stopping dead
        if flight.is_some() && self.on_ground {
            self.shove += self.glide_velocity * 0.5;
            self.glide_velocity = Vector2::zeros();
        }

        self.eye_offset *= (-12.0 * dt).exp();
        if self.eye_offset.abs() < 0.01 {
            self.eye_offset = 0.0;
        }
    }

    /// Flying on wings that truly fly, `feel` as they do, steering by where the player looks: up
    /// climbs, spending speed, and down dives, gathering it, and flying level it comes to the
    /// wings' cruising speed. They sink the more the slower they go, and their beats lift them,
    /// see beat(). Holding back brakes. Coming down near the ground they flare and ease onto it,
    /// and they never go higher than the sky's limit, see CEILING.
    fn fly(&mut self, world: &World, water: WaterLevel, feel: Soaring, wish: Vector2<f64>, dt: f64) {
        let vpm = world.voxels_per_meter as f64;
        let heading = self.glide_velocity.try_normalize(1e-6).unwrap_or(Vector2::y());
        let ahead = self.look.xy().try_normalize(1e-6).unwrap_or(heading);
        let pitch = self.look.z.clamp(-1.0, 1.0).asin();
        let path = (pitch - LEVEL * pitch.signum()).clamp(-STEEPEST_DIVE, STEEPEST_CLIMB) * (pitch.abs() > LEVEL) as u8 as f64;
        if self.airspeed <= 0.0 {
            self.airspeed = (self.velocity.xy() - self.shove).norm().max(TAKE_OFF * vpm);
            self.glide_velocity = self.velocity.xy() - self.shove;
        }
        let cruise = feel.cruise * vpm;
        self.airspeed -= GRAVITY * PULL * path.sin() * vpm * dt;
        self.airspeed += (cruise - self.airspeed) * (1.0 - (-SPEED_UP * dt).exp());
        let slowest = SLOWEST * vpm;
        if wish.dot(&ahead) < -0.5 {
            self.airspeed += (slowest - self.airspeed) * (1.0 - (-1.5 * dt).exp());
        }
        // How far down the ground or the water is, as far as they'd flare from
        let reach = FLARE * vpm;
        let (x, y) = (self.position.x.floor() as i64, self.position.y.floor() as i64);
        let ground = (0..reach as i64).find(|&d| blocks(world.material_at(x, y, self.position.z.floor() as i64 - 1 - d))).map_or(reach, |d| d as f64);
        let clearance = ground.min(self.position.z - water(self.position.x, self.position.y)).max(0.0);
        let coming_down = self.velocity.z < 0.0;
        let flare = if coming_down { 1.0 - clearance / reach } else { 0.0 };
        self.flare += (flare - self.flare) * (1.0 - (-8.0 * dt).exp());
        if flare > 0.0 {
            self.airspeed += (slowest * 2.0 - self.airspeed).min(0.0) * (1.0 - (-1.5 * flare * dt).exp());
        }
        self.airspeed = self.airspeed.clamp(slowest, feel.dive * vpm);
        // Coming round to where they look
        let along = ahead * self.airspeed * path.cos();
        self.glide_velocity += (along - self.glide_velocity) * (1.0 - (-feel.turn * dt).exp());
        // Up and down: the way they're going, the lift of their beats, less what they sink
        let sink = feel.sink * (SINK_AT_CRUISE * cruise / self.airspeed).powi(2).clamp(SINK_AT_CRUISE, 1.5);
        let mut up = self.airspeed * path.sin() + self.lift - sink * vpm;
        if flare > 0.0 {
            up = up.max(-(TOUCH_DOWN + clearance / vpm * 0.8) * vpm);
        }
        let ceiling = world.size[2] as f64 - CEILING * vpm;
        up = up.min((ceiling - self.position.z) * 1.5);
        let ease = if flare > 0.0 { 12.0 } else { 6.0 };
        self.velocity.z += (up - self.velocity.z) * (1.0 - (-ease * dt).exp());
        if self.position.z >= ceiling {
            self.velocity.z = self.velocity.z.min(0.0);
        }
        self.lift *= (-LIFT_FADE * dt).exp();
        self.velocity.x = self.glide_velocity.x + self.shove.x;
        self.velocity.y = self.glide_velocity.y + self.shove.y;
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
