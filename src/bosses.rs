//! Bosses: a few huge creatures guarding ruins, each fighting its own way. As far as everything
//! else goes they're animals, hit by weapons the same way. This is what they think and do: what
//! they do to the player is left to src\game.rs, as Deeds.

use std::f64::consts::{PI, TAU};

use nalgebra::Vector3;

use crate::animals::{Animal, Build, Look, Mood, POSE_SPECIAL, POSE_STRIKE, POSE_WINDUP};
use crate::worldgen::{World, is_passable};

/// How near the player has to come before a boss turns on them, and how far off they can get
/// before it gives up, in meters
const NOTICE: f64 = 24.0;
const GIVE_UP: f64 = 50.0;
/// The furthest from home it'll follow them
const LEASH: f64 = 60.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Move {
    /// Fists or paws brought down on the ground in front, hurting everything near
    Slam,
    /// A boulder hurled at the player
    Throw,
    /// Fire breathed out in a stream
    Breath,
    /// A fireball spat
    Spit,
    /// Charging headlong at the player
    Charge,
    /// A roar, on first seeing the player and again on getting angry
    Roar,
    /// Leaping through the air to come down on the player
    Leap,
    /// Roots bursting up from under the player, a moment after it calls them
    Roots,
    /// A volley of thorns
    Volley,
}

const MOVES: usize = 9;

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    /// Wandering round home
    Roam,
    /// Going after the player
    Chase,
    /// Getting ready, so the player can see it coming: with how long it's been at it
    WindUp(Move, f64),
    /// Doing it
    Act(Move, f64),
    /// Getting its breath back
    Recover(f64),
    /// Dazed from running into something
    Stunned(f64),
}

/// What a boss did this frame, for src\game.rs to see to. Positions are in voxels.
#[derive(Clone, Copy)]
pub enum Deed {
    /// Getting ready to make a move, here
    WindUp(Move, Vector3<f64>),
    /// Roaring on first seeing the player, or getting `angry`
    Roar { at: Vector3<f64>, angry: bool },
    /// A blow landing on the ground at `at`: hurting what's within `radius` meters, and blowing a
    /// hole `crater` meters round
    Slam { at: Vector3<f64>, radius: f64, damage: f64, crater: f64 },
    Throw { from: Vector3<f64>, to: Vector3<f64> },
    Spit { from: Vector3<f64>, to: Vector3<f64> },
    /// Breathing fire for `dt` seconds
    Breath { from: Vector3<f64>, direction: Vector3<f64>, dt: f64 },
    /// Charging into the player, who's thrown along `direction`
    Ram { damage: f64, direction: Vector3<f64> },
    /// A heavy footfall
    Stomp(Vector3<f64>),
    /// Roots called up from under `at`: they burst up a moment later
    Roots { at: Vector3<f64> },
    /// Thorns shot from `from` at `to`
    Volley { from: Vector3<f64>, to: Vector3<f64> },
    /// Showing on the ground where it's coming down, `radius` meters round, `closing` from 1 to 0
    /// as it comes
    Mark { at: Vector3<f64>, radius: f64, closing: f64 },
}

pub struct Boss {
    look: Look,
    /// Where its lair is in the whole landscape, to remember it's been beaten
    pub key: [i64; 3],
    /// Where it guards, in voxels
    pub home: Vector3<f64>,
    phase: Phase,
    /// Fighting the player
    pub engaged: bool,
    /// Hurt badly enough to attack faster
    pub enraged: bool,
    /// Seconds until each move can be made again
    cooldowns: [f64; MOVES],
    /// Where its move's aimed
    aim: Vector3<f64>,
    /// Whether its charge has hit already
    rammed: bool,
    /// Where a leap started and where it's coming down
    leap: (Vector3<f64>, Vector3<f64>),
    /// How far through its walk it was at the last footfall
    last_step: f64,
    pub deeds: Vec<Deed>,
}

