//! The Earthbreaker Colossus working stone. The boulder it tears up out of the ground: the ground
//! cracking round its fists as it pries it loose, rising up out of it, then torn free in a burst
//! of earth and chunks, leaving a hollow, carried overhead in its hands and hurled, tumbling and
//! trailing dust and embers, to shatter where it lands. Slabs of rock heaving up where its fists
//! come down, spikes of it bursting up along the ground at the player as its quake runs out, and
//! pillars of it erupting round the player as it calls its cataclysm, flinging rocks down out of
//! their tops; each crumbling back into the ground after. Chunks of stone thrown about, tumbling,
//! bouncing and settling, then sinking away; and pebbles shaken off it as it walks. Nothing of it
//! comes or goes all at once. It's the fight's own machinery as the rest is, see combat.rs, kept
//! here out of the way of the rest, drawn with effects::STONE_BOULDER and STONE_SLAB.

use super::*;
use crate::bosses::{Carry, PILLAR_RISE, ROCKFALL_LONGEST, TEAR_PRY, TORN_BOULDER};
use crate::effects::{SLAB_HEIGHT, STONE_BOULDER, STONE_ROUND, STONE_SLAB};

/// How many chunks of stone and slabs of it there can be about at once before the oldest go
const MOST_CHUNKS: usize = 16;
const MOST_SLABS: usize = 30;
/// Seconds a chunk lies where it came to rest, and then takes to sink away into the ground
const CHUNK_LIE: f64 = 1.4;
const CHUNK_SINK: f64 = 0.8;
/// How much bigger than a slab of effects::STONE_SLAB the pillars of the Colossus's cataclysm are
const PILLAR_SIZE: f64 = 3.0;
/// How quickly what's held follows the hands holding it: about one over the seconds it lags
const HOLD_EASE: f64 = 30.0;
/// Past this many meters from the eye, a flying rock's drawn bigger the further it is, so it's
/// never lost to the eye, as the player's shots are
const ROCK_HELD: f64 = 15.0;

/// The boulder a colossus is prying loose or has hold of
struct Held {
    /// Whose it is, see animals::Animal::id
    id: u32,
    position: Vector3<f64>,
    turn: UnitQuaternion<f64>,
    /// How far it was pried loose as of last time, None once it's torn free
    prying: Option<f64>,
    /// Where the ground is over it, its middle on top of that, in voxels
    ground: Vector3<f64>,
    /// Updates since it was last seen held, see hold_stones()
    stale: u8,
    /// Seconds until the ground round it next cracks, or the next of what clings to it falls off
    tick: f64,
}

/// A rock flying: a boulder hurled, or one flung down out of a pillar
struct Rock {
    position: Vector3<f64>,
    velocity: Vector3<f64>,
    /// How big round it is, in meters
    radius: f64,
    turn: UnitQuaternion<f64>,
    /// Which way it tumbles, end over end the way it's going, and how fast, in radians a second
    axis: Unit<Vector3<f64>>,
    spin: f64,
    /// Where it was aimed to come down, seconds it was to take getting there and seconds it's
    /// been flying
    target: Vector3<f64>,
    flight: f64,
    age: f64,
    /// How hard it hits, how far round where it lands it hurts, in meters, and of what element;
    /// none for one let fall
    damage: f64,
    blast: f64,
    element: Option<Element>,
    /// Where the ground is under where it was aimed, worked out the first time it's needed
    ring: Option<Vector3<f64>>,
    /// Seconds until its trail next puffs out
    puff: f64,
}

/// A chunk of stone thrown about, tumbling, then lying where it came to rest
struct Chunk {
    position: Vector3<f64>,
    velocity: Vector3<f64>,
    radius: f64,
    turn: UnitQuaternion<f64>,
    axis: Unit<Vector3<f64>>,
    spin: f64,
    bounces: u8,
    /// Seconds left lying where it came to rest, None while it's flying
    rest: Option<f64>,
}

/// A slab, spike or pillar of rock bursting up out of the ground, standing a while, then crumbling
/// back down into it, drawn with effects::STONE_SLAB
struct Slab {
    /// Where its foot is on the ground, in voxels
    base: Vector3<f64>,
    /// How much bigger than the model it's drawn
    size: f64,
    yaw: f64,
    /// Which way it leans, across the ground, and how far over, in radians
    lean: Vector3<f64>,
    tilt: f64,
    /// Seconds until it bursts up, then seconds it's been up, and how long it takes to come up,
    /// stands and takes crumbling away
    delay: f64,
    age: f64,
    rise: f64,
    stand: f64,
    fall: f64,
    /// One of a cataclysm's pillars, which rocks are flung out of
    pillar: bool,
    /// Heard as it comes up and crumbles, not one of many at once
    loud: bool,
}

impl Slab {
    fn tall(&self) -> f64 {
        self.size * SLAB_HEIGHT
    }

    /// Seconds since it burst up out of the ground, negative before
    fn up(&self) -> f64 {
        self.age - self.delay
    }

    fn standing(&self) -> bool {
        (self.rise * 0.7..self.rise + self.stand).contains(&self.up())
    }

    /// Crumbling away now, if it hasn't yet, rather than standing on
    fn hasten(&mut self) {
        let t = self.up();
        if t < self.rise + self.stand {
            self.stand = (t - self.rise).max(0.0);
        }
    }

    /// How far out of the ground it's come, 0-1, and how far over it leans, now: bursting up fast,
    /// standing and shivering a little, then sinking back down as it crumbles, leaning further over
    fn pose(&self, time: f64) -> (f64, f64) {
        let t = self.up();
        if t < self.rise {
            let u = (t / self.rise).clamp(0.0, 1.0);
            (1.0 - (1.0 - u).powi(3), self.tilt * (0.6 + 0.4 * u))
        } else if t < self.rise + self.stand {
            (1.0, self.tilt + 0.01 * (time * 37.0 + self.base.x).sin())
        } else {
            let u = ((t - self.rise - self.stand) / self.fall).clamp(0.0, 1.0);
            (1.0 - u * u, self.tilt + 0.3 * u)
        }
    }

    /// Where it's drawn from, its foot sunk as far into the ground as it's not come up out of it,
    /// and how it's turned, in voxels
    fn placed(&self, time: f64, vpm: f64) -> (Vector3<f64>, UnitQuaternion<f64>) {
        let (out, tilt) = self.pose(time);
        let tall = self.tall();
        // Always a little sunk, so its foot never shows leaning over
        let sunk = (1.0 - out) * tall * 1.05 + 0.12 * tall;
        let across = Unit::try_new(Vector3::z().cross(&self.lean), 1e-6).unwrap_or_else(Vector3::x_axis);
        let turn = UnitQuaternion::from_axis_angle(&across, tilt) * UnitQuaternion::from_axis_angle(&Vector3::z_axis(), self.yaw);
        (self.base - Vector3::new(0.0, 0.0, sunk * vpm), turn)
    }

