//! Roads between the towns and to the castles: tracks three or four meters wide winding over the
//! land between them, keeping off the steeps, round the hills and over the rivers on bridges, of
//! gravel out in the country, rough rocky stones near the towns and castles and up in the hills,
//! and sand in the desert and along the shore, with packed earth along their edges. Signposts
//! stand where they leave each town or castle and where two meet, an arm pointing along each
//! with the name of where it leads and how far, and a board with the name of the place itself.
//!
//! Which places a road joins only goes by where they are: two towns have a road between them if
//! they're near enough and no other town is nearer to both of them than they are to each other,
//! which joins up every town with any near it the shortest ways round and a few more besides, and
//! each castle has one to its nearest town. Each road is the cheapest way across the land between
//! them, steep ground costing much more than flat and every river crossed costing a bridge, worked
//! out once on a coarse grid and smoothed. All of it is worked out the same whichever piece of the
//! landscape asks, so the world can move on a strip at a time, and kept once worked out.

use super::*;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::OnceLock;

/// The furthest two towns can be apart and still have a road between them, and a castle from the
/// town its road goes to, in meters
const ROAD_REACH: f32 = 1300.0;
const CASTLE_ROAD_REACH: f32 = 800.0;
/// How far a road may wander from the straight line between its ends, in meters
const CORRIDOR: f32 = 200.0;
/// The grid the way is found on, in meters
const GRID: f32 = 4.0;
/// Half how wide roads are out in the country, and near the places they join
pub(super) const ROAD_HALF: f32 = 1.75;
const WIDE_HALF: f32 = 2.1;
/// How far past its edge the land is eased down or up to a road
const VERGE: f32 = 2.5;
/// How far out from the towns and castles roads are of rough stones
const NEAR_PLACE: f32 = 130.0;
/// How far apart the points along a road are, in meters
const SPACING: f32 = 2.0;
/// How far from the middle of a road it has a say in what the ground is
pub(super) const ROAD_TOUCH: f32 = WIDE_HALF + VERGE + 0.25;
/// How far out of the place it leaves a road's way is found from, past where it all is
const CLEAR_OF_PLACE: f32 = 6.0;
/// The squares roads are looked up by along them, in meters
const ROAD_CELL: f32 = 8.0;
/// How steep the ground can be for a road, as a rise over the run, before it's all but closed
const STEEPEST: f32 = 0.28;

/// Which town or castle: one in each of the squares they're planned in, see towns.rs and castles.rs
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub(super) struct PlaceKey {
    pub castle: bool,
    pub cx: i32,
    pub cy: i32,
}

/// A town or castle as the roads see it: what it's called, the middle of it, where roads leave
/// it, each a point at the edge of what's built and the way out from there, and where it all is,
/// x0, y0, x1, y1 in meters
pub(super) struct Place {
    pub key: PlaceKey,
    pub name: String,
    pub middle: [f32; 2],
    pub doors: Vec<([f32; 2], [f32; 2])>,
    pub bounds: [f32; 4],
}

impl Place {
    pub fn new(key: PlaceKey, settings: &TerrainSettings, middle: [f32; 2], doors: Vec<([f32; 2], [f32; 2])>, bounds: [f32; 4]) -> Place {
        Place { key, name: name_of(key, settings, middle), middle, doors, bounds }
    }

    /// Where a road out of door `k` leaves what's built of it, well clear of it
    fn clear_of(&self, k: usize) -> [f32; 2] {
        let (at, way) = self.doors[k];
        let out = (0..120)
            .map(|d| d as f32)
            .find(|&d| {
                let p = [at[0] + way[0] * d, at[1] + way[1] * d];
                p[0] < self.bounds[0] || p[1] < self.bounds[1] || p[0] > self.bounds[2] || p[1] > self.bounds[3]
            })
            .unwrap_or(0.0);
        [at[0] + way[0] * (out + CLEAR_OF_PLACE), at[1] + way[1] * (out + CLEAR_OF_PLACE)]
    }
}

// Names

const TEMPERATE_HEADS: [&str; 28] = [
    "Ash", "Oak", "Thorn", "Mill", "Elder", "Bram", "Hazel", "Wick", "Briar", "Har", "Dun", "Ox", "Kings", "Fair", "Stone", "Wolf", "Raven", "Mead", "Ald", "Wil", "Fen", "Barrow", "Holly", "Elm", "Bury", "Lark", "Haw", "Rush",
];
const TEMPERATE_TAILS: [&str; 20] = ["ford", "bury", "stead", "wick", "ham", "ton", "field", "brook", "hollow", "dale", "by", "worth", "more", "well", "cross", "gate", "mere", "leigh", "bridge", "combe"];
const SNOW_HEADS: [&str; 14] = ["Frost", "Rime", "Winter", "Snow", "Hoar", "Pine", "Cold", "White", "Storm", "Grey", "Bleak", "Ice", "Hel", "Var"];
const SNOW_TAILS: [&str; 12] = ["holm", "fell", "heim", "vik", "gard", "hold", "march", "stead", "watch", "mark", "by", "rest"];
const DESERT_HEADS: [&str; 14] = ["Khar", "Zar", "Ol", "Mir", "Qas", "Sab", "Tam", "Ash", "Ur", "Nah", "Bel", "Ras", "Dal", "Has"];
const DESERT_TAILS: [&str; 12] = ["ara", "esh", "im", "abad", "una", "eth", "ar", "ira", "oun", "al", "eya", "ad"];
const JUNGLE_HEADS: [&str; 12] = ["Ix", "Tla", "Oco", "Yax", "Cua", "Pal", "Ma", "Ka", "Xi", "Tep", "Ho", "Ze"];
const JUNGLE_TAILS: [&str; 10] = ["tlan", "pan", "coa", "lum", "naal", "que", "tec", "mal", "huac", "ba"];
const CASTLE_HEADS: [&str; 18] = ["Raven", "Black", "High", "Grey", "Iron", "Storm", "Wolf", "Dragon", "Eagle", "Lion", "Silver", "Red", "Crow", "Thorn", "Winter", "Dusk", "Gold", "Hawk"];
const CASTLE_TAILS: [&str; 12] = ["crag", "hold", "mont", "stone", "fell", "spire", "watch", "guard", "mere", "helm", "rock", "keep"];

/// What a town or castle is called: from the parts of names of the kind of land it's in, a
/// castle's ending in "Castle", or "Keep" for a small number of them
fn name_of(key: PlaceKey, settings: &TerrainSettings, at: [f32; 2]) -> String {
    let h = |k: u32| hash3(key.cx as u32, key.cy as u32, settings.seed ^ k.wrapping_mul(0x9e37_79b9) ^ if key.castle { 0xca57_1e00 } else { 0x7041_0000 }) as usize;
    let join = |heads: &[&str], tails: &[&str]| {
        let (head, mut tail) = (heads[h(1) % heads.len()], tails[h(2) % tails.len()]);
        // Not the same word twice, as Stonestone, nor the same letter either side of the join
        if head.to_lowercase().ends_with(&tail[..1]) || head.eq_ignore_ascii_case(tail) {
            tail = tails[(h(2) + 1) % tails.len()];
        }
        format!("{head}{tail}")
    };
    if key.castle {
        let name = join(&CASTLE_HEADS, &CASTLE_TAILS);
        return if name.ends_with("keep") || name.ends_with("hold") || h(3) % 4 != 0 { format!("{name} Castle") } else { format!("{name} Keep") };
    }
    let c = climate(settings, at[0], at[1]);
    if c.desert > 0.5 {
        join(&DESERT_HEADS, &DESERT_TAILS)
    } else if c.snow > 0.5 {
        join(&SNOW_HEADS, &SNOW_TAILS)
    } else if c.jungle > 0.5 {
        join(&JUNGLE_HEADS, &JUNGLE_TAILS)
    } else {
        join(&TEMPERATE_HEADS, &TEMPERATE_TAILS)
    }
}

// What's worked out, kept

/// A point along a road, in meters: where its middle is and how high, half how wide it is there,
/// how much of rough stones it's made, how far along it is, and whether it's on a bridge
#[derive(Clone, Copy, Debug)]
pub(super) struct RoadPoint {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub half: f32,
    pub rocky: f32,
    pub s: f32,
    pub bridge: bool,
}

/// A bridge over a river: its deck from `a` to `b` in meters, half how wide, and whether it's of
/// stone, for the wider rivers, or of timber
#[derive(Clone, Copy, Debug)]
pub(super) struct Bridge {
    pub a: [f32; 3],
    pub b: [f32; 3],
    pub half: f32,
    pub stone: bool,
    /// The water under it
    pub water: f32,
}

/// A road between two places, from the door of the first to the door of the second
pub(super) struct Road {
    pub ends: [PlaceKey; 2],
    pub names: [String; 2],
    pub doors: [usize; 2],
    pub points: Vec<RoadPoint>,
    pub bridges: Vec<Bridge>,
    /// Where it all is, and what it touches, x0, y0, x1, y1 in meters
    pub bounds: [f32; 4],
}

impl Road {
    fn length(&self) -> f32 {
        self.points.last().map_or(0.0, |p| p.s)
    }
}

/// A wooden signpost by a road, in meters: where the foot of its post is, its arms, and the board
/// with the name of the place it stands at the edge of, if it does, facing the way out
#[derive(Clone, Debug)]
pub struct Signpost {
    pub at: [f32; 3],
    pub arms: Vec<Arm>,
    pub home: Option<(String, [f32; 2])>,
}

/// An arm of a signpost: which way it points, how high it is over the foot of the post, where the
/// road that way leads and how far along it that is, in meters
#[derive(Clone, Debug)]
pub struct Arm {
    pub way: [f32; 2],
    pub height: f32,
    pub to: String,
    pub meters: f32,
}

impl Arm {
    /// How far it is, as a signpost would say it: in tens of meters, or kilometers further off
    pub fn how_far(&self) -> String {
        if self.meters >= 1000.0 { format!("{:.1} km", self.meters / 1000.0) } else { format!("{:.0} m", (self.meters / 10.0).round().max(1.0) * 10.0) }
    }
}

type Slot<T> = Arc<OnceLock<T>>;

/// The roads of one landscape and what they go by, worked out once each
#[derive(Default)]
struct Network {
    places: Mutex<HashMap<PlaceKey, Slot<Option<Arc<Place>>>>>,
    roads: Mutex<HashMap<(PlaceKey, PlaceKey), Slot<Option<Arc<Road>>>>>,
    /// The crypt over the way down into the dungeon in each square, see Terrain::dungeon_in()
    crypts: Mutex<HashMap<(i32, i32), Slot<Option<[f32; 4]>>>>,
    /// The signposts put up so far, by where they stand, for their words to be shown, see
    /// signposts_near()
    signs: Mutex<HashMap<(i32, i32), Arc<Signpost>>>,
}

/// The network of `settings`' landscape. Everything it goes by only depends on the land's shape,
/// which only depends on these, see height_at()
fn network(settings: &TerrainSettings) -> Arc<Network> {
    static NETWORKS: Mutex<Vec<((u32, u32, bool), Arc<Network>)>> = Mutex::new(Vec::new());
    let key = (settings.seed, settings.height_m, arena_of(settings).is_some());
    let mut all = NETWORKS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, n)) = all.iter().find(|(k, _)| *k == key) {
        return n.clone();
    }
    // Only a few landscapes are ever visited in a game
    if all.len() > 8 {
        all.remove(0);
    }
    let n = Arc::new(Network::default());
    all.push((key, n.clone()));
    n
}

