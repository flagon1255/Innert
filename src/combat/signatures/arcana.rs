//! Powers of arcane light, death and holy light, see weapons::Power, none of them lightning nor a
//! priest's spear: Arcane Missiles, seven let go one after another, weaving in onto what's aimed
//! at, each harder than the last; the Hourglass Field, time running slow round where it's thrown,
//! what's there slowed and laid open, struck all at once as time snaps back; the Soul Harvest, an
//! orb of death through everything in a line, tearing out wisps of soul that fly back to heal; the
//! Bone Cage, spikes of bone bursting up round what's aimed at, holding it, then crashing in; the
//! Sanctuary Circle, a circle of holy light that heals whoever stands in it and sears what's in
//! it; and Radiant Smite, a brand of light thrown at what's aimed at and a pillar of light coming
//! down on it.
//!
//! Each of its own school whatever's in the weapon, let go with a move of its own, leaving the
//! weapon as something flying that comes down where it's seen to, bright enough by day. What they
//! shoot and leave going is powers.rs's machinery, shapes and lastings of their own.

use super::powers::{Lasting, Shape};
use super::*;
use crate::weapons::Power;

/// Arcane Missiles: how far they reach, how many, when each is let go as parts of the move, how
/// fast they fly at the last, in meters a second, how hard each turns onto what it's after, and
/// how much harder each that strikes the same as the last one strikes, next to the first
pub(in crate::combat) const MISSILE_REACH: f64 = 30.0;
const MISSILE_RELEASES: [f64; 7] = [0.22, 0.3, 0.38, 0.46, 0.54, 0.62, 0.7];
const MISSILE_SPEED: f64 = 30.0;
const MISSILE_TURN: f64 = 7.0;
const MISSILE_STREAK: f64 = 0.125;
/// The Hourglass Field: how far round it slows, in meters, how long, how slow, how much more what's
/// in it takes, and how often it's worn at meanwhile, in seconds
pub(in crate::combat) const GLASS_ROUND: f64 = 4.5;
const GLASS_SECONDS: f64 = 5.0;
const GLASS_SLOW: f64 = 0.66;
const GLASS_OPEN: f64 = 0.25;
const GLASS_TICK: f64 = 0.5;
/// The Soul Harvest: how far its orb flies and how fast, how fast the wisps fly home, and how
/// much of what the orb does each gives back
pub(in crate::combat) const HARVEST_REACH: f64 = 24.0;
const HARVEST_SPEED: f64 = 18.0;
const WISP_SPEED: f64 = 15.0;
const WISP_HEALS: f64 = 0.35;
/// The Bone Cage: how far its shard's flung, how far round the cage holds, in meters, how long
/// before it crashes in, and how hard that is next to the shard
pub(in crate::combat) const CAGE_REACH: f64 = 22.0;
pub(in crate::combat) const CAGE_ROUND: f64 = 2.5;
const CAGE_HOLD: f64 = 1.5;
const CAGE_CRASH: f64 = 4.0;
/// The Sanctuary Circle: how far off it can be thrown, how far round it stands, in meters, how
/// long, how often it sears, and how much it heals a second next to what it sears each time
pub(in crate::combat) const SANCTUM_REACH: f64 = 20.0;
pub(in crate::combat) const SANCTUM_ROUND: f64 = 5.0;
const SANCTUM_SECONDS: f64 = 6.0;
const SANCTUM_TICK: f64 = 0.5;
const SANCTUM_HEALS: f64 = 1.2;
/// Radiant Smite: how far it reaches and how fast its brand flies, how much of what it does comes
/// back as health, and how far round what it slays the light bursts on
pub(in crate::combat) const SMITE_REACH: f64 = 30.0;
const SMITE_SPEED: f64 = 48.0;
const SMITE_HEALS: f64 = 0.15;
const SMITE_BURST: f64 = 7.0;

/// How long their own moves take, and when what they let go leaves the weapon
pub(in crate::combat) fn arcane_timing(power: Power) -> Option<(f64, f64)> {
    Some(match power {
        Power::Missiles => (1.0, MISSILE_RELEASES[0]),
        Power::Hourglass => (0.95, 0.5),
        Power::SoulHarvest => (0.9, 0.45),
        Power::BoneCage => (0.85, 0.44),
        Power::Sanctuary => (1.0, 0.52),
        Power::Smite => (0.8, 0.4),
        _ => return None,
    })
}

/// Every moment, as a part of its move, Arcane Missiles lets one go
pub(in crate::combat) fn arcane_releases(power: Power) -> Option<Vec<f64>> {
    (power == Power::Missiles).then(|| MISSILE_RELEASES.to_vec())
}

