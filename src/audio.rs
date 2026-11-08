//! Sound: swings, bowstrings, spells and hits, beasts growling and bones rattling, footsteps on
//! whatever's underfoot, birds, and the wind and water all around. Every sound is a recording,
//! kept compressed in assets\sounds and built into the program; see CREDITS.txt there for where
//! each came from. They're decoded on a thread of their own as soon as the game starts, so they're
//! ready long before the menu is. Sounds out in the world are quieter and duller further away and
//! come from their side. Everything is mixed together and goes out through a limiter, so however
//! much happens at once it never clips.

use std::collections::HashMap;
use std::f32::consts::PI;
use std::io::Cursor;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Once, OnceLock};
use std::time::{Duration, Instant};

use nalgebra::Vector3;
use rodio::cpal::traits::HostTrait;
use rodio::dynamic_mixer::{self, DynamicMixer, DynamicMixerController};
use rodio::source::{ChannelVolume, Zero};
use rodio::{DeviceTrait, OutputStream, Sink, Source};

use crate::combat::View;

/// The rate every recording is kept at
const RATE: u32 = 44100;

/// How many sounds can play at once. Past three quarters of this, quiet new ones are left out.
const VOICES: usize = 40;

/// The same sound started again within this long of the first is heard as one of a few, rather
/// than piling up: a dozen arrows landing at once shouldn't be a dozen times as loud
const BURST: Duration = Duration::from_millis(50);

/// The mix is held under this, out of 1
const CEILING: f32 = 0.9;

/// What's underfoot
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Ground {
    Grass,
    Sand,
    Stone,
    Snow,
    Water,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
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
    /// A boss beaten: a war horn sounding
    Fanfare,
    /// Something hitting water
    Splash,
    /// The ground shaking: a deep boom and a long rumble after it
    Quake,
    /// Something falling from high up, rushing down just before it lands
    Whistle,
    /// A heavy paw raking past
    Claw,
    /// Web shot out, wet and sticky
    Web,
    /// Old wood groaning as it bends
    Creak,
    /// Ice or stone cracking, or a whip
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
    /// A gun going off: a crack, a boom, and the roll of it dying away
    Gunshot,
    /// A crossbow loosed: its prod slamming forward and the string's slap
    Release,
    /// One click of a windlass's ratchet, or of a bolt dropped into a magazine
    Ratchet,
    /// A crossbow's string catching on its latch, a solid clack
    Latch,
    /// Powder poured down a barrel
    Powder,
    /// A ramrod scraping down a barrel and thudding home on the ball
    Ram,
    /// A gun's hammer drawn back, click-clack
    Cock,
    /// A ball glancing off stone, whining away
    Ricochet,
    /// An orb's beam pouring out, a crackling buzz
    Beam,
    /// A bell tolling, deep and long
    Toll,
    /// Something conjured: a breath of air rising with a shimmer of chimes
    Summon,
    /// A hex cast: a dark whisper over a low rush
    Hex,
    /// A hand crossbow loosed: a light, sharp snap
    Snap,
    /// A bolt-thrower's five strings slapping at once, a ragged volley
    Volley,
    /// A harpoon loosed: a deep thunk, and its line whizzing out after it
    Harpoon,
    /// A line or a great prod going taut: a creak and a deep thrum
    Taut,
    /// A siege crossbow's leg thudding down, its iron clanking
    Brace,
    /// A line wound onto a reel
    Reel,
    /// A net flung open: a soft rush, its weights thudding down
    Net,
    /// A chain flung, rattling
    Chain,
    /// Hard mandibles clicking fast, a burrower's
    Chitter,
    /// A hollow moan rising and sinking away, a wraith's
    Wail,
    /// A deep throaty croak, a lurker's
    Croak,
    /// A shrill screech, a gloomwing's
    Screech,
    /// A wet snort and a grunt, a tusker's
    Snort,
    /// A flail's chain whirring round, its head rushing through the air, links clinking
    Whirr,
    /// A maul slamming into the ground: a deep thud, a crunch of earth and a boom rolling out
    Slam,
    /// A fuse lit and sputtering, hissing and spitting
    Fuse,
    /// Lightning leaping from a rod: a sharp snap and a buzzing crackle
    Zap,
    /// Thunder: a tearing crack overhead, then a long rolling rumble
    Thunder,
    /// A whirlwind howling, rising and falling
    Gale,
    /// A swarm droning, a cloud of wings
    Buzz,
    /// A bog bubbling, blobs of it bursting
    Bubble,
    /// A censer's cloud billowing out, a soft breathy rush
    Billow,
    /// A blade's edge cut loose as a crescent of light: steel ringing out over a sweeping hiss
    Shing,
    /// A mark set on a quarry: a small bell
    Chime,
    /// A rocket going up: a sputtering hiss, ending in a crackling pop
    Rocket,
    /// A charge: feet pounding and a rush of air building
    Rush,
    /// A lasso whirled overhead: a rope swishing round, and the snap of its throw
    Lasso,
    /// Leather coils drawn tight: a creaking groan and a squeeze of rope on rope
    Squeeze,
    /// The air itself cracking: a sharp report and a deep boom under it
    Boom,
    /// Snakes slithering off: dry scales rasping over the ground under a hiss
    Slither,
    /// Fire catching along a line: a soft whump of flame and a sizzle
    Sear,
    /// A rope swung on, a deep sweeping whoosh rising and falling
    Swoop,
    /// Thorns bursting out: a dry crack and a flurry of little things whizzing off
    Thorns,
    /// Coins: gold picked up, paid or taken
    Coins,
    /// An axe biting into a trunk, a pick on rock, and a spade into earth or clay, see gather.rs
    Chop,
    Mine,
    Dig,
    /// A tree coming down: its wood groaning, then crashing apart
    Timber,
    /// A block set in place, or taken down, see bases.rs
    Place,
    DoorOpen,
    DoorClose,
    /// Something beating on a door, and a door giving way, splitting into pieces
    Bang,
    Splinter,
    /// The wind blowing all around, looped, see Audio::ambience()
    Wind,
    /// Water lapping nearby, looped, see Audio::ambience()
    Water,
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

/// Every recording decoded, each sound's versions together
type Library = HashMap<Sound, Vec<Arc<[f32]>>>;

static LIBRARY: OnceLock<Library> = OnceLock::new();
static DECODING: Once = Once::new();

/// Starts decoding every sound on a thread of its own, if that hasn't started already. Called as
/// soon as the game starts, so they're ready by the time anything plays.
pub fn preload() {
    DECODING.call_once(|| {
        let spawned = std::thread::Builder::new().name("sounds".into()).spawn(|| {
            let start = Instant::now();
            // Not getting in the way of the world being made, which is what the loading screen
            // waits on
            lower_priority();
            let library = decode_all(decoders());
            let _ = LIBRARY.set(library);
            crate::startup::finished("Decoding the sounds", start.elapsed());
        });
        if spawned.is_err() {
            let _ = LIBRARY.set(decode_all(1));
        }
    });
}

/// How many threads decode the sounds: a few, leaving the rest to the loading
fn decoders() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get() / 4).clamp(1, 3)
}

