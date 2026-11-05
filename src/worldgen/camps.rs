//! Bandits' camps, at most one in each CAMP_CELL square, on dry ground clear of buildings and
//! dungeons. There are several kinds, and which a camp is goes by the land round it and a roll of
//! its own: a fort of sharpened logs with a gate and a watchtower; a hamlet of log cabins; an old
//! stone ruin they've patched up and moved into; a hideout dug into a hillside behind a yard;
//! lean-tos of poles and boughs out in the woods; striped awnings on poles in the desert; and round
//! tents of fur about a great fire in the snow. Each is laid out from numbers of its own, so no two
//! look alike.
//!
//! Every camp has a fire in the middle where its bandits keep, "bandit camp", facing the way in,
//! and a spot where the chest of their hoard turns up once they're all dead, see Camp.

use super::*;
use std::f32::consts::PI;

/// At most one camp in each CAMP_CELL square, in CAMP_CHANCE of them, at the first of SPOTS spots
/// tried in it that suits
const CAMP_CELL: f32 = 120.0;
const CAMP_CHANCE: f32 = 0.7;
const SPOTS: u32 = 6;
/// The light a camp's fire gives, and a torch's
const FIRE_LIGHT: [f32; 3] = [1.0, 0.55, 0.22];
const TORCH_LIGHT: [f32; 3] = [1.0, 0.6, 0.25];

/// What kind of camp it is
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CampKind {
    /// A ring or a square of sharpened logs, a gate and a watchtower, huts and tents inside
    Fort,
    /// A hamlet of log cabins round a fire
    Cabins,
    /// The broken walls and tower of an old stone keep, patched with stakes, hides over a corner
    Ruin,
    /// A cave dug into a hillside and shored up with timber, a yard with a fire out front
    Hideout,
    /// Lean-tos of poles under hides or boughs, out in the woods
    Raiders,
    /// Striped awnings on poles over rugs, behind a low wall of mud brick, in the desert
    Awnings,
    /// Round tents of fur about a great fire, banked round with snow
    Yurts,
}

impl CampKind {
    pub const ALL: [CampKind; 7] = [CampKind::Fort, CampKind::Cabins, CampKind::Ruin, CampKind::Hideout, CampKind::Raiders, CampKind::Awnings, CampKind::Yurts];

    /// What it's called, as in `--set camp=fort`
    pub fn name(self) -> &'static str {
        match self {
            CampKind::Fort => "fort",
            CampKind::Cabins => "cabins",
            CampKind::Ruin => "ruin",
            CampKind::Hideout => "hideout",
            CampKind::Raiders => "raiders",
            CampKind::Awnings => "awnings",
            CampKind::Yurts => "yurts",
        }
    }

    pub fn named(name: &str) -> Option<CampKind> {
        CampKind::ALL.into_iter().find(|k| k.name() == name)
    }

    /// The kind of camp for land of climate `c`, picked by `roll`, 0-1. Hideouts only where
    /// there's a `hill` to dig one into.
    fn pick(c: Climate, roll: f32, hill: bool) -> CampKind {
        use CampKind::*;
        let table: &[(CampKind, f32)] = if c.desert > 0.5 {
            &[(Awnings, 0.55), (Ruin, 0.25), (Hideout, 0.2)]
        } else if c.snow > 0.5 {
            &[(Yurts, 0.45), (Cabins, 0.25), (Fort, 0.12), (Hideout, 0.18)]
        } else if c.jungle > 0.5 {
            &[(Raiders, 0.4), (Ruin, 0.3), (Fort, 0.1), (Hideout, 0.2)]
        } else {
            &[(Fort, 0.22), (Cabins, 0.2), (Ruin, 0.17), (Raiders, 0.17), (Hideout, 0.24)]
        };
        let fits = |k: CampKind| hill || k != Hideout;
        let total: f32 = table.iter().filter(|(k, _)| fits(*k)).map(|(_, w)| w).sum();
        let mut left = roll * total;
        for &(kind, weight) in table.iter().filter(|(k, _)| fits(*k)) {
            if left < weight {
                return kind;
            }
            left -= weight;
        }
        table[0].0
    }

    /// How far out from its fire it reaches, in meters. A hideout's cave goes on into the hill.
    fn reach(self) -> f32 {
        match self {
            CampKind::Fort => 11.0,
            CampKind::Cabins => 10.5,
            CampKind::Ruin => 9.0,
            CampKind::Hideout => 6.0,
            CampKind::Raiders => 7.5,
            CampKind::Awnings => 9.5,
            CampKind::Yurts => 11.0,
        }
    }

    /// How much higher the ground under it may be in one place than another, in meters
    fn flatness(self) -> f32 {
        5.0
    }
}

/// A bandits' camp, in meters: what kind, its fire on the ground in the middle of it, where its
/// bandits keep, and which way is out of it from there. Once they're all dead, a chest of their
/// hoard turns up at `chest`, its lock facing `heading`, and `hint` says where for the player.
#[derive(Clone, Copy)]
pub struct Camp {
    pub kind: CampKind,
    pub fire: [f32; 3],
    pub way: [f32; 2],
    pub chest: [f32; 3],
    pub heading: f32,
    pub hint: &'static str,
    /// How many bandits and brutes keep it, whether a warlord leads them, and how far round the
    /// fire they keep, in meters
    pub bandits: u32,
    pub brutes: u32,
    pub warlord: bool,
    pub spread: f32,
    /// How rare what's in the chest is likely to be, see Library::roll_rarity(): more so the more
    /// there are to fight, far more than in ordinary chests
    pub luck: f32,
}

/// Where a camp goes: its fire on the ground, which way out of it the way in is, what kind it is,
/// the lowest and highest ground under it and what it covers, x0, y0, x1, y1. A hideout's `way`
/// is into the hill instead, and its mouth `mouth` meters along it from the fire.
struct Plan {
    kind: CampKind,
    fire: Vec3,
    way: [f32; 2],
    mouth: f32,
    lowest: f32,
    highest: f32,
    area: [f32; 4],
    seed: u32,
}

/// The ground at a point, in meters, and whether a river runs there; None where it isn't known
type Ground<'a> = &'a dyn Fn(f32, f32) -> Option<(f32, bool)>;

fn flat(d: [f32; 2]) -> Vec3 {
    [d[0], d[1], 0.0]
}

/// The way along x or y nearest `d`
fn cardinal(d: [f32; 2]) -> [f32; 2] {
    if d[0].abs() >= d[1].abs() { [d[0].signum(), 0.0] } else { [0.0, d[1].signum()] }
}

impl Terrain {
    /// The bandits' camps in this piece of the landscape. None where `keep_out` says, x0, y0, x1,
    /// y1 in meters, where something's already built, or where something of `dug`, the caves,
    /// ravines and such, opens onto the ground.
    pub(super) fn camps(&self, heights: &Heights, features: &mut Features, keep_out: &[[f32; 4]], dug: &[Object]) -> Vec<Object> {
        let vpm = heights.voxels_per_meter;
        let ground = |x: f32, y: f32| -> Option<(f32, bool)> {
            let (gx, gy) = ((x * vpm).floor() as i64 - heights.origin[0], (y * vpm).floor() as i64 - heights.origin[1]);
            if gx < 0 || gy < 0 || gx >= heights.size[0] as i64 || gy >= heights.size[1] as i64 {
                return None;
            }
            let i = gx as usize + gy as usize * heights.size[0];
            Some((heights.ground[i], heights.river[i].is_finite()))
        };
        let (lo, hi) = (self.lo, self.hi);
        let first = [(lo[0] / CAMP_CELL).floor() as i32, (lo[1] / CAMP_CELL).floor() as i32];
        let last = [(hi[0] / CAMP_CELL).ceil() as i32, (hi[1] / CAMP_CELL).ceil() as i32];
        let piece = [lo[0], lo[1], hi[0], hi[1]];
        let flat = |s: &Shape| [s.min[0], s.min[1], s.max[0], s.max[1]];
        let dug: Vec<&Shape> = dug.iter().flat_map(|o| &o.shapes).filter(|s| s.placement == Placement::Carve && overlaps(flat(s), piece, 0.0)).collect();
        let mut objects = Vec::new();
        for cy in first[1]..last[1] {
            for cx in first[0]..last[0] {
                let plan = {
                    // Clear of what's built, and of what's dug out up to a meter under the ground
                    let footprints = &features.footprints;
                    let opens = |s: &Shape, area: [f32; 4]| {
                        let (x, y) = (((s.min[0] + s.max[0]) * 0.5).clamp(area[0], area[2]), ((s.min[1] + s.max[1]) * 0.5).clamp(area[1], area[3]));
                        overlaps(flat(s), area, 1.0) && ground(x, y).is_none_or(|g| s.max[2] > g.0 - 1.0)
                    };
                    let clear = |area: [f32; 4]| {
                        !keep_out.iter().any(|k| overlaps(*k, area, 2.0)) && !footprints.iter().any(|f| overlaps([f[0], f[1], f[2], f[3]], area, 2.0)) && !dug.iter().any(|s| opens(s, area))
                    };
                    self.plan_camp(cx, cy, &ground, &clear)
                };
                let Some(plan) = plan else {
                    continue;
                };
                let at = |x: f32, y: f32| ground(x, y).map_or(plan.lowest, |g| g.0);
                objects.push(self.camp(&plan, &at, features));
            }
        }
        objects
    }

    /// The camp in one CAMP_CELL square, if there's one: at the first of a few spots in it that's
    /// dry, flat enough for its kind and `clear` of everything else, with `ground` known all over
    /// it
    fn plan_camp(&self, cx: i32, cy: i32, ground: Ground, clear: &dyn Fn([f32; 4]) -> bool) -> Option<Plan> {
        let r = hash3(cx as u32, cy as u32, self.settings.seed ^ 0xba4d);
        if unit(r) > CAMP_CHANCE {
            return None;
        }
        let sea_level = sea_level_m(&self.settings);
        // The same roll at every spot, so the kind goes by the land and not by which spots suit
        let roll = unit(hash(r ^ 0x6b1d));
        (0..SPOTS).find_map(|attempt| {
            let s = hash3(r, attempt, 0xca3b);
            let (x, y) = ((cx as f32 + 0.15 + 0.7 * unit(s)) * CAMP_CELL, (cy as f32 + 0.15 + 0.7 * unit(s >> 16)) * CAMP_CELL);
            let c = climate(&self.settings, x, y);
            let mut kind = CampKind::pick(c, roll, true);
            let turn = unit(hash(s ^ 0x3a7e)) * TAU;
            let mut way = [turn.cos(), turn.sin()];
            let mut mouth = 0.0;
            if kind == CampKind::Hideout {
                match self.hillside(x, y, ground, hash(s ^ 0x41d3)) {
                    Some((into, m)) => (way, mouth) = (into, m),
                    None => kind = CampKind::pick(c, roll, false),
                }
            }
            let reach = kind.reach();
            let mut area = [x - reach, y - reach, x + reach, y + reach];
            let (lowest, highest) = if kind == CampKind::Hideout {
                // The cave goes on into the hill, which hillside() has looked over
                let side = [-way[1], way[0]];
                for (u, v) in [(mouth + 10.0, -5.0), (mouth + 10.0, 5.0), (mouth, -5.0), (mouth, 5.0)] {
                    let p = [x + way[0] * u + side[0] * v, y + way[1] * u + side[1] * v];
                    ground(p[0], p[1])?;
                    area = [area[0].min(p[0]), area[1].min(p[1]), area[2].max(p[0]), area[3].max(p[1])];
                }
                let floor = ground(x, y)?.0;
                (floor, floor)
            } else {
                let samples: Vec<(f32, bool)> = (0..25).map(|i| ground(x - reach + reach * 0.5 * (i % 5) as f32, y - reach + reach * 0.5 * (i / 5) as f32)).collect::<Option<_>>()?;
                let lowest = samples.iter().map(|s| s.0).fold(f32::MAX, f32::min);
                let highest = samples.iter().map(|s| s.0).fold(f32::MIN, f32::max);
                let dry = !samples.iter().any(|&(_, wet)| wet) && lowest > sea_level + 1.0;
                // Everything in it stands on the ground where it is, so it can be on a slope
                if !dry || highest - lowest > kind.flatness() {
                    return None;
                }
                (lowest, highest)
            };
            if !clear(area) {
                return None;
            }
            let (z, wet) = ground(x, y)?;
            (!wet).then_some(Plan { kind, fire: [x, y, z], way, mouth, lowest, highest, area, seed: hash(s ^ 0xca4e) })
        })
    }

