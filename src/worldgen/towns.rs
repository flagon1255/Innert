//! Towns: houses along streets round a square with a well, lanterns lit through the night,
//! merchants' stalls and the folk who live there. Most stand out on flat, dry ground, built of what
//! the land round them gives: plaster and timber or brick where it's mild, log cabins under the
//! snow, sandstone in the desert and temple stone in the jungle. Some are down in a great cavern
//! under a hill, reached by a tunnel from the hillside and lit by lanterns and crystals.

use super::*;

/// At most one town in each TOWN_CELL square, nothing of it further than TOWN_REACH from where
/// it's planned round, in meters
const TOWN_CELL: f32 = 500.0;
const TOWN_REACH: f32 = 100.0;
/// How far out from the middle of its square the streets of a town on the land go
const TOWN_RADIUS: f32 = 28.0;
/// Half how wide streets are, and how big the stones squares are paved with
const STREET_HALF: f32 = 1.2;
const TILE: f32 = 1.0;
/// The tunnel down to a town in a cavern: how far its sides are from its middle, how high that
/// middle is over its floor, and the steepest its floor gets
const TUNNEL_RADIUS: f32 = 1.9;
const TUNNEL_MIDDLE: f32 = 1.3;
const TUNNEL_SLOPE: f32 = 0.2;
/// How much of a cavern's height is the dome over the middle of the hollow cut for it, the rest
/// being how far that middle is over its floor
const DOME: f32 = 0.72;
/// The ways streets go out of a square
const WAYS: [[f32; 2]; 4] = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]];
/// The light lanterns give
const LANTERN_LIGHT: [f32; 3] = [1.0, 0.66, 0.3];

/// How far a cavern's flat floor goes out, next to the widest of the hollow cut for it
fn floor_width() -> f32 {
    (1.0 - ((1.0 - DOME) / DOME).powi(2)).sqrt()
}

/// What a town's built of, which goes by the land round it
#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Temperate,
    Desert,
    Snowy,
    Jungle,
}

impl Style {
    fn of(c: Climate) -> Style {
        if c.desert > 0.5 {
            Style::Desert
        } else if c.snow > 0.5 {
            Style::Snowy
        } else if c.jungle > 0.5 {
            Style::Jungle
        } else {
            Style::Temperate
        }
    }

    /// The stones its square is paved with, each a material and an accent
    fn paving(self) -> [(u8, u8); 3] {
        match self {
            Style::Desert => [(SANDSTONE, SAND), (TEMPLE_STONE, SANDSTONE), (LIMESTONE, SANDSTONE)],
            Style::Jungle => [(TEMPLE_STONE, MOSS), (TEMPLE_DARK, MOSS), (LIMESTONE, MOSS)],
            Style::Snowy => [(STONE, SNOW), (GRAVEL, SNOW), (ROCK, STONE)],
            Style::Temperate => [(STONE, GRAVEL), (ROCK, STONE), (GRAVEL, ROCK)],
        }
    }

    /// Trodden earth or gravel, for its streets
    fn street(self) -> (u8, u8) {
        match self {
            Style::Desert => (DIRT, SAND),
            Style::Jungle => (DIRT, MOSS),
            Style::Snowy => (GRAVEL, SNOW),
            Style::Temperate => (DIRT, GRAVEL),
        }
    }

    /// What footings, steps and wells are built of
    fn stone(self) -> (u8, u8) {
        match self {
            // Fired clay, with dark mortar
            Style::Desert => (CLAY_POT, SANDSTONE_DARK),
            Style::Jungle => (TEMPLE_DARK, MOSS),
            Style::Snowy => (BRICK_STONE, SNOW),
            Style::Temperate => (BRICK_STONE, MOSS),
        }
    }

    /// Picks how a house's walls are built, what of, and how its surface looks
    fn walls(self, rng: &mut Rng) -> (Walls, (u8, u8), Pattern) {
        let pick = rng.next();
        match self {
            Style::Temperate if pick < 0.45 => (Walls::Plaster, (LIMESTONE, CLAY), Pattern::Plain),
            Style::Temperate if pick < 0.8 => (Walls::Brick, (BRICK_STONE, MOSS), Pattern::Bricks),
            Style::Snowy if pick < 0.3 => (Walls::Brick, (BRICK_STONE, LIMESTONE), Pattern::Bricks),
            Style::Temperate | Style::Snowy => (Walls::Logs, (WOOD, AIR), Pattern::Plain),
            Style::Desert if pick < 0.6 => (Walls::Adobe, (SANDSTONE, SAND), Pattern::Plain),
            // Whitewashed
            Style::Desert => (Walls::Adobe, (LIMESTONE, SAND), Pattern::Plain),
            Style::Jungle => (Walls::Temple, (TEMPLE_STONE, LIMESTONE), Pattern::Plain),
        }
    }

    /// Picks a house's roof, None for a flat one
    fn roof(self, rng: &mut Rng) -> Option<Roof> {
        let pick = rng.next();
        match self {
            Style::Desert => None,
            // Thatch
            Style::Jungle => Some(Roof { cover: (PALM_WOOD, DRY_GRASS), ridge: BARK_DARK, pitch: 1.0, snow: false }),
            // Dark shingles, heavy with snow
            Style::Snowy => Some(Roof { cover: (BARK_DARK, WOOD), ridge: WOOD, pitch: 0.95, snow: true }),
            // Red tiles, or dark shingles
            Style::Temperate if pick < 0.6 => Some(Roof { cover: (TEMPLE_RED, CLAY_POT), ridge: CLAY_POT, pitch: 0.75, snow: false }),
            Style::Temperate => Some(Roof { cover: (BARK_DARK, WOOD), ridge: WOOD, pitch: 0.8, snow: false }),
        }
    }
}

/// How a house's walls are built
#[derive(Clone, Copy, PartialEq, Eq)]
enum Walls {
    /// Plaster in a frame of timber
    Plaster,
    Brick,
    /// Logs laid one on another, crossing at the corners
    Logs,
    /// Sandstone or whitewash, with the ends of the roof's beams showing
    Adobe,
    /// Temple stone with dark and painted bands round it
    Temple,
}

/// A pitched roof: what rows of tiles or shingles it's covered with, what runs along its ridge,
/// how steep it is, and whether snow lies on it
#[derive(Clone, Copy)]
struct Roof {
    cover: (u8, u8),
    ridge: u8,
    pitch: f32,
    snow: bool,
}

/// How far a pitched roof reaches past the gable ends, and past the walls along the eaves
const GABLE_OVERHANG: f32 = 0.3;
const EAVES_OVERHANG: f32 = 0.45;

/// A house of a town, laid out in its frame `at`, which faces the way its door does, onto the
/// street: `half` its size along u and v, in meters
#[derive(Clone, Copy)]
struct House {
    at: Frame,
    half: [f32; 2],
    /// The lowest ground under it, and its floor inside
    base: f32,
    floor: f32,
    /// How high its walls stand over the floor
    wall: f32,
    walls: Walls,
    /// What its walls are made of, and how they look
    stone: (u8, u8),
    pattern: Pattern,
    /// None for a flat roof
    roof: Option<Roof>,
    /// How far in front of its door the street is
    setback: f32,
    seed: u32,
}

impl House {
    /// Whether its roof's ridge runs along u, the longer way
    fn ridge_along_u(&self) -> bool {
        self.half[0] > self.half[1]
    }

    /// How far its roof rises over the eaves
    fn rise(&self) -> f32 {
        let across = if self.ridge_along_u() { self.half[1] } else { self.half[0] };
        self.roof.map_or(0.0, |r| (across + EAVES_OVERHANG) * r.pitch)
    }

    /// How high it reaches, chimney and all
    fn top(&self) -> f32 {
        self.floor + self.wall + if self.roof.is_some() { self.rise() + 0.6 } else { 1.0 }
    }

    /// What it covers, with its eaves, the steps in front and a little room round it, x0, y0, x1, y1
    fn rect(&self) -> [f32; 4] {
        let [hu, hv] = self.half;
        let (lo, hi) = self.at.rect([-hu - 0.6, hu + 1.2], [-hv - 0.6, hv + 0.6]);
        [lo[0], lo[1], hi[0], hi[1]]
    }

    /// What its footing covers, x0, y0, x1, y1
    fn plinth(&self) -> [f32; 4] {
        let [hu, hv] = self.half;
        let (lo, hi) = self.at.rect([-hu - 0.12, hu + 0.12], [-hv - 0.12, hv + 0.12]);
        [lo[0], lo[1], hi[0], hi[1]]
    }
}

/// The great cavern a town is down in, under a hill: how far its flat floor goes out from the
/// middle of the square along x and y, and how high its roof is over that, in meters. A tunnel
/// leads from it `way` along a street, leaving it at `gate` meters from the middle of the square,
/// up or down `rise` to its mouth on the hillside, `mouth` meters out.
#[derive(Clone, Copy)]
struct Cavern {
    radii: [f32; 2],
    height: f32,
    way: [f32; 2],
    gate: f32,
    mouth: f32,
    rise: f32,
}

impl Cavern {
    /// How high the roof is over the floor `d` meters along x and y from the middle of the square,
    /// None where there isn't flat floor to build on
    fn headroom(&self, d: [f32; 2]) -> Option<f32> {
        let e = (d[0] / self.radii[0]).powi(2) + (d[1] / self.radii[1]).powi(2);
        if e > 0.92 {
            return None;
        }
        let q = e * floor_width().powi(2);
        Some((1.0 - DOME) * self.height + DOME * self.height * (1.0 - q).max(0.0).sqrt())
    }

    /// How high the tunnel's floor is over the cavern's, `d` meters out from the middle of the square
    fn tunnel_floor(&self, d: f32) -> f32 {
        self.rise * ((d - self.gate) / (self.mouth - self.gate)).clamp(0.0, 1.0)
    }
}

