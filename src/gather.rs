//! Gathering what to build with, see bases.rs: felling trees for wood, a few blows at the trunk and
//! the whole tree comes apart, trunk, branches and leaves, giving wood for how big it was; breaking
//! stone out of rock, and clay out of the banks of rivers and lakes and the bands of it in the rock,
//! half a meter at a time; and digging earth out of the way, which gives nothing. Everything here
//! works in voxels of the world, counted from its first.

use std::collections::{HashMap, HashSet};

use nalgebra::Vector3;

use crate::bases::{BLOCK, Materials};
use crate::worldgen::{FOLIAGE_BRICK, World, is_passable, material::*};

/// What a voxel gives when it's gathered
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Yield {
    Wood,
    Stone,
    Clay,
    /// Earth, sand and snow: dug out of the way, for nothing
    Earth,
    /// Metal, crystal, and the like: left alone
    Nothing,
}

pub fn yield_of(material: u8) -> Yield {
    match material {
        WOOD | BARK_DARK | BIRCH_BARK | PALM_WOOD | DEAD_WOOD | MUSHROOM_STEM | CACTUS => Yield::Wood,
        STONE | ROCK | GRAVEL | LIMESTONE | SANDSTONE | SANDSTONE_DARK | BRICK_STONE | MORTAR | TEMPLE_STONE | TEMPLE_DARK | OBSIDIAN | MOSS => Yield::Stone,
        CLAY | CLAY_POT => Yield::Clay,
        GRASS | DIRT | SAND | SNOW | JUNGLE_GRASS | ICE => Yield::Earth,
        _ => Yield::Nothing,
    }
}

/// Wood that's part of a tree: its trunk and branches
pub fn is_trunk(material: u8) -> bool {
    matches!(material, WOOD | BARK_DARK | BIRCH_BARK | PALM_WOOD | DEAD_WOOD | MUSHROOM_STEM)
}

/// What grows on a tree: its leaves, and what hangs from it
pub fn is_crown(material: u8) -> bool {
    matches!(material, LEAVES | LEAVES_DARK | LEAVES_YELLOW | LEAVES_ORANGE | LEAVES_RED | LEAVES_LIME | LEAVES_CRIMSON | PINE | WILLOW | PALM_LEAF | BLOSSOM | VINE | MUSHROOM_CAP)
}

/// Blows it takes to gather a block of what a voxel's made of
pub fn blows_for(y: Yield) -> u32 {
    match y {
        Yield::Stone => 4,
        Yield::Clay => 2,
        Yield::Wood => 2,
        Yield::Earth => 1,
        Yield::Nothing => u32::MAX,
    }
}

/// The first voxel a ray from `from` along `dir` hits, in voxels of `world`, no further than `reach`
/// voxels, going through what can be walked through, like leaves and grass: the voxel, and the way
/// out of the face it came in through
pub fn raycast(world: &World, from: Vector3<f64>, dir: Vector3<f64>, reach: f64) -> Option<([i64; 3], [i64; 3])> {
    raycast_by(world, from, dir, reach, |m| !is_passable(m))
}

/// The same, stopping at whatever `stops` says
pub fn raycast_by(world: &World, from: Vector3<f64>, dir: Vector3<f64>, reach: f64, stops: impl Fn(u8) -> bool) -> Option<([i64; 3], [i64; 3])> {
    let dir = dir.try_normalize(1e-9)?;
    let mut voxel = [from.x.floor() as i64, from.y.floor() as i64, from.z.floor() as i64];
    let step = [0, 1, 2].map(|i| if dir[i] > 0.0 { 1 } else { -1 });
    let next = |i: usize, v: i64| if dir[i] > 0.0 { v as f64 + 1.0 } else { v as f64 };
    let mut t_max = [0, 1, 2].map(|i| if dir[i].abs() < 1e-12 { f64::INFINITY } else { (next(i, voxel[i]) - from[i]) / dir[i] });
    let t_delta = [0, 1, 2].map(|i| if dir[i].abs() < 1e-12 { f64::INFINITY } else { 1.0 / dir[i].abs() });
    let mut normal = [0i64; 3];
    let mut t = 0.0;
    while t <= reach {
        if stops(world.material_at(voxel[0], voxel[1], voxel[2])) {
            return Some((voxel, normal));
        }
        let axis = (0..3).min_by(|&a, &b| t_max[a].total_cmp(&t_max[b])).unwrap();
        t = t_max[axis];
        voxel[axis] += step[axis];
        t_max[axis] += t_delta[axis];
        normal = [0; 3];
        normal[axis] = -step[axis];
    }
    None
}