/// Their own moves, from where the weapon's held at rest
pub(in crate::combat) fn arcane_keys(power: Power, profile: &Profile) -> Option<Vec<(f64, Pose, bool)>> {
    let (_, release) = arcane_timing(power)?;
    let at = spell_at(profile.style);
    let magic = profile.kind == Kind::Magic && profile.style != Style::Glaive;
    Some(match power {
        // Held out at the crosshair, jabbing a little one way and the other as each missile goes
        Power::Missiles => {
            let mut keys = vec![(0.12, at([0.02, -0.04, 0.06], [0.04, 0.3, 0.95]), false)];
            for (k, &r) in MISSILE_RELEASES.iter().enumerate() {
                let s = if k % 2 == 0 { 1.0 } else { -1.0 };
                keys.push((r - 0.035, at([-0.03 + 0.02 * s, 0.1, 0.02], [-0.04 + 0.1 * s, 0.82, 0.56]), false));
                keys.push((r, at([-0.05 + 0.03 * s, 0.17, 0.0], [-0.05 + 0.14 * s, 0.9, 0.42]), true));
            }
            keys.push((0.88, at([-0.01, 0.03, 0.01], [-0.02, 0.4, 0.92]), false));
            keys
        }
        // Raised and turned over, as an hourglass is, then swung down and out as it's thrown
        Power::Hourglass => vec![
            (0.12, at([0.02, -0.04, 0.1], [0.3, -0.2, 0.93]), false),
            (0.26, at([-0.02, -0.05, 0.14], [-0.35, -0.25, 0.9]), false),
            (release - 0.06, at([0.0, -0.06, 0.16], [0.0, -0.35, 0.94]), false),
            (release, at([-0.05, 0.2, 0.02], [-0.05, 0.85, 0.52]), true),
            (release + 0.14, at([-0.04, 0.17, -0.02], [-0.04, 0.8, 0.6]), false),
            (0.88, at([-0.01, 0.03, 0.01], [-0.02, 0.4, 0.92]), false),
        ],
        // A caster drawn back over the shoulder and flung out at the crosshair; a scythe swept
        // round low and up as the orb leaves its blade
        Power::SoulHarvest if magic => vec![
            (0.14, at([0.06, -0.1, 0.1], [0.2, -0.35, 0.9]), false),
            (release - 0.06, at([0.08, -0.13, 0.12], [0.25, -0.45, 0.85]), false),
            (release, at([-0.07, 0.22, 0.0], [-0.08, 0.95, 0.3]), true),
            (release + 0.14, at([-0.06, 0.2, -0.04], [-0.07, 0.9, 0.42]), false),
            (0.88, at([-0.01, 0.03, 0.01], [-0.02, 0.4, 0.92]), false),
        ],
        Power::SoulHarvest => vec![
            (0.16, pose([0.38, 0.32, -0.2], [0.85, 0.1, 0.5], [-0.5, 0.0, 0.86]), false),
            (release - 0.05, pose([0.42, 0.28, -0.24], [0.9, 0.05, 0.42], [-0.42, 0.0, 0.9]), false),
            (release, pose([0.02, 0.66, -0.2], [-0.1, 0.62, 0.78], [-1.0, 0.0, 0.1]), true),
            (release + 0.14, pose([-0.3, 0.5, -0.22], [-0.85, 0.3, 0.42], [-0.4, 0.0, -0.9]), false),
            (0.88, pose([0.2, 0.48, -0.3], [0.0, 0.55, 0.83], [0.0, 1.0, -0.1]), false),
        ],
        // A caster flicked out at the crosshair; a blade drawn back and flung out overhand, the
        // shard leaving it
        Power::BoneCage if magic => vec![
            (0.14, at([0.04, -0.08, 0.08], [0.1, -0.2, 0.97]), false),
            (release - 0.05, at([0.05, -0.1, 0.1], [0.12, -0.28, 0.95]), false),
            (release, at([-0.06, 0.2, 0.0], [-0.07, 0.93, 0.36]), true),
            (release + 0.12, at([-0.05, 0.18, -0.02], [-0.06, 0.88, 0.46]), false),
            (0.88, at([-0.01, 0.03, 0.01], [-0.02, 0.4, 0.92]), false),
        ],
        Power::BoneCage => vec![
            (0.16, pose([0.3, 0.26, 0.0], [0.1, -0.4, 0.91], [1.0, 0.0, 0.0]), false),
            (release - 0.05, pose([0.33, 0.16, 0.06], [0.08, -0.62, 0.78], [1.0, 0.0, 0.0]), false),
            (release, pose([0.14, 0.72, -0.08], [0.0, 0.92, 0.38], [1.0, 0.0, 0.0]), true),
            (release + 0.14, pose([0.1, 0.62, -0.3], [0.0, 0.62, -0.78], [1.0, 0.0, 0.0]), false),
            (0.88, pose([0.26, 0.44, -0.27], [0.1, 0.3, 0.95], [1.0, 0.0, 0.0]), false),
        ],
        // Held up and shown, as a sacred thing is, then swung down and out as the mote's lobbed
        Power::Sanctuary => vec![
            (0.14, at([0.0, 0.02, 0.12], [0.0, 0.1, 1.0]), false),
            (0.32, at([0.0, 0.04, 0.16], [0.0, 0.05, 1.0]), false),
            (release - 0.06, at([0.01, 0.0, 0.17], [0.02, -0.15, 0.99]), false),
            (release, at([-0.04, 0.2, -0.02], [-0.04, 0.8, 0.6]), true),
            (release + 0.14, at([-0.03, 0.18, -0.05], [-0.03, 0.75, 0.66]), false),
            (0.88, at([-0.01, 0.03, 0.01], [-0.02, 0.4, 0.92]), false),
        ],
        // A caster raised and thrust out at the crosshair; a hammer raised high and brought down
        // as the brand leaves it
        Power::Smite if magic => vec![
            (0.14, at([0.0, -0.05, 0.12], [0.0, -0.25, 0.97]), false),
            (release - 0.05, at([0.0, -0.06, 0.16], [0.0, -0.35, 0.94]), false),
            (release, at([-0.06, 0.2, 0.02], [-0.06, 0.9, 0.42]), true),
            (release + 0.14, at([-0.05, 0.18, -0.01], [-0.05, 0.86, 0.5]), false),
            (0.88, at([-0.01, 0.03, 0.01], [-0.02, 0.4, 0.92]), false),
        ],
        Power::Smite => vec![
            (0.16, pose([0.22, 0.32, 0.02], [0.1, 0.05, 1.0], [0.0, 1.0, -0.05]), false),
            (release - 0.06, pose([0.22, 0.29, 0.08], [0.1, -0.08, 0.99], [0.0, 1.0, 0.08]), false),
            (release, pose([0.08, 0.64, -0.24], [0.0, 0.9, -0.42], [0.0, -0.42, -0.9]), true),
            (release + 0.1, pose([0.07, 0.63, -0.32], [0.0, 0.8, -0.6], [0.0, -0.6, -0.8]), false),
            (0.9, pose([0.27, 0.4, -0.28], [0.02, 0.4, 0.92], [1.0, 0.0, 0.0]), false),
        ],
        _ => return None,
    })
}

/// A profile for what `profile`'s weapon lets go of in `element`
fn of_school(profile: &Rc<Profile>, element: Option<Element>) -> Rc<Profile> {
    if profile.element == element { profile.clone() } else { Rc::new(Profile { element, ..(**profile).clone() }) }
}

