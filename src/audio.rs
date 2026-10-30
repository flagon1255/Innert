//! Sound: swings, bowstrings, spells and hits, beasts growling and bones rattling, footsteps on
//! whatever's underfoot, birds, and the wind and water all around. Every sound is made here when
//! the game starts, from noise and tones shaped and filtered, so there are no sound files. Sounds
//! out in the world are quieter further away and come from their side.

use std::f32::consts::{PI, TAU};

use nalgebra::Vector3;
use rodio::buffer::SamplesBuffer;
use rodio::source::ChannelVolume;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};

use crate::combat::View;

const RATE: u32 = 44100;
/// Versions of each footstep and birdsong, so they don't repeat exactly
const VARIANTS: usize = 4;

/// What's underfoot
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ground {
    Grass,
    Sand,
    Stone,
    Snow,
    Water,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sound {
    Swing,
    HeavySwing,
    /// A quick jab or lunge
    Stab,
    /// A flick of a wand
    Flick,
    Hit,
    /// A bowstring being drawn back
    Draw,
    Twang,
    ArrowHit,
    /// A spell cast, in the voice of its element, see Element::look()
    Cast(u8),
    Burst,
    /// Taking a weapon in hand
    Equip,
    Crackle,
    Chest,
    Pickup,
    Eat,
    Step(Ground),
    Bird,
    /// A boss roaring
    Roar,
    /// A boss getting ready to attack: a deep growl and rumble
    Rumble,
    /// A long gout of fire breathed out
    Breath,
    /// The player getting hurt
    Hurt,
    /// A boss beaten
    Fanfare,
    /// Something hitting water
    Splash,
    /// The ground shaking: a deep boom and a long rumble after it
    Quake,
    /// Something falling from high up, whistling down just before it lands
    Whistle,
    /// A heavy paw raking past
    Claw,
    /// Web shot out, wet and sticky
    Web,
    /// Old wood groaning as it bends
    Creak,
    /// Ice or stone cracking
    Crack,
    /// One beat of a great leathery wing
    Flap,
    /// A spider or a snake hissing
    Hiss,
    /// A wolf growling, low in its throat
    Growl,
    /// A wolf howling, as its pack spots you
    Howl,
    /// Jaws snapping shut
    Bite,
    /// Bones rattling
    Rattle,
    /// Metal ringing on metal: a sword on armor, or a parry
    Clang,
    /// Something wet and slimy squashing
    Squelch,
    /// Lots of little legs pattering
    Skitter,
    /// An attack rushing just past the ear
    Dodge,
}

/// A sound to play: where it comes from in the world, None for from the player, how loud, and
/// how high, 1 as it is and lower for something bigger
#[derive(Clone, Copy)]
pub struct Cue {
    pub sound: Sound,
    pub at: Option<Vector3<f64>>,
    pub volume: f32,
    pub pitch: f32,
}

impl Cue {
    pub fn here(sound: Sound, volume: f32) -> Cue {
        Cue { sound, at: None, volume, pitch: 1.0 }
    }

    pub fn at(sound: Sound, at: Vector3<f64>, volume: f32) -> Cue {
        Cue { sound, at: Some(at), volume, pitch: 1.0 }
    }

    pub fn pitched(mut self, pitch: f32) -> Cue {
        self.pitch = pitch;
        self
    }
}

pub struct Audio {
    // Has to be kept for the sound to keep playing
    _stream: OutputStream,
    handle: OutputStreamHandle,
    clips: Vec<(Sound, Vec<f32>)>,
    wind: Sink,
    water: Sink,
    /// Everything's this loud, 0-1
    pub volume: f32,
    bird_timer: f64,
    rng: u32,
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^ (x >> 16)
}

fn random(seed: &mut u32) -> f32 {
    *seed = hash(seed.wrapping_add(0x9e3779b9));
    (*seed >> 8) as f32 / 16777216.0
}

/// White noise, -1 to 1
fn noise(seed: &mut u32) -> f32 {
    random(seed) * 2.0 - 1.0
}

/// A one-pole low-pass filter, letting through what's below about `cutoff` Hz
struct Lowpass(f32);

impl Lowpass {
    fn step(&mut self, x: f32, cutoff: f32) -> f32 {
        let a = 1.0 - (-TAU * cutoff / RATE as f32).exp();
        self.0 += (x - self.0) * a;
        self.0
    }
}

/// A two-pole filter that rings at `frequency` Hz when struck, dying away over about `ring`
/// seconds, for the body of wood, stone and ice. Struck once with 1, it rings at about 1.
struct Resonator(f32, f32);

impl Resonator {
    fn step(&mut self, x: f32, frequency: f32, ring: f32) -> f32 {
        let w = TAU * frequency / RATE as f32;
        let r = (-1.0 / (ring * RATE as f32)).exp();
        let y = x * w.sin() + 2.0 * r * w.cos() * self.0 - r * r * self.1;
        self.1 = self.0;
        self.0 = y;
        y
    }
}

/// `seconds` of sound from `f`, given the time and the noise generator
fn render(seconds: f32, seed: u32, mut f: impl FnMut(f32, &mut u32) -> f32) -> Vec<f32> {
    let mut rng = hash(seed);
    (0..(seconds * RATE as f32) as usize).map(|i| f(i as f32 / RATE as f32, &mut rng)).collect()
}

/// Rises then falls over `length` seconds, smoothly
fn swell(t: f32, length: f32) -> f32 {
    (PI * (t / length).clamp(0.0, 1.0)).sin().powi(2)
}

/// Air rushing past a blade: noise swept through a band from `low` up to `high` Hz and back
fn whoosh(length: f32, low: f32, high: f32, seed: u32) -> Vec<f32> {
    let (mut a, mut b) = (Lowpass(0.0), Lowpass(0.0));
    render(length, seed, |t, rng| {
        let k = swell(t, length);
        let cutoff = low + (high - low) * k;
        let x = noise(rng);
        (a.step(x, cutoff) - b.step(x, cutoff * 0.25)) * k * 2.2
    })
}

/// A blade swung through the air: a whoosh, and under it the low hum of the air it pushes aside,
/// around `hum` Hz at its fastest, so it sounds like it has some weight behind it
fn swing(length: f32, low: f32, high: f32, hum: f32, seed: u32) -> Vec<f32> {
    let mut body = Resonator(0.0, 0.0);
    let push = render(length, seed ^ 0x5bd1e995, |t, rng| {
        let k = swell(t, length);
        body.step(noise(rng) * 0.05, hum * (0.7 + 0.3 * k), 0.008) * k * k
    });
    let mut air = whoosh(length, low, high, seed);
    for (a, p) in air.iter_mut().zip(push) {
        *a += p;
    }
    air
}

/// A blow landing: a hard crack at the very front, a slap of flesh or leather, and a deep thump
/// dropping from `thump` Hz, with a gritty crunch `crunch` of the way, 0 to 1, all pushed
/// together a little too hard so it lands with weight
fn hit(thump: f32, crunch: f32, seed: u32) -> Vec<f32> {
    let length = 0.28;
    let mut rng = hash(seed);
    let bright = 1600.0 + 1400.0 * random(&mut rng);
    let (mut lp, mut lp2, mut lp3, mut lp4) = (Lowpass(0.0), Lowpass(0.0), Lowpass(0.0), Lowpass(0.0));
    let (mut phase, mut grit) = (0.0f32, 0.0f32);
    render(length, seed, |t, rng| {
        let x = noise(rng);
        let crack = (x - lp.step(x, 3000.0)) * (-t * 400.0).exp() * 0.9;
        let slap = (lp2.step(x, bright) - lp3.step(x, 200.0)) * (-t * 30.0).exp() * 2.6;
        // Dropping quickly in pitch, like a drum
        phase += TAU * thump * (0.4 + 0.6 * (-t * 35.0).exp()) / RATE as f32;
        let body = phase.sin() * (-t * 16.0).exp();
        // Crunching only in the first moment
        grit = if random(rng) < 0.02 * crunch * (-t * 40.0).exp() { (-t * 30.0).exp() } else { grit * 0.9 };
        let y = noise(rng) * grit;
        let crunchy = (y - lp4.step(y, 1500.0)) * 0.8;
        // Squashed softly at the top: louder all through, but never past it
        ((crack + slap + body + crunchy) * 1.1).tanh() * 1.3 * ((length - t) * 40.0).min(1.0)
    })
}

/// Something bursting: a sharp crack, a deep boom sinking from `low` Hz, a roar of air rushing
/// out and dying down, and bits pattering down after
fn burst(low: f32, seed: u32) -> Vec<f32> {
    let length = 0.9;
    let (mut lp, mut lp2, mut lp3) = (Lowpass(0.0), Lowpass(0.0), Lowpass(0.0));
    let (mut phase, mut grit) = (0.0f32, 0.0f32);
    render(length, seed, |t, rng| {
        let x = noise(rng);
        let crack = (x - lp.step(x, 1200.0)) * (-t * 70.0).exp() * 0.7;
        phase += TAU * low * (0.55 + 0.45 * (-t * 10.0).exp()) / RATE as f32;
        let boom = (phase.sin() + 0.3 * (phase * 2.0).sin()) * (-t * 5.0).exp() * 0.6;
        let roar = lp2.step(x, 500.0 - 300.0 * t) * 1.9 * (-t * 4.5).exp();
        grit = if random(rng) < 0.003 * (-t * 3.0).exp() * (t * 10.0).min(1.0) { 0.5 + 0.5 * random(rng) } else { grit * 0.92 };
        let y = noise(rng) * grit;
        let debris = (y - lp3.step(y, 2000.0)) * 0.4;
        (crack + boom + roar + debris) * (t * 2000.0).min(1.0) * ((length - t) * 20.0).min(1.0)
    })
}

/// A plucked string, by Karplus-Strong: a burst of noise going round a delay that loses a little
/// each time
fn pluck(frequency: f32, length: f32, damping: f32, seed: u32) -> Vec<f32> {
    let mut rng = hash(seed);
    let period = (RATE as f32 / frequency) as usize;
    let mut ring: Vec<f32> = (0..period).map(|_| noise(&mut rng)).collect();
    let mut out = Vec::new();
    let mut i = 0;
    for _ in 0..(length * RATE as f32) as usize {
        let next = (i + 1) % period;
        let v = (ring[i] + ring[next]) * 0.5 * damping;
        out.push(ring[i]);
        ring[i] = v;
        i = next;
    }
    out
}

/// A spell being cast, in the voice of its element: none, then fire, frost, lightning, poison,
/// arcane, void, blood and holy, as in Element::look()
fn cast(look: u8) -> Vec<f32> {
    let length = 0.5;
    let seed = 40 + look as u32;
    let mut lp = Lowpass(0.0);
    let mut lp2 = Lowpass(0.0);
    let mut pop = 0.0f32;
    let mut pop_frequency = 0.0f32;
    let mut pop_time = 0.0f32;
    render(length, seed, |t, rng| {
        let k = swell(t, length);
        let rising = 300.0 * (4.0f32).powf((t / length).min(1.0));
        let air = lp.step(noise(rng), 2500.0) * 0.4;
        let sound = match look {
            // A roar of flame
            1 => {
                let x = noise(rng);
                (lp2.step(x, 300.0 + 1500.0 * t) * 2.5 + if random(rng) < 0.002 { 0.8 } else { 0.0 }) * 0.8
            }
            // Chiming ice
            2 => [1.0f32, 1.5, 2.25].iter().map(|m| (TAU * 1600.0 * m * t).sin()).sum::<f32>() * 0.18 * (1.0 + 0.5 * (TAU * 9.0 * t).sin()) + air * 0.6,
            // A crackling zap
            3 => {
                let buzz = ((TAU * 110.0 * t).sin()).signum() * 0.25;
                buzz + if random(rng) < 0.01 { noise(rng) * 1.5 } else { 0.0 }
            }
            // Bubbling
            4 => {
                if pop_time <= 0.0 && random(rng) < 0.002 {
                    pop_time = 0.04;
                    pop_frequency = 300.0 + 700.0 * random(rng);
                }
                pop_time -= 1.0 / RATE as f32;
                pop += TAU * (pop_frequency + 4000.0 * (0.04 - pop_time.max(0.0))) / RATE as f32;
                pop.sin() * if pop_time > 0.0 { 0.5 } else { 0.0 } + air * 0.4
            }
            // A deep hum falling away
            6 => {
                let f = 200.0 * (0.3f32).powf((t / length).min(1.0));
                ((TAU * f * t).sin() + 0.5 * (TAU * f * 2.01 * t).sin()) * 0.4 + air * 0.5
            }
            // A wet rush and a thump
            7 => lp2.step(noise(rng), 500.0) * 2.0 + (TAU * 70.0 * t).sin() * (-t * 10.0).exp() * 0.6,
            // A bright chord
            8 => [523.0f32, 659.0, 784.0, 1046.0].iter().map(|f| (TAU * f * t).sin()).sum::<f32>() * 0.14 + air * 0.4,
            // Shimmering tones sweeping up, over a rush of air
            _ => {
                let tremolo = if look == 5 { 0.7 } else { 0.4 };
                ((TAU * rising * t).sin() + (TAU * rising * 1.012 * t).sin()) * 0.3 * (1.0 + tremolo * (TAU * 32.0 * t).sin()) + air
            }
        };
        sound * k
    })
}

/// A footstep on `ground`
fn step(ground: Ground, seed: u32) -> Vec<f32> {
    let mut lp = Lowpass(0.0);
    let mut lp2 = Lowpass(0.0);
    match ground {
        Ground::Grass => render(0.14, seed, |t, rng| lp.step(noise(rng), 900.0) * (-t * 30.0).exp() * 1.6),
        Ground::Sand => render(0.16, seed, |t, rng| {
            let x = noise(rng);
            (lp.step(x, 5000.0) - lp2.step(x, 800.0)) * (-t * 22.0).exp() * 0.9
        }),
        Ground::Stone => render(0.1, seed, |t, rng| {
            let x = noise(rng);
            ((x - lp.step(x, 1500.0)) * (-t * 90.0).exp() + (TAU * 260.0 * t).sin() * (-t * 50.0).exp() * 0.5) * 0.8
        }),
        Ground::Snow => {
            // Lots of little crunches
            let grains: Vec<f32> = render(0.2, seed ^ 7, |_, rng| random(rng));
            render(0.2, seed, |t, rng| {
                let i = (t * RATE as f32) as usize;
                let crunch = if grains[i] < 0.02 { 1.0 } else { 0.25 };
                lp.step(noise(rng) * crunch, 3500.0) * (-t * 14.0).exp() * 1.3
            })
        }
        Ground::Water => render(0.22, seed, |t, rng| {
            let bubble = (TAU * (300.0 + 600.0 * t) * t).sin() * (-t * 18.0).exp() * 0.1;
            (lp.step(noise(rng), 1100.0) * (-t * 14.0).exp() * (t * 200.0).min(1.0) + bubble) * 0.9
        }),
    }
}

/// A bird calling: a few quick whistled notes, sliding
fn birdsong(seed: u32) -> Vec<f32> {
    let mut rng = hash(seed);
    let notes = 2 + (random(&mut rng) * 4.0) as usize;
    let base = 2200.0 + random(&mut rng) * 1800.0;
    let slide = (random(&mut rng) - 0.4) * 2500.0;
    let length = notes as f32 * 0.11;
    let mut phase = 0.0;
    render(length, seed, |t, _| {
        let note = (t / 0.11) as usize;
        let within = (t % 0.11) / 0.09;
        if within > 1.0 {
            return 0.0;
        }
        let frequency = base + slide * within + if note % 2 == 1 { 300.0 } else { 0.0 };
        phase += TAU * frequency / RATE as f32;
        phase.sin() * (PI * within).sin().powi(2) * 0.5
    })
}

/// The ground shaking: a thud, a deep boom sinking to `low` Hz, a long rolling rumble under it,
/// and stones rattling as the ground breaks
fn quake(low: f32, seed: u32) -> Vec<f32> {
    let length = 1.2;
    let (mut lp, mut lp2, mut lp3, mut lp4) = (Lowpass(0.0), Lowpass(0.0), Lowpass(0.0), Lowpass(0.0));
    let (mut phase, mut rattle) = (0.0f32, 0.0f32);
    render(length, seed, |t, rng| {
        phase += TAU * (low + 50.0 * (-t * 7.0).exp()) / RATE as f32;
        let boom = (phase.sin() + 0.35 * (phase * 2.0).sin()) * (-t * 3.5).exp();
        let thud = lp.step(noise(rng), 600.0) * (-t * 25.0).exp() * 1.5;
        // Rolling on, louder and softer
        let roll = 0.75 + 0.25 * (TAU * (low / 8.0) * t).sin() * (TAU * 1.7 * t).sin();
        let rumble = lp3.step(lp2.step(noise(rng), 110.0), 110.0) * 7.5 * roll * (-t * 2.2).exp();
        rattle = if random(rng) < 0.003 * (-t * 4.0).exp() { 1.0 } else { rattle * 0.95 };
        let x = noise(rng) * rattle;
        let stones = (x - lp4.step(x, 1200.0)) * 0.35;
        (boom * 0.8 + thud + rumble + stones) * (t * 150.0).min(1.0) * (1.0 - (t / length).powi(3))
    })
}

/// Something falling from high up: a whistle sliding down from `high` Hz, tumbling, louder as it
/// comes, over the rush of air around it, and cut short as it lands
fn whistle(high: f32, seed: u32) -> Vec<f32> {
    let length = 0.8;
    let mut rng = hash(seed);
    let tumble = 9.0 + 8.0 * random(&mut rng);
    let (mut lp, mut lp2) = (Lowpass(0.0), Lowpass(0.0));
    let mut phase = 0.0f32;
    render(length, seed, |t, rng| {
        let u = t / length;
        let f = high * (0.33f32).powf(u);
        phase += TAU * f / RATE as f32;
        let tone = (phase.sin() + 0.12 * (phase * 2.0).sin()) * 0.35 * (1.0 + 0.15 * (TAU * tumble * t).sin());
        let x = noise(rng);
        let air = (lp.step(x, f * 1.4) - lp2.step(x, f * 0.6)) * 0.9;
        (tone + air) * u.powf(1.5) * ((1.0 - u) * 25.0).min(1.0)
    })
}

/// A heavy paw raking past: a short low rush of air, torn ragged by three claws `gap` seconds
/// apart, and a meaty thud as it lands
fn claw(gap: f32, seed: u32) -> Vec<f32> {
    let length = 0.25;
    let (mut a, mut b, mut c, mut d) = (Lowpass(0.0), Lowpass(0.0), Lowpass(0.0), Lowpass(0.0));
    let (mut rasp, mut phase) = (0.0f32, 0.0f32);
    render(length, seed, |t, rng| {
        // Quick to come, slower to go
        let k = (t * 30.0).min(1.0) * (-t * 10.0).exp() * (1.0 - t / length);
        let cutoff = 1800.0 - 1300.0 * t / length;
        let x = noise(rng);
        let rush = (a.step(x, cutoff) - b.step(x, cutoff * 0.2)) * 2.4 * k;
        // A rough random flutter, roughest as each claw goes by
        if random(rng) < 0.05 {
            rasp = random(rng);
        }
        let claws: f32 = (0..3).map(|i| t - 0.04 - gap * i as f32).filter(|at| *at > 0.0).map(|at| (-at * 70.0).exp()).sum();
        let tear = (c.step(x, 4000.0) - d.step(x, 1000.0)) * rasp * claws * 1.4;
        phase += TAU * (110.0 - 50.0 * t / length) / RATE as f32;
        let thud = phase.sin() * (-((t - 0.06) * 40.0).powi(2)).exp() * 0.6;
        rush * (0.6 + 0.4 * rasp) + tear + thud
    })
}

/// Web shot out: a wet slap, a tone flicking down from `high` Hz through a spray of air, then
/// sticky little pops as it pulls tight
fn web(high: f32, seed: u32) -> Vec<f32> {
    let length = 0.3;
    let (mut lp, mut lp2, mut lp3) = (Lowpass(0.0), Lowpass(0.0), Lowpass(0.0));
    let mut phase = 0.0f32;
    let (mut pop, mut pop_phase, mut pop_frequency) = (0.0f32, 0.0f32, 0.0f32);
    render(length, seed, |t, rng| {
        let f = 250.0 + high * (-t * 30.0).exp();
        phase += TAU * f / RATE as f32;
        let thwip = phase.sin() * (t * 400.0).min(1.0) * (-t * 22.0).exp() * 0.5;
        let x = noise(rng);
        let spray = (lp.step(x, 700.0 + 6000.0 * (-t * 12.0).exp()) - lp2.step(x, 300.0)) * (t * 300.0).min(1.0) * (-t * 16.0).exp();
        let slap = lp3.step(x, 400.0) * (t * 300.0).min(1.0) * (-t * 30.0).exp();
        if random(rng) < 0.002 * swell(t - 0.04, 0.26) {
            pop = 1.0;
            pop_frequency = 900.0 + 1300.0 * random(rng);
        }
        pop *= 0.996;
        // Each pop sliding up as it goes
        pop_phase += TAU * pop_frequency * (3.0 - 2.0 * pop) / RATE as f32;
        thwip + spray + slap + pop_phase.sin() * pop * 0.35
    })
}

/// Old wood groaning as it bends: the grain catching and slipping, slowly then faster, each slip
/// ringing through the wood around `body` Hz
fn creak(body: f32, seed: u32) -> Vec<f32> {
    let length = 0.9;
    let (mut low, mut mid, mut high) = (Resonator(0.0, 0.0), Resonator(0.0, 0.0), Resonator(0.0, 0.0));
    let mut lp = Lowpass(0.0);
    let mut wait = 0.0f32;
    render(length, seed, |t, rng| {
        let rate = 16.0 + 70.0 * swell(t, length) + 8.0 * (TAU * 3.0 * t).sin();
        wait -= 1.0 / RATE as f32;
        let slip = if wait <= 0.0 {
            wait = (0.75 + 0.5 * random(rng)) / rate;
            0.5 + 0.5 * random(rng)
        } else {
            0.0
        };
        let wood = low.step(slip, body, 0.025) + mid.step(slip, body * 2.3, 0.015) * 0.7 + high.step(slip, body * 5.1, 0.006) * 0.4;
        let rub = lp.step(noise(rng), 900.0) * 0.4;
        (wood * 0.5 + rub) * swell(t, length).sqrt()
    })
}

/// Ice or stone giving way: a sharp snap, smaller cracks running on after it, a short ring
/// around `ring` Hz and a dull thud
fn crack(ring: f32, seed: u32) -> Vec<f32> {
    let length = 0.3;
    let (mut lp, mut lp2) = (Lowpass(0.0), Lowpass(0.0));
    let (mut bright, mut brighter, mut body) = (Resonator(0.0, 0.0), Resonator(0.0, 0.0), Resonator(0.0, 0.0));
    let (mut splinter, mut strike) = (0.0f32, 1.0f32);
    render(length, seed, |t, rng| {
        let x = noise(rng);
        let snap = (x - lp.step(x, 2000.0)) * (-t * 150.0).exp() * 0.8;
        if random(rng) < 0.003 * (-t * 12.0).exp() {
            splinter = 0.6 + 0.4 * random(rng);
            strike = splinter * 0.2;
        }
        splinter *= 0.9;
        let y = noise(rng) * splinter;
        let splinters = (y - lp2.step(y, 2500.0)) * 0.6;
        let rings = bright.step(strike, ring, 0.06) * 0.4 + brighter.step(strike, ring * 1.63, 0.04) * 0.25;
        let thud = body.step(strike, 150.0, 0.04) * 0.45;
        strike = 0.0;
        (snap + splinters + rings + thud) * (1.0 - t / length)
    })
}

/// One beat of a great leathery wing: air pushed down in a low whump around `low` Hz, the skin
/// fluttering as it pulls taut at the bottom of the stroke
fn flap(low: f32, seed: u32) -> Vec<f32> {
    let length = 0.35;
    let (mut lp, mut lp2, mut lp3, mut lp4) = (Lowpass(0.0), Lowpass(0.0), Lowpass(0.0), Lowpass(0.0));
    render(length, seed, |t, rng| {
        let push = (t / 0.12).min(1.0).powi(2) * (-(t - 0.12).max(0.0) * 14.0).exp() * (1.0 - t / length);
        let x = noise(rng);
        let air = (lp.step(x, 200.0 + 500.0 * push) - lp2.step(x, 60.0)) * 3.0 * push;
        let bottom = (-((t - 0.12) * 25.0).powi(2)).exp();
        let whump = (TAU * low * t).sin() * bottom * 0.8;
        let flutter = (0.5 + 0.5 * (TAU * 38.0 * t).sin()).powi(4);
        let skin = (lp3.step(x, 2500.0) - lp4.step(x, 600.0)) * flutter * (-((t - 0.13) * 33.0).powi(2)).exp();
        air + whump + skin
    })
}

/// A hiss through fangs: bright noise, quick to start and slow to fade, after a little spit,
/// shaking `rattle` times a second, or smooth for 0
fn hiss(rattle: f32, seed: u32) -> Vec<f32> {
    let length = 0.6;
    let (mut lp, mut lp2, mut lp3) = (Lowpass(0.0), Lowpass(0.0), Lowpass(0.0));
    render(length, seed, |t, rng| {
        let x = noise(rng);
        let sibilant = lp.step(x, 9000.0) - lp2.step(x, 3200.0);
        let throat = lp3.step(x, 2000.0) * (0.25 + 1.2 * (-t * 40.0).exp());
        let shake = 1.0 + 0.35 * (TAU * rattle * t).sin();
        (sibilant + throat) * shake * (t / 0.02).min(1.0) * (1.0 - (t / length).powi(2))
    })
}

/// A wolf growling low in its throat around `low` Hz: the breath catching unevenly, every other
/// catch weaker, so it comes out rough, and rolling louder and softer as it goes, with the lips
/// drawn back `bared` of the way, 0 to 1, for a snarl
fn growl(low: f32, bared: f32, seed: u32) -> Vec<f32> {
    let length = 0.8;
    let (mut chest, mut throat, mut mouth) = (Resonator(0.0, 0.0), Resonator(0.0, 0.0), Resonator(0.0, 0.0));
    let (mut lp, mut lp2, mut lp3) = (Lowpass(0.5), Lowpass(0.0), Lowpass(0.0));
    let (mut phase, mut jitter, mut weak, mut roll) = (0.0f32, 1.0f32, false, 0.5f32);
    render(length, seed, |t, rng| {
        phase += low * jitter * (1.0 + 0.08 * swell(t, length) + 0.04 * (TAU * 1.7 * t).sin()) / RATE as f32;
        let catch = if phase >= 1.0 {
            phase -= 1.0;
            jitter = 0.85 + 0.3 * random(rng);
            weak = !weak;
            (0.6 + 0.4 * random(rng)) * if weak { 0.45 } else { 1.0 }
        } else {
            0.0
        };
        // Wandering louder and softer
        if random(rng) < 40.0 / RATE as f32 {
            roll = random(rng);
        }
        let rolling = 0.25 + 0.75 * lp.step(roll, 20.0);
        let voice = chest.step(catch, 2.0 * low, 0.02) * 0.6
            + throat.step(catch, 420.0 + 200.0 * bared, 0.009)
            + mouth.step(catch, 1100.0 + 700.0 * bared, 0.004) * (0.4 + 0.5 * bared);
        let x = noise(rng);
        let breath = (lp2.step(x, 2500.0 + 4000.0 * bared) - lp3.step(x, 400.0)) * (0.3 + 0.5 * bared);
        (voice * 0.9 + breath) * rolling * (t / 0.06).min(1.0) * (1.0 - (t / length).powi(3))
    })
}

/// A wolf howling: sliding up to `high` Hz, held there wavering, then sinking away, brighter at
/// the top, and coming back softer off the trees a moment later
fn howl(high: f32, seed: u32) -> Vec<f32> {
    let length = 1.6;
    // The voice stops a little before the end, leaving the echo
    let voice = 1.4;
    let mut rng = hash(seed);
    let top = 0.22 + 0.1 * random(&mut rng);
    let waver = 5.0 + 1.5 * random(&mut rng);
    let (mut lp, mut lp2, mut echo) = (Lowpass(0.0), Lowpass(0.0), Lowpass(0.0));
    let mut phase = 0.0f32;
    let mut samples = render(length, seed, |t, rng| {
        let u = (t / voice).min(1.0);
        let up = (u / top).min(1.0);
        let up = up * up * (3.0 - 2.0 * up);
        let down = ((u - 0.6) / 0.4).max(0.0).powi(2);
        let f = high * (0.6 + 0.4 * up - 0.35 * down) * (1.0 + 0.02 * up * (TAU * waver * t).sin());
        phase += TAU * f / RATE as f32;
        let open = up * (1.0 - down);
        let tone = phase.sin() + 0.3 * open * (phase * 2.0).sin() + 0.1 * open * (phase * 3.0).sin();
        let x = noise(rng);
        let breath = (lp.step(x, f * 3.0) - lp2.step(x, f)) * (0.3 + 0.7 * open);
        (tone * 0.6 + breath * 0.3) * (u / 0.08).min(1.0).powi(2) * (1.0 - ((u - 0.75) / 0.25).max(0.0).powi(2))
    });
    // Coming back off the trees, duller, and dying out right at the end
    let delay = (0.14 * RATE as f32) as usize;
    let fade = 0.05 * RATE as f32;
    let n = samples.len();
    for i in 0..n {
        if i >= delay {
            samples[i] += echo.step(samples[i - delay], 1800.0) * 0.25;
        }
        samples[i] *= ((n - i) as f32 / fade).min(1.0);
    }
    samples
}

/// Jaws snapping shut: a hard clack of teeth ringing around `clack` Hz over the dull chomp of the
/// jaw, then a short ragged wet tear as it pulls away
fn bite(clack: f32, seed: u32) -> Vec<f32> {
    let length = 0.2;
    let (mut lp, mut lp2, mut lp3, mut lp4) = (Lowpass(0.0), Lowpass(0.0), Lowpass(0.0), Lowpass(0.0));
    let (mut tooth, mut tooth2) = (Resonator(0.0, 0.0), Resonator(0.0, 0.0));
    let (mut phase, mut strike, mut rasp) = (0.0f32, 1.0f32, 0.0f32);
    render(length, seed, |t, rng| {
        let x = noise(rng);
        let click = (x - lp.step(x, 2500.0)) * (-t * 400.0).exp() * 0.7;
        let teeth = tooth.step(strike, clack, 0.012) * 0.45 + tooth2.step(strike, clack * 1.47, 0.007) * 0.25;
        strike = 0.0;
        phase += TAU * (90.0 + 160.0 * (-t * 40.0).exp()) / RATE as f32;
        let chomp = phase.sin() * (-t * 35.0).exp() * 0.6;
        // Ripping: ragged, louder and softer at random
        if random(rng) < 0.008 {
            rasp = random(rng).powi(2);
        }
        let at = (t - 0.03).max(0.0);
        let pull = (at * 200.0).min(1.0) * (-at * 35.0).exp();
        let tear = (lp2.step(x, 4500.0) - lp3.step(x, 900.0)) * rasp * pull * 2.2;
        let wet = lp4.step(x, 600.0) * pull * 1.2;
        (click + teeth + chomp + tear + wet) * (1.0 - t / length)
    })
}

/// Bones rattling: a clatter of small dry knocks, thick at first and then in two smaller shakes,
/// each bone ringing only for a moment, around `bone` Hz or higher
fn rattle(bone: f32, seed: u32) -> Vec<f32> {
    let length = 0.5;
    let mut rng = hash(seed);
    let shakes = [0.17 + 0.05 * random(&mut rng), 0.3 + 0.05 * random(&mut rng)];
    // Bigger bones lower and ringing longer
    let (sizes, rings) = ([1.0, 1.7, 2.6, 3.9], [0.012, 0.008, 0.005, 0.004]);
    let mut bones = [Resonator(0.0, 0.0), Resonator(0.0, 0.0), Resonator(0.0, 0.0), Resonator(0.0, 0.0)];
    let (mut pitches, mut struck) = ([bone; 4], [0.0f32; 4]);
    let mut lp = Lowpass(0.0);
    let mut click = 0.0f32;
    render(length, seed, |t, rng| {
        let busy = (t / 0.01).min(1.0) * (-t * 9.0).exp()
            + 0.6 * (-((t - shakes[0]) / 0.04).powi(2)).exp()
            + 0.35 * (-((t - shakes[1]) / 0.04).powi(2)).exp();
        if random(rng) < 300.0 * busy / RATE as f32 {
            // A different bone each time, no two quite the same
            let i = (random(rng) * 4.0) as usize % 4;
            pitches[i] = bone * sizes[i] * (0.8 + 0.4 * random(rng));
            struck[i] = 0.4 + 0.6 * random(rng);
            click = struck[i];
        }
        let mut knocks = 0.0;
        for i in 0..4 {
            knocks += bones[i].step(struck[i], pitches[i], rings[i]);
            struck[i] = 0.0;
        }
        let x = noise(rng) * click;
        click *= 0.85;
        let tick = (x - lp.step(x, 3000.0)) * 0.5;
        (knocks * 0.55 + tick) * (1.0 - (t / length).powi(4))
    })
}

/// Metal struck on metal: a hard bright tick, then a blade ringing around `ring` Hz with
/// overtones out of tune with it, each beating against a twin a hair off, the highest fading first
fn clang(ring: f32, seed: u32) -> Vec<f32> {
    let length = 0.6;
    let mut rng = hash(seed);
    // How far above the lowest, how loud and how fast it fades
    let mut partials = Vec::new();
    for (m, a, d) in [(1.0f32, 0.4f32, 4.0f32), (2.76, 0.45, 6.0), (5.4, 0.4, 8.0), (8.93, 0.3, 11.0), (13.34, 0.2, 16.0)] {
        let twin = 1.004 + 0.006 * random(&mut rng);
        partials.push((m, a * 0.6, d));
        partials.push((m * twin, a * 0.4, d * 1.2));
    }
    let (mut lp, mut lp2) = (Lowpass(0.0), Lowpass(0.0));
    render(length, seed, |t, rng| {
        let x = noise(rng);
        let tick = (x - lp.step(x, 4000.0)) * (-t * 700.0).exp() * 0.7;
        let knock = lp2.step(x, 900.0) * (-t * 60.0).exp() * 0.4;
        let ringing: f32 = partials.iter().map(|(m, a, d)| (TAU * ring * m * t).sin() * a * (-t * d).exp()).sum();
        (tick + knock + ringing * 1.5) * 0.6 * ((length - t) * 15.0).min(1.0)
    })
}

/// Something soft and wet squashing and pulling free: a slap, a low give around `body` Hz, a
/// gurgling squish sliding down then back up, and a bubble or two popping
fn squelch(body: f32, seed: u32) -> Vec<f32> {
    let length = 0.35;
    let (mut lp, mut lp2) = (Lowpass(0.0), Lowpass(0.5));
    let mut squish = Resonator(0.0, 0.0);
    let (mut phase, mut wobble) = (0.0f32, 0.5f32);
    let (mut pop, mut pop_phase, mut pop_frequency) = (0.0f32, 0.0f32, 0.0f32);
    render(length, seed, |t, rng| {
        let x = noise(rng);
        let slap = lp.step(x, 900.0) * (t * 500.0).min(1.0) * (-t * 30.0).exp() * 1.6;
        phase += TAU * body * (1.0 + 0.8 * (-t * 25.0).exp()) / RATE as f32;
        let give = phase.sin() * (-t * 14.0).exp() * 0.7;
        // Sliding down as it squashes and up as it pulls free, gurgling unevenly
        if random(rng) < 120.0 / RATE as f32 {
            wobble = random(rng);
        }
        let gurgle = 0.7 + 0.6 * lp2.step(wobble, 60.0);
        let f = (250.0 + 1100.0 * (-t * 16.0).exp() + 900.0 * ((t - 0.17) / 0.18).clamp(0.0, 1.0).powi(2)) * gurgle;
        let wet = squish.step(x * 0.1, f, 0.005) * swell(t, length).sqrt();
        if random(rng) < 0.003 * swell(t - 0.05, 0.3) {
            pop = 1.0;
            pop_frequency = 500.0 + 900.0 * random(rng);
        }
        pop *= 0.995;
        // Each bubble sliding up as it pops
        pop_phase += TAU * pop_frequency * (2.5 - 1.5 * pop) / RATE as f32;
        (slap + give + wet + pop_phase.sin() * pop * 0.4) * 0.8 * ((length - t) * 30.0).min(1.0)
    })
}

/// Lots of little legs pattering past: tiny hard taps, each ringing for a moment around `tap` Hz
/// or higher, coming in quick runs as the legs move in turn, over a faint scratching
fn skitter(tap: f32, seed: u32) -> Vec<f32> {
    let length = 0.4;
    let mut rng = hash(seed);
    let gait = 10.0 + 6.0 * random(&mut rng);
    let mut shells = [Resonator(0.0, 0.0), Resonator(0.0, 0.0), Resonator(0.0, 0.0)];
    let mut struck = [0.0f32; 3];
    let (mut lp, mut lp2, mut lp3) = (Lowpass(0.0), Lowpass(0.0), Lowpass(0.0));
    let mut click = 0.0f32;
    render(length, seed, |t, rng| {
        let k = swell(t, length).sqrt();
        // A few legs coming down at each step
        let busy = (0.5 + 0.5 * (TAU * gait * t).sin()).powi(3) * k;
        if random(rng) < 600.0 * busy / RATE as f32 {
            let leg = (random(rng) * 3.0) as usize % 3;
            struck[leg] = 0.3 + 0.7 * random(rng);
            click = struck[leg];
        }
        let mut taps = 0.0;
        for (i, m) in [1.0, 1.45, 2.1].into_iter().enumerate() {
            taps += shells[i].step(struck[i], tap * m, 0.002);
            struck[i] = 0.0;
        }
        let x = noise(rng);
        let y = x * click;
        click *= 0.8;
        let tick = (y - lp.step(y, 5000.0)) * 0.6;
        let scratch = (lp2.step(x, 8000.0) - lp3.step(x, 2500.0)) * 0.08 * k;
        (taps * 0.8 + tick + scratch) * ((length - t) * 40.0).min(1.0)
    })
}

/// Something rushing past close by the ear: air rising to a sharp hiss as it comes, up around
/// `high` Hz, dropping lower as it goes by, and loudest as it passes
fn dodge(high: f32, seed: u32) -> Vec<f32> {
    let length = 0.3;
    let (mut a, mut b) = (Lowpass(0.0), Lowpass(0.0));
    render(length, seed, |t, rng| {
        // Nearest a tenth of a second in
        let s = (t - 0.1) / 0.03;
        let near = 1.0 / (1.0 + s * s);
        let cutoff = high * (0.6 - 0.4 * (s * 1.2).tanh());
        let x = noise(rng);
        (a.step(x, cutoff) - b.step(x, cutoff * 0.3)) * 2.6 * near * (t * 100.0).min(1.0) * ((length - t) * 30.0).min(1.0)
    })
}

/// `seconds` of sound that loops without a seam: the end fades into the start
fn looped(seconds: f32, seed: u32, f: impl FnMut(f32, &mut u32) -> f32) -> Vec<f32> {
    let fade = (0.5 * RATE as f32) as usize;
    let mut samples = render(seconds + 0.5, seed, f);
    let length = samples.len() - fade;
    for i in 0..fade {
        let k = i as f32 / fade as f32;
        samples[i] = samples[i] * k + samples[length + i] * (1.0 - k);
    }
    samples.truncate(length);
    samples
}

fn make_clips() -> Vec<(Sound, Vec<f32>)> {
    let mut clips = vec![
        (Sound::Swing, swing(0.28, 400.0, 2600.0, 230.0, 1)),
        (Sound::Swing, swing(0.26, 500.0, 3000.0, 260.0, 2)),
        (Sound::Swing, swing(0.3, 350.0, 2300.0, 200.0, 160)),
        (Sound::HeavySwing, swing(0.42, 250.0, 1500.0, 120.0, 3)),
        (Sound::HeavySwing, swing(0.45, 220.0, 1300.0, 105.0, 161)),
    ];
    clips.push((Sound::Hit, hit(150.0, 0.0, 4)));
    clips.push((Sound::Hit, hit(128.0, 0.5, 162)));
    clips.push((Sound::Hit, hit(170.0, 0.2, 163)));
    clips.push((Sound::Hit, hit(115.0, 0.8, 164)));
    clips.push((Sound::Hit, hit(140.0, 0.3, 165)));
    let mut string = pluck(98.0, 0.7, 0.996, 5);
    for (s, w) in string.iter_mut().zip(whoosh(0.2, 600.0, 3000.0, 6).iter().chain(std::iter::repeat(&0.0))) {
        *s = *s * 0.6 + w * 0.5;
    }
    clips.push((Sound::Twang, string));
    let mut lp = Lowpass(0.0);
    clips.push((Sound::ArrowHit, render(0.2, 7, |t, rng| lp.step(noise(rng), 900.0) * (-t * 35.0).exp() * 1.8 + (TAU * 180.0 * t).sin() * (-t * 30.0).exp() * 0.5)));
    for look in 0..9u8 {
        clips.push((Sound::Cast(look), cast(look)));
    }
    let mut lp = Lowpass(0.0);
    clips.push((
        Sound::Splash,
        render(0.6, 55, |t, rng| {
            // A slap on the water, a rush of spray, and a bubble or two
            let slap = lp.step(noise(rng), 1800.0 - 1500.0 * (t / 0.6)) * 2.0 * (-t * 7.0).exp() * (t * 300.0).min(1.0);
            let bubble = (TAU * (380.0 + 900.0 * t) * t).sin() * 0.25 * (-((t - 0.18) * 30.0).powi(2)).exp();
            slap + bubble
        }),
    ));
    let (mut lp, mut lp2) = (Lowpass(0.0), Lowpass(0.0));
    clips.push((
        Sound::Roar,
        render(1.6, 50, |t, rng| {
            // A growling buzz, wavering, through a rasp of breath
            let f = 85.0 + 25.0 * (t / 1.6) * (1.0 - t / 1.6) * 4.0 + 6.0 * (TAU * 7.0 * t).sin();
            let buzz = lp.step(((f * t).fract() * 2.0 - 1.0) + 0.5 * ((f * 1.5 * t).fract() * 2.0 - 1.0), 700.0);
            let rasp = lp2.step(noise(rng), 1200.0);
            (buzz * 1.1 + rasp * 0.9) * swell(t, 1.6).sqrt()
        }),
    ));
    let mut lp = Lowpass(0.0);
    clips.push((
        Sound::Rumble,
        render(0.9, 51, |t, rng| {
            let growl = ((TAU * (55.0 + 20.0 * t) * t).sin() + 0.4 * (TAU * 110.0 * t).sin()) * 0.5;
            (growl + lp.step(noise(rng), 150.0) * 2.0) * swell(t, 0.9)
        }),
    ));
    let (mut lp, mut crackle) = (Lowpass(0.0), 0.0f32);
    clips.push((
        Sound::Breath,
        render(1.9, 52, |t, rng| {
            // A roaring rush of flame, crackling
            crackle = if random(rng) < 0.004 { 1.0 } else { crackle * 0.9 };
            let roar = lp.step(noise(rng), 500.0 + 900.0 * swell(t, 1.9));
            (roar * 2.2 + noise(rng) * crackle * 0.5) * swell(t, 1.9).sqrt()
        }),
    ));
    let mut lp = Lowpass(0.0);
    clips.push((
        Sound::Hurt,
        render(0.28, 53, |t, rng| {
            // A thump and a grunt
            let thump = (TAU * (95.0 - 120.0 * t) * t).sin() * (-t * 14.0).exp();
            let grunt = lp.step(((150.0 * t).fract() * 2.0 - 1.0) + noise(rng) * 0.4, 900.0) * swell(t, 0.28);
            thump * 0.9 + grunt * 0.6
        }),
    ));
    clips.push((
        Sound::Fanfare,
        render(2.2, 54, |t, _| {
            // Rising notes, then a held chord
            let notes = [(0.0, 523.25), (0.18, 659.25), (0.36, 783.99), (0.54, 1046.5)];
            let mut sum = 0.0;
            for (start, f) in notes {
                if t >= start {
                    let age = t - start;
                    let held = if start >= 0.5 { (-age * 1.2).exp() } else { (-age * 5.0).exp() * 0.7 + (-age * 1.2).exp() * 0.3 };
                    sum += ((TAU * f * t).sin() + 0.3 * (TAU * f * 2.0 * t).sin()) * held;
                }
            }
            sum * 0.22 * (1.0 - (t / 2.2).powi(4))
        }),
    ));
    clips.push((Sound::Stab, whoosh(0.14, 900.0, 4200.0, 30)));
    clips.push((Sound::Stab, whoosh(0.16, 700.0, 3600.0, 31)));
    let mut lp = Lowpass(0.0);
    clips.push((
        Sound::Flick,
        render(0.25, 32, |t, rng| {
            let sparkle = (TAU * (1800.0 + 2400.0 * t) * t).sin() * 0.25 * (1.0 - t / 0.25);
            (lp.step(noise(rng), 3000.0) * 0.8 + sparkle) * swell(t, 0.25)
        }),
    ));
    let mut lp = Lowpass(0.0);
    clips.push((
        Sound::Draw,
        render(0.55, 33, |t, _| {
            // The bow creaking as it bends
            let f = 70.0 + 30.0 * (t / 0.55) + 5.0 * (TAU * 13.0 * t).sin();
            lp.step((f * t).fract() * 2.0 - 1.0, 500.0) * swell(t, 0.55) * 0.5
        }),
    ));
    let mut lp = Lowpass(0.0);
    clips.push((
        Sound::Equip,
        render(0.35, 34, |t, rng| {
            // Steel sliding out, then a ring
            let x = noise(rng);
            let scrape = (x - lp.step(x, 2500.0)) * (t * 8.0).min(1.0) * (-t * 12.0).exp() * 1.4;
            scrape + (TAU * 2300.0 * t).sin() * (-t * 9.0).exp() * 0.25
        }),
    ));
    clips.push((Sound::Burst, burst(85.0, 9)));
    clips.push((Sound::Burst, burst(70.0, 166)));
    clips.push((Sound::Burst, burst(100.0, 167)));
    let mut lp = Lowpass(0.0);
    let mut spark = 0.0f32;
    clips.push((
        Sound::Crackle,
        render(0.4, 10, |t, rng| {
            if random(rng) < 0.004 {
                spark = 1.0;
            }
            spark *= 0.93;
            let x = noise(rng) * spark;
            (x - lp.step(x, 2000.0)) * 1.6 * (1.0 - t / 0.4)
        }),
    ));
    let mut lp = Lowpass(0.0);
    clips.push((
        Sound::Chest,
        render(1.3, 11, |t, _| {
            // A creaking lid, then a chime
            let creak = if t < 0.4 {
                let f = 80.0 + 60.0 * t / 0.4 + 6.0 * (TAU * 9.0 * t).sin();
                lp.step((f * t).fract() * 2.0 - 1.0, 700.0) * swell(t, 0.4) * 0.8
            } else {
                0.0
            };
            let c = (t - 0.35).max(0.0);
            let chime = [1.0f32, 1.5, 2.0, 2.76].iter().enumerate().map(|(k, m)| (TAU * 880.0 * m * c).sin() * (-c * (3.0 + k as f32 * 2.0)).exp() / (k + 1) as f32).sum::<f32>();
            creak + if t > 0.35 { chime * 0.4 } else { 0.0 }
        }),
    ));
    clips.push((
        Sound::Pickup,
        render(0.2, 12, |t, _| {
            let note = if t < 0.08 { 880.0 } else { 1320.0 };
            (TAU * note * t).sin() * (-(t % 0.08) * 20.0).exp() * 0.35
        }),
    ));
    let mut lp = Lowpass(0.0);
    clips.push((Sound::Eat, render(0.4, 13, |t, rng| lp.step(noise(rng), 2500.0) * (-(t % 0.13) * 30.0).exp() * 1.4)));
    clips.push((Sound::Quake, quake(34.0, 70)));
    clips.push((Sound::Quake, quake(29.0, 71)));
    clips.push((Sound::Whistle, whistle(2400.0, 72)));
    clips.push((Sound::Whistle, whistle(2100.0, 73)));
    clips.push((Sound::Claw, claw(0.022, 74)));
    clips.push((Sound::Claw, claw(0.028, 75)));
    clips.push((Sound::Web, web(2200.0, 76)));
    clips.push((Sound::Web, web(1700.0, 77)));
    clips.push((Sound::Creak, creak(170.0, 78)));
    clips.push((Sound::Creak, creak(135.0, 79)));
    clips.push((Sound::Crack, crack(2800.0, 80)));
    clips.push((Sound::Crack, crack(2300.0, 81)));
    clips.push((Sound::Flap, flap(70.0, 82)));
    clips.push((Sound::Flap, flap(58.0, 83)));
    clips.push((Sound::Hiss, hiss(28.0, 84)));
    clips.push((Sound::Hiss, hiss(0.0, 85)));
    clips.push((Sound::Growl, growl(90.0, 0.0, 86)));
    clips.push((Sound::Growl, growl(76.0, 0.3, 87)));
    clips.push((Sound::Growl, growl(105.0, 0.8, 88)));
    clips.push((Sound::Howl, howl(480.0, 89)));
    clips.push((Sound::Howl, howl(410.0, 90)));
    clips.push((Sound::Howl, howl(540.0, 91)));
    clips.push((Sound::Bite, bite(2600.0, 92)));
    clips.push((Sound::Bite, bite(2200.0, 93)));
    clips.push((Sound::Bite, bite(3000.0, 94)));
    clips.push((Sound::Rattle, rattle(900.0, 95)));
    clips.push((Sound::Rattle, rattle(1150.0, 96)));
    clips.push((Sound::Rattle, rattle(780.0, 97)));
    clips.push((Sound::Clang, clang(720.0, 98)));
    clips.push((Sound::Clang, clang(910.0, 99)));
    clips.push((Sound::Clang, clang(560.0, 168)));
    clips.push((Sound::Squelch, squelch(110.0, 169)));
    clips.push((Sound::Squelch, squelch(90.0, 170)));
    clips.push((Sound::Squelch, squelch(135.0, 171)));
    clips.push((Sound::Skitter, skitter(2800.0, 172)));
    clips.push((Sound::Skitter, skitter(3400.0, 173)));
    clips.push((Sound::Skitter, skitter(2300.0, 174)));
    clips.push((Sound::Dodge, dodge(3200.0, 175)));
    clips.push((Sound::Dodge, dodge(2600.0, 176)));
    clips.push((Sound::Dodge, dodge(3800.0, 177)));
    for v in 0..VARIANTS as u32 {
        for ground in [Ground::Grass, Ground::Sand, Ground::Stone, Ground::Snow, Ground::Water] {
            clips.push((Sound::Step(ground), step(ground, 100 + v * 17 + ground as u32)));
        }
        clips.push((Sound::Bird, birdsong(200 + v * 31)));
        clips.push((Sound::Bird, birdsong(300 + v * 29)));
    }
    clips
}

impl Audio {
    /// None if there's nothing to play sound on
    pub fn new() -> Option<Audio> {
        let (stream, handle) = OutputStream::try_default().ok()?;
        // Wind: low rumbling noise, rising and falling in gusts
        let mut lp = Lowpass(0.0);
        let mut lp2 = Lowpass(0.0);
        let wind = looped(9.0, 50, |t, rng| {
            let gust = 0.55 + 0.3 * (TAU * t / 9.0).sin() + 0.15 * (TAU * t * 2.0 / 9.0 + 1.3).sin();
            let x = lp.step(noise(rng), 140.0 + 160.0 * gust);
            (x - lp2.step(x, 50.0)) * gust * 2.5
        });
        // Water: soft, low lapping in slow waves, a little brighter as each one breaks
        let mut lp = Lowpass(0.0);
        let mut lp2 = Lowpass(0.0);
        let water = looped(8.0, 60, |t, rng| {
            let wave = 0.55 + 0.3 * (TAU * t * 3.0 / 8.0).sin() + 0.15 * (TAU * t * 7.0 / 8.0 + 0.7).sin();
            let x = noise(rng);
            (lp.step(x, 250.0 + 450.0 * wave) - lp2.step(x, 90.0)) * wave * 1.6
        });
        let ambience = |samples: Vec<f32>| {
            let sink = Sink::try_new(&handle).ok()?;
            sink.append(SamplesBuffer::new(1, RATE, samples).repeat_infinite());
            sink.set_volume(0.0);
            Some(sink)
        };
        let (wind, water) = (ambience(wind)?, ambience(water)?);
        Some(Audio { _stream: stream, handle, clips: make_clips(), wind, water, volume: 0.7, bird_timer: 2.0, rng: 7 })
    }

    /// Plays `cue`, heard from where the camera is
    pub fn play(&mut self, cue: Cue, view: &View) {
        let mut volume = cue.volume * self.volume;
        let mut pan = 0.0;
        if let Some(at) = cue.at {
            let to = at - view.eye;
            let meters = (to.norm() / view.vpm) as f32;
            volume /= 1.0 + meters / 5.0;
            if meters > 0.1 {
                let right = view.rotation * Vector3::x();
                pan = (to.normalize().dot(&right) as f32).clamp(-1.0, 1.0) * 0.8;
            }
        }
        if volume < 0.005 {
            return;
        }
        let choices: Vec<&Vec<f32>> = self.clips.iter().filter(|(s, _)| *s == cue.sound).map(|(_, c)| c).collect();
        if choices.is_empty() {
            return;
        }
        let clip = choices[(random(&mut self.rng) * choices.len() as f32) as usize % choices.len()].clone();
        // Equal power panning, and a little variation in pitch
        let angle = (pan + 1.0) * PI * 0.25;
        let speed = cue.pitch * (0.92 + 0.16 * random(&mut self.rng));
        let source = ChannelVolume::new(SamplesBuffer::new(1, RATE, clip).speed(speed), vec![angle.cos() * volume, angle.sin() * volume]);
        let _ = self.handle.play_raw(source.convert_samples());
    }

    /// How loud the wind and the water around are, 0-1, and whether birds would be singing.
    /// Each played somewhere off in the trees now and then.
    pub fn ambience(&mut self, wind: f32, water: f32, birds: bool, view: &View, dt: f64) {
        self.wind.set_volume(wind * 0.25 * self.volume);
        self.water.set_volume(water * 0.25 * self.volume);
        self.bird_timer -= dt;
        if self.bird_timer <= 0.0 {
            self.bird_timer = 1.5 + 5.0 * random(&mut self.rng) as f64;
            if birds {
                // Somewhere off in the trees
                let angle = random(&mut self.rng) as f64 * std::f64::consts::TAU;
                let distance = (6.0 + 20.0 * random(&mut self.rng) as f64) * view.vpm;
                let at = view.eye + Vector3::new(angle.cos() * distance, angle.sin() * distance, 4.0 * view.vpm);
                self.play(Cue::at(Sound::Bird, at, 0.5), view);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sounds_are_audible_and_not_too_loud() {
        let start = std::time::Instant::now();
        let clips = make_clips();
        println!("made {} sounds in {:?}", clips.len(), start.elapsed());
        for (sound, clip) in &clips {
            assert!(!clip.is_empty(), "{sound:?} is empty");
            assert!(clip.iter().all(|s| s.is_finite()), "{sound:?} has broken samples");
            let peak = clip.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            println!("{sound:?}: {:.2} s, peak {peak:.2}", clip.len() as f32 / RATE as f32);
            assert!(peak > 0.05 && peak < 3.0, "{sound:?} peaks at {peak}");
        }
    }
}
