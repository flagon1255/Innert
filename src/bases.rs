//! Bases: what the player builds to see out the night in, from what they gather. Wood comes from
//! felling trees, stone from rock and clay from the banks of rivers and lakes and bands in the rock,
//! see gather.rs. Building is in blocks half a meter across, each one brick of the world's voxels,
//! so putting one in or taking it out sends the GPU a brick or two: walls of wood, clay or stone,
//! floors, stairs, doors and torches. Each costs what it's made of, and taking down one's own gives
//! it back.
//!
//! What's built and what's been dug or felled is kept as edits to the bricks of the whole landscape,
//! see Edit: each strip of the world is built with them as it comes in, see stream.rs, so a base is
//! still there however far the player goes and comes back, and they go into the save.
//!
//! Walls stand up to most of what comes at night: only the biggest monsters can break them down.
//! Doors are the weak point, see raids.rs.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::worldgen::{BRICK_WORDS, EMPTY_BRICK, SOLID_BRICK, World, material::*};

/// Voxels along each side of a block, as of a brick
pub const BLOCK: i64 = 8;
const BLOCK_VOXELS: usize = 512;

/// What the player has gathered to build with
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Materials {
    pub wood: u32,
    pub stone: u32,
    pub clay: u32,
}

impl Materials {
    pub const fn new(wood: u32, stone: u32, clay: u32) -> Self {
        Materials { wood, stone, clay }
    }

    pub fn covers(&self, cost: Materials) -> bool {
        self.wood >= cost.wood && self.stone >= cost.stone && self.clay >= cost.clay
    }

    pub fn take(&mut self, cost: Materials) -> bool {
        if !self.covers(cost) {
            return false;
        }
        self.wood -= cost.wood;
        self.stone -= cost.stone;
        self.clay -= cost.clay;
        true
    }

    pub fn add(&mut self, more: Materials) {
        self.wood = self.wood.saturating_add(more.wood);
        self.stone = self.stone.saturating_add(more.stone);
        self.clay = self.clay.saturating_add(more.clay);
    }

    /// As much of it as `share`, 0-1, of it, rounded down: what's given back for something taken
    /// down damaged
    pub fn share(&self, share: f64) -> Materials {
        let s = |n: u32| (n as f64 * share.clamp(0.0, 1.0) + 1e-9).floor() as u32;
        Materials::new(s(self.wood), s(self.stone), s(self.clay))
    }

    pub fn is_empty(&self) -> bool {
        self.wood == 0 && self.stone == 0 && self.clay == 0
    }

    /// Like "2 wood, 1 clay"
    pub fn describe(&self) -> String {
        let parts: Vec<String> = [(self.wood, "wood"), (self.stone, "stone"), (self.clay, "clay")].iter().filter(|(n, _)| *n > 0).map(|(n, name)| format!("{n} {name}")).collect();
        if parts.is_empty() { "nothing".to_string() } else { parts.join(", ") }
    }
}

/// What can be built
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    WoodWall,
    ClayWall,
    StoneWall,
    /// The top half of a block, to stand on and to put things on
    Floor,
    /// Two steps of a quarter meter
    Stairs,
    /// A meter wide and two high, opened and shut with E: the weak point raiders go for
    Door,
    Torch,
    /// A clay lamp, brighter than a torch
    Lantern,
}

impl Kind {
    /// In the order they're picked from
    pub const ALL: [Kind; 8] = [Kind::WoodWall, Kind::ClayWall, Kind::StoneWall, Kind::Floor, Kind::Stairs, Kind::Door, Kind::Torch, Kind::Lantern];

    pub fn name(self) -> &'static str {
        match self {
            Kind::WoodWall => "Wooden Wall",
            Kind::ClayWall => "Clay Brick Wall",
            Kind::StoneWall => "Stone Wall",
            Kind::Floor => "Wooden Floor",
            Kind::Stairs => "Wooden Stairs",
            Kind::Door => "Door",
            Kind::Torch => "Torch",
            Kind::Lantern => "Clay Lantern",
        }
    }

    /// What it costs to build, and what's given back for taking it down whole
    pub fn cost(self) -> Materials {
        match self {
            Kind::WoodWall => Materials::new(1, 0, 0),
            Kind::ClayWall => Materials::new(0, 0, 1),
            Kind::StoneWall => Materials::new(0, 2, 0),
            Kind::Floor => Materials::new(1, 0, 0),
            Kind::Stairs => Materials::new(1, 0, 0),
            Kind::Door => Materials::new(4, 0, 0),
            Kind::Torch => Materials::new(1, 0, 0),
            Kind::Lantern => Materials::new(1, 0, 1),
        }
    }

    /// How much it takes to break it down: stone walls the most, doors the least of what keeps
    /// raiders out
    pub fn health(self) -> f64 {
        match self {
            Kind::WoodWall => 300.0,
            Kind::ClayWall => 550.0,
            Kind::StoneWall => 1000.0,
            Kind::Floor => 200.0,
            Kind::Stairs => 220.0,
            Kind::Door => 160.0,
            Kind::Torch => 40.0,
            Kind::Lantern => 60.0,
        }
    }

    /// Whether ordinary monsters can harm it at all: they go for doors, and knock over lights, but
    /// only the biggest can break walls
    pub fn weak(self) -> bool {
        matches!(self, Kind::Door | Kind::Torch | Kind::Lantern)
    }

    /// Blocks across and high: a door's two by four, all else one
    pub fn size(self) -> (i64, i64) {
        if self == Kind::Door { (2, 4) } else { (1, 1) }
    }

    /// The light it gives off, if any: its color, and how far it reaches in meters
    pub fn light(self) -> Option<([f32; 3], f32)> {
        match self {
            Kind::Torch => Some(([1.0, 0.55, 0.22], 8.0)),
            Kind::Lantern => Some(([1.0, 0.72, 0.42], 11.0)),
            _ => None,
        }
    }

    /// Whether it turns, so it matters which way it's put: stairs go up one way, a door spans one
    pub fn turns(self) -> bool {
        matches!(self, Kind::Stairs | Kind::Door)
    }

    /// What it's mostly made of, for its debris
    pub fn material(self) -> u8 {
        match self {
            Kind::ClayWall | Kind::Lantern => CLAY_POT,
            Kind::StoneWall => BRICK_STONE,
            _ => WOOD,
        }
    }
}