/// What's to be taken out of the world: voxels by the brick of the world they're in, counted from
/// its first, each voxel by where it is in its brick
pub type Cut = HashMap<[i64; 3], Vec<usize>>;

fn add_to(cut: &mut Cut, v: [i64; 3]) {
    let b = v.map(|c| c.div_euclid(BLOCK));
    let l = v.map(|c| c.rem_euclid(BLOCK));
    cut.entry(b).or_default().push((l[0] + l[1] * 8 + l[2] * 64) as usize);
}

/// A tree to fell: what of it goes, the wood it gives, and for showing it come apart, where its
/// foot is, how tall it is, and what it's made of
pub struct Tree {
    pub cut: Cut,
    pub wood: u32,
    pub foot: Vector3<f64>,
    pub height: f64,
    pub bark: u8,
    pub leaf: Option<u8>,
}

/// How far out from its trunk a tree's searched, and how high, in meters
const TREE_REACH_M: f64 = 7.0;
const TREE_HEIGHT_M: f64 = 32.0;
/// The most of its wood a tree's let have, in voxels, so a building of wood isn't taken for one
const TREE_MOST: usize = 120_000;
/// Voxels of wood for each piece of wood it gives
pub const WOOD_VOXELS: usize = 256;

/// The tree whose wood `hit` is, in voxels of `world`, if it's a tree: wood joined up to wood, down
/// to the ground, and its leaves, those nearer its trunk than any other tree's. `built` says which
/// voxels are of what the player's built, which is never part of a tree.
pub fn tree_at(world: &World, hit: [i64; 3], built: &dyn Fn([i64; 3]) -> bool) -> Option<Tree> {
    let vpm = world.voxels_per_meter as i64;
    let bark = world.material_at(hit[0], hit[1], hit[2]);
    if !is_trunk(bark) || built(hit) {
        return None;
    }
    let reach = (TREE_REACH_M * vpm as f64) as i64;
    let lo = [hit[0] - reach, hit[1] - reach, hit[2] - 4 * vpm];
    let hi = [hit[0] + reach, hit[1] + reach, hit[2] + (TREE_HEIGHT_M * vpm as f64) as i64];
    // Each voxel of the box looked at once, marked in a bit of its own: far quicker than a set
    let dims = [0, 1, 2].map(|i| (hi[i] - lo[i] + 1) as usize);
    let mut seen = vec![0u64; (dims[0] * dims[1] * dims[2]).div_ceil(64)];
    let slot = |v: [i64; 3]| -> Option<usize> {
        (0..3).all(|i| v[i] >= lo[i] && v[i] <= hi[i]).then(|| (v[0] - lo[0]) as usize + ((v[1] - lo[1]) as usize + (v[2] - lo[2]) as usize * dims[1]) * dims[0])
    };
    // The wood, joined up, corners and all
    let mut wood: Vec<[i64; 3]> = vec![hit];
    let first = slot(hit)?;
    seen[first / 64] |= 1 << (first % 64);
    let mut next = 0;
    while next < wood.len() {
        let v = wood[next];
        next += 1;
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let n = [v[0] + dx, v[1] + dy, v[2] + dz];
                    let Some(k) = slot(n) else { continue };
                    if seen[k / 64] >> (k % 64) & 1 != 0 {
                        continue;
                    }
                    seen[k / 64] |= 1 << (k % 64);
                    if !is_trunk(world.material_at(n[0], n[1], n[2])) || built(n) {
                        continue;
                    }
                    wood.push(n);
                    if wood.len() > TREE_MOST {
                        return None;
                    }
                }
            }
        }
    }
    let is_wood = |v: [i64; 3]| slot(v).is_some_and(|k| seen[k / 64] >> (k % 64) & 1 != 0 && is_trunk(world.material_at(v[0], v[1], v[2])) && !built(v));
    // Standing on the ground, not up in a wall
    let base = wood.iter().map(|v| v[2]).min()?;
    let grounded = wood.iter().filter(|v| v[2] == base).any(|v| {
        let below = world.material_at(v[0], v[1], v[2] - 1);
        !is_passable(below) && !is_trunk(below)
    });
    if !grounded {
        return None;
    }
    let top = wood.iter().map(|v| v[2]).max()?;
    // Its trunk, where it rises from the ground
    let low: Vec<&[i64; 3]> = wood.iter().filter(|v| v[2] < base + vpm * 3 / 2).collect();
    let axis = [low.iter().map(|v| v[0] as f64).sum::<f64>() / low.len() as f64, low.iter().map(|v| v[1] as f64).sum::<f64>() / low.len() as f64];
    let spread = wood.iter().map(|v| ((v[0] as f64 - axis[0]).powi(2) + (v[1] as f64 - axis[1]).powi(2)).sqrt()).fold(0.0, f64::max);
    let crown = (spread + 3.5 * vpm as f64).min(reach as f64);
    // Other trees' trunks round about, a metre or two up, each by its middle, so their leaves are
    // left them
    let mut found: Vec<([f64; 2], f64)> = Vec::new();
    for up in [vpm, 2 * vpm] {
        let z = base + up;
        for y in (lo[1]..=hi[1]).step_by(2) {
            for x in (lo[0]..=hi[0]).step_by(2) {
                let p = [x, y, z];
                if !is_trunk(world.material_at(x, y, z)) || is_wood(p) || built(p) {
                    continue;
                }
                let near = (2 * vpm) as f64;
                match found.iter_mut().find(|(sum, n)| (sum[0] / *n - x as f64).abs() < near && (sum[1] / *n - y as f64).abs() < near) {
                    Some((sum, n)) => {
                        sum[0] += x as f64;
                        sum[1] += y as f64;
                        *n += 1.0;
                    }
                    None => found.push(([x as f64, y as f64], 1.0)),
                }
            }
        }
    }
    let others: Vec<[f64; 2]> = found.iter().map(|(sum, n)| [sum[0] / n, sum[1] / n]).collect();
    // Its leaves: looked for only in the bricks with any, which are marked
    let mut cut = Cut::new();
    let mut leaf = None;
    let blo = [lo[0].div_euclid(8), lo[1].div_euclid(8), (base + vpm).div_euclid(8)];
    let bhi = [hi[0].div_euclid(8), hi[1].div_euclid(8), (top + 6 * vpm).div_euclid(8)];
    let size = world.size.map(|n| n as i64);
    for bz in blo[2].max(0)..=bhi[2].min(size[2] / 8 - 1) {
        for by in blo[1].max(0)..=bhi[1].min(size[1] / 8 - 1) {
            for bx in blo[0].max(0)..=bhi[0].min(size[0] / 8 - 1) {
                let cell = world.brick_grid[world.cell([bx, by, bz])];
                let solid_crown = cell & crate::worldgen::SOLID_BRICK != 0 && is_crown(cell as u8);
                if !solid_crown && (cell & FOLIAGE_BRICK == 0 || cell & (crate::worldgen::SOLID_BRICK | crate::worldgen::EMPTY_BRICK) != 0) {
                    continue;
                }
                for k in 0..512i64 {
                    let v = [bx * 8 + k % 8, by * 8 + (k / 8) % 8, bz * 8 + k / 64];
                    let m = world.material_at(v[0], v[1], v[2]);
                    if !is_crown(m) || built(v) {
                        continue;
                    }
                    let d = ((v[0] as f64 - axis[0]).powi(2) + (v[1] as f64 - axis[1]).powi(2)).sqrt();
                    if d > crown || others.iter().any(|o| ((v[0] as f64 - o[0]).powi(2) + (v[1] as f64 - o[1]).powi(2)).sqrt() < d) {
                        continue;
                    }
                    add_to(&mut cut, v);
                    leaf.get_or_insert(m);
                }
            }
        }
    }
    for &v in &wood {
        add_to(&mut cut, v);
    }
    let wood_count = wood.len();
    Some(Tree {
        cut,
        wood: (wood_count / WOOD_VOXELS).clamp(2, 80) as u32,
        foot: Vector3::new(axis[0], axis[1], base as f64),
        height: (top - base) as f64,
        bark,
        leaf,
    })
}

