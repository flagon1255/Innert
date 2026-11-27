//! Throwing knives and a chakram, thrown by hand, see weapons::Style::thrown(). Knives are flicked
//! overhand one at a time as fast as the hand goes, a heavy throw a fan of three flung sidearm, and
//! a fresh one comes up into the hand after each. A chakram's hurled spinning flat out ahead and
//! comes back to the hand, cutting what it passes both ways; a heavy throw glances from one enemy
//! on to the next, three times, before it comes back. Each flies as the weapon itself, drawn with
//! its own model, see Flung: the powers' own shots, Sig::Power(Shape::Knife) and
//! Shape::Chakram, flown and struck as theirs are, see powers.rs.

use super::*;
use super::signatures::lead;
use super::signatures::powers::{Lasting, Shape};

/// How far through a throw the knife or chakram leaves the hand
pub(super) const THROW_RELEASE: f64 = 0.42;
/// How hard a chakram glancing from one enemy to the next goes on, and how far it looks for the
/// next, in meters
const CHAKRAM_GLANCES: u8 = 3;
const GLANCE_REACH: f64 = 10.0;
/// How long a knife stays stuck in the ground, in seconds
pub(super) const KNIFE_STUCK: f64 = 1.6;
/// How fast a chakram comes back, next to how fast it went out
const CHAKRAM_BACK: f64 = 1.3;

/// How the throwing knife or chakram in hand is drawn flying: its model, how big that is, its
/// anchor and voxels a meter as it's drawn in the hand, and its middle, which it spins round
#[derive(Clone, Copy)]
pub(super) struct Flung {
    model: u32,
    size: [u32; 3],
    anchor: Vector3<f64>,
    res: f64,
    middle: Vector3<f64>,
}

impl Flung {
    /// Drawn with its middle at `at`, turned by `rotation`, `size` times as big as in the hand
    pub(super) fn instance(&self, at: Vector3<f64>, rotation: UnitQuaternion<f64>, vpm: f64, size: f64, kind: u32) -> AnimalInstance {
        let [x, y, z] = self.size.map(|s| s as f64 / self.res);
        let reach = (x * x * 0.25 + y * y * 0.25 + z * z).sqrt() * vpm * size;
        let position = at - rotation * (self.middle - self.anchor) * vpm * size;
        AnimalInstance::new(self.model, position, rotation, vpm / self.res * size, reach, kind)
    }
}

/// Which way a knife's drawn turning end over end, flying `way`, `t` seconds in
pub(super) fn knife_turn(way: Vector3<f64>, t: f64) -> UnitQuaternion<f64> {
    let across = way.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::x);
    let lying = UnitQuaternion::rotation_between(&Vector3::z(), &way).unwrap_or_else(UnitQuaternion::identity);
    UnitQuaternion::from_axis_angle(&Unit::new_normalize(across), t * 26.0) * lying
}

/// Which way a chakram's drawn spinning flat, tipped a little, `t` seconds in: its ring, standing
/// in its model's x-z plane, laid flat and turned round
pub(super) fn chakram_turn(way: Vector3<f64>, t: f64) -> UnitQuaternion<f64> {
    let flat = UnitQuaternion::rotation_between(&Vector3::y(), &Vector3::z()).unwrap_or_else(UnitQuaternion::identity);
    let across = way.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::x);
    let tip = UnitQuaternion::from_axis_angle(&Unit::new_normalize(across), 0.18);
    tip * UnitQuaternion::from_axis_angle(&Vector3::z_axis(), -t * 24.0) * flat
}

impl Combat {
    /// The knives or chakram in hand, `weapon`, drawn with `look` and model `model`: remembered, so
    /// what's thrown of it is drawn as it is, see Flung
    pub(super) fn remember_flung(&self, weapon: &Weapon, look: &crate::weapons::WeaponLook, model: u32) {
        let style = weapon.profile.style;
        if !matches!(style, Style::Knife | Style::Ring) {
            return;
        }
        let (m, anchor) = &look.frames[0];
        let middle = if style == Style::Ring { look.tip } else { (look.grip + look.tip) * 0.5 };
        self.flung.set(Some(Flung { model, size: m.size, anchor: *anchor, res: look.res, middle }));
    }

    pub(super) fn flung(&self) -> Option<Flung> {
        self.flung.get()
    }

    /// Whether `weapon` is a chakram that's out of the hand, thrown and not yet back
    pub(super) fn chakram_out(&self, weapon: &Weapon) -> bool {
        weapon.profile.style == Style::Ring && self.projectiles.iter().any(|p| matches!(p.trick, Trick::Signature(Sig::Power(Shape::Chakram { .. }))))
    }