/// Something built, in blocks of the whole landscape: a block is a brick of its voxels, so block
/// `at` is voxels `at * 8` to `at * 8 + 7`
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Piece {
    pub id: u32,
    pub kind: Kind,
    /// Its first block, the bottom one at its hinge for a door
    pub at: [i64; 3],
    /// Quarter turns round from facing +x: which way stairs go up, and for a door which way it
    /// faces, as it spans the blocks across that
    pub turn: u8,
    pub health: f64,
    /// For a door, whether it's open
    #[serde(default)]
    pub open: bool,
}

/// The way a quarter turn faces, along x or y
pub fn facing(turn: u8) -> [i64; 2] {
    match turn % 4 {
        0 => [1, 0],
        1 => [0, 1],
        2 => [-1, 0],
        _ => [0, -1],
    }
}

/// Across from facing `turn`, a quarter turn on
fn across(turn: u8) -> [i64; 2] {
    facing(turn + 1)
}

impl Piece {
    pub fn new(id: u32, kind: Kind, at: [i64; 3], turn: u8) -> Self {
        Piece { id, kind, at, turn: if kind.turns() { turn % 4 } else { 0 }, health: kind.health(), open: false }
    }

    /// The blocks it takes up, the first `at`
    pub fn blocks(&self) -> Vec<[i64; 3]> {
        let (wide, high) = self.kind.size();
        let side = across(self.turn);
        let mut blocks = Vec::new();
        for k in 0..high {
            for i in 0..wide {
                blocks.push([self.at[0] + side[0] * i, self.at[1] + side[1] * i, self.at[2] + k]);
            }
        }
        blocks
    }

    /// How damaged it looks, 0 to 3, for cracks
    pub fn damage_stage(&self) -> u8 {
        let left = self.health / self.kind.health();
        if left > 0.75 {
            0
        } else if left > 0.5 {
            1
        } else if left > 0.25 {
            2
        } else {
            3
        }
    }

    /// What's given back for taking it down: all of it whole, less for the damage done
    pub fn refund(&self) -> Materials {
        let left = (self.health / self.kind.health()).clamp(0.0, 1.0);
        if left > 0.999 { self.kind.cost() } else { self.kind.cost().share(left) }
    }

    /// Where its light is, in voxels of the whole landscape, if it gives off any
    pub fn light_at(&self) -> Option<[f64; 3]> {
        self.kind.light()?;
        let b = self.at.map(|v| (v * BLOCK) as f64);
        Some([b[0] + 4.0, b[1] + 4.0, b[2] + if self.kind == Kind::Torch { 6.5 } else { 4.5 }])
    }

    /// The middle of it, in voxels of the whole landscape
    pub fn middle(&self) -> [f64; 3] {
        let blocks = self.blocks();
        let n = blocks.len() as f64;
        let mut sum = [0.0; 3];
        for b in &blocks {
            for i in 0..3 {
                sum[i] += (b[i] * BLOCK) as f64 + 4.0;
            }
        }
        sum.map(|s| s / n)
    }

