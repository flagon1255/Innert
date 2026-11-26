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
use super::powers::power_timing;

/// How far a talisman's great spear flies at most, and its cast one, in meters, and how fast,
/// in meters a second
pub(in crate::combat) const SPEAR_REACH: f64 = 30.0;
const CAST_REACH: f64 = 24.0;
const SPEAR_SPEED: f64 = 32.0;
/// How far a spear's drawn tipped back towards the hand it left as it leaves it, in radians, and
/// still once it's well off, see spear_axis()
const SPEAR_TILT: f64 = 0.5;
const SPEAR_TILT_FAR: f64 = 0.14;
/// How long right click's held aiming the great spear before it's as great as it gets, in seconds,
/// and how long a spear takes to form in the hand at all
pub(in crate::combat) const SPEAR_CHARGE: f64 = 1.2;
pub(in crate::combat) const SPEAR_FORMS: f64 = 0.4;

/// Which way a spear's drawn pointing, flying `way`, `flown` meters from the hand it left, which is
/// `hand` from the eye, `fallback` being out to the side and down from the eye in case its hand
/// was right in line with it: tipped back towards the hand, so it's seen side on rather than end
/// on, less the further it's gone
fn spear_axis(way: Vector3<f64>, hand: Vector3<f64>, fallback: Vector3<f64>, flown: f64) -> Vector3<f64> {
    let across = |v: Vector3<f64>| (v - way * v.dot(&way)).try_normalize(1e-3);
    let side = across(hand).or_else(|| across(fallback)).unwrap_or_else(|| way.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::y));
    let tilt = SPEAR_TILT_FAR + (SPEAR_TILT - SPEAR_TILT_FAR) * (-flown / 4.0).exp();
    // Its butt's the end towards the hand
    (way * tilt.cos() - side * tilt.sin()).normalize()
}

