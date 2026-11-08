//! Getting about the sky isles: the grappling hook, thrown up to realms::HOOK_REACH_M meters to
//! bite into rock or earth and haul the player there, up over the edge if it's an edge, wings to
//! glide on, see Player::gliding, and the winds bringing the player back to the last island they
//! stood safe on when they fall below the islands, behind a short fade rather than a death.

use crate::animals::Look;
use crate::game::Item;
use crate::realms;
use crate::worldgen::Land;

use super::*;

/// How fast the hook flies out, and comes back from a throw at nothing, and how fast its rope
/// hauls the player, in meters a second
const THROW_SPEED: f64 = 90.0;
const HAUL_SPEED: f64 = 17.0;
/// The most seconds the rope hauls before it gives, and how near where it's hauling them to it
/// lets go, in meters
const HAUL_MOST: f64 = 3.5;
const HAUL_NEAR: f64 = 0.9;
/// How long one length of rope is, see Look::HookRope, in meters, and the most of them drawn
const ROPE_LENGTH: f64 = 1.5;
const ROPE_MOST: usize = 30;
/// How far below the lowest of the sky isles' undersides the player falls before the winds
/// bring them back, in meters over the sea far below, and how long the fade each way takes
const SKY_FLOOR_M: f64 = 9.0;
const FALL_FADE: f32 = 0.35;

/// A grappling hook thrown, see App::throw_hook()
pub struct Hook {
    /// Which way it's thrown from the eye, and how far it's flown out, in voxels
    dir: Vector3<f64>,
    out: f64,
    /// Where it bites, in voxels of the world, and where its rope hauls the player's feet to: over
    /// the edge if it bit into an edge, by way of a point out past the edge and over it so they
    /// don't come up under its rim, just short of it otherwise. None thrown at nothing.
    bite: Option<(Vector3<f64>, Vector3<f64>, Option<Vector3<f64>>)>,
    /// Seconds it's been hauling, once it's bitten
    hauling: Option<f64>,
    /// Coming back from a throw at nothing, or let go of
    returning: bool,
}

/// The winds bringing the player back up after a fall below the sky isles, see App::update_fall():
/// since when, and whether they've been brought back yet, the fade lifting
pub struct FallBack {
    since: Instant,
    brought: bool,
}

impl App {
    /// The land of the world the player's in
    fn land(&self) -> Land {
        worldgen::land_of(self.terrain.seed)
    }