    /// The voxels it puts in block `index` of its blocks(), AIR where it has none
    pub fn voxels(&self, index: usize) -> [u8; BLOCK_VOXELS] {
        let blocks = self.blocks();
        let block = blocks[index];
        let mut out = [AIR; BLOCK_VOXELS];
        let (wide, _) = self.kind.size();
        let column = index as i64 % wide;
        let row = index as i64 / wide;
        let stage = self.damage_stage();
        let face = facing(self.turn);
        let side = across(self.turn);
        for z in 0..8i64 {
            for y in 0..8i64 {
                for x in 0..8i64 {
                    // Where it is in the whole landscape, for what's picked at random to stay put
                    let g = [block[0] * 8 + x, block[1] * 8 + y, block[2] * 8 + z];
                    let r = hash3(g[0] as u32, g[1] as u32, g[2] as u32);
                    // Along the way it faces and across it, 0-7
                    let along = |d: [i64; 2]| -> i64 {
                        let v = x * d[0] + y * d[1];
                        if d[0] + d[1] < 0 { v + 7 } else { v }
                    };
                    let m = match self.kind {
                        Kind::StoneWall => {
                            // Courses of stone a quarter meter high, the joints between them
                            // staggered, each stone a little different
                            let course = (g[2]).div_euclid(4);
                            let shift = if course % 2 == 0 { 0 } else { 4 };
                            let joint = z % 4 == 0 || (x + shift) % 8 == 0 || (y + shift) % 8 == 0;
                            if joint && r % 7 != 0 {
                                MORTAR
                            } else {
                                let stone = hash3((g[0] + shift).div_euclid(8) as u32, (g[1] + shift).div_euclid(8) as u32, course as u32);
                                [BRICK_STONE, BRICK_STONE, STONE, LIMESTONE][(stone % 4) as usize]
                            }
                        }
                        Kind::ClayWall => {
                            // Bricks three voxels high and seven long, laid in courses
                            let course = (g[2]).div_euclid(3);
                            let shift = if course % 2 == 0 { 0 } else { 3 };
                            let joint = g[2].rem_euclid(3) == 0 || (g[0] + shift).rem_euclid(7) == 0 || (g[1] + shift).rem_euclid(7) == 0;
                            if joint {
                                MORTAR
                            } else {
                                let brick = hash3((g[0] + shift).div_euclid(7) as u32, (g[1] + shift).div_euclid(7) as u32, course as u32 ^ 0xc1a7);
                                [CLAY_POT, CLAY_POT, CLAY_POT, SANDSTONE_DARK][(brick % 4) as usize]
                            }
                        }
                        Kind::WoodWall => {
                            // Planks laid flat, a quarter meter wide, with dark seams, and a post at
                            // each corner of every block
                            let plank = g[2].div_euclid(4);
                            if g[2].rem_euclid(4) == 0 && r % 5 != 0 {
                                BARK_DARK
                            } else {
                                let kind = hash3(block[0] as u32, block[1] as u32, plank as u32 ^ 0x9a1d);
                                [WOOD, WOOD, WOOD, PALM_WOOD, DEAD_WOOD][(kind % 5) as usize]
                            }
                        }
                        // The top half of its block, so what's put on it in the block over it stands
                        // right on it
                        Kind::Floor => {
                            if z < 4 {
                                AIR
                            } else if (x * side[0] + y * side[1]).rem_euclid(4) == 0 && z == 7 {
                                BARK_DARK
                            } else {
                                [WOOD, WOOD, PALM_WOOD][(hash3(block[0] as u32, block[1] as u32, ((x * side[0] + y * side[1]).div_euclid(4)) as u32) % 3) as usize]
                            }
                        }
                        Kind::Stairs => {
                            // The first step a quarter meter high all over, the second over the
                            // half of it further up the way it goes
                            let up = along(face);
                            if z < 4 || up >= 4 {
                                if z == 3 || z == 7 { BARK_DARK } else { WOOD }
                            } else {
                                AIR
                            }
                        }
                        Kind::Door => door_voxel(self, column, row, x, y, z, stage, r),
                        Kind::Torch => {
                            let (cx, cy) = ((x - 3).abs().min((x - 4).abs()), (y - 3).abs().min((y - 4).abs()));
                            let middle = (3..=4).contains(&x) && (3..=4).contains(&y);
                            if middle && z < 5 {
                                if z == 4 { BARK_DARK } else { WOOD }
                            } else if middle && (5..=6).contains(&z) || cx == 0 && cy == 0 && z == 7 && r % 2 == 0 {
                                FLAME
                            } else {
                                AIR
                            }
                        }
                        Kind::Lantern => {
                            let inner = (2..=5).contains(&x) && (2..=5).contains(&y);
                            let core = (3..=4).contains(&x) && (3..=4).contains(&y);
                            if inner && z < 2 {
                                CLAY_POT
                            } else if core && (2..=4).contains(&z) {
                                FLAME
                            } else if inner && (2..=5).contains(&z) && (x == 2 || x == 5) && (y == 2 || y == 5) {
                                DARK_STEEL
                            } else if inner && z == 6 {
                                CLAY_POT
                            } else if core && z == 7 {
                                DARK_STEEL
                            } else {
                                AIR
                            }
                        }
                    };
                    // Cracked as it's damaged: some voxels darkened, the worst knocked out
                    let m = if m != AIR && stage > 0 && self.kind != Kind::Door {
                        let crack = hash3(g[0] as u32 ^ 0x5eed, g[1] as u32, g[2] as u32) % 100;
                        if crack < stage as u32 * 4 && stage >= 2 && is_surface(x, y, z) {
                            AIR
                        } else if crack < stage as u32 * 12 {
                            if self.kind == Kind::StoneWall || self.kind == Kind::ClayWall { GRAVEL } else { DEAD_WOOD }
                        } else {
                            m
                        }
                    } else {
                        m
                    };
                    out[(x + y * 8 + z * 64) as usize] = m;
                }
            }
        }
        out
    }
}

fn is_surface(x: i64, y: i64, z: i64) -> bool {
    x == 0 || y == 0 || z == 0 || x == 7 || y == 7 || z == 7
}