/// How long each move takes to get ready for, to do, and to get over, in seconds, and then how
/// long until it can be made again
fn windup(look: Look, m: Move) -> f64 {
    match (look.build(), m) {
        (_, Move::Roar) => 0.0,
        (Some(Build::Drake), Move::Slam) => 0.6,
        (Some(Build::Bear), Move::Slam) => 0.8,
        (Some(Build::Spider), Move::Slam) => 0.7,
        (_, Move::Slam | Move::Throw | Move::Charge) => 0.9,
        (_, Move::Breath) => 0.7,
        (_, Move::Spit) => 0.55,
        (_, Move::Leap) => 0.6,
        (_, Move::Roots) => 0.8,
        (_, Move::Volley) => 0.7,
    }
}

fn duration(m: Move) -> f64 {
    match m {
        Move::Slam => 0.55,
        Move::Throw => 0.45,
        Move::Breath => 1.8,
        Move::Spit => 0.4,
        Move::Charge => 2.4,
        Move::Roar => 1.4,
        Move::Leap => LEAP_TIME,
        Move::Roots => 0.6,
        Move::Volley => 0.5,
    }
}

fn recovery(m: Move) -> f64 {
    match m {
        Move::Slam => 0.9,
        Move::Throw => 0.7,
        Move::Breath => 1.0,
        Move::Spit => 0.5,
        Move::Charge => 0.8,
        Move::Roar => 0.2,
        Move::Leap => 0.7,
        Move::Roots => 0.6,
        Move::Volley => 0.6,
    }
}

fn cooldown(m: Move) -> f64 {
    match m {
        Move::Slam => 2.6,
        Move::Throw => 4.5,
        Move::Breath => 6.5,
        Move::Spit => 3.0,
        Move::Charge => 5.5,
        Move::Roar => 0.0,
        Move::Leap => 5.0,
        Move::Roots => 4.0,
        Move::Volley => 4.5,
    }
}

/// Seconds a leap's in the air, and how far round in meters it hurts where it comes down
const LEAP_TIME: f64 = 0.8;
const LEAP_RADIUS: f64 = 3.0;
/// Radians a second it can turn while breathing, slow enough to run out of the way of
const BREATH_TURN: f64 = 0.7;

impl Boss {
    pub fn new(look: Look, home: Vector3<f64>, key: [i64; 3]) -> Self {
        Boss {
            look,
            key,
            home,
            phase: Phase::Roam,
            engaged: false,
            enraged: false,
            cooldowns: [0.0; MOVES],
            aim: home,
            rammed: false,
            leap: (home, home),
            last_step: 0.0,
            deeds: Vec::new(),
        }
    }

    /// Hit by the player: it turns on them
    pub fn provoke(&mut self) {
        self.engaged = true;
    }

    /// The world moved over by `shift` voxels
    pub fn recenter(&mut self, shift: Vector3<f64>) {
        self.home -= shift;
        self.aim -= shift;
        self.leap.0 -= shift;
        self.leap.1 -= shift;
    }

    /// Gives up the fight, as when the player's beaten, and goes home. Partway through a leap, it
    /// comes down first.
    pub fn calm(&mut self) {
        self.engaged = false;
        self.enraged = false;
        if !matches!(self.phase, Phase::Act(Move::Leap, _)) {
            self.phase = Phase::Roam;
        }
    }

    /// Which pose frame it's in, see animals::POSE_WINDUP, or None to walk or stand
    pub fn pose(&self) -> Option<usize> {
        match (self.phase, self.look.build()) {
            (Phase::Act(Move::Roar, _), Some(Build::Drake)) => Some(POSE_SPECIAL),
            (Phase::Act(Move::Roar, _), _) => Some(POSE_WINDUP),
            (Phase::WindUp(Move::Throw | Move::Charge | Move::Leap | Move::Roots, _), _) => Some(POSE_SPECIAL),
            (Phase::WindUp(Move::Slam | Move::Breath | Move::Spit | Move::Volley, _), _) => Some(POSE_WINDUP),
            (Phase::Act(Move::Roots, _), _) => Some(POSE_SPECIAL),
            (Phase::Act(Move::Slam | Move::Throw | Move::Breath | Move::Spit | Move::Leap | Move::Volley, _), _) => Some(POSE_STRIKE),
            _ => None,
        }
    }