/// Forgets all that's been worked out of `settings`' landscape's roads, to work it out afresh
#[cfg(test)]
fn forget(settings: &TerrainSettings) {
    let fresh = network(settings);
    *fresh.places.lock().unwrap() = HashMap::new();
    *fresh.roads.lock().unwrap() = HashMap::new();
    *fresh.crypts.lock().unwrap() = HashMap::new();
    *fresh.signs.lock().unwrap() = HashMap::new();
}

/// What's kept in `map` for `key`, worked out by `make` the first time it's wanted. Others wanting
/// it meanwhile wait for it, rather than working it out too.
fn kept<K: std::hash::Hash + Eq + Copy, T: Clone>(map: &Mutex<HashMap<K, Slot<T>>>, key: K, make: impl FnOnce() -> T) -> T {
    let slot = map.lock().unwrap_or_else(|e| e.into_inner()).entry(key).or_default().clone();
    slot.get_or_init(make).clone()
}

/// Whether roads are laid in this landscape: only where there are towns, out on the land
pub(super) fn has_roads(settings: &TerrainSettings) -> bool {
    matches!(land_of(settings.seed), Land::Normal | Land::Ice)
}

/// The shortest distance from `p` to the segment from `a` to `b`, and how far along it the
/// nearest point is, 0 to 1
fn to_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> (f32, f32) {
    let (ab, ap) = ([b[0] - a[0], b[1] - a[1]], [p[0] - a[0], p[1] - a[1]]);
    let length2 = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if length2 > 0.0 { ((ap[0] * ab[0] + ap[1] * ab[1]) / length2).clamp(0.0, 1.0) } else { 0.0 };
    ((ap[0] - ab[0] * t).hypot(ap[1] - ab[1] * t), t)
}

