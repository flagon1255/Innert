//! Fallen cities under the land: a great vaulted cavern deep under the hills, and in it the ruins
//! of a city of dark stone that sank into the earth long ago, overrun by the dead. A square shaft
//! comes down to it from the surface, a stair in ruins winding round its walls, ringed by broken
//! walls and tall leaning obelisks with void burning on their tops, so it can be seen from afar.
//! At its foot a gate under a broken portcullis opens onto the city: a street of broken houses and
//! towers, a dry canal crossed by a bridge, a square round a cracked obelisk under a cluster of
//! crystals hanging from the vault, a colonnade fallen in places, and past it all a chasm crossed
//! by a single bridge to the temple at the heart, where the city's dead lord waits by its hoard.
//! Roots and vines hang down through cracks in the vault, glowing fungus grows along the walls,
//! and bones lie everywhere.
//!
//! A city's laid out in a frame `at` facing from the shaft into the city: u goes from the middle
//! of the shaft on through the gate, the square and the chasm to the temple, v across, both in
//! meters.

use std::sync::{Arc, Mutex, OnceLock};

use super::*;

/// At most one city in each CITY_CELL square, its shaft well inside it, nothing of it further than
/// CITY_REACH from there, in meters
const CITY_CELL: f32 = 600.0;
const CITY_REACH: f32 = 125.0;
/// How many squares have a city where the land suits, and how many spots are tried in each
const CITY_CHANCE: f32 = 0.7;
const SPOTS: u32 = 28;
/// How many of the spots that suit are looked over for what else is in the way, as that takes a
/// while
const CHECKS: usize = 3;
/// Half how wide the shaft down is, inside its walls, and how wide its stair is
const SHAFT: f32 = 5.0;
const STAIR: f32 = 1.7;
/// Steps in each flight of the stair, from one corner of the shaft to the next
const FLIGHT: usize = 16;
const RISE: f32 = 0.25;
/// The least ground over the vault, in meters, so nothing on the land above breaks into it
const COVER: f32 = 5.0;
/// How much of a cavern's height is the dome over the middle of the hollow cut for it, as in
/// towns.rs, and how far its walls and vault bulge in and out
const DOME: f32 = 0.72;
const ROUGH: f32 = 1.0;
/// How long the gate passage from the foot of the shaft into the city is
const GATE: f32 = 3.5;
/// Half how wide the main street and the cross street are, and the square
const STREET: f32 = 2.0;
const CROSS: f32 = 1.6;
const PLAZA: f32 = 6.5;
/// Half how wide the lanes either side of the main street are, and how far out they are, as part
/// of how far out the city's floor goes
const ALLEY: f32 = 1.2;
const LANE: f32 = 0.6;
/// Half how wide the dry canal is, and how deep
const CANAL: f32 = 1.8;
const CANAL_DEPTH: f32 = 1.6;
/// Half how wide the chasm is, and how far it runs either side of the bridge
const CHASM: f32 = 2.6;
const CHASM_SIDE: f32 = 8.0;
/// Half how wide the way through from the city to the temple is
const THROAT: f32 = 4.5;
/// How far out round the shaft's walls nothing grows, so the obelisks round it stand in the open
const GLADE: f32 = 11.0;
/// The light braziers give, glowing fungus, the void, and crystals
const FIRE: [f32; 3] = [1.0, 0.6, 0.28];
const FUNGUS: [f32; 3] = [0.3, 0.85, 0.45];
const VOID: [f32; 3] = [0.7, 0.4, 1.0];
const CRYSTALS: [f32; 3] = [0.35, 0.75, 1.0];

/// How far a cavern's flat floor goes out, next to the widest of the hollow cut for it
fn floor_width() -> f32 {
    (1.0 - ((1.0 - DOME) / DOME).powi(2)).sqrt()
}

/// A vaulted cavern: the middle of its floor in the city's frame, how far its flat floor goes out
/// along u and v, and how high its vault is over the middle
#[derive(Clone, Copy)]
struct Lobe {
    middle: [f32; 2],
    half: [f32; 2],
    height: f32,
}

impl Lobe {
    /// How far out `u`, `v` is, 1 at the edge of its floor
    fn out(&self, u: f32, v: f32) -> f32 {
        ((u - self.middle[0]) / self.half[0]).powi(2) + ((v - self.middle[1]) / self.half[1]).powi(2)
    }

    /// How high the vault is over the floor at `u`, `v`, before it's roughened, None off its floor
    fn headroom(&self, u: f32, v: f32) -> Option<f32> {
        let e = self.out(u, v);
        (e <= 1.0).then(|| (1.0 - DOME) * self.height + DOME * self.height * (1.0 - e * floor_width().powi(2)).max(0.0).sqrt())
    }

    /// The least headroom there's sure to be over the rectangle `r`, u0, v0, u1, v1, roughness
    /// and all, None if any of it's off the floor
    fn room_over(&self, r: [f32; 4]) -> Option<f32> {
        [(r[0], r[1]), (r[2], r[1]), (r[0], r[3]), (r[2], r[3])].iter().map(|&(u, v)| self.headroom(u, v)).try_fold(f32::MAX, |m, h| Some(m.min(h?))).map(|h| h - ROUGH - 0.3)
    }
}

/// A broken house or tower of the city, in its frame: what it covers, u0, v0, u1, v1, which way
/// its door faces, how high its walls stood, and whether it still has some of its roof
#[derive(Clone, Copy)]
struct Ruin {
    rect: [f32; 4],
    door: [f32; 2],
    wall: f32,
    tower: bool,
    roofed: bool,
    chest: bool,
    seed: u32,
}

/// A step of the stair down the shaft, or a landing: what it covers in the city's frame, u0, v0,
/// u1, v1, and the height of its top
#[derive(Clone, Copy)]
struct Step {
    rect: [f32; 4],
    top: f32,
    /// Which way the wall it's built out from is, nothing for a landing
    wall: [f32; 2],
}

/// What a city's built of: its walls and the stone of their trim, its paving, and its temple
#[derive(Clone, Copy)]
struct Palette {
    wall: (u8, u8),
    trim: (u8, u8),
    paving: [(u8, u8); 2],
    temple: (u8, u8),
}

/// A fallen city, laid out
#[derive(Clone)]
pub(super) struct Undercity {
    at: Frame,
    /// The height of its streets, and the lowest and highest ground round the top of the shaft
    floor: f32,
    rim: [f32; 2],
    /// The cavern the city's in and the one the temple's in
    city: Lobe,
    temple: Lobe,
    /// Where the canal crosses the city along u, and how far out it runs either way along v
    canal: f32,
    canal_reach: f32,
    /// Where the chasm is along u, and how deep it is
    chasm: f32,
    chasm_depth: f32,
    /// The middle of the square
    plaza: [f32; 2],
    /// The stair down the shaft, from the foot up, and where it comes out at the top: the middle
    /// of its last step, which way is out through the wall there, and how high the step is
    stair: Vec<Step>,
    exit_point: [f32; 2],
    exit_out: [f32; 2],
    exit_at: f32,
    /// Which way the stair goes up along the side it leaves by
    exit_up: [f32; 2],
    ruins: Vec<Ruin>,
    stone: Palette,
    /// Where it all is, x0, y0, x1, y1 in meters
    bounds: [f32; 4],
    seed: u32,
}

/// Where the parts of a fallen city are, for trying it out, all in meters: the way down at the
/// top of the shaft, the top of the stair, and which way is out from it, the foot of the shaft, the gate into the city,
/// a street, the square, the bottom of the canal, the way into the temple and the middle of its
/// hall, and the way into the city along its main street
#[allow(dead_code)]
pub struct Sunken {
    pub entrance: [f32; 3],
    pub head: [f32; 3],
    pub out: [f32; 2],
    pub foot: [f32; 3],
    pub gate: [f32; 3],
    pub street: [f32; 3],
    pub plaza: [f32; 3],
    pub canal: [f32; 3],
    pub hall: [f32; 3],
    pub heart: [f32; 3],
    pub forward: [f32; 2],
}

/// The cities planned so far, by seed, height of the land, and square: planning one takes a
/// while, looking over what else is near, and every piece of the landscape it reaches into needs
/// it again
type Plans = HashMap<(u32, u32, i32, i32), Option<Arc<Undercity>>>;
static PLANS: OnceLock<Mutex<Plans>> = OnceLock::new();

/// The corners of the middle line of the stair round the shaft, going round from the far side
const CORNERS: [[f32; 2]; 4] = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
/// Which way is out of the shaft through the wall along each side, from each corner to the next
const OUTWARD: [[f32; 2]; 4] = [[0.0, -1.0], [1.0, 0.0], [0.0, 1.0], [-1.0, 0.0]];

/// The rectangle from `a` to `b`, either way round, u0, v0, u1, v1
fn rect(a: [f32; 2], b: [f32; 2]) -> [f32; 4] {
    [a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1])]
}

impl Terrain {
    /// The fallen cities reaching into this piece of the landscape, built: what they're made of,
    /// where each city near enough for a cave or a ravine reaching in here to run into it is, and
    /// where the top of each shaft is, x0, y0, x1, y1 in meters, so nothing's built over it
    pub(super) fn undercities(&self, cache: &RiverCache, features: &mut Features) -> (Vec<Object>, Vec<[f32; 4]>, Vec<[f32; 4]>) {
        let (lo, hi) = (self.lo, self.hi);
        let piece = [lo[0], lo[1], hi[0], hi[1]];
        let near = grown(piece, (RAVINE_REACH + RAVINE_CELL).max(2.0 * CAVE_REACH + CAVE_CELL) + 4.0);
        let first = [0, 1].map(|i| ((near[i] - CITY_REACH) / CITY_CELL).floor() as i32);
        let last = [0, 1].map(|i| ((near[i + 2] + CITY_REACH) / CITY_CELL).floor() as i32);
        let (mut objects, mut rects, mut shafts) = (Vec::new(), Vec::new(), Vec::new());
        for cy in first[1]..=last[1] {
            for cx in first[0]..=last[0] {
                // Only the squares whose city could come near, as planning one takes a while
                let (x0, y0) = (cx as f32 * CITY_CELL, cy as f32 * CITY_CELL);
                let could = [x0 + 0.2 * CITY_CELL - CITY_REACH, y0 + 0.2 * CITY_CELL - CITY_REACH, x0 + 0.8 * CITY_CELL + CITY_REACH, y0 + 0.8 * CITY_CELL + CITY_REACH];
                if !overlaps(could, near, 0.0) {
                    continue;
                }
                let Some(city) = self.undercity_in(cx, cy, cache) else {
                    continue;
                };
                if overlaps(city.bounds, piece, 0.0) {
                    objects.push(self.build_undercity(&city, features));
                    shafts.push(city.shaft_rect());
                }
                if overlaps(city.bounds, near, 0.0) {
                    rects.push(city.bounds);
                }
            }
        }
        (objects, rects, shafts)
    }

