use std::f32::consts::{FRAC_1_SQRT_2, TAU};
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
    )
}

pub fn is_water(material: u8) -> bool {
    matches!(material, WATER | FOAM)
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
        (self.bricks[(cell - 1) as usize * BRICK_WORDS + 16 + i / 4] >> ((i % 4) * 8)) as u8
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
    index as u32 + 1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerrainSettings {
    pub width_m: u32,
    pub height_m: u32,
    pub voxels_per_meter: u32,
    pub seed: u32,
}

impl TerrainSettings {
    pub fn size(&self) -> [u32; 3] {
        let v = self.voxels_per_meter;
        [self.width_m * v, self.width_m * v, self.height_m * v]
    }
}

/// Generates the landscape, and builds all of it into voxels at the settings' resolution.
pub fn generate_terrain(settings: &TerrainSettings) -> (Terrain, World) {
    print!("Generating terrain... ");

    let start = std::time::Instant::now();

    let mut heights = Heights::new(settings, [0, 0], [settings.size()[0] as usize; 2], settings.voxels_per_meter);
    let terrain = Terrain::new(settings, &mut heights);
    let mut world = terrain.build_from(&heights, 0, settings.size()[2]);
    world.grass = Some(terrain.grass_map(&heights));

    println!(
        "{} bricks stored, {} MB ({:?})",
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

/// A point along a river's course, in meters.
struct RiverPoint {
    x: f32,
    y: f32,
    surface: f32,
    half_width: f32,
    /// White water, where the river drops steeply
    foam: bool,
}

/// A piece of a tree, plant or rock, in meters: either a capsule whose radius goes linearly
/// from `radius_a` at `a` to `radius_b` at `b` (a sphere when they're the same point),
/// or a flat ellipse like a leaf.
struct Shape {
    a: Vec3,
    b: Vec3,
    radius_a: f32,
    radius_b: f32,
    /// For flat shapes: their axes, and the half-size along each. `a` is the middle.
    flat: Option<([Vec3; 3], Vec3)>,
    material: u8,
    /// Sprinkled in among `material`, AIR for none
    accent: u8,
    /// Ragged, see-through edges, for clumps of foliage
    ragged: bool,
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
            material,
            accent,
            ragged,
            min: [0, 1, 2].map(|i| a[i].min(b[i]) - r),
            max: [0, 1, 2].map(|i| a[i].max(b[i]) + r),
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
            material,
            accent,
            ragged: false,
            min: center.map(|v| v - reach),
            max: center.map(|v| v + reach),
        }
    }

    /// Squared distance from the shape's middle relative to its size there: <= 1 is inside.
    /// Nothing is thinner than about a voxel (`voxel` meters), so small parts don't fall apart.
    fn depth(&self, p: Vec3, voxel: f32) -> f32 {
        let d = sub(p, self.a);
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
    water: u8,
    blade: i32,
    blade_top: u8,
}

/// The ground under a rectangle of voxel columns, at some resolution.
struct Heights {
    voxels_per_meter: f32,
    /// Voxel coordinates of the first column
    origin: [i64; 2],
    size: [usize; 2],
    /// Height of the ground in meters, carved by the rivers
    ground: Vec<f32>,
    /// Height of river water in meters, NEG_INFINITY where there's none
    river: Vec<f32>,
    foam: Vec<bool>,
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
            origin,
            size,
            ground,
            river: vec![f32::NEG_INFINITY; size[0] * size[1]],
            foam: vec![false; size[0] * size[1]],
        }
    }

    /// Ground height in meters at a column, counted from this rectangle's corner (clamped to it)
    fn ground_m(&self, x: i64, y: i64) -> f32 {
        let x = x.clamp(0, self.size[0] as i64 - 1) as usize;
        let y = y.clamp(0, self.size[1] as i64 - 1) as usize;
        self.ground[x + y * self.size[0]]
    }

    /// Ground height in meters at a point in meters, from the nearest column
    fn ground_at(&self, x: f32, y: f32) -> f32 {
        let v = self.voxels_per_meter;
        self.ground_m((x * v) as i64 - self.origin[0], (y * v) as i64 - self.origin[1])
    }

    fn carve(&mut self, path: &[RiverPoint]) {
        let vpm = self.voxels_per_meter;
        // Banks slope back up to the land around
        let bank = 2.5;
        let [sx, sy] = self.size.map(|n| n as i64);

        for p in path {
            let reach = p.half_width + bank;
            let depth = 0.6 + p.half_width * 0.25;
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
                    let d = (xm - p.x).hypot(ym - p.y);
                    if d > reach {
                        continue;
                    }
                    let i = (cx + cy * sx) as usize;
                    let limit = if d < p.half_width {
                        let t = d / p.half_width;
                        p.surface - depth * (1.0 - t * t)
                    } else {
                        p.surface + (d - p.half_width) * 0.9
                    };
                    self.ground[i] = self.ground[i].min(limit);
                    if d < p.half_width {
                        self.river[i] = self.river[i].max(p.surface);
                        self.foam[i] |= p.foam;
                    }
                }
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

    let h = 0.36 + continent * 0.3 + hills * 0.06 + mountain_mask * ridges * 0.42;
    let h_m = h * height_m + bumps * 0.4;

    // Sink the land towards the edges so the world is an island in the sea.
    // The depth has to match SEABED_DEPTH in shaders\traverse.comp so the sea outside lines up.
    let width_m = settings.width_m as f32;
    let edge = x.min(y).min(width_m - x).min(width_m - y);
    let seabed = sea_level_m(settings) - 6.0;
    seabed + (h_m - seabed) * smoothstep(0.0, 30.0, edge)
}

fn sea_level_m(settings: &TerrainSettings) -> f32 {
    settings.height_m as f32 * 0.3
}

/// Everything about a landscape that doesn't depend on its resolution: the shape of the
/// land, the rivers, and every tree, plant and rock. Any piece of it can be built into
/// voxels at any resolution, see build().
pub struct Terrain {
    settings: TerrainSettings,
    rivers: Vec<Vec<RiverPoint>>,
    objects: Vec<Object>,
    // Indices into `objects` for each OBJECT_CELL-sized square they overlap, to find them by area
    object_cells: Vec<Vec<u32>>,
    cells_across: usize,
}

const OBJECT_CELL: f32 = 8.0;

impl Terrain {
    /// `heights` covers the whole world at its resolution, and gets the rivers carved into it.
    fn new(settings: &TerrainSettings, heights: &mut Heights) -> Self {
        let mut terrain = Terrain {
            settings: *settings,
            rivers: Vec::new(),
            objects: Vec::new(),
            object_cells: Vec::new(),
            cells_across: 0,
        };

        terrain.rivers = terrain.trace_rivers(heights);
        for path in &terrain.rivers {
            heights.carve(path);
        }

        terrain.objects = terrain.place_objects(heights);

        let cells = (settings.width_m as f32 / OBJECT_CELL).ceil() as usize;
        let mut object_cells = vec![Vec::new(); cells * cells];
        let cell = |v: f32| ((v / OBJECT_CELL).floor().max(0.0) as usize).min(cells - 1);
        for (i, object) in terrain.objects.iter().enumerate() {
            for cy in cell(object.min[1])..=cell(object.max[1]) {
                for cx in cell(object.min[0])..=cell(object.max[0]) {
                    object_cells[cx + cy * cells].push(i as u32);
                }
            }
        }
        terrain.object_cells = object_cells;
        terrain.cells_across = cells;

        terrain
    }

    /// Builds a box of the landscape into voxels. `origin` and `size` are in voxels at
    /// `voxels_per_meter`, counted from the corner of the whole landscape at that resolution.
    pub fn build(&self, origin: [i64; 3], size: [u32; 3], voxels_per_meter: u32) -> World {
        let mut heights = Heights::new(&self.settings, [origin[0], origin[1]], [size[0] as usize, size[1] as usize], voxels_per_meter);
        for path in &self.rivers {
            heights.carve(path);
        }
        let mut world = self.build_from(&heights, origin[2], size[2]);
        world.sea_level = None;
        world
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
                        if column.top != GRASS {
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
        let cell = |v: f32| ((v / OBJECT_CELL).floor().max(0.0) as usize).min(cells - 1);
        let mut indices: Vec<u32> = Vec::new();
        for cy in cell(lo[1])..=cell(hi[1]) {
            for cx in cell(lo[0])..=cell(hi[0]) {
                indices.extend_from_slice(&self.object_cells[cx + cy * cells]);
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
        let (water_top, water) = if river > h {
            (river.max(sea_level), if heights.foam[i] { FOAM } else { WATER })
        } else {
            (sea_level, WATER)
        };

        let top = if h <= sea_level + v {
            SAND
        } else if river > h {
            GRAVEL
        } else if slope > 1.4 {
            STONE
        } else if h >= snow_line + snow_jitter {
            SNOW
        } else {
            GRASS
        };
        let below = match top {
            SAND | STONE | GRAVEL => top,
            _ => DIRT,
        };

        // Flowers on short stems, growing in patches of one color. The grass itself is
        // made of triangles in the shader, see traceGrass() in shaders\traverse.comp.
        let max_stem = (0.35 * vpm).round().max(1.0) as u32;
        let (mut blade, mut blade_top) = (0, AIR);
        if top == GRASS {
            let r = hash3(gx as u32, gy as u32, self.settings.seed ^ 0xb1ade);
            let (xm, ym) = (gx as f32 / vpm, gy as f32 / vpm);
            let meadow = (fbm(xm / 14.0, ym / 14.0, 2, self.settings.seed.wrapping_add(7)) - 0.1).max(0.0) * 3.0;
            // About the same number of flowers per square meter at any resolution
            let flower_chance = 0.45 * (0.01 + meadow * 0.2) * (8.0 / vpm).powi(2).min(1.0);
            if unit(r) < flower_chance {
                blade = 1 + ((r >> 16) % max_stem) as i32;
                let patch = hash3((xm / 4.0) as u32, (ym / 4.0) as u32, self.settings.seed ^ 0xf10e);
                blade_top = [FLOWER_RED, FLOWER_YELLOW, FLOWER_PURPLE, FLOWER_WHITE][(patch % 4) as usize];
            }
        }

        Column {
            height: h,
            top,
            below,
            water_top,
            water,
            blade,
            blade_top,
        }
    }

    fn material(&self, column: &Column, z: i32, dirt_depth: i32) -> u8 {
        if z <= column.height {
            let depth = column.height - z;
            if depth == 0 {
                column.top
            } else if depth <= dirt_depth {
                column.below
            } else {
                STONE
            }
        } else if z <= column.water_top {
            column.water
        } else if z < column.height + column.blade {
            GRASS_BLADE
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
                max_top = max_top.max(column.height + column.blade).max(column.water_top);
                columns[lx + ly * 8] = column;
            }
        }

        // The shapes reaching into this column of bricks, along with their object's seed
        let column_lo = [(heights.origin[0] + cx as i64 * 8) as f32 * voxel - voxel, (heights.origin[1] + cy as i64 * 8) as f32 * voxel - voxel];
        let column_hi = [column_lo[0] + 10.0 * voxel, column_lo[1] + 10.0 * voxel];
        let mut shapes: Vec<(&Shape, u32)> = Vec::new();
        for object in objects {
            for shape in &object.shapes {
                if shape.max[0] >= column_lo[0] && shape.min[0] <= column_hi[0] && shape.max[1] >= column_lo[1] && shape.min[1] <= column_hi[1] {
                    shapes.push((shape, object.seed));
                    max_top = max_top.max((shape.max[2] * vpm).ceil() as i32);
                }
            }
        }

        // Below this every voxel in the column is stone
        let stone_below = min_height - dirt_depth;

        let mut materials = [AIR; BRICK_VOXELS];
        let mut nearby: Vec<(&Shape, u32)> = Vec::new();
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

            // Trees, plants and rocks, but only above the ground
            let (z_lo, z_hi) = ((z0 - 1) as f32 * voxel, (z0 + 9) as f32 * voxel);
            nearby.clear();
            nearby.extend(shapes.iter().filter(|(s, _)| s.max[2] >= z_lo && s.min[2] <= z_hi));
            if !nearby.is_empty() {
                let x0 = heights.origin[0] + (cx * 8) as i64;
                let y0 = heights.origin[1] + (cy * 8) as i64;
                for lz in 0..8 {
                    for ly in 0..8 {
                        for lx in 0..8 {
                            let (x, y, z) = (x0 + lx as i64, y0 + ly as i64, z0 + lz as i32);
                            if z <= columns[lx + ly * 8].height {
                                continue;
                            }
                            let p = [(x as f32 + 0.5) * voxel, (y as f32 + 0.5) * voxel, (z as f32 + 0.5) * voxel];
                            for &(shape, seed) in &nearby {
                                if (0..3).any(|i| p[i] < shape.min[i] - voxel || p[i] > shape.max[i] + voxel) {
                                    continue;
                                }
                                let depth = shape.depth(p, voxel);
                                if depth > 1.0 {
                                    continue;
                                }
                                let h = hash3(x as u32, y as u32, z as u32 ^ seed);
                                // Ragged, see-through edges so clumps of leaves aren't perfect balls
                                if shape.ragged && depth > 0.55 && unit(h) < 0.45 {
                                    continue;
                                }
                                let accent = shape.accent != AIR && unit(h >> 16) < 0.15;
                                materials[lx + ly * 8 + lz * 64] = if accent { shape.accent } else { shape.material };
                                break;
                            }
                        }
                    }
                }
            }

            let cell = push_brick(&materials, bricks);
            if cell != 0 {
                cells.push((index, cell));
            }
        }
    }

    // Rivers start at a few high springs and run downhill until they reach the sea
    fn trace_rivers(&self, heights: &Heights) -> Vec<Vec<RiverPoint>> {
        let width_m = self.settings.width_m as f32;
        let sea_level = sea_level_m(&self.settings);

        // Springs are the highest of a set of random spots, kept apart from each other
        let mut spots: Vec<(f32, f32, f32)> = (0..96u32)
            .map(|i| {
                let r = hash3(i, self.settings.seed, 0x5157);
                let x = width_m * (0.15 + 0.7 * unit(r));
                let y = width_m * (0.15 + 0.7 * unit(r >> 16));
                (heights.ground_at(x, y), x, y)
            })
            .collect();
        spots.sort_by(|a, b| b.0.total_cmp(&a.0));

        let count = ((width_m / 96.0).round() as usize).clamp(1, 8);
        let mut springs: Vec<(f32, f32)> = Vec::new();
        for (h, x, y) in spots {
            if springs.len() == count || h < sea_level + 8.0 {
                break;
            }
            if springs.iter().all(|&(px, py)| (px - x).hypot(py - y) > 60.0) {
                springs.push((x, y));
            }
        }

        springs
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| self.trace_river(heights, x, y, self.settings.seed.wrapping_add(31 * i as u32)))
            .collect()
    }

    fn trace_river(&self, heights: &Heights, x: f32, y: f32, seed: u32) -> Vec<RiverPoint> {
        let width_m = self.settings.width_m as f32;
        let sea_level = sea_level_m(&self.settings);
        let step = 0.5;
        let probe = 3.0;
        let height = |x: f32, y: f32| heights.ground_at(x, y);

        let (mut x, mut y) = (x, y);
        let mut dir = [0.0f32; 2];
        let mut surface = height(x, y) - 0.3;
        let mut foam_left = 0.0;
        let mut points = Vec::new();

        for i in 0..(3.0 * width_m / step) as usize {
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
            if x < 0.0 || y < 0.0 || x >= width_m || y >= width_m {
                break;
            }

            // Water never flows uphill
            let previous = surface;
            surface = surface.min(height(x, y) - 0.3);
            if (previous - surface) / step > 0.3 {
                foam_left = 3.0;
            } else {
                foam_left -= step;
            }

            // Wider the further it has run
            let progress = (i as f32 * step / 60.0).min(1.0);
            points.push(RiverPoint {
                x,
                y,
                surface,
                half_width: 1.0 + 2.0 * progress,
                foam: foam_left > 0.0,
            });

            if surface <= sea_level {
                break;
            }
        }

        points
    }

    fn place_objects(&self, heights: &Heights) -> Vec<Object> {
        let width_m = self.settings.width_m as f32;
        let seed = self.settings.seed;
        let vpm = heights.voxels_per_meter;
        let mut objects = Vec::new();

        // Calls `place` for one random spot in every `cell` meter square, with the column there
        let mut scatter = |cell: f32, salt: u32, place: &mut dyn FnMut(f32, f32, f32, &Column, u32) -> Option<Object>| {
            let cells = (width_m / cell).ceil() as u32;
            for cy in 0..cells {
                for cx in 0..cells {
                    let r = hash3(cx, cy, seed ^ salt);
                    let x = (cx as f32 + unit(r)) * cell;
                    let y = (cy as f32 + unit(r >> 16)) * cell;
                    if x >= width_m || y >= width_m {
                        continue;
                    }
                    let column = self.column(heights, (x * vpm) as i64, (y * vpm) as i64);
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

        // At most one tree per cell, denser where the forest noise is high
        scatter(7.5, 0x7a3e, &mut |x, y, ground, column, r| {
            let density = forest(x, y) * 0.85 + 0.03;
            (column.top == GRASS && unit(r) < density).then(|| self.tree(x, y, ground, hash(r)))
        });

        // Bushes, mostly around the forests
        scatter(4.0, 0xb054, &mut |x, y, ground, column, r| {
            let density = smoothstep(-0.3, 0.2, forest(x, y) * 2.0 - 1.0) * 0.3 + meadow(x, y) * 0.08 + 0.04;
            (column.top == GRASS && unit(r) < density).then(|| self.bush(x, y, ground, meadow(x, y), hash(r)))
        });

        // Undergrowth: ferns, big leaves and mushrooms in the forests, flowers in the meadows,
        // reeds along the shore
        scatter(1.2, 0x9a47, &mut |x, y, ground, column, r| {
            if column.top != GRASS {
                return None;
            }
            let (forest, meadow) = (forest(x, y), meadow(x, y));
            let shore = if ground - sea_level < 1.5 { 0.5 } else { 0.0 };
            let weights = [
                forest * 0.25,
                forest * 0.08,
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

        // Boulders, half sunk into the ground
        scatter(14.0, 0xb0d3, &mut |x, y, ground, column, r| {
            let dry = ground > sea_level && column.top != GRAVEL;
            (dry && unit(r) < 0.45).then(|| self.boulder(x, y, ground - 1.0 / vpm, hash(r)))
        });

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
                leaf_length: 0.2,
                leaves_per_twig: 14,
                clumps_per_twig: 3,
            };
            if choice < 0.3 {
                self.broadleaf(x, y, base, 1.0, 2, WOOD, foliage, &mut rng)
            } else if choice < 0.45 {
                self.birch(x, y, base, &mut rng)
            } else if choice < 0.55 {
                let blossom = Foliage {
                    material: BLOSSOM,
                    accent: FLOWER_WHITE,
                    leaf_length: 0.14,
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
            self.twig_leaves(leaves, end, length, foliage, rng);
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

    // A couple of small clumps around the end of a twig, and leaves sticking out all around them
    fn twig_leaves(&self, out: &mut Vec<Shape>, end: Vec3, length: f32, foliage: Foliage, rng: &mut Rng) {
        if foliage.material == AIR {
            return;
        }
        let spread = length * rng.range(1.4, 1.8);
        for _ in 0..foliage.clumps_per_twig {
            let center = add(end, scale(rng.direction(0.3), spread * 0.35));
            out.push(Shape::sphere(center, spread * rng.range(0.45, 0.65), foliage.material, foliage.accent, true));
        }
        for _ in 0..foliage.leaves_per_twig {
            let mut dir = rng.direction(0.2);
            dir[2] += 0.2;
            let center = add(end, scale(dir, spread * rng.range(0.35, 0.75)));
            let length = foliage.leaf_length * rng.range(0.8, 1.25);
            out.push(Shape::leaf(center, add(dir, [0.0, 0.0, -0.4]), [0.0, 0.0, 1.0], length, length * 0.55, foliage.material, foliage.accent));
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
            leaf_length: 0.12,
            leaves_per_twig: 8,
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
        vec![
            Shape::wood([x, y, base - 0.6], [x, y, base + height * 0.8], 0.24, 0.12, WOOD),
            Shape::capsule([x, y, base + 2.0 + radius], [x, y, base + height - radius * 0.5], radius, radius * 0.6, leaves, accent, true),
        ]
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
