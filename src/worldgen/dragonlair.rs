//! Dragons' lairs, see src\dragons.rs. In Free Roam the Worldpyre Ancient keeps a far lair, high on
//! open ground a long way from where the journey starts: a scorched hollow ringed with black
//! spires, a dais in its middle, the ribs and skull of something huge it brought down long ago and
//! the hoard it's gathered, embers glowing in cracks in the floor. On the way to it the land's
//! scorched here and there, and the great bones of what it killed lie about, the more of both the
//! nearer it. The three great dragons' lairs, wherever a realm has them, are each their own: a
//! crown of crags round a nest of bones on the heights, a caldera walled in black rock with cooling
//! magma at its heart, and a hollow under a dead giant's ribs heaped with gold. In the Cinder Crown,
//! whose land has roosts, calderas and bone caves of its own for them to keep their lairs by,
//! they're only touched with what's the dragons' own, see by_landmark(); its finale, the Ancient,
//! keeps a hollow like its far lair's.

use std::f32::consts::PI;
use std::sync::RwLock;

use super::*;

/// Where Free Roam starts, in meters of the landscape: where main.rs puts the player in a new game,
/// see default_start() there
pub const FREE_ROAM_START: [f32; 2] = [206.08, 179.2];
/// How far from there the far lair is, at the nearest and furthest, in meters
const LAIR_FROM: [f32; 2] = [1650.0, 2150.0];
/// How far round the middle of the far lair its hollow reaches, in meters
pub const HOLLOW: f32 = 38.0;
/// How far out from the far lair the omens of it lie, and how far either side of the way there
/// from where the journey starts
const OMENS_OUT: f32 = 950.0;
const OMENS_SIDE: f32 = 300.0;
/// One spot looked at for an omen in each square this many meters across, and how far what's put
/// there reaches from it
pub const OMEN_CELL: f32 = 26.0;
pub const OMEN_REACH: f32 = 12.0;

/// What kind of lair a dragon keeps
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LairKind {
    /// The Pyre Matriarch's crown of crags round a nest of bones
    Peak,
    /// The Cindermaw Tyrant's caldera
    Caldera,
    /// The Gildscale Hoardwarden's hollow under a giant's ribs, heaped with gold
    Hoard,
    /// The Worldpyre Ancient's scorched hollow
    Ancient,
}

impl LairKind {
    /// How far round its middle it reaches, in meters
    pub fn reach(self) -> f32 {
        match self {
            LairKind::Peak => 20.0,
            LairKind::Caldera => 24.0,
            LairKind::Hoard => 20.0,
            LairKind::Ancient => HOLLOW,
        }
    }

    /// How far round it reaches in landscape `settings`: less by a landmark the land has of its
    /// own, see by_landmark()
    fn reach_in(self, settings: &TerrainSettings) -> f32 {
        if by_landmark(self, settings) { TOUCHED } else { self.reach() }
    }
}

/// Whether a lair of `kind` in landscape `settings` is by a landmark the land has of its own, as
/// the Cinder Crown's raid bosses' are, by a roost, a caldera's rim and a bone cave, see
/// worldgen::Landmark: then the land's own is the lair, and it's only touched with what's the
/// dragon's, its floor scorched, the Hoardwarden's gold and the Tyrant's embers
fn by_landmark(kind: LairKind, settings: &TerrainSettings) -> bool {
    kind != LairKind::Ancient && land_of(settings.seed) == Land::Dragon
}

/// How far round its middle a lair by a landmark reaches, in meters
const TOUCHED: f32 = 11.0;

/// Where each landscape's far lair is, worked out once
static FAR: RwLock<Vec<(u32, Option<[f32; 2]>)>> = RwLock::new(Vec::new());
/// The great dragons' lairs a realm's said it has, by its landscape, see set_lairs()
static REALM: RwLock<Vec<(u32, Vec<(LairKind, [f32; 2])>)>> = RwLock::new(Vec::new());

