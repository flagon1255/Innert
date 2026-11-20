//! Wing shrines: windswept aeries on the high peaks, and on the highest of the sky isles, each a
//! round stone platform with a perch and a glowing orb, standing stones, banners and streamers in
//! the wind, and a nest with a pair of wings lying in it, see Game::set_aeries(). Rare: about one in
//! a few square kilometers of the normal world, and one or two near where each realm is come to.
//! Only where there's a way up on foot, a hard climb but one with no step too high, or in the sky
//! isles on an island the grappling hook and a glide reach.

use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

use super::*;

/// What a wing shrine's site is called, see Site
pub const SHRINE: &str = "wing shrine";
/// What a shrine's key's third number is, see Game::set_aeries(), as nothing else's is
pub const SHRINE_KEY: i64 = -7300;
/// How far round a shrine's platform reaches from its middle, in meters
const PLATFORM: f32 = 2.4;
/// How far apart the squares are that have a shrine at most each, in meters, and the chance one
/// does: far apart in the normal world, closer in the realms, which are played in near where
/// they're come to
const NORMAL_CELL: (f32, f32) = (1300.0, 0.7);
const REALM_CELL: (f32, f32) = (520.0, 0.55);
/// Where a shrine's looked for round where a game sets out, for there always to be one within a
/// walk of it, in meters from FREE_ROAM_START
const HOME_RING: [f32; 2] = [150.0, 420.0];
/// The highest step on the way up, in meters for each meter along: about what a jump climbs
const STEEPEST: f32 = 1.0;
/// How far round the top of a peak a shrine's looked for, in 3 m steps, how far below the top at
/// most, in meters, and how far down the steps up onto its platform go
const SHOULDER: i32 = 12;
const BELOW_TOP: f32 = 10.0;
const STEPS: f32 = 0.9;

/// The shrines found in each square, and round each landscape's start, by its seed
static FOUND: LazyLock<RwLock<HashMap<(u32, i32, i32), Option<[f32; 3]>>>> = LazyLock::new(Default::default);

/// The wing shrines from `lo` to `hi`, in meters of the landscape: where the middle of each one's
/// platform is and how high its top, the same every time
pub fn aeries(settings: &TerrainSettings, lo: [f32; 2], hi: [f32; 2]) -> Vec<[f32; 3]> {
    let (cell, _) = cell_of(settings);
    let (x0, y0) = ((lo[0] / cell).floor() as i32, (lo[1] / cell).floor() as i32);
    let (x1, y1) = ((hi[0] / cell).floor() as i32, (hi[1] / cell).floor() as i32);
    let mut found: Vec<[f32; 3]> = (y0..=y1).flat_map(|cy| (x0..=x1).map(move |cx| (cx, cy))).filter_map(|(cx, cy)| cached(settings, cx, cy)).collect();
    // And the one near where a game sets out, unless one of the squares' is near it already
    if let Some(home) = cached(settings, i32::MIN, i32::MIN)
        && !found.iter().any(|f| (f[0] - home[0]).hypot(f[1] - home[1]) < 60.0)
    {
        found.push(home);
    }
    found.retain(|f| f[0] >= lo[0] && f[1] >= lo[1] && f[0] < hi[0] && f[1] < hi[1]);
    found
}

/// The square's shrine, or with i32::MIN the home one, worked out once
fn cached(settings: &TerrainSettings, cx: i32, cy: i32) -> Option<[f32; 3]> {
    let key = (settings.seed, cx, cy);
    if let Some(found) = FOUND.read().ok().and_then(|f| f.get(&key).copied()) {
        return found;
    }
    let found = if cx == i32::MIN { home(settings) } else { in_cell(settings, cx, cy) };
    let found = found.map(|at| [at[0], at[1], top_of(settings, at)]);
    if let Ok(mut f) = FOUND.write() {
        f.insert(key, found);
    }
    found
}