/// A voxel of a door's panel: planks up and down, braced across, with iron hinges on one side and
/// a ring on the other. Shut, it stands in the middle of its blocks' thickness; open, it's swung
/// back against its hinge side, so the way through is clear but for a hand's breadth. Damaged, it
/// splits and splinters.
#[allow(clippy::too_many_arguments)]
fn door_voxel(piece: &Piece, column: i64, row: i64, x: i64, y: i64, z: i64, stage: u8, r: u32) -> u8 {
    let face = facing(piece.turn);
    let side = across(piece.turn);
    // Across the door from its hinge, 0-15, and through it, 0-7
    let local_across = |d: [i64; 2]| {
        let v = x * d[0] + y * d[1];
        if d[0] + d[1] < 0 { v + 7 } else { v }
    };
    let a = column * 8 + local_across(side);
    let t = local_across(face);
    let h = row * 8 + z;
    if piece.open {
        // Swung back against the hinge side, edge on
        return if a < 2 && (1..7).contains(&t) && h < 31 { if t % 3 == 0 { BARK_DARK } else { WOOD } } else { AIR };
    }
    if !(3..=5).contains(&t) || h >= 31 {
        return AIR;
    }
    // Split right through where it's worst
    if stage >= 2 {
        let split = (a == 7 || a == 8) && (h + (r % 3) as i64) % 9 < if stage == 3 { 7 } else { 3 };
        let holed = stage == 3 && (r % 100) < 14 && (t == 3 || t == 5);
        if split || holed {
            return AIR;
        }
    }
    let hinge = a <= 1 && (h % 15 == 4 || h % 15 == 5) && t == 5;
    let ring = (a == 13) && (h == 15 || h == 16) && t == 5;
    if hinge || ring {
        return DARK_STEEL;
    }
    let brace = h == 6 || h == 7 || h == 23 || h == 24;
    let seam = a % 4 == 0;
    let cracked = stage >= 1 && (r % 100) < stage as u32 * 10;
    if cracked {
        DEAD_WOOD
    } else if brace {
        BARK_DARK
    } else if seam && t != 4 {
        BARK_DARK
    } else {
        WOOD
    }
}

pub fn hash3(x: u32, y: u32, z: u32) -> u32 {
    let mut h = x.wrapping_mul(0x8da6_b343) ^ y.wrapping_mul(0xd816_3841) ^ z.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^ (h >> 15)
}

/// What's been done to one brick of the whole landscape: voxels taken out of it, by gathering or
/// for what's built there, and what's built in it, by its piece's id and which of its blocks
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Edit {
    pub cleared: [u64; 8],
    #[serde(default)]
    pub piece: Option<(u32, u8)>,
}

impl Edit {
    fn clear(&mut self, i: usize) {
        self.cleared[i / 64] |= 1 << (i % 64);
    }

    /// What was put in goes once it's changed: a door opened, cracks showing, or taken down. In
    /// the world as it is, what was there before isn't known any more, so what replaces it is put
    /// in over nothing, both there and in a strip built afresh, see apply_edit().
    fn retire(&mut self, put: &[u8; BLOCK_VOXELS]) {
        for (v, &m) in put.iter().enumerate() {
            if m != AIR {
                self.clear(v);
            }
        }
    }
}

/// An edit as a strip being built takes it, see stream.rs: which brick, which of its voxels go,
/// and what's put in, AIR where nothing is. `version` is when it last changed, see Bases::version.
#[derive(Clone)]
pub struct BrickEdit {
    pub at: [i64; 3],
    pub cleared: [u64; 8],
    pub put: Option<Box<[u8; BLOCK_VOXELS]>>,
    pub version: u64,
}

/// Every edit as strips take them, shared with the threads building them
pub type Edits = Arc<Vec<BrickEdit>>;

/// What's built in the world the player's in, and what's been dug and felled in it
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WorldBases {
    pub pieces: Vec<Piece>,
    /// By brick of the whole landscape
    pub edits: Vec<([i64; 3], Edit)>,
    pub next_id: u32,
}

/// What's kept of the whole game: what's carried to build with, the time of day, and how many
/// nights were seen out
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GameBases {
    pub materials: Materials,
    pub clock: Option<crate::daynight::Clock>,
    pub nights_survived: u32,
}

/// The save's names for them, see save.rs
pub const WORLD_SECTION: &str = "bases";
pub const GAME_SECTION: &str = "bases";

/// The bases' state, and the edits it's made of
#[derive(Default)]
pub struct Bases {
    pub materials: Materials,
    pieces: Vec<Piece>,
    edits: BTreeMap<[i64; 3], Edit>,
    /// Where each piece is in `pieces`, by id, and which piece has each block
    by_id: HashMap<u32, usize>,
    by_block: HashMap<[i64; 3], u32>,
    next_id: u32,
    /// Goes up with every change, each edit keeping when it last changed, so what changed while a
    /// strip was being built is put in it once it's in, see stream.rs
    pub version: u64,
    changed_at: HashMap<[i64; 3], u64>,
    /// Kept till the next change
    snapshot: RefCell<Option<Edits>>,
    pub nights_survived: u32,
}

impl Bases {
    pub fn new() -> Self {
        Bases { next_id: 1, ..Default::default() }
    }

    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    pub fn piece(&self, id: u32) -> Option<&Piece> {
        self.by_id.get(&id).map(|&i| &self.pieces[i])
    }

    /// The piece with a block at `block`, if any
    pub fn piece_at(&self, block: [i64; 3]) -> Option<&Piece> {
        self.by_block.get(&block).and_then(|id| self.piece(*id))
    }