/// Where the Worldpyre Ancient's far lair is in landscape `settings`, in meters: in the normal
/// world's land only, on the highest open ground of those looked at in the way the seed says, out
/// past LAIR_FROM's nearest from where the journey starts
pub fn far_lair(settings: &TerrainSettings) -> Option<[f32; 2]> {
    if land_of(settings.seed) != Land::Normal {
        return None;
    }
    if let Some(&(_, found)) = FAR.read().ok()?.iter().find(|(seed, _)| *seed == settings.seed) {
        return found;
    }
    let r = hash(settings.seed ^ 0xd4a6_0f1e);
    let toward = unit(r) * TAU;
    let [sx, sy] = FREE_ROAM_START;
    let mut best: Option<([f32; 2], f32)> = None;
    for ring in 0..4 {
        let d = LAIR_FROM[0] + (LAIR_FROM[1] - LAIR_FROM[0]) * ring as f32 / 3.0;
        for k in 0..17 {
            let a = toward + (k as f32 - 8.0) * 0.07;
            let at = [sx + a.cos() * d, sy + a.sin() * d];
            if !is_dry_land(settings, at, 22.0) {
                continue;
            }
            // High, but above all level enough to fight on: how far the ground round it rises or
            // falls from it counts against it
            let h = height_at(settings, at[0], at[1]);
            let rough = (0..16)
                .map(|j| {
                    let (b, r) = (j as f32 * TAU / 8.0, if j < 8 { 22.0 } else { 34.0 });
                    (height_at(settings, at[0] + b.cos() * r, at[1] + b.sin() * r) - h).abs()
                })
                .fold(0.0f32, f32::max);
            let score = h - 3.0 * rough;
            if best.is_none_or(|(_, b)| score > b) {
                best = Some((at, score));
            }
        }
    }
    let found = best.map(|(at, _)| at).or_else(|| dry_land_near(settings, [sx + toward.cos() * LAIR_FROM[0], sy + toward.sin() * LAIR_FROM[0]], 600.0, 14.0, &|_| true));
    if let Ok(mut far) = FAR.write() {
        far.push((settings.seed, found));
    }
    found
}

/// The great dragons' lairs landscape `seed` has, for a realm to say, see session.rs: where each is
/// in meters, of which kind. They're built into the land as it's made.
#[allow(dead_code)]
pub fn set_lairs(seed: u32, lairs: Vec<(LairKind, [f32; 2])>) {
    if let Ok(mut realm) = REALM.write() {
        realm.retain(|(s, _)| *s != seed);
        realm.push((seed, lairs));
    }
    SAID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// How many times lairs have been said to be anywhere, so what's kept of them on each thread is
/// known to be out of date
static SAID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// What's kept on each thread of where the lairs are, see scorch()
struct Near {
    seed: u32,
    said: u32,
    lairs: Vec<(LairKind, [f32; 2])>,
    far: Option<[f32; 2]>,
    /// The last half-meter square looked at, and how it's scorched
    cell: (i32, i32),
    burnt: Option<u8>,
}

thread_local! {
    /// The lairs of the landscape whose ground's being made on this thread, so each column needn't
    /// ask again
    static NEAR: std::cell::RefCell<Near> = const { std::cell::RefCell::new(Near { seed: u32::MAX, said: u32::MAX, lairs: Vec::new(), far: None, cell: (i32::MIN, i32::MIN), burnt: None }) };
}

/// How a column of ground at `x`, `y` in meters is scorched by a dragon, if it is: burnt black
/// across each lair's floor, ragged at its edge, and in patches along the way to the far lair, more
/// of them the nearer. What it's burnt to: obsidian-black, or charred. Worked out a half meter
/// square at a time, as the columns of each come one after another.
pub(super) fn scorch(settings: &TerrainSettings, x: f32, y: f32) -> Option<u8> {
    let said = SAID.load(std::sync::atomic::Ordering::Relaxed);
    NEAR.with(|near| {
        let mut near = near.borrow_mut();
        if near.seed != settings.seed || near.said != said {
            *near = Near { seed: settings.seed, said, lairs: lairs(settings), far: far_lair(settings), cell: (i32::MIN, i32::MIN), burnt: None };
        }
        // Nowhere near any of them, as for nearly all the ground there is
        let near_any = near.lairs.iter().any(|&(k, at)| (x - at[0]).abs() < k.reach_in(settings) + 2.0 && (y - at[1]).abs() < k.reach_in(settings) + 2.0);
        let near_far = near.far.is_some_and(|l| (x - l[0]).abs() < OMENS_OUT && (y - l[1]).abs() < OMENS_OUT);
        if !near_any && !near_far {
            return None;
        }
        let cell = ((x * 2.0).floor() as i32, (y * 2.0).floor() as i32);
        if cell == near.cell {
            return near.burnt;
        }
        let (cx, cy) = (cell.0 as f32 * 0.5 + 0.25, cell.1 as f32 * 0.5 + 0.25);
        let char = if hash3(cell.0 as u32, cell.1 as u32, settings.seed ^ 0x5c0c) % 3 == 0 { BARK_DARK } else { OBSIDIAN };
        let mut burnt = None;
        for &(kind, at) in &near.lairs {
            let d = (cx - at[0]).hypot(cy - at[1]);
            let reach = kind.reach_in(settings);
            if d < reach + 2.0 && d < reach - 2.0 + 3.0 * fbm(cx / 6.0, cy / 6.0, 2, settings.seed ^ 0x5c0d) {
                burnt = Some(char);
            }
        }
        // Patches of it on the way to the far lair, out to where the omens are: blotches where
        // the noise is high, more of the ground the nearer
        if burnt.is_none()
            && let Some(lair) = near.far
        {
            let to = (cx - lair[0]).hypot(cy - lair[1]);
            if to < OMENS_OUT && (to < 320.0 || off_the_way([cx, cy], FREE_ROAM_START, lair) < OMENS_SIDE) {
                let closeness = 1.0 - to / OMENS_OUT;
                let blotch = fbm(cx / 11.0, cy / 11.0, 3, settings.seed ^ 0x5c0e);
                if blotch > 0.55 - 0.45 * closeness * closeness {
                    burnt = Some(char);
                }
            }
        }
        near.cell = cell;
        near.burnt = burnt;
        burnt
    })
}

/// The great dragons' lairs said to be in landscape `seed`, see set_lairs()
pub fn lairs_set(seed: u32) -> Vec<(LairKind, [f32; 2])> {
    REALM.read().map_or(Vec::new(), |r| r.iter().filter(|(s, _)| *s == seed).flat_map(|(_, l)| l.clone()).collect())
}

/// Every dragon's lair in landscape `settings`
fn lairs(settings: &TerrainSettings) -> Vec<(LairKind, [f32; 2])> {
    let mut out: Vec<(LairKind, [f32; 2])> = REALM.read().map_or(Vec::new(), |r| r.iter().filter(|(s, _)| *s == settings.seed).flat_map(|(_, l)| l.clone()).collect());
    out.extend(far_lair(settings).map(|at| (LairKind::Ancient, at)));
    out
}

/// How far `p` is from the way from `a` to `b`, in meters
fn off_the_way(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let t = (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-6)).clamp(0.0, 1.0);
    (p[0] - a[0] - ab[0] * t).hypot(p[1] - a[1] - ab[1] * t)
}

