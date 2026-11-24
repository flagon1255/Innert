//! What a priest's talisman and a necromancer's skull focus do, see weapons::Rite: the spear of holy
//! lightning a talisman hurls with every cast, and the great one on right click, through
//! everything in a line, lightning arcing off all it strikes; a skull focus's bolt of death,
//! draining what it strikes, and on right click the dead raised to fight for the player a while.
//!
//! The spears are the powers' own shots, Sig::Power(Shape::Spear), drawn and flown as theirs are,
//! see powers.rs; the dead raised are familiars, as a tome conjures, that walk the ground as
//! skeletons and strike what's near rather than hanging in the air casting, see Risen.

use super::*;
use crate::weapons::Rite;

/// How far a talisman's great spear flies at most, and its cast one, in meters, and how fast,
/// in meters a second
pub(in crate::combat) const SPEAR_REACH: f64 = 30.0;
const CAST_REACH: f64 = 24.0;
const SPEAR_SPEED: f64 = 40.0;
/// How far lightning arcs off what a spear strikes, in meters, and to how many, for the cast one
/// and the great one
const ARC_REACH: f64 = 7.0;
const ARCS: [usize; 2] = [2, 4];
/// How much of what a spear does comes back to the player as health
const SPEAR_HEAL: f64 = 0.06;
/// How far round where it's aimed the dead are raised, in meters, how many, for how long, and how
/// far off the slain lying about are raised from instead
pub(in crate::combat) const RAISE_ROUND: f64 = 2.5;
pub(in crate::combat) const RAISED: usize = 3;
pub(in crate::combat) const RAISED_SECONDS: f64 = 16.0;
const GRAVE_REACH: f64 = 9.0;
/// How long the slain lie to be raised, and how many are remembered
const GRAVE_SECONDS: f64 = 40.0;
const GRAVES: usize = 16;
/// A risen skeleton: seconds it takes to climb out of the ground and to crumble back into it, how
/// fast it walks, in meters a second, how far off it hunts and how far it reaches, in meters, how
/// long its blow takes, when in that it lands, and how much of what it does drains back
pub(in crate::combat) const RISE_SECONDS: f64 = 0.9;
const CRUMBLE_SECONDS: f64 = 0.6;
const RISEN_SPEED: f64 = 4.2;
const RISEN_HUNT: f64 = 22.0;
const RISEN_REACH: f64 = 1.3;
const BLOW_SECONDS: f64 = 0.6;
const BLOW_LANDS: f64 = 0.55;
const RISEN_DRAIN: f64 = 0.15;
/// A risen skeleton's blow next to what the skull focus's spell does
const RISEN_POWER: f64 = 0.55;

/// A familiar that's one of the dead raised by a skull focus, walking the ground as a skeleton
#[derive(Clone, Copy, Debug)]
pub(in crate::combat) struct Risen {
    pub(in crate::combat) look: Look,
    /// Which way it faces, in radians, and how far through its stride it is
    pub(in crate::combat) heading: f64,
    pub(in crate::combat) phase: f64,
    /// Whether it's walking now
    pub(in crate::combat) moving: bool,
    /// Seconds it's been up, from when it started climbing out of the ground
    pub(in crate::combat) age: f64,
    /// How far through its blow it is, if it's swinging
    pub(in crate::combat) swing: Option<f64>,
}

/// Where the slain lie, to be raised, and for how much longer, and whose they were, so none is
/// counted twice
#[derive(Default)]
pub(in crate::combat) struct Graves {
    spots: Vec<(Vector3<f64>, f64)>,
    seen: Vec<u32>,
}

impl Graves {
    pub(in crate::combat) fn recenter(&mut self, shift: Vector3<f64>) {
        for (at, _) in &mut self.spots {
            *at -= shift;
        }
    }
}

fn hostile(a: &Animal) -> bool {
    a.health > 0.0 && !a.look.is_pet() && a.look.build().is_some()
}

