//! Bosses: a few huge creatures guarding ruins, each fighting its own way. As far as everything
//! else goes they're animals, hit by weapons the same way. This is what they think and do: what
//! they do to the player is left to src\game.rs, as Deeds.

use std::f64::consts::{PI, TAU};

use nalgebra::Vector3;

use crate::animals::{self, Animal, Build, Look, Mood, POSE_SPECIAL, POSE_STRIKE, POSE_WINDUP};
use crate::weapons::Element;
use crate::worldgen::{World, is_passable};

/// How near the player has to come before a boss turns on them, and how far off they can get
/// before it gives up, in meters
const NOTICE: f64 = 24.0;
const GIVE_UP: f64 = 50.0;
/// The furthest from home it'll follow them
const LEASH: f64 = 60.0;
/// The same for the walking dead
const MONSTER_NOTICE: f64 = 16.0;
const MONSTER_GIVE_UP: f64 = 28.0;
const MONSTER_LEASH: f64 = 35.0;

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
    /// The ground pounded so a shockwave runs out across it, to be jumped
    Quake,
    /// Its element called down out of the sky round the player
    Storm,
    /// Three lunging swipes, the last the hardest
    Maul,
    /// A ball of web spat, leaving the ground sticky
    Web,
    /// A line of roots bursting up one after another, dragging the player in
    RootWave,
    /// Calling up monsters of its kind to help it: a howl, the dead rising, spiderlings
    Summon,
}

const MOVES: usize = 15;

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
    /// A shockwave running out from `at`, its front gone from `from` to `to` meters out this frame
    /// of `dt` seconds
    Quake { at: Vector3<f64>, from: f64, to: f64, dt: f64 },
    /// The `k`th of its element coming down out of the sky on the player at `to`
    Storm { to: Vector3<f64>, k: usize },
    /// Web spat from `from` at `to`
    Web { from: Vector3<f64>, to: Vector3<f64> },
    /// A line of roots called up from `from` to `to`, `lanes` of them fanned out
    RootWave { from: Vector3<f64>, to: Vector3<f64>, lanes: usize },
    /// The `k`th swipe of a maul starting, here
    Swipe(Vector3<f64>, usize),
    /// `count` monsters of `look` called up round `at`
    Summon { at: Vector3<f64>, look: Look, count: usize },
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
    /// The pose it's in and the one it was in before, see animals::POSE_WINDUP, None to walk or
    /// stand, and seconds since it changed, to ease from one to the other
    posed: (Option<usize>, Option<usize>, f64),
    pub deeds: Vec<Deed>,
}

/// Seconds a boss takes easing from one pose to the next
const EASE: f64 = 0.14;

/// How long each move takes to get ready for, to do, and to get over, in seconds, and then how
/// long until it can be made again
fn windup(look: Look, m: Move) -> f64 {
    // Monsters are quick about it, but not so quick it can't be seen coming
    if look.is_monster() {
        return match (look, m) {
            (_, Move::Roar) => 0.0,
            (Look::Wolf, _) => 0.5,
            (Look::Spiderling, Move::Leap) => 0.45,
            (Look::Spiderling, _) => 0.4,
            (Look::Zombie, _) => 0.75,
            (Look::Skeleton, _) => 0.8,
            _ => 0.85,
        };
    }
    match (look.build(), m) {
        (_, Move::Roar) => 0.0,
        (_, Move::Summon) => 1.2,
        (Some(Build::Wolf), Move::Slam) => 0.55,
        (Some(Build::Drake), Move::Slam) => 0.6,
        (Some(Build::Bear), Move::Slam) => 0.8,
        (Some(Build::Spider), Move::Slam) => 0.7,
        (_, Move::Slam | Move::Throw | Move::Charge) => 0.9,
        (_, Move::Breath) => 0.7,
        (_, Move::Spit) => 0.55,
        (_, Move::Leap) => 0.6,
        (_, Move::Roots) => 0.8,
        (_, Move::Volley) => 0.7,
        (_, Move::Quake) => 1.1,
        (_, Move::Storm | Move::RootWave) => 1.0,
        (_, Move::Maul) => 0.7,
        (_, Move::Web) => 0.6,
    }
}