impl Terrain {
    /// Where nothing else is to be built or grow, for the dragons' lairs reaching into this piece:
    /// x0, y0, x1, y1 and how high they reach, in meters
    pub(super) fn dragon_footprints(&self) -> Vec<[f32; 5]> {
        lairs(&self.settings)
            .into_iter()
            // Not by a landmark, which mustn't be kept out of where it is, see by_landmark()
            .filter(|&(kind, _)| !by_landmark(kind, &self.settings))
            .map(|(kind, at)| {
                let r = kind.reach_in(&self.settings);
                [at[0] - r, at[1] - r, at[0] + r, at[1] + r, height_at(&self.settings, at[0], at[1]) + 20.0]
            })
            .filter(|f| f[2] > self.lo[0] && f[3] > self.lo[1] && f[0] < self.hi[0] && f[1] < self.hi[1])
            .collect()
    }

    /// An omen of the far lair at `x`, `y` on the ground at `ground`, if there's one there: the
    /// ground scorched, a great rib cage half sunk in it, or a skull and the line of a spine, more
    /// and more of them the nearer the lair
    pub(super) fn dragon_omen(&self, x: f32, y: f32, ground: f32, column: &Column, r: u32) -> Option<Object> {
        let lair = far_lair(&self.settings)?;
        let to = (x - lair[0]).hypot(y - lair[1]);
        if !(HOLLOW + 6.0..OMENS_OUT).contains(&to) || (off_the_way([x, y], FREE_ROAM_START, lair) > OMENS_SIDE && to > 320.0) {
            return None;
        }
        if ground < sea_level_m(&self.settings) + 0.8 || column.top == WATER {
            return None;
        }
        let near = 1.0 - to / OMENS_OUT;
        if unit(r) > 0.05 + 0.55 * near * near {
            return None;
        }
        let mut rng = Rng(hash(r ^ 0x0e11));
        let pick = rng.next();
        // The ground itself is scorched as it's made, see scorch(): what's on it here is bones,
        // and now and then the charred stump of a tree that burnt
        let shapes = if pick < 0.5 {
            ribs(x, y, ground, 0.7 + 0.5 * rng.next(), &mut rng)
        } else if pick < 0.8 {
            skull(x, y, ground, 0.8 + 0.4 * rng.next(), &mut rng)
        } else {
            stumps(x, y, ground, &mut rng)
        };
        Some(Object::new(shapes, rng.0))
    }