    /// Whether anything at all's been built or dug
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    fn reindex(&mut self) {
        self.by_id.clear();
        self.by_block.clear();
        for (i, p) in self.pieces.iter().enumerate() {
            self.by_id.insert(p.id, i);
            for b in p.blocks() {
                self.by_block.insert(b, p.id);
            }
        }
    }

    fn touch(&mut self, block: [i64; 3]) {
        self.version += 1;
        self.changed_at.insert(block, self.version);
        self.snapshot.replace(None);
    }

    /// Whether `piece` can go where it says: none of its blocks taken by something else built
    pub fn free_for(&self, piece: &Piece) -> bool {
        piece.blocks().iter().all(|b| !self.by_block.contains_key(b))
    }

    /// Builds `kind` at block `at` facing `turn`, paying for it, if there's room for it. Returns
    /// the blocks that changed.
    pub fn build(&mut self, kind: Kind, at: [i64; 3], turn: u8) -> Result<Vec<[i64; 3]>, &'static str> {
        let piece = Piece::new(self.next_id.max(1), kind, at, turn);
        if !self.free_for(&piece) {
            return Err("Something's built there already.");
        }
        if !self.materials.covers(kind.cost()) {
            return Err("Not enough to build that with.");
        }
        self.materials.take(kind.cost());
        self.next_id = piece.id + 1;
        let blocks = piece.blocks();
        for (k, b) in blocks.iter().enumerate() {
            let edit = self.edits.entry(*b).or_default();
            edit.piece = Some((piece.id, k as u8));
        }
        self.pieces.push(piece);
        self.reindex();
        for b in &blocks {
            self.touch(*b);
        }
        Ok(blocks)
    }

    /// Takes piece `id` down, giving back what it's worth if `refund`, or leaving it in pieces if
    /// it was broken. Returns it, and its blocks.
    pub fn remove(&mut self, id: u32, refund: bool) -> Option<(Piece, Vec<[i64; 3]>)> {
        let i = *self.by_id.get(&id)?;
        let piece = self.pieces.remove(i);
        if refund {
            self.materials.add(piece.refund());
        }
        let blocks = piece.blocks();
        for (k, b) in blocks.iter().enumerate() {
            let voxels = piece.voxels(k);
            let edit = self.edits.entry(*b).or_default();
            edit.piece = None;
            // What it took up goes with it
            edit.retire(&voxels);
        }
        self.reindex();
        for b in &blocks {
            self.touch(*b);
        }
        Some((piece, blocks))
    }

    /// Damages piece `id` by `amount`: returns whether it's broken, and whether how it looks
    /// changed, as cracks; a broken piece is taken down without anything given back
    pub fn damage(&mut self, id: u32, amount: f64) -> Option<(bool, bool, Vec<[i64; 3]>)> {
        let i = *self.by_id.get(&id)?;
        if self.pieces[i].health - amount <= 0.0 {
            let (_, blocks) = self.remove(id, false)?;
            return Some((true, true, blocks));
        }
        let before = self.pieces[i].damage_stage();
        let old = self.looks(i);
        self.pieces[i].health -= amount;
        let changed = self.pieces[i].damage_stage() != before;
        let blocks = self.pieces[i].blocks();
        if changed {
            self.retire(&blocks, old);
        }
        Some((false, changed, blocks))
    }

    /// How piece `i` looks, block by block, before it changes, see retire()
    fn looks(&self, i: usize) -> Vec<[u8; BLOCK_VOXELS]> {
        (0..self.pieces[i].blocks().len()).map(|k| self.pieces[i].voxels(k)).collect()
    }

    /// A piece's `blocks` changed from how they looked, `old`: see Edit::retire()
    fn retire(&mut self, blocks: &[[i64; 3]], old: Vec<[u8; BLOCK_VOXELS]>) {
        for (b, old) in blocks.iter().zip(old) {
            self.edits.entry(*b).or_default().retire(&old);
            self.touch(*b);
        }
    }

    /// Opens or shuts door `id`, returning whether it's open now and its blocks
    pub fn toggle_door(&mut self, id: u32) -> Option<(bool, Vec<[i64; 3]>)> {
        let i = *self.by_id.get(&id)?;
        if self.pieces[i].kind != Kind::Door {
            return None;
        }
        let old = self.looks(i);
        self.pieces[i].open = !self.pieces[i].open;
        let (open, blocks) = (self.pieces[i].open, self.pieces[i].blocks());
        self.retire(&blocks, old);
        Some((open, blocks))
    }

    /// Mends door `id` back to whole if `materials` pay for it: half what it cost
    pub fn repair_cost(&self, id: u32) -> Option<Materials> {
        let piece = self.piece(id)?;
        (piece.health < piece.kind.health()).then(|| piece.kind.cost().share(0.5))
    }

    pub fn repair(&mut self, id: u32) -> Result<Vec<[i64; 3]>, &'static str> {
        let cost = self.repair_cost(id).ok_or("It's not damaged.")?;
        if !self.materials.take(cost) {
            return Err("Not enough to mend it with.");
        }
        let i = self.by_id[&id];
        let old = self.looks(i);
        self.pieces[i].health = self.pieces[i].kind.health();
        let blocks = self.pieces[i].blocks();
        self.retire(&blocks, old);
        Ok(blocks)
    }

    /// Voxels `voxels` of brick `block` are taken out of the world for good, gathered or felled
    pub fn clear_voxels(&mut self, block: [i64; 3], voxels: &[usize]) {
        if voxels.is_empty() {
            return;
        }
        let edit = self.edits.entry(block).or_default();
        for &v in voxels {
            edit.clear(v);
        }
        self.touch(block);
    }

    /// What brick `block` ends up as, from what's in it to start with: what's been taken out goes,
    /// and what's built there goes in
    pub fn apply_to(&self, block: [i64; 3], materials: &mut [u8; BLOCK_VOXELS]) {
        let Some(edit) = self.edits.get(&block) else {
            return;
        };
        let put = edit.piece.and_then(|(id, k)| Some(self.piece(id)?.voxels(k as usize)));
        apply_edit(&edit.cleared, put.as_ref(), materials);
    }

    /// Every edit as strips take them, see BrickEdit, kept till something changes
    pub fn edits(&self) -> Edits {
        if let Some(edits) = self.snapshot.borrow().as_ref() {
            return edits.clone();
        }
        let edits: Vec<BrickEdit> = self
            .edits
            .iter()
            .map(|(at, edit)| BrickEdit {
                at: *at,
                cleared: edit.cleared,
                put: edit.piece.and_then(|(id, k)| Some(Box::new(self.piece(id)?.voxels(k as usize)))),
                version: self.changed_at.get(at).copied().unwrap_or(0),
            })
            .collect();
        let edits = Arc::new(edits);
        self.snapshot.replace(Some(edits.clone()));
        edits
    }

    /// The lights of what's built, in voxels of the whole landscape: where, how far they reach in
    /// voxels, and their color
    pub fn lights(&self, vpm: f64) -> Vec<([f64; 3], f32, [f32; 3])> {
        self.pieces.iter().filter_map(|p| {
            let (color, reach) = p.kind.light()?;
            Some((p.light_at()?, (reach as f64 * vpm) as f32, color))
        }).collect()
    }

    /// What goes in the save of the world: everything built and done in it
    pub fn world_save(&self) -> WorldBases {
        WorldBases { pieces: self.pieces.clone(), edits: self.edits.iter().map(|(a, e)| (*a, e.clone())).collect(), next_id: self.next_id }
    }

    /// Takes up what was saved of the world, in place of what there was
    pub fn load_world(&mut self, saved: WorldBases) {
        self.pieces = saved.pieces;
        self.edits = saved.edits.into_iter().collect();
        self.next_id = saved.next_id.max(self.pieces.iter().map(|p| p.id + 1).max().unwrap_or(1));
        self.changed_at.clear();
        self.version += 1;
        self.snapshot.replace(None);
        self.reindex();
    }

    /// The same for the whole game
    pub fn game_save(&self, clock: crate::daynight::Clock) -> GameBases {
        GameBases { materials: self.materials, clock: Some(clock), nights_survived: self.nights_survived }
    }

    pub fn load_game(&mut self, saved: &GameBases) {
        self.materials = saved.materials;
        self.nights_survived = saved.nights_survived;
    }
}

