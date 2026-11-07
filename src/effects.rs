//! Effect models baked ahead of time, so what goes off in a fight looks rich without drawing any
//! more of them: explosions in stages, from a white-hot core through a fireball billowing out to
//! smoke breaking up and drifting off, each element its own way; rings of what they throw up where
//! they go off by the ground; ice bursting out, cracking and shattering; pillars of light; sparks as
//! streaks with hot heads; orbs churning; spells with bright heads; and the blood moon. Most are made
//! voxel by voxel from noise rather than out of shapes, and go after the rest of
//! weapons::effect_models(), see BLASTS and the rest; orb() and bolt() are among them, see
//! weapons::SLOT_FIREBALL and SLOT_BOLT.

use std::f64::consts::TAU;
use std::sync::atomic::{AtomicUsize, Ordering};

use nalgebra::Vector3;

use crate::animals::{MODEL_RES, Model, ball, limb, voxelize};
use crate::weapons::{ELEMENT_LOOKS, Element, SERPENT};
use crate::worldgen::material::*;

/// Stages an explosion is drawn in: BLAST_LIVE while it goes off, see blast_stage(), then the rest
/// as its smoke drifts off after it
pub const BLAST_STAGES: usize = 8;
pub const BLAST_LIVE: usize = 6;
/// Where explosions start among weapons::effect_models(): stage `s` of look `look` (see
/// Element::look) is BLASTS + look * BLAST_STAGES + s, its middle at 0
pub const BLASTS: usize = SERPENT + 1;
/// A ring of what an explosion of each look throws up round it where it goes off by the ground, a
/// meter round, lying flat round 0
pub const BLAST_RINGS: usize = BLASTS + ELEMENT_LOOKS * BLAST_STAGES;
/// Spikes of ice bursting out of a glowing heart, its middle at 0: whole, cracked, then shattered
pub const ICE_STAGES: usize = BLAST_RINGS + ELEMENT_LOOKS;
pub const ICE_STAGE_COUNT: usize = 3;
/// A pillar of light six meters high standing on 0: blazing, then thinning away
pub const PILLAR_STAGES: usize = ICE_STAGES + ICE_STAGE_COUNT;
pub const PILLAR_STAGE_COUNT: usize = 2;
/// The blood moon, a meter round, its face along +z with a glowing rim round it
pub const MOON: usize = PILLAR_STAGES + PILLAR_STAGE_COUNT;
/// A spark as a streak along -x behind its hot head at 0, for each look
pub const STREAKS: usize = MOON + 1;
/// How many effect models there are in all
#[cfg(test)]
pub const EFFECT_MODELS: usize = STREAKS + ELEMENT_LOOKS;

/// Voxels per meter of each
pub const BLAST_RES: f64 = MODEL_RES;
pub const BLAST_RING_RES: f64 = 32.0;
pub const ICE_RES: f64 = 24.0;
pub const PILLAR_RES: f64 = MODEL_RES;
pub const MOON_RES: f64 = 32.0;
pub const STREAK_RES: f64 = 32.0;
/// How far an explosion model reaches from its middle at most, its rays, shards and debris
/// included, at size 1
pub const BLAST_REACH: f64 = 1.5;

/// Which stage of an explosion of `look` to draw `t` of the way through it, 0-1: the white-hot
/// core first, then the fireball forming, billowing, breaking up, dissolving, and wisps of it. Void
/// draws in on itself a little longer first.
pub fn blast_stage(look: usize, t: f64) -> usize {
    let ends = if element_of(look) == Some(Element::Void) { [0.14, 0.27, 0.42, 0.58, 0.76] } else { [0.08, 0.2, 0.38, 0.56, 0.75] };
    ends.iter().position(|&end| t < end).unwrap_or(BLAST_LIVE - 1)
}

/// Whether an explosion of `look` leaves smoke drifting off after it, rather than light just going
pub fn leaves_smoke(look: usize) -> bool {
    !matches!(element_of(look), Some(Element::Storm | Element::Arcane | Element::Holy))
}

fn element_of(look: usize) -> Option<Element> {
    look.checked_sub(1).and_then(|k| Element::ALL.get(k).copied())
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^ (x >> 16)
}

/// 0-1 from a hash
fn unit(h: u32) -> f64 {
    (h >> 8) as f64 / 16777216.0
}

fn random(seed: &mut u32) -> f64 {
    *seed = hash(seed.wrapping_add(0x9e3779b9));
    unit(*seed)
}

