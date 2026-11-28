//! Powers of fire, frost, earth and the wild's thorns, see weapons::Power, none of them lightning:
//! the Cinder Meteor, a burning rock called down out of the sky onto the ring, leaving the ground
//! there burning; the Cinder Whirl, a weapon swung round twice through fire, flinging cinders out
//! across the way ahead; the Kindling Arrow, leaving a line of fire burning along the ground it
//! flies over; the Ice Lance, formed in front of the hand and thrust out through three, shattering
//! at the end; Rime Snares, thrown in a fan to freeze what they strike solid, or to lie in wait;
//! the Bramble Wall, bursting up across the way to hold and tear at what comes into it; and the
//! Boulder Heave, a boulder torn up out of the ground and hurled, bouncing and rolling on over what's
//! in its way.
//!
//! Each of its own school whatever's in the weapon, let go with a move of its own, leaving the
//! weapon as something flying that comes down where it's seen to, bright enough by day. What they
//! shoot and leave going is powers.rs's machinery, shapes and lastings of their own.

use super::powers::{Lasting, Shape};
use super::*;
use crate::weapons::Power;

/// The Cinder Meteor: how far round it strikes, and hardest, in meters, how far up and on beyond
/// the ring it comes down from, how fast it falls, in meters a second, and the fires it leaves:
/// how many, how far round, and for how long
pub(in crate::combat) const METEOR_ROUND: f64 = 4.0;
const METEOR_CORE: f64 = 2.0;
const METEOR_HIGH: f64 = 16.0;
const METEOR_BEYOND: f64 = 12.0;
const METEOR_SPEED: f64 = 30.0;
const METEOR_FIRES: usize = 3;
const METEOR_BURNS: f64 = 5.0;
/// The Cinder Whirl: how far its cinders fly, in meters, the fan each sweep flings them in, in
/// radians either side, how fast they fly, and how far round the sweep sears as it passes
pub(in crate::combat) const WHIRL_REACH: f64 = 10.0;
pub(in crate::combat) const CINDER_FAN: [f64; 5] = [-0.9, -0.45, 0.0, 0.45, 0.9];
const CINDER_SPEED: f64 = 22.0;
const WHIRL_SEAR: f64 = 3.0;
/// When each of the Cinder Whirl's two sweeps lets its cinders go, as parts of its move
const WHIRL_RELEASES: [f64; 2] = [0.36, 0.66];
/// The Kindling Arrow: how far it reaches, as far as an arrow flies, how fast it flies, how far apart, in meters, it sets
/// the ground alight, how many fires at most, how far round each, and how long they burn
pub(in crate::combat) const KINDLE_REACH: f64 = AIM_RANGE;
const KINDLE_SPEED: f64 = 45.0;
const KINDLE_GAP: f64 = 1.8;
const KINDLE_MOST: f64 = 10.0;
const KINDLE_ROUND: f64 = 0.9;
const KINDLE_BURNS: f64 = 5.0;
/// The Ice Lance: how far it reaches and how fast, how many it goes through, and the shards it
/// shatters into at the end
pub(in crate::combat) const ICE_REACH: f64 = 35.0;
const ICE_SPEED: f64 = 42.0;
const ICE_THROUGH: usize = 3;
const ICE_SHARDS: usize = 6;
/// Rime Snares: how far they're thrown, the fan they're thrown in, in radians either side, how
/// near something has to come to set one off, how long one lies in wait, and how long what it
/// freezes stays frozen
pub(in crate::combat) const SNARE_REACH: f64 = 18.0;
pub(in crate::combat) const SNARE_FAN: [f64; 3] = [-0.22, 0.0, 0.22];
const SNARE_TRIGGER: f64 = 1.6;
const SNARE_SECONDS: f64 = 12.0;
const SNARE_FROZEN: f64 = 2.0;
/// The Bramble Wall: how far off its seed can be thrown, how far it reaches either side of where
/// it bursts up and how near it to be caught in it, in meters, how long it stands, how often it
/// tears at what's in it, and how long it holds what comes into it
pub(in crate::combat) const BRAMBLE_REACH: f64 = 12.0;
const BRAMBLE_HALF: f64 = 4.5;
const BRAMBLE_DEPTH: f64 = 1.4;
const BRAMBLE_SECONDS: f64 = 6.0;
const BRAMBLE_TICK: f64 = 0.6;
const BRAMBLE_HOLD: f64 = 1.5;
/// The Boulder Heave: how far it's hurled, in meters, how fast it rolls on after it's come down,
/// how far, and how big round it is
pub(in crate::combat) const BOULDER_REACH: f64 = 24.0;
const ROLL_SPEED: f64 = 9.0;
const ROLL_REACH: f64 = 6.0;
const BOULDER_ROUND: f64 = 0.75;

/// How long their own moves take, and when what they let go leaves the weapon
pub(in crate::combat) fn wild_timing(power: Power) -> Option<(f64, f64)> {
    Some(match power {
        Power::Meteor => (1.05, 0.55),
        Power::CinderWhirl => (1.1, WHIRL_RELEASES[0]),
        Power::Kindling => (0.85, 0.5),
        Power::IceLance => (0.9, 0.5),
        Power::RimeSnare => (0.75, 0.4),
        Power::Bramble => (0.95, 0.5),
        Power::Boulder => (1.1, 0.56),
        _ => return None,
    })
}

/// Every moment, as a part of its move, the Cinder Whirl flings its cinders: once with each sweep
pub(in crate::combat) fn wild_releases(power: Power) -> Option<Vec<f64>> {
    (power == Power::CinderWhirl).then(|| WHIRL_RELEASES.to_vec())
}