/// How far rectangle `r` is from the segment from `a` to `b`, near enough: 0 if it crosses it
fn segment_off_rect(a: [f32; 2], b: [f32; 2], r: [f32; 4]) -> f32 {
    let inside = |p: [f32; 2]| p[0] >= r[0] && p[1] >= r[1] && p[0] <= r[2] && p[1] <= r[3];
    let corners = [[r[0], r[1]], [r[2], r[1]], [r[2], r[3]], [r[0], r[3]]];
    if inside(a) || inside(b) {
        return 0.0;
    }
    let cross = |o: [f32; 2], p: [f32; 2], q: [f32; 2]| (p[0] - o[0]) * (q[1] - o[1]) - (p[1] - o[1]) * (q[0] - o[0]);
    let mut least = f32::MAX;
    for k in 0..4 {
        let (c, d) = (corners[k], corners[(k + 1) % 4]);
        if cross(a, b, c).signum() != cross(a, b, d).signum() && cross(c, d, a).signum() != cross(c, d, b).signum() {
            return 0.0;
        }
        least = least.min(to_segment(c, a, b).0);
    }
    let off = |p: [f32; 2]| ((r[0] - p[0]).max(p[0] - r[2]).max(0.0)).hypot((r[1] - p[1]).max(p[1] - r[3]).max(0.0));
    least.min(off(a)).min(off(b))
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

impl Terrain {
    /// The town or castle `key`, worked out once
    fn place_of(&self, key: PlaceKey, cache: &RiverCache) -> Option<Arc<Place>> {
        let net = network(&self.settings);
        kept(&net.places, key, || if key.castle { self.castle_place(key.cx, key.cy, cache) } else { self.town_place(key.cx, key.cy, cache) }.map(Arc::new))
    }

    /// The towns, or the castles, whose middles are in rectangle `r`, x0, y0, x1, y1 in meters.
    /// Those not yet worked out are worked out all at once, spread over the threads, as each
    /// takes a few milliseconds.
    fn places_in(&self, r: [f32; 4], castle: bool, cache: &RiverCache) -> Vec<Arc<Place>> {
        let (cell, reach) = if castle { (castles::CASTLE_CELL, castles::CASTLE_REACH) } else { (towns::TOWN_CELL, towns::TOWN_REACH) };
        let first = [0, 1].map(|i| ((r[i] - reach) / cell).floor() as i32);
        let last = [0, 1].map(|i| ((r[i + 2] + reach) / cell).floor() as i32);
        let keys: Vec<PlaceKey> = (first[1]..=last[1]).flat_map(|cy| (first[0]..=last[0]).map(move |cx| PlaceKey { castle, cx, cy })).collect();
        let net = network(&self.settings);
        let missing: Vec<PlaceKey> = {
            let places = net.places.lock().unwrap_or_else(|e| e.into_inner());
            keys.iter().filter(|k| places.get(k).is_none_or(|s| s.get().is_none())).copied().collect()
        };
        if missing.len() > 1 {
            let threads = threads().min(missing.len());
            thread::scope(|s| {
                for t in 0..threads {
                    let missing = &missing;
                    s.spawn(move || {
                        give_way();
                        for key in missing.iter().skip(t).step_by(threads) {
                            self.place_of(*key, cache);
                        }
                    });
                }
            });
        }
        keys.into_iter()
            .filter_map(|k| self.place_of(k, cache))
            .filter(|p| p.middle[0] >= r[0] && p.middle[1] >= r[1] && p.middle[0] < r[2] && p.middle[1] < r[3])
            .collect()
    }

    /// Where the crypt over the way down into the dungeon in DUNGEON_CELL square `cx`, `cy` is, if
    /// there's one, x0, y0, x1, y1 in meters
    fn crypt(&self, cx: i32, cy: i32, cache: &RiverCache) -> Option<[f32; 4]> {
        let net = network(&self.settings);
        kept(&net.crypts, (cx, cy), || self.dungeon_in(cx, cy, cache).and_then(|d| d.world_rects().first().copied()))
    }

    /// Every road coming within `margin` meters of rectangle `r`, x0, y0, x1, y1 in meters, and the
    /// towns and castles they join
    fn roads_near(&self, r: [f32; 4], margin: f32, cache: &RiverCache) -> Vec<Arc<Road>> {
        if !has_roads(&self.settings) {
            return Vec::new();
        }
        let reach = CORRIDOR + margin;
        // The towns a road coming near could join, and those that could be nearer to both ends of
        // one of those than they are to each other: no further from the middle of the road's ends
        // than half its length and as much again times the root of three over two. And for the
        // castles near enough for their roads to come near, the towns round them.
        let towns = self.places_in(grown(r, reach + ROAD_REACH * 1.37 + 1.0).max_with(grown(r, reach + 2.0 * CASTLE_ROAD_REACH + 1.0)), false, cache);
        let castles = self.places_in(grown(r, reach + CASTLE_ROAD_REACH), true, cache);
        let mut pairs: Vec<(Arc<Place>, Arc<Place>)> = Vec::new();
        for (i, a) in towns.iter().enumerate() {
            for b in &towns[i + 1..] {
                let d = distance(a.middle, b.middle);
                if d > ROAD_REACH || segment_off_rect(a.middle, b.middle, r) > reach {
                    continue;
                }
                // No other town nearer to both
                if towns.iter().any(|c| c.key != a.key && c.key != b.key && distance(c.middle, a.middle) < d && distance(c.middle, b.middle) < d) {
                    continue;
                }
                pairs.push((a.clone(), b.clone()));
            }
        }
        for k in &castles {
            let nearest = towns.iter().filter(|t| distance(t.middle, k.middle) <= CASTLE_ROAD_REACH).min_by(|a, b| distance(a.middle, k.middle).total_cmp(&distance(b.middle, k.middle)));
            if let Some(t) = nearest
                && segment_off_rect(t.middle, k.middle, r) <= reach
            {
                pairs.push((t.clone(), k.clone()));
            }
        }
        // The ways found one after another, spread over the threads too
        let net = network(&self.settings);
        let found: Vec<Option<Arc<Road>>> = if pairs.len() > 1 {
            let threads = threads().min(pairs.len());
            let mut found = vec![None; pairs.len()];
            thread::scope(|s| {
                let handles: Vec<_> = (0..threads)
                    .map(|t| {
                        let (pairs, net) = (&pairs, &net);
                        s.spawn(move || {
                            give_way();
                            pairs.iter().enumerate().skip(t).step_by(threads).map(|(i, (a, b))| (i, kept(&net.roads, (a.key.min(b.key), a.key.max(b.key)), || self.lay_road(a, b, cache).map(Arc::new)))).collect::<Vec<_>>()
                        })
                    })
                    .collect();
                for h in handles {
                    for (i, road) in h.join().unwrap() {
                        found[i] = road;
                    }
                }
            });
            found
        } else {
            pairs.iter().map(|(a, b)| kept(&net.roads, (a.key.min(b.key), a.key.max(b.key)), || self.lay_road(a, b, cache).map(Arc::new))).collect()
        };
        let touched = grown(r, margin + ROAD_TOUCH);
        let mut roads: Vec<Arc<Road>> = found.into_iter().flatten().filter(|road| overlaps(road.bounds, touched, 0.0)).collect();
        roads.sort_by_key(|road| road.ends);
        roads
    }

    /// Finds the way for a road from `a` to `b`, if there's one
    fn lay_road(&self, a: &Place, b: &Place, cache: &RiverCache) -> Option<Road> {
        // The first end the one that comes first, so it's the same road whichever way it's asked for
        let (a, b) = if a.key < b.key { (a, b) } else { (b, a) };
        // Out of whichever of their ways out makes for the cheapest way
        let (from, to): (Vec<[f32; 2]>, Vec<[f32; 2]>) = ((0..a.doors.len()).map(|k| a.clear_of(k)).collect(), (0..b.doors.len()).map(|k| b.clear_of(k)).collect());
        let (way, da, db) = self.find_way(&from, &to, a.middle, b.middle, cache)?;
        let (sa, sb) = (from[da], to[db]);
        // From the door of one out of it, along the way found, into the door of the other
        let mut line = vec![a.doors[da].0, sa];
        line.extend(way.iter().skip(1).take(way.len().saturating_sub(2)));
        line.extend([sb, b.doors[db].0]);
        let line = resample(&chaikin(&line, 3), SPACING);
        let mut road = self.shape_road(&line, cache);
        road.ends = [a.key, b.key];
        road.names = [a.name.clone(), b.name.clone()];
        road.doors = [da, db];
        Some(road)
    }

    /// The cheapest way from any of `starts` to any of `goals` over a grid GRID meters across,
    /// never more than CORRIDOR off the straight line between `a` and `b`, the middles of the
    /// places they're the ways out of: steep ground costs more, and very steep ground much more,
    /// each river crossed costs a bridge, and the most near a waterfall, and the sea, the towns and
    /// castles, the crypts over dungeons and the ravines are out of the way altogether. The points
    /// along it, each a grid point but the first and last, and which start and goal it joins.
    fn find_way(&self, starts: &[[f32; 2]], goals: &[[f32; 2]], a: [f32; 2], b: [f32; 2], cache: &RiverCache) -> Option<(Vec<[f32; 2]>, usize, usize)> {
        let settings = &self.settings;
        let sea = sea_level_m(settings);
        let (from, to) = (a, b);
        let lo = [from[0].min(to[0]) - CORRIDOR, from[1].min(to[1]) - CORRIDOR];
        let hi = [from[0].max(to[0]) + CORRIDOR, from[1].max(to[1]) + CORRIDOR];
        let n = [((hi[0] - lo[0]) / GRID).ceil() as usize + 1, ((hi[1] - lo[1]) / GRID).ceil() as usize + 1];
        let at = |i: usize| [lo[0] + (i % n[0]) as f32 * GRID, lo[1] + (i / n[0]) as f32 * GRID];
        let node = |p: [f32; 2]| -> usize {
            let i = (((p[0] - lo[0]) / GRID).round().max(0.0) as usize).min(n[0] - 1);
            let j = (((p[1] - lo[1]) / GRID).round().max(0.0) as usize).min(n[1] - 1);
            i + j * n[0]
        };
        // Open, out of the way, a river, or a river by a waterfall, which can be crossed, but only
        // where there's no other way
        const OPEN: u8 = 0;
        const SHUT: u8 = 1;
        const WATER: u8 = 2;
        const FALLS: u8 = 3;
        let mut what = vec![OPEN; n[0] * n[1]];
        let mark = |r: [f32; 4], kind: u8, what: &mut Vec<u8>| {
            let i0 = (((r[0] - lo[0]) / GRID).floor().max(0.0) as usize).min(n[0]);
            let i1 = (((r[2] - lo[0]) / GRID).ceil().max(0.0) as usize + 1).min(n[0]);
            let j0 = (((r[1] - lo[1]) / GRID).floor().max(0.0) as usize).min(n[1]);
            let j1 = (((r[3] - lo[1]) / GRID).ceil().max(0.0) as usize + 1).min(n[1]);
            for j in j0..j1 {
                for i in i0..i1 {
                    let w = &mut what[i + j * n[0]];
                    if *w != SHUT && (kind == SHUT || kind > *w) {
                        *w = kind;
                    }
                }
            }
        };
        let area = [lo[0], lo[1], hi[0], hi[1]];
        // Rivers, their banks and all, and round the lips of their waterfalls
        let rivers = self.rivers_near(cache, lo, hi);
        for river in &rivers {
            for (k, p) in river.iter().enumerate() {
                if p.x < lo[0] - 20.0 || p.y < lo[1] - 20.0 || p.x > hi[0] + 20.0 || p.y > hi[1] + 20.0 {
                    continue;
                }
                let falls = p.lip || river.get(k + 1).is_some_and(|q| p.surface - q.surface > 0.3);
                if falls {
                    mark(around([p.x, p.y], [p.half_width + RIVER_BANK + 3.0; 2]), FALLS, &mut what);
                } else if k % 2 == 0 {
                    mark(around([p.x, p.y], [p.half_width + RIVER_BANK + RIVER_WOBBLE; 2]), WATER, &mut what);
                }
            }
        }
        // The towns and castles round about, the crypts and the ravines
        let castles = self.places_in(grown(area, castles::CASTLE_REACH + RAVINE_REACH), true, cache);
        for place in self.places_in(grown(area, towns::TOWN_REACH), false, cache).iter().chain(&castles) {
            mark(grown(place.bounds, 3.0), SHUT, &mut what);
        }
        let reach = DUNGEON_REACH + 2.0;
        let first = [0, 1].map(|i| ((area[i] - reach) / DUNGEON_CELL).floor() as i32);
        let last = [0, 1].map(|i| ((area[i + 2] + reach) / DUNGEON_CELL).floor() as i32);
        for cy in first[1]..=last[1] {
            for cx in first[0]..=last[0] {
                if let Some(c) = self.crypt(cx, cy, cache) {
                    mark(grown(c, 4.0), SHUT, &mut what);
                }
            }
        }
        // Those coming near a castle aren't cut, see Terrain::ravines()
        for course in ravines::courses_near(settings, area) {
            if castles.iter().any(|c| overlaps(course.bounds, c.bounds, 3.0)) {
                continue;
            }
            for rect in course.rects() {
                mark(grown(rect, 4.0), SHUT, &mut what);
            }
        }

        let starts_at: Vec<usize> = starts.iter().map(|&p| node(p)).collect();
        let goals_at: Vec<usize> = goals.iter().map(|&p| node(p)).collect();
        let is_goal = |q: usize| goals_at.contains(&q);
        let guess = |p: [f32; 2]| goals.iter().map(|&g| distance(p, g)).fold(f32::MAX, f32::min);
        let in_corridor = |p: [f32; 2]| to_segment(p, from, to).0 <= CORRIDOR;
        let arena = self.arena;
        let mut heights = vec![f32::NAN; n[0] * n[1]];
        let mut height = |i: usize| {
            if heights[i].is_nan() {
                let p = at(i);
                heights[i] = height_at(settings, p[0], p[1]);
            }
            heights[i]
        };
        // Each step, as far along x and y, and how long
        const STEPS: [(i32, i32); 16] = [(1, 0), (0, 1), (-1, 0), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1), (2, 1), (1, 2), (-1, 2), (-2, 1), (-2, -1), (-1, -2), (1, -2), (2, -1)];
        let mut cost = vec![f32::INFINITY; n[0] * n[1]];
        let mut from_node = vec![u32::MAX; n[0] * n[1]];
        let mut done = vec![false; n[0] * n[1]];
        let mut open = BinaryHeap::new();
        for &s in &starts_at {
            cost[s] = 0.0;
            open.push(Reverse((guess(at(s)).to_bits(), s as u32)));
        }
        let mut looked = 0;
        let mut reached = None;
        while let Some(Reverse((_, i))) = open.pop() {
            let i = i as usize;
            if done[i] {
                continue;
            }
            done[i] = true;
            if is_goal(i) {
                reached = Some(i);
                break;
            }
            looked += 1;
            if looked > 400_000 {
                return None;
            }
            if looked % 4096 == 0 {
                give_way();
            }
            let (x, y) = ((i % n[0]) as i32, (i / n[0]) as i32);
            let here = height(i);
            for (dx, dy) in STEPS {
                let (qx, qy) = (x + dx, y + dy);
                if qx < 0 || qy < 0 || qx >= n[0] as i32 || qy >= n[1] as i32 {
                    continue;
                }
                let q = qx as usize + qy as usize * n[0];
                if done[q] {
                    continue;
                }
                let p = at(q);
                let end = is_goal(q);
                if !end && (what[q] == SHUT || !in_corridor(p)) {
                    continue;
                }
                // A long step mustn't cut across what's out of the way beside it
                let between = if dx.abs() + dy.abs() == 3 { [(x + dx / 2) as usize + (y + dy - dy / 2) as usize * n[0], (x + dx - dx / 2) as usize + (y + dy / 2) as usize * n[0]] } else { [q, q] };
                if between.iter().any(|&b| what[b] == SHUT && !is_goal(b) && !end) {
                    continue;
                }
                let there = height(q);
                if there < sea + 0.8 && !end {
                    continue;
                }
                if arena.is_some_and(|a| a.keeps_clear(p[0], p[1], 6.0)) {
                    continue;
                }
                let run = ((dx * dx + dy * dy) as f32).sqrt() * GRID;
                let grade = (there - here).abs() / run;
                let mut c = run * (1.0 + 40.0 * grade * grade);
                if grade > STEEPEST {
                    c += run * 80.0;
                }
                let wet = |k: usize| what[k] == WATER || what[k] == FALLS;
                if wet(q) || between.iter().any(|&b| wet(b)) {
                    c += run * 3.0;
                    if !wet(i) {
                        // Another bridge
                        c += 90.0;
                    }
                    if what[q] == FALLS {
                        c += run * 60.0;
                    }
                }
                let total = cost[i] + c;
                if total < cost[q] {
                    cost[q] = total;
                    from_node[q] = i as u32;
                    open.push(Reverse(((total + guess(p)).to_bits(), q as u32)));
                }
            }
        }
        let goal = reached?;
        let to_k = goals_at.iter().position(|&g| g == goal)?;
        let mut way = vec![goals[to_k]];
        let mut i = goal;
        while from_node[i] != u32::MAX {
            i = from_node[i] as usize;
            if from_node[i] != u32::MAX {
                way.push(at(i));
            }
        }
        let from_k = starts_at.iter().position(|&s| s == i)?;
        way.push(starts[from_k]);
        way.reverse();
        Some((way, from_k, to_k))
    }

    /// A road along `line`, points SPACING meters apart: how high it runs, eased over the bumps,
    /// level over bridges, how wide and of what
    fn shape_road(&self, line: &[[f32; 2]], cache: &RiverCache) -> Road {
        let settings = &self.settings;
        let n = line.len();
        let mut s = vec![0.0f32; n];
        for k in 1..n {
            s[k] = s[k - 1] + distance(line[k - 1], line[k]);
        }
        let length = s[n - 1];
        let ground: Vec<f32> = line.iter().map(|p| height_at(settings, p[0], p[1])).collect();
        // Eased over the bumps, cutting into them and banked up over the dips, but by no more than
        // a meter, and following the ground where it leaves the places at either end, as their
        // streets do
        let ease = |z: &[f32], reach: usize| -> Vec<f32> {
            (0..n)
                .map(|k| {
                    let (a, b) = (k.saturating_sub(reach), (k + reach).min(n - 1));
                    let (mut sum, mut weight) = (0.0, 0.0);
                    for (j, &v) in z.iter().enumerate().take(b + 1).skip(a) {
                        let w = (reach + 1 - k.abs_diff(j)) as f32;
                        sum += v * w;
                        weight += w;
                    }
                    sum / weight
                })
                .collect()
        };
        // The land round about each point, rather than along the road, so two roads running
        // together or crossing are as high as each other there
        let mut z: Vec<f32> = (0..n)
            .map(|k| {
                let pinned = smoothstep(4.0, 14.0, s[k].min(length - s[k]));
                ground[k] + (land_round(settings, line[k]) - ground[k]).clamp(-1.0, 1.0) * pinned
            })
            .collect();

        // Bridges where it crosses a river, its banks and all: each from where the river's cut into
        // the land starts to where it ends, level with the higher bank or over the water
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for p in line {
            lo = [lo[0].min(p[0]), lo[1].min(p[1])];
            hi = [hi[0].max(p[0]), hi[1].max(p[1])];
        }
        let rivers = self.rivers_near(cache, lo, hi);
        // The points along it by ROAD_CELL square, to find those near each river point quickly
        let across = [((hi[0] - lo[0]) / ROAD_CELL) as usize + 1, ((hi[1] - lo[1]) / ROAD_CELL) as usize + 1];
        let mut by_cell: Vec<Vec<u32>> = vec![Vec::new(); across[0] * across[1]];
        let cell = |v: f32, i: usize| (((v - lo[i]) / ROAD_CELL).floor().max(0.0) as usize).min(across[i] - 1);
        for (k, p) in line.iter().enumerate() {
            by_cell[cell(p[0], 0) + cell(p[1], 1) * across[0]].push(k as u32);
        }
        // The nearest river point to each point along it, how far past the water's edge it is, the
        // water's height and how wide the river is, half across
        let mut wet: Vec<Option<(f32, f32, f32)>> = vec![None; n];
        for river in &rivers {
            for p in river.iter() {
                let reach = p.half_width + RIVER_BANK + RIVER_WOBBLE;
                if p.x < lo[0] - reach || p.y < lo[1] - reach || p.x > hi[0] + reach || p.y > hi[1] + reach {
                    continue;
                }
                for cy in cell(p.y - reach, 1)..=cell(p.y + reach, 1) {
                    for cx in cell(p.x - reach, 0)..=cell(p.x + reach, 0) {
                        for &k in &by_cell[cx + cy * across[0]] {
                            let q = line[k as usize];
                            let d = (q[0] - p.x).hypot(q[1] - p.y);
                            if d < reach && wet[k as usize].is_none_or(|(e, _, _)| d - p.half_width < e) {
                                wet[k as usize] = Some((d - p.half_width, p.surface, p.half_width));
                            }
                        }
                    }
                }
            }
        }
        let mut bridges = Vec::new();
        let mut on_bridge = vec![false; n];
        let mut k = 0;
        while k < n {
            if wet[k].is_none() {
                k += 1;
                continue;
            }
            let first = k;
            while k < n && wet[k].is_some() {
                k += 1;
            }
            // Only over the water, not where it only runs along a bank
            if (first..k).all(|j| wet[j].is_some_and(|w| w.0 > 0.0)) {
                continue;
            }
            let (a, b) = (first.saturating_sub(1), k.min(n - 1));
            let water = (first..k).filter_map(|j| wet[j].map(|w| w.1)).fold(f32::MIN, f32::max);
            let widest = (first..k).filter_map(|j| wet[j].map(|w| w.2)).fold(0.0, f32::max);
            let deck = z[a].max(z[b]).max(water + 1.0);
            for j in a..=b {
                z[j] = deck;
                on_bridge[j] = true;
            }
            // Up to it, and down from it, no steeper than a ramp
            for j in (0..a).rev() {
                let ramp = deck - (s[a] - s[j]) * 0.16;
                if z[j] >= ramp {
                    break;
                }
                z[j] = ramp;
            }
            for j in b + 1..n {
                let ramp = deck - (s[j] - s[b]) * 0.16;
                if z[j] >= ramp {
                    break;
                }
                z[j] = ramp;
            }
            let pa = [line[a][0], line[a][1], deck];
            let pb = [line[b][0], line[b][1], deck];
            bridges.push(Bridge { a: pa, b: pb, half: ROAD_HALF + 0.35, stone: widest > 4.0, water });
        }

        // Rough stones near the places, up in the hills and here and there along the way; wider
        // near the places
        let seed = hash3((line[0][0] * 4.0) as i32 as u32, (line[0][1] * 4.0) as i32 as u32, settings.seed ^ 0x40ad);
        let high = settings.height_m as f32 * 0.42;
        let mut rocky: Vec<f32> = (0..n)
            .map(|k| {
                let near = smoothstep(NEAR_PLACE, NEAR_PLACE * 0.55, s[k].min(length - s[k]));
                let (a, b) = (k.saturating_sub(5), (k + 5).min(n - 1));
                let grade = (ground[b] - ground[a]).abs() / (s[b] - s[a]).max(1.0);
                let hills = smoothstep(0.09, 0.15, grade).max(smoothstep(high - 2.0, high + 2.0, ground[k]));
                let patches = smoothstep(0.25, 0.4, fbm(s[k] / 140.0, 0.5, 2, seed));
                near.max(hills).max(patches)
            })
            .collect();
        rocky = ease(&rocky, 8);
        let points = (0..n)
            .map(|k| RoadPoint {
                x: line[k][0],
                y: line[k][1],
                z: z[k],
                half: ROAD_HALF + (WIDE_HALF - ROAD_HALF) * smoothstep(90.0, 40.0, s[k].min(length - s[k])),
                rocky: rocky[k],
                s: s[k],
                bridge: on_bridge[k],
            })
            .collect::<Vec<_>>();
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for p in &points {
            lo = [lo[0].min(p.x), lo[1].min(p.y)];
            hi = [hi[0].max(p.x), hi[1].max(p.y)];
        }
        Road { ends: [PlaceKey { castle: false, cx: 0, cy: 0 }; 2], names: Default::default(), doors: [0; 2], points, bridges, bounds: grown([lo[0], lo[1], hi[0], hi[1]], ROAD_TOUCH + 1.0) }
    }
}