    /// The dragons' lairs reaching into this piece, and the sites of those whose middles are its
    /// own, for their dragons to turn up in, see src\main.rs
    pub(super) fn dragon_lairs(&self, features: &mut Features) -> Vec<Object> {
        let mut objects = Vec::new();
        for (kind, at) in lairs(&self.settings) {
            let reach = kind.reach_in(&self.settings) + 6.0;
            if at[0] + reach < self.lo[0] || at[1] + reach < self.lo[1] || at[0] - reach > self.hi[0] || at[1] - reach > self.hi[1] {
                continue;
            }
            let ground = height_at(&self.settings, at[0], at[1]);
            let mut rng = Rng(hash3(at[0] as i32 as u32, at[1] as i32 as u32, self.settings.seed ^ 0x1a14));
            let ground_at = |x: f32, y: f32| height_at(&self.settings, x, y);
            let (shapes, top) = match kind {
                _ if by_landmark(kind, &self.settings) => touched(kind, at, ground, &ground_at, &mut rng),
                LairKind::Ancient => ancient_hollow(at, ground, &ground_at, &mut rng),
                LairKind::Peak => peak_roost(at, ground, &ground_at, &mut rng),
                LairKind::Caldera => caldera(at, ground, &ground_at, &mut rng),
                LairKind::Hoard => hoard_hollow(at, ground, &ground_at, &mut rng),
            };
            objects.push(Object::new(shapes, rng.0));
            if self.inside(at[0], at[1]) {
                let back = [FREE_ROAM_START[0] - at[0], FREE_ROAM_START[1] - at[1]];
                let length = back[0].hypot(back[1]).max(1e-3);
                let kind = match kind {
                    LairKind::Ancient => "dragon lair",
                    LairKind::Peak => "dragon peak",
                    LairKind::Caldera => "dragon caldera",
                    LairKind::Hoard => "dragon hoard",
                };
                features.sites.push(Site { kind, position: [at[0], at[1], top], facing: [back[0] / length, back[1] / length] });
            }
        }
        objects
    }
}

/// A few charred stumps of trees that burnt, round `x`, `y`
fn stumps(x: f32, y: f32, ground: f32, rng: &mut Rng) -> Vec<Shape> {
    (0..1 + (rng.next() * 2.99) as usize)
        .map(|_| {
            let (a, r) = (rng.range(0.0, TAU), rng.range(0.0, 3.0));
            let foot = [x + a.cos() * r, y + a.sin() * r, ground - 0.3];
            let tall = rng.range(1.0, 2.8);
            Shape::capsule(foot, [foot[0] + rng.range(-0.3, 0.3), foot[1] + rng.range(-0.3, 0.3), foot[2] + tall], rng.range(0.22, 0.34), 0.1, BARK_DARK, OBSIDIAN, false)
        })
        .collect()
}

/// A great rib cage half sunk in the ground at `x`, `y`, `size` times the usual: a spine along the
/// ground, and ribs arching up from it either side
fn ribs(x: f32, y: f32, ground: f32, size: f32, rng: &mut Rng) -> Vec<Shape> {
    let mut shapes = Vec::new();
    let a = rng.range(0.0, TAU);
    let (along, across) = ([a.cos(), a.sin()], [-a.sin(), a.cos()]);
    let length = 8.0 * size;
    let base = ground - 0.5 * size;
    let at = |u: f32, v: f32, z: f32| [x + along[0] * u + across[0] * v, y + along[1] * u + across[1] * v, z];
    shapes.push(Shape::capsule(at(-length * 0.5, 0.0, base + 0.2), at(length * 0.5, 0.0, base + 0.35), 0.4 * size, 0.3 * size, BONE, LIMESTONE, false));
    for k in 0..6 {
        let u = -length * 0.4 + k as f32 * length * 0.15;
        let tall = (3.4 - (k as f32 - 1.5).abs() * 0.35) * size;
        // Some broken off
        let broken = rng.next() < 0.25;
        for side in [-1.0, 1.0] {
            let wide = 2.0 * size;
            let lean = rng.range(-0.15, 0.15) * size;
            let mid = at(u + lean, side * wide, base + tall * 0.75);
            shapes.push(Shape::capsule(at(u, side * 0.3 * size, base + 0.35), mid, 0.24 * size, 0.18 * size, BONE, LIMESTONE, false));
            if !(broken && side > 0.0) {
                shapes.push(Shape::capsule(mid, at(u + lean * 2.0, side * 0.9 * size, base + tall), 0.18 * size, 0.1 * size, BONE, LIMESTONE, false));
            }
        }
    }
    shapes
}