    /// The move it can make from `distance` meters off, if any's ready
    fn choose(&self, distance: f64) -> Option<Move> {
        let ready = |m: Move| self.cooldowns[m as usize] <= 0.0;
        let within = |m: Move, near: f64, far: f64| (near..far).contains(&distance) && ready(m);
        let moves: &[(Move, f64, f64)] = match self.look.build() {
            // Ranges meeting, so there's nowhere to stand it won't do something
            Some(Build::Golem) => &[(Move::Slam, 0.0, 4.6), (Move::Throw, 4.6, 32.0)],
            Some(Build::Drake) => &[(Move::Slam, 0.0, 4.0), (Move::Breath, 0.0, 9.0), (Move::Spit, 4.0, 35.0)],
            Some(Build::Bear) => &[(Move::Slam, 0.0, 4.0), (Move::Charge, 4.0, 32.0)],
            Some(Build::Spider) => &[(Move::Slam, 0.0, 3.5), (Move::Leap, 7.0, 22.0), (Move::Spit, 3.5, 25.0)],
            Some(Build::Treant) => &[(Move::Slam, 0.0, 5.0), (Move::Roots, 4.0, 28.0), (Move::Volley, 8.0, GIVE_UP)],
            None => &[],
        };
        moves.iter().find(|&&(m, near, far)| within(m, near, far)).map(|&(m, _, _)| m)
    }
}

/// Where a point of a boss's model, in meters with +x ahead, is in the world
fn on_body(a: &Animal, point: [f64; 3], vpm: f64) -> Vector3<f64> {
    let (c, s) = (a.heading.cos(), a.heading.sin());
    a.position + Vector3::new(c * point[0] - s * point[1], s * point[0] + c * point[1], point[2]) * vpm
}

/// Where fire, frost or venom comes out of it
fn mouth(a: &Animal, vpm: f64) -> Vector3<f64> {
    match a.look.build() {
        Some(Build::Spider) => on_body(a, [1.35, 0.0, 1.1], vpm),
        Some(Build::Treant) => on_body(a, [0.6, 0.0, 3.0], vpm),
        _ => on_body(a, [2.3, 0.0, 1.55], vpm),
    }
}

fn heading_to(from: Vector3<f64>, to: Vector3<f64>) -> f64 {
    let d = to - from;
    d.y.atan2(d.x)
}