fn cell_of(settings: &TerrainSettings) -> (f32, f32) {
    if land_of(settings.seed) == Land::Normal { NORMAL_CELL } else { REALM_CELL }
}

/// How high the ground is at a spot, in meters, or None in the open air
fn ground(settings: &TerrainSettings, x: f32, y: f32) -> Option<f32> {
    land_height_m(settings, x, y)
}

/// The square's shrine, if it has one: on its highest peak, if that's a good one
fn in_cell(settings: &TerrainSettings, cx: i32, cy: i32) -> Option<[f32; 2]> {
    let (cell, chance) = cell_of(settings);
    let r = hash3(cx as u32, cy as u32, settings.seed ^ 0xae21);
    if unit(r) > chance {
        return None;
    }
    let n = 9;
    let mut best: Option<([f32; 2], f32)> = None;
    for j in 0..n {
        for i in 0..n {
            let at = [(cx as f32 + 0.1 + 0.8 * i as f32 / (n - 1) as f32) * cell, (cy as f32 + 0.1 + 0.8 * j as f32 / (n - 1) as f32) * cell];
            if let Some(h) = ground(settings, at[0], at[1])
                && best.is_none_or(|(_, b)| h > b)
            {
                best = Some((at, h));
            }
        }
    }
    shoulder(settings, climb(settings, best?.0))
}

/// The best peak within a walk of where a game sets out, round FREE_ROAM_START
fn home(settings: &TerrainSettings) -> Option<[f32; 2]> {
    let mut candidates: Vec<([f32; 2], f32)> = Vec::new();
    for ring in 0..5 {
        let d = HOME_RING[0] + (HOME_RING[1] - HOME_RING[0]) * ring as f32 / 4.0;
        for k in 0..24 {
            let a = k as f32 / 24.0 * TAU + ring as f32 * 0.13;
            let at = [FREE_ROAM_START[0] + a.cos() * d, FREE_ROAM_START[1] + a.sin() * d];
            if let Some(h) = ground(settings, at[0], at[1]) {
                candidates.push((at, h));
            }
        }
    }
    candidates.sort_by(|a, b| b.1.total_cmp(&a.1));
    candidates.into_iter().take(8).find_map(|(at, _)| shoulder(settings, climb(settings, at)))
}

/// Up from `at` to the top of the hill it's on
fn climb(settings: &TerrainSettings, mut at: [f32; 2]) -> [f32; 2] {
    let Some(mut h) = ground(settings, at[0], at[1]) else {
        return at;
    };
    for step in [12.0, 6.0, 3.0, 1.5] {
        for _ in 0..40 {
            let next = (0..8)
                .map(|k| {
                    let a = k as f32 / 8.0 * TAU;
                    [at[0] + a.cos() * step, at[1] + a.sin() * step]
                })
                .filter_map(|p| Some((p, ground(settings, p[0], p[1])?)))
                .max_by(|a, b| a.1.total_cmp(&b.1));
            match next {
                Some((p, g)) if g > h + 0.01 => (at, h) = (p, g),
                _ => break,
            }
        }
    }
    at
}

/// How high the top of a shrine's platform at `at` is: over the highest ground under it
fn top_of(settings: &TerrainSettings, at: [f32; 2]) -> f32 {
    let mut top = ground(settings, at[0], at[1]).unwrap_or(0.0);
    for k in 0..12 {
        let a = k as f32 / 12.0 * TAU;
        if let Some(g) = ground(settings, at[0] + a.cos() * PLATFORM, at[1] + a.sin() * PLATFORM) {
            top = top.max(g);
        }
    }
    top + 0.3
}

