//! Camps down in the caves, in some of the caverns and the wider stretches of tunnel, where
//! crystals used to glow on the floor: a fire burning in a ring of stones, and round it the signs
//! of whoever keeps it. Bandits keep some, lived in, with bedrolls, logs to sit on, a stash and a
//! pot or a spit over the fire, their hoard turning up once they're beaten as at the camps out on
//! the land; the dead keep others, cold and gone to ruin, old bones and a skull or two about a
//! guttering pale green fire, the dead themselves lying under its floor until someone comes near,
//! see src\game\cavecamps.rs; and the rest are left, the fire still burning, a few supplies and a
//! chest. Which goes by a roll of its own for each camp, the dead keeping more of the deep ones.
//!
//! Each is "cave camp" at its fire, facing the way it's come to from, see CaveCamp.

use super::*;

/// What a cave camp is at its fire, see CaveCamp
pub const CAVE_CAMP_SITE: &str = "cave camp";
/// The light of a camp's fire in a cave: redder than a torch's, so it flickers more, see flicker()
/// in main.rs, and brighter, as all there is to see the camp by. And the dead's, pale green,
/// guttering.
pub const CAMPFIRE_LIGHT: [f32; 3] = [2.2, 1.1, 0.44];
pub const GHOST_FIRE_LIGHT: [f32; 3] = [0.63, 1.42, 1.2];

/// Of the camps in caves, how many bandits keep and how many the dead, out of one, the rest left
/// to no one; and how many meters under the ground a camp's deep, where the dead keep DEEPER more
/// of them and bandits that many fewer
pub const BANDITS: f32 = 0.4;
pub const DEAD: f32 = 0.3;
const DEEP: f32 = 18.0;
const DEEPER: f32 = 0.12;

/// The least room a camp's to have round its fire, and the most that's looked for, in meters: a
/// flat floor with room to stand over it that far out all round
pub const LEAST_ROOM: f32 = 1.6;
const MOST_ROOM: f32 = 3.6;
/// How much higher or lower the floor may be anywhere in a camp than at its fire, and how much
/// room there has to be over it to stand
const LEVEL: f32 = 0.3;
const HEADROOM: f32 = 1.9;

/// Who keeps a camp in a cave
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Folk {
    Bandits,
    Dead,
    /// No one: left with its fire burning
    Nobody,
}

impl Folk {
    /// As in `--set cave_camp=`
    pub fn named(name: &str) -> Option<Folk> {
        match name {
            "bandits" => Some(Folk::Bandits),
            "dead" | "skeletons" => Some(Folk::Dead),
            "nobody" | "empty" => Some(Folk::Nobody),
            _ => None,
        }
    }

    /// Who keeps a camp, by `roll`, 0-1, `depth` meters under the ground: none of them bandits in
    /// a land no one lives in, see place_objects()
    pub fn of(roll: f32, depth: f32, peopled: bool) -> Folk {
        let shift = if depth > DEEP { DEEPER } else { 0.0 };
        let bandits = BANDITS - shift;
        if roll < bandits {
            if peopled { Folk::Bandits } else { Folk::Dead }
        } else if roll < bandits + DEAD + shift {
            Folk::Dead
        } else {
            Folk::Nobody
        }
    }
}

/// A camp in a cave, in meters: its fire on the floor, which way the way it's come to from goes,
/// who keeps it and how far out from its fire there's room. For bandits, how many and how many
/// brutes, and how far round the fire they keep, and where the chest of their hoard turns up once
/// they're all dead, as for the camps out on the land, see Camp.
#[derive(Clone, Copy, Debug)]
pub struct CaveCamp {
    pub fire: [f32; 3],
    pub way: [f32; 2],
    pub folk: Folk,
    pub room: f32,
    pub bandits: u32,
    pub brutes: u32,
    pub spread: f32,
    pub chest: [f32; 3],
    pub heading: f32,
    pub luck: f32,
    pub hint: &'static str,
}

/// Where in a cave a camp might go: the floor round the middle of a cavern or a stretch of tunnel,
/// and which way the way in is from there. `folk` says who keeps it, if that's not rolled, and
/// `chest` whether it has a chest of its own, if no bandits keep it.
#[derive(Clone, Copy)]
pub(in crate::worldgen) struct CampSpot {
    pub at: Vec3,
    pub way: [f32; 2],
    pub folk: Option<Folk>,
    pub chest: bool,
}