impl Combat {
    /// The spell of a talisman or a skull focus, see Rite, from `from` at `target`: false if
    /// `profile` has none of its own, to be cast as its style and element have it
    pub(in crate::combat) fn cast_rite(&mut self, arena: &mut Arena, profile: &Rc<Profile>, from: Vector3<f64>, target: Vector3<f64>, damage: f64) -> bool {
        let vpm = arena.view.vpm;
        match profile.rite {
            Rite::None => false,
            // A spear of holy lightning, at what the crosshair's on
            Rite::Spear => {
                let range = ((target - from).norm() / vpm + 2.5).min(CAST_REACH);
                self.hurl_spear(profile, from, target, damage, range, false);
                true
            }
            // A bolt of death, fast and dark, draining what it strikes, see the lifesteal every skull
            // focus has
            Rite::Drain => {
                let way = (target - from).try_normalize(1e-6).unwrap_or_else(|| arena.view.forward());
                let speed = profile.stat("projectileSpeed").max(20.0) * vpm;
                self.shoot(arena.view, profile, None, Form::Lance, from, way, speed, damage * 1.05);
                let look = Element::look(profile.element.or(Some(Element::Void)));
                self.sparks(from, 10, 1.6, look, -0.2, 0.45);
                for _ in 0..4 {
                    let v = self.any_direction() * 0.4 * vpm;
                    self.drop(from, v, DROP_BLOOD, 0.6);
                }
                self.flashes.push(Flash { position: from, color: profile.element.map_or(Element::Void.light(), Element::light), reach: 4.0, life: 0.2, lifetime: 0.2 });
                self.cues.push(Cue::here(Sound::Hex, 0.5).pitched(0.7));
                self.cues.push(Cue::here(Sound::Cast(look as u8), 0.4));
                true
            }
        }
    }

    /// A spear of holy lightning hurled from `from` at `target`, flying `range` meters at most,
    /// the great one of the power on right click if `big`
    pub(in crate::combat) fn hurl_spear(&mut self, profile: &Rc<Profile>, from: Vector3<f64>, target: Vector3<f64>, damage: f64, range: f64, big: bool) {
        let vpm = self.vpm;
        let way = (target - from).try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let speed = if big { SPEAR_SPEED } else { SPEAR_SPEED * 1.15 };
        self.launch(profile, None, Form::Bolt, from, way * speed * vpm, damage, 0.0);
        self.signed(Sig::Power(Shape::Spear { big }));
        if let Some(p) = self.projectiles.last_mut() {
            p.range = range * vpm;
            p.size = if big { 1.6 } else { 1.0 };
            p.pierce = u32::MAX;
        }
        let holy = Element::look(Some(Element::Holy));
        let storm = Element::look(Some(Element::Storm));
        self.sparks(from, if big { 16 } else { 8 }, 2.5, holy, 0.0, 0.3);
        for _ in 0..if big { 8 } else { 4 } {
            let v = self.any_direction() * 1.4 * vpm;
            self.fine(from, v, storm, 0.18, 0.0);
        }
        self.flashes.push(Flash { position: from, color: Element::Holy.light(), reach: if big { 8.0 } else { 5.0 }, life: 0.2, lifetime: 0.2 });
        self.cues.push(Cue::here(Sound::Zap, if big { 0.9 } else { 0.6 }).pitched(if big { 0.7 } else { 1.1 }));
        self.cues.push(Cue::here(Sound::Shing, 0.5).pitched(if big { 0.6 } else { 0.9 }));
    }

    /// A spear in flight crackling as it goes: sparks of lightning spitting off it, and a jag of it
    /// leaping out to the ground now and then
    pub(in crate::combat) fn spear_crackles(&mut self, p: &Projectile, big: bool, dt: f64) {
        let vpm = self.vpm;
        let storm = Element::look(Some(Element::Storm));
        let holy = Element::look(Some(Element::Holy));
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let rate = if big { 70.0 } else { 40.0 };
        let n = (rate * dt + random(&mut self.rng)) as usize;
        for _ in 0..n {
            let back = random(&mut self.rng) * if big { 2.4 } else { 1.4 } * vpm;
            let at = p.position - way * back;
            let v = self.any_direction() * 1.8 * vpm;
            let look = if random(&mut self.rng) < 0.5 { storm } else { holy };
            self.fine(at, v, look, 0.14, 0.0);
        }
        if big && random(&mut self.rng) < dt * 6.0 {
            let jag = p.position + (self.any_direction() + Vector3::new(0.0, 0.0, -0.6)) * 1.4 * vpm;
            self.bolt_line(p.position, jag, storm);
        }
    }

