//! Sound: swings, bowstrings, spells and hits, footsteps on whatever's underfoot, birds, and the
//! wind and water all around. Every sound is made here when the game starts, from noise and tones
//! shaped and filtered, so there are no sound files. Sounds out in the world are quieter further
//! away and come from their side.

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
        (Sound::Swing, whoosh(0.28, 400.0, 2600.0, 1)),
        (Sound::Swing, whoosh(0.26, 500.0, 3000.0, 2)),
        (Sound::HeavySwing, whoosh(0.42, 250.0, 1500.0, 3)),
    ];
    let mut lp = Lowpass(0.0);
    clips.push((
        Sound::Hit,
        render(0.25, 4, |t, rng| {
            let thump = (TAU * (120.0 - 200.0 * t) * t).sin() * (-t * 16.0).exp();
            thump + lp.step(noise(rng), 1500.0) * (-t * 50.0).exp() * 1.5
        }),
    ));
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
    let mut lp = Lowpass(0.0);
    clips.push((
        Sound::Burst,
        render(0.9, 9, |t, rng| (lp.step(noise(rng), 500.0 - 300.0 * t) * 2.5 + (TAU * 50.0 * t).sin() * 0.6) * (-t * 4.5).exp() * (t * 60.0).min(1.0)),
    ));
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