impl Combat {
    /// One of them let go, the `shot`th of what its move lets go: true if it was one of them
    pub(in crate::combat) fn unleash_arcane(&mut self, arena: &mut Arena, weapon: &Weapon, power: Power, shot: usize, base: f64) -> bool {
        if !power.arcane() {
            return false;
        }
        let view = arena.view;
        let vpm = view.vpm;
        let up = Vector3::z();
        let profile = of_school(&weapon.profile, power.school());
        let look = Element::look(power.school());
        let light = power.school().map_or([1.0, 1.0, 1.0], Element::light);
        let forward = view.forward();
        let tip = self.held_point(weapon, view, false);
        match power {
            // One missile, off to one side or the other, to weave in onto what the first was
            // aimed at, or what's nearest that if that's gone
            Power::Missiles => {
                let alive = |id: u32| arena.animals.iter().any(|a| a.id == id && a.health > 0.0);
                if shot == 0 || !alive(self.missiled.0) {
                    let (targets, at) = Combat::seek_targets(arena, MISSILE_REACH);
                    self.missiled = (targets.first().map_or(u32::MAX, |&i| arena.animals[i].id), 0);
                    if shot == 0 {
                        self.missile_aim = at;
                    }
                }
                let side = if shot % 2 == 0 { 1.0 } else { -1.0 };
                let right = view.rotation * Vector3::x();
                let out = (forward * 0.75 + right * side * 0.55 + up * (0.25 + 0.08 * (shot % 3) as f64)).normalize();
                self.launch(&profile, None, Form::Bolt, tip, out * MISSILE_SPEED * 0.55 * vpm, base * 0.42, 0.0);
                self.signed(Sig::Power(Shape::Missile { target: self.missiled.0 }));
                if let Some(p) = self.projectiles.last_mut() {
                    p.range = MISSILE_REACH * 1.6 * vpm;
                }
                self.sparks(tip, 5, 2.0, look, 0.0, 0.25);
                self.flashes.push(Flash { position: tip, color: light, reach: 4.0, life: 0.12, lifetime: 0.12 });
                self.cues.push(Cue::here(Sound::Cast(look as u8), 0.45).pitched(1.2 + 0.07 * shot as f32));
            }
            // An hourglass of light lobbed onto the ring
            Power::Hourglass => {
                let target = self.aim(arena);
                let spot = self.power_spot(arena, weapon).unwrap_or(target);
                let time = ((spot - tip).norm() / (18.0 * vpm)).max(0.3);
                self.keep_power_ring(weapon, spot, time + 0.3);
                self.launch(&profile, None, Form::Bolt, tip, thrown(tip, spot, time, 0.5, vpm), base, 0.5);
                self.signed(Sig::Power(Shape::Glass));
                self.flashes.push(Flash { position: tip, color: light, reach: 5.0, life: 0.25, lifetime: 0.25 });
                self.cues.push(Cue::here(Sound::Summon, 0.7).pitched(1.4));
                self.cues.push(Cue::here(Sound::Whirr, 0.5).pitched(0.6));
            }
            // An orb of death hurled at the crosshair, on through everything
            Power::SoulHarvest => {
                let from = if weapon.profile.kind == Kind::Magic { tip } else { view.eye + view.rotation * Vector3::new(0.05, 0.9, -0.3) * vpm };
                let (to, _) = self.power_points(arena, power)[0];
                let way = (to - from).try_normalize(1e-6).unwrap_or(forward);
                self.launch(&profile, None, Form::Bolt, from, way * HARVEST_SPEED * vpm, base * 1.1, 0.0);
                self.signed(Sig::Power(Shape::Reaper));
                if let Some(p) = self.projectiles.last_mut() {
                    p.range = HARVEST_REACH * vpm;
                    p.size = 1.4;
                }
                self.sparks(from, 10, 2.5, look, -0.1, 0.4);
                self.flashes.push(Flash { position: from, color: Element::Void.light(), reach: 6.0, life: 0.3, lifetime: 0.3 });
                self.cues.push(Cue::here(Sound::Wail, 0.6).pitched(0.7));
                self.cues.push(Cue::here(Sound::Hex, 0.7).pitched(0.5));
            }
            // A shard of bone flung at what the crosshair first meets
            Power::BoneCage => {
                let (to, who) = self.power_points(arena, power)[0];
                let way = lead(tip, to, 30.0, 0.05, vpm);
                let range = (to - tip).norm() / vpm + if who.is_some() { 1.0 } else { 0.3 };
                self.launch(&profile, None, Form::Bolt, tip, way * 30.0 * vpm, base * 0.6, 0.05);
                self.signed(Sig::Power(Shape::Bone));
                if let Some(p) = self.projectiles.last_mut() {
                    p.range = range * vpm;
                }
                if who.is_none() {
                    self.keep_power_marks(power, &[to], range / 30.0);
                }
                self.sparks(tip, 8, 2.0, Element::look(None), 0.4, 0.3);
                self.cues.push(Cue::here(Sound::Rattle, 0.8).pitched(1.2));
                self.cues.push(Cue::here(Sound::Swing, 0.6).pitched(1.1));
            }
            // A mote of holy light lobbed onto the ring
            Power::Sanctuary => {
                let target = self.aim(arena);
                let spot = self.power_spot(arena, weapon).unwrap_or(target);
                let time = ((spot - tip).norm() / (16.0 * vpm)).max(0.3);
                self.keep_power_ring(weapon, spot, time + 0.3);
                self.launch(&profile, None, Form::Bolt, tip, thrown(tip, spot, time, 0.5, vpm), base * 0.3, 0.5);
                self.signed(Sig::Power(Shape::Hallow));
                self.flashes.push(Flash { position: tip, color: Element::Holy.light(), reach: 7.0, life: 0.3, lifetime: 0.3 });
                self.cues.push(Cue::here(Sound::Summon, 0.8).pitched(1.1));
                self.cues.push(Cue::here(Sound::Beam, 0.4).pitched(1.4));
            }
            // A brand of light thrown at what the crosshair first meets
            Power::Smite => {
                let from = if weapon.profile.kind == Kind::Magic { tip } else { view.eye + view.rotation * Vector3::new(0.1, 0.8, -0.25) * vpm };
                let (to, who) = self.power_points(arena, power)[0];
                let way = (to - from).try_normalize(1e-6).unwrap_or(forward);
                let range = (to - from).norm() / vpm + if who.is_some() { 2.0 } else { 0.3 };
                self.launch(&profile, None, Form::Bolt, from, way * SMITE_SPEED * vpm, base * 2.4, 0.0);
                self.signed(Sig::Power(Shape::Brand));
                if let Some(p) = self.projectiles.last_mut() {
                    p.range = range * vpm;
                }
                if who.is_none() {
                    self.keep_power_marks(power, &[to], range / SMITE_SPEED);
                }
                self.flashes.push(Flash { position: from, color: Element::Holy.light(), reach: 7.0, life: 0.25, lifetime: 0.25 });
                self.cues.push(Cue::here(Sound::Shing, 0.8).pitched(1.3));
                self.cues.push(Cue::here(Sound::Beam, 0.5).pitched(1.6));
                let _ = up;
            }
            _ => return false,
        }
        true
    }

    /// The weapon through one of their moves, `f` of the way through it: arcane light wheeling
    /// round it, sand falling through the air about it, death drawn in to it out of the ground,
    /// holy light rising round it
    pub(in crate::combat) fn arcane_gathers(&mut self, arena: &Arena, weapon: &Weapon, power: Power, f: f64, dt: f64) {
        let view = arena.view;
        let vpm = view.vpm;
        let up = Vector3::z();
        let Some((_, release)) = arcane_timing(power) else {
            return;
        };
        let tip = self.held_point(weapon, view, false);
        let feet = arena.player.unwrap_or(view.eye - up * 1.6 * vpm);
        match power {
            // Motes wheeling round the weapon all through its volley
            Power::Missiles if f > 0.05 && f < 0.75 => {
                if random(&mut self.rng) < dt * 40.0 {
                    let a = self.time * 9.0 + random(&mut self.rng) * 0.6;
                    let right = view.rotation * Vector3::x();
                    let at = tip + (right * a.cos() + up * a.sin()) * 0.22 * vpm;
                    self.fine(at, (right * -a.sin() + up * a.cos()) * 1.2 * vpm, Element::look(Some(Element::Arcane)), 0.25, 0.0);
                }
            }
            // Glittering sand trickling down out of the air round it
            Power::Hourglass if f > 0.05 && f < release => {
                if random(&mut self.rng) < dt * 45.0 {
                    let at = tip + (self.any_direction() * 0.3 + up * 0.25) * vpm;
                    self.fine(at, -up * 0.3 * vpm, Element::look(Some(Element::Arcane)), 0.5, 0.4);
                }
            }
            // Shadow drawn up out of the ground round the feet into it
            Power::SoulHarvest | Power::BoneCage if f > 0.05 && f < release => {
                if random(&mut self.rng) < dt * 30.0 {
                    let a = random(&mut self.rng) * TAU;
                    let at = feet + Vector3::new(a.cos(), a.sin(), 0.1) * (0.8 + random(&mut self.rng)) * vpm;
                    self.fine(at, (tip - at) * 2.2, Element::look(Some(Element::Void)), 0.45, 0.0);
                }
                if random(&mut self.rng) < dt * 5.0 {
                    self.cues.push(Cue::here(Sound::Wail, 0.15).pitched(1.3));
                }
            }
            // Holy light rising round the feet and gathering in it
            Power::Sanctuary | Power::Smite if f > 0.05 && f < release => {
                let holy = Element::look(Some(Element::Holy));
                if random(&mut self.rng) < dt * 35.0 {
                    let a = random(&mut self.rng) * TAU;
                    let at = feet + Vector3::new(a.cos(), a.sin(), 0.1) * (0.6 + 0.8 * random(&mut self.rng)) * vpm;
                    self.fine(at, up * 1.6 * vpm, holy, 0.6, -0.05);
                }
                if random(&mut self.rng) < dt * 25.0 {
                    let d = self.any_direction();
                    self.fine(tip + d * 0.3 * vpm, -d * 1.0 * vpm, holy, 0.3, 0.0);
                }
            }
            _ => {}
        }
    }