/// What's built for a camp in a cave: its shapes, in the hollows, its fire's light, its chest and
/// skulls, and the camp
pub(in crate::worldgen) struct Built {
    pub shapes: Vec<Shape>,
    pub lights: Vec<Light>,
    pub props: Vec<Prop>,
    pub camp: CaveCamp,
}

/// The floor and the room over it in the hollows near a camp
struct Hollows<'a> {
    near: Vec<&'a Shape>,
}

impl Hollows<'_> {
    fn open(&self, p: Vec3) -> bool {
        self.near.iter().any(|s| !s.clear_of(p, 0.0) && s.holds(p, s.depth(p, 1.0 / 16.0)))
    }

    /// The floor at `x`, `y` a little under `about`, if there's room over it there: the bottom of
    /// the hollow a meter over that, or None where that's in the rock or there's no floor within
    /// two and a half meters under it
    fn floor(&self, x: f32, y: f32, about: f32) -> Option<f32> {
        let step = 1.0 / 16.0;
        let mut z = about + 1.0;
        if !self.open([x, y, z]) {
            return None;
        }
        for _ in 0..40 {
            if !self.open([x, y, z - step]) {
                return Some(z);
            }
            z -= step;
        }
        None
    }

    /// Whether `x`, `y` is floor near enough level with `level` to put something on, with room
    /// over it to stand
    fn level(&self, x: f32, y: f32, level: f32) -> bool {
        self.floor(x, y, level).is_some_and(|f| (f - level).abs() <= LEVEL && self.open([x, y, f + HEADROOM]))
    }
}

/// Spots round the fire for what's about it, SLOTS ways out from it round the back, leaving the
/// way in clear
const SLOTS: usize = 9;

impl<'a> Hollows<'a> {
    /// Those of `hollows` near `at`
    fn near(hollows: &'a [Shape], at: Vec3) -> Self {
        Hollows { near: hollows.iter().filter(|s| s.placement == Placement::Carve && !s.clear_of(at, MOST_ROOM + 2.5)).collect() }
    }
}

/// Where a camp's fire goes at `at`, if there's room for one there: the floor under it, out of
/// the sea, with room over it, and how far out from it the floor's level all round
fn site(settings: &TerrainSettings, h: &Hollows, at: Vec3) -> Option<(f32, f32)> {
    let [x, y, _] = at;
    let f0 = h.floor(x, y, at[2])?;
    if f0 < sea_level_m(settings) + 0.3 || !h.open([x, y, f0 + 2.6]) {
        return None;
    }
    let mut room = 0.0;
    for k in 0..=((MOST_ROOM - 1.2) / 0.4).round() as usize {
        let r = 1.2 + k as f32 * 0.4;
        if !(0..12).all(|j| {
            let a = j as f32 * TAU / 12.0;
            h.level(x + a.cos() * r, y + a.sin() * r, f0)
        }) {
            break;
        }
        room = r;
    }
    (room >= LEAST_ROOM).then_some((f0, room))
}

/// Whether there's room for a camp at `at` in the hollows `hollows` cut so far for its cave, see
/// Terrain::cave_in()
pub(in crate::worldgen) fn fits(settings: &TerrainSettings, hollows: &[Shape], at: Vec3) -> bool {
    site(settings, &Hollows::near(hollows, at), at).is_some()
}