    /// Throwing the knives or chakram in hand: a throw for each click, or as fast as they go while
    /// the button's held, a heavy one on right click, each leaving the hand partway through the
    /// throw, see THROW_RELEASE; a chakram only once it's back in the hand
    pub(super) fn throw_by_hand(&mut self, arena: &mut Arena, weapon: &Weapon, controls: Controls, dt: f64) {
        let profile = weapon.profile.clone();
        match self.action {
            Action::Throw { t, length, thrown } => {
                let t = t + dt;
                let release = THROW_RELEASE * length;
                if !thrown && t >= release {
                    if profile.style == Style::Ring {
                        self.hurl_chakram(arena, weapon, self.throw_heavy);
                    } else {
                        self.flick_knives(arena, weapon, self.throw_heavy);
                    }
                }
                if t >= length {
                    // A fresh knife comes up into the hand
                    self.action = Action::Idle;
                    if profile.style == Style::Knife {
                        self.raise = self.raise.min(0.6);
                    }
                } else {
                    self.action = Action::Throw { t, length, thrown: thrown || t >= release };
                }
            }
            _ => {
                let heavy = controls.heavy;
                if (controls.attack || controls.holding || heavy) && self.cooldown <= 0.0 && self.raise >= 1.0 && !self.chakram_out(weapon) {
                    let speed = (profile.stat("attackSpeed") * self.speedup()).max(0.1);
                    let length = (0.55 / speed.sqrt()).clamp(0.3, 0.7) * if heavy { 1.35 } else { 1.0 };
                    self.action = Action::Throw { t: 0.0, length, thrown: false };
                    self.throw_heavy = heavy;
                    self.cooldown = (1.0 / speed).max(length) * if heavy { 1.3 } else { 1.0 };
                    self.cues.push(Cue::here(Sound::Swing, 0.35).pitched(if profile.style == Style::Ring { 0.8 } else { 1.5 }));
                }
            }
        }
    }

    /// A knife flicked from the hand at the crosshair, or with `fan` three flung out across it
    fn flick_knives(&mut self, arena: &mut Arena, weapon: &Weapon, fan: bool) {
        let view = arena.view;
        let vpm = view.vpm;
        let profile = weapon.profile.clone();
        let from = view.eye + view.rotation * Vector3::new(0.18, 0.55, -0.08) * vpm;
        let target = self.aim(arena);
        let speed = profile.stat("projectileSpeed").max(20.0);
        let damage = profile.stat("physicalDamage") + profile.stat("elementalDamage");
        let yaws: &[f64] = if fan { &[-0.12, 0.0, 0.12] } else { &[0.0] };
        let up = view.rotation * Vector3::z_axis();
        for &yaw in yaws {
            let way = UnitQuaternion::from_axis_angle(&up, yaw) * lead(from, target, speed, 0.3, vpm);
            self.launch(&profile, None, Form::Bolt, from, way * speed * vpm, damage * if fan { 0.8 } else { 1.0 }, 0.3);
            self.signed(Sig::Power(Shape::Knife));
            if let Some(p) = self.projectiles.last_mut() {
                p.range = f64::INFINITY;
            }
        }
        self.cues.push(Cue::here(Sound::Swing, 0.55).pitched(if fan { 1.25 } else { 1.6 }));
        if fan {
            self.cues.push(Cue::here(Sound::Shing, 0.4).pitched(1.5));
        }
        if profile.element.is_some() {
            self.sparks(from, 4, 1.0, Element::look(profile.element), 0.0, 0.25);
        }
    }

    /// The chakram in hand hurled spinning at the crosshair, as far as it reaches, to come back;
    /// with `heavy` glancing on from enemy to enemy first
    fn hurl_chakram(&mut self, arena: &mut Arena, weapon: &Weapon, heavy: bool) {
        let view = arena.view;
        let vpm = view.vpm;
        let profile = weapon.profile.clone();
        let from = view.eye + view.rotation * Vector3::new(0.12, 0.6, -0.15) * vpm;
        let target = self.aim(arena);
        let speed = profile.stat("projectileSpeed").max(12.0);
        let reach = profile.stat("range").max(6.0);
        let damage = profile.stat("physicalDamage") + profile.stat("elementalDamage");
        let way = (target - from).try_normalize(1e-6).unwrap_or_else(|| view.forward());
        // Out as far as the crosshair's on, a little past what it's on, or as far as it reaches
        let range = ((target - from).norm() / vpm + 1.5).min(reach);
        self.launch(&profile, None, Form::Bolt, from, way * speed * vpm, damage * if heavy { 1.2 } else { 1.0 }, 0.0);
        self.signed(Sig::Power(Shape::Chakram { back: false, glances: if heavy { CHAKRAM_GLANCES } else { 0 } }));
        if let Some(p) = self.projectiles.last_mut() {
            p.range = range * vpm;
        }
        self.cues.push(Cue::here(Sound::Whirr, 0.7).pitched(if heavy { 0.8 } else { 1.0 }));
        self.cues.push(Cue::here(Sound::Shing, 0.35).pitched(1.3));
    }

