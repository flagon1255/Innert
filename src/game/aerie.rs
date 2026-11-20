//! What avians and gargoyles do to the player and round them, see src\avians.rs: talons and claws
//! striking, feathers flung like knives, lightning called out of the sky and spat from a beak,
//! gales thrown at the player, the roc carrying them off and letting them fall, shards of stone
//! bursting out where a gargoyle comes down, and stone cracking off them as they wake. Everything
//! thrown is drawn the whole way, big enough to be followed by eye however far off it is.

use super::*;
use crate::avians::{self, Act, FX_BOLT, FX_FORK, Move, fx_model, fx_shape};
use crate::gargoyles;

/// A feather flung at the player: where it is and how fast it's going in voxels a second, how many
/// seconds it's flown, how hard it hits, which feather it is, and seconds it's been stuck in the
/// ground once it's come down
struct Feather {
    at: Vector3<f64>,
    velocity: Vector3<f64>,
    age: f64,
    damage: f64,
    look: Look,
    fx: usize,
    stuck: Option<f64>,
}

/// Lightning: from where to where, seconds since it was called, when it strikes and how far round
/// it hurts, and whether it has
struct Bolt {
    from: Vector3<f64>,
    to: Vector3<f64>,
    age: f64,
    delay: f64,
    radius: f64,
    damage: f64,
    look: Look,
    struck: bool,
    /// Spat from a beak, along its line, rather than down out of the sky on a spot
    arc: bool,
    seed: u32,
}

/// A shard of stone burst out of where a gargoyle came down, flying, then lying a moment
struct Shard {
    at: Vector3<f64>,
    velocity: Vector3<f64>,
    turn: UnitQuaternion<f64>,
    spin: Vector3<f64>,
    age: f64,
    damage: f64,
    look: Look,
    landed: Option<f64>,
}

/// How many of each there can be at once
const MOST_FEATHERS: usize = 48;
const MOST_BOLTS: usize = 16;
const MOST_SHARDS: usize = 36;
const MOST_SPARKS: usize = 16;
/// How far round the player's middle a feather or a shard catches them, in meters
const PLAYER_GIRTH: f64 = 0.5;
/// How long lightning's seen once it's struck, in seconds
const BOLT_SEEN: f64 = 0.3;
/// How long a feather stays stuck in the ground, and a shard lies there, in seconds
const STUCK_SECONDS: f64 = 1.6;

#[derive(Default)]
pub struct Sky {
    feathers: Vec<Feather>,
    bolts: Vec<Bolt>,
    shards: Vec<Shard>,
    /// Crackles of lightning at a beak, where and how long ago
    sparks: Vec<(Vector3<f64>, f64, u32)>,
    /// Where the roc's carrying the player to this frame, in voxels, and how fast to get them there
    carry: Option<Vector3<f64>>,
    haul: Option<Vector3<f64>>,
    /// Let fall from on high: how high the player was, and seconds since
    dropped: Option<(f64, f64)>,
    beat_wait: f64,
    rng: u32,
}

impl Sky {
    pub fn clear(&mut self) {
        let rng = self.rng;
        *self = Sky { rng, ..Default::default() };
    }

    pub fn recenter(&mut self, shift: Vector3<f64>) {
        for f in &mut self.feathers {
            f.at -= shift;
        }
        for b in &mut self.bolts {
            b.from -= shift;
            b.to -= shift;
        }
        for s in &mut self.shards {
            s.at -= shift;
        }
        for s in &mut self.sparks {
            s.0 -= shift;
        }
        self.carry = self.carry.map(|c| c - shift);
        self.dropped = self.dropped.map(|(z, t)| (z - shift.z, t));
    }

    fn random(&mut self) -> f64 {
        self.rng = hash(self.rng.wrapping_add(0x9e37_79b9));
        (self.rng >> 8) as f64 / 16777216.0
    }

    fn any_way(&mut self) -> Vector3<f64> {
        loop {
            let v = Vector3::new(self.random() * 2.0 - 1.0, self.random() * 2.0 - 1.0, self.random() * 2.0 - 1.0);
            if v.norm_squared() <= 1.0 && v.norm_squared() > 1e-4 {
                return v;
            }
        }
    }
}

/// How much bigger something's drawn the further off it is past 15 meters, so it's never lost to
/// the eye, as shots are
fn far_scale(at: Vector3<f64>, view: &View) -> f64 {
    ((at - view.eye).norm() / view.vpm / 15.0).max(1.0).sqrt()
}

/// The materials of the stone a gargoyle's made of, for the flakes cracking off it
fn flakes(look: Look) -> [u8; 3] {
    match look {
        Look::BloodGargoyle | Look::BloodwakeGargoyle => [TEMPLE_RED, OBSIDIAN, TEMPLE_RED],
        Look::RimeGargoyle => [LIMESTONE, ICE, SNOW],
        Look::CinderGargoyle => [OBSIDIAN, BARK_DARK, OBSIDIAN],
        Look::MossGargoyle => [TEMPLE_TEAL, MOSS, BRONZE],
        Look::Grotesque => [BRICK_STONE, MORTAR, LIMESTONE],
        _ => [STONE, ROCK, LIMESTONE],
    }
}