/// What boss `a` does this frame: noticing the player at `player`, their feet in voxels, going after
/// them, and attacking. What its attacks do goes in its deeds.
pub fn think(a: &mut Animal, world: &World, player: Option<Vector3<f64>>, dt: f64) {
    let vpm = world.voxels_per_meter as f64;
    let look = a.look;
    let vitality = a.vitality();
    let Some(mut boss) = a.boss.take() else {
        return;
    };
    for c in &mut boss.cooldowns {
        *c -= dt;
    }
    let forward = Vector3::new(a.heading.cos(), a.heading.sin(), 0.0);
    let distance = player.map_or(f64::MAX, |p| (p - a.position).xy().norm() / vpm);
    let from_home = (a.position - boss.home).xy().norm() / vpm;

    // Turning on the player when they come near, and giving up if they get away
    if player.is_some() && distance < NOTICE {
        boss.engaged = true;
    }
    if boss.engaged && (player.is_none() || distance > GIVE_UP || from_home > LEASH) {
        boss.calm();
    }
    let busy = matches!(boss.phase, Phase::WindUp(..) | Phase::Act(..));
    if boss.engaged && boss.phase == Phase::Roam {
        boss.phase = Phase::Act(Move::Roar, 0.0);
        boss.deeds.push(Deed::Roar { at: a.position, angry: false });
    } else if boss.engaged && !boss.enraged && vitality < 0.5 && !busy {
        boss.enraged = true;
        boss.phase = Phase::Act(Move::Roar, 0.0);
        boss.deeds.push(Deed::Roar { at: a.position, angry: true });
    }
    // Angry, it's quicker about everything
    let quick: f64 = if boss.enraged { 0.75 } else { 1.0 };

    // The heavy ones shake the ground as they walk
    let heavy = matches!(look.build(), Some(Build::Golem | Build::Bear | Build::Treant));
    if heavy && a.speed > 0.3 && (a.phase * 2.0).floor() != (boss.last_step * 2.0).floor() {
        boss.deeds.push(Deed::Stomp(a.position));
    }
    boss.last_step = a.phase;

    boss.phase = match boss.phase {
        Phase::Roam => {
            // Healing back up, wandering round home
            a.health = (a.health + look.health() * 0.05 * dt).min(look.health());
            if from_home > 6.0 {
                a.mood = Mood::Walk;
                a.target_heading = heading_to(a.position, boss.home);
            } else {
                a.mood = Mood::Idle;
            }
            Phase::Roam
        }
        Phase::Chase => {
            let p = player.unwrap_or(boss.home);
            a.target_heading = heading_to(a.position, p);
            // Only once it's facing them more or less
            let facing = ((a.target_heading - a.heading + PI).rem_euclid(TAU) - PI).abs() < 0.6;
            match boss.choose(distance).filter(|_| facing) {
                Some(m) => {
                    a.mood = Mood::Idle;
                    boss.aim = p;
                    boss.deeds.push(Deed::WindUp(m, a.position));
                    Phase::WindUp(m, 0.0)
                }
                None => {
                    let close = match look.build() {
                        Some(Build::Drake) => 6.0,
                        _ => 3.0,
                    };
                    a.mood = if distance < close {
                        Mood::Idle
                    } else if matches!(look.build(), Some(Build::Bear | Build::Treant)) {
                        Mood::Walk
                    } else {
                        Mood::Run
                    };
                    Phase::Chase
                }
            }
        }
        Phase::WindUp(m, t) => {
            a.mood = Mood::Idle;
            // Following the player with its eyes as it gets ready
            if let Some(p) = player {
                boss.aim = p;
                a.target_heading = heading_to(a.position, p);
            }
            let t = t + dt;
            if t < windup(look, m) * quick.max(0.8) {
                Phase::WindUp(m, t)
            } else {
                // Now it does it
                boss.cooldowns[m as usize] = cooldown(m) * quick;
                match m {
                    Move::Slam => {
                        let (reach, radius, damage, crater) = match look.build() {
                            Some(Build::Golem) => (2.0, 3.4, 32.0, 1.1),
                            Some(Build::Bear) => (2.2, 2.8, 26.0, 0.7),
                            Some(Build::Spider) => (1.8, 2.4, 20.0, 0.3),
                            Some(Build::Treant) => (2.4, 3.6, 30.0, 0.8),
                            _ => (2.0, 2.6, 22.0, 0.5),
                        };
                        boss.deeds.push(Deed::Slam { at: a.position + forward * reach * vpm, radius, damage, crater });
                    }
                    Move::Throw => boss.deeds.push(Deed::Throw { from: on_body(a, [-0.55, -0.55, 4.05], vpm), to: boss.aim }),
                    Move::Spit => boss.deeds.push(Deed::Spit { from: mouth(a, vpm), to: boss.aim + Vector3::new(0.0, 0.0, 1.0 * vpm) }),
                    Move::Charge => {
                        boss.rammed = false;
                        a.target_heading = heading_to(a.position, boss.aim);
                        a.heading = a.target_heading;
                    }
                    Move::Leap => {
                        // Coming down just short of the player, not on top of them
                        let off = boss.aim - a.position;
                        let short = Vector3::new(off.x, off.y, 0.0).try_normalize(1e-6).unwrap_or(forward) * (look.body().1 + 1.0) * vpm;
                        // On the ground there with room over it, not in the air or in a wall; where
                        // the player stands if there's nowhere like that
                        let to = boss.aim - short;
                        let (x, y) = (to.x.floor() as i64, to.y.floor() as i64);
                        let top = to.z.floor() as i64 + 2 * vpm as i64;
                        let room = |z: i64| (1..=(1.5 * vpm) as i64).all(|k| is_passable(world.material_at(x, y, z + k)));
                        let ground = (top - 6 * vpm as i64..=top).rev().find(|&z| !is_passable(world.material_at(x, y, z)) && room(z));
                        let to = ground.map_or(boss.aim, |z| Vector3::new(to.x, to.y, z as f64 + 1.0));
                        boss.leap = (a.position, to);
                        a.heading = heading_to(a.position, boss.aim);
                        a.target_heading = a.heading;
                        boss.deeds.push(Deed::Stomp(a.position));
                    }
                    Move::Roots => boss.deeds.push(Deed::Roots { at: boss.aim }),
                    Move::Volley => boss.deeds.push(Deed::Volley { from: mouth(a, vpm), to: boss.aim + Vector3::new(0.0, 0.0, 1.0 * vpm) }),
                    Move::Breath | Move::Roar => {}
                }
                Phase::Act(m, 0.0)
            }
        }
        Phase::Act(m, t) => {
            let t = t + dt;
            a.mood = Mood::Idle;
            let mut over = t >= duration(m);
            match m {
                Move::Breath => {
                    // Sweeping slowly round after the player as it breathes, along the way it's
                    // facing, so they can get out of the way of it
                    if let Some(p) = player {
                        let turn = (heading_to(a.position, p) - a.heading + PI).rem_euclid(TAU) - PI;
                        a.target_heading = a.heading + turn.clamp(-BREATH_TURN * dt, BREATH_TURN * dt);
                    }
                    let from = mouth(a, vpm);
                    let to = player.map_or(from + forward * 8.0 * vpm, |p| p + Vector3::new(0.0, 0.0, 1.0 * vpm));
                    let ahead = (to - from).xy().norm().max(1.0);
                    let direction = Vector3::new(forward.x * ahead, forward.y * ahead, to.z - from.z).normalize();
                    boss.deeds.push(Deed::Breath { from, direction, dt });
                }
                Move::Charge => {
                    a.mood = Mood::Run;
                    if !boss.rammed && distance < 2.8 {
                        boss.rammed = true;
                        boss.deeds.push(Deed::Ram { damage: 34.0, direction: forward });
                    }
                    // Run into a wall or a tree, it's left dazed: the player's chance
                    if t > 0.35 && a.speed < 1.0 {
                        boss.deeds.push(Deed::Stomp(a.position));
                        boss.deeds.push(Deed::Slam { at: a.position + forward * 2.0 * vpm, radius: 0.0, damage: 0.0, crater: 0.6 });
                        boss.phase = Phase::Stunned(2.0);
                        a.boss = Some(boss);
                        return;
                    }
                    // Past the player, it slows and turns round
                    over = over || (boss.rammed && distance > 6.0);
                }
                // Through the air in an arc, coming down hard where the player was
                Move::Leap => {
                    let (from, to) = boss.leap;
                    let f = (t / LEAP_TIME).min(1.0);
                    let height = 4.0 * vpm * (PI * f).sin();
                    a.position = from.lerp(&to, f) + Vector3::new(0.0, 0.0, height);
                    a.speed = 0.0;
                    boss.deeds.push(Deed::Mark { at: to, radius: LEAP_RADIUS, closing: 1.0 - f });
                    if over {
                        a.position = to;
                        boss.deeds.push(Deed::Slam { at: to, radius: LEAP_RADIUS, damage: 26.0, crater: 0.6 });
                        boss.deeds.push(Deed::Stomp(to));
                    }
                }
                _ => {}
            }
            if over { Phase::Recover(recovery(m) * quick) } else { Phase::Act(m, t) }
        }
        Phase::Recover(t) => {
            a.mood = Mood::Idle;
            if t - dt > 0.0 { Phase::Recover(t - dt) } else { Phase::Chase }
        }
        Phase::Stunned(t) => {
            a.mood = Mood::Idle;
            a.speed = 0.0;
            if t - dt > 0.0 { Phase::Stunned(t - dt) } else { Phase::Chase }
        }
    };
    if !boss.engaged && boss.phase != Phase::Roam && !matches!(boss.phase, Phase::Act(..)) {
        boss.phase = Phase::Roam;
    }
    a.boss = Some(boss);
}
