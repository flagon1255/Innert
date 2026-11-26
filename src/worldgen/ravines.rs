//! Ravines: long, deep cracks in the ground, wandering along with ledges down their sides, at most
//! one starting in each RAVINE_CELL square. Most are open to the sky and go down as deep as the
//! land does, a fall into one the end of whoever takes it, with a way down at the end they start
//! from, steps cut into the slope no higher than a stride, and a flat floor at the bottom with
//! something there worth climbing down for, see Hold: the bones of what fell in, crystals, glowing
//! fungus and clay, a spiders' nest, a climber's lost cache, and in one in five a demi-boss, see
//! src\demis.rs. The rest are crevices shut in under the ground, crystals glowing at their bottoms.
//!
//! Nothing's built over any of them: whatever's placed asks over_ravine() first, and moves or
//! keeps away if one's in the way, see Terrain::buildings() and camps.rs. Each ravine's course is
//! worked out once, from where it is in the whole landscape, see course().

use std::f32::consts::PI;
use std::sync::{LazyLock, RwLock};

use super::*;

/// The chance a RAVINE_CELL square has a ravine starting in it, and the share of those open to the
/// sky
const RAVINE_CHANCE: f32 = 0.62;
const OPEN_SHARE: f32 = 0.65;
/// How far along a ravine each step of its course goes, in meters
const STEP: f32 = 2.0;
/// The lowest a ravine's floor goes, in meters over the bottom of the world
pub const RAVINE_FLOOR: f32 = 2.0;
/// How steeply the way down into an open ravine goes, in meters down for each meter along: steps
/// just over half a meter high, a meter apart, so the player walks up them without jumping
pub const RAVINE_RAMP: f32 = 0.55;
/// The share of open ravines with a demi-boss down in them, see Hold::Lurker
pub const LURKER_CHANCE: f32 = 0.2;
/// How far round the hollow at the bottom of an open ravine is, across and up, in meters
const CHAMBER: [f32; 2] = [6.0, 4.4];
/// How many courses are kept before they're all forgotten and worked out again
const KEPT: usize = 40_000;

/// What's down at the bottom of an open ravine, worth climbing down for
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hold {
    /// The bones of what fell in, a chest among them, and the dead that fell in still walking
    Bones,
    /// Crystals growing up out of the floor, glowing, and gloomwings roosting among them
    Crystals,
    /// Fungus glowing on the floor, and mounds of clay to dig
    Fungus,
    /// Cave spiders' nest: cocoons, glowing egg sacs, the bones of what they caught, and a chest
    /// one of them had
    Nest,
    /// A climber's lost cache: a chest, a bedroll, their lamp still glowing, and their skull
    Cache,
    /// A demi-boss among the bones of what it's caught and a glint of gold, see src\demis.rs
    Lurker,
}

impl Hold {
    pub const ALL: [Hold; 6] = [Hold::Bones, Hold::Crystals, Hold::Fungus, Hold::Nest, Hold::Cache, Hold::Lurker];

    /// What's down in a ravine for `roll`, 0-1: a demi-boss in LURKER_CHANCE of them, any of the
    /// rest as likely as the others
    fn pick(roll: f32) -> Hold {
        if roll < LURKER_CHANCE {
            return Hold::Lurker;
        }
        let rest = [Hold::Bones, Hold::Crystals, Hold::Fungus, Hold::Nest, Hold::Cache];
        rest[(((roll - LURKER_CHANCE) / (1.0 - LURKER_CHANCE)) * rest.len() as f32) as usize % rest.len()]
    }

    /// The site down there for what keeps it, see src\main.rs: the dead walking among the bones,
    /// gloomwings roosting among the crystals, spiders in their nest, or the demi-boss
    pub fn site(self) -> Option<&'static str> {
        match self {
            Hold::Bones => Some(BONES_SITE),
            Hold::Crystals => Some(ROOST_SITE),
            Hold::Nest => Some(NEST_SITE),
            Hold::Lurker => Some(LURKER_SITE),
            Hold::Fungus | Hold::Cache => None,
        }
    }
}

/// What the sites at the bottom of an open ravine are called, see Hold::site(); "ravine" is at the
/// middle of the hollow at the bottom of every one, facing the way in
pub const RAVINE_SITE: &str = "ravine";
pub const BONES_SITE: &str = "ravine bones";
pub const ROOST_SITE: &str = "ravine roost";
pub const NEST_SITE: &str = "ravine nest";
pub const LURKER_SITE: &str = "chasm lurker";

/// One step along a ravine's course, in meters: where its middle is and which way it goes on, the
/// ground there, how high the top of what's cut out goes, its floor, and how wide it is
#[derive(Clone, Copy, Debug)]
pub(super) struct Step {
    pub x: f32,
    pub y: f32,
    pub along: [f32; 2],
    pub ground: f32,
    pub top: f32,
    pub floor: f32,
    pub width: f32,
}

impl Step {
    /// How far out from its middle what's cut out for it goes across the land
    fn reach(&self) -> f32 {
        STEP.max(self.width * 0.5) + 0.3
    }
}

/// A ravine's course: whether it's open to the sky, what's down in it, its steps, how many of them
/// the way down from the land takes, at which the hollow at its bottom is, and where all of it is,
/// x0, y0, x1, y1 in meters
#[derive(Debug)]
pub(super) struct Course {
    pub open: bool,
    pub hold: Hold,
    pub steps: Vec<Step>,
    pub ramp_end: usize,
    pub chamber: usize,
    pub bounds: [f32; 4],
    seed: u32,
}