/// Whether `summit` is a peak fit for a shrine: well over the sea, standing up out of the land
/// round it in the normal world, and away from the dragons' lairs
fn fine_summit(settings: &TerrainSettings, summit: [f32; 2]) -> bool {
    let Some(h) = ground(settings, summit[0], summit[1]) else {
        return false;
    };
    let land = land_of(settings.seed);
    if h < sea_level_m(settings) + 4.0 {
        return false;
    }
    let around: Vec<f32> = (0..12)
        .filter_map(|k| {
            let a = k as f32 / 12.0 * TAU;
            ground(settings, summit[0] + a.cos() * 60.0, summit[1] + a.sin() * 60.0)
        })
        .collect();
    let rise = if land == Land::Normal { 14.0 } else { 5.0 };
    if land != Land::Sky && (around.is_empty() || h - around.iter().sum::<f32>() / (around.len() as f32) < rise) {
        return false;
    }
    let lairs = dragonlair::lairs_set(settings.seed).into_iter().map(|(_, at)| at).chain(dragonlair::far_lair(settings));
    lairs.into_iter().all(|l| (l[0] - summit[0]).hypot(l[1] - summit[1]) > 60.0)
}

/// How far the ground under a shrine's platform at `at` rises or falls from its middle, in meters
fn unevenness(settings: &TerrainSettings, at: [f32; 2]) -> f32 {
    let h = ground(settings, at[0], at[1]).unwrap_or(0.0);
    (0..12)
        .map(|k| {
            let a = k as f32 / 12.0 * TAU;
            ground(settings, at[0] + a.cos() * PLATFORM, at[1] + a.sin() * PLATFORM).map_or(f32::MAX, |g| (g - h).abs())
        })
        .fold(0.0, f32::max)
}

/// Where on the peak at `summit` a shrine goes, if anywhere: the highest spot near the top level
/// enough for its platform with a way up to it, as the very tops of the peaks are sharp
fn shoulder(settings: &TerrainSettings, summit: [f32; 2]) -> Option<[f32; 2]> {
    if !fine_summit(settings, summit) {
        return None;
    }
    let peak = ground(settings, summit[0], summit[1])?;
    let mut spots: Vec<([f32; 2], f32)> = (-SHOULDER..=SHOULDER)
        .flat_map(|j| (-SHOULDER..=SHOULDER).map(move |i| [summit[0] + i as f32 * 3.0, summit[1] + j as f32 * 3.0]))
        .filter_map(|at| Some((at, ground(settings, at[0], at[1])?)))
        .filter(|&(_, h)| h > peak - BELOW_TOP)
        .collect();
    spots.sort_by(|a, b| b.1.total_cmp(&a.1));
    spots.into_iter().map(|(at, _)| at).filter(|&at| unevenness(settings, at) < 2.2).take(24).find(|&at| good_peak(settings, at))
}

/// Whether `at` is where a shrine can stand: on the top of a fine peak, near enough level for its
/// platform, with a way up to it, see way_up()
pub fn good_peak(settings: &TerrainSettings, at: [f32; 2]) -> bool {
    let summit = climb(settings, at);
    let (Some(h), Some(peak)) = (ground(settings, at[0], at[1]), ground(settings, summit[0], summit[1])) else {
        return false;
    };
    let near = (summit[0] - at[0]).hypot(summit[1] - at[1]) <= SHOULDER as f32 * 3.0 * 1.5;
    fine_summit(settings, summit) && near && h > peak - BELOW_TOP - 0.01 && unevenness(settings, at) < 2.2 && way_up(settings, at).is_some()
}
/// Which way there's a way up to a shrine at `at`, coming down from it: a way no step of which is
/// higher than STEEPEST for each meter along, down to land a good way lower, or in the sky isles,
/// any island at all, reached by a throw of the hook and a glide
pub fn way_up(settings: &TerrainSettings, at: [f32; 2]) -> Option<[f32; 2]> {
    ground(settings, at[0], at[1])?;
    if land_of(settings.seed) == Land::Sky {
        return Some([1.0, 0.0]);
    }
    let sea = sea_level_m(settings);
    (0..24)
        .map(|k| {
            let a = k as f32 / 24.0 * TAU;
            [a.cos(), a.sin()]
        })
        .find(|d| {
            // From the foot of the steps up onto its platform
            let top = top_of(settings, at);
            let mut last = top - STEPS;
            for step in 4..=120 {
                let Some(g) = ground(settings, at[0] + d[0] * step as f32, at[1] + d[1] * step as f32) else {
                    return false;
                };
                if (g - last).abs() > if step == 4 { STEEPEST + 0.3 } else { STEEPEST } {
                    return false;
                }
                last = g;
                // Down off the peak onto land a good way below, or down to the shore
                if top - g > 14.0 || g < sea + 1.0 {
                    return g > sea - 0.5 || step > 20;
                }
            }
            true
        })
}