/// The camp at `spot`, in the hollows `hollows` cut for its cave, if there's room there for one
pub(in crate::worldgen) fn cave_camp(settings: &TerrainSettings, hollows: &[Shape], spot: &CampSpot) -> Option<Built> {
    let h = Hollows::near(hollows, spot.at);
    let [x, y, _] = spot.at;
    let (f0, room) = site(settings, &h, spot.at)?;
    let key = [(x * 10.0).round() as i32, (y * 10.0).round() as i32];
    let seed = hash3(key[0] as u32, key[1] as u32, settings.seed ^ 0xcace);
    let peopled = !matches!(land_of(settings.seed), Land::Dream | Land::Water | Land::Dragon);
    let depth = height_at(settings, x, y) - f0;
    // Mixed again and taken from its high bits, as the low ones go along with the roll that put
    // a camp here at all, see Terrain::camp_here()
    let roll = unit(hash(seed.rotate_left(13) ^ 0x9e37_79b9) >> 16);
    let folk = spot.folk.unwrap_or_else(|| Folk::of(roll, depth, peopled));
    let ground = |gx: f32, gy: f32| h.floor(gx, gy, f0).filter(|f| (f - f0).abs() <= LEVEL).unwrap_or(f0);
    let mut lot = Lot { s: Vec::new(), lights: Vec::new(), ground: &ground, level: None, place: Placement::Inside, rng: Rng(hash(seed)) };
    let way = {
        let l = spot.way[0].hypot(spot.way[1]).max(1e-3);
        [spot.way[0] / l, spot.way[1] / l]
    };
    let mut c = Round { at: Frame { x, y, f: way }, f0, room, taken: [false; SLOTS], h: &h, props: Vec::new() };
    let mut camp = CaveCamp { fire: [x, y, f0], way, folk, room, bandits: 0, brutes: 0, spread: 0.0, chest: [x, y, f0], heading: 0.0, luck: 0.0, hint: "" };
    match folk {
        Folk::Bandits => bandits(&mut lot, &mut c, &mut camp),
        Folk::Dead => dead(&mut lot, &mut c, spot.chest),
        Folk::Nobody => nobody(&mut lot, &mut c, spot.chest),
    }
    Some(Built { shapes: lot.s, lights: lot.lights, props: c.props, camp })
}

/// Laying out what's round a camp's fire: in which of the spots round it things are already
struct Round<'a> {
    at: Frame,
    f0: f32,
    room: f32,
    taken: [bool; SLOTS],
    h: &'a Hollows<'a>,
    props: Vec<Prop>,
}

impl Round<'_> {
    /// Which way the spot `k` is from the fire: round the back, away from the way in, the middle
    /// one straight back
    fn angle(k: usize) -> f32 {
        PI + (k as f32 - (SLOTS - 1) as f32 * 0.5) * 0.2 * PI
    }

    /// Takes the first free spot, of those in `order`, where something `half` meters to either
    /// side of it round the fire and `deep` meters in toward it from `r` out stands level: where
    /// it is, which way round the fire is, and which way out
    fn take(&mut self, order: &[usize], r: f32, half: f32, deep: f32) -> Option<([f32; 2], [f32; 2], [f32; 2])> {
        let r = r.min(self.room - 0.2);
        for &k in order {
            if self.taken[k] {
                continue;
            }
            let a = Self::angle(k);
            let q = self.at.point(r * a.cos(), r * a.sin(), 0.0);
            let round = self.at.dir([-a.sin(), a.cos()]);
            let out = self.at.dir([a.cos(), a.sin()]);
            let fits = [(0.0, 0.0), (half, 0.0), (-half, 0.0), (0.0, -deep), (half, -deep), (-half, -deep)]
                .iter()
                .all(|&(s, d)| self.h.level(q[0] + round[0] * s + out[0] * d, q[1] + round[1] * s + out[1] * d, self.f0));
            if fits {
                self.taken[k] = true;
                return Some(([q[0], q[1]], round, out));
            }
        }
        None
    }

    fn skull(&mut self, x: f32, y: f32, z: f32, heading: f32) {
        self.props.push(Prop { kind: PropKind::Skull, position: [x, y, z], heading, luck: 0.0 });
    }

    fn chest(&mut self, at: [f32; 2], z: f32, luck: f32) -> ([f32; 3], f32) {
        // Its lock toward the fire
        let heading = (self.at.y - at[1]).atan2(self.at.x - at[0]);
        self.props.push(Prop { kind: PropKind::Chest, position: [at[0], at[1], z], heading, luck });
        ([at[0], at[1], z], heading)
    }
}