impl Course {
    /// What's cut out for it, each x0, y0, x1, y1 in meters: round each step, and the hollow at its
    /// bottom
    pub fn rects(&self) -> Vec<[f32; 4]> {
        let mut rects: Vec<[f32; 4]> = self.steps.iter().map(|s| around([s.x, s.y], [s.reach(); 2])).collect();
        if self.open {
            let c = &self.steps[self.chamber];
            rects.push(around([c.x, c.y], [CHAMBER[0] + 0.6; 2]));
        }
        rects
    }

    /// Whether any of what's cut out for it comes within `margin` meters of rectangle `r`, x0, y0,
    /// x1, y1 in meters: its whole width all along it, not only where its middle runs
    pub fn near(&self, r: [f32; 4], margin: f32) -> bool {
        if !overlaps(self.bounds, r, margin) {
            return false;
        }
        let off = |x: f32, y: f32| ((r[0] - x).max(x - r[2]).max(0.0)).hypot((r[1] - y).max(y - r[3]).max(0.0));
        let chamber = self.open && {
            let c = &self.steps[self.chamber];
            off(c.x, c.y) < CHAMBER[0] + 0.6 + margin
        };
        chamber || self.steps.iter().any(|s| off(s.x, s.y) < s.reach() + margin)
    }

    /// The step at the bottom of the way down from the land, the first as deep as it goes
    pub fn foot(&self) -> &Step {
        &self.steps[self.ramp_end.min(self.steps.len() - 1)]
    }
}

/// The courses worked out, by landscape, its height and RAVINE_CELL square
type Kept = HashMap<(u32, u32, i32, i32), Option<Arc<Course>>>;
static COURSES: LazyLock<RwLock<Kept>> = LazyLock::new(Default::default);

/// The course of the ravine starting in RAVINE_CELL square `cx`, `cy`, if there's one, worked out
/// once
pub(super) fn course(settings: &TerrainSettings, cx: i32, cy: i32) -> Option<Arc<Course>> {
    let key = (settings.seed, settings.height_m, cx, cy);
    if let Some(found) = COURSES.read().ok().and_then(|c| c.get(&key).cloned()) {
        return found;
    }
    let found = trace(settings, cx, cy).map(Arc::new);
    if let Ok(mut kept) = COURSES.write() {
        if kept.len() > KEPT {
            kept.clear();
        }
        kept.insert(key, found.clone());
    }
    found
}

/// The courses of every ravine that could reach into rectangle `r`, x0, y0, x1, y1 in meters
pub(super) fn courses_near(settings: &TerrainSettings, r: [f32; 4]) -> Vec<Arc<Course>> {
    let first = [0, 1].map(|i| ((r[i] - RAVINE_REACH) / RAVINE_CELL).floor() as i32);
    let last = [0, 1].map(|i| ((r[i + 2] + RAVINE_REACH) / RAVINE_CELL).ceil() as i32);
    let mut found = Vec::new();
    for cy in first[1]..last[1] {
        for cx in first[0]..last[0] {
            if let Some(c) = course(settings, cx, cy).filter(|c| overlaps(c.bounds, r, 0.0)) {
                found.push(c);
            }
        }
    }
    found
}

/// Whether any ravine comes within `margin` meters of rectangle `r`, x0, y0, x1, y1 in meters, all
/// across it: for whatever's to be built there to keep clear of. Those that give way to a royal
/// castle or a fallen city count as much as the rest, so it's the same wherever it's asked from.
pub(super) fn over_ravine(settings: &TerrainSettings, r: [f32; 4], margin: f32) -> bool {
    courses_near(settings, grown(r, margin)).iter().any(|c| c.near(r, margin))
}