impl Terrain {
    /// The wing shrines reaching into this piece, and the sites of those whose middles are its
    /// own, for their pairs of wings to lie in, see src\main.rs
    pub(super) fn wing_shrines(&self, features: &mut Features) -> Vec<Object> {
        let reach = 8.0;
        let mut objects = Vec::new();
        for at in aeries(&self.settings, [self.lo[0] - reach, self.lo[1] - reach], [self.hi[0] + reach, self.hi[1] + reach]) {
            let facing = way_up(&self.settings, [at[0], at[1]]).unwrap_or([1.0, 0.0]);
            let mut rng = Rng(hash3(at[0] as i32 as u32, at[1] as i32 as u32, self.settings.seed ^ 0x5417));
            let (shapes, orb) = shrine(at, facing, &mut rng);
            objects.push(Object::new(shapes, rng.0));
            if self.inside(at[0], at[1]) {
                features.sites.push(Site { kind: SHRINE, position: at, facing });
                features.lights.push(Light { position: orb, color: [1.0, 0.9, 0.62], reach: 12.0 });
            }
        }
        objects
    }
}

/// A wing shrine with the top of its platform at `at`, its way in facing `facing`: the platform,
/// a ring of standing stones open at the front, a perch behind with carved stone wings and a
/// glowing orb on it, two banner poles either side of the way in with streamers flying off them,
/// and a nest of sticks and feathers in the middle for the pair of wings to lie in. With where the
/// orb is, for its light.
fn shrine(at: [f32; 3], facing: [f32; 2], rng: &mut Rng) -> (Vec<Shape>, Vec3) {
    let [x, y, top] = at;
    let side = [-facing[1], facing[0]];
    let p = |u: f32, v: f32, z: f32| [x + facing[0] * u + side[0] * v, y + facing[1] * u + side[1] * v, top + z];
    let mut s = Vec::new();
    // The orb glowing on the perch, and the perch, carved wings spread from its head
    let orb = p(-1.3, 0.0, 1.85);
    s.push(Shape::sphere(orb, 0.3, GLOW_HOLY, CRYSTAL, false));
    for v in [-1.0f32, 1.0] {
        let root = p(-1.3, v * 0.25, 1.35);
        let along = [side[0] * v * 0.85, side[1] * v * 0.85, 0.5];
        let middle = add(root, scale(along, 0.8));
        s.push(Shape::leaf(middle, along, [facing[0], facing[1], 0.0], 1.7, 0.6, LIMESTONE, TEMPLE_STONE));
    }
    s.push(Shape::capsule(p(-1.3, -0.9, 1.3), p(-1.3, 0.9, 1.3), 0.08, 0.08, DEAD_WOOD, BARK_DARK, false));
    s.push(Shape::capsule(p(-1.3, 0.0, -0.2), p(-1.3, 0.0, 1.45), 0.4, 0.28, TEMPLE_STONE, MOSS, false));
    // Feathers stuck in the nest's rim, white and gold, and the nest, a ring of sticks
    for k in 0..10 {
        let a = k as f32 / 10.0 * TAU + rng.range(0.0, 0.3);
        let out = [a.cos(), a.sin()];
        let foot = [x + out[0] * 0.95, y + out[1] * 0.95, top + 0.35];
        let along = [out[0] * 0.5, out[1] * 0.5, 1.0];
        s.push(Shape::leaf(add(foot, scale(along, 0.22)), along, [out[0], out[1], 0.0], 0.55, 0.13, if k % 3 == 0 { GOLD } else { FUR_WHITE }, FUR_CREAM));
    }
    for k in 0..14 {
        let a = k as f32 / 14.0 * TAU;
        let b = a + rng.range(0.5, 0.9);
        let (r0, r1) = (rng.range(0.85, 1.1), rng.range(0.85, 1.1));
        let from = [x + a.cos() * r0, y + a.sin() * r0, top + rng.range(0.05, 0.3)];
        let to = [x + b.cos() * r1, y + b.sin() * r1, top + rng.range(0.05, 0.3)];
        s.push(Shape::capsule(from, to, 0.12, 0.09, if k % 4 == 0 { BARK_DARK } else { DEAD_WOOD }, BARK_DARK, false));
    }
    // Banner poles either side of the way in, a banner hanging from each and streamers flying off
    // their tops on the wind
    let wind = normalize([0.8, 0.45, -0.12]);
    for (v, cloth) in [(-1.0f32, TEMPLE_TEAL), (1.0, TEMPLE_RED)] {
        let foot = p(1.9, v * 1.9, -0.3);
        let head = p(1.9, v * 1.9, 5.4);
        s.push(Shape::sphere(head, 0.16, GOLD, GOLD, false));
        s.push(Shape::leaf(p(1.9, v * 2.35, 4.2), [0.0, 0.0, -1.0], [facing[0], facing[1], 0.0], 2.0, 0.75, cloth, GOLD));
        for (k, stuff) in [(0, LINEN), (1, GOLD)] {
            let length = 2.8 - 0.6 * k as f32;
            let from = sub(head, [0.0, 0.0, 0.25 + 0.35 * k as f32]);
            s.push(Shape::leaf(add(from, scale(wind, length * 0.5)), wind, [0.0, 0.0, 1.0], length, 0.2, stuff, stuff));
        }
        s.push(Shape::capsule(foot, head, 0.1, 0.08, WOOD, BARK_DARK, false));
    }
    // Standing stones round it, open at the front for the way in
    for k in 1..8 {
        let a = k as f32 / 8.0 * TAU;
        let (u, v) = (a.cos() * 3.4, a.sin() * 3.4);
        let tall = rng.range(1.8, 3.0);
        let lean = [rng.range(-0.15, 0.15), rng.range(-0.15, 0.15)];
        s.push(Shape::capsule(p(u, v, -1.0), p(u + lean[0], v + lean[1], tall), 0.42, 0.26, STONE, MOSS, false));
    }
    // Two steps up onto it at the way in, and the platform, down into the ground under it
    for (u, down) in [(2.75, 0.75), (3.35, 1.2)] {
        s.push(Shape::capsule(p(u, -0.8, -down), p(u, 0.8, -down), 0.3, 0.3, TEMPLE_STONE, LIMESTONE, false));
    }
    // The platform, down into the ground under it
    s.push(Shape::block([x - PLATFORM, y - PLATFORM, top - 3.0], [x + PLATFORM, y + PLATFORM, top], TEMPLE_STONE, LIMESTONE));
    (s, orb)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wing shrines stand on peaks with a way up, a few in the normal world, one or two near where
    /// each realm is come to, and they're the same every time
    #[test]
    fn wing_shrines_stand_on_reachable_peaks() {
        let settings = |seed: u32| TerrainSettings { width_m: 96, height_m: 64, voxels_per_meter: 4, seed, grass_tufts: false };
        let mut all = 0;
        for seed in [1, 2, 77, 4021] {
            let s = settings(seed);
            let lo = [FREE_ROAM_START[0] - 2600.0, FREE_ROAM_START[1] - 2600.0];
            let hi = [FREE_ROAM_START[0] + 2600.0, FREE_ROAM_START[1] + 2600.0];
            let started = std::time::Instant::now();
            let found = aeries(&s, lo, hi);
            println!("seed {seed}: {} wing shrines in 27 square km, found in {:.0} ms", found.len(), started.elapsed().as_secs_f64() * 1000.0);
            for at in &found {
                let g = land_height_m(&s, at[0], at[1]).expect("on land");
                assert!(at[2] >= g && at[2] - g < 2.6, "seed {seed}: its platform's on the ground there: {at:?} over {g}");
                assert!(good_peak(&s, [at[0], at[1]]), "seed {seed}: {at:?}");
                // On the top of its peak: none of it much higher, nor far off
                let summit = climb(&s, [at[0], at[1]]);
                let peak = land_height_m(&s, summit[0], summit[1]).unwrap();
                assert!(peak - g <= BELOW_TOP + 0.01 && (summit[0] - at[0]).hypot(summit[1] - at[1]) < 60.0, "seed {seed}: {at:?} is {:.1} m below its top", peak - g);
                // The way up: walked down it, no step higher than a jump
                let d = way_up(&s, [at[0], at[1]]).expect("a way up");
                let mut last = at[2] - STEPS;
                for step in 4..=16 {
                    let h = land_height_m(&s, at[0] + d[0] * step as f32, at[1] + d[1] * step as f32).unwrap();
                    assert!((h - last).abs() <= STEEPEST + 0.3 + 1e-3, "seed {seed}: a step of {} m", (h - last).abs());
                    last = h;
                }
            }
            assert_eq!(aeries(&s, lo, hi), found, "the same every time");
            assert!(found.len() <= 20, "seed {seed}: {} in 27 square km", found.len());
            all += found.len();
        }
        assert!(all >= 4, "only {all} in four landscapes");
        // Near where a realm's come to
        for seed in [0xF100_0001, 0xF200_0001, 0xF300_0001, 0xF400_0001, 0xF500_0001] {
            let s = settings(seed);
            let near = aeries(&s, [FREE_ROAM_START[0] - 500.0, FREE_ROAM_START[1] - 500.0], [FREE_ROAM_START[0] + 500.0, FREE_ROAM_START[1] + 500.0]);
            assert!(!near.is_empty(), "realm {seed:x} has none near its start");
            for at in &near {
                assert!(good_peak(&s, [at[0], at[1]]), "realm {seed:x}: {at:?}");
            }
        }
    }

    /// A shrine's built: its site there for its pair of wings, its platform of stone and its orb
    #[test]
    fn a_wing_shrine_is_built() {
        let settings = TerrainSettings { width_m: 64, height_m: 64, voxels_per_meter: 4, seed: 2, grass_tufts: false };
        let at = aeries(&settings, [FREE_ROAM_START[0] - 2600.0, FREE_ROAM_START[1] - 2600.0], [FREE_ROAM_START[0] + 2600.0, FREE_ROAM_START[1] + 2600.0])[0];
        let corner = [((at[0] - 32.0) * 4.0) as i64, ((at[1] - 32.0) * 4.0) as i64];
        let (terrain, world) = generate_area(&settings, corner, [64 * 4, 64 * 4], &RiverCache::default());
        let site = terrain.sites().iter().find(|s| s.kind == SHRINE).expect("the shrine's site");
        assert!((site.position[0] - at[0]).abs() < 0.5 && (site.position[1] - at[1]).abs() < 0.5);
        let count = |m: u8| (0..world.size[2] as i64).map(|z| (100..156).flat_map(|y| (100..156).map(move |x| (x, y))).filter(|&(x, y)| world.material_at(x, y, z) == m).count()).sum::<usize>();
        assert!(count(TEMPLE_STONE) > 500 && count(GLOW_HOLY) >= 2, "stone {} orb {}", count(TEMPLE_STONE), count(GLOW_HOLY));
    }
}