    /// Whether there's a hillside to dig a hideout into from a yard round `x`, `y`: which way into
    /// the hill, and how far from there its face is, where the ground's risen high enough over the
    /// yard for a way in. Dry, with enough hill over the cave all the way in.
    fn hillside(&self, x: f32, y: f32, ground: Ground, seed: u32) -> Option<([f32; 2], f32)> {
        let sea_level = sea_level_m(&self.settings);
        let (floor, wet) = ground(x, y)?;
        if wet || floor < sea_level + 1.5 {
            return None;
        }
        let turn = unit(seed) * TAU;
        'ways: for k in 0..12 {
            let a = turn + k as f32 * TAU / 12.0;
            let (d, side) = ([a.cos(), a.sin()], [-a.sin(), a.cos()]);
            let at = |u: f32, v: f32| ground(x + d[0] * u + side[0] * v, y + d[1] * u + side[1] * v);
            // The yard, flat behind the fire and to either side
            for (u, v) in [(-4.5, 0.0), (-3.0, -3.0), (-3.0, 3.0), (0.0, -4.0), (0.0, 4.0), (-1.5, 0.0), (1.5, 0.0)] {
                match at(u, v) {
                    Some((g, false)) if (g - floor).abs() < 1.4 => {}
                    _ => continue 'ways,
                }
            }
            // The face of the hill, a few meters on
            let Some(mouth) = (0..=11).map(|i| 2.5 + i as f32 * 0.5).find(|&u| at(u, 0.0).is_some_and(|(g, _)| g > floor + 2.7)) else {
                continue;
            };
            for (u, v, over) in [(1.5, 0.0, 3.4), (3.0, 0.0, 4.2), (5.5, 0.0, 4.6), (8.5, 0.0, 4.4), (10.0, 0.0, 3.8), (5.5, -2.8, 4.4), (5.5, 2.8, 4.4), (5.5, -4.0, 3.9), (5.5, 4.0, 3.9)] {
                match at(mouth + u, v) {
                    Some((g, false)) if g > floor + over => {}
                    _ => continue 'ways,
                }
            }
            return Some((d, mouth));
        }
        None
    }

    /// Builds the camp `p` plans on `ground`, adding its lights, its site, what it covers and its
    /// record to `features`
    fn camp(&self, p: &Plan, ground: &dyn Fn(f32, f32) -> f32, features: &mut Features) -> Object {
        let c = climate(&self.settings, p.fire[0], p.fire[1]);
        let mut lot = Lot { s: Vec::new(), lights: Vec::new(), ground, level: None, place: Placement::Above, rng: Rng(p.seed) };
        let (chest, heading, hint) = match p.kind {
            CampKind::Fort => fort(&mut lot, p, c),
            CampKind::Cabins => cabins(&mut lot, p, c),
            CampKind::Ruin => ruin(&mut lot, p, c),
            CampKind::Hideout => hideout(&mut lot, p),
            CampKind::Raiders => raiders(&mut lot, p, c),
            CampKind::Awnings => awnings(&mut lot, p),
            CampKind::Yurts => yurts(&mut lot, p),
        };
        let (bandits, brutes, warlord, spread) = band(p.kind, &mut Rng(hash(p.seed ^ 0xba7d)));
        let luck = (3.0 + 0.25 * (bandits + brutes) as f32 + if warlord { 1.0 } else { 0.0 }).min(5.5);
        let way = match p.kind {
            CampKind::Hideout => [-p.way[0], -p.way[1]],
            CampKind::Ruin | CampKind::Awnings => cardinal(p.way),
            _ => p.way,
        };
        features.lights.extend(lot.lights);
        features.sites.push(Site { kind: "bandit camp", position: p.fire, facing: way });
        features.footprints.push([p.area[0], p.area[1], p.area[2], p.area[3], p.highest + 3.0]);
        if p.kind == CampKind::Hideout {
            // Water lies on the floor of the way in, cut into the hill
            let (a, b) = ([p.fire[0] + p.way[0] * (p.mouth - 4.0), p.fire[1] + p.way[1] * (p.mouth - 4.0)], [p.fire[0] + p.way[0] * (p.mouth + 1.0), p.fire[1] + p.way[1] * (p.mouth + 1.0)]);
            features.recuts.push([a[0].min(b[0]) - 1.6, a[1].min(b[1]) - 1.6, a[0].max(b[0]) + 1.6, a[1].max(b[1]) + 1.6, p.fire[2] + 4.0]);
        }
        features.camps.push(Camp { kind: p.kind, fire: p.fire, way, chest, heading, hint, bandits, brutes, warlord, spread, luck });
        Object::new(lot.s, lot.rng.0)
    }
}

/// Where the nearest few camps of `kind` to `near` are planned, nearest first, in meters, out on
/// `land` if it says ("jungle", "desert", "snow" or "temperate"): each where it would be if nothing
/// were in the way, as the rivers and buildings round it aren't looked at. For trying them out.
pub fn camp_spots(settings: &TerrainSettings, kind: CampKind, near: [f32; 2], land: Option<&str>) -> Vec<[f32; 2]> {
    let on_land = |p: &Plan| {
        let c = climate(settings, p.fire[0], p.fire[1]);
        match land {
            Some("jungle") => c.jungle > 0.5,
            Some("desert") => c.desert > 0.5,
            Some("snow") => c.snow > 0.5,
            Some(_) => c.jungle.max(c.desert).max(c.snow) <= 0.5,
            None => true,
        }
    };
    let terrain = Terrain::new(settings, &Heights::new(settings, [0, 0], [8, 8], settings.voxels_per_meter));
    let ground = |x: f32, y: f32| Some((height_at(settings, x, y), false));
    let home = near.map(|v| (v / CAMP_CELL).floor() as i32);
    let mut found: Vec<(f32, [f32; 2])> = Vec::new();
    for ring in 0..30i32 {
        for cy in home[1] - ring..=home[1] + ring {
            for cx in home[0] - ring..=home[0] + ring {
                if (cx - home[0]).abs().max((cy - home[1]).abs()) != ring {
                    continue;
                }
                if let Some(p) = terrain.plan_camp(cx, cy, &ground, &|_: [f32; 4]| true).filter(|p| p.kind == kind && on_land(p)) {
                    found.push(((p.fire[0] - near[0]).hypot(p.fire[1] - near[1]), [p.fire[0], p.fire[1]]));
                }
            }
        }
        if found.len() >= 6 {
            break;
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, at)| at).collect()
}

/// Who keeps a camp of `kind`: how many bandits and brutes, whether a warlord leads them, and how
/// far round the fire they keep, in meters
fn band(kind: CampKind, rng: &mut Rng) -> (u32, u32, bool, f32) {
    let mut one = |chance: f32| (rng.next() < chance) as u32;
    let (bandits, brutes, warlord, spread) = match kind {
        CampKind::Fort => (3 + one(0.5), 1 + one(0.5), 0.55, 3.8),
        CampKind::Cabins => (3 + one(0.4), 1, 0.3, 3.5),
        CampKind::Ruin => (2 + one(0.5), 1 + one(0.5), 0.4, 3.5),
        CampKind::Hideout => (2 + one(0.5), 1, 0.35, 3.2),
        CampKind::Raiders => (3 + one(0.5) + one(0.3), one(0.5), 0.2, 3.5),
        CampKind::Awnings => (3 + one(0.4), 1, 0.3, 3.8),
        CampKind::Yurts => (2 + one(0.5), 1 + one(0.5), 0.35, 4.0),
    };
    (bandits, brutes, rng.next() < warlord, spread)
}