    /// The nearest fallen city to `near` within a few kilometers, for trying them out
    #[allow(dead_code)]
    pub fn nearest_undercity(&self, near: [f32; 2]) -> Option<Sunken> {
        let cache = RiverCache::default();
        let home = near.map(|v| (v / CITY_CELL).floor() as i32);
        let mut best: Option<(f32, Arc<Undercity>)> = None;
        for ring in 0..7i32 {
            for cy in home[1] - ring..=home[1] + ring {
                for cx in home[0] - ring..=home[0] + ring {
                    if (cx - home[0]).abs().max((cy - home[1]).abs()) != ring {
                        continue;
                    }
                    if let Some(city) = self.undercity_in(cx, cy, &cache) {
                        let distance = (city.at.x - near[0]).hypot(city.at.y - near[1]);
                        if best.as_ref().is_none_or(|(b, _)| distance < *b) {
                            best = Some((distance, city));
                        }
                    }
                }
            }
            if best.as_ref().is_some_and(|(b, _)| *b <= ring as f32 * CITY_CELL) {
                break;
            }
        }
        best.map(|(distance, c)| {
            println!("The nearest fallen city is {distance:.0} m away, its streets {:.1} m down, {} ruins in it", c.rim[0] - c.floor, c.ruins.len());
            c.sunken()
        })
    }

    /// The city in one CITY_CELL square, planned once and kept
    fn undercity_in(&self, cx: i32, cy: i32, cache: &RiverCache) -> Option<Arc<Undercity>> {
        let key = (self.settings.seed, self.settings.height_m, cx, cy);
        let plans = PLANS.get_or_init(|| Mutex::new(HashMap::new()));
        if let Some(plan) = plans.lock().unwrap().get(&key) {
            return plan.clone();
        }
        let plan = self.plan_undercity_in(cx, cy, cache).map(Arc::new);
        plans.lock().unwrap().insert(key, plan.clone());
        plan
    }