fn duration(look: Look, m: Move) -> f64 {
    match m {
        Move::Slam if look.is_monster() => 0.35,
        Move::Slam => 0.55,
        Move::Throw => 0.45,
        Move::Breath => 1.8,
        Move::Spit => 0.4,
        Move::Charge => 2.4,
        Move::Roar => 1.4,
        Move::Leap => leap(look).0,
        Move::Summon => 1.0,
        Move::Roots => 0.6,
        Move::Volley => 0.5,
        Move::Quake => QUAKE_REACH / QUAKE_PACE,
        Move::Storm => 1.5,
        Move::Maul => MAUL_SWIPE * 3.0,
        Move::Web => 0.4,
        Move::RootWave => 0.6,
    }
}

fn recovery(look: Look, m: Move) -> f64 {
    match m {
        Move::Slam if matches!(look, Look::Skeleton | Look::ArmoredSkeleton) => 1.0,
        Move::Slam if matches!(look, Look::Zombie | Look::ArmoredZombie) => 0.9,
        Move::Slam | Move::Leap if look.is_monster() => 0.5,
        Move::Summon => 0.8,
        Move::Slam => 0.9,
        Move::Throw => 0.7,
        Move::Breath => 1.0,
        Move::Spit => 0.5,
        Move::Charge => 0.8,
        Move::Roar => 0.2,
        Move::Leap => 0.7,
        Move::Roots => 0.6,
        Move::Volley => 0.6,
        Move::Quake | Move::Storm => 1.0,
        Move::Maul => 1.2,
        Move::Web => 0.6,
        Move::RootWave => 0.8,
    }
}

fn cooldown(look: Look, m: Move) -> f64 {
    match m {
        Move::Slam if matches!(look, Look::Wolf) => 1.6,
        Move::Slam if look == Look::Spiderling => 1.4,
        Move::Slam if look.is_monster() => 2.4,
        Move::Leap if look == Look::Wolf => 6.0,
        Move::Leap if look.is_monster() => 5.0,
        Move::Summon if look.build() == Some(Build::Spider) => 14.0,
        Move::Summon if look.build() == Some(Build::Wolf) => 20.0,
        Move::Summon => 24.0,
        Move::Slam => 2.6,
        Move::Throw => 4.5,
        Move::Breath => 6.5,
        Move::Spit => 3.0,
        Move::Charge => 5.5,
        Move::Roar => 0.0,
        Move::Leap => 5.0,
        Move::Roots => 4.0,
        Move::Volley => 4.5,
        Move::Quake | Move::RootWave => 8.0,
        Move::Storm => 9.0,
        Move::Maul => 7.0,
        Move::Web => 10.0,
    }
}

/// A leap: seconds in the air, how high it goes and how far round it hurts where it comes down in
/// meters, and how hard
fn leap(look: Look) -> (f64, f64, f64, f64) {
    match look {
        Look::Wolf => (0.6, 1.4, 1.8, 12.0),
        Look::Spiderling => (0.5, 1.0, 1.2, 8.0),
        Look::Direwolf | Look::WinterAlpha => (0.7, 3.0, 3.0, 24.0),
        _ => (0.8, 4.0, 3.0, 26.0),
    }
}

/// How near it comes before stopping to strike, in meters, and how far short of the player a
/// leap of its comes down
fn closeness(look: Look) -> (f64, f64) {
    match look {
        Look::Wolf => (1.3, 0.8),
        Look::Skeleton | Look::ArmoredSkeleton => (1.6, 0.5),
        Look::Zombie | Look::ArmoredZombie => (1.4, 0.5),
        Look::Spiderling => (0.9, 0.5),
        _ if look.build() == Some(Build::Drake) => (6.0, look.body().1 + 1.0),
        _ => (3.0, look.body().1 + 1.0),
    }
}