/// A town, laid out: the middle of its square on the paving, how far out the square goes, and
/// what it's built of
struct Town {
    square: Vec3,
    radius: f32,
    style: Style,
    /// Which ways streets lead out of the square, and how far from its middle they go
    streets: Vec<([f32; 2], f32)>,
    houses: Vec<House>,
    /// The merchants' stalls round the square, each facing the middle of it
    stalls: Vec<Frame>,
    /// Down in a cavern, or None out on the land
    cavern: Option<Cavern>,
    /// Where it all is, x0, y0, x1, y1 in meters
    bounds: [f32; 4],
    seed: u32,
}

/// What's round a town being planned, worked out once for all the spots tried in its square:
/// for each square of their own, the caves' hollows and caverns, the ravines and the dungeons
#[derive(Default)]
struct Near {
    caves: HashMap<(i32, i32), CaveProbe>,
    ravines: HashMap<(i32, i32), Vec<[f32; 4]>>,
    dungeons: HashMap<(i32, i32), Vec<[f32; 4]>>,
}

/// A cave as a town sees it: what's cut out for it, each x0, y0, x1, y1 with its top and the
/// ground over its middle, the middles of the floors of its caverns, and where it all is
#[derive(Default)]
struct CaveProbe {
    hollows: Vec<([f32; 4], f32, f32)>,
    caverns: Vec<Vec3>,
    bounds: [f32; 4],
}

impl Terrain {
    /// The towns reaching into this piece of the landscape, built: what they're made of, and
    /// where each is, x0, y0, x1, y1 in meters, so nothing else is built there
    pub(super) fn towns(&self, cache: &RiverCache, features: &mut Features) -> (Vec<Object>, Vec<[f32; 4]>) {
        let (lo, hi) = (self.lo, self.hi);
        let first = [0, 1].map(|i| ((lo[i] - TOWN_REACH) / TOWN_CELL).floor() as i32);
        let last = [0, 1].map(|i| ((hi[i] + TOWN_REACH) / TOWN_CELL).floor() as i32);
        let (mut objects, mut rects) = (Vec::new(), Vec::new());
        for cy in first[1]..=last[1] {
            for cx in first[0]..=last[0] {
                if let Some(town) = self.town_in(cx, cy, cache)
                    && overlaps(town.bounds, [lo[0], lo[1], hi[0], hi[1]], 0.0)
                {
                    objects.extend(self.town(&town, features));
                    rects.push(town.bounds);
                }
            }
        }
        (objects, rects)
    }

    /// Where the towns reaching into rectangle `r` are, each x0, y0, x1, y1 in meters, for castles
    /// to keep clear of, see castles.rs
    pub(super) fn towns_near(&self, r: [f32; 4], cache: &RiverCache) -> Vec<[f32; 4]> {
        let first = [0, 1].map(|i| ((r[i] - TOWN_REACH) / TOWN_CELL).floor() as i32);
        let last = [0, 1].map(|i| ((r[i + 2] + TOWN_REACH) / TOWN_CELL).floor() as i32);
        let mut found = Vec::new();
        for cy in first[1]..=last[1] {
            for cx in first[0]..=last[0] {
                if let Some(town) = self.town_in(cx, cy, cache)
                    && overlaps(town.bounds, r, 0.0)
                {
                    found.push(town.bounds);
                }
            }
        }
        found
    }

    /// The nearest town to `near` within a few kilometers, out on the land or, if `underground`,
    /// down in a cavern, and if `kind` says, built for that kind of land ("jungle", "desert",
    /// "snow" or "temperate"): the middle of its square, and where the way in is, the mouth of the
    /// tunnel down to it or just past the end of a street, all in meters. For trying them out.
    #[allow(dead_code)]
    pub fn nearest_town(&self, near: [f32; 2], underground: bool, kind: Option<&str>) -> Option<([f32; 3], [f32; 3])> {
        let fits = |t: &Town| {
            let land = match t.style {
                Style::Temperate => "temperate",
                Style::Desert => "desert",
                Style::Snowy => "snow",
                Style::Jungle => "jungle",
            };
            t.cavern.is_some() == underground && kind.is_none_or(|k| k == land)
        };
        let cache = RiverCache::default();
        let home = near.map(|v| (v / TOWN_CELL).floor() as i32);
        let mut best: Option<(f32, Town)> = None;
        for ring in 0..8i32 {
            for cy in home[1] - ring..=home[1] + ring {
                for cx in home[0] - ring..=home[0] + ring {
                    if (cx - home[0]).abs().max((cy - home[1]).abs()) != ring {
                        continue;
                    }
                    if let Some(town) = self.town_in(cx, cy, &cache).filter(fits) {
                        let distance = (town.square[0] - near[0]).hypot(town.square[1] - near[1]);
                        if best.as_ref().is_none_or(|(b, _)| distance < *b) {
                            best = Some((distance, town));
                        }
                    }
                }
            }
            // Any further out are at least this far away
            if best.as_ref().is_some_and(|(b, _)| *b <= ring as f32 * TOWN_CELL) {
                break;
            }
        }
        best.map(|(_, town)| (town.square, self.way_in(&town)))
    }

    /// Where the way into town `t` is: the mouth of the tunnel down to a town in a cavern, or a
    /// little past the end of the first street of one on the land, in meters
    fn way_in(&self, t: &Town) -> [f32; 3] {
        let [x, y, floor] = t.square;
        match &t.cavern {
            Some(c) => [x + c.way[0] * c.mouth, y + c.way[1] * c.mouth, floor + c.rise],
            None => {
                let (way, length) = t.streets[0];
                let (px, py) = (x + way[0] * (length + 8.0), y + way[1] * (length + 8.0));
                [px, py, height_at(&self.settings, px, py)]
            }
        }
    }

    /// The town in one TOWN_CELL square, if there's room for one at any of a few spots in it. Some
    /// are down in a cavern if there's a hill for one; the rest, and those that find no hill, are
    /// out on the land.
    fn town_in(&self, cx: i32, cy: i32, cache: &RiverCache) -> Option<Town> {
        let r = hash3(cx as u32, cy as u32, self.settings.seed ^ 0x7043);
        if unit(r) > 0.9 {
            return None;
        }
        let underground = unit(r >> 16) < 0.4;
        let spot = |attempt: u32| {
            let s = hash3(r, attempt, 0x70e5);
            ((cx as f32 + 0.15 + 0.7 * unit(s)) * TOWN_CELL, (cy as f32 + 0.15 + 0.7 * unit(s >> 16)) * TOWN_CELL, hash(s))
        };
        let mut near = Near::default();
        let down = if underground { 40 } else { 0 };
        let town = (0..down).find_map(|attempt| {
            let (x, y, seed) = spot(attempt);
            self.plan_cavern_town(x, y, seed, cache, &mut near)
        });
        town.or_else(|| {
            (40..120).find_map(|attempt| {
                let (x, y, seed) = spot(attempt);
                self.plan_town(x, y, seed, cache, &mut near)
            })
        })
    }

    /// Lays out a town on the land with the middle of its square at `x`, `y`, if it fits: on
    /// fairly flat, dry ground, flatter still under the square, with nothing else in the way
    fn plan_town(&self, x: f32, y: f32, seed: u32, cache: &RiverCache, near: &mut Near) -> Option<Town> {
        let settings = &self.settings;
        let ground = |x: f32, y: f32| height_at(settings, x, y);
        let (mut lowest, mut highest) = (f32::MAX, f32::MIN);
        for i in 0..49 {
            let (u, v) = ((i % 7) as f32 - 3.0, (i / 7) as f32 - 3.0);
            let g = ground(x + u * TOWN_RADIUS / 3.0, y + v * TOWN_RADIUS / 3.0);
            (lowest, highest) = (lowest.min(g), highest.max(g));
        }
        // Houses stand on footings of their own and the paving follows the ground, so it only has
        // to be fairly flat
        if lowest < sea_level_m(settings) + 2.0 || highest - lowest > 9.0 {
            return None;
        }
        let mut rng = Rng(seed);
        let radius = rng.range(7.0, 9.0);
        let (mut lowest, mut highest) = (f32::MAX, f32::MIN);
        for k in 0..13 {
            let (a, d) = (k as f32 * TAU / 12.0, if k == 12 { 0.0 } else { radius });
            let g = ground(x + a.cos() * d, y + a.sin() * d);
            (lowest, highest) = (lowest.min(g), highest.max(g));
        }
        if highest - lowest > 2.0 {
            return None;
        }
        let mut town = Town {
            square: [x, y, 0.0],
            radius,
            style: Style::of(climate(settings, x, y)),
            streets: Vec::new(),
            houses: Vec::new(),
            stalls: Vec::new(),
            cavern: None,
            bounds: around([x, y], [TOWN_RADIUS + 4.0; 2]),
            seed: hash(seed ^ 0x70ab),
        };
        town.square[2] = self.paving(&town, x, y);
        self.lay_out(&mut town, &mut rng);
        if town.houses.len() < 6 {
            return None;
        }
        self.clear_for_town(&town, cache, near).then_some(town)
    }