    /// The city in one CITY_CELL square: at the first of a few spots in it where the land suits
    /// and nothing else is in the way
    fn plan_undercity_in(&self, cx: i32, cy: i32, cache: &RiverCache) -> Option<Undercity> {
        let r = hash3(cx as u32, cy as u32, self.settings.seed ^ 0x5c17);
        if unit(r) > CITY_CHANCE {
            return None;
        }
        let mut checked = 0;
        for attempt in 0..SPOTS {
            let s = hash3(r, attempt, 0x5c18);
            let p = [(cx as f32 + 0.2 + 0.6 * unit(s)) * CITY_CELL, (cy as f32 + 0.2 + 0.6 * unit(s >> 16)) * CITY_CELL];
            let f = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]][(hash(s) % 4) as usize];
            let Some(city) = self.lay_out_undercity(p, f, hash(s ^ 0x5c19)) else {
                continue;
            };
            if self.clear_for_undercity(&city, cache) {
                return Some(city);
            }
            checked += 1;
            if checked >= CHECKS {
                return None;
            }
        }
        None
    }

    /// Lays out a city with the middle of its shaft at `p`, going on along `f`, if the land suits:
    /// dry and fairly flat round the top of the shaft, and high enough over all of it for the
    /// vaults to be well under the ground
    fn lay_out_undercity(&self, p: [f32; 2], f: [f32; 2], seed: u32) -> Option<Undercity> {
        let settings = &self.settings;
        let at = Frame { x: p[0], y: p[1], f };
        let ground = |u: f32, v: f32| {
            let q = at.point(u, v, 0.0);
            height_at(settings, q[0], q[1])
        };
        let sea = sea_level_m(settings);
        // Round the top of the shaft
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in 0..25 {
            let (u, v) = ((i % 5) as f32 * 0.25 * 2.0 - 1.0, (i / 5) as f32 * 0.25 * 2.0 - 1.0);
            let g = ground(u * (SHAFT + 3.0), v * (SHAFT + 3.0));
            (lo, hi) = (lo.min(g), hi.max(g));
        }
        if lo < sea + 2.5 || hi - lo > 3.0 {
            return None;
        }
        // Out in the open, not deep in a forest or the jungle, so the obelisks round it can be
        // seen from afar
        let forest = smoothstep(-0.15, 0.25, fbm(p[0] / 80.0, p[1] / 80.0, 3, settings.seed.wrapping_add(4)));
        if forest > 0.4 || climate(settings, p[0], p[1]).jungle > 0.5 {
            return None;
        }
        let mut rng = Rng(seed);
        let gate = SHAFT + GATE;
        let city = Lobe { middle: [gate + 0.0, 0.0], half: [rng.range(25.0, 28.0), rng.range(20.0, 23.0)], height: rng.range(11.5, 13.0) };
        let city = Lobe { middle: [gate + city.half[0], 0.0], ..city };
        let chasm = city.middle[0] + city.half[0] + 2.5;
        let temple = Lobe { middle: [0.0, 0.0], half: [rng.range(11.0, 12.5), rng.range(11.0, 12.5)], height: rng.range(11.5, 12.5) };
        let temple = Lobe { middle: [chasm + CHASM + 1.5 + temple.half[0], 0.0], ..temple };
        // Deep enough for the ground to stand well over every vault
        let mut floor = f32::MAX;
        let fw = floor_width();
        for lobe in [city, temple] {
            let middle = (1.0 - DOME) * lobe.height;
            for k in 0..37 {
                let (a, r) = ((k % 12) as f32 * TAU / 12.0, [0.95f32, 0.75, 0.45, 0.0][k / 12]);
                let roof = middle + DOME * lobe.height * (1.0 - r * r).sqrt();
                let g = ground(lobe.middle[0] + a.cos() * lobe.half[0] / fw * r, lobe.middle[1] + a.sin() * lobe.half[1] / fw * r);
                floor = floor.min(g - roof - ROUGH - COVER);
            }
        }
        for k in 0..5 {
            let u = city.middle[0] + city.half[0] - 4.0 + (temple.middle[0] - temple.half[0] - city.middle[0] - city.half[0] + 8.0) * k as f32 / 4.0;
            floor = floor.min(ground(u, 0.0) - 7.0 - ROUGH - COVER);
        }
        // And over the gate
        floor = floor.min(ground(gate, 0.0) - 4.6 - 2.0);
        let floor = snap_down(floor);
        if floor < 6.0 || hi - floor > 30.0 {
            return None;
        }
        let stone = match (rng.next() * 2.99) as usize {
            0 => Palette { wall: (TEMPLE_DARK, MORTAR), trim: (LIMESTONE, MORTAR), paving: [(STONE, MOSS), (MORTAR, GRAVEL)], temple: (OBSIDIAN, TEMPLE_DARK) },
            1 => Palette { wall: (MORTAR, BRICK_STONE), trim: (BRICK_STONE, LIMESTONE), paving: [(GRAVEL, MOSS), (STONE, MORTAR)], temple: (TEMPLE_DARK, OBSIDIAN) },
            _ => Palette { wall: (BRICK_STONE, MOSS), trim: (TEMPLE_DARK, MORTAR), paving: [(MORTAR, MOSS), (ROCK, STONE)], temple: (OBSIDIAN, DARK_STEEL) },
        };
        let canal = gate + city.half[0] * 0.42;
        let canal_reach = city.half[1] * (1.0 - ((canal - city.middle[0]) / city.half[0]).powi(2)).max(0.0).sqrt() - 1.0;
        let plaza = [city.middle[0] + city.half[0] * 0.22, 0.0];
        if city.room_over(around(plaza, [PLAZA; 2])).is_none_or(|h| h < 7.0) {
            return None;
        }
        let mut c = Undercity {
            at,
            floor,
            rim: [lo, hi],
            city,
            temple,
            canal,
            canal_reach,
            chasm,
            chasm_depth: (floor - 1.5).min(6.5),
            plaza,
            stair: Vec::new(),
            exit_point: [0.0; 2],
            exit_out: [0.0; 2],
            exit_at: 0.0,
            exit_up: [0.0; 2],
            ruins: Vec::new(),
            stone,
            bounds: [0.0; 4],
            seed: hash(seed ^ 0x5c1a),
        };
        if !self.lay_stair(&mut c) {
            return None;
        }
        self.lay_ruins(&mut c, &mut rng);
        if c.ruins.len() < 7 {
            return None;
        }
        let side = (city.half[1].max(temple.half[1]) / fw + ROUGH + 2.0).max(CHASM_SIDE + 10.0).max(SHAFT + GLADE);
        let (a, b) = at.rect([-SHAFT - GLADE, temple.middle[0] + temple.half[0] / fw + ROUGH + 2.0], [-side, side]);
        c.bounds = [a[0], a[1], b[0], b[1]];
        Some(c)
    }

    /// Lays the stair round the walls of the shaft, from its foot up, a flight along each side
    /// and a landing in each corner, till it reaches the ground round the top, where it goes out
    /// through the wall. Its foot's on the far side from the gate, and it's well over the gate by
    /// the time it comes round to that side. False if it never reaches the ground.
    fn lay_stair(&self, c: &mut Undercity) -> bool {
        let settings = &self.settings;
        let at = c.at;
        let ground = |u: f32, v: f32| {
            let q = at.point(u, v, 0.0);
            height_at(settings, q[0], q[1])
        };
        let line = SHAFT - STAIR * 0.5;
        let mut z = c.floor;
        let mut corner = 2;
        let mut steps = vec![Step { rect: around(CORNERS[corner].map(|k| k * line), [STAIR * 0.5; 2]), top: z, wall: [0.0; 2] }];
        for _ in 0..12 {
            let (a, b) = (CORNERS[corner].map(|k| k * line), CORNERS[(corner + 1) % 4].map(|k| k * line));
            let along = [(b[0] - a[0]) / (2.0 * line), (b[1] - a[1]) / (2.0 * line)];
            let out = OUTWARD[corner];
            // Each step as deep as fills the side from one landing to the next
            let run = (2.0 * line - STAIR) / FLIGHT as f32;
            for i in 1..=FLIGHT {
                let s = STAIR * 0.5 + run * (i - 1) as f32;
                let (p, q) = ([a[0] + along[0] * s, a[1] + along[1] * s], [a[0] + along[0] * (s + run), a[1] + along[1] * (s + run)]);
                let r = rect([p[0] - out[0] * STAIR * 0.5, p[1] - out[1] * STAIR * 0.5], [q[0] + out[0] * STAIR * 0.5, q[1] + out[1] * STAIR * 0.5]);
                let top = z + RISE * i as f32;
                steps.push(Step { rect: r, top, wall: out });
                // Out through the wall here, onto the ground, once it's come up to it
                let middle = [(p[0] + q[0]) * 0.5, (p[1] + q[1]) * 0.5];
                let outside = ground(middle[0] + out[0] * (STAIR * 0.5 + 1.2), middle[1] + out[1] * (STAIR * 0.5 + 1.2));
                if top >= outside - 0.35 {
                    c.stair = steps;
                    c.exit_point = middle;
                    c.exit_out = out;
                    c.exit_at = top;
                    c.exit_up = along;
                    return true;
                }
            }
            z += RISE * FLIGHT as f32;
            corner = (corner + 1) % 4;
            steps.push(Step { rect: around(CORNERS[corner].map(|k| k * line), [STAIR * 0.5; 2]), top: z, wall: [0.0; 2] });
        }
        false
    }

    /// Lays the city's houses and towers out along both sides of its streets, their doors onto
    /// the street, wherever there's room for them under the vault, clear of the canal, the square
    /// and each other
    fn lay_ruins(&self, c: &mut Undercity, rng: &mut Rng) {
        let gate = SHAFT + GATE;
        let end = c.city.middle[0] + c.city.half[0] - 5.0;
        let edge = |u: f32| c.city.half[1] * (1.0 - ((u - c.city.middle[0]) / c.city.half[0]).powi(2)).max(0.0).sqrt();
        // Each street: along u or v, from where to where, how wide, and how far back the houses
        // stand at least
        let mut streets: Vec<(bool, f32, [f32; 2], f32, f32)> = vec![
            (true, 0.0, [gate + 1.0, c.canal - CANAL - 1.2], STREET, 0.5),
            (true, 0.0, [c.canal + CANAL + 1.2, c.plaza[0] - PLAZA - 0.8], STREET, 1.9),
            (true, 0.0, [c.plaza[0] + PLAZA + 0.8, end], STREET, 0.5),
        ];
        for side in [-1.0f32, 1.0] {
            let reach = edge(c.plaza[0]) - 2.0;
            let (a, b) = (PLAZA + 0.8, reach);
            if b > a + 4.0 {
                streets.push((false, c.plaza[0], if side > 0.0 { [a, b] } else { [-b, -a] }, CROSS, 0.6));
            }
            // And the lanes either side of the main street
            let lane = side * c.city.half[1] * LANE;
            streets.push((true, lane, [gate + 3.0, c.canal - CANAL - 1.2], ALLEY, 0.3));
            streets.push((true, lane, [c.canal + CANAL + 1.2, c.city.middle[0] + c.city.half[0] * 0.8], ALLEY, 0.3));
        }
        let mut taken: Vec<[f32; 4]> = vec![
            around(c.plaza, [PLAZA + 1.0; 2]),
            [c.canal - CANAL - 1.4, -c.city.half[1] - 5.0, c.canal + CANAL + 1.4, c.city.half[1] + 5.0],
            [-SHAFT - 2.0, -SHAFT - 2.0, gate + 2.5, SHAFT + 2.0],
            [c.city.middle[0] + c.city.half[0] - 6.0, -THROAT - 2.0, c.chasm + 20.0, THROAT + 2.0],
        ];
        let ways: Vec<[f32; 4]> = streets.iter().map(|&(along_u, at, span, half, _)| if along_u { [span[0], at - half - 0.3, span[1], at + half + 0.3] } else { [at - half - 0.3, span[0], at + half + 0.3, span[1]] }).collect();
        let mut towers = 0;
        for &(along_u, line, span, half, setback) in &streets {
            for side in [-1.0f32, 1.0] {
                let mut s = span[0] + rng.range(0.2, 1.5);
                while s < span[1] - 3.0 {
                    let tower = towers < 3 && rng.next() < 0.14;
                    let (width, depth) = if tower { (3.4, 3.4) } else if half < STREET - 0.1 { (rng.range(3.4, 5.0), rng.range(3.0, 4.2)) } else { (rng.range(3.6, 5.6), rng.range(3.8, 5.2)) };
                    let back = half + setback + rng.range(0.0, 0.9);
                    let a = [s, line + side * back];
                    let b = [s + width, line + side * (back + depth)];
                    let r = if along_u { rect(a, b) } else { rect([a[1], a[0]], [b[1], b[0]]) };
                    // Facing the street
                    let door = if along_u { [0.0, -side] } else { [-side, 0.0] };
                    let wall = if tower { rng.range(7.0, 9.0) } else { rng.range(2.6, 5.0) };
                    let fits = s + width <= span[1]
                        && c.city.out((r[0] + r[2]) * 0.5, (r[1] + r[3]) * 0.5) < 0.62
                        && c.city.room_over(r).is_some_and(|h| h > wall + 0.6)
                        && !taken.iter().any(|t| overlaps(*t, r, 0.6))
                        && !ways.iter().any(|w| overlaps(*w, r, 0.0));
                    if fits {
                        towers += tower as usize;
                        c.ruins.push(Ruin { rect: r, door, wall, tower, roofed: !tower && rng.next() < 0.4, chest: false, seed: hash(rng.0 ^ c.ruins.len() as u32) });
                        taken.push(r);
                        s += width + rng.range(0.7, 2.2);
                    } else {
                        s += 1.3;
                    }
                }
            }
        }
        // The two biggest houses with some of their roof left keep a chest
        let mut roofed: Vec<usize> = (0..c.ruins.len()).filter(|&i| c.ruins[i].roofed).collect();
        roofed.sort_by(|&a, &b| {
            let size = |r: &Ruin| (r.rect[2] - r.rect[0]) * (r.rect[3] - r.rect[1]);
            size(&c.ruins[b]).total_cmp(&size(&c.ruins[a]))
        });
        for &i in roofed.iter().take(2) {
            c.ruins[i].chest = true;
        }
    }

    /// Whether nothing else is in the way of city `c`: no river near it, no dungeon, town or castle
    /// over it or in it
    fn clear_for_undercity(&self, c: &Undercity, cache: &RiverCache) -> bool {
        let rect = c.bounds;
        let margin = 12.0;
        for river in self.rivers_near(cache, [rect[0] - margin, rect[1] - margin], [rect[2] + margin, rect[3] + margin]) {
            let near_it = river.iter().any(|p| {
                let (dx, dy) = ((rect[0] - p.x).max(p.x - rect[2]).max(0.0), (rect[1] - p.y).max(p.y - rect[3]).max(0.0));
                dx.hypot(dy) < p.half_width + margin
            });
            if near_it {
                return false;
            }
        }
        let reach = DUNGEON_REACH + 2.0;
        let first = [0, 1].map(|i| ((rect[i] - reach) / DUNGEON_CELL).floor() as i32);
        let last = [0, 1].map(|i| ((rect[i + 2] + reach) / DUNGEON_CELL).floor() as i32);
        for cy in first[1]..=last[1] {
            for cx in first[0]..=last[0] {
                if self.dungeon_in(cx, cy, cache).is_some_and(|d| d.world_rects().iter().any(|r| overlaps(*r, rect, 6.0))) {
                    return false;
                }
            }
        }
        self.towns_near(grown(rect, 10.0), cache).is_empty() && self.castles_near(grown(rect, 10.0), cache).is_empty()
    }

    /// Builds city `c`: its shapes, and its lights, chests, skulls and sites in `features`
    fn build_undercity(&self, c: &Undercity, features: &mut Features) -> Object {
        let mut rng = Rng(c.seed);
        // What stands in it goes first, then the floors, then the space cut out of the ground for
        // it, so each wins over what comes after
        let mut b = Builder { at: c.at, solids: Vec::new(), floors: Vec::new(), rooms: Vec::new(), carves: Vec::new() };
        self.shaft(c, &mut b, features, &mut rng);
        self.city_cavern(c, &mut b, features, &mut rng);
        self.canal(c, &mut b, features, &mut rng);
        self.plaza(c, &mut b, features, &mut rng);
        for ruin in &c.ruins {
            self.ruin(c, ruin, &mut b, features);
        }
        self.chasm(c, &mut b, features, &mut rng);
        self.temple(c, &mut b, features, &mut rng);
        // Nothing grows over the shaft, no grass is drawn on it, and water falling in finds the
        // stair and the floor
        let glade = c.at.rect([-SHAFT - GLADE, SHAFT + GLADE], [-SHAFT - GLADE, SHAFT + GLADE]);
        features.footprints.push([glade.0[0], glade.0[1], glade.1[0], glade.1[1], c.rim[1] + 12.0]);
        let inner = c.at.rect([-SHAFT - 0.7, SHAFT + 0.7], [-SHAFT - 0.7, SHAFT + 0.7]);
        features.holes.push([inner.0[0], inner.0[1], inner.1[0], inner.1[1]]);
        features.recuts.push([inner.0[0], inner.0[1], inner.1[0], inner.1[1], c.rim[1] + 4.0]);
        let mut shapes = b.solids;
        shapes.extend(b.floors);
        shapes.extend(b.rooms);
        shapes.extend(b.carves);
        Object::new(shapes, c.seed)
    }

    fn light(&self, features: &mut Features, p: Vec3, color: [f32; 3], reach: f32) {
        if self.inside(p[0], p[1]) {
            features.lights.push(Light { position: p, color, reach });
        }
    }

    fn site(&self, features: &mut Features, kind: &'static str, p: Vec3, facing: [f32; 2]) {
        if self.inside(p[0], p[1]) {
            features.sites.push(Site { kind, position: p, facing });
        }
    }

    fn prop(&self, features: &mut Features, kind: PropKind, p: Vec3, heading: f32) {
        if self.inside(p[0], p[1]) {
            features.props.push(Prop { kind, position: p, heading, luck: 0.0 });
        }
    }

    fn hoard(&self, features: &mut Features, p: Vec3, heading: f32, luck: f32) {
        if self.inside(p[0], p[1]) {
            chest(features, p, heading, luck);
        }
    }

    /// The shaft down from the land: its stair, its walls lined with stone, the broken wall round
    /// its top and the obelisks at its corners, braziers by the way down, vines hanging down its
    /// walls, and the gate at its foot into the city
    fn shaft(&self, c: &Undercity, b: &mut Builder, features: &mut Features, rng: &mut Rng) {
        let settings = &self.settings;
        let at = c.at;
        let ground = |u: f32, v: f32| {
            let q = at.point(u, v, 0.0);
            height_at(settings, q[0], q[1])
        };
        let f = c.floor;
        let gate = SHAFT + GATE;
        // The way out at the top, along the side the stair leaves by
        let exit = c.exit_rect(SHAFT + 2.6);
        let top = c.exit_at;
        // The stair, each step a slab out from the wall, some with their open edge broken away,
        // and its landings, a stump of a post on the corner of some
        for step in &c.stair {
            let mut r = step.rect;
            let w = step.wall;
            if w != [0.0; 2] && rng.next() < 0.18 {
                // Broken back towards the wall, leaving enough to walk on
                let keep = rng.range(1.0, 1.25);
                let inner = SHAFT - keep;
                if w[0] > 0.5 {
                    r[0] = r[0].max(inner);
                } else if w[0] < -0.5 {
                    r[2] = r[2].min(-inner);
                } else if w[1] > 0.5 {
                    r[1] = r[1].max(inner);
                } else {
                    r[3] = r[3].min(-inner);
                }
            }
            b.solids.push(b.block(r, [step.top - 0.55, step.top], c.stone.trim, Pattern::Plain));
            if w == [0.0; 2] && rng.next() < 0.6 {
                // On the corner out over the shaft
                let m = [(r[0] + r[2]) * 0.5, (r[1] + r[3]) * 0.5];
                let post = [m[0] - m[0].signum() * (STAIR * 0.5 - 0.15), m[1] - m[1].signum() * (STAIR * 0.5 - 0.15)];
                b.solids.push(b.block(around(post, [0.14; 2]), [step.top, step.top + rng.range(0.3, 0.9)], c.stone.wall, Pattern::Plain));
            }
        }
        // Out through the wall at the top onto the ground
        b.floors.push(b.block(exit, [top - 0.6, top], c.stone.paving[0], Pattern::Plain));
        b.rooms.push(b.room(c.exit_rect(SHAFT + 3.2), [top, top + 3.2]));
        // The floor of the shaft, the space inside it right up through the ground, and its walls
        b.floors.push(b.block(around([0.0, 0.0], [SHAFT; 2]), [f - 0.6, f], c.stone.paving[1], Pattern::Plain));
        b.rooms.push(b.room(around([0.0, 0.0], [SHAFT; 2]), [f, c.rim[1] + 8.0]));
        for out in OUTWARD.iter() {
            let across = [-out[1], out[0]];
            let mut s = -SHAFT - 0.6;
            while s < SHAFT + 0.6 - 1e-3 {
                let e = (s + 2.0).min(SHAFT + 0.6);
                let a = [out[0] * SHAFT + across[0] * s, out[1] * SHAFT + across[1] * s];
                let z = [out[0] * (SHAFT + 0.6) + across[0] * e, out[1] * (SHAFT + 0.6) + across[1] * e];
                let r = rect(a, z);
                let mut lining_top = [s, e, (s + e) * 0.5].iter().map(|&k| ground(out[0] * (SHAFT + 0.8) + across[0] * k, out[1] * (SHAFT + 0.8) + across[1] * k)).fold(f32::MAX, f32::min) - 0.45;
                if overlaps(r, exit, 0.0) {
                    lining_top = lining_top.min(top - 0.65);
                }
                // Over the gate, only above it
                let bottom = if out[0] > 0.5 && s < 2.7 && e > -2.7 { f + 4.6 } else { f - 0.6 };
                if lining_top > bottom + 0.3 {
                    b.solids.push(b.block(r, [bottom, lining_top], c.stone.wall, Pattern::Bricks));
                }
                s = e;
            }
        }
        // The broken wall round the top, with gaps in it, and none in the way out
        for out in OUTWARD {
            let across = [-out[1], out[0]];
            let mut s = -SHAFT - 1.2;
            while s < SHAFT + 1.2 {
                let piece = rng.range(0.9, 1.8);
                let e = (s + piece).min(SHAFT + 1.2);
                let a = [out[0] * (SHAFT + 0.65) + across[0] * s, out[1] * (SHAFT + 0.65) + across[1] * s];
                let z = [out[0] * (SHAFT + 1.25) + across[0] * e, out[1] * (SHAFT + 1.25) + across[1] * e];
                let r = rect(a, z);
                if rng.next() > 0.25 && !overlaps(r, grown(exit, 0.6), 0.0) {
                    let g = [ground(a[0], a[1]), ground(z[0], z[1]), ground((a[0] + z[0]) * 0.5, (a[1] + z[1]) * 0.5)];
                    let (low, high) = (g.iter().copied().fold(f32::MAX, f32::min), g.iter().copied().fold(f32::MIN, f32::max));
                    b.solids.push(b.block(r, [low - 0.6, high + rng.range(0.25, 1.7)], c.stone.wall, Pattern::Bricks));
                }
                s = e;
            }
        }
        // Tall obelisks at the corners, leaning ones fallen against the wall, void burning on the
        // tops of those still standing: what's seen from afar
        let mut standing = 0;
        for (k, corner) in CORNERS.iter().enumerate() {
            let q = corner.map(|v| v * (SHAFT + 2.7));
            let g = ground(q[0], q[1]);
            let broken = k == (c.seed % 4) as usize || (standing >= 2 && rng.next() < 0.5);
            let high = rng.range(8.0, 11.0);
            pillar(&mut b.solids, &at, q, g - 0.3, g + high, 0.5, c.stone.wall, broken, rng);
            if !broken {
                standing += 1;
                b.solids.push(Shape::sphere(at.point(q[0], q[1], g + high + 0.55), 0.38, GLOW_VOID, AIR, false).placed(Placement::Clear));
                for s in 0..4 {
                    let a = s as f32 * FRAC_PI_2 + 0.4;
                    let tip = at.point(q[0] + a.cos() * 0.45, q[1] + a.sin() * 0.45, g + high + 1.1);
                    b.solids.push(Shape::capsule(at.point(q[0], q[1], g + high + 0.2), tip, 0.09, 0.03, c.stone.trim.0, AIR, false).placed(Placement::Clear));
                }
                self.light(features, at.point(q[0], q[1], g + high + 0.6), VOID, 10.0);
            }
        }
        // Braziers either side of the way down
        let m = c.exit_point;
        let out = c.exit_out;
        let along = [-out[1], out[0]];
        for side in [-1.0f32, 1.0] {
            let q = [m[0] + out[0] * (STAIR * 0.5 + 2.6) + along[0] * side * 2.0, m[1] + out[1] * (STAIR * 0.5 + 2.6) + along[1] * side * 2.0];
            self.brazier(c, b, features, q, ground(q[0], q[1]));
        }
        // Vines and roots hanging down the walls from the top
        for _ in 0..12 {
            let side = (rng.next() * 3.99) as usize;
            let out = OUTWARD[side];
            let across = [-out[1], out[0]];
            let s = rng.range(-SHAFT + 0.3, SHAFT - 0.3);
            let q = [out[0] * (SHAFT - 0.08) + across[0] * s, out[1] * (SHAFT - 0.08) + across[1] * s];
            let g = ground(q[0] + out[0], q[1] + out[1]);
            let long = rng.range(2.0, 7.0).min(g - f - 1.0);
            let wavy = rng.range(-0.2, 0.2);
            let mid = [q[0] - out[0] * 0.15 + across[0] * wavy, q[1] - out[1] * 0.15 + across[1] * wavy];
            b.solids.push(Shape::capsule(at.point(q[0], q[1], g - 0.1), at.point(mid[0], mid[1], g - long * 0.5), 0.06, 0.05, VINE, AIR, false).placed(Placement::Clear));
            b.solids.push(Shape::capsule(at.point(mid[0], mid[1], g - long * 0.5), at.point(q[0], q[1], g - long), 0.05, 0.03, VINE, AIR, false).placed(Placement::Clear));
        }
        // A void-lit crack down the far wall
        let crack = rng.range(-2.0, 2.0);
        b.solids.push(Shape::capsule(at.point(-SHAFT - 0.02, crack, f + 1.0), at.point(-SHAFT - 0.02, crack + 0.6, f + 6.0), 0.07, 0.04, GLOW_VOID, AIR, false).placed(Placement::Clear));
        // The entrance itself, on the ground just outside the way out
        let e = [m[0] + out[0] * (STAIR * 0.5 + 2.8), m[1] + out[1] * (STAIR * 0.5 + 2.8)];
        self.site(features, "undercity", at.point(e[0], e[1], ground(e[0], e[1])), at.dir(out));

        // The gate at its foot: a vaulted passage lined with stone, a broken portcullis hanging
        // over it, and a brazier inside
        b.rooms.push(b.room([SHAFT - 0.05, -2.0, gate + 0.8, 2.0], [f, f + 4.0]));
        b.floors.push(b.block([SHAFT - 0.05, -2.0, gate + 0.6, 2.0], [f - 0.6, f], c.stone.paving[0], Pattern::Plain));
        for side in [-1.0f32, 1.0] {
            b.solids.push(b.block(rect([SHAFT + 0.6, side * 2.0], [gate, side * 2.6]), [f - 0.6, f + 4.6], c.stone.wall, Pattern::Bricks));
        }
        b.solids.push(b.block([SHAFT + 0.6, -2.6, gate, 2.6], [f + 4.0, f + 4.6], c.stone.trim, Pattern::Plain));
        let mut v = -1.8;
        while v < 1.85 {
            if rng.next() < 0.75 {
                let low = f + rng.range(2.4, 3.3);
                b.solids.push(b.block(around([gate - 0.4, v], [0.04, 0.04]), [low, f + 4.05], (DARK_STEEL, AIR), Pattern::Plain));
            }
            v += 0.45;
        }
        b.solids.push(b.block([gate - 0.46, -1.9, gate - 0.34, 1.9], [f + 3.6, f + 3.7], (DARK_STEEL, AIR), Pattern::Plain));
        self.brazier(c, b, features, [SHAFT + 1.2, -1.3], f);
        // Bones at the foot of the stair, from those who came down
        for _ in 0..3 {
            let q = [rng.range(-SHAFT + 2.2, SHAFT - 2.2), rng.range(-SHAFT + 2.2, SHAFT - 2.2)];
            bones(&mut b.solids, &at, q, f, rng);
        }
        self.prop(features, PropKind::Skull, at.point(SHAFT - 1.5, 1.2, f), rng.next() * TAU);
        self.site(features, "undercity gate", at.point(gate + 3.0, 0.0, f), at.dir([-1.0, 0.0]));
    }

    /// A brazier of bronze on a short stone post at `q` in the city's frame, the post's foot at
    /// `z`, a flame in its bowl lighting what's round it
    fn brazier(&self, c: &Undercity, b: &mut Builder, features: &mut Features, q: [f32; 2], z: f32) {
        let at = c.at;
        b.solids.push(b.block(around(q, [0.22; 2]), [z - 0.4, z + 0.85], c.stone.trim, Pattern::Plain));
        b.solids.push(Shape::sphere(at.point(q[0], q[1], z + 1.12), 0.2, FLAME, AIR, false).placed(Placement::Clear));
        b.solids.push(Shape::ellipsoid(at.point(q[0], q[1], z + 0.98), [0.42, 0.42, 0.16], BRONZE, AIR).placed(Placement::Clear));
        self.light(features, at.point(q[0], q[1], z + 1.55), FIRE, 10.0);
    }

    /// Glowing fungus at `q` on the floor at `z`: a few stems with caps glowing green, lighting a
    /// little round them
    fn fungus(&self, c: &Undercity, b: &mut Builder, features: &mut Features, q: [f32; 2], z: f32, rng: &mut Rng) {
        let at = c.at;
        for _ in 0..(3 + (rng.next() * 3.0) as usize) {
            let (a, d) = (rng.next() * TAU, rng.range(0.0, 0.6));
            let p = [q[0] + a.cos() * d, q[1] + a.sin() * d];
            let tall = rng.range(0.25, 0.85);
            let lean = [rng.range(-0.12, 0.12), rng.range(-0.12, 0.12)];
            let top = at.point(p[0] + lean[0], p[1] + lean[1], z + tall);
            b.solids.push(Shape::capsule(at.point(p[0], p[1], z - 0.05), top, 0.06, 0.045, MUSHROOM_STEM, AIR, false).placed(Placement::Clear));
            let r = rng.range(0.16, 0.38);
            b.solids.push(Shape::ellipsoid([top[0], top[1], top[2] + 0.03], [r, r, r * 0.35], GLOW_POISON, AIR).placed(Placement::Clear));
        }
        self.light(features, at.point(q[0], q[1], z + 0.9), FUNGUS, 7.0);
    }

    /// A tangle of roots hanging down from the vault at `q`, where its roof is `roof` over the
    /// floor at `z`, and vines among them
    fn roots(&self, c: &Undercity, b: &mut Builder, q: [f32; 2], z: f32, roof: f32, rng: &mut Rng) {
        let at = c.at;
        let top = z + roof + ROUGH + 0.4;
        let low = (z + roof - rng.range(1.5, 4.5)).max(z + 2.6);
        if low > top - 1.0 {
            return;
        }
        let tip = [q[0] + rng.range(-0.4, 0.4), q[1] + rng.range(-0.4, 0.4)];
        b.solids.push(Shape::capsule(at.point(q[0], q[1], top), at.point(tip[0], tip[1], low), rng.range(0.14, 0.22), 0.04, BARK_DARK, AIR, false).placed(Placement::Clear));
        for _ in 0..2 {
            let from = (top + low) * 0.5 + rng.range(-0.5, 0.5);
            let a = rng.next() * TAU;
            let end = [tip[0] + a.cos() * rng.range(0.4, 1.0), tip[1] + a.sin() * rng.range(0.4, 1.0)];
            b.solids.push(Shape::capsule(at.point(q[0], q[1], from), at.point(end[0], end[1], (low + rng.range(-0.8, 0.3)).max(z + 2.4)), 0.07, 0.02, DEAD_WOOD, AIR, false).placed(Placement::Clear));
        }
        for _ in 0..2 {
            let a = rng.next() * TAU;
            let p = [q[0] + a.cos() * 0.3, q[1] + a.sin() * 0.3];
            b.solids.push(Shape::capsule(at.point(p[0], p[1], top - 0.5), at.point(p[0], p[1], (low - rng.range(0.2, 1.2)).max(z + 2.3)), 0.045, 0.03, VINE, AIR, false).placed(Placement::Clear));
        }
    }

    /// The great cavern the city's in, its vault rough and bulging, its floor laid with streets,
    /// roots and vines hanging down from it, fungus round its walls, bones and rubble about
    fn city_cavern(&self, c: &Undercity, b: &mut Builder, features: &mut Features, rng: &mut Rng) {
        let at = c.at;
        let f = c.floor;
        let l = c.city;
        self.vault(c, b, l, rng);
        let gate = SHAFT + GATE;
        // The main street, out from the gate across the canal and through the square to the way
        // through to the temple, and the cross street through the square
        let end = l.middle[0] + l.half[0];
        let pieces = [[gate - 0.2, c.canal - CANAL - 0.5], [c.canal + CANAL + 0.5, c.plaza[0] - PLAZA], [c.plaza[0] + PLAZA, end]];
        for [a, z] in pieces {
            pave(b, [a, -STREET, z, STREET], f, 2.0, c.stone.paving);
        }
        let edge = |u: f32| l.half[1] * (1.0 - ((u - l.middle[0]) / l.half[0]).powi(2)).max(0.0).sqrt();
        let reach = edge(c.plaza[0]) - 1.0;
        for side in [-1.0f32, 1.0] {
            let r = rect([c.plaza[0] - CROSS, side * PLAZA], [c.plaza[0] + CROSS, side * reach]);
            pave(b, r, f, 1.6, c.stone.paving);
            // The lanes, cobbled, a slab bridging the canal for each
            let lane = side * l.half[1] * LANE;
            for [a, z] in [[gate + 3.0, c.canal - CANAL - 0.5], [c.canal + CANAL + 0.5, l.middle[0] + l.half[0] * 0.8]] {
                pave(b, [a, lane - ALLEY, z, lane + ALLEY], f, 1.2, [(GRAVEL, MOSS), c.stone.paving[1]]);
            }
            if lane.abs() < c.canal_reach - ALLEY - 0.5 {
                b.solids.push(b.block([c.canal - CANAL - 0.5, lane - ALLEY, c.canal + CANAL + 0.5, lane + ALLEY], [f - 0.4, f], c.stone.trim, Pattern::Plain));
            }
        }
        // A colonnade along the street between the canal and the square, many of its pillars
        // fallen
        let mut u = c.canal + CANAL + 2.4;
        while u < c.plaza[0] - PLAZA - 1.0 {
            for side in [-1.0f32, 1.0] {
                let q = [u, side * (STREET + 0.85)];
                let broken = rng.next() < 0.45;
                pillar(&mut b.solids, &at, q, f, f + 5.2, 0.34, c.stone.trim, broken, rng);
                if broken && rng.next() < 0.5 {
                    // Lying where it fell, beside the street
                    let lie = [u + rng.range(0.6, 1.4), side * (STREET + rng.range(1.6, 2.4))];
                    let to = [lie[0] + rng.range(2.5, 3.5), lie[1] + side * rng.range(-0.3, 0.6)];
                    b.solids.push(Shape::capsule(at.point(lie[0], lie[1], f + 0.3), at.point(to[0], to[1], f + 0.3), 0.32, 0.3, c.stone.trim.0, c.stone.trim.1, false).placed(Placement::Clear).patterned(Pattern::Carved));
                }
            }
            u += 3.6;
        }
        // Roots and vines hanging through cracks in the vault
        for _ in 0..16 {
            let (a, r) = (rng.next() * TAU, rng.next().sqrt() * 0.8);
            let q = [l.middle[0] + a.cos() * l.half[0] * r, l.middle[1] + a.sin() * l.half[1] * r];
            if (q[0] - c.plaza[0]).abs() < 4.0 && q[1].abs() < 4.0 {
                continue;
            }
            if let Some(roof) = l.headroom(q[0], q[1]) {
                self.roots(c, b, q, f, roof, rng);
            }
        }
        // More of them near the gate, where the vault's cracked worst under the shaft
        for _ in 0..4 {
            let q = [gate + rng.range(1.0, 6.0), rng.range(-6.0, 6.0)];
            if let Some(roof) = l.headroom(q[0], q[1]) {
                self.roots(c, b, q, f, roof, rng);
            }
        }
        // Fungus glowing round the walls and in the corners of ruins
        let mut lit = 0;
        for k in 0..40 {
            if lit >= 12 {
                break;
            }
            let a = rng.next() * TAU;
            let r = rng.range(0.72, 0.9);
            let q = [l.middle[0] + a.cos() * l.half[0] * r, l.middle[1] + a.sin() * l.half[1] * r];
            let clear = q[1].abs() > STREET + 0.6 && (q[0] - c.canal).abs() > CANAL + 1.0 && !c.ruins.iter().any(|ruin| overlaps(ruin.rect, around(q, [0.7; 2]), 0.0));
            if clear && (k % 2 == 0 || q[0] > c.canal) {
                self.fungus(c, b, features, q, f, rng);
                lit += 1;
            }
        }
        // Rubble and bones all about, and skulls on the streets
        for _ in 0..26 {
            let (a, r) = (rng.next() * TAU, rng.next().sqrt() * 0.85);
            let q = [l.middle[0] + a.cos() * l.half[0] * r, l.middle[1] + a.sin() * l.half[1] * r];
            if (q[0] - c.canal).abs() < CANAL + 0.6 {
                continue;
            }
            if rng.next() < 0.5 {
                bones(&mut b.solids, &at, q, f, rng);
            } else {
                let size = rng.range(0.18, 0.45);
                b.solids.push(b.block(around(q, [size, size * rng.range(0.7, 1.3)]), [f - 0.1, f + size * rng.range(0.6, 1.1)], c.stone.wall, Pattern::Plain));
            }
        }
        for _ in 0..7 {
            let u = rng.range(gate + 2.0, end - 2.0);
            if (u - c.canal).abs() < CANAL + 0.6 || (u - c.plaza[0]).abs() < PLAZA {
                continue;
            }
            self.prop(features, PropKind::Skull, at.point(u, rng.range(-STREET + 0.4, STREET - 0.4), f), rng.next() * TAU);
        }
        self.site(features, "undercity street", at.point((gate + c.canal) * 0.5 - 1.0, 0.0, f), at.dir([-1.0, 0.0]));
        let cross = (PLAZA + reach) * 0.5;
        if reach > PLAZA + 3.0 {
            let side = if c.seed % 2 == 0 { 1.0 } else { -1.0 };
            self.site(features, "undercity street", at.point(c.plaza[0], side * cross, f), at.dir([0.0, -side]));
        }

        // The way through to the temple, past the end of the city's vault
        let (from, to) = (l.middle[0] + l.half[0] - 5.0, c.temple.middle[0] - c.temple.half[0] + 5.0);
        b.rooms.push(b.room([from, -THROAT, to, THROAT], [f, f + 6.0]));
        let tube = Shape::capsule(at.point(from - 1.0, 0.0, f + 2.8), at.point(to + 1.0, 0.0, f + 2.8), THROAT + 1.2, THROAT + 1.2, AIR, AIR, false).placed(Placement::Carve);
        b.carves.push(tube.roughened(0.7, f));
        pave(b, [end, -STREET, c.chasm - CHASM - 0.2, STREET], f, 2.0, c.stone.paving);
    }

    /// Cuts lobe `l` out of the ground: a great rough hollow over a flat floor, with bulges off its
    /// sides, and space kept clear in its middle
    fn vault(&self, c: &Undercity, b: &mut Builder, l: Lobe, rng: &mut Rng) {
        let at = c.at;
        let f = c.floor;
        let fw = floor_width();
        let (middle, dome) = ((1.0 - DOME) * l.height, DOME * l.height);
        let center = at.point(l.middle[0], l.middle[1], f + middle);
        let radii = b.radii([l.half[0] / fw, l.half[1] / fw, dome]);
        b.carves.push(Shape::ellipsoid(center, radii, AIR, AIR).placed(Placement::Carve).roughened(ROUGH, f));
        for k in 0..3 {
            let a = FRAC_PI_2 + k as f32 * 2.0 * std::f32::consts::PI / 3.0 + rng.range(-0.3, 0.3);
            let (r, high) = (rng.range(5.0, 7.0), rng.range(3.5, 5.0));
            let q = [l.middle[0] + a.cos() * l.half[0] * 0.78, l.middle[1] + a.sin() * l.half[1] * 0.78];
            b.carves.push(Shape::ellipsoid(at.point(q[0], q[1], f + high * 0.4), b.radii([r, r, high]), AIR, AIR).placed(Placement::Carve).roughened(0.6, f));
        }
        // Clear of anything left of the ground, so most of the space inside is quickly seen to be
        // empty
        let core = around(l.middle, [l.half[0] * 0.7, l.half[1] * 0.7]);
        if let Some(h) = l.room_over(core) {
            b.rooms.push(b.room(core, [f, f + h - 0.3]));
            let high = around(l.middle, [l.half[0] * 0.45, l.half[1] * 0.45]);
            if let Some(h2) = l.room_over(high) {
                b.rooms.push(b.room(high, [f + h - 0.3, f + h2 - 0.3]));
            }
        }
    }

    /// The dry canal across the city: walled in stone, a bridge carrying the street over it, steps
    /// down into it, and bones and rubble along its bottom
    fn canal(&self, c: &Undercity, b: &mut Builder, features: &mut Features, rng: &mut Rng) {
        let at = c.at;
        let f = c.floor;
        let (u, reach) = (c.canal, c.canal_reach);
        let bottom = f - CANAL_DEPTH;
        b.rooms.push(b.room([u - CANAL, -reach, u + CANAL, reach], [bottom, f + 0.3]));
        b.floors.push(b.block([u - CANAL, -reach, u + CANAL, reach], [bottom - 0.5, bottom], (GRAVEL, MOSS), Pattern::Plain));
        for side in [-1.0f32, 1.0] {
            let r = rect([u + side * CANAL, -reach], [u + side * (CANAL + 0.5), reach]);
            b.floors.push(b.block(r, [bottom - 0.5, f - 0.15], c.stone.wall, Pattern::Bricks));
            b.floors.push(b.block(r, [f - 0.15, f + 0.05], c.stone.trim, Pattern::Plain));
            let end = rect([u - CANAL - 0.5, side * reach], [u + CANAL + 0.5, side * (reach + 0.6)]);
            b.solids.push(b.block(end, [bottom - 0.5, f + 0.3], c.stone.wall, Pattern::Bricks));
        }
        // The bridge, with low walls along it
        b.solids.push(b.block([u - CANAL - 0.5, -STREET - 0.3, u + CANAL + 0.5, STREET + 0.3], [f - 0.55, f], c.stone.paving[0], Pattern::Plain));
        b.solids.push(b.block([u - CANAL - 0.4, -STREET - 0.3, u + CANAL + 0.4, STREET + 0.3], [f - 1.1, f - 0.55], c.stone.wall, Pattern::Bricks));
        for side in [-1.0f32, 1.0] {
            let r = rect([u - CANAL - 0.5, side * STREET], [u + CANAL + 0.5, side * (STREET + 0.3)]);
            b.solids.push(b.block(r, [f, f + rng.range(0.45, 0.75)], c.stone.trim, Pattern::Plain));
        }
        // Steps down from either bank, at either end
        let rise = CANAL_DEPTH / 7.0;
        for (side, v) in [(-1.0f32, reach * 0.55), (1.0, -reach * 0.55)] {
            for i in 0..6 {
                let a = u + side * (CANAL - 0.42 * i as f32);
                let r = rect([a, v - 0.7], [a - side * 0.42, v + 0.7]);
                b.solids.push(b.block(r, [bottom - 0.3, f - rise * (i + 1) as f32], c.stone.trim, Pattern::Plain));
            }
        }
        for _ in 0..6 {
            let q = [u + rng.range(-CANAL + 0.4, CANAL - 0.4), rng.range(-reach + 0.8, reach - 0.8)];
            if q[1].abs() < STREET + 0.6 {
                continue;
            }
            if rng.next() < 0.6 {
                bones(&mut b.solids, &at, q, bottom, rng);
            } else {
                b.solids.push(b.block(around(q, [0.3, 0.25]), [bottom - 0.1, bottom + 0.35], c.stone.wall, Pattern::Plain));
            }
        }
        // Braziers at the ends of the bridge over it
        for side in [-1.0f32, 1.0] {
            self.brazier(c, b, features, [u + side * (CANAL + 1.1), -side * (STREET + 0.7)], f);
        }
        let side = if c.seed % 3 == 0 { -1.0 } else { 1.0 };
        // The dead keeping it on its far bank, where they can come at whoever crosses, rather than
        // down in it where they'd be stuck
        self.site(features, "undercity canal", at.point(u + CANAL + 2.6, side * reach * 0.6, f), at.dir([-1.0, 0.0]));
        self.prop(features, PropKind::Skull, at.point(u + 0.6, side * reach * 0.3, bottom), rng.next() * TAU);
    }

    /// The square: paved, a dry basin in its middle round a cracked obelisk with void glowing in
    /// its runes, its top fallen into the basin, braziers at its corners, two towers over it,
    /// and a great cluster of crystals hanging from the vault over it, lighting the whole city
    /// dimly
    fn plaza(&self, c: &Undercity, b: &mut Builder, features: &mut Features, rng: &mut Rng) {
        let at = c.at;
        let f = c.floor;
        let p = c.plaza;
        // Paved in big stones of two kinds
        let tile = 1.5;
        let n = (2.0 * PLAZA / tile).round() as usize;
        for i in 0..n {
            for j in 0..n {
                let lo = [p[0] - PLAZA + i as f32 * tile, p[1] - PLAZA + j as f32 * tile];
                let stone = if (i + j) % 2 == 0 { c.stone.trim } else { c.stone.paving[1] };
                b.floors.push(b.block([lo[0], lo[1], lo[0] + tile, lo[1] + tile], [f - 0.5, f], stone, Pattern::Plain));
            }
        }
        // The basin, long dry, gravel in its bottom
        let basin = 2.6;
        for (u, v) in [(-1.0f32, 0.0f32), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
            let r = if u != 0.0 { rect([p[0] + u * basin, p[1] - basin - 0.45], [p[0] + u * (basin + 0.45), p[1] + basin + 0.45]) } else { rect([p[0] - basin, p[1] + v * basin], [p[0] + basin, p[1] + v * (basin + 0.45)]) };
            b.solids.push(b.block(r, [f - 0.2, f + 0.65], c.stone.trim, Pattern::Plain));
        }
        b.solids.push(b.block(around(p, [basin; 2]), [f - 0.4, f - 0.25], (GRAVEL, MOSS), Pattern::Plain));
        // The obelisk, its runes glowing with the void, its top broken off and lying across the
        // basin
        let high = rng.range(3.4, 4.4);
        b.solids.push(b.block(around(p, [0.6; 2]), [f - 0.3, f + 0.85], c.stone.trim, Pattern::Plain));
        for k in 0..4 {
            let a = k as f32 * FRAC_PI_2;
            let (du, dv) = (a.cos() * 0.4, a.sin() * 0.4);
            b.solids.push(Shape::capsule(at.point(p[0] + du, p[1] + dv, f + 1.4), at.point(p[0] + du * 0.85, p[1] + dv * 0.85, f + 2.9), 0.06, 0.05, GLOW_VOID, AIR, false).placed(Placement::Clear));
        }
        b.solids.push(Shape::capsule(at.point(p[0], p[1], f + 0.8), at.point(p[0], p[1], f + 0.85 + high), 0.42, 0.3, c.stone.wall.0, c.stone.wall.1, false).placed(Placement::Clear).patterned(Pattern::Carved));
        let a = rng.next() * TAU;
        let (q, r) = ([p[0] + a.cos() * 1.0, p[1] + a.sin() * 1.0], [p[0] + a.cos() * 3.2, p[1] + a.sin() * 3.2]);
        b.solids.push(Shape::capsule(at.point(q[0], q[1], f + 0.75), at.point(r[0], r[1], f + 0.55), 0.3, 0.2, c.stone.wall.0, c.stone.wall.1, false).placed(Placement::Clear).patterned(Pattern::Carved));
        // The basin's bottom, below the paving, after all that stands in it so it doesn't cut it
        b.solids.push(b.room(around(p, [basin; 2]), [f - 0.25, f + 0.05]));
        self.light(features, at.point(p[0], p[1], f + 2.4), VOID, 10.0);
        // Braziers at its corners
        for side in [-1.0f32, 1.0] {
            for end in [-1.0f32, 1.0] {
                self.brazier(c, b, features, [p[0] + end * (PLAZA - 0.8), p[1] + side * (PLAZA - 0.8)], f);
            }
        }
        // Two towers over the square, at its far corners, broken off at the top
        for side in [-1.0f32, 1.0] {
            let q = [p[0] + PLAZA + 1.2, p[1] + side * (PLAZA + 1.2)];
            let r = around([q[0] + 1.6, q[1] + side * 1.6], [1.7; 2]);
            if let Some(room) = c.city.room_over(r) {
                let wall = room.min(rng.range(7.5, 9.5)) - 0.4;
                if wall > 4.0 && !c.ruins.iter().any(|ruin| overlaps(ruin.rect, r, 0.4)) {
                    let door = [-1.0, 0.0];
                    self.ruin(c, &Ruin { rect: r, door, wall, tower: true, roofed: false, chest: false, seed: hash(c.seed ^ side as i32 as u32) }, b, features);
                }
            }
        }
        // The crystals hanging from the vault
        if let Some(roof) = c.city.headroom(p[0], p[1]) {
            let top = f + roof + ROUGH + 0.3;
            let low = f + roof - 2.6;
            b.solids.push(Shape::capsule(at.point(p[0], p[1], top), at.point(p[0], p[1], low), 1.0, 0.15, CRYSTAL, AIR, false).placed(Placement::Clear));
            for _ in 0..6 {
                let a = rng.next() * TAU;
                let tip = at.point(p[0] + a.cos() * 1.6, p[1] + a.sin() * 1.6, f + roof - rng.range(0.8, 2.0));
                b.solids.push(Shape::capsule(at.point(p[0], p[1], top - 0.4), tip, 0.45, 0.07, CRYSTAL, AIR, false).placed(Placement::Clear));
            }
            // Low enough over the square to light it, dimly, and the streets round it
            self.light(features, at.point(p[0], p[1], (f + 6.0).min(low - 0.8)), [1.4, 1.6, 2.2], 30.0);
        }
        for _ in 0..4 {
            let q = [p[0] + rng.range(-PLAZA + 0.6, PLAZA - 0.6), p[1] + rng.range(-PLAZA + 0.6, PLAZA - 0.6)];
            if (q[0] - p[0]).abs() > basin + 1.0 || (q[1] - p[1]).abs() > basin + 1.0 {
                bones(&mut b.solids, &at, q, f, rng);
            }
        }
        self.prop(features, PropKind::Skull, at.point(p[0] - basin - 1.2, p[1] + 1.5, f), rng.next() * TAU);
        self.site(features, "undercity plaza", at.point(p[0] - basin - 2.8, p[1], f), at.dir([-1.0, 0.0]));
    }

    /// A house or tower of the city, broken: its walls standing to different heights with a door
    /// onto the street, some of its roof left on it or fallen in, rubble round it, maybe a chest
    /// left inside
    fn ruin(&self, c: &Undercity, h: &Ruin, b: &mut Builder, features: &mut Features) {
        let at = c.at;
        let f = c.floor;
        let mut rng = Rng(h.seed);
        let [u0, v0, u1, v1] = h.rect;
        let t = if h.tower { 0.55 } else { 0.42 };
        let low = if h.tower { h.wall * 0.7 } else { h.wall * 0.4 };
        // Each wall in pieces, broken off at different heights, the door in the middle of the
        // one facing the street
        let walls = [([u0, v0], [u1, v0 + t], [0.0f32, -1.0f32]), ([u0, v1 - t], [u1, v1], [0.0, 1.0]), ([u0, v0], [u0 + t, v1], [-1.0, 0.0]), ([u1 - t, v0], [u1, v1], [1.0, 0.0])];
        for (a, z, out) in walls {
            let along_u = out[0] == 0.0;
            let (start, end) = if along_u { (a[0], z[0]) } else { (a[1], z[1]) };
            let middle = (start + end) * 0.5;
            let piece = |s: f32, e: f32| if along_u { rect([s, a[1]], [e, z[1]]) } else { rect([a[0], s], [z[0], e]) };
            let spans = if out == h.door {
                // The doorway, a lintel over some
                if h.wall > 3.2 && rng.next() < 0.5 {
                    b.solids.push(b.block(piece(middle - 0.75, middle + 0.75), [f + 2.3, f + 2.6], c.stone.trim, Pattern::Plain));
                }
                vec![[start, middle - 0.65], [middle + 0.65, end]]
            } else {
                vec![[start, end]]
            };
            for [s0, s1] in spans {
                let mut s = s0;
                while s < s1 - 1e-3 {
                    let e = (s + rng.range(0.8, 1.7)).min(s1);
                    b.solids.push(b.block(piece(s, e), [f - 0.3, f + rng.range(low, h.wall)], c.stone.wall, Pattern::Bricks));
                    s = e;
                }
            }
        }
        // Some of its roof still on, the rest fallen in
        if h.roofed {
            let keep = rng.range(0.45, 0.7);
            let r = if rng.next() < 0.5 { [u0, v0, u0 + (u1 - u0) * keep, v1] } else { [u0, v0, u1, v0 + (v1 - v0) * keep] };
            b.solids.push(b.block(r, [f + low.max(2.6) - 0.05, f + low.max(2.6) + 0.3], c.stone.trim, Pattern::Plain));
        }
        // Rubble fallen inside and round its walls
        for _ in 0..(3 + (rng.next() * 4.0) as usize) {
            let inside = rng.next() < 0.6;
            let q = if inside {
                [rng.range(u0 + t + 0.3, u1 - t - 0.3), rng.range(v0 + t + 0.3, v1 - t - 0.3)]
            } else {
                let a = rng.next() * TAU;
                [(u0 + u1) * 0.5 + a.cos() * ((u1 - u0) * 0.5 + 0.5), (v0 + v1) * 0.5 + a.sin() * ((v1 - v0) * 0.5 + 0.5)]
            };
            // Not in the doorway
            let door = [(u0 + u1) * 0.5 + h.door[0] * (u1 - u0) * 0.5, (v0 + v1) * 0.5 + h.door[1] * (v1 - v0) * 0.5];
            if (q[0] - door[0]).hypot(q[1] - door[1]) < 1.4 {
                continue;
            }
            let size = rng.range(0.2, 0.55);
            b.solids.push(b.block(around(q, [size, size * rng.range(0.6, 1.4)]), [f - 0.1, f + size * rng.range(0.5, 1.2)], c.stone.wall, Pattern::Plain));
        }
        // A chest left in the back of it, facing the door
        let middle = [(u0 + u1) * 0.5, (v0 + v1) * 0.5];
        if h.chest {
            let back = [middle[0] - h.door[0] * ((u1 - u0) * 0.5 - t - 0.5), middle[1] - h.door[1] * ((v1 - v0) * 0.5 - t - 0.5)];
            let heading = at.heading(h.door[1].atan2(h.door[0]));
            self.hoard(features, at.point(back[0], back[1], f), heading, 1.8 + rng.next() * 0.8);
        } else if rng.next() < 0.35 {
            self.prop(features, PropKind::Skull, at.point(middle[0], middle[1], f), rng.next() * TAU);
        }
    }

    /// The chasm between the city and its temple, a single stone bridge across it, its bottom deep
    /// in shadow with crystals glowing among the bones there, and scree up at one end to a ledge
    /// along its side, for whoever falls in to climb out
    fn chasm(&self, c: &Undercity, b: &mut Builder, features: &mut Features, rng: &mut Rng) {
        let at = c.at;
        let f = c.floor;
        let (u, depth) = (c.chasm, c.chasm_depth);
        let bottom = f - depth;
        let scree = ((depth / 0.46).ceil() as usize).max(1);
        let far = CHASM_SIDE + scree as f32 * 0.55;
        b.rooms.push(b.room([u - CHASM, -CHASM_SIDE, u + CHASM, far + 0.3], [bottom, f + 0.3]));
        let center = at.point(u, (far - CHASM_SIDE) * 0.5, bottom + depth * 0.5);
        let dir = at.dir([0.0, 1.0]);
        let slab = Shape::slab(center, [dir[0], dir[1], 0.0], far + CHASM_SIDE + 1.0, 2.0 * CHASM + 1.2, depth + 1.5);
        b.carves.push(slab.roughened(0.5, bottom));
        // The scree, in steps a little higher than a stair's
        for k in 0..scree {
            let r = [u - CHASM, CHASM_SIDE + 0.55 * k as f32, u + CHASM, CHASM_SIDE + 0.55 * (k + 1) as f32];
            b.solids.push(b.block(r, [bottom - 0.3, (bottom + 0.46 * (k + 1) as f32).min(f)], (ROCK, STONE), Pattern::Plain));
        }
        // The ledge back along the temple's side from the top of the scree
        b.rooms.push(b.room([u + CHASM, THROAT - 0.6, u + CHASM + 1.6, far + 0.3], [f, f + 2.6]));
        b.floors.push(b.block([u + CHASM - 0.05, THROAT - 0.6, u + CHASM + 1.6, far + 0.3], [f - 0.6, f], (ROCK, STONE), Pattern::Plain));
        // The bridge, its low walls broken in places
        b.solids.push(b.block([u - CHASM - 0.8, -1.5, u + CHASM + 0.8, 1.5], [f - 0.8, f], c.stone.paving[0], Pattern::Plain));
        b.solids.push(Shape::capsule(at.point(u, -1.2, f - 1.3), at.point(u, 1.2, f - 1.3), 0.7, 0.7, c.stone.wall.0, c.stone.wall.1, false).placed(Placement::Clear).patterned(Pattern::Bricks));
        for side in [-1.0f32, 1.0] {
            let mut s = u - CHASM - 0.8;
            while s < u + CHASM + 0.8 {
                let e = (s + rng.range(1.0, 1.8)).min(u + CHASM + 0.8);
                if rng.next() < 0.7 {
                    b.solids.push(b.block(rect([s, side * 1.5], [e, side * 1.8]), [f, f + rng.range(0.4, 0.8)], c.stone.trim, Pattern::Plain));
                }
                s = e;
            }
        }
        // Crystals glowing down at the bottom, among bones and a fallen pillar
        for k in 0..2 {
            let q = [u + rng.range(-1.2, 1.2), if k == 0 { rng.range(-6.0, -3.0) } else { rng.range(3.0, 6.0) }];
            for _ in 0..5 {
                let o = [rng.range(-0.5, 0.5), rng.range(-0.5, 0.5)];
                b.solids.push(Shape::sphere(at.point(q[0] + o[0], q[1] + o[1], bottom + rng.range(0.0, 0.4)), rng.range(0.18, 0.4), CRYSTAL, AIR, false).placed(Placement::Clear));
            }
            self.light(features, at.point(q[0], q[1], bottom + 0.9), CRYSTALS, 9.0);
            bones(&mut b.solids, &at, [q[0] + 1.0, q[1] + 0.5], bottom, rng);
        }
        b.solids.push(Shape::capsule(at.point(u - 1.0, -CHASM_SIDE + 1.5, bottom + 0.35), at.point(u + 1.2, -2.0, bottom + 0.4), 0.34, 0.3, c.stone.trim.0, c.stone.trim.1, false).placed(Placement::Clear).patterned(Pattern::Carved));
    }

    /// The temple at the city's heart, in a vault of its own past the chasm: dark walls with gold
    /// banded round them, its roof long fallen, a great doorway facing the bridge, pillars down
    /// either side, a dais at the back with an altar on it, void burning over it, and the hoard
    /// of the city's dead lord in front of it, where the lord waits
    fn temple(&self, c: &Undercity, b: &mut Builder, features: &mut Features, rng: &mut Rng) {
        let at = c.at;
        let f = c.floor;
        let l = c.temple;
        self.vault(c, b, l, rng);
        let half = [l.half[0] * 0.62, l.half[1] * 0.62];
        let [u0, v0, u1, v1] = around(l.middle, half);
        let wall = l.room_over([u0, v0, u1, v1]).map_or(6.0, |h| h.min(7.0) - 0.4);
        let t = 0.8;
        // Its floor, dark stone and black
        let tile = 2.0;
        let (nu, nv) = (((u1 - u0) / tile).ceil() as usize, ((v1 - v0) / tile).ceil() as usize);
        for i in 0..nu {
            for j in 0..nv {
                let lo = [u0 + i as f32 * tile, v0 + j as f32 * tile];
                let stone = if (i + j) % 2 == 0 { c.stone.temple } else { c.stone.paving[1] };
                b.floors.push(b.block([lo[0], lo[1], (lo[0] + tile).min(u1), (lo[1] + tile).min(v1)], [f - 0.5, f], stone, Pattern::Plain));
            }
        }
        // Its walls, broken off along the top, a band of gold round them, and the doorway
        let walls = [([u0, v0], [u1, v0 + t], [0.0f32, -1.0f32]), ([u0, v1 - t], [u1, v1], [0.0, 1.0]), ([u0, v0], [u0 + t, v1], [-1.0, 0.0]), ([u1 - t, v0], [u1, v1], [1.0, 0.0])];
        for (a, z, out) in walls {
            let along_u = out[0] == 0.0;
            let (start, end) = if along_u { (a[0], z[0]) } else { (a[1], z[1]) };
            let piece = |s: f32, e: f32, thick: f32| if along_u { rect([s, a[1] - thick], [e, z[1] + thick]) } else { rect([a[0] - thick, s], [z[0] + thick, e]) };
            let spans = if out[0] < -0.5 {
                // Over the doorway, gold over its lintel
                b.solids.push(b.block(piece(-2.6, 2.6, 0.0), [f + 5.0, f + wall - rng.range(0.0, 0.8)], c.stone.temple, Pattern::Bricks));
                b.solids.push(b.block(piece(-2.7, 2.7, 0.15), [f + 4.8, f + 5.1], (GOLD, BRONZE), Pattern::Plain));
                vec![[start, -2.2], [2.2, end]]
            } else {
                vec![[start, end]]
            };
            for [s0, s1] in spans {
                let mut s = s0;
                while s < s1 - 1e-3 {
                    let e = (s + rng.range(1.2, 2.4)).min(s1);
                    let top = f + wall - rng.range(0.0, 1.8);
                    b.solids.push(b.block(piece(s, e, 0.0), [f - 0.3, top], c.stone.temple, Pattern::Bricks));
                    if top > f + 3.2 {
                        b.solids.push(b.block(piece(s, e, 0.06), [f + 2.9, f + 3.1], (GOLD, BRONZE), Pattern::Plain));
                    }
                    s = e;
                }
            }
        }
        // Pillars down either side, one broken, one fallen across the hall's side
        for side in [-1.0f32, 1.0] {
            for k in 0..3 {
                let q = [u0 + (u1 - u0) * (0.25 + 0.22 * k as f32), side * (half[1] - 2.2)];
                let broken = (k + (side > 0.0) as usize) % 4 == 1;
                pillar(&mut b.solids, &at, q, f, f + wall - 0.3, 0.42, c.stone.temple, broken, rng);
            }
        }
        // The dais at the back, two steps up, the altar on it with void burning over it
        let dais = [u1 - t - 4.2, -3.2, u1 - t, 3.2];
        b.solids.push(b.block(dais, [f - 0.3, f + 0.3], c.stone.temple, Pattern::Plain));
        b.solids.push(b.block([dais[0] + 0.5, -2.6, dais[2], 2.6], [f + 0.3, f + 0.6], c.stone.temple, Pattern::Plain));
        let altar = [u1 - t - 1.6, 0.0];
        b.solids.push(b.block(around(altar, [0.6, 1.3]), [f + 0.6, f + 1.55], c.stone.trim, Pattern::Plain));
        b.solids.push(b.block(around(altar, [0.68, 1.38]), [f + 1.55, f + 1.7], (GOLD, BRONZE), Pattern::Plain));
        b.solids.push(b.block(around(altar, [0.15, 1.0]), [f + 1.7, f + 1.76], (GLOW_VOID, AIR), Pattern::Plain));
        let heart = at.point(altar[0] + 0.3, altar[1], f + 3.4);
        b.solids.push(Shape::sphere(heart, 0.45, GLOW_VOID, AIR, false).placed(Placement::Clear));
        for k in 0..6 {
            let a = k as f32 * TAU / 6.0;
            let tip = at.point(altar[0] + 0.3 + a.cos() * 1.3, altar[1] + a.sin() * 1.3, f + 3.4 + rng.range(-0.8, 0.8));
            b.solids.push(Shape::capsule(heart, tip, 0.12, 0.03, OBSIDIAN, AIR, false).placed(Placement::Clear));
        }
        self.light(features, at.point(altar[0] - 1.2, altar[1], f + 3.0), [VOID[0] * 1.6, VOID[1] * 1.6, VOID[2] * 1.6], 20.0);
        // Candles on the altar, and braziers inside the doorway
        for v in [-0.9f32, 0.9] {
            let p = at.point(altar[0] - 0.3, v, f + 1.7);
            self.prop(features, PropKind::Candle, p, rng.next() * TAU);
            self.light(features, [p[0], p[1], p[2] + 0.3], [1.0, 0.6, 0.25], 6.0);
        }
        for side in [-1.0f32, 1.0] {
            self.brazier(c, b, features, [u0 + t + 1.0, side * 3.4], f);
            // Outside, either side of the doorway, and either side of the dais
            self.brazier(c, b, features, [u0 - 1.3, side * 3.3], f);
            self.brazier(c, b, features, [dais[0] - 0.7, side * 3.8], f);
        }
        // Crystals round the temple's vault, outside its walls
        for k in 0..2 {
            let a = if k == 0 { 2.2 } else { -2.2 } + rng.range(-0.3, 0.3);
            let q = [l.middle[0] + a.cos() * l.half[0] * 0.85, l.middle[1] + a.sin() * l.half[1] * 0.85];
            for _ in 0..6 {
                let o = [rng.range(-0.7, 0.7), rng.range(-0.7, 0.7)];
                b.solids.push(Shape::sphere(at.point(q[0] + o[0], q[1] + o[1], f + rng.range(0.0, 0.6)), rng.range(0.2, 0.5), CRYSTAL, AIR, false).placed(Placement::Clear));
            }
            self.light(features, at.point(q[0], q[1], f + 1.2), CRYSTALS, 10.0);
        }
        for _ in 0..3 {
            let (a, r) = (rng.next() * TAU, rng.range(0.7, 0.85));
            let q = [l.middle[0] + a.cos() * l.half[0] * r, l.middle[1] + a.sin() * l.half[1] * r];
            if q[0] < u0 - 1.0 || q[1].abs() > half[1] + 1.0 {
                self.fungus(c, b, features, q, f, rng);
            }
        }
        for _ in 0..6 {
            let q = [rng.range(u0 + 1.5, u1 - 5.0), rng.range(v0 + 1.5, v1 - 1.5)];
            bones(&mut b.solids, &at, q, f, rng);
        }
        // The lord's hoard before the altar, and the lord in the middle of the hall
        let hoard = [dais[0] + 0.9, 0.0];
        self.hoard(features, at.point(hoard[0], hoard[1], f + 0.3), at.heading(std::f32::consts::PI), 5.0);
        self.site(features, "undercity hall", at.point(u0 - 3.0, 0.0, f), at.dir([-1.0, 0.0]));
        self.site(features, "undercity heart", at.point(l.middle[0] - 0.5, 0.0, f), at.dir([-1.0, 0.0]));
    }
}