/// -1 to 1 at a corner of the lattice
fn lattice(x: i32, y: i32, z: i32, seed: u32) -> f64 {
    let h = hash((x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ (z as u32).wrapping_mul(0xcb1a_b31f) ^ seed);
    unit(h) * 2.0 - 1.0
}

/// Smooth value noise, -1 to 1 though mostly well inside that, changing over about a unit
fn noise(p: [f64; 3], seed: u32) -> f64 {
    let f = p.map(f64::floor);
    let i = f.map(|v| v as i32);
    let t = [0, 1, 2].map(|k| {
        let u = p[k] - f[k];
        u * u * (3.0 - 2.0 * u)
    });
    let mut sum = 0.0;
    for c in 0..8 {
        let d = [c & 1, (c >> 1) & 1, (c >> 2) & 1];
        let w: f64 = (0..3).map(|k| if d[k] == 1 { t[k] } else { 1.0 - t[k] }).product();
        sum += w * lattice(i[0] + d[0], i[1] + d[1], i[2] + d[2], seed);
    }
    sum
}

/// Noise with finer noise over it, for billows and the like
fn fbm(p: [f64; 3], seed: u32) -> f64 {
    noise(p, seed) * 0.62 + noise([p[0] * 2.07 + 5.3, p[1] * 2.07 + 1.7, p[2] * 2.07 + 9.1], seed ^ 0x51ed) * 0.28 + noise([p[0] * 4.3 + 2.9, p[1] * 4.3 + 7.7, p[2] * 4.3 + 3.1], seed ^ 0x0a11) * 0.1
}

fn scale(p: [f64; 3], s: f64) -> [f64; 3] {
    p.map(|v| v * s)
}

fn length(p: [f64; 3]) -> f64 {
    (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt()
}

fn lerp(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

fn normalized(p: [f64; 3]) -> [f64; 3] {
    let n = length(p).max(1e-9);
    p.map(|v| v / n)
}

/// A direction picked at random, rising by `up` more than it would
fn direction(seed: &mut u32, up: f64) -> [f64; 3] {
    loop {
        let v = [random(seed) * 2.0 - 1.0, random(seed) * 2.0 - 1.0, random(seed) * 2.0 - 1.0];
        let n = length(v);
        if n > 0.1 && n <= 1.0 {
            return normalized([v[0] / n, v[1] / n, v[2] / n + up]);
        }
    }
}

/// Two directions square to `n` and to each other
fn across(n: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let n = normalized(n);
    let other = if n[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
    let u = normalized([n[1] * other[2] - n[2] * other[1], n[2] * other[0] - n[0] * other[2], n[0] * other[1] - n[1] * other[0]]);
    let v = [n[1] * u[2] - n[2] * u[1], n[2] * u[0] - n[0] * u[2], n[0] * u[1] - n[1] * u[0]];
    (u, v)
}

/// Voxels being filled in, `res` to the meter, from `lo` to `hi` in meters
struct Grid {
    n: [usize; 3],
    lo: [f64; 3],
    res: f64,
    voxels: Vec<u8>,
}

impl Grid {
    fn span(lo: [f64; 3], hi: [f64; 3], res: f64) -> Grid {
        let n = [0, 1, 2].map(|i| (((hi[i] - lo[i]) * res).ceil() as usize).clamp(1, 1023));
        Grid { n, lo, res, voxels: vec![AIR; n[0] * n[1] * n[2]] }
    }

    /// Reaching `half` meters out from 0 every way
    fn round(half: f64, res: f64) -> Grid {
        Grid::span([-half; 3], [half; 3], res)
    }

    fn middle(&self, x: usize, y: usize, z: usize) -> [f64; 3] {
        [self.lo[0] + (x as f64 + 0.5) / self.res, self.lo[1] + (y as f64 + 0.5) / self.res, self.lo[2] + (z as f64 + 0.5) / self.res]
    }

    /// Every voxel what `f` says is at its middle, given in meters with a number different for
    /// each voxel to pick things at random by
    fn fill(&mut self, f: impl Fn([f64; 3], u32) -> u8) {
        let n = self.n;
        for z in 0..n[2] {
            for y in 0..n[1] {
                for x in 0..n[0] {
                    let i = x + (y + z * n[1]) * n[0];
                    let m = f(self.middle(x, y, z), hash(i as u32 ^ 0x2545_f491));
                    if m != AIR {
                        self.voxels[i] = m;
                    }
                }
            }
        }
    }

    /// The voxels within `r` of `c`, at least the one it's in
    fn each_in(&mut self, c: [f64; 3], r: f64, mut f: impl FnMut(&mut u8, [f64; 3])) {
        let from = [0, 1, 2].map(|i| (((c[i] - r - self.lo[i]) * self.res).floor().max(0.0) as usize).min(self.n[i]));
        let to = [0, 1, 2].map(|i| (((c[i] + r - self.lo[i]) * self.res).ceil().max(0.0) as usize).min(self.n[i]));
        for z in from[2]..to[2] {
            for y in from[1]..to[1] {
                for x in from[0]..to[0] {
                    let p = self.middle(x, y, z);
                    let d = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
                    if d[0] * d[0] + d[1] * d[1] + d[2] * d[2] <= r * r {
                        let i = x + (y + z * self.n[1]) * self.n[0];
                        f(&mut self.voxels[i], d);
                    }
                }
            }
        }
    }

    /// The one voxel `p` is in
    fn dot(&mut self, p: [f64; 3], material: u8) {
        let c = [0, 1, 2].map(|i| ((p[i] - self.lo[i]) * self.res).floor());
        if (0..3).all(|i| c[i] >= 0.0 && (c[i] as usize) < self.n[i]) {
            let i = c[0] as usize + (c[1] as usize + c[2] as usize * self.n[1]) * self.n[0];
            self.voxels[i] = material;
        }
    }

    /// A ball `r` meters round, never thinner than a voxel; only where there's nothing yet with
    /// `under`
    fn ball(&mut self, c: [f64; 3], r: f64, material: u8, under: bool) {
        let r = r.max(0.75 / self.res);
        self.each_in(c, r, |v, _| {
            if !under || *v == AIR {
                *v = material;
            }
        });
    }

    /// Balls from `a` to `b`, `ra` round at `a` to `rb` at `b`
    fn line(&mut self, a: [f64; 3], b: [f64; 3], ra: f64, rb: f64, material: u8, under: bool) {
        let steps = ((length([b[0] - a[0], b[1] - a[1], b[2] - a[2]]) * self.res * 2.0).ceil() as usize).max(1);
        for k in 0..=steps {
            let t = k as f64 / steps as f64;
            self.ball(lerp(a, b, t), ra + (rb - ra) * t, material, under);
        }
    }

    /// A hollow ball: a shell `thick` meters thick of `material`, air inside it, and with `open` the
    /// top of it gone, burst
    fn bubble(&mut self, c: [f64; 3], r: f64, thick: f64, material: u8, open: bool) {
        self.each_in(c, r, |v, d| {
            let inside = d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < (r - thick) * (r - thick);
            if inside || (open && d[2] > r * 0.35) {
                *v = AIR;
            } else {
                *v = material;
            }
        });
    }

    /// Trimmed to what's in it, with where the shader puts the model's middle of its bottom, as
    /// voxelize() gives
    fn model(self) -> (Model, Vector3<f64>) {
        let n = self.n;
        let (mut a, mut b) = ([usize::MAX; 3], [0usize; 3]);
        for z in 0..n[2] {
            for y in 0..n[1] {
                for x in 0..n[0] {
                    if self.voxels[x + (y + z * n[1]) * n[0]] != AIR {
                        for (i, c) in [x, y, z].into_iter().enumerate() {
                            a[i] = a[i].min(c);
                            b[i] = b[i].max(c);
                        }
                    }
                }
            }
        }
        if a[0] == usize::MAX {
            return (Model { size: [1, 1, 1], voxels: vec![AIR] }, Vector3::zeros());
        }
        let size = [0, 1, 2].map(|i| b[i] - a[i] + 1);
        let mut voxels = vec![AIR; size[0] * size[1] * size[2]];
        for z in 0..size[2] {
            for y in 0..size[1] {
                let from = a[0] + (a[1] + y + (a[2] + z) * n[1]) * n[0];
                let to = (y + z * size[1]) * size[0];
                voxels[to..to + size[0]].copy_from_slice(&self.voxels[from..from + size[0]]);
            }
        }
        let res = self.res;
        let anchor = Vector3::new(self.lo[0] + (a[0] as f64 + size[0] as f64 * 0.5) / res, self.lo[1] + (a[1] as f64 + size[1] as f64 * 0.5) / res, self.lo[2] + a[2] as f64 / res);
        (Model { size: size.map(|s| s as u32), voxels }, anchor)
    }
}

/// What an explosion of an element is made of, from its hottest to the smoke it leaves: white-hot,
/// hot, its own glow, embers glowing deep in its edge, its edge, then two kinds of smoke, dust or
/// mist. The edge and the smoke don't glow, so the light falls on them and shows their shape.
struct Palette {
    white: u8,
    hot: u8,
    glow: u8,
    ember: u8,
    edge: u8,
    dark: u8,
    light: u8,
}

fn palette(element: Option<Element>) -> Palette {
    use Element::*;
    let p = |white, hot, glow, ember, edge, dark, light| Palette { white, hot, glow, ember, edge, dark, light };
    match element {
        // Powder: a fireball going to rust and pale powder smoke
        None => p(FLAME, GLOW_HOLY, GLOW_FIRE, GLOW_BLOOD, FUR_RED, CLAY, FUR_WHITE),
        // Flame going to dark smoke
        Some(Fire) => p(FLAME, GLOW_HOLY, GLOW_FIRE, GLOW_BLOOD, LEAVES_ORANGE, MORTAR, FUR_GRAY),
        // Cold light going to ice and mist
        Some(Frost) => p(GLOW_STORM, CRYSTAL, GLOW_FROST, CRYSTAL, ICE, ICE, FOAM),
        // Lightning in a thundercloud
        Some(Storm) => p(GLOW_HOLY, GLOW_STORM, GLOW_FROST, GLOW_STORM, STEEL, DARK_STEEL, FUR_GRAY),
        // A bubbling green cloud
        Some(Poison) => p(GLOW_POISON, GLOW_POISON, LEAVES_LIME, GLOW_POISON, MOSS, VINE, MOSS),
        Some(Arcane) => p(GLOW_HOLY, GLOW_ARCANE, GLOW_VOID, CRYSTAL, FLOWER_PURPLE, FLOWER_PURPLE, FUR_GRAY),
        // Drawn by void_shade() rather than by heat
        Some(Void) => p(GLOW_ARCANE, GLOW_VOID, GLOW_VOID, GLOW_ARCANE, OBSIDIAN, FUR_DARK, OBSIDIAN),
        Some(Blood) => p(GLOW_BLOOD, GLOW_BLOOD, CLOTH, GLOW_BLOOD, LEAVES_CRIMSON, LEAVES_CRIMSON, MEAT),
        // Light going to gold and pale motes
        Some(Holy) => p(FLAME, GLOW_HOLY, GOLD, FLAME, WAX, GOLD, LINEN),
    }
}

impl Palette {
    /// What's there `heat` hot, hottest above 1, smoke below 0.2, `grain` picking embers in the
    /// edge and which smoke
    fn at(&self, heat: f64, grain: f64) -> u8 {
        if heat > 0.92 {
            self.white
        } else if heat > 0.66 {
            self.hot
        } else if heat > 0.42 {
            self.glow
        } else if heat > 0.2 {
            if grain > 0.28 { self.ember } else { self.edge }
        } else if grain > -0.02 {
            self.dark
        } else {
            self.light
        }
    }
}

/// How each stage of an explosion looks: how far it's swollen, next to its radius, and how lumpy;
/// how much of it's broken away in clumps, and how much more voxel by voxel; how much of its middle
/// is gone; how hot its middle is and how fast that falls off out to its edge; how much its smoke
/// cools towards its top; its embers; and how far it's risen
struct Stage {
    swell: f64,
    lumps: f64,
    broken: f64,
    kept: f64,
    hollow: f64,
    heat: f64,
    falloff: f64,
    smoke: f64,
    embers: f64,
    rise: f64,
}

const STAGES: [Stage; BLAST_STAGES] = [
    Stage { swell: 0.5, lumps: 0.14, broken: 0.0, kept: 1.0, hollow: 0.0, heat: 1.8, falloff: 0.7, smoke: 0.0, embers: 0.0, rise: 0.0 },
    Stage { swell: 0.78, lumps: 0.22, broken: 0.0, kept: 1.0, hollow: 0.0, heat: 1.5, falloff: 0.85, smoke: 0.2, embers: 0.0, rise: 0.0 },
    Stage { swell: 0.95, lumps: 0.27, broken: 0.07, kept: 1.0, hollow: 0.0, heat: 1.38, falloff: 0.9, smoke: 0.4, embers: 0.0, rise: 0.03 },
    Stage { swell: 1.05, lumps: 0.3, broken: 0.3, kept: 0.9, hollow: 0.5, heat: 1.1, falloff: 0.9, smoke: 0.5, embers: 0.05, rise: 0.07 },
    Stage { swell: 1.12, lumps: 0.31, broken: 0.55, kept: 0.74, hollow: 0.62, heat: 0.78, falloff: 0.85, smoke: 0.55, embers: 0.06, rise: 0.12 },
    Stage { swell: 1.18, lumps: 0.32, broken: 0.72, kept: 0.56, hollow: 0.7, heat: 0.48, falloff: 0.8, smoke: 0.55, embers: 0.05, rise: 0.17 },
    Stage { swell: 1.24, lumps: 0.33, broken: 0.82, kept: 0.42, hollow: 0.74, heat: 0.22, falloff: 0.75, smoke: 0.55, embers: 0.03, rise: 0.22 },
    Stage { swell: 1.3, lumps: 0.34, broken: 0.9, kept: 0.3, hollow: 0.78, heat: 0.0, falloff: 0.7, smoke: 0.55, embers: 0.0, rise: 0.27 },
];

/// The value of `f` that `share` of the points in a ball `radius` round fall under
fn quantile(f: impl Fn([f64; 3]) -> f64, radius: f64, share: f64) -> f64 {
    let mut seed = 0x9a77;
    let mut values: Vec<f64> = (0..2500)
        .map(|_| {
            let d = direction(&mut seed, 0.0);
            let r = radius * random(&mut seed).cbrt();
            f(scale(d, r))
        })
        .collect();
    values.sort_by(f64::total_cmp);
    values[((share * values.len() as f64) as usize).min(values.len() - 1)]
}

/// Stage `stage` of an explosion of `look`, its middle at 0 and a meter round at full size
fn blast(look: usize, stage: usize) -> (Model, Vector3<f64>) {
    use Element::*;
    let element = element_of(look);
    let st = &STAGES[stage];
    let pal = palette(element);
    let seed = 0xb1a5 ^ (look as u32).wrapping_mul(0x9e37_79b1);
    let u = stage as f64 / (BLAST_STAGES - 1) as f64;
    let void = element == Some(Void);
    let mut g = Grid::round(BLAST_REACH, BLAST_RES);
    // Some elements' bodies are smaller, their shards, arcs, rays or spatter doing more
    let body = match element {
        Some(Frost) | Some(Storm) => 0.72,
        Some(Blood) => 0.62,
        Some(Holy) | Some(Arcane) => 0.85,
        _ => 1.0,
    };
    let swell = st.swell * body;
    // Void draws in on itself first, see implode(), then bursts as a thin shell of light broken
    // into shards round its dark heart
    let (hollow, broken) = match stage {
        2 if void => (0.8, 0.5),
        3 if void => (0.84, 0.66),
        _ => (st.hollow, st.broken),
    };
    if !(void && stage < 2) {
        let erosion = |p: [f64; 3]| fbm(scale(p, 3.1), seed ^ 0xe0de);
        let cut = if broken > 0.0 { quantile(erosion, swell, broken) } else { f64::MIN };
        let fire = element == Some(Fire);
        // Venom squats low and sinks rather than rising
        let (squat, rise) = if element == Some(Poison) { (1.3, -st.rise * 0.6) } else { (1.0, st.rise) };
        // As far as its surface can reach, so the noise needn't be worked out further out
        let furthest = (swell * (1.0 + st.lumps + 0.13) * if fire { 1.55 } else { 1.0 }).min(BLAST_REACH);
        g.fill(|p, h| {
            let q = [p[0], p[1], (p[2] - rise) * squat];
            let r = length(q);
            if r > furthest {
                return AIR;
            }
            let n = if r > 1e-6 { scale(q, 1.0 / r) } else { [0.0, 0.0, 1.0] };
            // Big lumps, the same through every stage so it grows out of itself, and billows
            // rolling up over them
            let lump = fbm(scale(n, 1.7), seed);
            let billow = fbm([q[0] * 2.4, q[1] * 2.4, q[2] * 2.4 - u * 1.6], seed ^ 0xb11);
            let mut surface = swell * (1.0 + st.lumps * lump + 0.13 * billow);
            // Tongues of flame licking up out of a fireball
            if fire && (1..=4).contains(&stage) {
                let tongue = noise([n[0] * 2.6, n[1] * 2.6, n[2] * 1.2 - u * 2.0], seed ^ 0x70e);
                surface *= 1.0 + 0.55 * tongue.max(0.0) * (n[2] + 0.3).clamp(0.0, 1.0);
            }
            if r > surface {
                return AIR;
            }
            let depth = r / surface;
            if depth < hollow || erosion(p) < cut || (st.kept < 1.0 && unit(h) > st.kept) {
                return AIR;
            }
            if st.embers > 0.0 && unit(hash(h ^ 0x3c3)) < st.embers {
                return if unit(hash(h ^ 0x77)) < 0.3 { pal.hot } else { pal.glow };
            }
            let grain = fbm(scale(q, 4.6), seed ^ 0x6a);
            if void {
                return void_shade(stage, fbm(scale(q, 3.3), seed ^ 0x7e1), grain);
            }
            // Powder throws up dust from under it; fire's smoke is blackest at its top
            if element.is_none() && stage >= 2 && unit(hash(h ^ 0xd057)) < 2.5 * (-q[2] / surface - 0.3) {
                return DIRT;
            }
            if fire && stage >= 3 && q[2] / surface > 0.45 && grain > -0.15 {
                return BARK_DARK;
            }
            let heat = st.heat - st.falloff * depth.powf(1.5) + 0.34 * billow - st.smoke * (q[2] / surface).max(0.0);
            pal.at(heat, grain)
        });
    }
    match element {
        None => debris(&mut g, seed, stage, 18, &[GRAVEL, ROCK, DIRT, BARK_DARK, GRAVEL, CLAY], None),
        Some(Fire) => debris(&mut g, seed, stage, 9, &[BARK_DARK, FUR_DARK, BARK_DARK], Some(GLOW_FIRE)),
        Some(Frost) => frost(&mut g, seed, stage),
        Some(Storm) => arcs(&mut g, seed, stage),
        Some(Poison) => bubbles(&mut g, seed, stage, swell),
        Some(Arcane) => runes(&mut g, seed, stage),
        Some(Void) => implode(&mut g, seed, stage),
        Some(Blood) => splatter(&mut g, seed, stage),
        Some(Holy) => rays(&mut g, seed, stage),
    }
    g.model()
}

/// Void's burst, after it's drawn in on itself: a thin shell of purple light broken into shards,
/// brightest along its cracks, then dark smoke with veins of its light going out in it. `vein` is
/// noise that's near 0 along the veins.
fn void_shade(stage: usize, vein: f64, grain: f64) -> u8 {
    if stage <= 3 {
        return if vein.abs() < 0.07 {
            GLOW_ARCANE
        } else if grain < -0.02 {
            if grain < -0.2 { OBSIDIAN } else { FUR_DARK }
        } else {
            GLOW_VOID
        };
    }
    let wide = [0.0, 0.0, 0.0, 0.0, 0.11, 0.08, 0.05, 0.03][stage];
    if vein.abs() < wide * 0.3 {
        GLOW_ARCANE
    } else if vein.abs() < wide {
        GLOW_VOID
    } else if grain > 0.05 {
        FUR_DARK
    } else {
        OBSIDIAN
    }
}

/// Void drawing in on itself before it bursts: a black heart ringed with light, motes spiralling
/// into it and streaks of it drawn in from all round, tighter the second stage; and after, the dark
/// heart left a moment in its shell, and specks of it flung out
fn implode(g: &mut Grid, seed: u32, stage: usize) {
    let (u, v) = across([0.2, 0.35, 1.0]);
    let around = |a: f64, r: f64| [0, 1, 2].map(|i| (u[i] * a.cos() + v[i] * a.sin()) * r);
    if stage >= 2 {
        if stage <= 3 {
            g.ball([0.0; 3], [0.36, 0.26][stage - 2], OBSIDIAN, false);
        }
        let specks = [0, 0, 26, 30, 26, 18, 10, 4][stage];
        let out = 0.9 + 0.08 * stage as f64;
        for k in 0..specks {
            let mut s = hash(seed ^ 0x5bec ^ (k as u32).wrapping_mul(0x9e37));
            let d = direction(&mut s, 0.0);
            let r = out * (0.85 + 0.35 * random(&mut s));
            g.dot(scale(d, r.min(BLAST_REACH - 0.05)), if k % 3 == 0 { GLOW_ARCANE } else { GLOW_VOID });
        }
        return;
    }
    let core = [0.4, 0.27][stage];
    g.ball([0.0; 3], core, OBSIDIAN, false);
    // A ring of light hugging it round its middle
    for k in 0..72 {
        let a = k as f64 / 72.0 * TAU;
        g.ball(around(a, core + 0.06), 0.05, if k % 3 == 0 { GLOW_ARCANE } else { GLOW_VOID }, false);
    }
    let motes = [150, 120][stage];
    let reach = [1.05, 0.62][stage];
    for k in 0..motes {
        let f = k as f64 / motes as f64;
        let mut s = hash(seed ^ 0x1a7e ^ (k as u32).wrapping_mul(0x9e37));
        let a = f * 4.0 * TAU + random(&mut s) * 0.5;
        let r = core + 0.06 + (1.0 - f) * reach * (0.85 + 0.3 * random(&mut s));
        let lift = (random(&mut s) - 0.5) * 0.12 * r;
        let at = [0, 1, 2].map(|i| (u[i] * a.cos() + v[i] * a.sin()) * r + [0.2, 0.35, 1.0][i] * lift);
        let material = if f > 0.7 { GLOW_ARCANE } else if k % 4 == 0 { FUR_DARK } else { GLOW_VOID };
        g.dot(at, material);
    }
    for k in 0..18 {
        let mut s = hash(seed ^ 0x57a4 ^ (k as u32).wrapping_mul(0x9e37));
        let d = direction(&mut s, 0.0);
        let from = core + 0.15 + (1.0 - stage as f64 * 0.5) * 0.3 * random(&mut s);
        g.line(scale(d, from), scale(d, from + 0.25), 0.03, 0.03, GLOW_VOID, true);
    }
}

/// Bits of debris flung out and falling, the same bits through every stage, some of them burning
fn debris(g: &mut Grid, seed: u32, stage: usize, count: usize, materials: &[u8], burning: Option<u8>) {
    if stage == 0 || stage >= 7 {
        return;
    }
    let out = [0.0, 0.62, 0.95, 1.12, 1.24, 1.31, 1.36][stage];
    let drop = 0.55 * (stage as f64 / 6.0).powi(2);
    for k in 0..count {
        let mut s = hash(seed ^ 0xdeb ^ (k as u32).wrapping_mul(0x9e37));
        let d = direction(&mut s, 0.45);
        let size = 0.045 + 0.055 * random(&mut s);
        let far = out * (0.75 + 0.25 * random(&mut s));
        let c = [d[0] * far, d[1] * far, d[2] * far - drop];
        if length(c) > BLAST_REACH - size {
            continue;
        }
        let material = match burning {
            Some(b) if k % 2 == 0 && stage <= 4 => b,
            _ => materials[k % materials.len()],
        };
        g.ball(c, size, material, false);
    }
}

/// Shards of ice thrusting out of a frost burst, then cracking, breaking off and falling, and cold
/// mist rolling out low round it
fn frost(g: &mut Grid, seed: u32, stage: usize) {
    let reach = [0.55, 1.1, 1.28, 1.3, 1.28, 1.2, 0.0, 0.0][stage];
    for k in 0..11 {
        let mut s = hash(seed ^ 0x5a4d ^ (k as u32).wrapping_mul(0x9e37));
        let a = k as f64 / 11.0 * TAU + random(&mut s) * 0.4;
        let up = -0.3 + 1.15 * random(&mut s);
        let out = (1.0 - up * up).max(0.0).sqrt();
        let d = [a.cos() * out, a.sin() * out, up];
        let long = reach * (0.6 + 0.4 * random(&mut s));
        let base = 0.1 * (0.8 + 0.4 * random(&mut s)) * if stage == 0 { 0.6 } else { 1.0 };
        if long <= 0.0 {
            continue;
        }
        let (side, _) = across(d);
        let shard = |g: &mut Grid, from: f64, to: f64, shift: [f64; 3]| {
            let a = [0, 1, 2].map(|i| d[i] * from * long + shift[i]);
            let b = [0, 1, 2].map(|i| d[i] * to * long + shift[i]);
            let ra = base * (1.0 - from * 0.85);
            let rb = base * (1.0 - to * 0.85);
            g.line(a, b, ra, rb, ICE, false);
            // A glowing edge down one side of it, and its tip
            g.line([0, 1, 2].map(|i| a[i] + side[i] * ra * 0.7), [0, 1, 2].map(|i| b[i] + side[i] * rb * 0.7), 0.02, 0.012, CRYSTAL, false);
            if to > 0.95 {
                g.ball(b, 0.035, GLOW_FROST, false);
            }
        };
        if stage <= 3 {
            shard(g, 0.0, 1.0, [0.0; 3]);
        } else {
            // Cracked across, the end of it flying on and falling
            let fall = (stage - 3) as f64;
            shard(g, 0.0, 0.5, [0.0; 3]);
            shard(g, 0.6, 1.0, [d[0] * 0.08 * fall, d[1] * 0.08 * fall, -0.09 * fall * fall]);
        }
    }
    // Bits of ice falling once it's gone
    if stage >= 6 {
        for k in 0..20 {
            let mut s = hash(seed ^ 0x1ce ^ (k as u32).wrapping_mul(0x9e37));
            let d = direction(&mut s, 0.2);
            let c = [d[0] * 1.1, d[1] * 1.1, d[2] * 0.6 - 0.25 * (stage - 5) as f64];
            g.ball(c, 0.04, if k % 3 == 0 { CRYSTAL } else { ICE }, false);
        }
    }
    // Mist rolling out low round it in a ring, further each stage, thinning at the end
    if stage >= 1 {
        let puffs = [0, 16, 22, 26, 26, 22, 18, 12][stage];
        let spread = 0.4 + 0.12 * stage as f64;
        for k in 0..puffs {
            let mut s = hash(seed ^ 0x3157 ^ (k as u32).wrapping_mul(0x9e37));
            let a = k as f64 / puffs as f64 * TAU + random(&mut s) * 0.4;
            let r = spread * (0.8 + 0.3 * random(&mut s));
            let c = [a.cos() * r, a.sin() * r, -0.3 - 0.06 * random(&mut s) - 0.02 * stage as f64];
            g.ball(c, 0.07 + 0.07 * random(&mut s), [FOAM, SNOW, ICE][k % 3], true);
        }
    }
}

/// Lightning crackling out of a storm burst, different arcs each stage so it flickers, forking
fn arcs(g: &mut Grid, seed: u32, stage: usize) {
    if stage >= BLAST_LIVE {
        return;
    }
    let count = [6, 6, 5, 4, 3, 2][stage];
    let reach = [0.85, 1.3, 1.4, 1.4, 1.35, 1.25][stage];
    for k in 0..count {
        let mut s = hash(seed ^ (stage as u32).wrapping_mul(0x51ab) ^ (k as u32).wrapping_mul(0x9e37));
        let mut way = direction(&mut s, 0.0);
        let mut at = [0.0; 3];
        let steps = 9;
        for j in 0..steps {
            way = normalized([0, 1, 2].map(|i| way[i] + (random(&mut s) - 0.5) * 0.9));
            let next = [0, 1, 2].map(|i| at[i] + way[i] * reach / steps as f64);
            if length(next) > BLAST_REACH - 0.05 {
                break;
            }
            g.line(at, next, 0.06, 0.05, if j < 3 { GLOW_HOLY } else { GLOW_STORM }, false);
            // A fork off it now and then
            if j == 4 && random(&mut s) < 0.7 {
                let mut fork = at;
                let mut turn = normalized([0, 1, 2].map(|i| way[i] + (random(&mut s) - 0.5) * 1.6));
                for _ in 0..3 {
                    turn = normalized([0, 1, 2].map(|i| turn[i] + (random(&mut s) - 0.5) * 0.8));
                    let next = [0, 1, 2].map(|i| fork[i] + turn[i] * reach / steps as f64);
                    if length(next) > BLAST_REACH - 0.05 {
                        break;
                    }
                    g.line(fork, next, 0.045, 0.04, GLOW_STORM, false);
                    fork = next;
                }
            }
            at = next;
        }
    }
}

/// Bubbles swelling on a venom burst and bursting
fn bubbles(g: &mut Grid, seed: u32, stage: usize, swell: f64) {
    if stage == 0 || stage >= BLAST_LIVE {
        return;
    }
    let count = [0, 12, 18, 16, 10, 5][stage];
    for k in 0..count {
        let mut s = hash(seed ^ 0xb0b ^ (k as u32).wrapping_mul(0x9e37));
        let d = direction(&mut s, 0.3);
        let r = 0.1 + 0.13 * random(&mut s) * (0.7 + 0.15 * stage as f64);
        let sit = swell * (0.85 + 0.25 * random(&mut s));
        let c = [d[0] * sit, d[1] * sit, d[2] * sit / 1.3 - 0.03 * stage as f64];
        if length(c) + r > BLAST_REACH {
            continue;
        }
        g.bubble(c, r, 0.065, if k % 3 == 0 { LEAVES_LIME } else { GLOW_POISON }, stage >= 4 && k % 2 == 0);
    }
}

/// Glints flashing round an arcane burst, and two rings of rune flecks turning out from it
fn runes(g: &mut Grid, seed: u32, stage: usize) {
    let glints = [6, 20, 30, 30, 24, 16, 8, 0][stage];
    for k in 0..glints {
        let mut s = hash(seed ^ 0x611 ^ (k as u32).wrapping_mul(0x9e37));
        let d = direction(&mut s, 0.0);
        let r = 0.4 + 0.95 * random(&mut s);
        g.dot(scale(d, r), if k % 4 == 0 { GLOW_HOLY } else { CRYSTAL });
    }
    if !(1..BLAST_LIVE).contains(&stage) {
        return;
    }
    for (ring, (normal, radius, material)) in [([0.25, 0.15, 1.0], 1.0, GLOW_HOLY), ([0.9, 0.3, 0.35], 1.12, GLOW_ARCANE)].into_iter().enumerate() {
        let (u, v) = across(normal);
        let radius = radius * (0.82 + 0.05 * stage as f64);
        let marks = 36;
        for j in 0..marks {
            let h = hash(seed ^ ((ring * 97 + j) as u32).wrapping_mul(0x9e37));
            // Breaking up as it fades
            if h % 3 == 0 || (stage >= 4 && h % 5 < 2) {
                continue;
            }
            let a0 = (j as f64 + 0.15) / marks as f64 * TAU + stage as f64 * 0.12 * if ring == 0 { 1.0 } else { -1.0 };
            let a1 = a0 + 0.55 * TAU / marks as f64;
            let at = |a: f64, r: f64| [0, 1, 2].map(|i| (u[i] * a.cos() + v[i] * a.sin()) * r);
            g.line(at(a0, radius), at(a1, radius), 0.028, 0.028, material, false);
            // A tick across now and then, like the strokes of a rune
            if h % 4 == 1 {
                g.line(at(a0, radius - 0.06), at(a0, radius + 0.07), 0.025, 0.025, material, false);
            }
        }
    }
}

/// Blood thrown out in spatters of drops, flying on and falling
fn splatter(g: &mut Grid, seed: u32, stage: usize) {
    if stage == 0 || stage >= 7 {
        return;
    }
    let reach = [0.0, 0.72, 1.02, 1.2, 1.3, 1.36, 1.4][stage];
    let fall = [0.0, 0.0, 0.05, 0.14, 0.28, 0.42, 0.56][stage];
    for k in 0..16 {
        let mut s = hash(seed ^ 0x5b1 ^ (k as u32).wrapping_mul(0x9e37));
        let d = direction(&mut s, 0.35);
        let long = reach * (0.75 + 0.25 * random(&mut s));
        let drops = 5;
        for j in 0..drops {
            if stage >= 5 && j % 2 == 0 {
                continue;
            }
            let f = (j + 1) as f64 / drops as f64;
            let c = [d[0] * long * f, d[1] * long * f, d[2] * long * f - fall * f * f];
            if length(c) > BLAST_REACH - 0.08 {
                continue;
            }
            let r = 0.1 * (1.0 - 0.5 * f) * if stage >= 5 { 0.8 } else { 1.0 };
            let material = if j == 0 && stage <= 3 { GLOW_BLOOD } else if j % 2 == 1 { CLOTH } else { LEAVES_CRIMSON };
            g.ball(c, r, material, false);
        }
        if stage <= 3 {
            g.line(scale(d, 0.2), [d[0] * long * 0.55, d[1] * long * 0.55, d[2] * long * 0.55 - fall * 0.3], 0.055, 0.03, CLOTH, true);
        }
    }
}

/// Rays of light shooting out of a holy burst, long and short, and motes of light drifting up
fn rays(g: &mut Grid, seed: u32, stage: usize) {
    let reach = [0.95, 1.42, 1.48, 1.4, 1.15, 0.8, 0.0, 0.0][stage];
    if reach > 0.0 {
        for k in 0..14 {
            let mut s = hash(seed ^ 0x7a5 ^ (k as u32).wrapping_mul(0x9e37));
            let a = k as f64 / 14.0 * TAU + random(&mut s) * 0.3;
            let up = (random(&mut s) * 1.1 - 0.15).clamp(-0.3, 0.95);
            let out = (1.0 - up * up).max(0.0).sqrt();
            let d = [a.cos() * out, a.sin() * out, up];
            let long = (reach * (0.75 + 0.25 * random(&mut s)) * if k % 2 == 1 { 0.7 } else { 1.0 }).min(BLAST_REACH - 0.05);
            g.line(scale(d, 0.1), scale(d, long), 0.065, 0.016, GLOW_HOLY, true);
        }
    }
    let motes = [8, 20, 34, 40, 36, 30, 22, 12][stage];
    for k in 0..motes {
        let mut s = hash(seed ^ 0x707 ^ (k as u32).wrapping_mul(0x9e37));
        let d = direction(&mut s, 0.0);
        let r = 0.5 + 0.8 * random(&mut s);
        let c = [d[0] * r, d[1] * r, (d[2] * r + 0.06 * stage as f64).min(BLAST_REACH - 0.05)];
        g.dot(c, if k % 3 == 0 { GOLD } else { GLOW_HOLY });
    }
}

/// A ring of what an explosion of `look` throws up round it on the ground: lumps of it, dust, and
/// a little of its glow
fn blast_ring(look: usize) -> (Model, Vector3<f64>) {
    use Element::*;
    // The lumps, a second kind, its glow, and dust
    let m: [u8; 4] = match element_of(look) {
        None => [DIRT, GRAVEL, GLOW_FIRE, CLAY],
        Some(Fire) => [BARK_DARK, FUR_DARK, GLOW_FIRE, MORTAR],
        Some(Frost) => [SNOW, ICE, CRYSTAL, FOAM],
        Some(Storm) => [FUR_GRAY, MORTAR, GLOW_STORM, CLAY],
        Some(Poison) => [MOSS, VINE, GLOW_POISON, LEAVES_LIME],
        Some(Arcane) => [FLOWER_PURPLE, FUR_GRAY, GLOW_ARCANE, CRYSTAL],
        Some(Void) => [OBSIDIAN, FUR_DARK, GLOW_VOID, BARK_DARK],
        Some(Blood) => [CLOTH, LEAVES_CRIMSON, GLOW_BLOOD, MEAT],
        Some(Holy) => [GOLD, WAX, GLOW_HOLY, LINEN],
    };
    let mut g = Grid::span([-1.25, -1.25, -0.05], [1.25, 1.25, 0.32], BLAST_RING_RES);
    let mut s = 0x41a7 + look as u32;
    for k in 0..72 {
        let a = (k as f64 + random(&mut s) * 0.7) / 72.0 * TAU;
        let r = 1.0 + (random(&mut s) - 0.5) * 0.1;
        let size = 0.04 + 0.045 * random(&mut s);
        let material = match k % 9 {
            0 | 5 => m[2],
            1 | 4 | 7 => m[1],
            3 => m[3],
            _ => m[0],
        };
        g.ball([a.cos() * r, a.sin() * r, size * 0.6], size, material, false);
        // Dust billowing up behind every few
        if k % 4 == 0 {
            let r = r - 0.07;
            g.ball([a.cos() * r, a.sin() * r, size * 1.6], size * 1.4, m[3], true);
        }
    }
    // Specks thrown up inside it
    for _ in 0..40 {
        let a = random(&mut s) * TAU;
        let r = 0.78 + 0.12 * random(&mut s);
        g.dot([a.cos() * r, a.sin() * r, 0.03 + 0.08 * random(&mut s)], if random(&mut s) < 0.3 { m[2] } else { m[1] });
    }
    g.model()
}

/// Spikes of ice burst out of a glowing heart, frost mist round its foot: whole, then cracked
/// across, then shattered to bits falling
fn ice(stage: usize) -> (Model, Vector3<f64>) {
    let mut g = Grid::span([-1.2, -1.2, -0.5], [1.2, 1.2, 1.2], ICE_RES);
    if stage < 2 {
        g.ball([0.0; 3], 0.21, CRYSTAL, false);
        g.ball([0.0; 3], 0.15, GLOW_FROST, false);
    }
    for k in 0..13 {
        let mut s = hash(0x1ce5 ^ (k as u32).wrapping_mul(0x9e37));
        let a = k as f64 / 13.0 * TAU + random(&mut s) * 0.4;
        let up = 0.15 + 0.85 * random(&mut s);
        let out = (1.0 - up * up).max(0.0).sqrt();
        let long = 0.55 + 0.45 * random(&mut s);
        let d = [a.cos() * out, a.sin() * out, up];
        let base = 0.1 + 0.04 * random(&mut s);
        let (side, _) = across(d);
        let shard = |g: &mut Grid, from: f64, to: f64, shift: [f64; 3]| {
            let a = [0, 1, 2].map(|i| d[i] * from * long + shift[i]);
            let b = [0, 1, 2].map(|i| d[i] * to * long + shift[i]);
            let (ra, rb) = (base * (1.0 - from * 0.88), base * (1.0 - to * 0.88));
            g.line(a, b, ra, rb, if k % 4 == 0 { CRYSTAL } else { ICE }, false);
            g.line([0, 1, 2].map(|i| a[i] + side[i] * ra * 0.7), [0, 1, 2].map(|i| b[i] + side[i] * rb * 0.7), 0.02, 0.012, CRYSTAL, false);
            if to > 0.95 {
                g.ball(b, 0.03, GLOW_FROST, false);
            }
        };
        match stage {
            0 => shard(&mut g, 0.0, 1.0, [0.0; 3]),
            1 => {
                shard(&mut g, 0.0, 0.55, [0.0; 3]);
                shard(&mut g, 0.63, 1.0, [d[0] * 0.07, d[1] * 0.07, -0.04]);
            }
            _ => {
                // Bits of it flying on and falling
                for j in 0..4 {
                    let f = (j + 1) as f64 / 5.0;
                    let c = [d[0] * long * f * 1.15, d[1] * long * f * 1.15, d[2] * long * f - 0.18 * f + (random(&mut s) - 0.5) * 0.08];
                    g.ball(c, 0.055 * (1.0 - 0.5 * f), if j % 2 == 0 { ICE } else { CRYSTAL }, false);
                }
            }
        }
    }
    // Frost mist round its foot, spreading
    let mut s = 0x3157;
    for k in 0..18 {
        let a = k as f64 / 18.0 * TAU + random(&mut s) * 0.4;
        let r = 0.35 + 0.45 * random(&mut s) + 0.15 * stage as f64;
        g.ball([a.cos() * r, a.sin() * r, -0.12 - 0.12 * random(&mut s)], 0.1 + 0.08 * random(&mut s), if k % 2 == 0 { FOAM } else { SNOW }, true);
    }
    g.model()
}

/// A pillar of light six meters high: spirals of white-hot light and gold wound round it, motes
/// rising round it and a blaze spreading over the ground at its foot; then thinning, breaking up
fn pillar(stage: usize) -> (Model, Vector3<f64>) {
    let mut g = Grid::span([-1.3, -1.3, 0.0], [1.3, 1.3, 6.2], PILLAR_RES);
    let fading = stage == 1;
    let (wide, narrow) = if fading { (0.2, 0.12) } else { (0.3, 0.2) };
    // Spirals of hotter light wound round it, first so they show on it
    for strand in 0..3 {
        let a0 = strand as f64 / 3.0 * TAU;
        let points: Vec<[f64; 3]> = (0..=48)
            .map(|k| {
                let t = k as f64 / 48.0;
                let a = a0 + t * 5.0 * TAU;
                let r = wide + (narrow - wide) * t + 0.01;
                [a.cos() * r, a.sin() * r, t * 6.0]
            })
            .collect();
        for (k, w) in points.windows(2).enumerate() {
            if fading && k % 3 == 0 {
                continue;
            }
            g.line(w[0], w[1], 0.045, 0.045, if strand == 0 { FLAME } else { GOLD }, false);
        }
    }
    // The column, broken into lengths as it fades
    let lengths = if fading { 10 } else { 1 };
    for k in 0..lengths {
        let (z0, z1) = (k as f64 / lengths as f64 * 6.0, (k as f64 + if fading { 0.7 } else { 1.0 }) / lengths as f64 * 6.0);
        let (r0, r1) = (wide + (narrow - wide) * z0 / 6.0, wide + (narrow - wide) * z1 / 6.0);
        g.line([0.0, 0.0, z0], [0.0, 0.0, z1], r0, r1, GLOW_HOLY, true);
    }
    // Its foot: a blaze over the ground, a ring of gold round it, and rays out over the ground
    if !fading {
        let mut s = 0xf007;
        for k in 0..8 {
            let a = k as f64 / 8.0 * TAU + random(&mut s) * 0.3;
            g.line([a.cos() * 0.4, a.sin() * 0.4, 0.04], [a.cos() * 1.25, a.sin() * 1.25, 0.03], 0.05, 0.02, GLOW_HOLY, true);
        }
        for k in 0..40 {
            let a = k as f64 / 40.0 * TAU;
            g.ball([a.cos() * 0.75, a.sin() * 0.75, 0.05], 0.05, GOLD, true);
        }
    }
    let (rx, rz) = if fading { (0.45, 0.07) } else { (0.7, 0.12) };
    g.each_in([0.0, 0.0, 0.1], rx, |v, d| {
        if *v == AIR && (d[0] * d[0] + d[1] * d[1]) / (rx * rx) + d[2] * d[2] / (rz * rz) <= 1.0 {
            *v = GLOW_HOLY;
        }
    });
    // Motes rising round it
    let mut s = 0x307e + stage as u32;
    for _ in 0..if fading { 70 } else { 50 } {
        let a = random(&mut s) * TAU;
        let r = 0.38 + 0.45 * random(&mut s) + if fading { 0.2 } else { 0.0 };
        let z = 6.0 * random(&mut s).powf(1.4);
        g.dot([a.cos() * r, a.sin() * r, z], if random(&mut s) < 0.35 { GOLD } else { GLOW_HOLY });
    }
    g.model()
}

/// The blood moon, a meter round, its face along +z: dark seas and paler highlands of blood, craters
/// over it, glowing cracks across it, its rim glowing all round, and a halo of glow round that
fn moon() -> (Model, Vector3<f64>) {
    let mut s = 0x3007;
    let craters: Vec<([f64; 3], f64)> = (0..20)
        .map(|_| {
            let d = loop {
                let d = direction(&mut s, 0.0);
                if d[2] > 0.2 {
                    break d;
                }
            };
            (d, 0.08 + 0.17 * random(&mut s).powi(2))
        })
        .collect();
    // How far round each crater reaches, and its rim, as 1 less the cosine of the angle
    let craters: Vec<([f64; 3], f64, f64)> = craters.into_iter().map(|(c, size)| (c, 1.0 - size.cos(), 1.0 - (size * 1.3).cos())).collect();
    let mut g = Grid::round(1.16, MOON_RES);
    g.fill(|p, h| {
        let r = length(p);
        // Deep inside, or round the back, it's never seen
        if r < 0.92 || (p[2] < -0.1 * r && r <= 1.0) {
            return LEAVES_CRIMSON;
        }
        let n = scale(p, 1.0 / r);
        // Off its surface, the halo's only round its edge
        if r > 1.035 && n[2].abs() >= 0.32 {
            return AIR;
        }
        // In a crater, lower, or on its rim, higher
        let (mut dent, mut rim) = (0.0f64, false);
        for &(c, inner, outer) in &craters {
            let off = 1.0 - (n[0] * c[0] + n[1] * c[1] + n[2] * c[2]);
            if off < inner {
                dent = dent.max(0.06 * (1.0 - off / inner));
            } else if off < outer {
                rim = true;
            }
        }
        let surface = 1.0 - dent + if rim && dent == 0.0 { 0.02 } else { 0.0 } + 0.012 * noise(scale(n, 7.0), 0x11);
        if r > surface {
            // A halo just off its edge as it's seen from the front
            let fade = 0.5 - (r - 1.03) * 4.0;
            return if r > 1.03 && n[2].abs() < 0.32 && unit(h) < fade { GLOW_BLOOD } else { AIR };
        }
        // Its rim glowing as it's seen from the front; its back's never seen
        if n[2] < -0.1 {
            return LEAVES_CRIMSON;
        }
        if n[2] < 0.24 + 0.07 * noise(scale(n, 5.0), 0x22) {
            return GLOW_BLOOD;
        }
        let crack = fbm(scale(n, 3.6), 0xc4ac);
        if crack.abs() < 0.028 && n[2] > 0.35 {
            return GLOW_BLOOD;
        }
        if dent > 0.0 {
            return if dent > 0.035 { CLOTH } else { LEAVES_CRIMSON };
        }
        if rim {
            return MEAT;
        }
        if fbm(scale(n, 2.0), 0x3a1a) > 0.06 { LEAVES_CRIMSON } else { MEAT }
    });
    g.model()
}

/// A spark as a streak: a white-hot head at 0 and a tail of its glow trailing off along -x
fn streak(look: usize) -> (Model, Vector3<f64>) {
    use Element::*;
    let (head, body, tail) = match element_of(look) {
        None => (FLAME, GLOW_HOLY, GLOW_FIRE),
        Some(Fire) => (FLAME, GLOW_FIRE, GLOW_BLOOD),
        Some(Frost) => (GLOW_STORM, GLOW_FROST, CRYSTAL),
        Some(Storm) => (GLOW_HOLY, GLOW_STORM, GLOW_FROST),
        Some(Poison) => (GLOW_POISON, GLOW_POISON, LEAVES_LIME),
        Some(Arcane) => (GLOW_HOLY, GLOW_ARCANE, GLOW_VOID),
        Some(Void) => (GLOW_ARCANE, GLOW_VOID, OBSIDIAN),
        Some(Blood) => (GLOW_BLOOD, CLOTH, LEAVES_CRIMSON),
        Some(Holy) => (FLAME, GLOW_HOLY, GOLD),
    };
    let mut g = Grid::span([-0.2, -0.05, -0.05], [0.05, 0.05, 0.05], STREAK_RES);
    g.line([-0.05, 0.0, 0.0], [-0.16, 0.0, 0.0], 0.016, 0.012, tail, false);
    g.line([-0.012, 0.0, 0.0], [-0.06, 0.0, 0.0], 0.022, 0.017, body, false);
    g.ball([0.0; 3], 0.026, head, false);
    g.model()
}

/// A ball of `element` a meter round, churning: a white-hot heart, bands of hotter glow swirling
/// round it, bites out of its shell showing the heart, darker patches, and flecks flying off it.
/// The fireballs, orbs and such of weapons::SLOT_FIREBALL.
pub fn orb(element: Option<Element>) -> (Model, Vector3<f64>) {
    use Element::*;
    // Its heart, the bands, the rest of it, its darkest, and patches over it
    let (heart, band, body, edge, patch) = match element {
        None | Some(Fire) => (FLAME, GLOW_HOLY, GLOW_FIRE, GLOW_BLOOD, BARK_DARK),
        Some(Frost) => (GLOW_STORM, CRYSTAL, GLOW_FROST, ICE, FOAM),
        Some(Storm) => (GLOW_HOLY, GLOW_HOLY, GLOW_STORM, GLOW_FROST, STEEL),
        Some(Poison) => (GLOW_HOLY, LEAVES_LIME, GLOW_POISON, MOSS, VINE),
        Some(Arcane) => (GLOW_HOLY, GLOW_HOLY, GLOW_ARCANE, GLOW_VOID, CRYSTAL),
        // Dark, light swirling through it
        Some(Void) => (GLOW_ARCANE, GLOW_VOID, OBSIDIAN, GLOW_ARCANE, FUR_DARK),
        // Dark blood, glowing bands swirling through it
        Some(Blood) => (GLOW_BLOOD, GLOW_BLOOD, LEAVES_CRIMSON, CLOTH, MEAT),
        Some(Holy) => (FLAME, FLAME, GLOW_HOLY, GOLD, WAX),
    };
    let seed = 0x0f1e ^ element.map_or(0, |e| e as u32 + 1).wrapping_mul(0x9e37);
    let mut g = Grid::round(1.2, MODEL_RES);
    g.fill(|p, h| {
        let r = length(p);
        if r > 1.2 {
            return AIR;
        }
        let n = if r > 1e-6 { scale(p, 1.0 / r) } else { [0.0, 0.0, 1.0] };
        let surface = 1.0 + 0.08 * fbm(scale(n, 2.6), seed);
        if r > surface {
            return if r < 1.17 && unit(h) < 0.03 { band } else { AIR };
        }
        if r < 0.42 {
            return heart;
        }
        let swirl = (3.0 * p[1].atan2(p[0]) + 6.0 * p[2] + 2.5 * fbm(scale(p, 2.0), seed ^ 0x5)).sin();
        if r < 0.78 {
            return if swirl > 0.3 { band } else { body };
        }
        if fbm(scale(p, 2.8), seed ^ 0xb17e) > 0.47 {
            return AIR;
        }
        if swirl > 0.72 {
            band
        } else if fbm(scale(p, 4.0), seed ^ 0xacc) > 0.36 {
            patch
        } else if swirl < -0.55 {
            edge
        } else {
            body
        }
    });
    g.model()
}

/// A spell flying along +x, its head at 0: a bright head and a tail of its glow trailing off behind,
/// going to its darker color at the end, the weapons::SLOT_BOLT models. As big as they always were,
/// so they cost no more to draw.
pub fn bolt(element: Option<Element>) -> (Model, Vector3<f64>) {
    use Element::*;
    let (head, tail, end) = match element {
        None => (CRYSTAL, CRYSTAL, GLOW_FROST),
        Some(Fire) => (GLOW_HOLY, GLOW_FIRE, GLOW_BLOOD),
        Some(Frost) => (GLOW_STORM, GLOW_FROST, CRYSTAL),
        Some(Storm) => (GLOW_HOLY, GLOW_STORM, GLOW_FROST),
        Some(Poison) => (GLOW_POISON, GLOW_POISON, LEAVES_LIME),
        Some(Arcane) => (GLOW_HOLY, GLOW_ARCANE, GLOW_VOID),
        Some(Void) => (GLOW_ARCANE, GLOW_VOID, OBSIDIAN),
        // A drop of blood, glowing at its head
        Some(Blood) => (GLOW_BLOOD, CLOTH, LEAVES_CRIMSON),
        Some(Holy) => (FLAME, GLOW_HOLY, GOLD),
    };
    let p = [
        ball([0.0; 3], 0.07, head),
        limb([0.0; 3], [-0.12, 0.0, 0.0], 0.055, 0.036, tail),
        limb([-0.12, 0.0, 0.0], [-0.28, 0.0, 0.0], 0.036, 0.012, end),
    ];
    voxelize(&p, MODEL_RES)
}

type Job = Box<dyn Fn() -> (Model, Vector3<f64>) + Sync>;

/// The models from BLASTS on, made on every core at once
pub fn models() -> Vec<(Model, Vector3<f64>)> {
    let mut jobs: Vec<Job> = Vec::new();
    for look in 0..ELEMENT_LOOKS {
        for stage in 0..BLAST_STAGES {
            jobs.push(Box::new(move || blast(look, stage)));
        }
    }
    for look in 0..ELEMENT_LOOKS {
        jobs.push(Box::new(move || blast_ring(look)));
    }
    for stage in 0..ICE_STAGE_COUNT {
        jobs.push(Box::new(move || ice(stage)));
    }
    for stage in 0..PILLAR_STAGE_COUNT {
        jobs.push(Box::new(move || pillar(stage)));
    }
    jobs.push(Box::new(moon));
    for look in 0..ELEMENT_LOOKS {
        jobs.push(Box::new(move || streak(look)));
    }
    let next = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 16);
    let mut made: Vec<(usize, (Model, Vector3<f64>))> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let k = next.fetch_add(1, Ordering::Relaxed);
                        if k >= jobs.len() {
                            break out;
                        }
                        out.push((k, jobs[k]()));
                    }
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("making an effect model failed")).collect()
    });
    made.sort_unstable_by_key(|(k, _)| *k);
    made.into_iter().map(|(_, m)| m).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every stage of every explosion has something in it, the stages after the first two thin
    /// out as they go, and none reaches past BLAST_REACH. `cargo test --release effect_models --
    /// --nocapture` says how big they are and how long they take to make.
    #[test]
    fn effect_models_thin_out_and_fit() {
        let start = std::time::Instant::now();
        let models = models();
        let took = start.elapsed();
        assert_eq!(models.len(), EFFECT_MODELS - BLASTS);
        let filled = |(m, _): &(Model, Vector3<f64>)| m.voxels.iter().filter(|&&v| v != AIR).count();
        assert!(models.iter().all(|m| filled(m) > 0), "an empty effect model");
        for look in 0..ELEMENT_LOOKS {
            let counts: Vec<usize> = (0..BLAST_STAGES).map(|s| filled(&models[look * BLAST_STAGES + s])).collect();
            assert!(counts.iter().all(|&c| c > 0), "look {look}: {counts:?}");
            assert!(counts[BLAST_STAGES - 1] < counts[2], "look {look} thins out: {counts:?}");
            for s in 0..BLAST_STAGES {
                let (m, anchor) = &models[look * BLAST_STAGES + s];
                // Where its far corners are from its middle, in meters
                let half = [0, 1].map(|i| m.size[i] as f64 * 0.5 / BLAST_RES);
                assert!(half.iter().all(|&h| h <= BLAST_REACH + 0.05), "look {look} stage {s} reaches {half:?}");
                let (bottom, top) = (anchor.z, anchor.z + m.size[2] as f64 / BLAST_RES);
                assert!(bottom >= -BLAST_REACH - 0.05 && top <= BLAST_REACH + 0.05, "look {look} stage {s} from {bottom} to {top}");
            }
            println!("look {look}: {counts:?} voxels by stage");
        }
        let bytes: usize = models.iter().map(|(m, _)| m.voxels.len()).sum();
        println!("{} effect models, {:.1} MB, made in {:.0} ms", models.len(), bytes as f64 / 1e6, took.as_secs_f64() * 1000.0);
    }

    /// What's on the outside of `model`, by material: the voxels next to air
    fn outside(model: &Model) -> Vec<(u8, usize)> {
        let [sx, sy, sz] = model.size.map(|s| s as i64);
        let at = |x: i64, y: i64, z: i64| if x < 0 || y < 0 || z < 0 || x >= sx || y >= sy || z >= sz { AIR } else { model.voxels[(x + (y + z * sy) * sx) as usize] };
        let mut counts = [0usize; 256];
        for z in 0..sz {
            for y in 0..sy {
                for x in 0..sx {
                    let v = at(x, y, z);
                    if v != AIR && [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)].iter().any(|&(dx, dy, dz)| at(x + dx, y + dy, z + dz) == AIR) {
                        counts[v as usize] += 1;
                    }
                }
            }
        }
        let mut out: Vec<(u8, usize)> = (0..256).filter(|&m| counts[m] > 0).map(|m| (m as u8, counts[m])).collect();
        out.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        out
    }

    /// Never a flat ball of one color, which was what an explosion looked like going off, nor an orb:
    /// once it's past its white-hot core, no one material is more than two thirds of what shows on
    /// the outside of any stage of any explosion, or of any orb
    #[test]
    fn effects_are_never_flat_balls() {
        let models = models();
        let mut flat = Vec::new();
        let mut check = |what: String, model: &Model| {
            let shown = outside(model);
            let all: usize = shown.iter().map(|&(_, n)| n).sum();
            let (material, most) = shown[0];
            if most * 3 >= all * 2 {
                flat.push(format!("{what}: {most} of {all} on its outside are material {material}"));
            }
        };
        for look in 0..ELEMENT_LOOKS {
            for stage in 1..BLAST_STAGES {
                check(format!("look {look} stage {stage}"), &models[look * BLAST_STAGES + stage].0);
            }
            check(format!("orb {look}"), &orb(element_of(look)).0);
        }
        check("the moon".into(), &models[MOON - BLASTS].0);
        assert!(flat.is_empty(), "{flat:#?}");
    }

    /// sRGB colors of the materials, as in the palette in shaders\traverse.comp
    const PALETTE: [[f64; 3]; 89] = [
        [0.00, 0.00, 0.00], [0.28, 0.48, 0.17], [0.50, 0.35, 0.22], [0.58, 0.56, 0.55], [0.86, 0.74, 0.50], [0.95, 0.97, 1.00], [0.10, 0.30, 0.45], [0.45, 0.29, 0.17],
        [0.27, 0.54, 0.15], [0.92, 0.14, 0.14], [0.98, 0.84, 0.14], [0.80, 0.78, 0.74], [0.42, 0.56, 0.22], [0.10, 0.36, 0.10], [0.95, 0.75, 0.12], [0.95, 0.42, 0.06],
        [0.78, 0.08, 0.06], [0.58, 0.28, 0.85], [0.96, 0.96, 0.92], [0.68, 0.60, 0.54], [0.32, 0.48, 0.14], [0.90, 0.96, 0.98], [0.38, 0.63, 0.20], [0.10, 0.32, 0.20],
        [0.56, 0.53, 0.48], [0.15, 0.45, 0.95], [0.95, 0.45, 0.70], [0.40, 0.75, 0.20], [0.90, 0.88, 0.82], [0.98, 0.70, 0.80], [0.55, 0.75, 0.25], [0.85, 0.12, 0.10],
        [0.92, 0.88, 0.78], [0.55, 0.65, 0.30], [0.40, 0.25, 0.12], [0.30, 0.68, 0.15], [0.55, 0.42, 0.28], [0.45, 0.40, 0.36], [0.15, 0.12, 0.10], [0.36, 0.22, 0.12],
        [0.16, 0.10, 0.07], [0.80, 0.36, 0.12], [0.80, 0.60, 0.36], [0.52, 0.52, 0.54], [0.94, 0.92, 0.88], [0.03, 0.03, 0.03], [0.90, 0.50, 0.55], [0.92, 0.55, 0.20],
        [0.95, 0.85, 0.65], [0.88, 0.10, 0.10], [0.72, 0.24, 0.22], [0.95, 0.92, 0.84], [0.80, 0.72, 0.56], [0.50, 0.44, 0.36], [0.68, 0.20, 0.14], [0.16, 0.56, 0.54],
        [0.50, 0.95, 1.00], [1.00, 0.80, 0.45], [0.95, 0.90, 0.78], [0.88, 0.66, 0.44], [0.62, 0.42, 0.28], [0.68, 0.68, 0.64], [0.42, 0.42, 0.40], [0.86, 0.80, 0.68],
        [0.20, 0.58, 0.14], [0.80, 0.70, 0.40], [0.30, 0.56, 0.22], [0.18, 0.48, 0.10], [0.74, 0.42, 0.26], [0.80, 0.82, 0.86], [0.34, 0.36, 0.40], [0.98, 0.76, 0.26],
        [0.70, 0.44, 0.20], [0.40, 0.23, 0.13], [0.60, 0.10, 0.12], [0.86, 0.82, 0.70], [0.10, 0.08, 0.13], [0.92, 0.90, 0.82], [0.72, 0.88, 0.98], [1.00, 0.50, 0.12],
        [0.55, 0.85, 1.00], [0.70, 0.80, 1.00], [0.45, 1.00, 0.25], [0.80, 0.45, 1.00], [0.45, 0.12, 0.75], [0.95, 0.08, 0.12], [1.00, 0.92, 0.60], [0.62, 0.88, 0.16],
        [0.46, 0.03, 0.06],
    ];

    /// A pixel of a voxel of `material` facing `normal`, lit roughly as traverse.comp and post.comp
    /// would by a sun up to one side, in sRGB
    fn shade(material: u8, normal: [f64; 3]) -> [u8; 3] {
        let albedo = PALETTE[material as usize].map(|c| c.powf(2.2));
        let emission = match material {
            FLAME => [6.0, 3.2, 1.2],
            CRYSTAL => [0.5, 1.6, 2.2],
            GLOW_FIRE..=GLOW_HOLY => albedo.map(|c| c * 3.0),
            _ => [0.0; 3],
        };
        let sun = normalized([0.5, -0.4, 0.75]);
        let lambert = (normal[0] * sun[0] + normal[1] * sun[1] + normal[2] * sun[2]).max(0.0);
        let light = 0.55 + 2.2 * lambert;
        [0, 1, 2].map(|i| {
            let x = (albedo[i] * light + emission[i]) * 0.78;
            let aces = ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0);
            (aces.powf(1.0 / 2.2) * 255.0) as u8
        })
    }

    /// `model` drawn looking at it from a little above, `px` pixels square, its middle in the middle
    /// and `meters` across
    fn render(model: &(Model, Vector3<f64>), res: f64, meters: f64, px: usize, view: [f64; 3]) -> Vec<[u8; 3]> {
        let (m, anchor) = model;
        let size = m.size.map(|s| s as f64);
        // The model's voxel coordinates of meter point p: voxel = (p - anchor) * res + (size.xy / 2, 0)
        let to_voxel = |p: [f64; 3]| [(p[0] - anchor.x) * res + size[0] * 0.5, (p[1] - anchor.y) * res + size[1] * 0.5, (p[2] - anchor.z) * res];
        let d = normalized(view);
        let (right, up) = {
            let r = normalized([d[1], -d[0], 0.0]);
            (r, [r[1] * d[2] - r[2] * d[1], r[2] * d[0] - r[0] * d[2], r[0] * d[1] - r[1] * d[0]])
        };
        let mut out = vec![[24u8, 26, 30]; px * px];
        for j in 0..px {
            for i in 0..px {
                let (sx, sy) = ((i as f64 + 0.5) / px as f64 - 0.5, 0.5 - (j as f64 + 0.5) / px as f64);
                let o = [0, 1, 2].map(|k| (right[k] * sx + up[k] * sy) * meters - d[k] * 4.0);
                let o = to_voxel(o);
                let dir = d;
                // Into the box, then voxel by voxel
                let inv = dir.map(|v| if v.abs() < 1e-9 { 1e9 } else { 1.0 / v });
                let t1 = [0, 1, 2].map(|k| (0.0 - o[k]) * inv[k]);
                let t2 = [0, 1, 2].map(|k| (size[k] - o[k]) * inv[k]);
                let tmin = (0..3).map(|k| t1[k].min(t2[k])).fold(0.0f64, f64::max);
                let tmax = (0..3).map(|k| t1[k].max(t2[k])).fold(f64::MAX, f64::min);
                if tmin > tmax {
                    continue;
                }
                let mut axis = (0..3).max_by(|&a, &b| t1[a].min(t2[a]).total_cmp(&t1[b].min(t2[b]))).unwrap();
                let p = [0, 1, 2].map(|k| o[k] + dir[k] * (tmin + 1e-6));
                let mut c = [0, 1, 2].map(|k| (p[k].floor() as i64).clamp(0, size[k] as i64 - 1));
                let step = dir.map(|v| if v > 0.0 { 1 } else { -1 });
                let mut next = [0, 1, 2].map(|k| ((c[k] as f64 + if dir[k] > 0.0 { 1.0 } else { 0.0 }) - o[k]) * inv[k]);
                let delta = inv.map(f64::abs);
                for _ in 0..1000 {
                    let index = (c[0] + (c[1] + c[2] * size[1] as i64) * size[0] as i64) as usize;
                    let v = m.voxels[index];
                    if v != AIR {
                        let mut n = [0.0; 3];
                        n[axis] = -step[axis] as f64;
                        out[i + j * px] = shade(v, n);
                        break;
                    }
                    axis = if next[0] < next[1] && next[0] < next[2] { 0 } else if next[1] < next[2] { 1 } else { 2 };
                    c[axis] += step[axis];
                    next[axis] += delta[axis];
                    if c[axis] < 0 || c[axis] >= size[axis] as i64 {
                        break;
                    }
                }
            }
        }
        out
    }

    /// A contact sheet of every effect model, to look at without the game: `EFFECT_SHEET=path.png
    /// cargo test --release effect_sheet -- --ignored`
    #[test]
    #[ignore]
    fn effect_sheet() {
        let Ok(path) = std::env::var("EFFECT_SHEET") else {
            return;
        };
        let models = models();
        let px = 150;
        let view = [-0.62, -0.5, -0.38];
        // A row for each look: its stages, its ring, and its orb
        let mut rows: Vec<Vec<Vec<[u8; 3]>>> = Vec::new();
        for look in 0..ELEMENT_LOOKS {
            let mut row: Vec<Vec<[u8; 3]>> = (0..BLAST_STAGES).map(|s| render(&models[look * BLAST_STAGES + s], BLAST_RES, 3.2, px, view)).collect();
            row.push(render(&models[BLAST_RINGS - BLASTS + look], BLAST_RING_RES, 2.6, px, [-0.6, -0.5, -0.6]));
            row.push(render(&orb(element_of(look)), MODEL_RES, 2.6, px, view));
            rows.push(row);
        }
        let mut last: Vec<Vec<[u8; 3]>> = (0..ICE_STAGE_COUNT).map(|s| render(&models[ICE_STAGES - BLASTS + s], ICE_RES, 2.8, px, view)).collect();
        last.extend((0..PILLAR_STAGE_COUNT).map(|s| render(&models[PILLAR_STAGES - BLASTS + s], PILLAR_RES, 7.0, px, [-0.7, -0.5, -0.12])));
        last.push(render(&models[MOON - BLASTS], MOON_RES, 2.6, px, [0.0, -0.05, -1.0]));
        last.extend((0..4).map(|k| render(&models[STREAKS - BLASTS + k * 2], STREAK_RES, 0.3, px, [-0.3, -0.9, -0.3])));
        rows.push(last);
        let columns = rows.iter().map(Vec::len).max().unwrap();
        let mut image = image::RgbImage::new((columns * px) as u32, (rows.len() * px) as u32);
        for (r, row) in rows.iter().enumerate() {
            for (c, tile) in row.iter().enumerate() {
                for (k, pixel) in tile.iter().enumerate() {
                    image.put_pixel((c * px + k % px) as u32, (r * px + k / px) as u32, image::Rgb(*pixel));
                }
            }
        }
        image.save(&path).unwrap();
    }

    /// How many voxels rays step through in `model`, drawn `res` voxels to the meter, over a grid of
    /// parallel rays 2 cm apart across it from a few ways: what it costs the shader to draw next to
    /// other models drawn the same size, as traverse.comp steps voxel by voxel through each it's in
    fn ray_steps(model: &(Model, Vector3<f64>), res: f64) -> f64 {
        let (m, _) = model;
        let size = m.size.map(|s| s as f64);
        let mut steps = 0usize;
        let views = [[-0.62, -0.5, -0.38], [0.3, -0.9, -0.2], [0.05, 0.1, -1.0]];
        for view in views {
            let d = normalized(view);
            let r = normalized([d[1], -d[0], 0.01]);
            let u = [r[1] * d[2] - r[2] * d[1], r[2] * d[0] - r[0] * d[2], r[0] * d[1] - r[1] * d[0]];
            let middle = scale(size, 0.5);
            let span = length(size) * 0.5;
            let n = (2.0 * span / res / 0.02).ceil() as i64;
            for j in -n / 2..=n / 2 {
                for i in -n / 2..=n / 2 {
                    let (sx, sy) = (i as f64 * 0.02 * res, j as f64 * 0.02 * res);
                    let o = [0, 1, 2].map(|k| middle[k] + r[k] * sx + u[k] * sy - d[k] * span * 2.0);
                    let inv = d.map(|v| if v.abs() < 1e-9 { 1e9 } else { 1.0 / v });
                    let t1 = [0, 1, 2].map(|k| -o[k] * inv[k]);
                    let t2 = [0, 1, 2].map(|k| (size[k] - o[k]) * inv[k]);
                    let tmin = (0..3).map(|k| t1[k].min(t2[k])).fold(0.0f64, f64::max);
                    let tmax = (0..3).map(|k| t1[k].max(t2[k])).fold(f64::MAX, f64::min);
                    if tmin > tmax {
                        continue;
                    }
                    let p = [0, 1, 2].map(|k| o[k] + d[k] * (tmin + 1e-6));
                    let mut c = [0, 1, 2].map(|k| (p[k].floor() as i64).clamp(0, size[k] as i64 - 1));
                    let step = d.map(|v| if v > 0.0 { 1 } else { -1 });
                    let mut next = [0, 1, 2].map(|k| ((c[k] as f64 + if d[k] > 0.0 { 1.0 } else { 0.0 }) - o[k]) * inv[k]);
                    let delta = inv.map(f64::abs);
                    loop {
                        steps += 1;
                        if m.voxels[(c[0] + (c[1] + c[2] * size[1] as i64) * size[0] as i64) as usize] != AIR {
                            break;
                        }
                        let axis = if next[0] < next[1] && next[0] < next[2] { 0 } else if next[1] < next[2] { 1 } else { 2 };
                        c[axis] += step[axis];
                        next[axis] += delta[axis];
                        if c[axis] < 0 || c[axis] >= size[axis] as i64 {
                            break;
                        }
                    }
                }
            }
        }
        steps as f64 / views.len() as f64
    }

    /// What drawing each stage of an explosion costs next to the fireball it was drawn with before,
    /// at the same size: `cargo test --release blast_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn blast_cost() {
        // The fireball as it was
        let mut seed = 0x0f1e + 1;
        let mut p = Vec::new();
        for _ in 0..14 {
            let d = [random(&mut seed) * 2.0 - 1.0, random(&mut seed) * 2.0 - 1.0, random(&mut seed) * 2.0 - 1.0];
            let n = length(d).max(0.1);
            p.push(ball(d.map(|v| v / n * 0.95), 0.25 + 0.2 * random(&mut seed), AIR));
        }
        p.push(ball([0.0; 3], 0.45, GLOW_HOLY));
        p.push(ball([0.0; 3], 0.75, GLOW_FIRE));
        p.push(ball([0.0; 3], 1.0, GLOW_FIRE));
        let old = ray_steps(&voxelize(&p, MODEL_RES), MODEL_RES);
        let models = models();
        for look in 0..ELEMENT_LOOKS {
            let costs: Vec<String> = (0..BLAST_STAGES).map(|s| format!("{:.2}", ray_steps(&models[look * BLAST_STAGES + s], BLAST_RES) / old)).collect();
            println!("look {look}: {} times the old fireball's ray steps by stage", costs.join(" "));
        }
        println!("orb (fire): {:.2}", ray_steps(&orb(Some(Element::Fire)), MODEL_RES) / old);
    }

    #[test]
    fn blasts_go_through_their_stages_in_order() {
        for look in 0..ELEMENT_LOOKS {
            let mut last = 0;
            for k in 0..=100 {
                let s = blast_stage(look, k as f64 / 100.0);
                assert!(s >= last && s < BLAST_LIVE);
                last = s;
            }
            assert_eq!(last, BLAST_LIVE - 1);
            assert_eq!(blast_stage(look, 0.0), 0);
        }
    }
}