    /// Where its top is, in voxels
    fn top(&self, time: f64, vpm: f64) -> Vector3<f64> {
        let (at, turn) = self.placed(time, vpm);
        at + turn * Vector3::new(0.0, 0.0, self.tall() * 0.95 * vpm)
    }
}

pub(super) struct Stones {
    held: Vec<Held>,
    rocks: Vec<Rock>,
    chunks: Vec<Chunk>,
    slabs: Vec<Slab>,
    /// Where the eye was as of the last update, so no slab comes up round it, filling the view
    eye: Vector3<f64>,
}

/// How near the eye a slab can come up, across the ground, in meters, and how near it a chunk's
/// drawn
const CLEAR_OF_EYE: f64 = 2.2;
const CHUNK_CLEAR: f64 = 0.8;

impl Stones {
    pub(super) fn new() -> Self {
        Stones { held: Vec::new(), rocks: Vec::new(), chunks: Vec::new(), slabs: Vec::new(), eye: Vector3::repeat(f64::MAX) }
    }

    /// The world moved over by `shift` voxels
    pub(super) fn recenter(&mut self, shift: Vector3<f64>) {
        for h in &mut self.held {
            h.position -= shift;
            h.ground -= shift;
        }
        for r in &mut self.rocks {
            r.position -= shift;
            r.target -= shift;
            if let Some(ring) = &mut r.ring {
                *ring -= shift;
            }
        }
        for c in &mut self.chunks {
            c.position -= shift;
        }
        for s in &mut self.slabs {
            s.base -= shift;
        }
        self.eye -= shift;
    }

    pub(super) fn clear(&mut self) {
        self.held.clear();
        self.rocks.clear();
        self.chunks.clear();
        self.slabs.clear();
    }
}

/// The ground at `at`, in voxels, from `up` voxels over it down to `down` under it: the first solid
/// voxel with air over it, and on top of that. None if there's none there.
fn ground_near(world: &World, at: Vector3<f64>, up: f64, down: f64) -> Option<Vector3<f64>> {
    let (x, y) = (at.x.floor() as i64, at.y.floor() as i64);
    if x < 0 || y < 0 || x >= world.size[0] as i64 || y >= world.size[1] as i64 {
        return None;
    }
    let top = (at.z + up).floor() as i64;
    let bottom = (at.z - down).floor() as i64;
    (bottom..=top).rev().find(|&z| !is_passable(world.material_at(x, y, z)) && is_passable(world.material_at(x, y, z + 1))).map(|z| Vector3::new(at.x, at.y, z as f64 + 1.0))
}

/// How big a rock `radius` meters round is drawn `distance` meters from the eye, as effects::
/// STONE_BOULDER's size: as big as it is up close, and bigger further off, so it's never lost
fn rock_size(radius: f64, distance: f64) -> f64 {
    radius / STONE_ROUND * (distance / ROCK_HELD).max(1.0).sqrt()
}

/// A way of turning picked from `seed`
fn any_turn(seed: &mut u32) -> UnitQuaternion<f64> {
    let axis = Vector3::new(random(seed) - 0.5, random(seed) - 0.5, random(seed) - 0.5);
    let axis = Unit::try_new(axis, 1e-6).unwrap_or_else(Vector3::z_axis);
    UnitQuaternion::from_axis_angle(&axis, random(seed) * TAU)
}

impl Combat {
    /// Each frame: the boulders colossi hold, the rocks flying, chunks tumbling, slabs heaving up
    /// and crumbling, and the fire in a colossus's seams throwing off embers as it strains
    pub(super) fn work_stone(&mut self, arena: &mut Arena, dt: f64) {
        self.stones.eye = arena.view.eye;
        self.hold_stones(arena, dt);
        self.fly_rocks(arena, dt);
        self.tumble_chunks(arena.world, dt);
        self.heave_slabs(dt);
        self.colossus_embers(arena, dt);
    }

