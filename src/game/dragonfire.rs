//! What dragons do to the player and the land round them, see src\dragons.rs: their breath
//! billowing out and setting the ground alight where it falls, the balls of their element they spit
//! and rain down, the ground cracking open where the great ones stamp, their wings throwing the
//! player back, and the drakelings they call down out of the sky. All of it's drawn big enough and
//! bright enough to be followed by eye however far off it is, against the sky or the ground.

use super::*;
use crate::dragons::{self, Act, FX_CRACK, FX_FLAME, FX_PUFF, Move, element_slot, fx_model, fx_shape};

/// A billow of breath flying out: where, how fast in voxels a second, how many seconds it's been
/// going and will go, what it is, how big it gets in meters, how much it burns a second, and
/// whether it hurts at all or is only to be seen
struct Puff {
    at: Vector3<f64>,
    velocity: Vector3<f64>,
    age: f64,
    life: f64,
    element: Element,
    size: f64,
    dps: f64,
    harmless: bool,
    seed: u32,
    landed: bool,
}

/// A ball of a dragon's element flying at the player or falling out of the sky: where it's going,
/// in how long, how hard it hits and how far round, and who sent it
struct Ball {
    at: Vector3<f64>,
    velocity: Vector3<f64>,
    age: f64,
    flight: f64,
    target: Vector3<f64>,
    element: Element,
    damage: f64,
    radius: f64,
    gravity: f64,
    look: Look,
    trail: f64,
    seed: u32,
    /// Spat at the player, rather than one of many rained down
    spat: bool,
}

/// The ground burning where breath fell or a ball burst: how far round, for how long, how much it
/// burns a second
struct Patch {
    at: Vector3<f64>,
    radius: f64,
    age: f64,
    life: f64,
    element: Element,
    dps: f64,
    seed: u32,
}

/// The ground cracking open out from where a great dragon stamped: from where, which way, how far,
/// spreading out over so many seconds then bursting, glowing a while after
struct Crack {
    from: Vector3<f64>,
    way: Vector3<f64>,
    length: f64,
    age: f64,
    spread: f64,
    life: f64,
    damage: f64,
    width: f64,
    burst: bool,
    look: Look,
    /// The ground along it, every CRACK_STEP meters, in voxels
    ground: Vec<f64>,
}

/// How long a piece of a crack's model is, in meters, and how far apart the ground along a crack's
/// worked out
const CRACK_PIECE: f64 = 3.0;
const CRACK_STEP: f64 = 1.0;
/// How many of each there can be at once
const MOST_PUFFS: usize = 40;
const MOST_BALLS: usize = 24;
const MOST_PATCHES: usize = 22;
const MOST_CRACKS: usize = 10;
/// How many billows of breath a second come out of a dragon breathing
const PUFF_RATE: f64 = 16.0;
/// How far round the player's middle a billow or a ball catches them, in meters
const PLAYER_GIRTH: f64 = 0.45;

#[derive(Default)]
pub struct Fire {
    puffs: Vec<Puff>,
    balls: Vec<Ball>,
    patches: Vec<Patch>,
    cracks: Vec<Crack>,
    /// Billows of breath owed, a fraction of one at a time
    owed: f64,
    /// What breath and burning ground's done to the player and not yet hurt them, and seconds till
    /// it does, and till a wingbeat's heard again
    dose: f64,
    dose_wait: f64,
    dose_element: Option<Element>,
    dose_by: &'static str,
    beat_wait: f64,
    /// Ground to be scorched, and little holes to be blown in it, not yet sent to the world: a few
    /// at a time, as each lot sent holds a frame up waiting on the GPU, see App::flush_edits()
    scorch: Vec<(Vector3<f64>, f64, u8, bool)>,
    holes: Vec<(Vector3<f64>, f64)>,
    edit_wait: f64,
    rng: u32,
}

/// The most a scorch mark's painted round, in meters: painting's paid for by the cube round it,
/// see World::paint_in(), so a great dragon's stamp scorching six meters round held a frame up a
/// fifth of a second
const MOST_SCORCH: f64 = 2.0;
/// How many scorch marks can wait to be painted, the oldest let go past it
const SCORCH_WAITING: usize = 8;
/// Seconds between lots of the world being scorched and holed
const EDIT_EVERY: f64 = 0.5;

fn fx_random(rng: &mut u32) -> f64 {
    *rng = hash(rng.wrapping_add(0x9e37_79b9));
    (*rng >> 8) as f64 / 16777216.0
}

impl Fire {
    pub fn clear(&mut self) {
        let rng = self.rng;
        *self = Fire { rng, ..Default::default() };
    }

    pub fn recenter(&mut self, shift: Vector3<f64>) {
        for p in &mut self.puffs {
            p.at -= shift;
        }
        for b in &mut self.balls {
            b.at -= shift;
            b.target -= shift;
        }
        for p in &mut self.patches {
            p.at -= shift;
        }
        for s in &mut self.scorch {
            s.0 -= shift;
        }
        for h in &mut self.holes {
            h.0 -= shift;
        }
        for c in &mut self.cracks {
            c.from -= shift;
            for z in &mut c.ground {
                *z -= shift.z;
            }
        }
    }

    fn any_way(&mut self) -> Vector3<f64> {
        loop {
            let v = Vector3::new(fx_random(&mut self.rng) * 2.0 - 1.0, fx_random(&mut self.rng) * 2.0 - 1.0, fx_random(&mut self.rng) * 2.0 - 1.0);
            if v.norm_squared() <= 1.0 && v.norm_squared() > 1e-4 {
                return v;
            }
        }
    }