    /// A spear striking animal `i` at `at`, going on through it: lightning arcing off it to what's
    /// near, and the player mended a little by it
    pub(in crate::combat) fn spear_strikes(&mut self, arena: &mut Arena, p: &mut Projectile, big: bool, i: usize, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let profile = p.profile.clone();
        let id = arena.animals[i].id;
        p.hit.push(id);
        let from = p.position - p.velocity * 0.02;
        let blow = Blow { damage: p.damage, crit: false, heavy: big, from, depth: 0, element: profile.element.or(Some(Element::Holy)) };
        self.element_hit(at, Some(Element::Holy), if big { 1.6 } else { 1.1 });
        self.sparks(at, if big { 12 } else { 7 }, 3.5, Element::look(Some(Element::Holy)), 0.4, 0.35);
        self.cues.push(Cue::at(Sound::Hit, at, 0.7));
        self.strike(arena, &profile, i, blow);
        if big && arena.animals[i].health > 0.0 {
            arena.animals[i].status.staggered = arena.animals[i].status.staggered.max(0.5);
        }
        self.heal += p.damage * SPEAR_HEAL;
        self.arc_on(arena, &profile, at, p.damage * 0.45, ARCS[big as usize], &p.hit.clone());
        let _ = vpm;
    }

    /// Lightning arcing from `at` to as many as `count` of what's within reach of it, the nearest
    /// first, but for those in `spared`, each struck for `damage`
    fn arc_on(&mut self, arena: &mut Arena, profile: &Rc<Profile>, at: Vector3<f64>, damage: f64, count: usize, spared: &[u32]) {
        let vpm = arena.view.vpm;
        let storm = Element::look(Some(Element::Storm));
        let near: Vec<usize> = Combat::foes_round(arena, at, ARC_REACH).into_iter().filter(|&j| !spared.contains(&arena.animals[j].id)).take(count).collect();
        for j in near {
            let to = arena.animals[j].center(vpm as u32);
            self.bolt_line(at, to, storm);
            self.flashes.push(Flash { position: to, color: Element::Storm.light(), reach: 4.0, life: 0.12, lifetime: 0.12 });
            self.sparks(to, 6, 3.0, storm, 0.2, 0.3);
            self.strike(arena, profile, j, Blow { damage, crit: false, heavy: false, from: at, depth: 1, element: Some(Element::Storm) });
            let s = &mut arena.animals[j].status;
            s.shocked = s.shocked.max(1.0);
        }
        self.cues.push(Cue::at(Sound::Zap, at, 0.5).pitched(1.3));
    }