    /// The boulders the colossi are prying loose and holding, where they're drawn: following their
    /// hands, the ground cracking and spitting earth round one as it's pried loose, then torn free
    /// in a burst of it, a hollow left; earth and pebbles falling off it as it's lifted. One no
    /// longer held that wasn't hurled is let fall, rather than gone.
    fn hold_stones(&mut self, arena: &mut Arena, dt: f64) {
        let vpm = arena.view.vpm;
        let world = arena.world;
        let eye = arena.view.eye;
        for h in &mut self.stones.held {
            h.stale = h.stale.saturating_add(1);
        }
        let seen: Vec<(u32, Vector3<f64>, Carry)> = arena.animals.iter().filter(|a| a.health > 0.0).filter_map(|a| a.carried(vpm).map(|(at, carry)| (a.id, at, carry))).collect();
        let ease = 1.0 - (-HOLD_EASE * dt).exp();
        for (id, at, carry) in seen {
            let prying = match carry {
                Carry::Prying(loose) => Some(loose),
                Carry::Held => None,
            };
            let k = match self.stones.held.iter().position(|h| h.id == id) {
                Some(k) => k,
                None => {
                    // Its fists driving into the ground over it: a thud, and earth thrown up
                    let up = (TORN_BOULDER + 2.0) * vpm;
                    let ground = ground_near(world, at + Vector3::new(0.0, 0.0, up), 0.0, up + 4.0 * vpm).unwrap_or(at);
                    let turn = any_turn(&mut self.rng);
                    self.stones.held.push(Held { id, position: at, turn, prying, ground, stale: 0, tick: 0.2 });
                    self.stir(ground, 1.3, 0.35, DROP_MUD);
                    self.cues.push(Cue::at(Sound::Slam, ground, 0.8).pitched(0.55));
                    self.cues.push(Cue::at(Sound::Dig, ground, 0.7).pitched(0.6));
                    // The ground cracking and heaving up in slabs round where its fists dig in,
                    // as it pries, till it tears free
                    let turn = random(&mut self.rng) * TAU;
                    for j in 0..4 {
                        let a = turn + (j as f64 + 0.3 * random(&mut self.rng)) / 4.0 * TAU;
                        let out = Vector3::new(a.cos(), a.sin(), 0.0);
                        let Some(base) = ground_near(world, ground + out * TORN_BOULDER * 1.35 * vpm, 1.5 * vpm, 2.5 * vpm) else {
                            continue;
                        };
                        let tall = 0.8 + 0.4 * random(&mut self.rng);
                        let (delay, tilt) = (0.12 + 0.3 * random(&mut self.rng), 0.5 + 0.3 * random(&mut self.rng));
                        let stand = (TEAR_PRY * 0.85 - delay - 0.3).max(0.05);
                        self.slab(Slab { base, size: tall / SLAB_HEIGHT, yaw: a - FRAC_PI_2, lean: out, tilt, delay, age: 0.0, rise: 0.3, stand, fall: 0.6, pillar: false, loud: false });
                    }
                    self.stones.held.len() - 1
                }
            };
            let (was, ground) = (self.stones.held[k].prying, self.stones.held[k].ground);
            {
                let h = &mut self.stones.held[k];
                h.stale = 0;
                h.prying = prying;
                h.position = if prying.is_some() { at } else { h.position + (at - h.position) * ease };
                h.tick -= dt;
            }
            let tick = self.stones.held[k].tick <= 0.0;
            match (was, prying) {
                // The ground cracking and heaving round it as it's worked loose, more as it comes
                (_, Some(loose)) => {
                    self.stir(ground, TORN_BOULDER * 1.4, dt * (0.6 + 2.5 * loose), DROP_MUD);
                    if tick {
                        self.stones.held[k].tick = 0.32 - 0.18 * loose;
                        self.cues.push(Cue::at(Sound::Crack, ground, 0.35 + 0.4 * loose as f32).pitched(0.45 + 0.2 * random(&mut self.rng) as f32));
                        if loose > 0.4 {
                            let a = random(&mut self.rng) * TAU;
                            let out = Vector3::new(a.cos(), a.sin(), 0.0);
                            let v = (out * 1.5 + Vector3::new(0.0, 0.0, 3.0 + 2.0 * random(&mut self.rng))) * vpm;
                            let size = 0.1 + 0.08 * random(&mut self.rng);
                            self.chunk(ground + out * TORN_BOULDER * 1.1 * vpm + Vector3::new(0.0, 0.0, 0.15 * vpm), v, size);
                        }
                    }
                }
                // Torn free: the ground bursting open round it, chunks and earth flying, dust
                // billowing up, a hollow left where it was
                (Some(_), None) => {
                    self.craters.push((ground, TORN_BOULDER * 1.25));
                    for _ in 0..9 {
                        let out = self.any_direction();
                        let v = Vector3::new(out.x * 3.5, out.y * 3.5, 4.0 + 4.0 * random(&mut self.rng)) * vpm;
                        let size = 0.1 + 0.16 * random(&mut self.rng);
                        let from = ground + Vector3::new(out.x, out.y, 0.0) * TORN_BOULDER * 0.9 * vpm + Vector3::new(0.0, 0.0, 0.2 * vpm);
                        self.chunk(from, v, size);
                    }
                    for _ in 0..26 {
                        let out = self.any_direction();
                        let v = Vector3::new(out.x * 3.0, out.y * 3.0, 3.0 + 4.0 * random(&mut self.rng)) * vpm;
                        let kind = self.earth(0.7);
                        self.drop(ground + Vector3::new(out.x, out.y, 0.3) * TORN_BOULDER * vpm, v, kind, 1.6);
                    }
                    for _ in 0..3 {
                        let out = self.any_direction();
                        let v = Vector3::new(out.x * 1.6, out.y * 1.6, 1.2) * vpm;
                        self.dust(ground + Vector3::new(out.x, out.y, 0.4) * TORN_BOULDER * vpm, v, SLOT_CLOUD, 1.8);
                    }
                    self.cues.push(Cue::at(Sound::Crack, ground, 1.0).pitched(0.4));
                    self.cues.push(Cue::at(Sound::Dig, ground, 1.0).pitched(0.45));
                    self.cues.push(Cue::at(Sound::Quake, ground, 0.6).pitched(1.15));
                    let near = (1.0 - (ground - eye).norm() / vpm / 30.0).max(0.0);
                    self.shake = self.shake.max(0.5 * near);
                }
                // Lifted, earth and pebbles falling off it, and embers off the fire in its cracks
                (None, None) => {
                    if tick {
                        self.stones.held[k].tick = 0.12 + 0.2 * random(&mut self.rng);
                        let position = self.stones.held[k].position;
                        let off = self.any_direction() * TORN_BOULDER * 0.7 * vpm - Vector3::new(0.0, 0.0, 0.4 * TORN_BOULDER * vpm);
                        let v = self.any_direction() * 0.5 * vpm;
                        let kind = self.earth(0.6);
                        self.drop(position + off, v, kind, 2.2);
                        let ember = position + self.any_direction() * TORN_BOULDER * vpm;
                        self.mote(ember, Some(Element::Fire), 1.6);
                    }
                }
            }
        }
        // Let go of without being hurled, as when the colossus is felled holding it: it falls, or
        // never torn free, it settles back down into the ground
        let mut k = 0;
        while k < self.stones.held.len() {
            if self.stones.held[k].stale < 2 {
                k += 1;
                continue;
            }
            let h = self.stones.held.swap_remove(k);
            if h.prying.is_some() {
                self.stones.chunks.push(Chunk { position: h.position, velocity: Vector3::zeros(), radius: TORN_BOULDER, turn: h.turn, axis: Vector3::z_axis(), spin: 0.0, bounces: 2, rest: Some(CHUNK_SINK) });
                continue;
            }
            let axis = Unit::new_normalize(Vector3::new(1.0, 0.3, 0.0));
            self.stones.rocks.push(Rock { position: h.position, velocity: Vector3::zeros(), radius: TORN_BOULDER, turn: h.turn, axis, spin: 1.0, target: h.position, flight: 1.0, age: 0.0, damage: 0.0, blast: 0.0, element: None, ring: None, puff: 0.0 });
        }
    }

    /// Colossus `id` hurling the boulder it holds at `to`, to come down there after `time`
    /// seconds, hitting for `damage` within `blast` meters: from right where it's drawn in its
    /// hands, as big, or from `from` if it has none
    pub fn hurl_boulder(&mut self, id: u32, from: Vector3<f64>, to: Vector3<f64>, time: f64, damage: f64, blast: f64) {
        let (start, turn) = match self.stones.held.iter().position(|h| h.id == id) {
            Some(k) => {
                let h = self.stones.held.swap_remove(k);
                (h.position, h.turn)
            }
            None => (from, any_turn(&mut self.rng)),
        };
        self.throw_rock(start, to, time, TORN_BOULDER, turn, damage, blast, None);
        // Earth shaken off it as it leaves its hands
        for _ in 0..6 {
            let v = self.any_direction() * 1.5 * self.vpm;
            let from = start + self.any_direction() * TORN_BOULDER * self.vpm;
            self.drop(from, v, DROP_MUD, 1.6);
        }
    }