/// One recording's samples, -1 to 1 before its gain, mono at RATE
fn decode(bytes: &[u8]) -> Option<Vec<f32>> {
    let mut reader = lewton::inside_ogg::OggStreamReader::new(Cursor::new(bytes)).ok()?;
    if reader.ident_hdr.audio_channels != 1 || reader.ident_hdr.audio_sample_rate != RATE {
        return None;
    }
    let mut samples = Vec::new();
    while let Some(packet) = reader.read_dec_packet_generic::<lewton::samples::InterleavedSamples<f32>>().ok()? {
        samples.extend(packet.samples);
    }
    Some(samples)
}

/// A recording ready to play: brought to its loudness, and, unless it loops, eased in and out over
/// a few milliseconds. The files end quietly already, but compressing them leaves a little noise
/// right at the edges of a sharp sound, enough to click.
fn finish(sound: Sound, mut samples: Vec<f32>, gain: f32) -> Arc<[f32]> {
    for s in samples.iter_mut() {
        *s *= gain;
    }
    if !matches!(sound, Sound::Wind | Sound::Water) {
        let n = samples.len();
        let (fade_in, fade_out) = ((RATE as usize * 3 / 2000).min(n / 4), (RATE as usize * 6 / 1000).min(n / 4));
        for k in 0..fade_in {
            samples[k] *= 0.5 - 0.5 * (PI * k as f32 / fade_in as f32).cos();
        }
        for k in 0..fade_out {
            samples[n - 1 - k] *= 0.5 - 0.5 * (PI * k as f32 / fade_out as f32).cos();
        }
    }
    samples.into()
}

/// Every recording decoded on `threads` threads, each brought to its loudness
fn decode_all(threads: usize) -> Library {
    let next = AtomicUsize::new(0);
    let decoded = Mutex::new(vec![None; RECORDINGS.len()]);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                lower_priority();
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&(sound, bytes, gain)) = RECORDINGS.get(k) else { break };
                    let clip = decode(bytes).map(|samples| finish(sound, samples, gain));
                    decoded.lock().unwrap()[k] = clip;
                }
            });
        }
    });
    let mut library = Library::new();
    for (&(sound, _, _), clip) in RECORDINGS.iter().zip(decoded.into_inner().unwrap()) {
        if let Some(clip) = clip {
            library.entry(sound).or_default().push(clip);
        }
    }
    library
}

/// Lets the thread it's called on give way to others that are busy
#[cfg(windows)]
fn lower_priority() {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThread() -> *mut c_void;
        fn SetThreadPriority(thread: *mut c_void, priority: i32) -> i32;
    }
    // THREAD_PRIORITY_BELOW_NORMAL
    unsafe {
        SetThreadPriority(GetCurrentThread(), -1);
    }
}

#[cfg(not(windows))]
fn lower_priority() {}

/// Counted among the sounds playing for as long as it's kept
struct Voice(Arc<AtomicUsize>);

impl Voice {
    fn new(playing: &Arc<AtomicUsize>) -> Voice {
        playing.fetch_add(1, Ordering::Relaxed);
        Voice(playing.clone())
    }
}

impl Drop for Voice {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// A recording playing, shared rather than copied, maybe muffled by distance
struct Clip {
    samples: Arc<[f32]>,
    at: usize,
    /// How much of each new sample gets through a one-pole low-pass, 1 for all of it
    muffle: f32,
    smoothed: f32,
    _voice: Option<Voice>,
}

impl Clip {
    fn new(samples: Arc<[f32]>) -> Clip {
        Clip { samples, at: 0, muffle: 1.0, smoothed: 0.0, _voice: None }
    }
}

impl Iterator for Clip {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = *self.samples.get(self.at)?;
        self.at += 1;
        if self.muffle < 1.0 {
            self.smoothed += (sample - self.smoothed) * self.muffle;
            Some(self.smoothed)
        } else {
            Some(sample)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = self.samples.len().saturating_sub(self.at);
        (left, Some(left))
    }
}

impl Source for Clip {
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.samples.len().saturating_sub(self.at))
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        RATE
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(self.samples.len() as f64 / RATE as f64))
    }
}

/// How much of a sound `meters` away gets through: the air takes the top off far sounds. 1, all of
/// it, up close.
fn muffle(meters: f32) -> f32 {
    if meters < 10.0 {
        return 1.0;
    }
    let cutoff = (20000.0 / (1.0 + (meters - 10.0) / 12.0)).max(2000.0);
    1.0 - (-2.0 * PI * cutoff / RATE as f32).exp()
}

/// The whole mix on its way out, brought down smoothly whenever it would go past CEILING, so that
/// however many sounds play at once it never clips: the level is followed quickly up and slowly
/// back down, and what still gets past in the first instant of a blow is rounded off gently
struct Limiter<S> {
    input: S,
    level: f32,
    attack: f32,
    release: f32,
}

impl<S: Source<Item = f32>> Limiter<S> {
    fn new(input: S) -> Limiter<S> {
        // Per sample, every channel's samples coming one after another
        let rate = (input.sample_rate() * input.channels() as u32) as f32;
        Limiter { level: 0.0, attack: (-1.0 / (0.0003 * rate)).exp(), release: (-1.0 / (0.12 * rate)).exp(), input }
    }
}

/// Unchanged up to CEILING, then curving smoothly to never quite reach 1
fn soft_clip(x: f32) -> f32 {
    let over = x.abs() - CEILING;
    if over <= 0.0 { x } else { x.signum() * (CEILING + (1.0 - CEILING) * (over / (1.0 - CEILING)).tanh()) }
}

impl<S: Source<Item = f32>> Iterator for Limiter<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.input.next()?;
        let level = sample.abs();
        let k = if level > self.level { self.attack } else { self.release };
        self.level = level + (self.level - level) * k;
        let gain = if self.level > CEILING { CEILING / self.level } else { 1.0 };
        Some(soft_clip(sample * gain))
    }
}