/// Takes `cleared` out of a brick's `materials`, and puts `put` in
pub fn apply_edit(cleared: &[u64; 8], put: Option<&[u8; BLOCK_VOXELS]>, materials: &mut [u8; BLOCK_VOXELS]) {
    for (i, m) in materials.iter_mut().enumerate() {
        if cleared[i / 64] >> (i % 64) & 1 != 0 {
            *m = AIR;
        }
        if let Some(put) = put
            && put[i] != AIR
        {
            *m = put[i];
        }
    }
}

/// The brick `b` of `world`, counted from its first, which has to be in it: every voxel's material
pub fn read_brick(world: &World, b: [i64; 3]) -> [u8; BLOCK_VOXELS] {
    let mut out = [AIR; BLOCK_VOXELS];
    let value = world.brick_grid[world.cell(b)];
    if value & EMPTY_BRICK != 0 {
        return out;
    }
    if value & SOLID_BRICK != 0 {
        return [value as u8; BLOCK_VOXELS];
    }
    if let Some((_, words)) = world.brick_words(world.cell(b)) {
        debug_assert_eq!(words.len(), BRICK_WORDS);
        for (i, m) in out.iter_mut().enumerate() {
            *m = (words[16 + i / 4] >> ((i % 4) * 8)) as u8;
        }
    }
    out
}

/// Where brick `block` of the whole landscape is in `world`, counted from its first, if it's in it
pub fn local_block(world: &World, block: [i64; 3]) -> Option<[i64; 3]> {
    let b = [block[0] - world.origin[0].div_euclid(BLOCK), block[1] - world.origin[1].div_euclid(BLOCK), block[2] - world.origin[2].div_euclid(BLOCK)];
    let size = world.size.map(|n| n as i64 / BLOCK);
    (0..3).all(|i| b[i] >= 0 && b[i] < size[i]).then_some(b)
}