/// Where a camp's hoard turns up, which way its lock faces, and where that is, for the player
type Hoard = (Vec3, f32, &'static str);

/// A fort: a ring or a square of sharpened logs with a gate out along `way`, its leaves swung
/// open, a skull over it and torches either side, and maybe another gate at the back. A
/// watchtower stands inside near the gate. Round the fire in the middle, the leader's hut at the
/// back with huts and tents either side, and weapons, loot and maybe a cage about.
fn fort(lot: &mut Lot, p: &Plan, c: Climate) -> Hoard {
    let at = Frame { x: p.fire[0], y: p.fire[1], f: p.way };
    let snow = c.snow > 0.5;
    let round = lot.rng.next() < 0.55;
    let r = if round { lot.rng.range(8.8, 10.2) } else { lot.rng.range(8.0, 9.0) };
    let gate = 1.3;
    let back_gate = lot.rng.next() < 0.35;
    let tower_side = lot.flip();

    // The palisade, leaving the gates
    let mut ring: Vec<[f32; 2]> = Vec::new();
    if round {
        let n = (TAU * r / 0.36) as usize;
        for k in 0..n {
            let a = k as f32 / n as f32 * TAU;
            ring.push([r * a.cos(), r * a.sin()]);
        }
    } else {
        let n = (2.0 * r / 0.36).round() as usize;
        for side in 0..4 {
            for k in 0..n {
                let t = -r + k as f32 * 2.0 * r / n as f32;
                ring.push(match side {
                    0 => [r, t],
                    1 => [-t, r],
                    2 => [-r, -t],
                    _ => [t, -r],
                });
            }
        }
    }
    let open = |q: &[f32; 2]| q[0] > 0.0 && q[1].abs() < gate + 0.12 || back_gate && q[0] < 0.0 && q[1].abs() < gate;
    let stakes: Vec<[f32; 2]> = ring.into_iter().filter(|q| !open(q)).collect();
    // Rails along the inside, lashing them together
    for k in (0..stakes.len().saturating_sub(6)).step_by(6) {
        let (a, b) = (stakes[k], stakes[k + 6]);
        if (a[0] - b[0]).hypot(a[1] - b[1]) > 2.6 {
            continue;
        }
        let inward = |q: [f32; 2]| {
            let d = q[0].hypot(q[1]);
            [q[0] * (d - 0.24) / d, q[1] * (d - 0.24) / d]
        };
        let (a, b) = (inward(a), inward(b));
        let (ga, gb) = (lot.on(&at, a[0], a[1]), lot.on(&at, b[0], b[1]));
        lot.stick([ga[0], ga[1], ga[2] + 1.25], [gb[0], gb[1], gb[2] + 1.25], 0.06, BARK_DARK);
    }
    for q in &stakes {
        let g = lot.on(&at, q[0], q[1]);
        let tall = lot.rng.range(2.5, 3.1);
        lot.put(Shape::capsule([g[0], g[1], g[2] - 0.6], [g[0], g[1], g[2] + tall], 0.17, 0.05, WOOD, BARK_DARK, false));
    }

    // The gate: tall posts, a lintel with a skull on it, the leaves swung open, torches outside
    let gu = if round { (r * r - (gate + 0.12) * (gate + 0.12)).sqrt() } else { r };
    let gz = lot.on(&at, gu, 0.0)[2];
    lot.put(Shape::sphere(at.point(gu + 0.12, 0.0, gz + 3.82), 0.14, BONE, AIR, false));
    lot.stick(at.point(gu, -gate - 0.55, gz + 3.55), at.point(gu, gate + 0.55, gz + 3.55), 0.17, BARK_DARK);
    for s in [-1.0f32, 1.0] {
        let post = lot.on(&at, gu, s * (gate + 0.12));
        lot.put(Shape::capsule([post[0], post[1], post[2] - 0.6], [post[0], post[1], gz + 3.9], 0.22, 0.19, BARK_DARK, WOOD, false));
        let swing = lot.rng.range(1.0, 1.35);
        let dir = at.dir([swing.sin(), -s * swing.cos()]);
        let face = [-dir[1] * s, dir[0] * s];
        for h in [0.55, 2.05] {
            let (a, b) = ([post[0] + dir[0] * 0.2 + face[0] * 0.13, post[1] + dir[1] * 0.2 + face[1] * 0.13], [post[0] + dir[0] * 1.15 + face[0] * 0.13, post[1] + dir[1] * 1.15 + face[1] * 0.13]);
            lot.stick([a[0], a[1], gz + h], [b[0], b[1], gz + h], 0.06, BARK_DARK);
        }
        for k in 0..4 {
            let t = 0.25 + k as f32 * 0.29;
            let q = [post[0] + dir[0] * t, post[1] + dir[1] * t];
            let g = lot.g(q[0], q[1]);
            lot.put(Shape::capsule([q[0], q[1], g - 0.1], [q[0], q[1], g + 2.6], 0.13, 0.11, WOOD, BARK_DARK, false));
        }
        let t = at.point(gu + 0.9, s * (gate + 0.8), 0.0);
        lot.torch(t[0], t[1], 2.0);
        if back_gate {
            let back = lot.on(&at, -gu, s * gate);
            lot.put(Shape::capsule([back[0], back[1], back[2] - 0.6], [back[0], back[1], back[2] + 3.4], 0.2, 0.17, BARK_DARK, WOOD, false));
        }
    }

    // The watchtower, inside by the gate
    let (tu, tv) = if round {
        let a = tower_side * 0.8;
        ((r - 2.5) * a.cos(), (r - 2.5) * a.sin())
    } else {
        (r - 2.3, tower_side * (r - 2.3))
    };
    let middle = at.point(tu, tv, 0.0);
    let tower = Frame { x: middle[0], y: middle[1], f: at.f };
    let roof = if snow || lot.rng.next() < 0.5 { (BARK_DARK, WOOD) } else { (DRY_GRASS, PALM_WOOD) };
    let high = lot.rng.range(3.8, 4.6);
    lot.watchtower(&tower, high, roof, snow);

    // The fire, logs to sit on round it
    lot.fire(at.x, at.y, 1.0, (ROCK, GRAVEL));
    lot.seats(&at, 3, 1.9, None);

    // The leader's hut at the back, huts and tents either side
    let n = 2 + (r > 9.4) as usize + (lot.rng.next() < 0.5) as usize;
    let leader = (lot.rng.next() * n as f32) as usize % n;
    let logs = if lot.rng.next() < 0.3 { (DEAD_WOOD, WOOD) } else { (WOOD, BARK_DARK) };
    let hides = [LEATHER, LINEN, FUR_TAN, FUR_BROWN];
    let mut hoard = None;
    for k in 0..n {
        let a = PI + (k as f32 - (n - 1) as f32 * 0.5) * 0.95 + lot.rng.range(-0.08, 0.08);
        let dir = [a.cos(), a.sin()];
        let toward = at.dir([-dir[0], -dir[1]]);
        if k == leader {
            let half = [lot.rng.range(2.0, 2.3), lot.rng.range(1.6, 1.8)];
            let d = (r - half[0] - 1.0).max(half[0] + 2.8);
            let middle = at.point(dir[0] * d, dir[1] * d, 0.0);
            let hut = Frame { x: middle[0], y: middle[1], f: toward };
            let chimney = lot.rng.next() < 0.5;
            let roof = cabin_roof(lot, snow);
            let spot = lot.cabin(&hut, half, 2.1, logs, roof, snow, chimney, true);
            hoard = Some((spot, hut.heading(0.0)));
            let side = lot.flip();
            let pole = hut.point(half[0] + 0.55, side * (half[1] + 0.25), 0.0);
            let cloth = if lot.rng.next() < 0.5 { CLOTH } else { TEMPLE_RED };
            lot.banner(pole[0], pole[1], toward, cloth);
            let pile = hut.point(0.2, -side * (half[1] + 0.7), 0.0);
            lot.stash(pile[0], pile[1], 3);
        } else if lot.rng.next() < 0.6 {
            let d = r - 2.4;
            let q = at.point(dir[0] * d, dir[1] * d, 0.0);
            let hide = hides[(lot.rng.next() * 3.99) as usize];
            lot.teepee(q[0], q[1], toward, hide);
        } else {
            let half = [lot.rng.range(1.5, 1.7), lot.rng.range(1.25, 1.4)];
            let d = r - half[0] - 1.0;
            let middle = at.point(dir[0] * d, dir[1] * d, 0.0);
            let hut = Frame { x: middle[0], y: middle[1], f: toward };
            let roof = cabin_roof(lot, snow);
            lot.cabin(&hut, half, 1.9, logs, roof, snow, false, false);
        }
    }

    // A rack of spears inside the gate, loot by the wall across from the tower, maybe a cage
    let rack = at.point(gu - 1.6, -tower_side * (gate + 1.5), 0.0);
    lot.weapon_rack(rack[0], rack[1], at.dir([1.0, 0.0]));
    let a = -tower_side * 0.75;
    let pile = at.point((r - 1.4) * a.cos(), (r - 1.4) * a.sin(), 0.0);
    lot.stash(pile[0], pile[1], 4);
    let a = -tower_side * 1.25;
    let wood = at.point((r - 1.2) * a.cos(), (r - 1.2) * a.sin(), 0.0);
    lot.woodpile(wood[0], wood[1], at.dir([-a.sin(), a.cos()]));
    if n <= 3 && lot.rng.next() < 0.45 {
        let a = tower_side * 1.55;
        let q = at.point((r - 1.9) * a.cos(), (r - 1.9) * a.sin(), 0.0);
        lot.cage(q[0], q[1]);
    }
    let (spot, heading) = hoard.unwrap_or_else(|| (at.point(-2.0, 0.0, p.fire[2]), at.heading(0.0)));
    (spot, heading, "in the leader's hut")
}

/// What a hut or a cabin's roof is covered with: material, accent and what runs along its ridge.
/// Shingles under the snow, and elsewhere thatch or turf as well.
fn cabin_roof(lot: &mut Lot, snow: bool) -> (u8, u8, u8) {
    let pick = lot.rng.next();
    if snow || pick < 0.4 {
        (BARK_DARK, WOOD, WOOD)
    } else if pick < 0.7 {
        (DRY_GRASS, PALM_WOOD, BARK_DARK)
    } else {
        (GRASS, MOSS, MOSS)
    }
}

/// A hamlet of log cabins round a fire, all facing it, the way in left open. The leader's is the
/// biggest. Woodpiles, racks of drying hides, barrels and stumps beside them, maybe a cart of
/// loot by the way in.
fn cabins(lot: &mut Lot, p: &Plan, c: Climate) -> Hoard {
    let at = Frame { x: p.fire[0], y: p.fire[1], f: p.way };
    let snow = c.snow > 0.5;
    let n = 3 + (lot.rng.next() * 2.6) as usize;
    let leader = (lot.rng.next() * n as f32) as usize % n;
    let span = TAU - 1.6;
    lot.fire(at.x, at.y, 1.0, (ROCK, GRAVEL));
    if lot.rng.next() < 0.55 {
        lot.spit(at.x, at.y, at.dir([0.0, 1.0]));
    }
    let seats = 2 + (lot.rng.next() < 0.5) as usize;
    lot.seats(&at, seats, 1.9, None);
    let logs = if lot.rng.next() < 0.35 { (DEAD_WOOD, WOOD) } else { (WOOD, BARK_DARK) };
    let hides = [FUR_BROWN, FUR_TAN, FUR_GRAY, LEATHER, FUR_RED];
    let mut hoard = None;
    for k in 0..n {
        let a = 0.8 + (k as f32 + 0.5) * span / n as f32 + lot.rng.range(-0.12, 0.12);
        let dir = [a.cos(), a.sin()];
        let big = k == leader;
        let half = if big { [lot.rng.range(2.3, 2.6), lot.rng.range(1.8, 2.0)] } else { [lot.rng.range(1.7, 2.1), lot.rng.range(1.35, 1.65)] };
        let d = lot.rng.range(3.6, 4.3) + 0.25 * (n - 3) as f32 + half[0];
        let middle = at.point(dir[0] * d, dir[1] * d, 0.0);
        let cabin = Frame { x: middle[0], y: middle[1], f: at.dir([-dir[0], -dir[1]]) };
        let chimney = big || lot.rng.next() < 0.4;
        let roof = cabin_roof(lot, snow);
        let spot = lot.cabin(&cabin, half, if big { 2.3 } else { 2.0 }, logs, roof, snow, chimney, big);
        let side = lot.flip();
        let [hu, hv] = half;
        if big {
            hoard = Some((spot, cabin.heading(0.0)));
            let pole = cabin.point(hu + 0.6, side * (hv + 0.2), 0.0);
            let cloth = if lot.rng.next() < 0.5 { CLOTH } else { TEMPLE_TEAL };
            lot.banner(pole[0], pole[1], cabin.f, cloth);
        }
        let along = cabin.dir([1.0, 0.0]);
        match (lot.rng.next() * 4.0) as usize {
            0 => {
                let q = cabin.point(-0.2, -side * (hv + 0.55), 0.0);
                lot.woodpile(q[0], q[1], along);
            }
            1 => {
                let q = cabin.point(hu + 0.55, -side * (hv - 0.35), 0.0);
                lot.stash(q[0], q[1], 2);
            }
            2 => {
                let q = cabin.point(-0.3, -side * (hv + 0.9), 0.0);
                let pair = [hides[(lot.rng.next() * 4.99) as usize], hides[(lot.rng.next() * 4.99) as usize]];
                lot.drying_rack(q[0], q[1], along, pair);
            }
            _ => {
                let q = cabin.point(hu + 0.9, -side * (hv + 0.5), 0.0);
                lot.stump(q[0], q[1]);
            }
        }
    }
    // A cart of what they've taken, by the way in, and torches either side of it
    let side = lot.flip();
    if lot.rng.next() < 0.6 {
        let middle = at.point(4.9, side * 2.7, 0.0);
        let cart = Frame { x: middle[0], y: middle[1], f: at.f };
        let bed = lot.cart(&cart);
        let q = cart.point(-0.5, 0.0, bed);
        lot.crate_at(q[0], q[1], q[2], 0.32);
        let q = cart.point(0.35, 0.2, bed);
        lot.sack_at(q[0], q[1], q[2]);
    }
    for s in [-1.0f32, 1.0] {
        let t = at.point(7.2, s * 1.6, 0.0);
        lot.torch(t[0], t[1], 2.0);
    }
    let (spot, heading) = hoard.unwrap_or_else(|| (at.point(-2.0, 0.0, p.fire[2]), at.heading(0.0)));
    (spot, heading, "in the leader's cabin")
}

/// An old stone keep they've moved into: broken walls round a yard, the way in through the front,
/// a round tower broken off at one back corner, rubble about, stakes in the gaps, and hides
/// stretched over the other back corner on poles, where they sleep and keep their hoard. Built of
/// the stone of the land round it.
fn ruin(lot: &mut Lot, p: &Plan, c: Climate) -> Hoard {
    let at = Frame { x: p.fire[0], y: p.fire[1], f: cardinal(p.way) };
    let jungle = c.jungle > 0.5;
    let (stone, cap) = if c.desert > 0.5 {
        ((SANDSTONE, SANDSTONE_DARK), None)
    } else if jungle {
        ((TEMPLE_STONE, MOSS), Some(MOSS))
    } else if c.snow > 0.5 {
        ((BRICK_STONE, SNOW), Some(SNOW))
    } else {
        ((BRICK_STONE, MOSS), None)
    };
    let (hu, hv) = (lot.rng.range(4.9, 6.0), lot.rng.range(4.5, 5.4));
    let t = 0.7;
    let tower_side = lot.flip();

    // The tower first, so it's hollow where the walls run into it
    let tr = lot.rng.range(1.9, 2.3);
    let tall = lot.rng.range(4.6, 6.6);
    lot.ruin_tower(&at, [-hu + 0.35, tower_side * (hv - 0.35)], tr, tall, stone);
    // The jambs either side of the way in, standing taller, maybe still with a lintel over it
    let door = lot.on(&at, hu, 0.0)[2];
    if lot.rng.next() < 0.5 {
        lot.put(at.block([hu - t, hu], [-1.65, 1.65], [door + 2.45, door + 2.85], stone.0, stone.1).patterned(Pattern::Bricks));
    }
    for s in [-1.0f32, 1.0] {
        let (g, _) = lot.span(&at, &[(hu - t, s * 1.0), (hu, s * 1.6)]);
        lot.put(at.block([hu - t - 0.05, hu + 0.05], [s * 1.0, s * 1.65], [g - 0.5, door + 2.85], stone.0, stone.1).patterned(Pattern::Bricks));
    }
    // The walls, broken here and there, the front and back running along v, the sides along u
    let mut gaps = Vec::new();
    for (u, v, height, fall, door, along) in [
        ([hu - t, hu], [-hv, hv], (0.8, 3.0), 0.15, Some([-1.0, 1.0]), [0.0, 1.0]),
        ([-hu, -hu + t], [-hv, hv], (1.6, 3.4), 0.08, None, [0.0, 1.0]),
        ([-hu, hu], [hv - t, hv], (0.6, 3.0), 0.2, None, [1.0, 0.0]),
        ([-hu, hu], [-hv, -hv + t], (0.6, 3.0), 0.2, None, [1.0, 0.0]),
    ] {
        gaps.extend(lot.ruin_wall(&at, u, v, height, stone, cap, fall, door, jungle).into_iter().map(|g| (g, along)));
    }
    // Sharpened stakes filling the gaps
    for ((u, v), along) in gaps {
        for k in -1..=1 {
            let q = at.point(u + along[0] * k as f32 * 0.38, v + along[1] * k as f32 * 0.38, 0.0);
            let g = lot.g(q[0], q[1]);
            let tall = lot.rng.range(1.9, 2.4);
            lot.put(Shape::capsule([q[0], q[1], g - 0.5], [q[0], q[1], g + tall], 0.16, 0.04, WOOD, BARK_DARK, false));
        }
    }
    // Rubble fallen off them
    for _ in 0..10 {
        let (side, along) = (lot.flip(), lot.rng.range(-1.0, 1.0));
        let (u, v) = if lot.rng.next() < 0.5 { (side * (hu + lot.rng.range(-1.6, 1.2)), along * hv) } else { (along * hu, side * (hv + lot.rng.range(-1.6, 1.2))) };
        let q = lot.on(&at, u, v);
        let size = lot.rng.range(0.18, 0.34);
        if lot.rng.next() < 0.6 {
            lot.put(Shape::block([q[0] - size, q[1] - size * 0.8, q[2] - 0.2], [q[0] + size, q[1] + size * 0.8, q[2] + size * 1.2], stone.0, stone.1));
        } else {
            lot.put(Shape::sphere([q[0], q[1], q[2]], size * 1.2, ROCK, stone.0, false));
        }
    }

    // Hides stretched over the other back corner, up to the top of the walls from two poles out in
    // the yard, with bedrolls under them and the hoard in the corner
    let s = -tower_side;
    let (u0, u1) = (-hu + t, -hu + t + 3.0);
    let (v0, v1) = (s * (hv - t), s * (hv - t - 3.4));
    let (_, back) = lot.span(&at, &[(u0, v0), (u0, v1)]);
    let (_, front) = lot.span(&at, &[(u1, v0), (u1, v1)]);
    let (high, low) = (back + 2.7, front + 2.0);
    let hide = [(LEATHER, FUR_BROWN), (LINEN, CLOTH), (FUR_TAN, LEATHER)][(lot.rng.next() * 2.99) as usize];
    let (hi, lo) = (at.point(u0, (v0 + v1) * 0.5, high), at.point(u1 + 0.2, (v0 + v1) * 0.5, low));
    let along = flat(at.dir([0.0, 1.0]));
    let slope = normalize(sub(lo, hi));
    let mut normal = cross(slope, along);
    if normal[2] < 0.0 {
        normal = scale(normal, -1.0);
    }
    let length = dot(sub(lo, hi), sub(lo, hi)).sqrt();
    lot.put(Shape::leaf(add(lerp3(hi, lo, 0.5), scale(normal, 0.05)), along, normal, 4.0, length + 0.6, hide.0, hide.1));
    for v in [v0 - s * 0.15, v1] {
        let (a, b) = (lot.on(&at, u0 + 0.15, v), lot.on(&at, u1, v));
        lot.stick([a[0], a[1], a[2] - 0.3], [a[0], a[1], high], 0.06, WOOD);
        lot.stick([b[0], b[1], b[2] - 0.3], [b[0], b[1], low], 0.06, WOOD);
    }
    let along_u = at.dir([1.0, 0.0]);
    let colors = [CLOTH, LINEN, TEMPLE_TEAL, LEATHER];
    for k in 0..2 {
        let q = at.point(u0 + 1.25, v0 - s * (1.75 + k as f32 * 0.85), 0.0);
        let color = colors[(lot.rng.next() * 3.99) as usize];
        lot.bedroll(q[0], q[1], [-along_u[0], -along_u[1]], color);
    }
    let pile = at.point(u1 + 0.7, v0 - s * 0.4, 0.0);
    lot.stash(pile[0], pile[1], 3);
    let chest = lot.on(&at, u0 + 0.6, v0 - s * 0.62);

    // The fire in the yard, torches either side of the way in, a banner, a rack of spears
    lot.fire(at.x, at.y, 1.0, (ROCK, GRAVEL));
    lot.seats(&at, 2, 1.9, None);
    for side in [-1.0f32, 1.0] {
        let q = at.point(hu + 0.75, side * 1.75, 0.0);
        lot.torch(q[0], q[1], 2.0);
    }
    let q = at.point(hu - 1.4, tower_side * 2.3, 0.0);
    let cloth = if lot.rng.next() < 0.5 { CLOTH } else { TEMPLE_RED };
    lot.banner(q[0], q[1], at.f, cloth);
    let q = at.point(0.6, tower_side * (hv - t - 0.75), 0.0);
    lot.weapon_rack(q[0], q[1], along_u);
    (chest, at.heading(0.0), "in the corner of the ruin, under the hides")
}

/// A hideout dug into a hillside: a way in cut into the slope and on under the hill, shored up
/// with timber, a skull over its mouth and torches either side, to a cave at the end where they
/// sleep in a nook off to one side, lit by a lantern, their hoard at the back. Out front, a yard
/// with the fire.
fn hideout(lot: &mut Lot, p: &Plan) -> Hoard {
    let d = p.way;
    let at = Frame { x: p.fire[0], y: p.fire[1], f: d };
    let floor = p.fire[2];
    let m = p.mouth;
    let nook = lot.flip();
    let point = |u: f32, v: f32, z: f32| at.point(u, v, floor + z);

    // Cut into the hill: the way in, the cave at the end of it and a nook to one side, then filled
    // back up to its floor of trodden earth, level all through
    lot.s.push(Shape::slab(point(m + 0.5, 0.0, 1.1), flat(d), 8.0, 3.0, 3.6));
    lot.s.push(Shape::slab(point(m + 6.0, 0.0, 1.5), flat(d), 6.6, 5.6, 4.6));
    lot.s.push(Shape::slab(point(m + 6.0, nook * 2.9, 0.9), flat(at.dir([0.0, 1.0])), 3.4, 2.6, 3.0));
    let corners = [point(m - 5.0, -5.0, 0.0), point(m - 5.0, 5.0, 0.0), point(m + 10.0, -5.0, 0.0), point(m + 10.0, 5.0, 0.0)];
    let lo = [0, 1].map(|i| corners.iter().map(|c| c[i]).fold(f32::MAX, f32::min));
    let hi = [0, 1].map(|i| corners.iter().map(|c| c[i]).fold(f32::MIN, f32::max));
    lot.s.push(Shape::block([lo[0], lo[1], floor - 1.2], [hi[0], hi[1], floor], DIRT, GRAVEL).placed(Placement::Inside));

    // In under the hill everything's built on that floor, and in the rock round it
    lot.level = Some(floor);
    lot.place = Placement::Clear;
    // The mouth, framed in heavy timber, a skull over it
    lot.put(Shape::sphere(point(m - 0.2, 0.0, 2.88), 0.14, BONE, AIR, false));
    for s in [-1.0f32, 1.0] {
        lot.stick(point(m - 0.12, 0.0, 2.95), point(m - 0.12, s * 0.3, 3.2), 0.035, BONE);
    }
    lot.stick(point(m, -1.75, 2.55), point(m, 1.75, 2.55), 0.18, BARK_DARK);
    for s in [-1.0f32, 1.0] {
        lot.put(Shape::capsule(point(m, s * 1.2, -0.4), point(m, s * 1.2, 2.5), 0.16, 0.15, BARK_DARK, WOOD, false));
    }
    // Frames shoring up the way in, and the cave's roof on a post and beams
    for u in [m + 1.3, m + 2.6] {
        lot.stick(point(u, -1.3, 2.35), point(u, 1.3, 2.35), 0.12, WOOD);
        for s in [-1.0f32, 1.0] {
            lot.stick(point(u, s * 1.0, -0.2), point(u, s * 1.0, 2.35), 0.11, WOOD);
        }
    }
    lot.stick(point(m + 6.0, -2.4, 3.05), point(m + 6.0, 2.4, 3.05), 0.13, WOOD);
    lot.stick(point(m + 3.3, 0.0, 3.15), point(m + 8.6, 0.0, 3.15), 0.12, BARK_DARK);
    lot.stick(point(m + 6.0, -nook * 1.9, -0.2), point(m + 6.0, -nook * 1.9, 3.05), 0.15, BARK_DARK);
    // A lantern hung from the beam
    let lamp = point(m + 6.0, 1.0, 2.4);
    lot.stick(point(m + 6.0, 1.0, 3.0), [lamp[0], lamp[1], lamp[2] + 0.2], 0.015, DARK_STEEL);
    lot.lantern(lamp);
    // Bedrolls in the nook, loot along the sides, a rack of spears
    let colors = [CLOTH, LINEN, FUR_BROWN, LEATHER];
    let across = at.dir([0.0, nook]);
    for u in [m + 5.55, m + 6.4] {
        let q = point(u, nook * 2.95, 0.0);
        let color = colors[(lot.rng.next() * 3.99) as usize];
        lot.bedroll(q[0], q[1], across, color);
    }
    let q = point(m + 3.9, nook * 1.3, 0.0);
    lot.stash(q[0], q[1], 3);
    let q = point(m + 7.3, -nook * 1.35, 0.0);
    lot.barrel_at(q[0], q[1], floor);
    // Its spears leaning in toward the middle
    let q = point(m + 4.3, -nook * 1.85, 0.0);
    lot.weapon_rack(q[0], q[1], [d[0] * nook, d[1] * nook]);
    let chest = point(m + 7.8, 0.0, 0.0);
    lot.level = None;
    lot.place = Placement::Above;

    // The yard out front: the fire, logs round it, torches either side of the way in, a woodpile
    lot.fire(at.x, at.y, 1.0, (ROCK, GRAVEL));
    for a in [PI * 0.75, PI * 1.25, PI] {
        let q = at.point(1.9 * a.cos(), 1.9 * a.sin(), 0.0);
        lot.seat(q[0], q[1], at.dir([-a.sin(), a.cos()]));
    }
    for s in [-1.0f32, 1.0] {
        let q = at.point(m - 1.3, s * 1.8, 0.0);
        lot.torch(q[0], q[1], 1.9);
    }
    let side = lot.flip();
    let q = at.point(-1.0, side * 3.4, 0.0);
    lot.woodpile(q[0], q[1], d);
    let q = at.point(-3.6, -side * 2.6, 0.0);
    lot.skull_pole(q[0], q[1]);
    (chest, at.heading(PI), "at the back of their cave")
}

/// A camp of lean-tos out in the woods, open to the fire, under hides, boughs, or palm leaves in
/// the jungle. The leader's is the widest, a skull on a pole beside it. Bedrolls under them, a
/// spit over the fire, racks of hides and spears, and spikes on logs by the way in.
fn raiders(lot: &mut Lot, p: &Plan, c: Climate) -> Hoard {
    let at = Frame { x: p.fire[0], y: p.fire[1], f: p.way };
    let jungle = c.jungle > 0.5;
    let n = 3 + (lot.rng.next() * 2.6) as usize;
    let leader = (lot.rng.next() * n as f32) as usize % n;
    let span = TAU - 1.8;
    lot.fire(at.x, at.y, 1.0, (ROCK, GRAVEL));
    if lot.rng.next() < 0.7 {
        lot.spit(at.x, at.y, at.dir([0.0, 1.0]));
    }
    let seats = 2 + (lot.rng.next() < 0.5) as usize;
    lot.seats(&at, seats, 1.9, None);
    let hides = [(LEATHER, FUR_BROWN), (FUR_TAN, LEATHER), (FUR_BROWN, FUR_DARK), (FUR_GRAY, FUR_WHITE)];
    let colors = [CLOTH, LINEN, FUR_BROWN, TEMPLE_TEAL, LEATHER];
    let mut hoard = None;
    let mut angles = Vec::new();
    for k in 0..n {
        let a = 0.9 + (k as f32 + 0.5) * span / n as f32 + lot.rng.range(-0.1, 0.1);
        angles.push(a);
        let dir = [a.cos(), a.sin()];
        let front = lot.rng.range(3.4, 3.9) + 0.2 * (n - 3) as f32;
        let middle = at.point(dir[0] * front, dir[1] * front, 0.0);
        let lean = Frame { x: middle[0], y: middle[1], f: at.dir([-dir[0], -dir[1]]) };
        let big = k == leader;
        let w = if big { 1.9 } else { lot.rng.range(1.15, 1.5) };
        let cover = if big {
            let (m, a) = hides[(lot.rng.next() * 3.99) as usize];
            Cover::Hide(m, a)
        } else if jungle {
            Cover::Thatch
        } else if lot.rng.next() < 0.5 {
            Cover::Boughs(if c.snow > 0.3 || lot.rng.next() < 0.5 { PINE } else { LEAVES_DARK })
        } else {
            let (m, a) = hides[(lot.rng.next() * 3.99) as usize];
            Cover::Hide(m, a)
        };
        let tall = if big { 2.15 } else { lot.rng.range(1.75, 2.05) };
        let under = lot.lean_to(&lean, w, tall, cover);
        let back = lean.dir([-1.0, 0.0]);
        if big {
            hoard = Some((under, lean.heading(0.0)));
            for s in [-1.0f32, 1.0] {
                let q = lean.point(-1.0, s * (w - 0.45), 0.0);
                let color = colors[(lot.rng.next() * 4.99) as usize];
                lot.bedroll(q[0], q[1], back, color);
            }
            let q = lean.point(0.35, w + 0.45, 0.0);
            lot.skull_pole(q[0], q[1]);
        } else {
            let q = lean.point(-1.0, lot.rng.range(-0.3, 0.3), 0.0);
            let color = colors[(lot.rng.next() * 4.99) as usize];
            lot.bedroll(q[0], q[1], back, color);
            if lot.rng.next() < 0.5 {
                let q = lot.on(&lean, -1.4, -(w - 0.3));
                lot.sack_at(q[0], q[1], q[2]);
            }
        }
    }
    // Between the lean-tos, out behind, a rack of hides, a rack of spears and what they've taken
    for k in 0..n.saturating_sub(1) {
        let a = (angles[k] + angles[k + 1]) * 0.5;
        let (dir, along) = ([a.cos(), a.sin()], at.dir([-a.sin(), a.cos()]));
        let q = at.point(dir[0] * 5.6, dir[1] * 5.6, 0.0);
        match k % 3 {
            0 => {
                let (m, _) = hides[(lot.rng.next() * 3.99) as usize];
                lot.drying_rack(q[0], q[1], along, [m, FUR_TAN]);
            }
            1 => lot.weapon_rack(q[0], q[1], along),
            _ => lot.stash(q[0], q[1], 3),
        }
    }
    // Spikes on logs either side of the way in, and a torch
    for s in [-1.0f32, 1.0] {
        let q = at.point(6.3, s * 2.4, 0.0);
        lot.spikes(q[0], q[1], at.dir([0.55, -s * 0.84]));
    }
    let q = at.point(5.2, lot.flip() * 1.3, 0.0);
    lot.torch(q[0], q[1], 1.9);
    let (spot, heading) = hoard.unwrap_or_else(|| (at.point(-2.0, 0.0, p.fire[2]), at.heading(0.0)));
    (spot, heading, "under the leader's lean-to")
}

/// A camp in the desert: the leader's striped awning at the back, hung round with cloth but for
/// the front, smaller awnings either side facing in, rugs, cushions and pots under them, a low
/// wall of mud brick behind against the wind, braziers by the way in.
fn awnings(lot: &mut Lot, p: &Plan) -> Hoard {
    let at = Frame { x: p.fire[0], y: p.fire[1], f: cardinal(p.way) };
    lot.fire(at.x, at.y, 1.0, (SANDSTONE, SAND));
    if lot.rng.next() < 0.5 {
        lot.kettle(at.x, at.y, 1.4);
    }
    lot.seats(&at, 2, 1.9, Some(TEMPLE_RED));
    let stripes = [(CLOTH, LINEN), (TEMPLE_RED, LINEN), (TEMPLE_TEAL, LINEN), (LINEN, LEATHER)];
    let rugs = [(TEMPLE_RED, TEMPLE_TEAL), (CLOTH, LINEN), (TEMPLE_TEAL, LINEN), (CLOTH, TEMPLE_TEAL)];
    let cushions = [TEMPLE_RED, TEMPLE_TEAL, CLOTH, LINEN];

    // The leader's, at the back, walled round but for the front
    let (hu, hv) = (lot.rng.range(1.7, 2.0), lot.rng.range(2.3, 2.7));
    let cu = -(lot.rng.range(3.4, 3.9) + hu);
    let middle = at.point(cu, 0.0, 0.0);
    let big = Frame { x: middle[0], y: middle[1], f: at.f };
    let cloth = stripes[(lot.rng.next() * 2.99) as usize];
    let rug = rugs[(lot.rng.next() * 3.99) as usize];
    let tall = lot.rng.range(2.4, 2.7);
    let floor = lot.awning(&big, [-hu, hu], [-hv, hv], tall, cloth, [true, true, true], rug);
    for s in [-1.0f32, 1.0] {
        let q = lot.on_rug(&big, -hu + 0.7, s * (hv - 0.75), floor);
        let color = cushions[(lot.rng.next() * 3.99) as usize];
        lot.cushion(q[0], q[1], q[2], color);
        let q = lot.on_rug(&big, -hu + 0.45, s * (hv - 0.3), floor);
        lot.pot(q[0], q[1], q[2], 1.0);
    }
    let chest = lot.on_rug(&big, -hu + 0.75, 0.0, floor);

    // Smaller ones either side, facing in, and maybe a third beside the leader's
    let mut spots: Vec<([f32; 2], f32)> = Vec::new();
    for s in [-1.0f32, 1.0] {
        spots.push(([lot.rng.range(-0.6, 1.0), s * lot.rng.range(4.4, 5.0)], s));
    }
    if lot.rng.next() < 0.5 {
        let s = lot.flip();
        spots.push(([cu + 0.3, s * (hv + 2.2)], s));
    }
    for (c, s) in spots {
        let middle = at.point(c[0], c[1], 0.0);
        let side = Frame { x: middle[0], y: middle[1], f: at.dir([0.0, -s]) };
        let (su, sv) = (lot.rng.range(1.15, 1.4), lot.rng.range(1.5, 1.8));
        let cloth = stripes[(lot.rng.next() * 3.99) as usize];
        let rug = rugs[(lot.rng.next() * 3.99) as usize];
        let (tall, walled) = (lot.rng.range(2.1, 2.35), lot.rng.next() < 0.6);
        let floor = lot.awning(&side, [-su, su], [-sv, sv], tall, cloth, [walled, false, false], rug);
        let q = lot.on_rug(&side, -su + 0.55, -sv + 0.6, floor);
        let color = cushions[(lot.rng.next() * 3.99) as usize];
        lot.cushion(q[0], q[1], q[2], color);
        let q = lot.on_rug(&side, -su + 0.45, sv - 0.4, floor);
        let size = if lot.rng.next() < 0.5 { 1.0 } else { 1.3 };
        lot.pot(q[0], q[1], q[2], size);
        let q = lot.on_rug(&side, -su + 0.5, 0.2, floor);
        lot.sack_at(q[0], q[1], q[2]);
    }

    // A low wall of mud brick behind, against the wind, and pots and braziers by the way in
    lot.ruin_wall(&at, [cu - hu - 1.6, cu - hu - 1.05], [-6.3, 6.3], (0.8, 1.3), (SANDSTONE, SAND), None, 0.12, None, false);
    for s in [-1.0f32, 1.0] {
        let q = at.point(5.6, s * 2.3, 0.0);
        lot.brazier(q[0], q[1]);
    }
    let s = lot.flip();
    for k in 0..3 {
        let q = lot.on(&at, 3.0 + k as f32 * 0.45, s * (2.9 + (k % 2) as f32 * 0.4));
        lot.pot(q[0], q[1], q[2], [1.0, 1.35, 0.8][k]);
    }
    let q = at.point(2.6, -s * 3.2, 0.0);
    lot.weapon_rack(q[0], q[1], at.dir([1.0, 0.0]));
    (chest, big.heading(0.0), "under the striped awning")
}

/// A camp in the snow: round tents of fur about a great fire with a kettle over it, the leader's
/// the biggest with antlers on a pole by its door, logs with furs on them round the fire, snow
/// banked up round the back against the wind, a sledge and torches by the way in.
fn yurts(lot: &mut Lot, p: &Plan) -> Hoard {
    let at = Frame { x: p.fire[0], y: p.fire[1], f: p.way };
    lot.fire(at.x, at.y, 1.5, (ROCK, SNOW));
    lot.kettle(at.x, at.y, 1.6);
    let seats = 3 + (lot.rng.next() < 0.5) as usize;
    lot.seats(&at, seats, 2.3, Some(FUR_WHITE));
    let furs = [(FUR_WHITE, FUR_GRAY), (FUR_GRAY, FUR_WHITE), (FUR_TAN, FUR_BROWN), (FUR_BROWN, FUR_DARK), (LEATHER, FUR_BROWN)];
    let n = 3 + (lot.rng.next() * 2.6) as usize;
    let leader = (lot.rng.next() * n as f32) as usize % n;
    let span = TAU - 1.7;
    let mut hoard = None;
    let mut angles = Vec::new();
    for k in 0..n {
        let a = 0.85 + (k as f32 + 0.5) * span / n as f32 + lot.rng.range(-0.1, 0.1);
        angles.push(a);
        let dir = [a.cos(), a.sin()];
        let big = k == leader;
        let r = if big { lot.rng.range(2.2, 2.4) } else { lot.rng.range(1.6, 1.95) };
        let d = r + lot.rng.range(3.6, 4.1) + 0.15 * (n - 3) as f32;
        let middle = at.point(dir[0] * d, dir[1] * d, 0.0);
        let toward = at.dir([-dir[0], -dir[1]]);
        let fur = furs[(lot.rng.next() * 4.99) as usize];
        let door = lot.yurt(middle[0], middle[1], r, toward, fur);
        if big {
            hoard = Some((door, toward[1].atan2(toward[0])));
            let side = [-toward[1], toward[0]];
            let q = [door[0] + side[0] * 1.3 - toward[0] * 0.4, door[1] + side[1] * 1.3 - toward[1] * 0.4];
            lot.antlers(q[0], q[1]);
        }
    }
    // Racks of pelts out behind, between the tents, and snow banked up round the back
    for k in 0..n.saturating_sub(1) {
        if k % 2 == 1 {
            continue;
        }
        let a = (angles[k] + angles[k + 1]) * 0.5;
        let q = at.point(7.4 * a.cos(), 7.4 * a.sin(), 0.0);
        let pelt = [FUR_WHITE, FUR_GRAY, FUR_TAN][(lot.rng.next() * 2.99) as usize];
        lot.drying_rack(q[0], q[1], at.dir([-a.sin(), a.cos()]), [pelt, FUR_WHITE]);
    }
    let reach = lot.rng.range(0.8, 1.1);
    lot.snowbank(&at, 10.0, PI - reach, PI + reach);
    let s = lot.flip();
    let middle = at.point(5.5, s * 2.9, 0.0);
    lot.sledge(&Frame { x: middle[0], y: middle[1], f: at.f });
    for side in [-1.0f32, 1.0] {
        let q = at.point(7.4, side * 1.4, 0.0);
        lot.torch(q[0], q[1], 2.0);
    }
    let (spot, heading) = hoard.unwrap_or_else(|| (at.point(-2.0, 0.0, p.fire[2]), at.heading(0.0)));
    (spot, heading, "before the chieftain's tent")
}

/// What a lean-to's covered with
#[derive(Clone, Copy)]
enum Cover {
    /// A hide stretched over it, of a material and an accent
    Hide(u8, u8),
    /// Leafy boughs piled on it
    Boughs(u8),
    /// Palm leaves laid down it in rows
    Thatch,
}

/// What's being built for a camp: its shapes in order, earlier ones winning where they overlap,
/// and the lights of its fires and torches, on `ground`. In under a hill, everything stands on
/// the `level` floor of the cave instead and is placed `place`, so in the hollow.
struct Lot<'a> {
    s: Vec<Shape>,
    lights: Vec<Light>,
    ground: &'a dyn Fn(f32, f32) -> f32,
    level: Option<f32>,
    place: Placement,
    rng: Rng,
}