/// How high the land is round `p` on the whole, out to ten meters or so, in meters: what a road
/// there levels the bumps to
fn land_round(settings: &TerrainSettings, p: [f32; 2]) -> f32 {
    let (mut sum, mut weight) = (height_at(settings, p[0], p[1]) * 3.0, 3.0);
    for (ring, count, w) in [(4.5f32, 6, 2.0f32), (9.5, 10, 1.0)] {
        for k in 0..count {
            let a = k as f32 * TAU / count as f32 + ring * 0.1;
            sum += height_at(settings, p[0] + a.cos() * ring, p[1] + a.sin() * ring) * w;
            weight += w;
        }
    }
    sum / weight
}

trait MaxWith {
    fn max_with(self, other: Self) -> Self;
}

impl MaxWith for [f32; 4] {
    /// The rectangle round both
    fn max_with(self, o: [f32; 4]) -> [f32; 4] {
        [self[0].min(o[0]), self[1].min(o[1]), self[2].max(o[2]), self[3].max(o[3])]
    }
}

/// `line` with its corners cut `times` over, its ends kept where they are
fn chaikin(line: &[[f32; 2]], times: usize) -> Vec<[f32; 2]> {
    let mut line = line.to_vec();
    for _ in 0..times {
        if line.len() < 3 {
            break;
        }
        let mut next = vec![line[0]];
        for w in line.windows(2) {
            let (a, b) = (w[0], w[1]);
            next.push([a[0] * 0.75 + b[0] * 0.25, a[1] * 0.75 + b[1] * 0.25]);
            next.push([a[0] * 0.25 + b[0] * 0.75, a[1] * 0.25 + b[1] * 0.75]);
        }
        next.push(line[line.len() - 1]);
        // The first and last cut points next to the ends go, so the ends stay put
        next.remove(1);
        next.remove(next.len() - 2);
        line = next;
    }
    line
}

/// Points along `line` `every` meters apart, its ends kept
fn resample(line: &[[f32; 2]], every: f32) -> Vec<[f32; 2]> {
    let mut out = vec![line[0]];
    let mut carried = 0.0;
    for w in line.windows(2) {
        let (a, b) = (w[0], w[1]);
        let d = distance(a, b);
        let mut t = every - carried;
        while t <= d {
            out.push([a[0] + (b[0] - a[0]) * t / d, a[1] + (b[1] - a[1]) * t / d]);
            t += every;
        }
        carried = d - (t - every);
    }
    if out.last().is_some_and(|p| distance(*p, line[line.len() - 1]) > every * 0.3) {
        out.push(line[line.len() - 1]);
    } else if let Some(last) = out.last_mut() {
        *last = line[line.len() - 1];
    }
    out
}

// Laid into the ground

/// A stretch of a road between two points along it
#[derive(Clone, Copy)]
struct Stretch {
    a: RoadPoint,
    b: RoadPoint,
}

/// What a road makes of a column of the ground: how far it is from the middle of the road, and
/// which side, how high the road is there and half how wide, how much of rough stones it's made,
/// and whether it's under a bridge, where the ground's left as it is
#[derive(Clone, Copy, Debug)]
pub(super) struct Spot {
    pub off: f32,
    pub side: f32,
    pub z: f32,
    pub half: f32,
    pub rocky: f32,
    pub bridge: bool,
}

impl Spot {
    /// The ground at a column `g` meters high once the road's laid: its own height across it,
    /// eased back to the land's over the verge
    pub fn ground(&self, g: f32) -> f32 {
        if self.bridge {
            g
        } else if self.off <= self.half {
            self.z
        } else {
            let t = smoothstep(0.0, 1.0, (self.off - self.half) / VERGE);
            self.z + (g - self.z) * t
        }
    }
}

/// What the top of the column at `x`, `y` in meters, `gx`, `gy` in voxels at `vpm` a meter, is
/// made of where road `s` is laid over it, what's under that, and whether it stands a voxel
/// proud, as the odd stone of a rocky road does: None past its packed earth edges. Rough stones of
/// mixed sizes and greys near the places and up in the hills, sand in the desert and along the
/// shore, gravel with ruts worn into it the rest of the way, each giving way to the next raggedly.
#[allow(clippy::too_many_arguments)]
pub(super) fn road_top(s: &Spot, seed: u32, x: f32, y: f32, gx: i64, gy: i64, vpm: f32, desert: bool, snowy: bool, shore: bool) -> Option<(u8, u8, bool)> {
    if s.bridge {
        return None;
    }
    let h = hash3(gx as u32, gy as u32, seed ^ 0x40ad_7e26);
    let roll = unit(h);
    if s.off > s.half {
        // Its edges trodden down to bare earth, thinning out into the land round it
        let edge = (s.off - s.half) / 0.9;
        return (roll > edge * 0.85 && !snowy).then_some(if desert || shore { (SAND, SAND, false) } else { (DIRT, DIRT, false) });
    }
    let ragged = fbm(x / 7.0, y / 7.0, 2, seed.wrapping_add(61)) * 0.3 + (unit(h >> 8) - 0.5) * 0.12;
    if s.rocky + ragged > 0.5 {
        // Stones of a few sizes, each its own grey, with grit between them, the odd one standing proud
        let size = 0.45;
        let (cx, cy) = ((x / size).floor() as i32, (y / size).floor() as i32);
        let (mut first, mut second, mut stone) = (f32::MAX, f32::MAX, 0u32);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let c = hash3((cx + dx) as u32, (cy + dy) as u32, seed ^ 0x570e);
                let p = [(cx + dx) as f32 * size + size * (0.15 + 0.7 * unit(c)), (cy + dy) as f32 * size + size * (0.15 + 0.7 * unit(c >> 10))];
                // Bigger stones push the joints out round them
                let d = (x - p[0]).hypot(y - p[1]) - unit(c >> 20) * 0.14;
                if d < first {
                    (second, first, stone) = (first, d, c);
                } else if d < second {
                    second = d;
                }
            }
        }
        if second - first < 1.1 / vpm {
            let grit = if snowy && roll < 0.6 {
                SNOW
            } else if desert {
                SAND
            } else if roll < 0.3 {
                DIRT
            } else {
                GRAVEL
            };
            return Some((grit, GRAVEL, false));
        }
        let greys = if desert { &[SANDSTONE, SANDSTONE, SANDSTONE_DARK, LIMESTONE, ROCK, SANDSTONE][..] } else { &[STONE, STONE, BRICK_STONE, CUT_STONE, ROCK, MORTAR, SLATE, STONE, BRICK_STONE][..] };
        let m = greys[(hash(stone) % greys.len() as u32) as usize];
        let proud = unit(stone >> 4) < 0.45 && first < size * 0.2;
        return Some((m, GRAVEL, proud));
    }
    let rut = (s.side.abs() - 0.75).abs() < 0.22;
    if desert || shore {
        // Sand, with grit in it, ruts worn darker, and a stone here and there along its edges
        if s.off > s.half - 0.35 && roll < 0.35 {
            return Some((if desert { SANDSTONE_DARK } else { ROCK }, SAND, roll < 0.15));
        }
        let m = if rut && roll < 0.5 {
            if desert { SANDSTONE_DARK } else { DIRT }
        } else if roll < 0.08 {
            GRAVEL
        } else {
            SAND
        };
        return Some((m, SAND, false));
    }
    // Gravel, ruts worn into it to the earth, stones in it, and snow packed down on it in the cold
    let m = if rut && roll < 0.55 {
        if snowy { GRAVEL } else { DIRT }
    } else if snowy && roll < 0.4 {
        SNOW
    } else if roll < 0.08 {
        STONE
    } else if roll < 0.13 {
        MORTAR
    } else {
        GRAVEL
    };
    Some((m, GRAVEL, false))
}

/// The roads near a piece of the landscape, laid out to be looked up column by column: the
/// stretches of them reaching each ROAD_CELL square
pub(super) struct Laid {
    pub roads: Vec<Arc<Road>>,
    stretches: Vec<Stretch>,
    lo: [f32; 2],
    across: [usize; 2],
    cells: Vec<(u32, u32)>,
    index: Vec<u32>,
}

impl Laid {
    pub fn empty() -> Laid {
        Laid { roads: Vec::new(), stretches: Vec::new(), lo: [0.0; 2], across: [0; 2], cells: Vec::new(), index: Vec::new() }
    }

