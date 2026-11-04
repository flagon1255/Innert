//! Royal castles: rare and huge, each on a hilltop, a ridge or a headland over the water that's
//! flat enough on top. A curtain wall goes round it all with towers along it and a gatehouse in
//! front, open ground inside, and up a grand stair the inner ward on its terrace: a courtyard
//! between two long wings, the great hall behind it and the keep behind that, where the princess
//! keeps the royal armoury in the throne room. Towers of every height rise all over it under tall
//! spires of copper or slate, joined by covered bridges over the gardens. Each is laid out from
//! numbers of its own, so no two are quite alike.
//!
//! A castle's laid out in a frame facing its gate: u goes out through the gate, v across, both in
//! meters from the middle of the front of the great hall.

use super::*;

/// At most one castle in each CASTLE_CELL square, its middle well inside it, nothing of it further
/// than CASTLE_REACH from there, in meters
const CASTLE_CELL: f32 = 720.0;
const CASTLE_REACH: f32 = 125.0;
/// How many squares have a castle where the land suits, and how many spots are tried in each
const CASTLE_CHANCE: f32 = 0.85;
const SPOTS: u32 = 96;
/// The least the tallest spire rises over the inner ward, in meters
const LEAST_HEIGHT: f32 = 28.0;
/// Stairs anyone can walk up: how high each step is and how deep
const RISE: f32 = 0.25;
const RUN: f32 = 0.45;
/// How steep the roofs of the great hall and the wings are
const HALL_PITCH: f32 = 0.68;
const WING_PITCH: f32 = 0.85;
/// How far a spire's finial or flag reaches over its tip
const FINIAL: f32 = 1.9;
/// The light flames give
const FIRE: [f32; 3] = [1.0, 0.62, 0.28];
const WAYS: [[f32; 2]; 4] = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]];

/// What a castle's built of, which goes by the land round it
#[derive(Clone, Copy)]
struct Stones {
    /// Its walls, and the stone round windows, along the foot of walls and on stairs
    wall: (u8, u8),
    trim: (u8, u8),
    /// Its spires and roofs
    roof: (u8, u8),
    /// What the courtyard and the ways in are paved with
    paving: [(u8, u8); 2],
    /// The cloth of its banners, and the glass of its lit windows
    banner: u8,
    glass: u8,
}

impl Stones {
    fn of(c: Climate, rng: &mut Rng) -> Stones {
        let pick = rng.next();
        let banner = [CLOTH, FLOWER_PURPLE, TEMPLE_TEAL, CLOTH][(rng.next() * 3.99) as usize];
        if c.desert > 0.5 {
            Stones { wall: (SANDSTONE, TEMPLE_STONE), trim: (TEMPLE_STONE, SANDSTONE), roof: (TEMPLE_TEAL, GOLD), paving: [(TEMPLE_STONE, SANDSTONE), (SANDSTONE, SAND)], banner: TEMPLE_RED, glass: GLOW_FIRE }
        } else if c.snow > 0.5 {
            Stones { wall: (BRICK_STONE, LIMESTONE), trim: (STONE, SNOW), roof: (DARK_STEEL, SNOW), paving: [(STONE, SNOW), (BRICK_STONE, GRAVEL)], banner, glass: GLOW_HOLY }
        } else if c.jungle > 0.5 {
            let roof = if pick < 0.5 { (TEMPLE_TEAL, MOSS) } else { (TEMPLE_RED, CLAY_POT) };
            Stones { wall: (TEMPLE_STONE, MOSS), trim: (TEMPLE_DARK, MOSS), roof, paving: [(TEMPLE_STONE, MOSS), (TEMPLE_DARK, MOSS)], banner, glass: GLOW_FIRE }
        } else {
            // Copper gone green, or slate
            let roof = if pick < 0.45 { (TEMPLE_TEAL, MOSS) } else if pick < 0.85 { (DARK_STEEL, STEEL) } else { (OBSIDIAN, DARK_STEEL) };
            Stones { wall: (LIMESTONE, BRICK_STONE), trim: (BRICK_STONE, MORTAR), roof, paving: [(LIMESTONE, BRICK_STONE), (BRICK_STONE, STONE)], banner, glass: GLOW_HOLY }
        }
    }
}

/// How a tower's topped off
#[derive(Clone, Copy, PartialEq)]
enum Crown {
    /// A tall spire this high
    Spire(f32),
    /// Merlons round a flat top, a turret in the middle of the bigger ones
    Battlements,
    /// Carried out from the corner of a wall on a cone of stone, with a spire this high
    Corbelled(f32),
}

/// A round tower, in the castle's frame: where its middle is, how wide, how far down its footing
/// goes, how high the ground or terrace it rises from is, and where its walls stop
#[derive(Clone, Copy)]
struct Tower {
    c: [f32; 2],
    r: f32,
    foot: f32,
    seen: f32,
    /// The sills of its lowest windows
    sill: f32,
    top: f32,
    crown: Crown,
    flag: bool,
    /// How many of its windows are lit
    lit: f32,
}

/// A covered bridge from `a` to `b` along u or v, its floor at `z`
#[derive(Clone, Copy)]
struct Bridge {
    a: [f32; 2],
    b: [f32; 2],
    z: f32,
}

/// A castle, laid out
struct Castle {
    at: Frame,
    settings: TerrainSettings,
    stones: Stones,
    /// The highest anything of it may reach
    sky: f32,
    /// How high the inner ward's terrace is, the lowest ground under it, and the throne room's floor
    ward: f32,
    lowest: f32,
    throne: f32,
    /// The keep: from keep[0] to keep[1] along u, keep_half either side
    keep: [f32; 2],
    keep_half: f32,
    keep_top: f32,
    /// The great hall, from keep[1] to 0 along u
    hall_half: f32,
    hall_eaves: f32,
    /// The courtyard, from 0 to `court` along u, and the front range from there to `range`, the
    /// front of the inner ward
    court: f32,
    court_half: f32,
    range: f32,
    range_top: f32,
    /// The wings along the courtyard, from court_half to court_half + wing across
    wing: f32,
    wing_eaves: f32,
    ward_half: f32,
    /// The curtain wall: its outer faces along u and across, and its top
    front: f32,
    back: f32,
    side: f32,
    wall_top: f32,
    gate_floor: f32,
    /// Steps down the grand stair, out in front of the inner ward
    steps: usize,
    towers: Vec<Tower>,
    bridges: Vec<Bridge>,
    /// Where it all is, x0, y0, x1, y1 in meters
    bounds: [f32; 4],
    seed: u32,
}

/// Where to look at a royal castle from and what's in it, for trying them out, in meters
pub struct Royal {
    /// The middle of it all, and just outside its gate, which faces `out`
    pub middle: [f32; 3],
    pub gate: [f32; 3],
    pub out: [f32; 2],
    /// The top of the keep's great tower, to look at
    pub tower: [f32; 3],
    /// Just inside the door of the great hall, and the top of the stair at its far end
    pub hall: [f32; 3],
    pub dais: [f32; 3],
    /// Inside the throne room, by the way in, and where the princess stands
    pub throne: [f32; 3],
    pub princess: [f32; 3],
}

/// The lowest and highest ground in castle frame `at` from u[0] to u[1] and v[0] to v[1], looked
/// at every `step` meters or so
fn span(settings: &TerrainSettings, at: &Frame, u: [f32; 2], v: [f32; 2], step: f32) -> (f32, f32) {
    let nu = ((u[1] - u[0]).abs() / step).ceil().max(1.0) as usize;
    let nv = ((v[1] - v[0]).abs() / step).ceil().max(1.0) as usize;
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for i in 0..=nu {
        for j in 0..=nv {
            let p = at.point(u[0] + (u[1] - u[0]) * i as f32 / nu as f32, v[0] + (v[1] - v[0]) * j as f32 / nv as f32, 0.0);
            let g = height_at(settings, p[0], p[1]);
            (lo, hi) = (lo.min(g), hi.max(g));
        }
    }
    (lo, hi)
}

fn bl(at: &Frame, u: [f32; 2], v: [f32; 2], z: [f32; 2], m: (u8, u8)) -> Shape {
    at.block(u, v, z, m.0, m.1)
}

fn bricks(at: &Frame, u: [f32; 2], v: [f32; 2], z: [f32; 2], m: (u8, u8)) -> Shape {
    at.block(u, v, z, m.0, m.1).patterned(Pattern::Bricks)
}

/// Upright, at `c` from z[0] to z[1], its radius going from r[0] to r[1]
fn up(at: &Frame, c: [f32; 2], z: [f32; 2], r: [f32; 2], m: (u8, u8)) -> Shape {
    Shape::capsule(at.point(c[0], c[1], z[0]), at.point(c[0], c[1], z[1]), r[0], r[1], m.0, m.1, false)
}

/// From `a` to `b`, u, v and z
fn rod(at: &Frame, a: Vec3, b: Vec3, r: f32, m: (u8, u8)) -> Shape {
    Shape::capsule(at.point(a[0], a[1], a[2]), at.point(b[0], b[1], b[2]), r, r, m.0, m.1, false)
}

/// An ellipsoid round `c`, its radii along u, v and z
fn blob(at: &Frame, c: Vec3, radii: Vec3, m: (u8, u8)) -> Shape {
    let world = if at.f[0] != 0.0 { radii } else { [radii[1], radii[0], radii[2]] };
    Shape::ellipsoid(at.point(c[0], c[1], c[2]), world, m.0, m.1)
}

/// A tall window with a round head, in a wall facing `out` whose outer face goes through `face`:
/// `depth` through the wall, `half` its half width, from its sill at z[0] to the top of its head
/// at z[1], of `glass`, set back a little in the wall with a sill of `trim` under it
#[allow(clippy::too_many_arguments)]
fn window(s: &mut Vec<Shape>, at: &Frame, face: [f32; 2], out: [f32; 2], depth: f32, half: f32, z: [f32; 2], glass: u8, trim: (u8, u8)) {
    let p = at.point(face[0], face[1], 0.0);
    let w = Frame { x: p[0], y: p[1], f: at.dir(out) };
    let head = (z[1] - half).max(z[0] + 0.2);
    s.push(w.block([-0.05, 0.24], [-half - 0.14, half + 0.14], [z[0] - 0.16, z[0]], trim.0, trim.1));
    s.push(w.block([-depth - 0.02, -0.16], [-half, half], [z[0], head], glass, AIR));
    s.push(blob(&w, [-(depth + 0.18) * 0.5, 0.0, head], [(depth - 0.14) * 0.5, half, half], (glass, AIR)));
    s.push(w.room([-0.16, 0.05], [-half, half], [z[0], head]));
    s.push(blob(&w, [0.05, 0.0, head], [0.21, half, half], (AIR, AIR)).placed(Placement::Clear));
}

/// A narrow window in a round tower round `c`, `r` across, looking `dir`
#[allow(clippy::too_many_arguments)]
fn slit(s: &mut Vec<Shape>, at: &Frame, c: [f32; 2], r: f32, dir: [f32; 2], z: f32, tall: f32, half: f32, glass: u8) {
    let p = at.point(c[0] + dir[0] * r, c[1] + dir[1] * r, 0.0);
    let w = Frame { x: p[0], y: p[1], f: at.dir(dir) };
    s.push(w.block([-0.7, 0.04], [-half, half], [z, z + tall - half], glass, AIR));
    s.push(blob(&w, [-0.33, 0.0, z + tall - half], [0.37, half, half], (glass, AIR)));
}

/// A parapet along the top of a wall at `z`, over u[0] to u[1] and v[0] to v[1], with merlons on it
fn battlements(s: &mut Vec<Shape>, at: &Frame, u: [f32; 2], v: [f32; 2], z: f32, stone: (u8, u8)) {
    let along_u = u[1] - u[0] > v[1] - v[0];
    let (a0, a1) = if along_u { (u[0], u[1]) } else { (v[0], v[1]) };
    let mut a = a0 + 0.1;
    while a + 0.9 <= a1 {
        let (mu, mv) = if along_u { ([a, a + 0.9], v) } else { (u, [a, a + 0.9]) };
        s.push(bricks(at, mu, mv, [z + 0.6, z + 1.55], stone));
        a += 1.7;
    }
    s.push(bricks(at, u, v, [z - 0.05, z + 0.6], stone));
}