impl Undercity {
    /// The rectangle over the top of the shaft and what's round it, x0, y0, x1, y1 in meters
    fn shaft_rect(&self) -> [f32; 4] {
        let (a, b) = self.at.rect([-SHAFT - GLADE, SHAFT + GLADE], [-SHAFT - GLADE, SHAFT + GLADE]);
        [a[0], a[1], b[0], b[1]]
    }

    /// The way out at the top of the stair, from its open side out to `out` from the middle of
    /// the shaft, u0, v0, u1, v1
    fn exit_rect(&self, outer: f32) -> [f32; 4] {
        let (m, o, up) = (self.exit_point, self.exit_out, self.exit_up);
        let s = m[0] * up[0] + m[1] * up[1];
        let point = |n: f32, k: f32| [o[0] * n + up[0] * k, o[1] * n + up[1] * k];
        // From the last step on up the way the stair was going, none of the steps before it
        // raised to its height
        rect(point(SHAFT - STAIR - 0.05, s - 0.2), point(outer, s + 1.5))
    }

    /// Where its parts are, for trying it out
    fn sunken(&self) -> Sunken {
        let at = self.at;
        let f = self.floor;
        let (m, o) = (self.exit_point, self.exit_out);
        let e = [m[0] + o[0] * (STAIR * 0.5 + 3.6), m[1] + o[1] * (STAIR * 0.5 + 3.6)];
        let gate = SHAFT + GATE;
        Sunken {
            entrance: at.point(e[0], e[1], self.exit_at),
            head: at.point(m[0] + o[0] * (STAIR * 0.5 + 0.4), m[1] + o[1] * (STAIR * 0.5 + 0.4), self.exit_at),
            out: at.dir(o),
            foot: at.point(0.0, 0.0, f),
            gate: at.point(gate + 2.0, 0.0, f),
            street: at.point((gate + self.canal) * 0.5, 0.0, f),
            plaza: at.point(self.plaza[0] - 4.5, self.plaza[1], f),
            canal: at.point(self.canal, self.canal_reach * 0.6, f - CANAL_DEPTH),
            hall: at.point(self.temple.middle[0] - self.temple.half[0] * 0.62 - 3.0, 0.0, f),
            heart: at.point(self.temple.middle[0] - 0.5, 0.0, f),
            forward: at.dir([1.0, 0.0]),
        }
    }
}

