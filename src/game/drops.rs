//! What's dropped on the ground, by foes, bosses and chests or by the player: each shown as
//! itself, a weapon as that very weapon of its own parts and materials, wings as that pair, a jewel
//! as that jewel, turning slowly so it reads; and found by a beam of light up from where it lies in
//! its rarity's colour, taller the rarer, a legendary's and a mythic's towering over the trees with
//! a glow on the ground round them. A beam's seen from far off, over tall grass, out of water and
//! up out of a hole, as long as its foot, its middle or its top is in sight; it glows by night and
//! stands out by day, and fades away as the player comes up to it, so it never washes over the
//! view. What's near is marked on the compass too.
//!
//! Nothing dropped sinks into the ground: buried, as when it landed on a slope, it rises to the
//! top; left hanging as what it lay on is dug away, it falls till something holds it; dropped in
//! deep water it floats up, and bobs there.

use super::*;
use crate::jewels;

/// Voxels a meter of the bucket's model: finer than the world's, as it's small
const BUCKET_RES: f64 = 3.0 * animals::MODEL_RES;
/// Where the bucket's model is among Game::drop_models, after every kind of jewel's
const BUCKET_MODEL: usize = jewels::KINDS;
/// What reaches no further than this from where it lies, in meters, is drawn a little over life
/// size, GROW times, so it reads among the grass
const SMALL: f64 = 0.3;
const GROW: f64 = 1.2;
/// Only what's dropped within this many meters of the camera is kept on what it lies on, as the
/// world further off may not be all there yet
const SETTLED_WITHIN: f64 = 120.0;
/// The most marked on the compass at once
const MARKS: usize = 12;

/// How something lying on the ground is drawn
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Shape {
    /// Its model among the animals', like food, gold, the hook or an Innert block
    Look(Look),
    /// The weapon's own model, of its parts and materials, lying on its side
    Weapon(usize),
    /// A few arrows or bolts side by side, or a little pile of shot, of Library::ammo
    Ammo(usize),
    /// A pair of wings of that kind, spread, each its kind's own model, see wings::lying()
    Wings(u8),
    /// One of Game::drop_models: a jewel of that kind, or the bucket
    Own(usize),
}

impl Item {
    /// How it's drawn lying on the ground: every kind of thing as itself
    pub(super) fn shape(self) -> Shape {
        match self {
            Item::Weapon(w) => Shape::Weapon(w),
            Item::Ammo(k, _) => Shape::Ammo(k),
            Item::Wings(k) => Shape::Wings(k),
            Item::Jewel(j) => Shape::Own(j as usize % jewels::KINDS),
            Item::Bucket => Shape::Own(BUCKET_MODEL),
            _ => Shape::Look(self.look().expect("everything else lies about as a model among the animals'")),
        }
    }
}

/// The models of what's dropped that has none elsewhere, see Game::drop_models: every kind of
/// jewel, then the bucket
pub(super) fn models() -> Vec<(Model, Vector3<f64>)> {
    let root = crate::assets::root();
    let mut out: Vec<(Model, Vector3<f64>)> = (0..jewels::KINDS as u8).map(|j| jewels::lying_model(&root, j)).collect();
    out.push(animals::voxelize(&bucket(), BUCKET_RES));
    out
}

/// A wooden bucket bound with two iron hoops, its handle arched over it, from the middle of its
/// bottom
fn bucket() -> Vec<animals::Part> {
    use animals::{ball, limb};
    let mut p = vec![
        // Hollow, taken first so nothing after fills it
        limb([0.0, 0.0, 0.03], [0.0, 0.0, 0.34], 0.105, 0.135, AIR),
        limb([0.0, 0.0, 0.035], [0.0, 0.0, 0.06], 0.136, 0.14, DARK_STEEL),
        limb([0.0, 0.0, 0.2], [0.0, 0.0, 0.225], 0.151, 0.155, DARK_STEEL),
        // Its staves, flaring out to the rim
        limb([0.0, 0.0, 0.0], [0.0, 0.0, 0.27], 0.125, 0.15, WOOD),
        ball([0.152, 0.0, 0.25], 0.018, DARK_STEEL),
        ball([-0.152, 0.0, 0.25], 0.018, DARK_STEEL),
    ];
    let arc = |k: usize| {
        let a = k as f64 / 6.0 * std::f64::consts::PI;
        [0.152 * a.cos(), 0.0, 0.25 + 0.13 * a.sin()]
    };
    p.extend((0..6).map(|k| limb(arc(k), arc(k + 1), 0.009, 0.009, DARK_STEEL)));
    p
}

/// Whether nothing solid stands between `eye` and `to`, in voxels: looked along a quarter of a
/// meter at a time near, a little coarser far off
fn in_sight(world: &World, eye: Vector3<f64>, to: Vector3<f64>, vpm: f64) -> bool {
    let along = to - eye;
    let length = along.norm();
    let steps = (length / (0.25 * vpm).max(length / 240.0)) as usize;
    (1..steps).all(|s| {
        let p = eye + along * (s as f64 / steps as f64);
        is_passable(world.material_at(p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64))
    })
}