    /// A knife striking animal `i` at `at`: struck as a blow of the weapon is, its effects and all,
    /// sometimes a crit, a spray of what it hit
    pub(super) fn knife_strikes(&mut self, arena: &mut Arena, p: &mut Projectile, i: usize, at: Vector3<f64>) {
        let profile = p.profile.clone();
        let crit = self.roll_crit(&profile);
        let damage = p.damage * if crit { profile.stat("critDamage").max(1.0) } else { 1.0 };
        let from = p.position - p.velocity * 0.03;
        self.element_hit(at, profile.element, 0.7);
        self.sparks(at, 4, 2.5, Element::look(None), 0.6, 0.25);
        self.cues.push(Cue::at(Sound::ArrowHit, at, 0.6).pitched(1.3));
        self.strike(arena, &profile, i, Blow { damage, crit, heavy: false, from, depth: 0, element: profile.element });
        self.landed = self.landed.wrapping_add(1);
    }

    /// A knife coming down at `at`, stuck in the ground there a while, see Lasting::Knife
    pub(super) fn knife_lands(&mut self, p: &Projectile, at: Vector3<f64>) {
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(|| -Vector3::z());
        self.sparks(at, 3, 1.5, Element::look(None), 0.8, 0.2);
        self.cues.push(Cue::at(Sound::ArrowHit, at, 0.4).pitched(1.5));
        let kind = ZoneKind::Signature(SigZone::Power(Lasting::Knife { way: [way.x as f32, way.y as f32, way.z as f32] }));
        // A few stuck about at most, the oldest sinking away first
        let stuck: Vec<usize> = self.zones.iter().enumerate().filter(|(_, z)| matches!(z.kind, ZoneKind::Signature(SigZone::Power(Lasting::Knife { .. })))).map(|(k, _)| k).collect();
        if stuck.len() >= 6 {
            self.zones[stuck[0]].life = self.zones[stuck[0]].life.min(0.3);
        }
        self.zone(kind, &p.profile, at, Vector3::zeros(), 0.0, KNIFE_STUCK, 0.0);
    }

    /// A chakram on its way: out as far as it goes and back to the hand, caught there, cutting
    /// what's in its way once each way; a heavy throw's glancing on from what it cut to the next
    /// near first. True once it's back in the hand.
    pub(super) fn chakram_flies(&mut self, arena: &mut Arena, p: &mut Projectile, back: bool, glances: u8, dt: f64) -> bool {
        let vpm = arena.view.vpm;
        let hand = arena.view.eye + arena.view.rotation * Vector3::new(0.2, 0.5, -0.3) * vpm;
        let speed = p.velocity.norm().max(1.0);
        if back {
            let to = hand - p.position;
            if to.norm() < 1.2 * vpm {
                self.cues.push(Cue::here(Sound::Equip, 0.5).pitched(1.25));
                self.raise = self.raise.min(0.8);
                return true;
            }
            let homing = to.normalize() * speed.max(p.profile.stat("projectileSpeed") * CHAKRAM_BACK * vpm);
            p.velocity = p.velocity.lerp(&homing, (8.0 * dt).min(1.0));
        } else if p.flown >= p.range {
            self.turn_chakram_back(p);
        }
        // Whirring as it spins
        if (p.flown / (1.6 * vpm)).floor() != ((p.flown - speed * dt) / (1.6 * vpm)).floor() {
            self.cues.push(Cue::at(Sound::Whirr, p.position, 0.35).pitched(1.4));
        }
        // What's in its way, cut once each way
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let step = speed * dt;
        let caught: Vec<usize> = (0..arena.animals.len())
            .filter(|&i| {
                let a = &arena.animals[i];
                if a.health <= 0.0 || a.look.is_pet() || p.hit.contains(&a.id) {
                    return false;
                }
                let body = a.look.body().1 * vpm;
                let rel = a.center(vpm as u32) - p.position;
                let along = rel.dot(&way);
                along > -body && along < step + body && (rel - way * along).norm() < 0.6 * vpm + body
            })
            .collect();
        for i in caught {
            let center = arena.animals[i].center(vpm as u32);
            p.hit.push(arena.animals[i].id);
            self.knife_strikes(arena, p, i, center);
            self.sparks(center, 6, 3.0, Element::look(p.profile.element), 0.4, 0.3);
            // Glancing on to the next near, or back once it's glanced all it does
            if !back && glances > 0 {
                let hit = p.hit.clone();
                let next = Combat::foes_round(arena, center, GLANCE_REACH).into_iter().find(|&j| !hit.contains(&arena.animals[j].id));
                match next {
                    Some(j) => {
                        let to = arena.animals[j].center(vpm as u32) - center;
                        p.velocity = to.normalize() * speed;
                        p.range = p.flown + to.norm() + 2.0 * vpm;
                        p.trick = Trick::Signature(Sig::Power(Shape::Chakram { back: false, glances: glances - 1 }));
                        self.cues.push(Cue::at(Sound::Clang, center, 0.5).pitched(1.5));
                    }
                    None => self.turn_chakram_back(p),
                }
            }
        }
        // Its element trailing off its rim
        if p.profile.element.is_some() && random(&mut self.rng) < dt * 30.0 {
            let at = p.position + self.any_direction() * 0.3 * vpm;
            self.mote(at, p.profile.element, 1.2);
        }
        false
    }