/// The order spots are tried in, the middle of the back first, then out to either side, from
/// `first`
fn order(first: usize, flip: bool) -> [usize; SLOTS] {
    let mut o = [0; SLOTS];
    for (n, slot) in o.iter_mut().enumerate() {
        let step = (n as i32 + 1) / 2;
        let side = if (n % 2 == 1) ^ flip { 1 } else { -1 };
        *slot = (first as i32 + step * side).rem_euclid(SLOTS as i32) as usize;
    }
    o
}

/// A fire burning in a cave: `lot.fire()`'s logs on embers in a ring of stones, a flame rising
/// out of them and its light, CAMPFIRE_LIGHT
fn bonfire(lot: &mut Lot, x: f32, y: f32, size: f32) {
    lot.fire(x, y, size, (ROCK, GRAVEL));
    let z = lot.g(x, y);
    lot.put(Shape::ellipsoid([x, y, z + 0.42 * size], [0.2 * size, 0.2 * size, 0.38 * size], FLAME, GLOW_FIRE));
    if let Some(light) = lot.lights.last_mut() {
        // Over the kettle or spit, so they don't shade the floor round the fire from it
        *light = Light { position: [x, y, z + 1.7], color: CAMPFIRE_LIGHT, reach: 12.5 * size.sqrt() };
    }
}

/// A few bones lying on the floor round `x`, `y`
fn bones(lot: &mut Lot, x: f32, y: f32) {
    for _ in 0..2 + (lot.rng.next() * 2.0) as usize {
        let (a, r) = (lot.rng.range(0.0, TAU), lot.rng.range(0.0, 0.45));
        let (bx, by) = (x + a.cos() * r, y + a.sin() * r);
        let lying = lot.rng.range(0.0, TAU);
        let half = lot.rng.range(0.2, 0.42);
        let (dx, dy) = (lying.cos() * half, lying.sin() * half);
        let z = lot.g(bx, by);
        let up = lot.rng.range(0.03, 0.1);
        lot.put(Shape::capsule([bx - dx, by - dy, z + 0.05], [bx + dx, by + dy, z + up], 0.055, 0.045, BONE, AIR, false));
    }
}

/// Bandits' camp, lived in: a good fire with a kettle or a spit over it, logs round it to sit on,
/// a fur over one, bedrolls, a stash of what they've taken and maybe a rack of spears, and where
/// their hoard turns up
fn bandits(lot: &mut Lot, c: &mut Round, camp: &mut CaveCamp) {
    let (x, y) = (c.at.x, c.at.y);
    let size = if c.room >= 2.4 { 1.1 } else { 0.85 };
    bonfire(lot, x, y, size);
    if c.room >= 2.0 && lot.rng.next() < 0.5 {
        lot.kettle(x, y, 1.25);
    } else {
        let along = c.at.dir([0.0, 1.0]);
        lot.spit(x, y, along);
    }
    let flip = lot.rng.next() < 0.5;
    // Logs to sit on near the fire, one with a fur over it
    let near = (c.room - 0.2).min(1.75);
    for (n, k) in [3usize, 1, 6].into_iter().enumerate() {
        if let Some((q, round, _)) = c.take(&[k, k + 1], near, 0.7, 0.2) {
            if n == 0 {
                let z = lot.g(q[0], q[1]);
                lot.put(Shape::ellipsoid([q[0], q[1], z + 0.35], [0.42, 0.42, 0.07], FUR_BROWN, AIR));
            }
            lot.seat(q[0], q[1], round);
        }
    }
    // Further out, round the walls: bedrolls, their stash, a rack, and room for the hoard
    let far = c.room - 0.45;
    let colors = [CLOTH, LINEN, FUR_BROWN, LEATHER];
    let all = order(SLOTS / 2, flip);
    for _ in 0..2 {
        if let Some((q, round, _)) = c.take(&all, far, 0.95, 0.4) {
            let color = colors[(lot.rng.next() * 3.99) as usize];
            lot.bedroll(q[0], q[1], round, color);
        }
    }
    if let Some((q, ..)) = c.take(&all, far, 0.6, 0.6) {
        lot.stash(q[0], q[1], 3);
    }
    if c.room >= 2.4
        && let Some((q, round, _)) = c.take(&all, c.room - 0.35, 0.85, 0.5)
    {
        lot.weapon_rack(q[0], q[1], round);
    }
    if let Some((q, ..)) = c.take(&all, far, 0.4, 0.4) {
        lot.barrel_at(q[0], q[1], lot.g(q[0], q[1]));
    }
    let hoard = c.take(&all, c.room - 0.55, 0.45, 0.4).map_or([x - c.at.f[0] * 1.2, y - c.at.f[1] * 1.2], |(q, ..)| q);
    let mut band = Rng(hash(lot.rng.0 ^ 0xba7d));
    let bandits = 2 + (band.next() < 0.5) as u32;
    let brutes = (band.next() < 0.4) as u32;
    let z = lot.g(hoard[0], hoard[1]);
    camp.bandits = bandits;
    camp.brutes = brutes;
    camp.spread = (c.room * 0.6).clamp(1.2, 2.4);
    camp.chest = [hoard[0], hoard[1], z];
    camp.heading = (y - hoard[1]).atan2(x - hoard[0]);
    camp.luck = 2.8 + 0.25 * (bandits + brutes) as f32;
    camp.hint = "by their fire in the cave";
}

