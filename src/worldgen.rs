use std::collections::HashMap;
use std::f32::consts::{FRAC_1_SQRT_2, FRAC_PI_2, TAU};
use std::sync::{Arc, Mutex};
use std::thread;

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


// The world is split into 8x8x8 bricks. Bricks that are empty or filled with a
// single material only take up one grid cell, the rest store every voxel.
const BRICK_VOXELS: usize = 512;
/// u32 words per stored brick: 16 words of occupancy bits, then 128 words of materials (one byte per voxel).
/// Has to match BRICK_WORDS in shaders\traverse.comp.
pub const BRICK_WORDS: usize = 16 + 128;
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
        LEAVES | LEAVES_DARK | LEAVES_YELLOW | LEAVES_ORANGE | LEAVES_RED | BLOSSOM | WILLOW | PALM_LEAF | PINE | FERN | VINE | GRASS_TIP
            | FLOWER_RED | FLOWER_YELLOW | FLOWER_PURPLE | FLOWER_WHITE | FLOWER_BLUE | FLOWER_PINK
    )
}

pub struct World {
    /// Size in voxels, all multiples of 8.
    pub size: [u32; 3],
    /// Where this world's first voxel is, in voxels of its own size from the corner of the
    /// whole landscape. Zero unless it's a piece of one, like the fine detail around the player.
    pub origin: [i64; 3],
    /// One cell per brick, indexed by x + y * bricks.x + z * bricks.x * bricks.y.
    /// EMPTY_BRICK | distance is empty, SOLID_BRICK | material is filled,
    /// anything else is 1 + the index of a stored brick.
    pub brick_grid: Vec<u32>,
    /// BRICK_WORDS per stored brick. Within a brick, voxels are indexed by x + y * 8 + z * 64.
    pub bricks: Vec<u32>,
    pub voxels_per_meter: u32,
    /// Height of the water surface in voxels, which carries on as an endless sea outside the world.
    /// None for worlds without water.
    pub sea_level: Option<f32>,
    /// Where grass grows, for the triangle grass in the shader: one value per column of voxels
    /// (x + y * size.x), with the ground's height in quarter voxels in bits 0-15 and how dense
    /// the grass is in bits 16-23 (0 where none grows). None for worlds without grass.
    pub grass: Option<Vec<u32>>,
    /// Where the water starts out, for the simulation in src\water.rs. Water isn't in the voxels.
    /// None for worlds without water.
    pub water: Option<WaterMap>,
}