/// How long the lightning crackling on behind a spear is, in meters, for the cast one and the great
/// one
fn spear_trail(big: bool) -> f64 {
    if big { 7.0 } else { 4.5 }
}
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
            // A prism's crystal shard splitting apart, a runestone's rune written where it's aimed,
            // see arsenal.rs
            Rite::Prism => {
                self.cast_prism(arena, profile, from, target, damage);
                true
            }
            Rite::Rune => {
                self.cast_rune(arena, profile, from, target, damage);
                true
            }
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
        // All along its crackling trail, see spear_instances()
        let reach = (p.flown / vpm).min(spear_trail(big) + crate::effects::SPEAR_LENGTH).max(0.5);
        for _ in 0..n {
            let back = random(&mut self.rng) * reach * vpm;
            let at = p.position - way * back;
            let v = self.any_direction() * 1.8 * vpm;
            let look = if random(&mut self.rng) < 0.5 { storm } else { holy };
            self.fine(at, v, look, 0.14, 0.0);
        }
        // Forks of lightning leaping off it now and then, down to the ground or off to the side, a
        // great spear's more often and further, see lightning.rs
        let size = p.size.max(1.0);
        if random(&mut self.rng) < dt * if big { 9.0 } else { 4.0 } * size {
            let back = random(&mut self.rng) * reach.min(3.0) * vpm;
            let from = p.position - way * back;
            let jag = from + (self.any_direction() + Vector3::new(0.0, 0.0, -0.6)) * (0.8 + 0.8 * size) * vpm;
            let look = if random(&mut self.rng) < 0.7 { storm } else { holy };
            self.lightning(from, jag, look, 0.03 * size, 1, 2, 0.0, 0.0);
        }
    }

    /// How long right click's held aiming a spear before the greatest one's formed, in seconds, and
    /// how long the spear takes to form in the hand at all
    pub(in crate::combat) fn spear_charge(&self) -> f64 {
        (self.power.aimed / SPEAR_CHARGE).clamp(0.0, 1.0)
    }

    /// A spear forming in the hand, crackling light gathering into it from all round, as a spear
    /// power's aimed or wound up: sparks drawn in to the hand, `formed` of the way to whole
    pub(in crate::combat) fn spear_gathers(&mut self, hand: Vector3<f64>, formed: f64, dt: f64) {
        let storm = Element::look(Some(Element::Storm));
        let holy = Element::look(Some(Element::Holy));
        let rate = 30.0 + 40.0 * formed;
        let n = (rate * dt + random(&mut self.rng)) as usize;
        for _ in 0..n {
            let look = if random(&mut self.rng) < 0.5 { storm } else { holy };
            let d = self.any_direction();
            let at = hand + d * (0.35 + 0.25 * random(&mut self.rng)) * self.vpm;
            self.fine(at, -d * 1.6 * self.vpm, look, 0.22, 0.0);
        }
        if random(&mut self.rng) < dt * (3.0 + 8.0 * formed) {
            let to = hand + self.any_direction() * 0.45 * self.vpm;
            self.lightning(hand, to, storm, 0.012 + 0.012 * formed, 0, 1, 0.0, 0.0);
        }
    }

    /// How far formed the spear in the hand is, 0 to 1 and on past 1 as a great one's charged, if
    /// one's forming: a spear power aimed, right click held, or wound up, till it's let go
    pub(in crate::combat) fn spear_forming(&self, weapon: &Weapon) -> Option<f64> {
        let power = weapon.profile.power.filter(|p| p.spear())?;
        if self.power.aiming {
            return Some(smooth(self.power.aimed / SPEAR_FORMS) * (1.0 + 0.45 * self.spear_charge()));
        }
        match self.action {
            Action::Power { t, length } => {
                let release = power_timing(power).1 * length;
                (t < release).then(|| smooth(t / release.max(1e-3) * 1.3) * (1.0 + 0.45 * self.power.charge))
            }
            _ => None,
        }
    }

    /// The spear forming in the hand at `hand`, `formed` of the way: drawn small and growing,
    /// held overhand over the shoulder, its point on ahead, turning slowly as it crackles
    pub(in crate::combat) fn forming_spear(&self, hand: Vector3<f64>, view: &View, formed: f64) -> Vec<AnimalInstance> {
        let vpm = view.vpm;
        if formed < 0.03 {
            return Vec::new();
        }
        // Held up over the right shoulder, well out in front of the eye so it never fills the view,
        // its point on ahead and a little up
        let axis = view.rotation * Vector3::new(-0.12, 1.0, 0.22).normalize();
        let size = 0.42 * formed;
        let middle = view.eye + view.rotation * Vector3::new(0.2, 0.95, 0.06) * vpm;
        let point = middle.lerp(&hand, 0.15) + axis * crate::effects::SPEAR_LENGTH * 0.5 * size * vpm;
        let (model, spear) = self.model_at(crate::effects::HOLY_SPEAR);
        let across = (view.rotation * Vector3::x()).cross(&axis).try_normalize(1e-6).unwrap_or_else(Vector3::z);
        let turn = UnitQuaternion::from_rotation_matrix(&nalgebra::Rotation3::from_basis_unchecked(&[axis, across.cross(&axis), across]));
        let roll = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.time * 4.0);
        let flicker = 1.0 + 0.06 * (self.time * 47.0).sin();
        vec![place(model, spear, crate::effects::SPEAR_RES, Vector3::zeros(), point, turn * roll, vpm, size * flicker, HELD)]
    }

    /// Where a priest's spear's held as it's aimed: drawn back over the shoulder from `rest`,
    /// trembling with what gathers in it, more the longer it's held
    pub(in crate::combat) fn spear_aim_pose(&self, rest: &Pose) -> Pose {
        let drawn = pose([0.4, 0.02, 0.1], [0.04, -0.45, 0.89], [1.0, 0.0, 0.0]);
        let mut p = blend(rest, &drawn, smooth(self.power.aimed / 0.3));
        let shake = 0.002 + 0.005 * self.spear_charge();
        p.hand.x += shake * (self.time * 31.0).sin();
        p.hand.z += shake * (self.time * 37.0).cos();
        p
    }

    /// A spear striking animal `i` at `at`, going on through it: lightning arcing off it to what's
    /// near, and the player mended a little by it
    pub(in crate::combat) fn spear_strikes(&mut self, arena: &mut Arena, p: &mut Projectile, big: bool, i: usize, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let profile = p.profile.clone();
        let id = arena.animals[i].id;
        p.hit.push(id);
        let from = p.position - p.velocity * 0.02;
        // Harder at what's standing in water, the lightning running through it
        let wet = self.wet(arena, i);
        let blow = Blow { damage: p.damage * wet, crit: false, heavy: big, from, depth: 0, element: profile.element.or(Some(Element::Holy)) };
        self.element_hit(at, Some(Element::Holy), if big { 1.6 } else { 1.1 });
        self.element_hit(at, Some(Element::Storm), if wet > 1.0 { 1.2 } else { 0.7 });
        self.sparks(at, if big { 12 } else { 7 }, 3.5, Element::look(Some(Element::Holy)), 0.4, 0.35);
        self.flashes.push(Flash { position: at, color: Element::Storm.light(), reach: if big { 7.0 } else { 4.5 }, life: 0.14, lifetime: 0.14 });
        self.cues.push(Cue::at(Sound::Hit, at, 0.7));
        self.cues.push(Cue::at(Sound::Crackle, at, 0.6).pitched(1.2));
        self.strike(arena, &profile, i, blow);
        // Staggered, the smaller ones, as it goes through
        if arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
            arena.animals[i].status.staggered = arena.animals[i].status.staggered.max(if big { 0.6 } else { 0.3 });
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
        // Leaping on to each a moment after the last, so it's seen jumping from one to the next
        for (k, j) in near.into_iter().enumerate() {
            let to = arena.animals[j].center(vpm as u32);
            let delay = (k + 1) as f64 * HOP_GAP;
            self.lightning(at, to, storm, 0.06, 2, 3, delay, 0.6);
            self.sparks(to, 6, 3.0, storm, 0.2, 0.3);
            let damage = damage * self.wet(arena, j);
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
        // Lightning out of the sky where it ends, forked, striking again and again, a flash
        // lighting everything round, sparks and a scorch where it comes down; one coming down out
        // of the sky itself, as Heavenfall's do, bursts into lightning crackling out along the
        // ground instead
        let falling = p.velocity.try_normalize(1e-6).is_some_and(|w| w.z < -0.7);
        let size = p.size.max(1.0);
        if falling {
            for _ in 0..3 {
                let a = random(&mut self.rng) * TAU;
                let out = ground + Vector3::new(a.cos(), a.sin(), 0.05) * (1.2 + random(&mut self.rng)) * vpm;
                self.lightning(ground + up * 0.2 * vpm, out, storm, 0.045, 1, 2, 0.0, 0.0);
            }
            self.scorch(ground, 0.7);
        } else {
            self.sky_bolt(ground, storm, if big { 1.0 + 0.4 * size } else { 0.8 }, 0.0);
            if big {
                self.sky_bolt(ground + Vector3::new(0.9, -0.5, 0.0) * vpm, holy, 0.8, 0.07);
            }
        }
        self.blasts.push(Blast { position: at, look: holy, radius: if big { 1.2 + 0.4 * size } else { 0.8 }, life: 0.35, lifetime: 0.35 });
        self.flashes.push(Flash { position: at + up * vpm, color: Element::Holy.light(), reach: if big { 7.0 + 2.0 * size } else { 5.0 }, life: 0.25, lifetime: 0.25 });
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

    /// How a spear's drawn: a spear of holy light, lightning wound round its shaft and its blade
    /// blazing, crackling lightning trailing a long way behind it and fading glimmers of it along
    /// the way it came. Seen from behind as it flies off it'd be end on, only the glow of its blade
    /// showing, so it's drawn tipped back towards the hand it left: side on as it leaves the hand,
    /// and a little all the way, growing to its full length as it clears the hand.
    pub(in crate::combat) fn spear_instances(&self, p: &Projectile, big: bool, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        let holy = Element::look(Some(Element::Holy));
        let storm = Element::look(Some(Element::Storm));
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let far = ((p.position - eye).norm() / vpm / 15.0).max(1.0).sqrt();
        let flown = p.flown / vpm;
        let axis = spear_axis(way, p.position - way * p.flown - eye, view.rotation * Vector3::new(1.0, 0.0, -0.4), flown);
        // A great spear's as big as it was charged, see spear_charge()
        let size = if big { 0.94 * p.size.max(1.0) } else { p.size.max(1.0) };
        // Heavenfall's are whole as they come down out of the sky
        let grown = if way.z < -0.7 { 1.0 } else { 0.35 + 0.65 * smooth(flown / 2.5) };
        let length = crate::effects::SPEAR_LENGTH * size * grown;
        let tail = p.position - axis * length * vpm;
        let mut out = Vec::new();
        // The spear itself, turning slowly round its length, once it's clear of the eye
        if (p.position - eye).norm() > 0.8 * vpm {
            let (model, spear) = self.model_at(crate::effects::HOLY_SPEAR);
            let across = Vector3::z().cross(&axis).try_normalize(1e-6).unwrap_or_else(Vector3::y);
            let turn = UnitQuaternion::from_rotation_matrix(&nalgebra::Rotation3::from_basis_unchecked(&[axis, across, axis.cross(&across)]));
            let roll = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.time * 5.0);
            out.push((eye, place(model, spear, crate::effects::SPEAR_RES, Vector3::zeros(), p.position, turn * roll, vpm, size * grown * far, MOVING)));
        }
        // A thin glow down its shaft, and its blade blazing
        out.extend(self.line_bolts(eye, tail, p.position - axis * 0.35 * vpm, holy, 0.55, (if big { 1.0 } else { 0.7 }) * far, 0.0));
        out.extend(self.streak_at(eye, p.position + way * 0.15 * vpm, way, holy, (if big { 2.6 } else { 1.7 }) * far));
        let (model, ball) = self.effect_model(SLOT_FIREBALL, holy);
        let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 9.0);
        out.push((eye, place(model, ball, MODEL_RES, Vector3::zeros(), p.position, spin, vpm, (if big { 0.11 } else { 0.07 }) * far, MOVING)));
        // Lightning crackling on behind it from its butt, back along the way it came
        let trail = flown.min(spear_trail(big));
        if trail > 0.3 {
            out.extend(self.line_bolts(eye, p.position - way * (length + trail) * vpm, tail, storm, 0.45, 0.8 * far, if big { 0.26 } else { 0.18 }));
        }
        // Glimmers of it fading along the way it came, as if it were still there
        for k in 1..=3 {
            let back = length + 0.9 * k as f64;
            if back < flown {
                out.extend(self.streak_at(eye, p.position - way * back * vpm, way, holy, (1.3 - 0.3 * k as f64) * size * far));
            }
        }
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
        let (mut solid, mut across) = (false, 0.0f64);
        let pack = fight(&talisman, 9.0, 2.0, |_, t| Controls { attack: t < 0.7, holding: t < 0.7, ..Default::default() }, |c, view, _, _| {
            let flying: Vec<&Projectile> = c.projectiles.iter().filter(|p| p.trick == Trick::Signature(Sig::Power(Shape::Spear { big: false }))).collect();
            spears = spears.max(flying.len());
            if seen(c, view) {
                seen_for += 1.0 / 60.0;
            }
            healed = healed.max(c.heal);
            // Drawn as a spear, and side on enough to be seen as one rather than end on: how far
            // across the screen what it's drawn with spreads, while it's on the way out
            for p in flying.iter().filter(|p| p.flown / view.vpm > 1.0 && p.flown / view.vpm < 6.0) {
                let drawn = c.spear_instances(p, false, view);
                solid |= drawn.iter().any(|(_, i)| i.model() == c.first_model + crate::effects::HOLY_SPEAR as u32);
                let on: Vec<[f64; 2]> = drawn.iter().filter_map(|(_, i)| view.project(i.sphere().0)).collect();
                let spread = |k: usize| on.iter().map(|q| q[k]).fold(f64::MIN, f64::max) - on.iter().map(|q| q[k]).fold(f64::MAX, f64::min);
                if on.len() > 1 {
                    across = across.max(spread(0).hypot(spread(1)));
                }
            }
        });
        assert!(spears >= 1, "a spear was hurled");
        assert!(seen_for > 0.15, "the spear's to be seen in flight, for {seen_for:.2} s");
        assert!(solid, "a spear's drawn with a spear");
        assert!(across > 0.06, "a spear flying off is seen side on, spread {across:.3} of the screen across");
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

    /// The great spear forms in the hand as right click's held, the longer the greater: held a
    /// moment it's whole, held long it's hurled bigger and striking harder than one let go at once,
    /// never leaving the hand till the throw's wound up, and never formed with nothing aimed
    #[test]
    fn the_great_spear_forms_in_the_hand_and_charges() {
        let library = Library::load();
        let world = field();
        let vpm = 8.0;
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let feet = Vector3::new(24.0, 10.0, 4.0) * vpm;
        let view = View { eye: feet + Vector3::new(0.0, 0.0, 1.6 * vpm), rotation: UnitQuaternion::identity(), tan: [1.2445, 0.7002], vpm, speed: 0.0, stride: 0.0 };
        let talisman = weapon(&library, "talisman");
        assert_eq!(talisman.profile.power, Some(Power::Thunderspear));
        // Held `hold` seconds: how formed it got, and the spear hurled, its size and damage
        let throw = |hold: f64| {
            let mut animals = Vec::new();
            let mut combat = Combat::new(1, 0);
            let dt = 1.0 / 60.0;
            let mut formed: f64 = 0.0;
            let mut spear = None;
            for frame in 0..((hold + 2.0) / dt) as usize {
                let t = frame as f64 * dt - 0.5;
                let controls = Controls { heavy: (0.0..dt).contains(&t), aiming: t >= 0.0 && t < hold, ..Default::default() };
                let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet), water: &water };
                combat.update(&mut arena, Some((0, &talisman)), None, controls, dt);
                if t < 0.0 {
                    assert!(combat.spear_forming(&talisman).is_none(), "a spear forming before it's aimed");
                }
                formed = formed.max(combat.spear_forming(&talisman).unwrap_or(0.0));
                if spear.is_none()
                    && let Some(p) = combat.projectiles.iter().find(|p| matches!(p.trick, Trick::Signature(Sig::Power(Shape::Spear { big: true }))))
                {
                    assert!(t >= hold, "hurled before right click was let go");
                    spear = Some((p.size, p.damage));
                }
            }
            (formed, spear.expect("no spear hurled"))
        };
        let (quick_formed, (quick_size, quick_damage)) = throw(0.15);
        let (full_formed, (full_size, full_damage)) = throw(1.4);
        assert!(quick_formed > 0.2, "a quick spear's never seen forming, {quick_formed:.2}");
        assert!(full_formed > 1.2, "a charged spear's only {full_formed:.2} formed");
        assert!(full_size > quick_size + 0.5 && full_damage > quick_damage * 1.5, "charged {full_size:.2} {full_damage:.1}, quick {quick_size:.2} {quick_damage:.1}");
    }

    /// A spear's drawn with its butt towards the hand it left, tipped well over as it leaves it
    /// and less so further off, but never end on
    #[test]
    fn a_spear_is_drawn_side_on_from_the_hand() {
        let way = Vector3::y();
        let hand = Vector3::new(0.3, 0.5, -0.3);
        let fallback = Vector3::new(1.0, 0.0, -0.4);
        let near = spear_axis(way, hand, fallback, 0.5);
        let far = spear_axis(way, hand, fallback, 20.0);
        assert!(near.x < 0.0 && near.z > 0.0, "its butt's back towards the hand, out right and down: {near:?}");
        assert!(near.dot(&way) < far.dot(&way), "it's tipped less the further it's gone");
        assert!(far.dot(&way) < 0.995, "but never end on");
        // From right in front of the eye, it's tipped out to the side all the same
        assert!(spear_axis(way, way * 0.5, fallback, 0.5).x < -0.2);
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