/// A pitched roof over a range from a[0] to a[1] along its ridge, `across` either side of `middle`:
/// rows of tiles or slates from `eaves` up to the ridge, `pitch` meters up for each across. Along
/// u, or along v. Returns how high its ridge is.
#[allow(clippy::too_many_arguments)]
fn pitched(s: &mut Vec<Shape>, at: &Frame, along_u: bool, a: [f32; 2], middle: f32, across: f32, eaves: f32, pitch: f32, cover: (u8, u8), ridge: u8) -> f32 {
    let point = |l: f32, c: f32, z: f32| if along_u { at.point(l, middle + c, z) } else { at.point(middle + c, l, z) };
    let rise = across * pitch;
    let rows = (across * (1.0 + pitch * pitch).sqrt() / 0.36).ceil() as usize;
    s.push(Shape::capsule(point(a[0], 0.0, eaves + rise + 0.05), point(a[1], 0.0, eaves + rise + 0.05), 0.27, 0.27, ridge, AIR, false));
    for k in 0..rows {
        let t = k as f32 / rows as f32;
        let (c, z) = (across * (1.0 - t), eaves + rise * t);
        for side in [-1.0, 1.0] {
            s.push(Shape::capsule(point(a[0], side * c, z), point(a[1], side * c, z), 0.24, 0.24, cover.0, cover.1, false));
        }
    }
    eaves + rise
}

/// The gable end under a pitched roof, from a[0] to a[1] along its ridge: stepping in from the top
/// of its walls at `wall_top`, `half` either side of `middle`, up under the roof's rows
#[allow(clippy::too_many_arguments)]
fn gable_end(s: &mut Vec<Shape>, at: &Frame, along_u: bool, a: [f32; 2], middle: f32, half: f32, wall_top: f32, across: f32, eaves: f32, pitch: f32, stone: (u8, u8)) {
    let rise = across * pitch;
    let mut z = wall_top - 0.05;
    loop {
        let w = (across * (1.0 - (z + 0.5 - eaves + 0.15) / rise) - 0.05).min(half);
        if w < 0.15 {
            break;
        }
        let (u, v) = if along_u { (a, [middle - w, middle + w]) } else { ([middle - w, middle + w], a) };
        s.push(bricks(at, u, v, [z, z + 0.5], stone));
        z += 0.5;
    }
}

/// Paves the ground from `lo` to `hi`, x and y in meters, where it's between about `low` and
/// `high`, as towns do their streets: following the ground voxel for voxel
fn pave(s: &mut Vec<Shape>, lo: [f32; 2], hi: [f32; 2], low: f32, high: f32, stone: (u8, u8)) {
    let (bottom, top) = ([lo[0], lo[1], low - 0.4], [hi[0], hi[1], high + 0.1]);
    s.push(Shape::block(bottom, top, AIR, AIR).placed(Placement::Carve));
    s.push(Shape::block(bottom, top, stone.0, stone.1).placed(Placement::Inside));
    s.push(Shape::room([lo[0], lo[1], low], [hi[0], hi[1], high + 0.6]));
}

impl Terrain {
    /// The royal castles reaching into this piece of the landscape, built, and where each castle
    /// near enough for a ravine reaching in here to run into it is, x0, y0, x1, y1 in meters, so
    /// nothing else is built there
    pub(super) fn castles(&self, cache: &RiverCache, features: &mut Features) -> (Vec<Object>, Vec<[f32; 4]>) {
        let (lo, hi) = (self.lo, self.hi);
        let piece = [lo[0], lo[1], hi[0], hi[1]];
        let near = grown(piece, RAVINE_REACH + RAVINE_CELL + 4.0);
        let first = [0, 1].map(|i| ((near[i] - CASTLE_REACH) / CASTLE_CELL).floor() as i32);
        let last = [0, 1].map(|i| ((near[i + 2] + CASTLE_REACH) / CASTLE_CELL).floor() as i32);
        let (mut objects, mut rects) = (Vec::new(), Vec::new());
        for cy in first[1]..=last[1] {
            for cx in first[0]..=last[0] {
                // Only the squares whose castle could come near, as planning one takes a while
                let (x0, y0) = (cx as f32 * CASTLE_CELL, cy as f32 * CASTLE_CELL);
                let could = [x0 + 0.2 * CASTLE_CELL - CASTLE_REACH, y0 + 0.2 * CASTLE_CELL - CASTLE_REACH, x0 + 0.8 * CASTLE_CELL + CASTLE_REACH, y0 + 0.8 * CASTLE_CELL + CASTLE_REACH];
                if !overlaps(could, near, 0.0) {
                    continue;
                }
                let Some(castle) = self.castle_in(cx, cy, cache) else {
                    continue;
                };
                if overlaps(castle.bounds, piece, 0.0) {
                    objects.extend(castle.build(self, features));
                }
                if overlaps(castle.bounds, near, 0.0) {
                    rects.push(castle.bounds);
                }
            }
        }
        (objects, rects)
    }

    /// The nearest royal castle to `near` within a few kilometers, for trying them out
    #[allow(dead_code)]
    pub fn nearest_castle(&self, near: [f32; 2]) -> Option<Royal> {
        let cache = RiverCache::default();
        let home = near.map(|v| (v / CASTLE_CELL).floor() as i32);
        let mut best: Option<(f32, Castle)> = None;
        for ring in 0..6i32 {
            for cy in home[1] - ring..=home[1] + ring {
                for cx in home[0] - ring..=home[0] + ring {
                    if (cx - home[0]).abs().max((cy - home[1]).abs()) != ring {
                        continue;
                    }
                    if let Some(castle) = self.castle_in(cx, cy, &cache) {
                        let m = castle.at.point((castle.front + castle.back) * 0.5, 0.0, 0.0);
                        let distance = (m[0] - near[0]).hypot(m[1] - near[1]);
                        if best.as_ref().is_none_or(|(b, _)| distance < *b) {
                            best = Some((distance, castle));
                        }
                    }
                }
            }
            if best.as_ref().is_some_and(|(b, _)| *b <= ring as f32 * CASTLE_CELL) {
                break;
            }
        }
        best.map(|(_, c)| c.royal())
    }

    /// The castle in one CASTLE_CELL square: at the best of a few spots in it where the land
    /// suits, if nothing else is in the way there
    fn castle_in(&self, cx: i32, cy: i32, cache: &RiverCache) -> Option<Castle> {
        let r = hash3(cx as u32, cy as u32, self.settings.seed ^ 0xca51);
        if unit(r) > CASTLE_CHANCE {
            return None;
        }
        let mut spots: Vec<(f32, [f32; 2], u32)> = (0..SPOTS)
            .filter_map(|attempt| {
                let s = hash3(r, attempt, 0xca52);
                let p = [(cx as f32 + 0.2 + 0.6 * unit(s)) * CASTLE_CELL, (cy as f32 + 0.2 + 0.6 * unit(s >> 16)) * CASTLE_CELL];
                self.castle_spot(p).map(|score| (score, p, hash(s)))
            })
            .collect();
        spots.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut caves = HashMap::new();
        spots.iter().take(8).find_map(|&(_, p, seed)| {
            let castle = self.lay_out_castle(p, seed)?;
            self.clear_for_castle(&castle, cache, &mut caves).then_some(castle)
        })
    }

    /// How well the land suits a castle with its middle at `p`, None if it doesn't: higher than
    /// the land round it or out over the water, flat enough on top, all of it dry and low enough
    /// for its spires to rise well over it
    fn castle_spot(&self, p: [f32; 2]) -> Option<f32> {
        let settings = &self.settings;
        let sea = sea_level_m(settings);
        let sky = settings.height_m as f32 - 1.0;
        let g = |dx: f32, dy: f32| height_at(settings, p[0] + dx, p[1] + dy);
        let (mut lo, mut hi, mut sum) = (f32::MAX, f32::MIN, 0.0);
        for i in 0..49 {
            let h = g(((i % 7) as f32 - 3.0) * 12.0, ((i / 7) as f32 - 3.0) * 12.0);
            (lo, hi, sum) = (lo.min(h), hi.max(h), sum + h);
        }
        // The terrace it stands on takes up what's uneven, but only so much
        if hi - lo > 14.0 || hi > sky - LEAST_HEIGHT - 0.6 || lo < sea + 1.0 {
            return None;
        }
        // Where the curtain wall goes
        let mut walls = f32::MAX;
        for k in 0..16 {
            let a = k as f32 * TAU / 16.0;
            walls = walls.min(g(a.cos() * 60.0, a.sin() * 50.0));
        }
        if walls < sea + 1.2 || hi - walls > 18.0 {
            return None;
        }
        // Lower land all round for a hill or a ridge, or water for a headland
        let (mut around, mut wet) = (0.0, 0);
        for k in 0..16 {
            let a = k as f32 * TAU / 16.0;
            let h = g(a.cos() * 115.0, a.sin() * 115.0);
            around += h;
            if h < sea + 0.3 {
                wet += 1;
            }
        }
        let prominence = sum / 49.0 - around / 16.0;
        let headland = wet >= 3;
        if prominence < 1.5 && !(headland && prominence > 0.0) {
            return None;
        }
        Some(prominence.min(12.0) + if headland { 3.0 } else { 0.0 } - (hi - lo) * 0.5 - (hi - walls) * 0.1)
    }