/// A dragon's skull, `size` times the usual, lying in the ground at `x`, `y`, horns and all, with a
/// few vertebrae of its neck trailing from it
fn skull(x: f32, y: f32, ground: f32, size: f32, rng: &mut Rng) -> Vec<Shape> {
    let mut shapes = Vec::new();
    let a = rng.range(0.0, TAU);
    let (along, across) = ([a.cos(), a.sin()], [-a.sin(), a.cos()]);
    let base = ground - 0.3 * size;
    let at = |u: f32, v: f32, z: f32| [x + along[0] * u + across[0] * v, y + along[1] * u + across[1] * v, base + z];
    // Its eye sockets and nostrils hollow, cut out first
    for side in [-1.0, 1.0] {
        shapes.push(Shape::sphere(at(0.6 * size, side * 0.55 * size, 1.2 * size), 0.32 * size, AIR, AIR, false));
        shapes.push(Shape::sphere(at(2.3 * size, side * 0.22 * size, 0.85 * size), 0.13 * size, AIR, AIR, false));
    }
    for side in [-1.0, 1.0] {
        shapes.push(Shape::capsule(at(-0.2 * size, side * 0.5 * size, 1.4 * size), at(-1.5 * size, side * 1.0 * size, 2.4 * size), 0.28 * size, 0.06 * size, BONE, LIMESTONE, false));
        // Teeth along its jaw
        for k in 0..4 {
            let u = (1.0 + k as f32 * 0.38) * size;
            shapes.push(Shape::capsule(at(u, side * 0.38 * size, 0.55 * size), at(u, side * 0.36 * size, 0.2 * size), 0.07 * size, 0.02 * size, BONE, AIR, false));
        }
    }
    shapes.push(Shape::ellipsoid(at(0.4 * size, 0.0, 1.0 * size), [1.1 * size, 0.85 * size, 0.75 * size], BONE, LIMESTONE));
    shapes.push(Shape::capsule(at(0.8 * size, 0.0, 0.85 * size), at(2.6 * size, 0.0, 0.7 * size), 0.55 * size, 0.32 * size, BONE, LIMESTONE, false));
    shapes.push(Shape::capsule(at(0.4 * size, 0.0, 0.35 * size), at(2.4 * size, 0.0, 0.25 * size), 0.3 * size, 0.2 * size, BONE, LIMESTONE, false));
    for k in 0..4 {
        let u = -(1.2 + k as f32 * 0.85) * size;
        shapes.push(Shape::sphere(at(u, rng.range(-0.2, 0.2) * size, 0.3 * size), 0.38 * size, BONE, LIMESTONE, false));
    }
    shapes
}

/// A ring of `count` spires of `stone` round `at`, from `inner` to `outer` meters out, `low` to
/// `high` meters tall, leaning out, on the ground there
#[allow(clippy::too_many_arguments)]
fn spires(shapes: &mut Vec<Shape>, at: [f32; 2], count: usize, inner: f32, outer: f32, low: f32, high: f32, stone: (u8, u8), ground: &dyn Fn(f32, f32) -> f32, rng: &mut Rng, gaps: &[f32]) {
    for k in 0..count {
        let a = k as f32 / count as f32 * TAU + rng.range(-0.12, 0.12);
        // Ways in, left clear
        if gaps.iter().any(|&g| ((a - g + PI).rem_euclid(TAU) - PI).abs() < 0.32) {
            continue;
        }
        let r = rng.range(inner, outer);
        let (x, y) = (at[0] + a.cos() * r, at[1] + a.sin() * r);
        let foot = [x, y, ground(x, y) - 1.0];
        let tall = rng.range(low, high);
        let lean = rng.range(0.1, 0.3);
        let tip = [x + a.cos() * tall * lean, y + a.sin() * tall * lean, foot[2] + tall];
        let width = rng.range(1.0, 1.8);
        // A crag broken partway, its top kinked off to one side
        let kink = [foot[0] + (tip[0] - foot[0]) * 0.6 + rng.range(-0.8, 0.8), foot[1] + (tip[1] - foot[1]) * 0.6 + rng.range(-0.8, 0.8), foot[2] + tall * 0.6];
        shapes.push(Shape::capsule(kink, tip, width * 0.45, 0.12, stone.0, stone.1, false));
        shapes.push(Shape::capsule(foot, kink, width, width * 0.45, stone.0, stone.1, false));
        // And a smaller one at its foot
        if rng.next() < 0.6 {
            let b = a + rng.range(-0.12, 0.12);
            let (x2, y2) = (x + b.cos() * rng.range(1.5, 3.0), y + b.sin() * rng.range(1.5, 3.0));
            let low = tall * rng.range(0.3, 0.5);
            shapes.push(Shape::capsule([x2, y2, ground(x2, y2) - 0.8], [x2 + b.cos() * low * 0.3, y2 + b.sin() * low * 0.3, ground(x2, y2) + low], width * 0.6, 0.1, stone.0, stone.1, false));
        }
    }
}

