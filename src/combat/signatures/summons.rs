//! A necromancer's summons, see weapons::Power::summons(): the Bone Phalanx, armored skeletons
//! standing their ground in a line; Bone Archers, skeletons keeping back and shooting; the Grave
//! Colossus, a grave brute, slow and huge, striking all round where its club lands; Wraith Flight,
//! wraiths flying at what's near and swooping through it again and again; and Corpse Rising, the
//! slain lying about raised as what they were.
//!
//! Each is a shade sent to the ring, seen flying there, and what it raises climbs up out of the
//! ground there, fights with blows or arrows of its own, and crumbles back into the ground when its
//! time's out. They're familiars, as Raise Dead's skeletons are, see faith.rs, drawn with the
//! creatures' own models, so they cost no memory of their own. No more than MINIONS of all of
//! them can be up at once, the oldest crumbling for the new; and right click while the power comes
//! round sends every one of them at what's aimed at.

use super::faith::{CRUMBLE_SECONDS, Minion, RISE_SECONDS, RISEN_DRAIN, Risen, hostile};
use super::powers::Shape;
use super::*;
use crate::weapons::Power;

/// How far off a shade can be sent to raise the dead, in meters
pub(in crate::combat) const CALL_REACH: f64 = 10.0;
/// How long, in seconds, the dead sent at something go after it
const ORDER_SECONDS: f64 = 8.0;
/// How far off the dead look for what to fight, in meters
const MINION_HUNT: f64 = 24.0;
/// The Bone Phalanx: how many, how far apart they stand, how far from where they stand they go
/// after what comes near, in meters, how long they last, and how hard their blows are next to
/// the spell's
const PHALANX: usize = 4;
const PHALANX_GAP: f64 = 1.3;
const PHALANX_GUARD: f64 = 7.0;
const PHALANX_SECONDS: f64 = 20.0;
const PHALANX_POWER: f64 = 0.5;
/// Bone Archers: how many, how far they shoot, in meters, how often, how fast their arrows fly,
/// how near they let things come before they step back, how long they last, and how hard
const ARCHERS: usize = 3;
const ARCHER_RANGE: f64 = 26.0;
const ARCHER_PACE: f64 = 1.3;
const ARROW_SPEED: f64 = 36.0;
const ARCHER_BACK: f64 = 1.0;
/// How long an archer takes to draw and loose
const ARCHER_DRAW: f64 = 0.5;
const ARCHER_SECONDS: f64 = 18.0;
const ARCHER_POWER: f64 = 0.45;
/// The Grave Colossus: how long it lasts, how fast it walks, how long its blow takes, how far
/// round where it lands it strikes, and how hard
const COLOSSUS_SECONDS: f64 = 22.0;
const COLOSSUS_SPEED: f64 = 3.2;
const COLOSSUS_BLOW: f64 = 1.1;
const COLOSSUS_ROUND: f64 = 2.6;
const COLOSSUS_POWER: f64 = 1.4;
/// Wraith Flight: how many, how long they last, how high they hover, how fast they fly and dive,
/// in meters a second, how long they rest between swoops, and how hard each is
const WRAITHS: usize = 3;
const WRAITH_SECONDS: f64 = 12.0;
const WRAITH_HOVER: f64 = 1.6;
const WRAITH_FLY: f64 = 7.0;
const WRAITH_DIVE: f64 = 12.0;
const WRAITH_REST: f64 = 0.9;
const WRAITH_POWER: f64 = 0.6;
/// Corpse Rising: how far round the ring the slain are raised from, in meters, how many at most,
/// how many of the long dead rise if too few lie there, how long they last, and how hard
const CORPSE_REACH: f64 = 12.0;
const CORPSES: usize = 5;
const CORPSES_LEAST: usize = 2;
const CORPSE_SECONDS: f64 = 16.0;
const CORPSE_POWER: f64 = 0.55;
/// How fast skeletons walk, how far they reach, and how long a skeleton's blow takes
const BONES_SPEED: f64 = 3.8;
const BONES_REACH: f64 = 1.3;
const BONES_BLOW: f64 = 0.6;
/// When in a blow or a draw it lands or looses
const LANDS: f64 = 0.55;

/// Which of them each kind of shade raises
fn kind_of(power: Power) -> u8 {
    match power {
        Power::Phalanx => 0,
        Power::Archers => 1,
        Power::Colossus => 2,
        Power::Wraiths => 3,
        _ => 4,
    }
}

/// How long their moves take, and when the shade leaves the weapon
pub(in crate::combat) fn summon_timing(power: Power) -> Option<(f64, f64)> {
    power.summons().then_some((1.0, 0.5))
}