    /// DROP_MUD, `mud` of the time, otherwise DROP_ASH: earth, or darker grit
    fn earth(&mut self, mud: f64) -> usize {
        if random(&mut self.rng) < mud { DROP_MUD } else { DROP_ASH }
    }

    /// A rock `radius` meters round thrown from `from` to come down at `to` after `time` seconds,
    /// tumbling end over end
    #[allow(clippy::too_many_arguments)]
    fn throw_rock(&mut self, from: Vector3<f64>, to: Vector3<f64>, time: f64, radius: f64, turn: UnitQuaternion<f64>, damage: f64, blast: f64, element: Option<Element>) {
        let gravity = 9.8 * self.vpm;
        let velocity = (to - from) / time + Vector3::new(0.0, 0.0, 0.5 * gravity * time);
        let axis = Unit::try_new(velocity.cross(&Vector3::z()), 1e-6).unwrap_or_else(Vector3::y_axis);
        let spin = -(1.5 + random(&mut self.rng)) / radius.max(0.3);
        self.stones.rocks.push(Rock { position: from, velocity, radius, turn, axis, spin, target: to, flight: time, age: 0.0, damage, blast, element, ring: None, puff: 0.0 });
    }

    /// The `k`th rock of a colossus's cataclysm flung at `to`, out of the top of one of the
    /// pillars it raised, round them in turn, a burst of rubble off it; from `from` over the
    /// colossus if none's standing
    pub fn rockfall(&mut self, from: Vector3<f64>, to: Vector3<f64>, k: usize, damage: f64, blast: f64) {
        let vpm = self.vpm;
        let time = self.time;
        let standing: Vec<usize> = (0..self.stones.slabs.len()).filter(|&i| self.stones.slabs[i].pillar && self.stones.slabs[i].standing()).collect();
        let (start, flight) = match standing.get(k % standing.len().max(1)) {
            Some(&i) => {
                let top = self.stones.slabs[i].top(time, vpm);
                for _ in 0..8 {
                    let out = self.any_direction();
                    let kind = self.earth(0.6);
                    self.drop(top + out * 0.4 * vpm, Vector3::new(out.x * 2.0, out.y * 2.0, 1.5 + 2.0 * out.z.abs()) * vpm, kind, 1.8);
                }
                self.sparks(top, 5, 2.5, Element::look(Some(Element::Fire)), 0.5, 0.5);
                self.dust(top, Vector3::new(0.0, 0.0, 0.6 * vpm), SLOT_SMOKE, 0.9);
                if k % 2 == 0 {
                    self.cues.push(Cue::at(Sound::Crack, top, 0.6).pitched(0.6 + 0.2 * random(&mut self.rng) as f32));
                }
                (top + Vector3::new(0.0, 0.0, 0.4 * vpm), ROCKFALL_LONGEST - 0.4 * random(&mut self.rng))
            }
            None => (from, 1.4),
        };
        let radius = 0.42 + 0.16 * random(&mut self.rng);
        let turn = any_turn(&mut self.rng);
        self.throw_rock(start, to, flight, radius, turn, damage, blast, Some(Element::Fire));
    }

    /// Rocks flying, tumbling, trailing dust and embers, the ground ringed where each is coming
    /// down; hitting the player, or the ground or water, where they shatter
    fn fly_rocks(&mut self, arena: &mut Arena, dt: f64) {
        let vpm = arena.view.vpm;
        let gravity = 9.8 * vpm;
        for mut r in std::mem::take(&mut self.stones.rocks) {
            r.velocity.z -= gravity * dt;
            r.age += dt;
            r.turn = UnitQuaternion::from_axis_angle(&r.axis, r.spin * dt) * r.turn;
            // A trail of dust and of embers off the fire in its cracks, the more the bigger it is
            r.puff -= dt;
            if r.puff <= 0.0 {
                let great = r.radius > 0.7;
                r.puff = if great { 0.06 } else { 0.12 };
                let back = -r.velocity * 0.05;
                let off = self.any_direction() * r.radius * 0.7 * vpm;
                let kind = self.earth(0.5);
                self.drop(r.position + off, back, kind, 0.6);
                let off = self.any_direction() * r.radius * 0.9 * vpm;
                self.mote(r.position + off, Some(Element::Fire), 1.6);
                if great {
                    self.dust(r.position - r.velocity * 0.02, back * 0.5, SLOT_SMOKE, 0.7);
                }
            }
            // Where it's coming down, ringed on the ground under where it was aimed
            if r.damage > 0.0 {
                let target = r.target;
                let ring = *r.ring.get_or_insert_with(|| ground_near(arena.world, target, vpm, 4.0 * vpm).unwrap_or(target));
                self.mark(ring, r.blast, (1.0 - r.age / r.flight).clamp(0.0, 1.0));
            }
            // Its bottom striking the ground, or it striking the player or water
            let (start, end) = (r.position, r.position + r.velocity * dt);
            let down = Vector3::new(0.0, 0.0, r.radius * 0.7 * vpm);
            let body = arena.player.map(|feet| feet + Vector3::new(0.0, 0.0, 1.0 * vpm));
            let direct = body.filter(|_| r.damage > 0.0).and_then(|b| sphere_hit(start, end, b, (0.6 + r.radius) * vpm));
            let wall = solid_along(arena.world, start - down, end - down);
            let (high, low) = ((arena.water)(start.x, start.y), (arena.water)(end.x, end.y));
            let water = (start.z > high && end.z <= low && low.is_finite()).then(|| ((start.z - low) / (start.z - end.z)).clamp(0.0, 1.0));
            let Some(t) = [direct, wall, water].into_iter().flatten().reduce(f64::min) else {
                r.position = end;
                if !lost(arena.world, arena.view.eye, r.position, true) && r.age < r.flight + 6.0 {
                    self.stones.rocks.push(r);
                }
                continue;
            };
            let at = start + (end - start) * t;
            if let Some(b) = body
                && r.damage > 0.0
            {
                let d = (b - at).norm() / vpm;
                if d < r.blast {
                    self.player_hits.push((r.damage * (1.0 - 0.5 * d / r.blast), at, r.element));
                } else if d < r.blast + 1.2 {
                    self.near_misses.push(at);
                }
            }
            let near = (1.0 - (at - arena.view.eye).norm() / vpm / 30.0).max(0.0);
            self.shake = self.shake.max(near * if r.radius > 0.7 { 0.7 } else { 0.4 });
            if water.is_some_and(|w| w <= t) && direct.is_none_or(|d| d > t) {
                self.splash(at, 1.2 + r.radius);
                continue;
            }
            // Where it struck: its bottom on the ground, or itself on the player
            let impact = if wall.is_some_and(|w| w <= t) { at - down } else { at };
            self.smash(impact, r.radius, r.velocity, vpm);
        }
    }