/// A blow at what's in front: how far ahead it lands and how far round it reaches in meters, how
/// hard it hits, and how big a hole it blows
fn slam(look: Look) -> (f64, f64, f64, f64) {
    match look {
        Look::Wolf => (1.0, 1.3, 8.0, 0.0),
        Look::Spiderling => (0.6, 1.0, 5.0, 0.0),
        Look::Skeleton => (1.4, 1.6, 20.0, 0.0),
        Look::ArmoredSkeleton => (1.4, 1.7, 23.0, 0.0),
        Look::Zombie => (1.1, 1.5, 14.0, 0.0),
        Look::ArmoredZombie => (1.1, 1.6, 17.0, 0.0),
        Look::GraveKnight => (2.8, 3.6, 32.0, 1.0),
        _ => match look.build() {
            Some(Build::Golem) => (2.0, 3.4, 32.0, 1.1),
            Some(Build::Bear) => (2.2, 2.8, 26.0, 0.7),
            Some(Build::Spider) => (1.8, 2.4, 20.0, 0.3),
            Some(Build::Treant) => (2.4, 3.6, 30.0, 0.8),
            Some(Build::Wolf) => (2.2, 3.0, 24.0, 0.4),
            _ => (2.0, 2.6, 22.0, 0.5),
        },
    }
}
/// Radians a second it can turn while breathing, slow enough to run out of the way of
const BREATH_TURN: f64 = 0.7;
/// How far a quake's shockwave runs out in meters, and how fast
const QUAKE_REACH: f64 = 14.0;
const QUAKE_PACE: f64 = 10.0;
/// Seconds each swipe of a maul takes, the first part of it lunging
const MAUL_SWIPE: f64 = 0.65;
const MAUL_LUNGE: f64 = 0.4;
/// Seconds between things coming down in a storm, after it starts
const STORM_EVERY: f64 = 0.22;
const STORM_START: f64 = 0.3;

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
            posed: (None, None, EASE),
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

    /// Which frame to draw it in, see animals::POSE_WINDUP, or None to walk or stand: easing through
    /// the frame halfway from its last pose into a new one
    pub fn pose(&self) -> Option<usize> {
        let (now, before, since) = self.posed;
        if since < EASE {
            if let Some(between) = animals::tween(before.unwrap_or(0), now.unwrap_or(0), since / EASE) {
                return Some(between);
            }
        }
        now
    }

    /// Which pose it's striking as it goes about its move, or None to walk or stand
    fn target_pose(&self) -> Option<usize> {
        match (self.phase, self.look.build()) {
            (Phase::Act(Move::Roar, _), Some(Build::Drake)) => Some(POSE_SPECIAL),
            (Phase::Act(Move::Roar, _), _) => Some(POSE_WINDUP),
            // A wolf crouches to spring; howling, the dead raising their arms, calling the rest up
            (Phase::WindUp(Move::Leap, _), Some(Build::Wolf)) => Some(POSE_WINDUP),
            (Phase::WindUp(Move::Summon, _) | Phase::Act(Move::Summon, _), _) => Some(POSE_SPECIAL),
            (Phase::WindUp(Move::Throw | Move::Charge | Move::Leap | Move::Roots | Move::Storm | Move::Maul, _), _) => Some(POSE_SPECIAL),
            (Phase::WindUp(Move::Slam | Move::Breath | Move::Spit | Move::Volley | Move::Quake | Move::Web | Move::RootWave, _), _) => Some(POSE_WINDUP),
            (Phase::Act(Move::Roots | Move::Storm, _), _) => Some(POSE_SPECIAL),
            // Rearing back as it lunges, then bringing it down, three times over
            (Phase::Act(Move::Maul, t), _) if t % MAUL_SWIPE < MAUL_LUNGE => Some(POSE_WINDUP),
            (Phase::Act(Move::Slam | Move::Throw | Move::Breath | Move::Spit | Move::Leap | Move::Volley | Move::Quake | Move::Maul | Move::Web | Move::RootWave, _), _) => Some(POSE_STRIKE),
            _ => None,
        }
    }

    /// The move it can make from `distance` meters off, if any's ready
    fn choose(&self, distance: f64) -> Option<Move> {
        let ready = |m: Move| self.cooldowns[m as usize] <= 0.0;
        let within = |m: Move, near: f64, far: f64| (near..far).contains(&distance) && ready(m);
        let monster = self.look.is_monster();
        let moves: &[(Move, f64, f64)] = match self.look.build() {
            // Monsters bite or swing at what's in reach, and some pounce
            Some(Build::Wolf) if monster => &[(Move::Slam, 0.0, 2.2), (Move::Leap, 4.0, 11.0)],
            Some(Build::Spider) if monster => &[(Move::Slam, 0.0, 1.5), (Move::Leap, 3.5, 9.0)],
            _ if matches!(self.look, Look::Skeleton | Look::ArmoredSkeleton) => &[(Move::Slam, 0.0, 2.8)],
            Some(_) if monster => &[(Move::Slam, 0.0, 2.4)],
            // Ranges meeting, so there's nowhere to stand it won't do something
            Some(Build::Golem) => &[(Move::Slam, 0.0, 4.6), (Move::Quake, 3.0, 13.0), (Move::Throw, 4.6, 32.0)],
            Some(Build::Drake) => &[(Move::Slam, 0.0, 4.0), (Move::Breath, 0.0, 9.0), (Move::Storm, 10.0, 30.0), (Move::Spit, 4.0, 35.0)],
            Some(Build::Bear) => &[(Move::Slam, 0.0, 4.0), (Move::Maul, 2.5, 8.0), (Move::Charge, 4.0, 32.0)],
            Some(Build::Spider) => &[(Move::Slam, 0.0, 3.5), (Move::Summon, 0.0, 30.0), (Move::Web, 6.0, 20.0), (Move::Leap, 7.0, 22.0), (Move::Spit, 3.5, 25.0)],
            Some(Build::Treant) => &[(Move::Slam, 0.0, 5.0), (Move::RootWave, 7.0, 22.0), (Move::Roots, 4.0, 28.0), (Move::Volley, 8.0, GIVE_UP)],
            Some(Build::Wolf) => &[(Move::Slam, 0.0, 3.2), (Move::Summon, 0.0, 30.0), (Move::Maul, 2.5, 8.0), (Move::Leap, 7.0, 22.0)],
            Some(Build::Undead) => &[(Move::Slam, 0.0, 4.6), (Move::Summon, 0.0, 30.0), (Move::Quake, 3.0, 12.0), (Move::Spit, 4.6, 30.0)],
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

/// Where fire, frost or venom comes out of it: its mouth, or the hand the dead cast with
fn mouth(a: &Animal, vpm: f64) -> Vector3<f64> {
    let point = match a.look.build() {
        Some(Build::Spider) => [1.35, 0.0, 1.1],
        Some(Build::Treant) => [0.6, 0.0, 3.0],
        Some(Build::Wolf) => [0.95, 0.0, 0.8],
        Some(Build::Undead) => [0.42, 0.26, 1.35],
        _ => [2.3, 0.0, 1.55],
    };
    on_body(a, point.map(|v| v * a.look.scale()), vpm)
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
    // Easing into whatever pose it took up last frame
    let pose = boss.target_pose();
    boss.posed = if pose == boss.posed.0 { (pose, boss.posed.1, boss.posed.2 + dt) } else { (pose, boss.posed.0, 0.0) };
    let forward = Vector3::new(a.heading.cos(), a.heading.sin(), 0.0);
    let distance = player.map_or(f64::MAX, |p| (p - a.position).xy().norm() / vpm);
    let from_home = (a.position - boss.home).xy().norm() / vpm;

    // Turning on the player when they come near, not through the floor of a dungeon, and giving
    // up if they get away. Monsters don't go as far.
    let monster = look.is_monster();
    let (notice, give_up, leash) = match look {
        Look::Wolf => (22.0, 34.0, 50.0),
        Look::Spiderling => (14.0, 24.0, 32.0),
        _ if monster => (MONSTER_NOTICE, MONSTER_GIVE_UP, MONSTER_LEASH),
        _ => (NOTICE, GIVE_UP, LEASH),
    };
    let level = player.is_some_and(|p| (p.z - a.position.z).abs() < 5.0 * vpm);
    if level && distance < notice {
        boss.engaged = true;
    }
    if boss.engaged && (player.is_none() || distance > give_up || from_home > leash) {
        boss.calm();
    }
    let busy = matches!(boss.phase, Phase::WindUp(..) | Phase::Act(..));
    if boss.engaged && boss.phase == Phase::Roam {
        // A monster gives a snarl or a rattle and comes straight on
        boss.phase = if monster { Phase::Chase } else { Phase::Act(Move::Roar, 0.0) };
        boss.deeds.push(Deed::Roar { at: a.position, angry: false });
    } else if boss.engaged && !boss.enraged && !monster && vitality < 0.5 && !busy {
        boss.enraged = true;
        boss.phase = Phase::Act(Move::Roar, 0.0);
        boss.deeds.push(Deed::Roar { at: a.position, angry: true });
    }
    // Angry, it's quicker about everything
    let quick: f64 = if boss.enraged { 0.75 } else { 1.0 };

    // The heavy ones shake the ground as they walk
    let heavy = matches!(look.build(), Some(Build::Golem | Build::Bear | Build::Treant)) || look == Look::GraveKnight;
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
                    // Monsters come right up to the player
                    let close = closeness(look).0;
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
            // Showing how far its quake will reach
            if m == Move::Quake {
                boss.deeds.push(Deed::Mark { at: a.position, radius: QUAKE_REACH, closing: 0.0 });
            }
            let t = t + dt;
            if t < windup(look, m) * quick.max(0.8) {
                Phase::WindUp(m, t)
            } else {
                // Now it does it
                boss.cooldowns[m as usize] = cooldown(look, m) * quick;
                match m {
                    Move::Slam => {
                        let (reach, radius, damage, crater) = slam(look);
                        boss.deeds.push(Deed::Slam { at: a.position + forward * reach * vpm, radius, damage, crater });
                    }
                    // The dead rising, a pack answering a howl, spiderlings swarming out
                    Move::Summon => {
                        if let Some(brood) = look.brood() {
                            let count = if brood == Look::Spiderling { 3 } else { 2 };
                            boss.deeds.push(Deed::Summon { at: a.position, look: brood, count });
                        }
                    }
                    Move::Throw => boss.deeds.push(Deed::Throw { from: on_body(a, [-0.55, -0.55, 4.05], vpm), to: boss.aim }),
                    Move::Spit => boss.deeds.push(Deed::Spit { from: mouth(a, vpm), to: boss.aim + Vector3::new(0.0, 0.0, 1.0 * vpm) }),
                    Move::Charge => {
                        boss.rammed = false;
                        a.target_heading = heading_to(a.position, boss.aim);
                        a.heading = a.target_heading;
                    }
                    Move::Leap => {
                        // Coming down just short of the player, not on top of them; a monster
                        // right in front of them
                        let off = boss.aim - a.position;
                        let short = Vector3::new(off.x, off.y, 0.0).try_normalize(1e-6).unwrap_or(forward) * closeness(look).1 * vpm;
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
                    // The shockwave runs out from where it stands as it pounds the ground
                    Move::Quake => {
                        boss.leap.0 = a.position;
                        boss.deeds.push(Deed::Stomp(a.position));
                    }
                    Move::Maul => boss.deeds.push(Deed::Stomp(a.position)),
                    // Web, then a pounce on whoever's stuck in it
                    Move::Web => {
                        boss.deeds.push(Deed::Web { from: mouth(a, vpm), to: boss.aim });
                        boss.cooldowns[Move::Leap as usize] = boss.cooldowns[Move::Leap as usize].min(1.2);
                    }
                    // Angry, it sends three lines fanned out
                    Move::RootWave => boss.deeds.push(Deed::RootWave { from: a.position, to: boss.aim, lanes: if boss.enraged { 3 } else { 1 } }),
                    Move::Breath | Move::Roar | Move::Storm => {}
                }
                Phase::Act(m, 0.0)
            }
        }
        Phase::Act(m, t) => {
            let t = t + dt;
            a.mood = Mood::Idle;
            let mut over = t >= duration(look, m);
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
                    let (time, high, radius, damage) = leap(look);
                    let f = (t / time).min(1.0);
                    a.position = from.lerp(&to, f) + Vector3::new(0.0, 0.0, high * vpm * (PI * f).sin());
                    a.speed = 0.0;
                    boss.deeds.push(Deed::Mark { at: to, radius, closing: 1.0 - f });
                    if over {
                        a.position = to;
                        boss.deeds.push(Deed::Slam { at: to, radius, damage, crater: if monster { 0.0 } else { 0.6 } });
                        boss.deeds.push(Deed::Stomp(to));
                    }
                }
                // The shockwave running out across the ground
                Move::Quake => {
                    let to = (t * QUAKE_PACE).min(QUAKE_REACH);
                    boss.deeds.push(Deed::Quake { at: boss.leap.0, from: (t - dt) * QUAKE_PACE, to, dt });
                }
                // Breathing up into the sky, then its element raining down round the player
                Move::Storm => {
                    if t < 0.5 {
                        let from = mouth(a, vpm);
                        boss.deeds.push(Deed::Breath { from, direction: (forward * 0.3 + Vector3::z()).normalize(), dt });
                    }
                    let count = |t: f64| if t < STORM_START { None } else { Some(((t - STORM_START) / STORM_EVERY) as usize) };
                    if let Some(k) = count(t).filter(|&k| count(t - dt) != Some(k)) {
                        boss.deeds.push(Deed::Storm { to: player.unwrap_or(boss.aim), k });
                    }
                }
                // Lunging and swiping three times, turning only a little after the player between
                Move::Maul => {
                    let (k, local) = ((t / MAUL_SWIPE) as usize, t % MAUL_SWIPE);
                    if t - dt <= 0.0 || k != ((t - dt) / MAUL_SWIPE) as usize {
                        let turn = if look.element() == Some(Element::Frost) { 0.45 } else { 0.7 };
                        if let Some(p) = player {
                            let off = (heading_to(a.position, p) - a.heading + PI).rem_euclid(TAU) - PI;
                            a.heading += off.clamp(-turn, turn);
                        }
                        boss.deeds.push(Deed::Swipe(a.position, k));
                        boss.deeds.push(Deed::Stomp(a.position));
                    }
                    a.target_heading = a.heading;
                    let forward = Vector3::new(a.heading.cos(), a.heading.sin(), 0.0);
                    let radius = if k >= 2 { 2.6 } else { 1.8 };
                    if local < MAUL_LUNGE {
                        // Up to the player, not through them
                        a.mood = if distance > look.reach() * 0.75 { Mood::Run } else { Mood::Idle };
                        let ahead = 2.2 + a.speed * (MAUL_LUNGE - local);
                        boss.deeds.push(Deed::Mark { at: a.position + forward * ahead * vpm, radius, closing: 1.0 - local / MAUL_LUNGE });
                    } else if local - dt < MAUL_LUNGE {
                        let last = if look.element() == Some(Element::Frost) { 20.0 } else { 26.0 };
                        let (damage, crater) = if k >= 2 { (last, 0.7) } else { (12.0, 0.0) };
                        boss.deeds.push(Deed::Slam { at: a.position + forward * 2.2 * vpm, radius, damage, crater });
                    }
                }
                _ => {}
            }
            if over { Phase::Recover(recovery(look, m) * quick) } else { Phase::Act(m, t) }
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