    /// A spear coming to the end of its flight at `at`, or into the ground or a wall: a bolt of
    /// lightning coming down there out of the sky, bursting and arcing to what's round
    pub(in crate::combat) fn spear_lands(&mut self, arena: &mut Arena, p: &Projectile, big: bool, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let profile = p.profile.clone();
        let up = Vector3::z();
        let storm = Element::look(Some(Element::Storm));
        let holy = Element::look(Some(Element::Holy));
        let ground = Combat::floor_at(arena, at);
        let sky = ground + up * if big { 14.0 } else { 7.0 } * vpm;
        self.bolt_line(sky, ground, storm);
        if big {
            self.bolt_line(sky + Vector3::new(0.7, -0.4, 0.0) * vpm, ground, holy);
        }
        self.blasts.push(Blast { position: at, look: holy, radius: if big { 1.6 } else { 0.8 }, life: 0.35, lifetime: 0.35 });
        self.flashes.push(Flash { position: at + up * vpm, color: Element::Holy.light(), reach: if big { 12.0 } else { 6.0 }, life: 0.3, lifetime: 0.3 });
        self.power_burst(arena, &profile, at, if big { 2.4 } else { 1.2 }, p.damage * if big { 0.6 } else { 0.35 }, None);
        self.arc_on(arena, &profile, at, p.damage * 0.35, ARCS[big as usize], &p.hit.clone());
        let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x)).unwrap_or_else(UnitQuaternion::identity);
        self.stuck.push(Stuck { position: at, rotation, look: holy, head: Some(0), quarrel: true, element: Some(Element::Holy), life: if big { 1.5 } else { 0.6 }, glowing: 1.5 });
        self.keep_stuck_few();
        self.cues.push(Cue::at(Sound::Thunder, at, if big { 0.9 } else { 0.4 }).pitched(if big { 0.9 } else { 1.3 }));
        if big {
            self.shake = self.shake.max(0.35);
        }
    }

    /// How a spear's drawn: a shaft of holy light, crackling lightning wound round it, its head
    /// blazing, a jag of lightning trailing behind
    pub(in crate::combat) fn spear_instances(&self, p: &Projectile, big: bool, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        let holy = Element::look(Some(Element::Holy));
        let storm = Element::look(Some(Element::Storm));
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let far = ((p.position - eye).norm() / vpm / 15.0).max(1.0).sqrt();
        let (length, thick) = if big { (3.0, 2.4) } else { (1.8, 1.5) };
        let mut out = self.line_bolts(eye, p.position - way * length * vpm, p.position, holy, 0.45, thick * far, 0.0);
        out.extend(self.line_bolts(eye, p.position - way * (length + 1.6) * vpm, p.position - way * 0.3 * vpm, storm, 0.4, 0.9 * far, if big { 0.22 } else { 0.14 }));
        out.extend(self.streak_at(eye, p.position + way * 0.25 * vpm, way, holy, (if big { 3.6 } else { 2.4 }) * far));
        let (model, ball) = self.effect_model(SLOT_FIREBALL, holy);
        let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 9.0);
        out.push((eye, place(model, ball, MODEL_RES, Vector3::zeros(), p.position, spin, vpm, (if big { 0.16 } else { 0.1 }) * far, MOVING)));
        out
    }

    /// The shade a skull focus's power sends to the ring, drifting there to raise the dead, from
    /// `from` to `aim`, taking `time` seconds
    pub(in crate::combat) fn send_shade(&mut self, profile: &Rc<Profile>, from: Vector3<f64>, aim: Vector3<f64>, time: f64, damage: f64) {
        let vpm = self.vpm;
        let velocity = (aim - from) / time.max(0.1) + Vector3::new(0.0, 0.0, 0.5 * 9.8 * 0.3 * time * vpm);
        self.launch(profile, None, Form::Bolt, from, velocity, damage, 0.3);
        self.signed(Sig::Power(Shape::Shade));
        let look = Element::look(profile.element.or(Some(Element::Void)));
        self.sparks(from, 14, 2.0, look, -0.2, 0.5);
        self.cues.push(Cue::here(Sound::Wail, 0.5).pitched(0.7));
        self.cues.push(Cue::here(Sound::Summon, 0.6).pitched(0.6));
    }

    /// A shade drifting on, darkness trailing off it and blood dripping
    pub(in crate::combat) fn shade_trails(&mut self, p: &Projectile, dt: f64) {
        let n = (50.0 * dt + random(&mut self.rng)) as usize;
        for _ in 0..n {
            let at = p.position + self.any_direction() * 0.2 * self.vpm;
            self.mote(at, Some(Element::Void), 1.4);
        }
        if random(&mut self.rng) < dt * 8.0 {
            self.drop(p.position, Vector3::zeros(), DROP_BLOOD, 0.8);
        }
    }

    /// How a shade's drawn: a dark orb, a ring of blood turning round it
    pub(in crate::combat) fn shade_instances(&self, p: &Projectile, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        let far = ((p.position - eye).norm() / vpm / 15.0).max(1.0).sqrt();
        let (model, ball) = self.effect_model(SLOT_FIREBALL, Element::look(Some(Element::Void)));
        let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 5.0);
        let mut out = vec![(eye, place(model, ball, MODEL_RES, Vector3::zeros(), p.position, spin, vpm, 0.24 * far, MOVING))];
        out.extend(self.ring_instances(eye, p.position, eye - p.position, 0.55 * far, Element::look(Some(Element::Blood)), 0.9 * far, self.time * 7.0));
        out
    }

    /// The shade come down at `at`: the dead raised there, out of the slain lying near if there are
    /// any, otherwise out of the ground round it, each climbing out of it
    pub(in crate::combat) fn raise_dead(&mut self, arena: &mut Arena, p: &Projectile, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let ground = Combat::floor_at(arena, at);
        let profile = p.profile.clone();
        // The slain lying nearest first, then round the ring
        let mut spots: Vec<(Vector3<f64>, bool)> = self.graves.spots.iter().filter(|(g, _)| (g - ground).xy().norm() < GRAVE_REACH * vpm).map(|(g, _)| (*g, true)).collect();
        spots.sort_by(|a, b| (a.0 - ground).norm().total_cmp(&(b.0 - ground).norm()));
        spots.truncate(RAISED);
        let turn = random(&mut self.rng) * TAU;
        for k in spots.len()..RAISED {
            let a = turn + k as f64 / RAISED as f64 * TAU;
            spots.push((ground + Vector3::new(a.cos(), a.sin(), 0.0) * RAISE_ROUND * 0.7 * vpm, false));
        }
        // The oldest of those already up crumbling away for these
        let up_now = self.familiars.iter().filter(|f| f.risen.is_some()).count();
        let mut over = (up_now + RAISED).saturating_sub(RAISED * 2);
        self.familiars.retain(|f| {
            if f.risen.is_some() && over > 0 {
                over -= 1;
                return false;
            }
            true
        });
        let damage = p.damage * RISEN_POWER;
        let toward = arena.view.eye - ground;
        for (k, (spot, grave)) in spots.into_iter().enumerate() {
            let feet = Combat::floor_at(arena, spot);
            if grave {
                self.graves.spots.retain(|(g, _)| (g - spot).norm() > 0.1 * vpm);
            }
            let look = if grave || k == 1 { Look::ArmoredSkeleton } else { Look::Skeleton };
            let heading = (-toward.y).atan2(-toward.x) + (random(&mut self.rng) - 0.5) * 0.8;
            self.familiars.push(Familiar {
                position: feet,
                life: RAISED_SECONDS + k as f64 * 0.25,
                wait: RISE_SECONDS + 0.2 * k as f64,
                profile: profile.clone(),
                damage,
                phantom: false,
                risen: Some(Risen { look, heading, phase: 0.0, moving: false, age: -0.2 * k as f64, swing: None }),
            });
            self.sparks(feet + up * 0.2 * vpm, 12, 2.4, Element::look(Some(Element::Void)), 0.6, 0.6);
            for _ in 0..6 {
                let v = (self.any_direction() + up * 1.4) * 2.5 * vpm;
                self.drop(feet + up * 0.1 * vpm, v, DROP_MUD, 1.0);
            }
            self.blasts.push(Blast { position: feet + up * 0.1 * vpm, look: Element::look(Some(Element::Blood)), radius: 0.7, life: 0.5, lifetime: 0.5 });
        }
        self.blasts.push(Blast { position: ground + up * 0.2 * vpm, look: Element::look(Some(Element::Void)), radius: RAISE_ROUND * 0.6, life: 0.6, lifetime: 0.6 });
        self.flashes.push(Flash { position: ground + up * vpm, color: [0.6, 0.12, 0.5], reach: 9.0, life: 0.5, lifetime: 0.5 });
        self.cues.push(Cue::at(Sound::Wail, ground, 0.9).pitched(0.6));
        self.cues.push(Cue::at(Sound::Quake, ground, 0.5).pitched(1.3));
        self.shake = self.shake.max(0.25);
    }

    /// Where the slain are lying, for the dead to be raised from: anything that fights, as it falls
    pub(in crate::combat) fn mark_graves(&mut self, arena: &Arena, dt: f64) {
        for (_, life) in &mut self.graves.spots {
            *life -= dt;
        }
        self.graves.spots.retain(|(_, life)| *life > 0.0);
        let alive: Vec<u32> = arena.animals.iter().filter(|a| a.health > 0.0).map(|a| a.id).collect();
        self.graves.seen.retain(|id| !alive.contains(id) && arena.animals.iter().any(|a| a.id == *id));
        for a in arena.animals.iter().filter(|a| a.health <= 0.0 && a.look.build().is_some() && !a.look.is_pet()) {
            if !self.graves.seen.contains(&a.id) {
                self.graves.seen.push(a.id);
                self.graves.spots.push((a.position, GRAVE_SECONDS));
            }
        }
        if self.graves.spots.len() > GRAVES {
            let extra = self.graves.spots.len() - GRAVES;
            self.graves.spots.drain(..extra);
        }
    }

    /// One of the dead raised, `f`, going on for `dt`: climbing out of the ground, then hunting
    /// down what's near and striking it, its blows draining life back to the player, following
    /// the player when there's nothing, and crumbling back into the ground when its time's out
    pub(in crate::combat) fn walk_risen(&mut self, arena: &mut Arena, f: &mut Familiar, dt: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let Some(mut r) = f.risen else { return };
        r.age += dt;
        f.wait -= dt;
        let bone = Element::look(Some(Element::Void));
        // Still climbing out
        if r.age < RISE_SECONDS {
            if r.age > 0.0 && random(&mut self.rng) < dt * 20.0 {
                let v = (self.any_direction() + up * 1.5) * 1.6 * vpm;
                self.drop(f.position + up * 0.1 * vpm, v, DROP_MUD, 0.8);
                self.fine(f.position + up * 0.3 * vpm, v * 0.5, bone, 0.4, -0.3);
            }
            f.risen = Some(r);
            return;
        }
        if f.life < CRUMBLE_SECONDS {
            r.moving = false;
            r.swing = None;
            if random(&mut self.rng) < dt * 25.0 {
                let v = (self.any_direction() + up) * 1.2 * vpm;
                let at = f.position + up * (0.2 + random(&mut self.rng)) * vpm;
                self.fine(at, v, bone, 0.5, 0.4);
            }
            f.risen = Some(r);
            return;
        }
        // A blow going on, landing partway through
        if let Some(s) = r.swing {
            let next = s + dt / BLOW_SECONDS;
            if s < BLOW_LANDS && next >= BLOW_LANDS {
                let reach = f.position + Vector3::new(r.heading.cos(), r.heading.sin(), 0.0) * RISEN_REACH * 0.7 * vpm + up * 0.9 * vpm;
                let struck: Vec<usize> = Combat::foes_round(arena, reach, 0.9).into_iter().filter(|&i| hostile(&arena.animals[i])).take(1).collect();
                for i in struck {
                    let at = arena.animals[i].center(vpm as u32);
                    let profile = f.profile.clone();
                    self.strike(arena, &profile, i, Blow { damage: f.damage, crit: false, heavy: false, from: f.position + up * vpm, depth: 1, element: profile.element.or(Some(Element::Void)) });
                    self.heal += f.damage * RISEN_DRAIN;
                    self.sparks(at, 8, 3.0, Element::look(Some(Element::Blood)), 0.6, 0.35);
                    let fling = self.any_direction() * vpm;
                    self.drop(at, fling, DROP_BLOOD, 0.8);
                    self.rays.push((at, f.position + up * 1.2 * vpm, Element::look(Some(Element::Blood))));
                    self.cues.push(Cue::at(Sound::Hit, at, 0.6).pitched(0.8));
                }
                self.cues.push(Cue::at(Sound::Swing, f.position, 0.4).pitched(0.8));
            }
            r.swing = (next < 1.0).then_some(next);
            r.moving = false;
            f.risen = Some(r);
            return;
        }
        // What it's after: the nearest thing that fights within its hunting reach, or else the
        // player, to keep near
        let feet = arena.player.unwrap_or(arena.view.eye - up * 1.6 * vpm);
        let prey = (0..arena.animals.len())
            .filter(|&i| hostile(&arena.animals[i]))
            .map(|i| ((arena.animals[i].position - f.position).xy().norm(), i))
            .filter(|&(d, _)| d < RISEN_HUNT * vpm)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let (goal, stop) = match prey {
            Some((_, i)) => (arena.animals[i].position, (RISEN_REACH + arena.animals[i].look.body().1 * 0.7) * vpm),
            None => (feet, 2.5 * vpm),
        };
        let to = (goal - f.position).xy();
        let d = to.norm();
        if d > stop {
            let way = to / d;
            r.heading = way.y.atan2(way.x);
            let step = (RISEN_SPEED * vpm * dt).min(d - stop);
            let next = f.position + Vector3::new(way.x, way.y, 0.0) * step;
            f.position = Combat::floor_at(arena, next);
            r.phase = (r.phase + step / (1.3 * vpm)).fract();
            r.moving = true;
        } else {
            r.moving = false;
            if d > 1e-6 {
                r.heading = to.y.atan2(to.x);
            }
            if prey.is_some() && f.wait <= 0.0 {
                r.swing = Some(0.0);
                f.wait = BLOW_SECONDS + 0.35;
            }
        }
        f.risen = Some(r);
    }

    /// How one of the dead raised is drawn: a skeleton, climbing up out of the ground, walking and
    /// striking, and sinking back into it as it crumbles; and its eyes' glow over its head
    pub(in crate::combat) fn risen_instances(&self, f: &Familiar, r: &Risen, view: &View, out: &mut Vec<(f64, AnimalInstance)>) {
        let vpm = view.vpm;
        let height = 1.9 * vpm;
        let rising = (r.age / RISE_SECONDS).clamp(0.0, 1.0);
        let sinking = (f.life / CRUMBLE_SECONDS).clamp(0.0, 1.0);
        let shown = rising.min(sinking);
        if shown <= 0.0 {
            return;
        }
        let ease = 1.0 - (1.0 - shown) * (1.0 - shown);
        let feet = f.position - Vector3::new(0.0, 0.0, (1.0 - ease) * height);
        let rotation = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), r.heading);
        let instance = crate::animals::posed_instance(r.look, feet, rotation, r.moving.then_some(r.phase), r.swing, vpm as u32);
        out.push(((f.position - view.eye).norm(), instance.tinted(if f.life < 2.0 && (f.life * 8.0).fract() < 0.5 { TINT_HIT } else { TINT_NONE })));
        // The glow of what raised it, hanging in its chest
        let (model, ball) = self.effect_model(SLOT_FIREBALL, Element::look(Some(Element::Blood)));
        let chest = feet + Vector3::new(0.0, 0.0, 1.25 * vpm);
        out.push(((chest - view.eye).norm(), place(model, ball, MODEL_RES, Vector3::zeros(), chest, rotation, vpm, 0.05 * shown, MOVING)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
use crate::weapons::Rite;
    use crate::weapons::{Library, Power};

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

    fn weapon(library: &Library, family: &str) -> Weapon {
        library.generate(11, library.family(family), 0.0)
    }

    /// A pack of bandits `ahead` meters off, and a player casting at them with `weapon`, the mouse
    /// as `controls` has it each frame, for `seconds`, with what's seen of each frame
    fn fight(weapon: &Weapon, ahead: f64, seconds: f64, controls: impl Fn(&Combat, f64) -> Controls, mut each: impl FnMut(&Combat, &View, &[Animal], f64)) -> Vec<Animal> {
        let world = field();
        let vpm = 8.0;
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let feet = Vector3::new(24.0, 10.0, 4.0) * vpm;
        let view = View { eye: feet + Vector3::new(0.0, 0.0, 1.6 * vpm), rotation: UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.1), tan: [1.2445, 0.7002], vpm, speed: 0.0, stride: 0.0 };
        let mut animals = crate::animals::spawn_pack(&world, &water, &[Look::Bandit; 3], feet + Vector3::new(0.0, ahead * vpm, 0.0), crate::animals::PACK_SPREAD, 3);
        let mut combat = Combat::new(1, 0);
        combat.mana = 1000.0;
        let dt = 1.0 / 60.0;
        for frame in 0..(seconds / dt) as usize {
            let t = frame as f64 * dt;
            let c = controls(&combat, t);
            let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet), water: &water };
            combat.update(&mut arena, Some((0, weapon)), None, c, dt);
            each(&combat, &view, &animals, t);
        }
        animals
    }

    /// Whether anything of `combat`'s flying about is on screen out past 2 m from the eye
    fn seen(combat: &Combat, view: &View) -> bool {
        combat.flying_instances(view).iter().any(|(_, i)| {
            let (at, _) = i.sphere();
            (at - view.eye).norm() > 2.0 * view.vpm && view.project(at).is_some_and(|[x, y]| (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y))
        })
    }

    /// A talisman's cast is a spear of holy lightning, flying out to be seen, through the pack,
    /// lightning arcing off it, mending the player; and its power on right click, at any rarity, a
    /// greater one, aimed while right click's held and hurled as it's let go
    #[test]
    fn a_priests_spear_flies_pierces_and_arcs() {
        let library = Library::load();
        let talisman = weapon(&library, "talisman");
        assert_eq!(talisman.profile.rite, Rite::Spear);
        assert_eq!(talisman.profile.power, Some(Power::Thunderspear), "a common talisman has its spear");
        let mut spears = 0;
        let mut seen_for = 0.0;
        let mut healed = 0.0f64;
        let pack = fight(&talisman, 9.0, 1.6, |_, t| Controls { attack: t < 0.05, holding: t < 0.05, ..Default::default() }, |c, view, _, _| {
            spears = spears.max(c.projectiles.iter().filter(|p| p.trick == Trick::Signature(Sig::Power(Shape::Spear { big: false }))).count());
            if seen(c, view) {
                seen_for += 1.0 / 60.0;
            }
            healed = healed.max(c.heal);
        });
        assert!(spears >= 1, "a spear was hurled");
        assert!(seen_for > 0.15, "the spear's to be seen in flight, for {seen_for:.2} s");
        let hurt = pack.iter().filter(|a| a.health < a.max_health).count();
        assert!(hurt >= 2, "through one and arcing on to the next, {hurt} hurt");
        // The great one, on right click: aimed while it's held, hurled as it's let go
        let mut big = false;
        let mut held_back = true;
        fight(&talisman, 9.0, 2.0, |c, t| c.power_controls(&talisman, t - 0.3, 1.0 / 60.0), |c, _, _, t| {
            let flying = c.projectiles.iter().any(|p| p.trick == Trick::Signature(Sig::Power(Shape::Spear { big: true })));
            if flying && t < 0.32 {
                held_back = false;
            }
            big |= flying;
        });
        assert!(big && held_back, "the great spear's hurled as right click's let go, not as it's pressed");
    }

    /// A skull focus's cast drains what it strikes, and its power on right click raises skeletons
    /// on the ring it was aimed at, that climb out of the ground, walk to the pack and strike it,
    /// to be seen doing it, and crumble once their time's out
    #[test]
    fn a_necromancers_dead_rise_and_fight() {
        let library = Library::load();
        let focus = weapon(&library, "skullfocus");
        assert_eq!(focus.profile.rite, Rite::Drain);
        assert!(focus.profile.has(Effect::Lifesteal), "every skull focus drains");
        assert_eq!(focus.profile.power, Some(Power::RaiseDead), "a common skull focus raises the dead");
        let mut risen = 0;
        let mut walked = false;
        let mut struck = false;
        let mut drawn = false;
        let mut shade = false;
        let pack = fight(&focus, 9.0, 7.0, |c, t| c.power_controls(&focus, t - 0.3, 1.0 / 60.0), |c, view, _, _| {
            shade |= c.projectiles.iter().any(|p| p.trick == Trick::Signature(Sig::Power(Shape::Shade)));
            let up: Vec<&Familiar> = c.familiars.iter().filter(|f| f.risen.is_some()).collect();
            risen = risen.max(up.len());
            walked |= up.iter().any(|f| f.risen.is_some_and(|r| r.moving));
            struck |= up.iter().any(|f| f.risen.is_some_and(|r| r.swing.is_some()));
            if !up.is_empty() {
                let mut out = Vec::new();
                for f in &up {
                    c.risen_instances(f, f.risen.as_ref().unwrap(), view, &mut out);
                }
                drawn |= out.iter().any(|(_, i)| view.project(i.sphere().0).is_some());
            }
        });
        assert!(shade, "a shade flew to the ring");
        assert_eq!(risen, RAISED, "{RAISED} raised");
        assert!(walked && struck && drawn, "they walked {walked}, struck {struck}, were drawn {drawn}");
        assert!(pack.iter().any(|a| a.health < a.max_health), "they hurt the pack");
        // Every look they're drawn in has the frames they're drawn with
        let models = crate::animals::animal_models();
        for look in [Look::Skeleton, Look::ArmoredSkeleton] {
            for (phase, strike) in [(None, None), (Some(0.3), None), (None, Some(0.2)), (None, Some(0.8))] {
                let i = crate::animals::posed_instance(look, Vector3::zeros(), UnitQuaternion::identity(), phase, strike, 16);
                let (_, _) = i.sphere();
                assert!(models[i.model() as usize].size != [1, 1, 1], "{look:?} {phase:?} {strike:?}");
            }
        }
    }
}