    /// `roads` laid out to be looked up from `lo` to `hi`, in meters
    fn new(roads: Vec<Arc<Road>>, lo: [f32; 2], hi: [f32; 2]) -> Laid {
        let across = [0, 1].map(|i| ((hi[i] - lo[i]) / ROAD_CELL).ceil().max(1.0) as usize);
        let mut stretches = Vec::new();
        let mut lists: Vec<Vec<u32>> = vec![Vec::new(); across[0] * across[1]];
        for road in &roads {
            for w in road.points.windows(2) {
                let (a, b) = (w[0], w[1]);
                let reach = a.half.max(b.half) + VERGE + 0.3;
                let r = [a.x.min(b.x) - reach, a.y.min(b.y) - reach, a.x.max(b.x) + reach, a.y.max(b.y) + reach];
                if r[2] < lo[0] || r[3] < lo[1] || r[0] > hi[0] || r[1] > hi[1] {
                    continue;
                }
                let cell = |v: f32, i: usize| (((v - lo[i]) / ROAD_CELL).floor().max(0.0) as usize).min(across[i] - 1);
                let k = stretches.len() as u32;
                stretches.push(Stretch { a, b });
                for cy in cell(r[1], 1)..=cell(r[3], 1) {
                    for cx in cell(r[0], 0)..=cell(r[2], 0) {
                        lists[cx + cy * across[0]].push(k);
                    }
                }
            }
        }
        let mut cells = Vec::with_capacity(lists.len());
        let mut index = Vec::new();
        for list in lists {
            cells.push((index.len() as u32, list.len() as u32));
            index.extend(list);
        }
        Laid { roads, stretches, lo, across, cells, index }
    }

    /// The stretches reaching the ROAD_CELL square `x`, `y` is in, in meters
    fn stretches_at(&self, x: f32, y: f32) -> &[u32] {
        let (cx, cy) = (((x - self.lo[0]) / ROAD_CELL).floor(), ((y - self.lo[1]) / ROAD_CELL).floor());
        if cx < 0.0 || cy < 0.0 || cx >= self.across[0] as f32 || cy >= self.across[1] as f32 {
            return &[];
        }
        let (start, count) = self.cells[cx as usize + cy as usize * self.across[0]];
        &self.index[start as usize..(start + count) as usize]
    }

    /// What the nearest road makes of the column at `x`, `y` in meters, if any's near enough to
    pub fn spot(&self, x: f32, y: f32) -> Option<Spot> {
        let near = self.stretches_at(x, y);
        if near.is_empty() {
            return None;
        }
        let mut best: Option<Spot> = None;
        for &k in near {
            let Stretch { a, b } = self.stretches[k as usize];
            let (off, t) = to_segment([x, y], [a.x, a.y], [b.x, b.y]);
            let half = a.half + (b.half - a.half) * t;
            if off > half + VERGE || best.is_some_and(|s| s.off - s.half <= off - half) {
                continue;
            }
            let side = (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x);
            best = Some(Spot {
                off,
                side: if side >= 0.0 { off } else { -off },
                z: a.z + (b.z - a.z) * t,
                half,
                rocky: a.rocky + (b.rocky - a.rocky) * t,
                bridge: a.bridge && b.bridge,
            });
        }
        best
    }

    /// Whether a road's edge comes within `margin` meters of `x`, `y`
    pub fn near(&self, x: f32, y: f32, margin: f32) -> bool {
        self.stretches_at(x, y).iter().any(|&k| {
            let Stretch { a, b } = self.stretches[k as usize];
            to_segment([x, y], [a.x, a.y], [b.x, b.y]).0 < a.half.max(b.half) + margin
        }) || (margin > ROAD_CELL * 0.5 && self.near_rect([x, y, x, y], margin))
    }

    /// Whether a road's edge comes within `margin` meters of rectangle `r`, x0, y0, x1, y1
    pub fn near_rect(&self, r: [f32; 4], margin: f32) -> bool {
        self.roads.iter().filter(|road| overlaps(road.bounds, r, margin)).any(|road| {
            road.points.windows(2).any(|w| {
                let (a, b) = (w[0], w[1]);
                segment_off_rect([a.x, a.y], [b.x, b.y], r) < a.half.max(b.half) + margin
            })
        })
    }
}

impl Terrain {
    /// The roads reaching this piece of the landscape and all round it as far as anything's placed,
    /// laid out to be looked up
    pub(super) fn find_roads(&self, cache: &RiverCache) -> Arc<Laid> {
        if !has_roads(&self.settings) {
            return Arc::new(Laid::empty());
        }
        // A little past where anything's placed, as buildings look at the ground round them
        const PAST: f32 = 32.0;
        let r = [self.lo[0], self.lo[1], self.hi[0], self.hi[1]];
        let roads = self.roads_near(r, PAST, cache);
        Arc::new(Laid::new(roads, self.lo.map(|v| v - PAST), self.hi.map(|v| v + PAST)))
    }
}

// Bridges and signposts

/// Which way and how far: `b` from `a`, level, as a unit and its length
fn heading(a: [f32; 2], b: [f32; 2]) -> ([f32; 2], f32) {
    let d = distance(a, b).max(1e-4);
    ([(b[0] - a[0]) / d, (b[1] - a[1]) / d], d)
}

/// Where a road is `ahead` meters on from `s` meters along it, going the way it's laid if
/// `forward`, the other way if not
fn ahead_of(road: &Road, s: f32, ahead: f32, forward: bool) -> [f32; 2] {
    let length = road.length();
    let want = if forward { (s + ahead).min(length) } else { (s - ahead).max(0.0) };
    let k = road.points.partition_point(|p| p.s < want).min(road.points.len() - 1);
    [road.points[k].x, road.points[k].y]
}

/// Of `spots`, the first standing clear of the edges of all of `roads`, or the clearest
fn clear_spot(roads: &[Arc<Road>], spots: &[[f32; 2]]) -> [f32; 2] {
    let clearance = |p: [f32; 2]| {
        roads
            .iter()
            .filter(|r| overlaps(r.bounds, [p[0], p[1], p[0], p[1]], 0.0))
            .flat_map(|r| r.points.windows(2))
            .map(|w| to_segment(p, [w[0].x, w[0].y], [w[1].x, w[1].y]).0 - w[0].half.max(w[1].half))
            .fold(f32::MAX, f32::min)
    };
    spots.iter().copied().find(|&p| clearance(p) > 0.9).unwrap_or_else(|| spots.iter().copied().max_by(|a, b| clearance(*a).total_cmp(&clearance(*b))).unwrap_or(spots[0]))
}

/// The nearest point of `road` to `p`: its index, and how far
fn nearest_point(road: &Road, p: [f32; 2]) -> (usize, f32) {
    road.points.iter().enumerate().map(|(k, q)| (k, distance(p, [q.x, q.y]))).min_by(|a, b| a.1.total_cmp(&b.1)).unwrap_or((0, f32::MAX))
}

impl Terrain {
    /// The ground at `x`, `y` with the roads laid, but without the rivers cut into it, for what
    /// stands beside the roads
    fn ground_by_road(&self, x: f32, y: f32) -> f32 {
        let g = height_at(&self.settings, x, y);
        self.roads.spot(x, y).map_or(g, |s| s.ground(g))
    }