/// A block broken out: what of it goes, and what it gives
pub struct Block {
    pub cut: Cut,
    pub gives: Materials,
    /// What it's mostly made of
    pub material: u8,
}

/// Breaks the brick of the world `brick` is, counted from its first, out of it: what's solid and
/// natural in it goes, and the grass and flowers over it with nothing left under them. Stone and
/// clay give about two of each a whole block, wood the same, earth nothing. None if there's nothing
/// in it that can be gathered.
pub fn block_at(world: &World, brick: [i64; 3], built: &dyn Fn([i64; 3]) -> bool) -> Option<Block> {
    let mut cut = Cut::new();
    let mut counts = [0usize; 4];
    let mut most: HashMap<u8, usize> = HashMap::new();
    let mut tops: HashSet<[i64; 2]> = HashSet::new();
    for k in 0..512i64 {
        let v = [brick[0] * 8 + k % 8, brick[1] * 8 + (k / 8) % 8, brick[2] * 8 + k / 64];
        let m = world.material_at(v[0], v[1], v[2]);
        if m == AIR || built(v) {
            continue;
        }
        let y = yield_of(m);
        if y == Yield::Nothing && !is_passable(m) {
            continue;
        }
        if is_passable(m) && y == Yield::Nothing {
            // Plants in the block go with it
            add_to(&mut cut, v);
            continue;
        }
        add_to(&mut cut, v);
        *most.entry(m).or_default() += 1;
        counts[match y {
            Yield::Wood => 0,
            Yield::Stone => 1,
            Yield::Clay => 2,
            _ => 3,
        }] += 1;
        if k / 64 == 7 {
            tops.insert([v[0], v[1]]);
        }
    }
    if counts.iter().sum::<usize>() == 0 {
        return None;
    }
    // What grew on top of it, left hanging
    for &[x, y] in &tops {
        for z in brick[2] * 8 + 8..brick[2] * 8 + 16 {
            let m = world.material_at(x, y, z);
            if !(is_passable(m) && m != AIR && m != WATER) || built([x, y, z]) {
                break;
            }
            add_to(&mut cut, [x, y, z]);
        }
    }
    let give = |n: usize| ((n * 2 + 256) / 512) as u32;
    let material = most.into_iter().max_by_key(|&(_, n)| n).map_or(DIRT, |(m, _)| m);
    Some(Block { cut, gives: Materials::new(give(counts[0]), give(counts[1]), give(counts[2])), material })
}