    /// A billow going out of `from` along `velocity`, in voxels a second
    #[allow(clippy::too_many_arguments)]
    fn puff(&mut self, from: Vector3<f64>, velocity: Vector3<f64>, life: f64, element: Element, size: f64, dps: f64, harmless: bool) {
        let seed = hash(self.rng ^ self.puffs.len() as u32);
        self.rng = seed;
        self.puffs.push(Puff { at: from, velocity, age: 0.0, life, element, size, dps, harmless, seed, landed: false });
        if self.puffs.len() > MOST_PUFFS {
            // The harmless ones go first, then the oldest
            let k = self.puffs.iter().position(|p| p.harmless).unwrap_or(0);
            self.puffs.remove(k);
        }
    }

    /// The ground burning at `at`, or the patch of it there already burning longer and wider
    fn ignite(&mut self, at: Vector3<f64>, radius: f64, life: f64, element: Element, dps: f64, vpm: f64) -> bool {
        if let Some(p) = self.patches.iter_mut().find(|p| (p.at - at).norm() < (p.radius * 0.8) * vpm && p.element == element) {
            p.life = p.life.max(p.age + life * 0.8);
            p.radius = (p.radius + radius * 0.08).min(radius * 1.8);
            return false;
        }
        let seed = hash(self.rng ^ 0x1a7e);
        self.rng = seed;
        self.patches.push(Patch { at, radius, age: 0.0, life, element, dps, seed });
        if self.patches.len() > MOST_PATCHES {
            self.patches.remove(0);
        }
        true
    }

    /// The ground at `at` to be stained by `element`, `radius` meters round, before long
    fn scorch(&mut self, at: Vector3<f64>, radius: f64, element: Element, vpm: f64) {
        let (material, burn) = match element {
            Element::Fire => (BARK_DARK, true),
            Element::Frost => (SNOW, false),
            Element::Poison => (MOSS, false),
            _ => return,
        };
        let radius = radius.min(MOST_SCORCH);
        // Not where it's to be stained already
        if self.scorch.iter().any(|s| (s.0 - at).norm() < s.1 * 0.5 * vpm) {
            return;
        }
        self.scorch.push((at, radius, material, burn));
        if self.scorch.len() > SCORCH_WAITING {
            self.scorch.remove(0);
        }
    }

    /// A few of the stains and holes waiting sent to the world, every so often: one stain and all
    /// the little holes together, costing one wait on the GPU
    fn send_edits(&mut self, dt: f64, stains: &mut Vec<(Vector3<f64>, f64, u8, bool)>, craters: &mut Vec<(Vector3<f64>, f64)>) {
        self.edit_wait -= dt;
        if self.edit_wait > 0.0 || (self.scorch.is_empty() && self.holes.is_empty()) {
            return;
        }
        self.edit_wait = EDIT_EVERY;
        if !self.scorch.is_empty() {
            stains.push(self.scorch.remove(0));
        }
        craters.append(&mut self.holes);
    }
}

/// How a billow of `size` meters is drawn `age` of its `life` through: swelling quickly from small,
/// then thinning away at the end
fn puff_size(size: f64, age: f64, life: f64) -> f64 {
    let f = (age / life.max(1e-6)).clamp(0.0, 1.0);
    size * (0.3 + 0.7 * (f * 3.0).min(1.0)) * (1.0 - (f - 0.8).max(0.0) * 3.0).max(0.25)
}

/// How much bigger something's drawn the further off it is past 15 meters, so it's never lost to
/// the eye, as shots are
fn far_scale(at: Vector3<f64>, view: &View) -> f64 {
    ((at - view.eye).norm() / view.vpm / 15.0).max(1.0).sqrt()
}

/// What's burnt by each element, and what it's called doing it
fn burns(element: Element) -> &'static str {
    match element {
        Element::Frost => "frost",
        Element::Storm => "lightning",
        Element::Poison => "venom",
        _ => "flames",
    }
}

