//! Lightning for any weapon that casts, see weapons::Power, beside a priest's spears: Forkbolt,
//! a bolt splitting at each enemy it strikes into two leaping on to the next; the Crackling Orb,
//! a ball of lightning drifting out and lashing whatever comes near; the Stormlash, a crackling
//! tether from the hand to what's aimed at, wringing it; the Static Field, ground that shocks
//! whatever walks onto it; Thunderstep, a dash on ahead in a flash, lightning striking all along
//! the way it went; and Overcharge, held to gather a charge that breaks out all round as it's let
//! go, the longer the harder. Every bolt jagged and forked, see lightning.rs, each leap of a chain
//! seen going, flashes lighting what's round, thunder, and harder on what stands in water.

use super::powers::{Lasting, Shape};
use super::*;
use crate::weapons::Power;

/// How far Forkbolt flies, how fast, how far it looks for the next to split on to, in meters, and
/// how many times it splits
pub(in crate::combat) const FORK_REACH: f64 = 30.0;
const FORK_SPEED: f64 = 42.0;
const FORK_LEAP: f64 = 9.0;
const FORK_SPLITS: u8 = 2;
/// The Crackling Orb: how far it drifts, how fast, how far round it lashes out, in meters, and how
/// often
pub(in crate::combat) const ORB_REACH: f64 = 16.0;
const ORB_SPEED: f64 = 8.0;
const ORB_LASH: f64 = 4.5;
const ORB_TICK: f64 = 0.22;
/// The Stormlash: how far it reaches, how long it holds, how often it bites, and how far what it's
/// on can get before it snaps, in meters and seconds
pub(in crate::combat) const TETHER_REACH: f64 = 18.0;
const TETHER_SECONDS: f64 = 2.6;
const TETHER_TICK: f64 = 0.25;
const TETHER_SNAPS: f64 = 24.0;
/// The Static Field: how far round it is, how long it lasts, how often it shocks
pub(in crate::combat) const FIELD_ROUND: f64 = 3.5;
const FIELD_SECONDS: f64 = 4.0;
const FIELD_TICK: f64 = 0.4;
/// Thunderstep: how far the dash goes at most, and where the strikes along its way fall, in meters
pub(in crate::combat) const STEP_FROM: f64 = 1.5;
pub(in crate::combat) const STEP_GAP: f64 = 2.0;
pub(in crate::combat) const STEP_STRIKES: usize = 5;
const STEP_EVERY: f64 = 0.06;
/// Overcharge: how far its discharge reaches, in meters, and how fast it breaks out, in meters a
/// second
pub(in crate::combat) const CHARGE_REACH: f64 = 10.0;
const CHARGE_SPEED: f64 = 18.0;

/// How long their own moves take, and when what they let go leaves the weapon
pub(in crate::combat) fn tempest_timing(power: Power) -> Option<(f64, f64)> {
    Some(match power {
        Power::Forkbolt => (0.8, 0.38),
        Power::CracklingOrb => (0.95, 0.46),
        Power::Stormlash => (0.8, 0.36),
        Power::StaticField => (0.9, 0.46),
        Power::Thunderstep => (0.7, 0.26),
        Power::Overcharge => (0.9, 0.4),
        _ => return None,
    })
}

