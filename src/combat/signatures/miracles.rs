//! New magic powers, see weapons::Power: a priest's spears of holy lightning, the Vigil Spear that
//! sticks fast and erupts, Heavenfall raining them out of the sky and Triune hurling three; and
//! Stormcall calling lightning down onto the ring again and again, Glacier bursting ice up out of
//! the ground in a line, Grasping Shadows reaching up to hold what's there, and Helix, two orbs
//! spiralling round each other at the crosshair. Each is aimed while right click's held, moves the
//! weapon through its own wind-up, release and recovery, shows what it lets go of flying, and lights
//! what's round it, brighter by night. What they shoot and leave going is powers.rs's machinery.

use super::powers::{Lasting, Shape};
use super::*;
use crate::weapons::Power;

/// A Vigil Spear: how long it stays stuck crackling before it erupts, and how far its lightning
/// leaps then, in meters, to how many at most
const VIGIL_SECONDS: f64 = 1.1;
const VIGIL_REACH: f64 = 8.0;
const VIGIL_LEAPS: usize = 6;
/// Heavenfall: how many spears come down, how far apart in seconds, after how long, from how high
/// and over how far round the ring, in meters
const HEAVEN_SPEARS: f64 = 9.0;
const HEAVEN_GAP: f64 = 0.12;
const HEAVEN_DELAY: f64 = 0.45;
const HEAVEN_HEIGHT: f64 = 14.0;
pub(in crate::combat) const HEAVEN_ROUND: f64 = 4.5;
/// When Triune's three spears each leave the hand, as parts of its move, and how they fan out
pub(in crate::combat) const TRIUNE_RELEASES: [f64; 3] = [0.28, 0.45, 0.62];
pub(in crate::combat) const TRIUNE_FAN: [f64; 3] = [0.0, -0.05, 0.05];
/// Stormcall: how far round the ring it strikes, how high its thunderhead gathers, in meters, how
/// many times it strikes and how far apart, in seconds
pub(in crate::combat) const STORM_ROUND: f64 = 4.0;
const STORM_HEIGHT: f64 = 6.5;
/// Seconds Stormcall's orb takes to get up over the ring
const STORM_RISE: f64 = 0.5;
const STORM_STRIKES: f64 = 6.0;
const STORM_GAP: f64 = 0.24;
/// Glacier: how far out its first spike is, how far apart they are, how many, and seconds between
/// each, and how long the ground stays frozen after
pub(in crate::combat) const GLACIER_FROM: f64 = 2.0;
pub(in crate::combat) const GLACIER_STEP: f64 = 1.5;
pub(in crate::combat) const GLACIER_STEPS: usize = 9;
const GLACIER_GAP: f64 = 0.07;
const GLACIER_FROZEN: f64 = 3.0;
/// Grasping Shadows: how far round they reach, in meters, and for how long they hold
pub(in crate::combat) const GRASP_ROUND: f64 = 3.5;
pub(in crate::combat) const GRASP_SECONDS: f64 = 3.0;
/// Helix: how far its orbs fly, how fast, how far apart they wheel round each other, in meters,
/// and how many turns they make a meter
pub(in crate::combat) const HELIX_REACH: f64 = 30.0;
const HELIX_SPEED: f64 = 22.0;
const HELIX_RADIUS: f64 = 0.45;
const HELIX_TURNS: f64 = 0.45;

/// How long their own moves take, and when what they let go leaves the weapon
pub(in crate::combat) fn miracle_timing(power: Power) -> Option<(f64, f64)> {
    Some(match power {
        Power::Vigil => (0.9, 0.48),
        Power::Heavenfall => (1.0, 0.52),
        Power::Triune => (0.95, TRIUNE_RELEASES[0]),
        Power::Stormcall => (1.0, 0.5),
        Power::Glacier => (0.9, 0.46),
        Power::Shadowgrasp => (0.9, 0.46),
        Power::Helix => (0.9, 0.44),
        _ => return None,
    })
}

/// Every moment, as a part of its move, Triune lets a spear go
pub(in crate::combat) fn miracle_releases(power: Power) -> Option<Vec<f64>> {
    (power == Power::Triune).then(|| TRIUNE_RELEASES.to_vec())
}