/// The dead's camp, cold and fallen to ruin: a pale fire guttering in a ring of blackened stones,
/// half burnt logs, a bedroll rotted through with the bones of whoever slept in it, more bones and
/// skulls about, a barrel on its side, a rusted blade, and maybe an old chest
fn dead(lot: &mut Lot, c: &mut Round, chest: bool) {
    let (x, y) = (c.at.x, c.at.y);
    let z = lot.g(x, y);
    let turn = lot.rng.next() * TAU;
    lot.put(Shape::sphere([x, y, z + 0.06], 0.2, GLOW_POISON, AIR, false));
    lot.put(Shape::ellipsoid([x, y, z + 0.28], [0.1, 0.1, 0.24], GLOW_FROST, GLOW_POISON));
    for k in 0..3 {
        let a = turn + k as f32 * TAU / 3.0 + lot.rng.range(-0.3, 0.3);
        let (dx, dy) = (a.cos() * 0.5, a.sin() * 0.5);
        lot.put(Shape::capsule([x + dx, y + dy, z + 0.06], [x + dx * 0.1, y + dy * 0.1, z + 0.12], 0.09, 0.07, OBSIDIAN, BARK_DARK, false));
    }
    for k in 0..8 {
        let a = turn + k as f32 / 8.0 * TAU;
        let (sx, sy) = (x + a.cos() * 0.68, y + a.sin() * 0.68);
        // A stone or two kicked out of the ring
        let out = if k % 3 == 1 { lot.rng.range(0.2, 0.5) } else { 0.0 };
        let (sx, sy) = (sx + a.cos() * out, sy + a.sin() * out);
        let r = lot.rng.range(0.12, 0.18);
        let g = lot.g(sx, sy);
        lot.put(Shape::sphere([sx, sy, g], r, BASALT, MOSS, false));
    }
    lot.light([x, y, z + 1.1], GHOST_FIRE_LIGHT, 9.0);
    let flip = lot.rng.next() < 0.5;
    let all = order(SLOTS / 2, flip);
    let far = c.room - 0.45;
    // Whoever slept here, still lying in what's left of their bedroll
    if let Some((q, round, out)) = c.take(&all, far, 0.95, 0.4) {
        lot.bedroll(q[0], q[1], round, CLOTH);
        let g = lot.g(q[0], q[1]);
        let head = [q[0] + round[0] * 0.65, q[1] + round[1] * 0.65];
        // The spine and ribs down the bedroll, the skull at its head
        lot.put(Shape::capsule([head[0] - round[0] * 0.25, head[1] - round[1] * 0.25, g + 0.1], [q[0] - round[0] * 0.35, q[1] - round[1] * 0.35, g + 0.09], 0.045, 0.04, BONE, AIR, false));
        for k in 0..4 {
            let t = 0.2 - k as f32 * 0.14;
            let m = [q[0] + round[0] * t, q[1] + round[1] * t];
            lot.put(Shape::capsule([m[0] - out[0] * 0.17, m[1] - out[1] * 0.17, g + 0.09], [m[0] + out[0] * 0.17, m[1] + out[1] * 0.17, g + 0.11], 0.03, 0.03, BONE, AIR, false));
        }
        c.skull(head[0], head[1], g + 0.08, lot.rng.next() * TAU);
    }
    for _ in 0..3 {
        if let Some((q, ..)) = c.take(&all, lot.rng.range(1.2, far), 0.3, 0.3) {
            bones(lot, q[0], q[1]);
        }
    }
    if let Some((q, round, _)) = c.take(&all, far, 0.5, 0.4) {
        // A barrel lying on its side, staved in
        let g = lot.g(q[0], q[1]);
        let (a, b) = ([q[0] - round[0] * 0.3, q[1] - round[1] * 0.3, g + 0.26], [q[0] + round[0] * 0.3, q[1] + round[1] * 0.3, g + 0.26]);
        lot.put(Shape::capsule(a, b, 0.27, 0.27, DEAD_WOOD, BARK_DARK, true));
        let g2 = lot.g(q[0] + round[0], q[1] + round[1]);
        let c2 = [q[0] + round[0] * 0.9, q[1] + round[1] * 0.9, g2];
        lot.put(Shape::ellipsoid([c2[0], c2[1], c2[2] + 0.02], [0.3, 0.3, 0.03], DARK_STEEL, AIR));
    }
    if let Some((q, round, _)) = c.take(&all, (far - 0.3).max(1.2), 0.6, 0.2) {
        // A blade rusted where it was dropped
        let g = lot.g(q[0], q[1]);
        let (a, b) = ([q[0] - round[0] * 0.5, q[1] - round[1] * 0.5, g + 0.05], [q[0] + round[0] * 0.35, q[1] + round[1] * 0.35, g + 0.05]);
        lot.put(Shape::capsule(a, b, 0.035, 0.012, DARK_STEEL, CLAY_BANK, false));
        let hilt = [q[0] + round[0] * 0.45, q[1] + round[1] * 0.45, g + 0.05];
        let across = [-round[1], round[0]];
        lot.stick([hilt[0] - across[0] * 0.15, hilt[1] - across[1] * 0.15, hilt[2]], [hilt[0] + across[0] * 0.15, hilt[1] + across[1] * 0.15, hilt[2]], 0.025, DARK_STEEL);
        lot.stick(hilt, [hilt[0] + round[0] * 0.2, hilt[1] + round[1] * 0.2, hilt[2]], 0.025, LEATHER);
    }
    if let Some((q, ..)) = c.take(&all, lot.rng.range(1.1, far), 0.2, 0.2) {
        let g = lot.g(q[0], q[1]);
        c.skull(q[0], q[1], g, lot.rng.next() * TAU);
    }
    if chest && let Some((q, ..)) = c.take(&all, c.room - 0.55, 0.45, 0.4) {
        let g = lot.g(q[0], q[1]);
        c.chest(q, g, 2.4);
    }
}