impl Lot<'_> {
    fn g(&self, x: f32, y: f32) -> f32 {
        self.level.unwrap_or_else(|| (self.ground)(x, y))
    }

    /// The point `u`, `v` of frame `at`, on the ground there
    fn on(&self, at: &Frame, u: f32, v: f32) -> Vec3 {
        let p = at.point(u, v, 0.0);
        [p[0], p[1], self.g(p[0], p[1])]
    }

    /// The lowest and highest ground under the points `uv` of frame `at`
    fn span(&self, at: &Frame, uv: &[(f32, f32)]) -> (f32, f32) {
        uv.iter().map(|&(u, v)| self.on(at, u, v)[2]).fold((f32::MAX, f32::MIN), |(lo, hi), g| (lo.min(g), hi.max(g)))
    }

    fn put(&mut self, shape: Shape) {
        let place = self.place;
        self.s.push(shape.placed(place));
    }

    /// A pole, a log or a rope from `a` to `b`
    fn stick(&mut self, a: Vec3, b: Vec3, r: f32, material: u8) {
        self.put(Shape::capsule(a, b, r, r, material, AIR, false));
    }

    fn light(&mut self, at: Vec3, color: [f32; 3], reach: f32) {
        self.lights.push(Light { position: at, color, reach });
    }

    /// -1 or 1
    fn flip(&mut self) -> f32 {
        if self.rng.next() < 0.5 { -1.0 } else { 1.0 }
    }

    /// A fire at `x`, `y`: logs burning on embers in a ring of `stone`. `size` 1 is a cooking fire.
    fn fire(&mut self, x: f32, y: f32, size: f32, stone: (u8, u8)) {
        let z = self.g(x, y);
        let turn = self.rng.next() * TAU;
        self.put(Shape::sphere([x, y, z + 0.1 * size], 0.32 * size, GLOW_FIRE, AIR, false));
        let logs = if size > 1.2 { 5 } else { 3 };
        for k in 0..logs {
            let a = turn + k as f32 * TAU / logs as f32;
            let (dx, dy) = (a.cos() * 0.55 * size, a.sin() * 0.55 * size);
            self.put(Shape::capsule([x + dx, y + dy, z + 0.05], [x - dx * 0.3, y - dy * 0.3, z + 0.45 * size], 0.1 * size, 0.07 * size, BARK_DARK, AIR, false));
        }
        let stones = (9.0 * size).round() as usize;
        for k in 0..stones {
            let a = turn + k as f32 / stones as f32 * TAU;
            let (sx, sy) = (x + a.cos() * 0.75 * size, y + a.sin() * 0.75 * size);
            let (r, g) = (self.rng.range(0.14, 0.2) * size.sqrt(), self.g(sx, sy));
            self.put(Shape::sphere([sx, sy, g], r, stone.0, stone.1, false));
        }
        self.light([x, y, z + 0.8 * size], FIRE_LIGHT, 13.0 * size.sqrt());
    }

    /// Three poles over the fire at `x`, `y`, crossing `height` over it, a kettle hung from them
    fn kettle(&mut self, x: f32, y: f32, height: f32) {
        let top = self.g(x, y) + height;
        let turn = self.rng.next() * TAU;
        for k in 0..3 {
            let a = turn + k as f32 * TAU / 3.0;
            let foot = [x + a.cos() * 1.0, y + a.sin() * 1.0];
            let g = self.g(foot[0], foot[1]);
            self.stick([foot[0], foot[1], g - 0.1], [x - a.cos() * 0.12, y - a.sin() * 0.12, top + 0.18], 0.04, BARK_DARK);
        }
        self.stick([x, y, top + 0.1], [x, y, top - 0.45], 0.015, DARK_STEEL);
        self.put(Shape::ellipsoid([x, y, top - 0.62], [0.24, 0.24, 0.18], DARK_STEEL, AIR));
    }

    /// `n` logs to sit on round the fire in the middle of `at`, `radius` out, leaving the way in
    /// along u, with a fur over each if `fur` says
    fn seats(&mut self, at: &Frame, n: usize, radius: f32, fur: Option<u8>) {
        for k in 0..n {
            let a = 0.8 + (k as f32 + 0.5) * (TAU - 1.6) / n as f32 + self.rng.range(-0.15, 0.15);
            let q = at.point(radius * a.cos(), radius * a.sin(), 0.0);
            let along = at.dir([-a.sin(), a.cos()]);
            if let Some(fur) = fur {
                let z = self.g(q[0], q[1]);
                self.put(Shape::ellipsoid([q[0], q[1], z + 0.35], [0.42, 0.42, 0.07], fur, AIR));
            }
            self.seat(q[0], q[1], along);
        }
    }

    fn seat(&mut self, x: f32, y: f32, along: [f32; 2]) {
        let z = self.g(x, y);
        self.put(Shape::capsule([x - along[0] * 0.7, y - along[1] * 0.7, z + 0.16], [x + along[0] * 0.7, y + along[1] * 0.7, z + 0.16], 0.19, 0.19, BARK_DARK, WOOD, false));
    }

    fn torch(&mut self, x: f32, y: f32, tall: f32) {
        let z = self.g(x, y);
        self.put(Shape::capsule([x, y, z - 0.3], [x, y, z + tall], 0.06, 0.05, WOOD, AIR, false));
        self.put(Shape::sphere([x, y, z + tall + 0.1], 0.12, GLOW_FIRE, AIR, false));
        self.light([x, y, z + tall + 0.4], TORCH_LIGHT, 7.0);
    }

    /// A bronze bowl of burning coals on three legs
    fn brazier(&mut self, x: f32, y: f32) {
        let z = self.g(x, y);
        let top = z + 1.0;
        self.put(Shape::ellipsoid([x, y, top + 0.18], [0.24, 0.24, 0.06], GLOW_FIRE, AIR));
        self.put(Shape::ellipsoid([x, y, top + 0.08], [0.32, 0.32, 0.14], BRONZE, AIR));
        for k in 0..3 {
            let a = k as f32 * TAU / 3.0;
            self.stick([x + a.cos() * 0.35, y + a.sin() * 0.35, z - 0.1], [x + a.cos() * 0.12, y + a.sin() * 0.12, top], 0.035, DARK_STEEL);
        }
        self.light([x, y, top + 0.6], TORCH_LIGHT, 8.0);
    }

    /// A lantern with its flame at `at`, in a frame of bronze
    fn lantern(&mut self, at: Vec3) {
        let [x, y, z] = at;
        self.put(Shape::block([x - 0.06, y - 0.06, z + 0.13], [x + 0.06, y + 0.06, z + 0.2], BRONZE, AIR));
        self.put(Shape::block([x - 0.085, y - 0.085, z - 0.11], [x + 0.085, y + 0.085, z + 0.11], FLAME, AIR));
        self.put(Shape::block([x - 0.12, y - 0.12, z - 0.18], [x + 0.12, y + 0.12, z - 0.12], BRONZE, AIR));
        for (dx, dy) in [(-0.1, -0.1), (0.1, -0.1), (-0.1, 0.1), (0.1, 0.1)] {
            self.put(Shape::block([x + dx - 0.02, y + dy - 0.02, z - 0.13], [x + dx + 0.02, y + dy + 0.02, z + 0.14], DARK_STEEL, AIR));
        }
        self.light([x, y, z - 0.35], TORCH_LIGHT, 9.0);
    }

    /// A crate `half` meters either way of its middle, on whatever's at height `z`
    fn crate_at(&mut self, x: f32, y: f32, z: f32, half: f32) {
        for z0 in [z, z + half * 2.0 - 0.07] {
            self.put(Shape::block([x - half - 0.03, y - half - 0.03, z0], [x + half + 0.03, y + half + 0.03, z0 + 0.07], BARK_DARK, AIR));
        }
        self.put(Shape::block([x - half, y - half, z - 0.15], [x + half, y + half, z + half * 2.0], WOOD, AIR));
    }

    /// A barrel bound with iron hoops, standing at height `z`
    fn barrel_at(&mut self, x: f32, y: f32, z: f32) {
        for h in [0.17, 0.63] {
            self.put(Shape::ellipsoid([x, y, z + h], [0.31, 0.31, 0.035], DARK_STEEL, AIR));
        }
        self.put(Shape::capsule([x, y, z + 0.3], [x, y, z + 0.5], 0.28, 0.28, WOOD, BARK_DARK, false));
    }

    /// A sack tied at the top
    fn sack_at(&mut self, x: f32, y: f32, z: f32) {
        self.put(Shape::capsule([x, y, z + 0.5], [x, y, z + 0.6], 0.06, 0.04, LEATHER, AIR, false));
        self.put(Shape::ellipsoid([x, y, z + 0.26], [0.25, 0.22, 0.28], LINEN, AIR));
    }

    /// A pile of what they've taken: `things` crates, barrels and sacks round `x`, `y`
    fn stash(&mut self, x: f32, y: f32, things: usize) {
        let turn = self.rng.next() * TAU;
        for k in 0..things {
            let a = turn + k as f32 * 2.4;
            let d = if k == 0 { 0.0 } else { 0.7 + 0.1 * k as f32 };
            let (px, py) = (x + a.cos() * d, y + a.sin() * d);
            let z = self.g(px, py);
            let roll = self.rng.next();
            if roll < 0.45 {
                let half = self.rng.range(0.27, 0.37);
                self.crate_at(px, py, z, half);
                if self.rng.next() < 0.35 {
                    self.crate_at(px + 0.04, py - 0.03, z + half * 2.0, half * 0.78);
                }
            } else if roll < 0.75 {
                self.barrel_at(px, py, z);
            } else {
                self.sack_at(px, py, z);
            }
        }
    }

    /// A bedroll laid out on the ground, its head rolled up at the end `along` points to
    fn bedroll(&mut self, x: f32, y: f32, along: [f32; 2], color: u8) {
        let z = self.g(x, y);
        let side = [-along[1], along[0]];
        let head = [x + along[0] * 0.75, y + along[1] * 0.75];
        self.put(Shape::capsule([head[0] - side[0] * 0.3, head[1] - side[1] * 0.3, z + 0.12], [head[0] + side[0] * 0.3, head[1] + side[1] * 0.3, z + 0.12], 0.12, 0.12, color, LINEN, false));
        self.put(Shape::leaf([x, y, z + 0.04], flat(along), [0.0, 0.0, 1.0], 1.85, 0.72, color, LINEN));
    }

    /// A rack of spears, its bar `along`
    fn weapon_rack(&mut self, x: f32, y: f32, along: [f32; 2]) {
        let z = self.g(x, y);
        let side = [-along[1], along[0]];
        let at = |t: f32, s: f32, h: f32| [x + along[0] * t + side[0] * s, y + along[1] * t + side[1] * s, z + h];
        for t in [-0.7, 0.7] {
            self.stick(at(t, 0.0, -0.3), at(t, 0.0, 1.25), 0.05, BARK_DARK);
        }
        self.stick(at(-0.8, 0.0, 1.15), at(0.8, 0.0, 1.15), 0.04, WOOD);
        for k in 0..4 {
            let t = -0.48 + k as f32 * 0.32;
            let (foot, top) = (at(t, 0.42, 0.0), at(t, -0.04, 1.55));
            let d = normalize(sub(top, foot));
            self.put(Shape::capsule(top, add(top, scale(d, 0.24)), 0.045, 0.006, STEEL, AIR, false));
            self.stick(foot, top, 0.025, WOOD);
        }
    }

    /// Two posts with a bar between them `along`, two hides hung over it to dry
    fn drying_rack(&mut self, x: f32, y: f32, along: [f32; 2], hides: [u8; 2]) {
        let z = self.g(x, y);
        let side = [-along[1], along[0]];
        let p = |t: f32, h: f32| [x + along[0] * t, y + along[1] * t, z + h];
        for (k, &hide) in hides.iter().enumerate() {
            self.put(Shape::leaf(p(-0.45 + k as f32 * 0.9, 1.3), [0.0, 0.0, 1.0], flat(side), 1.0, 0.72, hide, FUR_DARK));
        }
        self.stick(p(-1.05, 1.82), p(1.05, 1.82), 0.04, BARK_DARK);
        for t in [-0.95, 0.95] {
            self.stick(p(t, -0.3), p(t, 1.9), 0.05, WOOD);
        }
    }

    /// Logs piled up `along`, three, two and one
    fn woodpile(&mut self, x: f32, y: f32, along: [f32; 2]) {
        let z = self.g(x, y);
        let side = [-along[1], along[0]];
        for row in 0..3 {
            let n = 3 - row;
            for k in 0..n {
                let off = (k as f32 - (n - 1) as f32 * 0.5) * 0.25;
                let c = [x + side[0] * off, y + side[1] * off, z + 0.12 + row as f32 * 0.21];
                let (m, a) = if (k + row) % 2 == 0 { (WOOD, BARK_DARK) } else { (BARK_DARK, WOOD) };
                self.put(Shape::capsule([c[0] - along[0] * 0.6, c[1] - along[1] * 0.6, c[2]], [c[0] + along[0] * 0.6, c[1] + along[1] * 0.6, c[2]], 0.12, 0.12, m, a, false));
            }
        }
    }

    /// A stump with an axe stuck in it
    fn stump(&mut self, x: f32, y: f32) {
        let z = self.g(x, y);
        let a = self.rng.next() * TAU;
        self.put(Shape::block([x - 0.06, y - 0.06, z + 0.36], [x + 0.06, y + 0.06, z + 0.52], DARK_STEEL, AIR));
        self.stick([x, y, z + 0.5], [x + a.cos() * 0.32, y + a.sin() * 0.32, z + 0.98], 0.028, WOOD);
        self.put(Shape::ellipsoid([x, y, z + 0.42], [0.25, 0.25, 0.035], WOOD, AIR));
        self.put(Shape::capsule([x, y, z - 0.3], [x, y, z + 0.18], 0.27, 0.27, BARK_DARK, AIR, false));
    }

    /// Meat roasting over the fire at `x`, `y` on a spit `along`, on forked posts either side
    fn spit(&mut self, x: f32, y: f32, along: [f32; 2]) {
        let z = self.g(x, y);
        let side = [-along[1], along[0]];
        let p = |t: f32, s: f32, h: f32| [x + along[0] * t + side[0] * s, y + along[1] * t + side[1] * s, z + h];
        self.put(Shape::capsule(p(-0.28, 0.0, 0.93), p(0.28, 0.0, 0.93), 0.15, 0.15, MEAT, BONE, false));
        self.stick(p(-1.0, 0.0, 0.95), p(1.0, 0.0, 0.95), 0.025, DARK_STEEL);
        for t in [-0.88, 0.88] {
            self.stick(p(t, 0.0, -0.2), p(t, 0.0, 0.9), 0.04, WOOD);
            for s in [-0.13, 0.13] {
                self.stick(p(t, 0.0, 0.85), p(t, s, 1.08), 0.025, WOOD);
            }
        }
    }

    /// A tall pole with a skull on top and a banner of `cloth` hung from a bar, facing `facing`
    fn banner(&mut self, x: f32, y: f32, facing: [f32; 2], cloth: u8) {
        let z = self.g(x, y);
        let top = z + 3.1;
        let side = [-facing[1], facing[0]];
        self.put(Shape::sphere([x, y, top + 0.12], 0.12, BONE, AIR, false));
        self.stick([x, y, z - 0.4], [x, y, top], 0.05, BARK_DARK);
        self.stick([x - side[0] * 0.42, y - side[1] * 0.42, top - 0.15], [x + side[0] * 0.42, y + side[1] * 0.42, top - 0.15], 0.03, BARK_DARK);
        self.put(Shape::leaf([x + facing[0] * 0.06, y + facing[1] * 0.06, top - 0.78], [0.0, 0.0, 1.0], flat(facing), 1.25, 0.78, cloth, AIR));
    }

    /// A horned skull on a pole
    fn skull_pole(&mut self, x: f32, y: f32) {
        let z = self.g(x, y);
        let top = z + self.rng.range(1.6, 2.0);
        self.put(Shape::sphere([x, y, top + 0.08], 0.12, BONE, AIR, false));
        for s in [-1.0, 1.0] {
            self.stick([x, y, top + 0.12], [x + s * 0.26, y, top + 0.36], 0.03, BONE);
        }
        self.stick([x, y, z - 0.3], [x, y, top], 0.045, BARK_DARK);
    }

    /// A pole with a skull and antlers on top
    fn antlers(&mut self, x: f32, y: f32) {
        let z = self.g(x, y);
        let top = z + 2.2;
        self.put(Shape::sphere([x, y, top], 0.14, BONE, AIR, false));
        for s in [-1.0f32, 1.0] {
            let (base, tip) = ([x + s * 0.1, y, top + 0.08], [x + s * 0.55, y, top + 0.75]);
            self.stick(base, tip, 0.035, BONE);
            for k in 1..3 {
                let q = lerp3(base, tip, k as f32 * 0.33);
                self.stick(q, [q[0] + s * 0.08, q[1] + 0.12, q[2] + 0.28], 0.025, BONE);
            }
        }
        self.stick([x, y, z - 0.3], [x, y, top - 0.08], 0.05, BARK_DARK);
    }

    /// An iron cage, with bones in it
    fn cage(&mut self, x: f32, y: f32) {
        let z = self.g(x, y);
        self.put(Shape::sphere([x + 0.2, y - 0.1, z + 0.14], 0.11, BONE, AIR, false));
        self.put(Shape::ellipsoid([x, y, z + 1.62], [0.66, 0.66, 0.05], DARK_STEEL, AIR));
        self.put(Shape::ellipsoid([x, y, z + 0.03], [0.68, 0.68, 0.06], BARK_DARK, AIR));
        for k in 0..12 {
            let a = k as f32 * TAU / 12.0;
            let (bx, by) = (x + a.cos() * 0.6, y + a.sin() * 0.6);
            self.stick([bx, by, z - 0.05], [bx, by, z + 1.62], 0.025, DARK_STEEL);
        }
        self.stick([x, y, z + 1.62], [x, y, z + 1.95], 0.035, DARK_STEEL);
    }

    /// Sharpened stakes crossed through a log lying `along`, to keep riders out
    fn spikes(&mut self, x: f32, y: f32, along: [f32; 2]) {
        let z = self.g(x, y);
        let side = [-along[1], along[0]];
        let c = |t: f32| [x + along[0] * t, y + along[1] * t, z + 0.55];
        self.stick(c(-1.3), c(1.3), 0.11, BARK_DARK);
        for k in 0..4 {
            let m = c(-0.95 + k as f32 * 0.63);
            for s in [-1.0f32, 1.0] {
                let a = [m[0] - side[0] * s * 0.75, m[1] - side[1] * s * 0.75, z - 0.05];
                let b = [m[0] + side[0] * s * 0.8, m[1] + side[1] * s * 0.8, z + 1.25];
                self.put(Shape::capsule(a, b, 0.07, 0.02, WOOD, BARK_DARK, false));
            }
        }
    }

    /// A cart along u of `at`, its shafts down in front: two wheels, a bed of planks with rails
    /// round it. Returns how high its bed is, for what's loaded on it.
    fn cart(&mut self, at: &Frame) -> f32 {
        let (_, g) = self.span(at, &[(0.0, -0.85), (0.0, 0.85)]);
        let (u3, v3) = (flat(at.dir([1.0, 0.0])), flat(at.dir([0.0, 1.0])));
        let bed = g + 0.72;
        for v in [-0.86, 0.86] {
            for off in [-0.05, 0.05] {
                self.put(Shape::leaf(at.point(-0.2, v + off, g + 0.45), u3, v3, 0.9, 0.9, BARK_DARK, WOOD));
            }
        }
        self.stick(at.point(-0.2, -1.0, g + 0.45), at.point(-0.2, 1.0, g + 0.45), 0.06, DARK_STEEL);
        for k in 0..5 {
            let v = -0.6 + k as f32 * 0.3;
            self.stick(at.point(-1.15, v, bed), at.point(1.0, v, bed), 0.075, WOOD);
        }
        for v in [-0.68, 0.68] {
            self.stick(at.point(-1.15, v, bed + 0.3), at.point(1.0, v, bed + 0.3), 0.05, BARK_DARK);
            for u in [-1.1, 0.0, 0.95] {
                self.stick(at.point(u, v, bed), at.point(u, v, bed + 0.32), 0.04, BARK_DARK);
            }
        }
        let tip = self.on(at, 2.5, 0.0);
        for v in [-0.42, 0.42] {
            self.stick(at.point(1.0, v, bed), at.point(2.5, v * 0.85, tip[2] + 0.15), 0.04, WOOD);
        }
        bed + 0.07
    }

    /// A sledge along u of `at`, piled with furs
    fn sledge(&mut self, at: &Frame) {
        let g = self.on(at, 0.0, 0.0)[2];
        self.put(Shape::ellipsoid(at.point(-0.15, 0.0, g + 0.5), [0.55, 0.55, 0.16], FUR_WHITE, FUR_GRAY));
        for v in [-0.5, 0.5] {
            self.stick(at.point(-1.1, v, g + 0.06), at.point(0.9, v, g + 0.06), 0.05, BARK_DARK);
            self.stick(at.point(0.9, v, g + 0.06), at.point(1.25, v, g + 0.42), 0.05, BARK_DARK);
            for u in [-0.8, 0.0, 0.7] {
                self.stick(at.point(u, v, g + 0.06), at.point(u, v, g + 0.32), 0.035, WOOD);
            }
        }
        for k in 0..6 {
            let u = -0.95 + k as f32 * 0.32;
            self.stick(at.point(u, -0.55, g + 0.34), at.point(u, 0.55, g + 0.34), 0.06, WOOD);
        }
    }

    /// A tent of hide over poles that cross at the top, at `x`, `y`, a dark way in on the side
    /// facing `toward`
    fn teepee(&mut self, x: f32, y: f32, toward: [f32; 2], hide: u8) {
        let base = self.g(x, y);
        let tall = self.rng.range(2.4, 2.9);
        self.put(Shape::ellipsoid([x + toward[0], y + toward[1], base + 0.5], [0.3, 0.3, 0.55], BARK_DARK, AIR));
        self.put(Shape::capsule([x, y, base - 0.3], [x, y, base + tall], 1.55, 0.08, hide, FUR_BROWN, false));
        let a = toward[1].atan2(toward[0]);
        for p in 0..3 {
            let pa = a + PI + p as f32 * TAU / 3.0 + 0.5;
            let foot = [x + pa.cos() * 1.4, y + pa.sin() * 1.4, base];
            let tip = [x - pa.cos() * 0.35, y - pa.sin() * 0.35, base + tall + 0.6];
            self.stick(foot, tip, 0.045, WOOD);
        }
    }

    /// A log cabin in frame `at`, its door at the front, along u, `half` its size along u and v:
    /// walls of logs of `logs` crossing at the corners, `wall` high over the highest ground under
    /// it, a doorway and a window, and a pitched roof of `roof` (cover, accent and ridge) with its
    /// ridge running front to back, snowy on top if `snow`, maybe a stone chimney at the back, and
    /// a lantern hung inside if `lit`. Returns the spot on the floor inside, by the back wall.
    #[allow(clippy::too_many_arguments)]
    fn cabin(&mut self, at: &Frame, half: [f32; 2], wall: f32, logs: (u8, u8), roof: (u8, u8, u8), snow: bool, chimney: bool, lit: bool) -> Vec3 {
        let [hu, hv] = half;
        let (base, high) = self.span(at, &[(hu, hv), (hu, -hv), (-hu, hv), (-hu, -hv), (0.0, 0.0), (hu, 0.0)]);
        let door = self.on(at, hu, 0.0)[2];
        let top = high + wall;
        let (r, course) = (0.15, 0.27);
        let courses = ((top - base) / course).ceil() as usize;
        let window = self.flip();
        let sill = high + 0.95;
        let log = |lot: &mut Self, a: Vec3, b: Vec3| lot.put(Shape::capsule(a, b, r, r, logs.0, logs.1, false));
        for k in 0..courses {
            let z = base + 0.1 + k as f32 * course;
            for side in [-1.0f32, 1.0] {
                let v = side * (hv - r);
                if side == window && z > sill && z < sill + 0.7 {
                    log(self, at.point(-hu - 0.22, v, z), at.point(-0.45, v, z));
                    log(self, at.point(0.45, v, z), at.point(hu + 0.22, v, z));
                } else {
                    log(self, at.point(-hu - 0.22, v, z), at.point(hu + 0.22, v, z));
                }
            }
            let z = (z + course * 0.5).min(top);
            log(self, at.point(-(hu - r), -hv - 0.22, z), at.point(-(hu - r), hv + 0.22, z));
            if z < door + 1.95 {
                log(self, at.point(hu - r, -hv - 0.22, z), at.point(hu - r, -0.55, z));
                log(self, at.point(hu - r, 0.55, z), at.point(hu - r, hv + 0.22, z));
            } else {
                log(self, at.point(hu - r, -hv - 0.22, z), at.point(hu - r, hv + 0.22, z));
            }
        }
        // The doorway's frame
        for v in [-0.55, 0.55] {
            self.stick(at.point(hu - r, v, door - 0.2), at.point(hu - r, v, door + 2.0), 0.08, BARK_DARK);
        }
        self.stick(at.point(hu - r, -0.75, door + 2.05), at.point(hu - r, 0.75, door + 2.05), 0.1, BARK_DARK);

        // The roof, rows of shingles or thatch down either side of the ridge
        let eaves = top - 0.05;
        let across = hv + 0.4;
        let pitch = self.rng.range(0.7, 0.95);
        let rise = across * pitch;
        let rows = (across * (1.0 + pitch * pitch).sqrt() / 0.21).ceil() as usize;
        let long = hu + 0.35;
        let row = |k: usize| (across * (1.0 - k as f32 / rows as f32), eaves + rise * k as f32 / rows as f32);
        if snow {
            self.stick(at.point(-long, 0.0, eaves + rise + 0.14), at.point(long, 0.0, eaves + rise + 0.14), 0.18, SNOW);
            for k in 1..rows {
                let (b, z) = row(k);
                for side in [-1.0f32, 1.0] {
                    self.stick(at.point(-long, side * b, z + 0.1), at.point(long, side * b, z + 0.1), 0.12, SNOW);
                }
            }
        }
        self.stick(at.point(-long, 0.0, eaves + rise + 0.03), at.point(long, 0.0, eaves + rise + 0.03), 0.16, roof.2);
        for k in 0..rows {
            let (b, z) = row(k);
            for side in [-1.0f32, 1.0] {
                self.put(Shape::capsule(at.point(-long, side * b, z), at.point(long, side * b, z), 0.12, 0.12, roof.0, roof.1, false));
            }
        }
        // The gable ends, logs stepping in up under the roof
        let mut z = base + 0.1 + courses as f32 * course + course * 0.5;
        loop {
            let w = across * (1.0 - (z - eaves + 0.14) / rise) - 0.1;
            if w < 0.15 {
                break;
            }
            for u in [hu - r, -(hu - r)] {
                log(self, at.point(u, -w, z), at.point(u, w, z));
            }
            z += course;
        }
        if chimney {
            let side = self.flip();
            let foot = self.on(at, -hu - 0.32, side * hv * 0.35);
            self.put(Shape::capsule([foot[0], foot[1], foot[2] - 0.3], [foot[0], foot[1], eaves + rise + 0.5], 0.36, 0.3, ROCK, STONE, false));
        }
        if lit {
            let lamp = at.point(-hu * 0.25, 0.0, eaves - 0.35);
            self.stick([lamp[0], lamp[1], lamp[2] + 0.2], at.point(-hu * 0.25, 0.0, eaves + rise), 0.015, DARK_STEEL);
            self.lantern(lamp);
        }
        self.on(at, -hu + 0.68, 0.0)
    }

    /// A lean-to in frame `at`, open at the front, toward u: a ridge pole on two forked posts
    /// `w` either side and `tall` over the ground, poles leaning back from it to the ground, under
    /// `cover`. Returns the spot under it, about halfway back.
    fn lean_to(&mut self, at: &Frame, w: f32, tall: f32, cover: Cover) -> Vec3 {
        let back = 2.1;
        let (_, high) = self.span(at, &[(0.0, -w), (0.0, w)]);
        let top = high + tall;
        let v3 = flat(at.dir([0.0, 1.0]));
        let hi = at.point(0.0, 0.0, top + 0.1);
        let lo = {
            let q = self.on(at, -back - 0.15, 0.0);
            [q[0], q[1], q[2] + 0.12]
        };
        let slope = normalize(sub(lo, hi));
        let mut normal = cross(slope, v3);
        if normal[2] < 0.0 {
            normal = scale(normal, -1.0);
        }
        let length = dot(sub(lo, hi), sub(lo, hi)).sqrt();
        match cover {
            Cover::Hide(m, a) => self.put(Shape::leaf(add(lerp3(hi, lo, 0.5), scale(normal, 0.07)), v3, normal, 2.0 * w + 0.7, length + 0.35, m, a)),
            Cover::Boughs(m) => {
                let across = (w / 0.55).ceil() as i32;
                for i in 0..3 {
                    for j in -across..=across {
                        let c = add(add(lerp3(hi, lo, (i as f32 + 0.5) / 3.0), scale(v3, j as f32 * 0.55)), scale(normal, 0.2));
                        let r = self.rng.range(0.42, 0.55);
                        self.put(Shape::sphere(c, r, m, darker(m), true));
                    }
                }
            }
            Cover::Thatch => {
                let across = (w / 0.4).ceil() as i32;
                for i in 0..2 {
                    for j in -across..=across {
                        let off = j as f32 * 0.4 + if i == 1 { 0.2 } else { 0.0 };
                        let c = add(add(lerp3(hi, lo, 0.3 + i as f32 * 0.42), scale(v3, off)), scale(normal, 0.08 + 0.05 * i as f32));
                        self.put(Shape::leaf(c, slope, normal, length * 0.62, 0.5, PALM_LEAF, LEAVES_DARK));
                    }
                }
            }
        }
        for v in [-w, w] {
            let f = self.on(at, 0.0, v);
            self.stick([f[0], f[1], f[2] - 0.3], [f[0], f[1], top + 0.08], 0.07, WOOD);
            for s in [-0.14, 0.14] {
                self.stick(at.point(0.0, v, top - 0.12), at.point(s, v, top + 0.2), 0.035, WOOD);
            }
        }
        self.stick(at.point(0.0, -w - 0.4, top), at.point(0.0, w + 0.4, top), 0.075, BARK_DARK);
        let poles = ((2.0 * w) / 0.36).round().max(2.0) as usize;
        for k in 0..=poles {
            let v = -w + k as f32 * 2.0 * w / poles as f32;
            let foot = self.on(at, -back, v);
            self.stick(at.point(0.04, v, top + 0.03), [foot[0], foot[1], foot[2] - 0.05], 0.045, DEAD_WOOD);
        }
        self.on(at, -0.95, 0.0)
    }

    /// An awning in frame `at`, which faces along x or y, over u from `u[0]` to `u[1]` and v from
    /// `v[0]` to `v[1]`, `tall` over the highest ground under it: cloth in stripes of `cloth`, its
    /// edge hanging scalloped along the front, poles at the corners with ropes out to pegs, cloth
    /// hung down the back and the sides as `walls` says, and a bordered rug of `rug` under it.
    /// Returns how high the rug is.
    #[allow(clippy::too_many_arguments)]
    fn awning(&mut self, at: &Frame, u: [f32; 2], v: [f32; 2], tall: f32, cloth: (u8, u8), walls: [bool; 3], rug: (u8, u8)) -> f32 {
        let corners = [(u[0], v[0]), (u[0], v[1]), (u[1], v[0]), (u[1], v[1])];
        let (low, high) = self.span(at, &corners);
        let top = high + tall;
        let stripes = ((v[1] - v[0]) / 0.5).round().max(1.0) as usize;
        let width = (v[1] - v[0]) / stripes as f32;
        for k in 0..stripes {
            let (a, b) = (v[0] + k as f32 * width, v[0] + (k + 1) as f32 * width);
            let m = if k % 2 == 0 { cloth.0 } else { cloth.1 };
            let hang = if k % 2 == 0 { 0.34 } else { 0.22 };
            self.put(at.block([u[1] + 0.05, u[1] + 0.11], [a, b], [top - hang, top + 0.06], m, AIR));
            self.put(at.block([u[0] - 0.12, u[1] + 0.11], [a, b], [top, top + 0.06], m, AIR));
        }
        let hem = low + 0.25;
        if walls[0] {
            self.put(at.block([u[0] - 0.12, u[0] - 0.06], [v[0], v[1]], [hem, top], cloth.1, AIR));
        }
        if walls[1] {
            self.put(at.block([u[0], u[1]], [v[0] - 0.06, v[0]], [hem, top], cloth.0, AIR));
        }
        if walls[2] {
            self.put(at.block([u[0], u[1]], [v[1], v[1] + 0.06], [hem, top], cloth.0, AIR));
        }
        // Poles, each with a rope out to a peg
        let (mu, mv) = ((u[0] + u[1]) * 0.5, (v[0] + v[1]) * 0.5);
        let mut poles = corners.to_vec();
        if u[1] - u[0] > 3.0 {
            poles.extend([(mu, v[0]), (mu, v[1])]);
        }
        for (pu, pv) in poles {
            let foot = self.on(at, pu, pv);
            self.put(Shape::sphere([foot[0], foot[1], top + 0.16], 0.07, BRONZE, AIR, false));
            self.stick([foot[0], foot[1], foot[2] - 0.3], [foot[0], foot[1], top + 0.12], 0.05, WOOD);
            let su = if pu == mu { 0.0 } else { (pu - mu).signum() };
            let sv = (pv - mv).signum();
            let peg = self.on(at, pu + su * 0.8, pv + sv * 0.8);
            self.stick([foot[0], foot[1], top + 0.08], [peg[0], peg[1], peg[2] + 0.15], 0.015, LINEN);
            self.stick([peg[0], peg[1], peg[2] - 0.2], [peg[0], peg[1], peg[2] + 0.2], 0.03, WOOD);
        }
        // Level with the ground in the middle, so on a slope it's under the ground on one side
        let flat = self.on(at, mu, mv)[2];
        let (ru, rv) = ([u[0] + 0.4, u[1] - 0.4], [v[0] + 0.4, v[1] - 0.4]);
        self.put(at.block([ru[0] + 0.18, ru[1] - 0.18], [rv[0] + 0.18, rv[1] - 0.18], [low - 0.1, flat + 0.045], rug.0, AIR));
        self.put(at.block(ru, rv, [low - 0.1, flat + 0.03], rug.1, AIR));
        flat + 0.045
    }

    /// The point `u`, `v` of frame `at` on whichever is higher there, the ground or a rug `rug`
    /// high
    fn on_rug(&self, at: &Frame, u: f32, v: f32, rug: f32) -> Vec3 {
        let q = self.on(at, u, v);
        [q[0], q[1], q[2].max(rug)]
    }

    /// A clay pot standing at height `z`, `size` 1 about knee high
    fn pot(&mut self, x: f32, y: f32, z: f32, size: f32) {
        self.put(Shape::capsule([x, y, z + 0.44 * size], [x, y, z + 0.55 * size], 0.085 * size, 0.11 * size, CLAY_POT, AIR, false));
        self.put(Shape::ellipsoid([x, y, z + 0.25 * size], [0.21 * size, 0.21 * size, 0.27 * size], CLAY_POT, SANDSTONE_DARK));
    }

    fn cushion(&mut self, x: f32, y: f32, z: f32, color: u8) {
        self.put(Shape::ellipsoid([x, y, z + 0.09], [0.3, 0.26, 0.11], color, LINEN));
    }

    /// A round tent of fur at `x`, `y`, `r` across, its door facing `toward`: a dome whose sides
    /// come straight down to the ground, snow lying on top, bound round with rope, a collar of
    /// wood round the smoke hole and a hide rolled up over the doorway. Returns the spot on the
    /// ground in front of its door.
    fn yurt(&mut self, x: f32, y: f32, r: f32, toward: [f32; 2], fur: (u8, u8)) -> Vec3 {
        let around = [(0.0, 0.0), (r, 0.0), (-r, 0.0), (0.0, r), (0.0, -r)];
        let base = around.iter().map(|&(dx, dy)| self.g(x + dx, y + dy)).fold(f32::MAX, f32::min);
        let (c, h) = (base + 0.35, r * 0.75 + 1.35);
        let side = [-toward[1], toward[0]];
        let door = [x + toward[0] * r, y + toward[1] * r];
        let width = |z: f32| r * (1.0 - ((z - c) / h).powi(2)).max(0.0).sqrt();
        self.stick([door[0] - side[0] * 0.55, door[1] - side[1] * 0.55, base + 1.66], [door[0] + side[0] * 0.55, door[1] + side[1] * 0.55, base + 1.66], 0.11, LEATHER);
        self.put(Shape::ellipsoid([door[0], door[1], base + 0.75], [0.48, 0.48, 0.86], AIR, AIR));
        self.put(Shape::ellipsoid([door[0] - toward[0] * 0.45, door[1] - toward[1] * 0.45, base + 0.75], [0.44, 0.44, 0.84], BARK_DARK, AIR));
        self.put(Shape::ellipsoid([x, y, c + 0.14], [r * 0.62, r * 0.62, h], SNOW, AIR));
        for z in [base + 0.95, base + 1.75] {
            let w = width(z) + 0.035;
            self.put(Shape::ellipsoid([x, y, z], [w, w, 0.05], BARK_DARK, AIR));
        }
        self.put(Shape::capsule([x, y, c + h - 0.2], [x, y, c + h + 0.12], 0.3, 0.3, BARK_DARK, AIR, false));
        self.put(Shape::ellipsoid([x, y, c], [r, r, h], fur.0, fur.1));
        let front = [x + toward[0] * (r + 0.6), y + toward[1] * (r + 0.6)];
        [front[0], front[1], self.g(front[0], front[1])]
    }

    /// Snow banked up along an arc `radius` out from the middle of `at`, from angle `from` to `to`
    fn snowbank(&mut self, at: &Frame, radius: f32, from: f32, to: f32) {
        let steps = ((to - from) * radius / 0.7).ceil() as usize;
        for k in 0..steps {
            let (a, b) = (from + (to - from) * k as f32 / steps as f32, from + (to - from) * (k + 1) as f32 / steps as f32);
            let (pa, pb) = (self.on(at, radius * a.cos(), radius * a.sin()), self.on(at, radius * b.cos(), radius * b.sin()));
            self.put(Shape::capsule([pa[0], pa[1], pa[2] + 0.15], [pb[0], pb[1], pb[2] + 0.15], 0.55, 0.55, SNOW, ICE, false));
            if k % 2 == 0 {
                self.put(Shape::capsule([pa[0], pa[1], pa[2] + 0.75], [pb[0], pb[1], pb[2] + 0.75], 0.35, 0.35, SNOW, AIR, false));
            }
        }
    }

    /// A watchtower in frame `at`, its platform `high` over the ground: legs splayed out at the
    /// foot and braced, a floor of planks with a rail round it, a roof of `roof` on posts over
    /// that, snowy if `snow`, a ladder up the back and a torch at one corner
    fn watchtower(&mut self, at: &Frame, high: f32, roof: (u8, u8), snow: bool) {
        let (foot, head) = (1.25, 0.95);
        let corners = [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, 1.0), (1.0, -1.0)];
        let (base, _) = self.span(at, &corners.map(|(su, sv)| (su * foot, sv * foot)));
        let floor = base + high;
        let middle = at.point(0.0, 0.0, 0.0);
        // The roof: discs of shingles, stepping in and up, snow lying on them
        let eaves = floor + 2.2;
        if snow {
            for k in 1..6 {
                let rad = 1.65 * (1.0 - k as f32 / 6.5) * 0.92;
                self.put(Shape::ellipsoid([middle[0], middle[1], eaves + k as f32 * 0.17 + 0.07], [rad, rad, 0.1], SNOW, AIR));
            }
        }
        for k in 0..6 {
            let rad = 1.65 * (1.0 - k as f32 / 6.5) + 0.1;
            self.put(Shape::ellipsoid([middle[0], middle[1], eaves + k as f32 * 0.17], [rad, rad, 0.12], roof.0, roof.1));
        }
        self.stick([middle[0], middle[1], eaves + 0.9], [middle[0], middle[1], eaves + 1.5], 0.04, BARK_DARK);
        // Legs up to the rail, and posts on up to the roof
        for (su, sv) in corners {
            let f = self.on(at, su * foot, sv * foot);
            self.stick([f[0], f[1], f[2] - 0.4], at.point(su * head, sv * head, floor + 1.0), 0.12, BARK_DARK);
            self.stick(at.point(su * head, sv * head, floor + 0.95), at.point(su * head, sv * head, eaves + 0.05), 0.07, WOOD);
        }
        // Braces across each side, from low on one leg to under the floor on the next
        let leg = |su: f32, sv: f32, z: f32| {
            let t = (z - base) / (floor + 1.0 - base);
            let s = foot + (head - foot) * t;
            at.point(su * s, sv * s, z)
        };
        for k in 0..4 {
            let (a, b) = (corners[k], corners[(k + 1) % 4]);
            self.stick(leg(a.0, a.1, base + 0.5), leg(b.0, b.1, floor - 0.25), 0.06, WOOD);
            self.stick(leg(b.0, b.1, base + 0.5), leg(a.0, a.1, floor - 0.25), 0.06, WOOD);
            self.stick(leg(a.0, a.1, floor + 0.95), leg(b.0, b.1, floor + 0.95), 0.05, WOOD);
        }
        // The floor of planks on two beams
        for v in [-0.85, 0.85] {
            self.stick(at.point(-1.15, v, floor - 0.16), at.point(1.15, v, floor - 0.16), 0.1, BARK_DARK);
        }
        for k in 0..12 {
            let u = -1.08 + k as f32 * 0.196;
            self.stick(at.point(u, -1.1, floor), at.point(u, 1.1, floor), 0.09, WOOD);
        }
        // A ladder up the back
        let ground = self.on(at, -foot - 0.75, 0.0)[2];
        for v in [-0.26, 0.26] {
            self.stick(at.point(-foot - 0.75, v, ground - 0.2), at.point(-head - 0.12, v, floor + 0.9), 0.04, WOOD);
        }
        let rungs = ((floor - ground) / 0.32) as usize;
        for k in 1..=rungs {
            let t = k as f32 / (rungs + 1) as f32;
            let u = -foot - 0.75 + (foot + 0.75 - head - 0.12) * t;
            let z = ground - 0.2 + (floor + 1.1 - ground) * t;
            self.stick(at.point(u, -0.26, z), at.point(u, 0.26, z), 0.03, WOOD);
        }
        // A torch at the front corner
        let t = at.point(head - 0.1, head - 0.1, floor);
        self.stick(t, [t[0], t[1], floor + 1.45], 0.05, WOOD);
        self.put(Shape::sphere([t[0], t[1], floor + 1.55], 0.12, GLOW_FIRE, AIR, false));
        self.light([t[0], t[1], floor + 1.85], TORCH_LIGHT, 9.0);
    }

    /// A wall of `stone` in frame `at`, which faces along x or y, from u[0] to u[1] and v[0] to
    /// v[1], in pieces laid in courses broken off between `height.0` and `height.1` over the
    /// ground under each, with `cap` lying on them and vines hanging down if `vines`. Gaps are left
    /// where pieces have fallen, `fall` of them, and through `door`, along the wall. Returns the
    /// middles of the gaps fallen, u and v.
    #[allow(clippy::too_many_arguments)]
    fn ruin_wall(&mut self, at: &Frame, u: [f32; 2], v: [f32; 2], height: (f32, f32), stone: (u8, u8), cap: Option<u8>, fall: f32, door: Option<[f32; 2]>, vines: bool) -> Vec<(f32, f32)> {
        let along_u = (u[1] - u[0]).abs() >= (v[1] - v[0]).abs();
        let (start, end) = if along_u { (u[0], u[1]) } else { (v[0], v[1]) };
        let mut gaps = Vec::new();
        let mut a = start;
        while a < end - 1e-3 {
            let piece = self.rng.range(0.9, 1.7).min(end - a);
            let (pu, pv) = if along_u { ([a, a + piece], v) } else { (u, [a, a + piece]) };
            let middle = a + piece * 0.5;
            a += piece;
            if door.is_some_and(|d| middle > d[0] && middle < d[1]) {
                continue;
            }
            if self.rng.next() < fall {
                gaps.push(if along_u { (middle, (v[0] + v[1]) * 0.5) } else { ((u[0] + u[1]) * 0.5, middle) });
                continue;
            }
            let (g0, g1) = self.span(at, &[(pu[0], pv[0]), (pu[1], pv[1]), (pu[0], pv[1]), (pu[1], pv[0])]);
            let z = g0 + self.rng.range(height.0, height.1).max(g1 - g0 + 0.4);
            if let Some(m) = cap {
                self.put(at.block(pu, pv, [z, z + 0.1], m, AIR));
            }
            if vines && self.rng.next() < 0.4 {
                let face = self.rng.next().round() as usize;
                let q = if along_u { at.point(middle, v[face], 0.0) } else { at.point(u[face], middle, 0.0) };
                let drop = self.rng.range(1.0, z - g0 - 0.2).max(0.6);
                self.stick([q[0], q[1], z], [q[0], q[1], z - drop], 0.05, VINE);
            }
            self.put(at.block(pu, pv, [g0 - 0.5, z], stone.0, stone.1).patterned(Pattern::Bricks));
        }
        gaps
    }

    /// A round tower of `stone` at `c`, u and v in frame `at`, `r` across and broken off `tall`
    /// over the ground, lower on one side, hollow, a doorway in it facing along u
    fn ruin_tower(&mut self, at: &Frame, c: [f32; 2], r: f32, tall: f32, stone: (u8, u8)) {
        let (g, _) = self.span(at, &[(c[0], c[1]), (c[0] + r, c[1]), (c[0], c[1] + r), (c[0], c[1] - r)]);
        let middle = at.point(c[0], c[1], 0.0);
        let [x, y, _] = middle;
        let top = g + tall;
        // Broken off flat, and lower on one side
        self.put(Shape::room([x - r - 0.2, y - r - 0.2, top], [x + r + 0.2, y + r + 0.2, top + r + 1.0]));
        let notch = self.rng.next() * TAU;
        let n = [x + notch.cos() * r * 0.8, y + notch.sin() * r * 0.8];
        let low = top - self.rng.range(0.9, 1.8);
        self.put(Shape::room([n[0] - r * 0.7, n[1] - r * 0.7, low], [n[0] + r * 0.7, n[1] + r * 0.7, top + 1.0]));
        // Hollow inside, with a doorway
        self.put(Shape::capsule([x, y, g + 0.2], [x, y, top + 1.0], r - 0.45, r - 0.45, AIR, AIR, false));
        self.put(at.block([c[0] + r - 0.8, c[0] + r + 0.3], [c[1] - 0.55, c[1] + 0.55], [g - 0.1, g + 2.1], AIR, AIR));
        self.put(Shape::capsule([x, y, g - 0.6], [x, y, top], r, r, stone.0, stone.1, false).patterned(Pattern::Bricks));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Camps of every kind turn up over a few kilometers, each cheap enough to build, with its
    /// hoard's chest standing clear of everything built for it, out on the ground or in the
    /// hollow of a hideout
    #[test]
    fn camps_of_every_kind_turn_up() {
        let settings = TerrainSettings { width_m: 32, height_m: 64, voxels_per_meter: 4, seed: 1, grass_tufts: false };
        let (terrain, _) = generate_terrain(&settings, [0, 0], &RiverCache::default());
        let height = |x: f32, y: f32| height_at(&settings, x, y);
        let ground = |x: f32, y: f32| Some((height(x, y), false));
        let mut counts = [0; CampKind::ALL.len()];
        let (mut shapes, mut built) = (0, 0);
        let mut wrong = Vec::new();
        for cy in -20..20 {
            for cx in -20..20 {
                let Some(plan) = terrain.plan_camp(cx, cy, &ground, &|_: [f32; 4]| true) else {
                    continue;
                };
                let kind = plan.kind as usize;
                counts[kind] += 1;
                if counts[kind] > 4 {
                    continue;
                }
                let mut features = Features::default();
                let object = terrain.camp(&plan, &height, &mut features);
                let camp = features.camps[0];
                shapes += object.shapes.len();
                built += 1;
                assert!(object.shapes.len() < 900, "a {} of {} shapes", camp.kind.name(), object.shapes.len());
                assert!(features.lights.len() <= 8, "a {} with {} lights", camp.kind.name(), features.lights.len());
                assert!((2..=6).contains(&(camp.bandits + camp.brutes)) && camp.luck > 3.0);
                // Clear all through where the chest goes
                let [x, y, z] = camp.chest;
                let (c, s) = (camp.heading.cos(), camp.heading.sin());
                for (a, b, h) in [(0.0, 0.0, 0.2), (0.2, 0.3, 0.1), (-0.2, 0.3, 0.3), (0.2, -0.3, 0.3), (-0.2, -0.3, 0.1)] {
                    let p = [x + a * c - b * s, y + a * s + b * c, z + h];
                    for (i, shape) in object.shapes.iter().enumerate() {
                        let solid = shape.material != AIR && shape.placement != Placement::Carve;
                        if solid && shape.holds(p, shape.depth(p, 0.0625)) {
                            wrong.push(format!("the chest of a {} at {:?} is in shape {i}, of material {} from {:?} to {:?}", camp.kind.name(), camp.chest, shape.material, shape.min, shape.max));
                        }
                    }
                    if camp.kind == CampKind::Hideout {
                        if !object.shapes.iter().any(|s| s.placement == Placement::Carve && s.holds(p, s.depth(p, 0.0625))) {
                            wrong.push(format!("the chest of a hideout at {:?} isn't in its cave", camp.chest));
                        }
                    } else {
                        // Standing on the ground, or on a rug or a floor built over it
                        let under = [p[0], p[1], z - 0.04];
                        let floor = object.shapes.iter().any(|s| s.material != AIR && s.holds(under, s.depth(under, 0.0625)));
                        let ground = height(p[0], p[1]);
                        if z < ground - 0.25 || z > ground + 0.12 && !floor {
                            wrong.push(format!("the chest of a {} at {:?} isn't on anything, the ground's at {ground}", camp.kind.name(), camp.chest));
                        }
                    }
                }
            }
        }
        // The hill over a hideout's cave isn't cut through, but for the way in up to its mouth
        for cy in -20..20 {
            for cx in -20..20 {
                let Some(plan) = terrain.plan_camp(cx, cy, &ground, &|_: [f32; 4]| true).filter(|p| p.kind == CampKind::Hideout) else {
                    continue;
                };
                let object = terrain.camp(&plan, &height, &mut Features::default());
                let carves: Vec<&Shape> = object.shapes.iter().filter(|s| s.placement == Placement::Carve).collect();
                let side = [-plan.way[1], plan.way[0]];
                let mut open = 0;
                for i in 0..40 {
                    for j in -12..=12 {
                        let (u, v) = (plan.mouth + 1.5 + i as f32 * 0.25, j as f32 * 0.4);
                        let (x, y) = (plan.fire[0] + plan.way[0] * u + side[0] * v, plan.fire[1] + plan.way[1] * u + side[1] * v);
                        let p = [x, y, height(x, y) - 0.1];
                        if carves.iter().any(|s| s.holds(p, s.depth(p, 0.0625))) {
                            open += 1;
                        }
                    }
                }
                if open > 0 {
                    wrong.push(format!("the cave of a hideout at {:?} breaks out of the hill in {open} places", plan.fire));
                }
            }
        }
        let names: Vec<String> = CampKind::ALL.iter().map(|k| format!("{} {}", k.name(), counts[*k as usize])).collect();
        println!("camps: {}; {} shapes each on average", names.join(", "), shapes / built.max(1));
        wrong.dedup();
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
        assert!(counts.iter().all(|&n| n > 0));
    }
}