/// Works out the course of the ravine starting in RAVINE_CELL square `cx`, `cy`, if there's one:
/// wandering on from a spot in the square while the ground's over the sea, deepest along the
/// middle, an open one going down from the land at its first end, see RAVINE_RAMP, and closing in
/// at its far end, a shut one at both
fn trace(settings: &TerrainSettings, cx: i32, cy: i32) -> Option<Course> {
    let seed = settings.seed;
    let sea_level = sea_level_m(settings);
    let r = hash3(cx as u32, cy as u32, seed ^ 0x7a71);
    if unit(r) > RAVINE_CHANCE {
        return None;
    }
    let mut rng = Rng(hash(r));
    let (mut x, mut y) = ((cx as f32 + rng.next()) * RAVINE_CELL, (cy as f32 + rng.next()) * RAVINE_CELL);
    if height_at(settings, x, y) < sea_level + 4.0 {
        return None;
    }
    let open = rng.next() < OPEN_SHARE;
    let mut heading = rng.next() * TAU;
    let width = rng.range(3.0, 5.5);
    let depth = rng.range(22.0, 36.0);
    let shut = rng.range(4.0, 7.0);
    let count = if open { 36 + (rng.next() * 26.0) as usize } else { 15 + (rng.next() * 30.0) as usize };
    let mut path: Vec<([f32; 2], [f32; 2], f32)> = Vec::with_capacity(count);
    for _ in 0..count {
        heading += rng.range(-0.25, 0.25);
        let along = [heading.cos(), heading.sin()];
        x += along[0] * STEP;
        y += along[1] * STEP;
        let ground = height_at(settings, x, y);
        if ground < sea_level + 1.0 {
            break;
        }
        path.push(([x, y], along, ground));
    }
    if path.len() < 8 {
        return None;
    }
    // An open one goes down from the land at its lower end, so the way down doesn't run out over
    // the land falling away before it
    if open && path[path.len() - 1].2 < path[0].2 {
        path.reverse();
        for p in &mut path {
            p.1 = [-p.1[0], -p.1[1]];
        }
    }
    let n = path.len();
    let entry = path[0].2;
    // An open one no deeper than the way down reaches by three fifths of the way along, so there's
    // always a floor at the bottom to come down to, the hollow on it, however short it is
    let reachable = entry - 0.5 - RAVINE_RAMP * STEP * (n as f32 * 0.6);
    let mut ramp_end = 0;
    let steps: Vec<Step> = path
        .iter()
        .enumerate()
        .map(|(i, &([x, y], along, ground))| {
            let t = (i as f32 + 0.5) / n as f32;
            let taper = (PI * t).sin().sqrt();
            let closing = if open && t < 0.5 { 1.0 } else { taper };
            let top = if open { ground + 2.0 } else { ground - shut };
            let deep = (top - 2.0 - depth * closing).max(RAVINE_FLOOR).max(if open { reachable } else { f32::MIN });
            let floor = if open {
                // Down from the land at the first end, a stride at a time
                let ramp = entry - 0.5 - RAVINE_RAMP * i as f32 * STEP;
                if ramp > deep {
                    ramp_end = i + 1;
                }
                ramp.max(deep)
            } else {
                deep
            };
            let width = width * if open && t < 0.5 { taper.max(0.6) } else { taper } + 0.6;
            Step { x, y, along, ground, top, floor, width }
        })
        .collect();
    let ramp_end = ramp_end.min(n - 1);
    let mut steps = steps;
    // Where the land rises and falls along the way down, no step's higher than the way down goes
    // either, up or down: the floor's lifted where it'd drop or rise more steeply
    let most = RAVINE_RAMP * STEP;
    for i in 1..=ramp_end {
        steps[i].floor = steps[i].floor.max(steps[i - 1].floor - most);
    }
    for i in (0..ramp_end).rev() {
        steps[i].floor = steps[i].floor.max(steps[i + 1].floor - most);
    }
    // Where the land falls away below the floor past the way down, nothing's cut, so the ravine ends
    // where it last was
    if open {
        while steps.len() > ramp_end + 1 && steps.last().is_some_and(|s| s.floor > s.ground - 1.0) {
            steps.pop();
        }
    }
    let n = steps.len();
    if n < 8 {
        return None;
    }
    let ramp_end = ramp_end.min(n - 1);
    // The hollow at the bottom past the foot of the way down, where it's deepest
    let cut = |i: usize| steps[i].ground - steps[i].floor;
    let chamber = ((ramp_end + 2).min(n - 3)..=n - 3).max_by(|&a, &b| cut(a).total_cmp(&cut(b))).unwrap_or(n - 3);
    // Not worth calling a ravine if it's not cut deep where it's deepest
    if open && cut(chamber) < 8.0 {
        return None;
    }
    let hold = if open { Hold::pick(unit(hash(r ^ 0x401d))) } else { Hold::Crystals };
    let mut course = Course { open, hold, steps, ramp_end, chamber, bounds: [f32::MAX, f32::MAX, f32::MIN, f32::MIN], seed: hash(r ^ 0x5eed) };
    for rect in course.rects() {
        course.bounds = [course.bounds[0].min(rect[0]), course.bounds[1].min(rect[1]), course.bounds[2].max(rect[2]), course.bounds[3].max(rect[3])];
    }
    Some(course)
}

/// An open ravine, for trying them out, in meters: a spot on its rim beside the hollow at its
/// bottom and which way is across into it from there, the middle of the hollow's floor and which
/// way the ravine runs on there, the top of the way down from the land and its foot, and what's
/// down there
#[derive(Clone, Copy, Debug)]
pub struct Chasm {
    pub rim: [f32; 3],
    pub across: [f32; 2],
    pub floor: [f32; 3],
    pub along: [f32; 2],
    pub top: [f32; 3],
    pub foot: [f32; 3],
    pub hold: Hold,
}

impl Course {
    fn chasm(&self, settings: &TerrainSettings) -> Chasm {
        let c = &self.steps[self.chamber];
        let side = [-c.along[1], c.along[0]];
        let out = CHAMBER[0] + 2.5;
        let (rx, ry) = (c.x + side[0] * out, c.y + side[1] * out);
        let (top, foot) = (&self.steps[0], self.foot());
        Chasm {
            rim: [rx, ry, height_at(settings, rx, ry)],
            across: [-side[0], -side[1]],
            floor: [c.x, c.y, c.floor],
            along: c.along,
            top: [top.x - top.along[0] * STEP, top.y - top.along[1] * STEP, top.ground],
            foot: [foot.x, foot.y, foot.floor],
            hold: self.hold,
        }
    }
}