    /// The signposts beside the roads reaching this piece of the landscape: where each road leaves
    /// a town or castle, an arm for each road leaving it there and a board with its name; where
    /// two roads leaving the same way part, and where two roads cross, an arm each way along both.
    /// Only those standing within `r`, x0, y0, x1, y1 in meters, worked out from the roads near,
    /// which are all the roads that pass there.
    fn signposts(&self, r: [f32; 4], cache: &RiverCache) -> Vec<Signpost> {
        let roads = &self.roads.roads;
        let mut signs: Vec<Signpost> = Vec::new();
        // Those a little further out too, as one too near another isn't put up, so it's the same
        // whichever piece they're worked out for
        let wide = grown(r, 24.0);
        let inside = |p: [f32; 2]| p[0] >= wide[0] && p[1] >= wide[1] && p[0] < wide[2] && p[1] < wide[3];
        // Each road's ends: the road, which end, the place and the door
        let mut leaving: Vec<(usize, usize, PlaceKey, usize)> = Vec::new();
        for (i, road) in roads.iter().enumerate() {
            for end in 0..2 {
                leaving.push((i, end, road.ends[end], road.doors[end]));
            }
        }
        leaving.sort_by_key(|&(i, end, key, door)| (key, door, roads[i].ends, end));
        let mut groups: Vec<Vec<(usize, usize)>> = Vec::new();
        for (k, &(i, end, key, door)) in leaving.iter().enumerate() {
            if k > 0 && leaving[k - 1].2 == key && leaving[k - 1].3 == door {
                groups.last_mut().unwrap().push((i, end));
            } else {
                groups.push(vec![(i, end)]);
            }
        }
        for group in &groups {
            let (i, end) = group[0];
            let key = roads[i].ends[end];
            let Some(place) = self.place_of(key, cache) else {
                continue;
            };
            let door = roads[i].doors[end];
            let (_, out) = place.doors[door];
            let clear = place.clear_of(door);
            // Too far off for its signpost, or for where its roads part, to be in the piece
            if !overlaps([clear[0], clear[1], clear[0], clear[1]], wide, 430.0) {
                continue;
            }
            // To the right of the way out, or the left, as it happens
            // clear of the roads leaving there
            let right = if hash3(key.cx as u32, key.cy as u32, door as u32 ^ 0x5197) & 1 == 0 { 1.0 } else { -1.0 };
            let spots: Vec<[f32; 2]> = [(right, 0.0), (-right, 0.0), (right, -3.0), (-right, -3.0), (right, -6.0), (-right, -6.0)]
                .iter()
                .map(|&(side, back)| [clear[0] + out[1] * side * (WIDE_HALF + 1.2) + out[0] * back, clear[1] - out[0] * side * (WIDE_HALF + 1.2) + out[1] * back])
                .collect();
            let foot = clear_spot(roads, &spots);
            if inside(foot) {
                let arms = group
                    .iter()
                    .map(|&(i, end)| {
                        let road = &roads[i];
                        let forward = end == 0;
                        let (k, _) = nearest_point(road, clear);
                        let s = road.points[k].s;
                        let (way, _) = heading(foot, ahead_of(road, s, 30.0, forward));
                        let meters = if forward { road.length() - s } else { s };
                        Arm { way, height: 0.0, to: road.names[1 - end].clone(), meters }
                    })
                    .collect();
                signs.push(Signpost { at: [foot[0], foot[1], self.ground_by_road(foot[0], foot[1])], arms, home: Some((place.name.clone(), out)) });
            }
            // Where roads leaving the same way part, well out of the place
            // How far from end `end` of `road` a point along it is, and the point `d` from that end
            let from = |road: &Road, end: usize, s: f32| if end == 0 { s } else { road.length() - s };
            let at = |road: &Road, end: usize, d: f32| ahead_of(road, from(road, end, d.clamp(0.0, road.length())), 0.0, true);
            for (a, &(i, ei)) in group.iter().enumerate() {
                for &(j, ej) in &group[a + 1..] {
                    let (ri, rj) = (&roads[i], &roads[j]);
                    let start = from(ri, ei, ri.points[nearest_point(ri, clear).0].s);
                    let mut d = start + 10.0;
                    let parted = loop {
                        if d > start + 400.0 || d > ri.length() - 30.0 {
                            break None;
                        }
                        if nearest_point(rj, at(ri, ei, d)).1 > 2.0 * ROAD_HALF + 1.5 {
                            break Some(d);
                        }
                        d += SPACING;
                    };
                    let Some(d) = parted.filter(|&d| d - start > 40.0) else {
                        continue;
                    };
                    // Back a little to where they're still one road, beside it on the side away from
                    // the other
                    let (back, on) = (at(ri, ei, d - 6.0), at(ri, ei, d));
                    let (along, _) = heading(back, on);
                    let dj = from(rj, ej, rj.points[nearest_point(rj, back).0].s);
                    let other = at(rj, ej, dj + 12.0);
                    let away = if (other[0] - back[0]) * along[1] - (other[1] - back[1]) * along[0] > 0.0 { 1.0 } else { -1.0 };
                    let spots: Vec<[f32; 2]> = [(away, 1.3), (away, 3.0), (-away, 1.3), (-away, 3.0)]
                        .iter()
                        .flat_map(|&(side, out)| [0.0f32, -4.0, -8.0, -12.0].map(|b| [back[0] + along[1] * side * (ROAD_HALF + out) + along[0] * b, back[1] - along[0] * side * (ROAD_HALF + out) + along[1] * b]))
                        .collect();
                    let foot = clear_spot(roads, &spots);
                    if !inside(foot) || signs.iter().any(|s| distance([s.at[0], s.at[1]], foot) < 20.0) {
                        continue;
                    }
                    let di = d - 6.0;
                    let arms = vec![
                        Arm { way: heading(foot, at(ri, ei, di - 30.0)).0, height: 0.0, to: place.name.clone(), meters: di },
                        Arm { way: heading(foot, at(ri, ei, di + 30.0)).0, height: 0.0, to: ri.names[1 - ei].clone(), meters: ri.length() - di },
                        Arm { way: heading(foot, at(rj, ej, dj + 30.0)).0, height: 0.0, to: rj.names[1 - ej].clone(), meters: rj.length() - dj },
                    ];
                    signs.push(Signpost { at: [foot[0], foot[1], self.ground_by_road(foot[0], foot[1])], arms, home: None });
                }
            }
        }
        // Where two roads cross, out in the country
        for (i, ri) in roads.iter().enumerate() {
            for rj in &roads[i + 1..] {
                if !overlaps(ri.bounds, rj.bounds, 0.0) || ri.ends.iter().any(|e| rj.ends.contains(e)) {
                    continue;
                }
                // Only where a signpost could stand in the piece
                let here = |w: &&[RoadPoint]| overlaps([w[0].x.min(w[1].x), w[0].y.min(w[1].y), w[0].x.max(w[1].x), w[0].y.max(w[1].y)], wide, 8.0);
                let near_j: Vec<&[RoadPoint]> = rj.points.windows(2).filter(here).collect();
                for wi in ri.points.windows(2).filter(here) {
                    for &wj in &near_j {
                        let (a, b, c, d) = ([wi[0].x, wi[0].y], [wi[1].x, wi[1].y], [wj[0].x, wj[0].y], [wj[1].x, wj[1].y]);
                        let cross = |o: [f32; 2], p: [f32; 2], q: [f32; 2]| (p[0] - o[0]) * (q[1] - o[1]) - (p[1] - o[1]) * (q[0] - o[0]);
                        if cross(a, b, c).signum() == cross(a, b, d).signum() || cross(c, d, a).signum() == cross(c, d, b).signum() {
                            continue;
                        }
                        if wi[0].bridge || wj[0].bridge {
                            continue;
                        }
                        let (along, _) = heading(a, b);
                        // In one of the corners between them
                        let spots: Vec<[f32; 2]> = [(1.0f32, -1.0f32), (-1.0, 1.0), (1.0, 1.0), (-1.0, -1.0)]
                            .iter()
                            .map(|&(side, back)| [a[0] + (along[1] * side + along[0] * back) * (WIDE_HALF + 2.5), a[1] + (-along[0] * side + along[1] * back) * (WIDE_HALF + 2.5)])
                            .collect();
                        let foot = clear_spot(roads, &spots);
                        if !inside(foot) || signs.iter().any(|s| distance([s.at[0], s.at[1]], foot) < 20.0) {
                            continue;
                        }
                        let (si, sj) = (wi[0].s, wj[0].s);
                        let arms = vec![
                            Arm { way: heading(foot, ahead_of(ri, si, 30.0, false)).0, height: 0.0, to: ri.names[0].clone(), meters: si },
                            Arm { way: heading(foot, ahead_of(ri, si, 30.0, true)).0, height: 0.0, to: ri.names[1].clone(), meters: ri.length() - si },
                            Arm { way: heading(foot, ahead_of(rj, sj, 30.0, false)).0, height: 0.0, to: rj.names[0].clone(), meters: sj },
                            Arm { way: heading(foot, ahead_of(rj, sj, 30.0, true)).0, height: 0.0, to: rj.names[1].clone(), meters: rj.length() - sj },
                        ];
                        signs.push(Signpost { at: [foot[0], foot[1], self.ground_by_road(foot[0], foot[1])], arms, home: None });
                    }
                }
            }
        }
        signs.retain(|s| s.at[0] >= r[0] && s.at[1] >= r[1] && s.at[0] < r[2] && s.at[1] < r[3]);
        // The arms one over another down the post, in the order of the way they point
        for sign in &mut signs {
            sign.arms.sort_by(|a, b| a.way[1].atan2(a.way[0]).total_cmp(&b.way[1].atan2(b.way[0])));
            let step = (1.0f32 / sign.arms.len().max(1) as f32).min(0.3);
            for (k, arm) in sign.arms.iter_mut().enumerate() {
                arm.height = 2.2 - k as f32 * step;
            }
        }
        signs
    }

    /// The bridges and signposts of the roads reaching this piece of the landscape, and where
    /// they stand so nothing grows there. The signposts in the piece itself are kept to be read,
    /// see signposts_near().
    pub(super) fn road_works(&self, features: &mut Features, cache: &RiverCache) -> Vec<Object> {
        let mut objects = Vec::new();
        if self.roads.roads.is_empty() {
            return objects;
        }
        let area = [self.lo[0], self.lo[1], self.hi[0], self.hi[1]];
        for road in &self.roads.roads {
            for b in &road.bridges {
                let reach = b.half + 1.5;
                let r = [b.a[0].min(b.b[0]) - reach, b.a[1].min(b.b[1]) - reach, b.a[0].max(b.b[0]) + reach, b.a[1].max(b.b[1]) + reach];
                if overlaps(r, area, 0.0) {
                    objects.push(bridge(b));
                    features.footprints.push([r[0], r[1], r[2], r[3], 0.0]);
                }
            }
        }
        let net = network(&self.settings);
        for sign in self.signposts(grown(area, 2.0), cache) {
            let [x, y, _] = sign.at;
            objects.push(signpost(&sign, self.settings.seed));
            features.footprints.push([x - 1.4, y - 1.4, x + 1.4, y + 1.4, 0.0]);
            if self.inside(x, y) {
                net.signs.lock().unwrap_or_else(|e| e.into_inner()).entry(((x * 4.0) as i32, (y * 4.0) as i32)).or_insert_with(|| Arc::new(sign));
            }
        }
        objects
    }
}

/// Bridge `b`: of cut stone, its deck between parapets, on piers down into the river, or of
/// timber, planks across beams with a rail along either side, on legs
fn bridge(b: &Bridge) -> Object {
    let (along, length) = heading([b.a[0], b.a[1]], [b.b[0], b.b[1]]);
    let (u, v, up) = ([along[0], along[1], 0.0], [-along[1], along[0], 0.0], [0.0, 0.0, 1.0]);
    let deck = b.a[2];
    let point = |a: f32, c: f32, z: f32| [b.a[0] + along[0] * a - along[1] * c, b.a[1] + along[1] * a + along[0] * c, z];
    let (start, end) = (-1.0, length + 1.0);
    let middle = (start + end) * 0.5;
    let half_long = (end - start) * 0.5;
    let mut s = Vec::new();
    if b.stone {
        for side in [-1.0f32, 1.0] {
            s.push(Shape::beam(point(middle, side * (b.half + 0.25), deck + 0.1), [u, v, up], [half_long, 0.25, 0.8], BRICK_STONE, MOSS));
        }
        s.push(Shape::beam(point(middle, 0.0, deck - 0.35), [u, v, up], [half_long, b.half, 0.35], CUT_STONE, STONE));
        let piers = ((length - 4.0) / 7.0).floor().max(0.0) as usize;
        for k in 1..=piers {
            let a = length * k as f32 / (piers + 1) as f32;
            let bottom = b.water - 3.0;
            s.push(Shape::beam(point(a, 0.0, (deck - 0.5 + bottom) * 0.5), [u, v, up], [0.7, b.half + 0.5, (deck - 0.5 - bottom) * 0.5], BRICK_STONE, MOSS));
        }
    } else {
        let mut a = start;
        let mut k = 0;
        while a < end - 0.05 {
            let m = if k % 3 == 1 { WOOD } else { PLANK_PINE };
            s.push(Shape::beam(point(a + 0.15, 0.0, deck - 0.06), [u, v, up], [0.15, b.half + 0.12, 0.06], m, AIR));
            a += 0.34;
            k += 1;
        }
        for side in [-1.0f32, 1.0] {
            s.push(Shape::beam(point(middle, side * (b.half - 0.35), deck - 0.24), [u, v, up], [half_long, 0.13, 0.12], BARK_DARK, AIR));
            let posts = (length / 2.5).ceil().max(1.0) as usize;
            for p in 0..=posts {
                let a = start + 0.2 + (end - start - 0.4) * p as f32 / posts as f32;
                s.push(Shape::capsule(point(a, side * (b.half + 0.08), deck - 0.35), point(a, side * (b.half + 0.08), deck + 1.05), 0.08, 0.075, WOOD, AIR, false));
            }
            s.push(Shape::capsule(point(start + 0.2, side * (b.half + 0.08), deck + 1.0), point(end - 0.2, side * (b.half + 0.08), deck + 1.0), 0.06, 0.06, WOOD, AIR, false));
            s.push(Shape::capsule(point(start + 0.2, side * (b.half + 0.08), deck + 0.55), point(end - 0.2, side * (b.half + 0.08), deck + 0.55), 0.045, 0.045, WOOD, AIR, false));
            let legs = (length / 4.0).floor() as usize;
            for l in 1..legs.max(1) {
                let a = length * l as f32 / legs as f32;
                s.push(Shape::capsule(point(a, side * (b.half - 0.35), deck - 0.3), point(a, side * (b.half - 0.15), b.water - 2.0), 0.12, 0.13, BARK_DARK, AIR, false));
            }
        }
    }
    Object::new(s, hash3((b.a[0] * 4.0) as i32 as u32, (b.a[1] * 4.0) as i32 as u32, 0xb81d))
}