/// Puts `edits` into `world`, those of them that changed after `since` and are in the columns
/// `clip` says, if it does, counted from the world's first voxel. Returns the cells that changed,
/// and which bricks, counted from the world's first, aren't empty now that were: what empty bricks
/// round them skip has to be made to stop short of them, see stop_skips().
pub fn apply_edits(world: &mut World, edits: &[BrickEdit], since: Option<u64>, clip: Option<([i64; 2], [i64; 2])>) -> (Vec<usize>, Vec<[i64; 3]>) {
    let mut cells = Vec::new();
    let mut filled = Vec::new();
    for edit in edits {
        if since.is_some_and(|v| edit.version <= v) {
            continue;
        }
        let Some(b) = local_block(world, edit.at) else {
            continue;
        };
        if let Some((lo, hi)) = clip
            && !(0..2).all(|i| b[i] * BLOCK >= lo[i] && b[i] * BLOCK < hi[i])
        {
            continue;
        }
        let was_empty = world.brick_grid[world.cell(b)] & EMPTY_BRICK != 0;
        let mut materials = read_brick(world, b);
        apply_edit(&edit.cleared, edit.put.as_deref(), &mut materials);
        cells.push(world.set_brick(b, &materials));
        if was_empty && materials.iter().any(|&m| m != AIR) {
            filled.push(b);
        }
    }
    (cells, filled)
}

/// Bricks that have just stopped being empty, `filled`, counted from the world's first: every empty
/// brick within `reach` of them is made to skip no further than they are, see EMPTY_BRICK, so no
/// ray skips past them. Empty bricks further off than `reach` mustn't skip further than that. Done
/// for each group of them at a time, as near as the box round the group. Returns the cells changed.
pub fn stop_skips(world: &mut World, filled: &[[i64; 3]], reach: i64) -> Vec<usize> {
    let mut changed = Vec::new();
    for (lo, hi) in groups(filled) {
        let size = world.size.map(|n| n as i64 / BLOCK);
        let from = [0, 1, 2].map(|i| (lo[i] - reach).max(0));
        let to = [0, 1, 2].map(|i| (hi[i] + reach).min(size[i]));
        for z in from[2]..to[2] {
            for y in from[1]..to[1] {
                for x in from[0]..to[0] {
                    let c = world.cell([x, y, z]);
                    let cell = world.brick_grid[c];
                    if cell & EMPTY_BRICK == 0 {
                        continue;
                    }
                    let off = [x, y, z];
                    let d = (0..3).map(|i| (lo[i] - off[i]).max(off[i] - (hi[i] - 1)).max(0)).max().unwrap_or(0).max(1) as u32;
                    if d < cell & 0xFFFF {
                        world.brick_grid[c] = (cell & !0xFFFF) | d;
                        changed.push(c);
                    }
                }
            }
        }
    }
    changed
}

/// Bricks put in near one another, gathered into boxes: from the first brick to one past the last
pub fn groups(bricks: &[[i64; 3]]) -> Vec<([i64; 3], [i64; 3])> {
    const CELL: i64 = 16;
    let mut boxes: BTreeMap<[i64; 2], ([i64; 3], [i64; 3])> = BTreeMap::new();
    for b in bricks {
        let key = [b[0].div_euclid(CELL), b[1].div_euclid(CELL)];
        let e = boxes.entry(key).or_insert((*b, [b[0] + 1, b[1] + 1, b[2] + 1]));
        for i in 0..3 {
            e.0[i] = e.0[i].min(b[i]);
            e.1[i] = e.1[i].max(b[i] + 1);
        }
    }
    boxes.into_values().collect()
}


#[cfg(test)]
mod tests {
    use super::*;

    // Building pays for it, and taking it down gives it back, all of it whole and less damaged
    #[test]
    fn costs_and_refunds() {
        let mut bases = Bases::new();
        assert!(bases.build(Kind::StoneWall, [0, 0, 0], 0).is_err(), "nothing to build with");
        bases.materials = Materials::new(10, 3, 2);
        let blocks = bases.build(Kind::StoneWall, [0, 0, 0], 0).unwrap();
        assert_eq!(blocks, vec![[0, 0, 0]]);
        assert_eq!(bases.materials, Materials::new(10, 1, 2));
        assert!(bases.build(Kind::WoodWall, [0, 0, 0], 0).is_err(), "something's built there");
        let door = bases.build(Kind::Door, [1, 0, 0], 3).unwrap();
        assert_eq!(door.len(), 8);
        assert_eq!(bases.materials.wood, 6);
        let id = bases.piece_at([1, 0, 3]).unwrap().id;
        // Taken down whole, everything back
        let (piece, _) = bases.remove(id, true).unwrap();
        assert_eq!(piece.kind, Kind::Door);
        assert_eq!(bases.materials, Materials::new(10, 1, 2));
        assert!(bases.piece_at([1, 0, 0]).is_none());
        // Taken down half broken, half back, rounded down
        let wall = bases.piece_at([0, 0, 0]).unwrap().id;
        bases.damage(wall, Kind::StoneWall.health() * 0.5).unwrap();
        bases.remove(wall, true).unwrap();
        assert_eq!(bases.materials.stone, 2);
        // Every piece costs something, and only of wood, stone and clay
        for kind in Kind::ALL {
            assert!(!kind.cost().is_empty(), "{kind:?} is free");
        }
        // Stone's the strongest, and doors the weak point
        assert!(Kind::StoneWall.health() > Kind::ClayWall.health() && Kind::ClayWall.health() > Kind::WoodWall.health());
        assert!(Kind::Door.health() < Kind::WoodWall.health() && Kind::Door.weak() && !Kind::StoneWall.weak());
    }