/// Embers glowing in cracks in a lair's floor, out from `at` this many ways
fn embers(shapes: &mut Vec<Shape>, at: [f32; 2], ways: usize, from: f32, to: f32, ground: &dyn Fn(f32, f32) -> f32, rng: &mut Rng) {
    for k in 0..ways {
        let a = k as f32 / ways as f32 * TAU + rng.range(-0.3, 0.3);
        let mut last = [at[0] + a.cos() * from, at[1] + a.sin() * from];
        let steps = 4;
        for j in 1..=steps {
            let r = from + (to - from) * j as f32 / steps as f32;
            let b = a + rng.range(-0.25, 0.25);
            let next = [at[0] + b.cos() * r, at[1] + b.sin() * r];
            shapes.push(Shape::capsule([last[0], last[1], ground(last[0], last[1]) - 0.08], [next[0], next[1], ground(next[0], next[1]) - 0.08], 0.16, 0.12, GLOW_FIRE, FLAME, false).placed(Placement::Clear));
            last = next;
        }
    }
}

/// Gold heaped up round `at`, `count` mounds of it out to `reach` meters, gems and bronze among it
fn hoard(shapes: &mut Vec<Shape>, at: [f32; 2], count: usize, reach: f32, ground: &dyn Fn(f32, f32) -> f32, rng: &mut Rng) {
    for _ in 0..count {
        let (a, r) = (rng.range(0.0, TAU), rng.range(reach * 0.3, reach));
        let (x, y) = (at[0] + a.cos() * r, at[1] + a.sin() * r);
        let s = rng.range(1.2, 2.6);
        let z = ground(x, y);
        if rng.next() < 0.3 {
            shapes.push(Shape::sphere([x + rng.range(-0.8, 0.8), y + rng.range(-0.8, 0.8), z + s * 0.35], 0.3, CRYSTAL, CRYSTAL, false));
        }
        shapes.push(Shape::ellipsoid([x, y, z - s * 0.2], [s, s * 0.85, s * 0.55], GOLD, BRONZE));
    }
}

/// A great dragon's lair by a landmark of the land's own, see by_landmark(): the bones of what it's
/// eaten strewn round where it waits on the heights, embers in cracks in the scorched ground by a
/// caldera, and gold heaped up at the mouth of a bone cave, skulls among it
fn touched(kind: LairKind, at: [f32; 2], ground: f32, height: &dyn Fn(f32, f32) -> f32, rng: &mut Rng) -> (Vec<Shape>, f32) {
    let mut shapes = Vec::new();
    match kind {
        LairKind::Peak => {
            for k in 0..9 {
                let a = k as f32 / 9.0 * TAU + rng.range(-0.3, 0.3);
                let r = rng.range(3.0, 7.0);
                let (x, y) = (at[0] + a.cos() * r, at[1] + a.sin() * r);
                let b = rng.range(0.0, TAU);
                let long = rng.range(0.8, 1.6);
                let z = height(x, y) + 0.15;
                shapes.push(Shape::capsule([x - b.cos() * long, y - b.sin() * long, z], [x + b.cos() * long, y + b.sin() * long, z + rng.range(0.0, 0.3)], 0.2, 0.14, BONE, LIMESTONE, false));
            }
        }
        LairKind::Caldera => embers(&mut shapes, at, 6, 2.5, TOUCHED - 1.0, height, rng),
        _ => {
            hoard(&mut shapes, at, 12, TOUCHED - 2.0, height, rng);
            for k in 0..2 {
                let a = rng.range(0.0, TAU);
                let (x, y) = (at[0] + a.cos() * 5.0, at[1] + a.sin() * 5.0);
                shapes.extend(skull(x, y, height(x, y), 0.6 + 0.25 * k as f32, rng));
            }
        }
    }
    (shapes, ground)
}

/// The Worldpyre Ancient's hollow: see the top of this file. Returns its shapes and how high its
/// dais stands, where it waits.
fn ancient_hollow(at: [f32; 2], ground: f32, height: &dyn Fn(f32, f32) -> f32, rng: &mut Rng) -> (Vec<Shape>, f32) {
    let mut shapes = Vec::new();
    // A broad dais of black rock in the middle, where it rests
    let dais = ground + 0.8;
    embers(&mut shapes, at, 7, 7.0, HOLLOW - 6.0, height, rng);
    // The skull of what it brought down long ago, by the dais, and its ribs off to one side
    let side = rng.range(0.0, TAU);
    let (sx, sy) = (at[0] + side.cos() * 13.0, at[1] + side.sin() * 13.0);
    shapes.extend(skull(sx, sy, height(sx, sy), 2.6, rng));
    let (rx, ry) = (at[0] - side.cos() * 20.0, at[1] - side.sin() * 20.0);
    shapes.extend(ribs(rx, ry, height(rx, ry), 2.2, rng));
    hoard(&mut shapes, at, 9, 10.0, height, rng);
    shapes.push(Shape::ellipsoid([at[0], at[1], ground - 0.4], [8.5, 8.5, 1.2], OBSIDIAN, STONE).placed(Placement::Clear));
    spires(&mut shapes, at, 18, HOLLOW - 4.0, HOLLOW + 1.0, 7.0, 17.0, (OBSIDIAN, STONE), height, rng, &[]);
    (shapes, dais)
}