impl Game {
    /// What a dragon did, see dragons::Act
    pub(super) fn dragon_act(&mut self, look: Look, act: Act, world: &World, water: WaterLevel, player: Option<Vector3<f64>>, view: &View) {
        let vpm = world.voxels_per_meter as f64;
        let felt = |at: Vector3<f64>| (1.0 - (at - view.eye).norm() / vpm / 60.0).max(0.0);
        let element = dragons::element(look);
        let big = !dragons::is_drakeling(look);
        let size = dragons::size(look);
        // Its voice: higher the smaller it is
        let voice = (1.5 / size.powf(0.35)) as f32;
        match act {
            Act::Warn { m, at } => {
                let cast = Sound::Cast(Element::look(Some(element)) as u8);
                let cue = match m {
                    Move::Bite | Move::Hover => Cue::at(Sound::Growl, at, 0.8).pitched(voice * 0.8),
                    Move::Tail => Cue::at(Sound::HeavySwing, at, 0.8).pitched(voice * 0.5),
                    Move::Breath | Move::Strafe => {
                        self.cues.push(Cue::at(cast, at, 0.8).pitched(0.45 * voice));
                        Cue::at(Sound::Rumble, at, 0.8).pitched(voice)
                    }
                    Move::Gust => Cue::at(Sound::Flap, at, 1.0).pitched(0.6 * voice),
                    Move::Stomp | Move::Dive => Cue::at(Sound::Roar, at, 1.0).pitched(0.75 * voice),
                    Move::Swoop if big => Cue::at(Sound::Roar, at, 0.9).pitched(voice),
                    Move::Swoop => Cue::at(Sound::Screech, at, 0.8).pitched(voice * 0.6),
                    Move::Spit => Cue::at(cast, at, 0.9).pitched(0.55 * voice),
                    Move::Rain => {
                        if element == Element::Storm {
                            self.cues.push(Cue::at(Sound::Thunder, at, 0.9).pitched(0.8));
                        }
                        Cue::at(Sound::Roar, at, 1.0).pitched(0.8 * voice)
                    }
                    Move::Roar => return,
                };
                self.cues.push(cue);
            }
            Act::Roar { at, brood, first } => {
                self.cues.push(Cue::at(Sound::Roar, at, 1.0).pitched(voice * if big { 0.75 } else { 1.1 }));
                if !big {
                    self.cues.push(Cue::at(Sound::Screech, at, 0.7).pitched(voice * 0.5));
                }
                self.shake = self.shake.max(if big { 0.7 } else { 0.2 } * felt(at));
                if big && first {
                    self.say(format!("The {} turns on you!", look.name()));
                }
                if let Some((kind, count)) = brood {
                    self.call_brood(look, kind, count, at, world, player);
                }
            }
            Act::Bite { at, radius, damage } | Act::Claw { at, radius, damage } | Act::Sweep { at, radius, damage } => {
                let sweep = matches!(act, Act::Sweep { .. });
                let cue = match act {
                    Act::Bite { .. } => Cue::at(Sound::Bite, at, 0.9).pitched(voice * 0.7),
                    Act::Claw { .. } => Cue::at(Sound::Claw, at, 0.9).pitched(voice * 0.7),
                    _ => Cue::at(Sound::HeavySwing, at, 1.0).pitched(voice * 0.45),
                };
                self.cues.push(cue);
                self.blows.push(Blow { at, radius, damage, look });
                if big {
                    self.shake = self.shake.max(0.3 * felt(at));
                }
                if let Some(feet) = player {
                    let off = feet - at;
                    let d = off.xy().norm() / vpm;
                    // A tail sweeping round low passes under whoever jumps it
                    let airborne = (1..=(0.3 * vpm) as i64).all(|k| is_passable(world.material_at(feet.x.floor() as i64, feet.y.floor() as i64, feet.z.floor() as i64 - k)));
                    if d < radius && off.z.abs() < 2.5 * vpm.max(1.0) * size.sqrt() && open_between(world, at, feet) && !(sweep && airborne) {
                        self.hurt(damage * (1.0 - 0.4 * d / radius), at, feet, world, look.name());
                        let away = Vector3::new(off.x, off.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros);
                        self.thrown += (away * if sweep { 4.0 } else { 2.0 } + Vector3::new(0.0, 0.0, 1.5)) * vpm;
                    } else if d < radius + 1.2 && off.z.abs() < 3.0 * vpm {
                        self.near_miss(at);
                    }
                }
            }
            Act::Breath { from, dir, dt, dps, size: puff, speed, spread } => {
                let fire = &mut self.dragonfire;
                fire.owed += dt * PUFF_RATE * (1.0 + spread * 0.3);
                while fire.owed >= 1.0 {
                    fire.owed -= 1.0;
                    let side = Vector3::new(-dir.y, dir.x, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                    let across = (fx_random(&mut fire.rng) - 0.5) * 2.0 * spread;
                    let jitter = fire.any_way() * 0.1;
                    let way = (dir + jitter + side * across * 0.12).normalize();
                    let pace = speed * (0.85 + 0.3 * fx_random(&mut fire.rng));
                    let life = 0.85 * size.powf(0.2) * (0.9 + 0.2 * fx_random(&mut fire.rng));
                    fire.puff(from + side * across * 0.3 * vpm, way * pace * vpm, life, element, puff, dps, false);
                }
                if self.breath_sound <= 0.0 {
                    self.breath_sound = 1.6;
                    self.cues.push(Cue::at(Sound::Breath, from, 1.0).pitched(match element {
                        Element::Fire => 0.9 * voice,
                        Element::Frost => 1.2 * voice,
                        _ => voice,
                    }));
                }
            }
            Act::Fireball { from, to, speed, damage, radius, lob } => {
                let time = ((to - from).norm() / vpm / speed).max(0.25);
                let gravity = if lob { 9.8 } else { 0.0 };
                let velocity = (to - from) / time + Vector3::new(0.0, 0.0, 0.5 * gravity * vpm * time);
                let seed = hash(self.rng ^ 0xba11);
                self.rng = seed;
                self.dragonfire.balls.push(Ball { at: from, velocity, age: 0.0, flight: time, target: to, element, damage, radius, gravity, look, trail: 0.0, seed, spat: true });
                if self.dragonfire.balls.len() > MOST_BALLS {
                    self.dragonfire.balls.remove(0);
                }
                self.cues.push(Cue::at(Sound::Cast(Element::look(Some(element)) as u8), from, 1.0).pitched(0.6 * voice));
            }
            Act::Rain { to, damage, radius } => {
                let a = random(&mut self.rng) * TAU;
                let from = to + Vector3::new(a.cos() * 4.0, a.sin() * 4.0, 26.0) * vpm;
                let time = 1.3;
                let seed = hash(self.rng ^ 0x4a17);
                self.rng = seed;
                self.dragonfire.balls.push(Ball { at: from, velocity: (to - from) / time, age: 0.0, flight: time, target: to, element, damage, radius, gravity: 0.0, look, trail: 0.0, seed, spat: false });
                if self.dragonfire.balls.len() > MOST_BALLS {
                    self.dragonfire.balls.remove(0);
                }
                if random(&mut self.rng) < 0.3 {
                    self.cues.push(Cue::at(Sound::Whistle, to, 0.6).pitched(0.7 + 0.2 * random(&mut self.rng) as f32));
                }
            }
            Act::Gust { at, dir, reach, push, damage } => {
                self.cues.push(Cue::at(Sound::Gale, at, 1.0).pitched(0.8 * voice));
                self.cues.push(Cue::at(Sound::Flap, at, 1.0).pitched(0.55 * voice));
                self.shake = self.shake.max(0.25 * felt(at));
                // Dust and embers blown out before it
                for k in 1..5 {
                    let p = at + dir * reach * (k as f64 / 5.0) * vpm;
                    self.combat.stir(p, 0.6 + 0.4 * k as f64, 0.25, DROP_MUD);
                }
                if let Some(feet) = player {
                    let off = feet - at;
                    let d = off.xy().norm() / vpm;
                    let ahead = off.xy().try_normalize(1e-6).is_some_and(|o| o.dot(&dir.xy()) > 0.55);
                    if d < reach && ahead && off.z.abs() < 4.0 * vpm {
                        self.hurt(damage, at, feet, world, look.name());
                        self.thrown += (dir * push + Vector3::new(0.0, 0.0, 2.5)) * vpm;
                        self.afflict(Some(element));
                    }
                }
            }
            Act::Stomp { at, radius, damage, cracks, length, crack } => {
                self.blows.push(Blow { at, radius, damage, look });
                // Blown in now, as it comes down, the scorch round it a moment after
                self.combat.craters.push((at, (radius * 0.3).min(1.8)));
                self.dragonfire.scorch(at, radius, element, vpm);
                self.cues.push(Cue::at(Sound::Quake, at, 1.0).pitched(0.8));
                self.cues.push(Cue::at(Sound::Burst, at, 1.0).pitched(0.45));
                self.shake = self.shake.max(0.9 * felt(at));
                self.combat.stir(at, radius, 0.6, DROP_MUD);
                if let Some(feet) = player {
                    let off = feet - at;
                    let d = off.xy().norm() / vpm;
                    if d < radius && off.z.abs() < 3.0 * vpm && open_between(world, at, feet) {
                        self.hurt(damage * (1.0 - 0.5 * d / radius), at, feet, world, look.name());
                        let away = Vector3::new(off.x, off.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros);
                        self.thrown += (away * 3.0 + Vector3::new(0.0, 0.0, 3.0)) * vpm;
                        self.afflict(Some(element));
                    } else if d < radius + 2.0 {
                        self.near_miss(at);
                    }
                }
                // The ground cracking open, out from it each way it's to, glowing with what's under
                let lasts = if look == dragons::ULTRA && self.animals.iter().any(|a| a.look == look && a.boss.as_ref().is_some_and(|b| b.stage >= 1)) { 5.0 } else { 3.2 };
                for &c in cracks.iter().filter(|c| c.is_finite()) {
                    let way = Vector3::new(c.cos(), c.sin(), 0.0);
                    let from = at + way * radius * 0.8 * vpm;
                    let steps = (length / CRACK_STEP).ceil() as usize + 1;
                    let mut z = at.z;
                    let ground: Vec<f64> = (0..steps)
                        .map(|k| {
                            let p = from + way * (k as f64 * CRACK_STEP) * vpm;
                            z = ground_top_near(world, p.x, p.y, z);
                            z
                        })
                        .collect();
                    self.dragonfire.cracks.push(Crack { from, way, length, age: 0.0, spread: 0.6, life: 0.6 + lasts, damage: crack, width: size.powf(0.3), burst: false, look, ground });
                }
                while self.dragonfire.cracks.len() > MOST_CRACKS {
                    self.dragonfire.cracks.remove(0);
                }
            }
            Act::TakeOff { at, size } | Act::Land { at, size } | Act::Crash { at, size } => {
                let crash = matches!(act, Act::Crash { .. });
                self.combat.stir(at, (1.2 * size).min(9.0), 0.4 + 0.1 * size, DROP_MUD);
                self.cues.push(Cue::at(Sound::Flap, at, 0.9).pitched(0.9 / size.powf(0.4) as f32));
                if !matches!(act, Act::TakeOff { .. }) {
                    self.cues.push(Cue::at(Sound::Step(Ground::Stone), at, 1.0).pitched((0.6 / size.powf(0.3)) as f32));
                    self.shake = self.shake.max((0.15 * size.sqrt()).min(0.8) * felt(at));
                }
                if crash {
                    self.cues.push(Cue::at(Sound::Burst, at, 0.9).pitched(0.5));
                    if size > 2.0 {
                        self.combat.craters.push((at, (0.6 * size.sqrt()).min(1.6)));
                    }
                }
            }
            Act::Wingbeat { at, size } => {
                let fire = &mut self.dragonfire;
                if fire.beat_wait <= 0.0 && (at - view.eye).norm() < 70.0 * vpm * size.sqrt() {
                    fire.beat_wait = 0.22;
                    let near = felt(at).max(0.15) as f32;
                    self.cues.push(Cue::at(Sound::Flap, at, (0.35 + 0.15 * size.sqrt()).min(1.0) as f32 * near.sqrt()).pitched(1.0 / size.powf(0.35) as f32));
                    // The great ones' wings blowing up the dust when they're low
                    if size > 2.0 {
                        let ground = ground_top_near(world, at.x, at.y, at.z);
                        if at.z - ground < 9.0 * size.sqrt() * vpm {
                            self.combat.stir(Vector3::new(at.x, at.y, ground), 1.5 * size, 0.3, DROP_MUD);
                        }
                    }
                }
            }
            Act::Kindle { at, size } => {
                let fire = &mut self.dragonfire;
                let up = Vector3::new(0.0, 0.0, 1.2) + fire.any_way() * 0.4;
                fire.puff(at, up * vpm, 0.35, element, 0.35 * size.powf(0.6), 0.0, true);
            }
            Act::Death { at } => {
                self.cues.push(Cue::at(Sound::Roar, at, 1.0).pitched(voice * 0.55));
                if big {
                    self.cues.push(Cue::at(Sound::Thunder, at, 0.6).pitched(0.6));
                }
            }
        }
        let _ = water;
    }

    /// Drakelings of `kind` coming down out of the sky to a great dragon's roar at `at`: never more
    /// than a few about at once
    fn call_brood(&mut self, look: Look, kind: Look, count: usize, at: Vector3<f64>, world: &World, player: Option<Vector3<f64>>) {
        let vpm = world.voxels_per_meter as f64;
        let most = if look == dragons::ULTRA { 5 } else { 4 };
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
            let r = (48.0 + 14.0 * random(&mut self.rng)) * vpm;
            let mut from = toward + Vector3::new(a.cos() * r, a.sin() * r, (24.0 + 4.0 * k as f64) * vpm);
            from.x = from.x.clamp(4.0 * vpm, sx - 4.0 * vpm);
            from.y = from.y.clamp(4.0 * vpm, sy - 4.0 * vpm);
            from.z = from.z.min(sz - 6.0 * vpm);
            self.animals.push(dragons::called(kind, from, toward, pack, vpm, self.rng));
        }
        self.say(format!("The {} calls its brood down out of the sky!", look.name()));
    }