/// Keeps `pickup` on what it lies on, see the module's doc, `water` the top of the water at a
/// point and `dt` seconds on
fn settle(world: &World, water: WaterLevel, pickup: &mut Pickup, vpm: f64, dt: f64) {
    let p = &mut pickup.position;
    let (x, y) = (p.x.floor() as i64, p.y.floor() as i64);
    let at = |z: f64| world.material_at(x, y, z.floor() as i64);
    // Buried: up to the top of what it's in
    if !is_passable(at(p.z + 0.25)) {
        if let Some(k) = (1..=(3.0 * vpm) as i64).find(|&k| is_passable(at(p.z + 0.25 + k as f64))) {
            p.z = (p.z + 0.25 + k as f64).floor();
        }
        pickup.afloat = false;
        return;
    }
    let surface = water(p.x, p.y);
    // Afloat, it rides the water as it rises and falls, till it drains away
    if pickup.afloat && surface > p.z - 0.3 * vpm && surface < p.z + 0.3 * vpm {
        p.z = surface;
        return;
    }
    pickup.afloat = false;
    // Under the water, it floats up through it, unless there's rock over it, as in a cave under a
    // lake
    if surface > p.z + 0.05 * vpm {
        let up = (p.z + DROP_RISE * vpm * dt).min(surface);
        if is_passable(at(up + 0.25)) {
            p.z = up;
            pickup.afloat = surface - up < 0.05 * vpm;
            return;
        }
    }
    // Nothing under it: it falls, as far as it would this frame, till something holds it
    let holds = |m: u8| !is_passable(m) || is_leaves(m);
    if holds(at(p.z - 0.5)) {
        return;
    }
    let to = (p.z - DROP_FALL * vpm * dt).max(1.0);
    let lowest = (to - 0.5).floor() as i64;
    let mut k = (p.z - 0.5).floor() as i64 - 1;
    while k >= lowest {
        if holds(world.material_at(x, y, k)) {
            p.z = k as f64 + 1.0;
            return;
        }
        k -= 1;
    }
    p.z = to;
}

impl Game {
    /// How light it is, 0 at night to 1 by day, for the beams of what's dropped, see main.rs
    pub fn set_daylight(&mut self, daylight: f64) {
        self.daylight = daylight;
    }

    /// Where drop model `i` is among model_words(), after the stage's
    pub(super) fn drop_model(&self, i: usize) -> u32 {
        self.stage_model() + (self.stage_models.len() + i) as u32
    }

    /// How rare something dropped is, common 0 to mythic 5 as the weapons' rarities go, and the
    /// colour its beam shines: its rarity's, or gold's own for gold, as bright as an epic's for the
    /// gold the player lost when they died
    pub(super) fn shine(&self, pickup: &Pickup) -> (usize, [u8; 3]) {
        let last = self.library.rarities.len().saturating_sub(1);
        let rarity = |g: usize| (g.min(last), self.library.rarities.get(g.min(last)).map_or([255; 3], |r| r.color));
        let gold = [GOLD.r(), GOLD.g(), GOLD.b()];
        match pickup.item {
            Item::Weapon(w) => rarity(self.weapons[w].grade),
            Item::Gold(_) if pickup.beacon => (3, gold),
            Item::Gold(n) => (
                match n {
                    0..20 => 0,
                    20..100 => 1,
                    100..300 => 2,
                    _ => 3,
                },
                gold,
            ),
            Item::Wings(k) => rarity(wings::KINDS[k as usize % wings::KINDS.len()].rarity),
            Item::Jewel(j) => rarity(jewels::parts(j).1),
            Item::InnertBlock(_) => rarity(5),
            Item::Hook => rarity(2),
            Item::Ammo(k, _) => rarity(usize::from(self.library.ammo.get(k).is_some_and(|a| a.element.is_some() || a.burst.is_some()))),
            Item::Potion | Item::SharkTooth => rarity(1),
            _ => rarity(0),
        }
    }