/// Their own moves, from where the weapon's held at rest
pub(in crate::combat) fn wild_keys(power: Power, profile: &Profile) -> Option<Vec<(f64, Pose, bool)>> {
    let (_, release) = wild_timing(power)?;
    let at = spell_at(profile.style);
    let magic = profile.kind == Kind::Magic && profile.style != Style::Glaive;
    Some(match power {
        // Raised high and held trembling as the sky's called on, thrust up at it, then brought
        // down to point at the ring
        Power::Meteor => vec![
            (0.12, at([0.0, -0.05, 0.12], [0.0, -0.3, 0.95]), false),
            (0.3, at([0.01, -0.07, 0.18], [0.02, -0.45, 0.89]), false),
            (release - 0.06, at([0.0, -0.06, 0.19], [0.0, -0.42, 0.9]), false),
            (release, at([-0.03, 0.06, 0.2], [-0.03, 0.25, 0.97]), true),
            (0.72, at([-0.05, 0.16, 0.0], [-0.05, 0.86, 0.5]), false),
            (0.9, at([-0.01, 0.03, 0.01], [-0.02, 0.4, 0.92]), false),
        ],
        // Wound back out to the right, swept right across to the left, swung back up over and swept
        // back across to the right, each sweep flinging its cinders
        Power::CinderWhirl => {
            let right = pose([0.42, 0.3, -0.1], [0.86, 0.05, 0.5], [-0.5, 0.0, 0.86]);
            let left = pose([-0.36, 0.44, -0.2], [-0.86, 0.3, 0.4], [-0.4, 0.0, -0.9]);
            let middle_out = pose([0.02, 0.64, -0.16], [-0.1, 0.62, 0.78], [-1.0, 0.0, 0.1]);
            let middle_back = pose([0.0, 0.62, -0.12], [0.1, 0.64, 0.76], [1.0, 0.0, -0.1]);
            let over = pose([-0.2, 0.36, 0.06], [-0.4, 0.1, 0.9], [0.9, 0.0, 0.4]);
            vec![
                (0.12, right.clone(), false),
                (WHIRL_RELEASES[0] - 0.06, right.clone(), false),
                (WHIRL_RELEASES[0], middle_out, true),
                (WHIRL_RELEASES[0] + 0.1, left.clone(), false),
                (0.52, over, false),
                (WHIRL_RELEASES[1] - 0.04, left, false),
                (WHIRL_RELEASES[1], middle_back, true),
                (WHIRL_RELEASES[1] + 0.1, right.clone(), false),
                (0.92, pose([0.26, 0.42, -0.26], [0.2, 0.45, 0.86], [-0.9, 0.0, 0.3]), false),
            ]
        }
        // A bow raised, drawn a long moment as the arrow catches alight at the nock, and loosed
        // low; a crossbow shouldered and held steady as its bolt catches, kicking as it goes
        Power::Kindling if profile.build == Build::None => vec![
            (0.16, bow_aimed(0.1, 0.0), false),
            (0.32, bow_held(0.04, 0.0), false),
            (release - 0.05, bow_held(0.03, 0.0), false),
            (release, bow_loosed(0.03, 0.0), true),
            (0.82, bow_aimed(0.02, 0.0), false),
        ],
        Power::Kindling => {
            let aimed = levelled(0.02, 0.0);
            vec![(0.14, levelled(0.1, 0.08), false), (0.26, aimed.clone(), false), (release - 0.04, steadied(&aimed, 1.2), false), (release, gun_kicked(&aimed, 1.2), true), (release + 0.16, rocked(&aimed, 1.2), false), (0.86, levelled(0.0, 0.0), false)]
        }
        // Drawn back in to the chest as the lance forms ahead of it, held, then thrust out hard at
        // the crosshair
        Power::IceLance => vec![
            (0.12, at([0.03, -0.08, 0.05], [0.05, 0.2, 0.98]), false),
            (0.3, at([0.04, -0.12, 0.06], [0.04, 0.35, 0.94]), false),
            (release - 0.05, at([0.04, -0.13, 0.06], [0.03, 0.38, 0.92]), false),
            (release, at([-0.06, 0.24, -0.02], [-0.06, 0.97, 0.22]), true),
            (release + 0.14, at([-0.05, 0.2, -0.01], [-0.05, 0.92, 0.38]), false),
            (0.88, at([-0.01, 0.03, 0.01], [-0.02, 0.4, 0.92]), false),
        ],
        // Drawn back across the body, low, and flung out sidearm to the right, the hand coming on
        // round after
        Power::RimeSnare => vec![
            (0.14, pose([-0.12, 0.3, -0.22], [-0.85, 0.35, 0.38], [0.0, 0.0, 1.0]), false),
            (release - 0.06, pose([-0.16, 0.26, -0.24], [-0.9, 0.2, 0.38], [0.0, 0.0, 1.0]), false),
            (release, pose([0.2, 0.72, -0.2], [0.55, 0.8, 0.2], [0.0, 0.0, 1.0]), true),
            (release + 0.12, pose([0.38, 0.5, -0.24], [0.9, 0.3, 0.25], [0.0, 0.0, 1.0]), false),
            (0.88, pose([0.26, 0.42, -0.27], [0.1, 0.3, 0.95], [1.0, 0.0, 0.0]), false),
        ],
        // A caster swept low across, then driven down at the ground ahead as the seed goes; a
        // spear or battle staff raised and driven down point first into the ground ahead
        Power::Bramble if magic => vec![
            (0.15, at([0.04, -0.02, 0.06], [0.3, -0.1, 0.95]), false),
            (0.32, at([0.02, -0.05, 0.14], [0.05, -0.3, 0.95]), false),
            (release - 0.05, at([0.01, -0.05, 0.16], [0.02, -0.28, 0.96]), false),
            (release, at([-0.04, 0.22, -0.08], [-0.05, 0.75, -0.66]), true),
            (release + 0.12, at([-0.04, 0.22, -0.11], [-0.05, 0.68, -0.73]), false),
            (0.86, at([-0.01, 0.03, 0.0], [-0.02, 0.4, 0.92]), false),
        ],
        Power::Bramble => vec![
            (0.18, pose([0.22, 0.24, 0.06], [0.05, 0.4, 0.92], [1.0, 0.0, 0.0]), false),
            (release - 0.06, pose([0.24, 0.2, 0.12], [0.05, 0.3, 0.95], [1.0, 0.0, 0.0]), false),
            (release, pose([0.1, 0.66, -0.3], [0.0, 0.62, -0.78], [1.0, 0.0, 0.0]), true),
            (release + 0.14, pose([0.1, 0.68, -0.36], [0.0, 0.55, -0.83], [1.0, 0.0, 0.0]), false),
            (0.88, pose([0.27, 0.4, -0.26], [0.05, 0.45, 0.89], [1.0, 0.0, 0.0]), false),
        ],
        // Raised up before the face, held as the ground ahead cracks, then slammed down onto it as
        // the boulder heaves up out of it, and lifted on after it as it's flung
        Power::Boulder => {
            let up = [0.0, 0.92, 0.4];
            let slammed = pose([0.05, 0.62, -0.42], [0.0, 0.62, -0.78], [0.0, -0.78, -0.62]);
            vec![
                (0.1, pose([0.3, 0.42, -0.36], [0.05, 0.85, -0.5], [0.0, 0.5, 0.85]), false),
                (0.24, pose([0.18, 0.48, -0.16], [0.02, 0.66, 0.75], up), false),
                (0.4, pose([0.185, 0.49, -0.135], [0.04, 0.68, 0.73], up), false),
                (release - 0.07, pose([0.16, 0.28, 0.1], [0.05, -0.4, 0.92], up), false),
                (release, slammed.clone(), true),
                (release + 0.1, slammed, false),
                (0.8, pose([0.14, 0.56, -0.2], [0.02, 0.85, 0.52], up), false),
            ]
        }
        _ => return None,
    })
}

/// A profile for what `profile`'s weapon lets go of in `element`, or in none
fn of_school(profile: &Rc<Profile>, element: Option<Element>) -> Rc<Profile> {
    if profile.element == element { profile.clone() } else { Rc::new(Profile { element, ..(**profile).clone() }) }
}