    /// Throws the grappling hook the way the camera looks: to the first rock or earth it comes to
    /// within its reach, where its rope hauls the player, or out to its reach and back if there's
    /// nothing there to bite into
    pub(super) fn throw_hook(&mut self) {
        if self.hook.is_some() || self.player.is_none() || self.held_still() {
            return;
        }
        let vpm = self.world.voxels_per_meter as f64;
        let eye = camera_to_voxels(self.camera.position, self.world.size);
        let dir = (self.camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz().normalize();
        let reach = realms::HOOK_REACH_M as f64 * vpm;
        // Only into what's there in the world: out past its sides or down under everything there's
        // nothing to bite into, whatever's said to be beyond them
        let [sx, sy, _] = self.world.size.map(f64::from);
        let inside = |p: &Vector3<f64>| p.x >= 0.0 && p.y >= 0.0 && p.z >= 1.0 && p.x < sx && p.y < sy;
        let bite = (2..(reach * 2.0) as usize)
            .map(|i| eye + dir * (i as f64 * 0.5))
            .take_while(inside)
            .find(|p| !worldgen::is_passable(self.world.material_at(p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64)))
            .map(|at| {
                let (target, by) = self.haul_to(at, dir);
                (at, target, by)
            });
        let view = self.view();
        if let Some(sound) = &mut self.audio {
            sound.play(audio::Cue::here(audio::Sound::Dodge, 0.7).pitched(1.3), &view);
        }
        self.hook = Some(Hook { dir, out: 0.0, bite, hauling: None, returning: false });
    }

    /// Where a hook biting at `at`, thrown along `dir`, hauls the player's feet to: up onto the top
    /// of what it bit into, a little way in, if that's within a few meters over it and there's room
    /// to stand there, as at an island's edge; otherwise just short of it, under it or against it
    fn haul_to(&self, at: Vector3<f64>, dir: Vector3<f64>) -> (Vector3<f64>, Option<Vector3<f64>>) {
        let vpm = self.world.voxels_per_meter as f64;
        let solid = |p: Vector3<f64>| !worldgen::is_passable(self.world.material_at(p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64));
        let flat = Vector3::new(dir.x, dir.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros);
        let over = at + flat * 1.2 * vpm;
        let top = (0..(3.5 * vpm) as i64).map(|k| over + Vector3::new(0.0, 0.0, k as f64)).find(|p| !solid(*p) && !solid(*p + Vector3::new(0.0, 0.0, 1.0)));
        if let Some(top) = top
            && (1..(1.8 * vpm) as i64).all(|k| !solid(top + Vector3::new(0.0, 0.0, k as f64)))
        {
            let target = Vector3::new(top.x, top.y, top.z.floor() + 0.5);
            return (target, Some(target - flat * 2.2 * vpm + Vector3::new(0.0, 0.0, 0.8 * vpm)));
        }
        (at - dir * 0.8 * vpm - Vector3::new(0.0, 0.0, 1.75 * vpm), None)
    }

    /// The hook flying out, hauling the player along its rope once it's bitten, or coming back:
    /// before the player moves each frame. Jump lets go of it.
    pub(super) fn update_hook(&mut self, dt: f64, let_go: bool) {
        if self.hook.is_none() {
            return;
        }
        if self.held_still() || self.game.dead() || self.menu_open || self.player.is_none() {
            self.hook = None;
            return;
        }
        let view = self.view();
        let vpm = self.world.voxels_per_meter as f64;
        let eye = view.eye;
        let (Some(hook), Some(player)) = (&mut self.hook, &mut self.player) else {
            return;
        };
        if hook.returning {
            hook.out -= THROW_SPEED * 1.5 * vpm * dt;
            if hook.out <= 0.0 {
                self.hook = None;
            }
            return;
        }
        let Some((at, target, by)) = hook.bite else {
            hook.out += THROW_SPEED * vpm * dt;
            hook.returning = hook.out >= realms::HOOK_REACH_M as f64 * vpm;
            return;
        };
        match &mut hook.hauling {
            None => {
                hook.out += THROW_SPEED * vpm * dt;
                if hook.out >= (at - eye).norm() {
                    hook.hauling = Some(0.0);
                    if let Some(sound) = &mut self.audio {
                        sound.play(audio::Cue::at(audio::Sound::Taut, at, 0.8), &view);
                    }
                }
            }
            Some(t) => {
                *t += dt;
                // Out past the edge and up over it first, then in over it
                if let Some(point) = by
                    && (point - player.position).norm() < HAUL_NEAR * vpm
                {
                    hook.bite = Some((at, target, None));
                }
                let goal = by.unwrap_or(target);
                let to = goal - player.position;
                let d = to.norm();
                if by.is_none() && d < HAUL_NEAR * vpm || *t > HAUL_MOST || let_go {
                    // Stepping on in over the edge with a little hop, or let go of in the air
                    if d < 2.0 * vpm {
                        let flat = Vector3::new(to.x, to.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros);
                        player.push(flat * 2.0 * vpm + Vector3::new(0.0, 0.0, 2.5 * vpm));
                    }
                    hook.returning = true;
                    hook.out = (at - eye).norm();
                    hook.bite = None;
                } else {
                    // Slowing as they get there, so it doesn't fling them past
                    let speed = (HAUL_SPEED * vpm).min(d * 6.0).max(4.0 * vpm);
                    player.haul(to / d * speed);
                }
            }
        }
    }

    /// The rope from the player's hand to the hook, and the hook's head, and the wings spread on
    /// their back while they glide
    pub(super) fn rope_instances(&self, view: &combat::View) -> Vec<AnimalInstance> {
        let mut out = Vec::new();
        let vpm = self.world.voxels_per_meter;
        let m = vpm as f64;
        let eye = view.eye;
        let yaw = self.camera.rotation.z;
        let forward = Vector3::new(-yaw.sin(), yaw.cos(), 0.0);
        let right = Vector3::new(yaw.cos(), yaw.sin(), 0.0);
        if let Some(player) = &self.player
            && player.gliding
            && !player.on_ground()
        {
            let back = player.position + Vector3::new(0.0, 0.0, 0.75 * m) - forward * 0.35 * m;
            out.push(animals::moving_instance(Look::Wings, back, UnitQuaternion::from_axis_angle(&Vector3::z_axis(), forward.y.atan2(forward.x)), vpm));
        }
        let Some(hook) = &self.hook else {
            return out;
        };
        // From the hand held out ahead, far enough from the eye that the rope's no thicker to it
        // than it is
        let hand = eye + right * 0.45 * m - Vector3::new(0.0, 0.0, 0.55 * m) + forward * 1.4 * m;
        let head = match hook.bite {
            Some((at, ..)) if hook.hauling.is_some() => at,
            _ => eye + hook.dir * hook.out,
        };
        let line = head - hand;
        let length = line.norm();
        if length < 0.3 * m {
            return out;
        }
        let along = line / length;
        let turn = UnitQuaternion::rotation_between(&Vector3::x(), &along).unwrap_or_else(|| UnitQuaternion::from_axis_angle(&Vector3::y_axis(), std::f64::consts::PI));
        // The rope's axis is a little over the foot of its model, see Look::HookRope
        let lift = turn * Vector3::new(0.0, 0.0, 0.03 * m);
        let pieces = ((length / (ROPE_LENGTH * m)).ceil() as usize).clamp(1, ROPE_MOST);
        let piece = length / pieces as f64;
        for k in 0..pieces {
            let middle = hand + along * piece * (k as f64 + 0.5);
            out.push(animals::moving_instance(Look::HookRope, middle - lift, turn, vpm));
        }
        out.push(animals::moving_instance(Look::GrappleHook, head - turn * Vector3::new(0.0, 0.0, 0.14 * m), turn, vpm));
        out
    }

    /// In the sky isles, a fall below the islands brings the player back up to the last island they
    /// stood safe on, or to the portal home, behind a short fade
    pub(super) fn update_fall(&mut self) {
        if self.land() != Land::Sky || self.held_still() {
            self.fall_back = None;
            return;
        }
        let vpm = self.world.voxels_per_meter as f64;
        if self.fall_back.is_none()
            && let Some(player) = &self.player
            && player.position.z < SKY_FLOOR_M * vpm
        {
            self.fall_back = Some(FallBack { since: Instant::now(), brought: false });
            self.hook = None;
        }
        let Some(fall) = &mut self.fall_back else {
            return;
        };
        let t = fall.since.elapsed().as_secs_f32();
        if !fall.brought && t >= FALL_FADE {
            fall.brought = true;
            let water = &self.water;
            let spot = self.game.rest_spot();
            let landed = match spot {
                Some(feet) => Player::new(feet + Vector3::new(0.0, 0.0, 1.6 * vpm), &self.world),
                None => {
                    // Never yet stood safe here: back by the portal home
                    let arrival = self.session.places.as_ref().map_or([0.0; 2], |p| p.arrival);
                    let at = Vector3::new(arrival[0] * vpm - self.world.origin[0] as f64, arrival[1] * vpm - self.world.origin[1] as f64, self.world.size[2] as f64 - 2.0 * vpm);
                    Player::on_dry_ground(at, &self.world, &|x, y| water.surface_at(x, y))
                }
            };
            self.camera.position = voxels_to_camera(landed.eye(&self.world), self.world.size);
            self.player = Some(landed);
            self.last_feet = None;
            self.game.say("The winds catch you and bear you back to solid ground.");
            println!("FELL below the sky isles, brought back {}", if spot.is_some() { "to where they last stood safe" } else { "to the portal home" });
        }
        if fall.brought && t >= 2.0 * FALL_FADE {
            self.fall_back = None;
        }
    }

    /// How dark the screen is as the winds bring the player back up, 0 to 1
    pub(super) fn fall_dark(&self) -> f32 {
        self.fall_back.as_ref().map_or(0.0, |f| {
            let t = f.since.elapsed().as_secs_f32();
            if t < FALL_FADE { t / FALL_FADE } else { (2.0 - t / FALL_FADE).clamp(0.0, 1.0) }
        })
    }

    /// Whether the player's gliding: wings carried, jump held in the air
    pub(super) fn gliding(&self, jump: bool) -> bool {
        jump && self.player.as_ref().is_some_and(|p| !p.on_ground()) && self.game.carries(Item::Wings)
    }

    /// For trying the hook out: the camera turned to the edge of the nearest other sky isle, as
    /// seen from where the player stands, and the hook thrown at it
    pub(super) fn hook_nearest_island(&mut self) {
        let Some(player) = &self.player else {
            return;
        };
        let vpm = self.world.voxels_per_meter as f64;
        let seed = self.terrain.seed;
        let feet = (player.position + Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0)) / vpm;
        let (cx, cy) = ((feet.x / 46.0).floor() as i32, (feet.y / 46.0).floor() as i32);
        let here = worldgen::sky_island(seed, feet.x as f32, feet.y as f32).map(|(top, _)| top);
        // The nearest other island's middle
        let mut best: Option<([f32; 2], f32, f32)> = None;
        for j in cy - 2..=cy + 2 {
            for i in cx - 2..=cx + 2 {
                let (middle, radius, _) = worldgen::sky_isle(seed, i, j);
                let d = (middle[0] as f64 - feet.x).hypot(middle[1] as f64 - feet.y);
                if d > radius as f64 + 2.0 && best.is_none_or(|(m, r, _)| d - (radius as f64) < (m[0] as f64 - feet.x).hypot(m[1] as f64 - feet.y) - r as f64) {
                    best = Some((middle, radius, 0.0));
                }
            }
        }
        let Some((middle, _, _)) = best else {
            return;
        };
        // Along the way to it, the first of its ground
        let to = Vector3::new(middle[0] as f64 - feet.x, middle[1] as f64 - feet.y, 0.0).normalize();
        let edge = (0..400).map(|k| feet + to * (k as f64 * 0.25)).find(|p| {
            let island = worldgen::sky_island(seed, p.x as f32, p.y as f32);
            island.is_some_and(|(top, _)| here.is_none_or(|h| (top - h).abs() > 0.5 || (p - feet).norm() > 20.0)) && (p - feet).xy().norm() > 6.0
        });
        let Some(edge) = edge else {
            return;
        };
        // At the ground a little way in from the edge, rather than the very rim, which a throw can
        // graze past
        let edge = edge + to * 1.5;
        let (top, _) = worldgen::sky_island(seed, edge.x as f32, edge.y as f32).unwrap_or((edge.z as f32, 0.0));
        let target = Vector3::new(edge.x, edge.y, top as f64 - 0.3) * vpm - Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        self.camera.look_at(voxels_to_camera(target, self.world.size));
        println!("HOOK thrown at the edge of the island at {:.0}, {:.0}, {:.1} m off", middle[0], middle[1], (edge - feet).xy().norm());
        self.throw_hook();
    }
}