    /// Lays out a castle with its middle at `p`, if it fits there
    fn lay_out_castle(&self, p: [f32; 2], seed: u32) -> Option<Castle> {
        let settings = self.settings;
        let sea = sea_level_m(&settings);
        let ground = |x: f32, y: f32| height_at(&settings, x, y);
        let mut rng = Rng(seed);
        // The gate on the low side, so the way up to it comes up the hill
        let f = WAYS
            .into_iter()
            .min_by(|a, b| {
                let g = |d: &[f32; 2]| ground(p[0] + d[0] * 55.0, p[1] + d[1] * 55.0) + ground(p[0] + d[0] * 75.0, p[1] + d[1] * 75.0);
                g(a).total_cmp(&g(b))
            })
            .unwrap();
        let keep_depth = rng.range(22.0, 24.0);
        let keep_half = rng.range(11.0, 12.5);
        let hall_len = rng.range(24.0, 28.0);
        let hall_half = rng.range(8.5, 9.6);
        let court = rng.range(22.0, 26.0);
        let court_half = hall_half + rng.range(4.0, 5.5);
        let range_depth = rng.range(7.2, 8.0);
        let wing = rng.range(9.0, 10.5);
        let garden = rng.range(8.0, 9.5);
        let keep = [-(hall_len + keep_depth), -hall_len];
        let range = court + range_depth;
        let ward_half = court_half + wing + garden;
        let back = keep[0] - rng.range(9.0, 11.0);
        let side = ward_half + rng.range(7.5, 9.0);
        // Laid out round the middle of the inner ward, however far out the gate ends up
        let ward_middle = (keep[0] + range) * 0.5;
        let at = Frame { x: p[0] - f[0] * ward_middle, y: p[1] - f[1] * ward_middle, f };
        let span = |u: [f32; 2], v: [f32; 2], step: f32| span(&settings, &at, u, v, step);

        // The inner ward stands on a terrace over the highest ground under it
        let (lowest, highest) = span([keep[0], range], [-ward_half, ward_half], 4.0);
        let ward = highest + 0.6;
        let sky = settings.height_m as f32 - 1.0;
        let tall = sky - ward;
        if tall < LEAST_HEIGHT || ward - lowest > 14.5 {
            return None;
        }
        // The grand stair down from the ward to where it meets the ground, with room enough in
        // front of it for the road and the gatehouse
        let steps = (1..64).find(|&k| {
            let u = range + (k - 1) as f32 * RUN;
            ward - k as f32 * RISE <= span([u, u + RUN], [-4.0, 4.0], 2.0).1 + 0.15
        })?;
        let front = range + rng.range(22.0, 26.0).max(steps as f32 * RUN + 14.0);
        let middle = (front + back) * 0.5;
        // The curtain wall over the highest ground along it, but not lost under the ward
        let mut ring = (f32::MAX, f32::MIN);
        for (u, v) in [([front - 3.0, front], [-side, side]), ([back, back + 3.0], [-side, side]), ([back, front], [side - 3.0, side]), ([back, front], [-side, 3.0 - side])] {
            let (lo, hi) = span(u, v, 6.0);
            ring = (ring.0.min(lo), ring.1.max(hi));
        }
        if ring.0 < sea + 1.0 {
            return None;
        }
        // Low enough for the inner ward to rise well over it from outside
        let wall_top = (ring.1 + rng.range(6.5, 8.0)).clamp(ward - 1.0, ward + 3.0);
        if wall_top - ring.0 > 24.0 {
            return None;
        }
        // Dry ground out in front of the gate, and the floor of the way through it
        if span([front + 1.5, front + 16.0], [-3.0, 3.0], 3.0).0 < sea + 0.8 {
            return None;
        }
        let gate_floor = span([front - 7.6, front + 1.6], [-2.2, 2.2], 1.5).1 + 0.06;

        // How high it all goes, the higher the more room there is under the sky
        let keep_top = ward + (0.44 * tall).max(14.0);
        let hall_eaves = (keep_top - 7.2).max(ward + 7.0);
        let range_top = ward + (0.27 * tall).max(10.0);
        let wing_eaves = range_top + 0.6;
        let wing_ridge = wing_eaves - 0.25 + (wing * 0.5 + 0.45) * WING_PITCH;
        // A spire's top and height: no lower than `least`, reaching about `tip` with its finial
        let spire = |least: f32, tip: f32, r: f32| -> (f32, f32) {
            let tip = tip.min(sky - 0.2);
            let top = (tip - FINIAL - 3.2 * r).max(least);
            let h = (tip - FINIAL - top).clamp(1.6 * r, 3.6 * r).min(sky - 0.2 - FINIAL - top);
            (top, h.max(1.0))
        };
        let wv = court_half + wing;
        let inside = |c: [f32; 2], r: f32| c[0] - r > keep[0] && c[0] + r < range && c[1].abs() + r < ward_half;
        let mut towers = Vec::new();
        let mut corbels = Vec::new();
        let mut tower = |c: [f32; 2], r: f32, top: f32, crown: Crown, flag: bool, lit: f32| {
            let (foot, seen) = if inside(c, r) {
                (ward - 0.4, ward)
            } else {
                let g = [(0.0, 0.0), (r, 0.0), (-r, 0.0), (0.0, r), (0.0, -r)].map(|(du, dv)| {
                    let p = at.point(c[0] + du, c[1] + dv, 0.0);
                    ground(p[0], p[1])
                });
                let low = g.iter().copied().fold(f32::MAX, f32::min);
                (low.min(ward) - 0.6, low)
            };
            towers.push(Tower { c, r, foot, seen, sill: seen + 3.0, top, crown, flag, lit });
        };
        // The great tower, rising out of the back of the keep: the tallest of them all
        let r = rng.range(4.6, 5.2);
        let (top, h) = spire(keep_top + 4.0, sky, r);
        tower([keep[0] + 3.9, 0.0], r, top, Crown::Spire(h), true, 0.6);
        // Towers in the gardens either side of the keep, the next tallest, and at the back
        // corners of the wings and the corners of the front range
        let garden_u = keep[0] + keep_depth * rng.range(0.38, 0.5);
        let gr = rng.range(3.2, 3.7);
        let (gtop, gh) = spire(keep_top - 2.0, ward + tall * rng.range(0.8, 0.9), gr);
        let br = rng.range(3.9, 4.4);
        let (btop, bh) = spire(wing_ridge + 1.2, ward + tall * rng.range(0.6, 0.72), br);
        let fr = rng.range(4.3, 4.8);
        let (ftop, fh) = spire(wing_ridge + 1.2, ward + tall * rng.range(0.66, 0.8), fr);
        let cr = rng.range(2.3, 2.7);
        let (ctop, ch) = spire(wing_ridge + 0.8, wing_ridge + 2.8 + FINIAL + 3.2 * cr, cr);
        for side in [-1.0, 1.0] {
            tower([garden_u, side * wv], gr, gtop, Crown::Spire(gh), true, 0.55);
            tower([0.0, side * wv], br, btop, Crown::Spire(bh), false, 0.5);
            tower([court + range_depth * 0.5, side * wv], fr, ftop, Crown::Spire(fh), false, 0.5);
            // Turrets on the corners of the courtyard
            for u in [0.0, court] {
                tower([u, side * court_half], cr, ctop, Crown::Spire(ch), false, 0.5);
            }
            // Bartizans on the corners of the keep
            for u in keep {
                let foot = keep_top - 4.5;
                corbels.push(Tower { c: [u, side * keep_half], r: 2.1, foot, seen: foot, sill: keep_top - 1.5, top: keep_top + 2.6, crown: Crown::Corbelled(5.4), flag: false, lit: 0.4 });
            }
            // Turrets on the corners of the terrace
            for u in [keep[0], range] {
                let r = rng.range(2.0, 2.3);
                tower([u, side * ward_half], r, ward + rng.range(4.5, 5.5), Crown::Spire(2.8 * r), false, 0.3);
            }
        }
        // Along the curtain wall: on its corners, partway along its sides, and either side of the gate
        let corner = rng.range(4.6, 5.2);
        let spired = rng.next() < 0.65;
        let crown = |r: f32, top: f32, tall: f32| if spired { Crown::Spire((tall * r).min(sky - 0.2 - FINIAL - top)) } else { Crown::Battlements };
        let mid_u = middle + rng.range(-6.0, 6.0);
        let gate_r = rng.range(4.0, 4.4);
        for side_v in [-1.0, 1.0] {
            for u in [front - 1.5, back + 1.5] {
                tower([u, side_v * (side - 1.5)], corner, wall_top + 4.5, crown(corner, wall_top + 4.5, 2.5), false, 0.3);
            }
            tower([mid_u, side_v * (side - 1.5)], 3.8, wall_top + 3.2, Crown::Battlements, false, 0.3);
            tower([back + 1.5, side_v * side * 0.5], 3.6, wall_top + 3.2, Crown::Battlements, false, 0.3);
            tower([front - 1.5, side_v * side * 0.6], 3.6, wall_top + 3.2, crown(3.6, wall_top + 3.2, 2.2), false, 0.3);
            let top = wall_top + 5.5;
            tower([front - 1.0, side_v * (4.4 + gate_r - 1.0)], gate_r, top, Crown::Spire((2.4 * gate_r).min(sky - 0.2 - FINIAL - top)), true, 0.4);
        }
        towers.extend(corbels);
        // The great tower's windows start over the roof of the keep it rises out of
        towers[0].sill = keep_top + 1.0;

        // Covered bridges from the sides of the keep over the gardens to their towers, and on from
        // those to the towers at the backs of the wings
        let mut bridges = Vec::new();
        for side in [-1.0, 1.0] {
            bridges.push(Bridge { a: [garden_u, side * (keep_half - 0.3)], b: [garden_u, side * (wv - gr + 0.3)], z: (keep_top - 5.2).min(gtop - 5.0) });
            bridges.push(Bridge { a: [garden_u + gr - 0.3, side * wv], b: [-br + 0.3, side * wv], z: (wing_eaves - 4.5).min(btop - 5.0).max(ward + 4.5) });
        }

        let (lo, hi) = at.rect([back - 7.0, front + 17.0], [-side - 7.0, side + 7.0]);
        if [lo, hi].iter().any(|q| (q[0] - p[0]).abs() > CASTLE_REACH || (q[1] - p[1]).abs() > CASTLE_REACH) {
            return None;
        }
        let c = climate(&settings, p[0], p[1]);
        Some(Castle {
            at,
            settings,
            stones: Stones::of(c, &mut rng),
            sky,
            ward,
            lowest,
            throne: ward + 6.0 * RISE,
            keep,
            keep_half,
            keep_top,
            hall_half,
            hall_eaves,
            court,
            court_half,
            range,
            range_top,
            wing,
            wing_eaves,
            ward_half,
            front,
            back,
            side,
            wall_top,
            gate_floor,
            steps,
            towers,
            bridges,
            bounds: [lo[0], lo[1], hi[0], hi[1]],
            seed: hash(seed ^ 0xca5e),
        })
    }

    /// Whether nothing else is in the way of castle `c`: no river, dungeon or town near it, nor any
    /// cave breaking out of the ground under it. Caves deep down can pass under, and ravines keep
    /// clear of castles themselves.
    fn clear_for_castle(&self, c: &Castle, cache: &RiverCache, caves: &mut HashMap<(i32, i32), Vec<[f32; 4]>>) -> bool {
        self.castle_blocked(c, cache, caves).is_none()
    }

    /// What's in the way of castle `c`, if anything
    fn castle_blocked(&self, c: &Castle, cache: &RiverCache, caves: &mut HashMap<(i32, i32), Vec<[f32; 4]>>) -> Option<&'static str> {
        let rect = c.bounds;
        let margin = 10.0;
        for river in self.rivers_near(cache, [rect[0] - margin, rect[1] - margin], [rect[2] + margin, rect[3] + margin]) {
            let near_it = river.iter().any(|p| {
                let (dx, dy) = ((rect[0] - p.x).max(p.x - rect[2]).max(0.0), (rect[1] - p.y).max(p.y - rect[3]).max(0.0));
                dx.hypot(dy) < p.half_width + margin
            });
            if near_it {
                return Some("river");
            }
        }
        let reach = DUNGEON_REACH + 2.0;
        let first = [0, 1].map(|i| ((rect[i] - reach) / DUNGEON_CELL).floor() as i32);
        let last = [0, 1].map(|i| ((rect[i + 2] + reach) / DUNGEON_CELL).floor() as i32);
        for cy in first[1]..=last[1] {
            for cx in first[0]..=last[0] {
                // Its halls are deep enough under the ground for the castle to stand over them,
                // but not the crypt over the way down
                if self.dungeon_in(cx, cy, cache).is_some_and(|d| d.world_rects().first().is_some_and(|r| overlaps(*r, rect, 6.0))) {
                    return Some("dungeon");
                }
            }
        }
        let first = [0, 1].map(|i| ((rect[i] - CAVE_REACH) / CAVE_CELL).floor() as i32);
        let last = [0, 1].map(|i| ((rect[i + 2] + CAVE_REACH) / CAVE_CELL).ceil() as i32);
        for cy in first[1]..last[1] {
            for cx in first[0]..last[0] {
                let breaks = caves.entry((cx, cy)).or_insert_with(|| {
                    self.cave_in(cx, cy).map_or(Vec::new(), |cave| {
                        cave.shapes
                            .iter()
                            .filter(|s| s.placement == Placement::Carve && s.max[2] > height_at(&self.settings, (s.min[0] + s.max[0]) * 0.5, (s.min[1] + s.max[1]) * 0.5) - 1.5)
                            .map(|s| [s.min[0], s.min[1], s.max[0], s.max[1]])
                            .collect()
                    })
                });
                if breaks.iter().any(|r| overlaps(*r, rect, 2.0)) {
                    return Some("cave");
                }
            }
        }
        // Towns last, as planning them takes longest
        (!self.towns_near(grown(rect, 15.0), cache).is_empty()).then_some("town")
    }
}

impl Castle {
    fn ground(&self, u: f32, v: f32) -> f32 {
        let p = self.at.point(u, v, 0.0);
        height_at(&self.settings, p[0], p[1])
    }

    fn span(&self, u: [f32; 2], v: [f32; 2], step: f32) -> (f32, f32) {
        span(&self.settings, &self.at, u, v, step)
    }

    /// The throne room: from tu0 to tu1 along u, either side of the middle out to tv
    fn throne_room_box(&self) -> (f32, f32, f32) {
        (self.keep[0] + 9.5, self.keep[1] - 2.2, self.keep_half - 2.2)
    }

    /// Where the stair up to the throne room ends in the great hall, and where the pillars down
    /// its sides stand, along u
    fn hall_posts(&self) -> (f32, Vec<f32>) {
        let n = ((self.throne - self.ward) / RISE).round();
        let stair_end = self.keep[1] + 1.4 + n * RUN;
        let mut u = stair_end + 2.2;
        let mut posts = Vec::new();
        while u < -2.6 {
            posts.push(u);
            u += 3.8;
        }
        (stair_end, posts)
    }

    /// Where the princess stands: on the dais beside the throne
    fn princess(&self) -> Vec3 {
        let (tu0, ..) = self.throne_room_box();
        self.at.point(tu0 + 1.7, 1.5, self.throne + 3.0 * RISE + 0.0625)
    }

    fn royal(&self) -> Royal {
        let (tu0, tu1, _) = self.throne_room_box();
        let at = &self.at;
        Royal {
            middle: at.point((self.front + self.back) * 0.5, 0.0, self.ward),
            gate: at.point(self.front + 2.0, 0.0, self.gate_floor),
            out: at.dir([1.0, 0.0]),
            tower: at.point(self.keep[0] + 3.9, 0.0, self.keep_top + 6.0),
            hall: at.point(-3.0, 0.0, self.ward),
            dais: at.point(self.keep[1] + 1.4, 0.0, self.throne),
            throne: at.point(tu1 - 1.2, -1.6, self.throne),
            princess: at.point(tu0 + 1.7, 1.5, self.throne + 3.0 * RISE + 0.0625),
        }
    }