    /// A rock `radius` meters round shattering at `at` as it lands, going along `velocity`: a
    /// hole knocked in the ground, chunks of it thrown out every way, on along the way it was
    /// going, earth and embers flying, and dust billowing up
    fn smash(&mut self, at: Vector3<f64>, radius: f64, velocity: Vector3<f64>, vpm: f64) {
        let great = radius > 0.7;
        self.craters.push((at, if great { 1.0 } else { 0.55 }));
        let on = Vector3::new(velocity.x, velocity.y, 0.0) * 0.12;
        let (chunks, drops, puffs) = if great { (8, 22, 3) } else { (2, 8, 1) };
        for _ in 0..chunks {
            let out = self.any_direction();
            let v = Vector3::new(out.x, out.y, out.z.abs() * 0.8 + 0.6) * (3.0 + 3.0 * random(&mut self.rng)) * vpm + on;
            let size = radius * (0.12 + 0.16 * random(&mut self.rng));
            self.chunk(at + out * radius * 0.4 * vpm + Vector3::new(0.0, 0.0, 0.25 * vpm), v, size);
        }
        for _ in 0..drops {
            let out = self.any_direction();
            let v = Vector3::new(out.x * 3.5, out.y * 3.5, 2.5 + 3.5 * random(&mut self.rng)) * vpm + on * 0.5;
            let kind = match (random(&mut self.rng) * 3.0) as usize {
                0 => DROP_MUD,
                1 => DROP_ASH,
                _ => DROP_EMBER,
            };
            self.drop(at + Vector3::new(0.0, 0.0, 0.2 * vpm), v, kind, 1.4);
        }
        for _ in 0..puffs {
            let out = self.any_direction();
            self.dust(at + Vector3::new(out.x, out.y, 0.5) * radius * vpm, Vector3::new(out.x * 1.4, out.y * 1.4, 0.9) * vpm, if great { SLOT_CLOUD } else { SLOT_SMOKE }, 1.6);
        }
        self.sparks(at + Vector3::new(0.0, 0.0, 0.3 * vpm), if great { 10 } else { 4 }, 3.0, Element::look(Some(Element::Fire)), 0.6, 0.6);
        if great {
            self.flashes.push(Flash { position: at, color: Element::Fire.light(), reach: 6.0, life: 0.25, lifetime: 0.25 });
            self.cues.push(Cue::at(Sound::Slam, at, 1.0).pitched(0.6));
            self.cues.push(Cue::at(Sound::Quake, at, 0.7).pitched(1.1));
        } else {
            self.cues.push(Cue::at(Sound::Slam, at, 0.55).pitched(0.85 + 0.25 * random(&mut self.rng) as f32));
        }
        self.cap_sparks();
    }

    /// A puff of dust at `at`, drifting off along `velocity`: a little one, SLOT_SMOKE, or a great
    /// billow of it, SLOT_CLOUD
    fn dust(&mut self, at: Vector3<f64>, velocity: Vector3<f64>, slot: usize, lifetime: f64) {
        let life = lifetime * (0.75 + 0.25 * random(&mut self.rng));
        self.sparks.push(Spark { position: at, velocity, look: Element::look(None), life, lifetime: life, gravity: -0.02, slot });
        self.cap_sparks();
    }

    /// A chunk of stone `radius` meters round thrown from `at` at `velocity`, tumbling. Past
    /// MOST_CHUNKS, the oldest lying still sinks away now.
    fn chunk(&mut self, at: Vector3<f64>, velocity: Vector3<f64>, radius: f64) {
        let axis = Unit::try_new(self.any_direction(), 1e-6).unwrap_or_else(Vector3::z_axis);
        let turn = any_turn(&mut self.rng);
        let spin = (4.0 + 6.0 * random(&mut self.rng)) / radius.max(0.08).sqrt() * 0.3;
        self.stones.chunks.push(Chunk { position: at, velocity, radius, turn, axis, spin, bounces: 0, rest: None });
        if self.stones.chunks.len() > MOST_CHUNKS {
            if let Some(c) = self.stones.chunks.iter_mut().find(|c| c.rest.is_some_and(|left| left > CHUNK_SINK)) {
                c.rest = Some(CHUNK_SINK);
            } else if self.stones.chunks.len() > MOST_CHUNKS + 8 {
                self.stones.chunks.remove(0);
            }
        }
    }

    /// Chunks flying and falling, tumbling, bouncing once or twice off the ground and coming to
    /// rest on it, then after a while sinking away into it
    fn tumble_chunks(&mut self, world: &World, dt: f64) {
        let vpm = self.vpm;
        let [sx, sy, _] = world.size.map(f64::from);
        for c in &mut self.stones.chunks {
            if let Some(left) = &mut c.rest {
                *left -= dt;
                continue;
            }
            c.velocity.z -= 9.8 * vpm * dt;
            c.turn = UnitQuaternion::from_axis_angle(&c.axis, c.spin * dt) * c.turn;
            let next = c.position + c.velocity * dt;
            let foot = next - Vector3::new(0.0, 0.0, c.radius * 0.7 * vpm);
            let (x, y, z) = (foot.x.floor() as i64, foot.y.floor() as i64, foot.z.floor() as i64);
            if z >= 0 && is_passable(world.material_at(x, y, z)) {
                c.position = next;
                continue;
            }
            // Into the side of something: it stops going that way and drops
            let beside = !is_passable(world.material_at(x, y, (c.position.z - c.radius * 0.7 * vpm).floor() as i64));
            if beside && c.position.z >= 0.0 {
                c.velocity.x *= -0.2;
                c.velocity.y *= -0.2;
                continue;
            }
            if c.velocity.z < -2.0 * vpm && c.bounces < 2 {
                c.velocity.z *= -0.3;
                c.velocity.x *= 0.5;
                c.velocity.y *= 0.5;
                c.spin *= 0.5;
                c.bounces += 1;
                continue;
            }
            c.position.z = (z.max(0) + 1) as f64 + c.radius * 0.6 * vpm;
            c.velocity = Vector3::zeros();
            c.rest = Some(CHUNK_LIE + CHUNK_SINK);
        }
        self.stones.chunks.retain(|c| c.rest.is_none_or(|left| left > 0.0) && c.position.x >= 0.0 && c.position.y >= 0.0 && c.position.x < sx && c.position.y < sy && c.position.z > -8.0 * vpm);
    }