impl Combat {
    /// One of them let go, the `shot`th of what its move lets go: true if it was one of them
    pub(in crate::combat) fn unleash_wild(&mut self, arena: &mut Arena, weapon: &Weapon, power: Power, shot: usize, base: f64) -> bool {
        if !power.wild() {
            return false;
        }
        let view = arena.view;
        let vpm = view.vpm;
        let up = Vector3::z();
        let profile = of_school(&weapon.profile, power.school());
        let look = Element::look(power.school());
        let feet = arena.player.unwrap_or(view.eye - up * 1.6 * vpm);
        let forward = view.forward();
        let flat = flat_of(forward);
        let across = flat.cross(&up);
        let tip = self.held_point(weapon, view, false);
        let fire = Element::look(Some(Element::Fire));
        match power {
            // A burning rock called down out of the sky well on beyond the ring and to one side, so
            // it's seen coming down across it onto the ring
            Power::Meteor => {
                let target = self.aim(arena);
                let spot = self.power_spot(arena, weapon).unwrap_or(target);
                let side = if random(&mut self.rng) < 0.5 { -1.0 } else { 1.0 };
                let start = spot + (flat * METEOR_BEYOND + across * side * 4.0 + up * METEOR_HIGH) * vpm;
                let fall = (spot - start).norm();
                self.keep_power_ring(weapon, spot, fall / (METEOR_SPEED * vpm) + 0.2);
                self.launch(&profile, None, Form::Bolt, start, (spot - start) / fall * METEOR_SPEED * vpm, base * 2.8, 0.0);
                self.signed(Sig::Power(Shape::Meteor));
                if let Some(p) = self.projectiles.last_mut() {
                    p.range = fall + 2.0 * vpm;
                    p.size = 1.6;
                }
                // Fire streaming up off the weapon to the sky as it's called
                for _ in 0..14 {
                    let v = (up * 6.0 + self.any_direction() * 1.5) * vpm;
                    self.fine(tip, v, fire, 0.5, -0.2);
                }
                self.flashes.push(Flash { position: tip, color: Element::Fire.light(), reach: 7.0, life: 0.3, lifetime: 0.3 });
                self.cues.push(Cue::here(Sound::Summon, 0.8).pitched(0.7));
                self.cues.push(Cue::at(Sound::Rush, start, 0.9).pitched(0.6));
            }
            // Five cinders flung out across the way ahead from the sweep, and what's near seared
            Power::CinderWhirl => {
                let from = view.eye + view.rotation * Vector3::new(0.0, 0.85, -0.3) * vpm;
                let turn = if shot % 2 == 1 { 0.22 } else { 0.0 };
                for &yaw in &CINDER_FAN {
                    let way = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), yaw + turn) * forward;
                    let (to, who) = Combat::ray_point(arena, view.eye, way, WHIRL_REACH);
                    let along = (to - from).try_normalize(1e-6).unwrap_or(way);
                    let range = (to - from).norm() / vpm + if who.is_some() { 1.0 } else { 0.3 };
                    self.launch(&profile, None, Form::Bolt, from, along * CINDER_SPEED * vpm, base * 0.75, 0.15);
                    self.signed(Sig::Power(Shape::Cinder));
                    if let Some(p) = self.projectiles.last_mut() {
                        p.range = range * vpm;
                    }
                }
                // The sweep searing what's close in front of it
                for i in Combat::foes_round(arena, feet + up * vpm, WHIRL_SEAR) {
                    let center = arena.animals[i].center(vpm as u32);
                    if (center - feet).dot(&flat) < 0.0 {
                        continue;
                    }
                    self.power_hit(arena, &profile, i, center, base * 0.5, feet);
                }
                for _ in 0..12 {
                    let v = (across * (random(&mut self.rng) - 0.5) * 6.0 + flat * 3.0 + up * random(&mut self.rng) * 2.0) * vpm;
                    self.fine(from + flat * 0.4 * vpm, v, fire, 0.4, -0.1);
                }
                self.flashes.push(Flash { position: from + flat * vpm, color: Element::Fire.light(), reach: 6.0, life: 0.25, lifetime: 0.25 });
                self.cues.push(Cue::here(Sound::Breath, 0.7).pitched(1.3 - 0.15 * shot as f32));
                self.cues.push(Cue::here(Sound::HeavySwing, 0.8).pitched(0.9));
                self.shake = self.shake.max(0.15);
            }
            // An arrow or bolt of fire at the crosshair, low and fast
            Power::Kindling => {
                let bow = weapon.profile.build == Build::None;
                let from = self.held_point(weapon, view, bow);
                let (to, _) = self.power_points(arena, power)[0];
                let way = lead(from, to, KINDLE_SPEED, 0.15, vpm);
                let missile = if bow { Missile { shot: Shot::Arrow, head: weapon.arrowhead(), power: true, size: 1.5 } } else { Missile { shot: Shot::Quarrel, head: 0, power: true, size: 1.6 } };
                self.launch(&profile, Some(missile), Form::Bolt, from, way * KINDLE_SPEED * vpm, base * 1.6, 0.15);
                self.signed(Sig::Power(Shape::Kindle));
                if let Some(p) = self.projectiles.last_mut() {
                    p.size = missile.size;
                    p.blast = 0.0;
                }
                self.flashes.push(Flash { position: from, color: Element::Fire.light(), reach: 6.0, life: 0.25, lifetime: 0.25 });
                if bow {
                    self.cues.push(Cue::here(Sound::Twang, 1.0).pitched(0.85));
                } else {
                    self.cues.push(Cue::here(Sound::Release, 1.0).pitched(0.8));
                    self.kick += 0.04;
                }
                self.cues.push(Cue::here(Sound::Sear, 0.6).pitched(1.2));
            }
            // The lance formed ahead of the hand thrust out at what the crosshair first meets
            Power::IceLance => {
                let from = tip + forward * 0.4 * vpm;
                let (to, _) = self.power_points(arena, power)[0];
                let way = (to - from).try_normalize(1e-6).unwrap_or(forward);
                self.launch(&profile, None, Form::Bolt, from, way * ICE_SPEED * vpm, base * 2.0, 0.0);
                self.signed(Sig::Power(Shape::Icicle { shard: false }));
                if let Some(p) = self.projectiles.last_mut() {
                    p.range = ICE_REACH * vpm;
                    p.size = 1.4;
                }
                self.sparks(from, 12, 3.0, look, 0.2, 0.35);
                self.flashes.push(Flash { position: from, color: Element::Frost.light(), reach: 6.0, life: 0.25, lifetime: 0.25 });
                self.cues.push(Cue::here(Sound::Crack, 0.9).pitched(1.5));
                self.cues.push(Cue::here(Sound::Swing, 0.8).pitched(0.6));
            }
            // Three shards of rime flung in a fan, each to where its way first meets something
            Power::RimeSnare => {
                let from = view.eye + view.rotation * Vector3::new(0.2, 0.8, -0.2) * vpm;
                for (to, who) in self.power_points(arena, power) {
                    let at = match who {
                        Some(i) => arena.animals[i].center(vpm as u32),
                        None => to,
                    };
                    let time = ((at - from).norm() / (24.0 * vpm)).max(0.2);
                    self.launch(&profile, None, Form::Bolt, from, thrown(from, at, time, 0.25, vpm), base * 1.3, 0.25);
                    self.signed(Sig::Power(Shape::Rime));
                    // Coming down where its ring showed, even in the air, the snare lying under it
                    if let Some(p) = self.projectiles.last_mut() {
                        p.range = (at - from).norm() * 1.04 + if who.is_some() { vpm } else { 0.0 };
                    }
                    if who.is_none() {
                        self.keep_power_marks(power, &[to], time);
                    }
                }
                self.sparks(from, 8, 2.0, look, 0.3, 0.3);
                self.cues.push(Cue::here(Sound::Swing, 0.9).pitched(1.4));
                self.cues.push(Cue::here(Sound::Crack, 0.5).pitched(1.8));
            }
            // A seed lobbed onto the ring, where the wall bursts up
            Power::Bramble => {
                let target = self.aim(arena);
                let spot = self.power_spot(arena, weapon).unwrap_or(target);
                self.keep_power_ring(weapon, spot, 1.8);
                self.launch(&profile, None, Form::Bolt, tip, arced(tip, spot, 1.5, vpm), base * 0.6, 1.5);
                self.signed(Sig::Power(Shape::Seed));
                for _ in 0..8 {
                    let at = tip + self.any_direction() * 0.2 * vpm;
                    self.mote(at, Some(Element::Poison), 1.3);
                }
                self.cues.push(Cue::here(Sound::Thorns, 0.6).pitched(1.3));
                self.cues.push(Cue::here(Sound::Swing, 0.6).pitched(0.8));
            }
            // The ground ahead burst open and a boulder heaved up out of it, hurled at what the
            // crosshair first meets
            Power::Boulder => {
                let start = ground_under(arena.world, feet + flat * 1.8 * vpm + up * vpm, vpm) + up * (BOULDER_ROUND + 0.1) * vpm;
                let target = self.aim(arena);
                let to = self.power_spot(arena, weapon).unwrap_or(target);
                self.keep_power_ring(weapon, to, 1.2);
                let time = ((to - start).norm() / (15.0 * vpm)).clamp(0.35, 1.4);
                let none = Rc::new(Profile { element: None, ..(*weapon.profile).clone() });
                self.launch(&none, None, Form::Bolt, start, thrown(start, to, time, 1.0, vpm), base * 2.2, 1.0);
                self.signed(Sig::Power(Shape::Boulder { rolling: false }));
                // Earth thrown up where it tears out of the ground, not fire
                let dust = Element::look(None);
                for _ in 0..8 {
                    let v = (self.any_direction() * 2.0 + up * 3.5) * vpm;
                    self.drop(start, v, DROP_MUD, 1.1);
                }
                if (start - view.eye).norm() > 2.5 * vpm {
                    self.swells.push(Swell { model: WAVE, position: start - up * BOULDER_ROUND * vpm, radius: 1.6 * WAVE_SWELL, life: 0.5, lifetime: 0.5 });
                }
                self.sparks(start, 18, 4.5, dust, 1.2, 0.5);
                self.cues.push(Cue::here(Sound::Slam, 0.9).pitched(0.7));
                self.cues.push(Cue::here(Sound::Quake, 0.8).pitched(1.1));
                self.shake = self.shake.max(0.45);
            }
            _ => return false,
        }
        true
    }

    /// The weapon through one of their moves, `f` of the way through it: fire streaming up off a
    /// staff raised to the sky, licking along a blade, catching at a nock; frost gathering into a
    /// lance ahead of the hand or about it; motes of the wild round a seed; the ground cracking
    /// under a slam
    pub(in crate::combat) fn wild_gathers(&mut self, arena: &Arena, weapon: &Weapon, power: Power, f: f64, dt: f64) {
        let view = arena.view;
        let vpm = view.vpm;
        let up = Vector3::z();
        let Some((length, release)) = wild_timing(power) else {
            return;
        };
        let gathering = f > 0.06 && f < release;
        let tip = self.held_point(weapon, view, power == Power::Kindling && weapon.profile.build == Build::None);
        let fire = Element::look(Some(Element::Fire));
        let frost = Element::look(Some(Element::Frost));
        match power {
            Power::Meteor if gathering => {
                let n = ((30.0 + 50.0 * f) * dt + random(&mut self.rng)) as usize;
                for _ in 0..n {
                    let v = (up * (2.5 + 3.0 * f) + self.any_direction() * 0.8) * vpm;
                    let at = tip + self.any_direction() * 0.15 * vpm;
                    self.fine(at, v, fire, 0.45, -0.3);
                }
                if random(&mut self.rng) < dt * 6.0 {
                    self.cues.push(Cue::here(Sound::Sear, 0.25).pitched(0.8 + f as f32));
                }
            }
            // Flames licking along the blade from the first sweep to the last
            Power::CinderWhirl if f > 0.06 && f < 0.85 => {
                let hand = view.eye + view.rotation * self.pose(weapon).hand * vpm;
                for _ in 0..((60.0 * dt + random(&mut self.rng)) as usize) {
                    let at = hand + (tip - hand) * (0.3 + 0.7 * random(&mut self.rng));
                    let v = (up * 1.5 + self.any_direction() * 0.6) * vpm;
                    self.fine(at, v, fire, 0.3, -0.2);
                }
            }
            Power::Kindling if gathering => {
                if random(&mut self.rng) < dt * 40.0 {
                    let v = (up * 1.2 + self.any_direction() * 0.4) * vpm;
                    let at = tip + self.any_direction() * 0.05 * vpm;
                    self.fine(at, v, fire, 0.3, -0.2);
                }
                if random(&mut self.rng) < dt * 4.0 {
                    self.cues.push(Cue::here(Sound::Sear, 0.2).pitched(1.4));
                }
            }
            // Frost drawn in to where the lance is forming ahead of the hand, out of the air round
            Power::IceLance if gathering => {
                let point = tip + view.forward() * 0.45 * vpm;
                let n = ((40.0 + 60.0 * f) * dt + random(&mut self.rng)) as usize;
                for _ in 0..n {
                    let d = self.any_direction();
                    let at = point + d * (0.5 + 0.3 * random(&mut self.rng)) * vpm;
                    self.fine(at, -d * 2.0 * vpm, frost, 0.25, 0.0);
                }
                if random(&mut self.rng) < dt * 8.0 {
                    self.cues.push(Cue::here(Sound::Crack, 0.2).pitched(1.6 + f as f32));
                }
            }
            Power::RimeSnare if gathering => {
                if random(&mut self.rng) < dt * 30.0 {
                    let v = self.any_direction() * 0.6 * vpm;
                    let at = tip + self.any_direction() * 0.1 * vpm;
                    self.fine(at, v, frost, 0.3, 0.2);
                }
            }
            Power::Bramble if gathering => {
                if random(&mut self.rng) < dt * 25.0 {
                    let at = tip + self.any_direction() * 0.25 * vpm;
                    self.mote(at, Some(Element::Poison), 1.1);
                }
            }
            // The ground ahead cracking and smoking as the weapon's raised, dust leaping off it
            Power::Boulder if gathering => {
                if random(&mut self.rng) < dt * 18.0 {
                    let feet = arena.player.unwrap_or(view.eye - up * 1.6 * vpm);
                    let at = ground_under(arena.world, feet + flat_of(view.forward()) * (1.4 + 0.8 * random(&mut self.rng)) * vpm + self.any_direction().component_mul(&Vector3::new(1.0, 1.0, 0.0)) * 0.5 * vpm + up * vpm, vpm);
                    self.sparks(at, 3, 2.5, Element::look(None), 1.0, 0.4);
                }
            }
            _ => {}
        }
        let _ = length;
    }

    /// One of their shots on its way: a meteor trailing fire and smoke, a kindling arrow setting
    /// the ground alight under it, a boulder rolling once it's come down, the rest trailing their
    /// school. Some(true) if it's gone, None for the rest of its flight to go on as any power's
    /// does.
    pub(in crate::combat) fn wild_flies(&mut self, arena: &mut Arena, p: &mut Projectile, shape: Shape, dt: f64) -> Option<bool> {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let fire = Element::look(Some(Element::Fire));
        match shape {
            Shape::Meteor => {
                for _ in 0..((90.0 * dt + random(&mut self.rng)) as usize) {
                    let v = (-way * 4.0 + self.any_direction() * 1.5) * vpm;
                    let at = p.position + self.any_direction() * 0.4 * vpm;
                    self.fine(at, v, fire, 0.5, -0.1);
                }
                if random(&mut self.rng) < dt * 20.0 {
                    let at = p.position - way * 0.8 * vpm;
                    self.mote(at, Some(Element::Fire), 2.2);
                }
                None
            }
            Shape::Cinder => {
                if random(&mut self.rng) < dt * 35.0 {
                    let v = self.any_direction() * 0.5 * vpm;
                    self.fine(p.position, v, fire, 0.3, -0.1);
                }
                None
            }
            // Fire left burning on the ground under it every KINDLE_GAP it flies, while it's low
            // enough over the ground for it to catch, and not over water
            Shape::Kindle => {
                let gap = KINDLE_GAP * vpm;
                while p.blast < KINDLE_MOST && p.flown >= (p.blast + 0.6) * gap {
                    p.blast += 1.0;
                    let back = p.flown - (p.blast - 0.4) * gap;
                    let at = p.position - way * back.max(0.0);
                    let ground = ground_under(arena.world, at + up * 0.5 * vpm, vpm);
                    let surface = (arena.water)(ground.x, ground.y);
                    if at.z - ground.z > 3.0 * vpm || (surface.is_finite() && surface > ground.z) {
                        continue;
                    }
                    let profile = p.profile.clone();
                    self.lingering.push(Lingering { tall: true, effects: true, ..Lingering::new(ground, KINDLE_ROUND, KINDLE_BURNS, Some(Element::Fire), &profile, p.damage * 0.12) });
                    // The ground smouldering under it, so the line reads by day too
                    self.scorch_with(ground, KINDLE_ROUND, Element::Fire, KINDLE_BURNS);
                    self.sparks(ground + up * 0.2 * vpm, 4, 2.0, fire, -0.3, 0.4);
                    if p.blast as usize % 3 == 1 {
                        self.cues.push(Cue::at(Sound::Sear, ground, 0.35).pitched(1.1));
                    }
                }
                None
            }
            Shape::Icicle { shard } => {
                if random(&mut self.rng) < dt * if shard { 15.0 } else { 45.0 } {
                    let v = (self.any_direction() * 0.5 - way * 1.0) * vpm;
                    self.fine(p.position, v, Element::look(Some(Element::Frost)), 0.35, 0.1);
                }
                None
            }
            Shape::Rime | Shape::Seed => {
                if random(&mut self.rng) < dt * 25.0 {
                    let element = if shape == Shape::Rime { Element::Frost } else { Element::Poison };
                    self.mote(p.position, Some(element), 1.1);
                }
                None
            }
            // Rolling on over the ground, up slopes and down off ledges, crushing what it rolls
            // into and throwing it aside
            Shape::Boulder { rolling: true } => {
                let ground = ground_under(arena.world, p.position + up * 0.6 * vpm, vpm);
                p.position.z = ground.z + BOULDER_ROUND * vpm;
                p.velocity.z = 0.0;
                if random(&mut self.rng) < dt * 20.0 {
                    self.sparks(ground, 2, 2.0, Element::look(None), 1.2, 0.4);
                }
                let profile = p.profile.clone();
                let caught: Vec<usize> = (0..arena.animals.len())
                    .filter(|&i| {
                        let a = &arena.animals[i];
                        let to = a.center(vpm as u32) - p.position;
                        a.health > 0.0 && !a.look.is_pet() && !p.hit.contains(&a.id) && to.xy().norm() < (BOULDER_ROUND + 0.3 + a.body().1) * vpm && to.z.abs() < 2.0 * vpm
                    })
                    .collect();
                for i in caught {
                    let center = arena.animals[i].center(vpm as u32);
                    p.hit.push(arena.animals[i].id);
                    self.crushed(arena, &profile, i, center, p.damage * 0.75, way);
                }
                None
            }
            Shape::Boulder { rolling: false } => {
                if random(&mut self.rng) < dt * 12.0 {
                    let v = self.any_direction() * 0.6 * vpm;
                    self.drop(p.position, v, DROP_MUD, 0.6);
                }
                None
            }
            _ => None,
        }
    }

    /// Animal `i`, struck by a boulder rolling `way` at `at`, crushed for `damage` and thrown
    /// aside off its way
    fn crushed(&mut self, arena: &mut Arena, profile: &Rc<Profile>, i: usize, at: Vector3<f64>, damage: f64, way: Vector3<f64>) {
        let vpm = arena.view.vpm;
        self.landed(at);
        self.strike(arena, profile, i, Blow { damage, crit: false, heavy: true, from: at - way * vpm, depth: 0, element: None });
        self.sparks(at, 10, 3.5, Element::look(None), 1.0, 0.4);
        self.cues.push(Cue::at(Sound::Slam, at, 0.7).pitched(1.2));
        let a = &mut arena.animals[i];
        if a.health > 0.0 {
            a.status.staggered = a.status.staggered.max(0.8);
            if a.boss.is_none() {
                let off = (a.position - (at - way * vpm)).xy();
                let side = way.cross(&Vector3::z()) * if off.dot(&way.cross(&Vector3::z()).xy()) >= 0.0 { 1.0 } else { -1.0 };
                a.status.push = (way * 3.0 + side * 5.0 + Vector3::new(0.0, 0.0, 3.0)) * vpm;
            } else {
                a.status.limit_for_boss();
            }
        }
    }

    /// One of their shots striking animal `i` at `at`. True if it's gone.
    pub(in crate::combat) fn wild_strikes(&mut self, arena: &mut Arena, p: &mut Projectile, shape: Shape, i: usize, at: Vector3<f64>) -> bool {
        let vpm = arena.view.vpm;
        let profile = p.profile.clone();
        let id = arena.animals[i].id;
        let from = p.position - p.velocity * 0.02;
        match shape {
            // Coming down onto it: the meteor's struck where it is
            Shape::Meteor => {
                self.wild_lands(arena, p, shape, at);
                true
            }
            Shape::Cinder => {
                self.power_hit(arena, &profile, i, at, p.damage, from);
                self.power_pop(arena, &profile, at, 1.2, p.damage * 0.35, Some(i));
                true
            }
            Shape::Kindle => {
                self.power_hit(arena, &profile, i, at, p.damage, from);
                self.wild_lands(arena, p, shape, at);
                true
            }
            // Through the first three, a little less each, freezing what's chilled already
            Shape::Icicle { shard: false } => {
                p.hit.push(id);
                self.power_hit(arena, &profile, i, at, p.damage, from);
                p.damage *= 0.85;
                self.sparks(at, 10, 3.0, Element::look(Some(Element::Frost)), 0.6, 0.4);
                self.cues.push(Cue::at(Sound::Crack, at, 0.6).pitched(1.3));
                if p.hit.len() >= ICE_THROUGH {
                    self.wild_lands(arena, p, shape, at);
                    return true;
                }
                false
            }
            Shape::Icicle { shard: true } => {
                self.power_hit(arena, &profile, i, at, p.damage, from);
                true
            }
            // Bursting into ice on it, freezing it solid
            Shape::Rime => {
                self.rime_bursts(arena, &profile, Combat::floor_at(arena, arena.animals[i].position + Vector3::new(0.0, 0.0, 0.5 * vpm)), p.damage);
                true
            }
            Shape::Seed => {
                self.wild_lands(arena, p, shape, at);
                true
            }
            // Struck full on as it's hurled, then rolling on from it
            Shape::Boulder { rolling: false } => {
                let way = flat_of(p.velocity);
                p.hit.push(id);
                self.crushed(arena, &profile, i, at, p.damage, way);
                self.roll_on(arena, p, at);
                true
            }
            Shape::Boulder { rolling: true } => false,
            _ => true,
        }
    }

    /// A rime shard bursting into ice at `ground`, freezing solid what's on it and chilling what's
    /// round it, for `damage`
    fn rime_bursts(&mut self, arena: &mut Arena, profile: &Rc<Profile>, ground: Vector3<f64>, damage: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let frost = Element::look(Some(Element::Frost));
        self.landed(ground);
        if (ground - arena.view.eye).norm() > 3.0 * vpm {
            self.swells.push(Swell { model: ICE_BURST, position: ground, radius: 1.5, life: 1.0, lifetime: 1.0 });
        }
        for i in Combat::foes_round(arena, ground + up * 0.5 * vpm, SNARE_TRIGGER + 0.4) {
            let center = arena.animals[i].center(vpm as u32);
            self.strike(arena, profile, i, Blow { damage, crit: false, heavy: true, from: ground, depth: 0, element: Some(Element::Frost) });
            let a = &mut arena.animals[i];
            if a.health > 0.0 {
                let s = &mut a.status;
                s.frozen = s.frozen.max(SNARE_FROZEN);
                s.rooted = s.rooted.max(SNARE_FROZEN);
                s.chill = 1.0;
                s.slowed = s.slowed.max(SNARE_FROZEN + 1.5);
                s.slow = s.slow.max(0.5);
                if a.boss.is_some() {
                    a.status.limit_for_boss();
                }
            }
            self.sparks(center, 6, 2.5, frost, 0.5, 0.4);
        }
        // Chilling what's a little further off
        for i in Combat::foes_round(arena, ground + up * 0.5 * vpm, 3.0) {
            let s = &mut arena.animals[i].status;
            s.chill = (s.chill + 0.4).min(1.0);
            s.slowed = s.slowed.max(1.5);
            s.slow = s.slow.max(0.3);
        }
        self.sparks(ground + up * 0.3 * vpm, 16, 3.5, frost, 0.8, 0.5);
        self.scorch_with(ground, 1.4, Element::Frost, 3.0);
        self.flashes.push(Flash { position: ground + up * 0.5 * vpm, color: Element::Frost.light(), reach: 5.0, life: 0.25, lifetime: 0.25 });
        self.cues.push(Cue::at(Sound::Crack, ground, 0.9).pitched(1.1));
    }

    /// A hurled boulder `p` come down at `at`, bouncing and rolling on from there along its way
    fn roll_on(&mut self, arena: &mut Arena, p: &Projectile, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let way = flat_of(p.velocity);
        let ground = ground_under(arena.world, at + up * 0.6 * vpm, vpm);
        let start = ground + up * BOULDER_ROUND * vpm - way * 0.1 * vpm;
        self.launch(&p.profile, None, Form::Bolt, start, way * ROLL_SPEED * vpm, p.damage, 0.0);
        self.signed(Sig::Power(Shape::Boulder { rolling: true }));
        if let Some(n) = self.projectiles.last_mut() {
            n.range = ROLL_REACH * vpm;
            n.hit = p.hit.clone();
        }
        for _ in 0..6 {
            let v = (self.any_direction() * 2.0 + up * 3.0) * vpm;
            self.drop(ground + up * 0.2 * vpm, v, DROP_MUD, 1.0);
        }
        self.sparks(ground, 14, 4.0, Element::look(None), 1.2, 0.45);
        self.cues.push(Cue::at(Sound::Slam, ground, 0.9).pitched(0.65));
        let near = (1.0 - (ground - arena.view.eye).norm() / vpm / 25.0).max(0.0);
        self.shake = self.shake.max(0.25 * near);
    }

    /// One of their shots coming down at `at`, or as far as it goes
    pub(in crate::combat) fn wild_lands(&mut self, arena: &mut Arena, p: &Projectile, shape: Shape, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let profile = p.profile.clone();
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let fire = Element::look(Some(Element::Fire));
        let frost = Element::look(Some(Element::Frost));
        match shape {
            Shape::Meteor => self.meteor_strikes(arena, p, at),
            Shape::Cinder => {
                self.power_pop(arena, &profile, at, 1.2, p.damage * 0.5, None);
                self.scorch(Combat::floor_at(arena, at), 0.5);
            }
            Shape::Kindle => {
                self.power_burst(arena, &profile, at, 2.5, p.damage * 0.7, None);
                let ground = Combat::floor_at(arena, at);
                if (at - ground).norm() < 1.5 * vpm {
                    self.lingering.push(Lingering { tall: true, effects: true, ..Lingering::new(ground, KINDLE_ROUND * 1.4, KINDLE_BURNS, Some(Element::Fire), &profile, p.damage * 0.12) });
                }
                self.sparks(at, 14, 4.0, fire, 0.4, 0.4);
                self.cues.push(Cue::at(Sound::Burst, at, 0.7).pitched(1.0));
            }
            // Shattering into a spray of shards on ahead, and ice bursting up where it struck
            Shape::Icicle { shard: false } => {
                let across = way.cross(&up).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                for k in 0..ICE_SHARDS {
                    let s = k as f64 / (ICE_SHARDS - 1) as f64 * 2.0 - 1.0;
                    let dir = (way + across * s * 0.6 + up * (0.15 + 0.1 * (k % 2) as f64)).normalize();
                    self.launch(&profile, None, Form::Bolt, at - way * 0.3 * vpm + dir * 0.3 * vpm, dir * 18.0 * vpm, p.damage * 0.35, 0.4);
                    self.signed(Sig::Power(Shape::Icicle { shard: true }));
                    if let Some(n) = self.projectiles.last_mut() {
                        n.range = 5.0 * vpm;
                        n.hit = p.hit.clone();
                    }
                }
                let ground = Combat::floor_at(arena, at);
                if (at - ground).norm() < 1.5 * vpm && (ground - arena.view.eye).norm() > 3.0 * vpm {
                    self.swells.push(Swell { model: ICE_BURST, position: ground, radius: 1.2, life: 0.9, lifetime: 0.9 });
                }
                self.sparks(at, 18, 4.5, frost, 0.6, 0.45);
                self.flashes.push(Flash { position: at, color: Element::Frost.light(), reach: 6.0, life: 0.25, lifetime: 0.25 });
                self.cues.push(Cue::at(Sound::Crack, at, 1.0).pitched(0.9));
            }
            Shape::Icicle { shard: true } => self.sparks(at, 5, 2.0, frost, 0.5, 0.3),
            // Lying in wait on the ground as a snare, or bursting at once on what's there already
            Shape::Rime => {
                let ground = Combat::floor_at(arena, at);
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Snare)), &profile, ground, Vector3::zeros(), SNARE_TRIGGER, SNARE_SECONDS, p.damage);
                self.sparks(ground + up * 0.1 * vpm, 8, 2.0, frost, 0.3, 0.35);
                self.cues.push(Cue::at(Sound::Crack, ground, 0.4).pitched(1.9));
            }
            // The wall bursting up across the way where it comes down
            Shape::Seed => {
                let ground = Combat::floor_at(arena, at);
                let feet = arena.player.unwrap_or(arena.view.eye - up * 1.6 * vpm);
                let across = flat_of(ground - feet).cross(&up);
                self.zone(ZoneKind::Signature(SigZone::Power(Lasting::Bramble)), &profile, ground, across * BRAMBLE_HALF * vpm, BRAMBLE_DEPTH, BRAMBLE_SECONDS, p.damage);
                self.bramble_bursts(arena, ground, across);
            }
            Shape::Boulder { rolling: false } => self.roll_on(arena, p, at),
            // Breaking apart where it stops
            Shape::Boulder { rolling: true } => {
                let ground = ground_under(arena.world, at + up * 0.6 * vpm, vpm);
                for i in Combat::foes_round(arena, ground + up * 0.5 * vpm, 2.0) {
                    if !p.hit.contains(&arena.animals[i].id) {
                        let center = arena.animals[i].center(vpm as u32);
                        self.crushed(arena, &profile, i, center, p.damage * 0.4, way);
                    }
                }
                let dust = Element::look(None);
                if (ground - arena.view.eye).norm() > 3.0 * vpm {
                    self.swells.push(Swell { model: WAVE, position: ground, radius: 2.0 * WAVE_SWELL, life: 0.55, lifetime: 0.55 });
                }
                for _ in 0..6 {
                    let v = (self.any_direction() * 2.5 + up * 3.0) * vpm;
                    self.drop(ground + up * 0.5 * vpm, v, DROP_MUD, 1.0);
                }
                self.sparks(ground + up * 0.3 * vpm, 24, 5.0, dust, 1.2, 0.6);
                self.cues.push(Cue::at(Sound::Crack, ground, 1.0).pitched(0.6));
                self.cues.push(Cue::at(Sound::Boom, ground, 0.5).pitched(1.4));
                let near = (1.0 - (ground - arena.view.eye).norm() / vpm / 25.0).max(0.0);
                self.shake = self.shake.max(0.35 * near);
            }
            _ => {}
        }
    }

    /// A Cinder Meteor coming down at `at`: everything round struck, hardest in the middle, thrown
    /// back and set burning, fire left burning round where it struck, the ground scorched, a blast
    /// and a flash lighting everything round
    fn meteor_strikes(&mut self, arena: &mut Arena, p: &Projectile, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let fire = Element::look(Some(Element::Fire));
        let profile = p.profile.clone();
        let ground = Combat::floor_at(arena, at);
        self.landed(ground);
        for i in Combat::foes_round(arena, ground + up * 0.5 * vpm, METEOR_ROUND) {
            let center = arena.animals[i].center(vpm as u32);
            let d = (center - ground).xy().norm() / vpm;
            let share = if d <= METEOR_CORE { 1.0 } else { 0.55 };
            self.power_hit(arena, &profile, i, center, p.damage * share, ground);
            let a = &mut arena.animals[i];
            if a.health > 0.0 {
                if a.boss.is_none() {
                    let away = (a.position - ground).xy().try_normalize(1e-6).map_or(Vector3::zeros(), |v| Vector3::new(v.x, v.y, 0.6));
                    a.status.push = away * 8.0 * vpm;
                    a.status.staggered = a.status.staggered.max(0.6);
                } else {
                    a.status.limit_for_boss();
                }
            }
        }
        for k in 0..METEOR_FIRES {
            let a = k as f64 / METEOR_FIRES as f64 * TAU + random(&mut self.rng);
            let spot = ground_under(arena.world, ground + Vector3::new(a.cos(), a.sin(), 0.0) * (0.6 + 1.2 * random(&mut self.rng)) * vpm + up * vpm, vpm);
            self.lingering.push(Lingering { tall: true, effects: true, ..Lingering::new(spot, 1.2, METEOR_BURNS, Some(Element::Fire), &profile, p.damage * 0.08) });
        }
        self.blasts.push(Blast { position: ground + up * 0.6 * vpm, look: fire, radius: 2.6, life: 0.5, lifetime: 0.5 });
        self.blasts.push(Blast { position: ground + up * 0.3 * vpm, look: Element::look(None), radius: 1.6, life: 0.45, lifetime: 0.45 });
        self.sparks(ground + up * 0.3 * vpm, 30, 7.0, fire, 0.8, 0.6);
        self.sparks(ground + up * 0.3 * vpm, 16, 5.0, Element::look(None), 1.2, 0.6);
        for _ in 0..6 {
            let v = (self.any_direction() * 3.0 + up * 4.0) * vpm;
            self.drop(ground + up * 0.5 * vpm, v, DROP_EMBER, 1.2);
        }
        self.scorch(ground, 2.4);
        self.flashes.push(Flash { position: ground + up * 1.5 * vpm, color: Element::Fire.light(), reach: 18.0, life: 0.45, lifetime: 0.45 });
        self.cues.push(Cue::at(Sound::Boom, ground, 1.0).pitched(0.7));
        self.cues.push(Cue::at(Sound::Quake, ground, 0.8).pitched(0.9));
        let near = (1.0 - (ground - arena.view.eye).norm() / vpm / 40.0).max(0.0);
        self.shake = self.shake.max(0.3 + 0.5 * near);
    }

    /// A Bramble Wall bursting up at `ground` along `across`: brambles heaving up along it, earth
    /// and leaves thrown up, and everything near it lashed and held
    fn bramble_bursts(&mut self, arena: &mut Arena, ground: Vector3<f64>, across: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let green = Element::look(Some(Element::Poison));
        for k in 0..7 {
            let s = k as f64 / 3.0 - 1.0;
            let at = ground_under(arena.world, ground + across * s * BRAMBLE_HALF * vpm + up * 1.5 * vpm, vpm);
            self.sparks(at + up * 0.2 * vpm, 4, 3.0, green, 0.6, 0.5);
            self.sparks(at, 3, 2.5, Element::look(None), 1.2, 0.4);
        }
        self.flashes.push(Flash { position: ground + up * vpm, color: Element::Poison.light(), reach: 8.0, life: 0.3, lifetime: 0.3 });
        self.cues.push(Cue::at(Sound::Thorns, ground, 1.0).pitched(0.8));
        self.cues.push(Cue::at(Sound::Quake, ground, 0.5).pitched(1.4));
        self.shake = self.shake.max(0.15);
    }

    /// One of their zones going on
    pub(in crate::combat) fn wild_zone(&mut self, arena: &mut Arena, z: &mut Zone, kind: Lasting, dt: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let age = z.lifetime - z.life;
        let profile = z.profile.clone();
        match kind {
            // Lying in wait, glinting, till something comes near: bursting into ice on it
            Lasting::Snare => {
                if age > 0.3 && !Combat::foes_round(arena, z.position + up * 0.5 * vpm, z.radius).is_empty() {
                    self.rime_bursts(arena, &profile, z.position, z.damage);
                    z.life = 0.0;
                    z.owed = 1.0;
                } else if random(&mut self.rng) < dt * 3.0 {
                    let v = up * 0.8 * vpm;
                    self.fine(z.position + up * 0.1 * vpm, v, Element::look(Some(Element::Frost)), 0.5, -0.05);
                }
            }
            // Holding what comes into it a moment, once each, and tearing at what's in it
            Lasting::Bramble => {
                let a = z.position - z.velocity;
                let b = z.position + z.velocity;
                let near: Vec<usize> = (0..arena.animals.len())
                    .filter(|&i| {
                        let animal = &arena.animals[i];
                        animal.health > 0.0 && !animal.look.is_pet() && off_line(animal.center(vpm as u32), a, b) < (z.radius + animal.body().1) * vpm && (animal.position.z - z.position.z).abs() < 3.0 * vpm
                    })
                    .collect();
                let lashing = age < 0.3;
                for &i in &near {
                    let id = arena.animals[i].id;
                    if z.hit.contains(&id) {
                        continue;
                    }
                    z.hit.push(id);
                    let center = arena.animals[i].center(vpm as u32);
                    self.power_hit(arena, &profile, i, center, z.damage * if lashing { 1.6 } else { 1.0 }, z.position);
                    let s = &mut arena.animals[i];
                    if s.health > 0.0 {
                        s.status.rooted = s.status.rooted.max(BRAMBLE_HOLD);
                        s.status.push = Vector3::zeros();
                        if s.boss.is_some() {
                            s.status.limit_for_boss();
                        }
                    }
                    self.cues.push(Cue::at(Sound::Thorns, center, 0.5).pitched(1.2));
                }
                if z.tick <= 0.0 {
                    z.tick = BRAMBLE_TICK;
                    for &i in &near {
                        let center = arena.animals[i].center(vpm as u32);
                        self.strike(arena, &profile, i, Blow { damage: z.damage * 0.55, crit: false, heavy: false, from: z.position, depth: 1, element: Some(Element::Poison) });
                        let s = &mut arena.animals[i].status;
                        s.poison = s.poison.max(3.0);
                        s.poison_stacks = (s.poison_stacks + 1).min(5);
                        s.poison_dps = s.poison_dps.max(z.damage * 0.1);
                        s.slowed = s.slowed.max(BRAMBLE_TICK + 0.2);
                        s.slow = s.slow.max(0.5);
                        self.sparks(center, 4, 2.0, Element::look(Some(Element::Poison)), 0.5, 0.35);
                    }
                }
                if random(&mut self.rng) < dt * 6.0 {
                    let at = a.lerp(&b, random(&mut self.rng)) + up * (0.4 + random(&mut self.rng)) * vpm;
                    self.mote(at, Some(Element::Poison), 1.2);
                }
            }
            _ => {}
        }
    }

    /// One of their zones whose time is out
    pub(in crate::combat) fn wild_zone_ends(&mut self, arena: &mut Arena, z: &Zone, kind: Lasting) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        match kind {
            // A snare never set off melting away
            Lasting::Snare if z.owed <= 0.0 => self.sparks(z.position + up * 0.1 * vpm, 6, 1.5, Element::look(Some(Element::Frost)), -0.1, 0.4),
            // The wall withering back into the ground
            Lasting::Bramble => {
                for k in 0..5 {
                    let at = z.position + z.velocity * (k as f64 / 2.0 - 1.0) + up * 0.3 * vpm;
                    self.sparks(at, 3, 1.5, Element::look(None), 0.8, 0.5);
                }
            }
            _ => {}
        }
    }

    /// How their shots are drawn: a burning rock in a ball of fire with its tail, cinders, the
    /// kindling arrow or bolt alight, a lance of ice with a cluster of spikes at its head and its
    /// shards, shards of rime, a seed glowing green, a boulder tumbling or rolling
    pub(in crate::combat) fn wild_shot_instances(&self, p: &Projectile, shape: Shape, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        let way = p.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
        let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &way).unwrap_or_else(UnitQuaternion::identity);
        let far = ((p.position - eye).norm() / vpm / 15.0).max(1.0).sqrt();
        let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 9.0);
        let ball = |at: Vector3<f64>, look: usize, size: f64| {
            let (model, ball) = self.effect_model(SLOT_FIREBALL, look);
            (eye, place(model, ball, MODEL_RES, Vector3::zeros(), at, spin, vpm, size * far, MOVING))
        };
        let fire = Element::look(Some(Element::Fire));
        let frost = Element::look(Some(Element::Frost));
        let mut out = Vec::new();
        match shape {
            // The rock tumbling in its ball of fire, a long tail of it streaming behind
            Shape::Meteor => {
                let tumble = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.time * 4.0) * UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 2.5);
                let (model, rock) = self.model_at(crate::effects::STONE_BOULDER);
                out.push((eye, place(model, rock, MODEL_RES, Vector3::zeros(), p.position, tumble, vpm, 1.5 * far.min(1.6), MOVING)));
                // Blazing round it and behind it, so it reads against a bright sky
                // The rock's half a meter round at size 1 and the ball a meter across, so this
                // wraps the back of it in flame, its face showing out of the front
                out.push(ball(p.position - way * 0.45 * vpm, fire, 1.3));
                out.push(ball(p.position - way * 1.5 * vpm, fire, 0.9));
                out.push(ball(p.position - way * 2.6 * vpm, fire, 0.55));
                let back = (p.flown / vpm).min(8.0) * vpm;
                out.extend(self.line_bolts(eye, p.position - way * back, p.position - way * 0.6 * vpm, fire, 0.8, 3.6 * far, 0.15));
                out.extend(self.streak_at(eye, p.position, way, fire, 4.0 * far));
            }
            Shape::Cinder => {
                out.push(ball(p.position, fire, 0.1));
                out.extend(self.streak_at(eye, p.position - way * 0.2 * vpm, way, fire, 1.1 * far));
            }
            // The arrow or bolt, its head burning, a streak of fire behind it
            Shape::Kindle => {
                if (p.position - eye).norm() > 1.5 * vpm {
                    let instance = if p.shot == Shot::Quarrel {
                        let (model, quarrel) = self.effect_model(SLOT_QUARREL, fire);
                        place(model, quarrel, ARROW_RES, Vector3::new(QUARREL_HEAD, 0.0, 0.0), p.position, rotation, vpm, 1.6 * far, MOVING)
                    } else {
                        let (model, arrow) = self.effect_model(SLOT_ARROW + p.head.min(ARROW_HEADS - 1), fire);
                        place(model, arrow, ARROW_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), p.position, rotation, vpm, 1.5 * far, MOVING)
                    };
                    out.push((eye, instance));
                }
                out.push(ball(p.position, fire, 0.07));
                let back = (p.flown / vpm).min(2.4) * vpm;
                out.extend(self.line_bolts(eye, p.position - way * back, p.position - way * 0.3 * vpm, fire, 0.5, 1.2 * far, 0.0));
            }
            // A shaft of ice, a cluster of spikes at its head, frost streaming off it
            Shape::Icicle { shard: false } => {
                // Not its shaft while it's still right by the eye, where it'd fill the view
                let length = 1.7 * vpm;
                if (p.position - way * length - eye).norm() > 2.5 * vpm {
                    if let Some(bar) = self.bar(p.position - way * length, p.position, 0.07 * far, frost) {
                        out.push((p.position - way * length * 0.5, bar));
                    }
                    let (model, spikes) = self.model_at(crate::effects::ICE_STAGES);
                    out.push((eye, place(model, spikes, crate::effects::ICE_RES, Vector3::zeros(), p.position, rotation, vpm, 0.32 * far, MOVING)));
                }
                out.push(ball(p.position, frost, 0.09));
                out.extend(self.streak_at(eye, p.position - way * 0.4 * vpm, way, frost, 1.8 * far));
            }
            Shape::Icicle { shard: true } => out.extend(self.streak_at(eye, p.position, way, frost, 0.9 * far)),
            Shape::Rime => {
                out.push(ball(p.position, frost, 0.07));
                if (p.position - eye).norm() > 3.0 * vpm {
                    out.extend(self.ring_instances(eye, p.position, eye - p.position, 0.14 * far, frost, 0.35 * far, self.time * 14.0));
                }
            }
            Shape::Seed => {
                let green = Element::look(Some(Element::Poison));
                out.push(ball(p.position, green, 0.12));
                out.extend(self.streak_at(eye, p.position - way * 0.2 * vpm, way, green, 1.2 * far));
            }
            // Tumbling as it flies, and rolling over and over once it's down
            Shape::Boulder { rolling } => {
                let turn = if rolling {
                    let axis = Vector3::z().cross(&way).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                    UnitQuaternion::from_axis_angle(&nalgebra::Unit::new_normalize(axis), p.flown / (BOULDER_ROUND * vpm))
                } else {
                    UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.time * 5.0) * UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 3.0)
                };
                let (model, boulder) = self.model_at(BOULDER);
                out.push((eye, place(model, boulder, MODEL_RES, Vector3::zeros(), p.position, turn, vpm, 1.6, MOVING)));
            }
            _ => {}
        }
        out
    }

    /// How their zones are drawn: a snare's glinting glyph of rime; the bramble wall, brambles
    /// and thorns all along it, sinking back into the ground at the last
    pub(in crate::combat) fn wild_zone_instances(&self, z: &Zone, kind: Lasting, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        let up = Vector3::z();
        let age = z.lifetime - z.life;
        let mut out = Vec::new();
        match kind {
            Lasting::Snare => {
                let frost = Element::look(Some(Element::Frost));
                // Shrinking away into the ground at the last, never just gone
                let left = smooth(z.life / 0.6).max(0.05);
                let pulse = 1.0 + 0.15 * (self.time * 4.0 + z.position.x).sin();
                out.extend(self.circle_instances(eye, z.position + up * 0.06 * vpm, 0.7 * left, frost, 0.7 * pulse, age * 1.2));
                let (model, ball) = self.effect_model(SLOT_FIREBALL, frost);
                out.push((eye, place(model, ball, MODEL_RES, Vector3::zeros(), z.position + up * 0.15 * vpm, UnitQuaternion::identity(), vpm, 0.07 * pulse * left, MOVING)));
            }
            Lasting::Bramble => {
                // Bursting up out of the ground at the first, sinking back into it at the last
                let grown = smooth(age / 0.25) * smooth(z.life / 0.6);
                if grown <= 0.02 {
                    return out;
                }
                let (model, roots) = self.model_at(ROOTS);
                let (thorn_model, thorn) = self.model_at(THORN);
                let across = z.velocity.try_normalize(1e-6).unwrap_or_else(Vector3::x);
                // A glow of the wild along its foot, so it reads against any ground by day
                let green = Element::look(Some(Element::Poison));
                out.extend(self.line_bolts(eye, z.position - z.velocity + up * 0.15 * vpm, z.position + z.velocity + up * 0.15 * vpm, green, 1.0, 1.3 * grown, 0.08));
                for k in 0..7 {
                    let s = k as f64 / 3.0 - 1.0;
                    let at = z.position + z.velocity * s;
                    if (at - eye).norm() < 2.0 * vpm {
                        continue;
                    }
                    let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), k as f64 * 1.9 + z.position.x * 0.01);
                    out.push((at, place(model, roots, MODEL_RES, Vector3::zeros(), at - up * (1.0 - grown) * 0.8 * vpm, turn, vpm, 1.8 * grown, STILL)));
                    // Thorns standing out of it either side
                    for side in [-1.0, 1.0] {
                        let out_way = (across.cross(&up) * side + up * 0.6).normalize();
                        let rotation = UnitQuaternion::rotation_between(&Vector3::x(), &out_way).unwrap_or_else(UnitQuaternion::identity);
                        let base = at + up * (0.5 + 0.2 * (k % 2) as f64) * grown * vpm + across * 0.3 * side * vpm;
                        out.push((base, place(thorn_model, thorn, MODEL_RES, Vector3::new(ARROW_HEAD, 0.0, 0.0), base + out_way * 0.4 * vpm, rotation, vpm, THORN_SIZE * 1.4 * grown, STILL)));
                    }
                }
            }
            _ => {}
        }
        out
    }

    /// What their zones light round them: the bramble wall a little, green; a snare not at all,
    /// as several may lie about at once
    pub(in crate::combat) fn wild_light(&self, z: &Zone, kind: Lasting) -> Option<(Vector3<f64>, f64, [f32; 3], f32)> {
        let fade = (z.life / 0.6).clamp(0.0, 1.0) as f32;
        let up = Vector3::new(0.0, 0.0, self.vpm);
        match kind {
            Lasting::Bramble => Some((z.position + up, 7.0, Element::Poison.light(), 0.45 * fade)),
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
    /// crosshair on it: how many of the pack it hurt, the pack after, and the combat after
    fn used(part: &str, ahead: f64, pitch: f64, seconds: f64) -> (usize, Vec<crate::animals::Animal>, Combat) {
        let library = Library::load();
        let world = field();
        let weapon = library.with_part(part, None).unwrap_or_else(|| panic!("no weapon with {part}"));
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let view = view(pitch);
        let mut animals = crate::animals::spawn_pack(&world, &dry, &[Look::Bandit; 4], feet() + Vector3::new(0.0, ahead * VPM, 0.0), crate::animals::PACK_SPREAD, 5);
        let mut combat = Combat::new(1, 0);
        combat.mana = 1000.0;
        let dt = 1.0 / 120.0;
        for frame in 0..(seconds / dt) as usize {
            let t = frame as f64 * dt - 0.4;
            let controls = combat.power_controls(&weapon, t, dt);
            let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet()), water: &dry };
            combat.update(&mut arena, Some((0, &weapon)), None, controls, dt);
        }
        (animals.iter().filter(|a| a.health < a.max_health).count(), animals, combat)
    }

    /// Each strikes as it should at a pack: the meteor all of it round the ring, the whirl's
    /// cinders, the kindling arrow and its fire, the lance through some of it, the snares, the
    /// wall and the boulder
    #[test]
    fn the_wilds_strike_the_pack() {
        for (part, least, pitch) in [
            ("cinderfall_crystal", 3, -0.15),
            ("cinderwhirl_core", 2, -0.1),
            ("kindling_string", 2, -0.07),
            ("kindling_sight", 2, -0.07),
            ("icelance_focus", 2, -0.07),
            ("rimesnare_core", 2, -0.12),
            ("bramble_binding", 2, -0.12),
            ("boulder_core", 2, -0.07),
        ] {
            let (hurt, _, _) = used(part, 8.5, pitch, 4.0);
            assert!(hurt >= least, "{part}'s power hurt only {hurt} of the pack");
        }
    }

    /// The meteor leaves the ground burning, the kindling arrow a line of fire behind it
    #[test]
    fn fire_is_left_burning() {
        let (_, _, combat) = used("cinderfall_crystal", 8.5, -0.15, 2.6);
        assert!(combat.lingering.iter().filter(|l| l.element == Some(Element::Fire) && l.tall).count() >= METEOR_FIRES, "the meteor left no fires");
        let (_, _, combat) = used("kindling_string", 14.0, -0.03, 1.8);
        let fires: Vec<&Lingering> = combat.lingering.iter().filter(|l| l.element == Some(Element::Fire) && l.tall).collect();
        assert!(fires.len() >= 4, "the kindling arrow left only {} fires", fires.len());
        // In a line along the way it flew, ahead
        let ys: Vec<f64> = fires.iter().map(|l| (l.position.y - feet().y) / VPM).collect();
        assert!(ys.iter().all(|&y| y > 0.5) && ys.iter().any(|&y| y > 6.0), "the fires lie at {ys:?} meters ahead");
    }

    /// A rime snare thrown onto the ground lies in wait, and freezes solid what then walks onto it
    #[test]
    fn a_snare_lies_in_wait_and_freezes() {
        let library = Library::load();
        let world = field();
        let weapon = library.with_part("rimesnare_core", None).unwrap();
        let profile = weapon.profile.clone();
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let view = view(-0.2);
        let mut combat = Combat::new(1, 0);
        let spot = feet() + Vector3::new(0.0, 6.0 * VPM, 0.0);
        combat.zone(ZoneKind::Signature(SigZone::Power(Lasting::Snare)), &profile, spot, Vector3::zeros(), SNARE_TRIGGER, SNARE_SECONDS, 10.0);
        // A wolf well away at first, then right on it
        let mut animals = crate::animals::spawn_pack(&world, &dry, &[Look::Wolf; 1], feet() + Vector3::new(8.0 * VPM, 14.0 * VPM, 0.0), 0.0, 5);
        let dt = 1.0 / 60.0;
        for _ in 0..60 {
            let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: None, water: &dry };
            combat.zones(&mut arena, dt);
        }
        assert_eq!(combat.zones.len(), 1, "the snare went off with nothing near it");
        animals[0].position = spot;
        let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: None, water: &dry };
        combat.zones(&mut arena, dt);
        assert!(combat.zones.is_empty(), "the snare never went off");
        // Frozen, as long as anything that fights is ever frozen, and held fast longer
        assert!(animals[0].status.frozen > 0.5 && animals[0].status.rooted > 1.0 && animals[0].health < animals[0].max_health, "what walked onto it wasn't frozen and hurt: frozen {}, health {} of {}", animals[0].status.frozen, animals[0].health, animals[0].max_health);
    }

    /// The lance goes through three of a line and no more, and the boulder rolls on after it's
    /// come down
    #[test]
    fn the_lance_goes_through_three_and_the_boulder_rolls() {
        let library = Library::load();
        let world = field();
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let view = view(-0.07);
        // Five bandits in a line straight ahead, 5 to 13 m off
        let line = |combat: &mut Combat, weapon: &Weapon, seconds: f64| {
            let mut animals = Vec::new();
            for k in 0..5 {
                animals.extend(crate::animals::spawn_pack(&world, &dry, &[Look::Bandit; 1], feet() + Vector3::new(0.0, (5.0 + 2.0 * k as f64) * VPM, 0.0), 0.0, 5 + k));
            }
            let dt = 1.0 / 120.0;
            for frame in 0..(seconds / dt) as usize {
                let t = frame as f64 * dt - 0.5;
                let controls = combat.power_controls(weapon, t, dt);
                for a in &mut animals {
                    a.status.rooted = 1.0;
                }
                let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet()), water: &dry };
                combat.update(&mut arena, Some((0, weapon)), None, controls, dt);
            }
            animals
        };
        let lance = library.with_part("icelance_focus", None).unwrap();
        let mut combat = Combat::new(1, 0);
        combat.mana = 1000.0;
        let animals = line(&mut combat, &lance, 1.6);
        let hurt: Vec<bool> = animals.iter().map(|a| a.health < a.max_health).collect();
        assert!(hurt[0] && hurt[1] && hurt[2], "the lance didn't go through the first three: {hurt:?}");
        assert!(!hurt[4], "the lance went on through the fifth: {hurt:?}");
        let boulder = library.with_part("boulder_core", None).unwrap();
        let mut combat = Combat::new(1, 0);
        let mut rolled = false;
        let mut animals = Vec::new();
        for k in 0..2 {
            animals.extend(crate::animals::spawn_pack(&world, &dry, &[Look::Bandit; 1], feet() + Vector3::new(0.0, (6.0 + 3.0 * k as f64) * VPM, 0.0), 0.0, 9 + k));
        }
        let dt = 1.0 / 120.0;
        for frame in 0..(3.0 / dt) as usize {
            let t = frame as f64 * dt - 0.5;
            let controls = combat.power_controls(&boulder, t, dt);
            let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet()), water: &dry };
            combat.update(&mut arena, Some((0, &boulder)), None, controls, dt);
            rolled |= combat.projectiles.iter().any(|p| p.trick == Trick::Signature(Sig::Power(Shape::Boulder { rolling: true })));
        }
        assert!(rolled, "the boulder never rolled on");
        assert!(animals.iter().all(|a| a.health < a.max_health), "the boulder didn't crush both");
    }
}
