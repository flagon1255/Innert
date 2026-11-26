//! The third batch's powers, see weapons::Power: a war pick's Starbreak, splitting the ground in a
//! star of cracks; throwing knives' Shadowstep, stepping through the shadows behind what's aimed at
//! to strike it in the back; a chakram's Ringstorm, three rings hurled in a fan; a prism's Spectrum,
//! the light split into five rays of five elements; and a runestone's Runeburst, a circle of runes
//! written on the ground one by one, blasting up once it's whole. What they shoot and leave going
//! is powers.rs's machinery, shapes and lastings of their own, see Shape and Lasting.

use super::powers::{FISSURE_STEP, Lasting, Shape};
use super::*;
use crate::weapons::Power;

/// How far the cracks of a war pick's Starbreak run out, how many steps each, how far out the first
/// is, in meters, and how many cracks
pub(in crate::combat) const STAR_REACH: f64 = 7.5;
pub(in crate::combat) const STAR_STEPS: f64 = 5.0;
pub(in crate::combat) const STAR_FROM: f64 = 1.2;
const STAR_CRACKS: usize = 5;
/// How far a Shadowstep reaches, and how far behind what it's aimed at it comes out, in meters,
/// and how fast the shade it steps through flies
pub(in crate::combat) const STEP_REACH: f64 = 16.0;
const STEP_BEHIND: f64 = 1.3;
const STEP_SPEED: f64 = 34.0;
/// How much harder a Shadowstep's blow lands than a throw
const STEP_POWER: f64 = 3.0;
/// How far a Spectrum's rays reach, in meters, how fast they fly, and how they fan out
pub(in crate::combat) const SPECTRUM_REACH: f64 = 28.0;
const RAY_SPEED: f64 = 40.0;
pub(in crate::combat) const SPECTRUM_FAN: [f64; 5] = [-0.24, -0.12, 0.0, 0.12, 0.24];
/// A Runeburst: how far round its circle is, how many runes, how long apart each is written, and
/// how long after the last the circle blasts up, in meters and seconds
pub(in crate::combat) const RUNE_ROUND: f64 = 3.5;
const RUNE_COUNT: usize = 6;
const RUNE_WRITE: f64 = 0.12;
const RUNE_HOLD: f64 = 0.2;
/// A runestone's cast's one rune: how far round it blasts, at size 1, and how long after it's
/// written
const SIGIL_ROUND: f64 = 1.8;
const SIGIL_BLAST: f64 = 0.45;

/// When a Runeburst's circle blasts up, `age` seconds in being when it's whole and held a beat
fn rune_blast_at() -> f64 {
    RUNE_COUNT as f64 * RUNE_WRITE + RUNE_HOLD
}

/// Where rune `k` of a circle round `center` is written
fn rune_spot(center: Vector3<f64>, k: usize, vpm: f64) -> Vector3<f64> {
    let a = k as f64 / RUNE_COUNT as f64 * TAU + 0.3;
    center + Vector3::new(a.cos(), a.sin(), 0.0) * RUNE_ROUND * 0.8 * vpm + Vector3::new(0.0, 0.0, 0.06 * vpm)
}

/// How long the third batch's powers' own moves take, and when what they let go leaves the weapon
pub(in crate::combat) fn arsenal_timing(power: Power) -> Option<(f64, f64)> {
    Some(match power {
        Power::Starbreak => (1.0, 0.56),
        Power::Shadowstep => (0.7, 0.32),
        Power::Ringstorm => (0.8, 0.42),
        Power::Spectrum => (0.9, 0.45),
        Power::Runeburst => (1.0, 0.5),
        _ => return None,
    })
}