    /// Slabs and pillars coming up, the ground stirring where each is about to, rubble thrown up
    /// as it bursts out and shed as it crumbles away
    fn heave_slabs(&mut self, dt: f64) {
        let vpm = self.vpm;
        let mut slabs = std::mem::take(&mut self.stones.slabs);
        for s in &mut slabs {
            let before = s.up();
            s.age += dt;
            let t = s.up();
            if t < 0.0 {
                if random(&mut self.rng) < dt * 14.0 {
                    let off = self.any_direction() * 0.4 * s.size * vpm;
                    self.drop(s.base + Vector3::new(off.x, off.y, 0.1 * vpm), Vector3::new(0.0, 0.0, 2.5 * vpm), DROP_MUD, 0.5);
                }
                continue;
            }
            let tall = s.tall();
            if before < 0.0 {
                // Bursting up out of the ground
                for _ in 0..(6.0 + 6.0 * s.size) as usize {
                    let out = self.any_direction();
                    let v = Vector3::new(out.x * 2.5, out.y * 2.5, 3.0 + 3.0 * random(&mut self.rng)) * vpm;
                    let kind = self.earth(0.7);
                    self.drop(s.base + Vector3::new(out.x, out.y, 0.2) * 0.5 * s.size * vpm, v, kind, 1.3);
                }
                if s.loud || s.pillar {
                    let out = self.any_direction();
                    let v = Vector3::new(out.x * 2.0, out.y * 2.0, 4.0 + 2.0 * random(&mut self.rng)) * vpm;
                    let size = 0.1 + 0.1 * random(&mut self.rng);
                    self.chunk(s.base + Vector3::new(0.0, 0.0, 0.3 * vpm), v, size);
                    self.dust(s.base + Vector3::new(0.0, 0.0, 0.4 * vpm), Vector3::new(out.x, out.y, 0.8) * vpm, if s.pillar { SLOT_CLOUD } else { SLOT_SMOKE }, 1.4);
                }
                if s.loud {
                    self.cues.push(Cue::at(Sound::Crack, s.base, if s.pillar { 0.9 } else { 0.6 }).pitched(if s.pillar { 0.45 } else { 0.7 + 0.2 * random(&mut self.rng) as f32 }));
                }
                if s.pillar {
                    self.cues.push(Cue::at(Sound::Quake, s.base, 0.7).pitched(0.8));
                }
            }
            let crumbling = s.rise + s.stand;
            if t > crumbling {
                if before <= crumbling && s.loud {
                    self.cues.push(Cue::at(Sound::Quake, s.base, if s.pillar { 0.8 } else { 0.45 }).pitched(if s.pillar { 0.9 } else { 1.3 }));
                    self.dust(s.base + Vector3::new(0.0, 0.0, 0.3 * tall * vpm), Vector3::new(0.0, 0.0, 0.5 * vpm), if s.pillar { SLOT_CLOUD } else { SLOT_SMOKE }, 1.5);
                }
                // Shedding rubble down its face as it crumbles away
                if random(&mut self.rng) < dt * 12.0 * s.size {
                    let (at, turn) = s.placed(self.time, vpm);
                    let up = (0.3 + 0.6 * random(&mut self.rng)) * tall;
                    let from = at + turn * Vector3::new((random(&mut self.rng) - 0.5) * 0.8 * s.size, 0.0, up) * vpm;
                    if from.z > s.base.z {
                        let v = (s.lean * 0.6 + self.any_direction() * 0.4) * vpm;
                        let kind = self.earth(0.6);
                        self.drop(from, v, kind, 1.4);
                    }
                }
                if s.pillar && random(&mut self.rng) < dt * 3.0 {
                    let top = s.top(self.time, vpm);
                    if top.z > s.base.z + 0.5 * vpm {
                        let v = (s.lean + self.any_direction() * 0.5) * 1.5 * vpm;
                        let size = 0.12 + 0.12 * random(&mut self.rng);
                        self.chunk(top, v, size);
                    }
                }
            }
        }
        slabs.retain(|s| s.up() < s.rise + s.stand + s.fall);
        slabs.append(&mut self.stones.slabs);
        self.stones.slabs = slabs;
    }

    /// A slab of rock to come up, unless it would come up round the eye, filling the view. Past
    /// MOST_SLABS, the oldest crumble away now rather than stand on, never gone all at once unless
    /// there are a great many more.
    fn slab(&mut self, s: Slab) {
        let off = s.base - self.stones.eye;
        if off.xy().norm() < (CLEAR_OF_EYE + 0.4 * s.size) * self.vpm && off.z.abs() < 4.0 * self.vpm {
            return;
        }
        self.stones.slabs.push(s);
        let excess = self.stones.slabs.len().saturating_sub(MOST_SLABS);
        for s in self.stones.slabs.iter_mut().filter(|s| !s.pillar).take(excess) {
            s.hasten();
        }
        if self.stones.slabs.len() > MOST_SLABS * 2 {
            self.stones.slabs.remove(0);
        }
    }

    /// Slabs of stone heaving up in a ring `radius` meters round `at`, broad faces out, leaning
    /// out, and falling back: where a colossus's fists came down, or round it as it roars. Rubble
    /// thrown up in the middle.
    pub fn heave(&mut self, world: &World, at: Vector3<f64>, radius: f64) {
        let vpm = self.vpm;
        let count = 7;
        // As much bigger as the blow reaches further, so they're seen from well off
        let big = (radius / 4.0).clamp(1.0, 1.6);
        let turn = random(&mut self.rng) * TAU;
        for k in 0..count {
            let a = turn + (k as f64 + 0.3 * (random(&mut self.rng) - 0.5)) / count as f64 * TAU;
            let out = Vector3::new(a.cos(), a.sin(), 0.0);
            let r = radius * (0.5 + 0.3 * random(&mut self.rng)) * vpm;
            let Some(base) = ground_near(world, at + out * r, 2.5 * vpm, 4.0 * vpm) else {
                continue;
            };
            let tall = (1.5 + 0.8 * random(&mut self.rng)) * big;
            let (stand, tilt, delay) = (0.35 + 0.15 * random(&mut self.rng), 0.35 + 0.25 * random(&mut self.rng), 0.04 * random(&mut self.rng));
            self.slab(Slab { base, size: tall / SLAB_HEIGHT, yaw: a - FRAC_PI_2, lean: out, tilt, delay, age: 0.0, rise: 0.14, stand, fall: 0.7, pillar: false, loud: k == 0 });
        }
        for _ in 0..5 {
            let out = self.any_direction();
            let v = Vector3::new(out.x * 3.0, out.y * 3.0, 4.0 + 3.0 * random(&mut self.rng)) * vpm;
            let size = 0.12 + 0.14 * random(&mut self.rng);
            self.chunk(at + Vector3::new(0.0, 0.0, 0.3 * vpm), v, size);
        }
        self.cues.push(Cue::at(Sound::Quake, at, 0.8).pitched(1.0));
    }