#[cfg(test)]
mod tests {
    use super::*;

    // A world with a flat ground, a tree on it and another beside it, their crowns touching
    fn grove() -> World {
        let size = [128u32, 128, 256];
        let mut world = World::window(size, 0, [0, 0], 16, None);
        let set = |world: &mut World, x: i64, y: i64, z: i64, m: u8| {
            let b = [x / 8, y / 8, z / 8];
            let mut materials = crate::bases::read_brick(world, b);
            materials[((x % 8) + (y % 8) * 8 + (z % 8) * 64) as usize] = m;
            world.set_brick(b, &materials);
        };
        for y in 0..128 {
            for x in 0..128 {
                for z in 0..16 {
                    set(&mut world, x, y, z, if z < 12 { STONE } else { DIRT });
                }
                // Grass blades on top
                if (x + y) % 3 == 0 {
                    set(&mut world, x, y, 16, GRASS_BLADE);
                }
            }
        }
        for (cx, cy) in [(40i64, 60i64), (66, 60)] {
            for z in 16..90 {
                for y in cy - 3..=cy + 3 {
                    for x in cx - 3..=cx + 3 {
                        set(&mut world, x, y, z, WOOD);
                    }
                }
            }
            for z in 70i64..110 {
                for y in cy - 14..=cy + 14 {
                    for x in cx - 14..=cx + 14 {
                        if (x - cx).pow(2) + (y - cy).pow(2) + (z - 90).pow(2) < 196 && world.material_at(x, y, z) == AIR {
                            set(&mut world, x, y, z, LEAVES);
                        }
                    }
                }
            }
        }
        world
    }