impl<S: Source<Item = f32>> Source for Limiter<S> {
    fn current_frame_len(&self) -> Option<usize> {
        self.input.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.input.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.input.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// Where every sound is mixed, stereo at `rate`, and the mix that comes out of it through the
/// limiter. It keeps going with nothing playing, rather than ending.
fn bus(rate: u32) -> (Arc<DynamicMixerController<f32>>, Limiter<DynamicMixer<f32>>) {
    let (bus, mix) = dynamic_mixer::mixer::<f32>(2, rate);
    bus.add(Zero::<f32>::new(2, rate));
    (bus, Limiter::new(mix))
}

/// The rate sound goes out at, so the mix is made at it and isn't converted twice
fn output_rate() -> Option<u32> {
    let device = rodio::cpal::default_host().default_output_device()?;
    Some(device.default_output_config().ok()?.sample_rate().0)
}

/// `samples` as heard `meters` away, `pan` -1 left to 1 right, counted among `playing`
fn voice(samples: Arc<[f32]>, volume: f32, pan: f32, speed: f32, meters: f32, playing: &Arc<AtomicUsize>) -> impl Source<Item = f32> + Send + 'static {
    let clip = Clip { muffle: muffle(meters), _voice: Some(Voice::new(playing)), ..Clip::new(samples) };
    // Equal power panning
    let angle = (pan + 1.0) * PI * 0.25;
    ChannelVolume::new(clip.speed(speed), vec![angle.cos() * volume, angle.sin() * volume])
}

pub struct Audio {
    // Has to be kept for the sound to keep playing
    _stream: OutputStream,
    /// Everything heard is mixed here
    bus: Arc<DynamicMixerController<f32>>,
    wind: Sink,
    water: Sink,
    /// Whether the wind and water have been started, once the sounds were ready
    ambience_on: bool,
    /// Which version of each sound played last, so the same one isn't heard twice running
    last: HashMap<Sound, usize>,
    /// When each sound was last started fresh, and how many times again since, within BURST
    recent: HashMap<Sound, (Instant, u32)>,
    /// How many sounds are playing
    playing: Arc<AtomicUsize>,
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

impl Audio {
    /// None if there's nothing to play sound on
    pub fn new() -> Option<Audio> {
        preload();
        let (stream, handle) = OutputStream::try_default().ok()?;
        let (bus, mix) = bus(output_rate().unwrap_or(RATE));
        handle.play_raw(mix).ok()?;
        let quiet_sink = || {
            let (sink, out) = Sink::new_idle();
            sink.set_volume(0.0);
            bus.add(out);
            sink
        };
        let (wind, water) = (quiet_sink(), quiet_sink());
        Some(Audio {
            _stream: stream,
            bus,
            wind,
            water,
            ambience_on: false,
            last: HashMap::new(),
            recent: HashMap::new(),
            playing: Arc::new(AtomicUsize::new(0)),
            volume: 0.7,
            bird_timer: 2.0,
            rng: 7,
        })
    }

    /// Plays `cue`, heard from where the camera is. Nothing's heard if the sounds are still being
    /// decoded, which they're only in the first moment after the game starts.
    pub fn play(&mut self, cue: Cue, view: &View) {
        let mut volume = cue.volume * self.volume;
        let mut pan = 0.0;
        let mut meters = 0.0;
        if let Some(at) = cue.at {
            let to = at - view.eye;
            meters = (to.norm() / view.vpm) as f32;
            volume /= 1.0 + meters / 5.0;
            if meters > 0.1 {
                let right = view.rotation * Vector3::x();
                pan = (to.normalize().dot(&right) as f32).clamp(-1.0, 1.0) * 0.8;
            }
        }
        if volume < 0.005 {
            return;
        }
        let Some(versions) = LIBRARY.get().and_then(|library| library.get(&cue.sound)) else { return };
        // The same sound started over and over at once is heard as a few, each a little quieter
        let now = Instant::now();
        let again = match self.recent.get_mut(&cue.sound) {
            Some((first, times)) if now.duration_since(*first) < BURST => {
                *times += 1;
                *times
            }
            _ => {
                self.recent.insert(cue.sound, (now, 0));
                0
            }
        };
        if again >= 3 {
            return;
        }
        volume *= 0.75f32.powi(again as i32);
        // With a great deal playing already, quiet new sounds wouldn't be heard anyway
        let playing = self.playing.load(Ordering::Relaxed);
        if playing >= VOICES || (playing >= VOICES * 3 / 4 && volume < 0.1) {
            return;
        }
        // Any version but the one heard last time
        let n = versions.len();
        let mut pick = (random(&mut self.rng) * n as f32) as usize % n;
        if n > 1 && self.last.get(&cue.sound) == Some(&pick) {
            pick = (pick + 1 + (random(&mut self.rng) * (n - 1) as f32) as usize % (n - 1)) % n;
        }
        self.last.insert(cue.sound, pick);
        // A little variation in pitch, so no two are quite alike
        let speed = cue.pitch * (0.93 + 0.14 * random(&mut self.rng));
        self.bus.add(voice(versions[pick].clone(), volume, pan, speed, meters, &self.playing));
    }