    /// A spike of rock `height` meters tall bursting up out of the ground at `at`, leaning on along
    /// `lean`, as a colossus's quake runs out under it, then crumbling
    pub fn spire(&mut self, world: &World, at: Vector3<f64>, height: f64, lean: Vector3<f64>) {
        let vpm = self.vpm;
        let Some(base) = ground_near(world, at, 3.0 * vpm, 4.0 * vpm) else {
            return;
        };
        let side = random(&mut self.rng) - 0.5;
        let (c, s) = ((side * 0.5).cos(), (side * 0.5).sin());
        let lean = Vector3::new(lean.x * c - lean.y * s, lean.x * s + lean.y * c, 0.0);
        let size = height / SLAB_HEIGHT * (0.9 + 0.2 * random(&mut self.rng));
        let tilt = 0.3 + 0.2 * random(&mut self.rng);
        let loud = random(&mut self.rng) < 0.35;
        self.slab(Slab { base, size, yaw: lean.y.atan2(lean.x) - FRAC_PI_2 + side * 0.6, lean, tilt, delay: 0.0, age: 0.0, rise: 0.12, stand: 0.45, fall: 0.8, pillar: false, loud });
    }

    /// A pillar of stone erupting out of the ground at `at` for a colossus's cataclysm, standing
    /// `stand` seconds, leaning in a little towards `towards`, then crumbling away
    pub fn pillar(&mut self, world: &World, at: Vector3<f64>, towards: Vector3<f64>, stand: f64) {
        let vpm = self.vpm;
        let Some(base) = ground_near(world, at, 4.0 * vpm, 6.0 * vpm) else {
            return;
        };
        let lean = Vector3::new(towards.x - base.x, towards.y - base.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
        // A monolith of the Colossus's stone, its narrow side to the player, its seams glowing
        let size = PILLAR_SIZE * (0.85 + 0.3 * random(&mut self.rng));
        let yaw = lean.y.atan2(lean.x) + 0.3 * (random(&mut self.rng) - 0.5);
        let tilt = 0.06 + 0.06 * random(&mut self.rng);
        self.slab(Slab { base, size, yaw, lean, tilt, delay: 0.0, age: 0.0, rise: PILLAR_RISE, stand, fall: 1.3, pillar: true, loud: true });
    }

    /// Pebbles and dust shaken off a colossus at `from`, falling round `at` on the ground below:
    /// as a foot comes down there, or `loud`, its plates grinding as it stands
    pub fn shed(&mut self, at: Vector3<f64>, from: Vector3<f64>, loud: bool) {
        let vpm = self.vpm;
        self.stir(at, 0.9, 0.1, DROP_MUD);
        for _ in 0..3 {
            let v = (self.any_direction() * 0.6 + Vector3::new(0.0, 0.0, 0.2)) * vpm;
            let off = self.any_direction() * 0.3 * vpm;
            let kind = self.earth(0.5);
            self.drop(from + off, v, kind, 2.2);
        }
        if loud || random(&mut self.rng) < 0.15 {
            let v = (self.any_direction() * 0.8 + Vector3::new(0.0, 0.0, 0.3)) * vpm;
            let size = 0.07 + 0.06 * random(&mut self.rng);
            self.chunk(from, v, size);
        }
        if loud {
            self.cues.push(Cue::at(Sound::Quake, from, 0.22).pitched(1.7 + 0.2 * random(&mut self.rng) as f32));
        }
    }

    /// Embers thrown off the fire in a colossus's seams as it strains
    fn colossus_embers(&mut self, arena: &Arena, dt: f64) {
        let vpm = arena.view.vpm;
        for a in arena.animals.iter().filter(|a| a.look == Look::Colossus && a.health > 0.0) {
            let strain = a.boss.as_ref().map_or(0.0, |b| b.strain());
            if strain > 0.2 && random(&mut self.rng) < dt * 10.0 * strain {
                let at = a.center(vpm as u32) + self.any_direction() * 1.3 * vpm;
                self.mote(at, Some(Element::Fire), 1.6);
            }
        }
    }

    /// What to draw for the stone: what's held and flying, which the eye follows, as if it were
    /// right at the eye, so it's drawn however much else is about; slabs before what's nearer;
    /// chunks as far off as they are. Into `out` with how far each is from the camera, see
    /// flying_instances().
    pub(super) fn stone_instances(&self, view: &View, out: &mut Vec<(f64, AnimalInstance)>) {
        let vpm = view.vpm;
        let d = |at: Vector3<f64>| (at - view.eye).norm();
        let (model, boulder) = self.model_at(STONE_BOULDER);
        for h in &self.stones.held {
            let size = rock_size(TORN_BOULDER, d(h.position) / vpm);
            out.push((d(h.position) * 0.02, place(model, boulder, MODEL_RES, Vector3::zeros(), h.position, h.turn, vpm, size, MOVING)));
        }
        for r in &self.stones.rocks {
            let size = rock_size(r.radius, d(r.position) / vpm);
            out.push((d(r.position) * 0.02, place(model, boulder, MODEL_RES, Vector3::zeros(), r.position, r.turn, vpm, size, MOVING)));
        }
        for c in &self.stones.chunks {
            let sunk = c.rest.map_or(0.0, |left| (1.0 - left / CHUNK_SINK).max(0.0)) * c.radius * 1.8 * vpm;
            let at = c.position - Vector3::new(0.0, 0.0, sunk);
            // Not right up against the eye, where it would be a wall of stone across the view
            if d(at) < (CHUNK_CLEAR + c.radius) * vpm {
                continue;
            }
            out.push((d(at), place(model, boulder, MODEL_RES, Vector3::zeros(), at, c.turn, vpm, c.radius / STONE_ROUND, MOVING)));
        }
        for s in self.stones.slabs.iter().filter(|s| s.up() >= 0.0) {
            let (at, turn) = s.placed(self.time, vpm);
            let (model, shape) = self.model_at(STONE_SLAB);
            out.push((d(at) * 0.3, place(model, shape, MODEL_RES, Vector3::zeros(), at, turn, vpm, s.size, MOVING)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bosses::Move;
    use crate::worldgen::material::DIRT as GROUND;

    /// A flat world 48 m by 48 m at 8 voxels a meter, the ground 4 m deep
    fn flat() -> World {
        let size = [384u32, 384, 64];
        let (tx, ty) = (size[0] as usize / 4, size[1] as usize / 4);
        let mut occupancy = vec![0u128; tx * ty * (size[2] as usize / 8)];
        occupancy[..tx * ty * 4].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, GROUND);
        world.voxels_per_meter = 8;
        world
    }

    /// The Colossus tearing a boulder up and hurling it at the player, played through frame by
    /// frame: it's drawn coming up out of the ground and in its hands before it's hurled, it leaves
    /// its hands from right where it was drawn, as big, and it's drawn all the way to where it
    /// lands, never shrinking much from one frame to the next nor ever too small to see, going in
    /// an arc over the ground, then shattering into chunks and leaving a hole
    #[test]
    fn hurled_boulder_is_seen_all_the_way() {
        let world = flat();
        let vpm = 8.0;
        let water = |_: f64, _: f64| f64::NEG_INFINITY;
        let ground = 32.0;
        let feet = Vector3::new(24.0, 6.0, 0.0) * vpm + Vector3::new(0.0, 0.0, ground);
        let view = View { eye: feet + Vector3::new(0.0, 0.0, 1.6 * vpm), rotation: UnitQuaternion::identity(), tan: [0.75, 0.42], vpm, speed: 0.0, stride: 0.0 };
        let home = Vector3::new(24.0, 26.0, 0.0) * vpm + Vector3::new(0.0, 0.0, ground);
        let mut colossus = Animal::boss(Look::Colossus, home, [0, 0, -1], 7);
        if let Some(boss) = &mut colossus.boss {
            boss.forced = Some(Move::Throw);
            boss.provoke();
        }
        let mut animals = vec![colossus];
        let mut combat = Combat::new(1, 0);
        let dt = 1.0 / 60.0;
        let (mut held_frames, mut hurled, mut landed) = (0, None, None);
        let mut last_size: Option<f64> = None;
        let mut lowest_px = f64::MAX;
        for frame in 0..900 {
            for a in &mut animals {
                a.update(&world, &water, Some(feet), dt);
            }
            let mut arena = Arena { world: &world, animals: &mut animals, view: &view, player: Some(feet), water: &water };
            combat.update(&mut arena, None, None, Controls::default(), dt);
            let mut deeds = Vec::new();
            for a in &mut animals {
                if let Some(boss) = &mut a.boss {
                    deeds.append(&mut boss.deeds);
                }
            }
            let before = combat.stones.held.first().map(|h| h.position);
            for deed in deeds {
                if let crate::bosses::Deed::Hurl { id, from, to } = deed {
                    assert!(hurled.is_none(), "hurled twice");
                    combat.hurl_boulder(id, from, to, 1.2, 28.0, 2.4);
                    let rock = combat.stones.rocks.last().expect("nothing hurled");
                    let held = before.expect("hurled with nothing in its hands");
                    assert!((rock.position - held).norm() < 1e-9, "left its hands {:.2} m from where it was drawn", (rock.position - held).norm() / vpm);
                    assert!(combat.stones.held.is_empty());
                    hurled = Some(frame);
                }
            }
            if !combat.stones.held.is_empty() {
                held_frames += 1;
            }
            let mut drawn = Vec::new();
            combat.stone_instances(&view, &mut drawn);
            if hurled.is_some() && landed.is_none() {
                if let Some(rock) = combat.stones.rocks.first() {
                    let distance = (rock.position - view.eye).norm() / vpm;
                    let size = rock_size(rock.radius, distance);
                    // Kept ahead of all but what's right at the eye, so it's never left out
                    let key = (rock.position - view.eye).norm() * 0.02;
                    assert!(drawn.iter().any(|(k, _)| (*k - key).abs() < 1e-9));
                    assert!(key < 1.5 * vpm, "kept as if {:.1} m off", key / vpm);
                    if let Some(last) = last_size {
                        assert!(size > last * 0.97, "shrank from {last:.2} to {size:.2}");
                    }
                    last_size = Some(size);
                    // How many pixels across it is on a 1080 line screen with this field of view
                    let px = 2.0 * size * STONE_ROUND / distance / view.tan[1] * 540.0;
                    lowest_px = lowest_px.min(px);
                    assert!(rock.position.z > ground - 0.5 * vpm, "under the ground in flight");
                } else {
                    landed = Some(frame);
                }
            }
            if landed.is_some() && frame > landed.unwrap() + 30 {
                break;
            }
        }
        let hurled = hurled.expect("never hurled");
        let landed = landed.expect("never landed");
        assert!(held_frames >= 60, "held only {held_frames} frames before it was hurled");
        assert!((landed - hurled) as f64 * dt > 0.6, "landed {} s after it was hurled", (landed - hurled) as f64 * dt);
        assert!(lowest_px > 12.0, "only {lowest_px:.1} pixels across");
        assert!(!combat.stones.chunks.is_empty(), "it didn't shatter");
    }

    /// However far off, a flying rock's never drawn so small it's lost against the sky or the
    /// ground: at least a dozen pixels across for a hurled boulder on a 1080 line screen, a few
    /// for the smallest rock flung down out of a pillar, out to as far as anything's seen
    #[test]
    fn rocks_are_never_too_small_to_see() {
        let tan = 0.42;
        for (radius, least) in [(TORN_BOULDER, 12.0), (0.42, 5.0)] {
            for d in [2.0, 10.0, 30.0, 60.0, 120.0, 200.0] {
                let px = 2.0 * rock_size(radius, d) * STONE_ROUND / d / tan * 540.0;
                assert!(px >= least || d <= 2.0, "{radius} m round, {d} m off: {px:.1} pixels");
            }
            // Never smaller further off, so it never seems to pop away
            assert!((1..400).all(|k| rock_size(radius, k as f64 * 0.5 + 0.5) >= rock_size(radius, k as f64 * 0.5)));
        }
    }

    /// A slab comes up out of the ground and goes back down into it, never there all at once nor
    /// gone all at once: its top out of the ground only as far as it's come up
    #[test]
    fn slabs_come_and_go_through_the_ground() {
        let vpm = 16.0;
        let mut s = Slab { base: Vector3::new(0.0, 0.0, 100.0), size: 1.0, yaw: 0.3, lean: Vector3::x(), tilt: 0.4, delay: 0.1, age: 0.0, rise: 0.12, stand: 0.4, fall: 0.7, pillar: false, loud: false };
        let dt = 1.0 / 480.0;
        let mut tops = Vec::new();
        while s.up() < s.rise + s.stand + s.fall {
            s.age += dt;
            if s.up() >= 0.0 {
                tops.push(s.top(0.0, vpm).z - s.base.z);
            }
        }
        let step = SLAB_HEIGHT * vpm * 0.2;
        assert!(tops[0] < step, "first seen {:.2} m out of the ground", tops[0] / vpm);
        assert!(*tops.last().unwrap() < 0.1 * vpm, "last seen {:.2} m out of the ground", tops.last().unwrap() / vpm);
        assert!(tops.windows(2).all(|w| (w[1] - w[0]).abs() < step), "jumps");
        assert!(tops.iter().cloned().fold(0.0, f64::max) > 0.7 * SLAB_HEIGHT * vpm);
    }
}