    // A tree comes down whole, its leaves and all, and leaves the one beside it be
    #[test]
    fn trees_come_down_whole() {
        let world = grove();
        let none = |_: [i64; 3]| false;
        let tree = tree_at(&world, [40, 63, 30], &none).expect("a tree");
        let mut gone: HashSet<[i64; 3]> = HashSet::new();
        for (b, voxels) in &tree.cut {
            for &k in voxels {
                gone.insert([b[0] * 8 + k as i64 % 8, b[1] * 8 + (k as i64 / 8) % 8, b[2] * 8 + k as i64 / 64]);
            }
        }
        // All its trunk and every leaf on its side, nothing of the other
        for z in 16..90 {
            assert!(gone.contains(&[40, 60, z]));
            assert!(!gone.contains(&[66, 60, z]));
        }
        let floating = (0..128).flat_map(|y| (0..53).flat_map(move |x| (70..110).map(move |z| [x, y, z]))).filter(|v| world.material_at(v[0], v[1], v[2]) == LEAVES && !gone.contains(v)).count();
        assert_eq!(floating, 0, "leaves left floating");
        assert!(!gone.contains(&[55, 60, 90]), "the other tree's leaves are its own");
        assert!((tree.wood as usize) * WOOD_VOXELS <= 49 * 74 + WOOD_VOXELS && tree.wood >= 2);
        // Not the ground under it
        assert!(!gone.contains(&[40, 60, 15]));
        // What's built of wood isn't a tree
        assert!(tree_at(&world, [40, 63, 30], &|_| true).is_none());
    }

    // Blocks of stone and clay give what they're made of, earth nothing, and the grass on top goes
    #[test]
    fn blocks_give_what_they_are_made_of() {
        let world = grove();
        let none = |_: [i64; 3]| false;
        let stone = block_at(&world, [2, 2, 0], &none).unwrap();
        assert_eq!(stone.gives, Materials::new(0, 2, 0));
        assert_eq!(stone.cut.values().map(Vec::len).sum::<usize>(), 512);
        let top = block_at(&world, [2, 2, 1], &none).unwrap();
        assert_eq!(top.gives.stone, 1, "half stone, half earth");
        assert_eq!(top.gives.clay, 0);
        // The grass blades over it go too
        assert!(top.cut.contains_key(&[2, 2, 2]));
        assert!(block_at(&world, [2, 2, 5], &none).is_none(), "nothing in the air");
        assert_eq!(yield_of(CLAY), Yield::Clay);
        assert!(blows_for(Yield::Stone) > blows_for(Yield::Clay));
    }

    #[test]
    fn rays_find_what_they_hit() {
        let world = grove();
        let hit = raycast(&world, Vector3::new(40.5, 30.5, 40.5), Vector3::new(0.0, 1.0, 0.0), 64.0).unwrap();
        assert_eq!(hit, ([40, 57, 40], [0, -1, 0]));
        // Through leaves to the trunk
        let hit = raycast(&world, Vector3::new(40.5, 40.5, 80.5), Vector3::new(0.0, 1.0, 0.0), 64.0).unwrap();
        assert_eq!(hit.0, [40, 57, 80]);
        assert!(raycast(&world, Vector3::new(5.5, 5.5, 60.5), Vector3::new(0.0, 0.0, 1.0), 64.0).is_none());
    }
}