/// Collects the shapes of a city in the order they have to go in, so each wins over what comes
/// after it, all in the city's frame
struct Builder {
    at: Frame,
    solids: Vec<Shape>,
    floors: Vec<Shape>,
    rooms: Vec<Shape>,
    carves: Vec<Shape>,
}

impl Builder {
    /// Stone over rectangle `r`, u0, v0, u1, v1, from `z[0]` to `z[1]`, in the ground or out of it
    fn block(&self, r: [f32; 4], z: [f32; 2], stone: (u8, u8), pattern: Pattern) -> Shape {
        self.at.block([r[0], r[2]], [r[1], r[3]], z, stone.0, stone.1).placed(Placement::Clear).patterned(pattern)
    }

    /// Empty space over rectangle `r`, cut out of the ground and anything after it
    fn room(&self, r: [f32; 4], z: [f32; 2]) -> Shape {
        self.at.room([r[0], r[2]], [r[1], r[3]], z)
    }

    /// Radii along u, v and up, along x, y and up
    fn radii(&self, r: Vec3) -> Vec3 {
        if self.at.f[0] != 0.0 { r } else { [r[1], r[0], r[2]] }
    }
}

/// Paves rectangle `r`, u0, v0, u1, v1, with its top at `z`, in tiles `tile` long along it of two
/// kinds of stone
fn pave(b: &mut Builder, r: [f32; 4], z: f32, tile: f32, paving: [(u8, u8); 2]) {
    let along_u = r[2] - r[0] >= r[3] - r[1];
    let (start, end) = if along_u { (r[0], r[2]) } else { (r[1], r[3]) };
    let mut s = start;
    let mut k = 0;
    while s < end - 1e-3 {
        let e = (s + tile).min(end);
        let piece = if along_u { [s, r[1], e, r[3]] } else { [r[0], s, r[2], e] };
        b.floors.push(b.block(piece, [z - 0.5, z], paving[k % 2], Pattern::Plain));
        s = e;
        k += 1;
    }
}