/// Their own moves, from where the weapon's held at rest
pub(in crate::combat) fn tempest_keys(power: Power, profile: &Profile) -> Option<Vec<(f64, Pose, bool)>> {
    let (_, release) = tempest_timing(power)?;
    let at = spell_at(profile.style);
    Some(match power {
        // Drawn back to the shoulder, crackling, and flung out at the crosshair
        Power::Forkbolt | Power::Stormlash => vec![
            (0.12, at([0.04, -0.06, 0.08], [0.2, -0.25, 0.95]), false),
            (release - 0.06, at([0.05, -0.08, 0.1], [0.25, -0.35, 0.9]), false),
            (release, at([-0.06, 0.18, -0.02], [-0.08, 0.95, 0.3]), true),
            (release + 0.12, at([-0.05, 0.16, -0.02], [-0.06, 0.92, 0.38]), false),
            (0.86, at([-0.01, 0.03, 0.01], [-0.02, 0.4, 0.92]), false),
        ],
        // Cupped in close as the orb gathers, then pushed off ahead, slow
        Power::CracklingOrb => vec![
            (0.15, at([0.02, -0.05, 0.04], [0.0, 0.1, 1.0]), false),
            (0.3, at([0.01, -0.07, 0.06], [0.0, 0.0, 1.0]), false),
            (release - 0.05, at([0.01, -0.06, 0.07], [0.02, 0.05, 1.0]), false),
            (release, at([-0.04, 0.16, 0.0], [-0.04, 0.8, 0.6]), true),
            (release + 0.16, at([-0.04, 0.15, 0.0], [-0.04, 0.78, 0.62]), false),
            (0.86, at([-0.01, 0.03, 0.01], [-0.02, 0.4, 0.92]), false),
        ],
        // Raised, then driven down at the ground ahead
        Power::StaticField => vec![
            (0.15, at([0.0, -0.04, 0.12], [0.0, -0.2, 0.98]), false),
            (release - 0.05, at([0.01, -0.05, 0.17], [0.02, -0.28, 0.96]), false),
            (release, at([-0.04, 0.2, -0.1], [-0.05, 0.7, -0.71]), true),
            (release + 0.12, at([-0.04, 0.21, -0.12], [-0.05, 0.65, -0.76]), false),
            (0.86, at([-0.01, 0.03, 0.0], [-0.02, 0.4, 0.92]), false),
        ],
        // Swept back low and thrust on ahead as the dash goes
        Power::Thunderstep => vec![
            (0.1, at([0.04, -0.08, -0.04], [0.2, 0.3, 0.93]), false),
            (release - 0.05, at([0.05, -0.1, -0.06], [0.25, 0.2, 0.95]), false),
            (release, at([-0.06, 0.2, -0.04], [-0.06, 0.96, 0.25]), true),
            (release + 0.2, at([-0.05, 0.18, -0.03], [-0.05, 0.94, 0.3]), false),
            (0.88, at([-0.01, 0.03, 0.01], [-0.02, 0.4, 0.92]), false),
        ],
        // Held high, the charge breaking out of it, then thrust down to set it loose all round
        Power::Overcharge => vec![
            (0.1, at([0.0, -0.06, 0.14], [0.0, -0.3, 0.95]), false),
            (release - 0.05, at([0.01, -0.07, 0.18], [0.02, -0.36, 0.93]), false),
            (release, at([-0.03, 0.2, -0.12], [-0.04, 0.65, -0.76]), true),
            (release + 0.15, at([-0.03, 0.21, -0.14], [-0.04, 0.6, -0.8]), false),
            (0.86, at([-0.01, 0.03, 0.0], [-0.02, 0.4, 0.92]), false),
        ],
        _ => return None,
    })
}