/// The distance from `p` to the segment from `a` to `b`
fn off_segment(p: Vector3<f64>, a: Vector3<f64>, b: Vector3<f64>) -> f64 {
    let ab = b - a;
    let t = ((p - a).dot(&ab) / ab.norm_squared().max(1e-9)).clamp(0.0, 1.0);
    (p - (a + ab * t)).norm()
}

impl Game {
    /// What one of them did, see avians::Act
    pub(super) fn aerie_act(&mut self, look: Look, act: Act, world: &World, player: Option<Vector3<f64>>, view: &View) {
        let vpm = world.voxels_per_meter as f64;
        let felt = |at: Vector3<f64>| (1.0 - (at - view.eye).norm() / vpm / 60.0).max(0.0);
        let size = avians::size(look);
        let stone = gargoyles::is_gargoyle(look);
        let harpy = matches!(look, Look::Harpy | Look::Harridan | Look::HarpyQueen);
        // Its voice: higher the smaller it is
        let voice = (1.5 / size.powf(0.35)) as f32;
        let cry = |at: Vector3<f64>, loud: f32| if stone { Cue::at(Sound::Screech, at, loud).pitched(voice * 0.55) } else if harpy { Cue::at(Sound::Wail, at, loud).pitched(voice * 1.25) } else { Cue::at(Sound::Screech, at, loud).pitched(voice * 0.8) };
        match act {
            Act::Warn { m, at } => {
                let cue = match m {
                    Move::Rake | Move::Snatch => {
                        self.cues.push(Cue::at(Sound::Swoop, at, 0.6).pitched(voice * 0.7));
                        cry(at, 0.8)
                    }
                    Move::Stoop => Cue::at(Sound::Swoop, at, 0.9).pitched(voice * 0.5),
                    Move::Claw | Move::Peck => {
                        if stone {
                            Cue::at(Sound::Growl, at, 0.7).pitched(voice * 0.6)
                        } else {
                            cry(at, 0.5)
                        }
                    }
                    Move::Pounce => Cue::at(Sound::Growl, at, 0.8).pitched(voice * 0.5),
                    Move::Gust => Cue::at(Sound::Flap, at, 1.0).pitched(0.6 * voice),
                    Move::Volley => {
                        self.cues.push(Cue::at(Sound::Flap, at, 0.8).pitched(1.2 * voice));
                        Cue::at(Sound::Creak, at, 0.6).pitched(1.6)
                    }
                    Move::Lightning => {
                        self.cues.push(Cue::at(Sound::Thunder, at, 0.8).pitched(0.7));
                        Cue::at(Sound::Cast(Element::look(Some(Element::Storm)) as u8), at, 0.9).pitched(0.4)
                    }
                    Move::Arc => Cue::at(Sound::Cast(Element::look(Some(Element::Storm)) as u8), at, 0.9).pitched(0.6 * voice),
                    Move::Shatter => {
                        self.cues.push(Cue::at(Sound::Rumble, at, 0.9).pitched(0.6));
                        Cue::at(Sound::Growl, at, 1.0).pitched(voice * 0.4)
                    }
                    Move::Shriek | Move::Harden => return,
                };
                self.cues.push(cue);
            }
            Act::Shriek { at, brood, first, wake } => {
                self.cues.push(cry(at, 1.0));
                if stone {
                    self.cues.push(Cue::at(Sound::Growl, at, 0.8).pitched(voice * 0.4));
                }
                if !avians::is_monster(look) {
                    self.shake = self.shake.max(0.4 * felt(at));
                    if first {
                        self.say(format!("The {} turns on you!", look.name()));
                    }
                }
                if let Some((kind, count)) = brood {
                    self.call_flock(look, kind, count, at, world, player);
                }
                if wake {
                    self.wake_gargoyles(at, 45.0 * vpm);
                }
            }
            Act::Strike { m, at, radius, damage } => {
                let cue = match m {
                    Move::Peck => Cue::at(Sound::Bite, at, 0.9).pitched(voice * 0.8),
                    Move::Rake => {
                        self.cues.push(Cue::at(Sound::Swoop, at, 0.8).pitched(voice * 0.8));
                        Cue::at(Sound::Claw, at, 0.9).pitched(voice * 0.8)
                    }
                    _ => Cue::at(Sound::Claw, at, 0.9).pitched(voice * if stone { 0.6 } else { 0.9 }),
                };
                self.cues.push(cue);
                self.blows.push(Blow { at, radius, damage, look });
                if let Some(feet) = player {
                    let off = feet - at;
                    let d = off.xy().norm() / vpm;
                    if d < radius && off.z.abs() < 2.5 * vpm * size.sqrt() && open_between(world, at, feet) {
                        self.hurt(damage * (1.0 - 0.4 * d / radius), at, feet, world, look.name());
                        self.afflict(look.element());
                        let away = Vector3::new(off.x, off.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros);
                        self.thrown += (away * 2.0 + Vector3::new(0.0, 0.0, 1.2)) * vpm;
                    } else if d < radius + 1.2 && off.z.abs() < 3.0 * vpm {
                        self.near_miss(at);
                    }
                }
            }
            Act::Gust { at, dir, reach, push, damage } => {
                self.cues.push(Cue::at(Sound::Gale, at, 1.0).pitched(0.8 * voice));
                self.cues.push(Cue::at(Sound::Flap, at, 1.0).pitched(0.55 * voice));
                self.shake = self.shake.max(0.25 * felt(at));
                for k in 1..5 {
                    let p = at + dir * reach * (k as f64 / 5.0) * vpm;
                    self.combat.stir(p, 0.6 + 0.4 * k as f64, 0.25, DROP_MUD);
                }
                if let Some(feet) = player {
                    let off = feet - at;
                    let d = off.xy().norm() / vpm;
                    let ahead = off.xy().try_normalize(1e-6).is_some_and(|o| o.dot(&dir.xy()) > 0.5);
                    if d < reach && ahead && off.z.abs() < 5.0 * vpm {
                        self.hurt(damage, at, feet, world, look.name());
                        // The queen's gales lift the player off their feet, to be thrown the further
                        let lift = if look == Look::HarpyQueen { 4.5 } else { 2.5 };
                        self.thrown += (dir * push + Vector3::new(0.0, 0.0, lift)) * vpm;
                    }
                }
            }
            Act::Feathers { from, to, count, spread, speed, damage } => {
                let off = to - from;
                let side = Vector3::new(-off.y, off.x, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                let time = (off.norm() / vpm / speed).max(0.15);
                for k in 0..count as usize {
                    let across = if count > 1 { (k as f64 / (count - 1) as f64 - 0.5) * 2.0 } else { 0.0 };
                    let lift = (self.aerie.random() - 0.5) * 0.6;
                    let aim = to + side * across * spread * vpm + Vector3::new(0.0, 0.0, lift * vpm);
                    // Aimed a little high, as they drop on the way
                    let velocity = (aim - from) / time + Vector3::new(0.0, 0.0, 0.5 * 2.0 * vpm * time);
                    self.aerie.feathers.push(Feather { at: from, velocity, age: 0.0, damage, look, fx: avians::feather_of(look), stuck: None });
                }
                while self.aerie.feathers.len() > MOST_FEATHERS {
                    self.aerie.feathers.remove(0);
                }
                self.cues.push(Cue::at(Sound::Volley, from, 0.8).pitched(1.4));
                self.cues.push(Cue::at(Sound::Swoop, from, 0.6).pitched(1.6));
            }
            Act::Lightning { to, radius, damage, delay } => {
                let a = self.aerie.random() * TAU;
                let from = to + Vector3::new(a.cos() * 5.0, a.sin() * 5.0, 38.0) * vpm;
                let seed = hash(self.aerie.rng ^ 0x7b01);
                self.aerie.bolts.push(Bolt { from, to, age: 0.0, delay, radius, damage, look, struck: false, arc: false, seed });
                while self.aerie.bolts.len() > MOST_BOLTS {
                    self.aerie.bolts.remove(0);
                }
            }
            Act::Arc { from, to, damage } => {
                let seed = hash(self.aerie.rng ^ 0xa4c0);
                self.aerie.rng = seed;
                // On past where it was aimed, a little, as lightning goes
                let far = from + (to - from) * 1.15;
                self.aerie.bolts.push(Bolt { from, to: far, age: 0.0, delay: 0.0, radius: 1.1, damage, look, struck: false, arc: true, seed });
                while self.aerie.bolts.len() > MOST_BOLTS {
                    self.aerie.bolts.remove(0);
                }
            }
            Act::Impact { at, radius, damage } => {
                self.blows.push(Blow { at, radius, damage, look });
                self.combat.craters.push((at, (radius * 0.18).min(1.2)));
                self.combat.stir(at, radius, 0.6, DROP_MUD);
                self.cues.push(Cue::at(Sound::Quake, at, 1.0).pitched((0.9 / size.powf(0.2)) as f32));
                self.cues.push(Cue::at(Sound::Burst, at, 0.9).pitched(0.55));
                self.shake = self.shake.max((0.3 + 0.15 * size).min(1.0) * felt(at));
                if stone {
                    self.leaves.debris(at, &flakes(look), vpm);
                }
                if let Some(feet) = player {
                    let off = feet - at;
                    let d = off.xy().norm() / vpm;
                    if d < radius && off.z.abs() < 3.0 * vpm && open_between(world, at, feet) {
                        self.hurt(damage * (1.0 - 0.5 * d / radius), at, feet, world, look.name());
                        self.afflict(look.element());
                        let away = Vector3::new(off.x, off.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros);
                        self.thrown += (away * 3.0 + Vector3::new(0.0, 0.0, 3.0)) * vpm;
                    } else if d < radius + 2.0 {
                        self.near_miss(at);
                    }
                }
            }
            Act::Shards { at, count, damage } => {
                for k in 0..count as usize {
                    let a = k as f64 / count as f64 * TAU + self.aerie.random() * 0.4;
                    let out = 11.0 + 6.0 * self.aerie.random();
                    let velocity = Vector3::new(a.cos() * out, a.sin() * out, 2.5 + 3.0 * self.aerie.random()) * vpm;
                    let spin = self.aerie.any_way() * 12.0;
                    let turn = UnitQuaternion::rotation_between(&Vector3::x(), &velocity).unwrap_or_else(UnitQuaternion::identity);
                    self.aerie.shards.push(Shard { at: at + Vector3::new(a.cos(), a.sin(), 0.6) * vpm, velocity, turn, spin, age: 0.0, damage, look, landed: None });
                }
                while self.aerie.shards.len() > MOST_SHARDS {
                    self.aerie.shards.remove(0);
                }
                self.cues.push(Cue::at(Sound::Crack, at, 1.0).pitched(0.6));
                self.leaves.debris(at, &flakes(look), vpm);
            }
            Act::Grab { at, damage } => {
                if let Some(feet) = player {
                    self.hurt(damage, at, feet, world, look.name());
                }
                self.aerie.carry = Some(at);
                self.cues.push(cry(at, 1.0));
                self.cues.push(Cue::at(Sound::Hit, at, 0.9).pitched(0.6));
                self.say(format!("The {} seizes you in its talons! Strike at it to be let go.", look.name()));
            }
            Act::Carry { to } => self.aerie.carry = Some(to),
            Act::Drop { at } => {
                self.aerie.carry = None;
                self.aerie.dropped = player.map(|f| (f.z, 0.0));
                self.cues.push(cry(at, 0.9));
                self.say(format!("The {} lets you fall!", look.name()));
            }
            Act::TakeOff { at, size } | Act::Land { at, size } | Act::Crash { at, size } => {
                let crash = matches!(act, Act::Crash { .. });
                self.combat.stir(at, (1.0 * size).min(8.0), 0.3 + 0.08 * size, DROP_MUD);
                self.cues.push(Cue::at(Sound::Flap, at, 0.8).pitched(1.1 / size.powf(0.4) as f32));
                if !matches!(act, Act::TakeOff { .. }) {
                    self.cues.push(Cue::at(Sound::Step(Ground::Stone), at, 0.9).pitched((0.7 / size.powf(0.3)) as f32));
                    self.shake = self.shake.max((0.1 * size.sqrt()).min(0.7) * felt(at));
                }
                if crash && size > 2.0 {
                    self.cues.push(Cue::at(Sound::Burst, at, 0.9).pitched(0.5));
                    self.combat.craters.push((at, (0.5 * size.sqrt()).min(1.4)));
                }
            }
            Act::Wingbeat { at, size } => {
                if self.aerie.beat_wait <= 0.0 && (at - view.eye).norm() < 60.0 * vpm * size.sqrt() {
                    self.aerie.beat_wait = 0.2;
                    let near = felt(at).max(0.15) as f32;
                    self.cues.push(Cue::at(Sound::Flap, at, (0.3 + 0.15 * size.sqrt()).min(1.0) as f32 * near.sqrt()).pitched(1.2 / size.powf(0.35) as f32));
                }
            }
            Act::Spark { at } => {
                let seed = hash(self.aerie.rng ^ 0x5a1);
                self.aerie.rng = seed;
                self.aerie.sparks.push((at, 0.0, seed));
                while self.aerie.sparks.len() > MOST_SPARKS {
                    self.aerie.sparks.remove(0);
                }
                if seed % 4 == 0 {
                    self.cues.push(Cue::at(Sound::Crackle, at, 0.5).pitched(1.3));
                }
            }
            Act::Crumble { at, size } => {
                // Flakes of its stone flying off it, more the bigger it is
                let lots = (2.0 + 2.0 * size.sqrt()) as usize;
                let mut chips = Vec::new();
                for _ in 0..lots {
                    chips.extend(flakes(look));
                }
                self.leaves.debris(at, &chips, vpm);
                self.combat.stir(at - Vector3::new(0.0, 0.0, 0.5 * size * vpm), 0.5 * size.sqrt(), 0.3, DROP_MUD);
                self.cues.push(Cue::at(Sound::Crack, at, 0.9).pitched((1.2 / size.powf(0.3)) as f32));
                self.cues.push(Cue::at(Sound::Mine, at, 0.6).pitched(0.8));
                self.shake = self.shake.max(0.15 * size.sqrt().min(2.0) * felt(at));
            }
            Act::Petrify { at, size } => {
                self.cues.push(Cue::at(Sound::Creak, at, 0.9).pitched((0.45 / size.powf(0.2)) as f32));
                self.cues.push(Cue::at(Sound::Rumble, at, 0.6).pitched(0.9));
            }
            Act::Death { at } => {
                self.cues.push(cry(at, 1.0));
                if stone {
                    self.cues.push(Cue::at(Sound::Crack, at, 1.0).pitched(0.5));
                    self.cues.push(Cue::at(Sound::Rumble, at, 0.8).pitched(0.7));
                }
                self.aerie.carry = None;
            }
        }
    }

    /// One of its kind called down out of the sky to a shriek at `at`: never more than a few
    /// about at once
    fn call_flock(&mut self, look: Look, kind: Look, count: usize, at: Vector3<f64>, world: &World, player: Option<Vector3<f64>>) {
        let vpm = world.voxels_per_meter as f64;
        let most = if avians::is_raid(look) { 5 } else { 3 };
        let about = self.animals.iter().filter(|a| a.look == kind && a.health > 0.0 && (a.position - at).norm() < 120.0 * vpm).count();
        let count = count.min(most - about.min(most));
        if count == 0 {
            return;
        }
        let pack = self.animals.iter().find(|a| a.look == look).and_then(|a| a.boss.as_ref()).map_or(0, |b| b.pack);
        let pack = if pack == 0 { animals::new_pack() } else { pack };
        let toward = player.unwrap_or(at);
        let [sx, sy, sz] = world.size.map(|n| n as f64);
        for k in 0..count {
            self.rng = hash(self.rng);
            let a = random(&mut self.rng) * TAU;
            let r = (40.0 + 12.0 * random(&mut self.rng)) * vpm;
            let mut from = toward + Vector3::new(a.cos() * r, a.sin() * r, (18.0 + 3.0 * k as f64) * vpm);
            from.x = from.x.clamp(4.0 * vpm, sx - 4.0 * vpm);
            from.y = from.y.clamp(4.0 * vpm, sy - 4.0 * vpm);
            from.z = from.z.min(sz - 6.0 * vpm);
            self.animals.push(avians::called(kind, from, toward, pack, vpm, self.rng));
        }
        let said = match kind {
            Look::Harpy => "calls her daughters down out of the sky",
            Look::Skyhunter => "screams, and skyhunters stoop out of the clouds",
            _ => "shrieks, and its kind come to it on stone wings",
        };
        self.say(format!("The {} {said}!", look.name()));
    }

    /// Every gargoyle asleep within `within` voxels of `at` woken by a shriek
    fn wake_gargoyles(&mut self, at: Vector3<f64>, within: f64) {
        for a in self.animals.iter_mut().filter(|a| gargoyles::is_gargoyle(a.look) && a.health > 0.0 && (a.position - at).norm() < within) {
            if let Some(boss) = a.boss.as_mut().filter(|b| !b.engaged) {
                boss.provoke();
            }
        }
    }

    /// One of them going into the next phase of its fight: a shriek that shakes the ground, and
    /// word of what it's doing
    pub(super) fn aerie_phase(&mut self, look: Look, at: Vector3<f64>, stage: u8, view: &View) {
        let vpm = view.vpm;
        let felt = (1.0 - (at - view.eye).norm() / vpm / 60.0).max(0.0);
        self.cues.push(Cue::at(Sound::Screech, at, 1.0).pitched(0.7 / avians::size(look).powf(0.25) as f32));
        self.cues.push(Cue::at(if look == Look::Thunderbird { Sound::Thunder } else { Sound::Quake }, at, 0.9).pitched(0.7));
        self.shake = self.shake.max(0.8 * felt);
        let last = avians::phases(look).len() as u8;
        let said = match (look, stage) {
            (_, s) if s >= last && last > 1 => format!("The {} is enraged! Its last fury is upon you.", look.name()),
            (Look::Roc, _) => format!("The {} screams, and the clouds answer it!", look.name()),
            (Look::Thunderbird, _) => format!("The sky darkens round the {}, and the storm breaks!", look.name()),
            (Look::HarpyQueen, _) => format!("The {} shrieks for her daughters!", look.name()),
            (Look::Grotesque, _) => format!("The {} calls every gargoyle on the walls awake!", look.name()),
            _ => format!("The {}'s shriek wakes the stone round it!", look.name()),
        };
        self.say(said);
    }

    /// The velocity to carry the player along at this frame, in voxels a second, while the roc has
    /// them in its talons
    pub fn take_haul(&mut self) -> Option<Vector3<f64>> {
        self.aerie.haul.take()
    }

    /// Everything they threw and called down going on: flying, falling, striking, and hurting the
    /// player where it catches them; the player carried, or landing after being let fall
    pub(super) fn tend_aerie(&mut self, world: &World, player: Option<Vector3<f64>>, view: &View, dt: f64) {
        let vpm = world.voxels_per_meter as f64;
        let chest = player.map(|p| p + Vector3::new(0.0, 0.0, 1.0 * vpm));
        let mut sky = std::mem::take(&mut self.aerie);
        sky.beat_wait -= dt;
        // Carried by the roc: towards its talons, fast, gravity let be
        sky.haul = match (sky.carry.take(), player) {
            (Some(to), Some(feet)) if self.dead.is_none() => {
                let off = to - feet;
                let v = off * 7.0;
                let most = 32.0 * vpm;
                Some(if v.norm() > most { v.normalize() * most } else { v })
            }
            _ => None,
        };
        // Let fall: hitting the ground hard, unless the wings broke the fall
        let mut landed: Option<f64> = None;
        if let Some((from, t)) = sky.dropped.as_mut() {
            *t += dt;
            match player {
                Some(feet) if *t > 0.25 => {
                    // On the ground again: something solid just under their feet
                    let (x, y) = (feet.x.floor() as i64, feet.y.floor() as i64);
                    let down = (feet.z - 0.15 * vpm).floor() as i64;
                    if !is_passable(world.material_at(x, y, down)) {
                        let height = (*from - feet.z) / vpm;
                        let free = (2.0 * height.max(0.0) / 22.0).sqrt();
                        if height > 4.0 && *t < free * 1.8 + 0.4 {
                            landed = Some(height);
                        }
                        sky.dropped = None;
                    } else if *t > 14.0 {
                        sky.dropped = None;
                    }
                }
                None => sky.dropped = None,
                _ => {}
            }
        }
        // Feathers flying, dropping a little, sticking where they come down
        let mut struck: Vec<(f64, Vector3<f64>, Look)> = Vec::new();
        for f in &mut sky.feathers {
            if let Some(stuck) = &mut f.stuck {
                *stuck += dt;
                continue;
            }
            f.age += dt;
            f.velocity.z -= 2.0 * vpm * dt;
            let next = f.at + f.velocity * dt;
            if let Some(c) = chest
                && off_segment(c, f.at, next) < PLAYER_GIRTH * vpm
            {
                struck.push((f.damage, f.at, f.look));
                f.stuck = Some(STUCK_SECONDS);
                continue;
            }
            if !is_passable(world.material_at(next.x.floor() as i64, next.y.floor() as i64, next.z.floor() as i64)) || f.age > 9.0 {
                f.stuck = Some(0.0);
            } else {
                f.at = next;
            }
        }
        sky.feathers.retain(|f| f.stuck.is_none_or(|s| s < STUCK_SECONDS));
        // Lightning: the ring shown where it'll strike, closing in, then striking
        let mut strikes: Vec<(Vector3<f64>, Vector3<f64>, f64, f64, Look, bool)> = Vec::new();
        for b in &mut sky.bolts {
            b.age += dt;
            if !b.struck {
                if !b.arc {
                    self.combat.mark(b.to, b.radius, (1.0 - b.age / b.delay.max(1e-6)).clamp(0.0, 1.0));
                }
                if b.age >= b.delay {
                    b.struck = true;
                    strikes.push((b.from, b.to, b.radius, b.damage, b.look, b.arc));
                }
            }
        }
        sky.bolts.retain(|b| b.age < b.delay + BOLT_SEEN);
        // Shards of stone flying out, falling, lying a moment
        let mut shard_hits: Vec<(f64, Vector3<f64>, Look)> = Vec::new();
        for s in &mut sky.shards {
            if let Some(lying) = &mut s.landed {
                *lying += dt;
                continue;
            }
            s.age += dt;
            s.velocity.z -= 9.8 * vpm * dt;
            let next = s.at + s.velocity * dt;
            s.turn = UnitQuaternion::from_scaled_axis(s.spin * dt) * s.turn;
            if let Some(c) = chest
                && off_segment(c, s.at, next) < (PLAYER_GIRTH + 0.2) * vpm
            {
                shard_hits.push((s.damage, s.at, s.look));
                s.landed = Some(STUCK_SECONDS * 0.5);
                continue;
            }
            if !is_passable(world.material_at(next.x.floor() as i64, next.y.floor() as i64, next.z.floor() as i64)) || s.age > 5.0 {
                s.landed = Some(0.0);
            } else {
                s.at = next;
            }
        }
        sky.shards.retain(|s| s.landed.is_none_or(|t| t < STUCK_SECONDS));
        for s in &mut sky.sparks {
            s.1 += dt;
        }
        sky.sparks.retain(|s| s.1 < 0.22);
        self.aerie = sky;
        for (damage, from, look) in struck {
            if let Some(feet) = player {
                self.hurt(damage, from, feet, world, look.name());
                self.cues.push(Cue::at(Sound::ArrowHit, feet, 0.7).pitched(1.3));
                if look == Look::Thunderbird || look == Look::Stormtalon {
                    self.afflict(Some(Element::Storm));
                }
            }
        }
        for (damage, from, look) in shard_hits {
            if let Some(feet) = player {
                self.hurt(damage, from, feet, world, look.name());
                self.afflict(look.element());
                self.cues.push(Cue::at(Sound::Hit, feet, 0.8).pitched(0.8));
            }
        }
        for (from, to, radius, damage, look, arc) in strikes {
            let felt = (1.0 - (to - view.eye).norm() / vpm / 70.0).max(0.0);
            self.cues.push(Cue::at(Sound::Zap, to, 1.0).pitched(0.7));
            self.cues.push(Cue::at(Sound::Thunder, to, if arc { 0.6 } else { 1.0 }).pitched(0.8 + 0.2 * random(&mut self.rng) as f32));
            self.shake = self.shake.max(0.35 * felt);
            self.blows.push(Blow { at: to, radius, damage, look });
            if !arc {
                self.combat.stir(to, radius * 0.6, 0.3, DROP_MUD);
            }
            if let Some(feet) = player {
                let middle = feet + Vector3::new(0.0, 0.0, 1.0 * vpm);
                let hit = if arc {
                    off_segment(middle, from, to) < radius * vpm
                } else {
                    let off = feet - to;
                    off.xy().norm() < radius * vpm && off.z.abs() < 3.0 * vpm
                };
                if hit {
                    self.hurt(damage, to, feet, world, look.name());
                    self.afflict(Some(Element::Storm));
                } else if (middle - to).norm() < (radius + 2.0) * vpm {
                    self.near_miss(to);
                }
            }
        }
        if let (Some(height), Some(feet)) = (landed, player) {
            let damage = ((height - 4.0) * 2.6).min(60.0);
            self.hurt(damage, feet - Vector3::new(0.0, 0.0, 1.0 * vpm), feet, world, "a fall");
            self.cues.push(Cue::at(Sound::Step(Ground::Stone), feet, 1.0).pitched(0.5));
            self.shake = self.shake.max(0.6);
        }
    }

    /// What to draw of what they threw and called down, with how far each is from the eye, kept
    /// nearer than it is, as shots are, so it's drawn however much else is about
    pub(super) fn aerie_instances(&self, view: &View) -> Vec<(f64, AnimalInstance)> {
        let vpm = view.vpm;
        let sky = &self.aerie;
        let time = self.combat.time();
        let mut out = Vec::new();
        // Model `fx` with its point `point` at `at`, turned by `turn`, `size` times as big
        let place = |fx: usize, at: Vector3<f64>, point: Vector3<f64>, turn: UnitQuaternion<f64>, size: f64| {
            let (anchor, dims) = fx_shape(fx);
            let [x, y, z] = dims.map(|s| s as f64 / animals::MODEL_RES);
            let reach = (x * x * 0.25 + y * y * 0.25 + z * z).sqrt() * vpm * size;
            AnimalInstance::new(fx_model(fx), at - turn * (point - anchor) * vpm * size, turn, vpm / animals::MODEL_RES * size, reach, animals::MOVING)
        };
        let key = |at: Vector3<f64>| (at - view.eye).norm() * 0.2;
        let along = |v: Vector3<f64>| UnitQuaternion::rotation_between(&Vector3::x(), &v).unwrap_or_else(UnitQuaternion::identity);
        for f in &sky.feathers {
            let turn = if f.stuck.is_some() { along(f.velocity) } else { along(f.velocity) * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), time * 9.0) };
            let size = 1.0 * far_scale(f.at, view);
            out.push((key(f.at) * 0.5, place(f.fx, f.at, Vector3::zeros(), turn, size)));
        }
        for b in sky.bolts.iter().filter(|b| b.struck) {
            // A jagged line of it from where it came to where it struck, flickering, and a fork
            let length = (b.to - b.from).norm();
            let pieces = ((length / vpm / 5.0).ceil() as usize).clamp(2, 9);
            let flicker = (time * 40.0) as u32;
            let mut rng = b.seed ^ flicker;
            let mut last = b.from;
            let side = Vector3::new(-(b.to - b.from).y, (b.to - b.from).x, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
            for k in 1..=pieces {
                rng = hash(rng);
                let f = k as f64 / pieces as f64;
                let jag = if k == pieces { 0.0 } else { ((rng >> 8) as f64 / 16777216.0 - 0.5) * 0.18 * length };
                let up = Vector3::new(0.0, 0.0, if k == pieces { 0.0 } else { ((rng >> 4) as f64 / 268435456.0 - 0.5) * 0.06 * length });
                let next = b.from + (b.to - b.from) * f + side * jag + up;
                let seg = next - last;
                let size = (seg.norm() / vpm).max(0.5);
                out.push((key(last) * 0.4, place(FX_BOLT, (last + next) * 0.5, Vector3::zeros(), along(seg), size)));
                if k == pieces / 2 && pieces > 3 {
                    let turn = along(seg) * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), (rng % 628) as f64 * 0.01);
                    out.push((key(next) * 0.4, place(FX_FORK, next, Vector3::zeros(), turn, size * 0.7)));
                }
                last = next;
            }
        }
        for s in &sky.shards {
            out.push((key(s.at) * 0.5, place(avians::shard_of(s.look), s.at, Vector3::zeros(), s.turn, 1.2 * far_scale(s.at, view))));
        }
        for &(at, age, seed) in &sky.sparks {
            let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), (seed % 628) as f64 * 0.01) * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), ((seed >> 10) % 628) as f64 * 0.01);
            out.push((key(at), place(FX_FORK, at, Vector3::zeros(), turn, 0.6 * (1.0 - age / 0.22).max(0.2))));
        }
        out
    }

    /// The light of lightning striking and crackling: the brightest few, nearest first
    pub(super) fn aerie_lights(&self, view: &View, lights: &mut Vec<[f32; 8]>) {
        let vpm = view.vpm;
        let c = Element::Storm.light();
        let mut mine: Vec<[f32; 8]> = Vec::new();
        for b in self.aerie.bolts.iter().filter(|b| b.struck) {
            let fade = (1.0 - (b.age - b.delay) / BOLT_SEEN).clamp(0.0, 1.0) as f32;
            let p = b.to + Vector3::new(0.0, 0.0, 2.0 * vpm);
            mine.push([p.x as f32, p.y as f32, p.z as f32, (18.0 * vpm) as f32, c[0] * 4.0 * fade, c[1] * 4.0 * fade, c[2] * 4.0 * fade, 1.0]);
        }
        if let Some(&(p, _, _)) = self.aerie.sparks.last() {
            mine.push([p.x as f32, p.y as f32, p.z as f32, (4.0 * vpm) as f32, c[0] * 1.2, c[1] * 1.2, c[2] * 1.2, 1.0]);
        }
        mine.sort_by(|a, b| {
            let d = |l: &[f32; 8]| (Vector3::new(l[0] as f64, l[1] as f64, l[2] as f64) - view.eye).norm();
            d(a).total_cmp(&d(b))
        });
        lights.extend(mine.into_iter().take(2));
    }

    /// The middle of the body of the winged creature nearest `near`, in voxels, and how far its
    /// wings reach in meters, to look at it, see the "aerie" script in main.rs
    pub fn nearest_winged(&self, near: Vector3<f64>) -> Option<(Vector3<f64>, f64)> {
        let vpm = self.combat.vpm();
        self.animals
            .iter()
            .filter(|a| a.winged.is_some() && (a.health > 0.0 || avians::dying(a)))
            .min_by(|a, b| (a.position - near).norm().total_cmp(&(b.position - near).norm()))
            .map(|a| (a.position + Vector3::new(0.0, 0.0, a.look.body().0 * vpm), a.look.reach()))
    }

    /// Whether the boss being fought is a gargoyle turned to stone, for its bar, see hud()
    pub(super) fn foe_hardened(&self, foe: Look) -> bool {
        self.animals.iter().any(|a| a.look == foe && a.winged.as_ref().is_some_and(|w| w.hardened()))
    }

    /// The wings a winged boss leaves as it dies: a roc's, a harpy's, or a gargoyle's of stone
    pub(super) fn winged_spoils(&mut self, look: Look) {
        if !avians::is_winged(look) || !look.is_boss() {
            return;
        }
        let id = if look == Look::Roc {
            "roc"
        } else if gargoyles::is_gargoyle(look) {
            "gargoyle"
        } else {
            "harpy"
        };
        if let Some(k) = wings::by_id(id) {
            let name = wings::KINDS[k as usize].name;
            self.find_once(Item::Wings(k), &format!("From the {} you take the {name}!", look.name()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How wide something has to look to the eye to be seen, in radians, as for shots
    const SEEN_ANGLE: f64 = 0.004;

    fn open_ground(meters: u32, vpm: u32) -> World {
        let size = [meters * vpm, meters * vpm, 64 * vpm];
        let (tx, ty) = (size[0] as usize / 4, size[1] as usize / 4);
        let mut occupancy = vec![0u128; tx * ty * (size[2] as usize / 8)];
        occupancy[..tx * ty * (4 * vpm as usize / 8)].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, STONE);
        world.voxels_per_meter = vpm;
        world
    }

    /// Feathers flung, lightning called and spat, and shards of stone are drawn every frame from
    /// near or far, big enough to be followed by eye, and never gone in mid-air: a feather's in
    /// the air, stuck in the ground or in the player
    #[test]
    fn everything_thrown_can_be_followed() {
        let vpm = 4u32;
        let v = vpm as f64;
        let world = open_ground(200, vpm);
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let feet = Vector3::new(100.0, 40.0, 4.0) * v;
        let eye = feet + Vector3::new(0.0, 0.0, 1.6 * v);
        let view = View { eye, rotation: UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 0.15), tan: [1.2445, 0.7002], vpm: v, speed: 0.0, stride: 0.0 };
        for meters in [10.0, 30.0, 60.0] {
            let mut game = Game::new(&world, &water, 3, feet);
            let from = feet + Vector3::new(0.0, meters, 8.0) * v;
            let dt = 1.0 / 60.0;
            game.aerie_act(Look::HarpyQueen, Act::Feathers { from, to: eye, count: 7, spread: 3.0, speed: 24.0, damage: 1.0 }, &world, None, &view);
            game.aerie_act(Look::BloodwakeGargoyle, Act::Shards { at: feet + Vector3::new(0.0, meters, 0.0) * v, count: 9, damage: 1.0 }, &world, None, &view);
            game.aerie_act(Look::Thunderbird, Act::Lightning { to: feet + Vector3::new(4.0, meters, 0.0) * v, radius: 2.0, damage: 1.0, delay: 0.1 }, &world, None, &view);
            let mut seen_bolt = false;
            for _ in 0..30 {
                game.tend_aerie(&world, None, &view, dt);
                let drawn = game.aerie_instances(&view);
                assert!(!drawn.is_empty(), "nothing drawn {meters} m off");
                seen_bolt |= !game.aerie.bolts.is_empty() && game.aerie.bolts.iter().any(|b| b.struck);
                for (_, instance) in drawn {
                    let (at, reach) = instance.sphere();
                    let d = (at - eye).norm();
                    if d > 2.0 * v {
                        assert!(reach / d > SEEN_ANGLE, "something {:.1} m off only {:.4} rad across", d / v, reach / d);
                    }
                }
                assert_eq!(game.aerie.feathers.len(), 7, "a feather vanished {meters} m off");
            }
            assert!(seen_bolt, "the lightning was never seen striking");
        }
    }

    /// The roc carrying the player hauls them towards its talons, and once let fall from high
    /// they're hurt landing
    #[test]
    fn carried_off_and_let_fall() {
        let vpm = 4u32;
        let v = vpm as f64;
        let world = open_ground(120, vpm);
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let feet = Vector3::new(60.0, 60.0, 4.0) * v;
        let view = View { eye: feet, rotation: UnitQuaternion::identity(), tan: [1.2, 0.7], vpm: v, speed: 0.0, stride: 0.0 };
        let mut game = Game::new(&world, &water, 3, feet);
        game.aerie_act(Look::Roc, Act::Carry { to: feet + Vector3::new(0.0, 0.0, 10.0 * v) }, &world, Some(feet), &view);
        game.tend_aerie(&world, Some(feet), &view, 1.0 / 60.0);
        let haul = game.take_haul().expect("hauled");
        assert!(haul.z > 0.0, "{haul:?}");
        let high = feet + Vector3::new(0.0, 0.0, 18.0 * v);
        game.aerie_act(Look::Roc, Act::Drop { at: high }, &world, Some(high), &view);
        let before = game.health;
        game.tend_aerie(&world, Some(high), &view, 0.3);
        game.tend_aerie(&world, Some(feet), &view, 0.3);
        assert!(game.health < before, "landing from 18 m didn't hurt");
    }
}