/// A few bones lying about at `q` in frame `at`, on the floor at `z`
fn bones(s: &mut Vec<Shape>, at: &Frame, q: [f32; 2], z: f32, rng: &mut Rng) {
    for _ in 0..(2 + (rng.next() * 3.0) as usize) {
        let (a, d) = (rng.next() * TAU, rng.range(0.0, 0.5));
        let p = [q[0] + a.cos() * d, q[1] + a.sin() * d];
        let (turn, long) = (rng.next() * TAU, rng.range(0.22, 0.45));
        let end = [p[0] + turn.cos() * long, p[1] + turn.sin() * long];
        s.push(Shape::capsule(at.point(p[0], p[1], z + 0.03), at.point(end[0], end[1], z + 0.03), 0.035, 0.03, BONE, AIR, false).placed(Placement::Clear));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fallen cities turn up now and then, and each one can be walked out of: from its square,
    /// along its streets, across the canal, to the foot of the shaft and up the stair round it
    /// to the land above, and from the square across the chasm to the temple at its heart
    #[test]
    fn undercities_turn_up_with_a_way_down_from_the_land() {
        let small = TerrainSettings { width_m: 32, height_m: 64, voxels_per_meter: 4, seed: 1, grass_tufts: false };
        let (terrain, _) = generate_terrain(&small, [0, 0], &RiverCache::default());
        let cache = RiverCache::default();
        let mut cities = Vec::new();
        let (side, n) = (CITY_CELL, 6);
        for cy in -n..n {
            for cx in -n..n {
                if let Some(c) = terrain.undercity_in(cx, cy, &cache) {
                    cities.push(c);
                }
            }
        }
        let squares = (2 * n * 2 * n) as f32;
        println!("{} fallen cities in {:.0} square kilometers", cities.len(), squares * side * side / 1e6);
        assert!(cities.len() >= 4, "only {} cities", cities.len());

        // Walked round in a world built round each of the first few
        for c in cities.iter().take(8) {
            walk_round(c, &cache);
        }
    }

    /// Walks round city `c` in a world built round it: from its square to the foot of the shaft,
    /// to the temple, and up the stair out onto the land, and back down from there to the square
    fn walk_round(c: &Undercity, cache: &RiverCache) {
        let vpm = 4;
        let settings = TerrainSettings { width_m: 192, height_m: 64, voxels_per_meter: vpm, seed: 1, grass_tufts: false };
        let middle = c.at.point(c.temple.middle[0] * 0.5, 0.0, 0.0);
        let origin = [0, 1].map(|i| ((middle[i] * vpm as f32) as i64 - (settings.size()[i] / 2) as i64).div_euclid(8) * 8);
        let (_, world) = generate_terrain(&settings, origin, cache);
        let v = vpm as f32;
        let to_voxels = |p: Vec3| [(p[0] * v).floor() as i64 - origin[0], (p[1] * v).floor() as i64 - origin[1], (p[2] * v).round() as i64];
        let solid = |x: i64, y: i64, z: i64| z < 0 || !is_passable(world.material_at(x, y, z));
        let headroom = (1.8 * v) as i64;
        let stands = |x: i64, y: i64, z: i64| solid(x, y, z - 1) && (0..headroom).all(|k| !solid(x, y, z + k));
        // Everywhere reachable from `from`, stepping up at most half a meter at a time, or dropping
        let reach = |from: [i64; 3]| {
            let mut seen = std::collections::HashSet::new();
            let start = (from[0], from[1], (from[2] - 3..from[2] + 8).find(|&z| stands(from[0], from[1], z)).expect("nowhere to stand"));
            let mut queue = vec![start];
            seen.insert(start);
            while let Some((x, y, z)) = queue.pop() {
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= world.size[0] as i64 || ny >= world.size[1] as i64 {
                        continue;
                    }
                    // Up a step, level, or down as far as it drops
                    let next = (z - 80..=z + 2).rev().find(|&k| stands(nx, ny, k) && (k..k.max(z) + headroom).all(|j| !solid(nx, ny, j)));
                    if let Some(k) = next
                        && seen.insert((nx, ny, k))
                    {
                        queue.push((nx, ny, k));
                    }
                }
            }
            seen
        };
        let s = c.sunken();
        let from_plaza = reach(to_voxels(s.plaza));
        let near = |p: Vec3, set: &std::collections::HashSet<(i64, i64, i64)>| {
            let q = to_voxels(p);
            set.iter().any(|&(x, y, z)| (x - q[0]).abs() <= 2 && (y - q[1]).abs() <= 2 && (z - q[2]).abs() <= 3)
        };
        assert!(near(s.foot, &from_plaza), "the foot of the shaft can't be reached from the square");
        assert!(near(s.heart, &from_plaza), "the temple can't be reached from the square");
        // Out on the land, away from the shaft
        let out = to_voxels(s.entrance);
        let land = from_plaza.iter().any(|&(x, y, z)| (x - out[0]).abs() <= 3 && (y - out[1]).abs() <= 3 && (z - out[2]).abs() <= 6);
        assert!(land, "no way up out of the shaft onto the land");
        // And the other way, down from the land into the square
        let from_land = reach(out);
        assert!(near(s.plaza, &from_land), "the square can't be reached from the land");
        println!("walked from the land {:.1} m down into the city at {:.0}, {:.0} and out again", c.exit_at - c.floor, c.at.x, c.at.y);    }
}