impl Combat {
    /// One of them let go: true if it was one of them
    pub(in crate::combat) fn unleash_tempest(&mut self, arena: &mut Arena, weapon: &Weapon, power: Power, base: f64) -> bool {
        let view = arena.view;
        let vpm = view.vpm;
        let up = Vector3::z();
        let storm_profile = Rc::new(Profile { element: Some(Element::Storm), ..(*weapon.profile).clone() });
        let profile = &storm_profile;
        let storm = Element::look(Some(Element::Storm));
        let feet = arena.player.unwrap_or(view.eye - up * 1.6 * vpm);
        let forward = view.forward();
        let flat = flat_of(forward);
        let from = self.held_point(weapon, view, false);
        let crackle = |c: &mut Combat| {
            c.flashes.push(Flash { position: from, color: Element::Storm.light(), reach: 5.0, life: 0.18, lifetime: 0.18 });
            c.cues.push(Cue::here(Sound::Zap, 0.8).pitched(0.9));
        };
        match power {
            Power::Forkbolt | Power::CracklingOrb | Power::Stormlash => {
                let (to, who) = self.power_points(arena, power)[0];
                let way = (to - from).try_normalize(1e-6).unwrap_or(forward);
                let (shape, speed, extra) = match power {
                    Power::Forkbolt => (Shape::Fork { splits: FORK_SPLITS }, FORK_SPEED, if who.is_some() { 1.0 } else { 0.3 }),
                    Power::CracklingOrb => (Shape::Crackle, ORB_SPEED, 0.0),
                    _ => (Shape::Lash, 34.0, if who.is_some() { 1.0 } else { 0.3 }),
                };
                let range = (to - from).norm() / vpm + extra;
                let damage = match power {
                    Power::Forkbolt => base * 1.4,
                    Power::CracklingOrb => base * 0.5,
                    _ => base * 0.45,
                };
                self.launch(profile, None, Form::Bolt, from, way * speed * vpm, damage, 0.0);
                self.signed(Sig::Power(shape));
                if let Some(p) = self.projectiles.last_mut() {
                    p.range = range * vpm;
                }
                if who.is_none() {
                    self.keep_power_marks(power, &[to], range / speed);
                }
                self.lightning(from, from + way * 1.2 * vpm, storm, 0.025, 1, 2, 0.0, 0.0);
                crackle(self);
            }
            // A charge of it cast down onto the ring, where the ground's charged
            Power::StaticField => {
                let aim = self.power_spot(arena, weapon).unwrap_or_else(|| self.aim(arena));
                let time = ((aim - from).norm() / (20.0 * vpm)).max(0.3);
                self.keep_power_ring(weapon, aim, time + 0.4);
                self.launch(profile, None, Form::Bolt, from, thrown(from, aim, time, 0.25, vpm), base * 0.5, 0.25);
                self.signed(Sig::Power(Shape::Static));
                crackle(self);
            }
            // Gone on ahead in a flash, lightning striking all along the way
            Power::Thunderstep => {
                let line = Combat::power_line(arena, STEP_FROM, STEP_GAP, STEP_STRIKES);
                let go = Combat::room_ahead(arena, feet, flat).min(STEP_FROM + STEP_GAP * (STEP_STRIKES - 1) as f64);
                self.dash = flat * (go / 0.31).min(34.0) * vpm + up * 1.2 * vpm;
                self.keep_power_marks(power, &line, STEP_STRIKES as f64 * STEP_EVERY + 0.2);
                let start = line.first().copied().unwrap_or(feet);
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Trail)), profile, start, flat * STEP_GAP * vpm, 1.6, STEP_STRIKES as f64 * STEP_EVERY + 0.3, base * 1.1);
                if let Some(z) = self.zones.last_mut() {
                    z.path = line;
                }
                // Sparks streaming off where you were
                for _ in 0..10 {
                    let v = (self.any_direction() * 1.5 - flat * 2.0) * vpm;
                    let at = feet + up * (0.5 + random(&mut self.rng)) * vpm;
                    self.fine(at, v, storm, 0.4, 0.0);
                }
                self.cues.push(Cue::here(Sound::Dodge, 1.0).pitched(1.2));
                self.cues.push(Cue::here(Sound::Thunder, 0.5).pitched(1.4));
                crackle(self);
            }
            // The charge breaking out all round, harder the longer it was gathered, leaping to
            // each enemy as it reaches it, see Lasting::Discharge
            Power::Overcharge => {
                let charge = self.power.charge;
                let center = feet + up * 0.9 * vpm;
                self.landed(feet);
                self.keep_power_marks(power, &[feet + up * 0.05 * vpm], 0.4);
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Discharge)), profile, feet + up * 0.05 * vpm, Vector3::zeros(), CHARGE_REACH, CHARGE_REACH / CHARGE_SPEED + 0.3, base * (1.2 + 1.3 * charge));
                if let Some(z) = self.zones.last_mut() {
                    z.owed = charge;
                }
                // Forks of it crackling out all round whether there's anything to strike or not
                for k in 0..6 {
                    let a = k as f64 / 6.0 * TAU + random(&mut self.rng) * 0.5;
                    let to = ground_under(arena.world, feet + Vector3::new(a.cos(), a.sin(), 0.0) * (3.0 + 2.0 * charge) * vpm + up * vpm, vpm);
                    self.lightning(center, to, storm, 0.05, 1, 2, 0.02 * k as f64, 0.0);
                }
                self.flashes.push(Flash { position: center, color: Element::Storm.light(), reach: 8.0 + 6.0 * charge, life: 0.35, lifetime: 0.35 });
                self.cues.push(Cue::here(Sound::Thunder, 1.0).pitched(1.0 - 0.2 * charge as f32));
                self.shake = self.shake.max(0.3 + 0.3 * charge);
            }
            _ => return false,
        }
        true
    }

    /// Overcharge gathering in the weapon while right click's held: sparks drawn in to it, arcs
    /// crackling off it, down to the ground round the feet once it's well charged, more and
    /// brighter the longer it's held
    pub(in crate::combat) fn overcharge_gathers(&mut self, arena: &Arena, hand: Vector3<f64>, charge: f64, dt: f64) {
        let vpm = arena.view.vpm;
        let storm = Element::look(Some(Element::Storm));
        let n = ((25.0 + 50.0 * charge) * dt + random(&mut self.rng)) as usize;
        for _ in 0..n {
            let d = self.any_direction();
            let at = hand + d * (0.4 + 0.3 * random(&mut self.rng)) * vpm;
            self.fine(at, -d * 1.8 * vpm, storm, 0.22, 0.0);
        }
        if random(&mut self.rng) < dt * (4.0 + 10.0 * charge) {
            let to = hand + self.any_direction() * (0.4 + 0.3 * charge) * vpm;
            self.lightning(hand, to, storm, 0.015 + 0.015 * charge, 0, 1, 0.0, 0.0);
        }
        if charge > 0.3 && random(&mut self.rng) < dt * 6.0 * charge {
            let feet = arena.player.unwrap_or(arena.view.eye - Vector3::z() * 1.6 * vpm);
            let a = random(&mut self.rng) * TAU;
            let to = ground_under(arena.world, feet + Vector3::new(a.cos(), a.sin(), 0.0) * (1.5 + 2.0 * random(&mut self.rng)) * vpm + Vector3::z() * vpm, vpm);
            self.lightning(hand, to, storm, 0.03, 1, 1, 0.0, 0.0);
            self.cues.push(Cue::here(Sound::Zap, 0.3 + 0.3 * charge as f32).pitched(1.4));
        }
    }

    /// One of their shots on its way: the orb lashing out at what's near as it drifts, sparks
    /// streaming off the rest. True if it's gone.
    pub(in crate::combat) fn tempest_flies(&mut self, arena: &mut Arena, p: &mut Projectile, shape: Shape, dt: f64) -> bool {
        let vpm = arena.view.vpm;
        let storm = Element::look(Some(Element::Storm));
        if random(&mut self.rng) < dt * 30.0 {
            let at = p.position + self.any_direction() * 0.2 * vpm;
            self.mote(at, Some(Element::Storm), 1.2);
        }
        if shape == Shape::Crackle {
            // Every ORB_TICK seconds of its drift
            let step = ORB_SPEED * vpm * ORB_TICK;
            let was = p.flown - p.velocity.norm() * dt;
            if (p.flown / step).floor() > (was / step).floor() {
                let profile = p.profile.clone();
                let near: Vec<usize> = Combat::foes_round(arena, p.position, ORB_LASH).into_iter().take(2).collect();
                for i in near {
                    let to = arena.animals[i].center(vpm as u32);
                    self.lightning(p.position, to, storm, 0.05, 1, 2, 0.0, 0.0);
                    let damage = p.damage * self.wet(arena, i);
                    self.strike(arena, &profile, i, Blow { damage, crit: false, heavy: false, from: p.position, depth: 1, element: Some(Element::Storm) });
                    arena.animals[i].status.shocked = arena.animals[i].status.shocked.max(0.5);
                    self.cues.push(Cue::at(Sound::Zap, to, 0.4).pitched(1.3));
                }
                // And little forks off it to the ground now and then
                if random(&mut self.rng) < 0.5 {
                    let to = p.position + (self.any_direction() + Vector3::new(0.0, 0.0, -0.8)) * 1.2 * vpm;
                    self.lightning(p.position, to, storm, 0.03, 0, 1, 0.0, 0.0);
                }
            }
        }
        false
    }

    /// One of their shots striking animal `i` at `at`. True if it's gone.
    pub(in crate::combat) fn tempest_strikes(&mut self, arena: &mut Arena, p: &mut Projectile, shape: Shape, i: usize, at: Vector3<f64>) -> bool {
        let profile = p.profile.clone();
        let id = arena.animals[i].id;
        let from = p.position - p.velocity * 0.02;
        match shape {
            // Struck, and splitting into two leaping on to the next nearest, each a moment after
            Shape::Fork { splits } => {
                p.hit.push(id);
                let damage = p.damage * self.wet(arena, i);
                self.power_hit(arena, &profile, i, at, damage, from);
                self.flashes.push(Flash { position: at, color: Element::Storm.light(), reach: 5.0, life: 0.15, lifetime: 0.15 });
                self.cues.push(Cue::at(Sound::Crackle, at, 0.7).pitched(1.1));
                self.fork_on(arena, p, at, splits, FORK_LEAP);
                true
            }
            // A tether caught on what it struck, crackling from the hand to it, see Lasting::Tether
            Shape::Lash => {
                self.power_hit(arena, &profile, i, at, p.damage, from);
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Tether)), &profile, at, Vector3::zeros(), TETHER_SNAPS, TETHER_SECONDS, p.damage);
                if let Some(z) = self.zones.last_mut() {
                    z.target = Some(id);
                }
                self.cues.push(Cue::at(Sound::Taut, at, 0.7).pitched(1.6));
                true
            }
            // The orb goes on through, lashing out as it does
            Shape::Crackle => {
                p.hit.push(id);
                self.power_hit(arena, &profile, i, at, p.damage, from);
                false
            }
            // A charge struck something on its way down: the ground charged under it
            Shape::Static => {
                self.tempest_lands(arena, p, shape, at);
                true
            }
            _ => true,
        }
    }

    /// A Forkbolt `p` splitting at `at` into two, leaping on to the two nearest within `reach`
    /// meters it hasn't struck yet, each a moment after, `splits` more times to go
    fn fork_on(&mut self, arena: &Arena, p: &Projectile, at: Vector3<f64>, splits: u8, reach: f64) {
        if splits == 0 {
            return;
        }
        let vpm = arena.view.vpm;
        let storm = Element::look(Some(Element::Storm));
        let next: Vec<usize> = Combat::foes_round(arena, at, reach).into_iter().filter(|&j| !p.hit.contains(&arena.animals[j].id)).take(2).collect();
        for j in next {
            let to = arena.animals[j].center(vpm as u32);
            let way = (to - at).try_normalize(1e-6).unwrap_or_else(Vector3::z);
            self.lightning(at, to, storm, 0.05, 1, 2, HOP_GAP, 0.0);
            self.launch(&p.profile, None, Form::Bolt, at + way * 0.4 * vpm, way * FORK_SPEED * vpm, p.damage * 0.75, 0.0);
            self.signed(Sig::Power(Shape::Fork { splits: splits - 1 }));
            if let Some(n) = self.projectiles.last_mut() {
                n.hit = p.hit.clone();
                n.range = (to - at).norm() + 2.0 * vpm;
            }
        }
    }

    /// One of their shots coming down at `at`, or as far as it goes
    pub(in crate::combat) fn tempest_lands(&mut self, arena: &mut Arena, p: &Projectile, shape: Shape, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let storm = Element::look(Some(Element::Storm));
        let profile = p.profile.clone();
        match shape {
            // Struck down into the ground, leaping on from there to what's near
            Shape::Fork { splits } => {
                self.power_pop(arena, &profile, at, 1.2, p.damage * 0.4, None);
                self.scorch(Combat::floor_at(arena, at), 0.6);
                self.fork_on(arena, p, at, splits, FORK_LEAP * 0.5);
            }
            Shape::Lash => {
                self.power_pop(arena, &profile, at, 1.2, p.damage * 0.4, None);
                self.scorch(Combat::floor_at(arena, at), 0.6);
            }
            // The orb bursting as far as it drifts, a bolt down onto it
            Shape::Crackle => {
                self.sky_bolt(Combat::floor_at(arena, at), storm, 0.8, 0.0);
                self.power_burst(arena, &profile, at, 2.2, p.damage * 2.0, None);
            }
            // The ground charged where it comes down
            Shape::Static => {
                let ground = Combat::floor_at(arena, at);
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Field)), &profile, ground, Vector3::zeros(), FIELD_ROUND, FIELD_SECONDS, p.damage);
                self.lightning(ground + Vector3::new(0.0, 0.0, 0.2 * vpm), ground + Vector3::new(FIELD_ROUND * 0.7, 0.0, 0.1) * vpm, storm, 0.05, 2, 2, 0.0, 1.0);
                self.cues.push(Cue::at(Sound::Thunder, ground, 0.6).pitched(1.3));
                self.scorch(ground, FIELD_ROUND * 0.6);
            }
            _ => {}
        }
    }

    /// One of their zones going on: true if it was one of theirs
    pub(in crate::combat) fn tempest_zone(&mut self, arena: &mut Arena, z: &mut Zone, kind: Lasting, dt: f64) -> bool {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let age = z.lifetime - z.life;
        let profile = z.profile.clone();
        let storm = Element::look(Some(Element::Storm));
        match kind {
            // Crackling from the hand to what it's on, biting every so often, till it snaps
            Lasting::Tether => {
                let hand = arena.view.eye + arena.view.rotation * Vector3::new(0.25, 0.6, -0.2) * vpm;
                let held = z.target.and_then(|id| arena.animals.iter().position(|a| a.id == id && a.health > 0.0));
                let Some(i) = held.filter(|&i| (arena.animals[i].center(vpm as u32) - hand).norm() < z.radius * vpm) else {
                    z.life = z.life.min(0.05);
                    return true;
                };
                z.position = arena.animals[i].center(vpm as u32);
                // A new flicker of it every few frames, down the line between them
                z.owed += dt;
                if z.owed >= 0.07 {
                    z.owed = 0.0;
                    self.lightning(hand, z.position, storm, 0.035, 1, 1, 0.0, 0.0);
                }
                if z.tick <= 0.0 {
                    z.tick = TETHER_TICK;
                    let damage = z.damage * self.wet(arena, i);
                    self.strike(arena, &profile, i, Blow { damage, crit: false, heavy: false, from: hand, depth: 1, element: Some(Element::Storm) });
                    let s = &mut arena.animals[i].status;
                    s.shocked = s.shocked.max(0.4);
                    s.slowed = s.slowed.max(0.4);
                    s.slow = s.slow.max(0.35);
                    self.sparks(z.position, 4, 3.0, storm, 0.3, 0.25);
                    self.cues.push(Cue::at(Sound::Zap, z.position, 0.35).pitched(1.2 + 0.1 * random(&mut self.rng) as f32));
                }
            }
            // Arcs crawling over the charged ground, and a bolt up into whatever walks onto it
            Lasting::Field => {
                if random(&mut self.rng) < dt * 8.0 {
                    let a = random(&mut self.rng) * TAU;
                    let b = a + 1.0 + random(&mut self.rng) * 2.0;
                    let r = z.radius * vpm;
                    let p = |t: f64, k: f64| z.position + Vector3::new(t.cos(), t.sin(), 0.0) * r * k + up * 0.08 * vpm;
                    let (ka, kb) = (0.3 + 0.6 * random(&mut self.rng), 0.3 + 0.6 * random(&mut self.rng));
                    self.lightning(p(a, ka), p(b, kb), storm, 0.03, 1, 1, 0.0, 0.0);
                }
                if z.tick <= 0.0 {
                    z.tick = FIELD_TICK;
                    for i in Combat::foes_round(arena, z.position, z.radius) {
                        let center = arena.animals[i].center(vpm as u32);
                        let foot = Combat::floor_at(arena, arena.animals[i].position + up * 0.5 * vpm);
                        self.lightning(foot, center, storm, 0.045, 1, 2, 0.0, 0.0);
                        let damage = z.damage * self.wet(arena, i);
                        self.strike(arena, &profile, i, Blow { damage, crit: false, heavy: false, from: foot, depth: 1, element: Some(Element::Storm) });
                        let s = &mut arena.animals[i].status;
                        s.shocked = s.shocked.max(0.6);
                        s.slowed = s.slowed.max(0.5);
                        s.slow = s.slow.max(0.4);
                    }
                }
            }
            // Lightning striking down one step after another along the way the dash went
            Lasting::Trail => {
                while (z.owed as usize) < STEP_STRIKES && age >= z.owed * STEP_EVERY {
                    let k = z.owed as usize;
                    let at = z.path.get(k).copied().unwrap_or_else(|| ground_under(arena.world, z.position + z.velocity * z.owed + up * 1.5 * vpm, vpm));
                    self.landed(at);
                    z.owed += 1.0;
                    let top = at + up * 4.0 * vpm;
                    self.lightning(top, at, storm, 0.06, 1, 2, 0.0, if z.owed as usize == STEP_STRIKES { 1.0 } else { 0.0 });
                    self.sparks(at + up * 0.2 * vpm, 5, 3.0, storm, 0.5, 0.3);
                    for i in Combat::foes_round(arena, at, z.radius) {
                        let id = arena.animals[i].id;
                        if z.hit.contains(&id) {
                            continue;
                        }
                        z.hit.push(id);
                        let center = arena.animals[i].center(vpm as u32);
                        let damage = z.damage * self.wet(arena, i);
                        self.power_hit(arena, &profile, i, center, damage, at);
                    }
                    self.cues.push(Cue::at(Sound::Crackle, at, 0.5).pitched(1.0 + 0.1 * z.owed as f32));
                }
            }
            // Breaking out all round, lightning leaping from the middle to each enemy as the edge of
            // it reaches it, throwing it back; `owed` how charged it was
            Lasting::Discharge => {
                let front = (age * CHARGE_SPEED).min(z.radius);
                let center = z.position + up * 0.9 * vpm;
                let charge = z.owed;
                for i in Combat::foes_round(arena, z.position, front) {
                    let id = arena.animals[i].id;
                    if z.hit.contains(&id) {
                        continue;
                    }
                    z.hit.push(id);
                    let to = arena.animals[i].center(vpm as u32);
                    self.lightning(center + up * 0.3 * vpm, to, storm, 0.06 + 0.03 * charge, 2, 3, 0.0, 0.4 + 0.4 * charge);
                    let damage = z.damage * self.wet(arena, i);
                    self.strike(arena, &profile, i, Blow { damage, crit: false, heavy: true, from: center, depth: 0, element: Some(Element::Storm) });
                    let a = &mut arena.animals[i];
                    a.status.shocked = a.status.shocked.max(1.0);
                    if a.boss.is_none() && a.health > 0.0 {
                        let away = (a.position - z.position).xy().try_normalize(1e-6).map_or(Vector3::zeros(), |v| Vector3::new(v.x, v.y, 0.4));
                        a.status.push = away * (5.0 + 4.0 * charge) * vpm;
                    }
                    self.cues.push(Cue::at(Sound::Zap, to, 0.6).pitched(0.9));
                }
            }
            _ => return false,
        }
        true
    }

    /// How their shots are drawn
    pub(in crate::combat) fn tempest_shot_instances(&self, p: &Projectile, shape: Shape, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        let storm = Element::look(Some(Element::Storm));
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let far = ((p.position - eye).norm() / vpm / 15.0).max(1.0).sqrt();
        let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 11.0);
        let ball = |size: f64| {
            let (model, ball) = self.effect_model(SLOT_FIREBALL, storm);
            (eye, place(model, ball, MODEL_RES, Vector3::zeros(), p.position, spin, vpm, size * far, MOVING))
        };
        let mut out = Vec::new();
        match shape {
            Shape::Fork { .. } | Shape::Lash => {
                out.push(ball(0.07));
                out.extend(self.line_bolts(eye, p.position - way * (p.flown / vpm).min(2.5) * vpm, p.position, storm, 0.4, 1.3 * far, 0.12));
            }
            Shape::Crackle => {
                let flicker = 1.0 + 0.15 * (self.time * 37.0).sin();
                out.push(ball(0.2 * flicker));
                out.extend(self.ring_instances(eye, p.position, eye - p.position, 0.42 * far, storm, 0.8 * far, self.time * 9.0));
            }
            Shape::Static => {
                out.push(ball(0.12));
                out.extend(self.streak_at(eye, p.position - way * 0.3 * vpm, way, storm, 1.2 * far));
            }
            _ => {}
        }
        out
    }

    /// How their zones are drawn
    pub(in crate::combat) fn tempest_zone_instances(&self, z: &Zone, kind: Lasting, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let storm = Element::look(Some(Element::Storm));
        let fade = (z.life / 0.5).clamp(0.1, 1.0);
        match kind {
            Lasting::Field => {
                let age = z.lifetime - z.life;
                let mut out = self.circle_instances(view.eye, z.position + Vector3::new(0.0, 0.0, 0.06 * vpm), z.radius, storm, 1.3 * fade, age * 0.8);
                out.extend(self.circle_instances(view.eye, z.position + Vector3::new(0.0, 0.0, 0.06 * vpm), z.radius * 0.55, storm, 0.9 * fade, -age * 1.4));
                out
            }
            // The edge of it rolling out, crackling
            Lasting::Discharge => {
                let age = z.lifetime - z.life;
                let front = (age * CHARGE_SPEED).min(z.radius);
                let mut out = self.circle_instances(view.eye, z.position, front.max(0.5), storm, 1.4 * fade, age * 3.0);
                out.extend(self.circle_instances(view.eye, z.position + Vector3::new(0.0, 0.0, 0.3 * vpm), (front * 0.8).max(0.4), storm, fade, -age * 4.0));
                out
            }
            _ => Vec::new(),
        }
    }

    /// What their zones light round them
    pub(in crate::combat) fn tempest_light(&self, z: &Zone, kind: Lasting) -> Option<(Vector3<f64>, f64, [f32; 3], f32)> {
        let fade = (z.life / 0.3).min(1.0) as f32;
        let flicker = 0.7 + 0.3 * (self.time * 29.0).sin().abs() as f32;
        match kind {
            Lasting::Field => Some((z.position + Vector3::new(0.0, 0.0, self.vpm), 6.0, Element::Storm.light(), 0.7 * fade * flicker)),
            Lasting::Tether => Some((z.position, 4.0, Element::Storm.light(), 0.6 * flicker)),
            Lasting::Discharge => Some((z.position + Vector3::new(0.0, 0.0, self.vpm), 12.0, Element::Storm.light(), (0.8 + 0.5 * z.owed) as f32 * fade)),
            _ => None,
        }
    }
}