/// Signpost `sign`: a post with its arms, each a board pointed at its end with dark marks along it
/// for the writing, the board with the name of the place its foot's at, and stones round its foot
fn signpost(sign: &Signpost, seed: u32) -> Object {
    let [x, y, g] = sign.at;
    let up = [0.0, 0.0, 1.0];
    let h = hash3((x * 4.0) as i32 as u32, (y * 4.0) as i32 as u32, seed ^ 0x5196);
    let mut s = Vec::new();
    // Writing: dashes a word or so long along a board, in front of it so it wins
    let writing = |s: &mut Vec<Shape>, middle: Vec3, axes: [Vec3; 3], long: f32, z: f32, h: u32| {
        let mut a = -long;
        let mut k = 0;
        while a < long - 0.05 {
            let word = (0.08 + 0.16 * unit(hash(h ^ k))).min(long - a);
            let c = add(middle, add(scale(axes[0], a + word * 0.5), [0.0, 0.0, z]));
            s.push(Shape::beam(c, axes, [word * 0.5, 0.045, 0.022], BARK_DARK, AIR));
            a += word + 0.07;
            k += 1;
        }
    };
    for (k, arm) in sign.arms.iter().enumerate() {
        let w = [arm.way[0], arm.way[1], 0.0];
        let side = [-w[1], w[0], 0.0];
        let axes = [w, side, up];
        let middle = [x + w[0] * 0.62, y + w[1] * 0.62, g + arm.height];
        writing(&mut s, add(middle, scale(w, -0.04)), axes, 0.36, 0.0, h ^ k as u32);
        s.push(Shape::beam(middle, axes, [0.5, 0.035, 0.105], PLANK_BIRCH, AIR));
        // Its end pointed, a square turned on its corner
        let tip = [x + w[0] * 1.1, y + w[1] * 1.1, g + arm.height];
        let d = FRAC_1_SQRT_2;
        s.push(Shape::beam(tip, [[w[0] * d, w[1] * d, d], side, [-w[0] * d, -w[1] * d, d]], [0.075, 0.035, 0.075], PLANK_BIRCH, AIR));
    }
    if let Some((_, out)) = &sign.home {
        let face = [out[0], out[1], 0.0];
        let across = [-out[1], out[0], 0.0];
        let axes = [across, face, up];
        let middle = [x + out[0] * 0.13, y + out[1] * 0.13, g + 1.3];
        writing(&mut s, middle, axes, 0.45, 0.07, h ^ 0x40e);
        writing(&mut s, middle, axes, 0.3, -0.07, h ^ 0x40f);
        s.push(Shape::beam(middle, axes, [0.62, 0.04, 0.2], PLANK_BIRCH, AIR));
        s.push(Shape::beam(middle, axes, [0.68, 0.03, 0.26], BARK_DARK, AIR));
    }
    s.push(Shape::sphere([x, y, g + 2.48], 0.1, BARK_DARK, AIR, false));
    s.push(Shape::capsule([x, y, g - 0.5], [x, y, g + 2.45], 0.085, 0.07, DEAD_WOOD, BARK_DARK, false));
    for k in 0..3 {
        let a = k as f32 * 2.1 + unit(h >> k) * 1.5;
        s.push(Shape::sphere([x + a.cos() * 0.25, y + a.sin() * 0.25, g + 0.02], 0.11 + 0.06 * unit(h >> (k + 8)), ROCK, MOSS, false));
    }
    Object::new(s, h)
}

/// The signposts within `within` meters of `at`, in meters, of `settings`' landscape, of those put
/// up in the pieces of it built so far, nearest first: for what's written on them to be shown
pub fn signposts_near(settings: &TerrainSettings, at: [f32; 2], within: f32) -> Vec<Signpost> {
    if !has_roads(settings) {
        return Vec::new();
    }
    let net = network(settings);
    let signs = net.signs.lock().unwrap_or_else(|e| e.into_inner());
    let mut near: Vec<Signpost> = signs.values().filter(|s| distance([s.at[0], s.at[1]], at) < within).map(|s| (**s).clone()).collect();
    near.sort_by(|a, b| distance([a.at[0], a.at[1]], at).total_cmp(&distance([b.at[0], b.at[1]], at)));
    near
}