    /// A dragon's phase of its fight beginning: the ground shaking, and word of what it's doing
    pub(super) fn dragon_phase(&mut self, look: Look, at: Vector3<f64>, stage: u8, view: &View) {
        let vpm = view.vpm;
        let felt = (1.0 - (at - view.eye).norm() / vpm / 60.0).max(0.0);
        self.cues.push(Cue::at(Sound::Roar, at, 1.0).pitched(0.6 / dragons::size(look).powf(0.25) as f32));
        self.cues.push(Cue::at(Sound::Quake, at, 0.8).pitched(0.7));
        self.shake = self.shake.max(0.9 * felt);
        let last = dragons::phases(look).len() as u8;
        let said = match (look, stage) {
            (dragons::ULTRA, 1) => format!("The {} comes down to the earth, and the earth burns!", look.name()),
            (dragons::ULTRA, 2) => format!("The {} calls the first fire's children to it!", look.name()),
            (_, s) if s >= last => format!("The {} is enraged! Its last fury is upon you.", look.name()),
            _ => format!("The {} rises in fury!", look.name()),
        };
        self.say(said);
    }

    /// Everything dragons breathed, spat, rained down and cracked open going on: flying, falling,
    /// burning, bursting, and hurting the player where it catches them
    pub(super) fn tend_dragonfire(&mut self, world: &World, player: Option<Vector3<f64>>, view: &View, dt: f64) {
        let vpm = world.voxels_per_meter as f64;
        let chest = player.map(|p| p + Vector3::new(0.0, 0.0, 1.0 * vpm));
        let burning_long = self.animals.iter().any(|a| a.look == dragons::ULTRA && a.health > 0.0 && a.boss.as_ref().is_some_and(|b| b.stage >= 1));
        let mut fire = std::mem::take(&mut self.dragonfire);
        fire.beat_wait -= dt;
        fire.send_edits(dt, &mut self.combat.stains, &mut self.combat.craters);
        // The hottest thing on the player this frame, how hot, and what it was
        let mut worst: Option<(f64, Element, Look)> = None;
        let mut catch = |dps: f64, element: Element, look: Look| {
            if worst.is_none_or(|(w, ..)| dps > w) {
                worst = Some((dps, element, look));
            }
        };
        // Breath: flying out, swelling, falling on the ground and setting it alight
        let mut lit: Vec<(Vector3<f64>, f64, Element, f64)> = Vec::new();
        for p in &mut fire.puffs {
            p.age += dt;
            let lift = match p.element {
                Element::Fire => 0.8,
                Element::Storm => 0.0,
                _ => -2.5,
            };
            p.velocity.z += lift * vpm * dt;
            p.velocity *= (-0.6 * dt).exp();
            let next = p.at + p.velocity * dt;
            if !p.landed && !is_passable(world.material_at(next.x.floor() as i64, next.y.floor() as i64, next.z.floor() as i64)) {
                // Landing: it spreads out along the ground and sets it alight there
                p.landed = true;
                p.velocity = Vector3::new(p.velocity.x, p.velocity.y, 0.0) * 0.2;
                p.age = p.age.max(p.life - 0.3);
                if !p.harmless {
                    let ground = ground_top_near(world, p.at.x, p.at.y, p.at.z);
                    lit.push((Vector3::new(p.at.x, p.at.y, ground), p.size * 0.7, p.element, p.dps * 0.5));
                }
            } else if !p.landed {
                p.at = next;
            }
            if let Some(c) = chest
                && !p.harmless
                && (p.at - c).norm() < (puff_size(p.size, p.age, p.life) * 0.5 + PLAYER_GIRTH) * vpm
            {
                catch(p.dps, p.element, Look::Ashwing);
            }
        }
        fire.puffs.retain(|p| p.age < p.life);
        for (at, radius, element, dps) in lit {
            let life = if burning_long { 8.0 } else { 3.5 };
            if fire.ignite(at, radius.max(0.8), life, element, dps, vpm) && random(&mut self.rng) < 0.5 {
                fire.scorch(at, radius * 0.8, element, vpm);
            }
        }
        // Balls flying and falling, trailing their element, bursting where they come down
        let mut burst: Vec<(Vector3<f64>, Element, f64, f64, Look, bool)> = Vec::new();
        let mut trails: Vec<(Vector3<f64>, Element, f64)> = Vec::new();
        for b in &mut fire.balls {
            b.age += dt;
            b.velocity.z -= b.gravity * vpm * dt;
            let next = b.at + b.velocity * dt;
            b.trail -= dt;
            if b.trail <= 0.0 {
                b.trail = 0.05;
                trails.push((b.at, b.element, b.radius));
            }
            // Its landing shown as it comes
            self.combat.mark(b.target, b.radius, (1.0 - b.age / b.flight).clamp(0.0, 1.0));
            let hit_player = chest.is_some_and(|c| (b.at - c).norm() < (0.6 + PLAYER_GIRTH) * vpm);
            let hit_ground = !is_passable(world.material_at(next.x.floor() as i64, next.y.floor() as i64, next.z.floor() as i64));
            if hit_player || hit_ground || b.age >= b.flight + 1.5 {
                burst.push((b.at, b.element, b.damage, b.radius, b.look, b.spat));
                b.age = f64::MAX;
            } else {
                b.at = next;
            }
        }
        fire.balls.retain(|b| b.age < f64::MAX);
        for (at, element, radius) in trails {
            let drift = fire.any_way() * 0.6 * vpm;
            fire.puff(at, drift, 0.4, element, 0.55 * radius.powf(0.5), 0.0, true);
        }
        // The ground burning, hurting what stands in it, dying down
        for p in &mut fire.patches {
            p.age += dt;
            if let Some(feet) = player {
                let off = feet - p.at;
                if off.xy().norm() < p.radius * vpm && off.z.abs() < 1.6 * vpm {
                    catch(p.dps, p.element, Look::Ashwing);
                }
            }
        }
        fire.patches.retain(|p| p.age < p.life);
        // Cracks spreading out, bursting, glowing a while
        let mut erupted: Vec<(Vector3<f64>, Vector3<f64>, f64, Look)> = Vec::new();
        for c in &mut fire.cracks {
            c.age += dt;
            let reach = (c.age / c.spread).min(1.0) * c.length;
            let at = |m: f64| -> Vector3<f64> {
                let k = ((m / CRACK_STEP) as usize).min(c.ground.len() - 1);
                let p = c.from + c.way * m * vpm;
                Vector3::new(p.x, p.y, c.ground[k])
            };
            if !c.burst {
                // Showing where it'll burst, as it spreads
                for j in 0..3 {
                    self.combat.mark(at(c.length * (j as f64 + 0.5) / 3.0), 0.8 * c.width, 1.0 - c.age / c.spread);
                }
                if c.age >= c.spread {
                    c.burst = true;
                    erupted.push((at(0.0), at(c.length), c.damage, c.look));
                }
            }
            // Standing on what's glowing burns, after
            if c.burst
                && let Some(feet) = player
            {
                let (a, b) = (at(0.0), at(reach));
                let ab = b - a;
                let t = ((feet - a).dot(&ab) / ab.norm_squared().max(1e-9)).clamp(0.0, 1.0);
                let off = feet - (a + ab * t);
                if off.xy().norm() < 0.6 * c.width * vpm && off.z.abs() < 1.5 * vpm {
                    catch(9.0, Element::Fire, c.look);
                }
            }
        }
        fire.cracks.retain(|c| c.age < c.life);
        self.dragonfire = fire;
        for (a, b, damage, look) in erupted {
            self.cues.push(Cue::at(Sound::Crack, a, 0.9).pitched(0.6));
            self.cues.push(Cue::at(Sound::Rumble, a, 0.8).pitched(0.7));
            for k in 0..5 {
                let p = a + (b - a) * ((k as f64 + 0.5) / 5.0);
                let up = Vector3::new(0.0, 0.0, 4.0) + self.dragonfire.any_way() * 1.5;
                self.dragonfire.puff(p, up * vpm, 0.6, Element::Fire, 1.4, 0.0, true);
            }
            if let Some(feet) = player {
                let ab = b - a;
                let t = ((feet - a).dot(&ab) / ab.norm_squared().max(1e-9)).clamp(0.0, 1.0);
                let off = feet - (a + ab * t);
                if off.xy().norm() < 1.3 * vpm && off.z.abs() < 2.0 * vpm {
                    self.hurt(damage, a + ab * t, feet, world, look.name());
                    self.thrown += Vector3::new(0.0, 0.0, 4.0) * vpm;
                    self.afflict(Some(Element::Fire));
                }
            }
        }
        for (at, element, damage, radius, look, spat) in burst {
            self.ball_bursts(at, element, damage, radius, look, spat, world, player, view);
        }
        // What's caught the player burning them, a few times a second
        let fire = &mut self.dragonfire;
        fire.dose_wait -= dt;
        if let Some((dps, element, look)) = worst {
            fire.dose += dps * dt;
            fire.dose_element = Some(element);
            fire.dose_by = if look == Look::Ashwing { burns(element) } else { look.name() };
        }
        if fire.dose_wait <= 0.0 {
            fire.dose_wait = 0.25;
            let (dose, element, by) = (std::mem::take(&mut fire.dose), fire.dose_element.take(), fire.dose_by);
            if dose > 0.0
                && let Some(feet) = player
            {
                self.hurt(dose, feet + Vector3::new(0.0, 0.0, 2.0 * vpm), feet, world, by);
                self.afflict(element);
            }
        }
    }