/// A camp left with its fire burning: a pot on it, a bedroll, a log to sit on, a few supplies,
/// some wood, and a chest
fn nobody(lot: &mut Lot, c: &mut Round, chest: bool) {
    let (x, y) = (c.at.x, c.at.y);
    bonfire(lot, x, y, 0.8);
    if c.room >= 2.0 && lot.rng.next() < 0.6 {
        lot.kettle(x, y, 1.2);
    }
    let flip = lot.rng.next() < 0.5;
    let all = order(SLOTS / 2, flip);
    let near = (c.room - 0.2).min(1.7);
    if let Some((q, round, _)) = c.take(&[3, 5, 2, 6], near, 0.7, 0.2) {
        lot.seat(q[0], q[1], round);
    }
    let far = c.room - 0.45;
    if let Some((q, round, _)) = c.take(&all, far, 0.95, 0.4) {
        let color = [LINEN, FUR_BROWN, LEATHER][(lot.rng.next() * 2.99) as usize];
        lot.bedroll(q[0], q[1], round, color);
    }
    if let Some((q, ..)) = c.take(&all, far, 0.5, 0.5) {
        lot.stash(q[0], q[1], 2);
    }
    if c.room >= 2.4
        && let Some((q, round, _)) = c.take(&all, far, 0.7, 0.4)
    {
        lot.woodpile(q[0], q[1], round);
    }
    if let Some((q, ..)) = c.take(&all, far, 0.3, 0.3) {
        let g = lot.g(q[0], q[1]);
        lot.pot(q[0], q[1], g, 1.0);
    }
    if chest && let Some((q, ..)) = c.take(&all, c.room - 0.55, 0.45, 0.4) {
        let g = lot.g(q[0], q[1]);
        c.chest(q, g, 2.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The caves of the first few hundred cells round the start of seed 1, and the camps in them
    fn caves_and_camps(seed: u32) -> Vec<(Cave, Vec<Built>)> {
        let settings = TerrainSettings { width_m: 32, height_m: 64, voxels_per_meter: 4, seed, grass_tufts: false };
        let terrain = Terrain::around(&settings, [0.0, 0.0], [32.0, 32.0]);
        (0..400)
            .filter_map(|c| terrain.cave_in(c % 20 - 10, c / 20 - 10))
            .map(|cave| {
                let built = cave.camps.iter().filter_map(|spot| cave_camp(&settings, &cave.shapes, spot)).collect();
                (cave, built)
            })
            .collect()
    }

    fn is_orb(s: &Shape) -> bool {
        s.material == CRYSTAL && s.a == s.b && s.radius_a == s.radius_b && s.flat.is_none() && s.boxed.is_none()
    }

    /// No crystal orbs lie on the floors of caves any more, nor light them: only torches, the
    /// crystals hanging from the roofs of caverns no camp's in, and the camps' fires
    #[test]
    fn caves_have_no_orbs_left() {
        let caves = caves_and_camps(1);
        let roofs: Vec<[f32; 3]> = [[0.35f32, 0.85, 1.0], [0.7, 0.45, 1.0], [0.4, 1.0, 0.7]].iter().map(|t| t.map(|v| v * 0.6 + 0.4)).collect();
        for (cave, built) in &caves {
            assert!(!cave.shapes.iter().any(is_orb), "an orb of crystal in a cave");
            for l in &cave.lights {
                assert!(l.color == crate::worldgen::TORCH_LIGHT || roofs.contains(&l.color), "a light of {:?} in a cave", l.color);
            }
            for b in built {
                assert!(!b.shapes.iter().any(|s| s.material == CRYSTAL), "crystal in a camp");
                let fire = if b.camp.folk == Folk::Dead { GHOST_FIRE_LIGHT } else { CAMPFIRE_LIGHT };
                assert_eq!(b.lights.len(), 1, "a camp lit by more than its fire");
                assert_eq!(b.lights[0].color, fire);
                assert!(b.lights[0].reach <= 13.5);
            }
        }
    }

    /// Each camp's fire sits on the floor of its cave, the rock right under it, with room to stand
    /// over it and level ground all round, everything of it in the hollow, and no two in a cave
    /// close together; there are camps of all three kinds, about as many of each as rolled
    #[test]
    fn camps_sit_on_cave_floors_with_room_over_them() {
        let mut counts = [0usize; 3];
        let mut spots = 0;
        for seed in [1, 2, 3] {
            for (cave, built) in caves_and_camps(seed) {
                spots += cave.camps.len();
                let h = Hollows { near: cave.shapes.iter().filter(|s| s.placement == Placement::Carve).collect() };
                for b in &built {
                    let c = b.camp;
                    let [x, y, z] = c.fire;
                    assert!(h.open([x, y, z + 0.05]) && !h.open([x, y, z - 0.1]), "a fire not on the floor at {:?}", c.fire);
                    assert!(h.open([x, y, z + 2.5]), "no room over the fire at {:?}", c.fire);
                    assert!(c.room >= LEAST_ROOM, "{} m of room", c.room);
                    for k in 0..12 {
                        let a = k as f32 * TAU / 12.0;
                        assert!(h.level(x + a.cos() * LEAST_ROOM, y + a.sin() * LEAST_ROOM, z), "uneven or low round the fire at {:?}", c.fire);
                    }
                    assert!(b.shapes.iter().all(|s| s.placement == Placement::Inside), "something of a camp not in the hollow");
                    for p in &b.props {
                        assert!((p.position[0] - x).hypot(p.position[1] - y) <= c.room + 0.05, "a {:?} {} m out", p.kind, (p.position[0] - x).hypot(p.position[1] - y));
                        assert!(h.open([p.position[0], p.position[1], p.position[2] + 0.3]), "a {:?} in the rock", p.kind);
                    }
                    counts[c.folk as usize] += 1;
                }
                for (i, a) in built.iter().enumerate() {
                    for b in &built[i + 1..] {
                        let d = (0..3).map(|k| (a.camp.fire[k] - b.camp.fire[k]).powi(2)).sum::<f32>().sqrt();
                        assert!(d > 35.0, "two camps {d} m apart in a cave");
                    }
                }
            }
        }
        let total: usize = counts.iter().sum();
        println!("{total} camps of {spots} spots: {} bandits', {} the dead's, {} left", counts[0], counts[1], counts[2]);
        // A cavern a camp was planned for has no crystals on its roof nor spiders: it has its camp
        assert!(total * 20 >= spots * 19, "only {total} of {spots} camps built");
        assert!(total >= 12, "only {total} camps");
        let share = |n: usize| n as f32 / total as f32;
        assert!((share(counts[0]) - BANDITS).abs() < 0.12 && (share(counts[1]) - DEAD).abs() < 0.12, "{counts:?}");
        assert!(share(counts[2]) > 0.15, "{counts:?}");
    }

    /// Who keeps each camp, and all of it, goes by where it is and the seed alone: built again, or
    /// for another piece of the landscape, it's the same. Bandits keep two to four, the dead and
    /// those left have a chest of their own, and no bandits keep any in a land no one lives in.
    #[test]
    fn camps_are_the_same_every_time_and_kept_as_rolled() {
        let settings = TerrainSettings { width_m: 32, height_m: 64, voxels_per_meter: 4, seed: 1, grass_tufts: false };
        let here = Terrain::around(&settings, [0.0, 0.0], [32.0, 32.0]);
        let there = Terrain::around(&settings, [500.0, -300.0], [532.0, -268.0]);
        let mut seen = 0;
        for c in 0..400 {
            let (cx, cy) = (c % 20 - 10, c / 20 - 10);
            let (Some(a), Some(b)) = (here.cave_in(cx, cy), there.cave_in(cx, cy)) else {
                continue;
            };
            assert_eq!(a.camps.len(), b.camps.len());
            for (s, t) in a.camps.iter().zip(&b.camps) {
                let (Some(p), Some(q)) = (cave_camp(&settings, &a.shapes, s), cave_camp(&settings, &b.shapes, t)) else {
                    continue;
                };
                assert_eq!(p.camp.fire, q.camp.fire);
                assert_eq!(p.camp.folk, q.camp.folk);
                assert_eq!(p.shapes.len(), q.shapes.len());
                assert_eq!(p.props.len(), q.props.len());
                match p.camp.folk {
                    Folk::Bandits => {
                        assert!((2..=4).contains(&(p.camp.bandits + p.camp.brutes)));
                        assert!((p.camp.chest[0] - p.camp.fire[0]).hypot(p.camp.chest[1] - p.camp.fire[1]) < p.camp.room + 0.1);
                        assert!(!p.props.iter().any(|q| q.kind == PropKind::Chest), "bandits' hoard out before they're beaten");
                    }
                    _ => assert!(p.props.iter().filter(|q| q.kind == PropKind::Chest).count() <= 1),
                }
                seen += 1;
            }
        }
        assert!(seen > 0);
        // The rolls themselves
        assert_eq!(Folk::of(0.0, 5.0, true), Folk::Bandits);
        assert_eq!(Folk::of(0.0, 5.0, false), Folk::Dead);
        assert_eq!(Folk::of(0.999, 30.0, true), Folk::Nobody);
        let deep = (0..1000).filter(|&k| Folk::of(k as f32 / 1000.0, 30.0, true) == Folk::Dead).count();
        let shallow = (0..1000).filter(|&k| Folk::of(k as f32 / 1000.0, 5.0, true) == Folk::Dead).count();
        assert!(deep > shallow, "the dead keep no more of the deep camps");
        let all = (0..1000).filter(|&k| Folk::of(k as f32 / 1000.0, 5.0, false) == Folk::Bandits).count();
        assert_eq!(all, 0);
    }
}