/// Their own moves, from where the weapon's held at rest
pub(in crate::combat) fn arsenal_keys(power: Power, profile: &Profile) -> Option<Vec<(f64, Pose, bool)>> {
    let (_, release) = arsenal_timing(power)?;
    Some(match power {
        // Raised high in both hands, held there trembling, then driven straight down into the
        // ground in front, levered after as the ground splits
        Power::Starbreak => vec![
            (0.2, pose([0.18, 0.3, 0.04], [0.1, 0.05, 1.0], [0.0, 1.0, -0.05]), false),
            (0.34, pose([0.17, 0.28, 0.09], [0.12, -0.08, 0.99], [0.0, 0.99, 0.08]), false),
            (release - 0.06, pose([0.18, 0.27, 0.1], [0.1, -0.12, 0.99], [0.0, 0.99, 0.12]), false),
            (release, pose([0.04, 0.6, -0.38], [0.0, 0.62, -0.78], [0.0, -0.78, -0.62]), true),
            (release + 0.1, pose([0.04, 0.62, -0.44], [0.0, 0.52, -0.85], [0.0, -0.85, -0.52]), false),
            (release + 0.22, pose([0.1, 0.55, -0.36], [0.2, 0.7, -0.68], [0.0, -0.68, -0.73]), false),
            (0.9, pose([0.2, 0.48, -0.3], [0.0, 0.55, 0.83], [0.0, 1.0, -0.1]), false),
        ],
        // Dropped low and drawn back, then driven forward and up into a backhanded stab, as if
        // stepping out of the shadow behind what it's at
        Power::Shadowstep => vec![
            (0.12, pose([0.26, 0.3, -0.38], [0.05, 0.6, 0.8], [1.0, 0.0, 0.0]), false),
            (release - 0.06, pose([0.28, 0.2, -0.42], [0.05, 0.4, 0.9], [1.0, 0.0, 0.0]), false),
            (release, pose([0.14, 0.7, -0.2], [-0.05, 0.98, 0.15], [1.0, 0.0, 0.0]), true),
            (release + 0.14, pose([0.1, 0.72, -0.26], [-0.1, 0.9, -0.42], [1.0, 0.0, 0.0]), false),
            (0.88, pose([0.24, 0.5, -0.28], [0.0, 0.5, 0.86], [1.0, 0.0, 0.0]), false),
        ],
        // Raised high and drawn back over the shoulder, held, and hurled overhand hard, three rings
        // leaving it
        Power::Ringstorm => vec![
            (0.18, pose([0.3, 0.28, 0.0], [0.1, -0.4, 0.91], [1.0, 0.0, 0.0]), false),
            (release - 0.06, pose([0.33, 0.16, 0.08], [0.08, -0.66, 0.75], [1.0, 0.0, 0.0]), false),
            (release, pose([0.14, 0.72, -0.06], [0.0, 0.92, 0.38], [1.0, 0.0, 0.0]), true),
            (release + 0.16, pose([0.1, 0.62, -0.32], [0.0, 0.6, -0.8], [1.0, 0.0, 0.0]), false),
            (0.88, pose([0.26, 0.46, -0.27], [0.1, 0.25, 0.96], [1.0, 0.0, 0.0]), false),
        ],
        // Turned in the hand to catch the light, held up, then levelled at the crosshair as the
        // rays fan out of it
        Power::Spectrum => {
            let at = spell_at(profile.style);
            vec![
                (0.14, at([0.02, -0.04, 0.1], [0.2, -0.1, 0.97]), false),
                (0.3, at([-0.02, -0.05, 0.12], [-0.2, -0.15, 0.96]), false),
                (release - 0.05, at([0.0, -0.06, 0.13], [0.0, -0.2, 0.98]), false),
                (release, at([-0.04, 0.16, 0.0], [-0.04, 0.95, 0.3]), true),
                (release + 0.15, at([-0.03, 0.15, 0.0], [-0.03, 0.92, 0.38]), false),
                (0.86, at([-0.01, 0.03, 0.01], [-0.02, 0.4, 0.92]), false),
            ]
        }
        // Raised up and spoken over, then thrust down at the ground ahead, writing the first rune
        Power::Runeburst => {
            let at = spell_at(profile.style);
            vec![
                (0.15, at([0.0, -0.04, 0.12], [0.0, -0.2, 0.98]), false),
                (0.34, at([0.0, -0.06, 0.17], [0.02, -0.3, 0.95]), false),
                (release - 0.05, at([0.01, -0.05, 0.18], [0.02, -0.28, 0.96]), false),
                (release, at([-0.04, 0.2, -0.1], [-0.05, 0.7, -0.71]), true),
                (release + 0.12, at([-0.04, 0.21, -0.12], [-0.05, 0.65, -0.76]), false),
                (0.86, at([-0.01, 0.03, 0.0], [-0.02, 0.4, 0.92]), false),
            ]
        }
        _ => return None,
    })
}