    /// One of their shots on its way: missiles weaving in onto what they're after, wisps flying
    /// home to heal, the rest trailing their school. Some(true) if it's gone, None for the rest of
    /// its flight to go on as any power's does.
    pub(in crate::combat) fn arcane_flies(&mut self, arena: &mut Arena, p: &mut Projectile, shape: Shape, dt: f64) -> Option<bool> {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        match shape {
            // Turning in onto what it's after, quicker and quicker
            Shape::Missile { target } => {
                let speed = (p.velocity.norm() + 40.0 * vpm * dt).min(MISSILE_SPEED * vpm);
                let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
                // What it's after, or with nothing there, where the crosshair was as it was cast
                let goal = arena.animals.iter().find(|a| a.id == target && a.health > 0.0).map_or(self.missile_aim, |a| a.center(vpm as u32));
                // Turning harder the nearer it gets, so it closes in rather than circling
                let off = goal - p.position;
                let to = off.try_normalize(1e-6).unwrap_or(way);
                let turn = MISSILE_TURN + 30.0 / (off.norm() / vpm).max(0.5);
                p.velocity = way.lerp(&to, (turn * dt).min(1.0)).try_normalize(1e-6).unwrap_or(way) * speed;
                if random(&mut self.rng) < dt * 40.0 {
                    let v = self.any_direction() * 0.4 * vpm;
                    self.fine(p.position, v, Element::look(Some(Element::Arcane)), 0.3, 0.0);
                }
                None
            }
            // Flying home to the chest, healing as it gets there
            Shape::Wisp { slain } => {
                let chest = arena.view.eye - up * 0.5 * vpm;
                let to = chest - p.position;
                if to.norm() < 0.8 * vpm {
                    self.heal += p.damage;
                    self.sparks(chest + arena.view.forward() * 0.6 * vpm, 4, 1.5, Element::look(Some(Element::Blood)), 0.0, 0.25);
                    self.cues.push(Cue::here(Sound::Wail, 0.2).pitched(if slain { 1.0 } else { 1.5 }));
                    return Some(true);
                }
                let speed = (p.velocity.norm() + 30.0 * vpm * dt).min(WISP_SPEED * vpm);
                let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::z);
                p.velocity = way.lerp(&to.normalize(), (9.0 * dt).min(1.0)).try_normalize(1e-6).unwrap_or(way) * speed;
                p.range = f64::INFINITY;
                if random(&mut self.rng) < dt * 30.0 {
                    let v = self.any_direction() * 0.3 * vpm;
                    self.fine(p.position, v, Element::look(Some(Element::Void)), 0.35, 0.0);
                }
                None
            }
            Shape::Reaper => {
                if random(&mut self.rng) < dt * 30.0 {
                    let at = p.position + self.any_direction() * 0.3 * vpm;
                    self.mote(at, Some(Element::Void), 1.6);
                }
                None
            }
            Shape::Glass | Shape::Hallow | Shape::Brand | Shape::Bone => {
                let element = match shape {
                    Shape::Glass => Some(Element::Arcane),
                    Shape::Hallow | Shape::Brand => Some(Element::Holy),
                    _ => Some(Element::Void),
                };
                if random(&mut self.rng) < dt * 30.0 {
                    let v = self.any_direction() * 0.4 * vpm;
                    self.fine(p.position, v, Element::look(element), 0.35, 0.0);
                }
                None
            }
            _ => None,
        }
    }

    /// One of their shots striking animal `i` at `at`. True if it's gone.
    pub(in crate::combat) fn arcane_strikes(&mut self, arena: &mut Arena, p: &mut Projectile, shape: Shape, i: usize, at: Vector3<f64>) -> bool {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let profile = p.profile.clone();
        let id = arena.animals[i].id;
        let from = p.position - p.velocity * 0.02;
        match shape {
            // Harder for each that struck the same one running
            Shape::Missile { .. } => {
                self.missiled = if self.missiled.0 == id { (id, self.missiled.1 + 1) } else { (id, 1) };
                let damage = p.damage * (1.0 + MISSILE_STREAK * (self.missiled.1 - 1) as f64);
                self.element_hit(at, Some(Element::Arcane), 1.0);
                self.sparks(at, 6, 2.5, Element::look(Some(Element::Arcane)), 0.2, 0.3);
                self.cues.push(Cue::at(Sound::Hit, at, 0.5).pitched(1.3));
                self.strike(arena, &profile, i, Blow { damage, crit: false, heavy: false, from, depth: 0, element: Some(Element::Arcane) });
                let a = &mut arena.animals[i];
                if a.health > 0.0 && a.boss.is_none() {
                    a.status.staggered = a.status.staggered.max(0.15);
                }
                true
            }
            // Through it, a wisp of soul torn out of it flying back
            Shape::Reaper => {
                if p.hit.contains(&id) {
                    return false;
                }
                p.hit.push(id);
                self.power_hit(arena, &profile, i, at, p.damage, from);
                let slain = arena.animals[i].health <= 0.0;
                let heals = p.damage * WISP_HEALS * if slain { 2.0 } else { 1.0 };
                let center = arena.animals[i].center(vpm as u32);
                // Torn out upward and back towards you
                let home = (arena.view.eye - center).try_normalize(1e-6).unwrap_or_else(Vector3::z);
                let v = (up * 2.5 + home * 3.0 + self.any_direction() * 1.0) * vpm;
                self.launch(&profile, None, Form::Bolt, center + up * 0.3 * vpm, v, heals, 0.0);
                self.signed(Sig::Power(Shape::Wisp { slain }));
                self.cues.push(Cue::at(Sound::Wail, at, 0.5).pitched(if slain { 0.7 } else { 1.1 }));
                false
            }
            // Passing through whatever it passes
            Shape::Wisp { .. } => false,
            // The cage bursting up round it
            Shape::Bone => {
                self.power_hit(arena, &profile, i, at, p.damage, from);
                let ground = Combat::floor_at(arena, arena.animals[i].position + up * 0.5 * vpm);
                self.cage(arena, &profile, ground, p.damage * CAGE_CRASH);
                true
            }
            // A pillar of light coming down on it
            Shape::Brand => {
                self.smite(arena, &profile, i, p.damage);
                true
            }
            Shape::Glass | Shape::Hallow => {
                self.arcane_lands(arena, p, shape, at);
                true
            }
            _ => true,
        }
    }

    /// A Bone Cage bursting up at `ground`: spikes of bone round it, holding what's in it till they
    /// crash in on it for `damage`
    fn cage(&mut self, arena: &mut Arena, profile: &Rc<Profile>, ground: Vector3<f64>, damage: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Cage)), profile, ground, Vector3::zeros(), CAGE_ROUND, CAGE_HOLD + 0.6, damage);
        if let Some(z) = self.zones.last_mut() {
            for i in Combat::foes_round(arena, ground + up * 0.5 * vpm, CAGE_ROUND) {
                z.hit.push(arena.animals[i].id);
            }
        }
        if (ground - arena.view.eye).norm() > 3.0 * vpm {
            self.swells.push(Swell { model: crate::effects::BONE_SPIKES, position: ground, radius: 2.6, life: CAGE_HOLD + 0.5, lifetime: CAGE_HOLD + 0.5 });
        }
        self.sparks(ground + up * 0.3 * vpm, 16, 3.5, Element::look(None), 1.0, 0.5);
        self.blasts.push(Blast { position: ground + up * 0.3 * vpm, look: Element::look(Some(Element::Void)), radius: 0.8, life: 0.3, lifetime: 0.3 });
        self.cues.push(Cue::at(Sound::Crack, ground, 0.9).pitched(0.8));
        self.cues.push(Cue::at(Sound::Rattle, ground, 0.9).pitched(0.7));
    }

    /// A pillar of light coming down on animal `i` for `damage`, staggering it and healing a
    /// little; what it slays bursts into light striking the two nearest
    fn smite(&mut self, arena: &mut Arena, profile: &Rc<Profile>, i: usize, damage: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let holy = Element::look(Some(Element::Holy));
        let center = arena.animals[i].center(vpm as u32);
        let ground = Combat::floor_at(arena, arena.animals[i].position + up * 0.5 * vpm);
        self.landed(center);
        if (ground - arena.view.eye).norm() > 3.5 * vpm {
            self.swells.push(Swell { model: PILLAR, position: ground, radius: 1.2, life: 0.6, lifetime: 0.6 });
        }
        self.strike(arena, profile, i, Blow { damage, crit: false, heavy: true, from: center + up * 3.0 * vpm, depth: 0, element: Some(Element::Holy) });
        self.heal += damage * SMITE_HEALS;
        let slain = arena.animals[i].health <= 0.0;
        {
            let a = &mut arena.animals[i];
            if !slain {
                a.status.staggered = a.status.staggered.max(1.0);
                if a.boss.is_some() {
                    a.status.limit_for_boss();
                }
            }
        }
        if slain {
            let id = arena.animals[i].id;
            let next: Vec<usize> = Combat::foes_round(arena, center, SMITE_BURST).into_iter().filter(|&j| arena.animals[j].id != id).take(2).collect();
            for (k, j) in next.into_iter().enumerate() {
                let to = arena.animals[j].center(vpm as u32);
                self.lightning(center, to, holy, 0.05, 1, 2, HOP_GAP * (k + 1) as f64, 0.0);
                self.strike(arena, profile, j, Blow { damage: damage * 0.6, crit: false, heavy: false, from: center, depth: 1, element: Some(Element::Holy) });
                if arena.animals[j].health > 0.0 && arena.animals[j].boss.is_none() {
                    arena.animals[j].status.staggered = arena.animals[j].status.staggered.max(0.5);
                }
            }
            self.blasts.push(Blast { position: center, look: holy, radius: 1.4, life: 0.35, lifetime: 0.35 });
        }
        self.sparks(center, 14, 4.0, holy, 0.3, 0.45);
        self.flashes.push(Flash { position: center + up * vpm, color: Element::Holy.light(), reach: 10.0, life: 0.35, lifetime: 0.35 });
        self.cues.push(Cue::at(Sound::Beam, center, 0.9).pitched(1.2));
        self.cues.push(Cue::at(Sound::Thunder, center, 0.35).pitched(1.8));
    }

    /// One of their shots coming down at `at`, or as far as it goes
    pub(in crate::combat) fn arcane_lands(&mut self, arena: &mut Arena, p: &Projectile, shape: Shape, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let profile = p.profile.clone();
        match shape {
            Shape::Missile { .. } => self.power_pop(arena, &profile, at, 1.0, p.damage * 0.4, None),
            Shape::Glass => {
                let ground = Combat::floor_at(arena, at);
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Hourglass)), &profile, ground, Vector3::zeros(), GLASS_ROUND, GLASS_SECONDS, p.damage);
                self.blasts.push(Blast { position: ground + up * 0.3 * vpm, look: Element::look(Some(Element::Arcane)), radius: 0.9, life: 0.3, lifetime: 0.3 });
                self.flashes.push(Flash { position: ground + up * vpm, color: Element::Arcane.light(), reach: 9.0, life: 0.35, lifetime: 0.35 });
                self.cues.push(Cue::at(Sound::Crack, ground, 0.6).pitched(1.7));
                self.cues.push(Cue::at(Sound::Whirr, ground, 0.6).pitched(0.4));
            }
            Shape::Reaper => {
                self.power_pop(arena, &profile, at, 1.6, p.damage * 0.5, None);
                self.sparks(at, 12, 3.0, Element::look(Some(Element::Void)), -0.2, 0.45);
            }
            // A wisp that met a wall on its way home: given back where it is
            Shape::Wisp { .. } => {
                self.heal += p.damage;
                self.sparks(at, 4, 1.5, Element::look(Some(Element::Void)), 0.0, 0.3);
            }
            Shape::Bone => {
                let ground = Combat::floor_at(arena, at);
                self.cage(arena, &profile, ground, p.damage * CAGE_CRASH);
            }
            Shape::Hallow => {
                let ground = Combat::floor_at(arena, at);
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Sanctuary)), &profile, ground, Vector3::zeros(), SANCTUM_ROUND, SANCTUM_SECONDS, p.damage);
                if (ground - arena.view.eye).norm() > 3.5 * vpm {
                    self.swells.push(Swell { model: PILLAR, position: ground, radius: 1.8, life: 0.9, lifetime: 0.9 });
                }
                self.flashes.push(Flash { position: ground + up * 1.5 * vpm, color: Element::Holy.light(), reach: 12.0, life: 0.45, lifetime: 0.45 });
                self.sparks(ground + up * 0.2 * vpm, 20, 3.0, Element::look(Some(Element::Holy)), -0.3, 0.6);
                self.cues.push(Cue::at(Sound::Beam, ground, 0.9).pitched(0.9));
                self.cues.push(Cue::at(Sound::Summon, ground, 0.6).pitched(1.5));
            }
            // A pillar of light where it comes down short of anything
            Shape::Brand => {
                let ground = Combat::floor_at(arena, at);
                if (ground - arena.view.eye).norm() > 3.5 * vpm {
                    self.swells.push(Swell { model: PILLAR, position: ground, radius: 1.0, life: 0.5, lifetime: 0.5 });
                }
                self.power_pop(arena, &profile, ground + up * 0.4 * vpm, 1.5, p.damage * 0.4, None);
                self.cues.push(Cue::at(Sound::Beam, ground, 0.6).pitched(1.3));
            }
            _ => {}
        }
    }

    /// One of their zones going on
    pub(in crate::combat) fn arcane_zone(&mut self, arena: &mut Arena, z: &mut Zone, kind: Lasting, dt: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let age = z.lifetime - z.life;
        let profile = z.profile.clone();
        match kind {
            // Everything in it slowed and laid open, worn at now and then, motes hanging in the
            // air all but still
            Lasting::Hourglass => {
                let caught = Combat::foes_round(arena, z.position + up * 0.5 * vpm, z.radius);
                for &i in &caught {
                    let s = &mut arena.animals[i].status;
                    s.slowed = s.slowed.max(0.25);
                    s.slow = s.slow.max(GLASS_SLOW);
                    s.marked = s.marked.max(0.25);
                    s.mark_bonus = s.mark_bonus.max(GLASS_OPEN);
                    s.push *= 0.5;
                }
                if z.tick <= 0.0 {
                    z.tick = GLASS_TICK;
                    for &i in &caught {
                        self.strike(arena, &profile, i, Blow { damage: z.damage * 0.12, crit: false, heavy: false, from: z.position, depth: 1, element: Some(Element::Arcane) });
                    }
                }
                if random(&mut self.rng) < dt * 14.0 {
                    let a = random(&mut self.rng) * TAU;
                    let r = z.radius * random(&mut self.rng).sqrt();
                    let at = z.position + Vector3::new(a.cos() * r, a.sin() * r, 0.3 + 1.6 * random(&mut self.rng)) * vpm;
                    self.fine(at, up * 0.12 * vpm, Element::look(Some(Element::Arcane)), 1.2, 0.0);
                }
                let _ = age;
            }
            // Holding what it caught till it crashes in on it
            Lasting::Cage => {
                if z.owed <= 0.0 {
                    for k in 0..z.hit.len() {
                        let id = z.hit[k];
                        if let Some(a) = arena.animals.iter_mut().find(|a| a.id == id && a.health > 0.0) {
                            a.status.rooted = a.status.rooted.max(0.2);
                            a.status.push = Vector3::zeros();
                        }
                    }
                }
                if age >= CAGE_HOLD && z.owed <= 0.0 {
                    z.owed = 1.0;
                    for i in Combat::foes_round(arena, z.position + up * 0.5 * vpm, z.radius) {
                        let center = arena.animals[i].center(vpm as u32);
                        self.landed(center);
                        self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: true, from: z.position, depth: 0, element: Some(Element::Void) });
                        let a = &mut arena.animals[i];
                        if a.health > 0.0 {
                            a.status.bleed = a.status.bleed.max(4.0);
                            a.status.bleed_stacks = (a.status.bleed_stacks + 2).min(5);
                            a.status.bleed_dps = a.status.bleed_dps.max(z.damage * 0.08);
                            a.status.staggered = a.status.staggered.max(0.6);
                            if a.boss.is_some() {
                                a.status.limit_for_boss();
                            }
                        }
                        self.sparks(center, 10, 3.0, Element::look(Some(Element::Blood)), 0.8, 0.4);
                    }
                    if (z.position - arena.view.eye).norm() > 3.0 * vpm {
                        self.swells.push(Swell { model: crate::effects::BONE_SPIKES, position: z.position, radius: 1.4, life: 0.5, lifetime: 0.5 });
                    }
                    self.sparks(z.position + up * 0.8 * vpm, 20, 4.0, Element::look(None), 0.8, 0.5);
                    self.cues.push(Cue::at(Sound::Crack, z.position, 1.0).pitched(0.6));
                    self.cues.push(Cue::at(Sound::Squelch, z.position, 0.7).pitched(0.8));
                    let near = (1.0 - (z.position - arena.view.eye).norm() / vpm / 25.0).max(0.0);
                    self.shake = self.shake.max(0.3 * near);
                }
            }
            // Searing and staggering what's in it, pressing it out, healing whoever stands in it,
            // motes of light rising
            Lasting::Sanctuary => {
                if let Some(feet) = arena.player
                    && (feet - z.position).xy().norm() < z.radius * vpm
                    && (feet.z - z.position.z).abs() < 3.0 * vpm
                {
                    self.heal += z.damage * SANCTUM_HEALS * dt;
                }
                if z.tick <= 0.0 {
                    z.tick = SANCTUM_TICK;
                    for i in Combat::foes_round(arena, z.position + up * 0.5 * vpm, z.radius) {
                        let center = arena.animals[i].center(vpm as u32);
                        self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: false, from: z.position, depth: 1, element: Some(Element::Holy) });
                        self.sparks(center, 4, 2.0, Element::look(Some(Element::Holy)), -0.2, 0.35);
                        let a = &mut arena.animals[i];
                        if a.health > 0.0 {
                            a.status.staggered = a.status.staggered.max(if a.boss.is_some() { 0.1 } else { 0.3 });
                            if a.boss.is_none() {
                                let out = (a.position - z.position).xy().try_normalize(1e-6).map_or(Vector3::zeros(), |v| Vector3::new(v.x, v.y, 0.0));
                                a.status.push = out * 3.0 * vpm;
                            }
                        }
                    }
                }
                if random(&mut self.rng) < dt * 18.0 {
                    let a = random(&mut self.rng) * TAU;
                    let r = z.radius * random(&mut self.rng).sqrt();
                    let at = z.position + Vector3::new(a.cos() * r, a.sin() * r, 0.1) * vpm;
                    self.fine(at, up * 1.4 * vpm, Element::look(Some(Element::Holy)), 0.9, -0.05);
                }
            }
            _ => {}
        }
    }

    /// One of their zones whose time is out: time snapping back in the hourglass, everything in it
    /// struck all at once
    pub(in crate::combat) fn arcane_zone_ends(&mut self, arena: &mut Arena, z: &Zone, kind: Lasting) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let profile = z.profile.clone();
        if kind == Lasting::Hourglass {
            for i in Combat::foes_round(arena, z.position + up * 0.5 * vpm, z.radius) {
                let center = arena.animals[i].center(vpm as u32);
                self.landed(center);
                self.strike(arena, &profile, i, Blow { damage: z.damage, crit: false, heavy: true, from: z.position, depth: 0, element: Some(Element::Arcane) });
                let a = &mut arena.animals[i];
                if a.health > 0.0 {
                    a.status.staggered = a.status.staggered.max(if a.boss.is_some() { 0.2 } else { 0.5 });
                }
            }
            self.blasts.push(Blast { position: z.position + up * 0.8 * vpm, look: Element::look(Some(Element::Arcane)), radius: 2.0, life: 0.35, lifetime: 0.35 });
            self.flashes.push(Flash { position: z.position + up * vpm, color: Element::Arcane.light(), reach: 12.0, life: 0.3, lifetime: 0.3 });
            self.cues.push(Cue::at(Sound::Crack, z.position, 0.9).pitched(1.4));
            self.cues.push(Cue::at(Sound::Boom, z.position, 0.5).pitched(1.6));
        }
    }

    /// How their shots are drawn: missiles, an hourglass of light, the orb of death and its wisps,
    /// the shard of bone, the mote of holy light and the brand
    pub(in crate::combat) fn arcane_shot_instances(&self, p: &Projectile, shape: Shape, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let far = ((p.position - eye).norm() / vpm / 15.0).max(1.0).sqrt();
        let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 9.0);
        let ball = |at: Vector3<f64>, look: usize, size: f64| {
            let (model, ball) = self.effect_model(SLOT_FIREBALL, look);
            (eye, place(model, ball, MODEL_RES, Vector3::zeros(), at, spin, vpm, size * far, MOVING))
        };
        let arcane = Element::look(Some(Element::Arcane));
        let void = Element::look(Some(Element::Void));
        let holy = Element::look(Some(Element::Holy));
        let mut out = Vec::new();
        match shape {
            Shape::Missile { .. } => {
                out.push(ball(p.position, arcane, 0.08));
                let back = (p.flown / vpm).min(1.6) * vpm;
                out.extend(self.line_bolts(eye, p.position - way * back, p.position, arcane, 0.5, 0.9 * far, 0.0));
            }
            // A glowing hourglass, turning end over end
            Shape::Glass => {
                let tumble = self.time * 6.0;
                let axis = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), tumble) * Vector3::z();
                out.push(ball(p.position, arcane, 0.07));
                for s in [-1.0, 1.0] {
                    out.extend(self.ring_instances(eye, p.position + axis * s * 0.2 * vpm, axis, 0.14 * far, arcane, 0.4 * far, self.time * 4.0));
                }
            }
            // A churning orb of death, a dark ring about it, a trail of it behind
            Shape::Reaper => {
                // Smaller right by the eye as it leaves the hand, so it never fills the view
                let near = ((p.position - eye).norm() / vpm / 4.0).clamp(0.3, 1.0);
                out.push(ball(p.position, void, 0.24 * near));
                out.push(ball(p.position, Element::look(Some(Element::Blood)), 0.1 * near));
                out.extend(self.ring_instances(eye, p.position, eye - p.position, 0.45 * far * near, void, 0.7 * far * near, self.time * 7.0));
                let back = (p.flown / vpm).min(2.5) * vpm;
                out.extend(self.line_bolts(eye, p.position - way * back, p.position, void, 0.6, 1.3 * far, 0.08));
            }
            Shape::Wisp { slain } => {
                out.push(ball(p.position, if slain { Element::look(Some(Element::Blood)) } else { void }, if slain { 0.09 } else { 0.07 }));
                out.extend(self.streak_at(eye, p.position, way, void, 0.9 * far));
            }
            // A shard of bone spinning end over end
            Shape::Bone => {
                let tumble = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.time * 14.0) * UnitQuaternion::rotation_between(&Vector3::x(), &way).unwrap_or_else(UnitQuaternion::identity);
                let (model, thorn) = self.model_at(THORN);
                out.push((eye, place(model, thorn, MODEL_RES, Vector3::new(ARROW_HEAD * 0.5, 0.0, 0.0), p.position, tumble, vpm, THORN_SIZE * 1.6 * far, MOVING)));
                out.push(ball(p.position, void, 0.06));
                out.extend(self.streak_at(eye, p.position - way * 0.3 * vpm, way, void, 1.0 * far));
            }
            Shape::Hallow => {
                out.push(ball(p.position, holy, 0.13));
                out.extend(self.ring_instances(eye, p.position, eye - p.position, 0.3 * far, holy, 0.6 * far, self.time * 6.0));
            }
            // A brand of light, a cross of it at its head, streaming behind
            Shape::Brand => {
                out.push(ball(p.position, holy, 0.12));
                let across = way.cross(&Vector3::z()).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                out.extend(self.streak_at(eye, p.position + across * 0.18 * vpm, across, holy, 0.8 * far));
                out.extend(self.streak_at(eye, p.position - across * 0.18 * vpm, -across, holy, 0.8 * far));
                let back = (p.flown / vpm).min(3.0) * vpm;
                out.extend(self.line_bolts(eye, p.position - way * back, p.position, holy, 0.6, 1.4 * far, 0.0));
            }
            _ => {}
        }
        out
    }

    /// How their zones are drawn: an hourglass of light standing over the ring, two circles turning
    /// slowly round it; a cage's ring; the sanctuary's circle of light with posts of it round its
    /// rim
    pub(in crate::combat) fn arcane_zone_instances(&self, z: &Zone, kind: Lasting, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        let up = Vector3::z();
        let age = z.lifetime - z.life;
        let fade = (smooth(age / 0.3) * smooth(z.life / 0.5)).max(0.05);
        let mut out = Vec::new();
        match kind {
            Lasting::Hourglass => {
                let arcane = Element::look(Some(Element::Arcane));
                out.extend(self.circle_instances(eye, z.position + up * 0.06 * vpm, z.radius, arcane, 2.0 * fade, age * 0.25));
                out.extend(self.circle_instances(eye, z.position + up * 0.08 * vpm, z.radius * 0.5, arcane, 1.5 * fade, -age * 0.4));
                // The glass itself, its two bulbs' rims and the four lines joining them through its
                // waist, sand trickling down the middle
                let (top, bottom, waist) = (z.position + up * 3.2 * vpm, z.position + up * 1.4 * vpm, z.position + up * 2.3 * vpm);
                let r = 1.0 * fade;
                out.extend(self.circle_instances(eye, top, r, arcane, 1.3 * fade, age * 0.3));
                out.extend(self.circle_instances(eye, bottom, r, arcane, 1.3 * fade, -age * 0.3));
                for k in 0..4 {
                    let a = k as f64 / 4.0 * TAU + age * 0.3;
                    let rim = Vector3::new(a.cos(), a.sin(), 0.0) * r * vpm;
                    out.extend(self.line_bolts(eye, top + rim, waist, arcane, 0.5, 1.1 * fade, 0.0));
                    out.extend(self.line_bolts(eye, waist, bottom - rim, arcane, 0.5, 1.1 * fade, 0.0));
                }
                let fall = (age * 1.5).fract();
                out.extend(self.streak_at(eye, waist - up * fall * 0.8 * vpm, -up, arcane, 0.6 * fade));
            }
            Lasting::Cage => {
                let void = Element::look(Some(Element::Void));
                out.extend(self.circle_instances(eye, z.position + up * 0.06 * vpm, z.radius, void, 1.0 * fade, age * 1.5));
            }
            Lasting::Sanctuary => {
                let holy = Element::look(Some(Element::Holy));
                out.extend(self.circle_instances(eye, z.position + up * 0.06 * vpm, z.radius, holy, 2.2 * fade, age * 0.35));
                out.extend(self.circle_instances(eye, z.position + up * 0.08 * vpm, z.radius * 0.55, holy, 1.6 * fade, -age * 0.5));
                // Posts of light standing round its rim, rising and falling
                for k in 0..8 {
                    let a = k as f64 / 8.0 * TAU + age * 0.35;
                    let at = z.position + Vector3::new(a.cos(), a.sin(), 0.0) * z.radius * vpm;
                    let rise = 1.8 + 0.3 * (self.time * 3.0 + k as f64).sin();
                    out.extend(self.line_bolts(eye, at + up * 0.1 * vpm, at + up * rise * vpm, holy, 0.45, 1.5 * fade, 0.0));
                }
            }
            _ => {}
        }
        out
    }

    /// What their zones light round them
    pub(in crate::combat) fn arcane_light(&self, z: &Zone, kind: Lasting) -> Option<(Vector3<f64>, f64, [f32; 3], f32)> {
        let fade = (z.life / 0.5).clamp(0.0, 1.0) as f32;
        let up = Vector3::new(0.0, 0.0, self.vpm);
        match kind {
            Lasting::Hourglass => Some((z.position + up * 2.0, 9.0, Element::Arcane.light(), 0.6 * fade)),
            Lasting::Cage => Some((z.position + up, 5.0, Element::Void.light(), 0.5 * fade)),
            Lasting::Sanctuary => Some((z.position + up, 10.0, Element::Holy.light(), 0.8 * fade)),
            _ => None,
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

    const VPM: f64 = 8.0;

    fn feet() -> Vector3<f64> {
        Vector3::new(24.0, 10.0, 4.0) * VPM
    }

    fn view(pitch: f64) -> View {
        View { eye: feet() + Vector3::new(0.0, 0.0, 1.6 * VPM), rotation: UnitQuaternion::from_axis_angle(&Vector3::x_axis(), pitch), tan: [1.2445, 0.7002], vpm: VPM, speed: 0.0, stride: 0.0 }
    }

    /// What using `part`'s power as a player would does to a pack of four `ahead` meters off, the
    /// crosshair on it, watched `seconds`: how many of the pack it hurt, the health it gave back,
    /// the most any one of the pack was slowed to, and the combat after
    fn used(part: &str, ahead: f64, pitch: f64, seconds: f64) -> (usize, f64, f64, Combat) {
        let library = Library::load();
        let world = field();
        let weapon = library.with_part(part, None).unwrap_or_else(|| panic!("no weapon with {part}"));
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let view = view(pitch);
        let mut animals = crate::animals::spawn_pack(&world, &dry, &[Look::Bandit; 4], feet() + Vector3::new(0.0, ahead * VPM, 0.0), crate::animals::PACK_SPREAD, 5);
        let mut combat = Combat::new(1, 0);
        combat.mana = 1000.0;
        let dt = 1.0 / 120.0;
        let mut healed = 0.0;
        let mut slowest: f64 = 1.0;
        for frame in 0..(seconds / dt) as usize {
            let t = frame as f64 * dt - 0.4;
            let controls = combat.power_controls(&weapon, t, dt);
            let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet()), water: &dry };
            combat.update(&mut arena, Some((0, &weapon)), None, controls, dt);
            healed += combat.heal;
            combat.heal = 0.0;
            slowest = animals.iter().filter(|a| a.health > 0.0).map(|a| a.status.pace()).fold(slowest, f64::min);
        }
        (animals.iter().filter(|a| a.health < a.max_health).count(), healed, slowest, combat)
    }

    /// Each strikes as it should at a pack: missiles, the hourglass slowing what's in it, the orb
    /// of death through it, giving life back, the cage, the sanctuary searing it, and the smite
    #[test]
    fn the_arcana_strike_the_pack() {
        for (part, least, pitch) in [
            ("missile_tome_focus", 1, -0.07),
            ("hourglass_crystal", 3, -0.15),
            ("soulharvest_focus", 2, -0.07),
            ("bonecage_crystal", 1, -0.07),
            ("sanctuary_crystal", 2, -0.15),
            ("smite_core", 1, -0.07),
        ] {
            let (hurt, healed, slowest, _) = used(part, 8.5, pitch, 4.5);
            assert!(hurt >= least, "{part}'s power hurt only {hurt} of the pack");
            if part.starts_with("soulharvest") {
                assert!(healed > 0.0, "the soul harvest's wisps healed nothing");
            }
            if part.starts_with("hourglass") {
                assert!(slowest < 0.5, "the hourglass slowed nothing below {slowest}");
            }
        }
    }

    /// Arcane Missiles all go for the one aimed at, each that strikes it harder than the last
    #[test]
    fn missiles_strike_one_harder_and_harder() {
        let (_, _, _, combat) = used("missile_tome_focus", 8.5, -0.07, 3.0);
        assert!(combat.missiled.1 >= 4, "only {} missiles struck the one aimed at running", combat.missiled.1);
    }

    /// Standing in a sanctuary heals; one thrown away from the player doesn't
    #[test]
    fn a_sanctuary_heals_who_stands_in_it() {
        let library = Library::load();
        let world = field();
        let weapon = library.with_part("sanctuary_crystal", None).unwrap();
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let view = view(-0.2);
        let healed = |at: Vector3<f64>| {
            let mut combat = Combat::new(1, 0);
            combat.zone(ZoneKind::Signature(SigZone::Power(Lasting::Sanctuary)), &weapon.profile, at, Vector3::zeros(), SANCTUM_ROUND, SANCTUM_SECONDS, 10.0);
            let mut animals = Vec::new();
            let mut total = 0.0;
            for _ in 0..120 {
                let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet()), water: &dry };
                combat.zones(&mut arena, 1.0 / 60.0);
                total += combat.heal;
                combat.heal = 0.0;
            }
            total
        };
        let inside = healed(feet() + Vector3::new(1.0, 1.0, 0.0) * VPM);
        let outside = healed(feet() + Vector3::new(0.0, 12.0, 0.0) * VPM);
        assert!(inside > 15.0 && outside == 0.0, "healed {inside:.1} standing in it, {outside:.1} out of it");
    }
}