/// The Pyre Matriarch's crown of crags on the heights, round a nest of bones and charred branches
/// with her eggs glowing in it
fn peak_roost(at: [f32; 2], ground: f32, height: &dyn Fn(f32, f32) -> f32, rng: &mut Rng) -> (Vec<Shape>, f32) {
    let mut shapes = Vec::new();
    for k in 0..3 {
        let a = k as f32 * TAU / 3.0 + 0.4;
        shapes.push(Shape::ellipsoid([at[0] + a.cos() * 1.4, at[1] + a.sin() * 1.4, ground + 0.9], [0.7, 0.6, 0.95], OBSIDIAN, GLOW_FIRE));
    }
    // The nest: a ring of charred branches and bones
    for k in 0..16 {
        let a = k as f32 / 16.0 * TAU;
        let b = a + rng.range(0.4, 0.9);
        let (r0, r1) = (rng.range(3.5, 5.0), rng.range(3.5, 5.0));
        let from = [at[0] + a.cos() * r0, at[1] + a.sin() * r0, ground + rng.range(0.1, 0.8)];
        let to = [at[0] + b.cos() * r1, at[1] + b.sin() * r1, ground + rng.range(0.1, 0.8)];
        let (wood, accent) = if k % 3 == 0 { (BONE, LIMESTONE) } else { (BARK_DARK, DEAD_WOOD) };
        shapes.push(Shape::capsule(from, to, 0.3, 0.22, wood, accent, false));
    }
    spires(&mut shapes, at, 11, 14.0, 19.0, 6.0, 14.0, (STONE, OBSIDIAN), height, rng, &[]);
    (shapes, ground)
}

/// The Cindermaw Tyrant's caldera: a rim of black rock walled round, two ways in, its floor
/// scorched and cooling magma glowing through the crust at its heart
fn caldera(at: [f32; 2], ground: f32, height: &dyn Fn(f32, f32) -> f32, rng: &mut Rng) -> (Vec<Shape>, f32) {
    let mut shapes = Vec::new();
    let gaps = [rng.range(0.0, TAU)];
    let gaps = [gaps[0], gaps[0] + PI];
    // The rim, a ring of jagged crags of black rock leaning out, two or three to each, the lowest
    // in front
    for k in 0..22 {
        let a = k as f32 / 22.0 * TAU + rng.range(-0.08, 0.08);
        if gaps.iter().any(|&g| ((a - g + PI).rem_euclid(TAU) - PI).abs() < 0.3) {
            continue;
        }
        for j in 0..2 + (rng.next() * 1.99) as usize {
            let r = 22.0 + rng.range(-1.5, 2.0) + j as f32 * 1.2;
            let b = a + rng.range(-0.06, 0.06);
            let (x, y) = (at[0] + b.cos() * r, at[1] + b.sin() * r);
            let foot = [x, y, height(x, y) - 1.0];
            let tall = rng.range(3.0, 7.5) * (0.6 + 0.25 * j as f32);
            let lean = rng.range(0.15, 0.4);
            let tip = [x + b.cos() * tall * lean + rng.range(-0.6, 0.6), y + b.sin() * tall * lean + rng.range(-0.6, 0.6), foot[2] + tall];
            shapes.push(Shape::capsule(foot, tip, rng.range(1.6, 2.6), rng.range(0.3, 0.7), OBSIDIAN, BARK_DARK, false));
        }
    }
    embers(&mut shapes, at, 6, 5.0, 18.0, height, rng);
    shapes.push(Shape::ellipsoid([at[0], at[1], ground - 0.35], [5.5, 5.0, 0.5], OBSIDIAN, GLOW_FIRE).placed(Placement::Clear));
    shapes.push(Shape::ellipsoid([at[0], at[1], ground - 0.5], [4.2, 3.8, 0.5], GLOW_FIRE, FLAME).placed(Placement::Clear));
    (shapes, ground)
}