    /// How loud the wind and the water around are, 0-1, and whether birds would be singing.
    /// Each played somewhere off in the trees now and then.
    pub fn ambience(&mut self, wind: f32, water: f32, birds: bool, view: &View, dt: f64) {
        if !self.ambience_on {
            if let Some(library) = LIBRARY.get() {
                for (sound, sink) in [(Sound::Wind, &self.wind), (Sound::Water, &self.water)] {
                    if let Some(versions) = library.get(&sound) {
                        sink.append(Clip::new(versions[0].clone()).repeat_infinite());
                    }
                }
                self.ambience_on = true;
            }
        }
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

/// Every recording: what it's the sound of, the file, and how much louder or quieter to play it so
/// all the sounds sit together. A sound with more than one is played as one of them at random.
/// The files are made by trimming, layering and evening out the recordings credited in
/// assets\sounds\CREDITS.txt; each is at a peak of about -1 dB, and the gain brings it to the
/// loudness its sound should have. A static rather than a const, so the files are in the program
/// once rather than once for every place that reads it.
#[rustfmt::skip]
static RECORDINGS: &[(Sound, &[u8], f32)] = &[
    (Sound::Swing, include_bytes!("../assets/sounds/swing_1.ogg"), 2.66),
    (Sound::Swing, include_bytes!("../assets/sounds/swing_2.ogg"), 2.88),
    (Sound::Swing, include_bytes!("../assets/sounds/swing_3.ogg"), 3.06),
    (Sound::Swing, include_bytes!("../assets/sounds/swing_4.ogg"), 2.16),
    (Sound::Swing, include_bytes!("../assets/sounds/swing_5.ogg"), 2.04),
    (Sound::Swing, include_bytes!("../assets/sounds/swing_6.ogg"), 3.00),
    (Sound::HeavySwing, include_bytes!("../assets/sounds/heavy_swing_1.ogg"), 2.23),
    (Sound::HeavySwing, include_bytes!("../assets/sounds/heavy_swing_2.ogg"), 1.36),
    (Sound::HeavySwing, include_bytes!("../assets/sounds/heavy_swing_3.ogg"), 2.10),
    (Sound::HeavySwing, include_bytes!("../assets/sounds/heavy_swing_4.ogg"), 1.52),
    (Sound::Stab, include_bytes!("../assets/sounds/stab_1.ogg"), 1.82),
    (Sound::Stab, include_bytes!("../assets/sounds/stab_2.ogg"), 2.63),
    (Sound::Stab, include_bytes!("../assets/sounds/stab_3.ogg"), 3.02),
    (Sound::Flick, include_bytes!("../assets/sounds/flick_1.ogg"), 1.31),
    (Sound::Flick, include_bytes!("../assets/sounds/flick_2.ogg"), 1.97),
    (Sound::Dodge, include_bytes!("../assets/sounds/dodge_1.ogg"), 0.92),
    (Sound::Dodge, include_bytes!("../assets/sounds/dodge_2.ogg"), 1.74),
    (Sound::Dodge, include_bytes!("../assets/sounds/dodge_3.ogg"), 2.04),
    (Sound::Hit, include_bytes!("../assets/sounds/hit_1.ogg"), 2.54),
    (Sound::Hit, include_bytes!("../assets/sounds/hit_2.ogg"), 2.57),
    (Sound::Hit, include_bytes!("../assets/sounds/hit_3.ogg"), 1.11),
    (Sound::Hit, include_bytes!("../assets/sounds/hit_4.ogg"), 3.83),
    (Sound::Hit, include_bytes!("../assets/sounds/hit_5.ogg"), 2.50),
    (Sound::Clang, include_bytes!("../assets/sounds/clang_1.ogg"), 0.86),
    (Sound::Clang, include_bytes!("../assets/sounds/clang_2.ogg"), 2.98),
    (Sound::Clang, include_bytes!("../assets/sounds/clang_3.ogg"), 1.55),
    (Sound::Clang, include_bytes!("../assets/sounds/clang_4.ogg"), 1.00),
    (Sound::ArrowHit, include_bytes!("../assets/sounds/arrow_hit_1.ogg"), 2.01),
    (Sound::ArrowHit, include_bytes!("../assets/sounds/arrow_hit_2.ogg"), 1.24),
    (Sound::ArrowHit, include_bytes!("../assets/sounds/arrow_hit_3.ogg"), 3.47),
    (Sound::Crack, include_bytes!("../assets/sounds/crack_1.ogg"), 0.75),
    (Sound::Crack, include_bytes!("../assets/sounds/crack_2.ogg"), 0.72),
    (Sound::Crack, include_bytes!("../assets/sounds/crack_3.ogg"), 1.94),
    (Sound::Crack, include_bytes!("../assets/sounds/crack_4.ogg"), 1.55),
    (Sound::Draw, include_bytes!("../assets/sounds/draw_1.ogg"), 1.98),
    (Sound::Twang, include_bytes!("../assets/sounds/twang_1.ogg"), 2.00),
    (Sound::Twang, include_bytes!("../assets/sounds/twang_2.ogg"), 2.14),
    (Sound::Twang, include_bytes!("../assets/sounds/twang_3.ogg"), 2.43),
    (Sound::Release, include_bytes!("../assets/sounds/release_1.ogg"), 2.93),
    (Sound::Release, include_bytes!("../assets/sounds/release_2.ogg"), 0.85),
    (Sound::Release, include_bytes!("../assets/sounds/release_3.ogg"), 1.32),
    (Sound::Release, include_bytes!("../assets/sounds/release_4.ogg"), 1.27),
    (Sound::Snap, include_bytes!("../assets/sounds/snap_1.ogg"), 3.33),
    (Sound::Snap, include_bytes!("../assets/sounds/snap_2.ogg"), 0.60),
    (Sound::Snap, include_bytes!("../assets/sounds/snap_3.ogg"), 2.04),
    (Sound::Volley, include_bytes!("../assets/sounds/volley_1.ogg"), 0.87),
    (Sound::Harpoon, include_bytes!("../assets/sounds/harpoon_1.ogg"), 0.84),
    (Sound::Taut, include_bytes!("../assets/sounds/taut_1.ogg"), 2.42),
    (Sound::Taut, include_bytes!("../assets/sounds/taut_2.ogg"), 3.81),
    (Sound::Ratchet, include_bytes!("../assets/sounds/ratchet_1.ogg"), 0.88),
    (Sound::Ratchet, include_bytes!("../assets/sounds/ratchet_2.ogg"), 1.88),
    (Sound::Ratchet, include_bytes!("../assets/sounds/ratchet_3.ogg"), 1.48),
    (Sound::Ratchet, include_bytes!("../assets/sounds/ratchet_4.ogg"), 1.51),
    (Sound::Latch, include_bytes!("../assets/sounds/latch_1.ogg"), 1.72),
    (Sound::Latch, include_bytes!("../assets/sounds/latch_2.ogg"), 1.53),
    (Sound::Latch, include_bytes!("../assets/sounds/latch_3.ogg"), 2.52),
    (Sound::Brace, include_bytes!("../assets/sounds/brace_1.ogg"), 0.84),
    (Sound::Brace, include_bytes!("../assets/sounds/brace_2.ogg"), 0.79),
    (Sound::Reel, include_bytes!("../assets/sounds/reel_1.ogg"), 1.45),
    (Sound::Reel, include_bytes!("../assets/sounds/reel_2.ogg"), 0.93),
    (Sound::Net, include_bytes!("../assets/sounds/net_1.ogg"), 1.05),
    (Sound::Chain, include_bytes!("../assets/sounds/chain_1.ogg"), 0.71),
    (Sound::Chain, include_bytes!("../assets/sounds/chain_2.ogg"), 2.20),
    (Sound::Chain, include_bytes!("../assets/sounds/chain_3.ogg"), 2.81),
    (Sound::Gunshot, include_bytes!("../assets/sounds/gunshot_1.ogg"), 1.89),
    (Sound::Gunshot, include_bytes!("../assets/sounds/gunshot_2.ogg"), 1.77),
    (Sound::Gunshot, include_bytes!("../assets/sounds/gunshot_3.ogg"), 1.68),
    (Sound::Cock, include_bytes!("../assets/sounds/cock_1.ogg"), 0.71),
    (Sound::Cock, include_bytes!("../assets/sounds/cock_2.ogg"), 0.57),
    (Sound::Powder, include_bytes!("../assets/sounds/powder_1.ogg"), 1.39),
    (Sound::Powder, include_bytes!("../assets/sounds/powder_2.ogg"), 0.61),
    (Sound::Ram, include_bytes!("../assets/sounds/ram_1.ogg"), 0.90),
    (Sound::Ricochet, include_bytes!("../assets/sounds/ricochet_1.ogg"), 0.88),
    (Sound::Ricochet, include_bytes!("../assets/sounds/ricochet_2.ogg"), 3.61),
    (Sound::Cast(0), include_bytes!("../assets/sounds/cast_plain_1.ogg"), 1.64),
    (Sound::Cast(1), include_bytes!("../assets/sounds/cast_fire_1.ogg"), 1.15),
    (Sound::Cast(1), include_bytes!("../assets/sounds/cast_fire_2.ogg"), 2.08),
    (Sound::Cast(2), include_bytes!("../assets/sounds/cast_frost_1.ogg"), 3.22),
    (Sound::Cast(2), include_bytes!("../assets/sounds/cast_frost_2.ogg"), 2.90),
    (Sound::Cast(3), include_bytes!("../assets/sounds/cast_storm_1.ogg"), 1.00),
    (Sound::Cast(3), include_bytes!("../assets/sounds/cast_storm_2.ogg"), 1.13),
    (Sound::Cast(4), include_bytes!("../assets/sounds/cast_poison_1.ogg"), 1.36),
    (Sound::Cast(5), include_bytes!("../assets/sounds/cast_arcane_1.ogg"), 1.42),
    (Sound::Cast(6), include_bytes!("../assets/sounds/cast_void_1.ogg"), 0.69),
    (Sound::Cast(7), include_bytes!("../assets/sounds/cast_blood_1.ogg"), 1.28),
    (Sound::Cast(8), include_bytes!("../assets/sounds/cast_holy_1.ogg"), 0.78),
    (Sound::Beam, include_bytes!("../assets/sounds/beam_1.ogg"), 0.83),
    (Sound::Beam, include_bytes!("../assets/sounds/beam_2.ogg"), 0.79),
    (Sound::Zap, include_bytes!("../assets/sounds/zap_1.ogg"), 0.78),
    (Sound::Zap, include_bytes!("../assets/sounds/zap_2.ogg"), 1.02),
    (Sound::Crackle, include_bytes!("../assets/sounds/crackle_1.ogg"), 2.95),
    (Sound::Crackle, include_bytes!("../assets/sounds/crackle_2.ogg"), 3.37),
    (Sound::Crackle, include_bytes!("../assets/sounds/crackle_3.ogg"), 0.79),
    (Sound::Summon, include_bytes!("../assets/sounds/summon_1.ogg"), 1.32),
    (Sound::Hex, include_bytes!("../assets/sounds/hex_1.ogg"), 0.92),
    (Sound::Hex, include_bytes!("../assets/sounds/hex_2.ogg"), 0.74),
    (Sound::Shing, include_bytes!("../assets/sounds/shing_1.ogg"), 1.02),
    (Sound::Shing, include_bytes!("../assets/sounds/shing_2.ogg"), 0.94),
    (Sound::Chime, include_bytes!("../assets/sounds/chime_1.ogg"), 0.45),
    (Sound::Chime, include_bytes!("../assets/sounds/chime_2.ogg"), 0.80),
    (Sound::Toll, include_bytes!("../assets/sounds/toll_1.ogg"), 1.17),
    (Sound::Toll, include_bytes!("../assets/sounds/toll_2.ogg"), 1.16),
    (Sound::Burst, include_bytes!("../assets/sounds/burst_1.ogg"), 1.61),
    (Sound::Burst, include_bytes!("../assets/sounds/burst_2.ogg"), 1.54),
    (Sound::Burst, include_bytes!("../assets/sounds/burst_3.ogg"), 2.32),
    (Sound::Burst, include_bytes!("../assets/sounds/burst_4.ogg"), 0.94),
    (Sound::Boom, include_bytes!("../assets/sounds/boom_1.ogg"), 0.73),
    (Sound::Boom, include_bytes!("../assets/sounds/boom_2.ogg"), 0.81),
    (Sound::Quake, include_bytes!("../assets/sounds/quake_1.ogg"), 1.26),
    (Sound::Quake, include_bytes!("../assets/sounds/quake_2.ogg"), 1.16),
    (Sound::Slam, include_bytes!("../assets/sounds/slam_1.ogg"), 1.36),
    (Sound::Slam, include_bytes!("../assets/sounds/slam_2.ogg"), 1.40),
    (Sound::Thunder, include_bytes!("../assets/sounds/thunder_1.ogg"), 1.48),
    (Sound::Thunder, include_bytes!("../assets/sounds/thunder_2.ogg"), 0.78),
    (Sound::Rocket, include_bytes!("../assets/sounds/rocket_1.ogg"), 0.88),
    (Sound::Rocket, include_bytes!("../assets/sounds/rocket_2.ogg"), 1.22),
    (Sound::Fuse, include_bytes!("../assets/sounds/fuse_1.ogg"), 1.01),
    (Sound::Fuse, include_bytes!("../assets/sounds/fuse_2.ogg"), 1.30),
    (Sound::Whistle, include_bytes!("../assets/sounds/whistle_1.ogg"), 0.70),
    (Sound::Whistle, include_bytes!("../assets/sounds/whistle_2.ogg"), 0.94),
    (Sound::Gale, include_bytes!("../assets/sounds/gale_1.ogg"), 0.86),
    (Sound::Breath, include_bytes!("../assets/sounds/breath_1.ogg"), 2.36),
    (Sound::Breath, include_bytes!("../assets/sounds/breath_2.ogg"), 2.19),
    (Sound::Breath, include_bytes!("../assets/sounds/breath_3.ogg"), 2.36),
    (Sound::Sear, include_bytes!("../assets/sounds/sear_1.ogg"), 1.65),
    (Sound::Sear, include_bytes!("../assets/sounds/sear_2.ogg"), 0.89),
    (Sound::Billow, include_bytes!("../assets/sounds/billow_1.ogg"), 2.35),
    (Sound::Billow, include_bytes!("../assets/sounds/billow_2.ogg"), 1.04),
    (Sound::Equip, include_bytes!("../assets/sounds/equip_1.ogg"), 1.08),
    (Sound::Equip, include_bytes!("../assets/sounds/equip_2.ogg"), 0.89),
    (Sound::Equip, include_bytes!("../assets/sounds/equip_3.ogg"), 3.79),
    (Sound::Chest, include_bytes!("../assets/sounds/chest_1.ogg"), 2.57),
    (Sound::Chest, include_bytes!("../assets/sounds/chest_2.ogg"), 3.69),
    (Sound::Pickup, include_bytes!("../assets/sounds/pickup_1.ogg"), 2.49),
    (Sound::Pickup, include_bytes!("../assets/sounds/pickup_2.ogg"), 1.91),
    (Sound::Pickup, include_bytes!("../assets/sounds/pickup_3.ogg"), 1.70),
    (Sound::Coins, include_bytes!("../assets/sounds/coins_1.ogg"), 2.74),
    (Sound::Coins, include_bytes!("../assets/sounds/coins_2.ogg"), 1.79),
    (Sound::Coins, include_bytes!("../assets/sounds/coins_3.ogg"), 0.95),
    (Sound::Eat, include_bytes!("../assets/sounds/eat_1.ogg"), 2.36),
    (Sound::Eat, include_bytes!("../assets/sounds/eat_2.ogg"), 2.11),
    (Sound::Eat, include_bytes!("../assets/sounds/eat_3.ogg"), 1.42),
    (Sound::Creak, include_bytes!("../assets/sounds/creak_1.ogg"), 1.31),
    (Sound::Creak, include_bytes!("../assets/sounds/creak_2.ogg"), 1.41),
    (Sound::Creak, include_bytes!("../assets/sounds/creak_3.ogg"), 1.46),
    (Sound::Creak, include_bytes!("../assets/sounds/creak_4.ogg"), 2.44),
    (Sound::Squeeze, include_bytes!("../assets/sounds/squeeze_1.ogg"), 2.82),
    (Sound::Squeeze, include_bytes!("../assets/sounds/squeeze_2.ogg"), 3.66),
    (Sound::Lasso, include_bytes!("../assets/sounds/lasso_1.ogg"), 1.15),
    (Sound::Swoop, include_bytes!("../assets/sounds/swoop_1.ogg"), 0.75),
    (Sound::Whirr, include_bytes!("../assets/sounds/whirr_1.ogg"), 0.65),
    (Sound::Whirr, include_bytes!("../assets/sounds/whirr_2.ogg"), 0.94),
    (Sound::Rush, include_bytes!("../assets/sounds/rush_1.ogg"), 2.13),
    (Sound::Thorns, include_bytes!("../assets/sounds/thorns_1.ogg"), 1.39),
    (Sound::Splash, include_bytes!("../assets/sounds/splash_1.ogg"), 2.87),
    (Sound::Splash, include_bytes!("../assets/sounds/splash_2.ogg"), 1.27),
    (Sound::Splash, include_bytes!("../assets/sounds/splash_3.ogg"), 1.90),
    (Sound::Bubble, include_bytes!("../assets/sounds/bubble_1.ogg"), 1.78),
    (Sound::Bubble, include_bytes!("../assets/sounds/bubble_2.ogg"), 1.27),
    (Sound::Roar, include_bytes!("../assets/sounds/roar_1.ogg"), 1.61),
    (Sound::Roar, include_bytes!("../assets/sounds/roar_2.ogg"), 1.21),
    (Sound::Roar, include_bytes!("../assets/sounds/roar_3.ogg"), 2.20),
    (Sound::Rumble, include_bytes!("../assets/sounds/rumble_1.ogg"), 1.52),
    (Sound::Rumble, include_bytes!("../assets/sounds/rumble_2.ogg"), 1.61),
    (Sound::Growl, include_bytes!("../assets/sounds/growl_1.ogg"), 1.05),
    (Sound::Growl, include_bytes!("../assets/sounds/growl_2.ogg"), 0.77),
    (Sound::Growl, include_bytes!("../assets/sounds/growl_3.ogg"), 1.30),
    (Sound::Howl, include_bytes!("../assets/sounds/howl_1.ogg"), 0.75),
    (Sound::Howl, include_bytes!("../assets/sounds/howl_2.ogg"), 0.94),
    (Sound::Howl, include_bytes!("../assets/sounds/howl_3.ogg"), 1.17),
    (Sound::Hiss, include_bytes!("../assets/sounds/hiss_1.ogg"), 1.26),
    (Sound::Hiss, include_bytes!("../assets/sounds/hiss_2.ogg"), 1.26),
    (Sound::Hiss, include_bytes!("../assets/sounds/hiss_3.ogg"), 1.37),
    (Sound::Bite, include_bytes!("../assets/sounds/bite_1.ogg"), 0.71),
    (Sound::Bite, include_bytes!("../assets/sounds/bite_2.ogg"), 3.47),
    (Sound::Bite, include_bytes!("../assets/sounds/bite_3.ogg"), 2.53),
    (Sound::Rattle, include_bytes!("../assets/sounds/rattle_1.ogg"), 1.96),
    (Sound::Rattle, include_bytes!("../assets/sounds/rattle_2.ogg"), 2.08),
    (Sound::Rattle, include_bytes!("../assets/sounds/rattle_3.ogg"), 1.91),
    (Sound::Claw, include_bytes!("../assets/sounds/claw_1.ogg"), 0.82),
    (Sound::Claw, include_bytes!("../assets/sounds/claw_2.ogg"), 0.59),
    (Sound::Web, include_bytes!("../assets/sounds/web_1.ogg"), 3.65),
    (Sound::Web, include_bytes!("../assets/sounds/web_2.ogg"), 2.77),
    (Sound::Flap, include_bytes!("../assets/sounds/flap_1.ogg"), 2.95),
    (Sound::Flap, include_bytes!("../assets/sounds/flap_2.ogg"), 0.96),
    (Sound::Flap, include_bytes!("../assets/sounds/flap_3.ogg"), 2.17),
    (Sound::Squelch, include_bytes!("../assets/sounds/squelch_1.ogg"), 1.10),
    (Sound::Squelch, include_bytes!("../assets/sounds/squelch_2.ogg"), 3.84),
    (Sound::Squelch, include_bytes!("../assets/sounds/squelch_3.ogg"), 3.47),
    (Sound::Squelch, include_bytes!("../assets/sounds/squelch_4.ogg"), 2.12),
    (Sound::Skitter, include_bytes!("../assets/sounds/skitter_1.ogg"), 2.19),
    (Sound::Skitter, include_bytes!("../assets/sounds/skitter_2.ogg"), 2.41),
    (Sound::Chitter, include_bytes!("../assets/sounds/chitter_1.ogg"), 1.26),
    (Sound::Chitter, include_bytes!("../assets/sounds/chitter_2.ogg"), 0.99),
    (Sound::Chitter, include_bytes!("../assets/sounds/chitter_3.ogg"), 0.82),
    (Sound::Wail, include_bytes!("../assets/sounds/wail_1.ogg"), 1.08),
    (Sound::Wail, include_bytes!("../assets/sounds/wail_2.ogg"), 0.78),
    (Sound::Croak, include_bytes!("../assets/sounds/croak_1.ogg"), 0.68),
    (Sound::Croak, include_bytes!("../assets/sounds/croak_2.ogg"), 0.49),
    (Sound::Croak, include_bytes!("../assets/sounds/croak_3.ogg"), 0.76),
    (Sound::Screech, include_bytes!("../assets/sounds/screech_1.ogg"), 0.71),
    (Sound::Screech, include_bytes!("../assets/sounds/screech_2.ogg"), 0.62),
    (Sound::Screech, include_bytes!("../assets/sounds/screech_3.ogg"), 0.66),
    (Sound::Snort, include_bytes!("../assets/sounds/snort_1.ogg"), 1.32),
    (Sound::Snort, include_bytes!("../assets/sounds/snort_2.ogg"), 1.84),
    (Sound::Snort, include_bytes!("../assets/sounds/snort_3.ogg"), 2.20),
    (Sound::Buzz, include_bytes!("../assets/sounds/buzz_1.ogg"), 0.58),
    (Sound::Buzz, include_bytes!("../assets/sounds/buzz_2.ogg"), 1.61),
    (Sound::Slither, include_bytes!("../assets/sounds/slither_1.ogg"), 1.49),
    (Sound::Hurt, include_bytes!("../assets/sounds/hurt_1.ogg"), 0.98),
    (Sound::Hurt, include_bytes!("../assets/sounds/hurt_2.ogg"), 1.41),
    (Sound::Hurt, include_bytes!("../assets/sounds/hurt_3.ogg"), 1.43),
    (Sound::Hurt, include_bytes!("../assets/sounds/hurt_4.ogg"), 1.59),
    (Sound::Fanfare, include_bytes!("../assets/sounds/fanfare_1.ogg"), 0.46),
    (Sound::Fanfare, include_bytes!("../assets/sounds/fanfare_2.ogg"), 0.41),
    (Sound::Step(Ground::Grass), include_bytes!("../assets/sounds/step_grass_1.ogg"), 1.69),
    (Sound::Step(Ground::Grass), include_bytes!("../assets/sounds/step_grass_2.ogg"), 1.16),
    (Sound::Step(Ground::Grass), include_bytes!("../assets/sounds/step_grass_3.ogg"), 1.50),
    (Sound::Step(Ground::Grass), include_bytes!("../assets/sounds/step_grass_4.ogg"), 1.87),
    (Sound::Step(Ground::Grass), include_bytes!("../assets/sounds/step_grass_5.ogg"), 1.89),
    (Sound::Step(Ground::Snow), include_bytes!("../assets/sounds/step_snow_1.ogg"), 0.90),
    (Sound::Step(Ground::Snow), include_bytes!("../assets/sounds/step_snow_2.ogg"), 0.73),
    (Sound::Step(Ground::Snow), include_bytes!("../assets/sounds/step_snow_3.ogg"), 0.78),
    (Sound::Step(Ground::Snow), include_bytes!("../assets/sounds/step_snow_4.ogg"), 1.00),
    (Sound::Step(Ground::Snow), include_bytes!("../assets/sounds/step_snow_5.ogg"), 0.78),
    (Sound::Step(Ground::Stone), include_bytes!("../assets/sounds/step_stone_1.ogg"), 2.09),
    (Sound::Step(Ground::Stone), include_bytes!("../assets/sounds/step_stone_2.ogg"), 2.34),
    (Sound::Step(Ground::Stone), include_bytes!("../assets/sounds/step_stone_3.ogg"), 2.59),
    (Sound::Step(Ground::Stone), include_bytes!("../assets/sounds/step_stone_4.ogg"), 2.18),
    (Sound::Step(Ground::Stone), include_bytes!("../assets/sounds/step_stone_5.ogg"), 2.13),
    (Sound::Step(Ground::Sand), include_bytes!("../assets/sounds/step_sand_1.ogg"), 1.29),
    (Sound::Step(Ground::Sand), include_bytes!("../assets/sounds/step_sand_2.ogg"), 1.08),
    (Sound::Step(Ground::Sand), include_bytes!("../assets/sounds/step_sand_3.ogg"), 2.50),
    (Sound::Step(Ground::Sand), include_bytes!("../assets/sounds/step_sand_4.ogg"), 1.87),
    (Sound::Step(Ground::Sand), include_bytes!("../assets/sounds/step_sand_5.ogg"), 0.57),
    (Sound::Step(Ground::Water), include_bytes!("../assets/sounds/step_water_1.ogg"), 0.83),
    (Sound::Step(Ground::Water), include_bytes!("../assets/sounds/step_water_2.ogg"), 0.44),
    (Sound::Step(Ground::Water), include_bytes!("../assets/sounds/step_water_3.ogg"), 0.83),
    (Sound::Step(Ground::Water), include_bytes!("../assets/sounds/step_water_4.ogg"), 2.00),
    (Sound::Bird, include_bytes!("../assets/sounds/bird_1.ogg"), 1.25),
    (Sound::Bird, include_bytes!("../assets/sounds/bird_2.ogg"), 1.16),
    (Sound::Bird, include_bytes!("../assets/sounds/bird_3.ogg"), 0.33),
    (Sound::Bird, include_bytes!("../assets/sounds/bird_4.ogg"), 0.65),
    (Sound::Bird, include_bytes!("../assets/sounds/bird_5.ogg"), 1.18),
    (Sound::Bird, include_bytes!("../assets/sounds/bird_6.ogg"), 1.09),
    (Sound::Bird, include_bytes!("../assets/sounds/bird_7.ogg"), 0.75),
    (Sound::Bird, include_bytes!("../assets/sounds/bird_8.ogg"), 0.91),
    (Sound::Wind, include_bytes!("../assets/sounds/wind_1.ogg"), 0.77),
    (Sound::Water, include_bytes!("../assets/sounds/water_1.ogg"), 2.42),
    (Sound::Chop, include_bytes!("../assets/sounds/chop_1.ogg"), 3.34),
    (Sound::Chop, include_bytes!("../assets/sounds/chop_2.ogg"), 2.93),
    (Sound::Chop, include_bytes!("../assets/sounds/chop_3.ogg"), 3.33),
    (Sound::Mine, include_bytes!("../assets/sounds/mine_1.ogg"), 2.10),
    (Sound::Mine, include_bytes!("../assets/sounds/mine_2.ogg"), 3.73),
    (Sound::Mine, include_bytes!("../assets/sounds/mine_3.ogg"), 2.82),
    (Sound::Dig, include_bytes!("../assets/sounds/dig_1.ogg"), 1.95),
    (Sound::Dig, include_bytes!("../assets/sounds/dig_2.ogg"), 1.50),
    (Sound::Timber, include_bytes!("../assets/sounds/timber_1.ogg"), 1.23),
    (Sound::Timber, include_bytes!("../assets/sounds/timber_2.ogg"), 1.32),
    (Sound::Place, include_bytes!("../assets/sounds/place_1.ogg"), 2.86),
    (Sound::Place, include_bytes!("../assets/sounds/place_2.ogg"), 3.80),
    (Sound::Place, include_bytes!("../assets/sounds/place_3.ogg"), 2.43),
    (Sound::DoorOpen, include_bytes!("../assets/sounds/door_open_1.ogg"), 2.23),
    (Sound::DoorOpen, include_bytes!("../assets/sounds/door_open_2.ogg"), 2.74),
    (Sound::DoorClose, include_bytes!("../assets/sounds/door_close_1.ogg"), 1.29),
    (Sound::DoorClose, include_bytes!("../assets/sounds/door_close_2.ogg"), 1.34),
    (Sound::DoorClose, include_bytes!("../assets/sounds/door_close_3.ogg"), 1.97),
    (Sound::Bang, include_bytes!("../assets/sounds/bang_1.ogg"), 2.54),
    (Sound::Bang, include_bytes!("../assets/sounds/bang_2.ogg"), 3.22),
    (Sound::Bang, include_bytes!("../assets/sounds/bang_3.ogg"), 2.54),
    (Sound::Splinter, include_bytes!("../assets/sounds/splinter_1.ogg"), 1.42),
    (Sound::Splinter, include_bytes!("../assets/sounds/splinter_2.ogg"), 1.91),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every sound there is
    fn all_sounds() -> Vec<Sound> {
        use Sound::*;
        let mut all = vec![
            Swing, HeavySwing, Stab, Flick, Hit, Draw, Twang, ArrowHit, Burst, Equip, Crackle, Chest, Pickup, Eat, Bird, Roar, Rumble, Breath, Hurt, Fanfare, Splash, Quake,
            Whistle, Claw, Web, Creak, Crack, Flap, Hiss, Growl, Howl, Bite, Rattle, Clang, Squelch, Skitter, Dodge, Gunshot, Release, Ratchet, Latch, Powder, Ram, Cock,
            Ricochet, Beam, Toll, Summon, Hex, Snap, Volley, Harpoon, Taut, Brace, Reel, Net, Chain, Chitter, Wail, Croak, Screech, Snort, Whirr, Slam, Fuse, Zap, Thunder,
            Gale, Buzz, Bubble, Billow, Shing, Chime, Rocket, Rush, Lasso, Squeeze, Boom, Slither, Sear, Swoop, Thorns, Coins, Wind, Water,
            Chop, Mine, Dig, Timber, Place, DoorOpen, DoorClose, Bang, Splinter,
        ];
        all.extend((0..9).map(Cast));
        all.extend([Ground::Grass, Ground::Sand, Ground::Stone, Ground::Snow, Ground::Water].map(Step));
        all
    }

    #[test]
    fn sounds_are_audible_and_not_too_loud() {
        let start = Instant::now();
        let library = decode_all(decoders());
        let count: usize = library.values().map(|v| v.len()).sum();
        println!("decoded {count} sounds on {} threads in {:?}", decoders(), start.elapsed());
        assert_eq!(count, RECORDINGS.len(), "some sounds didn't decode");
        for sound in all_sounds() {
            let versions = library.get(&sound).unwrap_or_else(|| panic!("{sound:?} has no recording"));
            for clip in versions {
                assert!(!clip.is_empty(), "{sound:?} is empty");
                assert!(clip.iter().all(|s| s.is_finite()), "{sound:?} has broken samples");
                let peak = clip.iter().fold(0.0f32, |m, s| m.max(s.abs()));
                println!("{sound:?}: {:.2} s, peak {peak:.2}", clip.len() as f32 / RATE as f32);
                // Played at full volume they'd get past the limiter's ceiling, but not so far it
                // has to squash them hard
                assert!(peak > 0.05 && peak < 2.6, "{sound:?} peaks at {peak}");
                if matches!(sound, Sound::Wind | Sound::Water) {
                    // Running on into its own start without a jump
                    let jump = (clip[0] - clip[clip.len() - 1]).abs();
                    assert!(jump < 0.05, "{sound:?} jumps where it loops: {jump}");
                } else {
                    // Starting and ending quietly, so it never clicks
                    let edge = clip[0].abs().max(clip[clip.len() - 1].abs());
                    assert!(edge < 0.02, "{sound:?} clicks at its start or end: {edge}");
                }
            }
        }
    }

    /// A sound on its own, not too loud, comes out of the mix just as it went in
    #[test]
    fn the_mix_leaves_a_quiet_sound_alone() {
        let library = decode_all(1);
        let clip = library[&Sound::Hit][0].clone();
        let (bus, mut mix) = bus(RATE);
        let playing = Arc::new(AtomicUsize::new(0));
        bus.add(voice(clip.clone(), 0.3, 0.0, 1.0, 0.0, &playing));
        let side = 0.3 * (PI * 0.25).cos();
        let mut worst = 0.0f32;
        for &s in clip.iter() {
            let (left, right) = (mix.next().unwrap(), mix.next().unwrap());
            worst = worst.max((left - s * side).abs()).max((right - s * side).abs());
        }
        assert!(worst < 1e-4, "the mix changed a quiet sound by {worst}");
    }

    /// The loudest sounds, dozens at once at full volume, never take the mix past full scale, and
    /// every one of them is let go of once it's over, so nothing builds up over a long game
    #[test]
    fn the_mix_never_clips_and_lets_sounds_go() {
        let library = decode_all(decoders());
        let (bus, mut mix) = bus(48000);
        let playing = Arc::new(AtomicUsize::new(0));
        let loud = [Sound::Gunshot, Sound::Boom, Sound::Roar, Sound::Burst, Sound::Hit, Sound::Clang];
        for (k, sound) in loud.iter().cycle().take(60).enumerate() {
            let versions = &library[sound];
            bus.add(voice(versions[k % versions.len()].clone(), 1.0, (k as f32 * 0.7).sin(), 0.9 + 0.01 * k as f32, 0.0, &playing));
        }
        assert_eq!(playing.load(Ordering::Relaxed), 60);
        let mut peak = 0.0f32;
        // Four seconds, longer than any of them
        for _ in 0..48000 * 2 * 4 {
            peak = peak.max(mix.next().unwrap().abs());
        }
        println!("peak of the mix {peak:.3}");
        assert!(peak <= 1.0, "the mix clipped: {peak}");
        assert!(peak > CEILING * 0.9, "the mix was squashed too far: {peak}");
        assert_eq!(playing.load(Ordering::Relaxed), 0, "sounds were kept after they ended");
    }

    #[test]
    fn far_sounds_are_duller_but_near_ones_untouched() {
        assert_eq!(muffle(0.0), 1.0);
        assert_eq!(muffle(9.0), 1.0);
        assert!(muffle(20.0) < 1.0 && muffle(60.0) < muffle(20.0) && muffle(1000.0) > 0.2);
    }

    /// `INNERT_SOUND_DUMP=<folder> cargo test --release dump_sounds -- --ignored` writes every
    /// sound as it's played, gain and all, to wave files, to listen to them outside the game
    #[test]
    #[ignore]
    fn dump_sounds() {
        let dir = std::env::var("INNERT_SOUND_DUMP").expect("INNERT_SOUND_DUMP names the folder to write to");
        let library = decode_all(decoders());
        for (sound, versions) in &library {
            let name = format!("{sound:?}").replace("Ground::", "").replace(['(', ')'], "_");
            for (k, clip) in versions.iter().enumerate() {
                let data = (clip.len() * 4) as u32;
                let mut bytes = Vec::with_capacity(44 + data as usize);
                bytes.extend(b"RIFF");
                bytes.extend((36 + data).to_le_bytes());
                bytes.extend(b"WAVEfmt ");
                bytes.extend(16u32.to_le_bytes());
                // Floating point, mono
                bytes.extend(3u16.to_le_bytes());
                bytes.extend(1u16.to_le_bytes());
                bytes.extend(RATE.to_le_bytes());
                bytes.extend((RATE * 4).to_le_bytes());
                bytes.extend(4u16.to_le_bytes());
                bytes.extend(32u16.to_le_bytes());
                bytes.extend(b"data");
                bytes.extend(data.to_le_bytes());
                for s in clip.iter() {
                    bytes.extend(s.to_le_bytes());
                }
                std::fs::write(format!("{dir}/{}{}.wav", name.trim_end_matches('_'), k + 1), bytes).unwrap();
            }
        }
    }
}