    /// A ball of `element` bursting at `at`: hurting what's near, setting the ground alight
    #[allow(clippy::too_many_arguments)]
    fn ball_bursts(&mut self, at: Vector3<f64>, element: Element, damage: f64, radius: f64, look: Look, spat: bool, world: &World, player: Option<Vector3<f64>>, view: &View) {
        let vpm = world.voxels_per_meter as f64;
        let felt = (1.0 - (at - view.eye).norm() / vpm / 50.0).max(0.0);
        let ground = ground_top_near(world, at.x, at.y, at.z);
        let on = Vector3::new(at.x, at.y, ground.min(at.z + 1.0 * vpm));
        // A hole blown where a ball spat at the player bursts; not for each of the many it rains
        // down, each lot of holes holding a frame up, see Fire::send_edits()
        if spat {
            self.dragonfire.holes.push((on, (radius * 0.3).min(1.0)));
        }
        let life = if self.animals.iter().any(|a| a.look == dragons::ULTRA && a.boss.as_ref().is_some_and(|b| b.stage >= 1)) { 7.0 } else { 3.0 };
        if self.dragonfire.ignite(on, radius * 0.7, life, element, 9.0, vpm) && element == Element::Fire {
            self.dragonfire.scorch(on, radius * 0.8, element, vpm);
        }
        for _ in 0..9 {
            let out = self.dragonfire.any_way() + Vector3::new(0.0, 0.0, 0.6);
            self.dragonfire.puff(at, out * 3.5 * vpm, 0.5, element, 0.9 * radius.powf(0.6), 0.0, true);
        }
        self.cues.push(Cue::at(Sound::Burst, at, 1.0).pitched(0.7));
        if element == Element::Storm {
            self.cues.push(Cue::at(Sound::Zap, at, 0.8).pitched(0.8));
        }
        self.shake = self.shake.max(0.35 * felt);
        self.blows.push(Blow { at: on, radius, damage, look });
        if let Some(feet) = player {
            let off = feet + Vector3::new(0.0, 0.0, 1.0 * vpm) - at;
            let d = off.norm() / vpm;
            if d < radius + PLAYER_GIRTH {
                self.hurt(damage * (1.0 - 0.5 * (d / (radius + PLAYER_GIRTH)).min(1.0)), at, feet, world, look.name());
                self.afflict(Some(element));
            } else if d < radius + 1.5 {
                self.near_miss(at);
            }
        }
    }

