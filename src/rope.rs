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
/// How thick the hook's rope is drawn: as thick as a real one in the hand, in meters, out to this
/// many meters from the eye, and further off as thick as this many meters for every meter it is
/// from the eye, up to so thick, so it shows as a line a few pixels wide however far it reaches
const ROPE_THICK: f64 = 0.009;
const ROPE_NEAR_M: f64 = 4.0;
const ROPE_SPREAD: f64 = 0.0022;
const ROPE_THICKEST: f64 = 0.04;
/// How long the arm takes to swing forward throwing the hook, in seconds
const THROW_SWING: f64 = 0.22;
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
    /// When it was thrown, for the arm's swing
    thrown: Instant,
}

impl Hook {
    /// The world's corner moved on `shift` voxels over the landscape, see App::shift_everything():
    /// where the hook bites and its rope hauls to, counted from there, move with it, or it'd haul
    /// the player off past where it bit as the world moves on under them
    pub fn shift(&mut self, shift: Vector3<f64>) {
        if let Some((at, target, by)) = &mut self.bite {
            *at -= shift;
            *target -= shift;
            if let Some(by) = by {
                *by -= shift;
            }
        }
    }
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
        self.hook = Some(Hook { dir, out: 0.0, bite, hauling: None, returning: false, thrown: Instant::now() });
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
                    if self.capture.is_some() {
                        let why = if let_go { "let go" } else if *t > HAUL_MOST { "gave" } else { "got there" };
                        println!("HOOK haul ends after {:.2} s, {why}, {:.1} m short, at {:.1}, {:.1}, {:.1} m", *t, d / vpm, player.position.x / vpm, player.position.y / vpm, player.position.z / vpm);
                    }
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

    /// What the player holds that isn't a weapon, see Game::held_arm(): the grappling hook in hand,
    /// swung forward as it's thrown, its rope paying out from the hand to its head as it flies and
    /// hauls, sagging while it's slack; and wings to glide on, held up overhead by the bar under
    /// them, sweeping down past the edges of the view
    pub(super) fn rope_instances(&self, view: &combat::View) -> Vec<AnimalInstance> {
        let mut out = Vec::new();
        let vpm = self.world.voxels_per_meter;
        let m = vpm as f64;
        let eye = view.eye;
        // Right, ahead and up of the view, in meters, to where that is in the world
        let at = |v: [f64; 3]| eye + view.rotation * Vector3::new(v[0], v[1], v[2]) * m;
        let way = |v: [f64; 3]| (view.rotation * Vector3::new(v[0], v[1], v[2])).normalize();
        // A model's x turned to point along `along`
        let pointing = |along: Vector3<f64>| UnitQuaternion::rotation_between(&Vector3::x(), &along).unwrap_or_else(|| UnitQuaternion::from_axis_angle(&Vector3::z_axis(), std::f64::consts::PI));
        if self.player.as_ref().is_some_and(|p| p.gliding && !p.on_ground()) {
            // The wings over the head facing the way the view does, their bar under them in both
            // hands low in front, see the Wings model: its bar's middle is 0.22 m back and 0.15 m
            // up from where it stands
            let turn = view.rotation * UnitQuaternion::from_axis_angle(&Vector3::z_axis(), FRAC_PI_2);
            let bar = at([0.0, 0.55, -0.22]);
            out.push(animals::held_instance(Look::Wings, bar - turn * Vector3::new(-0.22, 0.0, 0.15) * m, turn, vpm));
            for (which, side) in [(0, 1.0), (1, -1.0)] {
                // Each hand on a strut just over the bar, the forearm down and out to the elbow
                let grip = at([0.25 * side, 0.56, -0.17]);
                out.push(self.game.held_arm(which, grip, way([-0.13 * side, 0.27, 0.58]), at([0.4 * side, 0.25, -0.47])));
            }
            return out;
        }
        let thrown = self.hook.as_ref();
        if thrown.is_none() && !self.game.holding_hook() {
            return out;
        }
        // The hand: low at the right holding the hook ready, swung forward and up as it's thrown
        let swing = thrown.map_or(0.0, |h| (h.thrown.elapsed().as_secs_f64() / THROW_SWING).min(1.0));
        let eased = swing * swing * (3.0 - 2.0 * swing);
        let lerp = |a: [f64; 3], b: [f64; 3]| [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * eased);
        let grip = at(lerp([0.3, 0.6, -0.3], [0.2, 0.85, -0.1]));
        let elbow = at(lerp([0.44, 0.22, -0.5], [0.3, 0.45, -0.32]));
        let Some(hook) = thrown else {
            // Ready to throw: the hook held up by its tail, its prongs forward and up, drawn half the
            // size it's made, which is to be seen flying out a long way off
            let axis = way([0.05, 0.6, 0.8]);
            out.push(self.game.held_arm(0, grip, axis, elbow));
            let turn = pointing(axis);
            out.push(animals::held_instance(Look::GrappleHook, grip + axis * 0.1 * m - turn * Vector3::new(0.0, 0.0, 0.11 * m), turn, vpm / 2));
            return out;
        };
        // Thrown: the hand holding the rope as it pays out to the hook's head, flying or fast
        let head = match hook.bite {
            Some((bite, ..)) if hook.hauling.is_some() => bite,
            _ => eye + hook.dir * hook.out,
        };
        let line = head - grip;
        let length = line.norm();
        let along = if length > 1e-6 { line / length } else { hook.dir };
        out.push(self.game.held_arm(0, grip, along, elbow));
        if length > 0.2 * m {
            // Slack while it flies and comes back, nearly taut while it hauls: drawn in stretches
            // each three times as far from the eye as the last, as thick as the middle of each is
            // far, so it's a rope in the hand and a line a few pixels wide however far it reaches
            let sag = if hook.hauling.is_some() { 0.015 } else { 0.05 };
            let drop = sag * length;
            let point = |t: f64| grip.lerp(&head, t) - Vector3::new(0.0, 0.0, 4.0 * t * (1.0 - t) * drop);
            let near = (grip - eye).norm() / m;
            let (mut t, mut far) = (0.0, ROPE_NEAR_M);
            while t < 1.0 {
                let next = ((far - near) * m / length).max(t + 0.05).min(1.0);
                let (a, b) = (point(t), point(next));
                let thick = ((far + far / 3.0) / 2.0 * ROPE_SPREAD).clamp(ROPE_THICK, ROPE_THICKEST);
                // The same curve over this stretch of it as over the whole
                let stretch = (b - a).norm().max(1e-6);
                out.extend(self.game.rope_instances(a, b, drop * (next - t).powi(2) / stretch, thick));
                (t, far) = (next, far * 3.0);
            }
            let turn = pointing(if hook.returning { -along } else { along });
            out.push(animals::moving_instance(Look::GrappleHook, head - turn * Vector3::new(0.0, 0.0, 0.22 * m), turn, vpm));
        }
        out
    }

    /// The glint of the hook's head as it flies out and bites, for it to be seen a long way off
    pub(super) fn rope_lights(&self, view: &combat::View) -> Vec<[f32; 8]> {
        let Some(hook) = &self.hook else {
            return Vec::new();
        };
        let m = self.world.voxels_per_meter as f64;
        let head = match hook.bite {
            Some((bite, ..)) if hook.hauling.is_some() => bite,
            _ => view.eye + hook.dir * hook.out,
        };
        vec![[head.x as f32, head.y as f32, head.z as f32, (3.0 * m) as f32, 1.4, 1.25, 0.9, 1.0]]
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
        // In hand, thrown as a player would
        self.game.hold_item(Item::Hook);
        self.throw_hook();
    }
}