/// A javelin's overhand throw: drawn far back over the shoulder as the spear forms in the hand,
/// held a beat, and hurled with the body behind it, the arm coming on down after it
pub(in crate::combat) fn hurl_keys(release: f64) -> Vec<(f64, Pose, bool)> {
    vec![
        (0.16, pose([0.34, 0.1, 0.02], [0.05, -0.3, 0.95], [1.0, 0.0, 0.0]), false),
        (0.32, pose([0.4, 0.02, 0.1], [0.04, -0.45, 0.89], [1.0, 0.0, 0.0]), false),
        (release - 0.06, pose([0.41, 0.0, 0.11], [0.05, -0.5, 0.86], [1.0, 0.0, 0.0]), false),
        (release, pose([0.16, 0.72, 0.0], [-0.05, 0.85, 0.52], [1.0, 0.0, 0.0]), true),
        (release + 0.16, pose([0.12, 0.62, -0.26], [0.0, 0.7, -0.7], [1.0, 0.0, 0.0]), false),
        (0.86, pose([0.3, 0.38, -0.3], [0.04, 0.4, 0.92], [1.0, 0.0, 0.0]), false),
    ]
}

/// Their own moves, from where the weapon's held at rest
pub(in crate::combat) fn miracle_keys(power: Power, profile: &Profile) -> Option<Vec<(f64, Pose, bool)>> {
    let (_, release) = miracle_timing(power)?;
    let at = spell_at(profile.style);
    Some(match power {
        Power::Vigil => hurl_keys(release),
        // Drawn down low, then driven straight up at the sky, the spear leaving it going up
        Power::Heavenfall => vec![
            (0.16, pose([0.3, 0.3, -0.36], [0.05, 0.35, 0.93], [1.0, 0.0, 0.0]), false),
            (release - 0.12, pose([0.3, 0.22, -0.42], [0.05, 0.2, 0.98], [1.0, 0.0, 0.0]), false),
            (release - 0.04, pose([0.26, 0.28, -0.2], [0.02, 0.15, 0.99], [1.0, 0.0, 0.0]), false),
            (release, pose([0.2, 0.42, 0.14], [0.0, 0.12, 0.99], [1.0, 0.0, 0.0]), true),
            (release + 0.16, pose([0.2, 0.42, 0.16], [0.0, 0.1, 0.99], [1.0, 0.0, 0.0]), false),
            (0.88, pose([0.27, 0.4, -0.25], [0.02, 0.3, 0.95], [1.0, 0.0, 0.0]), false),
        ],
        // Three quick throws, each drawn back and hurled
        Power::Triune => {
            let back = |p: &mut Vec<(f64, Pose, bool)>, t: f64| p.push((t, pose([0.37, 0.08, 0.06], [0.05, -0.42, 0.9], [1.0, 0.0, 0.0]), false));
            let throw = |p: &mut Vec<(f64, Pose, bool)>, t: f64| p.push((t, pose([0.17, 0.7, 0.0], [-0.05, 0.85, 0.52], [1.0, 0.0, 0.0]), true));
            let mut keys = Vec::new();
            back(&mut keys, 0.14);
            for (k, &r) in TRIUNE_RELEASES.iter().enumerate() {
                if k > 0 {
                    back(&mut keys, r - 0.07);
                }
                throw(&mut keys, r);
            }
            keys.push((0.8, pose([0.12, 0.6, -0.26], [0.0, 0.7, -0.7], [1.0, 0.0, 0.0]), false));
            keys
        }
        // Raised high and held trembling as the storm gathers, then brought down to point
        Power::Stormcall => vec![
            (0.12, at([0.0, -0.05, 0.12], [0.0, -0.3, 0.95]), false),
            (0.3, at([0.01, -0.07, 0.16], [0.02, -0.42, 0.9]), false),
            (release - 0.05, at([0.0, -0.06, 0.17], [0.0, -0.38, 0.92]), false),
            (release, at([-0.05, 0.16, -0.02], [-0.05, 0.85, 0.5]), true),
            (0.62, at([-0.05, 0.14, -0.01], [-0.05, 0.82, 0.55]), false),
            (0.86, at([-0.01, 0.03, 0.01], [-0.02, 0.4, 0.92]), false),
        ],
        // Swept low across, then driven down at the ground ahead
        Power::Glacier | Power::Shadowgrasp => vec![
            (0.15, at([0.04, -0.02, 0.06], [0.3, -0.1, 0.95]), false),
            (0.32, at([0.02, -0.05, 0.14], [0.05, -0.3, 0.95]), false),
            (release - 0.05, at([0.01, -0.05, 0.16], [0.02, -0.28, 0.96]), false),
            (release, at([-0.04, 0.2, -0.1], [-0.05, 0.7, -0.71]), true),
            (release + 0.12, at([-0.04, 0.21, -0.12], [-0.05, 0.65, -0.76]), false),
            (0.86, at([-0.01, 0.03, 0.0], [-0.02, 0.4, 0.92]), false),
        ],
        // Circled round twice as the orbs wheel round it, then thrust out at the crosshair
        Power::Helix => {
            let mut keys = Vec::new();
            for k in 0..5 {
                let a = k as f64 / 4.0 * TAU;
                keys.push((0.06 + 0.07 * k as f64, at([0.03 * a.cos(), -0.03, 0.08 + 0.03 * a.sin()], [0.15 * a.cos(), -0.1, 0.98]), false));
            }
            keys.push((release, at([-0.06, 0.16, 0.0], [-0.05, 0.92, 0.38]), true));
            keys.push((release + 0.14, at([-0.05, 0.14, 0.0], [-0.05, 0.88, 0.45]), false));
            keys.push((0.86, at([-0.01, 0.03, 0.01], [-0.02, 0.45, 0.9]), false));
            keys
        }
        _ => return None,
    })
}