/// The roads and places within the square `across` meters across round `middle`, for the map:
/// each road as the points along it, and each town and castle, where its middle is, its name, and
/// whether it's a castle. Worked out if they haven't been, which can take a second or two.
pub fn map_roads(settings: &TerrainSettings, middle: [f32; 2], across: f32) -> (Vec<Vec<[f32; 2]>>, Vec<([f32; 2], String, bool)>) {
    if !has_roads(settings) {
        return (Vec::new(), Vec::new());
    }
    let half = across * 0.5;
    let r = [middle[0] - half, middle[1] - half, middle[0] + half, middle[1] + half];
    let terrain = Terrain::around(settings, [r[0], r[1]], [r[2], r[3]]);
    let cache = RiverCache::default();
    let roads = terrain.roads_near(r, 0.0, &cache).iter().map(|road| road.points.iter().map(|p| [p.x, p.y]).collect()).collect();
    let places = terrain.places_in(r, false, &cache).into_iter().chain(terrain.places_in(r, true, &cache)).map(|p| (p.middle, p.name.clone(), p.key.castle)).collect();
    (roads, places)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SETTINGS: TerrainSettings = TerrainSettings { width_m: 64, height_m: 64, voxels_per_meter: 4, seed: 1, grass_tufts: false };
    /// Three kilometers round the start of Free Roam
    const REGION: [f32; 4] = [-1300.0, -1300.0, 1700.0, 1700.0];

    /// The roads of REGION, and the towns and castles they could go to
    fn region() -> (Terrain, RiverCache, Vec<Arc<Road>>, Vec<Arc<Place>>, Vec<Arc<Place>>) {
        let r = REGION;
        let terrain = Terrain::around(&SETTINGS, [r[0], r[1]], [r[2], r[3]]);
        let cache = RiverCache::default();
        let roads = terrain.roads_near(r, 0.0, &cache);
        let far = grown(r, CORRIDOR + ROAD_REACH + 200.0);
        let towns = terrain.places_in(far, false, &cache);
        let castles = terrain.places_in(far, true, &cache);
        (terrain, cache, roads, towns, castles)
    }

    /// Every town with another near enough has a road to the rest, and the roads join up every
    /// town that's near enough to another to be joined, the shortest ways round; every castle with
    /// a town near enough has a road to it
    #[test]
    fn every_town_is_reached() {
        let (terrain, cache, _, _, _) = region();
        // The roads of the region and a good way round it, and the towns and castles they join
        let wider = grown(REGION, 800.0);
        let roads = terrain.roads_near(wider, 0.0, &cache);
        let towns = terrain.places_in(grown(wider, CORRIDOR + ROAD_REACH), false, &cache);
        let castles = terrain.places_in(REGION, true, &cache);
        // Those in the region itself, whose roads and neighbours are all found
        let inner = REGION;
        let inside = |p: &Place| p.middle[0] > inner[0] && p.middle[1] > inner[1] && p.middle[0] < inner[2] && p.middle[1] < inner[3];
        let roads_of = |p: &Place| roads.iter().filter(|r| r.ends.contains(&p.key)).count();
        // Which towns can be got to from each, by road, and by hops no longer than ROAD_REACH
        let reach = |linked: &dyn Fn(usize, usize) -> bool| {
            let mut group: Vec<usize> = (0..towns.len()).collect();
            for _ in 0..towns.len() {
                for i in 0..towns.len() {
                    for j in 0..towns.len() {
                        if linked(i, j) && group[j] < group[i] {
                            group[i] = group[j];
                        }
                    }
                }
            }
            group
        };
        let by_road = reach(&|i, j| roads.iter().any(|r| r.ends.contains(&towns[i].key) && r.ends.contains(&towns[j].key)));
        let near = reach(&|i, j| distance(towns[i].middle, towns[j].middle) <= ROAD_REACH);
        let (mut checked, mut joined) = (0, 0);
        for (i, t) in towns.iter().enumerate() {
            if !inside(t) {
                continue;
            }
            let neighbours = towns.iter().filter(|o| o.key != t.key && distance(o.middle, t.middle) <= ROAD_REACH).count();
            if neighbours > 0 {
                assert!(roads_of(t) > 0, "{} has {neighbours} towns near but no road", t.name);
            }
            checked += 1;
            // Every town in the region it could be joined to by hops near enough, it is, by road
            let all_near = (0..towns.len()).filter(|&j| near[j] == near[i] && inside(&towns[j])).all(|j| by_road[j] == by_road[i]);
            if !all_near {
                println!("{} isn't joined to every town near enough", t.name);
            }
            joined += all_near as usize;
        }
        let mut with_road = 0;
        for k in &castles {
            if towns.iter().any(|t| distance(t.middle, k.middle) <= CASTLE_ROAD_REACH) {
                assert!(roads_of(k) == 1, "{} has a town near but {} roads", k.name, roads_of(k));
                with_road += 1;
            }
        }
        println!("{} roads; {checked} towns well inside, {joined} joined to every town near enough by hops; {with_road} of {} castles with a road", roads.len(), castles.len());
        assert!(checked > 0 && joined == checked);
    }

    /// No road runs through a town or a castle but out of its own, by the way out it leaves by, nor
    /// over the crypt of a dungeon or a ravine, nor through a river but on a bridge
    #[test]
    fn roads_keep_out_of_whats_built() {
        let (terrain, cache, roads, towns, castles) = region();
        let places: Vec<&Arc<Place>> = towns.iter().chain(&castles).collect();
        let inside = |r: [f32; 4], p: [f32; 2]| p[0] > r[0] && p[1] > r[1] && p[0] < r[2] && p[1] < r[3];
        let mut crypts = Vec::new();
        for cy in (REGION[1] / DUNGEON_CELL) as i32 - 1..=(REGION[3] / DUNGEON_CELL) as i32 + 1 {
            for cx in (REGION[0] / DUNGEON_CELL) as i32 - 1..=(REGION[2] / DUNGEON_CELL) as i32 + 1 {
                crypts.extend(terrain.crypt(cx, cy, &cache));
            }
        }
        let (mut points, mut bridged) = (0, 0);
        for road in &roads {
            let ends: Vec<&&Arc<Place>> = places.iter().filter(|p| road.ends.contains(&p.key)).collect();
            assert_eq!(ends.len(), 2, "{} to {}", road.names[0], road.names[1]);
            for p in &road.points {
                let at = [p.x, p.y];
                points += 1;
                for place in &places {
                    if !inside(place.bounds, at) {
                        continue;
                    }
                    // Only in its own, straight out of the way out it leaves by
                    let end = road.ends.iter().position(|k| *k == place.key);
                    let door = end.map(|e| place.doors[road.doors[e]]);
                    let on_way = door.is_some_and(|(d, way)| {
                        let off = [at[0] - d[0], at[1] - d[1]];
                        (off[0] * way[1] - off[1] * way[0]).abs() < 1.0 && off[0] * way[0] + off[1] * way[1] > -1.0
                    });
                    assert!(on_way, "{} to {} goes through {} at {at:?}", road.names[0], road.names[1], place.name);
                }
                assert!(!crypts.iter().any(|c| inside(*c, at)), "{} to {} goes over a crypt at {at:?}", road.names[0], road.names[1]);
                bridged += p.bridge as usize;
            }
            for course in ravines::courses_near(&SETTINGS, road.bounds) {
                if castles.iter().any(|c| overlaps(course.bounds, c.bounds, 3.0)) {
                    continue;
                }
                for r in course.rects() {
                    assert!(!road.points.iter().any(|p| inside(r, [p.x, p.y])), "{} to {} crosses a ravine", road.names[0], road.names[1]);
                }
            }
            // Over the water only on bridges
            for river in terrain.rivers_near(&cache, [road.bounds[0], road.bounds[1]], [road.bounds[2], road.bounds[3]]) {
                for q in river.iter() {
                    for p in road.points.iter().filter(|p| !p.bridge) {
                        assert!((p.x - q.x).hypot(p.y - q.y) > q.half_width - 0.5, "{} to {} wades a river at {:.0}, {:.0}", road.names[0], road.names[1], p.x, p.y);
                    }
                }
            }
        }
        println!("{} roads, {points} points along them, {bridged} on bridges", roads.len());
    }

    /// The signposts name real towns and castles, and say how far along the road to them it is;
    /// no two places near each other share a name
    #[test]
    fn signposts_name_real_places() {
        let (mut terrain, cache, roads, towns, castles) = region();
        let names: Vec<&str> = towns.iter().chain(&castles).map(|p| p.name.as_str()).collect();
        for (i, a) in names.iter().enumerate() {
            assert!(!names[i + 1..].contains(a), "two places called {a}");
        }
        let r = REGION;
        terrain.roads = Arc::new(Laid::new(roads.clone(), [r[0] - 40.0, r[1] - 40.0], [r[2] + 40.0, r[3] + 40.0]));
        let signs = terrain.signposts(r, &cache);
        let (mut homes, mut arms) = (0, 0);
        for sign in &signs {
            assert!(!sign.arms.is_empty());
            if let Some((home, _)) = &sign.home {
                assert!(names.contains(&home.as_str()), "a sign for {home}");
                homes += 1;
            }
            for arm in &sign.arms {
                assert!(names.contains(&arm.to.as_str()), "an arm to {}", arm.to);
                let longest = roads.iter().filter(|road| road.names.contains(&arm.to)).map(|road| road.length()).fold(0.0, f32::max);
                assert!(arm.meters > 0.0 && arm.meters <= longest + 1.0, "{} {} m along a road at most {longest} m", arm.to, arm.meters);
                assert!((arm.way[0].hypot(arm.way[1]) - 1.0).abs() < 1e-3);
                arms += 1;
            }
            // Beside the road, not on it
            let off = roads.iter().flat_map(|road| road.points.iter()).map(|p| (p.x - sign.at[0]).hypot(p.y - sign.at[1])).fold(f32::MAX, f32::min);
            assert!(off > ROAD_HALF + 0.3 && off < 8.0, "a sign {off} m from the road: {sign:?}");
        }
        // Every place with a road has a sign where it leaves
        let inner = grown(REGION, -100.0);
        for p in towns.iter().chain(&castles).filter(|p| overlaps(p.bounds, inner, 0.0) && p.middle[0] > inner[0] && p.middle[1] > inner[1] && p.middle[0] < inner[2] && p.middle[1] < inner[3]) {
            if roads.iter().any(|road| road.ends.contains(&p.key)) {
                assert!(signs.iter().any(|s| s.home.as_ref().is_some_and(|h| h.0 == p.name)), "no sign at {}", p.name);
            }
        }
        println!("{} signposts, {homes} at the edges of places, {arms} arms", signs.len());
        assert!(homes > 0);
    }

    /// Built in strips one way and the other, the ground along a road comes out the same as built
    /// in one go, road, bridge, signpost and all
    #[test]
    fn roads_are_the_same_however_the_world_is_built() {
        let (_, _, roads, _, _) = region();
        // Round the first signpost by a town, where there's most to get wrong
        let road = &roads[0];
        let p = road.points[road.points.len() / 2];
        let vpm = SETTINGS.voxels_per_meter as i64;
        let origin = [((p.x - 32.0) as i64) * vpm, ((p.y - 32.0) as i64) * vpm];
        let side = 64 * vpm as u32;
        let cache = RiverCache::default();
        let (whole_terrain, whole) = generate_area(&SETTINGS, origin, [side, side], &cache);
        assert!(!whole_terrain.roads.roads.is_empty());
        for axis in 0..2 {
            // Strips are whole bricks across
            for cut in [side / 4, side / 2 + 8] {
                let mut size = [side, side];
                size[axis] = cut;
                let (_, first) = generate_area(&SETTINGS, origin, size, &cache);
                let mut later = origin;
                later[axis] += cut as i64;
                size[axis] = side - cut;
                let (_, second) = generate_area(&SETTINGS, later, size, &cache);
                for x in 0..side as i64 {
                    for y in 0..side as i64 {
                        let (piece, lx, ly) = if [x, y][axis] < cut as i64 { (&first, x, y) } else if axis == 0 { (&second, x - cut as i64, y) } else { (&second, x, y - cut as i64) };
                        for z in 0..whole.size[2] as i64 {
                            assert_eq!(piece.material_at(lx, ly, z), whole.material_at(x, y, z), "voxel {x}, {y}, {z}, cut along {axis} at {cut}");
                        }
                    }
                }
            }
        }
    }

    /// Worked out afresh, from somewhere else first, the roads come out just the same
    #[test]
    fn roads_are_the_same_whoever_asks_first() {
        let (terrain, cache, roads, _, _) = region();
        let first: Vec<Vec<[f32; 3]>> = roads.iter().map(|r| r.points.iter().map(|p| [p.x, p.y, p.z]).collect()).collect();
        // Forgotten, then asked for from the far corner of the region first, a piece at a time
        forget(&SETTINGS);
        for k in (0..6).rev() {
            let at = REGION[0] + k as f32 * 500.0;
            terrain.roads_near([at, at, at + 224.0, at + 224.0], 0.0, &cache);
        }
        let again = terrain.roads_near(REGION, 0.0, &cache);
        assert_eq!(roads.len(), again.len());
        for (a, b) in first.iter().zip(&again) {
            assert_eq!(a.len(), b.points.len());
            for (p, q) in a.iter().zip(&b.points) {
                assert_eq!(*p, [q.x, q.y, q.z]);
            }
        }
    }

    /// A map of the roads over a few kilometers, two meters a pixel: towns white, castles red, roads
    /// brown, their rocky stretches grey, bridges blue, signposts yellow. `cargo test --release
    /// road_map -- --ignored`, saved where ROAD_MAP says or in the temp folder, round ROAD_MAP_AT
    /// (x,y in meters) or the start of Free Roam, of ROAD_MAP_SEED's landscape or Free Roam's.
    #[test]
    #[ignore]
    fn road_map() {
        let seed = std::env::var("ROAD_MAP_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
        let settings = TerrainSettings { width_m: 224, height_m: 64, voxels_per_meter: 16, seed, grass_tufts: true };
        let middle = std::env::var("ROAD_MAP_AT").ok().map_or(FREE_ROAM_START, |v| {
            let n: Vec<f32> = v.split(',').filter_map(|n| n.trim().parse().ok()).collect();
            [n[0], n[1]]
        });
        let (side, px) = (3000.0f32, 2.0f32);
        let n = (side / px) as u32;
        let lo = [middle[0] - side * 0.5, middle[1] - side * 0.5];
        let r = [lo[0], lo[1], lo[0] + side, lo[1] + side];
        let terrain = Terrain::around(&settings, [r[0], r[1]], [r[2], r[3]]);
        let cache = RiverCache::default();
        let start = std::time::Instant::now();
        let roads = terrain.roads_near(r, 0.0, &cache);
        let towns = terrain.places_in(r, false, &cache);
        let castles = terrain.places_in(r, true, &cache);
        println!("{} roads, {} towns, {} castles in {:.0} ms", roads.len(), towns.len(), castles.len(), start.elapsed().as_secs_f64() * 1000.0);
        let sea = sea_level_m(&settings);
        let mut map = image::RgbImage::from_fn(n, n, |i, j| {
            let (x, y) = (lo[0] + (i as f32 + 0.5) * px, lo[1] + side - (j as f32 + 0.5) * px);
            let g = height_at(&settings, x, y);
            if g < sea {
                image::Rgb([30, 60, 110])
            } else {
                let up = ((g - sea) / (settings.height_m as f32 - sea)).clamp(0.0, 1.0);
                image::Rgb([(80.0 + 150.0 * up) as u8, (130.0 + 100.0 * up) as u8, (60.0 + 160.0 * up) as u8])
            }
        });
        for river in terrain.rivers_near(&cache, [r[0], r[1]], [r[2], r[3]]) {
            for p in river.iter() {
                let (i, j) = (((p.x - lo[0]) / px) as i64, ((lo[1] + side - p.y) / px) as i64);
                if i >= 0 && j >= 0 && i < n as i64 && j < n as i64 {
                    map.put_pixel(i as u32, j as u32, image::Rgb([60, 170, 255]));
                }
            }
        }
        let mut dot = |x: f32, y: f32, size: i64, c: [u8; 3]| {
            let (ci, cj) = (((x - lo[0]) / px) as i64, ((lo[1] + side - y) / px) as i64);
            for dj in -size..=size {
                for di in -size..=size {
                    let (i, j) = (ci + di, cj + dj);
                    if i >= 0 && j >= 0 && i < n as i64 && j < n as i64 {
                        map.put_pixel(i as u32, j as u32, image::Rgb(c));
                    }
                }
            }
        };
        for road in &roads {
            for p in &road.points {
                let c = if p.bridge { [40, 90, 255] } else if p.rocky > 0.5 { [150, 150, 150] } else { [120, 70, 30] };
                dot(p.x, p.y, 1, c);
            }
            println!("{} to {}: {:.0} m, {} bridges, from {:.0}, {:.0}", road.names[0], road.names[1], road.length(), road.bridges.len(), road.points[0].x, road.points[0].y);
            let along: Vec<String> = road.points.iter().step_by(50).map(|p| format!("{:.0},{:.0} {}", p.x, p.y, if p.rocky > 0.5 { "r" } else { "g" })).collect();
            println!("  every 100 m: {}", along.join("; "));
            for b in &road.bridges {
                println!("  bridge {:?} to {:?}, {}", b.a, b.b, if b.stone { "stone" } else { "timber" });
            }
        }
        for t in &towns {
            dot(t.middle[0], t.middle[1], 8, [255, 255, 255]);
            println!("town {} at {:.0}, {:.0}", t.name, t.middle[0], t.middle[1]);
        }
        for k in &castles {
            dot(k.middle[0], k.middle[1], 8, [220, 30, 30]);
            let joined = roads.iter().any(|road| road.ends.contains(&k.key));
            println!("castle {} at {:.0}, {:.0}{}", k.name, k.middle[0], k.middle[1], if joined { "" } else { ", no road" });
        }
        let mut signs = 0;
        let mut whole = Terrain::around(&settings, [r[0], r[1]], [r[2], r[3]]);
        whole.roads = Arc::new(Laid::new(roads.clone(), [r[0] - 40.0, r[1] - 40.0], [r[2] + 40.0, r[3] + 40.0]));
        for sign in whole.signposts(r, &cache) {
            dot(sign.at[0], sign.at[1], 3, [255, 230, 0]);
            signs += 1;
            println!("sign at {:.0}, {:.0}: {}{}", sign.at[0], sign.at[1], sign.home.as_ref().map_or(String::new(), |h| format!("[{}] ", h.0)), sign.arms.iter().map(|a| format!("{} {}", a.to, a.how_far())).collect::<Vec<_>>().join(", "));
        }
        println!("{signs} signposts");
        let path = std::env::var("ROAD_MAP").map_or(std::env::temp_dir().join("road_map.png"), std::path::PathBuf::from);
        map.save(&path).unwrap();
        println!("map in {}", path.display());
    }

    /// How long finding the roads round where Free Roam starts takes, the first time and once
    /// they're kept: `cargo test --release roads_take -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn roads_take() {
        let settings = TerrainSettings { width_m: 224, height_m: 64, voxels_per_meter: 16, seed: 1, grass_tufts: true };
        let terrain = Terrain::around(&settings, [0.0, 0.0], [224.0, 224.0]);
        let cache = RiverCache::default();
        let start = std::time::Instant::now();
        let roads = terrain.roads_near([0.0, 0.0, 224.0, 224.0], 32.0, &cache);
        println!("{} roads near in {:.0} ms", roads.len(), start.elapsed().as_secs_f64() * 1000.0);
        let start = std::time::Instant::now();
        let roads = terrain.roads_near([0.0, 16.0, 224.0, 240.0], 32.0, &cache);
        println!("a strip on, {} roads near in {:.0} ms", roads.len(), start.elapsed().as_secs_f64() * 1000.0);
    }
}