    /// Lays out a town down in a cavern under a hill, with the middle of its square at `x`, `y`,
    /// if it fits: with the hill standing well over the cavern's roof, and a tunnel from it out to
    /// the hillside that's never steep, coming out of the hill into a cutting near its mouth
    fn plan_cavern_town(&self, x: f32, y: f32, seed: u32, cache: &RiverCache, near: &mut Near) -> Option<Town> {
        let settings = &self.settings;
        let ground = |x: f32, y: f32| height_at(settings, x, y);
        let sea_level = sea_level_m(settings);
        let mut rng = Rng(seed);
        let radii = [rng.range(16.0, 20.0), rng.range(16.0, 20.0)];
        let height = rng.range(10.0, 13.0);
        // The hill has to stand well over the roof all round, higher over the middle where the
        // roof's highest, as it's a dome
        let wide = radii.map(|r| r / floor_width());
        let (middle, dome) = ((1.0 - DOME) * height, DOME * height);
        let mut floor = f32::MAX;
        for k in 0..37 {
            let (a, r) = ((k % 12) as f32 * TAU / 12.0, [0.95f32, 0.75, 0.45, 0.0][k / 12]);
            let roof = middle + dome * (1.0 - r * r).sqrt();
            floor = floor.min(ground(x + a.cos() * wide[0] * r, y + a.sin() * wide[1] * r) - roof - 3.0);
        }
        if floor < sea_level + 0.5 {
            return None;
        }

        // The shortest way out: up or down to where the hillside comes down to it, never steeply,
        // without coming out of the ground on the way
        let mut best: Option<Cavern> = None;
        for way in WAYS {
            let gate = if way[0] != 0.0 { radii[0] } else { radii[1] };
            let at = |d: f32| ground(x + way[0] * d, y + way[1] * d);
            let Some(mouth) = (0..40).map(|k| gate + 4.0 + k as f32 * 1.5).find(|&d| at(d) < floor + TUNNEL_SLOPE * (d - gate) + 0.5) else {
                continue;
            };
            let rise = (at(mouth) - floor).clamp(-TUNNEL_SLOPE * (mouth - gate), TUNNEL_SLOPE * (mouth - gate));
            let cavern = Cavern { radii, height, way, gate, mouth, rise };
            let covered = (0..((mouth - gate - 20.0) / 2.0).max(0.0) as usize)
                .map(|k| gate + k as f32 * 2.0)
                .all(|d| at(d) > floor + cavern.tunnel_floor(d) + TUNNEL_MIDDLE + TUNNEL_RADIUS + 0.5);
            if covered && at(mouth) > sea_level + 1.5 && best.is_none_or(|b| mouth - gate < b.mouth - b.gate) {
                best = Some(cavern);
            }
        }
        let cavern = best?;
        let mut bounds = around([x, y], wide.map(|w| w + 2.0));
        let (a, b) = ([x + cavern.way[0] * cavern.gate, y + cavern.way[1] * cavern.gate], [x + cavern.way[0] * (cavern.mouth + 7.0), y + cavern.way[1] * (cavern.mouth + 7.0)]);
        bounds = [bounds[0].min(a[0].min(b[0]) - 4.0), bounds[1].min(a[1].min(b[1]) - 4.0), bounds[2].max(a[0].max(b[0]) + 4.0), bounds[3].max(a[1].max(b[1]) + 4.0)];
        let mut town = Town {
            square: [x, y, floor],
            radius: rng.range(5.5, 6.5),
            style: Style::of(climate(settings, x, y)),
            streets: Vec::new(),
            houses: Vec::new(),
            stalls: Vec::new(),
            cavern: Some(cavern),
            bounds,
            seed: hash(seed ^ 0x70ab),
        };
        self.lay_out(&mut town, &mut rng);
        if town.houses.len() < 4 {
            return None;
        }
        self.clear_for_town(&town, cache, near).then_some(town)
    }