    /// What to draw of what dragons breathed and left burning, with how far each is from the eye,
    /// kept nearer than it is, as shots are, so it's drawn however much else is about
    pub(super) fn dragonfire_instances(&self, view: &View) -> Vec<(f64, AnimalInstance)> {
        let vpm = view.vpm;
        let fire = &self.dragonfire;
        let time = self.combat.time();
        let mut out = Vec::new();
        let place = |fx: usize, at: Vector3<f64>, point: Vector3<f64>, turn: UnitQuaternion<f64>, size: f64| {
            let (anchor, dims) = fx_shape(fx);
            let [x, y, z] = dims.map(|s| s as f64 / animals::MODEL_RES);
            let reach = (x * x * 0.25 + y * y * 0.25 + z * z).sqrt() * vpm * size;
            AnimalInstance::new(fx_model(fx), at - turn * (point - anchor) * vpm * size, turn, vpm / animals::MODEL_RES * size, reach, animals::MOVING)
        };
        let spin = |seed: u32, rate: f64| UnitQuaternion::from_axis_angle(&Vector3::z_axis(), (seed % 628) as f64 * 0.01 + time * rate) * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), ((seed >> 10) % 314) as f64 * 0.01 + time * rate * 0.7);
        let key = |at: Vector3<f64>| (at - view.eye).norm() * 0.2;
        // What's breathed right over the eye is drawn smaller the nearer it is, and not at all
        // once the eye's inside it, so it never fills the view: being caught in it shows round
        // the edges of the view instead, see Game::hud()
        let near_eye = |at: Vector3<f64>, size: f64| {
            let d = (at - view.eye).norm() / vpm;
            if d < size * 0.55 + 0.5 { 0.0 } else { (d / (size * 2.2)).clamp(0.2, 1.0) }
        };
        for p in &fire.puffs {
            let size = puff_size(p.size, p.age, p.life) * far_scale(p.at, view);
            let size = size * near_eye(p.at, size);
            if size <= 0.0 {
                continue;
            }
            let fx = FX_PUFF + element_slot(p.element) * 2 + (p.seed % 2) as usize;
            out.push((key(p.at), place(fx, p.at, Vector3::new(0.0, 0.0, 0.5), spin(p.seed, 2.5), size)));
        }
        for b in &fire.balls {
            let size = (0.8 * b.radius.powf(0.5)).max(0.9) * far_scale(b.at, view);
            let fx = FX_PUFF + element_slot(b.element) * 2;
            out.push((key(b.at) * 0.5, place(fx, b.at, Vector3::new(0.0, 0.0, 0.5), spin(b.seed, 6.0), size)));
        }
        for p in &fire.patches {
            let fade = ((p.life - p.age) / 0.8).min(1.0).min(p.age / 0.25).max(0.15);
            let flicker = 1.0 + 0.08 * (time * 13.0 + p.seed as f64).sin();
            let fx = FX_FLAME + element_slot(p.element) * 2 + ((time * 6.0) as u32 + p.seed) as usize % 2;
            let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), (p.seed % 628) as f64 * 0.01 + time * 0.6);
            // Flames knee high on a little patch, waist high at the most on a great one, and low
            // round the player's feet when they're standing in it, so they can see out
            let size = (p.radius * 1.1).min(1.5) * fade * flicker;
            let size = size * near_eye(p.at + Vector3::new(0.0, 0.0, 0.6 * size * vpm), size).max(0.35);
            out.push((key(p.at), place(fx, p.at, Vector3::zeros(), turn, size)));
        }
        for c in &fire.cracks {
            let reach = (c.age / c.spread).min(1.0) * c.length;
            let pieces = (reach / (CRACK_PIECE * c.width)).ceil() as usize;
            for k in 0..pieces {
                let m = (k as f64 + 0.5) * CRACK_PIECE * c.width;
                if m > reach + CRACK_PIECE * c.width * 0.5 {
                    break;
                }
                let at = |m: f64| -> Vector3<f64> {
                    let i = ((m / CRACK_STEP) as usize).min(c.ground.len() - 1);
                    let p = c.from + c.way * m * vpm;
                    Vector3::new(p.x, p.y, c.ground[i])
                };
                let (a, b) = (at(m - CRACK_PIECE * c.width * 0.5), at(m + CRACK_PIECE * c.width * 0.5));
                let along = (b - a).try_normalize(1e-6).unwrap_or(c.way);
                let turn = UnitQuaternion::rotation_between(&Vector3::x(), &along).unwrap_or_else(UnitQuaternion::identity);
                let fade = ((c.life - c.age) / 1.0).min(1.0).max(0.2);
                out.push((key(a), place(FX_CRACK + k % 2, (a + b) * 0.5, Vector3::new(0.0, 0.0, 0.03), turn, c.width * fade.sqrt())));
            }
        }
        out
    }

    /// The middle of the body of the dragon nearest `near`, in voxels, and how far its wings reach
    /// in meters, to look at it, see the "dragon" script in main.rs
    pub fn nearest_dragon(&self, near: Vector3<f64>) -> Option<(Vector3<f64>, f64)> {
        let vpm = self.combat.vpm();
        self.animals
            .iter()
            .filter(|a| a.dragon.is_some() && a.health > 0.0)
            .min_by(|a, b| (a.position - near).norm().total_cmp(&(b.position - near).norm()))
            .map(|a| (a.position + Vector3::new(0.0, 0.0, a.look.body().0 * vpm), a.look.reach()))
    }

    /// The light of what dragons breathed and left burning: the brightest few, nearest first
    pub(super) fn dragonfire_lights(&self, view: &View, lights: &mut Vec<[f32; 8]>) {
        let vpm = view.vpm;
        let fire = &self.dragonfire;
        let light = |p: Vector3<f64>, reach: f64, element: Element, strength: f32| {
            let c = element.light();
            [p.x as f32, p.y as f32, p.z as f32, (reach * vpm) as f32, c[0] * strength, c[1] * strength, c[2] * strength, 1.0]
        };
        let mut mine: Vec<[f32; 8]> = Vec::new();
        // Where the breath's thickest, one light for all of it near each other
        if let Some(p) = fire.puffs.iter().filter(|p| !p.harmless).min_by(|a, b| (a.at - view.eye).norm().total_cmp(&(b.at - view.eye).norm())) {
            mine.push(light(p.at, 6.0 + 2.0 * p.size, p.element, 1.6));
        }
        for b in &fire.balls {
            mine.push(light(b.at, 5.0 + b.radius, b.element, 1.4));
        }
        for p in &fire.patches {
            let flicker = 0.8 + 0.2 * (self.combat.time() * 11.0 + p.seed as f64).sin() as f32;
            mine.push(light(p.at + Vector3::new(0.0, 0.0, 0.6 * vpm), 3.0 + 2.0 * p.radius, p.element, 0.9 * flicker));
        }
        mine.sort_by(|a, b| {
            let d = |l: &[f32; 8]| (Vector3::new(l[0] as f64, l[1] as f64, l[2] as f64) - view.eye).norm();
            d(a).total_cmp(&d(b))
        });
        lights.extend(mine.into_iter().take(3));
    }
}