    /// A chakram turned back for the hand, what it's cut cut again on its way back
    pub(super) fn turn_chakram_back(&mut self, p: &mut Projectile) {
        p.trick = Trick::Signature(Sig::Power(Shape::Chakram { back: true, glances: 0 }));
        p.hit.clear();
        p.range = f64::INFINITY;
    }

    /// Where the knives or chakram in hand are held, and how they're thrown, see target_pose()
    pub(super) fn thrown_pose(&self, weapon: &Weapon) -> Pose {
        let ring = weapon.profile.style == Style::Ring;
        // A knife held point up by its grip at the right, a chakram up by its rim
        let rest = if ring { pose([0.27, 0.46, -0.27], [0.1, 0.25, 0.96], [1.0, 0.0, 0.0]) } else { pose([0.27, 0.46, -0.26], [0.0, 0.45, 0.89], [1.0, 0.0, 0.0]) };
        let r = THROW_RELEASE;
        match self.action {
            Action::Throw { t, length, .. } => {
                let keys: Vec<(f64, Pose, bool)> = match (ring, self.throw_heavy) {
                    // Drawn back by the ear, point up, and flicked out overhand from the wrist
                    (false, false) => vec![
                        (0.16, pose([0.3, 0.3, -0.06], [0.1, -0.4, 0.91], [1.0, 0.0, 0.0]), false),
                        (r - 0.05, pose([0.31, 0.24, -0.02], [0.08, -0.55, 0.83], [1.0, 0.0, 0.0]), false),
                        (r, pose([0.16, 0.66, -0.12], [0.0, 0.95, 0.3], [1.0, 0.0, 0.0]), true),
                        (0.78, pose([0.12, 0.56, -0.34], [0.0, 0.6, -0.8], [1.0, 0.0, 0.0]), false),
                    ],
                    // Drawn across to the left and slung out sidearm, three fanning off the hand
                    (false, true) => vec![
                        (0.2, pose([-0.04, 0.36, -0.24], [-0.85, 0.35, 0.35], [0.0, 0.0, 1.0]), false),
                        (r - 0.06, pose([-0.08, 0.34, -0.25], [-0.9, 0.25, 0.3], [0.0, 0.0, 1.0]), false),
                        (r, pose([0.22, 0.62, -0.22], [0.55, 0.83, 0.0], [0.0, 0.0, 1.0]), true),
                        (0.8, pose([0.4, 0.46, -0.3], [0.9, 0.3, -0.2], [0.0, 0.0, 1.0]), false),
                    ],
                    // Wound in across the chest, flat, and flung out backhand, the wrist snapping
                    (true, false) => vec![
                        (0.2, pose([-0.04, 0.38, -0.22], [-0.85, 0.3, 0.42], [0.0, 0.2, 1.0]), false),
                        (r - 0.06, pose([-0.08, 0.36, -0.22], [-0.92, 0.2, 0.33], [0.0, 0.2, 1.0]), false),
                        (r, pose([0.2, 0.68, -0.18], [0.4, 0.9, 0.18], [0.0, -0.2, 1.0]), true),
                        (0.8, pose([0.38, 0.5, -0.26], [0.85, 0.4, 0.0], [0.0, -0.3, 0.95]), false),
                    ],
                    // Raised high, drawn back over the shoulder and hurled overhand, hard
                    (true, true) => vec![
                        (0.22, pose([0.3, 0.26, 0.02], [0.1, -0.5, 0.86], [1.0, 0.0, 0.0]), false),
                        (r - 0.06, pose([0.32, 0.18, 0.06], [0.08, -0.65, 0.76], [1.0, 0.0, 0.0]), false),
                        (r, pose([0.16, 0.72, -0.06], [0.0, 0.92, 0.38], [1.0, 0.0, 0.0]), true),
                        (0.8, pose([0.12, 0.58, -0.36], [0.0, 0.55, -0.83], [1.0, 0.0, 0.0]), false),
                    ],
                };
                animate(&rest, &keys, t / length)
            }
            Action::Special { t, length, special } => animate(&rest, &special_move(special, Kind::Melee, weapon.profile.style).1, t / length),
            _ => rest,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapons::Library;

    /// Open ground, 48 m across, four meters of earth under four of sky
    fn field() -> World {
        let size = [384u32, 384, 64];
        let (tx, ty) = (size[0] as usize / 4, size[1] as usize / 4);
        let mut occupancy = vec![0u128; tx * ty * (size[2] as usize / 8)];
        occupancy[..tx * ty * 4].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, crate::worldgen::material::DIRT);
        world.voxels_per_meter = 8;
        world
    }

    /// Thrown at a pack of bandits 7 m ahead, clicking as a player would: what's thrown flies out
    /// of the hand and hurts them, never there the moment it's thrown, and a chakram comes back
    /// to the hand, out of it meanwhile
    #[test]
    fn knives_and_a_chakram_fly_strike_and_come_back() {
        let library = Library::load();
        let world = field();
        let vpm = 8.0;
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let feet = Vector3::new(24.0, 10.0, 4.0) * vpm;
        let view = View { eye: feet + Vector3::new(0.0, 0.0, 1.6 * vpm), rotation: UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.1), tan: [1.2445, 0.7002], vpm, speed: 0.0, stride: 0.0 };
        for (id, heavy) in [("knives", false), ("knives", true), ("chakram", false), ("chakram", true)] {
            let weapon = library.generate(5, library.family(id), 1.0);
            let mut animals = crate::animals::spawn_pack(&world, &water, &[Look::Bandit; 3], feet + Vector3::new(0.0, 7.0 * vpm, 0.0), 1.0, 3);
            let health: Vec<f64> = animals.iter().map(|a| a.health).collect();
            let mut combat = Combat::new(1, 0);
            // A rogue's Shadowstep is on right click while it's ready, the fan while it comes round
            if heavy {
                combat.power.cooldown = 30.0;
            }
            let dt = 1.0 / 120.0;
            let (mut thrown_at, mut hurt_at, mut out, mut back) = (None, None, false, false);
            for frame in 0..(3.0 / dt) as usize {
                let t = frame as f64 * dt - 0.4;
                let click = (0.0..dt).contains(&t);
                let controls = Controls { attack: click && !heavy, heavy: click && heavy, ..Default::default() };
                let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet), water: &water };
                combat.update(&mut arena, Some((0, &weapon)), None, controls, dt);
                let flying = combat.projectiles.iter().any(|p| matches!(p.trick, Trick::Signature(Sig::Power(Shape::Knife | Shape::Chakram { .. }))));
                if flying && thrown_at.is_none() {
                    thrown_at = Some(t);
                }
                if hurt_at.is_none() && animals.iter().zip(&health).any(|(a, &h)| a.health < h - 1e-6) {
                    hurt_at = Some(t);
                }
                if id == "chakram" {
                    out |= combat.chakram_out(&weapon);
                    back |= out && !flying;
                }
            }
            let thrown_at = thrown_at.unwrap_or_else(|| panic!("{id} never left the hand"));
            let hurt_at = hurt_at.unwrap_or_else(|| panic!("{id} never reached the pack"));
            assert!(hurt_at > thrown_at + 0.05, "{id} hurt them the moment it was thrown");
            if id == "chakram" {
                assert!(out && back, "the chakram never came back to the hand");
            }
        }
    }
}