/// Their moves: a caster raised high and held as the dead are called, then thrust down at the
/// ground ahead; a scythe or dagger raised in both hands and driven down into it
pub(in crate::combat) fn summon_keys(power: Power, profile: &Profile) -> Option<Vec<(f64, Pose, bool)>> {
    let (_, release) = summon_timing(power)?;
    let magic = profile.kind == Kind::Magic && profile.style != Style::Glaive;
    if magic {
        let at = spell_at(profile.style);
        return Some(vec![
            (0.15, at([0.0, -0.05, 0.12], [0.0, -0.25, 0.97]), false),
            (0.36, at([0.01, -0.07, 0.17], [0.02, -0.35, 0.94]), false),
            (release - 0.05, at([0.0, -0.06, 0.18], [0.0, -0.3, 0.95]), false),
            (release, at([-0.03, 0.22, -0.1], [-0.04, 0.7, -0.71]), true),
            (release + 0.14, at([-0.03, 0.23, -0.13], [-0.04, 0.64, -0.77]), false),
            (0.86, at([-0.01, 0.03, 0.0], [-0.02, 0.4, 0.92]), false),
        ]);
    }
    Some(vec![
        (0.2, pose([0.18, 0.3, 0.04], [0.1, 0.05, 1.0], [0.0, 1.0, -0.05]), false),
        (release - 0.06, pose([0.18, 0.27, 0.1], [0.1, -0.12, 0.99], [0.0, 0.99, 0.12]), false),
        (release, pose([0.04, 0.6, -0.38], [0.0, 0.62, -0.78], [0.0, -0.78, -0.62]), true),
        (release + 0.12, pose([0.04, 0.62, -0.44], [0.0, 0.52, -0.85], [0.0, -0.85, -0.52]), false),
        (0.9, pose([0.2, 0.48, -0.3], [0.0, 0.55, 0.83], [0.0, 1.0, -0.1]), false),
    ])
}

/// Whether one of the slain can rise as what it was: what walks on two legs or four and is no
/// bigger than a man much, the rest rising as the long dead
fn risen_as(look: Look) -> Look {
    if look.build().is_some() && look.reach() < 3.0 && !look.is_pet() { look } else { Look::Zombie }
}

impl Combat {
    /// One of them let go: a shade sent to the ring, and what's raised already sent at what's aimed
    /// at. True if it was one of them.
    pub(in crate::combat) fn unleash_summons(&mut self, arena: &mut Arena, weapon: &Weapon, power: Power, base: f64) -> bool {
        if !power.summons() {
            return false;
        }
        let view = arena.view;
        let vpm = view.vpm;
        let from = self.held_point(weapon, view, false);
        let target = self.aim(arena);
        let spot = self.power_spot(arena, weapon).unwrap_or(target);
        self.keep_power_ring(weapon, spot, 1.5 + RISE_SECONDS);
        let shadow = Rc::new(Profile { element: Some(Element::Void), ..(*weapon.profile).clone() });
        // Lobbed, so it clears a ledge it's sent down off
        self.launch(&shadow, None, Form::Bolt, from, arced(from, spot, 2.0, vpm), base, 2.0);
        self.signed(Sig::Power(Shape::Calling { kind: kind_of(power) }));
        self.command_minions(arena);
        self.cues.push(Cue::here(Sound::Wail, 0.6).pitched(0.7));
        self.cues.push(Cue::here(Sound::Hex, 0.6).pitched(0.6));
        true
    }

    /// The weapon through one of their moves, `f` of the way through it: death drawn up out of
    /// the ground into it
    pub(in crate::combat) fn summon_gathers(&mut self, arena: &Arena, weapon: &Weapon, power: Power, f: f64, dt: f64) {
        let Some((_, release)) = summon_timing(power) else {
            return;
        };
        if f < 0.05 || f > release {
            return;
        }
        let view = arena.view;
        let vpm = view.vpm;
        let tip = self.held_point(weapon, view, false);
        let feet = arena.player.unwrap_or(view.eye - Vector3::z() * 1.6 * vpm);
        if random(&mut self.rng) < dt * 35.0 {
            let a = random(&mut self.rng) * TAU;
            let at = feet + Vector3::new(a.cos(), a.sin(), 0.1) * (0.8 + random(&mut self.rng)) * vpm;
            let look = Element::look(Some(if random(&mut self.rng) < 0.3 { Element::Blood } else { Element::Void }));
            self.fine(at, (tip - at) * 2.0, look, 0.45, 0.0);
        }
    }