/// The top of the ground at `x`, `y`, in voxels, looking down from a little over `z`, or `z`
fn ground_top_near(world: &World, x: f64, y: f64, z: f64) -> f64 {
    let vpm = world.voxels_per_meter as f64;
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    let top = (z + 3.0 * vpm) as i64;
    (top - (30.0 * vpm) as i64..=top).rev().find(|&k| !is_passable(world.material_at(xi, yi, k))).map_or(z, |k| k as f64 + 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How wide something has to look to the eye to be seen, in radians, as for shots, see the
    /// tests in combat.rs
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

    /// Everything a dragon breathes or spits at the player, from near or far, is drawn on screen
    /// and big enough to be followed by eye all the way, every frame, never gone in mid-air
    #[test]
    fn every_breath_and_fireball_can_be_followed() {
        let vpm = 4u32;
        let v = vpm as f64;
        let world = open_ground(200, vpm);
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let feet = Vector3::new(100.0, 40.0, 4.0) * v;
        let eye = feet + Vector3::new(0.0, 0.0, 1.6 * v);
        // Looking out along y, a little up, at what's coming
        let view = View { eye, rotation: UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 0.15), tan: [1.2445, 0.7002], vpm: v, speed: 0.0, stride: 0.0 };
        for look in dragons::KINDS {
            for meters in [12.0, 35.0, 70.0] {
                let mut game = Game::new(&world, &water, 3, feet);
                let from = feet + Vector3::new(0.0, meters, 10.0) * v;
                let dt = 1.0 / 60.0;
                game.dragon_act(look, Act::Fireball { from, to: feet, speed: 16.0, damage: 1.0, radius: 1.6, lob: false }, &world, &water, None, &view);
                for k in 0..30 {
                    // Breathing all the while, down towards the player
                    let dir = (feet - from).normalize();
                    game.dragon_act(look, Act::Breath { from, dir, dt, dps: 1.0, size: 1.2 * dragons::size(look).powf(0.6), speed: 10.0, spread: 0.0 }, &world, &water, None, &view);
                    game.tend_dragonfire(&world, None, &view, dt);
                    let drawn = game.dragonfire_instances(&view);
                    if k > 2 {
                        assert!(!drawn.is_empty(), "{look:?} at {meters} m: nothing drawn");
                    }
                    for (_, instance) in drawn {
                        let (at, reach) = instance.sphere();
                        let d = (at - eye).norm();
                        if d < 2.0 * v {
                            continue;
                        }
                        assert!(reach / d > SEEN_ANGLE, "{look:?} at {meters} m: something {:.1} m off only {:.4} rad across", d / v, reach / d);
                    }
                }
                // The ball's still there, flying, or it's burst where it came down
                assert!(!game.dragonfire.balls.is_empty() || !game.dragonfire.patches.is_empty(), "{look:?}'s fireball from {meters} m vanished");
            }
        }
    }
}