    /// Whether nothing else is in the way of town `t`: no river or ravine near it, no dungeon
    /// under it, and for a town on the land, no cave breaking out of the ground in its square,
    /// its streets or by its houses. Everything built in a town wins over what caves cut away
    /// under it, so they can pass under the ground. A town down in a cavern doesn't mind caves
    /// breaking into it, so long as no cave spiders nest there, nor a boss.
    fn clear_for_town(&self, t: &Town, cache: &RiverCache, near: &mut Near) -> bool {
        let rect = t.bounds;
        let mut built: Vec<[f32; 4]> = t.houses.iter().map(|h| grown(h.plinth(), 2.0)).collect();
        built.extend(t.streets.iter().map(|&(way, length)| street_rect(t.square, way, [0.0, length], STREET_HALF + 1.5)));
        built.push(around([t.square[0], t.square[1]], [t.radius + 1.0; 2]));
        // Caves first, as they're the most likely to be in the way
        let first = [0, 1].map(|i| ((rect[i] - CAVE_REACH) / CAVE_CELL).floor() as i32);
        let last = [0, 1].map(|i| ((rect[i + 2] + CAVE_REACH) / CAVE_CELL).ceil() as i32);
        for cy in first[1]..last[1] {
            for cx in first[0]..last[0] {
                let probe = near.caves.entry((cx, cy)).or_insert_with(|| self.probe_cave(cx, cy));
                if !overlaps(probe.bounds, rect, 0.0) {
                    continue;
                }
                let c = t.square;
                let blocked = if t.cavern.is_some() {
                    probe.caverns.iter().any(|f| (f[0] - c[0]).hypot(f[1] - c[1]) < 36.0 && (f[2] - c[2]).abs() < 25.0)
                } else {
                    probe.hollows.iter().any(|&(r, top, ground)| top > ground - 1.0 && built.iter().any(|b| overlaps(r, *b, 0.0)))
                };
                if blocked {
                    return false;
                }
            }
        }
        let first = [0, 1].map(|i| ((rect[i] - RAVINE_REACH - 2.0) / RAVINE_CELL).floor() as i32);
        let last = [0, 1].map(|i| ((rect[i + 2] + RAVINE_REACH + 2.0) / RAVINE_CELL).ceil() as i32);
        for cy in first[1]..last[1] {
            for cx in first[0]..last[0] {
                let ravine = near.ravines.entry((cx, cy)).or_insert_with(|| {
                    let mut scratch = Features::default();
                    self.ravine(cx, cy, &mut scratch).map_or(Vec::new(), |o| o.shapes.iter().map(|s| [s.min[0], s.min[1], s.max[0], s.max[1]]).collect())
                });
                if ravine.iter().any(|r| overlaps(*r, rect, 3.0)) {
                    return false;
                }
            }
        }
        let margin = 8.0;
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
                let dungeon = near.dungeons.entry((cx, cy)).or_insert_with(|| self.dungeon_in(cx, cy, cache).map_or(Vec::new(), |d| d.world_rects()));
                if dungeon.iter().any(|r| overlaps(*r, rect, 6.0)) {
                    return false;
                }
            }
        }
        true
    }

    /// The cave starting in one CAVE_CELL square, as a town sees it
    fn probe_cave(&self, cx: i32, cy: i32) -> CaveProbe {
        let Some(cave) = self.cave_in(cx, cy) else {
            return CaveProbe::default();
        };
        let hollows = cave
            .shapes
            .iter()
            .filter(|s| s.placement == Placement::Carve)
            .map(|s| {
                let ground = height_at(&self.settings, (s.min[0] + s.max[0]) * 0.5, (s.min[1] + s.max[1]) * 0.5);
                ([s.min[0], s.min[1], s.max[0], s.max[1]], s.max[2], ground)
            })
            .collect();
        let caverns = cave.lairs.iter().chain(&cave.nests).map(|(floor, ..)| *floor).collect();
        CaveProbe { hollows, caverns, bounds: cave.bounds }
    }

    /// Lays out town `t` round its square: streets out of it, houses along both sides of them with
    /// their doors onto the street, and merchants' stalls on the corners of the square between
    /// the streets. A town in a cavern has a street out to the tunnel, and houses only where
    /// there's room under its roof.
    fn lay_out(&self, t: &mut Town, rng: &mut Rng) {
        // Two to four streets: straight through, round a corner, or every way
        let turn = (rng.next() * 4.0) as usize;
        let mut ways: Vec<[f32; 2]> = (0..4).map(|k| WAYS[(turn + k) % 4]).collect();
        if let Some(c) = &t.cavern {
            ways.retain(|w| *w != c.way);
            ways.insert(0, c.way);
        }
        let count = 2 + (rng.next() * 2.99) as usize;
        if count == 2 && rng.next() < 0.5 {
            // Straight through
            ways.swap(1, 2);
        }
        ways.truncate(count);
        for way in ways {
            let length = match &t.cavern {
                Some(c) if way == c.way => c.gate,
                Some(c) => (if way[0] != 0.0 { c.radii[0] } else { c.radii[1] }) - 1.5,
                None => TOWN_RADIUS - rng.range(0.0, 4.0),
            };
            t.streets.push((way, length));
        }

        // Where houses could go along the streets, tried in no particular order
        let mut slots = Vec::new();
        for &(way, length) in &t.streets {
            for side in [-1.0f32, 1.0] {
                let out = [-way[1] * side, way[0] * side];
                let mut along = t.radius + rng.range(0.0, 2.0);
                // Smaller and closer together down in a cavern
                let small = t.cavern.is_some();
                loop {
                    let half = if small { [rng.range(2.1, 2.7), rng.range(2.3, 3.1)] } else { [rng.range(2.3, 3.2), rng.range(2.6, 3.8)] };
                    if along + 2.0 * half[1] > length + 2.0 {
                        break;
                    }
                    let setback = if small { rng.range(0.7, 1.3) } else { rng.range(1.2, 2.4) };
                    let (a, b) = (along + half[1], STREET_HALF + setback + half[0]);
                    let middle = [t.square[0] + way[0] * a + out[0] * b, t.square[1] + way[1] * a + out[1] * b];
                    slots.push((Frame { x: middle[0], y: middle[1], f: [-out[0], -out[1]] }, half, setback));
                    along += 2.0 * half[1] + rng.range(1.4, 3.5);
                }
            }
        }
        for i in (1..slots.len()).rev() {
            let j = ((rng.next() * (i + 1) as f32) as usize).min(i);
            slots.swap(i, j);
        }
        let want = if t.cavern.is_some() { 5 + (rng.next() * 4.99) as usize } else { 6 + (rng.next() * 8.99) as usize };
        let streets: Vec<[f32; 4]> = t.streets.iter().map(|&(way, length)| street_rect(t.square, way, [0.0, length], STREET_HALF)).collect();
        for (at, half, setback) in slots {
            if t.houses.len() >= want {
                break;
            }
            let (walls, stone, pattern) = t.style.walls(rng);
            let mut house = House {
                at,
                half,
                base: 0.0,
                floor: 0.0,
                wall: if walls == Walls::Logs { rng.range(2.5, 2.9) } else { rng.range(2.7, 3.2) },
                walls,
                stone,
                pattern,
                roof: t.style.roof(rng),
                setback,
                seed: hash(t.seed ^ hash(t.houses.len() as u32 + 1)),
            };
            if t.houses.iter().any(|h| overlaps(h.rect(), house.rect(), 0.6)) || streets.iter().any(|s| overlaps(*s, house.plinth(), 0.3)) {
                continue;
            }
            match &t.cavern {
                Some(c) => {
                    // On the flat floor, its eaves and its ridge well under the roof
                    house.base = t.square[2];
                    house.floor = t.square[2] + 0.15;
                    let [x, y, floor] = t.square;
                    let r = grown(house.plinth(), 0.4);
                    let eaves = [(r[0], r[1]), (r[2], r[1]), (r[0], r[3]), (r[2], r[3])]
                        .iter()
                        .all(|&(px, py)| c.headroom([px - x, py - y]).is_some_and(|h| h > house.floor + house.wall - floor + 0.6));
                    let ridge = c.headroom([at.x - x, at.y - y]).is_some_and(|h| h > house.top() - floor + 0.4);
                    if !eaves || !ridge {
                        continue;
                    }
                }
                None => {
                    let [hu, hv] = half;
                    let g: Vec<f32> = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0), (0.0, 0.0), (1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)]
                        .iter()
                        .map(|&(a, b)| {
                            let p = at.point(a * (hu + 0.12), b * (hv + 0.12), 0.0);
                            height_at(&self.settings, p[0], p[1])
                        })
                        .collect();
                    let (low, high) = (g.iter().copied().fold(f32::MAX, f32::min), g.iter().copied().fold(f32::MIN, f32::max));
                    if high - low > 1.4 {
                        continue;
                    }
                    house.base = low;
                    house.floor = high + 0.15;
                }
            }
            t.houses.push(house);
        }

        // Stalls on the corners of the square, between the streets
        let mut corners = [0, 1, 2, 3];
        for i in (1..4).rev() {
            let j = ((rng.next() * (i + 1) as f32) as usize).min(i);
            corners.swap(i, j);
        }
        for &k in corners.iter().take(1 + (rng.next() * 2.99) as usize) {
            let a = std::f32::consts::FRAC_PI_4 + k as f32 * FRAC_PI_2 + rng.range(-0.2, 0.2);
            let (x, y) = (t.square[0] + a.cos() * (t.radius - 2.2), t.square[1] + a.sin() * (t.radius - 2.2));
            let f = if a.cos().abs() > a.sin().abs() { [-a.cos().signum(), 0.0] } else { [0.0, -a.sin().signum()] };
            t.stalls.push(Frame { x, y, f });
        }
    }

    /// The top of the paving at x, y in town `t`: the flat floor of a cavern, or the ground itself,
    /// paved, out on the land
    fn paving(&self, t: &Town, x: f32, y: f32) -> f32 {
        if t.cavern.is_some() {
            return t.square[2];
        }
        height_at(&self.settings, x, y) + 0.03
    }

    /// Paves the ground from `lo` to `hi` in town `t` with `stone`, see pave(), or lays it flat
    /// on the floor of a cavern
    fn lay(&self, t: &Town, s: &mut Vec<Shape>, lo: [f32; 2], hi: [f32; 2], stone: (u8, u8)) {
        if t.cavern.is_some() {
            let z = t.square[2];
            s.push(Shape::block([lo[0], lo[1], z - 0.45], [hi[0], hi[1], z], stone.0, stone.1).placed(Placement::Clear));
            return;
        }
        let g = [(lo[0], lo[1]), (hi[0], lo[1]), (lo[0], hi[1]), (hi[0], hi[1])].map(|(x, y)| height_at(&self.settings, x, y));
        pave(s, lo, hi, g.iter().copied().fold(f32::MAX, f32::min) - 0.15, g.iter().copied().fold(f32::MIN, f32::max) + 0.15, stone);
    }

    /// Builds town `t`: what stands in it, then what it's paved with, then for a town in a cavern
    /// the cavern itself and the tunnel to it, so each wins over what comes after
    fn town(&self, t: &Town, features: &mut Features) -> Vec<Object> {
        let mut rng = Rng(t.seed);
        let mut objects = Vec::new();
        let [x, y, z] = t.square;
        let surface = t.cavern.is_none();
        for h in &t.houses {
            let mut s = Vec::new();
            self.house(h, t.style, &mut s, features);
            objects.push(Object::new(s, h.seed));
            // Nothing grows close round it, and no grass in the shader under it
            let p = h.plinth();
            features.footprints.push([p[0], p[1], p[2], p[3], h.floor + 0.3]);
            let r = grown(h.rect(), 2.5);
            features.footprints.push([r[0], r[1], r[2], r[3], 0.0]);
            features.holes.push(p);
        }

        // The well, the stalls, and lanterns lighting the square and the streets
        let mut s = Vec::new();
        self.well(t, &mut s, features);
        for stall in &t.stalls {
            self.stall(t, stall, &mut s, features, &mut rng);
        }
        for (k, &(way, length)) in t.streets.iter().enumerate() {
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            let out = [-way[1] * side, way[0] * side];
            // Where it leaves the square, and partway along
            for (along, by) in [(t.radius - 0.4, STREET_HALF + 0.8), (length * 0.62, STREET_HALF + 0.35)] {
                let (px, py) = (x + way[0] * along + out[0] * by, y + way[1] * along + out[1] * by);
                let foot = if along < t.radius { self.paving(t, px, py) } else if surface { height_at(&self.settings, px, py) + 0.03 } else { z };
                self.lantern_post(&mut s, features, [px, py, foot], [-out[0], -out[1]]);
            }
        }
        // On the corners of the square with no stall
        for k in 0..4 {
            let a = std::f32::consts::FRAC_PI_4 + k as f32 * FRAC_PI_2;
            let (px, py) = (x + a.cos() * (t.radius - 0.9), y + a.sin() * (t.radius - 0.9));
            if t.stalls.iter().all(|f| (f.x - px).hypot(f.y - py) > 3.0) && rng.next() < 0.6 {
                let f = if a.cos().abs() > a.sin().abs() { [-a.cos().signum(), 0.0] } else { [0.0, -a.sin().signum()] };
                self.lantern_post(&mut s, features, [px, py, self.paving(t, px, py)], f);
            }
        }
        if let Some(c) = &t.cavern {
            self.tunnel_fittings(t, c, &mut s, features);
        }
        objects.push(Object::new(s, rng.0));

        // The square, paved in stones of a few kinds, the streets, and paths from each door out to
        // its street
        let mut s = Vec::new();
        let paving = t.style.paving();
        let n = (t.radius / TILE).ceil() as i32;
        for j in -n..n {
            for i in -n..n {
                let (x0, y0) = (x + i as f32 * TILE, y + j as f32 * TILE);
                if (x0 + TILE * 0.5 - x).hypot(y0 + TILE * 0.5 - y) > t.radius {
                    continue;
                }
                // Mostly the one stone, some of the others here and there
                let pick = unit(hash3(i as u32, j as u32, t.seed));
                let stone = paving[if pick < 0.7 { 0 } else if pick < 0.9 { 1 } else { 2 }];
                self.lay(t, &mut s, [x0, y0], [x0 + TILE, y0 + TILE], stone);
            }
        }
        let r = t.radius;
        features.holes.push([x - r, y - r, x + r, y + r]);
        features.footprints.push([x - r, y - r, x + r, y + r, 0.0]);
        let street = t.style.street();
        for &(way, length) in &t.streets {
            let at = Frame { x, y, f: way };
            let mut a = t.radius - 0.15;
            while a < length - 0.05 {
                let b = (a + 2.0).min(length);
                let (lo, hi) = at.rect([a, b], [-STREET_HALF, STREET_HALF]);
                self.lay(t, &mut s, lo, hi, street);
                a = b;
            }
            let r = street_rect(t.square, way, [t.radius - 0.15, length], STREET_HALF);
            features.holes.push(r);
            features.footprints.push([r[0], r[1], r[2], r[3], 0.0]);
        }
        for h in &t.houses {
            let hu = h.half[0];
            let (lo, hi) = h.at.rect([hu + 0.12, hu + h.setback + 0.3], [-0.6, 0.6]);
            self.lay(t, &mut s, lo, hi, street);
            features.holes.push([lo[0], lo[1], hi[0], hi[1]]);
        }
        objects.push(Object::new(s, hash(t.seed ^ 0x9a7e)));

        match &t.cavern {
            Some(c) => objects.push(self.cavern_of(t, c, features)),
            None => {
                // A few trees round the outside of a town on the land
                for _ in 0..4 {
                    let a = rng.next() * TAU;
                    let d = rng.range(t.radius + 6.0, TOWN_RADIUS + 2.0);
                    let (px, py) = (x + a.cos() * d, y + a.sin() * d);
                    let spot = around([px, py], [1.0, 1.0]);
                    let crowded = t.houses.iter().any(|h| overlaps(h.rect(), spot, 3.5))
                        || t.streets.iter().any(|&(way, length)| overlaps(street_rect(t.square, way, [0.0, length], STREET_HALF), spot, 3.0));
                    if !crowded {
                        objects.push(self.tree(px, py, height_at(&self.settings, px, py), hash(rng.0 ^ 0x7ee)));
                    }
                }
            }
        }

        if self.inside(x, y) {
            let facing = match &t.cavern {
                Some(c) => c.way,
                None => t.streets[0].0,
            };
            features.sites.push(Site { kind: if surface { "town" } else { "cave town" }, position: t.square, facing });
        }
        objects
    }

    /// House `h`, of a town of `style`: walls with a doorway at the front and windows round the
    /// sides, a room inside with a bed and a table, its roof, and the footing it stands on, down
    /// into the ground. Maybe a lantern by its door, and a candle burning on its table.
    fn house(&self, h: &House, style: Style, s: &mut Vec<Shape>, features: &mut Features) {
        let at = h.at;
        let [hu, hv] = h.half;
        let (f, top) = (h.floor, h.floor + h.wall);
        let mut rng = Rng(h.seed);
        let wall = if h.walls == Walls::Logs { 0.34 } else { 0.32 };
        let (iu, iv) = (hu - wall, hv - wall);
        let block = |u: [f32; 2], v: [f32; 2], z: [f32; 2], material: (u8, u8)| at.block(u, v, z, material.0, material.1).placed(Placement::Clear);

        // The doorway, and windows down the sides, at the back and either side of the door
        s.push(at.room([iu - 0.1, hu + 0.3], [-0.5, 0.5], [f, f + 2.1]));
        let windows: Vec<f32> = if hu > 2.7 { vec![-hu * 0.45, hu * 0.4] } else { vec![-hu * 0.15] };
        let sill = if h.walls == Walls::Logs { (BARK_DARK, AIR) } else { (WOOD, BARK_DARK) };
        let shutters = [TEMPLE_TEAL, TEMPLE_RED, MOSS, WOOD][(rng.next() * 3.99) as usize];
        let painted = matches!(style, Style::Temperate | Style::Snowy) && rng.next() < 0.7;
        for side in [-1.0f32, 1.0] {
            for &u in &windows {
                s.push(at.room([u - 0.35, u + 0.35], [side * (iv - 0.1), side * (hv + 0.1)], [f + 1.05, f + 1.8]));
                s.push(block([u - 0.45, u + 0.45], [side * (hv - 0.05), side * (hv + 0.1)], [f + 0.96, f + 1.05], sill));
                if painted {
                    for k in [-1.0, 1.0] {
                        s.push(block([u + k * 0.38, u + k * 0.68], [side * (hv - 0.02), side * (hv + 0.07)], [f + 1.02, f + 1.83], (shutters, AIR)));
                    }
                }
            }
        }
        s.push(at.room([-hu - 0.1, -iu + 0.1], [-0.35, 0.35], [f + 1.05, f + 1.8]));
        if hv > 3.0 {
            for side in [-1.0f32, 1.0] {
                let v = side * hv * 0.62;
                s.push(at.room([iu - 0.1, hu + 0.1], [v - 0.35, v + 0.35], [f + 1.05, f + 1.8]));
                s.push(block([hu - 0.05, hu + 0.1], [v - 0.45, v + 0.45], [f + 0.96, f + 1.05], sill));
            }
        }
        // The lintel over the door
        s.push(block([iu - 0.05, hu + 0.08], [-0.65, 0.65], [f + 2.1, f + 2.3], (WOOD, BARK_DARK)));

        // A lantern by the door, or an awning over it in the desert
        if rng.next() < 0.4 {
            let side = if rng.next() < 0.5 { 1.0 } else { -1.0 };
            s.push(block([hu - 0.05, hu + 0.36], [side * 0.9 - 0.03, side * 0.9 + 0.03], [f + 2.32, f + 2.38], (DARK_STEEL, AIR)));
            let p = at.point(hu + 0.32, side * 0.9, f + 2.05);
            self.lantern(s, features, p, 8.0);
        } else if style == Style::Desert && rng.next() < 0.6 {
            let cloth = if rng.next() < 0.5 { (CLOTH, LINEN) } else { (LINEN, CLOTH) };
            s.push(block([hu, hu + 1.0], [-0.85, 0.85], [f + 2.32, f + 2.4], cloth));
            for v in [-0.8, 0.8] {
                s.push(Shape::capsule(at.point(hu + 0.95, v, f - 0.2), at.point(hu + 0.95, v, f + 2.32), 0.04, 0.04, WOOD, AIR, false).placed(Placement::Clear));
            }
        }

        // What's inside: a bed against the back wall, a table with a stool, maybe a candle, a
        // barrel in the corner by the door
        let side = if rng.next() < 0.5 { 1.0 } else { -1.0 };
        let blanket = [CLOTH, LINEN, MOSS, TEMPLE_TEAL][(rng.next() * 3.99) as usize];
        let bed = [side * (iv - 0.95), side * iv];
        s.push(block([-iu + 0.08, -iu + 0.5], bed, [f + 0.5, f + 0.62], (LINEN, AIR)));
        s.push(block([-iu, -iu + 1.95], bed, [f + 0.42, f + 0.56], (blanket, AIR)));
        s.push(block([-iu, -iu + 1.95], bed, [f, f + 0.42], (WOOD, BARK_DARK)));
        let (tu, tv) = (rng.range(-0.4, 0.3).min(iu - 1.2), -side * (iv - 0.75));
        s.push(block([tu - 0.5, tu + 0.5], [tv - 0.4, tv + 0.4], [f + 0.72, f + 0.8], (WOOD, AIR)));
        s.push(Shape::capsule(at.point(tu, tv, f), at.point(tu, tv, f + 0.72), 0.07, 0.07, BARK_DARK, AIR, false).placed(Placement::Clear));
        s.push(block([tu + 0.65, tu + 0.95], [tv - 0.15, tv + 0.15], [f, f + 0.45], (WOOD, BARK_DARK)));
        // Whichever piece of the landscape it's in, so the random numbers run on the same way for
        // every piece the house is built for; those of other pieces aren't kept, see Terrain::place()
        if rng.next() < 0.6 {
            candle(features, at.point(tu, tv + side * 0.15, f + 0.8), &mut rng);
        }
        barrel(s, at.point(iu - 0.38, side * (iv - 0.38), f));

        // The room inside, and its floorboards
        s.push(at.room([-iu, iu], [-iv, iv], [f, top]));
        s.push(block([-iu, iu], [-iv, iv], [f - 0.12, f], (WOOD, BARK_DARK)));

        // The walls, framed in timber, banded, or of logs
        match h.walls {
            Walls::Plaster => {
                let timber = (WOOD, BARK_DARK);
                for (su, sv) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                    s.push(block([su * (hu + 0.07), su * (hu - 0.2)], [sv * (hv + 0.07), sv * (hv - 0.2)], [f - 0.1, top + 0.02], timber));
                }
                for (z0, z1) in [(f - 0.1, f + 0.12), (f + 0.95, f + 1.05), (top - 0.2, top + 0.02)] {
                    for su in [-1.0f32, 1.0] {
                        s.push(block([su * (hu + 0.07), su * (hu - 0.1)], [-hv, hv], [z0, z1], timber));
                    }
                    for sv in [-1.0f32, 1.0] {
                        s.push(block([-hu, hu], [sv * (hv + 0.07), sv * (hv - 0.1)], [z0, z1], timber));
                    }
                }
                // Between the windows and the corners
                for sv in [-1.0f32, 1.0] {
                    for u in [-hu * 0.75, hu * 0.75] {
                        s.push(block([u - 0.09, u + 0.09], [sv * (hv + 0.07), sv * (hv - 0.1)], [f, top], timber));
                    }
                }
            }
            Walls::Temple => {
                let paint = if rng.next() < 0.5 { TEMPLE_TEAL } else { TEMPLE_RED };
                s.push(block([-hu - 0.07, hu + 0.07], [-hv - 0.07, hv + 0.07], [top - 0.55, top - 0.3], (paint, AIR)));
                s.push(block([-hu - 0.07, hu + 0.07], [-hv - 0.07, hv + 0.07], [f - 0.1, f + 0.35], (TEMPLE_DARK, MOSS)));
            }
            Walls::Adobe => {
                // The ends of the beams holding up the roof
                let mut v = -hv + 0.45;
                while v < hv - 0.3 {
                    for su in [-1.0f32, 1.0] {
                        s.push(Shape::capsule(at.point(su * (hu - 0.3), v, top - 0.12), at.point(su * (hu + 0.42), v, top - 0.12), 0.075, 0.075, WOOD, BARK_DARK, false).placed(Placement::Clear));
                    }
                    v += 0.8;
                }
            }
            _ => {}
        }
        match h.walls {
            Walls::Logs => {
                let (r, course) = (0.17, 0.3);
                let courses = (h.wall / course).ceil() as usize;
                for k in 0..courses {
                    let z = f + 0.1 + k as f32 * course;
                    for side in [-1.0f32, 1.0] {
                        let v = side * (hv - r);
                        s.push(Shape::capsule(at.point(-hu - 0.22, v, z), at.point(hu + 0.22, v, z), r, r, h.stone.0, h.stone.1, false).placed(Placement::Clear));
                        let u = side * (hu - r);
                        let z = (z + course * 0.5).min(top);
                        s.push(Shape::capsule(at.point(u, -hv - 0.22, z), at.point(u, hv + 0.22, z), r, r, h.stone.0, h.stone.1, false).placed(Placement::Clear));
                    }
                }
                // Filling in under the first course
                s.push(block([-hu, hu], [-hv, hv], [f - 0.3, f + 0.1], (BARK_DARK, WOOD)));
            }
            _ => s.push(at.block([-hu, hu], [-hv, hv], [f - 0.3, top], h.stone.0, h.stone.1).placed(Placement::Clear).patterned(h.pattern)),
        }

        match h.roof {
            Some(roof) => self.gable(h, roof, s, &mut rng),
            None => {
                // Flat, with a low wall round it, and a dome on some
                if rng.next() < 0.3 {
                    let r = hu.min(hv) * 0.55;
                    s.push(Shape::sphere(at.point(0.0, 0.0, top + 0.25), r, h.stone.0, h.stone.1, false).placed(Placement::Clear));
                }
                for su in [-1.0f32, 1.0] {
                    s.push(block([su * (hu + 0.12), su * (hu - 0.15)], [-hv - 0.12, hv + 0.12], [top + 0.25, top + 0.7], h.stone));
                }
                for sv in [-1.0f32, 1.0] {
                    s.push(block([-hu - 0.12, hu + 0.12], [sv * (hv + 0.12), sv * (hv - 0.15)], [top + 0.25, top + 0.7], h.stone));
                }
                s.push(block([-hu - 0.12, hu + 0.12], [-hv - 0.12, hv + 0.12], [top, top + 0.25], h.stone));
            }
        }

        // Steps up to the door, and the footing it all stands on
        let stone = style.stone();
        let front = at.point(hu + 0.6, 0.0, 0.0);
        let ground = height_at(&self.settings, front[0], front[1]).min(f);
        let drop = f - ground;
        let steps = ((drop - 0.2) / 0.32).ceil().max(0.0) as usize;
        for k in 0..steps {
            let rise = drop / (steps + 1) as f32;
            let u = hu + 0.12 + k as f32 * 0.36;
            s.push(block([u, u + 0.36], [-0.75, 0.75], [ground - 0.3, f - (k + 1) as f32 * rise], stone).patterned(Pattern::Bricks));
        }
        s.push(block([-hu - 0.12, hu + 0.12], [-hv - 0.12, hv + 0.12], [h.base - 0.6, f + 0.12], stone).patterned(Pattern::Bricks));
    }

    /// The pitched roof of house `h`: rows of tiles or shingles down either side of its ridge,
    /// snow lying on them, maybe a chimney, and the gable ends filled in under it
    fn gable(&self, h: &House, roof: Roof, s: &mut Vec<Shape>, rng: &mut Rng) {
        let at = h.at;
        let [hu, hv] = h.half;
        let top = h.floor + h.wall;
        let along_u = h.ridge_along_u();
        let (long, across) = if along_u { (hu + GABLE_OVERHANG, hv + EAVES_OVERHANG) } else { (hv + GABLE_OVERHANG, hu + EAVES_OVERHANG) };
        let rise = h.rise();
        let eaves = top - 0.25;
        // `a` along the ridge, `b` across it
        let point = |a: f32, b: f32, z: f32| if along_u { at.point(a, b, z) } else { at.point(b, a, z) };
        let log = |a: f32, b: f32, z: f32, r: f32, material: (u8, u8)| {
            Shape::capsule(point(-a, b, z), point(a, b, z), r, r, material.0, material.1, false).placed(Placement::Clear)
        };
        let rows = (across * (1.0 + roof.pitch * roof.pitch).sqrt() / 0.24).ceil() as usize;
        let row = |k: usize| (across * (1.0 - k as f32 / rows as f32), eaves + rise * k as f32 / rows as f32);

        // A chimney on many, though not through thatch
        if h.walls != Walls::Temple && (h.walls != Walls::Logs || rng.next() < 0.5) && rng.next() < 0.6 {
            let (a, b) = (rng.range(-0.5, 0.5) * (long - 1.2), if rng.next() < 0.5 { 1.0 } else { -1.0 } * across * 0.45);
            let c = point(a, b, 0.0);
            s.push(Shape::block([c[0] - 0.3, c[1] - 0.3, top - 0.3], [c[0] + 0.3, c[1] + 0.3, eaves + rise + 0.6], BRICK_STONE, MORTAR).placed(Placement::Clear).patterned(Pattern::Bricks));
        }
        if roof.snow {
            s.push(log(long + 0.04, 0.0, eaves + rise + 0.14, 0.2, (SNOW, AIR)));
            for k in 0..rows {
                let (b, z) = row(k);
                for side in [-1.0f32, 1.0] {
                    s.push(log(long + 0.04, side * b, z + 0.1, 0.16, (SNOW, AIR)));
                }
            }
        }
        s.push(log(long, 0.0, eaves + rise + 0.04, 0.2, (roof.ridge, AIR)));
        for k in 0..rows {
            let (b, z) = row(k);
            for side in [-1.0f32, 1.0] {
                s.push(log(long, side * b, z, 0.17, roof.cover));
            }
        }
        // The gable ends, stepping in up under the roof
        let mut z = top - 0.05;
        loop {
            let w = across * (1.0 - (z + 0.25 - eaves + 0.17) / rise);
            if w < 0.1 {
                break;
            }
            let (lo, hi) = (point(-(long - GABLE_OVERHANG), -w, z), point(long - GABLE_OVERHANG, w, z + 0.25));
            s.push(Shape::block(lo, hi, h.stone.0, h.stone.1).placed(Placement::Clear).patterned(h.pattern));
            z += 0.25;
        }
    }

    /// The well in the middle of the square: a round stone wall round a deep shaft, a little roof
    /// over it on two posts, a windlass and a bucket on its rope. In the desert and the jungle a
    /// fountain instead, a pillar rising from a basin of still water.
    fn well(&self, t: &Town, s: &mut Vec<Shape>, features: &mut Features) {
        let [x, y, _] = t.square;
        let g = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)].iter().map(|&(dx, dy)| self.paving(t, x + dx, y + dy)).fold(f32::MIN, f32::max);
        let stone = t.style.stone();
        let clear = |shape: Shape| shape.placed(Placement::Clear);
        if t.cavern.is_none() && matches!(t.style, Style::Desert | Style::Jungle) {
            let (outer, inner) = (1.8, 1.45);
            let cap = if t.style == Style::Desert { (LIMESTONE, SANDSTONE) } else { (TEMPLE_STONE, MOSS) };
            s.push(clear(Shape::ellipsoid([x, y, g + 1.5], [0.55, 0.55, 0.12], cap.0, cap.1)));
            s.push(clear(Shape::capsule([x, y, g], [x, y, g + 1.45], 0.24, 0.2, cap.0, cap.1, false)).patterned(Pattern::Carved));
            s.push(clear(Shape::room([x - inner, y - inner, g + 0.15], [x + inner, y + inner, g + 1.0])));
            // A coping of paler stone round the top of the basin's wall
            s.push(clear(Shape::block([x - outer - 0.06, y - outer - 0.06, g + 0.5], [x + outer + 0.06, y + outer + 0.06, g + 0.62], cap.0, cap.1)));
            s.push(clear(Shape::block([x - outer, y - outer, g - 0.4], [x + outer, y + outer, g + 0.5], stone.0, stone.1)).patterned(Pattern::Bricks));
            features.floors.push(Floor { lo: [x - inner, y - inner], hi: [x + inner, y + inner], height: g + 0.15, water: g + 0.48 });
            features.footprints.push([x - outer, y - outer, x + outer, y + outer, g + 0.7]);
            return;
        }
        // The little roof, its ridge along the windlass, on posts up to the ridge
        let (eaves, rise) = (g + 2.3, 0.85);
        s.push(clear(Shape::capsule([x - 1.15, y, eaves + rise + 0.04], [x + 1.15, y, eaves + rise + 0.04], 0.12, 0.12, WOOD, AIR, false)));
        for k in 0..7 {
            let t = k as f32 / 7.0;
            let (b, z) = (1.0 * (1.0 - t), eaves + rise * t);
            for side in [-1.0, 1.0] {
                s.push(clear(Shape::capsule([x - 1.1, y + side * b, z], [x + 1.1, y + side * b, z], 0.11, 0.11, BARK_DARK, WOOD, false)));
            }
        }
        for sx in [-1.0, 1.0] {
            s.push(clear(Shape::block([x + sx * 0.82 - 0.07, y - 0.07, g + 0.4], [x + sx * 0.82 + 0.07, y + 0.07, eaves + rise], WOOD, AIR)));
        }
        s.push(clear(Shape::capsule([x - 0.8, y, g + 1.95], [x + 0.8, y, g + 1.95], 0.08, 0.08, WOOD, BARK_DARK, false)));
        s.push(clear(Shape::block([x + 0.89, y - 0.02, g + 1.72], [x + 1.0, y + 0.02, g + 1.98], DARK_STEEL, AIR)));
        s.push(clear(Shape::capsule([x, y, g + 1.9], [x, y, g + 1.3], 0.02, 0.02, LINEN, AIR, false)));
        s.push(clear(Shape::ellipsoid([x, y, g + 1.31], [0.16, 0.16, 0.025], DARK_STEEL, AIR)));
        s.push(clear(Shape::capsule([x, y, g + 1.12], [x, y, g + 1.22], 0.14, 0.14, WOOD, BARK_DARK, false)));
        // The shaft, and the top of the wall cut flat
        s.push(clear(Shape::capsule([x, y, g - 3.0], [x, y, g + 1.0], 0.6, 0.6, AIR, AIR, false)));
        s.push(Shape::room([x - 1.0, y - 1.0, g + 0.7], [x + 1.0, y + 1.0, g + 1.6]));
        s.push(clear(Shape::capsule([x, y, g - 0.6], [x, y, g + 0.2], 0.95, 0.95, stone.0, stone.1, false)).patterned(Pattern::Bricks));
        features.footprints.push([x - 1.0, y - 1.0, x + 1.0, y + 1.0, g + 0.75]);
    }

    /// A merchant's stall at `at` on the square of town `t`, facing the middle of it: a counter
    /// of goods under a striped awning on four posts, a crate and a barrel beside it, and the
    /// merchant's spot behind it, "town stall"
    fn stall(&self, t: &Town, at: &Frame, s: &mut Vec<Shape>, features: &mut Features, rng: &mut Rng) {
        let g = self.paving(t, at.x, at.y);
        let clear = |shape: Shape| shape.placed(Placement::Clear);
        let (cloth, stripe) = [(CLOTH, LINEN), (TEMPLE_TEAL, LINEN), (MOSS, LINEN), (TEMPLE_RED, FLOWER_YELLOW)][(rng.next() * 3.99) as usize];
        let counter = g + 0.92;
        // The goods along the counter
        let mut v = -0.8;
        while v < 0.85 {
            let p = at.point(rng.range(-0.1, 0.1), v, counter);
            match (rng.next() * 5.0) as usize {
                0 => {
                    s.push(clear(Shape::capsule([p[0], p[1], p[2] + 0.2], [p[0], p[1], p[2] + 0.3], 0.06, 0.07, CLAY_POT, AIR, false)));
                    s.push(clear(Shape::sphere([p[0], p[1], p[2] + 0.13], 0.13, CLAY_POT, SANDSTONE_DARK, false)));
                }
                1 => {
                    for (du, dv, dz) in [(-0.07, -0.07, 0.06), (0.07, -0.05, 0.06), (-0.05, 0.07, 0.06), (0.07, 0.07, 0.06), (0.0, 0.0, 0.16)] {
                        let q = at.point(du, v + dv, counter + dz);
                        s.push(clear(Shape::sphere(q, 0.065, APPLE, AIR, false)));
                    }
                }
                2 => {
                    for dv in [-0.09, 0.09] {
                        let q = at.point(0.0, v + dv, counter + 0.06);
                        s.push(clear(Shape::ellipsoid(q, [0.07, 0.12, 0.06], SANDSTONE_DARK, AIR)));
                    }
                }
                3 => s.push(clear(Shape::ellipsoid([p[0], p[1], p[2] + 0.07], [0.15, 0.15, 0.07], FLOWER_YELLOW, AIR))),
                _ => {
                    let bolt = [CLOTH, TEMPLE_TEAL, LINEN][(rng.next() * 2.99) as usize];
                    s.push(clear(Shape::capsule(at.point(-0.22, v, counter + 0.08), at.point(0.22, v, counter + 0.08), 0.08, 0.08, bolt, AIR, false)));
                }
            }
            v += 0.42;
        }
        // The counter, the awning over it and the posts holding it up
        s.push(clear(at.block([-0.35, 0.35], [-1.05, 1.05], [g - 0.2, counter], WOOD, AIR)));
        for k in 0..6 {
            let v0 = -1.25 + k as f32 * 2.5 / 6.0;
            let material = if k % 2 == 0 { cloth } else { stripe };
            s.push(clear(at.block([-0.55, 0.8], [v0, v0 + 2.5 / 6.0], [g + 2.3, g + 2.38], material, AIR)));
        }
        s.push(clear(at.block([0.74, 0.8], [-1.25, 1.25], [g + 2.1, g + 2.3], cloth, AIR)));
        for (u, v) in [(-0.45, -1.15), (-0.45, 1.15), (0.68, -1.15), (0.68, 1.15)] {
            s.push(clear(at.block([u - 0.04, u + 0.04], [v - 0.04, v + 0.04], [g - 0.2, g + 2.3], WOOD, AIR)));
        }
        let c = at.point(-0.75, 1.6, g);
        crate_box(s, c, 0.28);
        barrel(s, at.point(-0.8, -1.65, g));

        let p = at.point(-1.0, 0.0, 0.0);
        let spot = [p[0], p[1], self.paving(t, p[0], p[1])];
        if self.inside(spot[0], spot[1]) {
            features.sites.push(Site { kind: "town stall", position: spot, facing: at.f });
        }
    }

    /// A post standing at `foot`, on the ground, with a lantern hung from an arm reaching `out`
    fn lantern_post(&self, s: &mut Vec<Shape>, features: &mut Features, foot: Vec3, out: [f32; 2]) {
        let [x, y, z] = foot;
        let top = z + 2.75;
        let clear = |lo: Vec3, hi: Vec3, material: u8| Shape::block(lo, hi, material, AIR).placed(Placement::Clear);
        let tip = [x + out[0] * 0.55, y + out[1] * 0.55];
        s.push(clear([x.min(tip[0]) - 0.04, y.min(tip[1]) - 0.04, top - 0.12], [x.max(tip[0]) + 0.04, y.max(tip[1]) + 0.04, top - 0.04], DARK_STEEL));
        s.push(clear([tip[0] - 0.015, tip[1] - 0.015, top - 0.2], [tip[0] + 0.015, tip[1] + 0.015, top - 0.12], DARK_STEEL));
        s.push(clear([x - 0.08, y - 0.08, z - 0.4], [x + 0.08, y + 0.08, top], BARK_DARK));
        s.push(clear([x - 0.16, y - 0.16, z - 0.3], [x + 0.16, y + 0.16, z + 0.15], STONE));
        self.lantern(s, features, [tip[0], tip[1], top - 0.47], 12.0);
    }

    /// A lantern with its flame at `at`, in a bronze frame with iron bars at its corners, lighting
    /// up `reach` meters round
    fn lantern(&self, s: &mut Vec<Shape>, features: &mut Features, at: Vec3, reach: f32) {
        let [x, y, z] = at;
        let clear = |lo: Vec3, hi: Vec3, material: u8| Shape::block(lo, hi, material, AIR).placed(Placement::Clear);
        s.push(clear([x - 0.06, y - 0.06, z + 0.19], [x + 0.06, y + 0.06, z + 0.27], BRONZE));
        s.push(clear([x - 0.14, y - 0.14, z + 0.13], [x + 0.14, y + 0.14, z + 0.19], BRONZE));
        for (dx, dy) in [(-0.11, -0.11), (0.11, -0.11), (-0.11, 0.11), (0.11, 0.11)] {
            s.push(clear([x + dx - 0.025, y + dy - 0.025, z - 0.13], [x + dx + 0.025, y + dy + 0.025, z + 0.13], DARK_STEEL));
        }
        s.push(clear([x - 0.085, y - 0.085, z - 0.11], [x + 0.085, y + 0.085, z + 0.11], FLAME));
        s.push(clear([x - 0.13, y - 0.13, z - 0.18], [x + 0.13, y + 0.13, z - 0.12], BRONZE));
        // Just under it, so the lantern itself doesn't stand in the way of its light
        if self.inside(x, y) {
            features.lights.push(Light { position: [x, y, z - 0.35], color: LANTERN_LIGHT, reach });
        }
    }

    /// How far out from the middle of the square the tunnel up from cavern `c` of town `t` comes
    /// out from under the hill, into the cutting up to its mouth
    fn tunnel_face(&self, t: &Town, c: &Cavern) -> f32 {
        let [x, y, floor] = t.square;
        let mut d = c.mouth;
        while d > c.gate + 2.0 && height_at(&self.settings, x + c.way[0] * d, y + c.way[1] * d) < floor + c.tunnel_floor(d) + TUNNEL_MIDDLE + TUNNEL_RADIUS + 0.3 {
            d -= 0.5;
        }
        d
    }

    /// What's built along the tunnel up from a town in a cavern: posts and beams holding up its
    /// roof, lanterns hung from some, a frame of timber where it goes in under the hill, and
    /// lanterns either side of where the cutting up to it starts
    fn tunnel_fittings(&self, t: &Town, c: &Cavern, s: &mut Vec<Shape>, features: &mut Features) {
        let [x, y, floor] = t.square;
        let at = Frame { x, y, f: c.way };
        let timber = (WOOD, BARK_DARK);
        let block = |u: [f32; 2], v: [f32; 2], z: [f32; 2], material: (u8, u8)| at.block(u, v, z, material.0, material.1).placed(Placement::Clear);
        let face = self.tunnel_face(t, c);
        let mut d = c.gate + 1.5;
        let mut k = 0;
        while d < face - 1.5 {
            let z = floor + c.tunnel_floor(d);
            for side in [-1.0f32, 1.0] {
                s.push(block([d - 0.1, d + 0.1], [side * 1.25 - 0.09, side * 1.25 + 0.09], [z - 0.2, z + 2.6], timber));
            }
            s.push(block([d - 0.12, d + 0.12], [-1.7, 1.7], [z + 2.55, z + 2.78], timber));
            if k % 2 == 0 {
                s.push(block([d - 0.015, d + 0.015], [-0.015, 0.015], [z + 2.45, z + 2.55], (DARK_STEEL, AIR)));
                self.lantern(s, features, at.point(d, 0.0, z + 2.18), 9.0);
            }
            d += 3.5;
            k += 1;
        }
        // Where it goes in under the hill, framed in timber, a lantern hung inside
        let (m, z) = (face - 0.2, floor + c.tunnel_floor(face - 0.2));
        for side in [-1.0f32, 1.0] {
            s.push(block([m - 0.14, m + 0.14], [side * 1.32 - 0.14, side * 1.32 + 0.14], [z - 0.4, z + 2.85], timber));
        }
        s.push(block([m - 0.16, m + 0.16], [-1.9, 1.9], [z + 2.8, z + 3.1], timber));
        s.push(block([m - 0.6, m - 0.57], [-0.015, 0.015], [z + 2.45, z + 2.8], (DARK_STEEL, AIR)));
        self.lantern(s, features, at.point(m - 0.585, 0.0, z + 2.18), 9.0);
        // Lanterns either side of the mouth, lighting the way in
        for side in [-1.0f32, 1.0] {
            let p = at.point(c.mouth + 1.0, side * 2.5, 0.0);
            let out = at.dir([0.0, -side]);
            self.lantern_post(s, features, [p[0], p[1], height_at(&self.settings, p[0], p[1])], out);
        }
        // Nothing grows in the way in
        let (lo, hi) = at.rect([face - 2.0, c.mouth + 6.0], [-3.5, 3.5]);
        features.footprints.push([lo[0], lo[1], hi[0], hi[1], 0.0]);
        features.holes.push([lo[0], lo[1], hi[0], hi[1]]);
    }

    /// The great cavern town `t` is down in: its flat floor over the rounded bottom of the hollow
    /// cut for it, lobes off its sides, the tunnel up to the hillside and its floor, crystals glowing
    /// round the sides and a great cluster of them hanging from the middle of the roof, lighting
    /// the whole town, and rocks hanging from the roof round the sides
    fn cavern_of(&self, t: &Town, c: &Cavern, features: &mut Features) -> Object {
        let [x, y, floor] = t.square;
        let mut rng = Rng(hash(t.seed ^ 0xca7e));
        let mut s = Vec::new();
        let (middle, dome) = ((1.0 - DOME) * c.height, DOME * c.height);
        let wide = c.radii.map(|r| r / floor_width());
        let street = t.style.street();
        s.push(Shape::block([x - c.radii[0], y - c.radii[1], floor + middle - dome - 0.3], [x + c.radii[0], y + c.radii[1], floor], GRAVEL, STONE).placed(Placement::Clear));
        s.push(Shape::ellipsoid([x, y, floor + middle], [wide[0], wide[1], dome], AIR, AIR).placed(Placement::Carve));
        let tunnel = c.way[1].atan2(c.way[0]);
        for k in 0..2 {
            let a = tunnel + FRAC_PI_2 + k as f32 * std::f32::consts::PI + rng.range(-0.6, 0.6);
            let (r, high) = (rng.range(6.0, 8.0), rng.range(4.0, 5.5));
            let center = [x + a.cos() * wide[0] * 0.72, y + a.sin() * wide[1] * 0.72, floor + high];
            s.push(Shape::ellipsoid(center, [r, r, high], AIR, AIR).placed(Placement::Carve));
        }

        // The tunnel, and its floor in steps no higher than a stair
        let at = Frame { x, y, f: c.way };
        let end = c.mouth + 4.0;
        let mut d = c.gate - 3.0;
        while d < end {
            let e = (d + 2.0).min(end);
            let (a, b) = (at.point(d, 0.0, floor + c.tunnel_floor(d) + TUNNEL_MIDDLE), at.point(e, 0.0, floor + c.tunnel_floor(e) + TUNNEL_MIDDLE));
            s.push(Shape::capsule(a, b, TUNNEL_RADIUS, TUNNEL_RADIUS, AIR, AIR, false).placed(Placement::Carve));
            d = e;
        }
        let mut d = c.gate - 1.0;
        while d < end {
            let z = floor + c.tunnel_floor(d + 0.5);
            if d < c.mouth {
                s.push(at.block([d, d + 1.0], [-1.75, 1.75], [z - 0.9, z], street.0, street.1).placed(Placement::Clear));
            } else {
                // Out of the mouth, a path paved into the ground
                let (lo, hi) = at.rect([d, d + 1.0], [-1.2, 1.2]);
                let g = [(lo[0], lo[1]), (hi[0], lo[1]), (lo[0], hi[1]), (hi[0], hi[1])].map(|(x, y)| height_at(&self.settings, x, y));
                pave(&mut s, lo, hi, g.iter().copied().fold(f32::MAX, f32::min) - 0.15, g.iter().copied().fold(f32::MIN, f32::max) + 0.15, street);
            }
            d += 1.0;
        }

        // Crystals glowing round the sides, on the corners between the streets
        for k in 0..5 {
            let a = tunnel + std::f32::consts::FRAC_PI_4 + k as f32 * TAU / 5.0 + rng.range(-0.2, 0.2);
            let r = rng.range(0.86, 0.97);
            let at = [x + a.cos() * c.radii[0] * r, y + a.sin() * c.radii[1] * r, floor];
            for _ in 0..6 {
                let offset = [rng.range(-0.7, 0.7), rng.range(-0.7, 0.7), rng.range(0.0, 0.7)];
                s.push(Shape::sphere(add(at, offset), rng.range(0.2, 0.5), CRYSTAL, AIR, false).placed(Placement::Inside));
            }
            if self.inside(at[0], at[1]) {
                features.lights.push(Light { position: [at[0], at[1], at[2] + 1.4], color: [0.4, 0.85, 1.0], reach: 13.0 });
            }
        }
        // The great cluster hanging over the square
        let roof = floor + c.height;
        s.push(Shape::capsule([x, y, roof + 0.5], [x, y, roof - 2.4], 1.1, 0.15, CRYSTAL, AIR, false).placed(Placement::Inside));
        for _ in 0..6 {
            let a = rng.next() * TAU;
            let tip = [x + a.cos() * 1.5, y + a.sin() * 1.5, roof - rng.range(1.0, 2.0)];
            s.push(Shape::capsule([x, y, roof + 0.3], tip, 0.5, 0.08, CRYSTAL, AIR, false).placed(Placement::Inside));
        }
        if self.inside(x, y) {
            features.lights.push(Light { position: [x, y, roof - 3.4], color: [0.85, 0.92, 1.0], reach: 36.0 });
        }
        // Rocks hanging from the roof round the sides, clear of the roofs of the houses
        for _ in 0..12 {
            let a = rng.next() * TAU;
            let r = rng.range(0.5, 0.85);
            let d = [a.cos() * c.radii[0] * r, a.sin() * c.radii[1] * r];
            let Some(room) = c.headroom(d) else {
                continue;
            };
            let long = rng.range(1.0, 2.6).min(room - 6.8);
            if long < 0.5 {
                continue;
            }
            let top = floor + room;
            s.push(Shape::capsule([x + d[0], y + d[1], top + 0.5], [x + d[0], y + d[1], top - long], rng.range(0.35, 0.7), 0.05, LIMESTONE, STONE, false).placed(Placement::Inside));
        }
        Object::new(s, rng.0)
    }
}