    /// Every one of the dead raised sent at what the crosshair's on, or what's nearest it, for a
    /// while: true if there were any up and something to send them at
    pub(in crate::combat) fn command_minions(&mut self, arena: &Arena) -> bool {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let raised: Vec<Vector3<f64>> = self.familiars.iter().filter(|f| f.risen.is_some() && f.life > CRUMBLE_SECONDS).map(|f| f.position).collect();
        if raised.is_empty() {
            return false;
        }
        let (targets, _) = Combat::seek_targets(arena, MINION_HUNT + 6.0);
        let Some(&i) = targets.first() else {
            return false;
        };
        let a = &arena.animals[i];
        self.ordered = Some((a.id, ORDER_SECONDS));
        let over = Combat::over(arena, i);
        self.spots.push((a.position + up * 0.05 * vpm, a.look.body().1 + 0.6, 0.8));
        let void = Element::look(Some(Element::Void));
        for at in raised {
            self.lightning(at + up * 1.3 * vpm, over, void, 0.02, 0, 1, 0.0, 0.0);
        }
        self.cues.push(Cue::here(Sound::Wail, 0.5).pitched(1.3));
        true
    }

    /// What the dead were sent at, if they were a little while ago and it's still up, by index
    pub(in crate::combat) fn ordered_prey(&self, arena: &Arena) -> Option<usize> {
        let (id, left) = self.ordered?;
        if left <= 0.0 {
            return None;
        }
        arena.animals.iter().position(|a| a.id == id && hostile(a))
    }

    /// Room made for `n` more of the dead: the oldest of those up crumbling away, so no more than
    /// MINIONS are ever up at once
    fn make_room(&mut self, n: usize) {
        let up = self.familiars.iter().filter(|f| f.risen.is_some() && f.life > CRUMBLE_SECONDS).count();
        let mut over = (up + n).saturating_sub(MINIONS);
        for f in self.familiars.iter_mut().filter(|f| f.risen.is_some() && f.life > CRUMBLE_SECONDS) {
            if over == 0 {
                break;
            }
            f.life = CRUMBLE_SECONDS;
            over -= 1;
        }
    }