/// The open ravine nearest `near`, in meters, within a kilometer or so, with `hold` down in it if
/// that says, for trying them out
pub fn nearest_chasm(settings: &TerrainSettings, near: [f32; 2], hold: Option<Hold>) -> Option<Chasm> {
    let home = near.map(|v| (v / RAVINE_CELL).floor() as i32);
    let mut found: Vec<(f32, Arc<Course>)> = Vec::new();
    for cy in home[1] - 12..=home[1] + 12 {
        for cx in home[0] - 12..=home[0] + 12 {
            if let Some(c) = course(settings, cx, cy).filter(|c| c.open && hold.is_none_or(|h| h == c.hold)) {
                let f = &c.steps[c.chamber];
                found.push(((f.x - near[0]).hypot(f.y - near[1]), c));
            }
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    // The nearest that's cut out of the ground, not giving way to a royal castle, a fallen city or
    // a dragon's lair, see Terrain::ravines()
    let cache = RiverCache::default();
    found.into_iter().map(|(_, c)| c).find(|c| {
        let terrain = Terrain::around(settings, [c.bounds[0], c.bounds[1]], [c.bounds[2], c.bounds[3]]);
        let shut = terrain.castles_near(grown(c.bounds, 3.0), &cache).into_iter().chain(terrain.cities_near(grown(c.bounds, 3.0), &cache));
        let lairs: Vec<[f32; 5]> = terrain.dragon_footprints();
        !shut.into_iter().any(|r| overlaps(c.bounds, r, 3.0)) && !lairs.iter().any(|l| c.near([l[0], l[1], l[2], l[3]], 3.0))
    }).map(|c| c.chasm(settings))
}

/// Something built near the edge of an open ravine, for seeing that it stands on solid ground: the
/// nearest to `near` that comes within `within` meters of one, in pieces of the landscape placed one
/// after another round there: what it is, where its middle is and the ravine, in meters
pub fn built_by_chasm(settings: &TerrainSettings, near: [f32; 2], within: f32, rivers: &RiverCache) -> Option<(&'static str, [f32; 3], Chasm)> {
    const BUILT: [&str; 10] = ["castle", "watchtower", "shrine", "pyramid", "colonnade", "flooded church", "desert ruins", "bandit camp", "town", "dungeon"];
    let rough = TerrainSettings { voxels_per_meter: 2, grass_tufts: false, ..*settings };
    let size = 256.0;
    for ring in 0..6i32 {
        for j in -ring..=ring {
            for i in -ring..=ring {
                if i.abs().max(j.abs()) != ring {
                    continue;
                }
                let corner = [near[0] - size * 0.5 + i as f32 * size, near[1] - size * 0.5 + j as f32 * size];
                let vpm = rough.voxels_per_meter;
                let mut heights = Heights::new(&rough, corner.map(|v| (v * vpm as f32) as i64), [(size as u32 * vpm) as usize; 2], vpm);
                let mut terrain = Terrain::new(&rough, &heights);
                terrain.carve_rivers(&mut heights, rivers);
                terrain.place(&heights, rivers);
                let found = terrain.sites().iter().filter(|s| BUILT.contains(&s.kind)).find_map(|s| {
                    let r = [s.position[0] - 6.0, s.position[1] - 6.0, s.position[0] + 6.0, s.position[1] + 6.0];
                    let c = terrain.features.ravines.iter().find(|c| c.open && c.near(r, within))?;
                    Some((s.kind, s.position, c.chasm(settings)))
                });
                if found.is_some() {
                    return found;
                }
            }
        }
    }
    None
}

impl Terrain {
    /// The ravines reaching into this piece. None breaks into a royal castle or a fallen city,
    /// `shut`, see castles.rs and undercity.rs, nor into a dragon's lair, see dragonlair.rs: it
    /// gives way to them.
    pub(super) fn ravines(&self, features: &mut Features, shut: &[[f32; 4]]) -> Vec<Object> {
        let piece = [self.lo[0], self.lo[1], self.hi[0], self.hi[1]];
        let lairs: Vec<[f32; 4]> = self.dragon_footprints().iter().map(|f| [f[0], f[1], f[2], f[3]]).collect();
        let mut objects = Vec::new();
        for course in courses_near(&self.settings, piece) {
            let into = |r: &[f32; 4]| overlaps(course.bounds, *r, 3.0);
            if shut.iter().any(into) || lairs.iter().any(|l| course.near(*l, 3.0)) {
                continue;
            }
            objects.push(self.ravine(&course, features));
            features.ravines.push(course);
        }
        objects
    }

    /// What's cut out of the ground for the ravine of `course`, and what's down at its bottom
    fn ravine(&self, course: &Course, features: &mut Features) -> Object {
        let mut shapes = Vec::new();
        let mut inside = Vec::new();
        let steps = &course.steps;
        // A wide upper part and a narrower deeper one, leaving ledges down the sides, over a flat
        // floor. Each reaches its floor a meter and a half either side of its middle, so on the way
        // down from the land there's a slab between each two, and no step's higher than a stride.
        let cut = |shapes: &mut Vec<Shape>, at: [f32; 2], along: [f32; 2], top: f32, floor: f32, w: f32| {
            let h = top - floor;
            if h < 0.5 {
                return;
            }
            let along = [along[0], along[1], 0.0];
            shapes.push(Shape::slab([at[0], at[1], floor + 0.675 * h], along, STEP * 2.0, w, 0.75 * h).floored(floor));
            shapes.push(Shape::slab([at[0], at[1], floor + 0.35 * h], along, STEP * 2.0, w * 0.7, 1.2 * h).floored(floor));
        };
        for (i, s) in steps.iter().enumerate() {
            cut(&mut shapes, [s.x, s.y], s.along, s.top, s.floor, s.width);
            if course.open && i < course.ramp_end && i + 1 < steps.len() {
                let next = &steps[i + 1];
                let mid = |a: f32, b: f32| (a + b) * 0.5;
                cut(&mut shapes, [mid(s.x, next.x), mid(s.y, next.y)], s.along, mid(s.top, next.top), mid(s.floor, next.floor), mid(s.width, next.width));
            }
            features.recuts.push([s.x - s.width - STEP, s.y - s.width - STEP, s.x + s.width + STEP, s.y + s.width + STEP, s.ground + 3.0]);
        }
        let mut rng = Rng(course.seed);
        if !course.open {
            // Crystals glowing at the bottom of a crevice shut in under the ground
            for s in steps.iter().skip(3).step_by(6) {
                let at = [s.x + rng.range(-0.3, 0.3), s.y + rng.range(-0.3, 0.3), s.floor + 0.1];
                crystals(&mut inside, &mut features.lights, at, 3, 0.3, &mut rng);
            }
            let middle = &steps[steps.len() / 2];
            features.sites.push(Site { kind: "crevice", position: [middle.x, middle.y, middle.floor], facing: middle.along });
            shapes.extend(inside);
            return Object::new(shapes, rng.0);
        }

        // The hollow at the bottom, widened out of the walls, where what's down there is
        let c = steps[course.chamber];
        let room = Shape::ellipsoid([c.x, c.y, c.floor + CHAMBER[1] * 0.5], [CHAMBER[0], CHAMBER[0], CHAMBER[1]], AIR, AIR).placed(Placement::Carve);
        shapes.push(room.roughened(0.45, c.floor));
        let back = [-c.along[0], -c.along[1]];
        features.sites.push(Site { kind: RAVINE_SITE, position: [c.x, c.y, c.floor], facing: back });
        if let Some(kind) = course.hold.site() {
            features.sites.push(Site { kind, position: [c.x, c.y, c.floor], facing: back });
        }
        // Crystals glowing faintly here and there along the bottom, so it's never wholly dark
        for s in steps.iter().skip(course.ramp_end.saturating_sub(4)).step_by(7) {
            let off = rng.range(-0.3, 0.3) * s.width * 0.6;
            crystals(&mut inside, &mut features.lights, [s.x - s.along[1] * off, s.y + s.along[0] * off, s.floor], 2, 0.2, &mut rng);
        }
        // Spots on the floor: in the hollow, and along the bottom from the foot of the way down
        let in_room = |rng: &mut Rng, out: f32| {
            let (a, r) = (rng.range(0.0, TAU), rng.range(0.5, out));
            [c.x + a.cos() * r, c.y + a.sin() * r, c.floor]
        };
        let along_bottom = |rng: &mut Rng| {
            let k = course.ramp_end + (rng.next() * (steps.len() - 2 - course.ramp_end.min(steps.len() - 3)) as f32) as usize;
            let s = steps[k.min(steps.len() - 2)];
            let off = rng.range(-0.25, 0.25) * s.width * 0.7;
            [s.x - s.along[1] * off, s.y + s.along[0] * off, s.floor]
        };
        match course.hold {
            Hold::Bones => {
                for _ in 0..7 {
                    let at = if rng.next() < 0.6 { in_room(&mut rng, 4.5) } else { along_bottom(&mut rng) };
                    bones(&mut inside, at, &mut rng);
                }
                ribs(&mut inside, in_room(&mut rng, 3.0), &mut rng);
                for _ in 0..3 {
                    let at = in_room(&mut rng, 4.5);
                    features.props.push(Prop { kind: PropKind::Skull, position: at, heading: rng.next() * TAU, luck: 0.0 });
                }
                let at = in_room(&mut rng, 3.5);
                chest(features, at, rng.next() * TAU, 1.8);
                crystals(&mut inside, &mut features.lights, in_room(&mut rng, 4.0), 3, 0.25, &mut rng);
            }
            Hold::Crystals => {
                for k in 0..6 {
                    let at = if k < 4 { in_room(&mut rng, 4.8) } else { along_bottom(&mut rng) };
                    crystals(&mut inside, &mut features.lights, at, 4 + (rng.next() * 3.0) as usize, 0.45, &mut rng);
                }
            }
            Hold::Fungus => {
                for k in 0..10 {
                    let at = if k < 6 { in_room(&mut rng, 4.8) } else { along_bottom(&mut rng) };
                    mushroom(&mut inside, &mut features.lights, at, k % 3 == 0, &mut rng);
                }
                for _ in 0..3 {
                    let at = in_room(&mut rng, 4.5);
                    let r = [rng.range(0.7, 1.2), rng.range(0.7, 1.2), rng.range(0.4, 0.7)];
                    inside.push(Shape::ellipsoid([at[0], at[1], at[2]], r, CLAY, CLAY_BANK).placed(Placement::Inside));
                }
            }
            Hold::Nest => {
                for _ in 0..5 {
                    let at = in_room(&mut rng, 4.6);
                    let lying = rng.range(0.0, TAU);
                    let (dx, dy) = (lying.cos() * 0.4, lying.sin() * 0.4);
                    inside.push(Shape::capsule([at[0] - dx, at[1] - dy, at[2] + 0.35], [at[0] + dx, at[1] + dy, at[2] + 0.4], 0.38, 0.3, LINEN, FUR_CREAM, false).placed(Placement::Inside));
                }
                for _ in 0..4 {
                    let at = in_room(&mut rng, 4.0);
                    for _ in 0..3 {
                        let p = [at[0] + rng.range(-0.3, 0.3), at[1] + rng.range(-0.3, 0.3), at[2] + rng.range(0.15, 0.3)];
                        inside.push(Shape::sphere(p, rng.range(0.14, 0.22), GLOW_POISON, AIR, false).placed(Placement::Inside));
                    }
                    features.lights.push(Light { position: [at[0], at[1], at[2] + 0.6], color: [0.45, 1.0, 0.35], reach: 6.0 });
                }
                for _ in 0..4 {
                    bones(&mut inside, along_bottom(&mut rng), &mut rng);
                }
                let at = in_room(&mut rng, 4.0);
                chest(features, at, rng.next() * TAU, 1.5);
            }
            Hold::Cache => {
                // Under the wall at the back of the hollow, out of the way of what falls
                let a = rng.range(0.0, TAU);
                let at = |r: f32, turn: f32| [c.x + (a + turn).cos() * r, c.y + (a + turn).sin() * r, c.floor];
                let bed = at(3.6, 0.0);
                inside.push(Shape::ellipsoid([bed[0], bed[1], bed[2] + 0.08], [0.9, 0.9, 0.12], LINEN, LEATHER).placed(Placement::Inside));
                let lamp = at(3.9, 0.45);
                inside.push(Shape::capsule([lamp[0], lamp[1], lamp[2]], [lamp[0], lamp[1], lamp[2] + 0.35], 0.12, 0.1, DARK_STEEL, AIR, false).placed(Placement::Inside));
                inside.push(Shape::sphere([lamp[0], lamp[1], lamp[2] + 0.22], 0.1, GLOW_HOLY, AIR, false).placed(Placement::Inside));
                features.lights.push(Light { position: [lamp[0], lamp[1], lamp[2] + 0.6], color: [1.0, 0.8, 0.5], reach: 8.0 });
                // Their pack and rope, coiled, and what's left of them
                let pack = at(3.4, -0.4);
                inside.push(Shape::ellipsoid([pack[0], pack[1], pack[2] + 0.3], [0.35, 0.3, 0.32], LEATHER, CLOTH).placed(Placement::Inside));
                let coil = at(2.8, 0.5);
                for k in 0..3 {
                    inside.push(Shape::ellipsoid([coil[0], coil[1], coil[2] + 0.06 + k as f32 * 0.09], [0.4 - k as f32 * 0.05, 0.4 - k as f32 * 0.05, 0.05], LINEN, AIR).placed(Placement::Inside));
                }
                bones(&mut inside, at(3.0, -0.9), &mut rng);
                features.props.push(Prop { kind: PropKind::Skull, position: at(3.2, -0.75), heading: rng.next() * TAU, luck: 0.0 });
                chest(features, at(3.3, 0.25), a + PI, 2.6);
                for _ in 0..2 {
                    crystals(&mut inside, &mut features.lights, along_bottom(&mut rng), 3, 0.3, &mut rng);
                }
            }
            Hold::Lurker => {
                for _ in 0..10 {
                    let at = if rng.next() < 0.7 { in_room(&mut rng, 5.0) } else { along_bottom(&mut rng) };
                    bones(&mut inside, at, &mut rng);
                }
                ribs(&mut inside, in_room(&mut rng, 4.0), &mut rng);
                for _ in 0..4 {
                    let at = in_room(&mut rng, 4.8);
                    features.props.push(Prop { kind: PropKind::Skull, position: at, heading: rng.next() * TAU, luck: 0.0 });
                }
                // A glint of gold among it, of those it caught
                let hoard = in_room(&mut rng, 3.5);
                for _ in 0..6 {
                    let p = [hoard[0] + rng.range(-0.5, 0.5), hoard[1] + rng.range(-0.5, 0.5), hoard[2] + rng.range(0.0, 0.15)];
                    inside.push(Shape::sphere(p, rng.range(0.1, 0.18), GOLD, AIR, false).placed(Placement::Inside));
                }
                features.lights.push(Light { position: [c.x, c.y, c.floor + 3.0], color: [1.0, 0.35, 0.3], reach: 14.0 });
                for _ in 0..2 {
                    crystals(&mut inside, &mut features.lights, in_room(&mut rng, 5.0), 3, 0.3, &mut rng);
                }
            }
        }
        shapes.extend(inside);
        Object::new(shapes, rng.0)
    }
}

/// A cluster of `count` crystals jutting up out of the floor at `at`, up to `size` meters across,
/// and its light
fn crystals(inside: &mut Vec<Shape>, lights: &mut Vec<Light>, at: Vec3, count: usize, size: f32, rng: &mut Rng) {
    for _ in 0..count {
        let foot = [at[0] + rng.range(-0.35, 0.35), at[1] + rng.range(-0.35, 0.35), at[2] - 0.1];
        let tall = rng.range(0.4, 1.0) * size * 3.0;
        let tip = [foot[0] + rng.range(-0.3, 0.3) * tall, foot[1] + rng.range(-0.3, 0.3) * tall, foot[2] + tall];
        inside.push(Shape::capsule(foot, tip, rng.range(0.5, 1.0) * size, 0.04, CRYSTAL, AIR, false).placed(Placement::Inside));
    }
    lights.push(Light { position: [at[0], at[1], at[2] + 0.6], color: [0.35, 0.85, 1.0], reach: 7.0 });
}

/// A few bones lying on the floor at `at`
fn bones(inside: &mut Vec<Shape>, at: Vec3, rng: &mut Rng) {
    for _ in 0..2 + (rng.next() * 2.0) as usize {
        let (a, r) = (rng.range(0.0, TAU), rng.range(0.0, 0.5));
        let (x, y) = (at[0] + a.cos() * r, at[1] + a.sin() * r);
        let lying = rng.range(0.0, TAU);
        let half = rng.range(0.2, 0.45);
        let (dx, dy) = (lying.cos() * half, lying.sin() * half);
        inside.push(Shape::capsule([x - dx, y - dy, at[2] + 0.05], [x + dx, y + dy, at[2] + rng.range(0.03, 0.12)], 0.06, 0.05, BONE, AIR, false).placed(Placement::Inside));
    }
}

/// The ribs of something big that fell in, arching up out of the floor at `at` either side of its
/// spine
fn ribs(inside: &mut Vec<Shape>, at: Vec3, rng: &mut Rng) {
    let a = rng.range(0.0, TAU);
    let (along, across) = ([a.cos(), a.sin()], [-a.sin(), a.cos()]);
    let p = |u: f32, v: f32, z: f32| [at[0] + along[0] * u + across[0] * v, at[1] + along[1] * u + across[1] * v, at[2] + z];
    inside.push(Shape::capsule(p(-1.4, 0.0, 0.1), p(1.4, 0.0, 0.15), 0.12, 0.1, BONE, AIR, false).placed(Placement::Inside));
    for k in 0..5 {
        let u = -1.0 + k as f32 * 0.5;
        for side in [-1.0f32, 1.0] {
            let tall = 1.0 - (u * 0.4).abs();
            inside.push(Shape::capsule(p(u, 0.0, 0.15), p(u, side * 0.6, 0.6 * tall), 0.07, 0.06, BONE, AIR, false).placed(Placement::Inside));
            inside.push(Shape::capsule(p(u, side * 0.6, 0.6 * tall), p(u + 0.1, side * 0.75, 1.1 * tall), 0.06, 0.05, BONE, AIR, false).placed(Placement::Inside));
        }
    }
}

/// A mushroom growing up out of the floor at `at`, its cap glowing, a big one if `big`, and its
/// light
fn mushroom(inside: &mut Vec<Shape>, lights: &mut Vec<Light>, at: Vec3, big: bool, rng: &mut Rng) {
    let tall = if big { rng.range(1.2, 2.2) } else { rng.range(0.3, 0.8) };
    let lean = [rng.range(-0.15, 0.15) * tall, rng.range(-0.15, 0.15) * tall];
    let head = [at[0] + lean[0], at[1] + lean[1], at[2] + tall];
    inside.push(Shape::capsule([at[0], at[1], at[2] - 0.1], head, tall * 0.1 + 0.04, tall * 0.07 + 0.03, MUSHROOM_STEM, AIR, false).placed(Placement::Inside));
    let (glow, color) = if rng.next() < 0.5 { (GLOW_POISON, [0.45, 1.0, 0.4]) } else { (GLOW_ARCANE, [0.7, 0.45, 1.0]) };
    let r = tall * 0.45 + 0.1;
    inside.push(Shape::ellipsoid(head, [r, r, r * 0.45], glow, MUSHROOM_CAP).placed(Placement::Inside));
    if big || rng.next() < 0.3 {
        lights.push(Light { position: [head[0], head[1], head[2] + 0.3], color, reach: if big { 7.0 } else { 4.0 } });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The piece of the landscape `size` meters across from `corner`, with everything placed in it
    /// but not built into voxels
    fn placed(settings: &TerrainSettings, corner: [f32; 2], size: u32, rivers: &RiverCache) -> Terrain {
        let vpm = settings.voxels_per_meter;
        let origin = corner.map(|v| (v * vpm as f32) as i64);
        let mut heights = Heights::new(settings, origin, [(size * vpm) as usize; 2], vpm);
        let mut terrain = Terrain::new(settings, &heights);
        terrain.carve_rivers(&mut heights, rivers);
        terrain.place(&heights, rivers);
        terrain
    }

    /// The water a ravine starts with, cut below the sea and below rivers: none down in it but the
    /// sea's, where the sea reaches it, never a column of a river's water standing in the air
    #[test]
    fn ravines_start_dry_or_at_the_sea() {
        let settings = TerrainSettings { width_m: 64, height_m: 64, voxels_per_meter: 4, seed: 1, grass_tufts: false };
        let vpm = settings.voxels_per_meter as f32;
        let sea = ((sea_level_m(&settings) * vpm) as i32 + 1) as f32;
        let rivers = RiverCache::default();
        let mut checked = 0;
        for near in [[0.0, 0.0], [600.0, 200.0], [-400.0, 700.0], [900.0, -800.0]] {
            let Some(c) = nearest_chasm(&settings, near, None) else {
                continue;
            };
            let corner = [((c.floor[0] - 32.0) * vpm) as i64, ((c.floor[1] - 32.0) * vpm) as i64];
            let (_, world) = generate_area(&settings, corner, [256, 256], &rivers);
            let water = world.water.as_ref().expect("its water");
            for (i, (&g, &s)) in water.ground.iter().zip(&water.surface).enumerate() {
                // Down in it, under the sea
                if g < sea - 2.0 * vpm {
                    checked += 1;
                    assert!(s == 0.0 || (s - sea).abs() < 0.5, "water at {s} on the floor at {g} in column {i}, the sea at {sea}");
                }
            }
        }
        assert!(checked > 1000, "{checked} columns down in ravines");
    }

    /// Nothing's built over a ravine: no part of any building, ruin, camp, town, castle, dungeon,
    /// fallen city's way down or dragon's lair, nor wing shrine, comes over one, across the whole of
    /// what it covers, in several landscapes; some stand near the edge of one all the same
    #[test]
    fn nothing_is_built_over_a_ravine() {
        let (mut checked, mut cut, mut near) = (0, 0, 0);
        let mut wrong = Vec::new();
        for seed in [1, 2, 77, 4021] {
            let settings = TerrainSettings { width_m: 64, height_m: 64, voxels_per_meter: 2, seed, grass_tufts: false };
            let rivers = RiverCache::default();
            for corner in [[0.0, 0.0], [700.0, -300.0], [-500.0, 900.0]] {
                let terrain = placed(&settings, corner, 256, &rivers);
                let courses = &terrain.features.ravines;
                cut += courses.len();
                let mut covers: Vec<(String, [f32; 4])> = terrain.features.footprints.iter().map(|f| (format!("what's built from {:?} to {:?}", [f[0], f[1]], [f[2], f[3]]), [f[0], f[1], f[2], f[3]])).collect();
                for s in terrain.sites().iter().filter(|s| s.kind == SHRINE) {
                    covers.push((format!("a wing shrine at {:?}", s.position), [s.position[0] - 4.0, s.position[1] - 4.0, s.position[0] + 4.0, s.position[1] + 4.0]));
                }
                for (what, rect) in covers {
                    checked += 1;
                    if courses.iter().any(|c| c.near(rect, 12.0)) {
                        near += 1;
                    }
                    // All across it, corners, edges and middle, against everything cut out for each
                    if let Some(c) = courses.iter().find(|c| c.near(rect, 0.0)) {
                        wrong.push(format!("seed {seed}: {what} is over the ravine from {:?}", [c.steps[0].x, c.steps[0].y]));
                    }
                }
            }
        }
        println!("{checked} things built, {near} of them near the {cut} ravines");
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
        assert!(checked > 100 && cut > 10 && near > 0, "{checked} built, {cut} ravines, {near} near one");
    }

    /// Open ravines go down as deep as the land lets them, deeper than they did, with a way down at
    /// their first end no step of which is higher than the player steps up, and a hollow at the
    /// bottom; each sort of what's down there turns up, a demi-boss in about one in five
    #[test]
    fn ravines_are_deep_with_a_way_down() {
        let mut holds = [0usize; Hold::ALL.len()];
        let (mut open, mut deep, mut past) = (0, 0.0f32, 0);
        for seed in [1, 2, 77, 4021] {
            let settings = TerrainSettings { width_m: 64, height_m: 64, voxels_per_meter: 4, seed, grass_tufts: false };
            for cy in -12..12 {
                for cx in -12..12 {
                    let Some(c) = course(&settings, cx, cy) else {
                        continue;
                    };
                    assert!(c.steps.iter().all(|s| s.floor >= RAVINE_FLOOR - 1e-3), "seed {seed}: a floor under the world");
                    // Cut into the ground past the way down, which goes on over the land where it
                    // falls away more steeply than the way down
                    let hollow = &c.steps[c.chamber];
                    let last = c.steps.last().unwrap();
                    assert!(hollow.floor < hollow.ground - 3.0 && last.floor < last.ground, "seed {seed}: not cut into the ground: the hollow {hollow:?}, the last {last:?}, way down to {} of {}", c.ramp_end, c.steps.len());
                    if !c.open {
                        continue;
                    }
                    open += 1;
                    holds[Hold::ALL.iter().position(|&h| h == c.hold).unwrap()] += 1;
                    // Walked down from the land: half-steps no higher than a stride
                    let first = &c.steps[0];
                    assert!(first.ground - first.floor < 1.0, "seed {seed}: a drop of {} m at the top of the way down", first.ground - first.floor);
                    for pair in c.steps[..=c.ramp_end.min(c.steps.len() - 2)].windows(2) {
                        // Half of it, as there's a slab between each two
                        let rise = (pair[0].floor - pair[1].floor).abs() * 0.5;
                        assert!(rise <= RAVINE_RAMP + 1e-3, "seed {seed}: a step of {rise} m on the way down");
                    }
                    assert!(c.chamber < c.steps.len() && c.foot().floor <= c.steps[0].floor);
                    if c.chamber > c.ramp_end {
                        past += 1;
                    }
                    deep += c.steps[c.chamber].ground - c.steps[c.chamber].floor;
                }
            }
        }
        let mean = deep / open as f32;
        println!("{open} open ravines, {mean:.1} m deep on average where it's deepest; what's down in them: {holds:?}");
        assert!(open > 100 && mean > 20.0, "{open} open, {mean} m deep");
        // Nearly always the hollow's on the floor past the foot of the way down
        assert!(past * 10 >= open * 8, "only {past} of {open} have their hollow past the way down");
        assert!(holds.iter().all(|&n| n > 0));
        let lurkers = holds[5] as f32 / open as f32;
        assert!((0.12..0.3).contains(&lurkers), "{lurkers} of them have a demi-boss");
    }
}