impl Combat {
    /// One of the new magic powers let go, the `shot`th of what its move lets go: true if it was
    /// one of them
    pub(in crate::combat) fn unleash_miracle(&mut self, arena: &mut Arena, weapon: &Weapon, power: Power, shot: usize, base: f64) -> bool {
        let view = arena.view;
        let vpm = view.vpm;
        let up = Vector3::z();
        let profile = weapon.profile.clone();
        let look = Element::look(profile.element);
        let light = profile.element.map_or([0.95, 0.9, 0.8], Element::light);
        let feet = arena.player.unwrap_or(view.eye - up * 1.6 * vpm);
        let forward = view.forward();
        let flat = flat_of(forward);
        let target = self.aim(arena);
        match power {
            // A spear hurled at what the crosshair first meets, to stick fast there
            Power::Vigil => {
                let from = view.eye + view.rotation * Vector3::new(0.2, 0.75, 0.0) * vpm;
                let (to, who) = self.power_points(arena, power)[0];
                let range = (to - from).norm() / vpm * 1.03 + if who.is_some() { 1.0 } else { 0.3 };
                self.hurl_spear(&profile, from, to, base * 1.8, range, false);
                self.signed(Sig::Power(Shape::Vigil));
                if let Some(p) = self.projectiles.last_mut() {
                    p.pierce = 0;
                    p.size = 1.3;
                }
                if who.is_none() {
                    self.keep_power_marks(power, &[to], range / 32.0);
                }
                self.shake = self.shake.max(0.15);
            }
            // A spear hurled straight up into the sky, and spears raining down out of it onto the
            // ring a moment later
            Power::Heavenfall => {
                let from = view.eye + view.rotation * Vector3::new(0.2, 0.45, 0.2) * vpm;
                let aim = self.power_spot(arena, weapon).unwrap_or(target);
                let lasts = HEAVEN_DELAY + HEAVEN_SPEARS * HEAVEN_GAP + 0.5;
                self.keep_power_ring(weapon, aim, lasts);
                self.launch(&profile, None, Form::Bolt, from, (up * 34.0 + flat * 3.0) * vpm, 0.0, 0.0);
                self.signed(Sig::Power(Shape::Ascent));
                if let Some(p) = self.projectiles.last_mut() {
                    p.range = 11.0 * vpm;
                    p.size = 1.2;
                }
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Heavenfall)), &profile, aim, Vector3::zeros(), HEAVEN_ROUND, lasts, base * 1.2);
                self.flashes.push(Flash { position: from, color: Element::Holy.light(), reach: 6.0, life: 0.25, lifetime: 0.25 });
                self.cues.push(Cue::here(Sound::Zap, 0.8).pitched(0.8));
                self.cues.push(Cue::here(Sound::Summon, 0.6).pitched(1.3));
            }
            // A quick spear at each of what the crosshair and its fan first meet
            Power::Triune => {
                let points = self.power_points(arena, power);
                let (to, who) = points[shot.min(points.len() - 1)];
                let from = view.eye + view.rotation * Vector3::new(0.2 - 0.03 * shot as f64, 0.75, 0.0) * vpm;
                let range = (to - from).norm() / vpm * 1.03 + if who.is_some() { 3.0 } else { 0.3 };
                self.hurl_spear(&profile, from, to, base * 1.0, range, false);
                if who.is_none() {
                    self.keep_power_marks(power, &[to], range / 32.0);
                }
            }
            // An orb sent up to hang over the ring, gathering into a thunderhead there
            Power::Stormcall => {
                let from = self.held_point(weapon, view, false);
                let aim = self.power_spot(arena, weapon).unwrap_or(target);
                // Straight up over the ring, quick, gathering into a thunderhead as it gets there
                let top = aim + up * STORM_HEIGHT * vpm;
                let time = STORM_RISE;
                self.keep_power_ring(weapon, aim, time + 0.3 + STORM_STRIKES * STORM_GAP);
                self.launch(&profile, None, Form::Bolt, from, (top - from) / time, base * 1.4, 0.0);
                self.signed(Sig::Power(Shape::Storm));
                if let Some(p) = self.projectiles.last_mut() {
                    p.range = (top - from).norm();
                }
                self.cues.push(Cue::here(Sound::Summon, 0.8).pitched(0.7));
                self.cues.push(Cue::here(Sound::Rumble, 0.5).pitched(1.2));
                self.flashes.push(Flash { position: from, color: Element::Storm.light(), reach: 6.0, life: 0.25, lifetime: 0.25 });
            }
            // Ice bursting up out of the ground in a line ahead, step by step
            Power::Glacier => {
                let start = ground_under(arena.world, feet + flat * GLACIER_FROM * vpm + up * vpm, vpm);
                let line = Combat::power_line(arena, GLACIER_FROM, GLACIER_STEP, GLACIER_STEPS);
                self.keep_power_marks(power, &line, GLACIER_STEPS as f64 * GLACIER_GAP + 0.3);
                let frost = Rc::new(Profile { element: profile.element.or(Some(Element::Frost)), ..(*profile).clone() });
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Glacier)), &frost, start, flat * GLACIER_STEP * vpm, 1.4, GLACIER_STEPS as f64 * GLACIER_GAP + GLACIER_FROZEN, base * 1.3);
                self.cues.push(Cue::here(Sound::Crack, 1.0).pitched(1.2));
                self.cues.push(Cue::here(Sound::Quake, 0.6).pitched(1.5));
                self.flashes.push(Flash { position: start + up * vpm, color: Element::Frost.light(), reach: 7.0, life: 0.3, lifetime: 0.3 });
            }
            // A knot of shadow cast down onto the ring, where the tendrils come up
            Power::Shadowgrasp => {
                let from = self.held_point(weapon, view, false);
                let aim = self.power_spot(arena, weapon).unwrap_or(target);
                let time = ((aim - from).norm() / (18.0 * vpm)).max(0.3);
                self.keep_power_ring(weapon, aim, time + 0.4);
                let shadow = Rc::new(Profile { element: profile.element.or(Some(Element::Void)), ..(*profile).clone() });
                self.launch(&shadow, None, Form::Bolt, from, thrown(from, aim, time, 0.3, vpm), base * 0.55, 0.3);
                self.signed(Sig::Power(Shape::Grasp));
                self.cues.push(Cue::here(Sound::Hex, 0.8).pitched(0.6));
                self.cues.push(Cue::here(Sound::Wail, 0.3).pitched(1.4));
            }
            // Two orbs flying off at the crosshair, wheeling round each other
            Power::Helix => {
                let from = self.held_point(weapon, view, false);
                let (to, who) = self.power_points(arena, power)[0];
                let way = (to - from).try_normalize(1e-6).unwrap_or(forward);
                let range = (to - from).norm() / vpm + if who.is_some() { 1.0 } else { 0.3 };
                for phase in 0..2u8 {
                    self.launch(&profile, None, Form::Bolt, from, way * HELIX_SPEED * vpm, base * 1.1, 0.0);
                    self.signed(Sig::Power(Shape::Helix { phase }));
                    if let Some(p) = self.projectiles.last_mut() {
                        p.range = range * vpm;
                    }
                }
                if who.is_none() {
                    self.keep_power_marks(power, &[to], range / HELIX_SPEED);
                }
                self.sparks(from, 8, 1.5, look, 0.0, 0.3);
                self.cues.push(Cue::here(Sound::Cast(look as u8), 0.7));
                self.cues.push(Cue::here(Sound::Whirr, 0.4).pitched(1.6));
                self.flashes.push(Flash { position: from, color: light, reach: 5.0, life: 0.2, lifetime: 0.2 });
            }
            _ => return false,
        }
        true
    }

    /// A Helix orb on its way: wheeling round the line it flies along, round the other, further out
    /// as it leaves the hand
    pub(in crate::combat) fn helix_flies(&mut self, p: &mut Projectile, phase: u8, dt: f64) {
        let vpm = self.vpm;
        let speed = p.velocity.norm().max(1.0);
        let way = p.velocity / speed;
        let side = way.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let over = side.cross(&way);
        let offset = |flown: f64| {
            let meters = (flown / vpm).max(0.0);
            let a = meters * HELIX_TURNS * TAU + phase as f64 * PI;
            (side * a.cos() + over * a.sin()) * HELIX_RADIUS * (meters / 1.2).min(1.0) * vpm
        };
        p.position += offset(p.flown) - offset(p.flown - speed * dt);
        if random(&mut self.rng) < dt * 35.0 {
            self.mote(p.position, p.profile.element.or(Some(Element::Arcane)), 1.2);
        }
    }

    /// A Vigil Spear striking what it's thrown at, `i` at `at`: stuck fast in it, crackling
    pub(in crate::combat) fn vigil_strikes(&mut self, arena: &mut Arena, p: &Projectile, i: usize, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let profile = p.profile.clone();
        let from = p.position - p.velocity * 0.02;
        self.element_hit(at, Some(Element::Storm), 1.0);
        self.cues.push(Cue::at(Sound::Hit, at, 0.8));
        self.strike(arena, &profile, i, Blow { damage: p.damage * self.wet(arena, i), crit: false, heavy: true, from, depth: 0, element: Some(Element::Holy) });
        let center = arena.animals[i].center(vpm as u32);
        let id = arena.animals[i].id;
        self.stick_vigil(p, at, Some((id, at - center)));
    }

    /// A Vigil Spear stuck fast at `at`, in what it struck, `on` its id and where on it, or in the
    /// ground: crackling there till it erupts, see Lasting::Vigil
    pub(in crate::combat) fn stick_vigil(&mut self, p: &Projectile, at: Vector3<f64>, on: Option<(u32, Vector3<f64>)>) {
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(|| -Vector3::z());
        self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Vigil)), &p.profile, at, on.map_or(Vector3::zeros(), |o| o.1), VIGIL_REACH, VIGIL_SECONDS, p.damage);
        if let Some(z) = self.zones.last_mut() {
            z.target = on.map(|o| o.0);
            z.path = vec![way];
        }
        self.flashes.push(Flash { position: at, color: Element::Holy.light(), reach: 5.0, life: 0.2, lifetime: 0.2 });
        self.cues.push(Cue::at(Sound::ArrowHit, at, 0.9).pitched(0.7));
        self.cues.push(Cue::at(Sound::Crackle, at, 0.6).pitched(1.2));
    }

    /// How much harder lightning strikes animal `i`: half as hard again standing in water
    pub(in crate::combat) fn wet(&self, arena: &Arena, i: usize) -> f64 {
        let a = &arena.animals[i];
        let surface = (arena.water)(a.position.x, a.position.y);
        if surface.is_finite() && surface > a.position.z + 0.1 * arena.view.vpm { 1.5 } else { 1.0 }
    }

    /// One of their zones going on: true if it was one of theirs
    pub(in crate::combat) fn miracle_zone(&mut self, arena: &mut Arena, z: &mut Zone, kind: Lasting, dt: f64) -> bool {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let age = z.lifetime - z.life;
        let profile = z.profile.clone();
        let storm = Element::look(Some(Element::Storm));
        let holy = Element::look(Some(Element::Holy));
        match kind {
            // Riding along on what it's stuck in, crackling faster and faster, little forks of
            // lightning jumping off it to the ground round it
            Lasting::Vigil => {
                if let Some(id) = z.target {
                    match arena.animals.iter().position(|a| a.id == id && a.health > 0.0) {
                        Some(i) => z.position = arena.animals[i].center(vpm as u32) + z.velocity,
                        None => z.target = None,
                    }
                }
                let rate = 6.0 + 18.0 * age / VIGIL_SECONDS;
                if random(&mut self.rng) < dt * rate {
                    let to = z.position + (self.any_direction() + Vector3::new(0.0, 0.0, -0.5)) * (0.8 + random(&mut self.rng)) * vpm;
                    let look = if random(&mut self.rng) < 0.5 { storm } else { holy };
                    self.lightning(z.position, to, look, 0.035, 1, 1, 0.0, 0.0);
                }
                if random(&mut self.rng) < dt * 20.0 {
                    let v = self.any_direction() * 1.6 * vpm;
                    self.fine(z.position, v, storm, 0.2, 0.0);
                }
            }
            // Spears let fall one after another out of the sky over the ring, the first onto the
            // middle of it, the rest onto what's there or anywhere round it
            Lasting::Heavenfall => {
                while z.owed < HEAVEN_SPEARS && age >= HEAVEN_DELAY + z.owed * HEAVEN_GAP {
                    let k = z.owed as usize;
                    z.owed += 1.0;
                    let foes = Combat::foes_round(arena, z.position, z.radius);
                    let spot = if k == 0 {
                        z.position
                    } else if !foes.is_empty() && k % 2 == 1 {
                        arena.animals[foes[(k / 2) % foes.len()]].position
                    } else {
                        let a = random(&mut self.rng) * TAU;
                        let r = z.radius * random(&mut self.rng).sqrt();
                        ground_under(arena.world, z.position + Vector3::new(a.cos() * r, a.sin() * r, 1.5) * vpm, vpm)
                    };
                    let lean = Vector3::new(random(&mut self.rng) - 0.5, random(&mut self.rng) - 0.5, 0.0) * 2.5 * vpm;
                    let top = spot + up * HEAVEN_HEIGHT * vpm + lean;
                    let way = (spot - top).normalize();
                    self.launch(&profile, None, Form::Bolt, top, way * 46.0 * vpm, z.damage, 0.0);
                    self.signed(Sig::Power(Shape::Spear { big: false }));
                    if let Some(p) = self.projectiles.last_mut() {
                        p.range = (spot - top).norm() + 2.0 * vpm;
                        p.pierce = 0;
                        p.size = 1.15;
                    }
                    self.flashes.push(Flash { position: top, color: Element::Holy.light(), reach: 8.0, life: 0.15, lifetime: 0.15 });
                    self.cues.push(Cue::at(Sound::Zap, top, 0.5).pitched(0.9 + 0.05 * k as f32));
                }
            }
            // A thunderhead hanging over the ring, lightning coming down out of it onto what's under
            // it, or anywhere round, again and again
            Lasting::Storm => {
                let top = z.position + up * STORM_HEIGHT * vpm;
                if z.tick <= 0.0 {
                    z.tick = 0.35;
                    self.thunderhead(top, STORM_ROUND * 0.8, 3);
                }
                while z.owed < STORM_STRIKES && age >= 0.25 + z.owed * STORM_GAP {
                    let k = z.owed as usize;
                    z.owed += 1.0;
                    let foes = Combat::foes_round(arena, z.position, z.radius);
                    let spot = if let Some(&i) = foes.get(k % foes.len().max(1)) {
                        Combat::floor_at(arena, arena.animals[i].position + up * 0.5 * vpm)
                    } else if k == 0 {
                        z.position
                    } else {
                        let a = random(&mut self.rng) * TAU;
                        let r = z.radius * random(&mut self.rng).sqrt();
                        ground_under(arena.world, z.position + Vector3::new(a.cos() * r, a.sin() * r, 1.5) * vpm, vpm)
                    };
                    self.landed(spot);
                    self.sky_bolt(spot, storm, 1.0, 0.0);
                    self.power_burst(arena, &profile, spot + up * 0.4 * vpm, 1.4, z.damage, None);
                    // Leaping on from the nearest it struck to the next
                    let near: Vec<usize> = Combat::foes_round(arena, spot, 6.0);
                    if let (Some(&a), Some(&b)) = (near.first(), near.get(1)) {
                        let (from, to) = (arena.animals[a].center(vpm as u32), arena.animals[b].center(vpm as u32));
                        self.lightning(from, to, storm, 0.05, 1, 2, HOP_GAP, 0.4);
                        self.strike(arena, &profile, b, Blow { damage: z.damage * 0.4, crit: false, heavy: false, from, depth: 1, element: Some(Element::Storm) });
                    }
                }
            }
            // Ice bursting up a step at a time along its line, freezing what it catches, then the
            // ground along it frozen a while, chilling what stands on it
            Lasting::Glacier => {
                while (z.owed as usize) < GLACIER_STEPS && age >= z.owed * GLACIER_GAP {
                    let at = ground_under(arena.world, z.position + z.velocity * z.owed + up * 1.5 * vpm, vpm);
                    self.landed(at);
                    z.owed += 1.0;
                    z.path.push(at);
                    if (at - arena.view.eye).norm() > 3.0 * vpm {
                        self.swells.push(Swell { model: ICE_BURST, position: at, radius: 0.8, life: 0.9, lifetime: 0.9 });
                    }
                    self.sparks(at + up * 0.3 * vpm, 6, 3.0, Element::look(Some(Element::Frost)), 1.2, 0.5);
                    self.cues.push(Cue::at(Sound::Crack, at, 0.4).pitched(1.3 + 0.06 * z.owed as f32));
                    // The ground frozen under every third spike
                    if z.owed as usize % 3 == 1 {
                        self.scorch_with(at, 1.8, Element::Frost, GLACIER_FROZEN);
                    }
                    for i in Combat::foes_round(arena, at, z.radius) {
                        let id = arena.animals[i].id;
                        if z.hit.contains(&id) {
                            continue;
                        }
                        z.hit.push(id);
                        let center = arena.animals[i].center(vpm as u32);
                        self.power_hit(arena, &profile, i, center, z.damage, at);
                        if arena.animals[i].health > 0.0 && arena.animals[i].boss.is_none() {
                            arena.animals[i].status.push = up * 4.0 * vpm;
                        }
                    }
                }
                // What stands on the frozen ground slowed
                for k in 0..z.path.len() {
                    let at = z.path[k];
                    for i in Combat::foes_round(arena, at, 1.0) {
                        let s = &mut arena.animals[i].status;
                        s.slowed = s.slowed.max(0.3);
                        s.slow = s.slow.max(0.4);
                        s.chill = s.chill.max(0.3);
                    }
                }
            }
            // Holding fast all within reach, wringing it now and then, dragging what's near the edge
            // in
            Lasting::Grasp => {
                for i in Combat::foes_round(arena, z.position, z.radius) {
                    if arena.animals[i].boss.is_none() {
                        let s = &mut arena.animals[i].status;
                        s.rooted = s.rooted.max(0.2);
                    }
                }
                if z.tick <= 0.0 {
                    z.tick = 0.6;
                    for i in Combat::foes_round(arena, z.position, z.radius) {
                        let center = arena.animals[i].center(vpm as u32);
                        self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: false, from: z.position, depth: 1, element: profile.element });
                        self.sparks(center, 4, 1.5, Element::look(profile.element), -0.2, 0.4);
                        if arena.animals[i].health > 0.0 {
                            arena.animals[i].status.staggered = arena.animals[i].status.staggered.max(0.3);
                        }
                    }
                    self.cues.push(Cue::at(Sound::Hex, z.position, 0.4).pitched(0.6));
                }
                if random(&mut self.rng) < dt * 25.0 {
                    let a = random(&mut self.rng) * TAU;
                    let r = z.radius * random(&mut self.rng).sqrt();
                    let at = z.position + Vector3::new(a.cos() * r, a.sin() * r, 0.1) * vpm;
                    self.mote(at, Some(Element::Void), 1.5);
                }
            }
            _ => return false,
        }
        true
    }

    /// One of their zones whose time is out
    pub(in crate::combat) fn miracle_zone_ends(&mut self, arena: &mut Arena, z: &Zone, kind: Lasting) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let profile = z.profile.clone();
        let storm = Element::look(Some(Element::Storm));
        // A Vigil Spear erupting: lightning down onto it out of the sky, and leaping from it to
        // everything round, one after another, a burst where it stood
        if kind == Lasting::Vigil {
            let ground = Combat::floor_at(arena, z.position);
            self.landed(z.position);
            self.sky_bolt(ground, Element::look(Some(Element::Holy)), 1.3, 0.0);
            self.power_burst(arena, &profile, z.position, 2.5, z.damage * 0.8, None);
            let near: Vec<usize> = Combat::foes_round(arena, z.position, z.radius).into_iter().take(VIGIL_LEAPS).collect();
            for (k, i) in near.into_iter().enumerate() {
                let to = arena.animals[i].center(vpm as u32);
                self.lightning(z.position + up * 0.3 * vpm, to, storm, 0.07, 2, 3, k as f64 * HOP_GAP, 0.7);
                let damage = z.damage * 0.7 * self.wet(arena, i);
                self.strike(arena, &profile, i, Blow { damage, crit: false, heavy: false, from: z.position, depth: 1, element: Some(Element::Storm) });
                let s = &mut arena.animals[i].status;
                s.shocked = s.shocked.max(1.2);
                if arena.animals[i].boss.is_none() {
                    arena.animals[i].status.staggered = arena.animals[i].status.staggered.max(0.6);
                }
            }
            self.flashes.push(Flash { position: z.position + up * vpm, color: Element::Holy.light(), reach: 14.0, life: 0.35, lifetime: 0.35 });
            self.cues.push(Cue::at(Sound::Thunder, z.position, 1.0).pitched(0.8));
            self.shake = self.shake.max(0.4);
        }
    }

    /// How their shots are drawn
    pub(in crate::combat) fn miracle_shot_instances(&self, p: &Projectile, shape: Shape, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        let look = Element::look(p.profile.element);
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let far = ((p.position - eye).norm() / vpm / 15.0).max(1.0).sqrt();
        let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 7.0);
        let ball = |look: usize, size: f64| {
            let (model, ball) = self.effect_model(SLOT_FIREBALL, look);
            (eye, place(model, ball, MODEL_RES, Vector3::zeros(), p.position, spin, vpm, size * far, MOVING))
        };
        let mut out = Vec::new();
        match shape {
            Shape::Vigil | Shape::Ascent => out.extend(self.spear_instances(p, false, view)),
            // A churning orb going up, dark clouds already gathering round it, lightning in it
            Shape::Storm => {
                out.push(ball(Element::look(Some(Element::Storm)), 0.22));
                out.extend(self.ring_instances(eye, p.position, eye - p.position, 0.5 * far, Element::look(Some(Element::Storm)), 0.9 * far, self.time * 8.0));
            }
            // A knot of shadow, tendrils of it trailing
            Shape::Grasp => {
                let shadow = Element::look(Some(Element::Void));
                out.push(ball(shadow, 0.2));
                out.extend(self.line_bolts(eye, p.position - way * (p.flown / vpm).min(2.0) * vpm, p.position, shadow, 0.5, 1.4 * far, 0.12));
            }
            // An orb of its element, a streak behind it
            Shape::Helix { .. } => {
                let look = Element::look(p.profile.element.or(Some(Element::Arcane)));
                out.push(ball(look, 0.1));
                out.extend(self.streak_at(eye, p.position - way * 0.25 * vpm, way, look, 1.3 * far));
            }
            _ => {}
        }
        let _ = look;
        out
    }

    /// How their zones are drawn
    pub(in crate::combat) fn miracle_zone_instances(&self, z: &Zone, kind: Lasting, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        let age = z.lifetime - z.life;
        let up = Vector3::z();
        let mut out = Vec::new();
        match kind {
            // The spear stuck fast, point in, a ring blinking faster and faster round it
            Lasting::Vigil => {
                let way = z.path.first().copied().unwrap_or(-up);
                let (model, spear) = self.model_at(crate::effects::HOLY_SPEAR);
                let across = Vector3::z().cross(&way).try_normalize(1e-6).unwrap_or_else(Vector3::y);
                let turn = UnitQuaternion::from_rotation_matrix(&nalgebra::Rotation3::from_basis_unchecked(&[way, across, way.cross(&across)]));
                let tremble = (self.time * 60.0).sin() * 0.01 * vpm * (age / VIGIL_SECONDS);
                out.push((eye, place(model, spear, crate::effects::SPEAR_RES, Vector3::zeros(), z.position + way * 0.25 * vpm + across * tremble, turn, vpm, 1.05, MOVING)));
                if (age * (5.0 + 22.0 * age / VIGIL_SECONDS)).fract() < 0.5 {
                    out.extend(self.ring_instances(eye, z.position, eye - z.position, 0.5, Element::look(Some(Element::Holy)), 1.0, self.time * 5.0));
                }
            }
            // A ring of light on the ground where they're coming down
            Lasting::Heavenfall => {
                let fade = (z.life / 0.4).clamp(0.1, 1.0);
                out.extend(self.circle_instances(eye, z.position + up * 0.05 * vpm, z.radius, Element::look(Some(Element::Holy)), 1.5 * fade, age * 0.5));
            }
            Lasting::Storm => {
                let fade = (z.life / 0.4).clamp(0.1, 1.0);
                out.extend(self.circle_instances(eye, z.position + up * 0.05 * vpm, z.radius, Element::look(Some(Element::Storm)), 1.2 * fade, -age * 0.4));
            }
            // A glowing crack along the frozen ground
            Lasting::Glacier => {
                if let (Some(&a), Some(&b)) = (z.path.first(), z.path.last()) {
                    let fade = (z.life / 0.6).clamp(0.1, 1.0);
                    out.extend(self.line_bolts(eye, a + up * 0.1 * vpm, b + up * 0.1 * vpm, Element::look(Some(Element::Frost)), 1.2, 1.2 * fade, 0.1));
                }
            }
            // Two dark rings turning, the tendrils' hold
            Lasting::Grasp => {
                let fade = (z.life / 0.5).clamp(0.1, 1.0);
                let shadow = Element::look(Some(Element::Void));
                out.extend(self.circle_instances(eye, z.position + up * 0.05 * vpm, z.radius, shadow, 1.4 * fade, age * 1.5));
                out.extend(self.circle_instances(eye, z.position + up * 0.4 * vpm, z.radius * 0.6, shadow, 1.1 * fade, -age * 2.2));
            }
            _ => {}
        }
        out
    }

    /// What their zones light round them
    pub(in crate::combat) fn miracle_light(&self, z: &Zone, kind: Lasting) -> Option<(Vector3<f64>, f64, [f32; 3], f32)> {
        let fade = (z.life / 0.3).min(1.0) as f32;
        let age = z.lifetime - z.life;
        let up = Vector3::new(0.0, 0.0, self.vpm);
        match kind {
            Lasting::Vigil => Some((z.position, 4.0, Element::Holy.light(), (0.5 + 0.6 * age / VIGIL_SECONDS) as f32)),
            Lasting::Heavenfall => Some((z.position + up, 9.0, Element::Holy.light(), 0.5 * fade)),
            Lasting::Glacier => Some((z.position + up, 6.0, Element::Frost.light(), 0.4 * fade)),
            Lasting::Grasp => Some((z.position + up, 6.0, Element::Void.light(), 0.6 * fade)),
            _ => None,
        }
    }
}