/// The Gildscale Hoardwarden's hollow: a dead giant's ribs arching over a floor heaped with gold,
/// skulls among it
fn hoard_hollow(at: [f32; 2], ground: f32, height: &dyn Fn(f32, f32) -> f32, rng: &mut Rng) -> (Vec<Shape>, f32) {
    let mut shapes = Vec::new();
    for k in 0..3 {
        let a = rng.range(0.0, TAU);
        let r = rng.range(6.0, 13.0);
        let (x, y) = (at[0] + a.cos() * r, at[1] + a.sin() * r);
        shapes.extend(skull(x, y, height(x, y), 0.6 + 0.3 * k as f32, rng));
    }
    hoard(&mut shapes, at, 16, 14.0, height, rng);
    // A giant's rib cage over it all
    let (x, y) = (at[0], at[1]);
    shapes.extend(ribs(x, y, ground - 0.5, 2.8, rng));
    (shapes, ground)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A great dragon's lair is built into the land where a realm says it is, with its site for its
    /// dragon to wait at, its floor scorched black and black rock round it
    #[test]
    fn a_great_dragons_lair_is_built() {
        let settings = TerrainSettings { width_m: 96, height_m: 64, voxels_per_meter: 4, seed: 0x00d7_a90c, grass_tufts: false };
        let at = dry_land_near(&settings, [300.0, 300.0], 600.0, 14.0, &|_| true).expect("open ground");
        set_lairs(settings.seed, vec![(LairKind::Caldera, at)]);
        let corner = [((at[0] - 48.0) * 4.0) as i64, ((at[1] - 48.0) * 4.0) as i64];
        let (terrain, world) = generate_area(&settings, corner, [96 * 4, 96 * 4], &RiverCache::default());
        let site = terrain.sites().iter().find(|s| s.kind == "dragon caldera").expect("the caldera's site");
        assert!((site.position[0] - at[0]).abs() < 1.0 && (site.position[1] - at[1]).abs() < 1.0);
        let mut found = [0usize; 256];
        for y in 0..96 * 4 {
            for x in 0..96 * 4 {
                if let Some(z) = (0..world.size[2] as i64).rev().find(|&z| !is_passable(world.material_at(x, y, z))) {
                    found[world.material_at(x, y, z) as usize] += 1;
                }
            }
        }
        assert!(found[OBSIDIAN as usize] > 1000, "obsidian {}", found[OBSIDIAN as usize]);
    }

    /// The far lair's a long way from where Free Roam starts, on open ground, the same every time,
    /// and only in the normal world's land
    #[test]
    fn the_far_lair_is_far_and_on_open_ground() {
        for seed in [1, 2, 77, 4021] {
            let settings = TerrainSettings { width_m: 64, height_m: 64, voxels_per_meter: 4, seed, grass_tufts: false };
            let at = far_lair(&settings).expect("a lair");
            let d = (at[0] - FREE_ROAM_START[0]).hypot(at[1] - FREE_ROAM_START[1]);
            assert!(d > 1500.0, "seed {seed}: only {d:.0} m from the start");
            assert!(is_dry_land(&settings, at, 22.0) || is_dry_land(&settings, at, 14.0), "seed {seed}: not on open ground");
            assert_eq!(far_lair(&settings), Some(at));
        }
        let sky = TerrainSettings { width_m: 64, height_m: 64, voxels_per_meter: 4, seed: 0xF100_0001, grass_tufts: false };
        assert_eq!(far_lair(&sky), None);
    }

    /// The piece of the landscape round the far lair has its site for the Ancient to wait at, and
    /// is built with the lair in it: black rock and gold where the hollow is
    #[test]
    fn the_far_lair_is_built() {
        let settings = TerrainSettings { width_m: 96, height_m: 64, voxels_per_meter: 4, seed: 1, grass_tufts: false };
        let at = far_lair(&settings).unwrap();
        let corner = [((at[0] - 48.0) * 4.0) as i64, ((at[1] - 48.0) * 4.0) as i64];
        let (terrain, world) = generate_area(&settings, corner, [96 * 4, 96 * 4], &RiverCache::default());
        let site = terrain.sites().iter().find(|s| s.kind == "dragon lair").expect("the lair's site");
        assert!((site.position[0] - at[0]).abs() < 1.0 && (site.position[1] - at[1]).abs() < 1.0);
        let mut found = [0usize; 256];
        for y in 0..96 * 4 {
            for x in 0..96 * 4 {
                if let Some(z) = (0..world.size[2] as i64).rev().find(|&z| !is_passable(world.material_at(x, y, z))) {
                    found[world.material_at(x, y, z) as usize] += 1;
                }
            }
        }
        assert!(found[OBSIDIAN as usize] > 1000 && found[GOLD as usize] > 20, "obsidian {} gold {}", found[OBSIDIAN as usize], found[GOLD as usize]);
    }
}