    /// Builds the castle: what's inside the rooms first, then the rooms and passages cut through
    /// the walls, the towers, bridges, buildings and walls, the ways in, the terrace under it all,
    /// and the trees in its gardens, so each wins over what comes after
    fn build(&self, t: &Terrain, features: &mut Features) -> Vec<Object> {
        debug_assert!(self.towers.iter().all(|t| t.top < self.sky));
        let mut rng = Rng(hash(self.seed ^ 0xb11d));
        let mut objects = Vec::new();
        let mut done = |s: &mut Vec<Shape>, rng: &mut Rng| {
            if !s.is_empty() {
                objects.push(Object::new(std::mem::take(s), hash(rng.0 ^ objects.len() as u32)));
            }
        };
        let mut s = Vec::new();
        self.throne_room(t, &mut s, features);
        done(&mut s, &mut rng);
        self.hall_inside(t, &mut s, features, &mut rng);
        done(&mut s, &mut rng);
        self.gate_fittings(t, &mut s, features);
        done(&mut s, &mut rng);
        self.rooms(&mut s);
        done(&mut s, &mut rng);
        for tower in &self.towers {
            self.tower(tower, &mut s, &mut rng);
            done(&mut s, &mut rng);
        }
        for bridge in &self.bridges {
            self.bridge(bridge, &mut s, &mut rng);
            done(&mut s, &mut rng);
        }
        self.keep(&mut s, &mut rng);
        done(&mut s, &mut rng);
        self.hall(t, &mut s, features, &mut rng);
        done(&mut s, &mut rng);
        for side in [-1.0, 1.0] {
            self.wing(side, &mut s, &mut rng);
            done(&mut s, &mut rng);
        }
        self.front_range(&mut s, &mut rng);
        done(&mut s, &mut rng);
        self.gatehouse(t, &mut s, features);
        done(&mut s, &mut rng);
        for k in 0..4 {
            self.curtain(k, &mut s, &mut rng);
            done(&mut s, &mut rng);
        }
        self.ways(t, &mut s, features);
        done(&mut s, &mut rng);
        self.courtyard(t, &mut s, features);
        done(&mut s, &mut rng);
        self.terrace(&mut s);
        done(&mut s, &mut rng);
        let trees = self.trees(t, &mut rng);
        objects.extend(trees);

        // Nothing else grows inside, and no grass is drawn in the shader where it's built over
        let b = self.bounds;
        features.footprints.push([b[0], b[1], b[2], b[3], 0.0]);
        let at = &self.at;
        let mut hole = |u: [f32; 2], v: [f32; 2]| {
            let (lo, hi) = at.rect(u, v);
            features.holes.push([lo[0], lo[1], hi[0], hi[1]]);
        };
        hole([self.keep[0] - 0.5, self.range + 0.5], [-self.ward_half - 0.5, self.ward_half + 0.5]);
        hole([self.front - 3.5, self.front + 0.5], [-self.side - 0.5, self.side + 0.5]);
        hole([self.back - 0.5, self.back + 3.5], [-self.side - 0.5, self.side + 0.5]);
        for side in [-1.0, 1.0] {
            hole([self.back - 0.5, self.front + 0.5], [side * (self.side + 0.5), side * (self.side - 3.5)]);
        }
        hole([self.range, self.front + 16.0], [-4.8, 4.8]);
        for tower in &self.towers {
            if tower.seen < self.ward - 0.5 {
                hole([tower.c[0] - tower.r - 0.8, tower.c[0] + tower.r + 0.8], [tower.c[1] - tower.r - 0.8, tower.c[1] + tower.r + 0.8]);
            }
        }

        // Where the princess, her guards and her court are, and the castle itself
        let mut site = |kind: &'static str, p: Vec3, facing: [f32; 2]| {
            if t.inside(p[0], p[1]) {
                features.sites.push(Site { kind, position: p, facing: at.dir(facing) });
            }
        };
        let (tu0, ..) = self.throne_room_box();
        site("royal castle", at.point(self.front + 2.0, 0.0, self.gate_floor), [1.0, 0.0]);
        site("royal court", at.point(self.court * 0.5 + 4.5, 0.0, self.ward), [1.0, 0.0]);
        site("royal stall", self.princess(), [1.0, 0.0]);
        for side in [-1.0, 1.0] {
            site("royal guard", at.point(self.front + 2.4, side * 3.4, self.gate_floor), [1.0, 0.0]);
            site("royal guard", at.point(1.5, side * 3.4, self.ward), [1.0, 0.0]);
            site("royal guard", at.point(tu0 + 5.6, side * 5.4, self.throne), [1.0, 0.0]);
        }
        objects
    }

    /// A spire on a round tower round `c`, `r` across, from `base` up `h`: over an eave, hollow
    /// inside, a gold finial at its tip or a flag flying from it
    #[allow(clippy::too_many_arguments)]
    fn spire(&self, s: &mut Vec<Shape>, c: [f32; 2], base: f32, r: f32, h: f32, flag: bool) {
        let at = &self.at;
        let tip = base + h;
        if flag {
            s.push(bl(at, [c[0] - 0.03, c[0] + 0.03], [c[1] + 0.06, c[1] + 1.5], [tip + 1.05, tip + 1.85], (self.stones.banner, AIR)));
            s.push(up(at, c, [tip - 0.4, tip + 1.9], [0.06, 0.04], (DARK_STEEL, AIR)));
        } else {
            s.push(Shape::sphere(at.point(c[0], c[1], tip + 0.35), 0.24, GOLD, AIR, false));
            s.push(up(at, c, [tip - 0.6, tip + 1.6], [0.1, 0.02], (GOLD, AIR)));
        }
        s.push(blob(at, [c[0], c[1], base + 0.1], [r + 0.42, r + 0.42, 0.32], self.stones.roof));
        if r > 2.0 && h > 4.0 {
            s.push(up(at, c, [base + 0.9, tip - 2.2], [r - 0.85, 0.15], (AIR, AIR)));
        }
        s.push(up(at, c, [base, tip], [r + 0.12, 0.06], self.stones.roof));
        // What of the tower's rounded top would poke out round the foot of the spire
        s.push(at.room([c[0] - r - 0.2, c[0] + r + 0.2], [c[1] - r - 0.2, c[1] + r + 0.2], [base, base + 0.75 * r]));
    }

    /// Tower `t`: its crown, bands of trim round it, windows at every level, hollow inside its
    /// round walls, sloping out at its foot where it stands on the ground
    fn tower(&self, t: &Tower, s: &mut Vec<Shape>, rng: &mut Rng) {
        let at = &self.at;
        let st = self.stones;
        let (c, r) = (t.c, t.r);
        let body_top = match t.crown {
            Crown::Spire(h) | Crown::Corbelled(h) => {
                self.spire(s, c, t.top, r, h, t.flag);
                t.top
            }
            Crown::Battlements => {
                let n = ((TAU * r / 1.7).round() as usize).max(6);
                for k in 0..n {
                    let a = k as f32 * TAU / n as f32;
                    let m = [c[0] + a.cos() * (r - 0.1), c[1] + a.sin() * (r - 0.1)];
                    s.push(bricks(at, [m[0] - 0.36, m[0] + 0.36], [m[1] - 0.36, m[1] + 0.36], [t.top + 0.5, t.top + 1.5], st.wall));
                }
                if r > 3.0 {
                    // A turret in the middle, with a little spire of its own
                    let tr = r * 0.36;
                    self.spire(s, c, t.top + 3.2, tr, 3.0 * tr, t.flag);
                    s.push(up(at, c, [t.top, t.top + 3.2], [tr, tr], st.wall).patterned(Pattern::Bricks));
                }
                s.push(at.room([c[0] - r - 0.5, c[0] + r + 0.5], [c[1] - r - 0.5, c[1] + r + 0.5], [t.top + 0.5, t.top + 0.5 + r]));
                // The band of machicolations they stand out on
                s.push(blob(at, [c[0], c[1], t.top - 0.1], [r + 0.38, r + 0.38, 0.6], st.trim));
                t.top + 0.5
            }
        };
        let corbelled = matches!(t.crown, Crown::Corbelled(_));
        let foot = if corbelled {
            s.push(up(at, c, [t.foot, t.foot + 2.4], [0.25, r], st.trim));
            t.foot + 2.4
        } else {
            t.foot
        };
        let mut z = t.seen + 6.0;
        while z < body_top - 3.0 && !corbelled {
            s.push(blob(at, [c[0], c[1], z], [r + 0.16, r + 0.16, 0.2], st.trim));
            z += 8.5;
        }
        // Windows, lit or dark, looking a different few ways at each level
        let half = if r > 3.0 { 0.32 } else { 0.24 };
        let mut z = t.sill.max(foot + 3.0);
        let mut k = (rng.next() * 4.0) as usize;
        while z + 1.6 < body_top - 1.0 {
            for d in 0..4 {
                if rng.next() < 0.3 {
                    continue;
                }
                let glass = if rng.next() < t.lit { st.glass } else { OBSIDIAN };
                slit(s, at, c, r, WAYS[(d + k) % 4], z, 1.6, half, glass);
            }
            k += 1;
            z += 4.4;
        }
        // Hollow inside, so only its walls are kept
        if r > 2.4 && body_top - foot > 2.0 * r + 2.0 {
            s.push(up(at, c, [foot + r, body_top - r], [r - 0.75, r - 0.75], (AIR, AIR)));
        }
        s.push(up(at, c, [foot, body_top], [r, r], st.wall).patterned(Pattern::Bricks));
        if !corbelled && t.seen + 1.8 > foot + 0.5 {
            s.push(up(at, c, [foot, t.seen + 1.8], [r + 0.6, r], st.trim).patterned(Pattern::Bricks));
        }
    }

    /// Bridge `b`: a hollow gallery with windows down both sides under a pitched roof, on a band
    /// of trim
    fn bridge(&self, b: &Bridge, s: &mut Vec<Shape>, rng: &mut Rng) {
        let at = &self.at;
        let st = self.stones;
        let along_u = (b.a[0] - b.b[0]).abs() > (b.a[1] - b.b[1]).abs();
        let (l0, l1, m) = if along_u { (b.a[0].min(b.b[0]), b.a[0].max(b.b[0]), b.a[1]) } else { (b.a[1].min(b.b[1]), b.a[1].max(b.b[1]), b.a[0]) };
        let half = 1.6;
        let (z0, z1) = (b.z, b.z + 3.0);
        let rect = |l: [f32; 2], c: [f32; 2]| if along_u { (l, [m + c[0], m + c[1]]) } else { ([m + c[0], m + c[1]], l) };
        let mut l = l0 + 1.5;
        while l < l1 - 1.1 {
            for side in [-1.0, 1.0] {
                let face = if along_u { [l, m + side * half] } else { [m + side * half, l] };
                let out = if along_u { [0.0, side] } else { [side, 0.0] };
                let glass = if rng.next() < 0.55 { st.glass } else { OBSIDIAN };
                window(s, at, face, out, 0.5, 0.32, [z0 + 0.9, z0 + 2.3], glass, st.trim);
            }
            l += 2.2;
        }
        let across = half + 0.35;
        pitched(s, at, along_u, [l0 - 0.2, l1 + 0.2], m, across, z1 - 0.2, 0.8, st.roof, st.trim.0);
        for end in [[l0, l0 + 0.4], [l1 - 0.4, l1]] {
            gable_end(s, at, along_u, end, m, half, z1, across, z1 - 0.2, 0.8, st.wall);
        }
        let (u, v) = rect([l0 + 0.4, l1 - 0.4], [-half + 0.4, half - 0.4]);
        s.push(bl(at, u, v, [z0 + 0.4, z1 - 0.4], (AIR, AIR)));
        let (u, v) = rect([l0, l1], [-half, half]);
        s.push(bricks(at, u, v, [z0, z1], st.wall));
        let (u, v) = rect([l0, l1], [-half - 0.2, half + 0.2]);
        s.push(bl(at, u, v, [z0 - 0.4, z0 + 0.12], st.trim));
        // Carried on corbels
        let mut l = l0 + 1.0;
        while l < l1 - 0.6 {
            for side in [-1.0, 1.0] {
                let c = if along_u { [l, m + side * (half - 0.1)] } else { [m + side * (half - 0.1), l] };
                s.push(up(at, c, [z0 - 1.3, z0 - 0.35], [0.08, 0.32], st.trim));
            }
            l += 1.8;
        }
    }

    /// A brazier standing at `c` on `floor`, burning
    fn brazier(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features, c: [f32; 2], floor: f32) {
        let at = &self.at;
        s.push(blob(at, [c[0], c[1], floor + 1.36], [0.26, 0.26, 0.3], (FLAME, AIR)));
        s.push(blob(at, [c[0], c[1], floor + 1.12], [0.46, 0.46, 0.17], (BRONZE, AIR)));
        s.push(up(at, c, [floor, floor + 1.05], [0.13, 0.07], (BRONZE, DARK_STEEL)));
        s.push(bl(at, [c[0] - 0.3, c[0] + 0.3], [c[1] - 0.3, c[1] + 0.3], [floor - 0.05, floor + 0.1], (DARK_STEEL, AIR)));
        let p = at.point(c[0], c[1], floor + 2.0);
        if t.inside(p[0], p[1]) {
            features.lights.push(Light { position: p, color: FIRE, reach: 13.0 });
        }
    }

    /// A torch in an iron bracket on a wall facing `out`, its face at `face`, `z` high
    fn torch(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features, face: [f32; 2], out: [f32; 2], z: f32) {
        let at = &self.at;
        let p = |d: f32, z: f32| [face[0] + out[0] * d, face[1] + out[1] * d, z];
        s.push(blob(at, p(0.3, z + 0.56), [0.08, 0.08, 0.14], (FLAME, AIR)));
        s.push(rod(at, p(0.12, z), p(0.3, z + 0.42), 0.05, (WOOD, BARK_DARK)));
        s.push(rod(at, p(-0.1, z + 0.05), p(0.2, z + 0.05), 0.035, (DARK_STEEL, AIR)));
        let light = at.point(face[0] + out[0] * 0.55, face[1] + out[1] * 0.55, z + 0.95);
        if t.inside(light[0], light[1]) {
            features.lights.push(Light { position: light, color: FIRE, reach: 9.0 });
        }
    }

    /// A ring of candles hanging on a chain at `z` from `ceiling`, lighting up the room
    #[allow(clippy::too_many_arguments)]
    fn chandelier(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features, c: [f32; 2], z: f32, ceiling: f32, reach: f32) {
        let at = &self.at;
        for k in 0..8 {
            let a = k as f32 * TAU / 8.0;
            s.push(blob(at, [c[0] + a.cos() * 1.0, c[1] + a.sin() * 1.0, z + 0.14], [0.06, 0.06, 0.11], (FLAME, AIR)));
        }
        s.push(blob(at, [c[0], c[1], z], [1.1, 1.1, 0.07], (DARK_STEEL, GOLD)));
        s.push(up(at, c, [z, ceiling + 0.2], [0.04, 0.04], (DARK_STEEL, AIR)));
        let p = at.point(c[0], c[1], z - 0.5);
        if t.inside(p[0], p[1]) {
            features.lights.push(Light { position: p, color: [1.0, 0.7, 0.4], reach });
        }
    }

    /// A lantern hanging at `p`, with its flame in a bronze frame
    fn lantern(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features, p: Vec3, reach: f32) {
        let at = &self.at;
        let [u, v, z] = p;
        let b = |du: [f32; 2], dv: [f32; 2], dz: [f32; 2], m: u8| bl(at, [u + du[0], u + du[1]], [v + dv[0], v + dv[1]], [z + dz[0], z + dz[1]], (m, AIR));
        s.push(b([-0.06, 0.06], [-0.06, 0.06], [0.19, 0.27], BRONZE));
        s.push(b([-0.14, 0.14], [-0.14, 0.14], [0.13, 0.19], BRONZE));
        for (du, dv) in [(-0.11, -0.11), (0.11, -0.11), (-0.11, 0.11), (0.11, 0.11)] {
            s.push(b([du - 0.025, du + 0.025], [dv - 0.025, dv + 0.025], [-0.13, 0.13], DARK_STEEL));
        }
        s.push(b([-0.085, 0.085], [-0.085, 0.085], [-0.11, 0.11], FLAME));
        s.push(b([-0.13, 0.13], [-0.13, 0.13], [-0.18, -0.12], BRONZE));
        let light = at.point(u, v, z - 0.35);
        if t.inside(light[0], light[1]) {
            features.lights.push(Light { position: light, color: FIRE, reach });
        }
    }

    /// A lamp post standing at `c` on the ground, with a lantern hung from an arm reaching `out`
    fn lamp_post(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features, c: [f32; 2], out: [f32; 2]) {
        let at = &self.at;
        let z = self.ground(c[0], c[1]);
        let top = z + 3.0;
        let tip = [c[0] + out[0] * 0.6, c[1] + out[1] * 0.6];
        s.push(bl(at, [c[0].min(tip[0]) - 0.04, c[0].max(tip[0]) + 0.04], [c[1].min(tip[1]) - 0.04, c[1].max(tip[1]) + 0.04], [top - 0.12, top - 0.04], (DARK_STEEL, AIR)));
        s.push(bl(at, [tip[0] - 0.015, tip[0] + 0.015], [tip[1] - 0.015, tip[1] + 0.015], [top - 0.2, top - 0.12], (DARK_STEEL, AIR)));
        s.push(bl(at, [c[0] - 0.08, c[0] + 0.08], [c[1] - 0.08, c[1] + 0.08], [z - 0.4, top], (DARK_STEEL, AIR)));
        s.push(bl(at, [c[0] - 0.18, c[0] + 0.18], [c[1] - 0.18, c[1] + 0.18], [z - 0.3, z + 0.2], self.stones.trim));
        self.lantern(t, s, features, [tip[0], tip[1], top - 0.47], 12.0);
    }

    /// A banner hanging on a wall facing `out` from a gold rod, its middle on the wall at `face`,
    /// `half` wide, from z[0] up to z[1]
    fn banner(&self, s: &mut Vec<Shape>, face: [f32; 2], out: [f32; 2], half: f32, z: [f32; 2]) {
        let p = self.at.point(face[0], face[1], 0.0);
        let w = Frame { x: p[0], y: p[1], f: self.at.dir(out) };
        let gold = (GOLD, AIR);
        s.push(Shape::capsule(w.point(0.18, -half - 0.25, z[1]), w.point(0.18, half + 0.25, z[1]), 0.06, 0.06, GOLD, AIR, false));
        s.push(w.block([0.0, 0.15], [-half * 0.2, half * 0.2], [z[0] + 0.3, z[1] - 0.15], gold.0, gold.1));
        s.push(w.block([0.0, 0.13], [-half, half], [z[0] + 0.3, z[1]], self.stones.banner, AIR));
        // Cut to a point at the bottom
        s.push(w.block([0.0, 0.13], [-half * 0.55, half * 0.55], [z[0], z[0] + 0.3], self.stones.banner, AIR));
    }

    /// What's in the throne room: the dais with the throne on it and a great banner behind,
    /// braziers either side, pillars, a carpet up to it, banners on the walls, a chandelier and
    /// torches either side of the way in
    fn throne_room(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features) {
        let at = &self.at;
        let st = self.stones;
        let (u0, u1, hv) = self.throne_room_box();
        let (f, ceiling) = (self.throne, self.throne + 9.5);
        let gold = (GOLD, AIR);
        let top = f + 3.0 * RISE;
        s.push(bl(at, [u0, u0 + 2.2], [-3.4, 3.4], [top, top + 0.0625], (CLOTH, AIR)));
        for j in 0..3 {
            let j = j as f32;
            s.push(bl(at, [u0, u0 + 4.2 - j], [-(5.0 - 0.7 * j), 5.0 - 0.7 * j], [f - 0.3, f + (j + 1.0) * RISE], st.trim));
        }
        // The throne, of gold, cushioned in red
        let (d, ut) = (top + 0.0625, u0 + 1.0);
        s.push(Shape::sphere(at.point(ut - 0.5, 0.0, d + 2.95), 0.2, GOLD, AIR, false));
        for v in [-0.6, 0.6] {
            s.push(Shape::sphere(at.point(ut - 0.5, v, d + 2.5), 0.11, GOLD, AIR, false));
            s.push(Shape::sphere(at.point(ut + 0.45, v * 0.97, d + 0.9), 0.08, GOLD, AIR, false));
        }
        s.push(bl(at, [ut - 0.43, ut - 0.37], [-0.42, 0.42], [d + 0.75, d + 2.35], (st.banner, AIR)));
        s.push(bl(at, [ut - 0.62, ut - 0.38], [-0.62, 0.62], [d, d + 2.75], gold));
        s.push(bl(at, [ut - 0.4, ut + 0.42], [-0.48, 0.48], [d + 0.48, d + 0.6], (CLOTH, AIR)));
        for v in [-1.0, 1.0] {
            s.push(bl(at, [ut - 0.4, ut + 0.48], [v * 0.5, v * 0.66], [d + 0.48, d + 0.86], gold));
        }
        s.push(bl(at, [ut - 0.45, ut + 0.48], [-0.62, 0.62], [d, d + 0.48], gold));
        // The great banner behind it, with the arms of the house on it
        s.push(blob(at, [u0 + 0.12, 0.0, f + 6.4], [0.06, 0.6, 0.7], gold));
        s.push(bl(at, [u0 - 0.05, u0 + 0.1], [-1.9, 1.9], [f + 3.2, f + 8.6], (st.banner, AIR)));
        s.push(bl(at, [u0 - 0.05, u0 + 0.06], [-2.1, 2.1], [f + 3.0, f + 8.8], gold));
        s.push(rod(at, [u0 + 0.16, -2.4, f + 8.75], [u0 + 0.16, 2.4, f + 8.75], 0.06, gold));
        for v in [-3.8, 3.8] {
            self.brazier(t, s, features, [u0 + 5.0, v], f);
        }
        // Pillars down both sides, banners on the walls behind them
        let middle = (u0 + u1) * 0.5 + 1.0;
        let pv = hv - 2.0;
        for u in [u0 + 3.2, middle, u1 - 1.8] {
            for v in [-pv, pv] {
                s.push(bl(at, [u - 0.62, u + 0.62], [v - 0.62, v + 0.62], [ceiling - 0.4, ceiling + 0.2], st.trim));
                s.push(bl(at, [u - 0.65, u + 0.65], [v - 0.65, v + 0.65], [f - 0.3, f + 0.45], st.trim));
                s.push(up(at, [u, v], [f, ceiling], [0.45, 0.45], st.trim));
            }
            for side in [-1.0, 1.0] {
                self.banner(s, [u, side * hv], [0.0, -side], 0.55, [f + 3.0, f + 7.4]);
            }
        }
        // The carpet from the way in up to the dais, edged in gold
        s.push(bl(at, [u0 + 4.2, self.keep[1] + 0.2], [-1.3, 1.3], [f, f + 0.0625], (CLOTH, AIR)));
        for v in [-1.0, 1.0] {
            s.push(bl(at, [u0 + 4.2, self.keep[1] + 0.2], [v * 1.3, v * 1.45], [f, f + 0.0625], gold));
        }
        self.chandelier(t, s, features, [(u0 + 4.2 + u1) * 0.5, 0.0], ceiling - 2.4, ceiling, 16.0);
        for v in [-3.4, 3.4] {
            self.torch(t, s, features, [u1, v], [-1.0, 0.0], f + 2.6);
        }
    }

    /// What's in the great hall: the stair up to the throne room at its far end, pillars down both
    /// sides holding up the beams of its open roof, long tables with benches and candles, a carpet
    /// up the middle, banners, chandeliers, and the doors standing open
    fn hall_inside(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features, rng: &mut Rng) {
        let at = &self.at;
        let st = self.stones;
        let (u0, u1, hv) = (self.keep[1] + 1.4, -1.4, self.hall_half - 1.4);
        let (w, eaves) = (self.ward, self.hall_eaves);
        let ridge = eaves - 0.25 + (self.hall_half + 0.6) * HALL_PITCH;
        let n = ((self.throne - w) / RISE).round() as usize;
        for k in 0..n {
            let top = self.throne - k as f32 * RISE;
            let (a, b) = (u0 + k as f32 * RUN, u0 + (k + 1) as f32 * RUN);
            s.push(bl(at, [a, b], [-3.0, 3.0], [w - 0.3, top], st.trim));
            for v in [-1.0, 1.0] {
                s.push(bl(at, [a, b], [v * 3.0, v * 3.5], [w - 0.3, top + 0.95], st.trim));
            }
        }
        // Pillars, the beams over them, and posts up from those to the ridge
        let pv = hv - 1.6;
        let (stair_end, posts) = self.hall_posts();
        for &u in &posts {
            for v in [-pv, pv] {
                s.push(bl(at, [u - 0.55, u + 0.55], [v - 0.55, v + 0.55], [w - 0.3, w + 0.4], st.trim));
                s.push(up(at, [u, v], [w, eaves + 0.3], [0.42, 0.42], st.trim));
            }
            s.push(rod(at, [u, -hv - 0.2, eaves - 0.1], [u, hv + 0.2, eaves - 0.1], 0.22, (WOOD, BARK_DARK)));
            s.push(up(at, [u, 0.0], [eaves - 0.1, ridge - 0.3], [0.17, 0.17], (WOOD, BARK_DARK)));
            for v in [-1.0, 1.0] {
                s.push(rod(at, [u, v * pv, eaves - 0.1], [u, 0.0, (eaves + ridge) * 0.5], 0.13, (WOOD, BARK_DARK)));
            }
            for side in [-1.0, 1.0] {
                self.banner(s, [u, side * hv], [0.0, -side], 0.5, [w + 2.6, eaves - 0.9]);
            }
        }
        // The long tables, with platters and goblets down them
        let (ta, tb) = (stair_end + 1.8, u1 - 2.6);
        for v in [-3.4, 3.4] {
            let mut u = ta + 0.8;
            while u < tb - 0.6 {
                s.push(up(at, [u, v + 0.25], [w + 0.82, w + 1.0], [0.04, 0.05], (GOLD, AIR)));
                s.push(blob(at, [u, v - 0.2, w + 0.84], [0.17, 0.17, 0.02], (BRONZE, AIR)));
                u += 1.6;
            }
            s.push(bl(at, [ta, tb], [v - 0.6, v + 0.6], [w + 0.72, w + 0.82], (WOOD, AIR)));
            for u in [ta + 0.3, (ta + tb) * 0.5, tb - 0.3] {
                s.push(bl(at, [u - 0.1, u + 0.1], [v - 0.45, v + 0.45], [w, w + 0.72], (BARK_DARK, AIR)));
            }
            for side in [-1.0, 1.0] {
                let b = v + side * 0.95;
                s.push(bl(at, [ta + 0.2, tb - 0.2], [b - 0.18, b + 0.18], [w, w + 0.45], (WOOD, BARK_DARK)));
            }
            for u in [ta + (tb - ta) * 0.3, ta + (tb - ta) * 0.7] {
                let p = at.point(u, v, w + 0.82);
                if t.inside(p[0], p[1]) {
                    candle(features, p, rng);
                }
            }
        }
        // The carpet from the door to the stair
        s.push(bl(at, [stair_end, 0.0], [-1.3, 1.3], [w, w + 0.0625], (CLOTH, AIR)));
        for v in [-1.0, 1.0] {
            s.push(bl(at, [stair_end, 0.0], [v * 1.3, v * 1.45], [w, w + 0.0625], (GOLD, AIR)));
        }
        for k in [0.28, 0.72] {
            self.chandelier(t, s, features, [ta + (tb - ta) * k, 0.0], eaves - 0.9, ridge - 0.4, 14.0);
        }
        // The doors, open against the wall either side of the doorway, and torches by them
        for side in [-1.0, 1.0] {
            s.push(bl(at, [u1 - 0.14, u1], [side * 2.2, side * 4.2], [w, w + 4.0], (WOOD, BARK_DARK)));
            self.torch(t, s, features, [u1, side * 5.0], [-1.0, 0.0], w + 2.6);
        }
    }

    /// The gate's fittings: its portcullis drawn up, the gates standing open against the sides of
    /// the way through, and a lantern hanging in the middle of it
    fn gate_fittings(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features) {
        let at = &self.at;
        let (f, g) = (self.front, self.gate_floor);
        let iron = (DARK_STEEL, AIR);
        let pu = f + 0.9;
        for k in 0..9 {
            let v = -1.9 + k as f32 * 0.475;
            s.push(rod(at, [pu, v, g + 3.6], [pu, v, g + 6.6], 0.06, iron));
        }
        for z in [g + 4.3, g + 5.5] {
            s.push(rod(at, [pu, -2.1, z], [pu, 2.1, z], 0.07, iron));
        }
        for side in [-1.0, 1.0] {
            for z in [g + 0.8, g + 3.2] {
                s.push(bl(at, [f - 2.7, f - 0.5], [side * 2.0, side * 2.06], [z, z + 0.16], iron));
            }
            s.push(bl(at, [f - 2.7, f - 0.5], [side * 2.04, side * 2.2], [g, g + 4.1], (WOOD, BARK_DARK)));
        }
        let lu = f - 3.2;
        s.push(rod(at, [lu, 0.0, g + 4.25], [lu, 0.0, g + 6.8], 0.03, iron));
        self.lantern(t, s, features, [lu, 0.0, g + 4.0], 10.0);
    }

    /// The rooms and passages cut through the walls, with their floors: the way through the
    /// gatehouse and the archway through the front range, the great hall and its doorway, the
    /// archway from it into the keep and the throne room in there
    fn rooms(&self, s: &mut Vec<Shape>) {
        let at = &self.at;
        let st = self.stones;
        let w = self.ward;
        let arch = |s: &mut Vec<Shape>, u: [f32; 2], z: f32, r: f32| s.push(rod(at, [u[0], 0.0, z], [u[1], 0.0, z], r, (AIR, AIR)).placed(Placement::Clear));
        let (f, g) = (self.front, self.gate_floor);
        s.push(at.room([f - 7.6, f + 1.6], [-2.2, 2.2], [g, g + 4.6]));
        arch(s, [f - 7.6, f + 1.6], g + 4.6, 2.2);
        s.push(bl(at, [f - 7.6, f + 1.6], [-2.2, 2.2], [g - 0.5, g], st.paving[1]).placed(Placement::Clear));
        s.push(at.room([self.court - 0.2, self.range + 0.2], [-3.0, 3.0], [w, w + 4.6]));
        arch(s, [self.court, self.range], w + 4.6, 3.0);
        s.push(at.room([-1.6, 0.3], [-2.2, 2.2], [w, w + 4.4]));
        arch(s, [-1.6, 0.3], w + 4.4, 2.2);
        let hv = self.hall_half - 1.4;
        s.push(at.room([self.keep[1] + 1.4, -1.4], [-hv, hv], [w, self.hall_eaves - 0.3]));
        s.push(bl(at, [self.keep[1] + 1.4, -1.4], [-hv, hv], [w - 0.3, w], st.paving[0]).placed(Placement::Clear));
        let (tu0, tu1, tv) = self.throne_room_box();
        let tf = self.throne;
        s.push(at.room([tu1 - 0.2, self.keep[1] + 1.6], [-2.4, 2.4], [tf, tf + 4.6]));
        arch(s, [tu1, self.keep[1] + 1.4], tf + 4.6, 2.4);
        s.push(bl(at, [tu1 - 0.2, self.keep[1] + 1.4], [-2.4, 2.4], [tf - 0.3, tf], st.trim).placed(Placement::Clear));
        s.push(at.room([tu0, tu1], [-tv, tv], [tf, tf + 9.5]));
        // The throne room's floor, in squares of pale and dark stone
        let (nu, nv) = (((tu1 - tu0) / 2.0).round().max(1.0), ((2.0 * tv) / 2.0).round().max(1.0));
        let (du, dv) = ((tu1 - tu0) / nu, 2.0 * tv / nv);
        for i in 0..nu as usize {
            for j in 0..nv as usize {
                let m = if (i + j) % 2 == 0 { (st.wall.0, AIR) } else { (OBSIDIAN, AIR) };
                let (a, b) = (tu0 + i as f32 * du, -tv + j as f32 * dv);
                s.push(bl(at, [a, a + du], [b, b + dv], [tf - 0.3, tf], m).placed(Placement::Clear));
            }
        }
    }

    /// The keep: battlements round its roof on a corbelled band, the throne room's tall windows
    /// lit down its sides, rows of smaller ones above and round the back, banners, solid inside
    /// round the throne room
    fn keep(&self, s: &mut Vec<Shape>, rng: &mut Rng) {
        let at = &self.at;
        let st = self.stones;
        let (u0, u1, kh, top) = (self.keep[0], self.keep[1], self.keep_half, self.keep_top);
        let w = self.ward;
        let (tu0, tu1, tv) = self.throne_room_box();
        let f = self.throne;
        battlements(s, at, [u1 - 0.6, u1 + 0.35], [-kh - 0.35, kh + 0.35], top, st.wall);
        battlements(s, at, [u0 - 0.35, u0 + 0.6], [-kh - 0.35, kh + 0.35], top, st.wall);
        for side in [-1.0, 1.0] {
            let v = if side < 0.0 { [-kh - 0.35, -kh + 0.6] } else { [kh - 0.6, kh + 0.35] };
            battlements(s, at, [u0 - 0.35, u1 + 0.35], v, top, st.wall);
            let band = if side < 0.0 { [-kh - 0.35, -kh + 0.65] } else { [kh - 0.65, kh + 0.35] };
            s.push(bl(at, [u0 - 0.35, u1 + 0.35], band, [top - 0.9, top], st.trim));
        }
        s.push(bl(at, [u1 - 0.65, u1 + 0.35], [-kh, kh], [top - 0.9, top], st.trim));
        s.push(bl(at, [u0 - 0.35, u0 + 0.65], [-kh, kh], [top - 0.9, top], st.trim));
        // The throne room's windows, between its pillars
        let middle = (tu0 + tu1) * 0.5 + 1.0;
        for side in [-1.0, 1.0] {
            for u in [(tu0 + 3.2 + middle) * 0.5, (middle + tu1 - 1.8) * 0.5] {
                window(s, at, [u, side * kh], [0.0, side], kh - tv, 0.66, [f + 2.4, f + 7.6], st.glass, st.trim);
            }
            // Smaller ones above, and round the back either side of the great tower
            let mut u = u0 + 4.0;
            while u < u1 - 3.0 {
                let glass = if rng.next() < 0.5 { st.glass } else { OBSIDIAN };
                window(s, at, [u, side * kh], [0.0, side], 0.7, 0.45, [top - 3.8, top - 1.5], glass, st.trim);
                u += 3.6;
            }
            for z in [[w + 4.0, w + 6.6], [top - 3.8, top - 1.5]] {
                let glass = if rng.next() < 0.5 { st.glass } else { OBSIDIAN };
                window(s, at, [u0, side * (kh - 3.4)], [-1.0, 0.0], 0.7, 0.45, z, glass, st.trim);
            }
            self.banner(s, [u0, side * 7.2], [-1.0, 0.0], 0.7, [w + 3.5, top - 4.5]);
            // Its front corners, beyond the roof of the great hall
            if kh - 2.6 > self.hall_half + 1.6 {
                window(s, at, [u1, side * (kh - 2.4)], [1.0, 0.0], 0.7, 0.45, [top - 3.8, top - 1.5], st.glass, st.trim);
            }
        }
        // Solid stone inside round the throne room, so only the walls are kept
        let core = (st.wall.0, AIR);
        s.push(bl(at, [u0 + 0.6, tu0 - 0.6], [-kh + 0.6, kh - 0.6], [w + 0.2, top - 0.6], core));
        if top - 0.6 > f + 10.3 {
            s.push(bl(at, [tu0 - 0.6, u1 - 0.6], [-kh + 0.6, kh - 0.6], [f + 10.3, top - 0.6], core));
        }
        for side in [-1.0, 1.0] {
            s.push(bl(at, [tu0, tu1], [side * (tv + 0.6), side * (kh - 0.6)], [f, f + 9.5], core));
        }
        s.push(bricks(at, [u0, u1], [-kh, kh], [w - 0.4, top], st.wall));
    }

    /// The great hall: its walls with tall windows down both sides and buttresses between them,
    /// a rose window over its door, banners and torches either side of that, its roof open inside
    /// and a slender spire on the ridge
    fn hall(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features, rng: &mut Rng) {
        let at = &self.at;
        let st = self.stones;
        let (u0, u1, hh) = (self.keep[1], 0.0, self.hall_half);
        let (w, eaves) = (self.ward, self.hall_eaves);
        let across = hh + 0.6;
        let roof_eaves = eaves - 0.25;
        let rise = across * HALL_PITCH;
        let ridge = roof_eaves + rise;
        let mid = (u0 + u1) * 0.5;
        self.spire(s, [mid, 0.0], ridge + 1.6, 1.2, 6.5, false);
        s.push(bricks(at, [mid - 1.2, mid + 1.2], [-1.2, 1.2], [ridge - 1.5, ridge + 1.6], st.wall));
        // The rose window, and windows either side of the door under it
        let rz = eaves + rise * 0.35;
        s.push(blob(at, [-0.5, 0.0, rz], [1.0, 1.5, 1.5], (st.glass, AIR)));
        s.push(blob(at, [0.0, 0.0, rz], [0.36, 1.85, 1.85], st.trim));
        for side in [-1.0, 1.0] {
            window(s, at, [0.0, side * hh * 0.64], [1.0, 0.0], 1.4, 0.55, [w + 2.0, w + 5.8], st.glass, st.trim);
            self.banner(s, [0.0, side * 3.3], [1.0, 0.0], 0.42, [w + 4.6, eaves - 0.6]);
            self.torch(t, s, features, [0.0, side * 3.3], [1.0, 0.0], w + 2.6);
        }
        // Between the pillars inside, with buttresses outside where they stand
        let (_, posts) = self.hall_posts();
        let mut u = posts.first().copied().unwrap_or(u0 + 6.0) - 1.9;
        while u < u1 - 2.4 {
            for side in [-1.0, 1.0] {
                let glass = if rng.next() < 0.75 { st.glass } else { OBSIDIAN };
                window(s, at, [u, side * hh], [0.0, side], 1.4, 0.62, [w + 2.4, eaves - 1.0], glass, st.trim);
                let b = u + 1.9;
                if b < u1 - 1.0 {
                    s.push(bl(at, [b - 0.45, b + 0.45], [side * hh, side * (hh + 0.5)], [eaves - 2.5, eaves - 0.4], st.trim));
                    s.push(bricks(at, [b - 0.5, b + 0.5], [side * hh, side * (hh + 0.95)], [w - 0.4, eaves - 2.5], st.trim));
                }
            }
            u += 3.8;
        }
        pitched(s, at, true, [u0 - 0.3, u1 + 0.35], 0.0, across, roof_eaves, HALL_PITCH, st.roof, st.trim.0);
        // The space under the roof, open over the hall
        let mut z = eaves - 0.3;
        loop {
            let half = (across * (1.0 - (z + 0.5 - roof_eaves) / rise) - 0.5).min(hh - 1.4);
            if half < 0.6 {
                break;
            }
            s.push(at.room([u0 + 1.4, u1 - 1.4], [-half, half], [z, z + 0.5]));
            z += 0.5;
        }
        for end in [[u1 - 1.4, u1], [u0, u0 + 1.4]] {
            gable_end(s, at, true, end, 0.0, hh, eaves, across, roof_eaves, HALL_PITCH, st.wall);
        }
        s.push(bricks(at, [u0, u1], [-hh, hh], [w - 0.4, eaves], st.wall));
        s.push(bl(at, [u0, u1 + 0.25], [-hh - 0.25, hh + 0.25], [w - 0.4, w + 0.5], st.trim));
    }

    /// One of the wings along the courtyard, on `side`: two rows of windows on both faces, a
    /// pitched roof with chimneys, solid inside
    fn wing(&self, side: f32, s: &mut Vec<Shape>, rng: &mut Rng) {
        let at = &self.at;
        let st = self.stones;
        let (inner, outer) = (side * self.court_half, side * (self.court_half + self.wing));
        let (u0, u1) = (0.0, self.court);
        let (w, eaves) = (self.ward, self.wing_eaves);
        let mid = side * (self.court_half + self.wing * 0.5);
        let across = self.wing * 0.5 + 0.45;
        let ridge = eaves - 0.25 + across * WING_PITCH;
        for u in [u1 * 0.3, u1 * 0.72] {
            let v = side * (self.court_half + self.wing * 0.72);
            s.push(bl(at, [u - 0.6, u + 0.6], [v - 0.6, v + 0.6], [ridge + 0.6, ridge + 0.85], st.trim));
            s.push(bricks(at, [u - 0.5, u + 0.5], [v - 0.5, v + 0.5], [eaves, ridge + 0.6], st.trim));
        }
        let rows = [[w + 1.8, w + 4.6], [w + 5.9, (eaves - 1.0).min(w + 8.8)]];
        let mut u = 4.0;
        while u < u1 - 3.5 {
            for z in rows {
                let glass = if rng.next() < 0.45 { st.glass } else { OBSIDIAN };
                window(s, at, [u, inner], [0.0, -side], 0.6, 0.5, z, glass, st.trim);
                // Clear of the tower at its back corner
                if u > 5.6 {
                    let glass = if rng.next() < 0.45 { st.glass } else { OBSIDIAN };
                    window(s, at, [u, outer], [0.0, side], 0.6, 0.5, z, glass, st.trim);
                }
            }
            u += 3.6;
        }
        pitched(s, at, true, [u0 - 0.3, u1 + 0.3], mid, across, eaves - 0.25, WING_PITCH, st.roof, st.trim.0);
        for end in [[u0, u0 + 0.6], [u1 - 0.6, u1]] {
            gable_end(s, at, true, end, mid, self.wing * 0.5, eaves, across, eaves - 0.25, WING_PITCH, st.wall);
        }
        let (a, b) = (inner.min(outer), inner.max(outer));
        s.push(bl(at, [u0 + 0.6, u1 - 0.6], [a + 0.6, b - 0.6], [w + 0.2, eaves - 0.6], (st.wall.0, AIR)));
        s.push(bricks(at, [u0, u1], [a, b], [w - 0.4, eaves], st.wall));
        s.push(bl(at, [u0, u1], [a - 0.25, b + 0.25], [w - 0.4, w + 0.5], st.trim));
    }

    /// The front range, across the front of the inner ward: battlements along both sides, the
    /// arms of the house over its archway, windows either side, solid inside
    fn front_range(&self, s: &mut Vec<Shape>, rng: &mut Rng) {
        let at = &self.at;
        let st = self.stones;
        let (u0, u1) = (self.court, self.range);
        let hv = self.court_half + self.wing;
        let (w, top) = (self.ward, self.range_top);
        battlements(s, at, [u1 - 0.6, u1], [-hv, hv], top, st.wall);
        battlements(s, at, [u0, u0 + 0.6], [-hv, hv], top, st.wall);
        let arms = (w + 7.6 + top) * 0.5 + 0.1;
        let tall = ((top - w - 7.6) * 0.5 - 0.15).clamp(0.6, 1.3);
        s.push(blob(at, [u1 + 0.06, 0.0, arms], [0.16, tall * 0.8, tall], (st.banner, GOLD)));
        s.push(blob(at, [u1 - 0.02, 0.0, arms], [0.12, tall * 0.8 + 0.18, tall + 0.18], (GOLD, AIR)));
        let rows = [[w + 1.6, w + 4.2], [w + 5.6, top - 1.6]];
        let mut v = 6.0;
        while v < hv - 5.8 {
            for side in [-1.0, 1.0] {
                for z in rows {
                    let glass = if rng.next() < 0.45 { st.glass } else { OBSIDIAN };
                    window(s, at, [u1, side * v], [1.0, 0.0], 0.6, 0.5, z, glass, st.trim);
                    if v < self.court_half - 3.4 {
                        let glass = if rng.next() < 0.45 { st.glass } else { OBSIDIAN };
                        window(s, at, [u0, side * v], [-1.0, 0.0], 0.6, 0.5, z, glass, st.trim);
                    }
                }
            }
            v += 3.6;
        }
        // Solid round the archway, so its sides are of brick, then solid stone inside
        s.push(bricks(at, [u0, u1], [-4.4, 4.4], [w - 0.4, top - 0.2], st.wall));
        s.push(bl(at, [u0 + 0.6, u1 - 0.6], [-hv + 0.6, hv - 0.6], [w + 0.2, top - 0.6], (st.wall.0, AIR)));
        s.push(bricks(at, [u0, u1], [-hv, hv], [w - 0.4, top], st.wall));
        s.push(bl(at, [u0 - 0.25, u1 + 0.25], [-hv, hv], [w - 0.4, w + 0.5], st.trim));
    }

    /// The gatehouse over the way in: battlements round its top, a banner over the archway,
    /// torches either side of it
    fn gatehouse(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features) {
        let at = &self.at;
        let st = self.stones;
        let (u0, u1, gv) = (self.front - 7.5, self.front + 1.5, 4.4);
        let top = self.wall_top + 3.5;
        let g = self.gate_floor;
        let (low, _) = self.span([u0, u1], [-gv, gv], 2.5);
        battlements(s, at, [u1 - 0.6, u1], [-gv, gv], top, st.wall);
        battlements(s, at, [u0, u0 + 0.6], [-gv, gv], top, st.wall);
        if top - 1.0 > g + 8.2 {
            self.banner(s, [u1, 0.0], [1.0, 0.0], 1.1, [g + 7.4, top - 0.9]);
        }
        for side in [-1.0, 1.0] {
            self.torch(t, s, features, [u1, side * 3.3], [1.0, 0.0], g + 2.9);
        }
        if top - 0.6 > g + 7.6 {
            s.push(bl(at, [u0 + 0.6, u1 - 0.6], [-gv + 0.6, gv - 0.6], [g + 7.6, top - 0.6], (st.wall.0, AIR)));
        }
        s.push(bricks(at, [u0, u1], [-gv, gv], [low - 0.6, top], st.wall));
    }

    /// Side `k` of the curtain wall, 0 the front, 1 the back, then either side: merlons along its
    /// outer edge on a corbelled band, a low parapet along the inside, arrow slits, solid inside
    fn curtain(&self, k: usize, s: &mut Vec<Shape>, rng: &mut Rng) {
        let at = &self.at;
        let st = self.stones;
        let (f, b, sd) = (self.front, self.back, self.side);
        let top = self.wall_top;
        // Where it goes, and which way is out
        let (u, v, out) = match k {
            0 => ([f - 3.0, f], [-sd, sd], [1.0, 0.0]),
            1 => ([b, b + 3.0], [-sd, sd], [-1.0, 0.0]),
            2 => ([b, f], [sd - 3.0, sd], [0.0, 1.0]),
            _ => ([b, f], [-sd, 3.0 - sd], [0.0, -1.0]),
        };
        let (low, _) = self.span(u, v, 5.0);
        let along_u = out[0] == 0.0;
        // A strip `a` to `b` in from the outer face, all along it
        let strip = |a: f32, b: f32| -> ([f32; 2], [f32; 2]) {
            let (o0, o1) = if out[0] + out[1] > 0.0 { (-b, -a) } else { (a, b) };
            if along_u {
                let face = if out[1] > 0.0 { v[1] } else { v[0] };
                (u, [face + o0, face + o1])
            } else {
                let face = if out[0] > 0.0 { u[1] } else { u[0] };
                ([face + o0, face + o1], v)
            }
        };
        let (su, sv) = strip(-0.35, 0.6);
        battlements(s, at, su, sv, top, st.wall);
        let (su, sv) = strip(2.55, 3.0);
        s.push(bricks(at, su, sv, [top - 0.05, top + 0.7], st.wall));
        let (su, sv) = strip(-0.35, 0.6);
        s.push(bl(at, su, sv, [top - 0.9, top], st.trim));
        // Arrow slits along its outer face, wherever it stands high over the ground
        let length = if along_u { u[1] - u[0] } else { v[1] - v[0] };
        let mut a = 6.0;
        while a < length - 6.0 {
            let (fu, fv) = if along_u {
                (u[0] + a, if out[1] > 0.0 { v[1] } else { v[0] })
            } else {
                (if out[0] > 0.0 { u[1] } else { u[0] }, v[0] + a)
            };
            if self.ground(fu, fv) < top - 6.5 && (k != 0 || fv.abs() > 6.0) {
                let glass = if rng.next() < 0.2 { st.glass } else { OBSIDIAN };
                window(s, at, [fu, fv], out, 0.6, 0.15, [top - 4.6, top - 2.6], glass, st.trim);
            }
            a += 7.0;
        }
        let (su, sv) = strip(0.6, 2.4);
        s.push(bl(at, su, sv, [low, top - 0.6], (st.wall.0, AIR)));
        s.push(bricks(at, u, v, [low - 0.6, top], st.wall));
    }

    /// The ways in: steps down from the gate outside and a paved way on from them, steps down from
    /// it inside and a road over the bailey to the grand stair, and that up to the inner ward,
    /// lamp posts along the way
    fn ways(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features) {
        let at = &self.at;
        let st = self.stones;
        let (f, g, w) = (self.front, self.gate_floor, self.ward);
        let path = st.paving[1];
        let out = self.descend(s, f + 1.6, 1.0, 2.4, g, path);
        self.paved(s, [out, f + 16.0], 2.4, path);
        let foot = self.range + self.steps as f32 * RUN;
        let inside = self.descend(s, f - 7.6, -1.0, 2.4, g, path);
        self.paved(s, [foot, inside], 2.4, path);
        for k in 1..=self.steps {
            let top = w - k as f32 * RISE;
            let a = self.range + (k - 1) as f32 * RUN;
            let (low, _) = self.span([a, a + RUN], [-4.6, 4.6], 2.0);
            s.push(bl(at, [a, a + RUN], [-4.0, 4.0], [low - 0.4, top], st.trim));
            for side in [-1.0, 1.0] {
                s.push(bricks(at, [a, a + RUN], [side * 4.0, side * 4.6], [low - 0.4, top + 0.95], st.trim));
            }
        }
        for side in [-1.0, 1.0] {
            self.lamp_post(t, s, features, [foot + 0.6, side * 5.0], [0.0, -side]);
            self.lamp_post(t, s, features, [(foot + inside) * 0.5, side * 3.4], [0.0, -side]);
            self.lamp_post(t, s, features, [f + 9.0, side * 3.4], [0.0, -side]);
        }
    }

    /// Steps down from `top` along u, `dir` +1 or -1, from `start`, `half` either side of the
    /// middle, until they meet the ground: returns where they end
    fn descend(&self, s: &mut Vec<Shape>, start: f32, dir: f32, half: f32, top: f32, stone: (u8, u8)) -> f32 {
        let mut u = start;
        for k in 1..48 {
            let z = top - k as f32 * RISE;
            let (a, b) = (u.min(u + dir * RUN), u.max(u + dir * RUN));
            let (low, high) = self.span([a, b], [-half, half], 2.0);
            if z <= high + 0.12 {
                break;
            }
            s.push(bl(&self.at, [a, b], [-half, half], [low - 0.4, z], stone));
            u += dir * RUN;
        }
        u
    }

    /// Paves the ground from u[0] to u[1] along the middle, `half` either side
    fn paved(&self, s: &mut Vec<Shape>, u: [f32; 2], half: f32, stone: (u8, u8)) {
        let (mut a, end) = (u[0].min(u[1]), u[0].max(u[1]));
        while a < end - 0.05 {
            let b = (a + 2.0).min(end);
            let (lo, hi) = self.at.rect([a, b], [-half, half]);
            let g = [(lo[0], lo[1]), (hi[0], lo[1]), (lo[0], hi[1]), (hi[0], hi[1])].map(|(x, y)| height_at(&self.settings, x, y));
            pave(s, lo, hi, g.iter().copied().fold(f32::MAX, f32::min) - 0.15, g.iter().copied().fold(f32::MIN, f32::max) + 0.15, stone);
            a = b;
        }
    }

    /// The courtyard's fountain, a basin of still water with a pillar rising from it to a bowl, and
    /// braziers round it
    fn courtyard(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features) {
        let at = &self.at;
        let st = self.stones;
        let (c, cw, w) = (self.court, self.court_half, self.ward);
        let fu = c * 0.5;
        let (outer, inner) = (2.6, 2.2);
        s.push(Shape::sphere(at.point(fu, 0.0, w + 2.75), 0.22, GOLD, AIR, false));
        s.push(blob(at, [fu, 0.0, w + 2.2], [0.9, 0.9, 0.22], st.trim));
        s.push(up(at, [fu, 0.0], [w, w + 2.2], [0.32, 0.22], st.trim));
        s.push(at.room([fu - inner, fu + inner], [-inner, inner], [w + 0.15, w + 1.0]));
        s.push(bl(at, [fu - outer - 0.08, fu + outer + 0.08], [-outer - 0.08, outer + 0.08], [w + 0.5, w + 0.62], st.trim));
        s.push(bricks(at, [fu - outer, fu + outer], [-outer, outer], [w - 0.35, w + 0.5], st.trim));
        let (lo, hi) = at.rect([fu - inner, fu + inner], [-inner, inner]);
        features.floors.push(Floor { lo, hi, height: w + 0.15, water: w + 0.45 });
        features.footprints.push([lo[0] - 0.4, lo[1] - 0.4, hi[0] + 0.4, hi[1] + 0.4, w + 0.7]);
        for u in [c * 0.18, c * 0.82] {
            for side in [-1.0, 1.0] {
                self.brazier(t, s, features, [u, side * (cw - 2.2)], w);
            }
        }
    }

    /// The inner ward's terrace: the courtyard paved in great squares of two stones, lawns round
    /// the rest with gravel walks, low parapets round its edges where nothing's built, and solid
    /// stone under it all, faced with brick
    fn terrace(&self, s: &mut Vec<Shape>) {
        let at = &self.at;
        let st = self.stones;
        let (u0, u1, hv) = (self.keep[0], self.range, self.ward_half);
        let (w, cw) = (self.ward, self.court_half);
        let (nu, nv) = ((self.court / 3.2).round().max(1.0), (2.0 * cw / 3.2).round().max(1.0));
        let (du, dv) = (self.court / nu, 2.0 * cw / nv);
        for i in 0..nu as usize {
            for j in 0..nv as usize {
                let (a, b) = (i as f32 * du, -cw + j as f32 * dv);
                s.push(bl(at, [a, a + du], [b, b + dv], [w - 0.35, w], st.paving[(i + j) % 2]).placed(Placement::Clear));
            }
        }
        s.push(bl(at, [self.court, u1], [-3.0, 3.0], [w - 0.35, w], st.paving[0]).placed(Placement::Clear));
        // Gravel walks round the great hall and out to the garden towers
        let gravel = (GRAVEL, STONE);
        for side in [-1.0, 1.0] {
            let v = [side * (self.hall_half + 1.3), side * (self.hall_half + 3.1)];
            s.push(bl(at, [self.keep[1] - 1.0, 0.0], v, [w - 0.35, w], gravel).placed(Placement::Clear));
        }
        s.push(bl(at, [self.keep[1] - 3.0, self.keep[1] - 1.0], [-self.hall_half - 3.1, self.hall_half + 3.1], [w - 0.35, w], gravel).placed(Placement::Clear));
        s.push(bl(at, [u0, u1], [-hv, hv], [w - 0.35, w], (GRASS, GRASS_TIP)));
        // Parapets round the edges where nothing's built
        let mut parapet = |u: [f32; 2], v: [f32; 2]| {
            let along_u = u[1] - u[0] > v[1] - v[0];
            let (a0, a1) = if along_u { (u[0], u[1]) } else { (v[0], v[1]) };
            let mut a = a0 + 1.0;
            while a < a1 - 0.8 {
                let (cu, cv) = if along_u { ([a - 0.35, a + 0.35], [v[0] - 0.08, v[1] + 0.08]) } else { ([u[0] - 0.08, u[1] + 0.08], [a - 0.35, a + 0.35]) };
                s.push(bl(at, cu, cv, [w + 1.0, w + 1.3], st.trim));
                a += 2.4;
            }
            s.push(bricks(at, u, v, [w - 0.35, w + 1.0], st.trim));
        };
        let wv = cw + self.wing;
        for side in [-1.0, 1.0] {
            let sv = |a: f32, b: f32| if side < 0.0 { [-b, -a] } else { [a, b] };
            parapet([u0, u0 + 0.5], sv(self.keep_half, hv));
            parapet([u0, u1], sv(hv - 0.5, hv));
            parapet([u1 - 0.5, u1], sv(wv, hv));
        }
        let low = self.lowest - 0.6;
        s.push(bl(at, [u0 + 0.6, u1 - 0.6], [-hv + 0.6, hv - 0.6], [low, w - 0.35], (STONE, AIR)));
        s.push(bricks(at, [u0, u1], [-hv, hv], [low, w - 0.35], st.trim));
    }

    /// Trees in the gardens of the inner ward and out in the bailey
    fn trees(&self, t: &Terrain, rng: &mut Rng) -> Vec<Object> {
        let mut spots = Vec::new();
        let hall = self.keep[1];
        let wv = self.court_half + self.wing;
        for side in [-1.0, 1.0] {
            for k in [0.3, 0.75] {
                spots.push(([hall * (1.0 - k), side * (self.hall_half + 5.6)], true));
            }
            spots.push(([self.court * 0.5, side * (wv + (self.ward_half - wv) * 0.5)], true));
            spots.push(([self.front - 10.0, side * (self.side - 9.0)], false));
            spots.push(([self.back + 5.5, side * self.side * 0.78], false));
        }
        spots
            .into_iter()
            .map(|(c, ward)| {
                let p = self.at.point(c[0], c[1], 0.0);
                let base = if ward { self.ward } else { height_at(&self.settings, p[0], p[1]) };
                t.tree(p[0], p[1], base, hash(rng.0 ^ hash(c[0].to_bits() ^ c[1].to_bits())))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn castle_spots_debug() {
        let settings = TerrainSettings { width_m: 32, height_m: 64, voxels_per_meter: 4, seed: 1, grass_tufts: false };
        let (terrain, _) = generate_terrain(&settings, [0, 0], &RiverCache::default());
        let sea = sea_level_m(&settings);
        let sky = settings.height_m as f32 - 1.0;
        let mut reasons = [0usize; 8];
        let mut tops = Vec::new();
        let (mut lows, mut reliefs) = (Vec::new(), Vec::new());
        for cy in -4..4 {
            for cx in -4..4 {
                let r = hash3(cx as u32, cy as u32, settings.seed ^ 0xca51);
                for attempt in 0..SPOTS {
                    let s = hash3(r, attempt, 0xca52);
                    let p = [(cx as f32 + 0.2 + 0.6 * unit(s)) * CASTLE_CELL, (cy as f32 + 0.2 + 0.6 * unit(s >> 16)) * CASTLE_CELL];
                    let g = |dx: f32, dy: f32| height_at(&settings, p[0] + dx, p[1] + dy);
                    let (mut lo, mut hi, mut sum) = (f32::MAX, f32::MIN, 0.0);
                    for i in 0..49 {
                        let h = g(((i % 7) as f32 - 3.0) * 12.0, ((i / 7) as f32 - 3.0) * 12.0);
                        (lo, hi, sum) = (lo.min(h), hi.max(h), sum + h);
                    }
                    tops.push(hi);
                    lows.push(lo);
                    reliefs.push(hi - lo);
                    let mut around = 0.0;
                    for k in 0..16 {
                        let a = k as f32 * TAU / 16.0;
                        around += g(a.cos() * 115.0, a.sin() * 115.0);
                    }
                    let prominence = sum / 49.0 - around / 16.0;
                    let k = if lo < sea + 1.0 { 0 } else if hi - lo > 14.0 { 1 } else if hi > sky - LEAST_HEIGHT - 0.6 { 2 } else if prominence < 1.5 { 3 } else if terrain.castle_spot(p).is_none() { 4 } else if terrain.lay_out_castle(p, 1).is_none() { 5 } else { 6 };
                    reasons[k] += 1;
                    if k == 6 {
                        let c = terrain.lay_out_castle(p, 1).unwrap();
                        let cache = RiverCache::default();
                        let rect = c.bounds;
                        let river = terrain.rivers_near(&cache, [rect[0] - 10.0, rect[1] - 10.0], [rect[2] + 10.0, rect[3] + 10.0]).iter().any(|r| r.iter().any(|q| q.x > rect[0] - 10.0 && q.x < rect[2] + 10.0 && q.y > rect[1] - 10.0 && q.y < rect[3] + 10.0));
                        let towns = !terrain.towns_near(grown(rect, 15.0), &cache).is_empty();
                        let all = terrain.castle_blocked(&c, &cache, &mut HashMap::new());
                        println!("spot ({:.0}, {:.0}): river {river} towns {towns} blocked {all:?}", p[0], p[1]);
                    }
                }
            }
        }
        tops.sort_by(f32::total_cmp);
        lows.sort_by(f32::total_cmp);
        reliefs.sort_by(f32::total_cmp);
        let q = |v: &Vec<f32>| format!("{:.1} {:.1} {:.1} {:.1} {:.1}", v[v.len() / 10], v[v.len() / 4], v[v.len() / 2], v[v.len() * 3 / 4], v[v.len() * 9 / 10]);
        println!("lows {} reliefs {}", q(&lows), q(&reliefs));
        println!("wet {} rough {} high {} flat-lying {} walls {} layout {} ok {}", reasons[0], reasons[1], reasons[2], reasons[3], reasons[4], reasons[5], reasons[6]);
        println!("tops: 10% {:.1} 50% {:.1} 90% {:.1}, sea {sea}, highest allowed {:.1}", tops[tops.len() / 10], tops[tops.len() / 2], tops[tops.len() * 9 / 10], sky - LEAST_HEIGHT - 0.6);
    }

    /// Castles turn up here and there, their towers and the rest under the sky, their ways up
    /// never steeper than a step
    #[test]
    fn castles_turn_up() {
        let settings = TerrainSettings { width_m: 32, height_m: 64, voxels_per_meter: 4, seed: 1, grass_tufts: false };
        let (terrain, _) = generate_terrain(&settings, [0, 0], &RiverCache::default());
        let cache = RiverCache::default();
        let mut found = 0;
        let start = std::time::Instant::now();
        for cy in -4..4 {
            for cx in -4..4 {
                let Some(c) = terrain.castle_in(cx, cy, &cache) else {
                    continue;
                };
                found += 1;
                let width = (c.bounds[2] - c.bounds[0]).max(c.bounds[3] - c.bounds[1]);
                assert!((80.0..=150.0).contains(&width), "{width} m across");
                for t in &c.towers {
                    let tip = match t.crown {
                        Crown::Spire(h) | Crown::Corbelled(h) => t.top + h + FINIAL,
                        Crown::Battlements => t.top + 1.5 + if t.r > 3.0 { 3.2 + 1.08 * t.r + FINIAL } else { 0.0 },
                    };
                    assert!(tip <= c.sky + 0.01, "a spire reaching {tip} m, over {}", c.sky);
                }
                let tallest = c.towers.iter().map(|t| if let Crown::Spire(h) = t.crown { t.top + h } else { t.top }).fold(0.0, f32::max);
                assert!(tallest - c.ward >= LEAST_HEIGHT - FINIAL - 0.5);
                assert!(c.throne - c.ward < 0.6 * 6.0 && c.steps >= 1);
                println!("castle in ({cx}, {cy}): {width:.0} m across, its ward at {:.1} m, the tallest spire {:.1} m over it", c.ward, tallest - c.ward);
            }
        }
        println!("{found} castles in 64 squares, planned in {:.2} s", start.elapsed().as_secs_f32());
        assert!(found > 0);
    }
}