/// The rectangle along a street going `way` from `middle`, from `a[0]` to `a[1]` along it and
/// `half` either side, x0, y0, x1, y1
fn street_rect(middle: Vec3, way: [f32; 2], a: [f32; 2], half: f32) -> [f32; 4] {
    let at = Frame { x: middle[0], y: middle[1], f: way };
    let (lo, hi) = at.rect(a, [-half, half]);
    [lo[0], lo[1], hi[0], hi[1]]
}

/// Paves the ground from `lo` to `hi`, x and y in meters, where it's between about `low` and
/// `high`: what the ground's made of is cut away and `stone` put in its place, following it voxel
/// for voxel, and what grew on it is cleared away. Caves cut later don't get through it.
fn pave(s: &mut Vec<Shape>, lo: [f32; 2], hi: [f32; 2], low: f32, high: f32, stone: (u8, u8)) {
    let (bottom, top) = ([lo[0], lo[1], low - 0.4], [hi[0], hi[1], high + 0.1]);
    s.push(Shape::block(bottom, top, AIR, AIR).placed(Placement::Carve));
    s.push(Shape::block(bottom, top, stone.0, stone.1).placed(Placement::Inside));
    // Only over the ground, as the stone's already taken what's in it
    s.push(Shape::room([lo[0], lo[1], low], [hi[0], hi[1], high + 0.6]));
}