    /// Each frame: what's dropped kept on what it lies on, and whether the camera can see each
    /// one's beam, its foot, its middle or its top, so one in tall grass, in water or down a hole
    /// still shows, easing in and out of sight. `water` is the top of the water at a point.
    pub(super) fn watch_drops(&mut self, world: &World, water: WaterLevel, view: &View, dt: f64) {
        let vpm = world.voxels_per_meter as f64;
        let eye = view.eye;
        let ease = (dt * 6.0).min(1.0) as f32;
        let heights: Vec<f64> = self.pickups.iter().map(|p| BEAM_HEIGHTS[self.shine(p).0.min(BEAM_HEIGHTS.len() - 1)]).collect();
        for (pickup, height) in self.pickups.iter_mut().zip(heights) {
            if pickup.flying <= 0.0 && pickup.item != Item::Apple && (pickup.position - eye).norm() < SETTLED_WITHIN * vpm {
                settle(world, water, pickup, vpm, dt);
            }
            if pickup.shines() {
                let foot = pickup.at(vpm) + Vector3::new(0.0, 0.0, 0.4 * vpm);
                pickup.seen = (foot - eye).norm() < BEAM_RANGE * vpm && [0.0, 0.5, 0.92].iter().any(|f| in_sight(world, eye, foot + Vector3::new(0.0, 0.0, f * height * vpm), vpm));
                let want = if pickup.seen && pickup.flying <= 0.0 { 1.0 } else { 0.0 };
                pickup.shown += (want - pickup.shown) * ease;
            }
        }
        // So do the hoards of the camps cleared, until they're opened
        self.beams.clear();
        for chest in self.chests.iter().filter(|c| c.hoard && !self.opened.contains(&c.key)) {
            let foot = chest.position + Vector3::new(0.0, 0.0, 0.5 * vpm);
            if (foot - eye).norm() < BEAM_RANGE * vpm && [0.0, 0.5, 0.92].iter().any(|f| in_sight(world, eye, foot + Vector3::new(0.0, 0.0, f * BEAM_HEIGHTS[4] * vpm), vpm)) {
                self.beams.push(chest.position);
            }
        }
    }