    /// The shade come down at `at`: what it was sent to raise climbing up out of the ground there
    pub(in crate::combat) fn summons_land(&mut self, arena: &mut Arena, p: &Projectile, kind: u8, at: Vector3<f64>) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let ground = Combat::floor_at(arena, at);
        let feet = arena.player.unwrap_or(arena.view.eye - up * 1.6 * vpm);
        let toward = flat_of(ground - feet);
        let across = toward.cross(&up);
        let base = p.damage;
        let ring = |k: usize, n: usize, r: f64| {
            let a = k as f64 / n as f64 * TAU + 0.4;
            ground + Vector3::new(a.cos(), a.sin(), 0.0) * r * vpm
        };
        // Where each rises, as what, fighting how, for how long, how hard
        let mut raised: Vec<(Vector3<f64>, Look, Minion, f64, f64)> = match kind {
            0 => (0..PHALANX).map(|k| (ground + across * (k as f64 - (PHALANX - 1) as f64 * 0.5) * PHALANX_GAP * vpm, Look::ArmoredSkeleton, Minion::Phalanx, PHALANX_SECONDS, base * PHALANX_POWER)).collect(),
            1 => (0..ARCHERS).map(|k| (ring(k, ARCHERS, 1.3), Look::BoneArcher, Minion::Archer, ARCHER_SECONDS, base * ARCHER_POWER)).collect(),
            2 => vec![(ground, Look::GraveBrute, Minion::Colossus, COLOSSUS_SECONDS, base * COLOSSUS_POWER)],
            3 => (0..WRAITHS).map(|k| (ring(k, WRAITHS, 1.2), Look::RimeWraith, Minion::Wraith, WRAITH_SECONDS, base * WRAITH_POWER)).collect(),
            _ => self.graves.take_near(ground, CORPSE_REACH * vpm, CORPSES).into_iter().map(|(at, look)| (at, risen_as(look), Minion::Corpse, CORPSE_SECONDS, base * CORPSE_POWER)).collect(),
        };
        // Too few of the slain lying there: the long dead rise out of the ground instead
        while kind == 4 && raised.len() < CORPSES_LEAST {
            let k = raised.len();
            raised.push((ring(k, CORPSES_LEAST, 1.0), if k % 2 == 0 { Look::Zombie } else { Look::ArmoredZombie }, Minion::Corpse, CORPSE_SECONDS, base * CORPSE_POWER));
        }
        self.make_room(raised.len());
        let profile = p.profile.clone();
        for (k, (spot, look, minion, life, damage)) in raised.into_iter().enumerate() {
            let at = Combat::floor_at(arena, spot);
            let heading = toward.y.atan2(toward.x) + (random(&mut self.rng) - 0.5) * 0.5;
            self.familiars.push(Familiar {
                position: at,
                life: life + k as f64 * 0.2,
                wait: RISE_SECONDS + 0.15 * k as f64,
                profile: profile.clone(),
                damage,
                phantom: false,
                risen: Some(Risen { look, heading, phase: 0.0, moving: false, age: -0.15 * k as f64, swing: None, kind: minion, post: if minion == Minion::Wraith { Vector3::zeros() } else { at } }),
            });
            self.sparks(at + up * 0.2 * vpm, 10, 2.4, Element::look(Some(Element::Void)), 0.6, 0.6);
            for _ in 0..4 {
                let v = (self.any_direction() + up * 1.4) * 2.5 * vpm;
                self.drop(at + up * 0.1 * vpm, v, DROP_MUD, 1.0);
            }
        }
        let big = kind == 2;
        self.blasts.push(Blast { position: ground + up * 0.2 * vpm, look: Element::look(Some(Element::Void)), radius: if big { 2.0 } else { 1.4 }, life: 0.6, lifetime: 0.6 });
        self.flashes.push(Flash { position: ground + up * vpm, color: [0.6, 0.12, 0.5], reach: 9.0, life: 0.5, lifetime: 0.5 });
        self.cues.push(Cue::at(Sound::Wail, ground, 0.9).pitched(if big { 0.45 } else { 0.65 }));
        self.cues.push(Cue::at(Sound::Quake, ground, if big { 0.9 } else { 0.5 }).pitched(1.2));
        self.shake = self.shake.max(if big { 0.4 } else { 0.2 });
    }

    /// How a shade sent to raise the dead is drawn: a dark orb, blood in its heart, a ring of
    /// shadow turning round it
    pub(in crate::combat) fn calling_instances(&self, p: &Projectile, view: &View) -> Vec<(Vector3<f64>, AnimalInstance)> {
        let vpm = view.vpm;
        let eye = view.eye;
        // Smaller right by the eye as it leaves the hand, so it never fills the view
        let near = ((p.position - eye).norm() / vpm / 4.0).clamp(0.3, 1.0);
        let far = ((p.position - eye).norm() / vpm / 15.0).max(1.0).sqrt() * near;
        let spin = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.time * 6.0);
        let void = Element::look(Some(Element::Void));
        let mut out = Vec::new();
        for (look, size) in [(void, 0.18), (Element::look(Some(Element::Blood)), 0.08)] {
            let (model, ball) = self.effect_model(SLOT_FIREBALL, look);
            out.push((eye, place(model, ball, MODEL_RES, Vector3::zeros(), p.position, spin, vpm, size * far, MOVING)));
        }
        out.extend(self.ring_instances(eye, p.position, eye - p.position, 0.35 * far, void, 0.6 * far, self.time * 8.0));
        out
    }

    /// One of a necromancer's summons going on, after it's climbed out of the ground and before it
    /// crumbles: each fighting its own way
    pub(in crate::combat) fn walk_minion(&mut self, arena: &mut Arena, f: &mut Familiar, dt: f64) {
        let Some(r) = f.risen else { return };
        match r.kind {
            Minion::Wraith => self.fly_wraith(arena, f, r, dt),
            Minion::Archer => self.walk_archer(arena, f, r, dt),
            Minion::Colossus => self.walk_bones(arena, f, r, dt, COLOSSUS_SPEED, 2.0, COLOSSUS_BLOW, f64::INFINITY),
            _ => self.walk_bones(arena, f, r, dt, BONES_SPEED, BONES_REACH, BONES_BLOW, PHALANX_GUARD),
        }
    }

    /// What one of the dead at `from` goes after: what it was sent at, or the nearest thing that
    /// fights within `hunt` meters of it and `guard` of `post`
    fn minion_prey(&self, arena: &Arena, from: Vector3<f64>, post: Vector3<f64>, hunt: f64, guard: f64) -> Option<usize> {
        let vpm = arena.view.vpm;
        self.ordered_prey(arena).or_else(|| {
            (0..arena.animals.len())
                .filter(|&i| hostile(&arena.animals[i]) && (arena.animals[i].position - post).xy().norm() < guard * vpm)
                .map(|i| ((arena.animals[i].position - from).xy().norm(), i))
                .filter(|&(d, _)| d < hunt * vpm)
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, i)| i)
        })
    }

    /// Walking `f` towards `goal` on the ground at `speed` meters a second till it's within `stop`
    /// voxels of it, facing it: true if it's there
    fn step_towards(&mut self, arena: &Arena, f: &mut Familiar, r: &mut Risen, goal: Vector3<f64>, stop: f64, speed: f64, dt: f64) -> bool {
        let vpm = arena.view.vpm;
        let to = (goal - f.position).xy();
        let d = to.norm();
        if d > 1e-6 {
            r.heading = to.y.atan2(to.x);
        }
        if d <= stop {
            r.moving = false;
            return true;
        }
        let way = to / d;
        let step = (speed * vpm * dt).min(d - stop);
        f.position = Combat::floor_at(arena, f.position + Vector3::new(way.x, way.y, 0.0) * step);
        r.phase = (r.phase + step / (1.3 * vpm * (speed / BONES_SPEED).max(0.6))).fract();
        r.moving = true;
        false
    }

    /// A skeleton of the phalanx, or the colossus, going on: walking at what it's after and
    /// striking it, a phalanx's going back to stand its ground where it rose when there's nothing
    /// near it, the colossus following the player; the colossus's blow striking all round where it
    /// lands and shaking the ground
    #[allow(clippy::too_many_arguments)]
    fn walk_bones(&mut self, arena: &mut Arena, f: &mut Familiar, mut r: Risen, dt: f64, speed: f64, reach: f64, blow: f64, guard: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let colossus = r.kind == Minion::Colossus;
        // The ground round the colossus heaving as it stands up out of it, throwing what's there
        if colossus && r.age - dt < RISE_SECONDS {
            let profile = f.profile.clone();
            let near: Vec<usize> = Combat::foes_round(arena, f.position + up * 0.5 * vpm, COLOSSUS_ROUND).into_iter().filter(|&i| hostile(&arena.animals[i])).collect();
            for i in near {
                let at = arena.animals[i].center(vpm as u32);
                self.strike(arena, &profile, i, Blow { damage: f.damage * 0.6, crit: false, heavy: true, from: f.position, depth: 1, element: profile.element.or(Some(Element::Void)) });
                self.sparks(at, 6, 3.0, Element::look(None), 1.0, 0.4);
                let a = &mut arena.animals[i];
                if a.health > 0.0 {
                    a.status.staggered = a.status.staggered.max(0.6);
                    if a.boss.is_some() {
                        a.status.limit_for_boss();
                    }
                }
            }
            for _ in 0..8 {
                let v = (self.any_direction() * 2.5 + up * 3.0) * vpm;
                self.drop(f.position + up * 0.3 * vpm, v, DROP_MUD, 1.1);
            }
            if (f.position - arena.view.eye).norm() > 3.0 * vpm {
                self.swells.push(Swell { model: WAVE, position: f.position, radius: COLOSSUS_ROUND, life: 0.5, lifetime: 0.5 });
            }
            self.cues.push(Cue::at(Sound::Quake, f.position, 0.9).pitched(0.7));
            self.shake = self.shake.max(0.3);
        }
        if let Some(s) = r.swing {
            let next = s + dt / blow;
            if s < LANDS && next >= LANDS {
                let ahead = Vector3::new(r.heading.cos(), r.heading.sin(), 0.0);
                let lands = f.position + ahead * reach * 0.8 * vpm + up * 0.9 * vpm;
                let round = if colossus { COLOSSUS_ROUND } else { 0.9 };
                let struck: Vec<usize> = Combat::foes_round(arena, lands, round).into_iter().filter(|&i| hostile(&arena.animals[i])).take(if colossus { 6 } else { 1 }).collect();
                let profile = f.profile.clone();
                for i in struck {
                    let at = arena.animals[i].center(vpm as u32);
                    self.strike(arena, &profile, i, Blow { damage: f.damage, crit: false, heavy: colossus, from: f.position + up * vpm, depth: 1, element: profile.element.or(Some(Element::Void)) });
                    self.heal += f.damage * RISEN_DRAIN * if colossus { 0.5 } else { 1.0 };
                    self.sparks(at, 8, 3.0, Element::look(Some(Element::Blood)), 0.6, 0.35);
                    let a = &mut arena.animals[i];
                    if a.health > 0.0 {
                        a.status.staggered = a.status.staggered.max(if colossus { 0.8 } else { 0.4 });
                        if a.boss.is_some() {
                            a.status.limit_for_boss();
                        } else if colossus {
                            let away = (a.position - lands).xy().try_normalize(1e-6).map_or(Vector3::zeros(), |v| Vector3::new(v.x, v.y, 0.5));
                            a.status.push = away * 6.0 * vpm;
                        }
                    }
                    self.cues.push(Cue::at(Sound::Hit, at, 0.6).pitched(if colossus { 0.6 } else { 0.8 }));
                }
                if colossus {
                    let ground = Combat::floor_at(arena, lands);
                    for _ in 0..5 {
                        let v = (self.any_direction() * 2.0 + up * 2.5) * vpm;
                        self.drop(ground + up * 0.2 * vpm, v, DROP_MUD, 1.0);
                    }
                    if (ground - arena.view.eye).norm() > 3.0 * vpm {
                        self.swells.push(Swell { model: WAVE, position: ground, radius: COLOSSUS_ROUND, life: 0.5, lifetime: 0.5 });
                    }
                    self.sparks(ground, 14, 4.0, Element::look(None), 1.2, 0.5);
                    self.cues.push(Cue::at(Sound::Slam, ground, 0.9).pitched(0.6));
                    let near = (1.0 - (ground - arena.view.eye).norm() / vpm / 20.0).max(0.0);
                    self.shake = self.shake.max(0.3 * near);
                } else {
                    self.cues.push(Cue::at(Sound::Swing, f.position, 0.4).pitched(0.8));
                }
            }
            r.swing = (next < 1.0).then_some(next);
            r.moving = false;
            f.risen = Some(r);
            return;
        }
        let feet = arena.player.unwrap_or(arena.view.eye - up * 1.6 * vpm);
        let post = if colossus { f.position } else { r.post };
        let prey = self.minion_prey(arena, f.position, post, MINION_HUNT, guard);
        match prey {
            Some(i) => {
                let a = &arena.animals[i];
                let goal = a.position;
                let stop = (reach + a.look.body().1 * 0.7) * vpm;
                if self.step_towards(arena, f, &mut r, goal, stop, speed, dt) && f.wait <= 0.0 {
                    r.swing = Some(0.0);
                    f.wait = blow + if colossus { 0.4 } else { 0.35 };
                }
            }
            // Back to where it stands its ground, or after the player
            None if !colossus => {
                let post = r.post;
                self.step_towards(arena, f, &mut r, post, 0.3 * vpm, speed, dt);
            }
            None => {
                self.step_towards(arena, f, &mut r, feet, 3.5 * vpm, speed, dt);
            }
        }
        f.risen = Some(r);
    }

    /// A skeleton archer going on: keeping back from what's after it, drawing and loosing an arrow
    /// at what's near every ARCHER_PACE, following the player when there's nothing
    fn walk_archer(&mut self, arena: &mut Arena, f: &mut Familiar, mut r: Risen, dt: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let prey = self.minion_prey(arena, f.position, f.position, ARCHER_RANGE, f64::INFINITY);
        if let Some(s) = r.swing {
            let next = s + dt / ARCHER_DRAW;
            if s < LANDS && next >= LANDS
                && let Some(i) = prey
            {
                let from = f.position + Vector3::new(r.heading.cos(), r.heading.sin(), 0.0) * 0.4 * vpm + up * 1.45 * vpm;
                let to = arena.animals[i].center(vpm as u32);
                let way = lead(from, to, ARROW_SPEED, ARROW_GRAVITY, vpm);
                self.launch(&f.profile, Some(Missile::arrow(0, true)), Form::Bolt, from, way * ARROW_SPEED * vpm, f.damage, ARROW_GRAVITY);
                self.cues.push(Cue::at(Sound::Twang, from, 0.5).pitched(1.1));
            }
            r.swing = (next < 1.0).then_some(next);
            r.moving = false;
            f.risen = Some(r);
            return;
        }
        match prey {
            Some(i) => {
                let a = &arena.animals[i];
                let off = (a.position - f.position).xy();
                let d = off.norm() / vpm;
                if d > 1e-6 {
                    r.heading = off.y.atan2(off.x);
                }
                // Stepping back from what's come too near
                if d < ARCHER_BACK + a.look.body().1 {
                    let away = f.position - Vector3::new(off.x, off.y, 0.0) / d;
                    let heading = r.heading;
                    self.step_towards(arena, f, &mut r, away, 0.0, BONES_SPEED * 0.8, dt);
                    r.heading = heading;
                } else {
                    r.moving = false;
                    if f.wait <= 0.0 {
                        r.swing = Some(0.0);
                        f.wait = ARCHER_PACE;
                    }
                }
            }
            None => {
                let feet = arena.player.unwrap_or(arena.view.eye - up * 1.6 * vpm);
                self.step_towards(arena, f, &mut r, feet, 3.5 * vpm, BONES_SPEED, dt);
            }
        }
        f.risen = Some(r);
    }

    /// A wraith going on: hovering, circling what it's after, then diving through it, striking and
    /// chilling it as it passes and flying on and up, resting a moment before the next; circling
    /// the player when there's nothing
    fn fly_wraith(&mut self, arena: &mut Arena, f: &mut Familiar, mut r: Risen, dt: f64) {
        let vpm = arena.view.vpm;
        let up = Vector3::z();
        let mut v = r.post;
        let prey = self.minion_prey(arena, f.position, f.position, MINION_HUNT, f64::INFINITY);
        match prey {
            Some(i) if f.wait <= 0.0 => {
                let center = arena.animals[i].center(vpm as u32);
                let to = center - f.position;
                let dive = to.try_normalize(1e-6).unwrap_or_else(Vector3::x) * WRAITH_DIVE * vpm;
                v = v.lerp(&dive, (8.0 * dt).min(1.0));
                if to.norm() < (0.7 + arena.animals[i].look.body().1) * vpm {
                    let profile = f.profile.clone();
                    self.strike(arena, &profile, i, Blow { damage: f.damage, crit: false, heavy: false, from: f.position, depth: 1, element: Some(Element::Void) });
                    let s = &mut arena.animals[i].status;
                    s.chill = (s.chill + 0.35).min(1.0);
                    s.slowed = s.slowed.max(1.2);
                    s.slow = s.slow.max(0.3);
                    self.sparks(center, 10, 3.0, Element::look(Some(Element::Frost)), 0.2, 0.4);
                    self.cues.push(Cue::at(Sound::Wail, center, 0.4).pitched(1.5));
                    f.wait = WRAITH_REST;
                    v = (to.normalize() * 0.6 + up * 0.6).normalize() * WRAITH_DIVE * 0.8 * vpm;
                }
            }
            _ => {
                let anchor = match prey {
                    Some(i) => arena.animals[i].position,
                    None => arena.player.unwrap_or(arena.view.eye - up * 1.6 * vpm),
                };
                // Each round at its own place, by when it rose
                let a = self.time * 1.3 + (self.time - r.age) * 14.0;
                let ground = Combat::floor_at(arena, anchor);
                let goal = Vector3::new(anchor.x, anchor.y, ground.z) + Vector3::new(a.cos(), a.sin(), 0.0) * 3.5 * vpm + up * (WRAITH_HOVER + 0.6) * vpm;
                let want = (goal - f.position) * 2.0;
                let want = if want.norm() > WRAITH_FLY * vpm { want.normalize() * WRAITH_FLY * vpm } else { want };
                v = v.lerp(&want, (3.0 * dt).min(1.0));
            }
        }
        f.position += v * dt;
        let ground = Combat::floor_at(arena, f.position);
        f.position.z = f.position.z.max(ground.z + 0.4 * vpm);
        r.post = v;
        if v.xy().norm() > 0.1 * vpm {
            r.heading = v.y.atan2(v.x);
        }
        r.moving = true;
        r.phase = (r.phase + dt * 1.5).fract();
        if random(&mut self.rng) < dt * 20.0 {
            let at = f.position + up * (0.5 + random(&mut self.rng)) * vpm;
            self.fine(at, -v * 0.2, Element::look(Some(Element::Void)), 0.4, 0.0);
        }
        f.risen = Some(r);
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

    /// Using `part`'s power as a player would at a pack of `pack` ahead, watched `seconds`: how
    /// many of the pack were hurt, the most of the dead up at once, what they were, and the
    /// combat after
    fn raised(part: &str, pack: &[Look], seconds: f64, again: Option<f64>) -> (usize, usize, Vec<(Look, Minion)>, Combat) {
        let library = Library::load();
        let world = field();
        let weapon = library.with_part(part, None).unwrap_or_else(|| panic!("no weapon with {part}"));
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let view = View { eye: feet() + Vector3::new(0.0, 0.0, 1.6 * VPM), rotation: UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.15), tan: [1.2445, 0.7002], vpm: VPM, speed: 0.0, stride: 0.0 };
        let mut animals = crate::animals::spawn_pack(&world, &dry, pack, feet() + Vector3::new(0.0, 9.0 * VPM, 0.0), crate::animals::PACK_SPREAD, 5);
        let mut combat = Combat::new(1, 0);
        combat.mana = 1000.0;
        let dt = 1.0 / 60.0;
        let (mut most, mut kinds) = (0, Vec::new());
        for frame in 0..(seconds / dt) as usize {
            let t = frame as f64 * dt - 0.4;
            let mut controls = combat.power_controls(&weapon, t, dt);
            if let Some(at) = again
                && t >= at
                && t < at + dt * 1.5
            {
                controls.heavy = true;
                controls.aiming = true;
            }
            let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet()), water: &dry };
            combat.update(&mut arena, Some((0, &weapon)), None, controls, dt);
            let up: Vec<(Look, Minion)> = combat.familiars.iter().filter_map(|f| f.risen.map(|r| (r.look, r.kind))).collect();
            most = most.max(up.len());
            for k in up {
                if !kinds.contains(&k) {
                    kinds.push(k);
                }
            }
        }
        (animals.iter().filter(|a| a.health < a.max_health).count(), most, kinds, combat)
    }

    /// Each summoning power raises what it should, as many as it should, and they fight: the
    /// phalanx's skeletons, the archers' arrows, the colossus's blows and the wraiths' swoops all
    /// hurt the pack
    #[test]
    fn each_raises_its_own_and_they_fight() {
        for (part, look, kind, count) in [
            ("phalanx_binding", Look::ArmoredSkeleton, Minion::Phalanx, PHALANX),
            ("archers_crystal", Look::BoneArcher, Minion::Archer, ARCHERS),
            ("colossus_focus", Look::GraveBrute, Minion::Colossus, 1),
            ("wraith_shaft", Look::RimeWraith, Minion::Wraith, WRAITHS),
            ("corpse_crystal", Look::Zombie, Minion::Corpse, CORPSES_LEAST),
        ] {
            let (hurt, most, kinds, _) = raised(part, &[Look::Bandit; 3], 8.0, None);
            assert!(kinds.contains(&(look, kind)), "{part} raised {kinds:?}");
            assert_eq!(most, count, "{part} raised {most}");
            assert!(hurt >= 1, "{part}'s dead hurt none of the pack");
        }
    }

    /// The archers shoot arrows to be seen flying, rather than striking from afar unseen
    #[test]
    fn archers_shoot_arrows() {
        let library = Library::load();
        let world = field();
        let weapon = library.with_part("archers_crystal", None).unwrap();
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let view = View { eye: feet() + Vector3::new(0.0, 0.0, 1.6 * VPM), rotation: UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.15), tan: [1.2445, 0.7002], vpm: VPM, speed: 0.0, stride: 0.0 };
        let mut animals = crate::animals::spawn_pack(&world, &dry, &[Look::Bandit; 2], feet() + Vector3::new(0.0, 20.0 * VPM, 0.0), crate::animals::PACK_SPREAD, 5);
        let mut combat = Combat::new(1, 0);
        combat.mana = 1000.0;
        let dt = 1.0 / 60.0;
        let mut arrows = 0;
        for frame in 0..(6.0 / dt) as usize {
            let t = frame as f64 * dt - 0.4;
            let controls = combat.power_controls(&weapon, t, dt);
            let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet()), water: &dry };
            combat.update(&mut arena, Some((0, &weapon)), None, controls, dt);
            arrows = arrows.max(combat.projectiles.iter().filter(|p| p.arrow && p.shot == Shot::Arrow).count());
        }
        assert!(arrows >= 1, "no arrows were seen flying");
    }

    /// The slain rise as what they were, and no more than MINIONS of the dead are ever up at once
    #[test]
    fn the_slain_rise_as_themselves_and_no_more_than_eight() {
        let library = Library::load();
        let world = field();
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let view = View { eye: feet() + Vector3::new(0.0, 0.0, 1.6 * VPM), rotation: UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -0.15), tan: [1.2445, 0.7002], vpm: VPM, speed: 0.0, stride: 0.0 };
        // Three wolves lying slain where the shade comes down
        let mut animals = crate::animals::spawn_pack(&world, &dry, &[Look::Wolf; 3], feet() + Vector3::new(0.0, 9.0 * VPM, 0.0), 1.5, 5);
        for a in &mut animals {
            a.health = 0.0;
        }
        let mut combat = Combat::new(1, 0);
        combat.mana = 1000.0;
        let corpses = library.with_part("corpse_crystal", None).unwrap();
        let phalanx = library.with_part("phalanx_binding", None).unwrap();
        let dt = 1.0 / 60.0;
        let mut most = 0;
        for (weapon, from) in [(&corpses, 0.0), (&phalanx, 2.5), (&corpses, 5.0)] {
            combat.power.cooldown = 0.0;
            for frame in 0..(2.8 / dt) as usize {
                let t = frame as f64 * dt + from;
                let controls = combat.power_controls(weapon, t - from - 0.5, dt);
                let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet()), water: &dry };
                combat.update(&mut arena, Some((0, weapon)), None, controls, dt);
                let up = combat.familiars.iter().filter(|f| f.risen.is_some() && f.life > CRUMBLE_SECONDS).count();
                most = most.max(up);
            }
            if from == 0.0 {
                let wolves = combat.familiars.iter().filter(|f| f.risen.is_some_and(|r| r.look == Look::Wolf && r.kind == Minion::Corpse)).count();
                assert_eq!(wolves, 3, "the three slain wolves rose as wolves");
            }
        }
        assert!(most <= MINIONS, "{most} of the dead were up at once");
        assert!(most >= 7, "only {most} were ever up");
    }

    /// Right click while it comes round sends every one of them at what's aimed at
    #[test]
    fn right_click_sends_them_at_what_is_aimed_at() {
        let (_, _, _, combat) = raised("phalanx_binding", &[Look::Bandit; 2], 4.0, Some(3.0));
        assert!(combat.ordered.is_some_and(|(_, left)| left > 0.0), "they weren't sent at anything");
        assert!(combat.power.cooldown > 0.0);
    }
}