/// A barrel standing at `at`, bound with iron hoops
fn barrel(s: &mut Vec<Shape>, at: Vec3) {
    let [x, y, z] = at;
    // The round cut off its top, so its lid is flat
    s.push(Shape::room([x - 0.34, y - 0.34, z + 0.82], [x + 0.34, y + 0.34, z + 1.1]));
    for h in [0.2, 0.62] {
        s.push(Shape::ellipsoid([x, y, z + h], [0.31, 0.31, 0.035], DARK_STEEL, AIR).placed(Placement::Clear));
    }
    s.push(Shape::capsule([x, y, z + 0.28], [x, y, z + 0.56], 0.28, 0.28, WOOD, AIR, false).placed(Placement::Clear));
}

/// A wooden crate `half` meters either way of its middle, standing at `at`
fn crate_box(s: &mut Vec<Shape>, at: Vec3, half: f32) {
    let [x, y, z] = at;
    // Bands of darker wood round its top and bottom
    for z0 in [z, z + half * 2.0 - 0.07] {
        s.push(Shape::block([x - half - 0.03, y - half - 0.03, z0], [x + half + 0.03, y + half + 0.03, z0 + 0.07], BARK_DARK, AIR).placed(Placement::Clear));
    }
    s.push(Shape::block([x - half, y - half, z - 0.1], [x + half, y + half, z + half * 2.0], WOOD, AIR).placed(Placement::Clear));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Towns turn up out on the land and down in caverns, their houses clear of each other and of
    /// the streets, and the caverns' houses under their roofs
    #[test]
    fn towns_turn_up() {
        let settings = TerrainSettings { width_m: 32, height_m: 64, voxels_per_meter: 4, seed: 1, grass_tufts: false };
        let (terrain, _) = generate_terrain(&settings, [0, 0], &RiverCache::default());
        let cache = RiverCache::default();
        let (mut land, mut caverns) = (0, 0);
        for cy in -5..5 {
            for cx in -5..5 {
                let Some(t) = terrain.town_in(cx, cy, &cache) else {
                    continue;
                };
                if t.cavern.is_some() {
                    caverns += 1;
                } else {
                    land += 1;
                }
                assert!((2..=4).contains(&t.streets.len()) && !t.stalls.is_empty());
                for (i, a) in t.houses.iter().enumerate() {
                    assert!(t.houses[i + 1..].iter().all(|b| !overlaps(a.plinth(), b.plinth(), 0.0)));
                    if let Some(c) = &t.cavern {
                        let d = [a.at.x - t.square[0], a.at.y - t.square[1]];
                        assert!(c.headroom(d).is_some_and(|h| h > a.top() - t.square[2]));
                    }
                }
            }
        }
        println!("towns out on the land {land}, down in caverns {caverns}");
        assert!(land > 0 && caverns > 0);
    }
}