    // Doors take blows, crack, and break, leaving nothing behind and giving nothing back
    #[test]
    fn doors_break() {
        let mut bases = Bases::new();
        bases.materials = Materials::new(4, 0, 0);
        bases.build(Kind::Door, [5, 5, 2], 0).unwrap();
        let id = bases.piece_at([5, 5, 2]).unwrap().id;
        let whole = bases.piece(id).unwrap().voxels(0);
        let (broken, changed, _) = bases.damage(id, 50.0).unwrap();
        assert!(!broken && changed, "a quarter and more off shows");
        let cracked = bases.piece(id).unwrap().voxels(0);
        assert_ne!(whole, cracked);
        let (broken, _, blocks) = bases.damage(id, 200.0).unwrap();
        assert!(broken && blocks.len() == 8);
        assert!(bases.piece(id).is_none() && bases.materials.wood == 0);
        // What it took up is gone from the world
        let mut brick = [AIR; BLOCK_VOXELS];
        brick[0] = DIRT;
        bases.apply_to([5, 5, 2], &mut brick);
        assert!(brick.iter().all(|&m| m == AIR || m == DIRT));
        assert!(whole.iter().zip(brick.iter()).all(|(&w, &b)| w == AIR || b == AIR));
    }

    // A door shut is in the way, open it isn't, but for the edge at its hinge
    #[test]
    fn doors_open_and_shut() {
        let mut bases = Bases::new();
        bases.materials = Materials::new(4, 0, 0);
        bases.build(Kind::Door, [0, 0, 0], 0).unwrap();
        let id = bases.piece_at([0, 0, 0]).unwrap().id;
        let count = |bases: &Bases| (0..8).map(|k| bases.piece(id).unwrap().voxels(k).iter().filter(|&&m| m != AIR).count()).sum::<usize>();
        let shut = count(&bases);
        assert_eq!(bases.toggle_door(id).unwrap().0, true);
        let open = count(&bases);
        assert!(open * 3 < shut, "{open} voxels open, {shut} shut");
        assert_eq!(bases.toggle_door(id).unwrap().0, false);
        assert_eq!(count(&bases), shut);
    }

    // What's built and dug comes back the same, saved and loaded, and as a strip takes it
    #[test]
    fn edits_go_round() {
        let mut bases = Bases::new();
        bases.materials = Materials::new(20, 9, 4);
        bases.build(Kind::StoneWall, [3, 4, 5], 0).unwrap();
        bases.build(Kind::Stairs, [3, 5, 5], 2).unwrap();
        bases.build(Kind::Torch, [4, 4, 5], 0).unwrap();
        bases.clear_voxels([9, 9, 1], &[0, 1, 2, 511]);
        let door = { bases.build(Kind::Door, [6, 4, 5], 1).unwrap(); bases.piece_at([6, 4, 5]).unwrap().id };
        bases.toggle_door(door);
        let saved = serde_json::to_string(&bases.world_save()).unwrap();
        let mut again = Bases::new();
        again.load_world(serde_json::from_str(&saved).unwrap());
        assert_eq!(again.pieces(), bases.pieces());
        let (a, b) = (bases.edits(), again.edits());
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!((x.at, x.cleared, x.put.as_deref().map(|p| p.to_vec())), (y.at, y.cleared, y.put.as_deref().map(|p| p.to_vec())));
        }
        let game = serde_json::to_string(&bases.game_save(crate::daynight::Clock::default())).unwrap();
        let game: GameBases = serde_json::from_str(&game).unwrap();
        assert_eq!(game.materials, bases.materials);
        // What's put in a brick is what it's built of, what's dug out of one goes
        let mut brick = [GRASS; BLOCK_VOXELS];
        again.apply_to([9, 9, 1], &mut brick);
        assert!(brick[0] == AIR && brick[1] == AIR && brick[3] == GRASS && brick[511] == AIR);
        let mut brick = [AIR; BLOCK_VOXELS];
        again.apply_to([3, 4, 5], &mut brick);
        assert!(brick.iter().all(|&m| matches!(m, BRICK_STONE | STONE | LIMESTONE | MORTAR)));
        // Each change after a strip's started is put in once it's in
        let before = bases.version;
        bases.clear_voxels([1, 1, 1], &[7]);
        let edits = bases.edits();
        assert_eq!(edits.iter().filter(|e| e.version > before).count(), 1);
    }

    // Empty bricks round what's built stop short of it, near and far
    #[test]
    fn skips_stop_short_of_what_is_built() {
        let filled = [[10, 10, 10], [11, 10, 10], [40, 40, 3]];
        let groups = groups(&filled);
        assert_eq!(groups.len(), 2);
        assert!(groups.contains(&([10, 10, 10], [12, 11, 11])));
        // Every empty brick's distance must be no more than how far it is from the nearest filled
        // brick, along any axis
        let cheb = |a: [i64; 3], b: [i64; 3]| (0..3).map(|i| (a[i] - b[i]).abs()).max().unwrap();
        for c in [[10, 10, 30], [0, 0, 0], [12, 10, 10], [10, 10, 9]] {
            let nearest = filled.iter().map(|&f| cheb(c, f)).min().unwrap();
            let (lo, hi) = groups[0];
            let d = (0..3).map(|i| (lo[i] - c[i]).max(c[i] - (hi[i] - 1)).max(0)).max().unwrap().max(1);
            assert!(d <= nearest.max(1), "{c:?}: {d} past {nearest}");
        }
    }
}
