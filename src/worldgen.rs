use std::collections::HashMap;
use std::f32::consts::{FRAC_1_SQRT_2, FRAC_PI_2, TAU};
use std::sync::{Arc, Mutex};
use std::thread;

mod camps;
mod castles;
mod towns;
mod trees;
mod undercity;

pub use camps::{Camp, CampKind, camp_spots};
#[allow(unused_imports)]
pub use undercity::Sunken;

/// Material ids, one byte per voxel. Has to match the palette in shaders\traverse.comp.
pub mod material {
    pub const AIR: u8 = 0;
    pub const GRASS: u8 = 1;
    pub const DIRT: u8 = 2;
    pub const STONE: u8 = 3;
    pub const SAND: u8 = 4;
    pub const SNOW: u8 = 5;
    pub const WATER: u8 = 6;
    pub const WOOD: u8 = 7;
    pub const LEAVES: u8 = 8;
    pub const FLOWER_RED: u8 = 9;
    pub const FLOWER_YELLOW: u8 = 10;
    pub const CLAY: u8 = 11;
    pub const GRASS_TIP: u8 = 12;
    pub const LEAVES_DARK: u8 = 13;
    pub const LEAVES_YELLOW: u8 = 14;
    pub const LEAVES_ORANGE: u8 = 15;
    pub const LEAVES_RED: u8 = 16;
    pub const FLOWER_PURPLE: u8 = 17;
    pub const FLOWER_WHITE: u8 = 18;
    pub const ROCK: u8 = 19;
    pub const MOSS: u8 = 20;
    pub const FOAM: u8 = 21;
    pub const GRASS_BLADE: u8 = 22;
    pub const PINE: u8 = 23;
    pub const GRAVEL: u8 = 24;
    pub const FLOWER_BLUE: u8 = 25;
    pub const FLOWER_PINK: u8 = 26;
    pub const FERN: u8 = 27;
    pub const BIRCH_BARK: u8 = 28;
    pub const BLOSSOM: u8 = 29;
    pub const WILLOW: u8 = 30;
    pub const MUSHROOM_CAP: u8 = 31;
    pub const MUSHROOM_STEM: u8 = 32;
    pub const REED: u8 = 33;
    pub const CATTAIL: u8 = 34;
    pub const PALM_LEAF: u8 = 35;
    pub const PALM_WOOD: u8 = 36;
    pub const DEAD_WOOD: u8 = 37;
    pub const BARK_DARK: u8 = 38;
    // Only on animals, see src\animals.rs
    pub const FUR_BROWN: u8 = 39;
    pub const FUR_DARK: u8 = 40;
    pub const FUR_RED: u8 = 41;
    pub const FUR_TAN: u8 = 42;
    pub const FUR_GRAY: u8 = 43;
    pub const FUR_WHITE: u8 = 44;
    pub const EYE: u8 = 45;
    pub const NOSE: u8 = 46;
    pub const FUR_GINGER: u8 = 47;
    pub const FUR_CREAM: u8 = 48;
    // Only on food lying around, see src\game.rs
    pub const APPLE: u8 = 49;
    pub const MEAT: u8 = 50;
    pub const BONE: u8 = 51;
    // Temples, see Terrain::pyramid() and the like
    pub const TEMPLE_STONE: u8 = 52;
    pub const TEMPLE_DARK: u8 = 53;
    pub const TEMPLE_RED: u8 = 54;
    pub const TEMPLE_TEAL: u8 = 55;
    // These glow
    pub const CRYSTAL: u8 = 56;
    pub const FLAME: u8 = 57;
    pub const WAX: u8 = 58;
    // Deserts and their ruins
    pub const SANDSTONE: u8 = 59;
    pub const SANDSTONE_DARK: u8 = 60;
    // Old stone buildings
    pub const BRICK_STONE: u8 = 61;
    pub const MORTAR: u8 = 62;
    pub const LIMESTONE: u8 = 63;
    // Jungles and deserts
    pub const JUNGLE_GRASS: u8 = 64;
    pub const DRY_GRASS: u8 = 65;
    pub const CACTUS: u8 = 66;
    pub const VINE: u8 = 67;
    pub const CLAY_POT: u8 = 68;
    // Weapons, see src\weapons.rs
    pub const STEEL: u8 = 69;
    pub const DARK_STEEL: u8 = 70;
    pub const GOLD: u8 = 71;
    pub const BRONZE: u8 = 72;
    pub const LEATHER: u8 = 73;
    pub const CLOTH: u8 = 74;
    pub const LINEN: u8 = 75;
    pub const OBSIDIAN: u8 = 76;
    pub const BOWSTRING: u8 = 77;
    pub const ICE: u8 = 78;
    // These glow, in the colors of the elements
    pub const GLOW_FIRE: u8 = 79;
    pub const GLOW_FROST: u8 = 80;
    pub const GLOW_STORM: u8 = 81;
    pub const GLOW_POISON: u8 = 82;
    pub const GLOW_ARCANE: u8 = 83;
    pub const GLOW_VOID: u8 = 84;
    pub const GLOW_BLOOD: u8 = 85;
    pub const GLOW_HOLY: u8 = 86;
    // The sunlit tips of fresh green crowns, and the shadowy depths of red ones
    pub const LEAVES_LIME: u8 = 87;
    pub const LEAVES_CRIMSON: u8 = 88;
}

use material::*;

/// Whether something can walk through a material: air, water and anything soft like leaves and flowers.
pub fn is_passable(material: u8) -> bool {
    matches!(
        material,
        AIR | WATER
            | FOAM
            | GRASS_BLADE
            | GRASS_TIP
            | FLOWER_RED
            | FLOWER_YELLOW
            | FLOWER_PURPLE
            | FLOWER_WHITE
            | FLOWER_BLUE
            | FLOWER_PINK
            | LEAVES
            | LEAVES_DARK
            | LEAVES_YELLOW
            | LEAVES_ORANGE
            | LEAVES_RED
            | LEAVES_LIME
            | LEAVES_CRIMSON
            | PINE
            | FERN
            | BLOSSOM
            | WILLOW
            | MUSHROOM_CAP
            | MUSHROOM_STEM
            | REED
            | CATTAIL
            | PALM_LEAF
            | DRY_GRASS
            | VINE
    )
}

/// Whether a material's the Cinder Crown's lava, dull red to white hot, see Terrain::column()
pub fn is_lava(material: u8) -> bool {
    matches!(material, GLOW_BLOOD | GLOW_FIRE | GLOW_HOLY)
}

// The world is split into 8x8x8 bricks. Bricks that are empty or filled with a
// single material only take up one grid cell, the rest store every voxel.
const BRICK_VOXELS: usize = 512;
/// u32 words per stored brick: 16 words of occupancy bits, then 128 words of materials (one byte per voxel).
/// Has to match BRICK_WORDS in shaders\traverse.comp.
pub const BRICK_WORDS: usize = 16 + 128;

/// A quick hash for brick cells, numbers well enough spread already: the usual one is slow for
/// the thousands of lookups blowing a hole takes
#[derive(Default)]
struct CellHasher(u64);

impl std::hash::Hasher for CellHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
    }

    fn write_usize(&mut self, n: usize) {
        self.0 = (n as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
}

/// Where each brick cell being changed is in a list of them
type Cells = std::collections::HashMap<usize, usize, std::hash::BuildHasherDefault<CellHasher>>;
/// The bricks World::carve() has unpacked, by where they are: in a box of them round the hole by
/// their place in it, anywhere else by their cell. None for a brick that's empty.
struct Found {
    lo: [i64; 3],
    dims: [i64; 3],
    slots: Vec<u32>,
    far: Cells,
}

impl Found {
    const UNSEEN: u32 = u32::MAX;
    const EMPTY: u32 = u32::MAX - 1;

    /// `lo` to `hi` in bricks, `hi` not included
    fn new(lo: [i64; 3], hi: [i64; 3]) -> Self {
        let dims = [0, 1, 2].map(|i| (hi[i] - lo[i]).max(1));
        Found { lo, dims, slots: vec![Self::UNSEEN; (dims[0] * dims[1] * dims[2]) as usize], far: Cells::default() }
    }

    fn slot(&self, at: [i64; 3]) -> Option<usize> {
        let p = [0, 1, 2].map(|i| at[i] - self.lo[i]);
        (0..3).all(|i| p[i] >= 0 && p[i] < self.dims[i]).then(|| (p[0] + (p[1] + p[2] * self.dims[1]) * self.dims[0]) as usize)
    }

    /// Whether brick `at`, at `cell`, has been seen, and if it has, which it is
    fn get(&self, at: [i64; 3], cell: usize) -> Option<Option<usize>> {
        let value = match self.slot(at) {
            Some(i) => self.slots[i],
            None => *self.far.get(&cell)? as u32,
        };
        match value {
            Self::UNSEEN => None,
            Self::EMPTY => Some(None),
            k => Some(Some(k as usize)),
        }
    }

    fn insert(&mut self, at: [i64; 3], cell: usize, k: Option<usize>) {
        let value = k.map_or(Self::EMPTY, |k| k as u32);
        match self.slot(at) {
            Some(i) => self.slots[i] = value,
            None => {
                self.far.insert(cell, value as usize);
            }
        }
    }
}

/// Bricks a world keeps room for beyond those it starts with, for holes blown in solid ground:
/// a solid brick with a hole in it has to be stored
pub const SPARE_BRICKS: usize = 131072;
/// Grid cells with this bit set are filled with one material, kept in the low byte.
/// Has to match SOLID_BRICK in shaders\traverse.comp.
pub const SOLID_BRICK: u32 = 1 << 31;
/// Grid cells with this bit set are empty. The low 16 bits hold the distance to the nearest
/// non-empty brick, in bricks along any axis, so rays can skip that far in one go.
/// Has to match EMPTY_BRICK in shaders\traverse.comp.
pub const EMPTY_BRICK: u32 = 1 << 30;
/// Stored bricks with this bit set have leaves or flowers in them, which sway in the wind, see
/// sways(). Has to match FOLIAGE_BRICK in shaders\traverse.comp.
pub const FOLIAGE_BRICK: u32 = 1 << 29;
/// The bits of a stored brick's cell that hold 1 + its index
const BRICK_INDEX: u32 = FOLIAGE_BRICK - 1;

/// Whether a material sways in the wind: leaves, and the flowers and tips of grass. Has to match
/// sways() in shaders\traverse.comp.
pub fn sways(material: u8) -> bool {
    matches!(
        material,
        LEAVES | LEAVES_DARK | LEAVES_YELLOW | LEAVES_ORANGE | LEAVES_RED | LEAVES_LIME | LEAVES_CRIMSON | BLOSSOM | WILLOW | PALM_LEAF | PINE | FERN | VINE | GRASS_TIP
            | FLOWER_RED | FLOWER_YELLOW | FLOWER_PURPLE | FLOWER_WHITE | FLOWER_BLUE | FLOWER_PINK
    )
}

pub struct World {
    /// Size in voxels, all multiples of 8.
    pub size: [u32; 3],
    /// Where this world's first voxel is, in voxels of its own size from the corner of the
    /// whole landscape. Zero unless it's a piece of one, like the fine detail around the player.
    pub origin: [i64; 3],
    /// One cell per brick, indexed by x + y * bricks.x + z * bricks.x * bricks.y, of the bricks
    /// there's room for along x and y, see `phys`; find a brick's with cell().
    /// EMPTY_BRICK | distance is empty, SOLID_BRICK | material is filled,
    /// anything else is 1 + the index of a stored brick.
    pub brick_grid: Vec<u32>,
    /// BRICK_WORDS per stored brick. Within a brick, voxels are indexed by x + y * 8 + z * 64.
    pub bricks: Vec<u32>,
    pub voxels_per_meter: u32,
    /// Height of the water surface in voxels, which carries on as an endless sea outside the world.
    /// None for worlds without water.
    pub sea_level: Option<f32>,
    /// Where grass grows, for the triangle grass in the shader: one value per column of voxels,
    /// see column(), with the ground's height in quarter voxels in bits 0-15 and how dense the
    /// grass is in bits 16-23 (0 where none grows). None for worlds without grass.
    pub grass: Option<Vec<u32>>,
    /// Where the water starts out, for the simulation in src\water.rs. Water isn't in the voxels.
    /// None for worlds without water.
    pub water: Option<WaterMap>,
    /// How many columns of voxels along x and y the brick grid and the maps of grass and water
    /// have room for: as many as the world's size, but for the world that moves on over the
    /// landscape a strip at a time a strip more, see World::window(). Its columns are each kept at
    /// where they are in the whole landscape modulo this, so as the world moves on nothing has to
    /// be moved, only what comes in put in where what's gone was.
    pub phys: [u32; 2],
    /// Where in that room column 0 of the world is, along x and y
    pub wrap: [u32; 2],
    /// For the world that moves on, which of its bricks are in use, see Pages
    pub pages: Option<Pages>,
}

/// One value per column of voxels, see World::column(), in voxels: the top of the ground, and the
/// top of the water on it, 0 where it's dry
pub struct WaterMap {
    pub ground: Vec<f32>,
    pub surface: Vec<f32>,
}

/// Bricks in each page of the world that moves on, see Pages: each square of the landscape a strip
/// across has pages of its own, see Strip::new(), with half one left over on average
pub const PAGE_BRICKS: usize = 512;

/// The bricks of the world that moves on are kept in pages of PAGE_BRICKS, each strip that comes in
/// in pages of its own wherever some are free, and a page is free again once none of its bricks
/// are in use any more, as the strips they're in go, see World::release()
#[derive(Default)]
pub struct Pages {
    /// What's in each page, BRICK_WORDS a brick: nothing for those not in use
    data: Vec<Box<[u32]>>,
    /// How many of each page's bricks are in use
    live: Vec<u32>,
    /// Pages none of whose bricks are in use any more, but which the frames still being drawn may
    /// have: see take_released(), and PagePool::give_back() once those are done
    released: Vec<u32>,
    /// The page the bricks of holes blown and stains splashed go in, and how many it has
    carving: Option<(u32, u32)>,
    /// Where pages are taken from, shared with the strips being built, see Strip::new()
    pub pool: Arc<Mutex<PagePool>>,
}

/// The pages of the world that moves on that are free to fill, see Pages: taken by the strips being
/// built as they come in, and by holes blown, given back once nothing's drawn with them any more
#[derive(Default)]
pub struct PagePool {
    free: Vec<u32>,
    /// How many pages there are, free or not
    count: u32,
}

impl PagePool {
    /// `n` pages to fill, made anew if there aren't enough free
    pub fn take(&mut self, n: usize) -> Vec<u32> {
        (0..n)
            .map(|_| {
                self.free.pop().unwrap_or_else(|| {
                    self.count += 1;
                    self.count - 1
                })
            })
            .collect()
    }

    /// Pages nothing's drawn with any more, to fill again
    pub fn give_back(&mut self, pages: impl IntoIterator<Item = u32>) {
        self.free.extend(pages);
    }

    /// How many pages there are, so how many bricks the GPU has to have room for, PAGE_BRICKS each
    pub fn count(&self) -> u32 {
        self.count
    }

}

impl Pages {
    /// Has room for page `page`, all of it zeros if it's new
    fn ensure(&mut self, page: u32) {
        let p = page as usize;
        self.reserve(page);
        if self.data[p].is_empty() {
            self.data[p] = vec![0u32; PAGE_BRICKS * BRICK_WORDS].into_boxed_slice();
        }
    }

    /// Has a place for page `page`, empty if it's new
    fn reserve(&mut self, page: u32) {
        let p = page as usize;
        if self.data.len() <= p {
            self.data.resize_with(p + 1, || Box::new([]));
            self.live.resize(p + 1, 0);
        }
    }

    /// One fewer of `page`'s bricks in use
    fn drop_one(&mut self, page: u32) {
        let live = &mut self.live[page as usize];
        *live -= 1;
        if *live == 0 && self.carving.is_none_or(|(p, _)| p != page) {
            self.released.push(page);
        }
    }
}

/// A strip of the landscape built to go into the world that moves on, its bricks in pages of their
/// own in it, see World::install()
pub struct Strip {
    /// Its first column in the whole landscape, and how many columns it is across
    pub origin: [i64; 2],
    pub size: [u32; 2],
    /// Its brick grid, counted from its first brick, with the bricks' indices in the world
    pub cells: Vec<u32>,
    /// The pages its bricks are in: which, how many of each are in use, and what's in each
    pub pages: Vec<(u32, u32, Box<[u32]>)>,
    /// Its maps of grass and water, see World
    pub grass: Vec<u32>,
    pub ground: Vec<f32>,
    pub surface: Vec<f32>,
}

impl Strip {
    /// `world`, built on its own, made ready to go into the world that moves on: its empty bricks
    /// skip no further than its edges, see keep_skips_inside(), and its bricks go in pages of the
    /// world's from `pool`, those of each square of the landscape `tile` columns across in pages of
    /// their own. The world lets go of a strip that wide at a time, along x or y, so its pages are
    /// all free again once it has: with the bricks of a whole piece of the landscape in the same
    /// pages, none of them were till all of it had gone, and the world needed twice the room.
    pub fn new(mut world: World, pool: &Mutex<PagePool>, tile: u32) -> Self {
        world.keep_skips_inside();
        let n = world.bricks.len() / BRICK_WORDS;
        let [bx, by, bz] = world.size.map(|v| (v / 8) as usize);
        let tile = (tile as i64 / 8).max(1);
        let first = [0, 1].map(|i| world.origin[i].div_euclid(8).div_euclid(tile));
        let tiles_x = ((world.origin[0].div_euclid(8) + bx as i64 - 1).div_euclid(tile) - first[0] + 1) as usize;
        let tiles_y = ((world.origin[1].div_euclid(8) + by as i64 - 1).div_euclid(tile) - first[1] + 1) as usize;
        let tile_x: Vec<usize> = (0..bx).map(|x| ((world.origin[0].div_euclid(8) + x as i64).div_euclid(tile) - first[0]) as usize).collect();
        let tile_y: Vec<usize> = (0..by).map(|y| ((world.origin[1].div_euclid(8) + y as i64).div_euclid(tile) - first[1]) as usize).collect();
        // Each stored brick's square, and where it is among those of its square
        let mut counts = vec![0u32; tiles_x * tiles_y];
        let mut place = vec![(0u32, 0u32); n];
        for z in 0..bz {
            for y in 0..by {
                for x in 0..bx {
                    let v = world.brick_grid[x + (y + z * by) * bx];
                    if v & (SOLID_BRICK | EMPTY_BRICK) == 0 {
                        let t = tile_x[x] + tile_y[y] * tiles_x;
                        place[((v & BRICK_INDEX) - 1) as usize] = (t as u32, counts[t]);
                        counts[t] += 1;
                    }
                }
            }
        }
        // Each square's first page among those taken
        let mut starts = Vec::with_capacity(counts.len());
        let mut total = 0;
        for &count in &counts {
            starts.push(total);
            total += (count as usize).div_ceil(PAGE_BRICKS);
        }
        let ids = pool.lock().unwrap_or_else(|e| e.into_inner()).take(total);
        let mut pages: Vec<(u32, u32, Box<[u32]>)> = ids.iter().map(|&id| (id, 0, vec![0u32; PAGE_BRICKS * BRICK_WORDS].into_boxed_slice())).collect();
        let at = |local: usize| {
            let (t, k) = place[local];
            (starts[t as usize] + k as usize / PAGE_BRICKS, k as usize % PAGE_BRICKS)
        };
        for local in 0..n {
            let (page, slot) = at(local);
            pages[page].1 += 1;
            pages[page].2[slot * BRICK_WORDS..(slot + 1) * BRICK_WORDS].copy_from_slice(&world.bricks[local * BRICK_WORDS..(local + 1) * BRICK_WORDS]);
        }
        let cells = world
            .brick_grid
            .iter()
            .map(|&v| {
                if v & (SOLID_BRICK | EMPTY_BRICK) != 0 {
                    return v;
                }
                let (page, slot) = at(((v & BRICK_INDEX) - 1) as usize);
                let index = ids[page] as usize * PAGE_BRICKS + slot;
                (index as u32 + 1) | (v & FOLIAGE_BRICK)
            })
            .collect();
        let map = world.water.take().unwrap_or(WaterMap { ground: Vec::new(), surface: Vec::new() });
        Strip {
            origin: [world.origin[0], world.origin[1]],
            size: [world.size[0], world.size[1]],
            cells,
            pages,
            grass: world.grass.take().unwrap_or_default(),
            ground: map.ground,
            surface: map.surface,
        }
    }

}

/// Where the `n` columns, or bricks, from `lo` of the whole landscape along one way are kept in room
/// for `room` of them, `n` at most: in one run, or two where they go round, each where in the room
/// it starts, how far into the `n` it is, and how many
pub fn runs(lo: i64, n: u32, room: u32) -> impl Iterator<Item = (usize, usize, usize)> {
    let (first, n, room) = (lo.rem_euclid(room as i64) as usize, n as usize, room as usize);
    let a = n.min(room - first);
    [(first, 0, a), (0, a, n - a)].into_iter().filter(|r| r.2 > 0)
}

/// Putting a strip into the world that moves on, a few layers of its bricks at a time so no frame's
/// held up, see World::install()
pub struct Installing {
    pub strip: Strip,
    /// The first layer of bricks not put in yet, and whether its pages and maps are in
    next_z: u32,
    started: bool,
}

/// Letting go of a part of the world that moves on, a few layers at a time, see World::release()
pub struct Releasing {
    /// Columns of the whole landscape, from and to
    lo: [i64; 2],
    hi: [i64; 2],
    next_z: u32,
}

impl World {
    /// `brick_grid` uses 0 for empty bricks, which get replaced by their EMPTY_BRICK distance.
    fn new(size: [u32; 3], origin: [i64; 3], mut brick_grid: Vec<u32>, bricks: Vec<u32>, voxels_per_meter: u32) -> Self {
        let dims = size.map(|n| n as usize / 8);
        let distances = empty_distances(&brick_grid, dims);
        for (cell, distance) in brick_grid.iter_mut().zip(distances) {
            if *cell == 0 {
                *cell = EMPTY_BRICK | distance as u32;
            }
        }
        World {
            size,
            origin,
            brick_grid,
            bricks,
            voxels_per_meter,
            sea_level: None,
            grass: None,
            water: None,
            phys: [size[0], size[1]],
            wrap: [0, 0],
            pages: None,
        }
    }

    /// A world `size` voxels across that moves on over the landscape a strip at a time, with room
    /// for `strip` columns more along x and y for the next strip to come in, see install(). Empty
    /// at `origin` till the strips of it are put in.
    pub fn window(size: [u32; 3], strip: u32, origin: [i64; 2], voxels_per_meter: u32, sea_level: Option<f32>) -> Self {
        let phys = [size[0] + strip, size[1] + strip];
        let cells = (phys[0] / 8) as usize * (phys[1] / 8) as usize * (size[2] / 8) as usize;
        let columns = phys[0] as usize * phys[1] as usize;
        let mut world = World {
            size,
            origin: [origin[0], origin[1], 0],
            brick_grid: vec![EMPTY_BRICK | 1; cells],
            bricks: Vec::new(),
            voxels_per_meter,
            sea_level,
            grass: Some(vec![0; columns]),
            water: Some(WaterMap { ground: vec![0.0; columns], surface: vec![0.0; columns] }),
            phys,
            wrap: [0, 0],
            pages: Some(Pages::default()),
        };
        world.move_to(origin);
        world
    }

    /// Moves the world on over the landscape so its corner's at `origin`, see window(): nothing
    /// in it moves, but the columns it's made of are counted from another one
    pub fn move_to(&mut self, origin: [i64; 2]) {
        self.origin[0] = origin[0];
        self.origin[1] = origin[1];
        self.wrap = [0, 1].map(|i| origin[i].rem_euclid(self.phys[i] as i64) as u32);
    }

    /// The cell in the brick grid of brick `b`, counted from the world's first, which has to be in it
    #[inline]
    pub fn cell(&self, b: [i64; 3]) -> usize {
        let room = [self.phys[0] as i64 / 8, self.phys[1] as i64 / 8];
        let at = [0, 1].map(|i| {
            let p = b[i] + self.wrap[i] as i64 / 8;
            if p >= room[i] { p - room[i] } else { p }
        });
        (at[0] + (at[1] + b[2] * room[1]) * room[0]) as usize
    }

    /// Where column `x`, `y`, counted from the world's first, which has to be in it, is in the maps
    /// of grass and water
    #[inline]
    pub fn column(&self, x: i64, y: i64) -> usize {
        let (px, py) = (self.phys[0] as i64, self.phys[1] as i64);
        let x = x + self.wrap[0] as i64;
        let y = y + self.wrap[1] as i64;
        let x = if x >= px { x - px } else { x };
        let y = if y >= py { y - py } else { y };
        (x + y * px) as usize
    }

    pub fn brick_count(&self) -> usize {
        match &self.pages {
            Some(pages) => pages.live.iter().map(|&n| n as usize).sum(),
            None => self.bricks.len() / BRICK_WORDS,
        }
    }

    /// How many bricks there's room for, on the CPU and so on the GPU: those it has, and for the
    /// world that moves on, every page so far
    pub fn brick_room(&self) -> usize {
        match &self.pages {
            Some(pages) => pages.data.len() * PAGE_BRICKS,
            None => self.bricks.len() / BRICK_WORDS,
        }
    }

    /// How much of the computer's memory its bricks take, in bytes
    pub fn brick_bytes(&self) -> usize {
        match &self.pages {
            Some(pages) => pages.data.iter().map(|d| d.len()).sum::<usize>() * 4,
            None => self.bricks.capacity() * 4,
        }
    }

    /// Lets go of where the water's surface starts out, once the water's been made from it, see
    /// Water::new() in water.rs: tens of megabytes kept for nothing
    pub fn forget_water_surface(&mut self) {
        if let Some(water) = &mut self.water {
            water.surface = Vec::new();
        }
    }

    /// Material of a voxel, counted from this world's first voxel. AIR outside the world.
    pub fn material_at(&self, x: i64, y: i64, z: i64) -> u8 {
        let [sx, sy, sz] = self.size.map(i64::from);
        if x < 0 || y < 0 || z < 0 || x >= sx || y >= sy || z >= sz {
            return AIR;
        }
        let cell = self.brick_grid[self.cell([x / 8, y / 8, z / 8])];
        if cell & EMPTY_BRICK != 0 {
            return AIR;
        }
        if cell & SOLID_BRICK != 0 {
            return cell as u8;
        }
        let i = (x % 8 + (y % 8) * 8 + (z % 8) * 64) as usize;
        (self.brick(((cell & BRICK_INDEX) - 1) as usize)[16 + i / 4] >> ((i % 4) * 8)) as u8
    }

    /// The words of stored brick `index`
    #[inline]
    fn brick(&self, index: usize) -> &[u32] {
        match &self.pages {
            Some(pages) => {
                let at = (index % PAGE_BRICKS) * BRICK_WORDS;
                &pages.data[index / PAGE_BRICKS][at..at + BRICK_WORDS]
            }
            None => &self.bricks[index * BRICK_WORDS..(index + 1) * BRICK_WORDS],
        }
    }

    fn brick_mut(&mut self, index: usize) -> &mut [u32] {
        match &mut self.pages {
            Some(pages) => {
                let at = (index % PAGE_BRICKS) * BRICK_WORDS;
                &mut pages.data[index / PAGE_BRICKS][at..at + BRICK_WORDS]
            }
            None => &mut self.bricks[index * BRICK_WORDS..(index + 1) * BRICK_WORDS],
        }
    }

    /// Stores a brick of `materials` that aren't all the same, for a hole blown or a stain
    /// splashed, returning its cell: at the end of the bricks, or in the world that moves on in
    /// the page they go in, see Pages
    fn new_brick(&mut self, materials: &[u8; BRICK_VOXELS]) -> u32 {
        let Some(pages) = &mut self.pages else {
            return push_brick(materials, &mut self.bricks);
        };
        let (page, used) = match pages.carving {
            Some((page, used)) if (used as usize) < PAGE_BRICKS => (page, used),
            full => {
                let page = pages.pool.lock().unwrap_or_else(|e| e.into_inner()).take(1)[0];
                pages.carving = Some((page, 0));
                // The last, full now, goes once none of it's in use, like any other
                if let Some((old, _)) = full
                    && pages.live[old as usize] == 0
                {
                    pages.released.push(old);
                }
                pages.ensure(page);
                pages.live[page as usize] = 0;
                (page, 0)
            }
        };
        pages.carving = Some((page, used + 1));
        pages.live[page as usize] += 1;
        let index = page as usize * PAGE_BRICKS + used as usize;
        pack_brick(materials, self.brick_mut(index));
        let foliage = if materials.iter().any(|&m| sways(m)) { FOLIAGE_BRICK } else { 0 };
        (index as u32 + 1) | foliage
    }

    /// Stored brick `index` isn't in use any more
    fn drop_brick(&mut self, index: usize) {
        if let Some(pages) = &mut self.pages {
            pages.drop_one((index / PAGE_BRICKS) as u32);
        }
    }

    /// The pages of the world that moves on none of whose bricks are in use any more since last
    /// asked, to give back to its pool once the frames drawn with them are done
    pub fn take_released(&mut self) -> Vec<u32> {
        self.pages.as_mut().map_or(Vec::new(), |p| std::mem::take(&mut p.released))
    }

    /// What's in page `page` of the world that moves on, to send to the GPU, if it's in use
    pub fn page(&self, page: u32) -> Option<&[u32]> {
        let data = self.pages.as_ref()?.data.get(page as usize)?;
        (!data.is_empty()).then_some(&data[..])
    }

    /// Where the pages of the world that moves on come from, for the strips built to go in it
    pub fn page_pool(&self) -> Option<Arc<Mutex<PagePool>>> {
        self.pages.as_ref().map(|p| p.pool.clone())
    }

    /// For a strip of the landscape built on its own: past its edges is what's put in beside it
    /// later, which it didn't know about, so its empty bricks skip no further than those, see
    /// EMPTY_BRICK
    pub fn keep_skips_inside(&mut self) {
        let [bx, by, bz] = self.size.map(|n| n as usize / 8);
        for z in 0..bz {
            for y in 0..by {
                let room_y = (y + 1).min(by - y);
                for x in 0..bx {
                    let cell = &mut self.brick_grid[x + (y + z * by) * bx];
                    if *cell & EMPTY_BRICK != 0 {
                        let room = (x + 1).min(bx - x).min(room_y) as u32;
                        *cell = EMPTY_BRICK | (*cell & 0xFFFF).min(room).max(1);
                    }
                }
            }
        }
    }

    /// Starts putting `strip` into the world that moves on, at where it is in the whole landscape
    /// modulo the room there is, which has to have been let go of, see release(): its bricks are
    /// in pages of their own already, see Strip::new(). It's done by install_step().
    pub fn install(strip: Strip) -> Installing {
        Installing { strip, next_z: 0, started: false }
    }

    /// Puts up to `layers` more layers of bricks of a strip in, or the first time only its pages
    /// and its maps of grass and water. Whether it's all in.
    pub fn install_step(&mut self, installing: &mut Installing, layers: u32) -> bool {
        let strip = &mut installing.strip;
        let [sx, sy] = strip.size.map(|n| n as i64);
        if !installing.started {
            installing.started = true;
            // What was in its pages before goes with the strip once it's in, to be let go of
            // aside: hundreds of them, each a few hundred kilobytes, took milliseconds to free
            let pages = self.pages.as_mut().expect("only the world that moves on has strips put in");
            for (id, live, data) in &mut strip.pages {
                pages.reserve(*id);
                std::mem::swap(&mut pages.data[*id as usize], data);
                pages.live[*id as usize] = *live;
                *live = 0;
            }
            // A row at a time, in one run or two where it goes round the room: hundreds of
            // thousands of columns, each worked out on its own took milliseconds
            fn put<T: Copy>(into: &mut [T], from: &[T], nothing: T) {
                if from.len() == into.len() { into.copy_from_slice(from) } else { into.fill(nothing) }
            }
            let px = self.phys[0] as usize;
            let xs: Vec<(usize, usize, usize)> = runs(strip.origin[0], sx as u32, self.phys[0]).collect();
            for y in 0..sy {
                let row = (strip.origin[1] + y).rem_euclid(self.phys[1] as i64) as usize * px;
                for &(at, along, n) in &xs {
                    let first = y as usize * sx as usize + along;
                    let (to, from) = (row + at..row + at + n, first..first + n);
                    if let Some(grass) = &mut self.grass {
                        put(&mut grass[to.clone()], strip.grass.get(from.clone()).unwrap_or(&[]), 0);
                    }
                    if let Some(water) = &mut self.water {
                        put(&mut water.ground[to.clone()], strip.ground.get(from.clone()).unwrap_or(&[]), 0.0);
                        if !water.surface.is_empty() {
                            put(&mut water.surface[to], strip.surface.get(from).unwrap_or(&[]), 0.0);
                        }
                    }
                }
            }
            // Its layers from the next time on, that being a few milliseconds' worth
            return false;
        }
        let [bx, by, bz] = [sx / 8, sy / 8, (self.size[2] / 8) as i64];
        let first = [strip.origin[0].div_euclid(8), strip.origin[1].div_euclid(8)];
        let room = [self.phys[0] as usize / 8, self.phys[1] as usize / 8];
        let xs: Vec<(usize, usize, usize)> = runs(first[0], bx as u32, room[0] as u32).collect();
        let end = (installing.next_z as i64 + layers as i64).min(bz);
        for z in installing.next_z as i64..end {
            for y in 0..by {
                let row = (z as usize * room[1] + (first[1] + y).rem_euclid(room[1] as i64) as usize) * room[0];
                let from = ((y + z * by) * bx) as usize;
                for &(at, along, n) in &xs {
                    for k in 0..n {
                        let cell = row + at + k;
                        // Anything still there's let go of, should it not have been: its page would
                        // never be free again
                        let old = self.brick_grid[cell];
                        if old & (SOLID_BRICK | EMPTY_BRICK) == 0 {
                            self.drop_brick(((old & BRICK_INDEX) - 1) as usize);
                        }
                        self.brick_grid[cell] = strip.cells[from + along + k];
                    }
                }
            }
        }
        installing.next_z = end as u32;
        end == bz
    }

    /// Starts letting go of the columns of the whole landscape from `lo` to `hi`, brick aligned, of
    /// the world that moves on, as that part of it's to have another strip of the landscape put
    /// in: none of its bricks are in use any more. It's done by release_step().
    pub fn release(lo: [i64; 2], hi: [i64; 2]) -> Releasing {
        Releasing { lo, hi, next_z: 0 }
    }

    /// Lets go of up to `layers` more layers of bricks of a part of the world. Whether that's all.
    pub fn release_step(&mut self, releasing: &mut Releasing, layers: u32) -> bool {
        let bz = (self.size[2] / 8) as i64;
        let (lo, hi) = (releasing.lo.map(|v| v.div_euclid(8)), releasing.hi.map(|v| v.div_euclid(8)));
        let end = (releasing.next_z as i64 + layers as i64).min(bz);
        let room = [self.phys[0] as usize / 8, self.phys[1] as usize / 8];
        let xs: Vec<(usize, usize, usize)> = runs(lo[0], (hi[0] - lo[0]) as u32, room[0] as u32).collect();
        for z in releasing.next_z as i64..end {
            for y in lo[1]..hi[1] {
                let row = (z as usize * room[1] + y.rem_euclid(room[1] as i64) as usize) * room[0];
                for &(at, _, n) in &xs {
                    for cell in row + at..row + at + n {
                        let value = self.brick_grid[cell];
                        if value & (SOLID_BRICK | EMPTY_BRICK) == 0 {
                            self.drop_brick(((value & BRICK_INDEX) - 1) as usize);
                        }
                        self.brick_grid[cell] = EMPTY_BRICK | 1;
                    }
                }
            }
        }
        releasing.next_z = end as u32;
        end == bz
    }

    /// Blows a hole in the world: everything within `radius` voxels of `center`, in voxels of this
    /// world, is gone, with a ragged edge, and grass and flowers left hanging over the hole fall in
    /// with it. Returns the cells of the bricks that changed, and some of what was blown out, for
    /// debris.
    pub fn carve(&mut self, center: [f64; 3], radius: f64) -> (Vec<usize>, Vec<u8>) {
        self.carve_in(center, radius, None)
    }

    /// The same, but only the columns from `clip`'s first to its second, if it says, change: for a
    /// hole blown while a strip that comes into the world was being built, blown in just that once
    /// it's in, see stream.rs
    pub fn carve_in(&mut self, center: [f64; 3], radius: f64, clip: Option<([i64; 2], [i64; 2])>) -> (Vec<usize>, Vec<u8>) {
        let [sx, sy, sz] = self.size.map(i64::from);
        let (bx, by) = (sx / 8, sy / 8);
        // The ragged edge goes by where each voxel is in the whole landscape, so it's the same
        // in every piece of the world the hole's blown in again
        let o = self.origin;
        let mut dirty: Vec<(usize, [u8; BRICK_VOXELS])> = Vec::new();
        let mut blown = Vec::new();

        let r = radius.ceil() as i64 + 1;
        let [cx, cy, cz] = center.map(|v| v.floor() as i64);
        // The bricks round the hole, those things fall into it from and those over it, are found
        // by where they are in a box of them, which is much quicker than looking them up
        let lo = [cx - r - 2, cy - r - 2, cz - r - 2].map(|v| v.div_euclid(8));
        let hi = [cx + r + 2, cy + r + 2, cz + r + 48].map(|v| v.div_euclid(8) + 1);
        let mut found = Found::new(lo, hi);

        // The bricks being changed, unpacked. Empty ones have nothing to blow away.
        let brick = |world: &World, dirty: &mut Vec<(usize, [u8; BRICK_VOXELS])>, found: &mut Found, x: i64, y: i64, z: i64| -> Option<usize> {
            if x < 0 || y < 0 || z < 0 || x >= sx || y >= sy || z >= sz {
                return None;
            }
            let at = [x / 8, y / 8, z / 8];
            let cell = world.cell(at);
            if let Some(k) = found.get(at, cell) {
                return k;
            }
            let value = world.brick_grid[cell];
            if value & EMPTY_BRICK != 0 {
                found.insert(at, cell, None);
                return None;
            }
            let mut materials = [AIR; BRICK_VOXELS];
            if value & SOLID_BRICK != 0 {
                materials = [value as u8; BRICK_VOXELS];
            } else {
                unpack_brick(world.brick(((value & BRICK_INDEX) - 1) as usize), &mut materials);
            }
            dirty.push((cell, materials));
            found.insert(at, cell, Some(dirty.len() - 1));
            Some(dirty.len() - 1)
        };
        let local = |x: i64, y: i64, z: i64| (x % 8 + (y % 8) * 8 + (z % 8) * 64) as usize;
        // The columns a hole was blown in, for what's loose above to fall into, by where they are
        // in the square round it
        let side = 2 * r + 1;
        let (inner, outer) = ((radius * 0.82).powi(2), (radius * 1.12).powi(2));
        // What flies out is picked from all through the hole, so from the top of the ground as
        // much as from deep down: about one in so many of the voxels, about half of it solid
        let stride = ((2.1 * radius.powi(3) / 60.0) as u32).max(1);
        // Whether a hash is a whole number of strides, without dividing, as it's asked of nearly
        // every voxel: see Lemire, Kaser and Kurz, "Faster remainder by direct computation"
        let per = (u64::MAX / stride as u64).wrapping_add(1);
        let picked = |h: u32| stride == 1 || (h as u64).wrapping_mul(per) <= per - 1;
        // The bricks of the layer of them the voxels are in, by where they are across it, each
        // looked up once for the 64 rows through it
        let (bx0, by0) = ((cx - r).div_euclid(8), (cy - r).div_euclid(8));
        let across = (cx + r).div_euclid(8) - bx0 + 1;
        let mut layer: Vec<Option<Option<usize>>> = vec![None; (across * ((cy + r).div_euclid(8) - by0 + 1)) as usize];
        let mut layer_z = i64::MIN;
        let mut holed_at = vec![false; (side * side) as usize];
        for z in cz - r..=cz + r {
            if z.div_euclid(8) != layer_z {
                layer_z = z.div_euclid(8);
                layer.fill(None);
            }
            let dz2 = (z as f64 + 0.5 - center[2]).powi(2);
            let (hz, hz_blown) = (hash((z + o[2]) as u32), hash((z + o[2]) as u32 ^ 0xb1));
            for y in cy - r..=cy + r {
                let dy2 = (y as f64 + 0.5 - center[1]).powi(2);
                // The voxels' hashes, hash3() of where each is, all but the last step worked out
                // once for the row
                let (hy, hy_blown) = (hash((y + o[1]) as u32 ^ hz), hash((y + o[1]) as u32 ^ hz_blown));
                // Only along the row as far as it reaches, and a voxel past that each way, as each
                // voxel's still looked at just as it always was
                let rest = outer - dy2 - dz2;
                if rest < -1.0 {
                    continue;
                }
                let reach = rest.max(0.0).sqrt();
                let (x0, x1) = (((center[0] - 0.5 - reach).floor() as i64 - 1).max(cx - r), ((center[0] - 0.5 + reach).ceil() as i64 + 1).min(cx + r));
                // A brick's run of the row at a time, the brick looked up once, at the first voxel
                // of it that goes
                let mut from = x0;
                while from <= x1 {
                    let to = ((from.div_euclid(8) + 1) * 8 - 1).min(x1);
                    let looked = &mut layer[((from.div_euclid(8) - bx0) + (y.div_euclid(8) - by0) * across) as usize];
                    for x in from..=to {
                        let d2 = (x as f64 + 0.5 - center[0]).powi(2) + dy2 + dz2;
                        if d2 > outer {
                            continue;
                        }
                        // Ragged at the edge: only there does each voxel get a say
                        if d2 > inner {
                            let ragged = 0.82 + 0.3 * unit(hash((x + o[0]) as u32 ^ hy)) as f64;
                            if d2 > (radius * ragged).powi(2) {
                                continue;
                            }
                        }
                        let k = match *looked {
                            Some(k) => k,
                            None => *looked.insert(brick(self, &mut dirty, &mut found, x, y, z)),
                        };
                        // Nothing stored there: nothing in the rest of the run either
                        let Some(k) = k else {
                            break;
                        };
                        let i = local(x, y, z);
                        if dirty[k].1[i] == AIR {
                            continue;
                        }
                        if blown.len() < 80 && picked(hash((x + o[0]) as u32 ^ hy_blown)) {
                            blown.push(dirty[k].1[i]);
                        }
                        dirty[k].1[i] = AIR;
                        holed_at[((x - cx + r) + (y - cy + r) * side) as usize] = true;
                    }
                    from = to + 1;
                }
            }
        }
        let holed: Vec<(i64, i64)> = (0..side * side).filter(|&i| holed_at[i as usize]).map(|i| (cx - r + i % side, cy - r + i / side)).collect();

        // Sand, gravel and snow with nothing under them any more slide down into the hole, each
        // column of them settling onto whatever's below. Not into bricks that were empty to begin
        // with: rays skip those, so what fell there wouldn't show. Only if there's some of it in
        // the bricks the hole was blown in or those over them, as there mostly isn't.
        let loose = |m: u8| matches!(m, SAND | GRAVEL | SNOW);
        let mut any_loose = dirty.iter().any(|(_, materials)| materials.iter().any(|&m| loose(m)));
        for bz in (cz + r).div_euclid(8)..=(cz + r + 24).div_euclid(8) {
            for by_ in (cy - r).div_euclid(8)..=(cy + r).div_euclid(8) {
                for bx_ in (cx - r).div_euclid(8)..=(cx + r).div_euclid(8) {
                    if any_loose || bx_ < 0 || by_ < 0 || bz < 0 || bx_ >= bx || by_ >= by || bz >= sz / 8 {
                        continue;
                    }
                    let value = self.brick_grid[self.cell([bx_, by_, bz])];
                    any_loose = if value & EMPTY_BRICK != 0 {
                        false
                    } else if value & SOLID_BRICK != 0 {
                        loose(value as u8)
                    } else {
                        let words = self.brick(((value & BRICK_INDEX) - 1) as usize);
                        words[16..16 + BRICK_VOXELS / 4].iter().any(|&w| (0..4).any(|j| loose((w >> (j * 8)) as u8)))
                    };
                }
            }
        }
        let columns = if any_loose { holed.clone() } else { Vec::new() };
        for (x, y) in columns {
            {
                // Where in this column the next loose voxel would come to rest, if anywhere
                let mut rest: Option<i64> = None;
                let mut open = 0;
                let mut z = cz - r - 1;
                // The brick the last voxel up the column was in, so it's looked up once
                let mut last: Option<(i64, Option<usize>)> = None;
                while z < sz && (z <= cz + r || open < 16) {
                    let here = match last {
                        Some((b, k)) if b == z.div_euclid(8) => k,
                        _ => last.insert((z.div_euclid(8), brick(self, &mut dirty, &mut found, x, y, z))).1,
                    };
                    let Some(k) = here else {
                        // Nothing stored here: nothing to fall, nowhere to fall to
                        rest = None;
                        open += 8;
                        z = (z / 8 + 1) * 8;
                        continue;
                    };
                    let i = local(x, y, z);
                    let m = dirty[k].1[i];
                    if m == AIR {
                        rest = rest.or(Some(z));
                        open += 1;
                    } else if loose(m) {
                        open = 0;
                        if let Some(to) = rest.filter(|&to| to < z) {
                            if let Some(j) = brick(self, &mut dirty, &mut found, x, y, to) {
                                dirty[j].1[local(x, y, to)] = m;
                                dirty[k].1[i] = AIR;
                                if blown.len() < 64 && hash3((x + o[0]) as u32, (y + o[1]) as u32, (z + o[2]) as u32 ^ 0x5a) % 9 == 0 {
                                    blown.push(m);
                                }
                                rest = Some(to + 1);
                            }
                        }
                    } else {
                        // Held up by this
                        rest = None;
                        open = 0;
                    }
                    z += 1;
                }
            }
        }

        // Grass and flowers with nothing under them any more fall in: only round where the hole is
        let plant = |m: u8| matches!(m, GRASS_BLADE | GRASS_TIP | DRY_GRASS | FLOWER_RED | FLOWER_YELLOW | FLOWER_PURPLE | FLOWER_WHITE | FLOWER_BLUE | FLOWER_PINK | REED | CATTAIL);
        // The columns holed and those beside them, marked in the square round the hole and a
        // column more each way, then a brick's worth of columns together
        let (nx, ny, nside) = (cx - r - 1, cy - r - 1, side + 2);
        let mut marked = vec![false; (nside * nside) as usize];
        for &(x, y) in &holed {
            for (a, b) in [(x, y), (x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                marked[((a - nx) + (b - ny) * nside) as usize] = true;
            }
        }
        let mut near: Vec<(i64, i64)> = Vec::new();
        for gy in ny.div_euclid(8)..=(ny + nside - 1).div_euclid(8) {
            for gx in nx.div_euclid(8)..=(nx + nside - 1).div_euclid(8) {
                for y in (gy * 8).max(ny)..(gy * 8 + 8).min(ny + nside) {
                    for x in (gx * 8).max(nx)..(gx * 8 + 8).min(nx + nside) {
                        if marked[((x - nx) + (y - ny) * nside) as usize] {
                            near.push((x, y));
                        }
                    }
                }
            }
        }
        // Grass only grows on the top of the ground, so only where the hole broke through to the
        // open air above does anything need to fall
        near.retain(|&(x, y)| self.material_at(x, y, cz + r + 1) == AIR);
        // Up through the bricks a brick's worth of columns share, each looked up once and passed
        // over if nothing grows anywhere in it: worked out once for each brick, and nothing more
        // grows as plants fall. Each column only hangs on what's under it in the same column, so
        // the columns go up together a brick at a time.
        let top = cz + r + 12;
        let mut grown: Vec<Option<bool>> = Vec::new();
        let mut first = 0;
        while first < near.len() {
            let (gx, gy) = (near[first].0.div_euclid(8), near[first].1.div_euclid(8));
            let end = first + near[first..].iter().take_while(|&&(x, y)| x.div_euclid(8) == gx && y.div_euclid(8) == gy).count();
            let (bx8, by8) = (gx * 8, gy * 8);
            for bz in (cz - r).div_euclid(8)..=top.div_euclid(8) {
                let (from, to) = ((bz * 8).max(cz - r), (bz * 8 + 7).min(top));
                let Some(k) = brick(self, &mut dirty, &mut found, bx8, by8, from) else {
                    continue;
                };
                if grown.len() <= k {
                    grown.resize(k + 1, None);
                }
                if !*grown[k].get_or_insert_with(|| dirty[k].1.iter().any(|&m| plant(m))) {
                    continue;
                }
                // The brick under, looked up if anything at the bottom of this one needs it
                let mut under: Option<Option<usize>> = None;
                for &(x, y) in &near[first..end] {
                    for z in from..=to {
                        if !plant(dirty[k].1[local(x, y, z)]) {
                            continue;
                        }
                        // What's under it, in the same brick unless it's the bottom of it
                        let below = if z.rem_euclid(8) != 0 {
                            dirty[k].1[local(x, y, z - 1)]
                        } else {
                            let b = match under {
                                Some(b) => b,
                                None => *under.insert(brick(self, &mut dirty, &mut found, bx8, by8, z - 1)),
                            };
                            b.map_or(AIR, |b| dirty[b].1[local(x, y, z - 1)])
                        };
                        if below == AIR {
                            dirty[k].1[local(x, y, z)] = AIR;
                        }
                    }
                }
            }
            first = end;
        }

        let dirty = self.clipped(dirty, clip);
        let changed = self.store(dirty);

        // Water can flow down into the hole
        if self.water.is_some() {
            let mut lowered = Vec::new();
            let (lo, hi) = clip.unwrap_or(([0, 0], [sx, sy]));
            for y in (cy - r).max(lo[1].max(0))..(cy + r + 1).min(hi[1].min(sy)) {
                for x in (cx - r).max(lo[0].max(0))..(cx + r + 1).min(hi[0].min(sx)) {
                    let i = self.column(x, y);
                    let old = self.water.as_ref().unwrap().ground[i] as i64;
                    let top = (0..old.min(sz)).rev().find(|&z| !is_passable(self.material_at(x, y, z))).map_or(0, |z| z + 1);
                    if top < old {
                        lowered.push((i, top as f32));
                    }
                }
            }
            let map = self.water.as_mut().unwrap();
            for (i, top) in lowered {
                map.ground[i] = top;
            }
        }
        (changed, blown)
    }

    /// Of bricks changed by carve() or paint(), those in the columns from `clip`'s first to its
    /// second, if it says, brick aligned
    fn clipped(&self, dirty: Vec<(usize, [u8; BRICK_VOXELS])>, clip: Option<([i64; 2], [i64; 2])>) -> Vec<(usize, [u8; BRICK_VOXELS])> {
        let Some((lo, hi)) = clip else {
            return dirty;
        };
        let room = [self.phys[0] as i64 / 8, self.phys[1] as i64 / 8];
        dirty
            .into_iter()
            .filter(|(cell, _)| {
                let at = self.cell_at(*cell);
                (0..2).all(|i| {
                    let b = (at[i] as i64 - self.wrap[i] as i64 / 8).rem_euclid(room[i]);
                    b >= lo[i].div_euclid(8) && b < hi[i].div_euclid(8)
                })
            })
            .collect()
    }

    /// Packs bricks changed by carve() or paint() back up, each in place where it was stored
    /// already if it was. Returns their cells.
    fn store(&mut self, dirty: Vec<(usize, [u8; BRICK_VOXELS])>) -> Vec<usize> {
        let mut changed = Vec::new();
        for (cell, materials) in dirty {
            let old = self.brick_grid[cell];
            let first = materials[0];
            let stored = old & (SOLID_BRICK | EMPTY_BRICK) == 0;
            let value = if materials.iter().all(|&m| m == first) {
                // What it was kept in isn't any more
                if stored {
                    self.drop_brick(((old & BRICK_INDEX) - 1) as usize);
                }
                if first == AIR { EMPTY_BRICK | 1 } else { SOLID_BRICK | first as u32 }
            } else if stored {
                let index = ((old & BRICK_INDEX) - 1) as usize;
                pack_brick(&materials, self.brick_mut(index));
                let foliage = if materials.iter().any(|&m| sways(m)) { FOLIAGE_BRICK } else { 0 };
                (index as u32 + 1) | foliage
            } else {
                self.new_brick(&materials)
            };
            if value != old {
                self.brick_grid[cell] = value;
            }
            changed.push(cell);
        }
        changed
    }

    /// Stains the ground and walls round `center` with `material`, in a ragged splotch `radius`
    /// voxels round: a scorch mark, frost, a pool of venom or blood. Only what's open to the air
    /// takes it. With `burn`, grass and flowers there are burnt away. Returns the cells of the
    /// bricks that changed.
    pub fn paint(&mut self, center: [f64; 3], radius: f64, material: u8, burn: bool) -> Vec<usize> {
        self.paint_in(center, radius, material, burn, None)
    }

    /// The same, but only in the columns `clip` says, if it does, see carve_in()
    pub fn paint_in(&mut self, center: [f64; 3], radius: f64, material: u8, burn: bool, clip: Option<([i64; 2], [i64; 2])>) -> Vec<usize> {
        let [sx, sy, sz] = self.size.map(i64::from);
        // Spattered by where each voxel is in the whole landscape, see carve()
        let o = self.origin;
        let plant = |m: u8| matches!(m, GRASS_BLADE | GRASS_TIP | DRY_GRASS | FLOWER_RED | FLOWER_YELLOW | FLOWER_PURPLE | FLOWER_WHITE | FLOWER_BLUE | FLOWER_PINK | REED | CATTAIL | FERN);
        let r = radius.ceil() as i64 + 1;
        let [cx, cy, cz] = center.map(|v| v.floor() as i64);
        let mut edits: Vec<([i64; 3], u8)> = Vec::new();
        for z in cz - r..=cz + r {
            for y in cy - r..=cy + r {
                for x in cx - r..=cx + r {
                    let d = ((x as f64 + 0.5 - center[0]).powi(2) + (y as f64 + 0.5 - center[1]).powi(2) + (z as f64 + 0.5 - center[2]).powi(2)).sqrt();
                    // Thinning out in spatters towards the edge
                    let spatter = 0.55 + 0.6 * unit(hash3((x + o[0]) as u32, (y + o[1]) as u32 ^ 0x5ca7, (z + o[2]) as u32)) as f64;
                    if d > radius * spatter {
                        continue;
                    }
                    let here = self.material_at(x, y, z);
                    if burn && plant(here) {
                        edits.push(([x, y, z], AIR));
                        continue;
                    }
                    if is_passable(here) || here == WATER || plant(here) {
                        continue;
                    }
                    let open = [[0, 0, 1], [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, -1]].iter().any(|o| {
                        let m = self.material_at(x + o[0], y + o[1], z + o[2]);
                        is_passable(m) || plant(m)
                    });
                    if open {
                        edits.push(([x, y, z], material));
                    }
                }
            }
        }
        let mut dirty: Vec<(usize, [u8; BRICK_VOXELS])> = Vec::new();
        let mut found: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for ([x, y, z], m) in edits {
            if x < 0 || y < 0 || z < 0 || x >= sx || y >= sy || z >= sz {
                continue;
            }
            let cell = self.cell([x / 8, y / 8, z / 8]);
            let k = match found.get(&cell) {
                Some(&k) => k,
                None => {
                    let value = self.brick_grid[cell];
                    if value & EMPTY_BRICK != 0 {
                        continue;
                    }
                    let mut materials = [AIR; BRICK_VOXELS];
                    if value & SOLID_BRICK != 0 {
                        materials = [value as u8; BRICK_VOXELS];
                    } else {
                        unpack_brick(self.brick(((value & BRICK_INDEX) - 1) as usize), &mut materials);
                    }
                    dirty.push((cell, materials));
                    found.insert(cell, dirty.len() - 1);
                    dirty.len() - 1
                }
            };
            dirty[k].1[(x % 8 + (y % 8) * 8 + (z % 8) * 64) as usize] = m;
        }
        let dirty = self.clipped(dirty, clip);
        self.store(dirty)
    }

    /// Puts `materials` in brick `b`, counted from the world's first, which has to be in it: for
    /// what's built and dug, see src\bases.rs. Returns its cell.
    pub fn set_brick(&mut self, b: [i64; 3], materials: &[u8; BRICK_VOXELS]) -> usize {
        let cell = self.cell(b);
        self.store(vec![(cell, *materials)]);
        cell
    }

    /// The words of the stored brick in `cell`, if it's a stored brick
    pub fn brick_words(&self, cell: usize) -> Option<(usize, &[u32])> {
        let value = self.brick_grid[cell];
        if value & (SOLID_BRICK | EMPTY_BRICK) != 0 {
            return None;
        }
        let index = ((value & BRICK_INDEX) - 1) as usize;
        Some((index, self.brick(index)))
    }

    /// Where cell `cell` of the brick grid is in it, x, y and z, as the GPU's copy of it has it
    pub fn cell_at(&self, cell: usize) -> [u32; 3] {
        let (bx, by) = ((self.phys[0] / 8) as usize, (self.phys[1] / 8) as usize);
        [(cell % bx) as u32, ((cell / bx) % by) as u32, (cell / (bx * by)) as u32]
    }

    /// How many bricks there's room for along x, y and z in the brick grid, see `phys`
    pub fn grid_size(&self) -> [u32; 3] {
        [self.phys[0] / 8, self.phys[1] / 8, self.size[2] / 8]
    }

    /// Converts occupancy bits packed like voxelize::voxelize_mesh() into bricks of one material.
    pub fn from_packed_occupancy(size: [u32; 3], occupancy: &[u128], material: u8) -> Self {
        let [sx, sy, sz] = size.map(|n| n as usize);
        let [bx, by, bz] = [sx / 8, sy / 8, sz / 8];
        let (tx, ty) = (sx / 4, sy / 4);
        // Each texel is 4x4x8 voxels, so a brick covers 2x2x1 texels
        let texel = |x: usize, y: usize, z: usize| occupancy[x + (y + z * ty) * tx];

        let mut brick_grid = vec![0; bx * by * bz];
        let mut bricks = Vec::new();
        let mut materials = [AIR; BRICK_VOXELS];

        for z in 0..bz {
            for y in 0..by {
                for x in 0..bx {
                    let texels = [
                        texel(2 * x, 2 * y, z),
                        texel(2 * x + 1, 2 * y, z),
                        texel(2 * x, 2 * y + 1, z),
                        texel(2 * x + 1, 2 * y + 1, z),
                    ];
                    let cell = if texels.iter().all(|&t| t == 0) {
                        0
                    } else if texels.iter().all(|&t| t == u128::MAX) {
                        SOLID_BRICK | material as u32
                    } else {
                        for (i, m) in materials.iter_mut().enumerate() {
                            let (vx, vy, vz) = (x * 8 + i % 8, y * 8 + (i / 8) % 8, z * 8 + i / 64);
                            let bit = (vx % 4) * 32 + (vy % 4) + (vz % 8) * 4;
                            let solid = texel(vx / 4, vy / 4, vz / 8) >> bit & 1 == 1;
                            *m = if solid { material } else { AIR };
                        }
                        push_brick(&materials, &mut bricks)
                    };
                    brick_grid[x + (y + z * by) * bx] = cell;
                }
            }
        }

        World::new(size, [0; 3], brick_grid, bricks, 1)
    }
}

/// Chebyshev distance from every brick to the nearest non-empty one, capped at u16::MAX.
/// Two passes over the 26 neighbors are exact for this metric. Outside the world counts as empty.
fn empty_distances(grid: &[u32], dims: [usize; 3]) -> Vec<u16> {
    let [bx, by, bz] = dims;
    let mut distances: Vec<u16> = grid.iter().map(|&c| if c == 0 { u16::MAX } else { 0 }).collect();

    let mut pass = |forward: bool| {
        let order = |i: usize, n: usize| if forward { i } else { n - 1 - i };
        let sign: i64 = if forward { 1 } else { -1 };
        for zi in 0..bz {
            let z = order(zi, bz);
            for yi in 0..by {
                let y = order(yi, by);
                for xi in 0..bx {
                    let x = order(xi, bx);
                    let i = x + (y + z * by) * bx;
                    let mut d = distances[i];
                    if d == 0 {
                        continue;
                    }
                    // The 13 neighbors that come earlier in this pass's order
                    for (dx, dy, dz) in [
                        (-1, -1, -1), (0, -1, -1), (1, -1, -1),
                        (-1, 0, -1), (0, 0, -1), (1, 0, -1),
                        (-1, 1, -1), (0, 1, -1), (1, 1, -1),
                        (-1, -1, 0), (0, -1, 0), (1, -1, 0),
                        (-1, 0, 0),
                    ] {
                        let (nx, ny, nz) = (x as i64 + dx * sign, y as i64 + dy * sign, z as i64 + dz * sign);
                        if nx < 0 || ny < 0 || nz < 0 || nx >= bx as i64 || ny >= by as i64 || nz >= bz as i64 {
                            continue;
                        }
                        let n = nx as usize + (ny as usize + nz as usize * by) * bx;
                        d = d.min(distances[n].saturating_add(1));
                    }
                    distances[i] = d;
                }
            }
        }
    };
    pass(true);
    pass(false);

    distances
}

/// What all of a brick is made of, if it's wholly inside the first of `shapes` reaching into it
/// and that's all one material: then none of the rest reach any of its voxels, and every one of
/// them is that. `lo` is its first voxel, and `above` whether it's all over the ground. Shapes are
/// convex, so it's wholly inside one if the voxels at its corners are.
fn uniform_brick(shapes: &[(&Shape, u32)], lo: [i64; 3], voxel: f32, above: bool) -> Option<u8> {
    let p0 = lo.map(|v| (v as f32 + 0.5) * voxel);
    let p1 = p0.map(|v| v + 7.0 * voxel);
    let (shape, _) = shapes.iter().find(|(s, _)| (0..3).all(|i| p1[i] >= s.min[i] - voxel && p0[i] <= s.max[i] + voxel))?;
    let plain = shape.pattern == Pattern::Plain && shape.accent == AIR && !shape.ragged && shape.rough <= 0.0;
    let placed = match shape.placement {
        Placement::Clear => true,
        Placement::Above => above,
        _ => false,
    };
    let inside = || (0..8).all(|k| shape.depth([0, 1, 2].map(|i| if k >> i & 1 == 0 { p0[i] } else { p1[i] }), voxel) <= 1.0);
    (plain && placed && inside()).then_some(shape.material)
}

/// Returns the grid cell for a brick, storing its voxels in `bricks` unless it's uniform.
fn push_brick(materials: &[u8; BRICK_VOXELS], bricks: &mut Vec<u32>) -> u32 {
    let first = materials[0];
    if materials.iter().all(|&m| m == first) {
        return if first == AIR {
            0
        } else {
            SOLID_BRICK | first as u32
        };
    }

    let index = bricks.len() / BRICK_WORDS;
    if bricks.len() + BRICK_WORDS > bricks.capacity() {
        // Half again at most, and no more than SPARE_BRICKS: doubling a world's hundreds of
        // megabytes of bricks for a hole blown in it held as many again in the computer's memory
        bricks.reserve_exact((bricks.len() / 2).clamp(64 * BRICK_WORDS, SPARE_BRICKS * BRICK_WORDS));
    }
    bricks.resize(bricks.len() + BRICK_WORDS, 0);
    let brick = &mut bricks[index * BRICK_WORDS..];
    for (i, &m) in materials.iter().enumerate() {
        if m != AIR {
            brick[i / 32] |= 1 << (i % 32);
        }
        brick[16 + i / 4] |= (m as u32) << ((i % 4) * 8);
    }
    let foliage = if materials.iter().any(|&m| sways(m)) { FOLIAGE_BRICK } else { 0 };
    (index as u32 + 1) | foliage
}

/// Packs a brick's `materials` into its BRICK_WORDS `words`, as push_brick() does: a bit for each
/// voxel that isn't air, then the materials four to a word, the first in the lowest byte
fn pack_brick(materials: &[u8; BRICK_VOXELS], words: &mut [u32]) {
    for (word, voxels) in words[..16].iter_mut().zip(materials.chunks_exact(32)) {
        *word = voxels.iter().enumerate().fold(0, |bits, (j, &m)| bits | ((m != AIR) as u32) << j);
    }
    for (word, four) in words[16..BRICK_WORDS].iter_mut().zip(materials.chunks_exact(4)) {
        *word = u32::from_le_bytes([four[0], four[1], four[2], four[3]]);
    }
}

/// The materials of a brick stored as `words`, see pack_brick()
fn unpack_brick(words: &[u32], materials: &mut [u8; BRICK_VOXELS]) {
    for (four, word) in materials.chunks_exact_mut(4).zip(&words[16..BRICK_WORDS]) {
        four.copy_from_slice(&word.to_le_bytes());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerrainSettings {
    pub width_m: u32,
    pub height_m: u32,
    pub voxels_per_meter: u32,
    pub seed: u32,
    /// Little tufts of grass a few voxels tall all over the meadows
    pub grass_tufts: bool,
}

impl TerrainSettings {
    pub fn size(&self) -> [u32; 3] {
        let v = self.voxels_per_meter;
        [self.width_m * v, self.width_m * v, self.height_m * v]
    }
}

/// Generates the piece of the endless landscape whose corner is at `origin`, in voxels from the
/// corner of the whole landscape, as big as the settings say, and builds it into voxels at their
/// resolution.
/// How many threads building the world spreads over, 0 for one per core
static THREADS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Whether the world's built in the background from here on, while the game goes on, see
/// leave_cores(): the threads building it then give way to the game's, see give_way()
static BACKGROUND: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Leaves `spare` cores free while building the world from here on, so the game goes on running
/// smoothly while the strips it moves on by are built in the background
pub fn leave_cores(spare: usize) {
    THREADS.store(cores().saturating_sub(spare).max(2), std::sync::atomic::Ordering::Relaxed);
    BACKGROUND.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// How many cores the game can run on: as many as the computer has, or as few as it's been held to,
/// as with Task Manager's "Set affinity", which available_parallelism() doesn't count on Windows,
/// where held to 2 of 16 the world was built on 14 threads
pub fn cores() -> usize {
    let all = thread::available_parallelism().map_or(4, |n| n.get());
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn GetProcessAffinityMask(process: *mut std::ffi::c_void, mine: *mut usize, system: *mut usize) -> i32;
        }
        let (mut mine, mut system) = (0usize, 0usize);
        if unsafe { GetProcessAffinityMask(GetCurrentProcess(), &mut mine, &mut system) } != 0 && mine != 0 {
            return all.min(mine.count_ones() as usize);
        }
    }
    all
}

/// Whether the world being built in the background is wanted soon, see hurry()
static HURRY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether what's built in the background is wanted soon, as the edge of the world is getting near
/// where it'd be seen: then it doesn't give way to the game, see give_way()
pub fn hurry(on: bool) {
    HURRY.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// A thread building the world in the background gives way to the game's own and the graphics
/// driver's, if the world's built in the background by now: at the same priority, 14 of them on 16
/// cores held up handing frames over to be shown by up to half a second while a strip was built,
/// at Low, where frames are drawn as fast as they can be, and held to 4 cores 64 frames in 100 s
/// took over 20 ms. It still has all the time they leave, which on two cores, the game drawing as
/// fast as it can, wasn't enough to keep up: so not while it's hurried, see hurry(). Each thread
/// building the world calls this as it starts on each step, and now and then along the longest, so
/// a hurry's taken up straight away. Only a load of an atomic unless it's to change, and one more
/// for `--set crash_thread`, see crash.rs.
pub fn give_way() {
    crate::crash::worker();
    if !BACKGROUND.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetCurrentThread() -> *mut std::ffi::c_void;
            fn SetThreadPriority(thread: *mut std::ffi::c_void, priority: i32) -> i32;
        }
        const THREAD_PRIORITY_NORMAL: i32 = 0;
        const THREAD_PRIORITY_BELOW_NORMAL: i32 = -1;
        thread_local! {
            /// What this thread's priority was last set to, if it was
            static SET: std::cell::Cell<Option<i32>> = const { std::cell::Cell::new(None) };
        }
        let priority = if HURRY.load(std::sync::atomic::Ordering::Relaxed) { THREAD_PRIORITY_NORMAL } else { THREAD_PRIORITY_BELOW_NORMAL };
        if SET.get() != Some(priority) {
            SET.set(Some(priority));
            unsafe {
                SetThreadPriority(GetCurrentThread(), priority);
            }
        }
    }
}

fn threads() -> usize {
    match THREADS.load(std::sync::atomic::Ordering::Relaxed) {
        0 => cores(),
        n => n,
    }
}

thread_local! {
    /// Told each step generate_terrain() starts on this thread, see on_step()
    static ON_STEP: std::cell::RefCell<Option<Box<dyn Fn(usize)>>> = const { std::cell::RefCell::new(None) };
}

/// Has generate_terrain() tell `step` the number of each step it starts on this thread, for the
/// loading screen: the ground, the rivers, what's on the land, the voxels, then grass and water.
/// None stops it.
pub fn on_step(step: Option<Box<dyn Fn(usize)>>) {
    ON_STEP.with(|s| *s.borrow_mut() = step);
}

fn step(n: usize) {
    give_way();
    ON_STEP.with(|s| {
        if let Some(step) = &*s.borrow() {
            step(n)
        }
    });
}

pub fn generate_terrain(settings: &TerrainSettings, origin: [i64; 2], rivers: &RiverCache) -> (Terrain, World) {
    let size = settings.size();
    generate_area(settings, origin, [size[0], size[1]], rivers)
}

/// Generates the rectangle of the endless landscape `size` voxel columns across whose corner is at
/// `origin`, as tall as the settings say, the same way generate_terrain() does a whole piece of it.
/// Everything in it only depends on where it is in the whole landscape, so a rectangle built in one
/// go and the same built a strip at a time come out the same, and so do strips built next to each
/// other: the world moves on a strip at a time, see main.rs.
pub fn generate_area(settings: &TerrainSettings, origin: [i64; 2], size: [u32; 2], rivers: &RiverCache) -> (Terrain, World) {
    let start = std::time::Instant::now();

    step(0);
    let mut heights = Heights::new(settings, origin, [size[0] as usize, size[1] as usize], settings.voxels_per_meter);
    let shaped = start.elapsed();
    step(1);
    let mut terrain = Terrain::new(settings, &heights);
    terrain.carve_rivers(&mut heights, rivers);
    let carved = start.elapsed();
    step(2);
    terrain.place(&heights, rivers);
    let placed = start.elapsed();
    step(3);
    let mut world = terrain.build_from(&heights, 0, settings.size()[2]);
    let built = start.elapsed();
    step(4);
    let (grass, mut water) = terrain.grass_and_water(&heights);
    world.grass = Some(grass);
    terrain.raise_water_ground(&world, &mut water);
    world.water = Some(water);

    let ms = |d: std::time::Duration| d.as_millis();
    println!(
        "Generated terrain around ({}, {}), {}x{}: {} bricks stored, {} MB ({:?}: ground {} ms, {} rivers {} ms, what's on the land {} ms, voxels {} ms, grass and water {} ms)",
        origin[0],
        origin[1],
        size[0],
        size[1],
        world.brick_count(),
        world.bricks.len() * 4 / (1024 * 1024),
        start.elapsed(),
        ms(shaped),
        terrain.rivers.len(),
        ms(carved - shaped),
        ms(placed - carved),
        ms(built - placed),
        ms(start.elapsed() - built)
    );

    (terrain, world)
}

/// Everything about the landscape from `origin`, `size` columns across, that doesn't depend on the
/// resolution, placed at that of `settings`: to build any part of it into voxels at any resolution,
/// see Terrain::build(), like the finer detail round the camera
pub fn place_for(settings: &TerrainSettings, origin: [i64; 2], size: [u32; 2], rivers: &RiverCache) -> Terrain {
    let mut heights = Heights::new(settings, origin, [size[0] as usize, size[1] as usize], settings.voxels_per_meter);
    let mut terrain = Terrain::new(settings, &heights);
    terrain.carve_rivers(&mut heights, rivers);
    terrain.place(&heights, rivers);
    terrain
}

/// What a piece of the landscape has besides its voxels, all in meters from the corner of the
/// whole landscape, and only what's its own, see Terrain::own_lo: what's kept of each strip the
/// world's made of as it moves on, see main.rs
#[derive(Clone, Default)]
pub struct PieceFeatures {
    /// The piece, from and to
    pub lo: [f32; 2],
    pub hi: [f32; 2],
    pub props: Vec<Prop>,
    pub lights: Vec<Light>,
    pub sites: Vec<Site>,
    pub camps: Vec<Camp>,
    /// Where water wells up of its own, see Terrain::piece_springs()
    pub springs: Vec<[f32; 4]>,
    /// See Terrain::waterfalls()
    pub waterfalls: Vec<([f32; 3], [f32; 2], f32)>,
}

type Vec3 = [f32; 3];

fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(a: Vec3, s: f32) -> Vec3 {
    a.map(|v| v * s)
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn lerp3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn normalize(a: Vec3) -> Vec3 {
    let length = dot(a, a).sqrt();
    if length > 0.0 { scale(a, 1.0 / length) } else { [0.0, 0.0, 1.0] }
}

/// Turns `dir` by `angle` radians, towards a side picked by `spin`.
fn bend(dir: Vec3, angle: f32, spin: f32) -> Vec3 {
    let helper = if dir[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
    let u = normalize(cross(dir, helper));
    let v = cross(dir, u);
    let side = add(scale(u, spin.cos()), scale(v, spin.sin()));
    normalize(add(scale(dir, angle.cos()), scale(side, angle.sin())))
}

/// Deterministic random numbers for building one object.
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = hash(self.0);
        unit(self.0)
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }

    /// A random direction, flattened out by `flat` towards the ground plane
    fn direction(&mut self, flat: f32) -> Vec3 {
        let z = self.range(-1.0, 1.0) * (1.0 - flat);
        let angle = self.range(0.0, TAU);
        let r = (1.0 - z * z).sqrt();
        [r * angle.cos(), r * angle.sin(), z]
    }
}

/// How far rivers drop at each waterfall, in meters
const FALL_HEIGHT: f32 = 3.5;

/// A point along a river's course, in meters.
struct RiverPoint {
    x: f32,
    y: f32,
    surface: f32,
    half_width: f32,
    /// Just before a waterfall, where the bed comes up to hold the pool behind it
    lip: bool,
}

/// How the surface of a shape is made up
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pattern {
    /// All `material`, with `accent` sprinkled in
    Plain,
    /// Laid in courses of bricks with mortar between, some bricks weathered to `accent`
    Bricks,
    /// Carved with bands round it and zigzags between them
    Carved,
    /// Old stone, roofs or paving, weathered, stained and gone green, see castles::aged()
    Aged(castles::Age),
    /// A clump of leaves, its outside all leaves standing out at different depths in these
    /// shades, see trees::leafy()
    Leafy(trees::Canopy),
    /// Weathered rock with moss creeping over its top, this much in 255, see trees::rock()
    Rock(u8),
}

/// Where a shape goes in relation to the ground
#[derive(Clone, Copy, PartialEq, Eq)]
enum Placement {
    /// Only above the ground, like trees and buildings
    Above,
    /// Cuts air out of the ground, like caves
    Carve,
    /// Air, above the ground and in it, like the rooms and passages of buildings
    Clear,
    /// In the hollows cut out of the ground, like crystals in caves
    Inside,
}

/// A piece of a tree, plant, rock or building, in meters: a capsule whose radius goes linearly
/// from `radius_a` at `a` to `radius_b` at `b` (a sphere when they're the same point), a flat
/// ellipse like a leaf, or a box.
struct Shape {
    a: Vec3,
    b: Vec3,
    radius_a: f32,
    radius_b: f32,
    /// For flat shapes: their axes, and the half-size along each. `a` is the middle.
    flat: Option<([Vec3; 3], Vec3)>,
    /// For boxes: the half-size along each axis. `a` is the middle.
    boxed: Option<Vec3>,
    material: u8,
    /// Sprinkled in among `material`, AIR for none
    accent: u8,
    /// Ragged, see-through edges, for clumps of foliage
    ragged: bool,
    placement: Placement,
    pattern: Pattern,
    /// How far in meters its surface is pushed in and out, lumpily, so a cave isn't a smooth tube
    rough: f32,
    /// How high in meters its bottom is at the lowest, for a cave's floor, flatter than its roof
    floor: f32,
    // Bounds
    min: Vec3,
    max: Vec3,
}

impl Shape {
    fn capsule(a: Vec3, b: Vec3, radius_a: f32, radius_b: f32, material: u8, accent: u8, ragged: bool) -> Self {
        let r = radius_a.max(radius_b);
        Shape {
            a,
            b,
            radius_a,
            radius_b,
            flat: None,
            boxed: None,
            material,
            accent,
            ragged,
            placement: Placement::Above,
            pattern: Pattern::Plain,
            rough: 0.0,
            floor: f32::MIN,
            min: [0, 1, 2].map(|i| a[i].min(b[i]) - r),
            max: [0, 1, 2].map(|i| a[i].max(b[i]) + r),
        }
    }

    /// A box from `lo` to `hi`
    fn block(lo: Vec3, hi: Vec3, material: u8, accent: u8) -> Self {
        let center = [0, 1, 2].map(|i| (lo[i] + hi[i]) * 0.5);
        let half = [0, 1, 2].map(|i| (hi[i] - lo[i]).abs() * 0.5);
        let reach = half[0].max(half[1]).max(half[2]);
        Shape {
            a: center,
            b: center,
            radius_a: reach,
            radius_b: reach,
            flat: None,
            boxed: Some(half),
            material,
            accent,
            ragged: false,
            placement: Placement::Above,
            pattern: Pattern::Plain,
            rough: 0.0,
            floor: f32::MIN,
            min: [0, 1, 2].map(|i| lo[i].min(hi[i])),
            max: [0, 1, 2].map(|i| lo[i].max(hi[i])),
        }
    }

    /// An upright ellipsoid with these radii
    fn ellipsoid(center: Vec3, radii: Vec3, material: u8, accent: u8) -> Self {
        let reach = radii[0].max(radii[1]).max(radii[2]);
        Shape {
            a: center,
            b: center,
            radius_a: reach,
            radius_b: reach,
            flat: Some(([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], radii)),
            boxed: None,
            material,
            accent,
            ragged: false,
            placement: Placement::Above,
            pattern: Pattern::Plain,
            rough: 0.0,
            floor: f32::MIN,
            min: [0, 1, 2].map(|i| center[i] - radii[i]),
            max: [0, 1, 2].map(|i| center[i] + radii[i]),
        }
    }

    /// Empty space from `lo` to `hi`, cut out of buildings and the ground alike
    fn room(lo: Vec3, hi: Vec3) -> Self {
        Shape::block(lo, hi, AIR, AIR).placed(Placement::Clear)
    }

    fn placed(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    fn patterned(mut self, pattern: Pattern) -> Self {
        self.pattern = pattern;
        self
    }

    /// Its surface pushed in and out by up to `rough` meters, lumpily, and nothing of it below
    /// `floor` meters high
    fn roughened(mut self, rough: f32, floor: f32) -> Self {
        self.rough = rough;
        self.floor = floor;
        self.min = self.min.map(|v| v - rough);
        self.max = self.max.map(|v| v + rough);
        self.min[2] = self.min[2].max(floor - rough * 0.3);
        self
    }

    /// Whether `p` is inside it, its squared depth being `depth`, see depth(). Rough shapes are
    /// lumpy, their surface pushed in and out by the noise only near it, as working it out costs.
    fn holds(&self, p: Vec3, depth: f32) -> bool {
        match self.pattern {
            Pattern::Leafy(_) => return trees::leafy_holds(self, p, depth),
            Pattern::Rock(_) => return trees::rock_holds(self, p, depth),
            _ => {}
        }
        if self.rough <= 0.0 {
            return depth <= 1.0;
        }
        // The floor bumpy a little, worked out only near it
        let swell = self.rough * 0.25;
        if p[2] < self.floor || p[2] < self.floor + swell && p[2] < self.floor + (cave_noise(p) * 0.5 + 0.5) * swell {
            return false;
        }
        let reach = self.radius_a.max(self.radius_b).max(0.1);
        let d = depth.sqrt();
        let margin = self.rough / reach;
        if d < 1.0 - margin {
            return true;
        }
        if d > 1.0 + margin {
            return false;
        }
        // The big lumps first, and the small ones only where they could make a difference
        let big = cave_lumps(p);
        if (d - 1.0 - big * margin).abs() > CAVE_KNOBS * margin {
            return d < 1.0 + big * margin;
        }
        d < 1.0 + (big + cave_knobs(p)).clamp(-1.0, 1.0) * margin
    }

    /// Whether none of it, roughness and all, comes within `m` meters of `p`: worked out from how
    /// fast its depth can change, so it's sure but not tight, as for telling quickly that a brick
    /// deep in the rock is nowhere near it
    fn clear_of(&self, p: Vec3, m: f32) -> bool {
        let m = m + self.rough;
        if (0..3).any(|i| p[i] < self.min[i] - m || p[i] > self.max[i] + m) {
            return true;
        }
        let d = sub(p, self.a);
        if let Some(half) = &self.boxed {
            return (0..3).any(|i| d[i].abs() > half[i] + m);
        }
        if let Some((axes, half)) = &self.flat {
            let least = half[0].min(half[1]).min(half[2]).max(1e-3);
            let out = (0..3).map(|i| (dot(axes[i], d) / half[i].max(1e-3)).powi(2)).sum::<f32>().sqrt();
            return out > 1.0 + m / least;
        }
        let ab = sub(self.b, self.a);
        let length2 = dot(ab, ab);
        let t = if length2 > 0.0 { (dot(d, ab) / length2).clamp(0.0, 1.0) } else { 0.0 };
        let e = sub(d, scale(ab, t));
        dot(e, e).sqrt() > self.radius_a.max(self.radius_b) + m
    }

    /// A long, thin, upright ellipsoid going `along` (level), `length` long, `width` wide and
    /// `height` high, like a slice of a crack in the ground
    fn slab(center: Vec3, along: Vec3, length: f32, width: f32, height: f32) -> Self {
        let along = normalize([along[0], along[1], 0.0]);
        let side = [-along[1], along[0], 0.0];
        let half = [length * 0.5, width * 0.5, height * 0.5];
        let reach = half[0].max(half[1]).max(half[2]);
        Shape {
            a: center,
            b: center,
            radius_a: reach,
            radius_b: reach,
            flat: Some(([along, side, [0.0, 0.0, 1.0]], half)),
            boxed: None,
            material: AIR,
            accent: AIR,
            ragged: false,
            placement: Placement::Carve,
            pattern: Pattern::Plain,
            rough: 0.0,
            floor: f32::MIN,
            min: [center[0] - half[0].max(half[1]), center[1] - half[0].max(half[1]), center[2] - half[2]],
            max: [center[0] + half[0].max(half[1]), center[1] + half[0].max(half[1]), center[2] + half[2]],
        }
    }

    /// What a voxel inside the shape is made of. `p` is its middle, `h` a hash of where it is,
    /// `ground` how high the ground under it is, in meters.
    fn material_at(&self, p: Vec3, voxel: f32, h: u32, ground: f32) -> u8 {
        let sprinkled = |chance: f32| self.accent != AIR && unit(h >> 16) < chance;
        let dark = if matches!(self.material, SANDSTONE | CLAY_POT) { SANDSTONE_DARK } else { MORTAR };
        match self.pattern {
            Pattern::Aged(age) => castles::aged(self, age, p, voxel, h, ground),
            Pattern::Leafy(canopy) => trees::leafy(self, canopy, p, h),
            Pattern::Rock(moss) => trees::rock(self, moss, p, voxel, h),
            Pattern::Plain => {
                if sprinkled(0.15) {
                    self.accent
                } else {
                    self.material
                }
            }
            Pattern::Bricks => {
                const COURSE: f32 = 0.375;
                const LENGTH: f32 = 0.75;
                let course = (p[2] / COURSE).floor();
                if p[2] - course * COURSE < voxel {
                    return dark;
                }
                // Every other course is offset by half a brick
                let along = p[0] + p[1] + if (course as i32).rem_euclid(2) == 1 { LENGTH * 0.5 } else { 0.0 };
                let brick = (along / LENGTH).floor();
                if along - brick * LENGTH < voxel {
                    return dark;
                }
                // Whole bricks weather, not just the odd voxel
                let weathered = unit(hash3(course as i32 as u32, brick as i32 as u32, 0xb71c)) < 0.2;
                if self.accent != AIR && weathered || sprinkled(0.04) {
                    self.accent
                } else {
                    self.material
                }
            }
            Pattern::Carved => {
                // Bands round it, with zigzags carved in the stone between them
                const BAND: f32 = 1.2;
                let rise = (p[2] - self.min[2]).rem_euclid(BAND);
                if rise < voxel * 1.5 {
                    return dark;
                }
                let around = if self.boxed.is_some() {
                    p[0] + p[1]
                } else {
                    (p[1] - self.a[1]).atan2(p[0] - self.a[0]) * self.radius_a.max(0.3)
                };
                let v = rise / BAND;
                let zig = ((around / 0.5 + 2.0 * v).rem_euclid(1.0) - 0.5).abs();
                let zag = ((around / 0.5 - 2.0 * v).rem_euclid(1.0) - 0.5).abs();
                if v > 0.15 && v < 0.85 && zig.min(zag) < 0.1 {
                    dark
                } else if sprinkled(0.08) {
                    self.accent
                } else {
                    self.material
                }
            }
        }
    }

    fn sphere(center: Vec3, radius: f32, material: u8, accent: u8, ragged: bool) -> Self {
        Shape::capsule(center, center, radius, radius, material, accent, ragged)
    }

    fn wood(a: Vec3, b: Vec3, radius_a: f32, radius_b: f32, bark: u8) -> Self {
        Shape::capsule(a, b, radius_a, radius_b, bark, AIR, false)
    }

    /// A flat ellipse `length` long in the direction `along`, `width` wide, and turned to face `facing` as much as it can.
    fn leaf(center: Vec3, along: Vec3, facing: Vec3, length: f32, width: f32, material: u8, accent: u8) -> Self {
        let along = normalize(along);
        let mut side = cross(facing, along);
        if dot(side, side) < 1e-6 {
            side = cross([0.0, 1.0, 0.0], along);
        }
        let side = normalize(side);
        let normal = cross(along, side);
        let reach = (length * 0.5).max(width * 0.5);
        Shape {
            a: center,
            b: center,
            radius_a: reach,
            radius_b: reach,
            flat: Some(([along, side, normal], [length * 0.5, width * 0.5, 0.01])),
            boxed: None,
            material,
            accent,
            ragged: false,
            placement: Placement::Above,
            pattern: Pattern::Plain,
            rough: 0.0,
            floor: f32::MIN,
            min: center.map(|v| v - reach),
            max: center.map(|v| v + reach),
        }
    }

    /// Squared distance from the shape's middle relative to its size there: <= 1 is inside.
    /// Nothing is thinner than about a voxel (`voxel` meters), so small parts don't fall apart.
    fn depth(&self, p: Vec3, voxel: f32) -> f32 {
        let d = sub(p, self.a);
        if let Some(half) = &self.boxed {
            return (0..3).map(|i| d[i].abs() / half[i].max(0.5 * voxel)).fold(0.0, f32::max).powi(2);
        }
        if let Some((axes, half)) = &self.flat {
            return (0..3).map(|i| (dot(axes[i], d) / half[i].max(0.6 * voxel)).powi(2)).sum();
        }
        let ab = sub(self.b, self.a);
        let length2 = dot(ab, ab);
        let t = if length2 > 0.0 { (dot(d, ab) / length2).clamp(0.0, 1.0) } else { 0.0 };
        let r = (self.radius_a + (self.radius_b - self.radius_a) * t).max(0.5 * voxel);
        let e = sub(d, scale(ab, t));
        dot(e, e) / (r * r)
    }
}

/// Trees, plants and rocks: shapes that get rasterized into whichever bricks they touch.
struct Object {
    /// Earlier shapes win where they overlap, so wood goes before leaves
    shapes: Vec<Shape>,
    seed: u32,
    min: Vec3,
    max: Vec3,
}

impl Object {
    fn new(shapes: Vec<Shape>, seed: u32) -> Self {
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for shape in &shapes {
            for i in 0..3 {
                min[i] = min[i].min(shape.min[i]);
                max[i] = max[i].max(shape.max[i]);
            }
        }
        Object {
            shapes,
            seed,
            min,
            max,
        }
    }
}

/// Where a building stands and which way it faces, for laying it out: `u` goes the way it faces,
/// `v` across, both in meters from its middle. It faces along x or y, so boxes stay boxes.
#[derive(Clone, Copy)]
struct Frame {
    x: f32,
    y: f32,
    f: [f32; 2],
}

impl Frame {
    fn point(&self, u: f32, v: f32, z: f32) -> Vec3 {
        [self.x + self.f[0] * u - self.f[1] * v, self.y + self.f[1] * u + self.f[0] * v, z]
    }

    fn block(&self, u: [f32; 2], v: [f32; 2], z: [f32; 2], material: u8, accent: u8) -> Shape {
        Shape::block(self.point(u[0], v[0], z[0]), self.point(u[1], v[1], z[1]), material, accent)
    }

    fn room(&self, u: [f32; 2], v: [f32; 2], z: [f32; 2]) -> Shape {
        Shape::room(self.point(u[0], v[0], z[0]), self.point(u[1], v[1], z[1]))
    }

    /// The same rectangle in the landscape's x and y, lowest corner first
    fn rect(&self, u: [f32; 2], v: [f32; 2]) -> ([f32; 2], [f32; 2]) {
        let (a, b) = (self.point(u[0], v[0], 0.0), self.point(u[1], v[1], 0.0));
        ([a[0].min(b[0]), a[1].min(b[1])], [a[0].max(b[0]), a[1].max(b[1])])
    }

    /// Which way something facing `angle` radians round from u faces in the world
    fn heading(&self, angle: f32) -> f32 {
        self.f[1].atan2(self.f[0]) + angle
    }

    /// Which way going `d` along u and v goes in the world
    fn dir(&self, d: [f32; 2]) -> [f32; 2] {
        [self.f[0] * d[0] - self.f[1] * d[1], self.f[1] * d[0] + self.f[0] * d[1]]
    }
}

/// A brick wall from (u0, v0) to (u1, v1) of a building at `at`, in pieces broken off at heights
/// between `low` and `high` above `base`, of `stone`: material and accent. Returns its highest top.
#[allow(clippy::too_many_arguments)]
fn broken_wall(s: &mut Vec<Shape>, at: &Frame, rng: &mut Rng, u: [f32; 2], v: [f32; 2], base: f32, low: f32, high: f32, stone: (u8, u8)) -> f32 {
    let along_u = (u[1] - u[0]).abs() >= (v[1] - v[0]).abs();
    let (start, end) = if along_u { (u[0], u[1]) } else { (v[0], v[1]) };
    let mut a = start;
    let mut top = base;
    while a < end - 1e-3 {
        let piece = rng.range(0.9, 1.8).min(end - a);
        let height = rng.range(low, high);
        let (us, vs) = if along_u { ([a, a + piece], v) } else { (u, [a, a + piece]) };
        s.push(at.block(us, vs, [base - 0.5, base + height], stone.0, stone.1).patterned(Pattern::Bricks));
        top = top.max(base + height);
        a += piece;
    }
    top
}

/// A candle standing at `at`, whose flame lights up the room
fn candle(features: &mut Features, at: Vec3, rng: &mut Rng) {
    features.props.push(Prop { kind: PropKind::Candle, position: at, heading: rng.next() * TAU, luck: 0.0 });
    features.lights.push(Light { position: [at[0], at[1], at[2] + 0.3], color: [1.0, 0.6, 0.25], reach: 7.0 });
}

fn skull(features: &mut Features, at: Vec3, rng: &mut Rng) {
    features.props.push(Prop { kind: PropKind::Skull, position: at, heading: rng.next() * TAU, luck: 0.0 });
}

/// A chest at `at` with its lock facing `heading`, holding a weapon. The deeper in the wilds, the
/// better what's in it is likely to be: `luck` as for Library::odds().
fn chest(features: &mut Features, at: Vec3, heading: f32, luck: f32) {
    features.props.push(Prop { kind: PropKind::Chest, position: at, heading, luck });
}

/// Ravines start at most one in each RAVINE_CELL square, and reach at most RAVINE_REACH from there
const RAVINE_CELL: f32 = 100.0;
const RAVINE_REACH: f32 = 100.0;

/// Dungeons: at most one in each DUNGEON_CELL square, nothing of it further than DUNGEON_REACH
/// along either axis from its doorway, all in meters
const DUNGEON_CELL: f32 = 300.0;
const DUNGEON_REACH: f32 = 56.0;

/// Caves: at most one starting in each CAVE_CELL square, wandering no further than CAVE_REACH
/// from where it starts, in meters
const CAVE_CELL: f32 = 60.0;
const CAVE_REACH: f32 = 310.0;

/// A cave: what's cut out of the ground for it and what grows in the hollow, the crystals
/// glowing in it, its caverns, and how far it reaches, x0, y0, x1, y1, in meters. Each cavern is
/// the middle of its floor, the way in from there, and how far out there's room all round: the
/// great ones bosses make their lairs in, the one at the end last, and the rest where cave
/// spiders nest.
struct Cave {
    shapes: Vec<Shape>,
    lights: Vec<Light>,
    lairs: Vec<(Vec3, [f32; 2], f32)>,
    nests: Vec<(Vec3, [f32; 2], f32)>,
    bounds: [f32; 4],
    seed: u32,
}
/// How thick the brick round the halls and passages of dungeons is
const DUNGEON_WALL: f32 = 0.6;
/// At least how much ground there is over the brick roofs of halls
const DUNGEON_COVER: f32 = 1.5;
/// Steps in dungeons: how high each is, and how deep. Floors are all at whole steps.
const STAIR_RISE: f32 = 0.25;
const STAIR_RUN: f32 = 0.4;
/// Half how wide the passages of dungeons are, and how high
const PASSAGE_HALF: f32 = 1.3;
const PASSAGE_HEIGHT: f32 = 3.25;
/// How high the crypt over the stairs down into a dungeon is inside, above its doorway
const CRYPT_HEIGHT: f32 = 3.5;
/// How deep the steps down from the landing in front of the crypt are
const LANDING_STEP: f32 = 0.45;

/// To the nearest whole step
fn snap(z: f32) -> f32 {
    (z / STAIR_RISE).round() * STAIR_RISE
}

/// Down to a whole step
fn snap_down(z: f32) -> f32 {
    (z / STAIR_RISE).floor() * STAIR_RISE
}

/// Whether two rectangles, x0, y0, x1, y1, come within `margin` of each other
fn overlaps(a: [f32; 4], b: [f32; 4], margin: f32) -> bool {
    a[0] < b[2] + margin && b[0] < a[2] + margin && a[1] < b[3] + margin && b[1] < a[3] + margin
}

fn grown(r: [f32; 4], by: f32) -> [f32; 4] {
    [r[0] - by, r[1] - by, r[2] + by, r[3] + by]
}

/// The rectangle `half` either way of `middle`
fn around(middle: [f32; 2], half: [f32; 2]) -> [f32; 4] {
    [middle[0] - half[0], middle[1] - half[1], middle[0] + half[0], middle[1] + half[1]]
}

/// A box of brick over rectangle `r` in the frame `at`, in the ground or out of it
fn stone_block(at: &Frame, r: [f32; 4], z: [f32; 2], stone: (u8, u8)) -> Shape {
    at.block([r[0], r[2]], [r[1], r[3]], z, stone.0, stone.1).placed(Placement::Clear).patterned(Pattern::Bricks)
}

/// Empty space over rectangle `r` in the frame `at`, cut out of the ground and anything else
fn hollow(at: &Frame, r: [f32; 4], z: [f32; 2]) -> Shape {
    at.room([r[0], r[2]], [r[1], r[3]], z)
}

/// A round, carved pillar at `q` in the frame `at`, from `floor` up to `ceiling`, with a square
/// foot and head, or broken off with chunks of it lying round its foot
#[allow(clippy::too_many_arguments)]
fn pillar(s: &mut Vec<Shape>, at: &Frame, q: [f32; 2], floor: f32, ceiling: f32, r: f32, stone: (u8, u8), broken: bool, rng: &mut Rng) {
    let top = if broken { floor + rng.range(0.6, (ceiling - floor) * 0.45) } else { ceiling };
    s.push(stone_block(at, around(q, [r + 0.18; 2]), [floor - 0.1, floor + 0.3], stone));
    if broken {
        for _ in 0..3 {
            let (a, d, size) = (rng.next() * TAU, rng.range(r + 0.4, r + 1.3), rng.range(0.2, 0.4));
            let c = [q[0] + a.cos() * d, q[1] + a.sin() * d];
            s.push(stone_block(at, around(c, [size, size * rng.range(0.7, 1.3)]), [floor - 0.05, floor + size * 0.9], stone));
        }
    } else {
        s.push(stone_block(at, around(q, [r + 0.22; 2]), [ceiling - 0.4, ceiling + 0.1], stone));
    }
    s.push(
        Shape::capsule(at.point(q[0], q[1], floor), at.point(q[0], q[1], top), r, r * 0.9, stone.0, stone.1, false)
            .placed(Placement::Clear)
            .patterned(Pattern::Carved),
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HallKind {
    /// On the way down to the boss
    Main,
    /// Off to one side, with a chest in it
    Side,
    /// The deepest and biggest, where the boss waits
    Boss,
}

/// A hall of a dungeon, in the dungeon's frame and in meters: the middle of its floor, half its
/// size along u and v, the height of its floor and how high it is inside
#[derive(Clone, Copy)]
struct Hall {
    middle: [f32; 2],
    half: [f32; 2],
    floor: f32,
    height: f32,
    /// Which way from its middle its way in is, along u or v
    door: [f32; 2],
    kind: HallKind,
    /// The hall its way in comes from, None for the one at the foot of the stairs down
    parent: Option<usize>,
}

impl Hall {
    /// Its floor from wall to wall, u0, v0, u1, v1
    fn rect(&self) -> [f32; 4] {
        around(self.middle, self.half)
    }

    /// Half its size along `d`, one of the axes
    fn reach(&self, d: [f32; 2]) -> f32 {
        d[0].abs() * self.half[0] + d[1].abs() * self.half[1]
    }
}

/// A straight passage of a dungeon, in its frame and in meters, `PASSAGE_HALF` either side of the
/// line from `start` going `dir` (along u or v) for `length`. Its floor goes from `from` at the
/// start down to `to` at the end, by stairs halfway along.
#[derive(Clone, Copy)]
struct Passage {
    start: [f32; 2],
    dir: [f32; 2],
    length: f32,
    from: f32,
    to: f32,
    /// How far along from the start it's under the crypt over the stairs down, 0 for none
    crypt: f32,
}

impl Passage {
    /// The point `s` along it and `side` across, in its dungeon's frame
    fn at(&self, s: f32, side: f32) -> [f32; 2] {
        let across = [-self.dir[1], self.dir[0]];
        [self.start[0] + self.dir[0] * s + across[0] * side, self.start[1] + self.dir[1] * s + across[1] * side]
    }

    /// The part of it from `s[0]` to `s[1]` along, `half` either side of its line, u0, v0, u1, v1
    fn rect(&self, s: [f32; 2], half: f32) -> [f32; 4] {
        let (a, b) = (self.at(s[0], -half), self.at(s[1], half));
        [a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1])]
    }

    /// How many steps down it has, and how far along the first of them is
    fn stairs(&self) -> (usize, f32) {
        let drops = ((self.from - self.to) / STAIR_RISE).round().max(0.0) as usize;
        (drops, (self.length - drops.saturating_sub(1) as f32 * STAIR_RUN) * 0.5)
    }

    /// How far along its `k`th step down is, from 1
    fn edge(&self, k: usize) -> f32 {
        self.stairs().1 + (k as f32 - 1.0) * STAIR_RUN
    }

    /// The height of its floor `s` along
    fn floor_at(&self, s: f32) -> f32 {
        let (drops, first) = self.stairs();
        if drops == 0 || s < first {
            self.from
        } else if s >= self.edge(drops) {
            self.to
        } else {
            self.from - (((s - first) / STAIR_RUN).floor() + 1.0) * STAIR_RISE
        }
    }

    /// Moves `s` to the middle of the step it's on, if it's on the stairs, so things stand flat
    fn on_step(&self, s: f32) -> f32 {
        let (drops, first) = self.stairs();
        if drops > 1 && s >= first && s < self.edge(drops) {
            first + ((s - first) / STAIR_RUN).floor() * STAIR_RUN + STAIR_RUN * 0.5
        } else {
            s
        }
    }

    /// The parts it's built in, each under a ceiling of its own: where along it each starts and
    /// ends, and its lowest and highest floor. Flat at either end, and flights of four steps.
    fn parts(&self) -> Vec<([f32; 2], f32, f32)> {
        let (drops, _) = self.stairs();
        if drops == 0 {
            return vec![([0.0, self.length], self.from, self.from)];
        }
        let floor = |k: usize| self.from - k as f32 * STAIR_RISE;
        let mut parts = vec![([0.0, self.edge(1)], self.from, self.from)];
        let mut k = 1;
        while k < drops {
            let last = (k + 3).min(drops - 1);
            parts.push(([self.edge(k), self.edge(last + 1)], floor(last), floor(k)));
            k = last + 1;
        }
        parts.push(([self.edge(drops), self.length], self.to, self.to));
        parts
    }
}

/// Halls under the ground, joined by passages, and a crypt over the stairs down into them. Laid
/// out in `at`, facing the way the stairs go down, with the doorway of the crypt at its middle.
struct Dungeon {
    at: Frame,
    /// The floor of the crypt's doorway, and the top of the stairs
    top: f32,
    /// How many steps go down from the landing in front of it to the ground round it
    steps: usize,
    /// What the walls are made of, and the floors
    stone: (u8, u8),
    paving: (u8, u8),
    snowy: bool,
    vines: bool,
    /// Its halls, and the passages into them: `passages[i]` leads into `halls[i]`, the stairs
    /// down from the crypt into the first
    halls: Vec<Hall>,
    passages: Vec<Passage>,
    seed: u32,
}

impl Dungeon {
    /// The landing in front of the crypt and the steps down from it on three sides, u0, v0, u1, v1
    /// in its frame
    fn landing(&self, step: usize) -> [f32; 4] {
        let (front, side) = (DUNGEON_WALL + 2.6 + LANDING_STEP * step as f32, PASSAGE_HALF + DUNGEON_WALL + 1.2 + LANDING_STEP * step as f32);
        [-front, -side, -DUNGEON_WALL, side]
    }

    /// The crypt over the stairs down, with the landing and steps in front of it and the rubble
    /// round it, u0, v0, u1, v1 in its frame
    fn entrance_rect(&self) -> [f32; 4] {
        let r = self.landing(self.steps);
        [r[0] - 3.5, r[1] - 2.5, self.passages[0].crypt + 3.5, r[3] + 2.5]
    }

    /// Where it all is in the landscape, x0, y0, x1, y1 in meters: the entrance, and the passages
    /// and halls with the brick round them
    fn world_rects(&self) -> Vec<[f32; 4]> {
        let mut rects = vec![self.entrance_rect()];
        rects.extend(self.passages.iter().map(|p| p.rect([0.0, p.length], PASSAGE_HALF + DUNGEON_WALL)));
        rects.extend(self.halls.iter().map(|h| grown(h.rect(), DUNGEON_WALL)));
        rects
            .into_iter()
            .map(|r| {
                let (lo, hi) = self.at.rect([r[0], r[2]], [r[1], r[3]]);
                [lo[0], lo[1], hi[0], hi[1]]
            })
            .collect()
    }

    /// On the landing in front of the crypt's doorway, in meters
    fn entrance(&self) -> [f32; 3] {
        self.at.point(-DUNGEON_WALL - 1.5, 0.0, self.top)
    }
}

/// How the leaves on the twigs of a tree look.
#[derive(Clone, Copy)]
struct Foliage {
    material: u8,
    accent: u8,
    // In meters
    leaf_length: f32,
    leaves_per_twig: usize,
    // Small round clumps of leaves behind the individual ones, so crowns look full from afar
    clumps_per_twig: usize,
}

/// A deeper shade of leaves, for the shadowy middle of a crown
fn darker(leaves: u8) -> u8 {
    match leaves {
        LEAVES | LEAVES_DARK => LEAVES_DARK,
        LEAVES_YELLOW => LEAVES_ORANGE,
        LEAVES_ORANGE | LEAVES_RED => LEAVES_RED,
        BLOSSOM => FLOWER_PINK,
        other => other,
    }
}

const BARE: Foliage = Foliage {
    material: AIR,
    accent: AIR,
    leaf_length: 0.0,
    leaves_per_twig: 0,
    clumps_per_twig: 0,
};

#[derive(Default, Clone, Copy)]
struct Column {
    // All heights in voxels
    height: i32,
    top: u8,
    below: u8,
    // Water fills the column from just above the ground up to here
    water_top: i32,
    // Rock comes in layers, see Terrain::rock(): how far they're shifted up or down here, in
    // voxels, and how likely moss is to grow on it
    strata: i32,
    moss: f32,
    // Sandstone rather than rock, in the deserts
    desert: bool,
    // What the tuft or stem growing here is made of
    blade_body: u8,
    // Where the column is in the whole landscape
    x: i64,
    y: i64,
    blade: i32,
    blade_top: u8,
    /// Nothing's under this height in voxels: the open air under a floating island in the sky
    /// isles, or all of it where there's none. 0 elsewhere.
    under: i32,
    /// Built of courses of stone all the way down, as the Drowned Ring is, see Terrain::masonry()
    built: bool,
}

/// The ground under a rectangle of voxel columns, at some resolution. Columns outside it are
/// worked out one at a time just the same, see Heights::at(), so whatever's placed near the edge of
/// a piece of the landscape stands on the same ground whichever piece it's placed for.
struct Heights {
    voxels_per_meter: f32,
    seed: u32,
    /// Voxel coordinates of the first column
    origin: [i64; 2],
    size: [usize; 2],
    /// Height of the ground in meters, carved by the rivers
    ground: Vec<f32>,
    /// Height of river water in meters, NEG_INFINITY where there's none
    river: Vec<f32>,
    /// For the columns outside: the land's shape, and the rivers carved into it with their runs,
    /// see carve()
    settings: TerrainSettings,
    paths: Vec<Arc<Vec<RiverPoint>>>,
    runs: Vec<RiverRun>,
}

/// A few points next to each other along a river in the same pool, so the same wobble, see
/// Heights::carve(), and the voxel columns they reach, from `lo` to `hi` in the whole landscape
struct RiverRun {
    river: usize,
    points: std::ops::Range<usize>,
    seed: u32,
    /// How far from them the river and its banks reach, in meters
    reach: f32,
    lo: [i64; 2],
    hi: [i64; 2],
}

/// No river point near a column yet, see reach_column()
const NO_RIVER: (u32, u32) = (u32::MAX, 0);

/// Takes `run` into account for the column at `xm`, `ym` in meters: if one of its points, wobbled,
/// is nearer than `distance`, that's the nearest so far. `plain` is room for the distances.
fn reach_column(paths: &[Arc<Vec<RiverPoint>>], run: &RiverRun, xm: f32, ym: f32, plain: &mut [f32; RIVER_RUN], distance: &mut f32, nearest: &mut (u32, u32)) {
    let points = &paths[run.river][run.points.clone()];
    let mut closest = f32::INFINITY;
    for (d, p) in plain.iter_mut().zip(points) {
        *d = ((xm - p.x) * (xm - p.x) + (ym - p.y) * (ym - p.y)).sqrt();
        closest = closest.min(*d);
    }
    // However the distance wobbles, too far to reach, or to be nearer than a point already found
    if closest - RIVER_WOBBLE > run.reach || closest - RIVER_WOBBLE >= *distance {
        return;
    }
    let wobble = RIVER_WOBBLE * fbm(xm / 1.5, ym / 1.5, 2, run.seed);
    for (k, (&d, p)) in plain.iter().zip(points).enumerate() {
        let d = d + wobble;
        if d <= p.half_width + RIVER_BANK && d < *distance {
            *distance = d;
            *nearest = (run.river as u32, (run.points.start + k) as u32);
        }
    }
}

/// Shapes the column at `xm`, `ym` in meters, whose ground is `g` and river `r`, by its nearest river
/// point `p`, `d` from it, see Heights::carve()
fn shape_column(p: &RiverPoint, d: f32, xm: f32, ym: f32, seed: u32, g: &mut f32, r: &mut f32) {
    if d < p.half_width {
        // Shallow at the lip of a waterfall, so the pool behind it holds its water
        let depth = if p.lip { 0.1 } else { 0.6 + p.half_width * 0.25 };
        let t = (d / p.half_width).max(0.0);
        *r = p.surface;
        *g = p.surface - depth * (1.0 - t * t).max(if p.lip { 1.0 } else { 0.0 });
    } else {
        // Rocky banks: their steepness wanders, with knobs and hollows in them, but they always
        // stand clear of the water so it doesn't spill out
        let out = d - p.half_width;
        let wander = fbm(xm / 4.0, ym / 4.0, 3, seed.wrapping_add(21));
        let knobs = fbm(xm / 1.2, ym / 1.2, 2, seed.wrapping_add(22));
        let rough = (knobs * 0.5 + wander * 0.6) * out.min(1.0);
        *g = g.min((p.surface + out * 0.9 * (1.0 + 0.5 * wander) + rough).max(p.surface + 0.15));
    }
}

/// How far rivers' banks slope back up to the land around, in meters
const RIVER_BANK: f32 = 2.5;
/// How much distances from a river wobble, in meters, so the edges of its pools are ragged
const RIVER_WOBBLE: f32 = 0.7;
/// How many points along a river are measured from together, in Heights::carve()
const RIVER_RUN: usize = 8;

impl Heights {
    fn new(settings: &TerrainSettings, origin: [i64; 2], size: [usize; 2], voxels_per_meter: u32) -> Self {
        let vpm = voxels_per_meter as f32;
        let mut ground = vec![0.0; size[0] * size[1]];

        // Rows split across threads
        let threads = threads();
        let rows_per_thread = size[1].div_ceil(threads).max(1);
        thread::scope(|s| {
            for (chunk_index, chunk) in ground.chunks_mut(rows_per_thread * size[0]).enumerate() {
                s.spawn(move || {
                    for (i, h) in chunk.iter_mut().enumerate() {
                        if i % 1024 == 0 {
                            give_way();
                        }
                        let j = chunk_index * rows_per_thread * size[0] + i;
                        let x = (origin[0] + (j % size[0]) as i64) as f32 / vpm;
                        let y = (origin[1] + (j / size[0]) as i64) as f32 / vpm;
                        *h = height_at(settings, x, y);
                    }
                });
            }
        });

        Heights {
            voxels_per_meter: vpm,
            seed: settings.seed,
            origin,
            size,
            ground,
            river: vec![f32::NEG_INFINITY; size[0] * size[1]],
            settings: *settings,
            paths: Vec::new(),
            runs: Vec::new(),
        }
    }

    /// The ground's height and the river's, NEG_INFINITY for none, in meters, at column `gx`, `gy`
    /// of the whole landscape: from this rectangle if it's in it, otherwise worked out just the same
    /// as it would be for a rectangle it's in
    fn at(&self, gx: i64, gy: i64) -> (f32, f32) {
        let (x, y) = (gx - self.origin[0], gy - self.origin[1]);
        if x >= 0 && y >= 0 && x < self.size[0] as i64 && y < self.size[1] as i64 {
            let i = x as usize + y as usize * self.size[0];
            return (self.ground[i], self.river[i]);
        }
        let vpm = self.voxels_per_meter;
        let (xm, ym) = (gx as f32 / vpm, gy as f32 / vpm);
        let mut g = height_at(&self.settings, xm, ym);
        let mut r = f32::NEG_INFINITY;
        let (mut distance, mut nearest, mut plain) = (f32::INFINITY, NO_RIVER, [0.0f32; RIVER_RUN]);
        for run in self.runs.iter().filter(|run| gx >= run.lo[0] && gx < run.hi[0] && gy >= run.lo[1] && gy < run.hi[1]) {
            reach_column(&self.paths, run, xm, ym, &mut plain, &mut distance, &mut nearest);
        }
        if nearest != NO_RIVER {
            shape_column(&self.paths[nearest.0 as usize][nearest.1 as usize], distance, xm, ym, self.seed, &mut g, &mut r);
        }
        (g, r)
    }

    /// The same at a point in meters
    fn at_m(&self, x: f32, y: f32) -> (f32, f32) {
        let vpm = self.voxels_per_meter;
        self.at((x * vpm).floor() as i64, (y * vpm).floor() as i64)
    }

    /// Ground height in meters at a column, counted from this rectangle's corner
    fn ground_m(&self, x: i64, y: i64) -> f32 {
        self.at(self.origin[0] + x, self.origin[1] + y).0
    }

    /// River water's height in meters at a column, counted from this rectangle's corner,
    /// NEG_INFINITY where there's none
    fn river_m(&self, x: i64, y: i64) -> f32 {
        self.at(self.origin[0] + x, self.origin[1] + y).1
    }

    /// Cuts the rivers' channels and banks into the ground.
    ///
    /// Each column is shaped by its nearest river point alone, so where a river drops over a
    /// waterfall there's a sheer edge, with the banks of the pool above still standing right up to
    /// it. Distances wobble differently for each pool, so that edge is ragged rather than a
    /// straight line. Beds are laid in even where the ground was lower, so the lips of waterfalls
    /// hold back the pools behind them.
    fn carve(&mut self, paths: &[Arc<Vec<RiverPoint>>]) {
        self.paths = paths.to_vec();
        if paths.is_empty() {
            return;
        }
        let vpm = self.voxels_per_meter;
        let (origin, seed) = (self.origin, self.seed);
        let [sx, sy] = self.size;
        let pool_seed = |p: &RiverPoint| seed ^ hash((p.surface * 4.0) as i32 as u32);

        // A few points next to each other in the same pool, so the same wobble, and the columns
        // they reach in the whole landscape, kept for those outside, see at()
        let mut runs = Vec::new();
        for (river, path) in paths.iter().enumerate() {
            let mut start = 0;
            while start < path.len() {
                let seed = pool_seed(&path[start]);
                let mut end = start + 1;
                while end < path.len() && end - start < RIVER_RUN && pool_seed(&path[end]) == seed {
                    end += 1;
                }
                let run = &path[start..end];
                let widest = run.iter().map(|p| p.half_width).fold(0.0, f32::max);
                let reach = widest + RIVER_BANK + RIVER_WOBBLE;
                let lo = [0, 1].map(|a| run.iter().map(|p| if a == 0 { p.x } else { p.y }).fold(f32::MAX, f32::min) - reach);
                let hi = [0, 1].map(|a| run.iter().map(|p| if a == 0 { p.x } else { p.y }).fold(f32::MIN, f32::max) + reach);
                let lo = [0, 1].map(|a| (lo[a] * vpm).floor() as i64);
                let hi = [0, 1].map(|a| (hi[a] * vpm).ceil() as i64 + 1);
                runs.push(RiverRun { river, points: start..end, seed, reach: widest + RIVER_BANK, lo, hi });
                start = end;
            }
        }
        // Those reaching into this rectangle, in its columns
        let local: Vec<(&RiverRun, [usize; 2], [usize; 2])> = runs
            .iter()
            .filter_map(|run| {
                let lo = [0, 1].map(|a| (run.lo[a] - origin[a]).max(0) as usize);
                let hi = [0, 1].map(|a| (run.hi[a] - origin[a]).clamp(0, self.size[a] as i64) as usize);
                (lo[0] < hi[0] && lo[1] < hi[1]).then_some((run, lo, hi))
            })
            .collect();

        // First the nearest point to each column, in rows split across threads: how far it is,
        // and which it is
        let mut distance = vec![f32::INFINITY; sx * sy];
        let mut nearest = vec![NO_RIVER; sx * sy];
        let rows = sy.div_ceil(threads()).max(1);
        let paths = &self.paths;
        thread::scope(|s| {
            for (band, (distance, nearest)) in distance.chunks_mut(rows * sx).zip(nearest.chunks_mut(rows * sx)).enumerate() {
                let local = &local;
                s.spawn(move || {
                    give_way();
                    let (first, last) = (band * rows, band * rows + distance.len() / sx);
                    let mut plain = [0.0f32; RIVER_RUN];
                    for (run, lo, hi) in local.iter().filter(|(_, lo, hi)| lo[1] < last && hi[1] > first) {
                        for cy in lo[1].max(first)..hi[1].min(last) {
                            let ym = (origin[1] + cy as i64) as f32 / vpm;
                            for cx in lo[0]..hi[0] {
                                let xm = (origin[0] + cx as i64) as f32 / vpm;
                                let i = cx + (cy - first) * sx;
                                reach_column(paths, run, xm, ym, &mut plain, &mut distance[i], &mut nearest[i]);
                            }
                        }
                    }
                });
            }
        });

        // Then each column shaped by its nearest point
        thread::scope(|s| {
            let chunks = self.ground.chunks_mut(rows * sx).zip(self.river.chunks_mut(rows * sx));
            for (band, (ground, river)) in chunks.enumerate() {
                let (distance, nearest) = (&distance[band * rows * sx..], &nearest[band * rows * sx..]);
                s.spawn(move || {
                    give_way();
                    for (i, (g, r)) in ground.iter_mut().zip(river.iter_mut()).enumerate() {
                        if nearest[i] == NO_RIVER {
                            continue;
                        }
                        let j = band * rows * sx + i;
                        let xm = (origin[0] + (j % sx) as i64) as f32 / vpm;
                        let ym = (origin[1] + (j / sx) as i64) as f32 / vpm;
                        shape_column(&paths[nearest[i].0 as usize][nearest[i].1 as usize], distance[i], xm, ym, seed, g, r);
                    }
                });
            }
        });
        self.runs = runs;
    }
}

/// What kind of land a landscape is, from its seed: the normal world's for every seed Free Roam
/// can have, and the realms' own for theirs past those, see src\realms.rs
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Land {
    Normal,
    /// Islands floating high over the sea, see sky_island()
    Sky,
    Dream,
    Ice,
    /// Open sea with islands strung across it on sandbars and reefs, see water_height()
    Water,
    /// Volcanic highlands, calderas, lava and the bones of dragons, see dragon_height()
    Dragon,
}

pub fn land_of(seed: u32) -> Land {
    match seed >> 24 {
        0xF1 => Land::Sky,
        0xF2 => Land::Dream,
        0xF3 => Land::Ice,
        0xF4 => Land::Water,
        0xF5 => Land::Dragon,
        _ => Land::Normal,
    }
}

/// How far apart the sky isles are, in meters: each SKY_CELL square has one, near its middle
const SKY_CELL: f32 = 46.0;

/// The island floating over a spot in the sky isles, in meters: how high the top of its ground is,
/// and how high its underside, or None in the open air between them. Each square SKY_CELL across
/// has an island near its middle, some of them great isles twice the size of the rest, their tops
/// at their own heights, their shores ragged, their undersides tapering down to a rocky point, so
/// none is out of reach of the next: a grappling hook's throw or a glide away.
pub fn sky_island(seed: u32, x: f32, y: f32) -> Option<(f32, f32)> {
    let (cx, cy) = ((x / SKY_CELL).floor() as i32, (y / SKY_CELL).floor() as i32);
    // The same ragged shore and gentle hills for all of them, worked out only where it's needed
    let mut shore = None;
    let mut best: Option<(f32, f32)> = None;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (middle, radius, height) = sky_isle(seed, cx + dx, cy + dy);
            let d = (x - middle[0]).hypot(y - middle[1]);
            if d > radius * 1.3 {
                continue;
            }
            let wobble = *shore.get_or_insert_with(|| fbm(x / 9.0, y / 9.0, 3, seed.wrapping_add(41)));
            let edge = radius * (1.0 + 0.28 * wobble);
            if d >= edge {
                continue;
            }
            let t = d / edge;
            let hills = fbm(x / 14.0, y / 14.0, 3, seed.wrapping_add(42));
            let top = height + (1.0 - t * t) * 2.5 + hills * 1.8 - smoothstep(0.82, 1.0, t) * 1.2;
            // Deeper the bigger it is, rocky knobs hanging from it, all of it coming to a point
            let r = hash3((cx + dx) as u32, (cy + dy) as u32, seed ^ 0x5c1e);
            let depth = (0.5 + 0.25 * unit(r >> 12)) * radius.min(20.0);
            let knobs = ridged(x / 3.5, y / 3.5, 2, seed.wrapping_add(43)) * 2.5 * (1.0 - t);
            let under = top - 1.8 - (depth + knobs) * (1.0 - t.powf(1.5));
            if best.is_none_or(|(b, _)| top > b) {
                best = Some((top, under));
            }
        }
    }
    best
}

/// The island of the SKY_CELL square `i`, `j` in the sky isles: its middle and how far out its
/// shore is, give or take its raggedness, and how high its edge is, in meters. A few are great
/// isles twice the size of the rest.
pub fn sky_isle(seed: u32, i: i32, j: i32) -> ([f32; 2], f32, f32) {
    let r = hash3(i as u32, j as u32, seed ^ 0x5c1e);
    let great = unit(hash(r)) < 0.22;
    let middle = [(i as f32 + 0.5 + 0.3 * (unit(r) - 0.5)) * SKY_CELL, (j as f32 + 0.5 + 0.3 * (unit(r >> 16) - 0.5)) * SKY_CELL];
    let radius = if great { 22.0 + 4.0 * unit(r >> 8) } else { 12.0 + 6.0 * unit(r >> 4) };
    (middle, radius, 26.0 + 14.0 * unit(hash(r ^ 0x77)))
}

/// The line between the dreaming realm's two halves, the soft one and the one bleeding red: east
/// of x meters, wandering this far either side of it, over this many meters along it, and how far
/// either side of it the one gives way to the other. Has to match DIVIDE_* in
/// shaders\traverse.comp, which colours the halves, see App::realm_look().
pub const DREAM_DIVIDE: [f32; 4] = [240.0, 18.0, 70.0, 2.0];

/// How much a spot in the dreaming realm, in meters, is of its blood-red half, 0 to 1: the half east
/// of a wandering line, see DREAM_DIVIDE
pub fn dream_blood(x: f32, y: f32) -> f32 {
    let [at, wander, along, edge] = DREAM_DIVIDE;
    smoothstep(-edge, edge, x - (at + wander * (y / along).sin()))
}

/// The ground of the dreaming realm, in meters: soft rolling land with great round domes rising
/// smooth out of it here and there, giving way past the divide to the bleeding half, torn into
/// sharp crests with gullies between where blood pools
fn dream_height(seed: u32, x: f32, y: f32) -> f32 {
    let blood = dream_blood(x, y);
    let mut soft = 23.0 + fbm(x / 140.0, y / 140.0, 3, seed) * 6.0 + fbm(x / 35.0, y / 35.0, 2, seed.wrapping_add(1)) * 1.5;
    if blood < 1.0 {
        const DOME_CELL: f32 = 64.0;
        let (cx, cy) = ((x / DOME_CELL).floor() as i32, (y / DOME_CELL).floor() as i32);
        let mut dome = 0.0f32;
        for j in cy - 1..=cy + 1 {
            for i in cx - 1..=cx + 1 {
                let r = hash3(i as u32, j as u32, seed ^ 0xd0e5);
                if unit(r) > 0.6 {
                    continue;
                }
                let middle = [(i as f32 + 0.25 + 0.5 * unit(r >> 4)) * DOME_CELL, (j as f32 + 0.25 + 0.5 * unit(r >> 12)) * DOME_CELL];
                let radius = 10.0 + 12.0 * unit(r >> 20);
                let d = (x - middle[0]).hypot(y - middle[1]) / radius;
                if d < 1.0 {
                    dome = dome.max(radius * (0.45 + 0.35 * unit(hash(r))) * (1.0 - d * d).sqrt());
                }
            }
        }
        soft += dome;
    }
    let torn = 18.0 + fbm(x / 90.0, y / 90.0, 3, seed.wrapping_add(2)) * 4.0 + ridged(x / 28.0, y / 28.0, 4, seed.wrapping_add(3)) * 11.0;
    soft + (torn - soft) * blood
}

/// How far apart the sunken reach's islands are, in meters: each WATER_CELL square has one
const WATER_CELL: f32 = 92.0;

/// The island of the WATER_CELL square `i`, `j` in the sunken reach: its middle and how far out its
/// shore is, in meters, how high its top is over the sea, and whether it's a ring of land round a
/// lagoon
fn water_isle(seed: u32, i: i32, j: i32) -> ([f32; 2], f32, f32, bool) {
    let r = hash3(i as u32, j as u32, seed ^ 0x5ea);
    let middle = [(i as f32 + 0.5 + 0.4 * (unit(r) - 0.5)) * WATER_CELL, (j as f32 + 0.5 + 0.4 * (unit(r >> 16) - 0.5)) * WATER_CELL];
    let radius = 15.0 + 15.0 * unit(r >> 8);
    let top = 2.0 + 7.0 * unit(hash(r));
    (middle, radius, top, unit(hash(r ^ 0xa70)) < 0.2)
}

/// Whether the island of square `i`, `j` is joined to the one north of it by a sandbar, as it
/// always is to the one east of it, so every island can be walked to, wading
fn water_bar_north(seed: u32, i: i32, j: i32) -> bool {
    unit(hash3(i as u32, j as u32, seed ^ 0xba5)) < 0.55
}

/// The ground of the sunken reach, in meters: deep water, `sea` meters up being where the sea is,
/// islands rising out of it, some of them rings round a shallow lagoon, reefs round some just under
/// the surface, and sandbars joining them up, a little under it, to wade across
fn water_height(seed: u32, sea: f32, x: f32, y: f32) -> f32 {
    let mut h = sea - 7.0 + fbm(x / 60.0, y / 60.0, 3, seed.wrapping_add(5)) * 2.0;
    let (cx, cy) = ((x / WATER_CELL).floor() as i32, (y / WATER_CELL).floor() as i32);
    let wobble = fbm(x / 11.0, y / 11.0, 3, seed.wrapping_add(6));
    for j in cy - 1..=cy + 1 {
        for i in cx - 1..=cx + 1 {
            let (middle, radius, top, atoll) = water_isle(seed, i, j);
            let d = (x - middle[0]).hypot(y - middle[1]) / (radius * (1.0 + 0.22 * wobble));
            if d < 1.7 {
                // Up out of the deep to the shore, then gently up to the island's top
                let land = if d < 1.0 {
                    sea + 0.6 + top * (1.0 - d * d).powf(0.8) + fbm(x / 9.0, y / 9.0, 2, seed.wrapping_add(7)) * 0.8
                } else {
                    sea + 0.6 - (d - 1.0) / 0.7 * 7.0
                };
                let land = if atoll && d < 0.55 { land.min(sea - 1.4 - (0.55 - d) * 2.0) } else { land };
                h = h.max(land);
                // A reef ringing some, just under the surface
                if unit(hash3(i as u32, j as u32, seed ^ 0x2eef)) < 0.4 {
                    h = h.max(sea - 0.5 - ((d - 1.4).abs() / 0.08).powi(2));
                }
            }
            // Sandbars to the neighbours, a little under the surface, a few meters wide
            for (di, dj, bar) in [(1, 0, true), (0, 1, water_bar_north(seed, i, j))] {
                if !bar {
                    continue;
                }
                let (other, ..) = water_isle(seed, i + di, j + dj);
                let (ax, ay, bx, by) = (middle[0], middle[1], other[0], other[1]);
                let (vx, vy) = (bx - ax, by - ay);
                let t = (((x - ax) * vx + (y - ay) * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
                let off = (x - ax - vx * t).hypot(y - ay - vy * t) + wobble * 1.2;
                if off < 4.0 {
                    h = h.max(sea - 0.25 - (off / 2.6).powi(2) * 1.6);
                }
            }
        }
    }
    h
}

/// How far apart the Cinder Crown's calderas may be, in meters: each CALDERA_CELL square may have one
const CALDERA_CELL: f32 = 250.0;

/// The caldera in the CALDERA_CELL square `i`, `j`, if there's one: its middle and the radius of its
/// rim, in meters
fn caldera_in(seed: u32, i: i32, j: i32) -> Option<([f32; 2], f32)> {
    let r = hash3(i as u32, j as u32, seed ^ 0xca1d);
    (unit(r) <= 0.6).then(|| ([(i as f32 + 0.3 + 0.4 * unit(r >> 4)) * CALDERA_CELL, (j as f32 + 0.3 + 0.4 * unit(r >> 12)) * CALDERA_CELL], 30.0 + 26.0 * unit(r >> 20)))
}

/// The ground of the Cinder Crown, in meters, `sea` being where lava lies: highlands of ash, peaks
/// rising sharp out of them, and calderas, a ring of rim round a bowl sinking down to a lake of
/// lava, which its rivers run down into
fn dragon_height(seed: u32, sea: f32, x: f32, y: f32) -> f32 {
    let base = 26.0 + fbm(x / 150.0, y / 150.0, 3, seed) * 6.0 + fbm(x / 38.0, y / 38.0, 2, seed.wrapping_add(1)) * 1.5;
    let mask = smoothstep(-0.15, 0.35, fbm(x / 210.0, y / 210.0, 2, seed.wrapping_add(2)));
    let mut h = base + ridged(x / 85.0, y / 85.0, 5, seed.wrapping_add(3)) * mask * 28.0;
    let (cx, cy) = ((x / CALDERA_CELL).floor() as i32, (y / CALDERA_CELL).floor() as i32);
    for j in cy - 1..=cy + 1 {
        for i in cx - 1..=cx + 1 {
            let Some((middle, radius)) = caldera_in(seed, i, j) else {
                continue;
            };
            let d = (x - middle[0]).hypot(y - middle[1]) / radius;
            if d > 2.0 {
                continue;
            }
            h += 9.0 * (-((d - 1.0) / 0.22).powi(2)).exp();
            if d < 1.0 {
                let floor = sea - 2.0 + d * d * 6.0;
                h += (floor - h) * smoothstep(1.0, 0.72, d);
            }
        }
    }
    h
}

/// The Drowned Ring: a colosseum of an old people sunk into the sea, where the Coil Unending is
/// fought, see serpents.rs. A deep round basin of open water, drowned statues of serpents coiled on
/// its mosaic floor, a ring of island stones and stepping stones round its middle to stand on, and
/// round it a wet walk at the water's edge and four tiers of stands, here and there fallen in or
/// flooded, ringed by pillars, with a passage through them under a broken arch to a causeway out to
/// the land. Built into the ground and of world voxels, see arena_ground() and arena_objects(), so
/// it costs nothing more to draw than the land.
///
/// There's one in the Sunken Reach, out in the deep water where its finale is, see realms.rs, and
/// one in the normal land, on a coast far out from where Free Roam starts. Where each is depends on
/// the landscape alone, so it's the same however the world's built, a strip at a time or in one go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arena {
    /// The middle of its basin, in meters
    pub middle: [f32; 2],
    /// Which way its gate faces, out along its causeway to the land, in radians
    pub facing: f32,
    /// How long the causeway is, from the foot of the stands to the land, in meters
    pub causeway: f32,
}

/// How far the Drowned Ring's open water reaches from its middle, in meters, and its stands, four
/// tiers of them each TIER deep; how far its outer wall comes down to the sea floor, and how far
/// beyond that it fades back into the land round it
pub const ARENA_BASIN: f32 = 24.0;
const ARENA_TIER: f32 = 2.75;
pub const ARENA_STANDS: f32 = ARENA_BASIN + 4.0 * ARENA_TIER;
const ARENA_OUTER: f32 = ARENA_STANDS + 4.0;
const ARENA_REACH: f32 = ARENA_STANDS + 15.0;
/// How wide the passage through the stands and the causeway are, from the middle of them to their
/// edge, in meters
const ARENA_WAY: f32 = 3.0;
/// The tops of its tiers, from the walk at the water's edge up, over the sea, in meters: each a
/// jump up from the one below, see player.rs
const ARENA_TIERS: [f32; 4] = [0.35, 1.2, 2.05, 2.9];
/// How deep its basin is in the middle and at its wall's foot, under the sea, in meters
const ARENA_DEEP: f32 = 11.0;
/// Its island stones, how far out from its middle and how wide, and the stepping stones between
/// them and the walk: five of each
const ARENA_ISLES: (f32, f32) = (12.5, 2.6);
const ARENA_STEPS: (f32, f32) = (19.0, 1.5);
/// About where the Sunken Reach's is, in meters: out in the deep water where its finale is, from the
/// gate it's come to by, see realms::REALMS
const REACH_DEEP: [f32; 2] = [229.0, -190.0];
/// Where Free Roam starts, in meters, the same spot on every seed, see session::default_start(), and
/// about how far out from it the normal land's is
pub const FREE_ROAM_START: [f32; 2] = [200.0, 180.0];
const FREE_ROAM_ARENA: f32 = 1850.0;

/// Whether the Drowned Rings are left out of the land, see leave_out_arenas()
static ARENAS_LEFT_OUT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// For measuring what the Drowned Ring costs to draw, `--set no_arena=1`: the land built as it'd be
/// without it, though it's still found where it would be, see arena_site_of()
pub fn leave_out_arenas() {
    ARENAS_LEFT_OUT.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// The Drowned Ring in `settings`' landscape, if it has one and it's not left out, see Arena
pub fn arena_of(settings: &TerrainSettings) -> Option<Arena> {
    if ARENAS_LEFT_OUT.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    arena_site_of(settings)
}

/// Where the Drowned Ring is in `settings`' landscape, if it has one, see Arena: worked out once for
/// each landscape, and kept for each thread too, as the ground everywhere is worked out with it
pub fn arena_site_of(settings: &TerrainSettings) -> Option<Arena> {
    let land = land_of(settings.seed);
    if !matches!(land, Land::Normal | Land::Water) {
        return None;
    }
    type Found = ((u32, u32), Option<Arena>);
    thread_local! {
        static LAST: std::cell::Cell<Option<Found>> = const { std::cell::Cell::new(None) };
    }
    let key = (settings.seed, settings.height_m);
    if let Some((k, arena)) = LAST.with(|l| l.get())
        && k == key
    {
        return arena;
    }
    static FOUND: Mutex<Vec<Found>> = Mutex::new(Vec::new());
    let arena = {
        let mut found = FOUND.lock().unwrap_or_else(|e| e.into_inner());
        match found.iter().find(|(k, _)| *k == key) {
            Some(&(_, arena)) => arena,
            None => {
                let arena = match land {
                    Land::Water => arena_site(settings, REACH_DEEP, 240.0),
                    _ => {
                        let way = unit(hash(settings.seed ^ 0x51e7_c011)) * TAU;
                        (0..8).find_map(|k| {
                            let a = way + k as f32 * 0.8;
                            arena_site(settings, [FREE_ROAM_START[0] + a.cos() * FREE_ROAM_ARENA, FREE_ROAM_START[1] + a.sin() * FREE_ROAM_ARENA], 300.0)
                        })
                    }
                };
                found.push((key, arena));
                arena
            }
        }
    };
    LAST.with(|l| l.set(Some((key, arena))));
    arena
}

/// The nearest spot to `near` within `within` meters, looking in rings eight meters apart, for the
/// Drowned Ring to sink into: open sea all over where it stands, deep enough for its basin, with
/// land not too far off one way for its causeway to reach, which way being the nearest. Looked for
/// on the land as it is without it.
fn arena_site(settings: &TerrainSettings, near: [f32; 2], within: f32) -> Option<Arena> {
    let sea = sea_level_m(settings);
    let ground = |p: [f32; 2]| base_height(settings, p[0], p[1]);
    let out = |at: [f32; 2], a: f32, d: f32| [at[0] + a.cos() * d, at[1] + a.sin() * d];
    for ring in 0..=(within / 8.0) as usize {
        let count = (ring * 6).max(1);
        for i in 0..count {
            let at = out(near, i as f32 / count as f32 * TAU, ring as f32 * 8.0);
            let open = ground(at) < sea - 3.0 && (0..12).all(|k| [16.0, ARENA_STANDS - 3.0].iter().all(|&d| ground(out(at, k as f32 * TAU / 12.0, d)) < sea - 1.0));
            if !open {
                continue;
            }
            // The nearest land, of sixteen ways out, past the stands but not too far
            let land = (0..16)
                .filter_map(|k| {
                    let a = k as f32 * TAU / 16.0;
                    (0..24).map(|s| ARENA_STANDS + 4.0 + s as f32 * 5.0).find(|&d| ground(out(at, a, d)) > sea + 1.0).map(|d| (d, a))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((d, a)) = land {
                return Some(Arena { middle: at, facing: a, causeway: d - ARENA_STANDS + 2.0 });
            }
        }
    }
    None
}

impl Arena {
    /// Where `x`, `y` is from its middle, in meters: how far, along the way its gate faces, and
    /// across that
    fn local(&self, x: f32, y: f32) -> (f32, f32, f32) {
        let (dx, dy) = (x - self.middle[0], y - self.middle[1]);
        let (fx, fy) = (self.facing.cos(), self.facing.sin());
        (dx.hypot(dy), dx * fx + dy * fy, (dy * fx - dx * fy).abs())
    }

    /// Whether `x`, `y` is on its passage through the stands or its causeway out to the land
    fn on_way(&self, x: f32, y: f32) -> bool {
        let (_, along, across) = self.local(x, y);
        along > ARENA_BASIN - 0.5 && along < ARENA_STANDS + self.causeway + 4.0 && across < ARENA_WAY
    }

    /// Whether nothing's to be put at `x`, `y` reaching `reach` meters from there, as it'd be on it
    fn keeps_clear(&self, x: f32, y: f32, reach: f32) -> bool {
        let (d, along, across) = self.local(x, y);
        d < ARENA_OUTER + 2.0 + reach || along > 0.0 && along < ARENA_STANDS + self.causeway + 4.0 && across < ARENA_WAY + 2.0 + reach
    }

    /// Which of the sixteen sectors of its stands `x`, `y` is in, and whether that one's fallen in,
    /// and whether its walk at the water's edge is flooded: never by its gate
    fn sector(&self, seed: u32, x: f32, y: f32) -> (bool, bool) {
        let (dx, dy) = (x - self.middle[0], y - self.middle[1]);
        let of = |a: f32| (a / TAU * 16.0).floor().rem_euclid(16.0) as i32;
        let (sector, gate) = (of(dy.atan2(dx)), of(self.facing));
        let r = hash3(sector as u32, 0xa2e7, seed);
        let by_gate = (sector - gate).rem_euclid(16) <= 1 || (gate - sector).rem_euclid(16) <= 1;
        (!by_gate && unit(r) < 0.28, unit(r >> 16) < 0.35)
    }

    /// Its island stones and stepping stones: where each is, in meters, how wide, and how high its top
    /// is over the sea
    fn stones(&self) -> impl Iterator<Item = ([f32; 2], f32, f32)> + '_ {
        (0..10).map(|k| {
            let (out, wide, top) = if k < 5 { (ARENA_ISLES.0, ARENA_ISLES.1, 0.3) } else { (ARENA_STEPS.0, ARENA_STEPS.1, 0.15) };
            let a = self.facing + (k as f32 + if k < 5 { 0.5 } else { 0.0 }) * TAU / 5.0;
            ([self.middle[0] + a.cos() * out, self.middle[1] + a.sin() * out], wide, top)
        })
    }

    /// What the ground's made of at `x`, `y`, if it's its own: the mosaic of its basin's floor, sand
    /// drifted over it here and there, the stone of its walls, stands and stones, and rubble where
    /// they've fallen in
    fn ground_material(&self, seed: u32, x: f32, y: f32) -> Option<u8> {
        let (d, ..) = self.local(x, y);
        if d > ARENA_OUTER && !self.on_way(x, y) {
            return None;
        }
        if self.stones().any(|(at, wide, _)| (x - at[0]).hypot(y - at[1]) < wide + 0.6) {
            return Some(LIMESTONE);
        }
        if d < ARENA_BASIN - 4.5 {
            // A spiral of three arms winding in to the middle, teal on pale stone, picked out in gold
            let a = (y - self.middle[1]).atan2(x - self.middle[0]);
            let t = (a / TAU * 3.0 + d / 7.0).rem_euclid(1.0);
            return Some(if fbm(x / 6.0, y / 6.0, 2, seed.wrapping_add(71)) > 0.3 {
                SAND
            } else if t < 0.14 {
                TEMPLE_TEAL
            } else if t < 0.18 {
                GOLD
            } else {
                LIMESTONE
            });
        }
        let (fallen, _) = self.sector(seed, x, y);
        Some(if self.on_way(x, y) {
            BRICK_STONE
        } else if fallen && d > ARENA_BASIN + ARENA_TIER {
            [GRAVEL, BRICK_STONE, LIMESTONE][(hash3(x as u32, y as u32, seed ^ 0x2b1e) % 3) as usize]
        } else {
            LIMESTONE
        })
    }
}

/// The ground at `x`, `y` in meters with the Drowned Ring `arena` sunk into it, the ground being
/// `natural` there without it, in meters: see Arena
fn arena_ground(arena: &Arena, seed: u32, sea: f32, x: f32, y: f32, natural: f32) -> f32 {
    let (d, along, across) = arena.local(x, y);
    let way = arena.on_way(x, y) || along > ARENA_STANDS && along < ARENA_STANDS + arena.causeway + 4.0 && across < ARENA_WAY + 2.5;
    if d >= ARENA_REACH && !way {
        return natural;
    }
    let mut g = natural;
    if d < ARENA_REACH {
        let (fallen, flooded) = arena.sector(seed, x, y);
        let rubble = fbm(x / 2.0, y / 2.0, 2, seed.wrapping_add(61)).abs();
        // A flooded sector's sunk, the walk ankle deep and its stands lower with it, still a jump
        // up from one tier to the next
        let sunk = if flooded { 0.55 } else { 0.0 };
        let built = if d < ARENA_BASIN - 4.5 {
            // The basin's floor, deepest in the middle
            sea - ARENA_DEEP + 2.5 * (d / (ARENA_BASIN - 4.5)).powi(2)
        } else if d < ARENA_BASIN {
            // Its wall, sheer up to just under the water
            sea - ARENA_DEEP + 2.5 + ((d - ARENA_BASIN + 4.5) / 4.5).powi(3) * (ARENA_DEEP - 3.0)
        } else if d < ARENA_STANDS {
            // The walk at the water's edge and the tiers of the stands
            let tier = (((d - ARENA_BASIN) / ARENA_TIER) as usize).min(3);
            if fallen && tier > 0 {
                sea + 0.9 + rubble * tier as f32 * 0.6 - sunk
            } else {
                sea + ARENA_TIERS[tier] - sunk
            }
        } else {
            // Its outer wall down to the sea floor
            let top = if fallen { sea + 1.2 } else { sea + ARENA_TIERS[3] } - sunk;
            top + (sea - 3.0 - top) * smoothstep(ARENA_STANDS, ARENA_OUTER, d)
        };
        // Its island stones and stepping stones, sheer sided
        let built = arena.stones().fold(built, |h, (at, wide, top)| {
            let off = (x - at[0]).hypot(y - at[1]);
            if off < wide + 2.0 { h.max(sea + top - (off - wide).max(0.0) * 7.0) } else { h }
        });
        g = built + (natural - built) * smoothstep(ARENA_OUTER, ARENA_REACH, d);
    }
    if way && along > ARENA_BASIN - 0.5 {
        // The passage through the stands, cut level at the walk's height, and the causeway on out
        // to the land, its edges falling away into the sea
        let top = sea + 0.5;
        if along < ARENA_STANDS + 0.5 && across < ARENA_WAY {
            g = top;
        } else {
            let edge = (across - ARENA_WAY).max(0.0);
            g = g.max(top - edge * edge * 2.0);
        }
    }
    g
}

/// What the Drowned Ring `arena` has standing on its ground, in meters: pillars round the top of its
/// stands, some of them broken off or fallen, a great arch over its passage half fallen, and on its
/// basin's floor serpents of bronze gone green coiled and rearing, and fallen columns
fn arena_objects(settings: &TerrainSettings, arena: &Arena) -> Vec<Object> {
    let seed = settings.seed;
    let sea = sea_level_m(settings);
    let ground = |x: f32, y: f32| height_at(settings, x, y);
    let at = |a: f32, d: f32| [arena.middle[0] + a.cos() * d, arena.middle[1] + a.sin() * d];
    let mut objects = Vec::new();
    // The pillars, but where the passage goes through
    for k in 0..18 {
        let a = arena.facing + (k as f32 + 0.5) * TAU / 18.0;
        let [x, y] = at(a, ARENA_STANDS - 1.4);
        let (_, along, across) = arena.local(x, y);
        if along > 0.0 && across < ARENA_WAY + 3.5 {
            continue;
        }
        let mut rng = Rng(hash3(k, seed, 0xc01));
        let (fallen, _) = arena.sector(seed, x, y);
        let foot = ground(x, y);
        let mut shapes = vec![Shape::block([x - 0.95, y - 0.95, foot - 1.0], [x + 0.95, y + 0.95, foot + 0.6], LIMESTONE, MOSS)];
        if fallen {
            // Broken off, its top lying where it fell, out over the stands
            let stump = rng.range(1.5, 3.5);
            shapes.push(Shape::capsule([x, y, foot], [x, y, foot + stump], 0.75, 0.7, LIMESTONE, TEMPLE_TEAL, false).patterned(Pattern::Carved));
            let b = a + rng.range(-0.5, 0.5);
            let lie = [x - b.cos() * 1.5, y - b.sin() * 1.5];
            let end = [x - b.cos() * 7.5, y - b.sin() * 7.5];
            shapes.push(Shape::capsule([lie[0], lie[1], ground(lie[0], lie[1]) + 0.6], [end[0], end[1], ground(end[0], end[1]) + 0.5], 0.7, 0.7, LIMESTONE, TEMPLE_TEAL, false).patterned(Pattern::Carved));
        } else {
            let tall = if rng.next() < 0.3 { rng.range(4.0, 7.0) } else { rng.range(10.0, 12.5) };
            shapes.push(Shape::capsule([x, y, foot], [x, y, foot + tall], 0.75, 0.68, LIMESTONE, TEMPLE_TEAL, false).patterned(Pattern::Carved));
            if tall > 9.0 {
                shapes.push(Shape::block([x - 1.05, y - 1.05, foot + tall - 0.2], [x + 1.05, y + 1.05, foot + tall + 0.6], LIMESTONE, MOSS));
            }
        }
        objects.push(Object::new(shapes, rng.0));
    }
    // The gate: two great pillars either side of the passage at the foot of the stands, and an arch
    // between them, the half of it on one side fallen into the water
    let (fx, fy) = (arena.facing.cos(), arena.facing.sin());
    let gate = |along: f32, across: f32| [arena.middle[0] + fx * along - fy * across, arena.middle[1] + fy * along + fx * across];
    let rng = Rng(hash3(seed, 0x9a7e, 1));
    let mut shapes = Vec::new();
    let tall = 15.0;
    let foot = sea + 0.5;
    for side in [-1.0f32, 1.0] {
        let [x, y] = gate(ARENA_STANDS + 1.5, side * (ARENA_WAY + 1.6));
        shapes.push(Shape::block([x - 1.5, y - 1.5, foot - 3.0], [x + 1.5, y + 1.5, foot + 0.8], BRICK_STONE, MOSS));
        shapes.push(Shape::capsule([x, y, foot], [x, y, foot + tall], 1.15, 1.0, LIMESTONE, TEMPLE_TEAL, false).patterned(Pattern::Carved));
    }
    // Round from one pillar's top over to the other's, the near half of it still standing
    let span = ARENA_WAY + 1.6;
    for k in 0..6 {
        let (t0, t1) = (k as f32 / 12.0, (k + 1) as f32 / 12.0);
        let p = |t: f32| {
            let a = t * std::f32::consts::PI;
            let [x, y] = gate(ARENA_STANDS + 1.5, -a.cos() * span);
            [x, y, foot + tall + a.sin() * 4.0]
        };
        shapes.push(Shape::capsule(p(t0), p(t1), 0.85, 0.85, LIMESTONE, TEMPLE_TEAL, false).patterned(Pattern::Carved));
    }
    // Its keystone, a serpent's head with eyes of gold, lying broken in the shallows by the passage
    let [kx, ky] = gate(ARENA_STANDS - 2.0, ARENA_WAY + 2.6);
    let kz = ground(kx, ky);
    shapes.push(Shape::sphere([kx, ky, kz + 0.7], 1.1, BRONZE, TEMPLE_TEAL, false));
    shapes.push(Shape::capsule([kx, ky, kz + 0.7], [kx + fx * 1.6, ky + fy * 1.6, kz + 0.5], 0.8, 0.45, BRONZE, TEMPLE_TEAL, false));
    for side in [-1.0f32, 1.0] {
        shapes.push(Shape::sphere([kx - fy * side * 0.6 + fx * 0.6, ky + fx * side * 0.6 + fy * 0.6, kz + 1.25], 0.22, GOLD, AIR, false));
    }
    objects.push(Object::new(shapes, rng.0));
    // On the basin's floor: three great serpents of bronze gone green, coiled round on themselves
    // and rearing their heads up nearly to the surface, and fallen columns between them
    for k in 0..3 {
        let a = arena.facing + 0.9 + k as f32 * TAU / 3.0;
        let [x, y] = at(a, 7.5);
        let floor = ground(x, y);
        let mut rng = Rng(hash3(k, seed, 0x5e7a));
        let turn = rng.range(0.0, TAU);
        let mut shapes = Vec::new();
        let coil = |t: f32| {
            let r = 2.6 - t * 0.35;
            let b = turn + t * TAU;
            [x + b.cos() * r, y + b.sin() * r, floor + 0.7 + t * 0.9]
        };
        for s in 0..18 {
            let (t0, t1) = (s as f32 / 6.0, (s + 1) as f32 / 6.0);
            shapes.push(Shape::capsule(coil(t0), coil(t1), 0.75 - t0 * 0.12, 0.75 - t1 * 0.12, BRONZE, TEMPLE_TEAL, false));
        }
        // Its neck rearing up out of the middle of its coils, and its head
        let top = sea - rng.range(1.6, 2.6);
        let neck = [x + turn.cos() * 0.6, y + turn.sin() * 0.6, top - 0.8];
        shapes.push(Shape::capsule([x, y, floor + 2.6], neck, 0.62, 0.5, BRONZE, TEMPLE_TEAL, false));
        let snout = [neck[0] + (a + 3.0).cos() * 1.8, neck[1] + (a + 3.0).sin() * 1.8, top - 0.6];
        shapes.push(Shape::capsule(neck, snout, 0.7, 0.35, BRONZE, TEMPLE_TEAL, false));
        shapes.push(Shape::sphere([neck[0], neck[1], neck[2] + 0.35], 0.18, GOLD, AIR, false));
        objects.push(Object::new(shapes, rng.0));
        // A column fallen across the floor beside it
        let b = a + 1.05;
        let [cx, cy] = at(b, 15.0);
        let lie = rng.range(0.0, TAU);
        let (ex, ey) = (cx + lie.cos() * 3.5, cy + lie.sin() * 3.5);
        let (sx, sy) = (cx - lie.cos() * 3.5, cy - lie.sin() * 3.5);
        objects.push(Object::new(vec![Shape::capsule([sx, sy, ground(sx, sy) + 0.6], [ex, ey, ground(ex, ey) + 0.6], 0.7, 0.7, LIMESTONE, TEMPLE_TEAL, false).patterned(Pattern::Carved)], rng.0));
    }
    objects
}

/// Where the open sky's ground is said to be in the sky isles, far under everything: nothing's
/// built there, see Column::under
const SKY_VOID: f32 = -40.0;

// Terrain shapes are defined in meters, so more voxels per meter means the same landscape with finer voxels
fn height_at(settings: &TerrainSettings, x: f32, y: f32) -> f32 {
    let natural = base_height(settings, x, y);
    match arena_of(settings) {
        Some(arena) => arena_ground(&arena, settings.seed, sea_level_m(settings), x, y, natural),
        None => natural,
    }
}

/// The ground as it'd be without the Drowned Ring, see arena_of()
fn base_height(settings: &TerrainSettings, x: f32, y: f32) -> f32 {
    let seed = settings.seed;
    match land_of(seed) {
        Land::Sky => return sky_island(seed, x, y).map_or(SKY_VOID, |(top, _)| top),
        Land::Dream => return dream_height(seed, x, y),
        Land::Water => return water_height(seed, sea_level_m(settings), x, y),
        Land::Dragon => return dragon_height(seed, sea_level_m(settings), x, y),
        _ => {}
    }
    let height_m = settings.height_m as f32;

    let continent = fbm(x / 160.0, y / 160.0, 4, seed);
    let hills = fbm(x / 40.0, y / 40.0, 4, seed.wrapping_add(1));
    let mountain_mask = smoothstep(-0.05, 0.35, fbm(x / 120.0, y / 120.0, 3, seed.wrapping_add(2)));
    let ridges = ridged(x / 70.0, y / 70.0, 5, seed.wrapping_add(3));
    let bumps = fbm(x / 5.0, y / 5.0, 3, seed.wrapping_add(5));

    // The land goes on forever, with seas wherever the continents dip low
    let h = 0.36 + continent * 0.3 + hills * 0.06 + mountain_mask * ridges * 0.42;
    h * height_m + bumps * 0.4
}

/// How much a spot is each kind of land, 0-1 each. Where it's none of them is the temperate land
/// in between. Each stretches for a few hundred meters.
#[derive(Clone, Copy, Default)]
pub struct Climate {
    pub jungle: f32,
    pub desert: f32,
    pub snow: f32,
}

pub fn climate(settings: &TerrainSettings, x: f32, y: f32) -> Climate {
    let seed = settings.seed;
    // The sky isles are all green meadows and woods, and so's the dreaming realm, in colours of its
    // own, see shaders\traverse.comp
    match land_of(seed) {
        Land::Sky | Land::Dream => return Climate::default(),
        // The frozen realm's snow from end to end
        Land::Ice => return Climate { snow: 1.0, ..Climate::default() },
        // The sunken reach's islands green, sandy at their shores
        Land::Water => return Climate::default(),
        // Ash and rock, nothing growing, see Terrain::column()
        Land::Dragon => return Climate::default(),
        Land::Normal => {}
    }
    let heat = fbm(x / 900.0, y / 900.0, 3, seed.wrapping_add(31)) * 1.6;
    let wet = fbm(x / 650.0, y / 650.0, 3, seed.wrapping_add(32)) * 1.6;
    let hot = smoothstep(0.1, 0.4, heat);
    Climate {
        jungle: hot * smoothstep(-0.05, 0.25, wet),
        desert: hot * smoothstep(0.05, -0.25, wet),
        snow: smoothstep(-0.2, -0.5, heat),
    }
}

/// The nearest spot to `near` that's very much `kind` of land ("jungle", "desert", "snow" or
/// "temperate") and above the sea, in meters. For trying out each kind of land.
pub fn find_climate(settings: &TerrainSettings, kind: &str, near: [f32; 2]) -> Option<[f32; 2]> {
    let sea_level = sea_level_m(settings);
    for ring in 0..120 {
        let count = (ring * 6).max(1);
        for i in 0..count {
            let angle = i as f32 / count as f32 * TAU;
            let (x, y) = (near[0] + angle.cos() * ring as f32 * 64.0, near[1] + angle.sin() * ring as f32 * 64.0);
            let c = climate(settings, x, y);
            let strength = match kind {
                "jungle" => c.jungle,
                "desert" => c.desert,
                "snow" => c.snow,
                // Woods where the leaves have turned, see Terrain::canopy()
                "autumn" => (1.0 - c.jungle.max(c.desert).max(c.snow)) * f32::from(trees::autumn_woods(settings, x, y)),
                _ => 1.0 - c.jungle.max(c.desert).max(c.snow),
            };
            if strength > 0.95 && height_at(settings, x, y) > sea_level + 3.0 {
                return Some([x, y]);
            }
        }
    }
    None
}

/// Whether `at`, in meters, is dry land `room` meters round, a little over the sea and not steep:
/// room for something to stand on, as the gate to the realms or a raid boss's lair, see realms.rs.
/// Only the land's shape is looked at, not the rivers cut into it, which are narrow.
pub fn is_dry_land(settings: &TerrainSettings, at: [f32; 2], room: f32) -> bool {
    let sea = sea_level_m(settings);
    let h = height_at(settings, at[0], at[1]);
    let step = room.min(3.0);
    let near = [[step, 0.0], [-step, 0.0], [0.0, step], [0.0, -step]].map(|[dx, dy]| height_at(settings, at[0] + dx, at[1] + dy));
    let far = (0..8).map(|k| {
        let a = k as f32 * TAU / 8.0;
        height_at(settings, at[0] + a.cos() * room, at[1] + a.sin() * room)
    });
    h > sea + 1.5 && near.iter().all(|&r| r > sea + 1.0 && (r - h).abs() < 1.6) && far.into_iter().all(|r| r > sea + 1.0 && (r - h).abs() < room * 0.5)
}

/// How a spot looks from high above, for the map, see src\compass.rs: how high its ground is over
/// the sea, in meters, under it if that's less than nothing, or None in the open air between the
/// sky isles. Only the land's shape, not the rivers cut into it.
pub fn seen_from_above(settings: &TerrainSettings, x: f32, y: f32) -> Option<f32> {
    land_height_m(settings, x, y).map(|h| h - sea_level_m(settings))
}

/// How high the land's own ground is at a spot, in meters, not what stands or floats on it, or
/// None in the open air between the sky isles. Only its shape: rivers cut it a little lower.
pub fn land_height_m(settings: &TerrainSettings, x: f32, y: f32) -> Option<f32> {
    let h = height_at(settings, x, y);
    (h > SKY_VOID).then_some(h)
}

/// The nearest dry land to `near` within `within` meters with `room` round it, see is_dry_land(),
/// that's `also` fine, looking in rings four meters apart
pub fn dry_land_near(settings: &TerrainSettings, near: [f32; 2], within: f32, room: f32, also: &dyn Fn([f32; 2]) -> bool) -> Option<[f32; 2]> {
    for ring in 0..=(within / 4.0) as usize {
        let count = (ring * 6).max(1);
        for i in 0..count {
            let angle = i as f32 / count as f32 * TAU;
            let at = [near[0] + angle.cos() * ring as f32 * 4.0, near[1] + angle.sin() * ring as f32 * 4.0];
            if is_dry_land(settings, at, room) && also(at) {
                return Some(at);
            }
        }
    }
    None
}

/// Whether a river runs within `margin` meters of `at`, its banks and all, in meters: as is_dry_land()
/// doesn't look, for where rivers mustn't be, as the Cinder Crown's of lava
pub fn river_near(settings: &TerrainSettings, at: [f32; 2], margin: f32, cache: &RiverCache) -> bool {
    let terrain = Terrain::around(settings, at, at);
    let (lo, hi) = (at.map(|v| v - margin), at.map(|v| v + margin));
    terrain.rivers_near(cache, lo, hi).iter().any(|river| river.iter().any(|p| (p.x - at[0]).hypot(p.y - at[1]) < p.half_width + margin))
}

/// What a raid boss's lair is by, in a realm that says, see realms.rs: anywhere there's room, or as
/// in the Cinder Crown, by a dragons' roost high on a peak, on the rim of a caldera, or at the mouth
/// of a bone cave
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Landmark {
    Anywhere,
    Roost,
    Caldera,
    BoneCave,
    /// In the Drowned Ring, out in the middle of its basin, see Arena: for a finale fought there
    Arena,
}

/// Where the nearest `mark` to `near` is, in meters, for a lair to be on the nearest dry land to:
/// a roost itself, a caldera's rim on the side towards `near`, out in front of a bone cave's great
/// skull. None for Anywhere, or if there's none of them anywhere near.
pub fn landmark_near(settings: &TerrainSettings, mark: Landmark, near: [f32; 2], cache: &RiverCache) -> Option<[f32; 2]> {
    let seed = settings.seed;
    let away = |p: [f32; 2]| (p[0] - near[0]).hypot(p[1] - near[1]);
    // Of the squares `cell` meters on a side out to `reach` of them round `near`, the nearest spot
    // `spot` finds in one
    let nearest = |cell: f32, reach: i32, spot: &dyn Fn(i32, i32) -> Option<([f32; 2], f32)>| {
        let (ci, cj) = ((near[0] / cell).floor() as i32, (near[1] / cell).floor() as i32);
        (cj - reach..=cj + reach).flat_map(|j| (ci - reach..=ci + reach).map(move |i| (i, j))).filter_map(|(i, j)| spot(i, j)).min_by(|a, b| away(a.0).total_cmp(&away(b.0)))
    };
    match mark {
        Landmark::Anywhere => None,
        Landmark::Roost => nearest(ROOST_CELL, 5, &|i, j| {
            let (at, r) = scatter_spot(seed, ROOST_CELL, ROOST_SALT, i, j);
            has_roost(settings, at[0], at[1], r).then_some((at, 0.0))
        })
        .map(|(at, _)| at),
        Landmark::Caldera => nearest(CALDERA_CELL, 2, &|i, j| caldera_in(seed, i, j)).map(|(middle, radius)| {
            let (dx, dy) = (near[0] - middle[0], near[1] - middle[1]);
            let length = dx.hypot(dy).max(1e-3);
            [middle[0] + dx / length * radius * 1.45, middle[1] + dy / length * radius * 1.45]
        }),
        Landmark::BoneCave => nearest(BONE_CAVE_CELL, 2, &|i, j| {
            let (at, r) = scatter_spot(seed, BONE_CAVE_CELL, BONE_CAVE_SALT, i, j);
            bone_cave_facing(settings, at[0], at[1], r, cache).map(|facing| (at, facing))
        })
        .map(|(at, facing)| [at[0] + facing.cos() * BONE_CAVE_MOUTH, at[1] + facing.sin() * BONE_CAVE_MOUTH]),
        Landmark::Arena => arena_of(settings).map(|a| a.middle).filter(|&m| away(m) < 600.0),
    }
}

/// The one spot Terrain::features() scatters something on in the `cell` meter square `i`, `j` with
/// `salt`, and the hash it hands on for what's put there, the same whatever piece is built
fn scatter_spot(seed: u32, cell: f32, salt: u32, i: i32, j: i32) -> ([f32; 2], u32) {
    let r = hash3(i as u32, j as u32, seed ^ salt);
    ([(i as f32 + unit(r)) * cell, (j as f32 + unit(r >> 16)) * cell], hash(r))
}

/// The Cinder Crown's dragons' roosts, at most one in each ROOST_CELL meter square, see has_roost()
const ROOST_CELL: f32 = 48.0;
const ROOST_SALT: u32 = 0x2005;

/// Whether the spot scattered on at `x`, `y` with hash `r` has a roost on it: high on a peak
fn has_roost(settings: &TerrainSettings, x: f32, y: f32, r: u32) -> bool {
    unit(r) < 0.55 && height_at(settings, x, y) > sea_level_m(settings) + 34.0
}

/// The Cinder Crown's great skeletons, at most one in each BONE_CAVE_CELL meter square, see
/// bone_cave(), and how far in front of the middle of one its mouth is, past its skull
const BONE_CAVE_CELL: f32 = 260.0;
const BONE_CAVE_SALT: u32 = 0xb0ca;
const BONE_CAVE_MOUTH: f32 = 26.0;

/// Which way the great skeleton at the spot scattered on at `x`, `y` with hash `r` faces, if
/// there's one there: on dry, level ash with room all round it and no river of lava near
fn bone_cave_facing(settings: &TerrainSettings, x: f32, y: f32, r: u32, cache: &RiverCache) -> Option<f32> {
    if unit(r) > 0.85 || !is_dry_land(settings, [x, y], 8.0) {
        return None;
    }
    let h = height_at(settings, x, y);
    let level = (0..8).all(|k| {
        let a = k as f32 * TAU / 8.0;
        (height_at(settings, x + a.cos() * 13.0, y + a.sin() * 13.0) - h).abs() < 3.0
    });
    (level && !river_near(settings, [x, y], BONE_CAVE_MOUTH + 4.0, cache)).then(|| unit(r >> 8) * TAU)
}

/// A bone cave: a dragon's skeleton far greater than the rest, lying on its belly facing `facing`,
/// its spine arched high and its ribs curving down either side into the ash, with room to walk in
/// under them, its skull at the mouth, horns swept back
fn bone_cave(settings: &TerrainSettings, at: [f32; 2], facing: f32, r: u32) -> Object {
    let mut rng = Rng(hash(r ^ 0xb0ca));
    let (dx, dy) = (facing.cos(), facing.sin());
    // `t` meters along it from its middle and `side` meters off to its left, `up` over the ground
    // there, or over its middle's for the spine
    let ground = |p: [f32; 2]| height_at(settings, p[0], p[1]);
    let flat = |t: f32, side: f32| [at[0] + dx * t - dy * side, at[1] + dy * t + dx * side];
    let base = ground(at);
    let on = |t: f32, side: f32, up: f32| {
        let p = flat(t, side);
        [p[0], p[1], ground(p) + up]
    };
    let over = |t: f32, side: f32, up: f32| {
        let p = flat(t, side);
        [p[0], p[1], base + up]
    };
    // From the tail lying in the ash up over the back and down the neck, how high along it
    const SPINE: [(f32, f32); 5] = [(-15.0, 0.6), (-8.0, 5.6), (0.0, 8.2), (6.0, 7.4), (11.0, 3.8)];
    let spine_at = |t: f32| {
        let k = SPINE.windows(2).position(|w| t <= w[1].0).unwrap_or(SPINE.len() - 2);
        let (a, b) = (SPINE[k], SPINE[k + 1]);
        a.1 + (b.1 - a.1) * ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0)
    };
    let mut shapes: Vec<Shape> = SPINE.windows(2).map(|w| Shape::capsule(over(w[0].0, 0.0, w[0].1), over(w[1].0, 0.0, w[1].1), 0.5 + w[0].1 * 0.05, 0.5 + w[1].1 * 0.05, BONE, FUR_CREAM, false)).collect();
    // Knobs of its backbone
    for k in 0..12 {
        let t = -12.0 + k as f32 * 2.0;
        shapes.push(Shape::sphere(over(t, 0.0, spine_at(t) + 0.55), 0.55, BONE, FUR_CREAM, false));
    }
    // Its ribs, longest in the middle, from the back out and down into the ground
    for k in 0..7 {
        let t = -7.0 + k as f32 * 2.0;
        let top = spine_at(t);
        let spread = 6.0 - ((t + 1.0) / 6.0).powi(2) * 1.2 + rng.range(-0.2, 0.2);
        for side in [-1.0f32, 1.0] {
            let p0 = over(t, 0.0, top - 0.2);
            let p1 = over(t + 0.3, side * spread * 0.62, top * 0.86);
            let p2 = over(t + 0.6, side * spread, top * 0.42);
            let foot = on(t + 0.8, side * spread * 1.04, -0.8);
            shapes.push(Shape::capsule(p0, p1, 0.46, 0.4, BONE, FUR_CREAM, false));
            shapes.push(Shape::capsule(p1, p2, 0.4, 0.33, BONE, FUR_CREAM, false));
            shapes.push(Shape::capsule(p2, foot, 0.33, 0.26, BONE, FUR_CREAM, false));
        }
    }
    // Its skull at the end of its neck, the long snout and jaw lying on the ash, empty black eyes
    // and horns swept back
    shapes.push(Shape::sphere(on(13.6, 0.0, 2.6), 2.6, BONE, FUR_CREAM, false));
    shapes.push(Shape::capsule(on(15.0, 0.0, 2.4), on(19.4, 0.0, 1.3), 1.7, 0.9, BONE, FUR_CREAM, false));
    shapes.push(Shape::capsule(on(14.4, 0.0, 0.7), on(19.0, 0.0, 0.4), 1.1, 0.6, BONE, FUR_CREAM, false));
    for side in [-1.0f32, 1.0] {
        shapes.push(Shape::sphere(on(15.4, side * 1.45, 3.1), 0.6, OBSIDIAN, AIR, false));
        shapes.push(Shape::capsule(on(12.6, side * 1.5, 4.2), on(8.8, side * 3.4, 6.6), 0.62, 0.12, BONE, OBSIDIAN, false));
    }
    Object::new(shapes, rng.0)
}

fn is_grassy(material: u8) -> bool {
    matches!(material, GRASS | JUNGLE_GRASS)
}

fn sea_level_m(settings: &TerrainSettings) -> f32 {
    match land_of(settings.seed) {
        // Far down under the floating islands, seen only beyond them, see sky_island()
        Land::Sky => 3.0,
        // Lava down in the calderas, and the rivers of it run down into them
        Land::Dragon => 6.0,
        _ => settings.height_m as f32 * 0.3,
    }
}

/// The landscape goes on forever. Rivers start at springs, at most one in each RIVER_CELL meter
/// square, and once traced they're kept here, so the same river turns up whichever part of the
/// landscape gets built. Shared between the parts built so far and the next.
#[derive(Default)]
pub struct RiverCache {
    cells: Mutex<HashMap<(i32, i32), Option<Arc<Vec<RiverPoint>>>>>,
}

const RIVER_CELL: f32 = 140.0;
/// The most points a river runs to, half a meter apart
const RIVER_STEPS: usize = 1200;
/// How much water a river's spring gives, in cubic meters a second, and how wide it is in meters
const RIVER_FLOW: f32 = 6.0;
const RIVER_SPRING_RADIUS: f32 = 2.0;
/// And how much more each waterfall's fed just over its lip
const WATERFALL_FLOW: f32 = 4.0;
/// How wide a stream gets, half across in meters, and how many of them grow into great wide
/// rivers, and how much wider those grow, over how many meters
const STREAM_HALF_WIDTH: f32 = 3.0;
const GREAT_RIVERS: f32 = 0.4;
const GREAT_RIVER_WIDENS: [f32; 2] = [2.5, 5.5];
const GREAT_RIVER_GROWS_M: f32 = 150.0;
/// A river that's come less than a few meters in this many steps is going round in a hollow. It
/// spills out over the lowest point this far round it, at most so many times.
const RIVER_STUCK_STEPS: usize = 40;
const RIVER_RIM: f32 = 30.0;
const RIVER_SPILLS: usize = 4;

/// How much more water a river or waterfall `half_width` across needs than a stream, to stay full
fn river_scale(half_width: f32) -> f32 {
    (half_width / STREAM_HALF_WIDTH).max(1.0)
}

/// Where each of `rivers` starts in the rectangle from `lo` to `hi`, in meters: at its spring, or
/// where it flows in from outside. x, y, cubic meters a second and radius, in meters.
fn river_springs(rivers: &[Arc<Vec<RiverPoint>>], lo: [f32; 2], hi: [f32; 2]) -> Vec<[f32; 4]> {
    let inside = |p: &&RiverPoint| p.x >= lo[0] && p.y >= lo[1] && p.x < hi[0] && p.y < hi[1];
    rivers
        .iter()
        .filter_map(|path| path.iter().find(inside))
        .map(|p| [p.x, p.y, RIVER_FLOW * river_scale(p.half_width), RIVER_SPRING_RADIUS * river_scale(p.half_width)])
        .collect()
}

/// The same for the world, from `lo` to `hi` in meters, wherever it's moved to: what water it
/// has flowing in besides the springs of each of its pieces, see Terrain::piece_springs()
pub fn window_springs(settings: &TerrainSettings, cache: &RiverCache, lo: [f32; 2], hi: [f32; 2]) -> Vec<[f32; 4]> {
    if matches!(land_of(settings.seed), Land::Ice | Land::Dragon) {
        return Vec::new();
    }
    let terrain = Terrain::around(settings, lo, hi);
    river_springs(&terrain.rivers_near(cache, lo, hi), lo, hi)
}

/// Where rivers pour over waterfalls from `lo` to `hi` in meters, as Terrain::waterfalls() says
#[cfg_attr(not(test), allow(dead_code))]
pub fn waterfalls_in(settings: &TerrainSettings, cache: &RiverCache, lo: [f32; 2], hi: [f32; 2]) -> Vec<([f32; 3], [f32; 2], f32)> {
    let mut terrain = Terrain::around(settings, lo, hi);
    terrain.rivers = terrain.rivers_near(cache, lo, hi);
    terrain.waterfalls()
}

/// Little things inside buildings, drawn like animals, see src\animals.rs
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PropKind {
    Candle,
    Skull,
    /// With a weapon in it, see src\weapons.rs
    Chest,
}

/// In meters, from the corner of the whole landscape
#[derive(Clone, Copy)]
pub struct Prop {
    pub kind: PropKind,
    pub position: [f32; 3],
    pub heading: f32,
    /// For chests, how much rarer what's in it is likely to be, see Library::odds()
    pub luck: f32,
}

/// Something glowing that lights up what's around it, like a candle flame. In meters. With a
/// negative color, a glow hanging in the air of a fallen city instead, see Terrain::glow() in
/// undercity.rs.
#[derive(Clone, Copy)]
pub struct Light {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub reach: f32,
}

/// Where water lies on something other than the ground, like the floor of a room inside a
/// building, from `lo` to `hi`, in meters. With water up to `water` to start with, if that's higher.
struct Floor {
    lo: [f32; 2],
    hi: [f32; 2],
    height: f32,
    water: f32,
}

/// What buildings and caves add besides voxels
#[derive(Default)]
struct Features {
    props: Vec<Prop>,
    lights: Vec<Light>,
    /// Water welling up inside buildings: x, y, cubic meters a second, radius, in meters
    springs: Vec<[f32; 4]>,
    floors: Vec<Floor>,
    /// Where buildings stand, so nothing else grows there: x0, y0, x1, y1 and how high they reach
    footprints: Vec<[f32; 5]>,
    /// Where the ground water lies on has to be found from the voxels, as something's been cut
    /// into it: x0, y0, x1, y1 and how high to look down from
    recuts: Vec<[f32; 5]>,
    /// Where the ground is built over or cut away, so the grass drawn in the shader doesn't grow
    /// there: x0, y0, x1, y1
    holes: Vec<[f32; 4]>,
    sites: Vec<Site>,
    /// Bandits' camps, see camps.rs
    camps: Vec<Camp>,
}

/// A building: what kind, where its middle is and which way it faces, with the lowest ground
/// under it, in meters
#[derive(Clone, Copy)]
pub struct Site {
    pub kind: &'static str,
    pub position: [f32; 3],
    pub facing: [f32; 2],
}

/// Everything about a piece of the landscape that doesn't depend on its resolution: the shape
/// of the land, the rivers, and every tree, plant, rock, building and cave. Any part of it can be
/// built into voxels at any resolution, see build().
pub struct Terrain {
    settings: TerrainSettings,
    /// What's placed for this piece of the landscape, in meters: everything reaching in here, well
    /// beyond `own_lo` to `own_hi`, so what's placed near the edge of that is placed just as it
    /// would be for any other piece, see PLACE_MARGIN_M
    lo: [f32; 2],
    hi: [f32; 2],
    /// The piece itself, which is built, and whose props, lights, springs, buildings and camps are
    /// its own, so pieces next to each other don't both have those along where they meet
    own_lo: [f32; 2],
    own_hi: [f32; 2],
    rivers: Vec<Arc<Vec<RiverPoint>>>,
    objects: Vec<Object>,
    // Indices into `objects` for each OBJECT_CELL-sized square they overlap, to find them by area
    object_cells: Vec<Vec<u32>>,
    cells_across: [usize; 2],
    features: Features,
    /// The Drowned Ring, if the landscape has one, see arena_of()
    arena: Option<Arena>,
}

const OBJECT_CELL: f32 = 8.0;

/// How far past the edge of a piece of the landscape things are placed, in meters, see Terrain::lo:
/// the farthest out something can be that reaches in, like the crown of a tree or a building, or
/// that has a say in whether something reaching in is there, like a building or a camp keeping
/// clear of another, and of what that keeps clear of
const PLACE_MARGIN_M: f32 = 64.0;
/// And past that how far out rivers are traced for the ground to be worked out at, see Heights::at()
const RIVER_MARGIN_M: f32 = 64.0;

impl Terrain {
    /// The piece of the landscape `heights` covers, with nothing in it yet
    fn new(settings: &TerrainSettings, heights: &Heights) -> Self {
        let vpm = heights.voxels_per_meter;
        let own_lo = [heights.origin[0] as f32 / vpm, heights.origin[1] as f32 / vpm];
        let own_hi = [own_lo[0] + heights.size[0] as f32 / vpm, own_lo[1] + heights.size[1] as f32 / vpm];
        Self::around(settings, own_lo, own_hi)
    }

    /// The piece of the landscape from `own_lo` to `own_hi` in meters, with nothing in it yet
    fn around(settings: &TerrainSettings, own_lo: [f32; 2], own_hi: [f32; 2]) -> Self {
        Terrain {
            settings: *settings,
            lo: own_lo.map(|v| v - PLACE_MARGIN_M),
            hi: own_hi.map(|v| v + PLACE_MARGIN_M),
            own_lo,
            own_hi,
            rivers: Vec::new(),
            objects: Vec::new(),
            object_cells: Vec::new(),
            cells_across: [0; 2],
            features: Features::default(),
            arena: arena_of(settings),
        }
    }

    /// Cuts the rivers running through this piece into `heights`
    fn carve_rivers(&mut self, heights: &mut Heights, rivers: &RiverCache) {
        self.rivers = self.rivers_in(rivers);
        heights.carve(&self.rivers);
    }

    /// Puts everything that's out on the land, or under it, once the rivers are in. Of the props,
    /// lights, springs, sites and camps, only those in the piece itself are kept.
    fn place(&mut self, heights: &Heights, rivers: &RiverCache) {
        let (lo, hi) = (self.lo, self.hi);
        let (objects, mut features) = self.place_objects(heights, rivers);
        let own = |p: &[f32]| self.inside(p[0], p[1]);
        features.props.retain(|p| own(&p.position));
        features.lights.retain(|l| own(&l.position));
        features.springs.retain(|s| own(s));
        features.sites.retain(|s| own(&s.position));
        features.camps.retain(|c| own(&c.fire));
        self.objects = objects;
        self.features = features;

        let cells = [0, 1].map(|i| ((hi[i] - lo[i]) / OBJECT_CELL).ceil() as usize);
        let mut object_cells = vec![Vec::new(); cells[0] * cells[1]];
        let cell = |v: f32, axis: usize| (((v - lo[axis]) / OBJECT_CELL).floor().max(0.0) as usize).min(cells[axis] - 1);
        for (i, object) in self.objects.iter().enumerate() {
            for cy in cell(object.min[1], 1)..=cell(object.max[1], 1) {
                for cx in cell(object.min[0], 0)..=cell(object.max[0], 0) {
                    object_cells[cx + cy * cells[0]].push(i as u32);
                }
            }
        }
        self.object_cells = object_cells;
        self.cells_across = cells;
    }

    /// Whether a point in meters is in the piece itself, see Terrain::own_lo
    fn inside(&self, x: f32, y: f32) -> bool {
        x >= self.own_lo[0] && y >= self.own_lo[1] && x < self.own_hi[0] && y < self.own_hi[1]
    }

    /// Where rivers pour over waterfalls in this piece: the lip of each in meters, which way the
    /// river flows there, and how wide it is, half across
    pub fn waterfalls(&self) -> Vec<([f32; 3], [f32; 2], f32)> {
        self.waterfalls_where(|x, y| self.inside(x, y))
    }

    /// The same for all of them placed for, out to Terrain::lo, for what's built at them
    fn waterfalls_near(&self) -> Vec<([f32; 3], [f32; 2], f32)> {
        let (lo, hi) = (self.lo, self.hi);
        self.waterfalls_where(|x, y| x >= lo[0] && y >= lo[1] && x < hi[0] && y < hi[1])
    }

    fn waterfalls_where(&self, keep: impl Fn(f32, f32) -> bool) -> Vec<([f32; 3], [f32; 2], f32)> {
        let mut falls = Vec::new();
        for path in &self.rivers {
            for pair in path.windows(2) {
                let (a, b) = (&pair[0], &pair[1]);
                if a.surface - b.surface > 1.0 && keep(a.x, a.y) {
                    let (dx, dy) = (b.x - a.x, b.y - a.y);
                    let length = dx.hypot(dy).max(1e-6);
                    falls.push(([a.x, a.y, a.surface], [dx / length, dy / length], a.half_width));
                }
            }
        }
        falls
    }

    /// Where the widest river within half a kilometer of `near` is at its widest, inland, in
    /// meters. For trying them out.
    pub fn widest_river_near(&self, near: [f32; 2], cache: &RiverCache) -> Option<[f32; 2]> {
        let (inland, reach) = (sea_level_m(&self.settings) + 2.0, 500.0);
        let mut widest: Option<(f32, [f32; 2])> = None;
        for river in self.rivers_near(cache, [near[0] - reach, near[1] - reach], [near[0] + reach, near[1] + reach]) {
            for p in river.iter() {
                let close = (p.x - near[0]).abs() < reach && (p.y - near[1]).abs() < reach;
                if close && p.surface > inland && widest.is_none_or(|(w, _)| p.half_width > w) {
                    widest = Some((p.half_width, [p.x, p.y]));
                }
            }
        }
        widest.map(|(_, at)| at)
    }

    /// Where a river in this piece is at its widest, away from waterfalls and the sea: the middle
    /// of its surface in meters, which way it flows, and how wide it is, half across. For trying
    /// them out.
    pub fn widest_river(&self) -> Option<([f32; 3], [f32; 2], f32)> {
        let inland = sea_level_m(&self.settings) + 2.0;
        let mut widest: Option<([f32; 3], [f32; 2], f32)> = None;
        for path in &self.rivers {
            for i in 8..path.len().saturating_sub(8) {
                let (a, b) = (&path[i - 8], &path[i + 8]);
                let p = &path[i];
                if self.inside(p.x, p.y) && p.surface > inland && a.surface == b.surface && widest.is_none_or(|w| p.half_width > w.2) {
                    let (dx, dy) = (b.x - a.x, b.y - a.y);
                    let length = dx.hypot(dy).max(1e-6);
                    widest = Some(([p.x, p.y, p.surface], [dx / length, dy / length], p.half_width));
                }
            }
        }
        widest
    }

    /// Where water comes from in this piece: x, y, cubic meters a second and radius, in meters.
    /// Rivers start where their spring is, or where they flow in from outside it, and some
    /// buildings have water welling up inside. Each waterfall is fed just over its lip too, so it
    /// always pours over full, however much the pools above have spread out what came down.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn springs(&self) -> Vec<[f32; 4]> {
        let mut springs = self.river_springs();
        springs.extend(self.piece_springs());
        springs
    }

    /// The springs of springs() where rivers start in this piece or flow into it, see river_springs()
    pub fn river_springs(&self) -> Vec<[f32; 4]> {
        if matches!(land_of(self.settings.seed), Land::Ice | Land::Dragon) {
            return Vec::new();
        }
        river_springs(&self.rivers, self.own_lo, self.own_hi)
    }

    /// The springs of springs() that belong to this piece wherever the world is, rather than to
    /// where its edge is: the waterfalls' and the buildings', see river_springs() for the rest.
    /// None in the frozen realm, where every river and fall is iced over, nor where they're lava.
    pub fn piece_springs(&self) -> Vec<[f32; 4]> {
        if matches!(land_of(self.settings.seed), Land::Ice | Land::Dragon) {
            return Vec::new();
        }
        let mut springs: Vec<[f32; 4]> = self
            .waterfalls()
            .into_iter()
            .map(|(lip, flow, wide)| {
                let more = river_scale(wide);
                [lip[0] - flow[0] * 1.5, lip[1] - flow[1] * 1.5, WATERFALL_FLOW * more, 1.2 * more]
            })
            .collect();
        springs.extend_from_slice(&self.features.springs);
        springs
    }

    /// What's in this piece of the landscape besides its voxels, see PieceFeatures, kept once the
    /// rest of it isn't
    pub fn piece_features(&self) -> PieceFeatures {
        PieceFeatures {
            lo: self.own_lo,
            hi: self.own_hi,
            props: self.features.props.clone(),
            lights: self.features.lights.clone(),
            sites: self.features.sites.clone(),
            camps: self.features.camps.clone(),
            springs: self.piece_springs(),
            waterfalls: self.waterfalls(),
        }
    }

    /// Candles, skulls and such, in meters
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn props(&self) -> &[Prop] {
        &self.features.props
    }

    /// Flames and glowing crystals, in meters
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn lights(&self) -> &[Light] {
        &self.features.lights
    }

    /// The buildings in this piece of the landscape, and the halls of dungeons: "dungeon hall" at
    /// the middle of each hall's floor, and "dungeon" at the middle of the floor of the deepest,
    /// the boss's, each facing its way in
    pub fn sites(&self) -> &[Site] {
        &self.features.sites
    }

    /// The bandits' camps in this piece of the landscape: who keeps each, and where the chest of
    /// their hoard turns up once they're all dead
    pub fn bandit_camps(&self) -> &[Camp] {
        &self.features.camps
    }

    /// Where the way into the nearest dungeon to `near` is, anywhere within about 3 km: the
    /// landing in front of the doorway of its crypt, in meters. For trying them out. Takes a
    /// tenth of a second or so, tracing the rivers round each dungeon it looks at.
    #[allow(dead_code)]
    pub fn nearest_dungeon(&self, near: [f32; 2]) -> Option<[f32; 3]> {
        let cache = RiverCache::default();
        let home = near.map(|v| (v / DUNGEON_CELL).floor() as i32);
        let mut best: Option<(f32, [f32; 3])> = None;
        for ring in 0..10i32 {
            for cy in home[1] - ring..=home[1] + ring {
                for cx in home[0] - ring..=home[0] + ring {
                    if (cx - home[0]).abs().max((cy - home[1]).abs()) != ring {
                        continue;
                    }
                    if let Some(d) = self.dungeon_in(cx, cy, &cache) {
                        let e = d.entrance();
                        let distance = (e[0] - near[0]).hypot(e[1] - near[1]);
                        if best.is_none_or(|(b, _)| distance < b) {
                            best = Some((distance, e));
                        }
                    }
                }
            }
            // Any further out are at least this far away
            if best.is_some_and(|(b, _)| b <= ring as f32 * DUNGEON_CELL) {
                break;
            }
        }
        best.map(|(_, e)| e)
    }

    /// The nearest boss's cavern to `near`, within a kilometer or so: the middle of its floor in
    /// meters, the way in from there, and how far out there's room all round. For trying them out.
    #[allow(dead_code)]
    pub fn nearest_cavern(&self, near: [f32; 2]) -> Option<([f32; 3], [f32; 2], f32)> {
        let home = near.map(|v| (v / CAVE_CELL).floor() as i32);
        let mut best: Option<(f32, ([f32; 3], [f32; 2], f32))> = None;
        for cy in home[1] - 8..=home[1] + 8 {
            for cx in home[0] - 8..=home[0] + 8 {
                if let Some(lair) = self.cave_in(cx, cy).and_then(|c| c.lairs.last().copied()) {
                    let distance = (lair.0[0] - near[0]).hypot(lair.0[1] - near[1]);
                    if best.is_none_or(|(b, _)| distance < b) {
                        best = Some((distance, lair));
                    }
                }
            }
        }
        best.map(|(_, lair)| lair)
    }

    /// Builds a box of the landscape into voxels. `origin` and `size` are in voxels at
    /// `voxels_per_meter`, counted from the corner of the whole landscape at that resolution.
    /// It has to be within this piece of it. Without water, which is simulated separately,
    /// see src\water.rs.
    pub fn build(&self, origin: [i64; 3], size: [u32; 3], voxels_per_meter: u32) -> World {
        let mut heights = Heights::new(&self.settings, [origin[0], origin[1]], [size[0] as usize, size[1] as usize], voxels_per_meter);
        heights.carve(&self.rivers);
        let mut world = self.build_from(&heights, origin[2], size[2]);
        world.sea_level = None;
        world
    }

    /// Buildings stand on their own floors, which water can't flow through but can lie on, and
    /// some have floors inside with water on them. Changes `water` to match `world`, built from
    /// this piece of landscape.
    fn raise_water_ground(&self, world: &World, water: &mut WaterMap) {
        let vpm = world.voxels_per_meter as f32;
        let [sx, sy] = [world.size[0] as i64, world.size[1] as i64];
        let columns = |lo: [f32; 2], hi: [f32; 2]| {
            let x0 = ((lo[0] * vpm).floor() as i64 - world.origin[0]).max(0);
            let y0 = ((lo[1] * vpm).floor() as i64 - world.origin[1]).max(0);
            let x1 = ((hi[0] * vpm).ceil() as i64 - world.origin[0]).min(sx);
            let y1 = ((hi[1] * vpm).ceil() as i64 - world.origin[1]).min(sy);
            (y0..y1).flat_map(move |y| (x0..x1).map(move |x| (x, y)))
        };
        for footprint in &self.features.footprints {
            let top = (footprint[4] * vpm).ceil() as i64;
            for (x, y) in columns([footprint[0], footprint[1]], [footprint[2], footprint[3]]) {
                let i = (x + y * sx) as usize;
                let ground = water.ground[i] as i64;
                if let Some(z) = (ground..=top).rev().find(|&z| !is_passable(world.material_at(x, y, z))) {
                    water.ground[i] = water.ground[i].max((z + 1) as f32);
                    if water.surface[i] <= water.ground[i] {
                        water.surface[i] = 0.0;
                    }
                }
            }
        }
        // Ravines and such cut into the ground: water lies on whatever's left, and falls in
        let is_wood = |m: u8| matches!(m, WOOD | BIRCH_BARK | PALM_WOOD | BARK_DARK | DEAD_WOOD);
        for cut in &self.features.recuts {
            let top = (cut[4] * vpm).ceil() as i64;
            for (x, y) in columns([cut[0], cut[1]], [cut[2], cut[3]]) {
                let i = (x + y * sx) as usize;
                if let Some(z) = (0..=top).rev().find(|&z| {
                    let m = world.material_at(x, y, z);
                    !is_passable(m) && !is_wood(m)
                }) {
                    water.ground[i] = (z + 1) as f32;
                    if water.surface[i] <= water.ground[i] {
                        water.surface[i] = 0.0;
                    }
                }
            }
        }
        for floor in &self.features.floors {
            for (x, y) in columns(floor.lo, floor.hi) {
                let i = (x + y * sx) as usize;
                water.ground[i] = floor.height * vpm;
                water.surface[i] = if floor.water > floor.height { floor.water * vpm } else { 0.0 };
            }
        }
    }


    fn build_from(&self, heights: &Heights, origin_z: i64, size_z: u32) -> World {
        let vpm = heights.voxels_per_meter;
        let [bx, by] = heights.size.map(|n| n / 8);
        let bz = size_z as usize / 8;
        let origin = [heights.origin[0], heights.origin[1], origin_z];

        // The objects reaching into this box, and for each column of bricks which of them reach
        // into it. Shapes have a say up to a voxel past them, see build_column(), so those ending
        // just short of the box too, as they'd be found for a bigger box with this in it.
        let lo = [(origin[0] - 2) as f32 / vpm, (origin[1] - 2) as f32 / vpm];
        let hi = [
            (origin[0] + heights.size[0] as i64 + 2) as f32 / vpm,
            (origin[1] + heights.size[1] as i64 + 2) as f32 / vpm,
        ];
        let objects = self.objects_in(lo, hi);
        let mut objects_by_column = vec![Vec::new(); bx * by];
        let to_brick = |v: f32, axis: usize| ((v * vpm).floor() as i64 - origin[axis]).div_euclid(8);
        for (i, object) in objects.iter().enumerate() {
            let x_range = to_brick(object.min[0], 0).max(0)..=to_brick(object.max[0], 0).min(bx as i64 - 1);
            for cy in to_brick(object.min[1], 1).max(0)..=to_brick(object.max[1], 1).min(by as i64 - 1) {
                for cx in x_range.clone() {
                    objects_by_column[cx as usize + cy as usize * bx].push(i as u32);
                }
            }
        }

        // Columns of bricks don't depend on each other, so rows of them are split across threads
        let threads = threads();
        let rows_per_thread = by.div_ceil(threads).max(1);
        let results: Vec<(Vec<(usize, u32)>, Vec<u32>)> = thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    let (objects, objects_by_column) = (&objects, &objects_by_column);
                    s.spawn(move || {
                        let mut cells = Vec::new();
                        let mut bricks = Vec::new();
                        for cy in (t * rows_per_thread)..((t + 1) * rows_per_thread).min(by) {
                            give_way();
                            for cx in 0..bx {
                                let column_objects: Vec<&Object> =
                                    objects_by_column[cx + cy * bx].iter().map(|&i| objects[i as usize]).collect();
                                self.build_column(heights, [cx, cy, bz], origin_z, &column_objects, &mut cells, &mut bricks);
                            }
                        }
                        (cells, bricks)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });

        let mut brick_grid = vec![0; bx * by * bz];
        let mut bricks = Vec::with_capacity(results.iter().map(|(_, b)| b.len()).sum());
        for (cells, thread_bricks) in results {
            // Stored brick indices are local to each thread until now
            let offset = (bricks.len() / BRICK_WORDS) as u32;
            for (index, cell) in cells {
                brick_grid[index] = if cell & SOLID_BRICK == 0 { cell + offset } else { cell };
            }
            bricks.extend_from_slice(&thread_bricks);
        }

        let size = [heights.size[0] as u32, heights.size[1] as u32, size_z];
        let mut world = World::new(size, origin, brick_grid, bricks, vpm as u32);
        // The top face of the highest water voxels
        world.sea_level = Some((sea_level_m(&self.settings) * vpm).floor() + 1.0);
        world
    }

    /// Where grass grows, see World::grass, and the ground and the water lying on it for every
    /// column, see World::water. Both in one go, as working out a column takes a while. Grass grows
    /// on grassy ground, a little thinner in the forests.
    fn grass_and_water(&self, heights: &Heights) -> (Vec<u32>, WaterMap) {
        let [sx, sy] = heights.size;
        let vpm = heights.voxels_per_meter;
        let seed = self.settings.seed;
        let mut map = vec![0u32; sx * sy];
        let mut ground = vec![0.0f32; sx * sy];
        let mut surface = vec![0.0f32; sx * sy];
        // None where the ground's built over or cut away: every column of every such rectangle
        // marked once, rather than each column looking through all of them
        let mut holes = vec![false; sx * sy];
        for h in &self.features.holes {
            let column = |m: f32, axis: usize, size: usize| ((m * vpm).floor() as i64 - heights.origin[axis]).clamp(0, size as i64) as usize;
            let (x0, x1) = (column(h[0], 0, sx), column(h[2], 0, sx));
            for y in column(h[1], 1, sy)..column(h[3], 1, sy) {
                holes[y * sx + x0..y * sx + x1].fill(true);
            }
        }
        let holes = &holes;

        // Rows split across threads
        let rows_per_thread = sy.div_ceil(threads()).max(1);
        let rows = rows_per_thread * sx;
        thread::scope(|s| {
            for (chunk_index, ((chunk, ground), surface)) in map.chunks_mut(rows).zip(ground.chunks_mut(rows)).zip(surface.chunks_mut(rows)).enumerate() {
                s.spawn(move || {
                    give_way();
                    for i in 0..chunk.len() {
                        let j = chunk_index * rows + i;
                        let (x, y) = ((j % sx) as i64, (j / sx) as i64);
                        let column = self.column(heights, x, y);
                        ground[i] = (column.height + 1) as f32;
                        surface[i] = if column.water_top > column.height { (column.water_top + 1) as f32 } else { 0.0 };
                        if holes[j] || !is_grassy(column.top) {
                            continue;
                        }
                        // Where it is in the whole landscape, so it's the same whichever piece it's in
                        let (gx, gy) = (heights.origin[0] + x, heights.origin[1] + y);
                        let forest = smoothstep(-0.15, 0.25, fbm(gx as f32 / vpm / 80.0, gy as f32 / vpm / 80.0, 3, seed.wrapping_add(4)));
                        let density = (255.0 * (1.0 - 0.45 * forest)) as u32;
                        let top = ((column.height + 1) as u32 * 4).min(0xFFFF);
                        chunk[i] = top | density << 16;
                    }
                });
            }
        });

        (map, WaterMap { ground, surface })
    }

    fn objects_in(&self, lo: [f32; 2], hi: [f32; 2]) -> Vec<&Object> {
        let cells = self.cells_across;
        let cell = |v: f32, axis: usize| (((v - self.lo[axis]) / OBJECT_CELL).floor().max(0.0) as usize).min(cells[axis] - 1);
        let mut indices: Vec<u32> = Vec::new();
        for cy in cell(lo[1], 1)..=cell(hi[1], 1) {
            for cx in cell(lo[0], 0)..=cell(hi[0], 0) {
                indices.extend_from_slice(&self.object_cells[cx + cy * cells[0]]);
            }
        }
        indices.sort_unstable();
        indices.dedup();
        indices
            .into_iter()
            .map(|i| &self.objects[i as usize])
            .filter(|o| o.max[0] >= lo[0] && o.min[0] <= hi[0] && o.max[1] >= lo[1] && o.min[1] <= hi[1])
            .collect()
    }

    fn column(&self, heights: &Heights, x: i64, y: i64) -> Column {
        let vpm = heights.voxels_per_meter;
        let v = vpm as i32;
        let to_voxels = |m: f32| ((m * vpm) as i32).clamp(1, (self.settings.height_m as f32 * vpm) as i32 - 2);
        let h = to_voxels(heights.ground_m(x, y));
        let slope = (to_voxels(heights.ground_m(x + 1, y)) - to_voxels(heights.ground_m(x - 1, y)))
            .abs()
            .max((to_voxels(heights.ground_m(x, y + 1)) - to_voxels(heights.ground_m(x, y - 1))).abs()) as f32
            / 2.0;
        let (gx, gy) = (heights.origin[0] + x, heights.origin[1] + y);
        let snow_jitter = (hash3(gx as u32, gy as u32, self.settings.seed) % (2 * v as u32 + 1)) as i32 - v;

        let sea_level = (sea_level_m(&self.settings) * vpm) as i32;
        let snow_line = (self.settings.height_m as f32 * 0.7 * vpm) as i32;
        let river = (heights.river_m(x, y) * vpm).floor() as i32;
        let water_top = if river > h { river.max(sea_level) } else { sea_level };
        // In the frozen realm the sea, the lakes and the rivers are iced over: the ground's ice up
        // to where the water would be, and no water flows there, see Terrain::piece_springs()
        let land = land_of(self.settings.seed);
        let frozen = land == Land::Ice && water_top > h;
        // And in the Cinder Crown they're lava, molten and glowing
        let molten = land == Land::Dragon && water_top > h;
        let (bed, h) = (h, if frozen || molten { water_top } else { h });
        // In the sky isles, the open air under an island, and everywhere between them; nor is
        // there any water, but for the sea far below, seen only past the islands
        let (under, water_top) = match land {
            Land::Sky => match sky_island(self.settings.seed, gx as f32 / vpm, gy as f32 / vpm) {
                Some((_, under)) => ((under * vpm) as i32, i32::MIN),
                None => (i32::MAX, i32::MIN),
            },
            Land::Ice | Land::Dragon => (0, i32::MIN),
            _ => (0, water_top),
        };

        // Which kind of land this is, with ragged edges where one meets the next
        let (xm, ym) = (gx as f32 / vpm, gy as f32 / vpm);
        let c = climate(&self.settings, xm, ym);
        let edge = (unit(hash3(gx as u32, gy as u32, self.settings.seed ^ 0xb10e)) - 0.5) * 0.3;
        let (jungle, desert, snowy) = (c.jungle + edge > 0.5, c.desert + edge > 0.5, c.snow + edge > 0.5);

        let top = if frozen {
            ICE
        } else if molten {
            // Dull red where it's shallow, cooling at its edges, glowing hotter the deeper it is,
            // with veins of brighter fire wandering through it, and here and there a plate of
            // black crust afloat on the cooler shallows
            let deep = (h - bed) as f32 / vpm;
            let jitter = (unit(hash3(gx as u32, gy as u32, self.settings.seed ^ 0x1a7a)) - 0.5) * 0.3;
            let hot = smoothstep(0.4, 2.4, deep) + jitter > 0.5;
            let vein = fbm(xm / 4.0, ym / 4.0, 2, self.settings.seed.wrapping_add(41)).abs() < 0.035;
            let crust = !hot && fbm(xm / 2.5, ym / 2.5, 2, self.settings.seed.wrapping_add(43)) > 0.32;
            if crust && !vein {
                OBSIDIAN
            } else {
                [GLOW_BLOOD, GLOW_FIRE, GLOW_HOLY][hot as usize + vein as usize]
            }
        } else if land == Land::Dragon {
            // Black glass where lava's cooled at its edges, bare rock up the steeps, ash the rest,
            // scorched darker in patches and strewn with grey cinders, now and then of black glass
            let cinder = unit(hash3(gx as u32, gy as u32, self.settings.seed ^ 0xc1d3));
            if h <= water_top.max(sea_level) + 2 * v || cinder < 0.012 {
                OBSIDIAN
            } else if cinder < 0.05 {
                MORTAR
            } else if slope > 1.4 {
                ROCK
            } else if fbm(xm / 11.0, ym / 11.0, 2, self.settings.seed.wrapping_add(44)) > 0.18 {
                DIRT
            } else {
                GRAVEL
            }
        } else if land == Land::Ice {
            // Snow down to the ice's edge, but for bare rock where it's too steep to lie
            if slope > 1.4 { STONE } else { SNOW }
        } else if h <= sea_level + v {
            SAND
        } else if river > h {
            GRAVEL
        } else if slope > 1.4 {
            STONE
        } else if h >= snow_line + snow_jitter || snowy {
            SNOW
        } else if desert {
            SAND
        } else if jungle {
            JUNGLE_GRASS
        } else {
            GRASS
        };
        // Clay in patches in the shallows at the edges of rivers, and along the shores of lakes and
        // the sea, just above the water and under it: for building with, see src\bases.rs
        let wet_edge = land != Land::Dragon && (river > h && river - h < v / 2) || (top == SAND && !desert && (h - sea_level).abs() <= v / 2);
        let top = if wet_edge && fbm(xm / 2.5, ym / 2.5, 2, self.settings.seed.wrapping_add(31)) > 0.22 { CLAY } else { top };
        let below = match top {
            SAND | STONE | GRAVEL | CLAY | ICE | ROCK | OBSIDIAN => top,
            GLOW_BLOOD | GLOW_FIRE | GLOW_HOLY => OBSIDIAN,
            _ => DIRT,
        };
        // The Drowned Ring's own stone, mosaic and rubble, laid in courses all the way down
        let built = self.arena.and_then(|a| a.ground_material(self.settings.seed, xm, ym));
        let (top, below) = built.map_or((top, below), |b| (b, BRICK_STONE));

        // Flowers on short stems, growing in patches of one color, and tufts of grass a few
        // voxels tall between them, taller in the jungle and dry in the desert. There's also grass
        // made of triangles in the shader, see traceGrass() in shaders\traverse.comp.
        let max_stem = (0.35 * vpm).round().max(1.0) as u32;
        let (mut blade, mut blade_body, mut blade_top) = (0, GRASS_BLADE, AIR);
        let tuft = hash3(gx as u32, gy as u32, self.settings.seed ^ 0x9a55);
        if is_grassy(top) {
            let r = hash3(gx as u32, gy as u32, self.settings.seed ^ 0xb1ade);
            let meadow = (fbm(xm / 14.0, ym / 14.0, 2, self.settings.seed.wrapping_add(7)) - 0.1).max(0.0) * 3.0;
            // About the same number of flowers per square meter at any resolution
            let flower_chance = 0.45 * (0.01 + meadow * 0.2) * (8.0 / vpm).powi(2).min(1.0) * if jungle { 0.3 } else { 1.0 };
            let lush = smoothstep(-0.3, 0.4, fbm(xm / 9.0, ym / 9.0, 2, self.settings.seed.wrapping_add(8)));
            let (lush, chance) = if jungle { (1.0, 0.8) } else { (lush, 0.4 + 0.4 * lush) };
            if unit(r) < flower_chance {
                blade = 1 + ((r >> 16) % max_stem) as i32;
                let patch = hash3((xm / 4.0) as u32, (ym / 4.0) as u32, self.settings.seed ^ 0xf10e);
                blade_top = [FLOWER_RED, FLOWER_YELLOW, FLOWER_PURPLE, FLOWER_WHITE][(patch % 4) as usize];
            } else if self.settings.grass_tufts && unit(tuft) < chance {
                // Taller in lush patches, with a lighter tip
                let tallest = (0.1 + if jungle { 0.4 } else { 0.3 } * lush) * vpm;
                blade = ((unit(tuft >> 16) * tallest).round() as i32).max(1);
                blade_top = GRASS_TIP;
            }
        } else if top == SAND && desert && self.settings.grass_tufts && unit(tuft) < 0.04 {
            blade = ((unit(tuft >> 16) * 0.25 * vpm).round() as i32).max(1);
            (blade_body, blade_top) = (DRY_GRASS, DRY_GRASS);
        }

        let strata = (fbm(xm / 7.0, ym / 7.0, 2, self.settings.seed.wrapping_add(23)) * 0.8 * vpm) as i32;
        // None in the desert, nor under the snow
        let bare = desert || snowy || h >= snow_line + snow_jitter || land == Land::Dragon;
        let moss = smoothstep(0.0, 0.5, fbm(xm / 3.0, ym / 3.0, 2, self.settings.seed.wrapping_add(24))) * if bare { 0.0 } else { 0.6 };

        Column {
            height: h,
            top,
            below,
            water_top,
            strata,
            moss,
            desert,
            x: gx,
            y: gy,
            blade,
            blade_body,
            blade_top,
            under,
            built: built.is_some(),
        }
    }

    /// The Drowned Ring's masonry `z` voxels up in `column`, under its top: courses half a meter
    /// high of blocks a meter long, mortar between them, the blocks of one course over the joints of
    /// the one below, some of them gone green with the sea, and moss where it laps
    fn masonry(&self, column: &Column, z: i32) -> u8 {
        let vpm = self.settings.voxels_per_meter.max(2) as i64;
        let course = (vpm / 2).max(2) as i32;
        let row = z.div_euclid(course);
        let off = if row % 2 == 0 { 0 } else { vpm / 2 };
        let (bx, by) = (column.x + off, column.y + off);
        if z.rem_euclid(course) == 0 || bx.rem_euclid(vpm) == 0 || by.rem_euclid(vpm) == 0 {
            return MORTAR;
        }
        let block = hash3(bx.div_euclid(vpm) as u32, by.div_euclid(vpm) as u32, row as u32 ^ self.settings.seed);
        if (z - column.water_top).abs() < course && unit(block >> 8) < 0.5 {
            return MOSS;
        }
        [BRICK_STONE, BRICK_STONE, BRICK_STONE, LIMESTONE, LIMESTONE, TEMPLE_TEAL][(block % 6) as usize]
    }

    /// Bare rock: layers a few tens of centimeters thick, gently wavy, of a few kinds of stone,
    /// with moss growing in patches, thick over the tops of ledges, `depth` voxels down from the
    /// top of the ground, and in flecks below. Sandstone in the deserts.
    fn rock(&self, column: &Column, z: i32, depth: i32) -> u8 {
        let thickness = (0.3 * self.settings.voxels_per_meter as f32).max(1.0) as i32;
        let layer = (z + column.strata).div_euclid(thickness);
        let h = hash3(column.x as u32, column.y as u32, z as u32 ^ self.settings.seed);
        if depth <= 1 && column.top == STONE && column.moss > 0.22 {
            return if depth == 0 && unit(h >> 16) < 0.5 { JUNGLE_GRASS } else { MOSS };
        }
        if unit(h) < column.moss * 0.5 {
            return MOSS;
        }
        let pick = hash3(layer as u32, self.settings.seed, 0x57a7a) % 6;
        if land_of(self.settings.seed) == Land::Dragon {
            return [OBSIDIAN, ROCK, ROCK, STONE, STONE, GRAVEL][pick as usize];
        }
        if column.desert {
            return [SANDSTONE, SANDSTONE, SANDSTONE, SANDSTONE_DARK, SAND, CLAY][pick as usize];
        }
        match pick {
            0 | 1 | 2 => STONE,
            3 | 4 => ROCK,
            _ => CLAY,
        }
    }

    fn material(&self, column: &Column, z: i32, dirt_depth: i32) -> u8 {
        if z < column.under {
            AIR
        } else if z <= column.height {
            let depth = column.height - z;
            if column.built && depth > 0 {
                self.masonry(column, z)
            } else if depth == 0 && column.top == STONE || depth > 0 && column.below == STONE || depth > dirt_depth {
                self.rock(column, z, depth)
            } else if depth == 0 {
                column.top
            } else {
                column.below
            }
        } else if z <= column.water_top {
            // Water isn't kept in the voxels, see src\water.rs
            AIR
        } else if z < column.height + column.blade {
            column.blade_body
        } else if z == column.height + column.blade {
            column.blade_top
        } else {
            AIR
        }
    }

    fn build_column(
        &self,
        heights: &Heights,
        [cx, cy, bz]: [usize; 3],
        origin_z: i64,
        objects: &[&Object],
        cells: &mut Vec<(usize, u32)>,
        bricks: &mut Vec<u32>,
    ) {
        let vpm = heights.voxels_per_meter;
        let voxel = 1.0 / vpm;
        let [bx, by] = heights.size.map(|n| n / 8);
        let dirt_depth = (0.6 * vpm).round().max(3.0) as i32;

        let mut columns = [Column::default(); 64];
        let (mut min_height, mut max_top) = (i32::MAX, i32::MIN);
        for ly in 0..8 {
            for lx in 0..8 {
                let column = self.column(heights, (cx * 8 + lx) as i64, (cy * 8 + ly) as i64);
                min_height = min_height.min(column.height);
                max_top = max_top.max(column.height + column.blade);
                columns[lx + ly * 8] = column;
            }
        }
        let highest_ground = columns.iter().map(|c| c.height).max().unwrap_or(i32::MAX);

        // The shapes reaching into this column of bricks, along with their object's seed
        let column_lo = [(heights.origin[0] + cx as i64 * 8) as f32 * voxel - voxel, (heights.origin[1] + cy as i64 * 8) as f32 * voxel - voxel];
        let column_hi = [column_lo[0] + 10.0 * voxel, column_lo[1] + 10.0 * voxel];
        let mut shapes: Vec<(&Shape, u32)> = Vec::new();
        // The lowest anything cuts into the ground
        let mut lowest_cut = i32::MAX;
        for object in objects {
            for shape in &object.shapes {
                if shape.max[0] >= column_lo[0] && shape.min[0] <= column_hi[0] && shape.max[1] >= column_lo[1] && shape.min[1] <= column_hi[1] {
                    shapes.push((shape, object.seed));
                    max_top = max_top.max((shape.max[2] * vpm).ceil() as i32);
                    if shape.placement != Placement::Above {
                        lowest_cut = lowest_cut.min((shape.min[2] * vpm).floor() as i32 - 1);
                    }
                }
            }
        }

        // Below this every voxel in the column is stone
        let stone_below = (min_height - dirt_depth).min(lowest_cut);
        // Under the floating islands of the sky isles, nothing below the lowest of their
        // undersides, and only above the highest is there stone all through
        let open_below = columns.iter().map(|c| c.under).min().unwrap_or(0);
        let solid_from = columns.iter().map(|c| c.under).max().unwrap_or(0);

        let mut materials = [AIR; BRICK_VOXELS];
        let mut nearby: Vec<(&Shape, u32)> = Vec::new();
        // Columns whose top of the ground was cut away, so the grass and flowers on it go too
        let mut bare = [false; 64];
        for z in 0..bz {
            let z0 = origin_z as i32 + (z * 8) as i32;
            if z0 > max_top {
                break;
            }
            let index = cx + (cy + z * by) * bx;
            if z0 + 7 < open_below {
                continue;
            }
            if z0 + 7 < stone_below && z0 >= solid_from {
                cells.push((index, SOLID_BRICK | STONE as u32));
                continue;
            }
            // Trees, plants, rocks and buildings above the ground, and caves and rooms cut into it
            let (z_lo, z_hi) = ((z0 - 1) as f32 * voxel, (z0 + 9) as f32 * voxel);
            nearby.clear();
            nearby.extend(shapes.iter().filter(|(s, _)| s.max[2] >= z_lo && s.min[2] <= z_hi));
            // Deep in the rock and nowhere near anything cut out of it, however deep what's cut out
            // of the ground nearby goes: plain stone, as nothing can see it, rather than layers of
            // rock that each take a brick of their own
            let middle = [(heights.origin[0] + (cx * 8) as i64) as f32 + 4.0, (heights.origin[1] + (cy * 8) as i64) as f32 + 4.0, z0 as f32 + 4.0].map(|v| v * voxel);
            let deep = z0 + 7 < min_height - dirt_depth && z0 >= solid_from;
            if deep && nearby.iter().all(|(s, _)| s.clear_of(middle, 8.5 * voxel)) {
                cells.push((index, SOLID_BRICK | STONE as u32));
                continue;
            }

            for lz in 0..8 {
                for ly in 0..8 {
                    for lx in 0..8 {
                        materials[lx + ly * 8 + lz * 64] = self.material(&columns[lx + ly * 8], z0 + lz as i32, dirt_depth);
                    }
                }
            }

            let cuts = nearby.iter().any(|(s, _)| s.placement != Placement::Above);
            // A brick wholly inside solid stone all of one kind, like that inside a castle's walls,
            // needn't be looked at voxel by voxel
            let uniform = uniform_brick(&nearby, [heights.origin[0] + (cx * 8) as i64, heights.origin[1] + (cy * 8) as i64, z0 as i64], voxel, z0 > highest_ground);
            if let Some(m) = uniform {
                materials.fill(m);
            } else if !nearby.is_empty() {
                let x0 = heights.origin[0] + (cx * 8) as i64;
                let y0 = heights.origin[1] + (cy * 8) as i64;
                for lz in 0..8 {
                    for ly in 0..8 {
                        for lx in 0..8 {
                            let (x, y, z) = (x0 + lx as i64, y0 + ly as i64, z0 + lz as i32);
                            let ground = z <= columns[lx + ly * 8].height;
                            if ground && !cuts {
                                continue;
                            }
                            let p = [(x as f32 + 0.5) * voxel, (y as f32 + 0.5) * voxel, (z as f32 + 0.5) * voxel];
                            let mut carved = false;
                            for &(shape, seed) in &nearby {
                                let applies = match shape.placement {
                                    Placement::Above => !ground,
                                    Placement::Clear => true,
                                    Placement::Carve => ground,
                                    Placement::Inside => carved,
                                };
                                if !applies || (0..3).any(|i| p[i] < shape.min[i] - voxel || p[i] > shape.max[i] + voxel) {
                                    continue;
                                }
                                let depth = shape.depth(p, voxel);
                                if !shape.holds(p, depth) {
                                    continue;
                                }
                                let i = lx + ly * 8 + lz * 64;
                                if shape.placement == Placement::Carve {
                                    // Things can still be inside the hollow, like crystals
                                    materials[i] = AIR;
                                    carved = true;
                                    continue;
                                }
                                let h = hash3(x as u32, y as u32, z as u32 ^ seed);
                                // Ragged, see-through edges so clumps of leaves aren't perfect balls
                                if shape.ragged && depth > 0.55 && unit(h) < 0.45 {
                                    continue;
                                }
                                materials[i] = shape.material_at(p, voxel, h, (columns[lx + ly * 8].height + 1) as f32 * voxel);
                                break;
                            }
                        }
                    }
                }
            }

            // Nothing grows in mid-air over a hole cut into the ground
            for (c, column) in columns.iter().enumerate() {
                let top = column.height - z0;
                if (0..8).contains(&top) && materials[c + top as usize * 64] == AIR {
                    bare[c] = true;
                }
                if !bare[c] || column.blade == 0 {
                    continue;
                }
                for lz in (top + 1).max(0)..(top + column.blade + 1).min(8) {
                    let i = c + lz as usize * 64;
                    if materials[i] == column.blade_body || materials[i] == column.blade_top {
                        materials[i] = AIR;
                    }
                }
            }

            let cell = push_brick(&materials, bricks);
            if cell != 0 {
                cells.push((index, cell));
            }
        }
    }

    /// The rivers running through this piece of the landscape, from springs near enough for their
    /// rivers to reach it
    fn rivers_in(&self, cache: &RiverCache) -> Vec<Arc<Vec<RiverPoint>>> {
        self.rivers_near(cache, self.lo.map(|v| v - RIVER_MARGIN_M), self.hi.map(|v| v + RIVER_MARGIN_M))
    }

    /// The rivers coming near the rectangle from `lo` to `hi`, in meters, wherever it is
    fn rivers_near(&self, cache: &RiverCache, lo: [f32; 2], hi: [f32; 2]) -> Vec<Arc<Vec<RiverPoint>>> {
        // No river runs off a floating island
        if land_of(self.settings.seed) == Land::Sky {
            return Vec::new();
        }
        let reach = RIVER_STEPS as f32 * 0.5;
        let cell = |v: f32| (v / RIVER_CELL).floor() as i32;
        let mut rivers = Vec::new();
        for cy in cell(lo[1] - reach)..=cell(hi[1] + reach) {
            for cx in cell(lo[0] - reach)..=cell(hi[0] + reach) {
                let cached = cache.cells.lock().unwrap().get(&(cx, cy)).cloned();
                let river = match cached {
                    Some(river) => river,
                    None => {
                        let river = self.river_from(cx, cy).map(Arc::new);
                        cache.cells.lock().unwrap().insert((cx, cy), river.clone());
                        river
                    }
                };
                // Only those that come near, banks and all
                if let Some(river) = river
                    && river.iter().any(|p| {
                        let margin = p.half_width + 5.0;
                        p.x > lo[0] - margin && p.y > lo[1] - margin && p.x < hi[0] + margin && p.y < hi[1] + margin
                    })
                {
                    rivers.push(river);
                }
            }
        }
        rivers
    }

    /// The river starting in one RIVER_CELL square, if there's a spring there: at the highest of a
    /// few random spots in it, if that's high enough above the sea
    fn river_from(&self, cx: i32, cy: i32) -> Option<Vec<RiverPoint>> {
        let seed = self.settings.seed;
        let r = hash3(cx as u32, cy as u32, seed ^ 0x5157);
        if unit(r) > 0.7 {
            return None;
        }
        let (h, x, y) = (0..12u32)
            .map(|i| {
                let s = hash3(r, i, 0x591);
                let x = (cx as f32 + 0.1 + 0.8 * unit(s)) * RIVER_CELL;
                let y = (cy as f32 + 0.1 + 0.8 * unit(s >> 16)) * RIVER_CELL;
                (height_at(&self.settings, x, y), x, y)
            })
            .max_by(|a, b| a.0.total_cmp(&b.0))?;
        // Most stay streams, some grow into great rivers
        let g = hash3(r, 0xb16, seed);
        let widens = if unit(g) < GREAT_RIVERS { GREAT_RIVER_WIDENS[0] + (GREAT_RIVER_WIDENS[1] - GREAT_RIVER_WIDENS[0]) * unit(hash(g)) } else { 0.0 };
        (h > sea_level_m(&self.settings) + 10.0).then(|| self.trace_river(x, y, hash(r), widens))
    }

    /// The course of a river from its spring at `x`, `y` down to the sea, `widens` meters wider on
    /// each side once it's run far enough
    fn trace_river(&self, x: f32, y: f32, seed: u32, widens: f32) -> Vec<RiverPoint> {
        let sea_level = sea_level_m(&self.settings);
        let step = 0.5;
        let probe = 3.0;
        let height = |x: f32, y: f32| height_at(&self.settings, x, y);

        let (mut x, mut y) = (x, y);
        let mut dir = [0.0f32; 2];
        // Where the water would be if it followed the ground down
        let mut surface = height(x, y) - 0.3;
        // Where it is: flat pools, each one dropping into the next over a waterfall. Each drop cuts
        // a gorge into the ground below it, which the ground has to fall into before the next one.
        let mut level = surface;
        let mut points: Vec<RiverPoint> = Vec::new();
        // How many times it's filled a hollow and spilled out of it, and how much further it keeps
        // straight on through the hollow's rim
        let (mut spills, mut straight) = (0, 0);

        for i in 0..RIVER_STEPS {
            // Going round and round in a hollow: it fills up into a pond, and spills over the
            // lowest point of the hollow's rim, cutting through it. After a few, the river just
            // ends in its pond.
            if straight == 0 && i >= RIVER_STUCK_STEPS && i % 10 == 0 {
                let back = &points[i - RIVER_STUCK_STEPS];
                if (x - back.x).hypot(y - back.y) < RIVER_STUCK_STEPS as f32 * step * 0.4 {
                    if spills == RIVER_SPILLS {
                        break;
                    }
                    spills += 1;
                    let rim = (0..24)
                        .map(|k| k as f32 * TAU / 24.0)
                        .min_by(|&a, &b| height(x + RIVER_RIM * a.cos(), y + RIVER_RIM * a.sin()).total_cmp(&height(x + RIVER_RIM * b.cos(), y + RIVER_RIM * b.sin())))
                        .unwrap_or(0.0);
                    dir = [rim.cos(), rim.sin()];
                    straight = (RIVER_RIM / step) as usize;
                }
            }
            if straight > 0 {
                // Bending gently as it goes, so it isn't a canal
                straight -= 1;
                let (sin, cos) = (0.05 * fbm(x / 12.0, y / 12.0, 2, seed ^ 0x5b11)).sin_cos();
                dir = [dir[0] * cos - dir[1] * sin, dir[0] * sin + dir[1] * cos];
            } else {
                // Downhill, wandering a little so the river isn't straight,
                // with momentum so it doesn't zigzag and can cut through small bumps
                let gx = height(x + probe, y) - height(x - probe, y);
                let gy = height(x, y + probe) - height(x, y - probe);
                let wander = fbm(x / 25.0, y / 25.0, 2, seed);
                let (sin, cos) = wander.sin_cos();
                let downhill = [-gx * cos + gy * sin, -gx * sin - gy * cos];
                let length = downhill[0].hypot(downhill[1]);
                if length > 0.0 {
                    let w = if i == 0 { 1.0 } else { 0.2 };
                    dir = [dir[0] * (1.0 - w) + downhill[0] / length * w, dir[1] * (1.0 - w) + downhill[1] / length * w];
                }
                let length = dir[0].hypot(dir[1]);
                dir = if length > 0.0 { [dir[0] / length, dir[1] / length] } else { [1.0, 0.0] };
            }

            x += dir[0] * step;
            y += dir[1] * step;

            // Water never flows uphill
            let previous = level;
            surface = surface.min(height(x, y) - 0.3);
            if surface < level - 0.05 {
                level = (surface - FALL_HEIGHT).max(sea_level);
            }
            if (previous - level) / step > 0.3 {
                // A shallow lip at the edge, which holds the pool above it back
                let n = points.len();
                for p in &mut points[n.saturating_sub(3)..] {
                    p.lip = true;
                }
            }

            // Wider the further it has run, a great river much wider
            let run = i as f32 * step;
            let stream = 1.0 + (STREAM_HALF_WIDTH - 1.0) * (run / 60.0).min(1.0);
            let great = widens * smoothstep(0.0, 1.0, run / GREAT_RIVER_GROWS_M);
            points.push(RiverPoint {
                x,
                y,
                surface: level,
                half_width: stream + great,
                lip: false,
            });

            if surface <= sea_level {
                break;
            }
        }

        points
    }

    /// Trees, plants, rocks, buildings, dungeons and caves, anywhere in this piece of the landscape. Each is
    /// placed at a spot that only depends on where it is in the whole landscape, so pieces built
    /// around different places agree where they overlap.
    fn place_objects(&self, heights: &Heights, rivers: &RiverCache) -> (Vec<Object>, Features) {
        let seed = self.settings.seed;
        let vpm = heights.voxels_per_meter;
        let mut features = Features::default();
        let mut objects = Vec::new();
        // Nothing's built on the floating islands of the sky isles, nor cut into them: only what
        // grows there. No one lives in the dreaming realm: there are ruins and the dead there, but
        // no towns, castles or bandits.
        let land = land_of(seed);
        let peopled = !matches!(land, Land::Dream | Land::Water | Land::Dragon);
        if !matches!(land, Land::Sky | Land::Water | Land::Dragon) {
            // Buildings go first, as where they overlap other things they win. None stands over a
            // dungeon, whose halls go before the caves so no cave breaks into them.
            let dungeons = self.dungeons(rivers);
            // Towns keep clear of dungeons, rivers, ravines and caves themselves, see towns.rs, and
            // no ruins stand in them
            let (in_towns, towns) = if peopled { self.towns(rivers, &mut features) } else { (Vec::new(), Vec::new()) };
            objects.extend(in_towns);
            // Royal castles keep clear of all those and of caves themselves, see castles.rs
            let (castles, royal) = if peopled { self.castles(rivers, &mut features) } else { (Vec::new(), Vec::new()) };
            objects.extend(castles);
            // Fallen cities deep under the land keep clear of all those themselves, and no cave or
            // ravine breaks into them, see undercity.rs
            let (sunken, cities, shafts) = self.undercities(rivers, &mut features);
            objects.extend(sunken);
            let keep_out: Vec<[f32; 4]> = dungeons.iter().flat_map(|d| d.world_rects()).chain(towns).chain(royal.iter().copied()).chain(shafts).collect();
            objects.extend(self.buildings(heights, &mut features, &keep_out));
            for dungeon in &dungeons {
                objects.push(self.dungeon(dungeon, &mut features));
            }
            let shut: Vec<[f32; 4]> = royal.iter().chain(&cities).copied().collect();
            objects.extend(self.caves(&mut features, &royal, &cities));
            objects.extend(self.waterfall_caves(&mut features));
            objects.extend(self.ravines(&mut features, &shut));
            if peopled {
                let camps = self.camps(heights, &mut features, &keep_out, &objects);
                objects.extend(camps);
            }
        }
        // The Drowned Ring's pillars, arch and statues, where they reach this piece
        let arena = self.arena;
        if let Some(arena) = &arena {
            let (lo, hi) = (self.lo, self.hi);
            let near = |m: usize| arena.middle[m] + ARENA_REACH + arena.causeway > lo[m] && arena.middle[m] - ARENA_REACH - arena.causeway < hi[m];
            if near(0) && near(1) {
                objects.extend(arena_objects(&self.settings, arena).into_iter().filter(|o| o.max[0] >= lo[0] && o.min[0] <= hi[0] && o.max[1] >= lo[1] && o.min[1] <= hi[1]));
            }
        }
        let footprints = std::mem::take(&mut features.footprints);
        let (own_lo, own_hi) = (self.own_lo, self.own_hi);

        // Calls `place` for one random spot in every `cell` meter square, with the column there,
        // except where buildings stand: out to `reach` meters past the piece itself, as far as what
        // `place` makes reaches from its spot, so all of what reaches in is there. Each spot goes by
        // where it is in the whole landscape, so pieces built round different places agree.
        let mut scatter = |cell: f32, salt: u32, reach: f32, place: &mut dyn FnMut(f32, f32, f32, &Column, u32) -> Option<Object>| {
            let (lo, hi) = (own_lo.map(|v| v - reach), own_hi.map(|v| v + reach));
            let first = [(lo[0] / cell).floor() as i32, (lo[1] / cell).floor() as i32];
            let last = [(hi[0] / cell).ceil() as i32, (hi[1] / cell).ceil() as i32];
            for cy in first[1]..last[1] {
                for cx in first[0]..last[0] {
                    let ([x, y], r) = scatter_spot(seed, cell, salt, cx, cy);
                    if x < lo[0] || y < lo[1] || x >= hi[0] || y >= hi[1] {
                        continue;
                    }
                    if footprints.iter().any(|f| x > f[0] - 1.5 && y > f[1] - 1.5 && x < f[2] + 1.5 && y < f[3] + 1.5) {
                        continue;
                    }
                    // Nor on the Drowned Ring or its causeway, nor anywhere it'd reach over them
                    if arena.is_some_and(|a| a.keeps_clear(x, y, reach.min(8.0))) {
                        continue;
                    }
                    let column = self.column(heights, (x * vpm).floor() as i64 - heights.origin[0], (y * vpm).floor() as i64 - heights.origin[1]);
                    // Nothing grows in the open air between the sky isles
                    if column.under > column.height {
                        continue;
                    }
                    let ground = (column.height + 1) as f32 / vpm;
                    if let Some(object) = place(x, y, ground, &column, r) {
                        // Anything reaching further than `reach` would be missing where it does
                        #[cfg(test)]
                        assert!(
                            object.min[0] >= x - reach && object.min[1] >= y - reach && object.max[0] <= x + reach && object.max[1] <= y + reach,
                            "salt {salt:#x}: something at {x}, {y} reaches from {:?} to {:?}, past {reach} m",
                            object.min,
                            object.max
                        );
                        objects.push(object);
                    }
                }
            }
        };

        let forest = |x: f32, y: f32| smoothstep(-0.15, 0.25, fbm(x / 80.0, y / 80.0, 3, seed.wrapping_add(4)));
        let meadow = |x: f32, y: f32| ((fbm(x / 14.0, y / 14.0, 2, seed.wrapping_add(7)) - 0.1).max(0.0) * 3.0).min(1.0);
        let sea_level = sea_level_m(&self.settings);
        let settings = self.settings;
        let climate = |x: f32, y: f32| climate(&settings, x, y);

        // Whether a river runs within a few meters of a spot
        let by_river = |x: f32, y: f32| [[3.5, 0.0], [-3.5, 0.0], [0.0, 3.5], [0.0, -3.5]].iter().any(|d| heights.at_m(x + d[0], y + d[1]).1.is_finite());

        // Mossy boulders and outcrops of rock in and around the woods of the milder lands, before
        // the trees so where one stands in the way of a rock, the rock wins. Boulders keep clear of
        // tree trunks.
        let mild_woods = |x: f32, y: f32, ground: f32, column: &Column| {
            let c = climate(x, y);
            let mild = c.jungle.max(c.desert).max(c.snow) < 0.45;
            if mild && is_grassy(column.top) && ground - sea_level > 1.5 { forest(x, y) } else { 0.0 }
        };
        scatter(17.0, 0xb01d, 4.0, &mut |x, y, ground, column, r| {
            let woods = mild_woods(x, y, ground, column);
            (unit(r) < woods * 0.4 && !self.near_tree(x, y, 2.2)).then(|| self.mossy_rock(x, y, ground, false, 150 + (woods * 100.0) as u8, hash(r)))
        });
        scatter(40.0, 0x0c7c, 9.0, &mut |x, y, ground, column, r| {
            let woods = mild_woods(x, y, ground, column);
            (unit(r) < woods * 0.45).then(|| self.mossy_rock(x, y, ground, true, 180 + (woods * 70.0) as u8, hash(r)))
        });

        // At most one tree per cell, denser where the forest noise is high. The jungle is thick
        // with them, the snow has pine forests, and in the desert only palms grow, by the water.
        // Willows lean out over the rivers in the milder lands.
        scatter(trees::TREE_CELL, trees::TREE_SALT, 16.0, &mut |x, y, ground, column, r| {
            let c = climate(x, y);
            let density = if c.jungle > 0.5 {
                0.95
            } else if column.top == SAND && c.desert > 0.5 {
                if ground - sea_level < 2.5 { 0.25 } else { 0.0 }
            } else if column.top == SNOW || c.snow > 0.5 {
                forest(x, y) * 0.7 + 0.08
            } else {
                // A little sparser in the thick of the broad-leaved woods, where the crowns are big
                // enough to meet anyway
                (forest(x, y) * 0.85 + 0.03) * (1.0 - 0.34 * self.broadleaf_woods(x, y, ground))
            };
            let soil = is_grassy(column.top) || column.top == SNOW && c.snow > 0.3 || column.top == SAND && c.desert > 0.5;
            let mild = c.jungle.max(c.desert).max(c.snow) < 0.4;
            if soil && mild && column.top == GRASS && unit(hash(r ^ 0x5117)) < 0.45 && by_river(x, y) {
                let mut rng = Rng(hash(r ^ 0x3171));
                return Some(Object::new(self.willow(x, y, ground, &mut rng), rng.0));
            }
            (soil && unit(r) < density).then(|| self.wild_tree(x, y, ground, hash(r)))
        });

        // In the jungle, smaller trees in between the big ones
        scatter(4.5, 0x9b1e, 5.0, &mut |x, y, ground, column, r| {
            let c = climate(x, y);
            (column.top == JUNGLE_GRASS && unit(r) < c.jungle * 0.7).then(|| {
                let mut rng = Rng(hash(r));
                Object::new(self.jungle_sapling(x, y, ground, &mut rng), rng.0)
            })
        });

        // Cacti and dry shrubs in the desert
        scatter(6.0, 0xcac7, 4.0, &mut |x, y, ground, column, r| {
            let c = climate(x, y);
            if column.top != SAND || c.desert < 0.5 || ground - sea_level < 2.0 {
                return None;
            }
            let mut rng = Rng(hash(r));
            let roll = unit(r);
            if roll < 0.25 {
                Some(Object::new(self.cactus(x, y, ground, &mut rng), rng.0))
            } else if roll < 0.4 {
                Some(Object::new(self.lupine(x, y, ground, FLOWER_PURPLE, &mut rng), rng.0))
            } else if roll < 0.5 {
                Some(Object::new(self.broadleaf(x, y, ground, 0.35, 2, DEAD_WOOD, BARE, &mut rng), rng.0))
            } else {
                None
            }
        });

        // Bushes, mostly around the forests, and in the thick of the broad-leaved woods a dense
        // undergrowth of them
        scatter(4.0, 0xb054, 4.0, &mut |x, y, ground, column, r| {
            if !is_grassy(column.top) {
                return None;
            }
            let woods = self.broadleaf_woods(x, y, ground);
            let density = smoothstep(-0.3, 0.2, forest(x, y) * 2.0 - 1.0) * 0.38 + meadow(x, y) * 0.08 + 0.04 + woods * 0.4;
            (unit(r) < density).then(|| self.bush(x, y, ground, meadow(x, y), woods, hash(r)))
        });

        // Fallen logs and old stumps in the woods, out of the jungle and the desert
        scatter(6.0, 0xd0a7, 5.0, &mut |x, y, ground, column, r| {
            let c = climate(x, y);
            let soil = column.top == GRASS || column.top == SNOW && c.snow > 0.3;
            if !soil || c.jungle > 0.4 || c.desert > 0.4 || ground - sea_level < 1.0 || unit(r) >= forest(x, y) * 0.2 {
                return None;
            }
            self.deadwood(x, y, ground, hash(r))
        });

        // Undergrowth: ferns, big leaves and mushrooms in the forests and all over the jungle,
        // flowers in the meadows, reeds along the shore
        scatter(1.2, 0x9a47, 2.0, &mut |x, y, ground, column, r| {
            if !is_grassy(column.top) {
                return None;
            }
            let jungle = climate(x, y).jungle;
            let (forest, meadow) = (forest(x, y).max(jungle * 1.6), meadow(x, y) * (1.0 - jungle));
            let shore = if ground - sea_level < 1.5 { 0.5 } else { 0.0 };
            let weights = [
                forest * 0.25,
                forest * 0.08 + jungle * 0.15,
                forest * 0.03,
                meadow * 0.15,
                shore,
            ];
            let mut roll = unit(r);
            let kind = weights.iter().position(|&w| {
                roll -= w;
                roll < 0.0
            })?;
            let mut rng = Rng(hash(r));
            let shapes = match kind {
                0 => self.fern(x, y, ground, &mut rng),
                1 => self.big_leaves(x, y, ground, &mut rng),
                2 => self.mushrooms(x, y, ground, &mut rng),
                3 => {
                    let patch = hash3((x / 6.0) as u32, (y / 6.0) as u32, seed ^ 0x1b9e);
                    let color = [FLOWER_BLUE, FLOWER_PURPLE, FLOWER_PINK][(patch % 3) as usize];
                    self.lupine(x, y, ground, color, &mut rng)
                }
                _ => self.reeds(x, y, ground, &mut rng),
            };
            Some(Object::new(shapes, rng.0))
        });

        // The dreaming realm's own: in its soft half mushrooms as tall as trees, their caps broad
        // enough to stand on, and orbs of crystal hanging in the air over the meadows; in its
        // bleeding half thorns of bone jutting from the torn ground
        if land == Land::Dream {
            scatter(30.0, 0xd4ea, 9.0, &mut |x, y, ground, _, r| {
                (dream_blood(x, y) < 0.2 && ground > sea_level + 1.0 && unit(r) < 0.5).then(|| {
                    let mut rng = Rng(hash(r));
                    Object::new(self.giant_mushroom(x, y, ground, &mut rng), rng.0)
                })
            });
            scatter(26.0, 0xd4eb, 4.0, &mut |x, y, ground, _, r| {
                (dream_blood(x, y) < 0.4 && unit(r) < 0.35).then(|| {
                    let mut rng = Rng(hash(r));
                    let size = rng.range(1.2, 2.8);
                    let up = ground.max(sea_level) + rng.range(9.0, 16.0);
                    Object::new(vec![Shape::sphere([x, y, up], size, CRYSTAL, CRYSTAL, false)], rng.0)
                })
            });
            scatter(12.0, 0xb100, 5.0, &mut |x, y, ground, _, r| {
                (dream_blood(x, y) > 0.7 && ground > sea_level + 0.5 && unit(r) < 0.55).then(|| {
                    let mut rng = Rng(hash(r));
                    let mut shapes = Vec::new();
                    for _ in 0..1 + (rng.next() * 2.99) as usize {
                        let foot = [x + rng.range(-1.5, 1.5), y + rng.range(-1.5, 1.5), ground - 0.5];
                        let tall = rng.range(3.0, 9.0);
                        let lean = [rng.range(-0.35, 0.35) * tall, rng.range(-0.35, 0.35) * tall, tall];
                        shapes.push(Shape::capsule(foot, add(foot, lean), rng.range(0.35, 0.7), 0.05, BONE, BONE, false));
                    }
                    Object::new(shapes, rng.0)
                })
            });
        }
        // The sunken reach's own: stacks of rock standing up out of the sea, the ruins of a city
        // drowned long ago under the water, to be seen down through it, and pillars of it fallen
        // across the shallows
        if land == Land::Water {
            scatter(58.0, 0x5eac, 9.0, &mut |x, y, ground, _, r| {
                (ground < sea_level - 2.0 && unit(r) < 0.4).then(|| {
                    let mut rng = Rng(hash(r));
                    let tall = sea_level - ground + rng.range(5.0, 15.0);
                    let wide = rng.range(1.8, 3.4);
                    let top = [x + rng.range(-1.0, 1.0), y + rng.range(-1.0, 1.0), ground + tall];
                    let shapes = vec![
                        Shape::capsule([x, y, ground - 1.0], top, wide, wide * 0.55, ROCK, STONE, true),
                        Shape::sphere([top[0], top[1], top[2] + 0.2], wide * 0.5, GRASS, MOSS, true),
                    ];
                    Object::new(shapes, rng.0)
                })
            });
            scatter(34.0, 0x5e1c, 8.0, &mut |x, y, ground, _, r| {
                (ground < sea_level - 1.6 && ground > sea_level - 7.5 && unit(r) < 0.5).then(|| {
                    let mut rng = Rng(hash(r));
                    let mut shapes = Vec::new();
                    // A ring of columns, some broken off short, round a fallen one
                    let turn = rng.range(0.0, TAU);
                    let ring = rng.range(3.0, 5.0);
                    for k in 0..6 {
                        let a = turn + k as f32 * TAU / 6.0;
                        let foot = [x + a.cos() * ring, y + a.sin() * ring, ground - 0.4];
                        let tall = if rng.next() < 0.4 { rng.range(0.6, 1.6) } else { rng.range(2.5, 4.5) };
                        shapes.push(Shape::capsule(foot, add(foot, [0.0, 0.0, tall]), 0.42, 0.38, LIMESTONE, TEMPLE_STONE, false));
                    }
                    let lie = [turn.cos() * 2.5, turn.sin() * 2.5, 0.0];
                    shapes.push(Shape::capsule([x - lie[0], y - lie[1], ground + 0.2], [x + lie[0], y + lie[1], ground + 0.3], 0.45, 0.45, LIMESTONE, TEMPLE_STONE, false));
                    Object::new(shapes, rng.0)
                })
            });
            scatter(46.0, 0xfa11, 7.0, &mut |x, y, ground, _, r| {
                (ground < sea_level - 0.4 && ground > sea_level - 3.5 && unit(r) < 0.35).then(|| {
                    let mut rng = Rng(hash(r));
                    let a = rng.range(0.0, TAU);
                    let half = rng.range(4.0, 6.0);
                    let (dx, dy) = (a.cos() * half, a.sin() * half);
                    let z = sea_level + 0.15;
                    Object::new(vec![Shape::capsule([x - dx, y - dy, z], [x + dx, y + dy, z], 0.65, 0.6, LIMESTONE, TEMPLE_STONE, false)], rng.0)
                })
            });
        }
        // The Cinder Crown's own: the bones of great dragons lying out on the ash, spikes of black
        // glass jutting from it, and on the peaks the dragons' roosts, rings of bones and dead wood
        // with eggs in them
        if land == Land::Dragon {
            scatter(80.0, 0xb07e, 14.0, &mut |x, y, ground, column, r| {
                (matches!(column.top, GRAVEL | DIRT) && ground > sea_level + 3.0 && unit(r) < 0.35).then(|| {
                    let mut rng = Rng(hash(r));
                    let a = rng.range(0.0, TAU);
                    let (dx, dy) = (a.cos(), a.sin());
                    let long = rng.range(5.0, 7.0);
                    let mut shapes = vec![Shape::capsule([x - dx * long, y - dy * long, ground + 0.3], [x + dx * long, y + dy * long, ground + 0.9], 0.45, 0.65, BONE, FUR_CREAM, false)];
                    // A skull at the head end, and ribs arching up off the spine
                    shapes.push(Shape::sphere([x + dx * (long + 1.4), y + dy * (long + 1.4), ground + 1.0], 1.3, BONE, FUR_CREAM, false));
                    for k in 0..7 {
                        let along = -long * 0.7 + k as f32 * long * 0.22;
                        let at = [x + dx * along, y + dy * along, ground + 0.7];
                        let tall = rng.range(2.6, 4.2) * (1.0 - (k as f32 - 3.0).abs() * 0.12);
                        for side in [-1.0f32, 1.0] {
                            let out = [-dy * side, dx * side];
                            let elbow = [at[0] + out[0] * 1.8, at[1] + out[1] * 1.8, at[2] + tall * 0.6];
                            let tip = [at[0] + out[0] * 1.3, at[1] + out[1] * 1.3, at[2] + tall];
                            shapes.push(Shape::capsule(at, elbow, 0.22, 0.18, BONE, FUR_CREAM, false));
                            shapes.push(Shape::capsule(elbow, tip, 0.18, 0.08, BONE, FUR_CREAM, false));
                        }
                    }
                    Object::new(shapes, rng.0)
                })
            });
            scatter(26.0, 0x0b51, 6.0, &mut |x, y, ground, _, r| {
                (ground > sea_level + 1.0 && unit(r) < 0.3).then(|| {
                    let mut rng = Rng(hash(r));
                    let shapes = (0..2 + (rng.next() * 2.99) as usize)
                        .map(|_| {
                            let foot = [x + rng.range(-1.6, 1.6), y + rng.range(-1.6, 1.6), ground - 0.5];
                            let tall = rng.range(1.5, 5.0);
                            Shape::capsule(foot, add(foot, [rng.range(-0.3, 0.3) * tall, rng.range(-0.3, 0.3) * tall, tall]), rng.range(0.35, 0.8), 0.04, OBSIDIAN, OBSIDIAN, false)
                        })
                        .collect();
                    Object::new(shapes, rng.0)
                })
            });
            scatter(ROOST_CELL, ROOST_SALT, 6.0, &mut |x, y, ground, _, r| {
                has_roost(&self.settings, x, y, r).then(|| {
                    let mut rng = Rng(hash(r));
                    let mut shapes = Vec::new();
                    let ring = rng.range(2.6, 3.6);
                    for k in 0..12 {
                        let (a, b) = (k as f32 * TAU / 12.0, (k as f32 + 1.3) * TAU / 12.0);
                        let p = |t: f32, up: f32| [x + t.cos() * ring, y + t.sin() * ring, ground + up];
                        let stuff = if k % 3 == 0 { BONE } else { DEAD_WOOD };
                        shapes.push(Shape::capsule(p(a, 0.4), p(b, 0.9), 0.3, 0.25, stuff, BARK_DARK, false));
                    }
                    for k in 0..1 + (rng.next() * 2.99) as usize {
                        let a = k as f32 * 2.1 + rng.range(0.0, 1.0);
                        shapes.push(Shape::sphere([x + a.cos() * 0.9, y + a.sin() * 0.9, ground + 0.6], 0.55, if k == 0 { GOLD } else { OBSIDIAN }, GLOW_FIRE, false));
                    }
                    Object::new(shapes, rng.0)
                })
            });
            // Far fewer and far greater, the bone caves
            scatter(BONE_CAVE_CELL, BONE_CAVE_SALT, 23.0, &mut |x, y, _, _, r| bone_cave_facing(&self.settings, x, y, r, rivers).map(|facing| bone_cave(&self.settings, [x, y], facing, r)));
        }
        // The frozen realm's own: clusters of spires of ice jutting up leaning out of the snow,
        // shorter ones out of the frozen sea and rivers, their points clear as crystal
        if land == Land::Ice {
            scatter(22.0, 0x1ce5, 6.0, &mut |x, y, ground, column, r| {
                (unit(r) < 0.45).then(|| {
                    let mut rng = Rng(hash(r));
                    let tallest = if column.top == ICE { 4.5 } else { 8.5 };
                    let shapes = (0..2 + (rng.next() * 3.99) as usize)
                        .map(|_| {
                            let foot = [x + rng.range(-2.0, 2.0), y + rng.range(-2.0, 2.0), ground - 0.6];
                            let tall = rng.range(2.0, tallest);
                            let tip = add(foot, [rng.range(-0.25, 0.25) * tall, rng.range(-0.25, 0.25) * tall, tall]);
                            Shape::capsule(foot, tip, rng.range(0.45, 1.0), 0.04, ICE, CRYSTAL, false)
                        })
                        .collect();
                    Object::new(shapes, rng.0)
                })
            });
        }

        features.footprints = footprints;
        (objects, features)
    }

    /// Temples and ruins on flat, dry ground, at most one in each BUILDING_CELL square, and
    /// gateways over some of the waterfalls. None where `keep_out` says, x0, y0, x1, y1 in meters.
    fn buildings(&self, heights: &Heights, features: &mut Features, keep_out: &[[f32; 4]]) -> Vec<Object> {
        const BUILDING_CELL: f32 = 56.0;
        let seed = self.settings.seed;
        let sea_level = sea_level_m(&self.settings);
        // The ground at a point, in meters, and whether a river runs there, the same whichever
        // piece of the landscape it's worked out for
        let ground = |x: f32, y: f32| -> Option<(f32, bool)> {
            let (g, r) = heights.at_m(x, y);
            Some((g, r.is_finite()))
        };
        let mut objects = Vec::new();

        let (lo, hi) = (self.lo, self.hi);
        let first = [(lo[0] / BUILDING_CELL).floor() as i32, (lo[1] / BUILDING_CELL).floor() as i32];
        let last = [(hi[0] / BUILDING_CELL).ceil() as i32, (hi[1] / BUILDING_CELL).ceil() as i32];
        for cy in first[1]..last[1] {
            for cx in first[0]..last[0] {
                let r = hash3(cx as u32, cy as u32, seed ^ 0x7e3b);
                if unit(r) > 0.85 {
                    continue;
                }
                let s = hash(r);
                let x = (cx as f32 + 0.2 + 0.6 * unit(s)) * BUILDING_CELL;
                let y = (cy as f32 + 0.2 + 0.6 * unit(s >> 16)) * BUILDING_CELL;

                // Each kind of land has buildings of its own: Aztec pyramids and temples deep in
                // the jungle, overgrown colonnades on its edges, carved sandstone ruins in the
                // desert, and castles, watchtowers, colonnades and flooded churches elsewhere.
                // They sink into the ground on the uphill side, so the ground only has to be
                // fairly flat.
                let c = climate(&self.settings, x, y);
                let pick = unit(r >> 8);
                let kind = if c.jungle > 0.7 {
                    if pick < 0.55 { "pyramid" } else { "shrine" }
                } else if c.jungle > 0.3 {
                    "colonnade"
                } else if c.desert > 0.6 {
                    if pick < 0.65 { "desert ruins" } else { "colonnade" }
                } else if c.snow > 0.6 {
                    if pick < 0.6 { "castle" } else { "watchtower" }
                } else if pick < 0.3 {
                    "castle"
                } else if pick < 0.55 {
                    "flooded church"
                } else if pick < 0.78 {
                    "colonnade"
                } else {
                    "watchtower"
                };
                let (half, flatness) = match kind {
                    "pyramid" => (12.5, 6.0),
                    "shrine" => (5.0, 3.5),
                    "flooded church" => (9.5, 4.0),
                    "colonnade" => (8.5, 4.5),
                    "watchtower" => (4.5, 3.0),
                    _ => (8.5, 6.0),
                };
                if keep_out.iter().any(|k| overlaps(*k, [x - half - 1.0, y - half - 1.0, x + half + 1.0, y + half + 1.0], 0.0)) {
                    continue;
                }

                // The whole site has to be dry and flat enough
                let samples: Option<Vec<(f32, bool)>> = (0..25)
                    .map(|i| ground(x - half + half * 0.5 * (i % 5) as f32, y - half + half * 0.5 * (i / 5) as f32))
                    .collect();
                let Some(samples) = samples else {
                    continue;
                };
                let lowest = samples.iter().map(|s| s.0).fold(f32::MAX, f32::min);
                let highest = samples.iter().map(|s| s.0).fold(f32::MIN, f32::max);
                if samples.iter().any(|&(_, wet)| wet) || lowest < sea_level + 1.0 || highest - lowest > flatness {
                    continue;
                }

                // Facing downhill, so the way in is at ground level
                let facing = [[1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]]
                    .into_iter()
                    .min_by(|a: &[f32; 2], b: &[f32; 2]| {
                        let g = |d: &[f32; 2]| ground(x + d[0] * half, y + d[1] * half).map_or(f32::MAX, |g| g.0);
                        g(a).total_cmp(&g(b))
                    })
                    .unwrap();
                let at = Frame { x, y, f: facing };
                let mut rng = Rng(hash(s));
                let (shapes, top) = match kind {
                    "pyramid" => self.pyramid(at, lowest, &mut rng, features),
                    "shrine" => self.shrine(at, lowest, &mut rng, features),
                    "desert ruins" => self.desert_ruins(at, lowest, &mut rng, features),
                    "flooded church" => self.flooded_church(at, lowest, &mut rng, features),
                    "colonnade" => {
                        let stone = if c.desert > 0.6 {
                            (SANDSTONE, SANDSTONE_DARK)
                        } else if c.jungle > 0.3 {
                            (TEMPLE_STONE, MOSS)
                        } else {
                            (LIMESTONE, MOSS)
                        };
                        self.colonnade(at, lowest, stone, &mut rng, features)
                    }
                    "watchtower" => self.watchtower(at, lowest, if c.snow > 0.6 { (BRICK_STONE, SNOW) } else { (BRICK_STONE, MOSS) }, &mut rng, features),
                    _ => self.castle(at, lowest, c.snow > 0.6, &mut rng, features),
                };
                features.sites.push(Site { kind, position: [x, y, lowest], facing });
                objects.push(Object::new(shapes, rng.0));
                features.footprints.push([x - half - 1.0, y - half - 1.0, x + half + 1.0, y + half + 1.0, top]);
            }
        }

        // Aztec gateways standing over waterfalls in the jungle, for the river to pour through, if
        // it isn't too wide to
        for (lip, flow, wide) in self.waterfalls_near() {
            let r = hash3((lip[0] * 4.0) as i32 as u32, (lip[1] * 4.0) as i32 as u32, seed ^ 0x9a7e);
            if unit(r) > 0.5 || wide > STREAM_HALF_WIDTH + 0.5 || climate(&self.settings, lip[0], lip[1]).jungle < 0.7 {
                continue;
            }
            let mut rng = Rng(hash(r));
            let (shapes, top) = self.gateway(lip, flow, &mut rng, features);
            features.sites.push(Site { kind: "gateway", position: lip, facing: flow });
            objects.push(Object::new(shapes, rng.0));
            features.footprints.push([lip[0] - 5.0, lip[1] - 5.0, lip[0] + 5.0, lip[1] + 5.0, top]);
        }
        objects
    }

    /// A stepped pyramid with stairs up the front to a temple on top. Inside, a passage under the
    /// stairs leads to a chamber lit by candles, where water pours from a spout in the back wall
    /// into a pool, overflows and runs out along the passage. `base` is the lowest ground under it.
    /// Returns its shapes and how high it reaches.
    fn pyramid(&self, at: Frame, base: f32, rng: &mut Rng, features: &mut Features) -> (Vec<Shape>, f32) {
        let half = rng.range(8.0, 11.0);
        let tiers = 4 + (rng.next() * 3.0) as usize;
        let tier = 1.75;
        let inset = (half - 4.0) / tiers as f32;
        let top = base + tier * tiers as f32;
        let floor = base + 0.25;
        let weathering = if rng.next() < 0.4 { MOSS } else { TEMPLE_DARK };
        let mut s = Vec::new();

        // The chamber, with the spout and an altar in it, the pool and the passage out
        s.push(at.block([-3.5, -2.9], [-0.45, 0.45], [floor, floor + 2.4], TEMPLE_TEAL, TEMPLE_DARK));
        s.push(at.block([-3.4, -2.6], [2.0, 3.3], [floor, floor + 0.9], TEMPLE_DARK, AIR));
        s.push(at.room([-2.9, -0.6], [-1.2, 1.2], [floor - 0.75, floor]));
        s.push(at.room([-3.5, 3.5], [-3.5, 3.5], [floor, floor + 3.5]));
        s.push(at.room([3.5, half + 1.5], [-0.8, 0.8], [floor, floor + 2.5]));
        features.springs.push({
            let p = at.point(-3.2, 0.0, 0.0);
            [p[0], p[1], 0.12, 0.35]
        });
        for (u, v, height, water) in [
            ([-3.5, 3.5], [-3.5, 3.5], floor, 0.0),
            ([3.5, half + 1.5], [-0.8, 0.8], floor, 0.0),
            ([-2.9, -0.6], [-1.2, 1.2], floor - 0.75, floor),
            ([-3.5, -2.9], [-0.45, 0.45], floor + 2.4, 0.0),
        ] {
            let (lo, hi) = at.rect(u, v);
            features.floors.push(Floor { lo, hi, height, water });
        }
        for (u, v) in [(-2.9, 1.5), (-0.6, 1.5), (-2.9, -1.5), (-0.6, -1.5)] {
            candle(features, at.point(u, v, floor), rng);
        }
        for (u, v) in [(-3.1, 2.3), (-2.9, 2.7), (-3.15, 3.05)] {
            candle(features, at.point(u, v, floor + 0.9), rng);
        }
        skull(features, at.point(-3.0, 2.9, floor + 0.9), rng);
        for (u, v) in [(-3.1, -2.9), (-2.8, -3.1), (-3.0, -3.0)] {
            skull(features, at.point(u, v, floor), rng);
        }
        skull(features, at.point(-3.15, -3.1, floor + 0.22), rng);
        // Pyramids hold the best treasure, deep in the jungle
        chest(features, at.point(1.5, -3.1, floor), at.heading(FRAC_PI_2), 2.0);

        // The temple on top, with its doorway facing the stairs
        s.push(at.room([-1.9, 1.1], [-1.7, 1.7], [top, top + 2.3]));
        s.push(at.room([1.0, 1.6], [-0.6, 0.6], [top, top + 1.9]));
        s.push(at.block([-1.0, -0.6], [-2.0, 2.0], [top + 3.2, top + 4.4], TEMPLE_TEAL, TEMPLE_RED));
        s.push(at.block([-2.8, 1.8], [-2.5, 2.5], [top + 2.8, top + 3.2], TEMPLE_DARK, AIR));
        s.push(at.block([-2.3, 1.4], [-2.1, 2.1], [top, top + 0.6], TEMPLE_DARK, AIR));
        s.push(at.block([-2.3, 1.4], [-2.1, 2.1], [top, top + 2.8], TEMPLE_RED, TEMPLE_DARK));
        for v in [-1.1, 1.1] {
            candle(features, at.point(-1.4, v, top), rng);
        }
        skull(features, at.point(-1.5, 0.0, top), rng);
        chest(features, at.point(-0.3, 1.25, top), at.heading(-FRAC_PI_2), 1.4);

        // A low wall round the top, open at the stairs
        let t = half - tiers as f32 * inset;
        s.push(at.block([-t, t], [t - 0.4, t], [top, top + 0.5], TEMPLE_DARK, AIR));
        s.push(at.block([-t, t], [-t, -t + 0.4], [top, top + 0.5], TEMPLE_DARK, AIR));
        s.push(at.block([-t, -t + 0.4], [-t, t], [top, top + 0.5], TEMPLE_DARK, AIR));
        s.push(at.block([t - 0.4, t], [1.6, t], [top, top + 0.5], TEMPLE_DARK, AIR));
        s.push(at.block([t - 0.4, t], [-t, -1.6], [top, top + 0.5], TEMPLE_DARK, AIR));

        // Stairs up the front between two balustrades
        let rise = 0.375;
        let run = rise * inset / tier;
        let steps = ((top - base) / rise).ceil() as usize;
        for k in 0..steps {
            let front = half + 1.0 - k as f32 * run;
            let z = base + (k + 1) as f32 * rise;
            if k % 2 == 0 {
                s.push(at.block([0.0, front + 0.1], [1.5, 2.1], [base - 1.0, z + 0.6], TEMPLE_DARK, AIR));
                s.push(at.block([0.0, front + 0.1], [-2.1, -1.5], [base - 1.0, z + 0.6], TEMPLE_DARK, AIR));
            }
            s.push(at.block([0.0, front], [-1.5, 1.5], [base - 1.0, z], TEMPLE_STONE, weathering));
        }

        // The tiers, each with a dark band round its top
        for k in 0..tiers {
            let h = half - k as f32 * inset;
            let z = base + (k + 1) as f32 * tier;
            s.push(at.block([-h - 0.12, h + 0.12], [-h - 0.12, h + 0.12], [z - 0.25, z], TEMPLE_DARK, AIR));
            s.push(at.block([-h, h], [-h, h], [if k == 0 { base - 1.0 } else { z - tier }, z], TEMPLE_STONE, weathering));
        }
        (s, top + 4.4)
    }

    /// A small temple on a platform, with an altar inside
    fn shrine(&self, at: Frame, base: f32, rng: &mut Rng, features: &mut Features) -> (Vec<Shape>, f32) {
        let floor = base + 1.0;
        let mut s = Vec::new();
        s.push(at.block([-1.3, -0.8], [-0.6, 0.6], [floor, floor + 0.7], TEMPLE_DARK, AIR));
        s.push(at.room([-1.4, 1.2], [-1.2, 1.2], [floor, floor + 2.1]));
        s.push(at.room([1.1, 1.8], [-0.55, 0.55], [floor, floor + 1.8]));
        s.push(at.block([-2.3, 2.1], [-2.1, 2.1], [floor + 2.5, floor + 2.9], TEMPLE_DARK, AIR));
        s.push(at.block([-0.9, -0.5], [-1.6, 1.6], [floor + 2.9, floor + 3.8], TEMPLE_TEAL, TEMPLE_RED));
        s.push(at.block([-1.8, 1.6], [-1.6, 1.6], [floor, floor + 0.4], TEMPLE_DARK, AIR));
        s.push(at.block([-1.8, 1.6], [-1.6, 1.6], [floor, floor + 2.5], TEMPLE_RED, TEMPLE_DARK));
        s.push(at.block([3.5, 4.3], [-1.2, 1.2], [base - 1.0, base + 0.35], TEMPLE_STONE, MOSS));
        s.push(at.block([2.9, 3.6], [-1.2, 1.2], [base - 1.0, base + 0.7], TEMPLE_STONE, MOSS));
        s.push(at.block([-3.0, 3.0], [-3.0, 3.0], [base - 1.0, floor], TEMPLE_STONE, MOSS));
        for v in [-0.4, 0.4] {
            candle(features, at.point(-1.05, v, floor + 0.7), rng);
        }
        skull(features, at.point(-1.05, 0.0, floor + 0.7), rng);
        chest(features, at.point(0.2, 0.85, floor), at.heading(-FRAC_PI_2), 1.2);
        (s, floor + 3.8)
    }

    /// The ruins of a small stone castle: a roofless hall with a doorway and windows, a tall
    /// tower with battlements and a smaller broken one, all of brick, overgrown with moss and
    /// vines, or snowed on. Candles burn inside.
    fn castle(&self, at: Frame, base: f32, snowy: bool, rng: &mut Rng, features: &mut Features) -> (Vec<Shape>, f32) {
        let weather = if snowy { SNOW } else { MOSS };
        let mut s = Vec::new();
        let hall = rng.range(3.5, 5.0);
        let tower = rng.range(7.0, 9.5);
        let small = rng.range(4.5, 6.0);

        // Vines hanging down the outside of the walls
        if !snowy {
            for _ in 0..8 {
                let along = rng.range(-1.5, 4.5);
                let (u, v) = if rng.next() < 0.5 { (along, if rng.next() < 0.5 { 3.35 } else { -3.35 }) } else { (5.35, rng.range(-2.5, 2.5)) };
                let length = rng.range(1.0, hall * 0.7);
                s.push(Shape::capsule(at.point(u, v, base + hall * 0.6), at.point(u, v, base + hall * 0.6 - length), 0.07, 0.05, VINE, LEAVES, false));
            }
        }

        // The hall's doorway, windows and inside, and the towers' insides and doors
        s.push(at.room([4.3, 5.2], [-0.65, 0.65], [base, base + 2.4]));
        for u in [-0.3, 2.6] {
            for v in [-3.1, 3.1] {
                s.push(at.room([u - 0.35, u + 0.35], [v - 0.45, v + 0.45], [base + 1.6, base + 2.8]));
            }
        }
        s.push(at.room([-1.4, 4.4], [-2.4, 2.4], [base, base + 9.0]));
        s.push(at.room([-3.9, -2.1], [2.1, 3.9], [base, base + tower - 0.6]));
        s.push(at.room([-2.2, -1.3], [2.5, 3.3], [base, base + 2.2]));
        s.push(at.room([-4.6, -3.4], [2.7, 3.3], [base + tower - 2.6, base + tower - 1.6]));
        s.push(at.room([-3.6, -2.4], [-4.0, -3.0], [base, base + small]));

        // The hall's walls, each broken off at different heights
        let stone = (BRICK_STONE, weather);
        broken_wall(&mut s, &at, rng, [-2.0, 5.0], [2.4, 3.0], base, hall * 0.5, hall, stone);
        broken_wall(&mut s, &at, rng, [-2.0, 5.0], [-3.0, -2.4], base, hall * 0.4, hall, stone);
        broken_wall(&mut s, &at, rng, [4.4, 5.0], [-2.4, 2.4], base, hall * 0.8, hall, stone);
        broken_wall(&mut s, &at, rng, [-2.0, -1.4], [-2.4, 2.4], base, hall * 0.6, hall, stone);

        // The tall tower, with battlements round the top
        for (u, v) in [(-4.5, 1.5), (-1.95, 1.5), (-4.5, 3.95), (-1.95, 3.95)] {
            s.push(at.block([u, u + 0.55], [v, v + 0.55], [base + tower, base + tower + 0.7], BRICK_STONE, weather).patterned(Pattern::Bricks));
        }
        s.push(at.block([-4.5, -1.4], [1.5, 4.5], [base - 0.5, base + tower], BRICK_STONE, weather).patterned(Pattern::Bricks));
        // The small tower, its top fallen in
        broken_wall(&mut s, &at, rng, [-4.2, -1.8], [-4.6, -2.4], base, small * 0.6, small, stone);

        // Steps up to the door, and rubble lying about
        s.push(at.block([5.0, 5.8], [-1.0, 1.0], [base - 0.5, base + 0.15], BRICK_STONE, weather).patterned(Pattern::Bricks));
        for _ in 0..8 {
            let (u, v) = (rng.range(-6.0, 7.0), rng.range(-5.5, 5.5));
            let size = rng.range(0.3, 0.7);
            s.push(at.block([u, u + size], [v, v + size * rng.range(0.8, 1.6)], [base - 0.2, base + size * 0.7], BRICK_STONE, weather));
        }

        for (u, v) in [(3.6, -1.8), (3.6, 1.8)] {
            candle(features, at.point(u, v, base), rng);
        }
        candle(features, at.point(-3.6, 3.0, base + tower - 2.6), rng);
        skull(features, at.point(-0.8, -1.9, base), rng);
        chest(features, at.point(0.6, 2.05, base), at.heading(-FRAC_PI_2), 1.3);
        (s, base + tower + 1.0)
    }

    /// A church long fallen into ruin: roofless walls with tall arched windows, a great doorway
    /// and a round window in the end, its sunken floor flooded with clear water, a little welling
    /// up and spilling out of the door, and a tree growing up in the middle
    fn flooded_church(&self, at: Frame, base: f32, rng: &mut Rng, features: &mut Features) -> (Vec<Shape>, f32) {
        let height = rng.range(7.0, 9.0);
        let floor = base - 0.6;
        let mut s = Vec::new();

        // The tree goes first, so the empty inside doesn't swallow it
        let (leaves, accent) = if rng.next() < 0.7 { (LEAVES_ORANGE, LEAVES_RED) } else { (LEAVES, LEAVES_YELLOW) };
        let foliage = Foliage { material: leaves, accent, leaf_length: 0.22, leaves_per_twig: 12, clumps_per_twig: 3 };
        let trunk = at.point(0.5, 0.0, 0.0);
        s.extend(self.broadleaf(trunk[0], trunk[1], floor, 1.1, 2, WOOD, foliage, rng));

        // The doorway with its round arch, tall arched windows down both sides, a round window
        // at the far end, and the inside
        s.push(at.room([6.2, 7.8], [-1.3, 1.3], [base - 0.05, base + 3.6]));
        s.push(Shape::capsule(at.point(6.2, 0.0, base + 3.6), at.point(7.8, 0.0, base + 3.6), 1.3, 1.3, AIR, AIR, false).placed(Placement::Clear));
        for u in [-4.5, -1.5, 1.5, 4.5] {
            for v in [-4.1, 4.1] {
                s.push(at.room([u - 0.55, u + 0.55], [v - 0.6, v + 0.6], [base + 2.2, base + 5.0]));
                s.push(Shape::capsule(at.point(u, v - 0.6, base + 5.0), at.point(u, v + 0.6, base + 5.0), 0.55, 0.55, AIR, AIR, false).placed(Placement::Clear));
            }
        }
        s.push(Shape::sphere(at.point(-7.0, 0.0, base + 5.8), 1.2, AIR, AIR, false).placed(Placement::Clear));
        s.push(at.room([-6.2, 6.2], [-3.7, 3.7], [floor, base + height + 1.0]));

        // The stone floor under the water, and the walls, broken off at different heights
        s.push(at.block([-6.2, 6.2], [-3.7, 3.7], [floor - 1.0, floor], LIMESTONE, MOSS).patterned(Pattern::Bricks));
        let stone = (LIMESTONE, MOSS);
        broken_wall(&mut s, &at, rng, [-7.6, 7.6], [3.7, 4.5], base, height * 0.55, height, stone);
        broken_wall(&mut s, &at, rng, [-7.6, 7.6], [-4.5, -3.7], base, height * 0.55, height, stone);
        broken_wall(&mut s, &at, rng, [-7.6, -6.2], [-3.7, 3.7], base, height * 0.8, height * 1.15, stone);
        broken_wall(&mut s, &at, rng, [6.2, 7.6], [-3.7, 3.7], base, height * 0.7, height * 1.1, stone);
        // Buttresses down the sides
        for u in [-6.0, -3.0, 0.0, 3.0, 6.0] {
            for v in [[4.5, 5.4], [-5.4, -4.5]] {
                s.push(at.block([u - 0.45, u + 0.45], v, [base - 0.5, base + height * 0.6], LIMESTONE, MOSS).patterned(Pattern::Bricks));
            }
        }

        // The water: still inside, held in by the doorstep, and topped up so it spills out
        let (lo, hi) = at.rect([-6.2, 6.2], [-3.7, 3.7]);
        features.floors.push(Floor { lo, hi, height: floor, water: base - 0.15 });
        let (lo, hi) = at.rect([6.2, 7.8], [-1.3, 1.3]);
        features.floors.push(Floor { lo, hi, height: base - 0.05, water: 0.0 });
        let spring = at.point(-3.0, 0.0, 0.0);
        features.springs.push([spring[0], spring[1], 0.03, 0.5]);
        // Sunk under the water at the far end
        chest(features, at.point(-5.6, 0.0, floor), at.heading(0.0), 1.6);
        (s, base + height * 1.15 + 1.0)
    }

    /// Ruins in the desert: carved sandstone pillars, some topped with onion domes, an arch, low
    /// walls half buried in the sand, clay urns and paving
    fn desert_ruins(&self, at: Frame, base: f32, rng: &mut Rng, features: &mut Features) -> (Vec<Shape>, f32) {
        let mut s = Vec::new();
        // Under the arch
        chest(features, at.point(0.0, 0.0, base + 0.1), at.heading(0.0), 1.2);
        for _ in 0..4 + (rng.next() * 4.0) as usize {
            let (u, v) = (rng.range(-6.5, 6.5), rng.range(-5.5, 5.5));
            let r = rng.range(0.3, 0.5);
            let body = at.point(u, v, base + r * 1.1);
            let mouth = at.point(u, v, base + r * 2.3);
            s.push(Shape::capsule(mouth, mouth, r * 0.55, r * 0.55, CLAY_POT, SANDSTONE_DARK, false));
            s.push(Shape::capsule(body, mouth, r * 0.45, r * 0.4, CLAY_POT, SANDSTONE_DARK, false));
            s.push(Shape::ellipsoid(body, [r, r, r * 1.2], CLAY_POT, SANDSTONE_DARK).patterned(Pattern::Carved));
        }

        let mut top = base + 2.0;
        for _ in 0..2 + (rng.next() * 2.0) as usize {
            let (u, v) = (rng.range(-6.0, 6.0), rng.range(-5.0, 5.0));
            let (r, h) = (rng.range(0.75, 1.0), rng.range(3.0, 6.0));
            let foot = at.point(u, v, base - 0.5);
            let head = at.point(u, v, base + h);
            if rng.next() < 0.5 {
                // An onion dome on top
                let dome = at.point(u, v, base + h + r * 0.7);
                s.push(Shape::capsule(dome, at.point(u, v, base + h + r * 2.2), r * 0.55, 0.04, SANDSTONE, SANDSTONE_DARK, false).patterned(Pattern::Carved));
                s.push(Shape::sphere(dome, r * 0.8, SANDSTONE, SANDSTONE_DARK, false).patterned(Pattern::Carved));
                top = top.max(base + h + r * 2.2);
            }
            s.push(Shape::capsule(head, at.point(u, v, base + h - 0.3), r + 0.2, r + 0.2, SANDSTONE_DARK, AIR, false));
            s.push(Shape::capsule(foot, head, r, r, SANDSTONE, SAND, false).patterned(Pattern::Carved));
            top = top.max(base + h);
        }

        // The arch, with fins standing up round its top
        let (span, rise) = (2.4, base + 3.2);
        for d in [-1.4, 0.0, 1.4] {
            s.push(at.block([-0.25, 0.25], [d - 0.35, d + 0.35], [rise + span - 0.2, rise + span + 1.0], SANDSTONE, SANDSTONE_DARK).patterned(Pattern::Carved));
        }
        for k in 0..10 {
            let (a, b) = (k as f32 / 10.0 * std::f32::consts::PI, (k + 1) as f32 / 10.0 * std::f32::consts::PI);
            let point = |t: f32| at.point(0.0, span * t.cos(), rise + span * t.sin());
            s.push(Shape::capsule(point(a), point(b), 0.5, 0.5, SANDSTONE, SANDSTONE_DARK, false).patterned(Pattern::Carved));
        }
        for v in [-span, span] {
            s.push(at.block([-0.55, 0.55], [v - 0.55, v + 0.55], [base - 0.5, rise], SANDSTONE, SANDSTONE_DARK).patterned(Pattern::Carved));
        }

        // Low walls, half buried in the sand, and paving
        let stone = (SANDSTONE, SAND);
        for _ in 0..2 + (rng.next() * 2.0) as usize {
            let (u, v) = (rng.range(-6.0, 6.0), rng.range(-5.0, 5.0));
            let length = rng.range(2.0, 4.5);
            if rng.next() < 0.5 {
                broken_wall(&mut s, &at, rng, [u, u + length], [v, v + 0.5], base, 0.5, 2.0, stone);
            } else {
                broken_wall(&mut s, &at, rng, [u, u + 0.5], [v, v + length], base, 0.5, 2.0, stone);
            }
        }
        s.push(at.block([-7.0, 7.0], [-6.0, 6.0], [base - 0.6, base + 0.1], SANDSTONE, SAND).patterned(Pattern::Bricks));
        (s, top + 1.0)
    }

    /// The colonnade of a temple long gone: a raised floor with steps all round, two rows of
    /// carved columns, many broken off, a lintel still bridging one pair, and fallen drums lying
    /// about. Of `stone`, weathered with its accent. A chest waits at the far end.
    fn colonnade(&self, at: Frame, base: f32, stone: (u8, u8), rng: &mut Rng, features: &mut Features) -> (Vec<Shape>, f32) {
        let mut s = Vec::new();
        let length = rng.range(9.0, 13.0);
        let count = (length / 2.2) as usize;
        let spacing = length / count as f32;
        let tall = rng.range(4.0, 5.5);
        let floor = base + 0.6;
        // The pair still holding up a lintel stays whole
        let lintel = 1 + (rng.next() * (count - 1) as f32) as usize;
        for k in 0..=count {
            let u = -length * 0.5 + k as f32 * spacing;
            for v in [-2.4f32, 2.4] {
                let broken = k != lintel && rng.next() < 0.5;
                let height = if broken { rng.range(0.4, tall * 0.8) } else { tall };
                if broken {
                    // Its fallen drums, lying where they rolled
                    for _ in 0..1 + (rng.next() * 2.0) as usize {
                        let (du, dv) = (rng.range(-1.6, 1.6), v.signum() * rng.range(0.8, 2.2));
                        let angle = rng.next() * TAU;
                        let a = at.point(u + du, v + dv, floor + 0.3);
                        let b = [a[0] + angle.cos() * 0.8, a[1] + angle.sin() * 0.8, a[2]];
                        s.push(Shape::capsule(a, b, 0.3, 0.3, stone.0, stone.1, false).patterned(Pattern::Carved));
                    }
                } else {
                    s.push(at.block([u - 0.46, u + 0.46], [v - 0.46, v + 0.46], [floor + height - 0.3, floor + height], stone.0, stone.1));
                }
                s.push(at.block([u - 0.44, u + 0.44], [v - 0.44, v + 0.44], [floor, floor + 0.3], stone.0, stone.1));
                s.push(Shape::capsule(at.point(u, v, floor), at.point(u, v, floor + height), 0.34, 0.3, stone.0, stone.1, false).patterned(Pattern::Carved));
            }
            if k == lintel {
                s.push(at.block([u - 0.5, u + 0.5], [-2.9, 2.9], [floor + tall, floor + tall + 0.55], stone.0, stone.1).patterned(Pattern::Carved));
            }
        }
        // The raised floor, and a step all round it
        let end = length * 0.5;
        s.push(at.block([-end - 1.2, end + 1.2], [-3.6, 3.6], [base - 1.0, floor], stone.0, stone.1).patterned(Pattern::Bricks));
        s.push(at.block([-end - 1.8, end + 1.8], [-4.2, 4.2], [base - 1.0, base + 0.3], stone.0, stone.1).patterned(Pattern::Bricks));
        chest(features, at.point(-end + 0.2, 0.0, floor), at.heading(0.0), 1.1);
        (s, floor + tall + 1.0)
    }

    /// A round watchtower crumbling away: thick walls broken off unevenly at the top, a doorway,
    /// narrow windows on the way up, rubble round its foot, and a candle still burning inside by a
    /// chest. Of `stone`, weathered with its accent.
    fn watchtower(&self, at: Frame, base: f32, stone: (u8, u8), rng: &mut Rng, features: &mut Features) -> (Vec<Shape>, f32) {
        let mut s = Vec::new();
        let r = rng.range(2.3, 3.0);
        let wall = 0.65;
        let height = rng.range(9.0, 13.0);

        // Openings first, so the walls don't fill them: the doorway, windows, bites out of the
        // top, and the hollow inside
        s.push(at.room([r - wall - 0.4, r + 0.4], [-0.55, 0.55], [base, base + 2.3]));
        for k in 0..4 {
            let a = k as f32 * TAU / 4.0 + 0.8;
            let (u, v) = (a.cos() * r, a.sin() * r);
            let z = base + 3.5 + k as f32 * 1.8;
            s.push(at.room([u - 0.3, u + 0.3], [v - 0.3, v + 0.3], [z, z + 1.0]));
        }
        for _ in 0..5 {
            let a = rng.next() * TAU;
            let z = base + height * rng.range(0.65, 1.0) + 1.2;
            s.push(Shape::sphere(at.point(a.cos() * r, a.sin() * r, z), rng.range(1.2, 2.2), AIR, AIR, false).placed(Placement::Clear));
        }
        s.push(Shape::capsule(at.point(0.0, 0.0, base), at.point(0.0, 0.0, base + height + 3.0), r - wall, r - wall, AIR, AIR, false).placed(Placement::Clear));

        // Rubble that's fallen from the top
        for _ in 0..10 {
            let a = rng.next() * TAU;
            let d = r + rng.range(0.4, 3.0);
            let size = rng.range(0.3, 0.7);
            let (u, v) = (a.cos() * d, a.sin() * d);
            s.push(at.block([u, u + size], [v, v + size * rng.range(0.8, 1.5)], [base - 0.2, base + size * 0.7], stone.0, stone.1).patterned(Pattern::Bricks));
        }
        s.push(Shape::capsule(at.point(0.0, 0.0, base - 1.0), at.point(0.0, 0.0, base + height), r, r, stone.0, stone.1, false).patterned(Pattern::Bricks));

        let inside = r - wall - 0.35;
        candle(features, at.point(-inside + 0.3, inside * 0.5, base), rng);
        chest(features, at.point(-inside + 0.25, -0.3, base), at.heading(0.0), 1.3);
        (s, base + height + r)
    }

    /// Two pillars on the banks either side of a waterfall's lip, joined by a beam over the water,
    /// with fires burning on top. `lip` is the middle of the lip and `flow` which way the river goes.
    fn gateway(&self, lip: [f32; 3], flow: [f32; 2], rng: &mut Rng, features: &mut Features) -> (Vec<Shape>, f32) {
        let mut s = Vec::new();
        let across = [-flow[1], flow[0]];
        let side = |d: f32, z: f32| [lip[0] + across[0] * d, lip[1] + across[1] * d, z];
        let (low, high) = (lip[2] - 6.0, lip[2] + 4.5);
        for d in [-4.0, 4.0] {
            let top = side(d, high);
            s.push(Shape::block([top[0] - 0.7, top[1] - 0.7, high - 0.1], [top[0] + 0.7, top[1] + 0.7, high + 0.5], TEMPLE_DARK, AIR));
            s.push(Shape::capsule(side(d, high - 1.2), side(d, high - 0.9), 0.62, 0.62, TEMPLE_TEAL, TEMPLE_RED, false));
            s.push(Shape::capsule(side(d, low), side(d, high), 0.55, 0.5, TEMPLE_STONE, MOSS, false));
            candle(features, side(d, high + 0.9), rng);
        }
        s.push(Shape::capsule(side(-4.6, high - 0.2), side(4.6, high - 0.2), 0.45, 0.45, TEMPLE_DARK, TEMPLE_TEAL, false));
        for d in [-1.6, 0.0, 1.6] {
            skull(features, side(d, high + 0.25), rng);
        }
        (s, high + 1.0)
    }

    /// Ravines: long, deep, narrow cracks in the ground, wandering along with ledges down their
    /// sides, at most one in each RAVINE_CELL square. Most are open to the sky; the rest are
    /// crevices hidden under the ground, with crystals glowing at the bottom.
    fn ravines(&self, features: &mut Features, royal: &[[f32; 4]]) -> Vec<Object> {
        let (lo, hi) = (self.lo, self.hi);
        let mut objects = Vec::new();
        let first = [((lo[0] - RAVINE_REACH) / RAVINE_CELL).floor() as i32, ((lo[1] - RAVINE_REACH) / RAVINE_CELL).floor() as i32];
        let last = [((hi[0] + RAVINE_REACH) / RAVINE_CELL).ceil() as i32, ((hi[1] + RAVINE_REACH) / RAVINE_CELL).ceil() as i32];
        for cy in first[1]..last[1] {
            for cx in first[0]..last[0] {
                // None breaks into a royal castle, see castles.rs
                let into = |o: &Object| royal.iter().any(|r| overlaps([o.min[0], o.min[1], o.max[0], o.max[1]], *r, 3.0));
                if !royal.is_empty() && self.ravine(cx, cy, &mut Features::default()).is_some_and(|o| into(&o)) {
                    continue;
                }
                objects.extend(self.ravine(cx, cy, features));
            }
        }
        objects
    }

    /// The ravine starting in one RAVINE_CELL square, if there's one
    fn ravine(&self, cx: i32, cy: i32, features: &mut Features) -> Option<Object> {
        const STEP: f32 = 2.0;
        let seed = self.settings.seed;
        let sea_level = sea_level_m(&self.settings);
        let r = hash3(cx as u32, cy as u32, seed ^ 0x7a71);
        if unit(r) > 0.7 {
            return None;
        }
        let mut rng = Rng(hash(r));
        let (mut x, mut y) = ((cx as f32 + rng.next()) * RAVINE_CELL, (cy as f32 + rng.next()) * RAVINE_CELL);
        if height_at(&self.settings, x, y) < sea_level + 4.0 {
            return None;
        }
        let open = rng.next() < 0.5;
        let mut heading = rng.next() * TAU;
        let width = rng.range(2.0, 4.5);
        let depth = rng.range(10.0, 22.0);
        let steps = 15 + (rng.next() * 30.0) as usize;
        let mut shapes = Vec::new();
        let mut crystals = Vec::new();
        for i in 0..steps {
            // Narrow at both ends
            let taper = (std::f32::consts::PI * (i as f32 + 0.5) / steps as f32).sin().sqrt();
            heading += rng.range(-0.25, 0.25);
            let along = [heading.cos(), heading.sin(), 0.0];
            x += along[0] * STEP;
            y += along[1] * STEP;
            let ground = height_at(&self.settings, x, y);
            if ground < sea_level + 1.0 {
                break;
            }
            let top = if open { ground + 2.0 } else { ground - rng.range(4.0, 7.0) };
            let bottom = top - depth * taper - 2.0;
            if i == steps / 2 && self.inside(x, y) {
                let kind = if open { "ravine" } else { "crevice" };
                features.sites.push(Site { kind, position: [x, y, if open { ground } else { bottom + 1.5 }], facing: [along[0], along[1]] });
            }
            let middle = (top + bottom) * 0.5;
            let w = width * taper + 0.6;
            // A wide upper part and a narrower deeper one, leaving ledges down the sides
            shapes.push(Shape::slab([x, y, middle + (top - bottom) * 0.15], along, STEP * 2.0, w, (top - bottom) * 0.75));
            shapes.push(Shape::slab([x, y, middle - (top - bottom) * 0.1], along, STEP * 2.0, w * 0.55, top - bottom));
            features.recuts.push([x - w - STEP, y - w - STEP, x + w + STEP, y + w + STEP, ground + 3.0]);
            if !open && i % 6 == 3 {
                let at = [x + rng.range(-0.3, 0.3), y + rng.range(-0.3, 0.3), bottom + 0.6];
                for _ in 0..3 {
                    let offset = [rng.range(-0.3, 0.3), rng.range(-0.3, 0.3), rng.range(0.0, 0.4)];
                    crystals.push(Shape::sphere(add(at, offset), rng.range(0.15, 0.3), CRYSTAL, AIR, false).placed(Placement::Inside));
                }
                if self.inside(at[0], at[1]) {
                    features.lights.push(Light { position: [at[0], at[1], at[2] + 0.5], color: [0.35, 0.85, 1.0], reach: 7.0 });
                }
            }
        }
        shapes.extend(crystals);
        Some(Object::new(shapes, rng.0))
    }

    /// Winding tunnels under the hills, at most one starting in each CAVE_CELL square, opening out
    /// into caverns along the way. Most open onto the hillside, and in those the last cavern is a
    /// great one where a boss makes its lair: "cavern", at the middle of its floor, facing the
    /// way in.
    fn caves(&self, features: &mut Features, royal: &[[f32; 4]], cities: &[[f32; 4]]) -> Vec<Object> {
        let (lo, hi) = (self.lo, self.hi);
        let mut objects = Vec::new();
        let first = [((lo[0] - CAVE_REACH) / CAVE_CELL).floor() as i32, ((lo[1] - CAVE_REACH) / CAVE_CELL).floor() as i32];
        let last = [((hi[0] + CAVE_REACH) / CAVE_CELL).ceil() as i32, ((hi[1] + CAVE_REACH) / CAVE_CELL).ceil() as i32];
        for cy in first[1]..last[1] {
            for cx in first[0]..last[0] {
                let Some(cave) = self.cave_in(cx, cy) else {
                    continue;
                };
                // Far out, it's only what reaches into this piece that matters. None breaks out of
                // the ground inside a royal castle, see castles.rs.
                if !overlaps([lo[0], lo[1], hi[0], hi[1]], cave.bounds, 0.0) || royal.iter().any(|r| self.breaks_into(&cave, *r)) {
                    continue;
                }
                // Nor anywhere under a fallen city, see undercity.rs
                let under = |r: &[f32; 4]| overlaps(cave.bounds, *r, 0.0) && cave.shapes.iter().any(|s| s.placement == Placement::Carve && overlaps([s.min[0], s.min[1], s.max[0], s.max[1]], *r, 2.0));
                if cities.iter().any(under) {
                    continue;
                }
                features.lights.extend(cave.lights.into_iter().filter(|l| self.inside(l.position[0], l.position[1])));
                for &(floor, facing, _) in cave.lairs.iter().filter(|(f, ..)| self.inside(f[0], f[1])) {
                    features.sites.push(Site { kind: "cavern", position: floor, facing });
                }
                for &(floor, facing, _) in cave.nests.iter().filter(|(f, ..)| self.inside(f[0], f[1])) {
                    features.sites.push(Site { kind: "cave nest", position: floor, facing });
                }
                objects.push(Object::new(cave.shapes, cave.seed));
            }
        }
        objects
    }

    /// Behind some waterfalls, a cave: in through the falling water, a low way in under the lip of
    /// the fall, round sideways from under the river, and back into the hill to a little cavern
    /// glowing with crystals, where someone hid a chest. Kept under the shallow lip, where the
    /// river's bed is high, so it never breaks into the river and drains it.
    fn waterfall_caves(&self, features: &mut Features) -> Vec<Object> {
        let sea_level = sea_level_m(&self.settings);
        let mut objects = Vec::new();
        for (lip, flow, wide) in self.waterfalls_near() {
            let r = hash3((lip[0] * 4.0) as i32 as u32, (lip[1] * 4.0) as i32 as u32, self.settings.seed ^ 0xfa11);
            if unit(r) > 0.45 {
                continue;
            }
            let mut rng = Rng(hash(r));
            let pool = (lip[2] - FALL_HEIGHT).max(sea_level);
            // Its floor a little over the pool the water falls into, so none of it runs in
            let floor = pool + 0.5;
            let side = if rng.next() < 0.5 { [-flow[1], flow[0]] } else { [flow[1], -flow[0]] };
            // Out from under a wider river, the further it is to its bank
            let wider = (wide - STREAM_HALF_WIDTH).max(0.0);
            let at = |along: f32, out: f32, up: f32| {
                let out = if out > 0.0 { out + wider } else { out };
                [lip[0] + flow[0] * along + side[0] * out, lip[1] + flow[1] * along + side[1] * out, floor + up]
            };
            // Low under the lip, where there's the least rock between it and the river
            let radius = 0.95;
            let path = [at(0.9, 0.0, radius), at(-0.4, 0.0, radius), at(-0.4, 4.0, radius), at(-0.4, 7.5, radius), at(-3.5, 9.5, radius + 0.1)];
            let mut shapes = Vec::new();
            for pair in path.windows(2) {
                let tunnel = Shape::capsule(pair[0], pair[1], radius, radius, AIR, AIR, false).placed(Placement::Carve);
                shapes.push(tunnel.roughened(0.12, floor - 0.05));
            }
            // Just inside, under the lip, looking out through the falling water
            let inside = at(-0.2, 0.0, 0.0);
            if self.inside(inside[0], inside[1]) {
                features.sites.push(Site { kind: "waterfall cave", position: inside, facing: flow });
            }
            // The cavern, with room enough over it in the hill, else the cave ends where it is
            let middle = at(-6.0, 11.0, 1.4);
            let ground = height_at(&self.settings, middle[0], middle[1]);
            if middle[2] + 2.0 + 2.0 < ground {
                let cavern = Shape::ellipsoid(middle, [3.2, 3.2, 1.9], AIR, AIR).placed(Placement::Carve);
                shapes.push(cavern.roughened(0.35, floor - 0.05));
                shapes.push(Shape::capsule(path[4], middle, radius, 1.3, AIR, AIR, false).placed(Placement::Carve).roughened(0.12, floor - 0.05));
                let mut crystals = Vec::new();
                for k in 0..3 {
                    let a = rng.next() * TAU + k as f32 * 2.1;
                    let c = [middle[0] + a.cos() * 2.4, middle[1] + a.sin() * 2.4, floor];
                    for _ in 0..4 {
                        let offset = [rng.range(-0.4, 0.4), rng.range(-0.4, 0.4), rng.range(0.0, 0.4)];
                        crystals.push(Shape::sphere(add(c, offset), rng.range(0.15, 0.3), CRYSTAL, AIR, false).placed(Placement::Inside));
                    }
                    if self.inside(c[0], c[1]) {
                        features.lights.push(Light { position: [c[0], c[1], c[2] + 0.7], color: [0.4, 0.85, 1.0], reach: 8.0 });
                    }
                }
                shapes.extend(crystals);
                // Rolled whichever piece of the landscape it's in, so the random numbers run on the
                // same way, and the cave's the same, in every piece it's built for
                let luck = 3.0 + rng.next() * 2.0;
                if self.inside(middle[0], middle[1]) {
                    let heading = (lip[1] - middle[1]).atan2(lip[0] - middle[0]);
                    chest(features, [middle[0], middle[1], floor], heading, luck);
                }
            }
            objects.push(Object::new(shapes, rng.0));
        }
        objects
    }

    /// The cave starting in one CAVE_CELL square, if there's one
    fn cave_in(&self, cx: i32, cy: i32) -> Option<Cave> {
        let sea_level = sea_level_m(&self.settings);
        let r = hash3(cx as u32, cy as u32, self.settings.seed ^ 0xca7e);
        if unit(r) > 0.65 {
            return None;
        }
        let mut rng = Rng(hash(r));
        let mut p = [(cx as f32 + rng.next()) * CAVE_CELL, (cy as f32 + rng.next()) * CAVE_CELL, 0.0];
        let surface = height_at(&self.settings, p[0], p[1]);
        if surface < sea_level + 3.0 {
            return None;
        }
        let mouth = rng.next() < 0.6;
        p[2] = if mouth { surface - 0.8 } else { surface - rng.range(8.0, 16.0) };
        let mut yaw = rng.next() * TAU;
        let mut pitch: f32 = if mouth { -0.4 } else { 0.0 };
        let mut radius = rng.range(1.8, 2.5);
        let mut cave = Cave { shapes: Vec::new(), lights: Vec::new(), lairs: Vec::new(), nests: Vec::new(), bounds: [p[0], p[1], p[0], p[1]], seed: 0 };
        let mut inside = Vec::new();
        let segments = 70 + (rng.next() * 60.0) as usize;
        let mut next_cavern = rng.range(10.0, 18.0) as usize;
        let mut caverns = Vec::new();
        let mut lairs = Vec::new();
        for i in 0..segments {
            let next_radius = (radius + rng.range(-0.35, 0.35)).clamp(1.8, 3.4);
            let dir = [yaw.cos() * pitch.cos(), yaw.sin() * pitch.cos(), pitch.sin()];
            let q = add(p, scale(dir, 2.2));
            // Lumpy walls and roof over a flatter floor, the round bottom of the tube left filled
            let floor = p[2].min(q[2]) - radius.min(next_radius) * 0.6;
            let tunnel = Shape::capsule(p, q, radius, next_radius, AIR, AIR, false).placed(Placement::Carve);
            cave.shapes.push(tunnel.roughened(0.3 * radius.max(next_radius), floor));
            if i % 8 == 4 {
                let at = [q[0] + rng.range(-0.6, 0.6), q[1] + rng.range(-0.6, 0.6), q[2] - next_radius * 0.8];
                for _ in 0..3 {
                    let offset = [rng.range(-0.4, 0.4), rng.range(-0.4, 0.4), rng.range(0.0, 0.3)];
                    inside.push(Shape::sphere(add(at, offset), rng.range(0.15, 0.32), CRYSTAL, AIR, false).placed(Placement::Inside));
                }
                cave.lights.push(Light { position: [at[0], at[1], at[2] + 0.5], color: [0.35, 0.85, 1.0], reach: 8.0 });
            }
            // Opening out into a cavern now and then, on the way down, where cave spiders nest.
            // Deep in a long cave, some are great caverns a boss of its own makes its lair in.
            if i == next_cavern && i > 4 {
                let floor = [q[0], q[1], q[2] - next_radius * 0.75];
                let great = mouth && i > 30 && rng.next() < 0.35;
                if let Some((at, wide)) = self.cavern(&mut cave, &mut inside, floor, great, &mut rng) {
                    (if great { &mut lairs } else { &mut caverns }).push((at, [-dir[0], -dir[1]], wide));
                }
                next_cavern += rng.range(14.0, 24.0) as usize;
            }
            // Wander about, staying well under the ground, and never under the sea
            let depth = height_at(&self.settings, q[0], q[1]) - q[2];
            yaw += rng.range(-0.3, 0.3);
            let want = if depth < 7.0 && i > 4 { -0.3 } else if depth > 20.0 { 0.25 } else { rng.range(-0.12, 0.12) };
            pitch = (pitch * 0.7 + want * 0.3).clamp(-0.45, 0.45);
            p = q;
            radius = next_radius;
            if q[2] < sea_level - 2.0 {
                break;
            }
        }
        // A cave a way into opens at its far end into a great cavern, a boss's lair
        if mouth {
            let floor = [p[0], p[1], p[2] - radius * 0.75];
            let way_in = [-yaw.cos(), -yaw.sin()];
            match self.cavern(&mut cave, &mut inside, floor, true, &mut rng) {
                Some((at, wide)) => lairs.push((at, way_in, wide)),
                None => lairs.extend(caverns.pop()),
            }
        }
        // Only where there's a way in
        cave.nests = if mouth { caverns } else { Vec::new() };
        cave.lairs = lairs;
        cave.shapes.extend(inside);
        for s in &cave.shapes {
            cave.bounds = [cave.bounds[0].min(s.min[0]), cave.bounds[1].min(s.min[1]), cave.bounds[2].max(s.max[0]), cave.bounds[3].max(s.max[1])];
        }
        cave.seed = rng.0;
        Some(cave)
    }

    /// Hollows a cavern out of the ground with its floor round `floor`, in meters: wide and high,
    /// a great one for a boss's lair, with rocks hanging from its roof and rising from its floor
    /// round the sides and crystals glowing among them. Smaller if there isn't ground enough over
    /// it, None if there isn't room at all. Gives the middle of its floor, and how far out there's
    /// room all round.
    fn cavern(&self, cave: &mut Cave, inside: &mut Vec<Shape>, floor: Vec3, great: bool, rng: &mut Rng) -> Option<(Vec3, f32)> {
        // How far out its sides are from the middle, and how high its roof is over that
        let (mut wide, mut high) = if great { (rng.range(11.0, 14.0), rng.range(6.0, 7.5)) } else { (rng.range(5.0, 8.5), rng.range(3.5, 5.0)) };
        let stretch = rng.range(0.75, 1.0);
        let lengthwise = rng.next() < 0.5;
        let turn = rng.next() * TAU;
        // The roof stays well under the ground all round
        let lowest_ground = |wide: f32| {
            (0..9)
                .map(|k| {
                    let a = k as f32 / 8.0 * TAU;
                    let r = if k == 8 { 0.0 } else { wide };
                    height_at(&self.settings, floor[0] + a.cos() * r, floor[1] + a.sin() * r)
                })
                .fold(f32::MAX, f32::min)
        };
        for _ in 0..3 {
            if floor[2] + high * 1.8 + 3.0 < lowest_ground(wide) {
                break;
            }
            wide *= 0.75;
            high *= 0.8;
        }
        if floor[2] + high * 1.8 + 3.0 >= lowest_ground(wide) || floor[2] < sea_level_m(&self.settings) {
            return None;
        }
        // The middle a little over the floor, so it dips only a little and is fairly flat
        let middle = [floor[0], floor[1], floor[2] + high * 0.8];
        let radii = if lengthwise { [wide, wide * stretch, high] } else { [wide * stretch, wide, high] };
        // Its walls and roof bulging in and hollowed out, its floor fairly flat
        let bottom = floor[2] - 0.3;
        cave.shapes.push(Shape::ellipsoid(middle, radii, AIR, AIR).placed(Placement::Carve).roughened(0.6 + high * 0.35, bottom));
        // Room to fight in is as far out as its narrower side
        let wide = wide * stretch;
        // Lobes off its sides, so it isn't a perfect bubble
        for k in 0..if great { 3 } else { 2 } {
            let a = turn + k as f32 * 2.1 + rng.range(-0.4, 0.4);
            let off = wide * rng.range(0.45, 0.65);
            let center = [middle[0] + a.cos() * off, middle[1] + a.sin() * off, middle[2] - high * rng.range(0.1, 0.3)];
            let w = wide * rng.range(0.4, 0.55);
            cave.shapes.push(Shape::ellipsoid(center, [w, w, high * rng.range(0.55, 0.8)], AIR, AIR).placed(Placement::Carve).roughened(0.4 + w * 0.25, bottom));
        }
        // How high the roof and floor are over the middle of the cavern, `r` of the way out to its side
        let roof = |r: f32| middle[2] + high * (1.0 - r * r).max(0.0).sqrt();
        let ground = |r: f32| middle[2] - high * (1.0 - r * r).max(0.0).sqrt();
        // Rocks round the sides, the middle left clear to fight in
        for _ in 0..if great { 14 } else { 7 } {
            let a = rng.next() * TAU;
            let r = rng.range(0.55, 0.85);
            let (x, y) = (middle[0] + a.cos() * wide * r, middle[1] + a.sin() * wide * r);
            let thick = rng.range(0.35, 0.8) * if great { 1.4 } else { 1.0 };
            let (top, bottom) = (roof(r), ground(r).max(floor[2] - 0.6));
            if rng.next() < 0.5 {
                let tall = rng.range(0.25, 0.5) * (top - bottom);
                inside.push(Shape::capsule([x, y, bottom - 0.5], [x, y, bottom + tall], thick, 0.06, LIMESTONE, STONE, false).placed(Placement::Inside));
            } else {
                let long = rng.range(0.2, 0.4) * (top - bottom);
                inside.push(Shape::capsule([x, y, top + 0.5], [x, y, top - long], thick, 0.05, LIMESTONE, STONE, false).placed(Placement::Inside));
            }
        }
        // Crystals glowing on the floor round the sides, and lighting it
        let colors = [[0.35, 0.85, 1.0], [0.7, 0.45, 1.0], [0.4, 1.0, 0.7]];
        let tint = colors[(rng.next() * 3.0) as usize % 3];
        for _ in 0..if great { 4 } else { 2 } {
            let a = rng.next() * TAU;
            let r = rng.range(0.5, 0.75);
            let at = [middle[0] + a.cos() * wide * r, middle[1] + a.sin() * wide * r, ground(r).max(floor[2] - 0.4)];
            for _ in 0..5 {
                let offset = [rng.range(-0.6, 0.6), rng.range(-0.6, 0.6), rng.range(0.0, 0.5)];
                inside.push(Shape::sphere(add(at, offset), rng.range(0.2, 0.45), CRYSTAL, AIR, false).placed(Placement::Inside));
            }
            cave.lights.push(Light { position: [at[0], at[1], at[2] + 0.8], color: tint, reach: if great { 14.0 } else { 10.0 } });
        }
        // A great cluster of crystals hanging from the middle of the roof, lighting the whole of
        // it, so what's in there can be seen to fight
        let top = roof(0.0);
        let hang = if great { 2.2 } else { 1.2 };
        inside.push(Shape::capsule([middle[0], middle[1], top + 0.5], [middle[0], middle[1], top - hang], hang * 0.45, 0.15, CRYSTAL, AIR, false).placed(Placement::Inside));
        for _ in 0..if great { 6 } else { 3 } {
            let a = rng.next() * TAU;
            let tip = [middle[0] + a.cos() * hang * 0.6, middle[1] + a.sin() * hang * 0.6, top - hang * rng.range(0.4, 0.8)];
            inside.push(Shape::capsule([middle[0], middle[1], top + 0.3], tip, hang * 0.22, 0.08, CRYSTAL, AIR, false).placed(Placement::Inside));
        }
        let glow = [tint[0] * 0.6 + 0.4, tint[1] * 0.6 + 0.4, tint[2] * 0.6 + 0.4];
        cave.lights.push(Light { position: [middle[0], middle[1], top - hang - 0.8], color: glow, reach: if great { 28.0 } else { 15.0 } });
        Some((floor, wide))
    }

    /// The dungeons reaching into this piece of the landscape
    fn dungeons(&self, cache: &RiverCache) -> Vec<Dungeon> {
        let (lo, hi) = (self.lo, self.hi);
        let reach = DUNGEON_REACH + 2.0;
        let first = [((lo[0] - reach) / DUNGEON_CELL).floor() as i32, ((lo[1] - reach) / DUNGEON_CELL).floor() as i32];
        let last = [((hi[0] + reach) / DUNGEON_CELL).floor() as i32, ((hi[1] + reach) / DUNGEON_CELL).floor() as i32];
        let mut dungeons = Vec::new();
        for cy in first[1]..=last[1] {
            for cx in first[0]..=last[0] {
                if let Some(d) = self.dungeon_in(cx, cy, cache)
                    && d.world_rects().iter().any(|r| overlaps(*r, [lo[0], lo[1], hi[0], hi[1]], 0.0))
                {
                    dungeons.push(d);
                }
            }
        }
        dungeons
    }

    /// The dungeon in one DUNGEON_CELL square, if there's room for one at any of a few spots in
    /// it. They're well away from its edges, so dungeons never meet.
    fn dungeon_in(&self, cx: i32, cy: i32, cache: &RiverCache) -> Option<Dungeon> {
        let r = hash3(cx as u32, cy as u32, self.settings.seed ^ 0xd07e);
        if unit(r) > 0.9 {
            return None;
        }
        (0..20u32).find_map(|attempt| {
            let s = hash3(r, attempt, 0x5eed);
            let x = snap((cx as f32 + 0.2 + 0.6 * unit(s)) * DUNGEON_CELL);
            let y = snap((cy as f32 + 0.2 + 0.6 * unit(s >> 16)) * DUNGEON_CELL);
            self.plan_dungeon(x, y, hash(s), cache)
        })
    }

    /// Lays out a dungeon with the doorway of its crypt at `x`, `y`, if it fits: on fairly flat,
    /// dry ground, with no rivers or ravines near any of it. Halls lead one to the next, each a
    /// little deeper, down to the boss's, with a hall or two off to the sides.
    fn plan_dungeon(&self, x: f32, y: f32, seed: u32, cache: &RiverCache) -> Option<Dungeon> {
        let settings = &self.settings;
        let ground = |x: f32, y: f32| height_at(settings, x, y);
        // The doorway faces downhill, and the stairs go down into the hill
        let downhill = [[1.0f32, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]]
            .into_iter()
            .min_by(|a, b| ground(x + a[0] * 8.0, y + a[1] * 8.0).total_cmp(&ground(x + b[0] * 8.0, y + b[1] * 8.0)))
            .unwrap();
        let at = Frame { x, y, f: [-downhill[0], -downhill[1]] };
        // The landing in front of it is as high as the ground gets there, with as many steps
        // down from it as it takes to reach the ground in front and to the sides
        let (mut lowest, mut highest) = (f32::MAX, f32::MIN);
        for i in 0..45 {
            let p = at.point(-10.0 + 2.5 * (i % 5) as f32, -10.0 + 2.5 * (i / 5) as f32, 0.0);
            let g = ground(p[0], p[1]);
            (lowest, highest) = (lowest.min(g), highest.max(g));
        }
        if lowest < sea_level_m(settings) + 2.0 || highest - lowest > 3.0 {
            return None;
        }
        let top = (highest / STAIR_RISE).ceil() * STAIR_RISE;
        let steps = (((top - lowest) / STAIR_RISE).ceil() as usize).max(2);
        // Of the stone of the land round it
        let c = climate(settings, x, y);
        let (stone, paving) = if c.desert > 0.5 {
            ((SANDSTONE, SANDSTONE_DARK), (SANDSTONE_DARK, SANDSTONE))
        } else if c.jungle > 0.5 {
            ((TEMPLE_STONE, MOSS), (TEMPLE_DARK, TEMPLE_STONE))
        } else if c.snow > 0.5 {
            ((BRICK_STONE, LIMESTONE), (LIMESTONE, BRICK_STONE))
        } else {
            ((BRICK_STONE, MOSS), (LIMESTONE, BRICK_STONE))
        };
        let mut rng = Rng(seed);
        let mut d = Dungeon {
            at,
            top,
            steps,
            stone,
            paving,
            snowy: c.snow > 0.5,
            vines: c.desert <= 0.5 && c.snow <= 0.5,
            halls: Vec::new(),
            passages: Vec::new(),
            seed: hash(seed ^ 0xd0e6),
        };
        let boss_depth = rng.range(11.5, 13.5);
        let mains = 2 + (rng.next() * 3.0) as usize;
        let sides = 1 + (rng.next() * 2.0) as usize;

        // The first hall, at the foot of the stairs down, which are under the roof of the crypt
        // for as long as there isn't ground enough over them
        let half = [snap(rng.range(4.5, 6.0)), snap(rng.range(4.0, 6.0))];
        let height = snap(rng.range(4.5, 5.5));
        let (mut stairs, first) = self.fit_hall(&at, [0.0, 0.0], [1.0, 0.0], top, 0.0, 1.2, half, height, 0.0, top - 7.0, HallKind::Main, None)?;
        let parts = stairs.parts();
        let mut crypt = parts[0].0[1];
        for &(s, _, high) in &parts {
            if high + PASSAGE_HEIGHT + DUNGEON_WALL > self.lowest_ground(&at, stairs.rect(s, PASSAGE_HALF + DUNGEON_WALL)) - 0.2 {
                crypt = crypt.max(s[1]);
            }
        }
        if crypt > 11.0 {
            return None;
        }
        stairs.crypt = crypt;
        d.passages.push(stairs);
        d.halls.push(first);
        // What's taken so far, with the brick round it, and which hall each is
        let mut taken = vec![
            (d.entrance_rect(), None),
            (stairs.rect([0.0, stairs.length], PASSAGE_HALF + DUNGEON_WALL), None),
            (grown(first.rect(), DUNGEON_WALL), Some(0)),
        ];

        // On down, further into the hill or off to one side, never back towards the entrance
        for i in 1..mains {
            let side = if rng.next() < 0.5 { [0.0, 1.0] } else { [0.0, -1.0] };
            let dirs = if rng.next() < 0.6 { [[1.0, 0.0], side, [-side[0], -side[1]]] } else { [side, [1.0, 0.0], [-side[0], -side[1]]] };
            let half = [snap(rng.range(4.0, 6.0)), snap(rng.range(4.0, 6.0))];
            let height = snap(rng.range(4.5, 5.5));
            let want = top - 7.0 - (boss_depth - 7.0) * i as f32 / mains as f32;
            let parent = d.halls.len() - 1;
            if !self.grow(&mut d, &mut taken, parent, &dirs, half, height, want, HallKind::Main, &mut rng) {
                break;
            }
        }
        // The boss's hall, deepest of all, off the last hall that has room for it
        let mains = d.halls.len();
        let boss = [snap(rng.range(9.0, 10.0)), snap(rng.range(9.0, 10.0))];
        let mut found = false;
        for parent in (0..mains).rev() {
            let side = if rng.next() < 0.5 { [0.0, 1.0] } else { [0.0, -1.0] };
            let dirs = [[1.0, 0.0], side, [-side[0], -side[1]]];
            if self.grow(&mut d, &mut taken, parent, &dirs, boss, 8.0, top - boss_depth, HallKind::Boss, &mut rng) {
                found = true;
                break;
            }
        }
        if !found {
            return None;
        }
        // Halls off to the side of those on the way, at much the same depth
        for _ in 0..sides {
            let start = (rng.next() * mains as f32) as usize;
            for k in 0..mains {
                let parent = (start + k) % mains;
                let turn = (rng.next() * 4.0) as usize;
                let all = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]];
                let dirs: Vec<[f32; 2]> = (0..4).map(|j| all[(turn + j) % 4]).collect();
                let half = [snap(rng.range(4.0, 5.5)), snap(rng.range(4.0, 5.5))];
                let want = d.halls[parent].floor - snap(rng.range(0.0, 1.0));
                if self.grow(&mut d, &mut taken, parent, &dirs, half, 4.5, want, HallKind::Side, &mut rng) {
                    break;
                }
            }
        }
        if d.halls.len() < 4 {
            return None;
        }

        // Well clear of rivers and their banks, and of ravines and crevices
        let rects = d.world_rects();
        let lo = [0, 1].map(|i| rects.iter().map(|r| r[i]).fold(f32::MAX, f32::min));
        let hi = [0, 1].map(|i| rects.iter().map(|r| r[i + 2]).fold(f32::MIN, f32::max));
        for river in self.rivers_near(cache, [lo[0] - 10.0, lo[1] - 10.0], [hi[0] + 10.0, hi[1] + 10.0]) {
            let near = river.iter().any(|p| {
                rects.iter().any(|r| {
                    let (dx, dy) = ((r[0] - p.x).max(p.x - r[2]).max(0.0), (r[1] - p.y).max(p.y - r[3]).max(0.0));
                    dx.hypot(dy) < p.half_width + 6.0
                })
            });
            if near {
                return None;
            }
        }
        let first = [0, 1].map(|i| ((lo[i] - RAVINE_REACH - 2.0) / RAVINE_CELL).floor() as i32);
        let last = [0, 1].map(|i| ((hi[i] + RAVINE_REACH + 2.0) / RAVINE_CELL).ceil() as i32);
        let mut scratch = Features::default();
        for cy in first[1]..last[1] {
            for cx in first[0]..last[0] {
                if let Some(ravine) = self.ravine(cx, cy, &mut scratch)
                    && ravine.shapes.iter().any(|s| rects.iter().any(|r| overlaps(*r, [s.min[0], s.min[1], s.max[0], s.max[1]], 2.0)))
                {
                    return None;
                }
            }
        }
        Some(d)
    }

    /// The lowest the ground gets over rectangle `r`, u0, v0, u1, v1 in frame `at`, sampled every
    /// couple of meters
    fn lowest_ground(&self, at: &Frame, r: [f32; 4]) -> f32 {
        let n = [0, 1].map(|i| ((r[i + 2] - r[i]) / 2.5).ceil().max(1.0) as usize + 1);
        let mut lowest = f32::MAX;
        for i in 0..n[0] {
            for j in 0..n[1] {
                let u = r[0] + (r[2] - r[0]) * i as f32 / (n[0] - 1) as f32;
                let v = r[1] + (r[3] - r[1]) * j as f32 / (n[1] - 1) as f32;
                let p = at.point(u, v, 0.0);
                lowest = lowest.min(height_at(&self.settings, p[0], p[1]));
            }
        }
        lowest
    }

    /// Fits a hall `half` the size along u and v and `height` high, in frame `at`, at the end of a
    /// passage from `start` going `dir` from a floor at `from`, at least `gap` long, with flat floor
    /// at least `flat` long at either end of its stairs. The hall's middle is `offset` across from
    /// the passage's line. Its floor is at most at `want`, and deep enough for DUNGEON_COVER of
    /// ground over it, so the passage gets longer if it has further to go down. None if there's
    /// sea over it or the passage would be too long.
    #[allow(clippy::too_many_arguments)]
    fn fit_hall(
        &self,
        at: &Frame,
        start: [f32; 2],
        dir: [f32; 2],
        from: f32,
        gap: f32,
        flat: f32,
        half: [f32; 2],
        height: f32,
        offset: f32,
        want: f32,
        kind: HallKind,
        parent: Option<usize>,
    ) -> Option<(Passage, Hall)> {
        let sea_level = sea_level_m(&self.settings);
        let across = [-dir[1], dir[0]];
        let along = dir[0].abs() * half[0] + dir[1].abs() * half[1];
        let mut length = gap;
        let mut floor = snap_down(want.min(from));
        for _ in 0..8 {
            let reach = length + along;
            let middle = [start[0] + dir[0] * reach + across[0] * offset, start[1] + dir[1] * reach + across[1] * offset];
            let lowest = self.lowest_ground(at, grown(around(middle, half), DUNGEON_WALL + 0.5));
            if lowest < sea_level + 1.5 {
                return None;
            }
            floor = floor.min(snap_down(lowest - DUNGEON_COVER - DUNGEON_WALL - height));
            let drops = ((from - floor) / STAIR_RISE).round();
            let needed = (drops - 1.0).max(0.0) * STAIR_RUN + 2.0 * flat;
            if needed <= length + 1e-3 {
                if floor < 2.0 {
                    return None;
                }
                let passage = Passage { start, dir, length, from, to: floor, crypt: 0.0 };
                let hall = Hall { middle, half, floor, height, door: [-dir[0], -dir[1]], kind, parent };
                return Some((passage, hall));
            }
            length = (needed / STAIR_RISE).ceil() * STAIR_RISE;
            if length > 20.0 {
                return None;
            }
        }
        None
    }

    /// Adds a hall `half` the size along u and v and `height` high to dungeon `d`, with a passage
    /// to it from hall `parent`, trying each way of `dirs` in turn till it fits clear of what's
    /// `taken`: rectangles in its frame, brick and all, and which hall each is. Its floor is at
    /// most at `want`. Returns whether it fitted.
    #[allow(clippy::too_many_arguments)]
    fn grow(
        &self,
        d: &mut Dungeon,
        taken: &mut Vec<([f32; 4], Option<usize>)>,
        parent: usize,
        dirs: &[[f32; 2]],
        half: [f32; 2],
        height: f32,
        want: f32,
        kind: HallKind,
        rng: &mut Rng,
    ) -> bool {
        let sea_level = sea_level_m(&self.settings);
        let within = |r: [f32; 4]| r.iter().all(|v| v.abs() <= DUNGEON_REACH);
        let jitter = |room: f32, rng: &mut Rng| if room > 0.0 { snap(rng.range(-room, room)) } else { 0.0 };
        for &dir in dirs {
            let p = d.halls[parent];
            if dir == p.door {
                continue;
            }
            // Where across the parent's wall the passage leaves it, and how far across from the
            // passage's line the new hall's middle is. The boss's doorway is near the middle.
            let across = [-dir[1], dir[0]];
            let out = jitter(p.reach(across) - PASSAGE_HALF - 1.0, rng);
            let room = if kind == HallKind::Boss { 2.0 } else { across[0].abs() * half[0] + across[1].abs() * half[1] - PASSAGE_HALF - 1.0 };
            let offset = jitter(room, rng);
            let along = p.reach(dir);
            let start = [p.middle[0] + dir[0] * along + across[0] * out, p.middle[1] + dir[1] * along + across[1] * out];
            let gap = snap(rng.range(4.0, 7.0));
            let Some((passage, hall)) = self.fit_hall(&d.at, start, dir, p.floor, gap, 1.0, half, height, offset, want, kind, Some(parent)) else {
                continue;
            };
            let hall_rect = grown(hall.rect(), DUNGEON_WALL);
            let passage_rect = passage.rect([0.0, passage.length], PASSAGE_HALF + DUNGEON_WALL);
            if !within(hall_rect) || !within(passage_rect) {
                continue;
            }
            if taken.iter().any(|(t, _)| overlaps(*t, hall_rect, 1.0)) || taken.iter().any(|(t, hall)| *hall != Some(parent) && overlaps(*t, passage_rect, 0.3)) {
                continue;
            }
            // With ground enough over the passage too, and no sea
            let covered = passage.parts().iter().all(|&(s, _, high)| {
                let lowest = self.lowest_ground(&d.at, passage.rect(s, PASSAGE_HALF + DUNGEON_WALL));
                lowest > sea_level + 1.5 && high + PASSAGE_HEIGHT + DUNGEON_WALL + 1.0 <= lowest
            });
            if !covered {
                continue;
            }
            taken.push((hall_rect, Some(d.halls.len())));
            taken.push((passage_rect, None));
            d.halls.push(hall);
            d.passages.push(passage);
            return true;
        }
        false
    }

    /// The shapes of dungeon `d`, with its candles, skulls and chests, its halls as sites, and
    /// where its entrance stands in `features`
    fn dungeon(&self, d: &Dungeon, features: &mut Features) -> Object {
        let at = d.at;
        let mut rng = Rng(d.seed);
        // What stands in the halls and passages goes first, then the space inside them, then the
        // paving and the brick round it all, so each wins over what comes after
        let (mut solids, mut rooms, mut floors, mut walls) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        self.dungeon_entrance(d, &mut solids, &mut rooms, &mut walls, features, &mut rng);

        for (i, p) in d.passages.iter().enumerate() {
            let entrance = i == 0;
            let (drops, first) = p.stairs();
            for k in 1..drops {
                let s = [first + (k - 1) as f32 * STAIR_RUN, first + k as f32 * STAIR_RUN];
                solids.push(stone_block(&at, p.rect(s, PASSAGE_HALF), [p.to - DUNGEON_WALL, p.from - k as f32 * STAIR_RISE], d.paving));
            }
            let parts = p.parts();
            let last = parts.len() - 1;
            for (n, &(s, low, high)) in parts.iter().enumerate() {
                let ceiling = if s[0] < p.crypt - 1e-3 { d.top + CRYPT_HEIGHT } else { high + PASSAGE_HEIGHT };
                // Through the walls of the halls at either end
                let inside = [if n == 0 && !entrance { -0.05 } else { s[0] }, if n == last { s[1] + 0.05 } else { s[1] }];
                rooms.push(hollow(&at, p.rect(inside, PASSAGE_HALF), [low, ceiling]));
                floors.push(stone_block(&at, p.rect(s, PASSAGE_HALF), [low - DUNGEON_WALL, low], d.paving));
                // A little past either end, to line the wall where the ceiling steps down. That's
                // the front wall of the crypt at the top of the stairs down.
                let outside = [(s[0] - DUNGEON_WALL).max(if entrance { -DUNGEON_WALL } else { 0.0 }), (s[1] + DUNGEON_WALL).min(p.length)];
                walls.push(stone_block(&at, p.rect(outside, PASSAGE_HALF + DUNGEON_WALL), [low - DUNGEON_WALL, ceiling + DUNGEON_WALL], d.stone));
            }
            // Candles every few meters along alternate walls, and a skull or so
            let mut spots = if entrance { vec![(0.6, 1.0), (0.6, -1.0)] } else { Vec::new() };
            let mut s = if entrance { 4.0 } else { 1.2 };
            let mut side = if rng.next() < 0.5 { 1.0 } else { -1.0 };
            while s < p.length - 0.8 {
                spots.push((s, side));
                side = -side;
                s += 3.6;
            }
            for (s, side) in spots {
                let s = p.on_step(s);
                let q = p.at(s, side * (PASSAGE_HALF - 0.2));
                self.dungeon_prop(features, PropKind::Candle, at.point(q[0], q[1], p.floor_at(s)));
            }
            if rng.next() < 0.7 {
                let s = p.on_step(rng.range(1.0, p.length - 1.0));
                let side = (if rng.next() < 0.5 { 1.0 } else { -1.0 }) * (PASSAGE_HALF - 0.35);
                let q = p.at(s, side);
                self.dungeon_prop(features, PropKind::Skull, at.point(q[0], q[1], p.floor_at(s)));
            }
        }

        for (i, h) in d.halls.iter().enumerate() {
            let (r, f) = (h.rect(), h.floor);
            rooms.push(hollow(&at, r, [f, f + h.height]));
            floors.push(stone_block(&at, grown(r, DUNGEON_WALL), [f - DUNGEON_WALL, f], d.paving));
            walls.push(stone_block(&at, grown(r, DUNGEON_WALL), [f - DUNGEON_WALL, f + h.height + DUNGEON_WALL], d.stone));
            // Its doorways: where the passage in reaches it, and where those out of it leave
            let mut doors = vec![d.passages[i].at(d.passages[i].length, 0.0)];
            doors.extend((0..d.halls.len()).filter(|&j| d.halls[j].parent == Some(i)).map(|j| d.passages[j].at(0.0, 0.0)));
            self.furnish(d, h, &doors, &mut solids, features, &mut rng);
            let middle = at.point(h.middle[0], h.middle[1], f);
            if self.inside(middle[0], middle[1]) {
                let kind = if h.kind == HallKind::Boss { "dungeon" } else { "dungeon hall" };
                features.sites.push(Site { kind, position: middle, facing: at.dir(h.door) });
            }
        }

        // Nothing grows on the entrance, and no grass on the landing or the crypt
        let e = d.entrance_rect();
        let (lo, hi) = at.rect([e[0], e[2]], [e[1], e[3]]);
        features.footprints.push([lo[0], lo[1], hi[0], hi[1], d.top + 7.6]);
        let plinth = d.landing(d.steps);
        let (lo, hi) = at.rect([plinth[0], d.passages[0].crypt + DUNGEON_WALL], [plinth[1], plinth[3]]);
        features.holes.push([lo[0], lo[1], hi[0], hi[1]]);

        let mut shapes = solids;
        shapes.extend(rooms);
        shapes.extend(floors);
        shapes.extend(walls);
        Object::new(shapes, d.seed)
    }

    /// The way into dungeon `d`: a crypt of brick over the top of the stairs down, its roof broken
    /// in, its doorway between two tall pillars under a broken arch, skulls on its roof, a landing
    /// in front with steps down to the ground, and rubble lying about
    #[allow(clippy::too_many_arguments)]
    fn dungeon_entrance(&self, d: &Dungeon, solids: &mut Vec<Shape>, rooms: &mut Vec<Shape>, walls: &mut Vec<Shape>, features: &mut Features, rng: &mut Rng) {
        let at = d.at;
        let top = d.top;
        let crypt = d.passages[0].crypt;
        let outer = PASSAGE_HALF + DUNGEON_WALL;
        let roof = top + CRYPT_HEIGHT + DUNGEON_WALL;

        // The doorway through the front wall, and the landing, with steps down to the ground on
        // three sides that sink into it where it's higher, and nothing in the way above it
        rooms.push(hollow(&at, [-DUNGEON_WALL - 0.05, -PASSAGE_HALF, 0.05, PASSAGE_HALF], [top, top + 3.0]));
        for k in 0..=d.steps {
            let z = top - k as f32 * STAIR_RISE;
            solids.push(stone_block(&at, d.landing(k), [z - 1.0, z], d.paving));
        }
        rooms.push(hollow(&at, d.landing(0), [top, top + 4.5]));

        // The pillars either side of the doorway, one of which may have fallen, taking its side of
        // the arch with it
        let (pu, pv) = (-DUNGEON_WALL - 0.75, outer + 0.6);
        let tall = roof + 0.7;
        let fallen = if rng.next() < 0.45 { if rng.next() < 0.5 { 1.0 } else { -1.0 } } else { 0.0 };
        for side in [-1.0f32, 1.0] {
            let q = [pu, side * pv];
            let height = if side == fallen { top + rng.range(1.4, 3.2) } else { tall };
            solids.push(stone_block(&at, around(q, [0.65, 0.65]), [top - 0.3, top + 0.35], d.stone));
            if side == fallen {
                self.dungeon_prop(features, PropKind::Skull, at.point(q[0], q[1], height + 0.4));
            } else {
                solids.push(stone_block(&at, around(q, [0.65, 0.65]), [tall - 0.35, tall + 0.1], d.stone));
            }
            solids.push(
                Shape::capsule(at.point(q[0], q[1], top), at.point(q[0], q[1], height), 0.45, 0.42, d.stone.0, d.stone.1, false)
                    .placed(Placement::Clear)
                    .patterned(Pattern::Carved),
            );
        }
        let gap = if fallen == 0.0 && rng.next() < 0.6 { Some(2 + (rng.next() * 5.0) as usize) } else { None };
        for k in 0..10 {
            // Round from the pillar on the +v side to the other
            let lost = fallen > 0.0 && k < 6 || fallen < 0.0 && k >= 4 || gap.is_some_and(|g| k == g || k == g + 1);
            if lost {
                continue;
            }
            let (a, b) = (k as f32 / 10.0 * std::f32::consts::PI, (k + 1) as f32 / 10.0 * std::f32::consts::PI);
            let point = |t: f32| at.point(pu, pv * t.cos(), tall + pv * t.sin());
            solids.push(Shape::capsule(point(a), point(b), 0.35, 0.35, d.stone.0, d.stone.1, false).placed(Placement::Clear).patterned(Pattern::Carved));
        }

        // Skulls on the roof over the doorway and by the door, and a hole fallen in the roof
        for v in [-0.9, 0.0, 0.9] {
            if rng.next() < 0.8 {
                self.dungeon_prop(features, PropKind::Skull, at.point(-DUNGEON_WALL * 0.5, v, roof));
            }
        }
        for side in [-1.0, 1.0] {
            let (u, v) = (-DUNGEON_WALL - rng.range(1.6, 2.4), side * rng.range(PASSAGE_HALF + 0.3, outer + 1.0));
            self.dungeon_prop(features, PropKind::Skull, at.point(u, v, top));
        }
        let side = if rng.next() < 0.5 { 1.0 } else { -1.0 };
        let bite = at.point(crypt - rng.range(0.5, 2.0), side * (outer - 0.2), roof + 0.4);
        rooms.push(Shape::sphere(bite, rng.range(0.9, 1.3), AIR, AIR, false).placed(Placement::Clear));
        if d.snowy {
            walls.push(at.block([-DUNGEON_WALL, crypt + DUNGEON_WALL], [-outer, outer], [roof, roof + 0.15], SNOW, AIR).placed(Placement::Clear));
        }
        if d.vines {
            for _ in 0..5 {
                let length = rng.range(0.8, 2.6);
                let side = if rng.next() < 0.5 { 1.0 } else { -1.0 };
                let (u, v) = if rng.next() < 0.4 {
                    (-DUNGEON_WALL - 0.07, side * rng.range(PASSAGE_HALF + 0.1, outer - 0.1))
                } else {
                    (rng.range(0.0, crypt), side * (outer + 0.07))
                };
                solids.push(Shape::capsule(at.point(u, v, roof - 0.1), at.point(u, v, roof - 0.1 - length), 0.07, 0.05, VINE, LEAVES, false));
            }
        }

        // Rubble that's fallen off it, but not in the way in
        let (plinth, area) = (d.landing(d.steps), d.entrance_rect());
        for _ in 0..8 {
            let (u, v) = (rng.range(area[0] + 0.5, area[2] - 0.5), rng.range(area[1] + 0.5, area[3] - 0.5));
            let (size, stretch) = (rng.range(0.3, 0.7), rng.range(0.7, 1.5));
            let on_it = u > plinth[0] - 0.2 && u < crypt + DUNGEON_WALL + 0.4 && v.abs() < plinth[3] + 0.2;
            if on_it || u < 0.0 && v.abs() < 2.5 {
                continue;
            }
            let p = at.point(u, v, 0.0);
            let ground = height_at(&self.settings, p[0], p[1]);
            solids.push(stone_block(&at, around([u, v], [size, size * stretch]), [ground - 0.3, ground + size * 0.7], d.stone));
        }
    }

    /// What's in hall `h` of dungeon `d`, kept clear of its doorways at `doors`: pillars, candles
    /// along the walls, skulls, rubble and maybe a stone coffin. A side hall has a chest at its far
    /// end. The boss's has rows of pillars, and a dais at the far end with an altar, braziers and
    /// the best chest of all, and heaps of bones in its corners.
    #[allow(clippy::too_many_arguments)]
    fn furnish(&self, d: &Dungeon, h: &Hall, doors: &[[f32; 2]], s: &mut Vec<Shape>, features: &mut Features, rng: &mut Rng) {
        let at = d.at;
        let (m, f) = (h.middle, h.floor);
        let ceiling = f + h.height;
        let by_door = |q: [f32; 2]| doors.iter().any(|p| (q[0] - p[0]).hypot(q[1] - p[1]) < 2.4);
        let point = |q: [f32; 2], z: f32| at.point(q[0], q[1], z);
        let flip = |rng: &mut Rng| if rng.next() < 0.5 { 1.0 } else { -1.0 };
        // Laid out `a` towards the far end from the doorway in, and `b` across
        let n = [-h.door[0], -h.door[1]];
        let t = [-n[1], n[0]];
        let (deep, wide) = (h.reach(n), h.reach(t));
        let q = |a: f32, b: f32| [m[0] + n[0] * a + t[0] * b, m[1] + n[1] * a + t[1] * b];
        let rect = |a: [f32; 2], b: [f32; 2]| {
            let (p, r) = (q(a[0], b[0]), q(a[1], b[1]));
            [p[0].min(r[0]), p[1].min(r[1]), p[0].max(r[0]), p[1].max(r[1])]
        };
        // Which way the chests' locks face: back towards the doorway in
        let heading = at.heading(h.door[1].atan2(h.door[0]));

        if h.kind == HallKind::Boss {
            for b in [-1.0, 1.0] {
                for k in 0..4 {
                    let a = -deep + 3.0 + (2.0 * deep - 7.5) * k as f32 / 3.0;
                    let broken = rng.next() < 0.15;
                    pillar(s, &at, q(a, b * (wide - 2.0)), f, ceiling, 0.55, d.stone, broken, rng);
                }
            }
            s.push(stone_block(&at, rect([deep - 3.5, deep], [-4.0, 4.0]), [f - 0.1, f + STAIR_RISE], d.paving));
            s.push(stone_block(&at, rect([deep - 2.2, deep], [-2.6, 2.6]), [f - 0.1, f + 2.0 * STAIR_RISE], d.paving));
            s.push(stone_block(&at, rect([deep - 1.1, deep - 0.2], [-1.2, 1.2]), [f, f + 1.3], d.stone).patterned(Pattern::Carved));
            let spot = point(q(deep - 1.65, 0.0), f + 2.0 * STAIR_RISE);
            if self.inside(spot[0], spot[1]) {
                chest(features, spot, heading, 2.6);
            }
            for b in [-0.8, 0.8] {
                self.dungeon_prop(features, PropKind::Candle, point(q(deep - 0.65, b), f + 1.3));
            }
            self.dungeon_prop(features, PropKind::Skull, point(q(deep - 0.65, 0.0), f + 1.3));
            for b in [-3.3, 3.3] {
                let c = q(deep - 2.9, b);
                s.push(stone_block(&at, around(c, [0.35, 0.35]), [f, f + 1.1], d.stone));
                s.push(stone_block(&at, around(c, [0.5, 0.5]), [f + 1.1, f + 1.25], d.stone));
                s.push(Shape::sphere(point(c, f + 1.4), 0.25, FLAME, AIR, false).placed(Placement::Clear));
                let flame = point(c, f + 1.9);
                if self.inside(flame[0], flame[1]) {
                    features.lights.push(Light { position: flame, color: [1.0, 0.5, 0.2], reach: 11.0 });
                }
            }
            // Candles down both sides between the pillars and the walls, and either side of the doorway
            for b in [-1.0, 1.0] {
                for frac in [-0.8, -0.4, 0.0, 0.4, 0.8] {
                    let c = q(frac * deep, b * (wide - 0.3));
                    if !by_door(c) && rng.next() < 0.85 {
                        self.dungeon_prop(features, PropKind::Candle, point(c, f));
                    }
                }
            }
            for b in [-0.75, -0.4, 0.4, 0.75] {
                let c = q(-deep + 0.3, b * wide);
                if !by_door(c) {
                    self.dungeon_prop(features, PropKind::Candle, point(c, f));
                }
            }
            for b in [-0.75, 0.75] {
                self.dungeon_prop(features, PropKind::Candle, point(q(deep - 0.3, b * wide), f));
            }
            // Skulls all over, though not in the middle where the boss waits, and heaps of bones
            for _ in 0..10 + (rng.next() * 5.0) as usize {
                let c = q(rng.range(-deep + 1.0, deep - 4.0), rng.range(-wide + 1.0, wide - 1.0));
                if (c[0] - m[0]).hypot(c[1] - m[1]) > 4.5 && !by_door(c) {
                    self.dungeon_prop(features, PropKind::Skull, point(c, f));
                }
            }
            for (sa, sb) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
                let c = q(sa * (deep - 1.3), sb * (wide - 1.3));
                if rng.next() < 0.3 || by_door(c) {
                    continue;
                }
                s.push(Shape::ellipsoid(point(c, f), [0.8, 0.8, 0.35], BONE, AIR).placed(Placement::Clear));
                for _ in 0..2 {
                    let p = [c[0] + rng.range(-0.3, 0.3), c[1] + rng.range(-0.3, 0.3)];
                    self.dungeon_prop(features, PropKind::Skull, point(p, f + 0.25));
                }
            }
            for _ in 0..4 {
                let c = q(rng.range(-deep + 1.0, deep - 4.0), flip(rng) * (wide - rng.range(0.6, 1.2)));
                let (size, stretch) = (rng.range(0.25, 0.5), rng.range(0.7, 1.4));
                if !by_door(c) {
                    s.push(stone_block(&at, around(c, [size, size * stretch]), [f - 0.05, f + size * 0.8], d.stone));
                }
            }
            return;
        }

        // Pillars holding up the roof of the bigger halls, some fallen, and square ones in the
        // corners of the rest. They're well out from the middle, where the monsters gather.
        if h.half[0] >= 5.0 && h.half[1] >= 5.0 && rng.next() < 0.7 {
            for (sa, sb) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let broken = rng.next() < 0.3;
                pillar(s, &at, q(sa * (deep - 1.8), sb * (wide - 1.8)), f, ceiling, 0.4, d.stone, broken, rng);
            }
        } else {
            for (sa, sb) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                s.push(stone_block(&at, around(q(sa * (deep - 0.35), sb * (wide - 0.35)), [0.35, 0.35]), [f - 0.1, ceiling], d.stone));
            }
        }

        // The walls, which way out each faces in a and b, the one with the doorway in last.
        // Whatever stands against the middle of one keeps candles off it.
        let walls: [(f32, f32); 4] = [(1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (-1.0, 0.0)];
        let mut taken: Vec<(f32, f32)> = Vec::new();
        // A side hall's chest, at the far end facing the way in, between two candles
        if h.kind == HallKind::Side {
            let spot = point(q(deep - 0.6, 0.0), f);
            if self.inside(spot[0], spot[1]) {
                chest(features, spot, heading, 1.7);
            }
            for b in [-1.0, 1.0] {
                self.dungeon_prop(features, PropKind::Candle, point(q(deep - 0.35, b), f));
            }
            self.dungeon_prop(features, PropKind::Skull, point(q(deep - 0.5, 1.6 * flip(rng)), f));
            taken.push(walls[0]);
        }
        // A stone coffin against a wall without a doorway, far enough out to leave the middle
        // clear, with a lid of paving, candles at either end and a skull on top
        if rng.next() < 0.5 {
            let turn = (rng.next() * 3.0) as usize;
            for k in 0..3 {
                let (wa, wb) = walls[(turn + k) % 3];
                let out = if wa != 0.0 { deep } else { wide };
                if out < 5.25 || taken.contains(&(wa, wb)) {
                    continue;
                }
                let (ca, cb) = (wa * (out - 0.75), wb * (out - 0.75));
                // Long along the wall
                let (la, lb) = (wb.abs(), wa.abs());
                let ends = [q(ca + la * 1.2, cb + lb * 1.2), q(ca - la * 1.2, cb - lb * 1.2)];
                if by_door(q(ca, cb)) || ends.iter().any(|&e| by_door(e)) {
                    continue;
                }
                let (ha, hb) = (wa.abs() * 0.5 + la * 1.1, wb.abs() * 0.5 + lb * 1.1);
                let body = rect([ca - ha, ca + ha], [cb - hb, cb + hb]);
                s.push(stone_block(&at, body, [f - 0.1, f + 0.8], d.stone));
                s.push(stone_block(&at, grown(body, 0.06), [f + 0.8, f + 0.95], d.paving));
                for e in [1.0, -1.0] {
                    self.dungeon_prop(features, PropKind::Candle, point(q(ca + la * 0.85 * e, cb + lb * 0.85 * e), f + 0.95));
                }
                self.dungeon_prop(features, PropKind::Skull, point(q(ca, cb), f + 0.95));
                taken.push((wa, wb));
                break;
            }
        }
        // Candles along the walls
        for (wa, wb) in walls {
            for frac in [-0.6, 0.0, 0.6] {
                let c = if wa != 0.0 { q(wa * (deep - 0.3), frac * wide) } else { q(frac * deep, wb * (wide - 0.3)) };
                if by_door(c) || frac == 0.0 && taken.contains(&(wa, wb)) || rng.next() > 0.75 {
                    continue;
                }
                self.dungeon_prop(features, PropKind::Candle, point(c, f));
            }
        }
        // Skulls by the walls, and rubble in the corners
        for _ in 0..2 + (rng.next() * 4.0) as usize {
            let (inset, along) = (rng.range(0.35, 1.2), rng.range(-0.8, 0.8));
            let c = match (rng.next() * 4.0) as usize {
                0 => q(deep - inset, along * wide),
                1 => q(-deep + inset, along * wide),
                2 => q(along * deep, wide - inset),
                _ => q(along * deep, -wide + inset),
            };
            if !by_door(c) {
                self.dungeon_prop(features, PropKind::Skull, point(c, f));
            }
        }
        for _ in 0..2 + (rng.next() * 3.0) as usize {
            let c = q(flip(rng) * (deep - rng.range(0.9, 1.8)), flip(rng) * (wide - rng.range(0.9, 1.8)));
            let (size, stretch) = (rng.range(0.2, 0.45), rng.range(0.7, 1.4));
            if !by_door(c) {
                s.push(stone_block(&at, around(c, [size, size * stretch]), [f - 0.05, f + size * 0.8], d.stone));
            }
        }
    }

    /// A candle or skull of a dungeon at `p`, if that's in this piece of the landscape, turned the
    /// way the spot it's on says, so it's the same whichever piece it's in
    fn dungeon_prop(&self, features: &mut Features, kind: PropKind, p: Vec3) {
        if !self.inside(p[0], p[1]) {
            return;
        }
        let mut rng = Rng(hash3((p[0] * 16.0) as i32 as u32, (p[1] * 16.0) as i32 as u32, (p[2] * 16.0) as i32 as u32 ^ 0x5c11));
        if kind == PropKind::Candle {
            candle(features, p, &mut rng);
        } else {
            skull(features, p, &mut rng);
        }
    }

    /// A cactus with a few arms bending up, sometimes in flower
    fn cactus(&self, x: f32, y: f32, ground: f32, rng: &mut Rng) -> Vec<Shape> {
        let height = rng.range(1.6, 3.6);
        let r = rng.range(0.18, 0.28);
        let mut shapes = Vec::new();
        if rng.next() < 0.3 {
            shapes.push(Shape::sphere([x, y, ground + height + r * 0.6], r * 0.6, FLOWER_PINK, FLOWER_YELLOW, false));
        }
        for _ in 0..(rng.next() * 3.0) as usize {
            let angle = rng.next() * TAU;
            let elbow_height = ground + height * rng.range(0.35, 0.65);
            let out = rng.range(0.5, 0.8);
            let elbow = [x + angle.cos() * out, y + angle.sin() * out, elbow_height];
            shapes.push(Shape::capsule([x, y, elbow_height], elbow, r * 0.75, r * 0.7, CACTUS, AIR, false));
            shapes.push(Shape::capsule(elbow, [elbow[0], elbow[1], elbow_height + rng.range(0.5, 1.2)], r * 0.7, r * 0.6, CACTUS, AIR, false));
        }
        shapes.push(Shape::capsule([x, y, ground - 0.3], [x, y, ground + height], r, r * 0.9, CACTUS, AIR, false));
        shapes
    }

    // A trunk splitting into limbs that keep forking, with leaves on every twig
    #[allow(clippy::too_many_arguments)]
    fn broadleaf(&self, x: f32, y: f32, base: f32, size: f32, depth: u32, bark: u8, foliage: Foliage, rng: &mut Rng) -> Vec<Shape> {
        let height = rng.range(2.5, 4.5) * size;
        let trunk_radius = rng.range(0.22, 0.38) * size;
        let reach = rng.range(2.8, 4.0) * size;

        let lean = rng.range(0.0, 0.5);
        let lean_angle = rng.range(0.0, TAU);
        let bottom = [x, y, base - 0.6];
        let top = [x + lean_angle.cos() * lean, y + lean_angle.sin() * lean, base + height];

        let mut wood = vec![Shape::wood(bottom, top, trunk_radius * 1.3, trunk_radius * 0.8, bark)];
        let mut leaves = Vec::new();

        let limbs = 3 + (rng.next() * 2.99) as usize;
        let spin = rng.range(0.0, TAU);
        for i in 0..limbs {
            let angle = spin + (i as f32 + rng.range(-0.25, 0.25)) / limbs as f32 * TAU;
            let dir = normalize([angle.cos(), angle.sin(), rng.range(0.45, 1.0)]);
            let start = lerp3(bottom, top, rng.range(0.8, 1.0));
            self.branch(&mut wood, &mut leaves, start, dir, reach * 0.55, trunk_radius * 0.7, depth, bark, foliage, rng);
        }
        let leader = bend([0.0, 0.0, 1.0], rng.range(0.0, 0.3), rng.range(0.0, TAU));
        self.branch(&mut wood, &mut leaves, top, leader, reach * 0.45, trunk_radius * 0.6, depth - 1, bark, foliage, rng);

        wood.append(&mut leaves);
        wood
    }

    #[allow(clippy::too_many_arguments)]
    fn branch(
        &self,
        wood: &mut Vec<Shape>,
        leaves: &mut Vec<Shape>,
        start: Vec3,
        dir: Vec3,
        length: f32,
        radius: f32,
        depth: u32,
        bark: u8,
        foliage: Foliage,
        rng: &mut Rng,
    ) {
        let end = add(start, scale(dir, length));
        wood.push(Shape::wood(start, end, radius, radius * 0.65, bark));

        if depth == 0 {
            self.twig_leaves(leaves, end, dir, length, bark, foliage, rng);
            return;
        }

        let forks = if rng.next() < 0.35 { 3 } else { 2 };
        for _ in 0..forks {
            // Spread out, but keep reaching for the light
            let mut next = bend(dir, rng.range(0.35, 0.8), rng.range(0.0, TAU));
            next[2] += 0.25;
            let from = lerp3(start, end, rng.range(0.6, 1.0));
            self.branch(wood, leaves, from, normalize(next), length * 0.65, radius * 0.6, depth - 1, bark, foliage, rng);
        }
    }

    // Twiglets fanning out from the end of a twig going `dir`, with leaves along them in pairs
    fn twig_leaves(&self, out: &mut Vec<Shape>, end: Vec3, dir: Vec3, length: f32, bark: u8, foliage: Foliage, rng: &mut Rng) {
        let spread = length * rng.range(1.2, 1.6);
        self.leaf_cluster(out, end, dir, spread, bark, foliage, rng);
    }

    /// Leaves you can tell apart, with sky showing between them: twiglets fanning out from `at`
    /// about `reach` meters, leaning `dir`, each with leaves along it in pairs and one at its tip,
    /// every leaf one shade or another. A small dark clump in the middle keeps the crown from
    /// looking see-through from afar.
    #[allow(clippy::too_many_arguments)]
    fn leaf_cluster(&self, out: &mut Vec<Shape>, at: Vec3, dir: Vec3, reach: f32, bark: u8, foliage: Foliage, rng: &mut Rng) {
        if foliage.material == AIR {
            return;
        }
        for _ in 0..foliage.clumps_per_twig.min(1) {
            out.push(Shape::sphere(add(at, scale(rng.direction(0.3), reach * 0.15)), reach * rng.range(0.38, 0.48), darker(foliage.material), foliage.material, true));
        }
        let twiglets = 3 + foliage.leaves_per_twig / 6 + (rng.next() * 2.0) as usize;
        for _ in 0..twiglets {
            let mut d = add(rng.direction(0.0), scale(dir, 0.7));
            d[2] += 0.2;
            let d = normalize(d);
            let tip = add(at, scale(d, reach * rng.range(0.55, 1.0)));
            out.push(Shape::wood(at, tip, 0.025, 0.015, bark));
            let mut side = cross(d, [0.0, 0.0, 1.0]);
            if dot(side, side) < 1e-4 {
                side = [1.0, 0.0, 0.0];
            }
            let side = normalize(side);
            // Leaves all along it, however long it is
            let pairs = ((reach / (foliage.leaf_length * 1.2)) as usize).clamp(2, 6) + (rng.next() * 1.5) as usize;
            for k in 0..=pairs {
                let t = (k as f32 + 1.0) / (pairs as f32 + 1.0);
                let stem = lerp3(at, tip, t);
                // A pair along the twiglet, then one off its tip
                let sides: &[f32] = if k == pairs { &[0.0] } else { &[-1.0, 1.0] };
                for &s in sides {
                    let leaf_dir = normalize(add(add(scale(side, s), scale(d, if s == 0.0 { 1.0 } else { 0.6 })), [0.0, 0.0, -0.25]));
                    let length = foliage.leaf_length * rng.range(0.85, 1.2);
                    let shade = if rng.next() < 0.68 { foliage.material } else { foliage.accent };
                    out.push(Shape::leaf(add(stem, scale(leaf_dir, length * 0.55)), leaf_dir, [0.0, 0.0, 1.0], length, length * 0.45, shade, shade));
                }
            }
        }
    }

    // A curving trunk with a crown of long arching fronds
    fn palm(&self, x: f32, y: f32, base: f32, rng: &mut Rng) -> Vec<Shape> {
        let height = rng.range(5.0, 8.0);
        let lean = rng.range(0.8, 2.0);
        let lean_angle = rng.range(0.0, TAU);
        let lean_dir = [lean_angle.cos(), lean_angle.sin(), 0.0];

        let mut shapes = Vec::new();
        let mut previous = [x, y, base - 0.6];
        for i in 1..=6 {
            let t = i as f32 / 6.0;
            let point = add([x, y, base + height * t], scale(lean_dir, lean * t * t));
            shapes.push(Shape::capsule(previous, point, 0.2 - 0.05 * t, 0.2 - 0.05 * t, PALM_WOOD, DEAD_WOOD, false));
            previous = point;
        }
        let top = previous;
        for _ in 0..(2 + (rng.next() * 2.99) as usize) {
            shapes.push(Shape::sphere(add(top, [rng.range(-0.2, 0.2), rng.range(-0.2, 0.2), -0.25]), 0.1, DEAD_WOOD, AIR, false));
        }

        // A few old fronds gone brown, hanging down against the trunk under the green ones
        for _ in 0..(1 + (rng.next() * 2.99) as usize) {
            let angle = rng.range(0.0, TAU);
            let (sin, cos) = angle.sin_cos();
            let side = [-sin, cos, 0.0];
            let length = rng.range(1.8, 2.6);
            let mut point = add(top, [0.0, 0.0, -0.2]);
            for k in 1..=3 {
                let t = k as f32 / 3.0;
                let next = add(top, [cos * length * (0.25 + 0.15 * t), sin * length * (0.25 + 0.15 * t), -0.2 - length * t * 0.9]);
                shapes.push(Shape::capsule(point, next, 0.04, 0.03, DEAD_WOOD, AIR, false));
                let at = lerp3(point, next, 0.5);
                for s in [-1.0, 1.0] {
                    let along = [side[0] * s * 0.6 + cos * 0.3, side[1] * s * 0.6 + sin * 0.3, -1.0];
                    shapes.push(Shape::leaf(add(at, scale(normalize(along), 0.25)), along, [cos, sin, 0.0], 0.5, 0.08, PALM_WOOD, DEAD_WOOD));
                }
                point = next;
            }
        }

        let fronds = 7 + (rng.next() * 3.99) as usize;
        for i in 0..fronds {
            let angle = (i as f32 + rng.range(-0.3, 0.3)) / fronds as f32 * TAU;
            let (sin, cos) = angle.sin_cos();
            let side = [-sin, cos, 0.0];
            let length = rng.range(2.5, 3.5);
            let mut point = top;
            for k in 1..=4 {
                let t = k as f32 / 4.0;
                // Arches up, then droops
                let next = add(top, [cos * length * t, sin * length * t, length * (0.35 * t - 0.55 * t * t)]);
                shapes.push(Shape::capsule(point, next, 0.04, 0.03, PALM_LEAF, AIR, false));
                for half in [0.25, 0.75] {
                    let at = lerp3(point, next, half);
                    let leaflet = 0.7 * (1.0 - 0.4 * t);
                    for s in [-1.0, 1.0] {
                        let along = [side[0] * s + cos * 0.3, side[1] * s + sin * 0.3, -0.3];
                        shapes.push(Shape::leaf(add(at, scale(normalize(along), leaflet * 0.5)), along, [0.0, 0.0, 1.0], leaflet, 0.08, PALM_LEAF, LEAVES));
                    }
                }
                point = next;
            }
        }
        shapes
    }

    // Fronds arching out from the middle, with leaflets along both sides
    fn fern(&self, x: f32, y: f32, ground: f32, rng: &mut Rng) -> Vec<Shape> {
        let size = rng.range(0.6, 1.2);
        let fronds = 6 + (rng.next() * 3.99) as usize;
        let mut shapes = Vec::new();
        for i in 0..fronds {
            let angle = (i as f32 + rng.range(-0.3, 0.3)) / fronds as f32 * TAU;
            let (sin, cos) = angle.sin_cos();
            let side = [-sin, cos, 0.0];
            let mut previous = [x, y, ground - 0.05];
            for k in 1..=4 {
                let t = k as f32 / 4.0;
                // Rises, then droops towards the tip
                let point = [x + cos * size * t, y + sin * size * t, ground + size * (0.9 * t - 0.7 * t * t)];
                shapes.push(Shape::capsule(previous, point, 0.015, 0.015, FERN, AIR, false));
                let leaflet = 0.25 * size * (1.0 - 0.5 * t);
                for s in [-1.0, 1.0] {
                    let along = [side[0] * s + cos * 0.4, side[1] * s + sin * 0.4, -0.2];
                    shapes.push(Shape::leaf(add(point, scale(normalize(along), leaflet * 0.5)), along, [0.0, 0.0, 1.0], leaflet, leaflet * 0.35, FERN, LEAVES));
                }
                previous = point;
            }
        }
        shapes
    }

    // A tall spike of flowers on a stem, like a lupine, with leaves fanning out at the bottom
    fn lupine(&self, x: f32, y: f32, ground: f32, color: u8, rng: &mut Rng) -> Vec<Shape> {
        let height = rng.range(0.8, 1.5);
        let base = [x, y, ground - 0.05];
        let top = [x + rng.range(-0.1, 0.1) * height, y + rng.range(-0.1, 0.1) * height, ground + height];
        let mut shapes = vec![
            Shape::capsule(lerp3(base, top, 0.45), top, 0.12, 0.03, color, FLOWER_WHITE, true),
            Shape::capsule(base, top, 0.015, 0.015, GRASS_BLADE, AIR, false),
        ];
        for i in 0..6 {
            let angle = (i as f32 + rng.next()) / 6.0 * TAU;
            let dir = [angle.cos(), angle.sin(), 0.5];
            let length = rng.range(0.15, 0.3);
            shapes.push(Shape::leaf(add([x, y, ground + 0.05], scale(dir, length * 0.5)), dir, [0.0, 0.0, 1.0], length, length * 0.3, FERN, AIR));
        }
        shapes
    }

    // Big broad leaves on short stems, arching outwards
    fn big_leaves(&self, x: f32, y: f32, ground: f32, rng: &mut Rng) -> Vec<Shape> {
        let leaves = 5 + (rng.next() * 3.99) as usize;
        let mut shapes = Vec::new();
        for i in 0..leaves {
            let angle = (i as f32 + rng.range(-0.3, 0.3)) / leaves as f32 * TAU;
            let out = [angle.cos(), angle.sin(), 0.0];
            let length = rng.range(0.35, 0.55);
            let stem_top = add([x, y, ground], add(scale(out, 0.12), [0.0, 0.0, rng.range(0.15, 0.3)]));
            shapes.push(Shape::capsule([x, y, ground - 0.05], stem_top, 0.015, 0.015, FERN, AIR, false));
            let along = add(out, [0.0, 0.0, rng.range(0.1, 0.5)]);
            shapes.push(Shape::leaf(add(stem_top, scale(normalize(along), length * 0.5)), along, [0.0, 0.0, 1.0], length, length * 0.6, LEAVES_DARK, LEAVES));
        }
        shapes
    }

    // Tall stems along the shore, some with a cattail on top
    fn reeds(&self, x: f32, y: f32, ground: f32, rng: &mut Rng) -> Vec<Shape> {
        let mut shapes = Vec::new();
        for _ in 0..(5 + (rng.next() * 4.99) as usize) {
            let at = [x + rng.range(-0.3, 0.3), y + rng.range(-0.3, 0.3), ground - 0.05];
            let height = rng.range(1.0, 1.8);
            let top = add(at, [rng.range(-0.15, 0.15), rng.range(-0.15, 0.15), height]);
            if rng.next() < 0.3 {
                shapes.push(Shape::capsule(lerp3(at, top, 0.75), lerp3(at, top, 0.9), 0.04, 0.04, CATTAIL, AIR, false));
            }
            shapes.push(Shape::capsule(at, top, 0.02, 0.012, REED, GRASS_TIP, false));
            let lean = rng.direction(0.8);
            shapes.push(Shape::leaf(add(at, [lean[0] * 0.15, lean[1] * 0.15, 0.4]), add(lean, [0.0, 0.0, 2.0]), lean, 0.7, 0.04, REED, AIR));
        }
        shapes
    }

    /// A mushroom of the dreaming realm, as tall as a tree: a pale stem swelling at its foot, under a
    /// broad rosy cap flecked with white, flat enough on top to stand on
    fn giant_mushroom(&self, x: f32, y: f32, ground: f32, rng: &mut Rng) -> Vec<Shape> {
        let tall = rng.range(6.0, 11.0);
        let cap = rng.range(3.0, 6.0);
        let stem = rng.range(0.6, 1.1);
        let lean = [rng.range(-0.15, 0.15) * tall, rng.range(-0.15, 0.15) * tall, tall];
        let foot = [x, y, ground - 0.4];
        let top = add(foot, lean);
        vec![
            Shape::ellipsoid(add(top, [0.0, 0.0, cap * 0.18]), [cap, cap, cap * 0.42], NOSE, BONE),
            Shape::capsule(foot, top, stem * 1.5, stem, LINEN, LINEN, false),
        ]
    }

    // A few red mushrooms with white spots
    fn mushrooms(&self, x: f32, y: f32, ground: f32, rng: &mut Rng) -> Vec<Shape> {
        let mut shapes = Vec::new();
        for _ in 0..(1 + (rng.next() * 3.99) as usize) {
            let at = [x + rng.range(-0.25, 0.25), y + rng.range(-0.25, 0.25), ground - 0.02];
            let height = rng.range(0.06, 0.16);
            let cap = rng.range(0.05, 0.12);
            let top = add(at, [0.0, 0.0, height]);
            shapes.push(Shape::sphere(add(top, [0.0, 0.0, cap * 0.2]), cap * 0.7, MUSHROOM_CAP, FLOWER_WHITE, false));
            shapes.push(Shape::leaf(top, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], cap * 2.0, cap * 2.0, MUSHROOM_CAP, FLOWER_WHITE));
            shapes.push(Shape::capsule(at, top, 0.03, 0.025, MUSHROOM_STEM, AIR, false));
        }
        shapes
    }
}

fn hash(mut x: u32) -> u32 {
    // https://nullprogram.com/blog/2018/07/31/ (lowbias32)
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

fn hash3(x: u32, y: u32, z: u32) -> u32 {
    hash(x ^ hash(y ^ hash(z)))
}

/// Maps the low 16 bits to [0, 1].
fn unit(bits: u32) -> f32 {
    (bits & 0xFFFF) as f32 / 65535.0
}

/// Value noise in 3D, -1 to 1, smooth over about a unit
fn noise3(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let (fx, fy, fz) = (x.floor(), y.floor(), z.floor());
    let (ix, iy, iz) = (fx as i32, fy as i32, fz as i32);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (tx, ty, tz) = (s(x - fx), s(y - fy), s(z - fz));
    let at = |dx: i32, dy: i32, dz: i32| unit(hash3((ix + dx) as u32, (iy + dy) as u32, (iz + dz) as u32 ^ seed)) * 2.0 - 1.0;
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let plane = |dz: i32| lerp(lerp(at(0, 0, dz), at(1, 0, dz), tx), lerp(at(0, 1, dz), at(1, 1, dz), tx), ty);
    lerp(plane(0), plane(1), tz)
}

/// The big bulges and hollows of a cave's walls at `p`, in meters, -0.7 to 0.7
fn cave_lumps(p: Vec3) -> f32 {
    noise3(p[0] / 2.2, p[1] / 2.2, p[2] / 1.6, 0xca7e) * 0.7
}

/// How far the smaller knobs and ledges on them go either way, and what they add at `p`
const CAVE_KNOBS: f32 = 0.45;
fn cave_knobs(p: Vec3) -> f32 {
    noise3(p[0] / 0.7, p[1] / 0.7, p[2] / 0.5, 0x5a1d) * CAVE_KNOBS
}

/// The lumps of a cave's walls at `p`, -1 to 1: big bulges and hollows, with smaller knobs and
/// ledges on them
fn cave_noise(p: Vec3) -> f32 {
    (cave_lumps(p) + cave_knobs(p)).clamp(-1.0, 1.0)
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 2D gradient noise in roughly [-1, 1].
fn noise2(x: f32, y: f32, seed: u32) -> f32 {
    const GRADIENTS: [(f32, f32); 8] = [
        (1.0, 0.0),
        (-1.0, 0.0),
        (0.0, 1.0),
        (0.0, -1.0),
        (FRAC_1_SQRT_2, FRAC_1_SQRT_2),
        (-FRAC_1_SQRT_2, FRAC_1_SQRT_2),
        (FRAC_1_SQRT_2, -FRAC_1_SQRT_2),
        (-FRAC_1_SQRT_2, -FRAC_1_SQRT_2),
    ];

    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (ix, iy) = (x0 as i32, y0 as i32);

    let corner = |dx: i32, dy: i32| {
        let (gx, gy) = GRADIENTS[(hash3((ix + dx) as u32, (iy + dy) as u32, seed) & 7) as usize];
        gx * (fx - dx as f32) + gy * (fy - dy as f32)
    };
    let fade = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;

    let (u, v) = (fade(fx), fade(fy));
    let bottom = lerp(corner(0, 0), corner(1, 0), u);
    let top = lerp(corner(0, 1), corner(1, 1), u);
    lerp(bottom, top, v) * std::f32::consts::SQRT_2
}

/// Fractal noise, normalized to roughly [-1, 1].
fn fbm(x: f32, y: f32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amplitude, mut frequency, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for i in 0..octaves {
        sum += amplitude * noise2(x * frequency, y * frequency, seed.wrapping_add(i * 101));
        norm += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    sum / norm
}

/// Ridged fractal noise in [0, 1], sharp crests that look like mountain ranges.
fn ridged(x: f32, y: f32, octaves: u32, seed: u32) -> f32 {
    let (mut sum, mut amplitude, mut frequency, mut norm, mut weight) = (0.0, 1.0, 1.0, 0.0, 1.0);
    for i in 0..octaves {
        let n = 1.0 - noise2(x * frequency, y * frequency, seed.wrapping_add(i * 101)).abs();
        let n = n * n * weight;
        weight = n.clamp(0.0, 1.0);
        sum += amplitude * n;
        norm += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    sum / norm
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wherever the world's built, out on the land, a river runs through most of it: counts the
    /// squares as big as a world, over a couple of kilometers, that have one
    #[test]
    fn rivers_run_through_most_of_the_land() {
        let settings = TerrainSettings { width_m: 32, height_m: 64, voxels_per_meter: 4, seed: 1, grass_tufts: false };
        let (terrain, _) = generate_terrain(&settings, [0, 0], &RiverCache::default());
        let cache = RiverCache::default();
        let sea = sea_level_m(&settings);
        let (side, n) = (224.0, 10);
        let (mut land, mut with) = (0, 0);
        for j in 0..n {
            for i in 0..n {
                let lo = [(i - n / 2) as f32 * side, (j - n / 2) as f32 * side];
                let hi = [lo[0] + side, lo[1] + side];
                let dry = (0..16).filter(|k| height_at(&settings, lo[0] + side * ((k % 4) as f32 + 0.5) / 4.0, lo[1] + side * ((k / 4) as f32 + 0.5) / 4.0) > sea).count();
                if dry < 8 {
                    continue;
                }
                land += 1;
                let rivers = terrain.rivers_near(&cache, lo, hi);
                if rivers.iter().any(|r| r.iter().any(|p| p.x > lo[0] && p.y > lo[1] && p.x < hi[0] && p.y < hi[1])) {
                    with += 1;
                }
            }
        }
        let rivers: Vec<_> = cache.cells.lock().unwrap().values().flatten().cloned().collect();
        let long = rivers.iter().map(|r| r.len() as f32 * 0.5).sum::<f32>() / rivers.len().max(1) as f32;
        println!("{with} of {land} squares of land have a river; {} rivers, {long:.0} m long on average", rivers.len());
        assert!(land > 0 && with * 4 >= land * 3);
    }

    /// A map of two kilometers of the landscape, a pixel to a meter, with its rivers bright blue:
    /// `cargo test --release river_map -- --ignored`, saved where RIVER_MAP says or in the temp folder
    #[test]
    #[ignore]
    fn river_map() {
        let settings = TerrainSettings { width_m: 32, height_m: 64, voxels_per_meter: 1, seed: 1, grass_tufts: false };
        let side = 2000;
        let origin = [-(side as i64) / 2, -(side as i64) / 2];
        let mut heights = Heights::new(&settings, origin, [side, side], 1);
        let mut terrain = Terrain::new(&settings, &heights);
        terrain.carve_rivers(&mut heights, &RiverCache::default());
        let sea = sea_level_m(&settings);
        let map = image::RgbImage::from_fn(side as u32, side as u32, |x, y| {
            let i = x as usize + (side - 1 - y as usize) * side;
            let g = heights.ground[i];
            let shade = |c: [f32; 3], k: f32| image::Rgb(c.map(|v| (v * k).clamp(0.0, 255.0) as u8));
            if heights.river[i].is_finite() {
                image::Rgb([60, 170, 255])
            } else if g < sea {
                shade([20.0, 50.0, 110.0], 0.6 + 0.4 * (g / sea))
            } else {
                let up = ((g - sea) / (settings.height_m as f32 - sea)).clamp(0.0, 1.0);
                let land = [90.0 + 150.0 * up, 140.0 + 100.0 * up, 70.0 + 170.0 * up];
                shade(land, 0.75 + 0.25 * up)
            }
        });
        let path = std::env::var("RIVER_MAP").map_or(std::env::temp_dir().join("river_map.png"), std::path::PathBuf::from);
        map.save(&path).unwrap();
        println!("{} rivers, map in {}", terrain.rivers.len(), path.display());
    }

    /// What all of a brick is, by voxel: x + y * 8 + z * 64
    fn brick_voxels(world: &World, cell: usize) -> [u8; BRICK_VOXELS] {
        let value = world.brick_grid[cell];
        if value & EMPTY_BRICK != 0 {
            return [AIR; BRICK_VOXELS];
        }
        if value & SOLID_BRICK != 0 {
            return [value as u8; BRICK_VOXELS];
        }
        let words = world.brick(((value & BRICK_INDEX) - 1) as usize);
        std::array::from_fn(|i| (words[16 + i / 4] >> ((i % 4) * 8)) as u8)
    }

    /// Where `window` differs from the same rectangle of the landscape built in one go, `whole`,
    /// voxel by voxel, in grass and in the ground water lies on
    fn differences(window: &World, whole: &World) -> Vec<String> {
        assert_eq!(window.size, whole.size);
        let [sx, sy, sz] = window.size.map(i64::from);
        let mut problems = Vec::new();
        for y in 0..sy {
            for x in 0..sx {
                for z in 0..sz {
                    let (a, b) = (window.material_at(x, y, z), whole.material_at(x, y, z));
                    if a != b && problems.len() < 20 {
                        problems.push(format!("voxel {x}, {y}, {z}: {a} moving on, {b} in one go"));
                    }
                }
                let (i, j) = (window.column(x, y), (x + y * sx) as usize);
                let (ga, gb) = (window.grass.as_ref().unwrap()[i], whole.grass.as_ref().unwrap()[j]);
                let (wa, wb) = (window.water.as_ref().unwrap().ground[i], whole.water.as_ref().unwrap().ground[j]);
                if (ga != gb || wa != wb) && problems.len() < 40 {
                    problems.push(format!("column {x}, {y}: grass {ga:#x}/{gb:#x}, ground {wa}/{wb}"));
                }
            }
        }
        problems
    }

    /// Puts the strip of the landscape from `origin`, `size` columns across, into `window`
    fn put_in(window: &mut World, settings: &TerrainSettings, cache: &RiverCache, origin: [i64; 2], size: [u32; 2]) {
        let (_, piece) = generate_area(settings, origin, size, cache);
        let pool = window.page_pool().unwrap();
        let strip = window.phys[0] - window.size[0];
        let mut installing = World::install(Strip::new(piece, &pool, strip));
        while !window.install_step(&mut installing, 3) {}
    }

    /// Lets go of the columns from `lo` to `hi` of `window`, giving its pages back at once
    fn let_go(window: &mut World, lo: [i64; 2], hi: [i64; 2]) {
        let mut releasing = World::release(lo, hi);
        while !window.release_step(&mut releasing, 5) {}
        let pages = window.take_released();
        window.page_pool().unwrap().lock().unwrap().give_back(pages);
    }

    /// How many cells of `world` hold a stored brick
    fn stored(world: &World) -> usize {
        world.brick_grid.iter().filter(|&&c| c & (SOLID_BRICK | EMPTY_BRICK) == 0).count()
    }

    /// The world moves on over the landscape a strip at a time, each column kept where it is in the
    /// landscape modulo the room there is, so it wraps round: moved on along x and y, back again
    /// and on, it's the same at every step as the same piece of the landscape built in one go, and
    /// what's let go of is used again
    #[test]
    fn the_world_moves_on_a_strip_at_a_time() {
        let settings = TerrainSettings { width_m: 64, height_m: 64, voxels_per_meter: 4, seed: 1, grass_tufts: true };
        let cache = RiverCache::default();
        let (w, s) = (64 * 4, 16 * 4);
        let size = [w as u32, w as u32, 64 * 4];
        let mut origin = [80 * 4, 72 * 4];
        let (_, first) = generate_area(&settings, origin, [w as u32, w as u32], &cache);
        let mut window = World::window(size, s as u32, origin, 4, first.sea_level);
        put_in(&mut window, &settings, &cache, origin, [w as u32, w as u32]);
        let mut pages_most = 0;
        // Along x, along y, back along x twice, then on along y and back
        for (k, step) in [[1, 0], [0, 1], [-1, 0], [-1, 0], [0, 1], [0, -1], [1, 0]].into_iter().enumerate() {
            let axis = if step[0] != 0 { 0 } else { 1 };
            let forward = step[axis] > 0;
            // The strip coming in, just past the edge it's moving towards
            let mut lo = origin;
            lo[axis] = if forward { origin[axis] + w } else { origin[axis] - s };
            let mut room = [w as u32; 2];
            room[axis] = s as u32;
            // Where it goes is let go of first: what left last time along this way
            let mut hi = [lo[0] + room[0] as i64, lo[1] + room[1] as i64];
            let_go(&mut window, lo, hi);
            put_in(&mut window, &settings, &cache, lo, room);
            origin[axis] += if forward { s } else { -s };
            window.move_to(origin);
            // What's left behind
            lo = origin;
            lo[axis] = if forward { origin[axis] - s } else { origin[axis] + w };
            hi = [lo[0] + room[0] as i64, lo[1] + room[1] as i64];
            let_go(&mut window, lo, hi);

            let (_, whole) = generate_area(&settings, origin, [w as u32, w as u32], &cache);
            let problems = differences(&window, &whole);
            assert!(problems.is_empty(), "after step {k}, {step:?}, at {origin:?}:\n{}", problems.join("\n"));
            assert_eq!(window.brick_count(), stored(&window), "after step {k}");
            pages_most = pages_most.max(window.page_pool().unwrap().lock().unwrap().count());
        }
        // Pages let go of were used again: room for little more than the world and a strip, give or
        // take one part filled for each square of the landscape a strip across they reach into
        let need = (stored(&window)).div_ceil(PAGE_BRICKS) as u32;
        let squares = ((w / s + 1) * (w / s + 2)) as u32;
        assert!(pages_most <= need * 2 + squares, "{pages_most} pages for {need}");
    }

    /// The bricks in each page of the world that moves on all lie in one square of the landscape a
    /// strip across, those of the piece built in one go it starts from and of the strips put in
    /// since alike, so as it lets go of a strip at a time each page comes free once the world's
    /// left its square: however far it goes, it needs little more room than it has bricks
    #[test]
    fn pages_come_free_as_the_world_moves_on() {
        let settings = TerrainSettings { width_m: 64, height_m: 64, voxels_per_meter: 4, seed: 3, grass_tufts: true };
        let cache = RiverCache::default();
        let (w, s) = (64 * 4, 16 * 4);
        let size = [w as u32, w as u32, 64 * 4];
        // A whole number of strips from the landscape's corner, as the game's world always is
        let mut origin = [48 * 4, -32 * 4];
        let (_, first) = generate_area(&settings, origin, [w as u32, w as u32], &cache);
        let mut window = World::window(size, s as u32, origin, 4, first.sea_level);
        put_in(&mut window, &settings, &cache, origin, [w as u32, w as u32]);
        // Which square of the landscape each page in use has bricks of
        let squares = |window: &World| -> usize {
            let mut square: HashMap<usize, [i64; 2]> = HashMap::new();
            let [bx, by, bz] = window.size.map(|n| n as i64 / 8);
            for z in 0..bz {
                for y in 0..by {
                    for x in 0..bx {
                        let value = window.brick_grid[window.cell([x, y, z])];
                        if value & (SOLID_BRICK | EMPTY_BRICK) != 0 {
                            continue;
                        }
                        let page = ((value & BRICK_INDEX) - 1) as usize / PAGE_BRICKS;
                        let at = [(window.origin[0] / 8 + x).div_euclid(s / 8), (window.origin[1] / 8 + y).div_euclid(s / 8)];
                        let was = *square.entry(page).or_insert(at);
                        assert_eq!(was, at, "page {page} has bricks of squares {was:?} and {at:?}");
                    }
                }
            }
            square.len()
        };
        squares(&window);
        let mut pages_most = 0;
        // On along x for more than the world's width, along y, then back along x
        let steps = [[1, 0]; 6].into_iter().chain([[0, 1]; 2]).chain([[-1, 0]; 3]);
        for (k, step) in steps.enumerate() {
            let axis = if step[0] != 0 { 0 } else { 1 };
            let forward = step[axis] > 0;
            let mut lo = origin;
            lo[axis] = if forward { origin[axis] + w } else { origin[axis] - s };
            let mut room = [w as u32; 2];
            room[axis] = s as u32;
            put_in(&mut window, &settings, &cache, lo, room);
            pages_most = pages_most.max(window.page_pool().unwrap().lock().unwrap().count());
            origin[axis] += if forward { s } else { -s };
            window.move_to(origin);
            // What's left behind goes at once, here
            lo = origin;
            lo[axis] = if forward { origin[axis] - s } else { origin[axis] + w };
            let_go(&mut window, lo, [lo[0] + room[0] as i64, lo[1] + room[1] as i64]);
            let in_use = squares(&window);
            assert_eq!(window.brick_count(), stored(&window), "after step {k}");
            // Every page let go of is free again: as many in use as there are bricks for, give or
            // take one part filled for each square
            let free = window.page_pool().unwrap().lock().unwrap().free.len();
            let made = window.page_pool().unwrap().lock().unwrap().count() as usize;
            assert_eq!(made - free, in_use, "after step {k}");
            assert!(in_use <= stored(&window).div_ceil(PAGE_BRICKS) + ((w / s) * (w / s)) as usize, "after step {k}");
        }
        // Never more than the world and the strip coming in, give or take one part filled for each
        // square of them
        let need = stored(&window).div_ceil(PAGE_BRICKS) as u32;
        let squares = ((w / s) * (w / s + 1)) as u32;
        assert!(pages_most <= need * 3 / 2 + squares, "{pages_most} pages for {need}");
    }

    /// A hole blown and a stain splashed right where the world wraps round in its room, after it's
    /// moved on, come out as they would in the same piece of the landscape built in one go
    #[test]
    fn holes_blown_where_the_world_wraps() {
        let settings = TerrainSettings { width_m: 64, height_m: 64, voxels_per_meter: 4, seed: 1, grass_tufts: true };
        let cache = RiverCache::default();
        let (w, s) = (64 * 4, 16 * 4);
        let size = [w as u32, w as u32, 64 * 4];
        // Started further back and moved on, so the world's column 0 isn't the room's
        let start = [96 * 4, 30 * 4];
        let (_, first) = generate_area(&settings, start, [w as u32, w as u32], &cache);
        let mut window = World::window(size, s as u32, start, 4, first.sea_level);
        put_in(&mut window, &settings, &cache, start, [w as u32, w as u32]);
        let origin = [start[0] + s, start[1] + s];
        let_go(&mut window, [start[0] + w, start[1]], [start[0] + w + s, start[1] + w]);
        put_in(&mut window, &settings, &cache, [start[0] + w, start[1]], [s as u32, w as u32]);
        window.move_to([origin[0], start[1]]);
        let_go(&mut window, [start[0] + s, start[1] + w], [start[0] + w + s, start[1] + w + s]);
        put_in(&mut window, &settings, &cache, [origin[0], start[1] + w], [w as u32, s as u32]);
        window.move_to(origin);
        assert!(window.wrap[0] > 0 && window.wrap[1] > 0, "{:?}", window.wrap);
        let (_, mut whole) = generate_area(&settings, origin, [w as u32, w as u32], &cache);
        // Where the room wraps round, inside the world both ways, on the ground
        let seam = [0, 1].map(|i| (window.phys[i] - window.wrap[i]) as i64);
        assert!(seam.iter().all(|&v| v > 16 && v < w - 16), "{seam:?}");
        let ground = (0..size[2] as i64).rev().find(|&z| !is_passable(whole.material_at(seam[0], seam[1], z))).unwrap();
        let center = [seam[0] as f64 + 0.5, seam[1] as f64 + 0.5, ground as f64];
        for world in [&mut window, &mut whole] {
            world.carve(center, 3.0 * 4.0);
            world.paint([center[0] + 6.0, center[1] - 5.0, center[2] - 6.0], 2.0 * 4.0, ROCK, true);
        }
        let problems = differences(&window, &whole);
        assert!(problems.is_empty(), "{}", problems.join("\n"));
        assert_eq!(window.brick_count(), stored(&window));
    }

    /// What of a piece of the landscape belongs to it alone, each as a line, sorted
    fn owned(terrain: &Terrain) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        lines.extend(terrain.props().iter().map(|p| format!("prop {:?} {:?}", p.kind, p.position)));
        lines.extend(terrain.lights().iter().map(|l| format!("light {:?} {:?}", l.position, l.color)));
        lines.extend(terrain.sites().iter().map(|s| format!("site {} {:?}", s.kind, s.position)));
        lines.extend(terrain.bandit_camps().iter().map(|c| format!("camp {:?} {:?}", c.kind.name(), c.fire)));
        lines.extend(terrain.piece_springs().iter().map(|s| format!("spring {s:?}")));
        lines.sort();
        lines
    }

    /// Builds the rectangle `size` voxel columns across at `origin` in one go, and again in strips
    /// `strip` columns across along `axis`, and says where they come out differently: voxels, grass,
    /// where water lies, and what's there besides
    fn seams(settings: &TerrainSettings, origin: [i64; 2], size: [u32; 2], strip: u32, axis: usize) -> Vec<String> {
        let cache = RiverCache::default();
        let (whole_terrain, whole) = generate_area(settings, origin, size, &cache);
        let vpm = settings.voxels_per_meter as f64;
        let [bx, by, bz] = whole.size.map(|n| n as usize / 8);
        let mut problems = Vec::new();
        let mut features = Vec::new();
        let at = |x: usize, y: usize| format!("{:.2}, {:.2} m", (origin[0] + x as i64) as f64 / vpm, (origin[1] + y as i64) as f64 / vpm);
        for k in 0..size[axis] / strip {
            let mut o = origin;
            o[axis] += (k * strip) as i64;
            let mut s = size;
            s[axis] = strip;
            let (terrain, piece) = generate_area(settings, o, s, &cache);
            let shift = [(o[0] - origin[0]) as usize, (o[1] - origin[1]) as usize];
            let [px, py] = [s[0] as usize / 8, s[1] as usize / 8];
            for z in 0..bz {
                for y in 0..py {
                    for x in 0..px {
                        let mine = (x + (y + z * py) * px, piece.brick_grid[x + (y + z * py) * px]);
                        let theirs = ((x + shift[0] / 8) + ((y + shift[1] / 8) + z * by) * bx, 0);
                        let theirs = (theirs.0, whole.brick_grid[theirs.0]);
                        // Empty ones say how far it is to the next that isn't, which goes by how
                        // much of the world there is
                        if mine.1 & EMPTY_BRICK != 0 && theirs.1 & EMPTY_BRICK != 0 || mine.1 == theirs.1 && mine.1 & SOLID_BRICK != 0 {
                            continue;
                        }
                        let (a, b) = (brick_voxels(&piece, mine.0), brick_voxels(&whole, theirs.0));
                        if let Some(i) = (0..BRICK_VOXELS).find(|&i| a[i] != b[i]) {
                            if problems.len() < 40 {
                                problems.push(format!("voxel at {}, {} up: {} in strip {k}, {} in one go", at(shift[0] + x * 8 + i % 8, shift[1] + y * 8 + i / 8 % 8), z * 8 + i / 64, a[i], b[i]));
                            }
                        } else if (mine.1 & FOLIAGE_BRICK) != (theirs.1 & FOLIAGE_BRICK) {
                            problems.push(format!("foliage flag at {}", at(shift[0] + x * 8, shift[1] + y * 8)));
                        }
                    }
                }
            }
            let (pg, wg) = (piece.grass.as_ref().unwrap(), whole.grass.as_ref().unwrap());
            let (pw, ww) = (piece.water.as_ref().unwrap(), whole.water.as_ref().unwrap());
            for y in 0..s[1] as usize {
                for x in 0..s[0] as usize {
                    let (i, j) = (x + y * s[0] as usize, (x + shift[0]) + (y + shift[1]) * size[0] as usize);
                    if problems.len() < 80 && (pg[i] != wg[j] || pw.ground[i] != ww.ground[j] || pw.surface[i] != ww.surface[j]) {
                        problems.push(format!(
                            "column at {}: grass {:#x}/{:#x}, ground {}/{}, water {}/{} in strip {k} and in one go",
                            at(x + shift[0], y + shift[1]),
                            pg[i],
                            wg[j],
                            pw.ground[i],
                            ww.ground[j],
                            pw.surface[i],
                            ww.surface[j]
                        ));
                    }
                }
            }
            features.extend(owned(&terrain));
        }
        features.sort();
        let all = owned(&whole_terrain);
        if features != all {
            let only = |a: &[String], b: &[String]| a.iter().filter(|l| !b.contains(l)).take(10).cloned().collect::<Vec<_>>();
            problems.push(format!("only in the strips: {:?}; only in one go: {:?}", only(&features, &all), only(&all, &features)));
        }
        println!("{} strips of {} m at {:?}: {} owned things, {} problems", size[axis] / strip, strip as f64 / vpm, origin.map(|v| v as f64 / vpm), all.len(), problems.len());
        problems
    }

    /// A rectangle of the landscape `size` meters across centered on `at`, in voxel columns at
    /// `vpm`, lined up with the bricks
    fn around(at: [f32; 2], size: [u32; 2], vpm: u32) -> ([i64; 2], [u32; 2]) {
        let origin = [0, 1].map(|i| (((at[i] - size[i] as f32 * 0.5) * vpm as f32) as i64).div_euclid(8) * 8);
        (origin, size.map(|n| n * vpm))
    }

    /// The world moves on a strip at a time, each built on its own: those built next to each other
    /// have to meet with no seam, through towns, castles, fallen cities, dungeons and caves as much as
    /// out in the woods, so a strip built later lines up with the rest just as if it had all been built
    /// in one go. Each spot here has a strip's edge going through it.
    #[test]
    fn strips_meet_without_seams() {
        let settings = TerrainSettings { width_m: 32, height_m: 64, voxels_per_meter: 4, seed: 1, grass_tufts: true };
        let finder = Terrain::around(&settings, [0.0; 2], [1.0; 2]);
        let mut spots: Vec<(&str, [f32; 2])> = vec![("start", [112.0, 112.0])];
        if let Some((square, _)) = finder.nearest_town([112.0, 112.0], false, None) {
            spots.push(("town", [square[0], square[1]]));
        }
        if let Some(royal) = finder.nearest_castle([112.0, 112.0], None) {
            spots.push(("royal castle", [royal.middle[0], royal.middle[1]]));
        }
        if let Some(sunken) = finder.nearest_undercity([112.0, 112.0]) {
            spots.push(("fallen city", [sunken.foot[0], sunken.foot[1]]));
        }
        if let Some(door) = finder.nearest_dungeon([112.0, 112.0]) {
            spots.push(("dungeon", [door[0], door[1]]));
        }
        if let Some((floor, ..)) = finder.nearest_cavern([112.0, 112.0]) {
            spots.push(("cavern", [floor[0], floor[1]]));
        }
        if let Some(fire) = camp_spots(&settings, CampKind::Fort, [112.0, 112.0], None).first() {
            spots.push(("camp", *fire));
        }
        let mut failed = Vec::new();
        for (k, (name, at)) in spots.iter().enumerate() {
            // Strips along x for some, along y for the others, 16 m across: both ways the world moves
            let axis = k % 2;
            let (origin, size) = around(*at, [96, 96], settings.voxels_per_meter);
            let problems = seams(&settings, origin, size, 16 * settings.voxels_per_meter, axis);
            if !problems.is_empty() {
                failed.push(format!("{name} at {at:?}:\n  {}", problems.join("\n  ")));
            }
        }
        assert!(failed.is_empty(), "{}", failed.join("\n"));
    }

    /// The Drowned Rings, the Sunken Reach's and the normal land's far out from where Free Roam
    /// starts: a deep basin with stones standing in it to stand on, a walk all round it at the
    /// water's edge, stands going up from it a jump at a time, fallen in or not, and a way in from the
    /// land along the causeway and through the passage without wading deep, built a strip at a time
    /// just as in one go
    #[test]
    fn the_drowned_rings_have_a_deep_basin_stands_and_a_way_in() {
        for seed in [1, 0xF400_0001] {
            let settings = TerrainSettings { width_m: 32, height_m: 64, voxels_per_meter: 4, seed, grass_tufts: false };
            let arena = arena_of(&settings).unwrap_or_else(|| panic!("seed {seed:#x} has no Drowned Ring"));
            let sea = sea_level_m(&settings);
            let h = |p: [f32; 2]| height_at(&settings, p[0], p[1]);
            let at = |a: f32, d: f32| [arena.middle[0] + a.cos() * d, arena.middle[1] + a.sin() * d];
            if seed == 1 {
                let far = (arena.middle[0] - FREE_ROAM_START[0]).hypot(arena.middle[1] - FREE_ROAM_START[1]);
                assert!(far >= 1500.0, "only {far} m out from where Free Roam starts");
            }
            assert!(h(arena.middle) < sea - 8.0, "seed {seed:#x}: its basin's {} m deep", sea - h(arena.middle));
            for (p, _, top) in arena.stones() {
                assert!((h(p) - sea - top).abs() < 0.2, "seed {seed:#x}: a stone's top is {} m over the sea", h(p) - sea);
            }
            // Half way between the sixteen ways the stands are split, as where a fallen sector meets
            // one still standing its side's sheer
            for k in 0..64 {
                let a = (k as f32 + 0.5) * TAU / 64.0;
                let walk = h(at(a, ARENA_BASIN + 1.3));
                assert!(walk > sea - 0.4 && walk < sea + 0.7, "seed {seed:#x}: the walk's {} m over the sea at {a}", walk - sea);
                // Up the stands, but for in the passage, whose sides are sheer
                if ((a - arena.facing + std::f32::consts::PI).rem_euclid(TAU) - std::f32::consts::PI).abs() < 0.22 {
                    continue;
                }
                let mut last = walk;
                for s in 1..40 {
                    let up = h(at(a, ARENA_BASIN + 1.3 + s as f32 * 0.25));
                    assert!(up - last < 0.95, "seed {seed:#x}: a step up of {} m at {a}, {} m out", up - last, ARENA_BASIN + 1.3 + s as f32 * 0.25);
                    last = up;
                }
            }
            // From the walk out through the passage and along the causeway, never wading deep,
            // to the land at the end of it
            let (fx, fy) = (arena.facing.cos(), arena.facing.sin());
            let mut along = ARENA_BASIN + 1.0;
            let landed = loop {
                let p = [arena.middle[0] + fx * along, arena.middle[1] + fy * along];
                let ground = h(p);
                assert!(ground > sea - 0.5, "seed {seed:#x}: {} m deep {along} m out along the way in", sea - ground);
                if along > ARENA_STANDS + 4.0 && base_height(&settings, p[0], p[1]) > sea + 1.0 {
                    break true;
                }
                along += 0.5;
                if along > ARENA_STANDS + arena.causeway + 6.0 {
                    break false;
                }
            };
            assert!(landed, "seed {seed:#x}: the causeway doesn't reach the land");
            // Built a strip at a time just as in one go, round its basin and where its causeway
            // goes through the stands
            for (k, spot) in [arena.middle, at(arena.facing, ARENA_STANDS)].into_iter().enumerate() {
                let (origin, size) = around(spot, [64, 64], settings.voxels_per_meter);
                let problems = seams(&settings, origin, size, 16 * settings.voxels_per_meter, k % 2);
                assert!(problems.is_empty(), "seed {seed:#x} at {spot:?}:\n  {}", problems.join("\n  "));
            }
        }
    }

    /// The same in every kind of land, over the widest river about and at a waterfall, in strips as
    /// narrow as the world moves on by, each way
    #[test]
    fn narrow_strips_meet_without_seams_anywhere() {
        let settings = TerrainSettings { width_m: 32, height_m: 64, voxels_per_meter: 4, seed: 1, grass_tufts: true };
        let cache = RiverCache::default();
        let mut spots: Vec<(String, [f32; 2])> = Vec::new();
        for kind in ["jungle", "desert", "snow", "autumn", "temperate"] {
            if let Some(at) = find_climate(&settings, kind, [160.0, 160.0]) {
                spots.push((kind.to_string(), at));
            }
        }
        let finder = Terrain::around(&settings, [0.0; 2], [1.0; 2]);
        if let Some(at) = finder.widest_river_near([112.0, 112.0], &cache) {
            spots.push(("river".into(), at));
        }
        if let Some((lip, ..)) = waterfalls_in(&settings, &cache, [-200.0, -200.0], [400.0, 400.0]).first() {
            spots.push(("waterfall".into(), [lip[0], lip[1]]));
        }
        let mut failed = Vec::new();
        for (k, (name, at)) in spots.iter().enumerate() {
            let (origin, size) = around(*at, [64, 64], settings.voxels_per_meter);
            let problems = seams(&settings, origin, size, 8 * settings.voxels_per_meter, k % 2);
            if !problems.is_empty() {
                failed.push(format!("{name} at {at:?}:\n  {}", problems.join("\n  ")));
            }
        }
        assert!(spots.len() >= 6, "{spots:?}");
        assert!(failed.is_empty(), "{}", failed.join("\n"));
    }

    /// The water a world built a strip at a time has flowing in is what it'd have built in one go:
    /// where the rivers come into it, see window_springs(), and what each strip has of its own
    #[test]
    fn strips_have_the_springs_of_the_whole() {
        let settings = TerrainSettings { width_m: 96, height_m: 64, voxels_per_meter: 4, seed: 1, grass_tufts: false };
        let cache = RiverCache::default();
        let (origin, size) = around([112.0, 112.0], [96, 96], 4);
        let (whole, _) = generate_area(&settings, origin, size, &cache);
        let key = |s: &[f32; 4]| format!("{s:?}");
        let mut wanted: Vec<String> = whole.springs().iter().map(key).collect();
        let (lo, hi) = (whole.own_lo, whole.own_hi);
        let mut got: Vec<String> = window_springs(&settings, &cache, lo, hi).iter().map(key).collect();
        for k in 0..4 {
            let o = [origin[0] + k * 24 * 4, origin[1]];
            let (strip, _) = generate_area(&settings, o, [24 * 4, size[1]], &cache);
            got.extend(strip.piece_springs().iter().map(key));
        }
        wanted.sort();
        got.sort();
        assert!(!wanted.is_empty());
        assert_eq!(got, wanted);
    }

    /// The same at the resolution the game's built at, out in the woods by the start, where the
    /// trees reach over the edges of the strips
    #[test]
    fn fine_strips_meet_without_seams() {
        let settings = TerrainSettings { width_m: 32, height_m: 64, voxels_per_meter: 16, seed: 1, grass_tufts: true };
        let (origin, size) = around([112.0, 112.0], [48, 32], 16);
        let problems = seams(&settings, origin, size, 8 * 16, 0);
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    /// Pictures of where strips meet: a royal castle and a town from above, built in one go and in
    /// strips 16 m across, each column the color of what's on top of it and lighter the higher it
    /// is, and where the two differ in red, which is nowhere: `cargo test --release seam_pictures --
    /// --ignored`, saved where SEAM_DIR says or in the temp folder
    #[test]
    #[ignore]
    fn seam_pictures() {
        let settings = TerrainSettings { width_m: 32, height_m: 64, voxels_per_meter: 8, seed: 1, grass_tufts: true };
        let finder = Terrain::around(&settings, [0.0; 2], [1.0; 2]);
        let dir = std::env::var("SEAM_DIR").map_or(std::env::temp_dir(), std::path::PathBuf::from);
        let mut spots = Vec::new();
        if let Some(royal) = finder.nearest_castle([112.0, 112.0], None) {
            spots.push(("castle", [royal.middle[0], royal.middle[1]]));
        }
        if let Some((square, _)) = finder.nearest_town([112.0, 112.0], false, None) {
            spots.push(("town", [square[0], square[1]]));
        }
        let cache = RiverCache::default();
        for (name, at) in spots {
            let (origin, size) = around(at, [96, 96], settings.voxels_per_meter);
            let (_, whole) = generate_area(&settings, origin, size, &cache);
            let strip = 16 * settings.voxels_per_meter;
            let pieces: Vec<World> = (0..size[0] / strip).map(|k| generate_area(&settings, [origin[0] + (k * strip) as i64, origin[1]], [strip, size[1]], &cache).1).collect();
            let top = |world: &World, x: i64, y: i64| (0..world.size[2] as i64).rev().find_map(|z| Some((z, world.material_at(x, y, z))).filter(|(_, m)| *m != AIR)).unwrap_or((0, AIR));
            let shade = |(z, m): (i64, u8)| {
                let c = hash(m as u32 * 2654435761);
                let lift = 0.45 + 0.55 * z as f32 / (settings.height_m * settings.voxels_per_meter) as f32 * 2.0;
                image::Rgb([c as u8, (c >> 8) as u8, (c >> 16) as u8].map(|v| ((v as f32 * 0.6 + 60.0) * lift).min(255.0) as u8))
            };
            let [w, h] = size;
            let one = image::RgbImage::from_fn(w, h, |x, y| shade(top(&whole, x as i64, (h - 1 - y) as i64)));
            let strips = image::RgbImage::from_fn(w, h, |x, y| {
                let (k, lx) = (x / strip, x % strip);
                // A thin line where each strip starts, so it's plain where they meet
                if lx == 0 && y % 8 < 3 {
                    return image::Rgb([255, 255, 255]);
                }
                shade(top(&pieces[k as usize], lx as i64, (h - 1 - y) as i64))
            });
            let mut differ = 0;
            let diff = image::RgbImage::from_fn(w, h, |x, y| {
                let (k, lx) = (x / strip, x % strip);
                let same = (0..whole.size[2] as i64).all(|z| whole.material_at(x as i64, (h - 1 - y) as i64, z) == pieces[k as usize].material_at(lx as i64, (h - 1 - y) as i64, z));
                if same {
                    image::Rgb([0, 0, 0])
                } else {
                    differ += 1;
                    image::Rgb([255, 0, 0])
                }
            });
            for (kind, picture) in [("one_go", &one), ("strips", &strips), ("difference", &diff)] {
                picture.save(dir.join(format!("seam_{name}_{kind}.png"))).unwrap();
            }
            println!("{name} at {at:?}: {differ} columns differ, pictures in {}", dir.display());
            assert_eq!(differ, 0);
        }
    }

    /// How long a strip of the world takes to build as the world moves on, 8 and 16 m across a
    /// world 224 m wide at 16 voxels a meter, one after the other along x then y, with the cores the
    /// game leaves it: `cargo test --release strip_takes -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn strip_takes() {
        let settings = TerrainSettings { width_m: 224, height_m: 64, voxels_per_meter: 16, seed: 1, grass_tufts: true };
        let cache = RiverCache::default();
        leave_cores(2);
        let side = 224 * 16;
        for strip in [128u32, 256] {
            let mut took = Vec::new();
            for k in 0..6 {
                let start = std::time::Instant::now();
                if k % 2 == 0 {
                    generate_area(&settings, [side + k as i64 * strip as i64, 0], [strip, side as u32], &cache);
                } else {
                    generate_area(&settings, [0, side + k as i64 * strip as i64], [side as u32, strip], &cache);
                }
                took.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            println!("Strips {} m across: {:?} ms", strip / 16, took.iter().map(|t| t.round()).collect::<Vec<_>>());
        }
    }

    /// How long holes blown in the ground and stains splashed on it take, as big as a fight's and
    /// as detailed as the game's world: `cargo test --release carving_takes -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn carving_takes() {
        let settings = TerrainSettings { width_m: 64, height_m: 64, voxels_per_meter: 16, seed: 1, grass_tufts: true };
        let (_, mut world) = generate_terrain(&settings, [0, 0], &RiverCache::default());
        let vpm = 16.0;
        let ground = |world: &World, x: f64, y: f64| (0..world.size[2] as i64).rev().find(|&z| !is_passable(world.material_at(x as i64, y as i64, z))).unwrap_or(0) as f64;
        let mut took = Vec::new();
        let mut painted = Vec::new();
        // What came of it all, to see a quicker way of doing it comes to the same
        let mut sum = 0xcbf2_9ce4_8422_2325u64;
        let mut mix = |v: u64| sum = (sum ^ v).wrapping_mul(0x100_0000_01b3);
        let mut boxes = Vec::new();
        for k in 0..24 {
            let (x, y) = ((12.0 + (k % 6) as f64 * 7.0) * vpm, (12.0 + (k / 6) as f64 * 9.0) * vpm);
            let radius = 1.2 + 1.3 * ((k * 7) % 10) as f64 / 10.0;
            let z = ground(&world, x, y);
            let start = std::time::Instant::now();
            let (changed, blown) = world.carve([x, y, z], radius * vpm);
            took.push((start.elapsed().as_secs_f64() * 1000.0, radius, changed.len()));
            blown.iter().for_each(|&m| mix(m as u64));
            mix(changed.len() as u64);
            let start = std::time::Instant::now();
            let changed = world.paint([x + 3.0 * vpm, y, ground(&world, x + 3.0 * vpm, y)], 0.6 * vpm, DIRT, true);
            painted.push((start.elapsed().as_secs_f64() * 1000.0, changed.len()));
            boxes.push(([x, y, z].map(|v| v as i64), (radius * vpm) as i64 + 4 + 3 * vpm as i64));
        }
        for (ms, radius, bricks) in &took {
            println!("hole {radius:.2} m: {ms:.1} ms, {bricks} bricks");
        }
        for ([x, y, z], r) in boxes {
            for zz in z - r..z + r + 48 {
                for yy in y - r..y + r {
                    for xx in x - r..x + r {
                        mix(world.material_at(xx, yy, zz) as u64);
                    }
                }
            }
        }
        if let Some(water) = &world.water {
            water.ground.iter().for_each(|g| mix(g.to_bits() as u64));
        }
        println!("what came of it: {sum:016x}");
        let worst = took.iter().map(|t| t.0).fold(0.0, f64::max);
        let mean = took.iter().map(|t| t.0).sum::<f64>() / took.len() as f64;
        let stain = painted.iter().map(|t| t.0).fold(0.0, f64::max);
        println!("holes: mean {mean:.1} ms, worst {worst:.1} ms; stains: worst {stain:.2} ms");
    }

    /// How long pieces of the world take to build, as big and as detailed as the game builds them:
    /// `cargo test --release generating -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn generating_takes() {
        let settings = TerrainSettings { width_m: 224, height_m: 64, voxels_per_meter: 16, seed: 1, grass_tufts: true };
        let cache = RiverCache::default();
        let side = 224 * 16;
        // The last round the fallen city nearest the start, see undercity.rs
        for origin in [[0, 0], [side, 0], [0, side], [-side, -side], [2 * side, side], [2560, 13312]] {
            generate_terrain(&settings, origin, &cache);
        }
    }
}