impl Combat {
    /// One of the third batch's powers let go, see unleash_power(): true if it was one of them
    pub(in crate::combat) fn unleash_arsenal(&mut self, arena: &mut Arena, weapon: &Weapon, power: Power, base: f64) -> bool {
        let view = arena.view;
        let vpm = view.vpm;
        let up = Vector3::z();
        let profile = weapon.profile.clone();
        let element = profile.element;
        let look = Element::look(element);
        let light = element.map_or([0.95, 0.9, 0.8], Element::light);
        let feet = arena.player.unwrap_or(view.eye - up * 1.6 * vpm);
        let forward = view.forward();
        let flat = flat_of(forward);
        match power {
            // Five cracks running out all round from where the pick went in, the first straight
            // ahead, each erupting step by step
            Power::Starbreak => {
                let center = ground_under(arena.world, feet + flat * 0.8 * vpm + up * vpm, vpm);
                self.landed(feet);
                for k in 0..STAR_CRACKS {
                    let way = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), k as f64 / STAR_CRACKS as f64 * TAU) * flat;
                    let start = ground_under(arena.world, feet + way * STAR_FROM * vpm + up * vpm, vpm);
                    self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Crack)), &profile, start, way * FISSURE_STEP * vpm, 1.5, STAR_STEPS * 0.07 + 0.35, base * 1.3);
                }
                self.keep_power_marks(power, &[feet + up * 0.05 * vpm], STAR_STEPS * 0.07 + 0.2);
                self.swells.push(Swell { model: STONE_PILLAR, position: center, radius: 0.5, life: 0.45, lifetime: 0.45 });
                for _ in 0..12 {
                    let v = (self.any_direction() + up * 1.4) * 4.5 * vpm;
                    self.drop(center + up * 0.2 * vpm, v, DROP_MUD, 1.0);
                }
                self.cues.push(Cue::here(Sound::Quake, 1.0).pitched(0.9));
                self.cues.push(Cue::here(Sound::Slam, 0.9).pitched(0.7));
                self.flashes.push(Flash { position: center + up * 0.5 * vpm, color: light, reach: 8.0, life: 0.3, lifetime: 0.3 });
                self.shake = self.shake.max(0.7);
            }
            // Gone in a puff of shadow, a shade of it flying at what's aimed at, and out of the dark
            // behind it, striking it in the back
            Power::Shadowstep => {
                let (to, who) = self.power_points(arena, power).first().copied().unwrap_or((view.eye + forward * STEP_REACH * vpm, None));
                let mark = who.or_else(|| Combat::foes_round(arena, to, 3.0).first().copied());
                let from = view.eye + view.rotation * Vector3::new(0.1, 0.8, -0.25) * vpm;
                let shadow = Element::look(Some(Element::Void));
                // A puff of shadow where you were
                for _ in 0..8 {
                    let v = (self.any_direction() * 1.2 + up * 0.4) * vpm;
                    let life = 0.7 + 0.4 * random(&mut self.rng);
                    self.sparks.push(Spark { position: feet + flat * 1.6 * vpm + up * (0.4 + random(&mut self.rng)) * vpm, velocity: v, look: shadow, life, lifetime: life, gravity: -0.02, slot: SLOT_SMOKE });
                }
                self.cap_sparks();
                let (goal, damage) = match mark {
                    Some(i) => {
                        let a = &arena.animals[i];
                        let center = a.center(vpm as u32);
                        let across = (a.position - feet).xy().try_normalize(1e-6).map_or(flat, |v| Vector3::new(v.x, v.y, 0.0));
                        // Out of the dark behind it
                        let behind = a.position + across * (a.look.body().1 + STEP_BEHIND) * vpm;
                        let go = (behind - feet).xy().norm() / vpm;
                        self.dash = across * (go / 0.31).min(48.0) * vpm + up * 1.5 * vpm;
                        (center, base * STEP_POWER)
                    }
                    None => {
                        let room = ((to - feet).xy().norm() / vpm - 1.0).clamp(0.0, 10.0);
                        self.dash = flat * (room / 0.31).min(32.0) * vpm + up * 1.5 * vpm;
                        (to, base)
                    }
                };
                let way = (goal - from).try_normalize(1e-6).unwrap_or(forward);
                self.launch(&profile, None, Form::Bolt, from, way * STEP_SPEED * vpm, damage, 0.0);
                self.signed(Sig::Power(Shape::Step));
                if let Some(p) = self.projectiles.last_mut() {
                    p.range = (goal - from).norm() + 1.5 * vpm;
                }
                if who.is_none() && mark.is_none() {
                    self.keep_power_marks(power, &[to], (to - from).norm() / (STEP_SPEED * vpm));
                }
                self.cues.push(Cue::here(Sound::Dodge, 1.0).pitched(0.6));
                self.cues.push(Cue::here(Sound::Hex, 0.4).pitched(1.4));
            }
            // Three rings hurled out in a fan, each to come back
            Power::Ringstorm => {
                let from = view.eye + view.rotation * Vector3::new(0.12, 0.9, -0.1) * vpm;
                for (to, who) in self.power_points(arena, power) {
                    let along = (to - from).try_normalize(1e-6).unwrap_or(forward);
                    let range = ((to - from).norm() / vpm + if who.is_some() { 1.5 } else { 0.0 }).min(super::powers::WHEEL_REACH);
                    self.launch(&profile, None, Form::Bolt, from, along * 18.0 * vpm, base * 1.1, 0.0);
                    self.signed(Sig::Power(Shape::Wheel { back: false }));
                    if let Some(p) = self.projectiles.last_mut() {
                        p.range = range * vpm;
                    }
                    if who.is_none() {
                        self.keep_power_marks(power, &[to], range / 18.0);
                    }
                }
                self.cues.push(Cue::here(Sound::Whirr, 1.0).pitched(0.7));
                self.cues.push(Cue::here(Sound::Shing, 0.7).pitched(1.1));
                self.flashes.push(Flash { position: from, color: light, reach: 5.0, life: 0.2, lifetime: 0.2 });
            }
            // A ray of each of five elements fanned out across the crosshair
            Power::Spectrum => {
                let from = self.held_point(weapon, view, false);
                for (k, (to, who)) in self.power_points(arena, power).into_iter().enumerate() {
                    let along = (to - from).try_normalize(1e-6).unwrap_or(forward);
                    let range = (to - from).norm() / vpm + if who.is_some() { 3.0 } else { 0.3 };
                    let hue = PRISM[k % PRISM.len()];
                    let tinted = Rc::new(Profile { element: Some(hue), ..(*profile).clone() });
                    self.launch(&tinted, None, Form::Bolt, from, along * RAY_SPEED * vpm, base * 0.9, 0.0);
                    self.signed(Sig::Power(Shape::Ray));
                    if let Some(p) = self.projectiles.last_mut() {
                        p.range = range * vpm;
                    }
                    if who.is_none() {
                        self.keep_power_marks(power, &[to], range / RAY_SPEED);
                    }
                    self.sparks(from, 3, 2.0, Element::look(Some(hue)), 0.0, 0.25);
                }
                self.cues.push(Cue::here(Sound::Shing, 0.9).pitched(1.6));
                self.cues.push(Cue::here(Sound::Cast(look as u8), 0.6));
                self.flashes.push(Flash { position: from, color: [1.0, 0.95, 0.9], reach: 7.0, life: 0.25, lifetime: 0.25 });
            }
            // A glyph cast down onto the ring, where its circle's written
            Power::Runeburst => {
                let from = self.held_point(weapon, view, false);
                let aim = self.power_spot(arena, weapon).unwrap_or_else(|| self.aim(arena));
                let time = ((aim - from).norm() / (20.0 * vpm)).max(0.3);
                self.keep_power_ring(weapon, aim, time + rune_blast_at() + 0.2);
                self.launch(&profile, None, Form::Bolt, from, thrown(from, aim, time, 0.25, vpm), base, 0.25);
                self.signed(Sig::Power(Shape::Glyph));
                self.cues.push(Cue::here(Sound::Summon, 0.8).pitched(1.2));
                self.flashes.push(Flash { position: from, color: light, reach: 6.0, life: 0.25, lifetime: 0.25 });
            }
            _ => return false,
        }
        true
    }

    /// The shade a Shadowstep steps through reaching what it's aimed at, `i`, struck in the back
    /// out of the dark, sure to be a crit
    pub(in crate::combat) fn step_strikes(&mut self, arena: &mut Arena, p: &Projectile, i: usize, at: Vector3<f64>) {
        let profile = p.profile.clone();
        let damage = p.damage * profile.stat("critDamage").max(1.5);
        self.element_hit(at, Some(Element::Void), 1.0);
        self.element_hit(at, profile.element, 0.8);
        self.sparks(at, 10, 3.5, Element::look(Some(Element::Blood)), 0.6, 0.35);
        self.cues.push(Cue::at(Sound::Stab, at, 1.0).pitched(0.8));
        self.strike(arena, &profile, i, Blow { damage, crit: true, heavy: true, from: at + p.velocity.normalize() * self.vpm, depth: 0, element: profile.element });
        if arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
            arena.animals[i].status.staggered = arena.animals[i].status.staggered.max(0.6);
        }
        self.step_out(p.position);
    }

    /// A puff of shadow where a Shadowstep comes out
    pub(in crate::combat) fn step_out(&mut self, at: Vector3<f64>) {
        let vpm = self.vpm;
        let shadow = Element::look(Some(Element::Void));
        for _ in 0..8 {
            let v = (self.any_direction() * 1.5 + Vector3::z() * 0.3) * vpm;
            let life = 0.6 + 0.4 * random(&mut self.rng);
            let position = at + self.any_direction() * 0.4 * vpm;
            self.sparks.push(Spark { position, velocity: v, look: shadow, life, lifetime: life, gravity: -0.02, slot: SLOT_SMOKE });
        }
        self.cap_sparks();
        self.flashes.push(Flash { position: at, color: Element::Void.light(), reach: 4.0, life: 0.2, lifetime: 0.2 });
    }

    /// A glyph come down at `at`: its circle of runes written round it there, or a runestone's
    /// cast, its one rune
    pub(in crate::combat) fn glyph_lands(&mut self, arena: &mut Arena, p: &Projectile, at: Vector3<f64>) {
        let ground = Combat::floor_at(arena, at);
        if matches!(p.trick, Trick::Signature(Sig::Power(Shape::Sigil))) {
            self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Sigil)), &p.profile, ground, Vector3::zeros(), SIGIL_ROUND * p.profile.stat("area").max(0.6), SIGIL_BLAST + 0.4, p.damage);
            self.cues.push(Cue::at(Sound::Hex, ground, 0.5).pitched(1.5));
            return;
        }
        self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Runes)), &p.profile, ground, Vector3::zeros(), RUNE_ROUND, rune_blast_at() + 0.5, p.damage * 2.2);
        self.sparks(ground + Vector3::new(0.0, 0.0, 0.2 * self.vpm), 10, 2.5, Element::look(p.profile.element.or(Some(Element::Arcane))), 0.2, 0.4);
        self.cues.push(Cue::at(Sound::Hex, ground, 0.7).pitched(1.3));
    }

    /// A circle of runes going on: a rune written round it each RUNE_WRITE, glowing up, then all of
    /// it blasting up at once, everything in it struck and thrown up
    pub(in crate::combat) fn runes_go_on(&mut self, arena: &mut Arena, z: &mut Zone, one: bool) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let age = z.lifetime - z.life;
        let profile = z.profile.clone();
        let look = Element::look(profile.element.or(Some(Element::Arcane)));
        // A runestone's cast: its one rune glowing up under what's there, then blasting
        if one {
            if z.dwell <= 0.0 && age >= SIGIL_BLAST {
                z.dwell = age;
                self.power_pop(arena, &profile, z.position + up * 0.3 * vpm, z.radius, z.damage, None);
                for i in Combat::foes_round(arena, z.position, z.radius) {
                    if arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
                        arena.animals[i].status.staggered = arena.animals[i].status.staggered.max(0.4);
                    }
                }
                self.sparks(z.position + up * 0.2 * vpm, 8, 4.0, look, -0.3, 0.45);
                self.cues.push(Cue::at(Sound::Burst, z.position, 0.7).pitched(1.1));
            } else if z.dwell <= 0.0 && random(&mut self.rng) < 0.3 {
                let a = random(&mut self.rng) * TAU;
                let at = z.position + Vector3::new(a.cos(), a.sin(), 0.0) * z.radius * 0.8 * vpm;
                self.fine(at, up * 0.8 * vpm, look, 0.35, -0.1);
            }
            return;
        }
        while (z.owed as usize) < RUNE_COUNT && age >= z.owed * RUNE_WRITE {
            let at = rune_spot(z.position, z.owed as usize, vpm);
            z.owed += 1.0;
            self.sparks(at + up * 0.1 * vpm, 5, 1.6, look, -0.3, 0.4);
            self.cues.push(Cue::at(Sound::Shing, at, 0.4).pitched(1.2 + 0.08 * z.owed as f32));
        }
        if z.dwell <= 0.0 && age >= rune_blast_at() {
            z.dwell = age;
            self.landed(z.position);
            self.power_burst(arena, &profile, z.position + up * 0.3 * vpm, z.radius, z.damage, None);
            for i in Combat::foes_round(arena, z.position, z.radius) {
                if arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
                    let s = &mut arena.animals[i].status;
                    s.push = up * 6.0 * vpm;
                    s.staggered = s.staggered.max(0.7);
                }
            }
            if (z.position - arena.view.eye).norm() > 4.5 * vpm {
                self.swells.push(Swell { model: PILLAR, position: z.position, radius: 0.8, life: 0.5, lifetime: 0.5 });
            }
            for k in 0..RUNE_COUNT {
                let at = rune_spot(z.position, k, vpm);
                self.sparks(at, 4, 4.0, look, -0.4, 0.5);
            }
            self.cues.push(Cue::at(Sound::Burst, z.position, 1.0).pitched(0.8));
            self.cues.push(Cue::at(Sound::Toll, z.position, 0.6).pitched(1.4));
            self.shake = self.shake.max(0.4);
        }
    }

    /// How a circle of runes is drawn: each rune written so far glowing on the ground, the circle
    /// round them drawn on as they are, and once it blasts, fading out
    pub(in crate::combat) fn runes_instances(&self, z: &Zone, view: &View, one: bool) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        let age = z.lifetime - z.life;
        let look = Element::look(z.profile.element.or(Some(Element::Arcane)));
        let fade = if z.dwell > 0.0 { (1.0 - (age - z.dwell) / 0.4).clamp(0.05, 1.0) } else { 1.0 };
        let mut out = Vec::new();
        let (model, rune) = self.model_at(RUNE);
        // A runestone's cast: one rune drawn out on the ground as it's written, turning, glowing
        // brighter till it blasts
        if one {
            let drawn = smooth(age / 0.2).max(0.1);
            let glow = 1.0 + 0.25 * (age / SIGIL_BLAST).min(1.0) + 0.08 * (self.time * 14.0).sin();
            let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), age * 2.5);
            out.push((eye, place(model, rune, RING_RES, Vector3::zeros(), z.position + Vector3::new(0.0, 0.0, 0.06 * vpm), turn, vpm, z.radius * drawn * glow * fade, MOVING)));
            return out;
        }
        let written = (z.owed as usize).min(RUNE_COUNT);
        for k in 0..written {
            let at = rune_spot(z.position, k, vpm);
            let glow = 1.0 + 0.15 * (self.time * 9.0 + k as f64).sin();
            let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 0.8 + k as f64);
            out.push((eye, place(model, rune, RING_RES, Vector3::zeros(), at, turn, vpm, 0.8 * glow * fade, MOVING)));
        }
        if written > 1 {
            out.extend(self.circle_instances(eye, z.position + Vector3::new(0.0, 0.0, 0.05 * vpm), RUNE_ROUND * 0.98, look, 1.2 * fade * written as f64 / RUNE_COUNT as f64, age * 0.3));
        }
        out
    }

    /// How a Shadowstep's shade, a prism's ray and a runestone's glyph are drawn
    pub(in crate::combat) fn arsenal_shot_instances(&self, p: &Projectile, shape: Shape, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        let look = Element::look(p.profile.element);
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let far = ((p.position - eye).norm() / vpm / 15.0).max(1.0).sqrt();
        let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 9.0);
        let ball = |look: usize, size: f64| {
            let (model, ball) = self.effect_model(SLOT_FIREBALL, look);
            (eye, place(model, ball, MODEL_RES, Vector3::zeros(), p.position, spin, vpm, size * far, MOVING))
        };
        let mut out = Vec::new();
        match shape {
            // A dark shade streaking off, a long smear of shadow behind it
            Shape::Step => {
                let shadow = Element::look(Some(Element::Void));
                out.push(ball(shadow, 0.14));
                out.extend(self.line_bolts(eye, p.position - way * (p.flown / vpm).min(3.0) * vpm, p.position, shadow, 0.5, 1.8 * far, 0.05));
            }
            // A thin bright ray of its element, its head blazing
            Shape::Ray => {
                out.extend(self.line_bolts(eye, p.position - way * (p.flown / vpm).min(3.5) * vpm, p.position, look, 0.55, 1.5 * far, 0.0));
                out.extend(self.streak_at(eye, p.position + way * 0.15 * vpm, way, look, 2.2 * far));
                out.push(ball(look, 0.07));
            }
            // A glyph turning as it arcs over, a ring of its light round it
            Shape::Glyph => {
                let look = Element::look(p.profile.element.or(Some(Element::Arcane)));
                out.push(ball(look, 0.12));
                let (model, rune) = self.model_at(RUNE);
                let turn = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.time * 6.0) * UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 4.0);
                out.push((eye, place(model, rune, RING_RES, Vector3::zeros(), p.position, turn, vpm, 0.45 * far, MOVING)));
            }
            // A mote of its light, a small rune turning in it
            Shape::Sigil => {
                let look = Element::look(p.profile.element.or(Some(Element::Arcane)));
                out.push(ball(look, 0.08));
                out.extend(self.streak_at(eye, p.position - way * 0.2 * vpm, way, look, 1.0 * far));
                let (model, rune) = self.model_at(RUNE);
                let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 7.0);
                out.push((eye, place(model, rune, RING_RES, Vector3::zeros(), p.position, turn, vpm, 0.3 * far, MOVING)));
            }
            _ => {}
        }
        out
    }

    /// A prism's cast: a shard of crystal flying at the crosshair, splitting apart partway into a
    /// fan of glittering splinters, each splitting again where it strikes
    pub(in crate::combat) fn cast_prism(&mut self, arena: &mut Arena, profile: &Rc<Profile>, from: Vector3<f64>, target: Vector3<f64>, damage: f64) {
        let vpm = arena.view.vpm;
        let speed = profile.stat("projectileSpeed").max(20.0) * vpm;
        let way = (target - from).try_normalize(1e-6).unwrap_or_else(|| arena.view.forward());
        // Splitting about a third of the way to what it's aimed at, never right in the face
        let split = ((target - from).norm() * 0.35 / speed).clamp(0.12, 0.4);
        self.shoot(arena.view, profile, None, Form::Shard, from, way, speed, damage * 0.5);
        if let Some(p) = self.projectiles.last_mut() {
            p.trick = Trick::Split(split);
            p.size = 1.3;
        }
        let look = Element::look(profile.element.or(Some(Element::Frost)));
        for _ in 0..10 {
            let v = self.any_direction() * 0.9 * vpm;
            self.fine(from, v, look, 0.3, 0.0);
        }
        self.cues.push(Cue::here(Sound::Shing, 0.5).pitched(1.8));
        self.cues.push(Cue::here(Sound::Cast(look as u8), 0.4));
    }

    /// A runestone's cast: a mote of its light flying to where it's aimed, writing a rune on the
    /// ground there that blasts up a moment later
    pub(in crate::combat) fn cast_rune(&mut self, arena: &mut Arena, profile: &Rc<Profile>, from: Vector3<f64>, target: Vector3<f64>, damage: f64) {
        let vpm = arena.view.vpm;
        let aim = self.ground_aim(arena, target, profile.stat("range").max(8.0));
        let time = ((aim - from).norm() / (26.0 * vpm)).clamp(0.15, 1.0);
        self.launch(profile, None, Form::Bolt, from, thrown(from, aim, time, 0.2, vpm), damage, 0.2);
        self.signed(Sig::Power(Shape::Sigil));
        let look = Element::look(profile.element.or(Some(Element::Arcane)));
        self.sparks(from, 6, 1.2, look, -0.1, 0.35);
        self.cues.push(Cue::here(Sound::Cast(look as u8), 0.5));
        self.cues.push(Cue::here(Sound::Hex, 0.3).pitched(1.6));
    }
}