    /// What's drawn of `pickup`, at `at` where it is now, in voxels: itself, turning slowly so it
    /// reads, small things a little over life size, tumbling as it flies out, bobbing afloat
    pub(super) fn drop_instances(&self, pickup: &Pickup, at: Vector3<f64>, vpm: f64, out: &mut Vec<AnimalInstance>) {
        let time = self.combat.time();
        let tumble = pickup.flying / DROP_SECONDS * 9.0;
        let bob = if pickup.afloat { 0.035 * (time * 1.7 + pickup.heading).sin() } else { 0.0 };
        let at = at + Vector3::new(0.0, 0.0, bob * vpm);
        let turn = |rate: f64| UnitQuaternion::from_axis_angle(&Vector3::z_axis(), pickup.heading + time * rate);
        let tumbling = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), tumble);
        match pickup.item.shape() {
            Shape::Look(look) => {
                let grow = if look.reach() <= SMALL { GROW } else { 1.0 };
                out.push(animals::lying_instance(look, at, turn(0.45) * tumbling, grow, vpm));
            }
            // A weapon lies on its side a hand over the ground, turning slowly and bobbing, its
            // middle over where it lies
            Shape::Weapon(w) => {
                let look = &self.weapons[w].look;
                let rotation = turn(0.7) * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), FRAC_PI_2 + tumble);
                let hover = (0.22 + 0.05 * (time * 2.2 + pickup.heading).sin()) * vpm;
                let middle = (look.grip + look.tip) * 0.5;
                out.push(placed(self.weapon_model(w), &look.frames[0], look.res, middle, at + Vector3::new(0.0, 0.0, hover), rotation, vpm));
            }
            Shape::Ammo(k) => {
                let def = &self.library.ammo[k];
                out.extend(self.combat.bundle_instances(def.shot(), def.head.unwrap_or(0), def.element, at, pickup.heading + time * 0.45));
            }
            Shape::Wings(k) => out.extend(wings::lying(self.wing_model(0), &self.wing_models, k, at, pickup.heading + time * 0.25, vpm)),
            // A jewel floats a hand over the ground on its point, turning; the bucket stands
            Shape::Own(i) => {
                let (res, hover, rate) = if i == BUCKET_MODEL { (BUCKET_RES, 0.0, 0.3) } else { (jewels::LYING_RES, 0.16 + 0.03 * (time * 2.0 + pickup.heading).sin(), 1.1) };
                out.push(placed(self.drop_model(i), &self.drop_models[i], res, Vector3::zeros(), at + Vector3::new(0.0, 0.0, hover * vpm), turn(rate) * tumbling, vpm));
            }
        }
    }

    /// What's dropped near enough to be marked on the compass, nearest first, at most MARKS: where
    /// it lies, in voxels, how rare it is and its beam's colour
    pub fn drop_marks(&self, eye: Vector3<f64>) -> Vec<(Vector3<f64>, usize, [u8; 3])> {
        let vpm = self.combat.vpm();
        let mut marks: Vec<(f64, Vector3<f64>, usize, [u8; 3])> = self
            .pickups
            .iter()
            .filter(|p| p.shines() && p.flying <= 0.0)
            .filter_map(|p| {
                let d = (p.position - eye).xy().norm();
                (d < DROP_MARKED * vpm).then(|| {
                    let (grade, color) = self.shine(p);
                    (d, p.position, grade, color)
                })
            })
            .collect();
        marks.sort_by(|a, b| a.0.total_cmp(&b.0));
        marks.into_iter().take(MARKS).map(|(_, at, grade, color)| (at, grade, color)).collect()
    }

    /// For trying it out: where the rarest weapon dropped within `within` meters of `eye` lies,
    /// nearest first among those as rare, see `--set loot_look_at=` in main.rs
    pub fn rarest_weapon_dropped(&self, eye: Vector3<f64>, within: f64) -> Option<Vector3<f64>> {
        let vpm = self.combat.vpm();
        self.pickups
            .iter()
            .filter_map(|p| match p.item {
                Item::Weapon(w) if (p.position - eye).norm() < within * vpm => Some((self.weapons[w].grade, -(p.position - eye).norm(), p.position)),
                _ => None,
            })
            .max_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)))
            .map(|(_, _, at)| at)
    }

    /// The beams of light up from what's dropped in sight, and from the hoards of camps cleared:
    /// see the module's doc. One mesh for them all over the view, and a few motes rising up the
    /// rarer ones'. By night they glow, adding their light to what's behind; by day they're laid
    /// over it, so they stand out against a bright sky.
    pub(super) fn draw_beams(&self, painter: &egui::Painter, view: &View, screen: Rect) {
        let vpm = view.vpm;
        let day = self.daylight.clamp(0.0, 1.0) as f32;
        let time = self.combat.time();
        let gold = [GOLD.r(), GOLD.g(), GOLD.b()];
        let lying = self.pickups.iter().filter(|p| p.shown > 0.01 && p.flying <= 0.0 && p.shines()).map(|p| {
            let (grade, color) = self.shine(p);
            (p.at(vpm), grade, color, p.heading * 3.1, p.shown)
        });
        let hoards = self.beams.iter().enumerate().map(|(k, &at)| (at + Vector3::new(0.0, 0.0, 0.25 * vpm), 4, gold, 7.0 + k as f64, 1.0f32));
        let project = |p: Vector3<f64>| view.project(p).map(|[x, y]| pos2(screen.left() + x as f32 * screen.width(), screen.top() + y as f32 * screen.height()));
        let right = view.rotation * Vector3::x();
        // How many pixels a meter is across at `p`, side on to the view
        let across = |p: Vector3<f64>, at: Pos2| project(p + right * vpm).map_or(0.0, |q| (q - at).length());
        // Laid over by day, added on by night, as egui's colours are premultiplied
        let blend = 0.3 + 0.7 * day;
        let tint = |[r, g, b]: [u8; 3], a: f32| {
            let a = a.clamp(0.0, 1.0);
            Color32::from_rgba_premultiplied((r as f32 * a) as u8, (g as f32 * a) as u8, (b as f32 * a) as u8, (a * blend * 255.0) as u8)
        };
        let mut mesh = egui::Mesh::default();
        let mut motes = Vec::new();
        for (at, grade, color, phase, shown) in lying.chain(hoards) {
            let grade = grade.min(BEAM_HEIGHTS.len() - 1);
            let foot = at + Vector3::new(0.0, 0.0, 0.08 * vpm);
            let to = foot - view.eye;
            let distance = to.norm() / vpm;
            let flat = to.xy().norm() / vpm;
            let far = ((BEAM_RANGE - distance) / 20.0).clamp(0.0, 1.0) as f32;
            let t = ((flat - BEAM_GONE) / (BEAM_WHOLE - BEAM_GONE)).clamp(0.0, 1.0) as f32;
            let near = t * t * (3.0 - 2.0 * t);
            let pulse = 0.86 + 0.14 * (time * 2.2 + phase).sin() as f32;
            let strength = shown * far * pulse;
            if strength < 0.01 {
                continue;
            }
            let rare = grade as f32;
            // The glow on the ground round it, wide for a legendary or a mythic, and staying as the
            // player walks up, as it's under their feet rather than in their eyes
            let pool = [0.35, 0.4, 0.5, 0.6, 1.25, 1.6][grade];
            let rim: Option<Vec<Pos2>> = (0..=20).map(|k| {
                let a = k as f64 / 20.0 * TAU;
                project(at + Vector3::new(a.cos(), a.sin(), 0.0) * pool * vpm + Vector3::new(0.0, 0.0, 0.04 * vpm))
            }).collect();
            if let (Some(rim), Some(middle)) = (rim, project(at + Vector3::new(0.0, 0.0, 0.04 * vpm))) {
                let first = mesh.vertices.len() as u32;
                let glow = if grade >= 4 { 0.5 } else { 0.3 } * strength * (0.45 + 0.55 * near);
                mesh.colored_vertex(middle, tint(color, glow));
                for (k, &p) in rim.iter().enumerate() {
                    mesh.colored_vertex(p, tint(color, 0.0));
                    if k > 0 {
                        mesh.add_triangle(first, first + k as u32, first + k as u32 + 1);
                    }
                }
            }
            let strength = strength * near;
            if strength < 0.01 {
                continue;
            }
            let height = BEAM_HEIGHTS[grade] * vpm;
            let top = foot + Vector3::new(0.0, 0.0, height);
            let (Some(base), Some(peak)) = (project(foot), project(top)) else {
                continue;
            };
            let (wide_foot, wide_top) = (across(foot, base), across(top, peak));
            let along = peak - base;
            if along.length() < 1.0 {
                continue;
            }
            let side = vec2(-along.y, along.x).normalized();
            let white = [0, 1, 2].map(|i| (color[i] as f32 + (255.0 - color[i] as f32) * (0.7 - 0.35 * day)) as u8);
            let dark = color.map(|c| (c as f32 * 0.3) as u8);
            // Its glow, a dark edge by day so it stands out against a bright sky or sand, its body
            // and its bright core, each a strip of quads fading up the beam, by day more slowly
            let body = 0.1 + 0.03 * rare as f64;
            let layers = [
                (body * 3.4, 5.0, 26.0, color, 0.22 * (1.0 - 0.3 * day)),
                (body * 1.45, 3.4 * day, 11.0, dark, 0.4 * day),
                (body, 2.2 + 0.8 * day, 8.0, color, 0.55 + 0.35 * day),
                (body * 0.32, 1.0, 2.5, white, 0.85),
            ];
            let fading = 1.5 - 0.75 * day;
            const ROWS: usize = 7;
            for (radius, least, most, rgb, alpha) in layers {
                if alpha <= 0.0 {
                    continue;
                }
                let first = mesh.vertices.len() as u32;
                for row in 0..ROWS {
                    let u = row as f32 / (ROWS - 1) as f32;
                    let half = ((wide_foot + (wide_top - wide_foot) * u) * radius as f32).clamp(least, most);
                    let at = base + along * u;
                    let c = tint(rgb, alpha * strength * (1.0 - u).powf(fading));
                    mesh.colored_vertex(at - side * half, c);
                    mesh.colored_vertex(at + side * half, c);
                    if row > 0 {
                        let k = first + 2 * row as u32;
                        mesh.add_triangle(k - 2, k - 1, k);
                        mesh.add_triangle(k - 1, k + 1, k);
                    }
                }
            }
            // Motes rising up a rare one's beam, more the rarer, fading as they go
            for k in 0..grade.saturating_sub(1) {
                let u = ((time * 0.3 + phase * 0.37 + k as f64 / (grade - 1) as f64).fract()) as f32;
                let wobble = side * (wide_foot * body as f32 * 0.8) * ((time * 3.0 + k as f64 * 2.0).sin() as f32);
                let size = (wide_foot + (wide_top - wide_foot) * u) * body as f32 * 0.7;
                motes.push((base + along * u + wobble, size.clamp(1.2, 5.0), tint(white, strength * (u * std::f32::consts::PI).sin())));
            }
        }
        if !mesh.is_empty() {
            painter.add(egui::Shape::mesh(mesh));
        }
        for (at, size, color) in motes {
            painter.circle_filled(at, size, color);
        }
    }

    /// For trying it out, see `--set drops=` in main.rs: `count` things dropped round `around`, in
    /// voxels, out to `radius` meters, flying out from over it to land, a mix of everything that
    /// can lie about with weapons of every rarity among it; or with `row` one weapon of each rarity
    /// lying in a row across `ahead`, the commonest on the left. Returns where on the ground the
    /// middle of it all is.
    pub fn strew(&mut self, world: &World, around: Vector3<f64>, ahead: Vector3<f64>, count: usize, radius: f64, row: bool) -> Vector3<f64> {
        let vpm = world.voxels_per_meter as f64;
        let ground = |p: Vector3<f64>| {
            let over = p + Vector3::new(0.0, 0.0, 30.0 * vpm);
            (0..(80.0 * vpm) as i64).map(|d| over - Vector3::new(0.0, 0.0, d as f64)).find(|q| !is_passable(world.material_at(q.x.floor() as i64, q.y.floor() as i64, q.z.floor() as i64))).map_or(p, |q| Vector3::new(p.x, p.y, q.z.floor() + 1.0))
        };
        let middle = ground(around);
        let rarities = self.library.rarities.len();
        // Of that rarity: a weapon's grade comes of its parts all told, all of them of one rarity
        // can lift it a grade, so they're of that rarity and the one under it, and a few are made
        // till one's right
        let weapon = |game: &mut Game, grade: usize| {
            let odds: Vec<f64> = (0..rarities).map(|g| if g == grade || g + 1 == grade { 1.0 } else { 0.0 }).collect();
            let mut made = game.library.generate_with(game.rng, None, &odds);
            for _ in 0..80 {
                game.rng = hash(game.rng ^ 0x5d70);
                if made.grade == grade {
                    break;
                }
                made = game.library.generate_with(game.rng, None, &odds);
            }
            game.rng = hash(game.rng ^ 0x5d71);
            game.weapons.push(made);
            game.models_version += 1;
            Item::Weapon(game.weapons.len() - 1)
        };
        if row {
            let flat = Vector3::new(ahead.x, ahead.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x());
            let side = Vector3::new(flat.y, -flat.x, 0.0);
            for g in 0..rarities {
                let item = weapon(self, g);
                if let Item::Weapon(w) = item {
                    println!("drops: {} for {}", self.describe(&self.weapons[w]), self.library.rarities[g].display_name);
                }
                let at = ground(around + side * (g as f64 - (rarities - 1) as f64 * 0.5) * 3.0 * vpm);
                self.pickups.push(Pickup::lying(item, at, side.y.atan2(side.x)));
            }
            return middle;
        }
        let ammo = (!self.library.ammo.is_empty()).then(|| Item::Ammo(0, 12));
        let others = [Item::Gold(160), Item::Potion, Item::Bread, Item::Meat, Item::CookedFish, Item::SharkTooth, Item::Jewel(jewels::jewel(jewels::Category::Magic, 2)), Item::Wings(wings::GULL), Item::Bucket, Item::Jewel(jewels::jewel(jewels::Category::Melee, 4)), Item::Wings(wings::AURELIAN), Item::Hook, Item::InnertBlock(0), Item::Jewel(jewels::jewel(jewels::Category::Ranged, 5))];
        let mut items = Vec::new();
        for k in 0..count {
            items.push(match k % 3 {
                0 => weapon(self, (k / 3) % rarities),
                1 => others[(k / 3) % others.len()],
                _ => ammo.filter(|_| k % 9 == 2).unwrap_or(others[(k / 3 + 7) % others.len()]),
            });
        }
        self.scatter(world, middle + Vector3::new(0.0, 0.0, 2.0 * vpm), middle, items, radius);
        middle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every kind of thing there is, once
    fn every_kind() -> Vec<Item> {
        let all = vec![
            Item::Bucket,
            Item::Apple,
            Item::Meat,
            Item::RoastMeat,
            Item::RawFish,
            Item::CookedFish,
            Item::SharkTooth,
            Item::Bread,
            Item::Potion,
            Item::Gold(25),
            Item::Weapon(0),
            Item::Ammo(0, 10),
            Item::InnertBlock(0),
            Item::Hook,
            Item::Wings(0),
            Item::Jewel(0),
        ];
        // A kind added has to be added above, and given a model, or this won't build
        for item in &all {
            match item {
                Item::Bucket | Item::Apple | Item::Meat | Item::RoastMeat | Item::RawFish | Item::CookedFish | Item::SharkTooth | Item::Bread | Item::Potion | Item::Gold(_) | Item::Weapon(_) | Item::Ammo(..) | Item::InnertBlock(_) | Item::Hook | Item::Wings(_) | Item::Jewel(_) => {}
            }
        }
        all
    }

    fn set(world: &mut World, x: i64, y: i64, z: i64, m: u8) {
        let b = [x / 8, y / 8, z / 8];
        let mut materials = crate::bases::read_brick(world, b);
        materials[((x % 8) + (y % 8) * 8 + (z % 8) * 64) as usize] = m;
        world.set_brick(b, &materials);
    }

    fn flat_world() -> World {
        let size = [64u32, 64, 64];
        let mut occupancy = vec![0u128; 16 * 16 * 8];
        occupancy[..16 * 16 * 2].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, DIRT);
        world.voxels_per_meter = 4;
        world
    }

    /// Every kind of thing dropped lies about as itself, with a model of its own that's there and
    /// has something in it: a weapon its own, wings their kind's, a jewel its kind's, the bucket
    /// its own, and everything else one among the animals'
    #[test]
    fn everything_dropped_lies_about_as_itself() {
        let world = flat_world();
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let mut game = Game::new(&world, &dry, 5, Vector3::new(32.0, 32.0, 16.0));
        game.weapons.push(game.library.generate(7, None, 1.0));
        let models = game.model_words().len() as u32;
        assert_eq!(game.drop_models.len(), jewels::KINDS + 1);
        for (model, _) in &game.drop_models {
            assert!(model.size.iter().all(|&s| s > 1 && s < 255) && model.voxels.iter().filter(|&&v| v != AIR).count() > 40, "{:?}", model.size);
        }
        // Each jewel of its own colour, rarer ones gold-shot
        assert_ne!(game.drop_models[jewels::jewel(jewels::Category::Melee, 2) as usize].0.voxels, game.drop_models[jewels::jewel(jewels::Category::Magic, 2) as usize].0.voxels);
        assert!(game.drop_models[jewels::jewel(jewels::Category::Melee, 4) as usize].0.voxels.contains(&crate::worldgen::material::GOLD));
        let vpm = world.voxels_per_meter as f64;
        let at = Vector3::new(32.0, 32.0, 16.0);
        for item in every_kind() {
            let pickup = Pickup::lying(item, at, 0.3);
            let mut out = Vec::new();
            game.drop_instances(&pickup, at, vpm, &mut out);
            assert!(!out.is_empty(), "{item:?} has nothing drawn");
            match item.shape() {
                Shape::Look(look) => assert_ne!(look, Look::Wings, "{item:?}"),
                Shape::Wings(k) => assert!(out.len() == 2 && out.iter().all(|i| i.model() == game.wing_model(k as usize)), "both wings the kind's own"),
                Shape::Weapon(w) => assert_eq!(out[0].model(), game.weapon_model(w)),
                Shape::Own(i) => {
                    assert_eq!(out[0].model(), game.drop_model(i));
                    // Among the models packed, before the weapons'
                    assert!(out[0].model() < models && out[0].model() < game.weapon_model(0));
                }
                Shape::Ammo(_) => {}
            }
        }
        // And every kind's beam is of a rarity there is, the apples on the trees' none
        for item in every_kind() {
            let pickup = Pickup::lying(item, at, 0.0);
            assert_eq!(pickup.shines(), item != Item::Apple);
            assert!(game.shine(&pickup).0 < BEAM_HEIGHTS.len());
        }
        // A row of every rarity's weapon, for its beam, one of each
        game.pickups.clear();
        game.strew(&world, at, Vector3::y(), 0, 3.0, true);
        let grades: Vec<usize> = game.pickups.iter().map(|p| game.shine(p).0).collect();
        assert_eq!(grades, (0..game.library.rarities.len()).collect::<Vec<_>>());
        // Wings of a rarer kind shine rarer
        let gull = game.shine(&Pickup::lying(Item::Wings(wings::GULL), at, 0.0)).0;
        let aurelian = game.shine(&Pickup::lying(Item::Wings(wings::AURELIAN), at, 0.0)).0;
        assert!(aurelian > gull, "{gull} {aurelian}");
    }

    /// What the models of what's dropped take, packed as they're sent to the graphics card: the
    /// jewels' and the bucket's, which are all that's new, everything else lying about as the
    /// models it had already, its own weapon's, its wings' kind's; and all the models with twenty
    /// different things dropped about, against the cap. `cargo test --release
    /// drop_models_fit -- --nocapture`
    #[test]
    fn drop_models_fit() {
        let world = flat_world();
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let mut game = Game::new(&world, &dry, 5, Vector3::new(32.0, 32.0, 16.0));
        let mb = |words: usize| words as f64 * 4.0 / 1e6;
        let own = mb(pack_models(&game.drop_models.iter().map(|(m, _)| m).collect::<Vec<_>>()).len());
        let before = mb(game.model_words().len() + game.weapon_model_words().len());
        game.strew(&world, Vector3::new(32.0, 32.0, 16.0), Vector3::y(), 20, 3.0, false);
        let after = mb(game.model_words().len() + game.weapon_model_words().len());
        let dropped: Vec<&Pickup> = game.pickups.iter().filter(|p| p.item != Item::Apple).collect();
        let kinds: HashSet<String> = dropped.iter().map(|p| format!("{:?}", p.item.shape())).collect();
        println!("drop models {own:.3} MB; all models {before:.1} MB, {after:.1} MB with {} things dropped, {} kinds of them", dropped.len(), kinds.len());
        assert!(own < 0.5, "{own} MB");
        assert!(after < 150.0, "{after} MB");
    }

    /// However much else there is about, the nearest of what's dropped are always drawn, and every
    /// beam in sight is drawn over the view whether its model is or not
    #[test]
    fn drops_are_drawn_whatever_else_is_about() {
        let world = flat_world();
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let mut game = Game::new(&world, &dry, 5, Vector3::new(32.0, 32.0, 16.0));
        game.animals.clear();
        game.pickups.clear();
        let vpm = world.voxels_per_meter as f64;
        let eye = Vector3::new(32.0, 4.0, 20.0);
        let view = View { eye, rotation: UnitQuaternion::identity(), tan: [0.7, 0.4], vpm, speed: 0.0, stride: 0.0 };
        // A crowd of props right by the eye to fill what's drawn, and thirty things dropped further
        for k in 0..(2 * MAX_INSTANCES) {
            game.props.push((Look::Skull, eye + Vector3::new((k % 20) as f64 * 0.3 - 3.0, 1.0 + (k / 20) as f64 * 0.2, -1.0), 0.0));
        }
        game.weapons.push(game.library.generate(7, None, 1.0));
        for k in 0..30 {
            game.pickups.push(Pickup::lying(Item::Weapon(0), Vector3::new(4.0 + k as f64 * 2.0, 40.0, 16.0), 0.0));
        }
        let drawn = game.instances(&world, &view);
        assert!(drawn.len() <= MAX_INSTANCES);
        let weapons = drawn.iter().filter(|i| i.model() == game.weapon_model(0)).count();
        assert!(weapons >= DROP_ROOM.min(30), "only {weapons} drawn");
        // Every one of them in sight has its beam
        game.watch_drops(&world, &dry, &view, 1.0);
        assert!(game.pickups.iter().all(|p| p.seen && p.shown > 0.9));
    }

    /// A beam's seen if its foot, its middle or its top is: one down a hole, its foot hidden by the
    /// rim, still shows; one behind a wall as tall as it doesn't
    #[test]
    fn beams_rise_out_of_holes() {
        let mut world = flat_world();
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let mut game = Game::new(&world, &dry, 5, Vector3::new(32.0, 32.0, 16.0));
        game.pickups.clear();
        let vpm = world.voxels_per_meter as f64;
        // A hole 2 m deep, 1 m across, and the eye low over the ground 10 m off
        for z in 8..16 {
            for x in 31..35 {
                for y in 40..44 {
                    set(&mut world, x, y, z, AIR);
                }
            }
        }
        let eye = Vector3::new(32.0, 0.0, 17.0);
        let view = View { eye, rotation: UnitQuaternion::identity(), tan: [0.7, 0.4], vpm, speed: 0.0, stride: 0.0 };
        game.pickups.push(Pickup::lying(Item::Potion, Vector3::new(32.5, 41.5, 8.0), 0.0));
        game.watch_drops(&world, &dry, &view, 1.0);
        assert!(!in_sight(&world, eye, game.pickups[0].position + Vector3::new(0.0, 0.0, 0.4 * vpm), vpm), "its foot's hidden");
        assert!(game.pickups[0].seen, "its beam shows");
        // A wall as tall as the tallest beam between
        for z in 16..64 {
            for x in 20..44 {
                set(&mut world, x, 20, z, STONE);
            }
        }
        game.watch_drops(&world, &dry, &view, 1.0);
        assert!(!game.pickups[0].seen);
    }

    /// Nothing dropped sinks into the ground: buried it rises to the top, hanging it falls till
    /// something holds it, and in deep water it floats up to ride on top; an apple on a tree stays
    #[test]
    fn nothing_dropped_falls_through_the_ground() {
        let mut world = flat_world();
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let mut game = Game::new(&world, &dry, 5, Vector3::new(32.0, 32.0, 16.0));
        game.pickups.clear();
        let vpm = world.voxels_per_meter as f64;
        let eye = Vector3::new(32.0, 0.0, 20.0);
        let view = View { eye, rotation: UnitQuaternion::identity(), tan: [0.7, 0.4], vpm, speed: 0.0, stride: 0.0 };
        // The ground's top is at 16: one buried in it, one hanging in the air, one lying on it
        game.pickups.push(Pickup::lying(Item::Bread, Vector3::new(10.5, 10.5, 12.0), 0.0));
        game.pickups.push(Pickup::lying(Item::Potion, Vector3::new(20.5, 10.5, 30.0), 0.0));
        game.pickups.push(Pickup::lying(Item::Meat, Vector3::new(30.5, 10.5, 16.0), 0.0));
        game.pickups.push(Pickup::lying(Item::Apple, Vector3::new(40.5, 10.5, 30.0), 0.0));
        for _ in 0..120 {
            game.watch_drops(&world, &dry, &view, 1.0 / 30.0);
        }
        let z: Vec<f64> = game.pickups.iter().map(|p| p.position.z).collect();
        assert_eq!(&z[..3], &[16.0, 16.0, 16.0], "{z:?}");
        assert_eq!(z[3], 30.0, "the apple hangs on");
        // The ground under the meat dug away, a hole 3 m deep: it falls to the bottom, not through
        for x in 28..33 {
            for y in 8..13 {
                for zz in 4..16 {
                    set(&mut world, x, y, zz, AIR);
                }
            }
        }
        for _ in 0..120 {
            game.watch_drops(&world, &dry, &view, 1.0 / 30.0);
        }
        assert_eq!(game.pickups[2].position.z, 4.0);
        // Deep water over the bread: it floats up to the top and rides there
        let lake = |x: f64, _: f64| if x < 15.0 { 24.0 } else { f64::NEG_INFINITY };
        for _ in 0..200 {
            game.watch_drops(&world, &lake, &view, 1.0 / 30.0);
        }
        assert!(game.pickups[0].afloat && game.pickups[0].position.z == 24.0, "{:?}", game.pickups[0].position);
        // Drained, it settles on the ground again
        for _ in 0..120 {
            game.watch_drops(&world, &dry, &view, 1.0 / 30.0);
        }
        assert!(!game.pickups[0].afloat && game.pickups[0].position.z == 16.0);
    }
}