/// One value per column of voxels (x + y * size.x), in voxels: the top of the ground, and the
/// top of the water on it, 0 where it's dry
pub struct WaterMap {
    pub ground: Vec<f32>,
    pub surface: Vec<f32>,
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
        }
    }

    pub fn brick_count(&self) -> usize {
        self.bricks.len() / BRICK_WORDS
    }

    /// Material of a voxel, counted from this world's first voxel. AIR outside the world.
    pub fn material_at(&self, x: i64, y: i64, z: i64) -> u8 {
        let [sx, sy, sz] = self.size.map(i64::from);
        if x < 0 || y < 0 || z < 0 || x >= sx || y >= sy || z >= sz {
            return AIR;
        }
        let (bx, by) = (sx / 8, sy / 8);
        let cell = self.brick_grid[(x / 8 + (y / 8 + (z / 8) * by) * bx) as usize];
        if cell & EMPTY_BRICK != 0 {
            return AIR;
        }
        if cell & SOLID_BRICK != 0 {
            return cell as u8;
        }
        let i = (x % 8 + (y % 8) * 8 + (z % 8) * 64) as usize;
        (self.bricks[((cell & BRICK_INDEX) - 1) as usize * BRICK_WORDS + 16 + i / 4] >> ((i % 4) * 8)) as u8
    }

    /// Blows a hole in the world: everything within `radius` voxels of `center`, in voxels of this
    /// world, is gone, with a ragged edge, and grass and flowers left hanging over the hole fall in
    /// with it. Returns the cells of the bricks that changed, and some of what was blown out, for
    /// debris.
    pub fn carve(&mut self, center: [f64; 3], radius: f64) -> (Vec<usize>, Vec<u8>) {
        let [sx, sy, sz] = self.size.map(i64::from);
        let (bx, by) = (sx / 8, sy / 8);
        let mut dirty: Vec<(usize, [u8; BRICK_VOXELS])> = Vec::new();
        let mut found: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        let mut blown = Vec::new();

        // The bricks being changed, unpacked. Empty ones have nothing to blow away.
        let brick = |world: &World, dirty: &mut Vec<(usize, [u8; BRICK_VOXELS])>, found: &mut std::collections::HashMap<usize, usize>, x: i64, y: i64, z: i64| -> Option<usize> {
            if x < 0 || y < 0 || z < 0 || x >= sx || y >= sy || z >= sz {
                return None;
            }
            let cell = (x / 8 + (y / 8 + (z / 8) * by) * bx) as usize;
            if let Some(&k) = found.get(&cell) {
                return Some(k);
            }
            let value = world.brick_grid[cell];
            if value & EMPTY_BRICK != 0 {
                return None;
            }
            let mut materials = [AIR; BRICK_VOXELS];
            if value & SOLID_BRICK != 0 {
                materials = [value as u8; BRICK_VOXELS];
            } else {
                let start = ((value & BRICK_INDEX) - 1) as usize * BRICK_WORDS + 16;
                for (i, m) in materials.iter_mut().enumerate() {
                    *m = (world.bricks[start + i / 4] >> ((i % 4) * 8)) as u8;
                }
            }
            dirty.push((cell, materials));
            found.insert(cell, dirty.len() - 1);
            Some(dirty.len() - 1)
        };
        let local = |x: i64, y: i64, z: i64| (x % 8 + (y % 8) * 8 + (z % 8) * 64) as usize;

        let r = radius.ceil() as i64 + 1;
        let [cx, cy, cz] = center.map(|v| v.floor() as i64);
        for z in cz - r..=cz + r {
            for y in cy - r..=cy + r {
                for x in cx - r..=cx + r {
                    let d = ((x as f64 + 0.5 - center[0]).powi(2) + (y as f64 + 0.5 - center[1]).powi(2) + (z as f64 + 0.5 - center[2]).powi(2)).sqrt();
                    let ragged = 0.82 + 0.3 * unit(hash3(x as u32, y as u32, z as u32)) as f64;
                    if d > radius * ragged || self.material_at(x, y, z) == AIR {
                        continue;
                    }
                    let Some(k) = brick(self, &mut dirty, &mut found, x, y, z) else {
                        continue;
                    };
                    let i = local(x, y, z);
                    if blown.len() < 64 && hash3(x as u32, y as u32, z as u32 ^ 0xb1) % 6 == 0 {
                        blown.push(dirty[k].1[i]);
                    }
                    dirty[k].1[i] = AIR;
                }
            }
        }

        // Grass and flowers with nothing under them any more fall in
        let plant = |m: u8| matches!(m, GRASS_BLADE | GRASS_TIP | DRY_GRASS | FLOWER_RED | FLOWER_YELLOW | FLOWER_PURPLE | FLOWER_WHITE | FLOWER_BLUE | FLOWER_PINK | REED | CATTAIL);
        for y in cy - r - 1..=cy + r + 1 {
            for x in cx - r - 1..=cx + r + 1 {
                for z in cz - r..=cz + r + 12 {
                    let Some(k) = brick(self, &mut dirty, &mut found, x, y, z) else {
                        continue;
                    };
                    if !plant(dirty[k].1[local(x, y, z)]) {
                        continue;
                    }
                    let below = match brick(self, &mut dirty, &mut found, x, y, z - 1) {
                        Some(b) => dirty[b].1[local(x, y, z - 1)],
                        None => AIR,
                    };
                    if below == AIR {
                        dirty[k].1[local(x, y, z)] = AIR;
                    }
                }
            }
        }

        // Pack the changed bricks back up, in place where they were stored already
        let mut changed = Vec::new();
        for (cell, materials) in dirty {
            let old = self.brick_grid[cell];
            let first = materials[0];
            let value = if materials.iter().all(|&m| m == first) {
                if first == AIR { EMPTY_BRICK | 1 } else { SOLID_BRICK | first as u32 }
            } else if old & (SOLID_BRICK | EMPTY_BRICK) == 0 {
                let index = ((old & BRICK_INDEX) - 1) as usize;
                let words = &mut self.bricks[index * BRICK_WORDS..(index + 1) * BRICK_WORDS];
                words.fill(0);
                for (i, &m) in materials.iter().enumerate() {
                    if m != AIR {
                        words[i / 32] |= 1 << (i % 32);
                    }
                    words[16 + i / 4] |= (m as u32) << ((i % 4) * 8);
                }
                let foliage = if materials.iter().any(|&m| sways(m)) { FOLIAGE_BRICK } else { 0 };
                (index as u32 + 1) | foliage
            } else {
                push_brick(&materials, &mut self.bricks)
            };
            if value != old {
                self.brick_grid[cell] = value;
            }
            changed.push(cell);
        }

        // Water can flow down into the hole
        if self.water.is_some() {
            let mut lowered = Vec::new();
            for y in (cy - r).max(0)..(cy + r + 1).min(sy) {
                for x in (cx - r).max(0)..(cx + r + 1).min(sx) {
                    let i = (x + y * sx) as usize;
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

    /// The words of the stored brick in `cell`, if it's a stored brick
    pub fn brick_words(&self, cell: usize) -> Option<(usize, &[u32])> {
        let value = self.brick_grid[cell];
        if value & (SOLID_BRICK | EMPTY_BRICK) != 0 {
            return None;
        }
        let index = ((value & BRICK_INDEX) - 1) as usize;
        Some((index, &self.bricks[index * BRICK_WORDS..(index + 1) * BRICK_WORDS]))
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
pub fn generate_terrain(settings: &TerrainSettings, origin: [i64; 2], rivers: &RiverCache) -> (Terrain, World) {
    let start = std::time::Instant::now();

    let mut heights = Heights::new(settings, origin, [settings.size()[0] as usize; 2], settings.voxels_per_meter);
    let terrain = Terrain::new(settings, &mut heights, rivers);
    let mut world = terrain.build_from(&heights, 0, settings.size()[2]);
    world.grass = Some(terrain.grass_map(&heights));
    let mut water = terrain.water_map(&heights);
    terrain.raise_water_ground(&world, &mut water);
    world.water = Some(water);

    println!(
        "Generated terrain around ({}, {}): {} bricks stored, {} MB ({:?})",
        origin[0],
        origin[1],
        world.brick_count(),
        world.bricks.len() * 4 / (1024 * 1024),
        start.elapsed()
    );

    (terrain, world)
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
            min: [center[0] - half[0].max(half[1]), center[1] - half[0].max(half[1]), center[2] - half[2]],
            max: [center[0] + half[0].max(half[1]), center[1] + half[0].max(half[1]), center[2] + half[2]],
        }
    }

    /// What a voxel inside the shape is made of. `p` is its middle, `h` a hash of where it is.
    fn material_at(&self, p: Vec3, voxel: f32, h: u32) -> u8 {
        let sprinkled = |chance: f32| self.accent != AIR && unit(h >> 16) < chance;
        let dark = if matches!(self.material, SANDSTONE | CLAY_POT) { SANDSTONE_DARK } else { MORTAR };
        match self.pattern {
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
/// better what's in it is likely to be: `luck` as for Library::roll_rarity().
fn chest(features: &mut Features, at: Vec3, heading: f32, luck: f32) {
    features.props.push(Prop { kind: PropKind::Chest, position: at, heading, luck });
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
}

/// The ground under a rectangle of voxel columns, at some resolution.
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
    /// How far each column is from the nearest river point reaching it, in meters
    river_distance: Vec<f32>,
    /// How low that point cuts the column, in meters: the bed of the river, or its bank.
    /// INFINITY where no river reaches.
    carved: Vec<f32>,
}

impl Heights {
    fn new(settings: &TerrainSettings, origin: [i64; 2], size: [usize; 2], voxels_per_meter: u32) -> Self {
        let vpm = voxels_per_meter as f32;
        let mut ground = vec![0.0; size[0] * size[1]];

        // Rows split across threads
        let threads = thread::available_parallelism().map_or(4, |n| n.get());
        let rows_per_thread = size[1].div_ceil(threads).max(1);
        thread::scope(|s| {
            for (chunk_index, chunk) in ground.chunks_mut(rows_per_thread * size[0]).enumerate() {
                s.spawn(move || {
                    for (i, h) in chunk.iter_mut().enumerate() {
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
            river_distance: vec![f32::INFINITY; size[0] * size[1]],
            carved: vec![f32::INFINITY; size[0] * size[1]],
        }
    }

    /// Ground height in meters at a column, counted from this rectangle's corner (clamped to it)
    fn ground_m(&self, x: i64, y: i64) -> f32 {
        let x = x.clamp(0, self.size[0] as i64 - 1) as usize;
        let y = y.clamp(0, self.size[1] as i64 - 1) as usize;
        self.ground[x + y * self.size[0]]
    }


    /// Cuts a river's channel and banks into the ground. Call settle_beds() once every river is in.
    fn carve(&mut self, path: &[RiverPoint]) {
        let vpm = self.voxels_per_meter;
        // Banks slope back up to the land around
        let bank = 2.5;
        let [sx, sy] = self.size.map(|n| n as i64);

        for p in path {
            let reach = p.half_width + bank;
            // Shallow at the lip of a waterfall, so the pool behind it holds its water
            let depth = if p.lip { 0.1 } else { 0.6 + p.half_width * 0.25 };
            let lo = [(p.x - reach) * vpm, (p.y - reach) * vpm].map(|v| v.floor() as i64);
            let hi = [(p.x + reach) * vpm, (p.y + reach) * vpm].map(|v| v.ceil() as i64);
            let (x0, x1) = ((lo[0] - self.origin[0]).max(0), (hi[0] - self.origin[0]).min(sx - 1));
            let (y0, y1) = ((lo[1] - self.origin[1]).max(0), (hi[1] - self.origin[1]).min(sy - 1));
            for cy in y0..=y1 {
                for cx in x0..=x1 {
                    let (xm, ym) = (
                        (self.origin[0] + cx) as f32 / vpm,
                        (self.origin[1] + cy) as f32 / vpm,
                    );
                    // Each column is shaped by its nearest point alone, so where the river drops
                    // over a waterfall there's a sheer edge, with the banks of the pool above
                    // still standing right up to it. Distances wobble differently for each pool,
                    // so that edge is ragged rather than a straight line.
                    let pool = (p.surface * 4.0) as i32 as u32;
                    let d = (xm - p.x).hypot(ym - p.y) + 0.7 * fbm(xm / 1.5, ym / 1.5, 2, self.seed ^ hash(pool));
                    if d > reach {
                        continue;
                    }
                    let i = (cx + cy * sx) as usize;
                    if d >= self.river_distance[i] {
                        continue;
                    }
                    self.river_distance[i] = d;
                    if d < p.half_width {
                        let t = (d / p.half_width).max(0.0);
                        self.river[i] = p.surface;
                        self.carved[i] = p.surface - depth * (1.0 - t * t).max(if p.lip { 1.0 } else { 0.0 });
                    } else {
                        // Rocky banks: their steepness wanders, with knobs and hollows in them, but
                        // they always stand clear of the water so it doesn't spill out
                        let out = d - p.half_width;
                        let wander = fbm(xm / 4.0, ym / 4.0, 3, self.seed.wrapping_add(21));
                        let knobs = fbm(xm / 1.2, ym / 1.2, 2, self.seed.wrapping_add(22));
                        let rough = (knobs * 0.5 + wander * 0.6) * out.min(1.0);
                        self.river[i] = f32::NEG_INFINITY;
                        self.carved[i] = (p.surface + out * 0.9 * (1.0 + 0.5 * wander) + rough).max(p.surface + 0.15);
                    }
                }
            }
        }
    }

    /// Cuts the beds and banks of all the rivers into the ground. Beds are laid in even where the
    /// ground was lower, so the lips of waterfalls hold back the pools behind them.
    fn settle_beds(&mut self) {
        for i in 0..self.ground.len() {
            if self.river[i].is_finite() {
                self.ground[i] = self.carved[i];
            } else {
                self.ground[i] = self.ground[i].min(self.carved[i]);
            }
        }
    }
}

// Terrain shapes are defined in meters, so more voxels per meter means the same landscape with finer voxels
fn height_at(settings: &TerrainSettings, x: f32, y: f32) -> f32 {
    let seed = settings.seed;
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
                _ => 1.0 - c.jungle.max(c.desert).max(c.snow),
            };
            if strength > 0.95 && height_at(settings, x, y) > sea_level + 3.0 {
                return Some([x, y]);
            }
        }
    }
    None
}

fn is_grassy(material: u8) -> bool {
    matches!(material, GRASS | JUNGLE_GRASS)
}

fn sea_level_m(settings: &TerrainSettings) -> f32 {
    settings.height_m as f32 * 0.3
}

/// The landscape goes on forever. Rivers start at springs, at most one in each RIVER_CELL meter
/// square, and once traced they're kept here, so the same river turns up whichever part of the
/// landscape gets built. Shared between the parts built so far and the next.
#[derive(Default)]
pub struct RiverCache {
    cells: Mutex<HashMap<(i32, i32), Option<Arc<Vec<RiverPoint>>>>>,
}

const RIVER_CELL: f32 = 200.0;
/// The most points a river runs to, half a meter apart
const RIVER_STEPS: usize = 1200;
/// How much water a river's spring gives, in cubic meters a second, and how wide it is in meters
const RIVER_FLOW: f32 = 4.0;
const RIVER_SPRING_RADIUS: f32 = 2.0;

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
    /// For chests, how much rarer what's in it is likely to be, see Library::roll_rarity()
    pub luck: f32,
}

/// Something glowing that lights up what's around it, like a candle flame. In meters.
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
    sites: Vec<Site>,
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
    /// The piece of the landscape this covers, in meters
    lo: [f32; 2],
    hi: [f32; 2],
    rivers: Vec<Arc<Vec<RiverPoint>>>,
    objects: Vec<Object>,
    // Indices into `objects` for each OBJECT_CELL-sized square they overlap, to find them by area
    object_cells: Vec<Vec<u32>>,
    cells_across: [usize; 2],
    features: Features,
}

const OBJECT_CELL: f32 = 8.0;

impl Terrain {
    /// `heights` covers the piece of the landscape to build, and gets the rivers carved into it.
    fn new(settings: &TerrainSettings, heights: &mut Heights, rivers: &RiverCache) -> Self {
        let vpm = heights.voxels_per_meter;
        let lo = [heights.origin[0] as f32 / vpm, heights.origin[1] as f32 / vpm];
        let hi = [lo[0] + heights.size[0] as f32 / vpm, lo[1] + heights.size[1] as f32 / vpm];
        let mut terrain = Terrain {
            settings: *settings,
            lo,
            hi,
            rivers: Vec::new(),
            objects: Vec::new(),
            object_cells: Vec::new(),
            cells_across: [0; 2],
            features: Features::default(),
        };

        terrain.rivers = terrain.rivers_in(rivers);
        for path in &terrain.rivers {
            heights.carve(path);
        }
        heights.settle_beds();

        let (objects, features) = terrain.place_objects(heights);
        terrain.objects = objects;
        terrain.features = features;

        let cells = [0, 1].map(|i| ((hi[i] - lo[i]) / OBJECT_CELL).ceil() as usize);
        let mut object_cells = vec![Vec::new(); cells[0] * cells[1]];
        let cell = |v: f32, axis: usize| (((v - lo[axis]) / OBJECT_CELL).floor().max(0.0) as usize).min(cells[axis] - 1);
        for (i, object) in terrain.objects.iter().enumerate() {
            for cy in cell(object.min[1], 1)..=cell(object.max[1], 1) {
                for cx in cell(object.min[0], 0)..=cell(object.max[0], 0) {
                    object_cells[cx + cy * cells[0]].push(i as u32);
                }
            }
        }
        terrain.object_cells = object_cells;
        terrain.cells_across = cells;

        terrain
    }

    fn inside(&self, x: f32, y: f32) -> bool {
        x >= self.lo[0] && y >= self.lo[1] && x < self.hi[0] && y < self.hi[1]
    }

    /// Where rivers pour over waterfalls in this piece: the lip of each in meters, and which way
    /// the river flows there
    pub fn waterfalls(&self) -> Vec<([f32; 3], [f32; 2])> {
        let mut falls = Vec::new();
        for path in &self.rivers {
            for pair in path.windows(2) {
                let (a, b) = (&pair[0], &pair[1]);
                if a.surface - b.surface > 1.0 && self.inside(a.x, a.y) {
                    let (dx, dy) = (b.x - a.x, b.y - a.y);
                    let length = dx.hypot(dy).max(1e-6);
                    falls.push(([a.x, a.y, a.surface], [dx / length, dy / length]));
                }
            }
        }
        falls
    }

    /// Where water comes from in this piece: x, y, cubic meters a second and radius, in meters.
    /// Rivers start where their spring is, or where they flow in from outside it, and some
    /// buildings have water welling up inside.
    pub fn springs(&self) -> Vec<[f32; 4]> {
        let mut springs: Vec<[f32; 4]> = self
            .rivers
            .iter()
            .filter_map(|path| path.iter().find(|p| self.inside(p.x, p.y)))
            .map(|p| [p.x, p.y, RIVER_FLOW, RIVER_SPRING_RADIUS])
            .collect();
        springs.extend_from_slice(&self.features.springs);
        springs
    }

    /// Candles, skulls and such, in meters
    pub fn props(&self) -> &[Prop] {
        &self.features.props
    }

    /// Flames and glowing crystals, in meters
    pub fn lights(&self) -> &[Light] {
        &self.features.lights
    }

    /// The buildings in this piece of the landscape
    pub fn sites(&self) -> &[Site] {
        &self.features.sites
    }

    /// Builds a box of the landscape into voxels. `origin` and `size` are in voxels at
    /// `voxels_per_meter`, counted from the corner of the whole landscape at that resolution.
    /// It has to be within this piece of it. Without water, which is simulated separately,
    /// see src\water.rs.
    pub fn build(&self, origin: [i64; 3], size: [u32; 3], voxels_per_meter: u32) -> World {
        let mut heights = Heights::new(&self.settings, [origin[0], origin[1]], [size[0] as usize, size[1] as usize], voxels_per_meter);
        for path in &self.rivers {
            heights.carve(path);
        }
        heights.settle_beds();
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

    /// The ground and the water lying on it for every column, see World::water
    fn water_map(&self, heights: &Heights) -> WaterMap {
        let [sx, sy] = heights.size;
        let mut ground = vec![0.0f32; sx * sy];
        let mut surface = vec![0.0f32; sx * sy];
        let threads = thread::available_parallelism().map_or(4, |n| n.get());
        let rows_per_thread = sy.div_ceil(threads).max(1);
        thread::scope(|s| {
            for (chunk_index, (ground, surface)) in ground
                .chunks_mut(rows_per_thread * sx)
                .zip(surface.chunks_mut(rows_per_thread * sx))
                .enumerate()
            {
                s.spawn(move || {
                    for i in 0..ground.len() {
                        let j = chunk_index * rows_per_thread * sx + i;
                        let column = self.column(heights, (j % sx) as i64, (j / sx) as i64);
                        ground[i] = (column.height + 1) as f32;
                        surface[i] = if column.water_top > column.height { (column.water_top + 1) as f32 } else { 0.0 };
                    }
                });
            }
        });
        WaterMap { ground, surface }
    }

    fn build_from(&self, heights: &Heights, origin_z: i64, size_z: u32) -> World {
        let vpm = heights.voxels_per_meter;
        let [bx, by] = heights.size.map(|n| n / 8);
        let bz = size_z as usize / 8;
        let origin = [heights.origin[0], heights.origin[1], origin_z];

        // The objects reaching into this box, and for each column of bricks which of them reach into it
        let lo = [origin[0] as f32 / vpm, origin[1] as f32 / vpm];
        let hi = [
            (origin[0] + heights.size[0] as i64) as f32 / vpm,
            (origin[1] + heights.size[1] as i64) as f32 / vpm,
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
        let threads = thread::available_parallelism().map_or(4, |n| n.get());
        let rows_per_thread = by.div_ceil(threads).max(1);
        let results: Vec<(Vec<(usize, u32)>, Vec<u32>)> = thread::scope(|s| {
            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    let (objects, objects_by_column) = (&objects, &objects_by_column);
                    s.spawn(move || {
                        let mut cells = Vec::new();
                        let mut bricks = Vec::new();
                        for cy in (t * rows_per_thread)..((t + 1) * rows_per_thread).min(by) {
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

    /// See World::grass. Grass grows on grassy ground, a little thinner in the forests.
    fn grass_map(&self, heights: &Heights) -> Vec<u32> {
        let [sx, sy] = heights.size;
        let vpm = heights.voxels_per_meter;
        let seed = self.settings.seed;
        let mut map = vec![0u32; sx * sy];

        // Rows split across threads
        let threads = thread::available_parallelism().map_or(4, |n| n.get());
        let rows_per_thread = sy.div_ceil(threads).max(1);
        thread::scope(|s| {
            for (chunk_index, chunk) in map.chunks_mut(rows_per_thread * sx).enumerate() {
                s.spawn(move || {
                    for (i, value) in chunk.iter_mut().enumerate() {
                        let j = chunk_index * rows_per_thread * sx + i;
                        let (x, y) = ((j % sx) as i64, (j / sx) as i64);
                        let column = self.column(heights, x, y);
                        if !is_grassy(column.top) {
                            continue;
                        }
                        let forest = smoothstep(-0.15, 0.25, fbm(x as f32 / vpm / 80.0, y as f32 / vpm / 80.0, 3, seed.wrapping_add(4)));
                        let density = (255.0 * (1.0 - 0.45 * forest)) as u32;
                        let ground = ((column.height + 1) as u32 * 4).min(0xFFFF);
                        *value = ground | density << 16;
                    }
                });
            }
        });

        map
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
        let i = (x.clamp(0, heights.size[0] as i64 - 1) + y.clamp(0, heights.size[1] as i64 - 1) * heights.size[0] as i64) as usize;
        let river = (heights.river[i] * vpm).floor() as i32;
        let water_top = if river > h { river.max(sea_level) } else { sea_level };

        // Which kind of land this is, with ragged edges where one meets the next
        let (xm, ym) = (gx as f32 / vpm, gy as f32 / vpm);
        let c = climate(&self.settings, xm, ym);
        let edge = (unit(hash3(gx as u32, gy as u32, self.settings.seed ^ 0xb10e)) - 0.5) * 0.3;
        let (jungle, desert, snowy) = (c.jungle + edge > 0.5, c.desert + edge > 0.5, c.snow + edge > 0.5);

        let top = if h <= sea_level + v {
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
        let below = match top {
            SAND | STONE | GRAVEL => top,
            _ => DIRT,
        };

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
            let (lush, chance) = if jungle { (1.0, 0.75) } else { (lush, 0.25 + 0.35 * lush) };
            if unit(r) < flower_chance {
                blade = 1 + ((r >> 16) % max_stem) as i32;
                let patch = hash3((xm / 4.0) as u32, (ym / 4.0) as u32, self.settings.seed ^ 0xf10e);
                blade_top = [FLOWER_RED, FLOWER_YELLOW, FLOWER_PURPLE, FLOWER_WHITE][(patch % 4) as usize];
            } else if self.settings.grass_tufts && unit(tuft) < chance {
                // Taller in lush patches, with a lighter tip
                let tallest = (0.08 + if jungle { 0.4 } else { 0.22 } * lush) * vpm;
                blade = ((unit(tuft >> 16) * tallest).round() as i32).max(1);
                blade_top = GRASS_TIP;
            }
        } else if top == SAND && desert && self.settings.grass_tufts && unit(tuft) < 0.04 {
            blade = ((unit(tuft >> 16) * 0.25 * vpm).round() as i32).max(1);
            (blade_body, blade_top) = (DRY_GRASS, DRY_GRASS);
        }

        let strata = (fbm(xm / 7.0, ym / 7.0, 2, self.settings.seed.wrapping_add(23)) * 0.8 * vpm) as i32;
        let moss = smoothstep(0.0, 0.5, fbm(xm / 3.0, ym / 3.0, 2, self.settings.seed.wrapping_add(24))) * if desert { 0.0 } else { 0.6 };

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
        }
    }

    /// Bare rock: layers a few tens of centimeters thick, gently wavy, of a few kinds of stone,
    /// with moss growing in patches. Sandstone in the deserts.
    fn rock(&self, column: &Column, z: i32) -> u8 {
        let thickness = (0.3 * self.settings.voxels_per_meter as f32).max(1.0) as i32;
        let layer = (z + column.strata).div_euclid(thickness);
        let h = hash3(column.x as u32, column.y as u32, z as u32 ^ self.settings.seed);
        if unit(h) < column.moss * 0.5 {
            return MOSS;
        }
        let pick = hash3(layer as u32, self.settings.seed, 0x57a7a) % 6;
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
        if z <= column.height {
            let depth = column.height - z;
            if depth == 0 && column.top == STONE || depth > 0 && column.below == STONE || depth > dirt_depth {
                self.rock(column, z)
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
            if z0 + 7 < stone_below {
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

            // Trees, plants, rocks and buildings above the ground, and caves and rooms cut into it
            let (z_lo, z_hi) = ((z0 - 1) as f32 * voxel, (z0 + 9) as f32 * voxel);
            nearby.clear();
            nearby.extend(shapes.iter().filter(|(s, _)| s.max[2] >= z_lo && s.min[2] <= z_hi));
            let cuts = nearby.iter().any(|(s, _)| s.placement != Placement::Above);
            if !nearby.is_empty() {
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
                                if depth > 1.0 {
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
                                materials[i] = shape.material_at(p, voxel, h);
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
        let reach = RIVER_STEPS as f32 * 0.5;
        let cell = |v: f32| (v / RIVER_CELL).floor() as i32;
        let (lo, hi) = (self.lo, self.hi);
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
        (h > sea_level_m(&self.settings) + 10.0).then(|| self.trace_river(x, y, hash(r)))
    }

    fn trace_river(&self, x: f32, y: f32, seed: u32) -> Vec<RiverPoint> {
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

        for i in 0..RIVER_STEPS {
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

            // Wider the further it has run
            let progress = (i as f32 * step / 60.0).min(1.0);
            points.push(RiverPoint {
                x,
                y,
                surface: level,
                half_width: 1.0 + 2.0 * progress,
                lip: false,
            });

            if surface <= sea_level {
                break;
            }
        }

        points
    }

    /// Trees, plants, rocks, buildings and caves, anywhere in this piece of the landscape. Each is
    /// placed at a spot that only depends on where it is in the whole landscape, so pieces built
    /// around different places agree where they overlap.
    fn place_objects(&self, heights: &Heights) -> (Vec<Object>, Features) {
        let seed = self.settings.seed;
        let vpm = heights.voxels_per_meter;
        let mut features = Features::default();
        // Buildings go first, as where they overlap other things they win
        let mut objects = self.buildings(heights, &mut features);
        objects.extend(self.caves(&mut features));
        objects.extend(self.ravines(&mut features));
        let footprints = std::mem::take(&mut features.footprints);
        let (lo, hi) = (self.lo, self.hi);

        // Calls `place` for one random spot in every `cell` meter square, with the column there,
        // except where buildings stand
        let mut scatter = |cell: f32, salt: u32, place: &mut dyn FnMut(f32, f32, f32, &Column, u32) -> Option<Object>| {
            let first = [(lo[0] / cell).floor() as i32, (lo[1] / cell).floor() as i32];
            let last = [(hi[0] / cell).ceil() as i32, (hi[1] / cell).ceil() as i32];
            for cy in first[1]..last[1] {
                for cx in first[0]..last[0] {
                    let r = hash3(cx as u32, cy as u32, seed ^ salt);
                    let x = (cx as f32 + unit(r)) * cell;
                    let y = (cy as f32 + unit(r >> 16)) * cell;
                    if x < lo[0] || y < lo[1] || x >= hi[0] || y >= hi[1] {
                        continue;
                    }
                    if footprints.iter().any(|f| x > f[0] - 1.5 && y > f[1] - 1.5 && x < f[2] + 1.5 && y < f[3] + 1.5) {
                        continue;
                    }
                    let column = self.column(heights, (x * vpm).floor() as i64 - heights.origin[0], (y * vpm).floor() as i64 - heights.origin[1]);
                    let ground = (column.height + 1) as f32 / vpm;
                    if let Some(object) = place(x, y, ground, &column, hash(r)) {
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

        // At most one tree per cell, denser where the forest noise is high. The jungle is thick
        // with them, the snow has pine forests, and in the desert only palms grow, by the water.
        scatter(7.5, 0x7a3e, &mut |x, y, ground, column, r| {
            let c = climate(x, y);
            let density = if c.jungle > 0.5 {
                0.95
            } else if column.top == SAND && c.desert > 0.5 {
                if ground - sea_level < 2.5 { 0.25 } else { 0.0 }
            } else if column.top == SNOW || c.snow > 0.5 {
                forest(x, y) * 0.6 + 0.05
            } else {
                forest(x, y) * 0.85 + 0.03
            };
            let soil = is_grassy(column.top) || column.top == SNOW && c.snow > 0.3 || column.top == SAND && c.desert > 0.5;
            (soil && unit(r) < density).then(|| self.tree(x, y, ground, hash(r)))
        });

        // In the jungle, smaller trees in between the big ones
        scatter(4.5, 0x9b1e, &mut |x, y, ground, column, r| {
            let c = climate(x, y);
            (column.top == JUNGLE_GRASS && unit(r) < c.jungle * 0.7).then(|| {
                let mut rng = Rng(hash(r));
                let foliage = Foliage { material: LEAVES_DARK, accent: LEAVES, leaf_length: 0.3, leaves_per_twig: 10, clumps_per_twig: 2 };
                Object::new(self.broadleaf(x, y, ground, 0.8, 2, BARK_DARK, foliage, &mut rng), rng.0)
            })
        });

        // Cacti and dry shrubs in the desert
        scatter(6.0, 0xcac7, &mut |x, y, ground, column, r| {
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

        // Bushes, mostly around the forests
        scatter(4.0, 0xb054, &mut |x, y, ground, column, r| {
            let density = smoothstep(-0.3, 0.2, forest(x, y) * 2.0 - 1.0) * 0.3 + meadow(x, y) * 0.08 + 0.04;
            (is_grassy(column.top) && unit(r) < density).then(|| self.bush(x, y, ground, meadow(x, y), hash(r)))
        });

        // Undergrowth: ferns, big leaves and mushrooms in the forests and all over the jungle,
        // flowers in the meadows, reeds along the shore
        scatter(1.2, 0x9a47, &mut |x, y, ground, column, r| {
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

        // Boulders, half sunk into the ground, of sandstone in the desert
        scatter(14.0, 0xb0d3, &mut |x, y, ground, column, r| {
            let dry = ground > sea_level && column.top != GRAVEL;
            (dry && unit(r) < 0.45).then(|| {
                let mut boulder = self.boulder(x, y, ground - 1.0 / vpm, hash(r));
                if column.top == SAND && climate(x, y).desert > 0.5 {
                    for shape in &mut boulder.shapes {
                        (shape.material, shape.accent) = (SANDSTONE, SANDSTONE_DARK);
                    }
                }
                boulder
            })
        });

        features.footprints = footprints;
        (objects, features)
    }

    /// Temples and ruins on flat, dry ground, at most one in each BUILDING_CELL square, and
    /// gateways over some of the waterfalls
    fn buildings(&self, heights: &Heights, features: &mut Features) -> Vec<Object> {
        const BUILDING_CELL: f32 = 56.0;
        let seed = self.settings.seed;
        let vpm = heights.voxels_per_meter;
        let sea_level = sea_level_m(&self.settings);
        // The ground at a point in this piece, in meters, and whether a river runs there
        let ground = |x: f32, y: f32| -> Option<(f32, bool)> {
            let (gx, gy) = ((x * vpm).floor() as i64 - heights.origin[0], (y * vpm).floor() as i64 - heights.origin[1]);
            if gx < 0 || gy < 0 || gx >= heights.size[0] as i64 || gy >= heights.size[1] as i64 {
                return None;
            }
            let i = gx as usize + gy as usize * heights.size[0];
            Some((heights.ground[i], heights.river[i].is_finite()))
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

                // The whole site has to be in this piece of the landscape, dry and flat enough
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

        // Aztec gateways standing over waterfalls in the jungle, for the river to pour through
        for (lip, flow) in self.waterfalls() {
            let r = hash3((lip[0] * 4.0) as i32 as u32, (lip[1] * 4.0) as i32 as u32, seed ^ 0x9a7e);
            if unit(r) > 0.5 || climate(&self.settings, lip[0], lip[1]).jungle < 0.7 {
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
    fn ravines(&self, features: &mut Features) -> Vec<Object> {
        const RAVINE_CELL: f32 = 100.0;
        const STEP: f32 = 2.0;
        // How far from where they start ravines can reach
        const REACH: f32 = 100.0;
        let seed = self.settings.seed;
        let sea_level = sea_level_m(&self.settings);
        let (lo, hi) = (self.lo, self.hi);
        let mut objects = Vec::new();
        let first = [((lo[0] - REACH) / RAVINE_CELL).floor() as i32, ((lo[1] - REACH) / RAVINE_CELL).floor() as i32];
        let last = [((hi[0] + REACH) / RAVINE_CELL).ceil() as i32, ((hi[1] + REACH) / RAVINE_CELL).ceil() as i32];
        for cy in first[1]..last[1] {
            for cx in first[0]..last[0] {
                let r = hash3(cx as u32, cy as u32, seed ^ 0x7a71);
                if unit(r) > 0.7 {
                    continue;
                }
                let mut rng = Rng(hash(r));
                let (mut x, mut y) = ((cx as f32 + rng.next()) * RAVINE_CELL, (cy as f32 + rng.next()) * RAVINE_CELL);
                if height_at(&self.settings, x, y) < sea_level + 4.0 {
                    continue;
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
                objects.push(Object::new(shapes, rng.0));
            }
        }
        objects
    }

    /// Winding tunnels under the hills, at most one starting in each CAVE_CELL square. Some open
    /// out onto the hillside, and glowing crystals grow on their floors here and there.
    fn caves(&self, features: &mut Features) -> Vec<Object> {
        const CAVE_CELL: f32 = 72.0;
        // How far from where they start caves can wander
        const REACH: f32 = 120.0;
        let seed = self.settings.seed;
        let sea_level = sea_level_m(&self.settings);
        let (lo, hi) = (self.lo, self.hi);
        let mut objects = Vec::new();
        let first = [((lo[0] - REACH) / CAVE_CELL).floor() as i32, ((lo[1] - REACH) / CAVE_CELL).floor() as i32];
        let last = [((hi[0] + REACH) / CAVE_CELL).ceil() as i32, ((hi[1] + REACH) / CAVE_CELL).ceil() as i32];
        for cy in first[1]..last[1] {
            for cx in first[0]..last[0] {
                let r = hash3(cx as u32, cy as u32, seed ^ 0xca7e);
                if unit(r) > 0.5 {
                    continue;
                }
                let mut rng = Rng(hash(r));
                let mut p = [(cx as f32 + rng.next()) * CAVE_CELL, (cy as f32 + rng.next()) * CAVE_CELL, 0.0];
                let surface = height_at(&self.settings, p[0], p[1]);
                if surface < sea_level + 3.0 {
                    continue;
                }
                let mouth = rng.next() < 0.5;
                p[2] = if mouth { surface - 0.5 } else { surface - rng.range(6.0, 14.0) };
                let mut yaw = rng.next() * TAU;
                let mut pitch: f32 = if mouth { -0.35 } else { 0.0 };
                let mut radius = rng.range(1.2, 1.8);
                let mut shapes = Vec::new();
                let mut crystals = Vec::new();
                for i in 0..30 + (rng.next() * 40.0) as usize {
                    let next_radius = (radius + rng.range(-0.25, 0.25)).clamp(1.0, 2.4);
                    let dir = [yaw.cos() * pitch.cos(), yaw.sin() * pitch.cos(), pitch.sin()];
                    let q = add(p, scale(dir, 1.6));
                    shapes.push(Shape::capsule(p, q, radius, next_radius, AIR, AIR, false).placed(Placement::Carve));
                    if i % 9 == 4 {
                        let at = [q[0] + rng.range(-0.5, 0.5), q[1] + rng.range(-0.5, 0.5), q[2] - next_radius * 0.8];
                        for _ in 0..3 {
                            let offset = [rng.range(-0.35, 0.35), rng.range(-0.35, 0.35), rng.range(0.0, 0.3)];
                            crystals.push(Shape::sphere(add(at, offset), rng.range(0.15, 0.3), CRYSTAL, AIR, false).placed(Placement::Inside));
                        }
                        if self.inside(at[0], at[1]) {
                            features.lights.push(Light { position: [at[0], at[1], at[2] + 0.5], color: [0.35, 0.85, 1.0], reach: 7.0 });
                        }
                    }
                    // Wander about, staying well under the ground, and never under the sea
                    let depth = height_at(&self.settings, q[0], q[1]) - q[2];
                    yaw += rng.range(-0.35, 0.35);
                    let want = if depth < 4.0 && i > 4 { -0.3 } else if depth > 16.0 { 0.25 } else { rng.range(-0.15, 0.15) };
                    pitch = (pitch * 0.7 + want * 0.3).clamp(-0.5, 0.5);
                    p = q;
                    radius = next_radius;
                    if q[2] < sea_level - 2.0 {
                        break;
                    }
                }
                shapes.extend(crystals);
                objects.push(Object::new(shapes, rng.0));
            }
        }
        objects
    }

    // Which kind of tree grows where: pines up high, palms and willows by the water, and groves of
    // one kind or another elsewhere
    fn tree(&self, x: f32, y: f32, base: f32, r: u32) -> Object {
        let seed = self.settings.seed;
        let mut rng = Rng(r);

        let sea_level = sea_level_m(&self.settings);
        let altitude = (base - sea_level) / (self.settings.height_m as f32 - sea_level);
        let pine_region = fbm(x / 90.0, y / 90.0, 2, seed.wrapping_add(8));
        let pines = smoothstep(0.25, 0.45, altitude + pine_region * 0.3);
        let grove = fbm(x / 50.0, y / 50.0, 2, seed.wrapping_add(9)) * 0.5 + 0.5;

        let c = climate(&self.settings, x, y);
        if c.jungle > 0.5 {
            return Object::new(self.jungle_tree(x, y, base, &mut rng), rng.0);
        }
        if c.desert > 0.5 {
            return Object::new(self.palm(x, y, base, &mut rng), rng.0);
        }
        if c.snow > 0.5 {
            // Pines with snow on their branches
            let mut shapes = if rng.next() < 0.5 { self.spruce(x, y, base, &mut rng) } else { self.fir(x, y, base, &mut rng) };
            for shape in &mut shapes {
                if shape.material == PINE {
                    shape.accent = SNOW;
                }
            }
            return Object::new(shapes, rng.0);
        }

        let pick = rng.next();
        let shapes = if altitude > 0.55 && pick < 0.12 {
            self.broadleaf(x, y, base, 1.0, 2, DEAD_WOOD, BARE, &mut rng)
        } else if pick < pines {
            if rng.next() < 0.5 { self.spruce(x, y, base, &mut rng) } else { self.fir(x, y, base, &mut rng) }
        } else if base - sea_level < 2.5 {
            if rng.next() < 0.6 { self.palm(x, y, base, &mut rng) } else { self.willow(x, y, base, &mut rng) }
        } else {
            let choice = grove * 0.7 + rng.next() * 0.3;
            let (leaves, accent) = self.leaf_colors(x, y, &mut rng);
            let foliage = Foliage {
                material: leaves,
                accent,
                leaf_length: 0.3,
                leaves_per_twig: 42,
                clumps_per_twig: 1,
            };
            if choice < 0.3 {
                self.broadleaf(x, y, base, 1.0, 2, WOOD, foliage, &mut rng)
            } else if choice < 0.45 {
                self.birch(x, y, base, &mut rng)
            } else if choice < 0.55 {
                let blossom = Foliage {
                    material: BLOSSOM,
                    accent: FLOWER_WHITE,
                    leaf_length: 0.22,
                    ..foliage
                };
                self.broadleaf(x, y, base, 0.8, 2, BARK_DARK, blossom, &mut rng)
            } else if choice < 0.65 {
                self.poplar(x, y, base, leaves, accent, &mut rng)
            } else if choice < 0.75 {
                self.willow(x, y, base, &mut rng)
            } else if rng.next() < 0.08 {
                // The odd giant
                self.broadleaf(x, y, base, 2.2, 3, WOOD, foliage, &mut rng)
            } else {
                self.broadleaf(x, y, base, 1.2, 2, WOOD, foliage, &mut rng)
            }
        };
        Object::new(shapes, rng.0)
    }

    /// A jungle giant: a tall trunk on buttress roots, a wide crown of dark leaves, and vines
    /// hanging down from it
    fn jungle_tree(&self, x: f32, y: f32, base: f32, rng: &mut Rng) -> Vec<Shape> {
        let height = rng.range(9.0, 15.0);
        let radius = rng.range(0.35, 0.55);
        let spread = rng.range(3.5, 5.5);
        let top = [x + rng.range(-0.8, 0.8), y + rng.range(-0.8, 0.8), base + height];
        let mut shapes = vec![Shape::wood([x, y, base - 0.5], top, radius, radius * 0.7, BARK_DARK)];
        for i in 0..4 {
            let angle = i as f32 / 4.0 * TAU + rng.range(-0.4, 0.4);
            let foot = [x + angle.cos() * 1.7, y + angle.sin() * 1.7, base - 0.3];
            shapes.push(Shape::wood([x, y, base + 1.8], foot, radius * 0.7, 0.12, BARK_DARK));
        }
        let mut clumps = Vec::new();
        let foliage = Foliage {
            material: LEAVES_DARK,
            accent: LEAVES,
            leaf_length: 0.45,
            leaves_per_twig: 54,
            clumps_per_twig: 1,
        };
        for i in 0..9 {
            let angle = i as f32 / 9.0 * TAU + rng.range(-0.3, 0.3);
            let d = if i == 0 { 0.0 } else { spread * rng.range(0.5, 1.0) };
            let at = [top[0] + angle.cos() * d, top[1] + angle.sin() * d, top[2] + rng.range(-0.6, 0.8) - d * 0.15];
            if i > 0 {
                shapes.push(Shape::wood([top[0], top[1], top[2] - 1.5], [at[0], at[1], at[2] - 0.5], 0.2, 0.1, BARK_DARK));
            }
            let out = normalize([angle.cos() * d.min(1.0), angle.sin() * d.min(1.0), 0.6]);
            self.leaf_cluster(&mut clumps, at, out, rng.range(1.6, 2.4), BARK_DARK, foliage, rng);
        }
        for _ in 0..12 {
            let angle = rng.next() * TAU;
            let d = spread * rng.range(0.4, 1.1);
            let start = [top[0] + angle.cos() * d, top[1] + angle.sin() * d, top[2] - 0.8];
            let end = [start[0] + rng.range(-0.2, 0.2), start[1] + rng.range(-0.2, 0.2), start[2] - rng.range(2.5, height * 0.6)];
            shapes.push(Shape::capsule(start, end, 0.07, 0.05, VINE, LEAVES, false));
        }
        shapes.extend(clumps);
        shapes
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

    // Leaf colors come in regions, so forests turn in patches like in autumn
    fn leaf_colors(&self, x: f32, y: f32, rng: &mut Rng) -> (u8, u8) {
        let autumn = fbm(x / 120.0, y / 120.0, 2, self.settings.seed.wrapping_add(6));
        let pick = (rng.next() * 0.7 + autumn * 0.8 + 0.15).clamp(0.0, 0.999);
        [
            (LEAVES, LEAVES_YELLOW),
            (LEAVES_DARK, LEAVES),
            (LEAVES, LEAVES_DARK),
            (LEAVES_YELLOW, LEAVES_ORANGE),
            (LEAVES_ORANGE, LEAVES_RED),
            (LEAVES_RED, LEAVES_ORANGE),
        ][(pick * 6.0) as usize]
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

    // Tall and slender with white bark, short branches and small leaves
    fn birch(&self, x: f32, y: f32, base: f32, rng: &mut Rng) -> Vec<Shape> {
        let height = rng.range(6.0, 9.0);
        let radius = rng.range(0.1, 0.16);
        let top = [x + rng.range(-0.3, 0.3), y + rng.range(-0.3, 0.3), base + height];
        let mut wood = vec![Shape::capsule([x, y, base - 0.6], top, radius * 1.2, radius * 0.4, BIRCH_BARK, BARK_DARK, false)];
        let mut leaves = Vec::new();

        let autumn = fbm(x / 120.0, y / 120.0, 2, self.settings.seed.wrapping_add(6));
        let foliage = Foliage {
            material: if autumn > 0.1 { LEAVES_YELLOW } else { LEAVES },
            accent: LEAVES_YELLOW,
            leaf_length: 0.2,
            leaves_per_twig: 24,
            clumps_per_twig: 1,
        };
        let branches = 6 + (rng.next() * 3.99) as usize;
        for i in 0..branches {
            let t = 0.35 + 0.6 * (i as f32 + rng.next()) / branches as f32;
            let angle = rng.range(0.0, TAU);
            let dir = normalize([angle.cos(), angle.sin(), rng.range(0.6, 1.2)]);
            let length = rng.range(0.8, 1.6) * (1.2 - t * 0.6);
            self.branch(&mut wood, &mut leaves, lerp3([x, y, base], top, t), dir, length, radius * 0.35, 1, BIRCH_BARK, foliage, rng);
        }

        wood.append(&mut leaves);
        wood
    }

    // A thick trunk with limbs spreading out, from which long strands of leaves hang down
    fn willow(&self, x: f32, y: f32, base: f32, rng: &mut Rng) -> Vec<Shape> {
        let height = rng.range(2.0, 3.0);
        let radius = rng.range(0.3, 0.45);
        let top = [x, y, base + height];
        let mut wood = vec![Shape::wood([x, y, base - 0.6], top, radius * 1.3, radius, WOOD)];
        let mut leaves = Vec::new();

        let limbs = 4 + (rng.next() * 2.99) as usize;
        for i in 0..limbs {
            let angle = (i as f32 + rng.range(-0.3, 0.3)) / limbs as f32 * TAU;
            let dir = normalize([angle.cos(), angle.sin(), rng.range(0.5, 0.9)]);
            let end = add(top, scale(dir, rng.range(1.5, 2.5)));
            wood.push(Shape::wood(top, end, radius * 0.5, radius * 0.25, WOOD));
            leaves.push(Shape::sphere(end, rng.range(0.6, 0.9), WILLOW, LEAVES, true));

            // Strands hanging down from along the limb, drifting outwards a little
            for _ in 0..(5 + (rng.next() * 3.99) as usize) {
                let mut point = add(lerp3(top, end, rng.range(0.5, 1.0)), [rng.range(-0.4, 0.4), rng.range(-0.4, 0.4), 0.0]);
                let length = rng.range(2.0, 3.5);
                for _ in 0..4 {
                    let next = add(point, [dir[0] * 0.1, dir[1] * 0.1, -length / 4.0]);
                    leaves.push(Shape::capsule(point, next, 0.04, 0.04, WILLOW, LEAVES, false));
                    let middle = lerp3(point, next, 0.5);
                    for side in [-1.0, 1.0] {
                        let out = [dir[1] * side, -dir[0] * side, 0.0];
                        leaves.push(Shape::leaf(add(middle, scale(out, 0.08)), add(out, [0.0, 0.0, -1.5]), dir, 0.3, 0.08, WILLOW, LEAVES));
                    }
                    point = next;
                }
            }
        }

        wood.append(&mut leaves);
        wood
    }

    // A straight trunk with narrow tiers of needles, like a spruce
    fn spruce(&self, x: f32, y: f32, base: f32, rng: &mut Rng) -> Vec<Shape> {
        let height = rng.range(8.0, 13.0);
        let radius = rng.range(0.18, 0.26);
        let crown = rng.range(1.4, 2.0);
        let mut shapes = vec![Shape::wood([x, y, base - 0.6], [x, y, base + height], radius * 1.2, radius * 0.3, WOOD)];
        let tiers = 7 + (rng.next() * 2.99) as usize;
        for i in 0..tiers {
            let t = i as f32 / tiers as f32;
            let r = crown * (1.0 - 0.8 * t) * rng.range(0.85, 1.1);
            let z = base + height * (0.2 + 0.75 * t);
            shapes.push(Shape::capsule([x, y, z], [x, y, z + r * 1.6], r, 0.1, PINE, LEAVES_DARK, true));
        }
        shapes
    }

    // Wider and darker than a spruce, with fewer, drooping tiers
    fn fir(&self, x: f32, y: f32, base: f32, rng: &mut Rng) -> Vec<Shape> {
        let height = rng.range(7.0, 11.0);
        let radius = rng.range(0.2, 0.3);
        let crown = rng.range(2.0, 2.8);
        let mut shapes = vec![Shape::wood([x, y, base - 0.6], [x, y, base + height], radius * 1.2, radius * 0.3, WOOD)];
        let tiers = 5 + (rng.next() * 2.99) as usize;
        for i in 0..tiers {
            let t = i as f32 / tiers as f32;
            let r = crown * (1.0 - 0.75 * t) * rng.range(0.85, 1.1);
            let z = base + height * (0.25 + 0.68 * t);
            shapes.push(Shape::capsule([x, y, z - r * 0.15], [x, y, z + r * 1.2], r, 0.12, LEAVES_DARK, PINE, true));
        }
        shapes
    }

    fn poplar(&self, x: f32, y: f32, base: f32, leaves: u8, accent: u8, rng: &mut Rng) -> Vec<Shape> {
        let height = rng.range(7.0, 10.0);
        let radius = rng.range(0.9, 1.4);
        let foliage = Foliage {
            material: leaves,
            accent,
            leaf_length: 0.26,
            leaves_per_twig: 48,
            clumps_per_twig: 1,
        };
        // A narrow tower of leaf clusters up the trunk, smaller towards the top
        let mut shapes = vec![Shape::wood([x, y, base - 0.6], [x, y, base + height * 0.8], 0.24, 0.12, WOOD)];
        let mut leaves = Vec::new();
        let tiers = 5;
        for k in 0..tiers {
            let t = k as f32 / (tiers - 1) as f32;
            let z = base + 2.0 + radius + (height - radius * 1.5 - 2.0) * t;
            let r = radius * (1.0 - 0.45 * t);
            self.leaf_cluster(&mut leaves, [x, y, z], [0.0, 0.0, 0.3], r, WOOD, foliage, rng);
        }
        shapes.append(&mut leaves);
        shapes
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

    // Round bushes: dark ones with red berries in the woods, ones covered in flowers in the meadows
    fn bush(&self, x: f32, y: f32, ground: f32, meadow: f32, r: u32) -> Object {
        let mut rng = Rng(r);
        let size = rng.range(0.5, 1.0);
        let flowering = rng.next() < meadow;
        let (leaves, accent) = if flowering || rng.next() < 0.4 {
            (LEAVES_DARK, FLOWER_RED)
        } else {
            self.leaf_colors(x, y, &mut rng)
        };
        let mut shapes: Vec<Shape> = Vec::new();
        if flowering {
            let color = [FLOWER_BLUE, FLOWER_PINK, FLOWER_WHITE][(rng.next() * 2.99) as usize];
            for _ in 0..(4 + (rng.next() * 3.99) as usize) {
                let mut dir = rng.direction(0.3);
                dir[2] = dir[2].abs();
                shapes.push(Shape::sphere(add([x, y, ground + size * 0.6], scale(dir, size * 0.8)), size * rng.range(0.2, 0.3), color, FLOWER_WHITE, true));
            }
        }
        for _ in 0..(2 + (rng.next() * 2.99) as usize) {
            let center = [x + rng.range(-0.7, 0.7) * size, y + rng.range(-0.7, 0.7) * size, ground + size * 0.6];
            shapes.push(Shape::sphere(center, size * rng.range(0.6, 1.0), leaves, if flowering { LEAVES } else { accent }, true));
        }
        Object::new(shapes, rng.0)
    }

    fn boulder(&self, x: f32, y: f32, ground: f32, r: u32) -> Object {
        let mut rng = Rng(r);
        let size = 0.5 + rng.next().powi(2) * 1.8;
        let center = [x, y, ground + size * 0.2];
        let mut shapes = vec![Shape::sphere(center, size, ROCK, MOSS, false)];
        for _ in 0..3 {
            let offset = [0, 1, 2].map(|_| rng.range(-0.5, 0.5) * size);
            shapes.push(Shape::sphere(add(center, [offset[0], offset[1], offset[2] * 0.5]), size * rng.range(0.5, 0.8), ROCK, MOSS, false));
        }
        Object::new(shapes, rng.0)
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
