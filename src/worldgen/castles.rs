//! Royal castles: rare, each on a hilltop, a ridge or a headland over the water that's flat enough
//! on top, some small keeps with a few towers, some sprawling. A curtain wall goes round it all
//! with towers along it and a gatehouse in front, open ground inside, and up a grand stair the
//! inner ward on its terrace: a courtyard between two wings, the great hall behind it and the keep
//! behind that, where the princess keeps the royal armoury in the throne room. Towers rise all
//! over it under spires and caps of slate, stone, tiles or shingles, or flat behind battlements,
//! the bigger castles' joined by covered bridges over the gardens. They're old: their stone's
//! stained and darker near the ground, bricks have fallen out and merlons crumbled, ivy climbs
//! their walls and moss grows on their roofs, paving and steps, worn hollow. Each is laid out from
//! numbers of its own, so no two are quite alike.
//!
//! A castle's laid out in a frame facing its gate: u goes out through the gate, v across, both in
//! meters from the middle of the front of the great hall.

use super::*;

/// At most one castle in each CASTLE_CELL square, its middle well inside it, nothing of it further
/// than CASTLE_REACH from there, in meters
const CASTLE_CELL: f32 = 720.0;
const CASTLE_REACH: f32 = 125.0;
/// How many squares have a castle where the land suits, and how many spots are tried in each:
/// smaller castles fit more places, so it's less likely than it would be for large ones alone
const CASTLE_CHANCE: f32 = 0.65;
/// How many castles have fallen to the dead: no princess, no guards, their windows dark, and
/// the dead walking their halls round a Grave Knight on the throne
const FALLEN_CASTLES: f32 = 0.35;
const SPOTS: u32 = 96;
/// The least the tallest spire of the largest castles rises over the inner ward, in meters
const LEAST_HEIGHT: f32 = 28.0;
/// Stairs anyone can walk up: how high each step is and how deep
const RISE: f32 = 0.25;
const RUN: f32 = 0.45;
/// How far a spire's finial or flag reaches over its tip
const FINIAL: f32 = 1.9;
/// The light flames give
const FIRE: [f32; 3] = [1.0, 0.62, 0.28];
const WAYS: [[f32; 2]; 4] = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]];

/// How big a castle is
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Size {
    /// A keep with a small courtyard and hall in front of it, a few towers round its walls
    Small,
    Medium,
    /// Sprawling, towers all over it, bridges between them
    Large,
}

impl Size {
    /// The size of the castle in the square whose hash is `r`
    fn of(r: u32) -> Size {
        let u = unit(hash(r ^ 0x512e));
        if u < 0.22 {
            Size::Small
        } else if u < 0.52 {
            Size::Medium
        } else {
            Size::Large
        }
    }

    /// Which of three goes with it, the first for small castles
    fn pick<T: Copy>(self, of: [T; 3]) -> T {
        of[self as usize]
    }

    /// How far out round it the land's looked at, as part of how far for the largest
    fn reach(self) -> f32 {
        self.pick([0.6, 0.8, 1.0])
    }

    /// The least its tallest tower rises over the inner ward, with its spire
    fn least(self) -> f32 {
        self.pick([21.0, 24.0, LEAST_HEIGHT])
    }

    fn name(self) -> &'static str {
        self.pick(["small", "medium", "large"])
    }
}

/// How its towers are topped off
#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    /// Tall pointed spires
    Spires,
    /// Low conical caps on taller towers
    Caps,
    /// Mostly flat tops behind battlements, low caps on the rest
    Battled,
}

/// A roof: what it's covered with, what darkens its joints and runs down it, and how much of it's
/// the second of its cover, out of 1
#[derive(Clone, Copy)]
struct Roofing {
    cover: (u8, u8),
    grime: u8,
    mix: f32,
}

/// Old slate, grey stone shingles, aged terracotta tiles, dark timber shingles, and copper gone a
/// dull, mossy green
const SLATE: Roofing = Roofing { cover: (DARK_STEEL, MORTAR), grime: OBSIDIAN, mix: 0.22 };
const SHINGLES: Roofing = Roofing { cover: (MORTAR, BRICK_STONE), grime: OBSIDIAN, mix: 0.25 };
const TERRACOTTA: Roofing = Roofing { cover: (CLAY_POT, SANDSTONE_DARK), grime: TEMPLE_DARK, mix: 0.3 };
const TIMBER: Roofing = Roofing { cover: (BARK_DARK, DEAD_WOOD), grime: OBSIDIAN, mix: 0.2 };
const VERDIGRIS: Roofing = Roofing { cover: (CACTUS, DARK_STEEL), grime: TEMPLE_TEAL, mix: 0.45 };

/// What a castle's built of, which goes by the land round it
#[derive(Clone, Copy)]
struct Stones {
    /// Its walls, and the stone round windows, along the foot of walls and on stairs
    wall: (u8, u8),
    trim: (u8, u8),
    /// What stains its stone, down from the tops of walls and up from the ground, and the duller
    /// stone of some of its bricks
    grime: u8,
    tone: u8,
    /// Its spires and the roofs of its towers and bridges, and those of its halls
    spire: Roofing,
    roof: Roofing,
    /// What grows on its roofs: moss, or snow lying in patches
    roof_moss: u8,
    /// How green it's gone, with moss and ivy, out of 1
    green: f32,
    /// What the courtyard and the ways in are paved with
    paving: [(u8, u8); 2],
    /// The cloth of its banners, the glass of its lit windows, and its finials
    banner: u8,
    glass: u8,
    finial: u8,
}

impl Stones {
    fn of(c: Climate, rng: &mut Rng) -> Stones {
        let pick = rng.next();
        let banner = [CLOTH, FLOWER_PURPLE, TEMPLE_TEAL, CLOTH][(rng.next() * 3.99) as usize];
        let finial = [GOLD, GOLD, BRONZE, DARK_STEEL][(rng.next() * 3.99) as usize];
        let green = rng.range(0.45, 0.85);
        // Moss low on the walls and on the roofs isn't anything a wolf would turn up on, see
        // animals::is_ground(), nor is the snow on the roofs
        if c.desert > 0.5 {
            // Dust and sun rather than moss
            let roof = if pick < 0.6 { TERRACOTTA } else { Roofing { cover: (TEMPLE_STONE, SANDSTONE), grime: TEMPLE_DARK, mix: 0.3 } };
            Stones {
                wall: (SANDSTONE, TEMPLE_STONE),
                trim: (TEMPLE_STONE, SANDSTONE),
                grime: TEMPLE_DARK,
                tone: TEMPLE_STONE,
                spire: roof,
                roof,
                roof_moss: TEMPLE_DARK,
                green: 0.0,
                paving: [(TEMPLE_STONE, SANDSTONE), (SANDSTONE, TEMPLE_DARK)],
                banner: TEMPLE_RED,
                glass: GLOW_FIRE,
                finial,
            }
        } else if c.snow > 0.5 {
            let roof = if pick < 0.7 { Roofing { cover: (DARK_STEEL, OBSIDIAN), ..SLATE } } else { TIMBER };
            Stones {
                wall: (BRICK_STONE, LIMESTONE),
                trim: (MORTAR, BRICK_STONE),
                grime: MORTAR,
                tone: STONE,
                spire: roof,
                roof,
                roof_moss: FUR_WHITE,
                green: 0.25,
                paving: [(BRICK_STONE, LIMESTONE), (MORTAR, BRICK_STONE)],
                banner,
                glass: GLOW_HOLY,
                finial,
            }
        } else if c.jungle > 0.5 {
            let (spire, roof) = if pick < 0.4 { (TERRACOTTA, TERRACOTTA) } else if pick < 0.75 { (SHINGLES, SHINGLES) } else { (VERDIGRIS, SHINGLES) };
            Stones {
                wall: (TEMPLE_STONE, TEMPLE_DARK),
                trim: (TEMPLE_DARK, TEMPLE_STONE),
                grime: DEAD_WOOD,
                tone: ROCK,
                spire,
                roof,
                roof_moss: CACTUS,
                green: 1.0,
                paving: [(TEMPLE_STONE, TEMPLE_DARK), (TEMPLE_DARK, CACTUS)],
                banner,
                glass: GLOW_FIRE,
                finial,
            }
        } else {
            // Pale old limestone, grey stone or warmer, honey-colored stone
            let (wall, trim, grime, tone) = match (rng.next() * 2.99) as usize {
                0 => ((LIMESTONE, BRICK_STONE), (BRICK_STONE, MORTAR), TEMPLE_DARK, ROCK),
                1 => ((BRICK_STONE, LIMESTONE), (MORTAR, BRICK_STONE), MORTAR, STONE),
                _ => ((TEMPLE_STONE, LIMESTONE), (TEMPLE_DARK, BRICK_STONE), TEMPLE_DARK, ROCK),
            };
            // Slate the most, copper only now and then
            let (spire, roof) = if pick < 0.34 {
                (SLATE, SLATE)
            } else if pick < 0.52 {
                (SHINGLES, SHINGLES)
            } else if pick < 0.7 {
                (TERRACOTTA, TERRACOTTA)
            } else if pick < 0.87 {
                (SLATE, TIMBER)
            } else {
                (VERDIGRIS, SLATE)
            };
            Stones {
                wall,
                trim,
                grime,
                tone,
                spire,
                roof,
                roof_moss: CACTUS,
                green,
                paving: [(wall.0, trim.0), (trim.0, wall.0)],
                banner,
                glass: GLOW_HOLY,
                finial,
            }
        }
    }
}

/// How something of a castle has weathered over the years, see aged()
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Age {
    worn: Worn,
    /// The ground it stands on is no lower than this, in tenths of a meter: the terrace or the
    /// floor it's built on, as the ground under that's lower
    floor: i16,
    /// How green it's gone with moss and ivy, and for roofs how much of the second of their cover
    /// there is, in 255ths
    green: u8,
    mix: u8,
    /// What stains it and darkens its joints, what grows on it, and the duller stone of some of
    /// its bricks
    grime: u8,
    moss: u8,
    tone: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Worn {
    /// Brick walls, ivy climbing them
    Wall,
    /// Rows of slates, tiles or shingles
    Roof,
    /// Paving
    Floor,
    /// Steps, worn hollow
    Steps,
}

/// What a voxel at `p` inside `s`, weathered as `age` says, is made of, `h` being a hash of where
/// it is and `ground` how high the ground under it is
pub(super) fn aged(s: &Shape, age: Age, p: Vec3, voxel: f32, h: u32, ground: f32) -> u8 {
    match age.worn {
        Worn::Wall => old_wall(s, age, p, voxel, h, ground),
        Worn::Roof => old_roof(s, age, p, voxel, h),
        Worn::Floor => old_floor(s, age, p, voxel, h, ground, false),
        Worn::Steps => old_floor(s, age, p, voxel, h, ground, true),
    }
}

/// How far in from the sides of `s` `p` is, level, along x and along y: for boxes and round
/// towers, which are the same both ways
fn skins(s: &Shape, p: Vec3) -> (f32, f32) {
    if let Some(half) = s.boxed {
        return (half[0] - (p[0] - s.a[0]).abs(), half[1] - (p[1] - s.a[1]).abs());
    }
    if s.flat.is_some() {
        return (0.0, 0.0);
    }
    let ab = sub(s.b, s.a);
    let length2 = dot(ab, ab);
    let t = if length2 > 0.0 { (dot(sub(p, s.a), ab) / length2).clamp(0.0, 1.0) } else { 0.0 };
    let e = sub(sub(p, s.a), scale(ab, t));
    let d = s.radius_a + (s.radius_b - s.radius_a) * t - dot(e, e).sqrt();
    (d, d)
}

/// How far along or round `s` `p` is, level, in meters, to lay patches of things out along it
fn around(s: &Shape, p: Vec3) -> f32 {
    if let Some(half) = s.boxed {
        return if half[0] >= half[1] { p[0] } else { p[1] };
    }
    let (dx, dy) = (p[0] - s.a[0], p[1] - s.a[1]);
    dy.atan2(dx) * s.radius_a.max(s.radius_b).max(0.5)
}

/// Which shape `s` is, near enough, to tell its patches from those of the rest
fn shape_id(s: &Shape) -> u32 {
    (s.a[0] * 2.0) as i32 as u32 ^ ((s.a[1] * 2.0) as i32 as u32).rotate_left(16) ^ ((s.a[2] * 2.0) as i32 as u32).rotate_left(8)
}

/// A hash of cell (a, b), whole numbers as floats, salted: cheaper than hash3 and as good for
/// laying out patches
fn cell(a: f32, b: f32, salt: u32) -> u32 {
    hash((a as i32 as u32).wrapping_mul(0x9e37_79b1) ^ (b as i32 as u32).wrapping_mul(0x85eb_ca77).rotate_left(13) ^ salt)
}

/// Patches about `size` meters across, `jitter` meters of ragged edge on them: a different number
/// from 0 to 1 in each
fn patch(a: f32, b: f32, size: f32, jitter: f32, salt: u32) -> f32 {
    unit(cell(((a + jitter) / size).floor(), ((b + jitter) / size).floor(), salt))
}

/// Old brickwork: bricks fallen out of the face, corners knocked off, ivy climbing up from the
/// ground in patches and hanging from the top in strands, moss along the foot and in the joints
/// low down and on top, the odd crack, grime running down from the top and up from the ground.
/// Only the outer skin weathers; deeper in, as where a room's cut out of it, it's plain brick.
/// What only goes on near the foot or the top isn't worked out in between, to keep it quick.
fn old_wall(s: &Shape, age: Age, p: Vec3, voxel: f32, h: u32, ground: f32) -> u8 {
    const COURSE: f32 = 0.375;
    const LENGTH: f32 = 0.75;
    let dark = if matches!(s.material, SANDSTONE | TEMPLE_STONE) { SANDSTONE_DARK } else { MORTAR };
    let course = (p[2] / COURSE).floor();
    let along = p[0] + p[1] + if (course as i32).rem_euclid(2) == 1 { LENGTH * 0.5 } else { 0.0 };
    let brick = (along / LENGTH).floor();
    let joint = p[2] - course * COURSE < voxel || along - brick * LENGTH < voxel;
    let b = cell(course, brick, 0xb71c);
    let (sx, sy) = skins(s, p);
    let skin = sx.min(sy);
    if skin > 2.0 * voxel {
        return if joint {
            dark
        } else if s.accent != AIR && unit(b) < 0.2 {
            s.accent
        } else {
            s.material
        };
    }
    let b2 = hash(b);
    let below_top = s.max[2] - p[2];
    let above = p[2] - ground.max(age.floor as f32 * 0.1);
    // Fallen out of the face, and the corners and top edges knocked about
    if below_top > 0.5 && above > 0.3 && unit(b2) < 0.012 {
        return AIR;
    }
    if (s.boxed.is_some() && sx < 1.5 * voxel && sy < 1.5 * voxel || below_top < 1.2 * voxel && skin < 1.2 * voxel) && unit(b2 >> 16) < 0.35 {
        return AIR;
    }
    let green = age.green as f32 / 255.0;
    let id = shape_id(s);
    // How high ivy climbs at most, and how far strands hang down
    let (climbs, hangs) = (10.0 * (0.4 + 0.6 * green) + 0.3, 5.5 * green.sqrt() + 0.2);
    let near = above < climbs.max(3.8) || below_top < hangs.max(5.2);
    let run = if near || s.boxed.is_some() { around(s, p) } else { 0.0 };
    let h2 = hash(h);
    if green > 0.0 && near {
        // Ivy up from the ground, higher in the middle of each patch, thinning out to its edges
        const IVY: f32 = 3.6;
        if above < climbs {
            let at = (run / IVY).floor();
            let i = cell(at, 0.0, id ^ 0x1717);
            if unit(i) < 0.6 * green {
                let j = hash(i);
                let half = 0.7 + 0.9 * unit(j);
                let middle = (at + 0.5) * IVY + (unit(j >> 16) - 0.5) * (IVY - 2.0 * half);
                let t = (run - middle).abs() / half;
                if t < 1.0 {
                    let tall = (1.0 + 9.0 * unit(hash(j))) * (0.4 + 0.6 * green);
                    let reach = tall * (1.0 - t * t) + (unit(h) - 0.5) * 0.5;
                    if above < reach && unit(h >> 16) < 0.9 - 0.3 * (above / reach).max(0.0) {
                        return if unit(h2) < 0.3 { VINE } else { LEAVES_DARK };
                    }
                }
            }
        }
        // Strands hanging from the top
        const STRAND: f32 = 0.7;
        if below_top < hangs {
            let at = (run / STRAND).floor();
            let k = cell(at, 0.0, id ^ 0x4a49);
            if unit(k) < 0.16 * green {
                let k2 = hash(k);
                if (run - (at * STRAND + 0.15 + 0.4 * unit(k >> 16))).abs() < 0.06 + 0.08 * unit(k2) {
                    let length = (0.8 + 4.5 * unit(k2 >> 16)) * green.sqrt();
                    if below_top < length + (unit(h) - 0.5) * 0.4 && unit(h >> 16) < 0.85 {
                        return if unit(h2) < 0.6 { VINE } else { LEAVES_DARK };
                    }
                }
            }
        }
        // Moss along the foot, in the joints over it and on top
        if above < 3.8 || below_top < 1.5 * voxel {
            let jitter = (unit(h2 >> 16) - 0.5) * 0.5;
            let m = patch(run, p[2], 1.3, jitter, id ^ 0x3055);
            let band = 0.1 + 1.4 * green * m;
            if above < band && unit(h2) < 0.85 || joint && above < 2.5 * band || below_top < 1.5 * voxel && m < 0.7 * green {
                return age.moss;
            }
        }
    }
    // The odd crack, stepping down through the joints
    if s.boxed.is_some() && below_top > 0.6 {
        let (at, rows) = ((run / 4.0).floor(), (p[2] / 3.0).floor());
        let c = cell(at, rows, id ^ 0xc4ac);
        if unit(c) < 0.06 {
            let down = ((rows + 1.0) * 3.0 - p[2]) / COURSE;
            let x = at * 4.0 + 1.0 + 2.0 * unit(c >> 16) + down.floor() * 0.19 * if c & 0x8000_0000 != 0 { 1.0 } else { -1.0 };
            if down < 2.0 + 6.0 * unit(hash(c)) && (run - x).abs() < 0.8 * voxel {
                return OBSIDIAN;
            }
        }
    }
    if joint {
        return dark;
    }
    // Grime run down from the top in streaks, and up from the ground over whole bricks
    if below_top < 5.2 {
        let k = cell((run / 0.45).floor(), 0.0, id ^ 0x57a1);
        if unit(k) < 0.3 {
            let f = below_top / (0.6 + 4.5 * unit(k >> 16));
            if f < 1.0 && unit(h2) < 0.85 - 0.6 * f {
                return age.grime;
            }
        }
    }
    // Some bricks duller than the rest, though not on top, as stone the color of the ground
    // could be taken for it, see animals::is_ground()
    let low = (1.0 - above / 3.5).max(0.0);
    let u = unit(b);
    if u > 0.97 || low > 0.0 && unit(hash(b2)) < low * low * 0.85 {
        age.grime
    } else if s.accent != AIR && u < 0.15 {
        s.accent
    } else if u < 0.35 && below_top > 2.0 * voxel {
        age.tone
    } else {
        s.material
    }
}

/// Old roofs: in courses of slates, tiles or shingles, the joints between them dark, some of them
/// of the second of the roof's cover or darker, with moss growing on them in patches, more of it
/// low down
fn old_roof(s: &Shape, age: Age, p: Vec3, voxel: f32, h: u32) -> u8 {
    const COURSE: f32 = 0.32;
    const PIECE: f32 = 0.36;
    // Upright for spires, where they go in courses up them, lying along their row on pitched roofs
    let upright = s.boxed.is_none() && (s.b[0] - s.a[0]).abs() + (s.b[1] - s.a[1]).abs() < 0.01;
    let (course, run, low) = if upright {
        let (dx, dy) = (p[0] - s.a[0], p[1] - s.a[1]);
        let tall = (s.max[2] - s.min[2]).max(0.1);
        ((p[2] / COURSE).floor(), dy.atan2(dx) * dx.hypot(dy), 1.0 - (p[2] - s.min[2]) / tall)
    } else {
        (0.0, p[0] + p[1], 0.5)
    };
    let along = run + if (course as i32).rem_euclid(2) == 1 { PIECE * 0.5 } else { 0.0 };
    let piece = (along / PIECE).floor();
    let green = age.green as f32 / 255.0;
    let k = cell(course, piece, 0x5419 ^ shape_id(s));
    // Moss on whole slates here and there, in clusters, more of them low down, never all of one
    if green > 0.0 {
        let jitter = (unit(h >> 16) - 0.5) * 0.4;
        let cluster = patch(p[0], p[1], 1.6, jitter, ((p[2] + jitter) / 1.6).floor() as i32 as u32 ^ 0x3055);
        if unit(hash(k)) < green * (0.06 + 0.4 * low) * (0.3 + 1.4 * cluster * cluster) && unit(h) < 0.7 {
            return age.moss;
        }
    }
    if upright && p[2] - course * COURSE < voxel || along - piece * PIECE < 0.8 * voxel {
        return age.grime;
    }
    let u = unit(k);
    let mix = age.mix as f32 / 255.0;
    if u < mix || unit(h) > 0.95 {
        s.accent
    } else if u > 0.93 {
        age.grime
    } else {
        s.material
    }
}

/// Old paving: moss along its edges and between its slabs and in patches, stains, and on steps
/// worn hollow in the middle where everyone's trodden, their edges chipped. Under its top, and
/// under the ground for paving laid in it, it's plain.
fn old_floor(s: &Shape, age: Age, p: Vec3, voxel: f32, h: u32, ground: f32, steps: bool) -> u8 {
    let (sx, sy) = if s.boxed.is_some() { skins(s, p) } else { (1.0, 1.0) };
    let below_top = s.max[2] - p[2];
    if sx.min(sy) > 2.0 * voxel && below_top > 2.0 * voxel && (p[2] - ground).abs() > 2.0 * voxel {
        return s.material;
    }
    if steps && below_top < voxel
        && let Some(half) = s.boxed
    {
        let (long, from_middle, edge) = if half[0] >= half[1] { (half[0], (p[0] - s.a[0]).abs(), sy) } else { (half[1], (p[1] - s.a[1]).abs(), sx) };
        if from_middle < long * 0.45 && edge > 1.5 * voxel && unit(h) < 0.85 || edge < 1.2 * voxel && unit(h >> 16) < 0.35 {
            return AIR;
        }
    }
    // Moss along its edges, between the slabs, and in the odd patch: on steps only at the ends,
    // out of the way of feet
    let green = age.green as f32 / 255.0;
    if green > 0.0 {
        let m = patch(p[0], p[1], 1.2, (unit(hash(h)) - 0.5) * 0.4, shape_id(s) ^ 0x3055);
        let edge = match s.boxed {
            Some(half) if steps => {
                if half[0] >= half[1] {
                    sx
                } else {
                    sy
                }
            }
            _ => sx.min(sy),
        };
        if edge < 0.05 + 0.2 * green * m && unit(h >> 16) < 0.7 * green || m < 0.1 * green && unit(h >> 16) < 0.7 {
            return age.moss;
        }
    }
    if unit(h) < 0.06 {
        age.grime
    } else if s.accent != AIR && unit(h) > 0.85 {
        s.accent
    } else {
        s.material
    }
}

/// How a tower's topped off
#[derive(Clone, Copy, PartialEq)]
enum Crown {
    /// A spire or a lower cap this high
    Spire(f32),
    /// Merlons round a flat top, a turret in the middle of the bigger ones
    Battlements,
    /// Carried out from the corner of a wall on a cone of stone, with a spire this high
    Corbelled(f32),
}

impl Tower {
    /// How high it reaches, its finial or flag and all
    fn tip(&self) -> f32 {
        match self.crown {
            Crown::Spire(h) | Crown::Corbelled(h) => self.top + h + FINIAL,
            Crown::Battlements => self.top + 1.5 + if self.r > 3.0 { 3.2 + 1.08 * self.r + FINIAL } else { 0.0 },
        }
    }
}

/// A round tower, in the castle's frame: where its middle is, how wide, how far down its footing
/// goes, how high the ground or terrace it rises from is, and where its walls stop
#[derive(Clone, Copy)]
struct Tower {
    c: [f32; 2],
    r: f32,
    foot: f32,
    seen: f32,
    /// The sills of its lowest windows
    sill: f32,
    top: f32,
    crown: Crown,
    flag: bool,
    /// How many of its windows are lit
    lit: f32,
}

/// A covered bridge from `a` to `b` along u or v, its floor at `z`
#[derive(Clone, Copy)]
struct Bridge {
    a: [f32; 2],
    b: [f32; 2],
    z: f32,
}

/// A castle, laid out
struct Castle {
    at: Frame,
    settings: TerrainSettings,
    stones: Stones,
    size: Size,
    /// The highest anything of it may reach
    sky: f32,
    /// How high the inner ward's terrace is, the lowest ground under it, and the throne room's floor
    ward: f32,
    lowest: f32,
    throne: f32,
    /// The keep: from keep[0] to keep[1] along u, keep_half either side, the throne room in it
    /// from throne_back on
    keep: [f32; 2],
    keep_half: f32,
    keep_top: f32,
    throne_back: f32,
    /// The great hall, from keep[1] to 0 along u, and how steep its roof is
    hall_half: f32,
    hall_eaves: f32,
    hall_pitch: f32,
    /// The courtyard, from 0 to `court` along u, and the front range from there to `range`, the
    /// front of the inner ward
    court: f32,
    court_half: f32,
    range: f32,
    range_top: f32,
    /// The wings along the courtyard, from court_half to court_half + wing across
    wing: f32,
    wing_eaves: f32,
    wing_pitch: f32,
    ward_half: f32,
    /// The curtain wall: its outer faces along u and across, and its top
    front: f32,
    back: f32,
    side: f32,
    wall_top: f32,
    gate_floor: f32,
    /// Steps down the grand stair, out in front of the inner ward
    steps: usize,
    towers: Vec<Tower>,
    bridges: Vec<Bridge>,
    /// Where it all is, x0, y0, x1, y1 in meters
    bounds: [f32; 4],
    seed: u32,
    /// Fallen to the dead, see FALLEN_CASTLES
    fallen: bool,
}

/// Where to look at a royal castle from and what's in it, for trying them out, in meters
pub struct Royal {
    /// The middle of it all, and just outside its gate, which faces `out`
    pub middle: [f32; 3],
    pub gate: [f32; 3],
    pub out: [f32; 2],
    /// In the courtyard by the front range, and the top of the keep's great tower, to look up at
    pub court: [f32; 3],
    pub tower: [f32; 3],
    /// Just inside the door of the great hall, and the top of the stair at its far end
    pub hall: [f32; 3],
    pub dais: [f32; 3],
    /// Inside the throne room, by the way in, and where the princess stands
    pub throne: [f32; 3],
    pub princess: [f32; 3],
}

/// The lowest and highest ground in castle frame `at` from u[0] to u[1] and v[0] to v[1], looked
/// at every `step` meters or so
fn span(settings: &TerrainSettings, at: &Frame, u: [f32; 2], v: [f32; 2], step: f32) -> (f32, f32) {
    let nu = ((u[1] - u[0]).abs() / step).ceil().max(1.0) as usize;
    let nv = ((v[1] - v[0]).abs() / step).ceil().max(1.0) as usize;
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for i in 0..=nu {
        for j in 0..=nv {
            let p = at.point(u[0] + (u[1] - u[0]) * i as f32 / nu as f32, v[0] + (v[1] - v[0]) * j as f32 / nv as f32, 0.0);
            let g = height_at(settings, p[0], p[1]);
            (lo, hi) = (lo.min(g), hi.max(g));
        }
    }
    (lo, hi)
}

fn bl(at: &Frame, u: [f32; 2], v: [f32; 2], z: [f32; 2], m: (u8, u8)) -> Shape {
    at.block(u, v, z, m.0, m.1)
}


/// Upright, at `c` from z[0] to z[1], its radius going from r[0] to r[1]
fn up(at: &Frame, c: [f32; 2], z: [f32; 2], r: [f32; 2], m: (u8, u8)) -> Shape {
    Shape::capsule(at.point(c[0], c[1], z[0]), at.point(c[0], c[1], z[1]), r[0], r[1], m.0, m.1, false)
}

/// From `a` to `b`, u, v and z
fn rod(at: &Frame, a: Vec3, b: Vec3, r: f32, m: (u8, u8)) -> Shape {
    Shape::capsule(at.point(a[0], a[1], a[2]), at.point(b[0], b[1], b[2]), r, r, m.0, m.1, false)
}

/// An ellipsoid round `c`, its radii along u, v and z
fn blob(at: &Frame, c: Vec3, radii: Vec3, m: (u8, u8)) -> Shape {
    let world = if at.f[0] != 0.0 { radii } else { [radii[1], radii[0], radii[2]] };
    Shape::ellipsoid(at.point(c[0], c[1], c[2]), world, m.0, m.1)
}

/// A tall window with a round head, in a wall facing `out` whose outer face goes through `face`:
/// `depth` through the wall, `half` its half width, from its sill at z[0] to the top of its head
/// at z[1], of `glass`, set back a little in the wall with a sill of `trim` under it
#[allow(clippy::too_many_arguments)]
fn window(s: &mut Vec<Shape>, at: &Frame, face: [f32; 2], out: [f32; 2], depth: f32, half: f32, z: [f32; 2], glass: u8, trim: (u8, u8)) {
    let p = at.point(face[0], face[1], 0.0);
    let w = Frame { x: p[0], y: p[1], f: at.dir(out) };
    let head = (z[1] - half).max(z[0] + 0.2);
    s.push(w.block([-0.05, 0.24], [-half - 0.14, half + 0.14], [z[0] - 0.16, z[0]], trim.0, trim.1));
    s.push(w.block([-depth - 0.02, -0.16], [-half, half], [z[0], head], glass, AIR));
    s.push(blob(&w, [-(depth + 0.18) * 0.5, 0.0, head], [(depth - 0.14) * 0.5, half, half], (glass, AIR)));
    s.push(w.room([-0.16, 0.05], [-half, half], [z[0], head]));
    s.push(blob(&w, [0.05, 0.0, head], [0.21, half, half], (AIR, AIR)).placed(Placement::Clear));
}

/// A narrow window in a round tower round `c`, `r` across, looking `dir`
#[allow(clippy::too_many_arguments)]
fn slit(s: &mut Vec<Shape>, at: &Frame, c: [f32; 2], r: f32, dir: [f32; 2], z: f32, tall: f32, half: f32, glass: u8) {
    let p = at.point(c[0] + dir[0] * r, c[1] + dir[1] * r, 0.0);
    let w = Frame { x: p[0], y: p[1], f: at.dir(dir) };
    s.push(w.block([-0.7, 0.04], [-half, half], [z, z + tall - half], glass, AIR));
    s.push(blob(&w, [-0.33, 0.0, z + tall - half], [0.37, half, half], (glass, AIR)));
}

/// A parapet along the top of a wall at `z`, over u[0] to u[1] and v[0] to v[1], with merlons on
/// it, of `stone` weathered as `old` says: the odd one crumbled, or fallen
fn battlements(s: &mut Vec<Shape>, at: &Frame, u: [f32; 2], v: [f32; 2], z: f32, stone: (u8, u8), old: Pattern) {
    let along_u = u[1] - u[0] > v[1] - v[0];
    let (a0, a1) = if along_u { (u[0], u[1]) } else { (v[0], v[1]) };
    let across = if along_u { v[0] + v[1] } else { u[0] + u[1] };
    let mut a = a0 + 0.1;
    while a + 0.9 <= a1 {
        let span = |a: f32, b: f32| if along_u { ([a, b], v) } else { (u, [a, b]) };
        let k = hash3((a * 4.0) as i32 as u32, (across * 4.0) as i32 as u32, 0x3e71);
        let fate = unit(k);
        if fate < 0.1 {
            // Crumbled to a stump, a broken end of it standing higher on one side
            let stump = z + 0.6 + 0.95 * (0.15 + 0.35 * unit(k >> 16));
            let (mu, mv) = span(a, a + 0.9);
            s.push(at.block(mu, mv, [z + 0.6, stump], stone.0, stone.1).patterned(old));
            if fate > 0.035 {
                let (b0, b1) = if k & 1 == 0 { (a, a + 0.45) } else { (a + 0.45, a + 0.9) };
                let (mu, mv) = span(b0, b1);
                s.push(at.block(mu, mv, [stump, (stump + 0.5).min(z + 1.4)], stone.0, stone.1).patterned(old));
            }
        } else {
            let (mu, mv) = span(a, a + 0.9);
            s.push(at.block(mu, mv, [z + 0.6, z + 1.55], stone.0, stone.1).patterned(old));
        }
        a += 1.7;
    }
    s.push(at.block(u, v, [z - 0.05, z + 0.6], stone.0, stone.1).patterned(old));
}

/// A pitched roof over a range from a[0] to a[1] along its ridge, `across` either side of `middle`:
/// rows of tiles or slates from `eaves` up to the ridge, `pitch` meters up for each across. Along
/// u, or along v, weathered as `old` says. Returns how high its ridge is.
#[allow(clippy::too_many_arguments)]
fn pitched(s: &mut Vec<Shape>, at: &Frame, along_u: bool, a: [f32; 2], middle: f32, across: f32, eaves: f32, pitch: f32, cover: (u8, u8), ridge: u8, old: Pattern) -> f32 {
    let point = |l: f32, c: f32, z: f32| if along_u { at.point(l, middle + c, z) } else { at.point(middle + c, l, z) };
    let rise = across * pitch;
    let rows = (across * (1.0 + pitch * pitch).sqrt() / 0.36).ceil() as usize;
    s.push(Shape::capsule(point(a[0], 0.0, eaves + rise + 0.05), point(a[1], 0.0, eaves + rise + 0.05), 0.27, 0.27, ridge, AIR, false));
    for k in 0..rows {
        let t = k as f32 / rows as f32;
        let (c, z) = (across * (1.0 - t), eaves + rise * t);
        for side in [-1.0, 1.0] {
            s.push(Shape::capsule(point(a[0], side * c, z), point(a[1], side * c, z), 0.24, 0.24, cover.0, cover.1, false).patterned(old));
        }
    }
    eaves + rise
}

/// The gable end under a pitched roof, from a[0] to a[1] along its ridge: stepping in from the top
/// of its walls at `wall_top`, `half` either side of `middle`, up under the roof's rows, of
/// `stone` weathered as `old` says, which had better not be green, as it's stepped
#[allow(clippy::too_many_arguments)]
fn gable_end(s: &mut Vec<Shape>, at: &Frame, along_u: bool, a: [f32; 2], middle: f32, half: f32, wall_top: f32, across: f32, eaves: f32, pitch: f32, stone: (u8, u8), old: Pattern) {
    let rise = across * pitch;
    let mut z = wall_top - 0.05;
    loop {
        let w = (across * (1.0 - (z + 0.5 - eaves + 0.15) / rise) - 0.05).min(half);
        if w < 0.15 {
            break;
        }
        let (u, v) = if along_u { (a, [middle - w, middle + w]) } else { ([middle - w, middle + w], a) };
        s.push(at.block(u, v, [z, z + 0.5], stone.0, stone.1).patterned(old));
        z += 0.5;
    }
}

/// Paves the ground from `lo` to `hi`, x and y in meters, where it's between about `low` and
/// `high`, as towns do their streets: following the ground voxel for voxel, weathered as `old` says
fn pave(s: &mut Vec<Shape>, lo: [f32; 2], hi: [f32; 2], low: f32, high: f32, stone: (u8, u8), old: Pattern) {
    let (bottom, top) = ([lo[0], lo[1], low - 0.4], [hi[0], hi[1], high + 0.1]);
    s.push(Shape::block(bottom, top, AIR, AIR).placed(Placement::Carve));
    s.push(Shape::block(bottom, top, stone.0, stone.1).placed(Placement::Inside).patterned(old));
    s.push(Shape::room([lo[0], lo[1], low], [hi[0], hi[1], high + 0.6]));
}

impl Terrain {
    /// The royal castles reaching into this piece of the landscape, built, and where each castle
    /// near enough for a ravine or a cave reaching in here to run into it is, x0, y0, x1, y1 in
    /// meters, so nothing else is built there
    pub(super) fn castles(&self, cache: &RiverCache, features: &mut Features) -> (Vec<Object>, Vec<[f32; 4]>) {
        let (lo, hi) = (self.lo, self.hi);
        let piece = [lo[0], lo[1], hi[0], hi[1]];
        let near = grown(piece, (RAVINE_REACH + RAVINE_CELL).max(2.0 * CAVE_REACH + CAVE_CELL) + 4.0);
        let first = [0, 1].map(|i| ((near[i] - CASTLE_REACH) / CASTLE_CELL).floor() as i32);
        let last = [0, 1].map(|i| ((near[i + 2] + CASTLE_REACH) / CASTLE_CELL).floor() as i32);
        let (mut objects, mut rects) = (Vec::new(), Vec::new());
        for cy in first[1]..=last[1] {
            for cx in first[0]..=last[0] {
                // Only the squares whose castle could come near, as planning one takes a while
                let (x0, y0) = (cx as f32 * CASTLE_CELL, cy as f32 * CASTLE_CELL);
                let could = [x0 + 0.2 * CASTLE_CELL - CASTLE_REACH, y0 + 0.2 * CASTLE_CELL - CASTLE_REACH, x0 + 0.8 * CASTLE_CELL + CASTLE_REACH, y0 + 0.8 * CASTLE_CELL + CASTLE_REACH];
                if !overlaps(could, near, 0.0) {
                    continue;
                }
                let Some(castle) = self.castle_in(cx, cy, cache) else {
                    continue;
                };
                if overlaps(castle.bounds, piece, 0.0) {
                    objects.extend(castle.build(self, features));
                }
                if overlaps(castle.bounds, near, 0.0) {
                    rects.push(castle.bounds);
                }
            }
        }
        (objects, rects)
    }

    /// Where the castles reaching into rectangle `r` are, each x0, y0, x1, y1 in meters, for fallen
    /// cities to keep clear of, see undercity.rs
    pub(super) fn castles_near(&self, r: [f32; 4], cache: &RiverCache) -> Vec<[f32; 4]> {
        let first = [0, 1].map(|i| ((r[i] - CASTLE_REACH) / CASTLE_CELL).floor() as i32);
        let last = [0, 1].map(|i| ((r[i + 2] + CASTLE_REACH) / CASTLE_CELL).floor() as i32);
        let mut found = Vec::new();
        for cy in first[1]..=last[1] {
            for cx in first[0]..=last[0] {
                let (x0, y0) = (cx as f32 * CASTLE_CELL, cy as f32 * CASTLE_CELL);
                let could = [x0 + 0.2 * CASTLE_CELL - CASTLE_REACH, y0 + 0.2 * CASTLE_CELL - CASTLE_REACH, x0 + 0.8 * CASTLE_CELL + CASTLE_REACH, y0 + 0.8 * CASTLE_CELL + CASTLE_REACH];
                if !overlaps(could, r, 0.0) {
                    continue;
                }
                if let Some(castle) = self.castle_in(cx, cy, cache)
                    && overlaps(castle.bounds, r, 0.0)
                {
                    found.push(castle.bounds);
                }
            }
        }
        found
    }

    /// The nearest royal castle to `near` within a few kilometers, for trying them out, of `size`
    /// if that's "small", "medium" or "large"
    #[allow(dead_code)]
    pub fn nearest_castle(&self, near: [f32; 2], size: Option<&str>) -> Option<Royal> {
        let cache = RiverCache::default();
        let home = near.map(|v| (v / CASTLE_CELL).floor() as i32);
        let mut best: Option<(f32, Castle)> = None;
        for ring in 0..8i32 {
            for cy in home[1] - ring..=home[1] + ring {
                for cx in home[0] - ring..=home[0] + ring {
                    if (cx - home[0]).abs().max((cy - home[1]).abs()) != ring {
                        continue;
                    }
                    // Or "fallen" for one fallen to the dead, "living" for one the princess keeps
                    let wanted = |c: &Castle| match size {
                        None => true,
                        Some("fallen") => c.fallen,
                        Some("living") => !c.fallen,
                        Some(s) => s == c.size.name(),
                    };
                    if let Some(castle) = self.castle_in(cx, cy, &cache).filter(wanted) {
                        let m = castle.at.point((castle.front + castle.back) * 0.5, 0.0, 0.0);
                        let distance = (m[0] - near[0]).hypot(m[1] - near[1]);
                        if best.as_ref().is_none_or(|(b, _)| distance < *b) {
                            best = Some((distance, castle));
                        }
                    }
                }
            }
            if best.as_ref().is_some_and(|(b, _)| *b <= ring as f32 * CASTLE_CELL) {
                break;
            }
        }
        best.map(|(distance, c)| {
            println!("The nearest {} {} castle is {distance:.0} m away, {} towers, its ward {:.0} by {:.0} m", c.size.name(), if c.fallen { "fallen" } else { "royal" }, c.towers.len(), c.range - c.keep[0], 2.0 * c.ward_half);
            c.royal()
        })
    }

    /// The castle in one CASTLE_CELL square: at the best of a few spots in it where the land
    /// suits, if nothing else is in the way there
    fn castle_in(&self, cx: i32, cy: i32, cache: &RiverCache) -> Option<Castle> {
        let r = hash3(cx as u32, cy as u32, self.settings.seed ^ 0xca51);
        if unit(r) > CASTLE_CHANCE {
            return None;
        }
        let size = Size::of(r);
        let mut spots: Vec<(f32, [f32; 2], u32)> = (0..SPOTS)
            .filter_map(|attempt| {
                let s = hash3(r, attempt, 0xca52);
                let p = [(cx as f32 + 0.2 + 0.6 * unit(s)) * CASTLE_CELL, (cy as f32 + 0.2 + 0.6 * unit(s >> 16)) * CASTLE_CELL];
                self.castle_spot(p, size).map(|score| (score, p, hash(s)))
            })
            .collect();
        spots.sort_by(|a, b| b.0.total_cmp(&a.0));
        spots.iter().take(16).find_map(|&(_, p, seed)| {
            let castle = self.lay_out_castle(p, seed, size)?;
            self.clear_for_castle(&castle, cache).then_some(castle)
        })
    }

    /// How well the land suits a castle with its middle at `p`, None if it doesn't: higher than
    /// the land round it or out over the water, flat enough on top, all of it dry and low enough
    /// for its spires to rise well over it. Smaller castles need less of it.
    fn castle_spot(&self, p: [f32; 2], size: Size) -> Option<f32> {
        let reach = size.reach();
        let settings = &self.settings;
        let sea = sea_level_m(settings);
        let sky = settings.height_m as f32 - 1.0;
        let g = |dx: f32, dy: f32| height_at(settings, p[0] + dx, p[1] + dy);
        let (mut lo, mut hi, mut sum) = (f32::MAX, f32::MIN, 0.0);
        for i in 0..49 {
            let h = g(((i % 7) as f32 - 3.0) * 12.0 * reach, ((i / 7) as f32 - 3.0) * 12.0 * reach);
            (lo, hi, sum) = (lo.min(h), hi.max(h), sum + h);
        }
        // The terrace it stands on takes up what's uneven, but only so much
        if hi - lo > 14.0 || hi > sky - size.least() - 0.6 || lo < sea + 1.0 {
            return None;
        }
        // Where the curtain wall goes
        let mut walls = f32::MAX;
        for k in 0..16 {
            let a = k as f32 * TAU / 16.0;
            walls = walls.min(g(a.cos() * 60.0 * reach, a.sin() * 50.0 * reach));
        }
        if walls < sea + 1.2 || hi - walls > 18.0 {
            return None;
        }
        // Lower land all round for a hill or a ridge, or water for a headland
        let (mut around, mut wet) = (0.0, 0);
        for k in 0..16 {
            let a = k as f32 * TAU / 16.0;
            let h = g(a.cos() * 115.0 * reach, a.sin() * 115.0 * reach);
            around += h;
            if h < sea + 0.3 {
                wet += 1;
            }
        }
        let prominence = sum / 49.0 - around / 16.0;
        let headland = wet >= 3;
        if prominence < 1.5 && !(headland && prominence > 0.0) {
            return None;
        }
        Some(prominence.min(12.0) + if headland { 3.0 } else { 0.0 } - (hi - lo) * 0.5 - (hi - walls) * 0.1)
    }

    /// Lays out a castle of `size` with its middle at `p`, if it fits there
    fn lay_out_castle(&self, p: [f32; 2], seed: u32, size: Size) -> Option<Castle> {
        let settings = self.settings;
        let sea = sea_level_m(&settings);
        let ground = |x: f32, y: f32| height_at(&settings, x, y);
        let mut rng = Rng(seed);
        let fallen = unit(hash3(seed, 0xdead, 0x7)) < FALLEN_CASTLES;
        let k = size.reach();
        // The gate on the low side, so the way up to it comes up the hill
        let f = WAYS
            .into_iter()
            .min_by(|a, b| {
                let g = |d: &[f32; 2]| ground(p[0] + d[0] * 55.0 * k, p[1] + d[1] * 55.0 * k) + ground(p[0] + d[0] * 75.0 * k, p[1] + d[1] * 75.0 * k);
                g(a).total_cmp(&g(b))
            })
            .unwrap();
        // Its walls on the edges of the bricks the world's stored in, half a meter across, so each
        // face of them only takes the one layer of bricks
        let grid = |v: f32| (v * 2.0).round() * 0.5;
        // Each measure somewhere between the least and the most for a castle of its size
        let measure = |rng: &mut Rng, of: [[f32; 2]; 3]| {
            let [lo, hi] = size.pick(of);
            grid(rng.range(lo, hi))
        };
        let keep_depth = measure(&mut rng, [[21.0, 22.0], [22.0, 23.0], [22.0, 24.0]]);
        let keep_half = measure(&mut rng, [[10.5, 11.0], [10.5, 11.5], [11.0, 12.5]]);
        let hall_len = measure(&mut rng, [[12.5, 14.0], [16.0, 19.0], [21.0, 24.0]]);
        let hall_half = measure(&mut rng, [[7.5, 8.0], [8.0, 8.6], [8.5, 9.6]]);
        let court = measure(&mut rng, [[10.0, 11.5], [15.0, 17.0], [19.0, 22.0]]);
        let court_half = hall_half + measure(&mut rng, [[2.5, 3.2], [3.0, 4.0], [4.0, 5.5]]);
        let range_depth = measure(&mut rng, [[6.5, 7.0], [6.8, 7.4], [7.2, 8.0]]);
        let wing = measure(&mut rng, [[6.0, 6.5], [7.5, 8.5], [9.0, 10.5]]);
        let garden = measure(&mut rng, [[3.5, 4.5], [5.0, 6.5], [8.0, 9.5]]);
        let keep = [-(hall_len + keep_depth), -hall_len];
        let range = court + range_depth;
        let ward_half = court_half + wing + garden;
        let back = keep[0] - measure(&mut rng, [[5.0, 6.0], [6.0, 7.0], [7.0, 8.5]]);
        let side = ward_half + measure(&mut rng, [[5.0, 6.0], [6.0, 7.5], [7.5, 9.0]]);
        // Laid out round the middle of the inner ward, however far out the gate ends up
        let ward_middle = (keep[0] + range) * 0.5;
        let at = Frame { x: grid(p[0] - f[0] * ward_middle), y: grid(p[1] - f[1] * ward_middle), f };
        let span = |u: [f32; 2], v: [f32; 2], step: f32| span(&settings, &at, u, v, step);

        // The inner ward stands on a terrace over the highest ground under it
        let (lowest, highest) = span([keep[0], range], [-ward_half, ward_half], 4.0);
        let ward = ((highest + 0.6) * 2.0).ceil() * 0.5;
        let sky = settings.height_m as f32 - 1.0;
        let tall = sky - ward;
        if tall < size.least() || ward - lowest > 14.5 {
            return None;
        }
        // The grand stair down from the ward to where it meets the ground, with room enough in
        // front of it for the road and the gatehouse
        let steps = (1..64).find(|&k| {
            let u = range + (k - 1) as f32 * RUN;
            ward - k as f32 * RISE <= span([u, u + RUN], [-4.0, 4.0], 2.0).1 + 0.15
        })?;
        let [near, far] = size.pick([[11.0, 12.0], [12.0, 14.0], [14.0, 17.0]]);
        let front = range + grid(rng.range(near, far).max(steps as f32 * RUN + 10.5) + 0.25);
        let middle = (front + back) * 0.5;
        // The curtain wall over the highest ground along it, but not lost under the ward
        let mut ring = (f32::MAX, f32::MIN);
        for (u, v) in [([front - 3.0, front], [-side, side]), ([back, back + 3.0], [-side, side]), ([back, front], [side - 3.0, side]), ([back, front], [-side, 3.0 - side])] {
            let (lo, hi) = span(u, v, 6.0);
            ring = (ring.0.min(lo), ring.1.max(hi));
        }
        if ring.0 < sea + 1.0 {
            return None;
        }
        // Low enough for the inner ward to rise well over it from outside, down the hill from it
        let wall_top = grid((ring.1 + rng.range(6.5, 8.0) * size.pick([0.8, 0.9, 1.0])).clamp(ward - 5.0, ward + 1.0));
        if wall_top - ring.0 > 24.0 {
            return None;
        }
        // Dry ground out in front of the gate, and the floor of the way through it
        if span([front + 1.5, front + 16.0], [-3.0, 3.0], 3.0).0 < sea + 0.8 {
            return None;
        }
        let gate_floor = span([front - 7.6, front + 1.6], [-2.2, 2.2], 1.5).1 + 0.06;

        // How high it all goes, the higher the more room there is under the sky, and the bigger
        // the castle
        let keep_top = ward + grid((size.pick([0.34, 0.39, 0.44]) * tall).max(13.5));
        let range_top = ward + grid((size.pick([0.22, 0.25, 0.27]) * tall).max(9.5));
        // The great hall's roof as steep as fits under the top of the keep behind it, the wings'
        // steep too
        let across = hall_half + 0.6;
        let steepest = (keep_top - 0.25 - ward - 7.0) / across;
        let hall_pitch = rng.range(0.68, steepest.clamp(0.68, 1.05));
        let hall_eaves = (keep_top - 0.25 - across * hall_pitch).max(ward + 7.0);
        let wing_pitch = rng.range(0.85, 1.15);
        let wing_eaves = range_top + 0.6;
        let wing_ridge = wing_eaves - 0.25 + (wing * 0.5 + 0.45) * wing_pitch;
        let style = match rng.next() {
            u if u < 0.4 => Style::Spires,
            u if u < 0.75 => Style::Caps,
            _ => Style::Battled,
        };
        // A spire's or a cap's top and height: no lower than `least`, reaching about `tip` with
        // its finial, as tall as `shape` says for each meter of its radius: about, at least and
        // at most
        let spire_of = |least: f32, tip: f32, r: f32, shape: [f32; 3]| -> (f32, f32) {
            let tip = tip.min(sky - 0.2);
            let top = (tip - FINIAL - shape[0] * r).max(least);
            let h = (tip - FINIAL - top).clamp(shape[1] * r, shape[2] * r).min(sky - 0.2 - FINIAL - top);
            (top, h.max(1.0))
        };
        let shape = if style == Style::Spires { [3.2, 1.6, 3.6] } else { [1.6, 1.1, 1.8] };
        let spire = |least: f32, tip: f32, r: f32| spire_of(least, tip, r, shape);
        // Flat behind battlements instead, where the style has them, no higher than lets the
        // turret on the bigger ones under the sky
        let crowned = |(top, h): (f32, f32), r: f32| {
            if style == Style::Battled {
                (top.min(sky - 0.2 - 1.5 - if r > 3.0 { 3.2 + 1.08 * r + FINIAL } else { 0.0 }), Crown::Battlements)
            } else {
                (top, Crown::Spire(h))
            }
        };
        let wv = court_half + wing;
        let inside = |c: [f32; 2], r: f32| c[0] - r > keep[0] && c[0] + r < range && c[1].abs() + r < ward_half;
        let mut towers = Vec::new();
        let mut corbels = Vec::new();
        let mut tower = |c: [f32; 2], r: f32, (top, crown): (f32, Crown), flag: bool, lit: f32| {
            let (foot, seen) = if inside(c, r) {
                (ward - 0.4, ward)
            } else {
                let g = [(0.0, 0.0), (r, 0.0), (-r, 0.0), (0.0, r), (0.0, -r)].map(|(du, dv)| {
                    let p = at.point(c[0] + du, c[1] + dv, 0.0);
                    ground(p[0], p[1])
                });
                let low = g.iter().copied().fold(f32::MAX, f32::min);
                (low.min(ward) - 0.6, low)
            };
            towers.push(Tower { c, r, foot, seen, sill: seen + 3.0, top, crown, flag, lit });
        };
        // The great tower, rising out of the back of the keep: the tallest of them all, under a
        // spire however the rest are topped off, the throne room in front of it
        let r = rng.range(4.6, 5.2) * size.pick([0.86, 0.94, 1.0]);
        let great = if style == Style::Spires { shape } else { [2.2, 1.3, 2.6] };
        let tip = ward + (size.pick([0.72, 0.86, 1.0]) * tall).max(size.least());
        let (top, h) = spire_of(keep_top + 4.0, tip, r, great);
        let great_u = keep[0] + r - 0.8;
        tower([great_u, 0.0], r, (top, Crown::Spire(h)), true, 0.6);
        let throne_back = grid(great_u + r + 0.7);
        // Towers in the gardens either side of the keep, the next tallest, at the back corners of
        // the wings and the corners of the front range: fewer of them the smaller the castle
        let (medium, large) = (size != Size::Small, size == Size::Large);
        let lower = size.pick([0.8, 0.92, 1.0]);
        let garden_u = keep[0] + keep_depth * rng.range(0.38, 0.5);
        let gr = rng.range(3.2, 3.7) * size.pick([0.85, 0.88, 1.0]);
        let (gtop, gcrown) = crowned(spire(keep_top - 2.0, ward + tall * rng.range(0.8, 0.9) * lower, gr), gr);
        let br = rng.range(3.9, 4.4);
        let wing_back = crowned(spire(wing_ridge + 1.2, ward + tall * rng.range(0.6, 0.72), br), br);
        let fr = rng.range(4.3, 4.8) * size.pick([0.78, 0.9, 1.0]);
        let range_front = crowned(spire(wing_ridge + 1.2, ward + tall * rng.range(0.78, 0.88) * lower, fr), fr);
        let cr = rng.range(2.3, 2.7);
        let (ctop, ch) = spire(wing_ridge + 0.8, wing_ridge + 2.8 + FINIAL + 3.2 * cr, cr);
        let bartizan = (sky - 0.2 - FINIAL - keep_top - 2.6).min(if style == Style::Spires { 5.4 } else { 2.8 }).max(1.0);
        for side in [-1.0, 1.0] {
            if medium {
                tower([garden_u, side * wv], gr, (gtop, gcrown), true, 0.55);
            }
            if large {
                tower([0.0, side * wv], br, wing_back, false, 0.5);
            }
            tower([court + range_depth * 0.5, side * wv], fr, range_front, false, 0.5);
            // Turrets on the corners of the courtyard
            for u in [0.0, court] {
                if large || medium && u > 0.0 {
                    tower([u, side * court_half], cr, (ctop, Crown::Spire(ch)), false, 0.5);
                }
            }
            // Bartizans on the corners of the keep
            for u in keep {
                let foot = keep_top - 4.5;
                corbels.push(Tower { c: [u, side * keep_half], r: 2.1, foot, seen: foot, sill: keep_top - 1.5, top: keep_top + 2.6, crown: Crown::Corbelled(bartizan), flag: false, lit: 0.4 });
            }
            // Turrets on the corners of the terrace
            for u in [keep[0], range] {
                if large || medium && u < 0.0 {
                    let r = rng.range(2.0, 2.3);
                    tower([u, side * ward_half], r, (ward + rng.range(4.5, 5.5), Crown::Spire(r * if style == Style::Spires { 2.8 } else { 1.6 })), false, 0.3);
                }
            }
        }
        // Along the curtain wall: on its corners, partway along its sides on the bigger castles,
        // and either side of the gate
        let corner = rng.range(4.6, 5.2) * size.pick([0.78, 0.88, 1.0]);
        let spired = rng.next() < 0.65 && style != Style::Battled;
        let low_cap = if style == Style::Spires { 1.0 } else { 0.62 };
        let crown = |r: f32, top: f32, tall: f32| if spired { Crown::Spire((tall * low_cap * r).min(sky - 0.2 - FINIAL - top)) } else { Crown::Battlements };
        let mid_u = middle + rng.range(-6.0, 6.0);
        let gate_r = rng.range(4.0, 4.4) * size.pick([0.85, 0.93, 1.0]);
        for side_v in [-1.0, 1.0] {
            for u in [front - 1.5, back + 1.5] {
                tower([u, side_v * (side - 1.5)], corner, (wall_top + 4.5, crown(corner, wall_top + 4.5, 2.5)), false, 0.3);
            }
            if medium {
                let r = size.pick([3.4, 3.5, 3.8]);
                tower([mid_u, side_v * (side - 1.5)], r, (wall_top + 3.2, Crown::Battlements), false, 0.3);
            }
            if large {
                tower([back + 1.5, side_v * side * 0.5], 3.6, (wall_top + 3.2, Crown::Battlements), false, 0.3);
                tower([front - 1.5, side_v * side * 0.6], 3.6, (wall_top + 3.2, crown(3.6, wall_top + 3.2, 2.2)), false, 0.3);
            }
            let top = wall_top + 5.5;
            let gate = if style == Style::Battled { Crown::Battlements } else { Crown::Spire((2.4 * low_cap * gate_r).min(sky - 0.2 - FINIAL - top)) };
            tower([front - 1.0, side_v * (4.4 + gate_r - 1.0)], gate_r, (top, gate), true, 0.4);
        }
        towers.extend(corbels);
        // The great tower's windows start over the roof of the keep it rises out of
        towers[0].sill = keep_top + 1.0;

        // Covered bridges from the sides of the keep over the gardens to their towers, and on the
        // largest from those to the towers at the backs of the wings
        let mut bridges = Vec::new();
        if medium {
            for side in [-1.0, 1.0] {
                bridges.push(Bridge { a: [garden_u, side * (keep_half - 0.3)], b: [garden_u, side * (wv - gr + 0.3)], z: (keep_top - 5.2).min(gtop - 5.0) });
                if large {
                    bridges.push(Bridge { a: [garden_u + gr - 0.3, side * wv], b: [-br + 0.3, side * wv], z: (wing_eaves - 4.5).min(wing_back.0 - 5.0).max(ward + 4.5) });
                }
            }
        }

        let (lo, hi) = at.rect([back - 7.0, front + 17.0], [-side - 7.0, side + 7.0]);
        if [lo, hi].iter().any(|q| (q[0] - p[0]).abs() > CASTLE_REACH || (q[1] - p[1]).abs() > CASTLE_REACH) {
            return None;
        }
        let c = climate(&settings, p[0], p[1]);
        Some(Castle {
            at,
            settings,
            // A fallen castle's windows are all dark
            stones: {
                let mut stones = Stones::of(c, &mut rng);
                if fallen {
                    stones.glass = OBSIDIAN;
                }
                stones
            },
            size,
            sky,
            ward,
            lowest,
            throne: ward + 6.0 * RISE,
            keep,
            keep_half,
            keep_top,
            throne_back,
            hall_half,
            hall_eaves,
            hall_pitch,
            court,
            court_half,
            range,
            range_top,
            wing,
            wing_eaves,
            wing_pitch,
            ward_half,
            front,
            back,
            side,
            wall_top,
            gate_floor,
            steps,
            towers,
            bridges,
            bounds: [lo[0], lo[1], hi[0], hi[1]],
            seed: hash(seed ^ 0xca5e),
            fallen,
        })
    }

    /// Whether `cave` breaks out of the ground anywhere in rectangle `r`, x0, y0, x1, y1 in meters:
    /// it can pass deep under a castle, but no more, see caves()
    pub(super) fn breaks_into(&self, cave: &Cave, r: [f32; 4]) -> bool {
        overlaps(cave.bounds, r, 0.0)
            && cave.shapes.iter().any(|s| {
                s.placement == Placement::Carve
                    && overlaps([s.min[0], s.min[1], s.max[0], s.max[1]], r, 2.0)
                    && s.max[2] > height_at(&self.settings, (s.min[0] + s.max[0]) * 0.5, (s.min[1] + s.max[1]) * 0.5) - 1.5
            })
    }

    /// Whether nothing else is in the way of castle `c`: no river, dungeon or town near it. Ravines
    /// and caves keep clear of castles themselves.
    fn clear_for_castle(&self, c: &Castle, cache: &RiverCache) -> bool {
        self.castle_blocked(c, cache).is_none()
    }

    /// What's in the way of castle `c`, if anything
    fn castle_blocked(&self, c: &Castle, cache: &RiverCache) -> Option<&'static str> {
        let rect = c.bounds;
        // Rivers can run by close outside the walls, their banks well clear of the towers along
        // them, but not across the way up to the gate
        let walls = c.at.rect([c.back - 6.5, c.front + 6.5], [-c.side - 6.5, c.side + 6.5]);
        let way = c.at.rect([c.front, c.front + 17.0], [-5.0, 5.0]);
        let keep_clear = [[walls.0[0], walls.0[1], walls.1[0], walls.1[1]], [way.0[0], way.0[1], way.1[0], way.1[1]]];
        let margin = 6.5;
        for river in self.rivers_near(cache, [rect[0] - margin, rect[1] - margin], [rect[2] + margin, rect[3] + margin]) {
            let near_it = river.iter().any(|p| {
                keep_clear.iter().any(|r| {
                    let (dx, dy) = ((r[0] - p.x).max(p.x - r[2]).max(0.0), (r[1] - p.y).max(p.y - r[3]).max(0.0));
                    dx.hypot(dy) < p.half_width + margin
                })
            });
            if near_it {
                return Some("river");
            }
        }
        let reach = DUNGEON_REACH + 2.0;
        let first = [0, 1].map(|i| ((rect[i] - reach) / DUNGEON_CELL).floor() as i32);
        let last = [0, 1].map(|i| ((rect[i + 2] + reach) / DUNGEON_CELL).floor() as i32);
        for cy in first[1]..=last[1] {
            for cx in first[0]..=last[0] {
                // Its halls are deep enough under the ground for the castle to stand over them,
                // but not the crypt over the way down
                if self.dungeon_in(cx, cy, cache).is_some_and(|d| d.world_rects().first().is_some_and(|r| overlaps(*r, rect, 6.0))) {
                    return Some("dungeon");
                }
            }
        }
        // Towns last, as planning them takes longest
        (!self.towns_near(grown(rect, 15.0), cache).is_empty()).then_some("town")
    }
}

impl Castle {
    fn ground(&self, u: f32, v: f32) -> f32 {
        let p = self.at.point(u, v, 0.0);
        height_at(&self.settings, p[0], p[1])
    }

    fn span(&self, u: [f32; 2], v: [f32; 2], step: f32) -> (f32, f32) {
        span(&self.settings, &self.at, u, v, step)
    }

    /// The throne room: from tu0 to tu1 along u, either side of the middle out to tv
    fn throne_room_box(&self) -> (f32, f32, f32) {
        (self.throne_back, self.keep[1] - 2.2, self.keep_half - 2.2)
    }

    /// Old stone, weathered as `worn` says, standing on the ground or on a floor no lower than
    /// `floor`, `green` times as green as the rest of the castle
    fn old(&self, worn: Worn, floor: f32, green: f32) -> Pattern {
        let st = &self.stones;
        Pattern::Aged(Age {
            worn,
            floor: (floor * 10.0).clamp(-30000.0, 30000.0) as i16,
            green: (st.green * green * 255.0).clamp(0.0, 255.0) as u8,
            mix: 0,
            grime: st.grime,
            moss: CACTUS,
            tone: st.tone,
        })
    }

    /// Old roofing, as green as the rest of the castle `green` times, mossy or snowy
    fn old_roof(&self, roof: Roofing, green: f32) -> Pattern {
        let st = &self.stones;
        // Snow lies on roofs all over, moss less so
        let green = if st.roof_moss == FUR_WHITE { 0.9 } else { st.green * green };
        Pattern::Aged(Age {
            worn: Worn::Roof,
            floor: 0,
            green: (green * 255.0).clamp(0.0, 255.0) as u8,
            mix: (roof.mix * 255.0) as u8,
            grime: roof.grime,
            moss: st.roof_moss,
            tone: roof.grime,
        })
    }

    /// How green a tower, a stretch of wall or anything round `c` has gone, as part of how green
    /// the castle has: some much more, some hardly
    fn greener(&self, c: [f32; 2]) -> f32 {
        let k = unit(hash3((c[0] * 2.0) as i32 as u32, (c[1] * 2.0) as i32 as u32, self.seed ^ 0x9e7));
        0.2 + 1.6 * k * k
    }

    /// Where the stair up to the throne room ends in the great hall, and where the pillars down
    /// its sides stand, along u
    fn hall_posts(&self) -> (f32, Vec<f32>) {
        let n = ((self.throne - self.ward) / RISE).round();
        let stair_end = self.keep[1] + 1.4 + n * RUN;
        let mut u = stair_end + 2.2;
        let mut posts = Vec::new();
        while u < -2.6 {
            posts.push(u);
            u += 3.8;
        }
        (stair_end, posts)
    }

    /// Where the princess stands: on the dais beside the throne
    fn princess(&self) -> Vec3 {
        let (tu0, ..) = self.throne_room_box();
        self.at.point(tu0 + 1.7, 1.5, self.throne + 3.0 * RISE + 0.0625)
    }

    fn royal(&self) -> Royal {
        let (tu0, tu1, _) = self.throne_room_box();
        let at = &self.at;
        Royal {
            middle: at.point((self.front + self.back) * 0.5, 0.0, self.ward),
            gate: at.point(self.front + 2.0, 0.0, self.gate_floor),
            out: at.dir([1.0, 0.0]),
            court: at.point(self.court - 1.5, 2.5, self.ward),
            tower: at.point(self.towers[0].c[0], 0.0, self.keep_top + 8.0),
            hall: at.point(-3.0, 0.0, self.ward),
            dais: at.point(self.keep[1] + 1.4, 0.0, self.throne),
            throne: at.point(tu1 - 1.2, -1.6, self.throne),
            princess: at.point(tu0 + 1.7, 1.5, self.throne + 3.0 * RISE + 0.0625),
        }
    }

    /// Builds the castle: what's inside the rooms first, then the rooms and passages cut through
    /// the walls, the towers, bridges, buildings and walls, the ways in, the terrace under it all,
    /// and the trees in its gardens, so each wins over what comes after
    fn build(&self, t: &Terrain, features: &mut Features) -> Vec<Object> {
        debug_assert!(self.towers.iter().all(|t| t.tip() <= self.sky + 0.01));
        let mut rng = Rng(hash(self.seed ^ 0xb11d));
        let mut objects = Vec::new();
        let mut done = |s: &mut Vec<Shape>, rng: &mut Rng| {
            if !s.is_empty() {
                objects.push(Object::new(std::mem::take(s), hash(rng.0 ^ objects.len() as u32)));
            }
        };
        let mut s = Vec::new();
        self.throne_room(t, &mut s, features);
        done(&mut s, &mut rng);
        self.hall_inside(t, &mut s, features, &mut rng);
        done(&mut s, &mut rng);
        self.gate_fittings(t, &mut s, features);
        done(&mut s, &mut rng);
        self.rooms(&mut s);
        done(&mut s, &mut rng);
        for tower in &self.towers {
            self.tower(tower, &mut s, &mut rng);
            done(&mut s, &mut rng);
        }
        for bridge in &self.bridges {
            self.bridge(bridge, &mut s, &mut rng);
            done(&mut s, &mut rng);
        }
        self.keep(&mut s, &mut rng);
        done(&mut s, &mut rng);
        self.hall(t, &mut s, features, &mut rng);
        done(&mut s, &mut rng);
        for side in [-1.0, 1.0] {
            self.wing(side, &mut s, &mut rng);
            done(&mut s, &mut rng);
        }
        self.front_range(&mut s, &mut rng);
        done(&mut s, &mut rng);
        self.gatehouse(t, &mut s, features);
        done(&mut s, &mut rng);
        for k in 0..4 {
            self.curtain(k, &mut s, &mut rng);
            done(&mut s, &mut rng);
        }
        self.ways(t, &mut s, features);
        done(&mut s, &mut rng);
        self.courtyard(t, &mut s, features);
        done(&mut s, &mut rng);
        self.terrace(&mut s);
        done(&mut s, &mut rng);
        let trees = self.trees(t, &mut rng);
        objects.extend(trees);

        // Nothing else grows inside, and no grass is drawn in the shader where it's built over
        let b = self.bounds;
        features.footprints.push([b[0], b[1], b[2], b[3], 0.0]);
        let at = &self.at;
        let mut hole = |u: [f32; 2], v: [f32; 2]| {
            let (lo, hi) = at.rect(u, v);
            features.holes.push([lo[0], lo[1], hi[0], hi[1]]);
        };
        hole([self.keep[0] - 0.5, self.range + 0.5], [-self.ward_half - 0.5, self.ward_half + 0.5]);
        hole([self.front - 3.5, self.front + 0.5], [-self.side - 0.5, self.side + 0.5]);
        hole([self.back - 0.5, self.back + 3.5], [-self.side - 0.5, self.side + 0.5]);
        for side in [-1.0, 1.0] {
            hole([self.back - 0.5, self.front + 0.5], [side * (self.side + 0.5), side * (self.side - 3.5)]);
        }
        hole([self.range, self.front + 16.0], [-4.8, 4.8]);
        for tower in &self.towers {
            if tower.seen < self.ward - 0.5 {
                hole([tower.c[0] - tower.r - 0.8, tower.c[0] + tower.r + 0.8], [tower.c[1] - tower.r - 0.8, tower.c[1] + tower.r + 0.8]);
            }
        }

        // Where the princess, her guards and her court are, and the castle itself
        let mut site = |kind: &'static str, p: Vec3, facing: [f32; 2]| {
            if t.inside(p[0], p[1]) {
                features.sites.push(Site { kind, position: p, facing: at.dir(facing) });
            }
        };
        let (tu0, _, tv) = self.throne_room_box();
        if self.fallen {
            // The dead at the gate, in the courtyard and the great hall, a Grave Knight in the
            // throne room, and what the court left behind by the throne
            site("fallen castle", at.point(self.front + 3.0, 0.0, self.gate_floor), [1.0, 0.0]);
            site("fallen court", at.point((self.court * 0.5 + 4.5).min(self.court - 1.5), 0.0, self.ward), [1.0, 0.0]);
            site("fallen hall", at.point(self.keep[1] * 0.5, 0.0, self.ward), [1.0, 0.0]);
            site("fallen throne", at.point(tu0 + 4.0, 0.0, self.throne), [1.0, 0.0]);
            let hoard = self.princess();
            if t.inside(hoard[0], hoard[1]) {
                chest(features, hoard, at.heading(0.0), 4.5);
            }
            return objects;
        }
        site("royal castle", at.point(self.front + 2.0, 0.0, self.gate_floor), [1.0, 0.0]);
        site("royal court", at.point((self.court * 0.5 + 4.5).min(self.court - 1.5), 0.0, self.ward), [1.0, 0.0]);
        site("royal stall", self.princess(), [1.0, 0.0]);
        for side in [-1.0, 1.0] {
            site("royal guard", at.point(self.front + 2.4, side * 3.4, self.gate_floor), [1.0, 0.0]);
            site("royal guard", at.point(1.5, side * 3.4, self.ward), [1.0, 0.0]);
            site("royal guard", at.point(tu0 + 5.6, side * (tv - 3.3).min(5.4), self.throne), [1.0, 0.0]);
        }
        objects
    }

    /// A spire or a lower cap on a round tower round `c`, `r` across, from `base` up `h`: over an
    /// eave, hollow inside, a finial at its tip or a flag flying from it
    #[allow(clippy::too_many_arguments)]
    fn spire(&self, s: &mut Vec<Shape>, c: [f32; 2], base: f32, r: f32, h: f32, flag: bool) {
        let at = &self.at;
        let roof = self.stones.spire;
        let old = self.old_roof(roof, 1.0);
        let tip = base + h;
        if flag {
            s.push(bl(at, [c[0] - 0.03, c[0] + 0.03], [c[1] + 0.06, c[1] + 1.5], [tip + 1.05, tip + 1.85], (self.stones.banner, AIR)));
            s.push(up(at, c, [tip - 0.4, tip + 1.9], [0.06, 0.04], (DARK_STEEL, AIR)));
        } else {
            let finial = self.stones.finial;
            s.push(Shape::sphere(at.point(c[0], c[1], tip + 0.35), 0.24, finial, AIR, false));
            s.push(up(at, c, [tip - 0.6, tip + 1.6], [0.1, 0.02], (finial, AIR)));
        }
        s.push(blob(at, [c[0], c[1], base + 0.1], [r + 0.42, r + 0.42, 0.32], roof.cover).patterned(old));
        if r > 1.0 && h > 2.0 {
            s.push(up(at, c, [base + 0.4, tip - 1.0], [(r - 0.3).min(r * (1.0 - 0.4 / h) - 0.15), 0.1], (roof.cover.0, AIR)));
        }
        s.push(up(at, c, [base, tip], [r + 0.12, 0.06], roof.cover).patterned(old));
        // What of the tower's rounded top would poke out round the foot of the spire
        s.push(at.room([c[0] - r - 0.2, c[0] + r + 0.2], [c[1] - r - 0.2, c[1] + r + 0.2], [base, base + 0.75 * r]));
    }

    /// Tower `t`: its crown, bands of trim round it, windows at every level, hollow inside its
    /// round walls, sloping out at its foot where it stands on the ground
    fn tower(&self, t: &Tower, s: &mut Vec<Shape>, rng: &mut Rng) {
        let at = &self.at;
        let st = self.stones;
        let (c, r) = (t.c, t.r);
        // Standing on the inner ward, or on the ground out in the bailey
        let floor = if t.seen >= self.ward { self.ward } else { f32::MIN };
        let old = self.old(Worn::Wall, floor, self.greener(c));
        let body_top = match t.crown {
            Crown::Spire(h) | Crown::Corbelled(h) => {
                self.spire(s, c, t.top, r, h, t.flag);
                t.top
            }
            Crown::Battlements => {
                let n = ((TAU * r / 1.7).round() as usize).max(6);
                for k in 0..n {
                    let a = k as f32 * TAU / n as f32;
                    let m = [c[0] + a.cos() * (r - 0.1), c[1] + a.sin() * (r - 0.1)];
                    // The odd one crumbled, or fallen
                    let fate = unit(hash3(k as u32, (c[0] * 4.0) as i32 as u32 ^ (c[1] * 4.0) as i32 as u32, self.seed ^ 0x3e71));
                    if fate < 0.03 {
                        continue;
                    }
                    let height = if fate < 0.09 { 0.3 + 4.0 * (fate - 0.03) } else { 1.0 };
                    s.push(at.block([m[0] - 0.36, m[0] + 0.36], [m[1] - 0.36, m[1] + 0.36], [t.top + 0.5, t.top + 0.5 + height], st.wall.0, st.wall.1).patterned(old));
                }
                if r > 3.0 {
                    // A turret in the middle, with a little spire of its own
                    let tr = r * 0.36;
                    self.spire(s, c, t.top + 3.2, tr, 3.0 * tr, t.flag);
                    s.push(up(at, c, [t.top, t.top + 3.2], [tr, tr], st.wall).patterned(old));
                }
                s.push(at.room([c[0] - r - 0.5, c[0] + r + 0.5], [c[1] - r - 0.5, c[1] + r + 0.5], [t.top + 0.5, t.top + 0.5 + r]));
                // The band of machicolations they stand out on
                s.push(blob(at, [c[0], c[1], t.top - 0.1], [r + 0.38, r + 0.38, 0.6], st.trim));
                t.top + 0.5
            }
        };
        let corbelled = matches!(t.crown, Crown::Corbelled(_));
        let foot = if corbelled {
            s.push(up(at, c, [t.foot, t.foot + 2.4], [0.25, r], st.trim));
            t.foot + 2.4
        } else {
            t.foot
        };
        let mut z = t.seen + 6.0;
        while z < body_top - 3.0 && !corbelled {
            s.push(blob(at, [c[0], c[1], z], [r + 0.16, r + 0.16, 0.2], st.trim));
            z += 8.5;
        }
        // Windows, lit or dark, looking a different few ways at each level
        let half = if r > 3.0 { 0.32 } else { 0.24 };
        let mut z = t.sill.max(foot + 3.0);
        let mut k = (rng.next() * 4.0) as usize;
        while z + 1.6 < body_top - 1.0 {
            for d in 0..4 {
                if rng.next() < 0.3 {
                    continue;
                }
                let glass = if rng.next() < t.lit { st.glass } else { OBSIDIAN };
                slit(s, at, c, r, WAYS[(d + k) % 4], z, 1.6, half, glass);
            }
            k += 1;
            z += 4.4;
        }
        // Plain stone inside, so only its walls are kept
        if r > 1.5 && body_top - foot > 2.0 * r + 1.0 {
            s.push(up(at, c, [foot + r, body_top - r], [r - 0.3, r - 0.3], (st.wall.0, AIR)));
        }
        s.push(up(at, c, [foot, body_top], [r, r], st.wall).patterned(old));
        if !corbelled && t.seen + 1.8 > foot + 0.5 {
            s.push(up(at, c, [foot, t.seen + 1.8], [r + 0.6, r], st.trim).patterned(old));
        }
    }

    /// Bridge `b`: a hollow gallery with windows down both sides under a pitched roof, on a band
    /// of trim
    fn bridge(&self, b: &Bridge, s: &mut Vec<Shape>, rng: &mut Rng) {
        let at = &self.at;
        let st = self.stones;
        let along_u = (b.a[0] - b.b[0]).abs() > (b.a[1] - b.b[1]).abs();
        let (l0, l1, m) = if along_u { (b.a[0].min(b.b[0]), b.a[0].max(b.b[0]), b.a[1]) } else { (b.a[1].min(b.b[1]), b.a[1].max(b.b[1]), b.a[0]) };
        let half = 1.6;
        let (z0, z1) = (b.z, b.z + 3.0);
        let rect = |l: [f32; 2], c: [f32; 2]| if along_u { (l, [m + c[0], m + c[1]]) } else { ([m + c[0], m + c[1]], l) };
        let mut l = l0 + 1.5;
        while l < l1 - 1.1 {
            for side in [-1.0, 1.0] {
                let face = if along_u { [l, m + side * half] } else { [m + side * half, l] };
                let out = if along_u { [0.0, side] } else { [side, 0.0] };
                let glass = if rng.next() < 0.55 { st.glass } else { OBSIDIAN };
                window(s, at, face, out, 0.5, 0.32, [z0 + 0.9, z0 + 2.3], glass, st.trim);
            }
            l += 2.2;
        }
        let across = half + 0.35;
        let old = self.old(Worn::Wall, f32::MIN, 0.6);
        pitched(s, at, along_u, [l0 - 0.2, l1 + 0.2], m, across, z1 - 0.2, 0.8, st.spire.cover, st.trim.0, self.old_roof(st.spire, 1.0));
        for end in [[l0, l0 + 0.4], [l1 - 0.4, l1]] {
            gable_end(s, at, along_u, end, m, half, z1, across, z1 - 0.2, 0.8, st.wall, self.old(Worn::Wall, f32::MIN, 0.0));
        }
        let (u, v) = rect([l0 + 0.3, l1 - 0.3], [-half + 0.3, half - 0.3]);
        s.push(bl(at, u, v, [z0 + 0.3, z1 - 0.3], (st.wall.0, AIR)));
        let (u, v) = rect([l0, l1], [-half, half]);
        s.push(bl(at, u, v, [z0, z1], st.wall).patterned(old));
        let (u, v) = rect([l0, l1], [-half - 0.2, half + 0.2]);
        s.push(bl(at, u, v, [z0 - 0.4, z0 + 0.12], st.trim));
        // Carried on corbels
        let mut l = l0 + 1.0;
        while l < l1 - 0.6 {
            for side in [-1.0, 1.0] {
                let c = if along_u { [l, m + side * (half - 0.1)] } else { [m + side * (half - 0.1), l] };
                s.push(up(at, c, [z0 - 1.3, z0 - 0.35], [0.08, 0.32], st.trim));
            }
            l += 1.8;
        }
    }

    /// A brazier standing at `c` on `floor`, burning
    fn brazier(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features, c: [f32; 2], floor: f32) {
        let at = &self.at;
        s.push(blob(at, [c[0], c[1], floor + 1.36], [0.26, 0.26, 0.3], (FLAME, AIR)));
        s.push(blob(at, [c[0], c[1], floor + 1.12], [0.46, 0.46, 0.17], (BRONZE, AIR)));
        s.push(up(at, c, [floor, floor + 1.05], [0.13, 0.07], (BRONZE, DARK_STEEL)));
        s.push(bl(at, [c[0] - 0.3, c[0] + 0.3], [c[1] - 0.3, c[1] + 0.3], [floor - 0.05, floor + 0.1], (DARK_STEEL, AIR)));
        let p = at.point(c[0], c[1], floor + 2.0);
        if t.inside(p[0], p[1]) {
            features.lights.push(Light { position: p, color: FIRE, reach: 13.0 });
        }
    }

    /// A torch in an iron bracket on a wall facing `out`, its face at `face`, `z` high
    fn torch(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features, face: [f32; 2], out: [f32; 2], z: f32) {
        let at = &self.at;
        let p = |d: f32, z: f32| [face[0] + out[0] * d, face[1] + out[1] * d, z];
        s.push(blob(at, p(0.3, z + 0.56), [0.08, 0.08, 0.14], (FLAME, AIR)));
        s.push(rod(at, p(0.12, z), p(0.3, z + 0.42), 0.05, (WOOD, BARK_DARK)));
        s.push(rod(at, p(-0.1, z + 0.05), p(0.2, z + 0.05), 0.035, (DARK_STEEL, AIR)));
        let light = at.point(face[0] + out[0] * 0.55, face[1] + out[1] * 0.55, z + 0.95);
        if t.inside(light[0], light[1]) {
            features.lights.push(Light { position: light, color: FIRE, reach: 9.0 });
        }
    }

    /// A ring of candles hanging on a chain at `z` from `ceiling`, lighting up the room
    #[allow(clippy::too_many_arguments)]
    fn chandelier(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features, c: [f32; 2], z: f32, ceiling: f32, reach: f32) {
        let at = &self.at;
        for k in 0..8 {
            let a = k as f32 * TAU / 8.0;
            s.push(blob(at, [c[0] + a.cos() * 1.0, c[1] + a.sin() * 1.0, z + 0.14], [0.06, 0.06, 0.11], (FLAME, AIR)));
        }
        s.push(blob(at, [c[0], c[1], z], [1.1, 1.1, 0.07], (DARK_STEEL, GOLD)));
        s.push(up(at, c, [z, ceiling + 0.2], [0.04, 0.04], (DARK_STEEL, AIR)));
        let p = at.point(c[0], c[1], z - 0.5);
        if t.inside(p[0], p[1]) {
            features.lights.push(Light { position: p, color: [1.0, 0.7, 0.4], reach });
        }
    }

    /// A lantern hanging at `p`, with its flame in a bronze frame
    fn lantern(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features, p: Vec3, reach: f32) {
        let at = &self.at;
        let [u, v, z] = p;
        let b = |du: [f32; 2], dv: [f32; 2], dz: [f32; 2], m: u8| bl(at, [u + du[0], u + du[1]], [v + dv[0], v + dv[1]], [z + dz[0], z + dz[1]], (m, AIR));
        s.push(b([-0.06, 0.06], [-0.06, 0.06], [0.19, 0.27], BRONZE));
        s.push(b([-0.14, 0.14], [-0.14, 0.14], [0.13, 0.19], BRONZE));
        for (du, dv) in [(-0.11, -0.11), (0.11, -0.11), (-0.11, 0.11), (0.11, 0.11)] {
            s.push(b([du - 0.025, du + 0.025], [dv - 0.025, dv + 0.025], [-0.13, 0.13], DARK_STEEL));
        }
        s.push(b([-0.085, 0.085], [-0.085, 0.085], [-0.11, 0.11], FLAME));
        s.push(b([-0.13, 0.13], [-0.13, 0.13], [-0.18, -0.12], BRONZE));
        let light = at.point(u, v, z - 0.35);
        if t.inside(light[0], light[1]) {
            features.lights.push(Light { position: light, color: FIRE, reach });
        }
    }

    /// A lamp post standing at `c` on the ground, with a lantern hung from an arm reaching `out`
    fn lamp_post(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features, c: [f32; 2], out: [f32; 2]) {
        let at = &self.at;
        let z = self.ground(c[0], c[1]);
        let top = z + 3.0;
        let tip = [c[0] + out[0] * 0.6, c[1] + out[1] * 0.6];
        s.push(bl(at, [c[0].min(tip[0]) - 0.04, c[0].max(tip[0]) + 0.04], [c[1].min(tip[1]) - 0.04, c[1].max(tip[1]) + 0.04], [top - 0.12, top - 0.04], (DARK_STEEL, AIR)));
        s.push(bl(at, [tip[0] - 0.015, tip[0] + 0.015], [tip[1] - 0.015, tip[1] + 0.015], [top - 0.2, top - 0.12], (DARK_STEEL, AIR)));
        s.push(bl(at, [c[0] - 0.08, c[0] + 0.08], [c[1] - 0.08, c[1] + 0.08], [z - 0.4, top], (DARK_STEEL, AIR)));
        s.push(bl(at, [c[0] - 0.18, c[0] + 0.18], [c[1] - 0.18, c[1] + 0.18], [z - 0.3, z + 0.2], self.stones.trim));
        self.lantern(t, s, features, [tip[0], tip[1], top - 0.47], 12.0);
    }

    /// A banner hanging on a wall facing `out` from a gold rod, its middle on the wall at `face`,
    /// `half` wide, from z[0] up to z[1]
    fn banner(&self, s: &mut Vec<Shape>, face: [f32; 2], out: [f32; 2], half: f32, z: [f32; 2]) {
        let p = self.at.point(face[0], face[1], 0.0);
        let w = Frame { x: p[0], y: p[1], f: self.at.dir(out) };
        let gold = (GOLD, AIR);
        s.push(Shape::capsule(w.point(0.18, -half - 0.25, z[1]), w.point(0.18, half + 0.25, z[1]), 0.06, 0.06, GOLD, AIR, false));
        s.push(w.block([0.0, 0.15], [-half * 0.2, half * 0.2], [z[0] + 0.3, z[1] - 0.15], gold.0, gold.1));
        s.push(w.block([0.0, 0.13], [-half, half], [z[0] + 0.3, z[1]], self.stones.banner, AIR));
        // Cut to a point at the bottom
        s.push(w.block([0.0, 0.13], [-half * 0.55, half * 0.55], [z[0], z[0] + 0.3], self.stones.banner, AIR));
    }

    /// What's in the throne room: the dais with the throne on it and a great banner behind,
    /// braziers either side, pillars, a carpet up to it, banners on the walls, a chandelier and
    /// torches either side of the way in
    fn throne_room(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features) {
        let at = &self.at;
        let st = self.stones;
        let (u0, u1, hv) = self.throne_room_box();
        let (f, ceiling) = (self.throne, self.throne + 9.5);
        let gold = (GOLD, AIR);
        let top = f + 3.0 * RISE;
        s.push(bl(at, [u0, u0 + 2.2], [-3.4, 3.4], [top, top + 0.0625], (CLOTH, AIR)));
        for j in 0..3 {
            let j = j as f32;
            s.push(bl(at, [u0, u0 + 4.2 - j], [-(5.0 - 0.7 * j), 5.0 - 0.7 * j], [f - 0.3, f + (j + 1.0) * RISE], st.trim));
        }
        // The throne, of gold, cushioned in red
        let (d, ut) = (top + 0.0625, u0 + 1.0);
        s.push(Shape::sphere(at.point(ut - 0.5, 0.0, d + 2.95), 0.2, GOLD, AIR, false));
        for v in [-0.6, 0.6] {
            s.push(Shape::sphere(at.point(ut - 0.5, v, d + 2.5), 0.11, GOLD, AIR, false));
            s.push(Shape::sphere(at.point(ut + 0.45, v * 0.97, d + 0.9), 0.08, GOLD, AIR, false));
        }
        s.push(bl(at, [ut - 0.43, ut - 0.37], [-0.42, 0.42], [d + 0.75, d + 2.35], (st.banner, AIR)));
        s.push(bl(at, [ut - 0.62, ut - 0.38], [-0.62, 0.62], [d, d + 2.75], gold));
        s.push(bl(at, [ut - 0.4, ut + 0.42], [-0.48, 0.48], [d + 0.48, d + 0.6], (CLOTH, AIR)));
        for v in [-1.0, 1.0] {
            s.push(bl(at, [ut - 0.4, ut + 0.48], [v * 0.5, v * 0.66], [d + 0.48, d + 0.86], gold));
        }
        s.push(bl(at, [ut - 0.45, ut + 0.48], [-0.62, 0.62], [d, d + 0.48], gold));
        // The great banner behind it, with the arms of the house on it
        s.push(blob(at, [u0 + 0.12, 0.0, f + 6.4], [0.06, 0.6, 0.7], gold));
        s.push(bl(at, [u0 - 0.05, u0 + 0.1], [-1.9, 1.9], [f + 3.2, f + 8.6], (st.banner, AIR)));
        s.push(bl(at, [u0 - 0.05, u0 + 0.06], [-2.1, 2.1], [f + 3.0, f + 8.8], gold));
        s.push(rod(at, [u0 + 0.16, -2.4, f + 8.75], [u0 + 0.16, 2.4, f + 8.75], 0.06, gold));
        for v in [-3.8, 3.8] {
            self.brazier(t, s, features, [u0 + 5.0, v], f);
        }
        // Pillars down both sides, banners on the walls behind them
        let middle = (u0 + u1) * 0.5 + 1.0;
        let pv = hv - 2.0;
        for u in [u0 + 3.2, middle, u1 - 1.8] {
            for v in [-pv, pv] {
                s.push(bl(at, [u - 0.62, u + 0.62], [v - 0.62, v + 0.62], [ceiling - 0.4, ceiling + 0.2], st.trim));
                s.push(bl(at, [u - 0.65, u + 0.65], [v - 0.65, v + 0.65], [f - 0.3, f + 0.45], st.trim));
                s.push(up(at, [u, v], [f, ceiling], [0.45, 0.45], st.trim));
            }
            for side in [-1.0, 1.0] {
                self.banner(s, [u, side * hv], [0.0, -side], 0.55, [f + 3.0, f + 7.4]);
            }
        }
        // The carpet from the way in up to the dais, edged in gold
        s.push(bl(at, [u0 + 4.2, self.keep[1] + 0.2], [-1.3, 1.3], [f, f + 0.0625], (CLOTH, AIR)));
        for v in [-1.0, 1.0] {
            s.push(bl(at, [u0 + 4.2, self.keep[1] + 0.2], [v * 1.3, v * 1.45], [f, f + 0.0625], gold));
        }
        self.chandelier(t, s, features, [(u0 + 4.2 + u1) * 0.5, 0.0], ceiling - 2.4, ceiling, 16.0);
        for v in [-3.4, 3.4] {
            self.torch(t, s, features, [u1, v], [-1.0, 0.0], f + 2.6);
        }
    }

    /// What's in the great hall: the stair up to the throne room at its far end, pillars down both
    /// sides holding up the beams of its open roof, long tables with benches and candles, a carpet
    /// up the middle, banners, chandeliers, and the doors standing open
    fn hall_inside(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features, rng: &mut Rng) {
        let at = &self.at;
        let st = self.stones;
        let (u0, u1, hv) = (self.keep[1] + 1.4, -1.4, self.hall_half - 1.4);
        let (w, eaves) = (self.ward, self.hall_eaves);
        let ridge = eaves - 0.25 + (self.hall_half + 0.6) * self.hall_pitch;
        let n = ((self.throne - w) / RISE).round() as usize;
        for k in 0..n {
            let top = self.throne - k as f32 * RISE;
            let (a, b) = (u0 + k as f32 * RUN, u0 + (k + 1) as f32 * RUN);
            s.push(bl(at, [a, b], [-3.0, 3.0], [w - 0.3, top], st.trim));
            for v in [-1.0, 1.0] {
                s.push(bl(at, [a, b], [v * 3.0, v * 3.5], [w - 0.3, top + 0.95], st.trim));
            }
        }
        // Pillars, the beams over them, and posts up from those to the ridge
        let pv = hv - 1.6;
        let (stair_end, posts) = self.hall_posts();
        for &u in &posts {
            for v in [-pv, pv] {
                s.push(bl(at, [u - 0.55, u + 0.55], [v - 0.55, v + 0.55], [w - 0.3, w + 0.4], st.trim));
                s.push(up(at, [u, v], [w, eaves + 0.3], [0.42, 0.42], st.trim));
            }
            s.push(rod(at, [u, -hv - 0.2, eaves - 0.1], [u, hv + 0.2, eaves - 0.1], 0.22, (WOOD, BARK_DARK)));
            s.push(up(at, [u, 0.0], [eaves - 0.1, ridge - 0.3], [0.17, 0.17], (WOOD, BARK_DARK)));
            for v in [-1.0, 1.0] {
                s.push(rod(at, [u, v * pv, eaves - 0.1], [u, 0.0, (eaves + ridge) * 0.5], 0.13, (WOOD, BARK_DARK)));
            }
            for side in [-1.0, 1.0] {
                self.banner(s, [u, side * hv], [0.0, -side], 0.5, [w + 2.6, eaves - 0.9]);
            }
        }
        // The long tables, with platters and goblets down them, their benches clear of the pillars
        let (ta, tb) = (stair_end + 1.8, u1 - 2.6);
        let tv = (pv - 1.6).min(3.4);
        for v in [-tv, tv] {
            let mut u = ta + 0.8;
            while u < tb - 0.6 {
                s.push(up(at, [u, v + 0.25], [w + 0.82, w + 1.0], [0.04, 0.05], (GOLD, AIR)));
                s.push(blob(at, [u, v - 0.2, w + 0.84], [0.17, 0.17, 0.02], (BRONZE, AIR)));
                u += 1.6;
            }
            s.push(bl(at, [ta, tb], [v - 0.6, v + 0.6], [w + 0.72, w + 0.82], (WOOD, AIR)));
            for u in [ta + 0.3, (ta + tb) * 0.5, tb - 0.3] {
                s.push(bl(at, [u - 0.1, u + 0.1], [v - 0.45, v + 0.45], [w, w + 0.72], (BARK_DARK, AIR)));
            }
            for side in [-1.0, 1.0] {
                let b = v + side * 0.95;
                s.push(bl(at, [ta + 0.2, tb - 0.2], [b - 0.18, b + 0.18], [w, w + 0.45], (WOOD, BARK_DARK)));
            }
            for u in [ta + (tb - ta) * 0.3, ta + (tb - ta) * 0.7] {
                let p = at.point(u, v, w + 0.82);
                if t.inside(p[0], p[1]) {
                    candle(features, p, rng);
                }
            }
        }
        // The carpet from the door to the stair
        s.push(bl(at, [stair_end, 0.0], [-1.3, 1.3], [w, w + 0.0625], (CLOTH, AIR)));
        for v in [-1.0, 1.0] {
            s.push(bl(at, [stair_end, 0.0], [v * 1.3, v * 1.45], [w, w + 0.0625], (GOLD, AIR)));
        }
        for k in [0.28, 0.72] {
            self.chandelier(t, s, features, [ta + (tb - ta) * k, 0.0], eaves - 0.9, ridge - 0.4, 14.0);
        }
        // The doors, open against the wall either side of the doorway, and torches by them
        for side in [-1.0, 1.0] {
            s.push(bl(at, [u1 - 0.14, u1], [side * 2.2, side * 4.2], [w, w + 4.0], (WOOD, BARK_DARK)));
            self.torch(t, s, features, [u1, side * 5.0], [-1.0, 0.0], w + 2.6);
        }
    }

    /// The gate's fittings: its portcullis drawn up, the gates standing open against the sides of
    /// the way through, and a lantern hanging in the middle of it
    fn gate_fittings(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features) {
        let at = &self.at;
        let (f, g) = (self.front, self.gate_floor);
        let iron = (DARK_STEEL, AIR);
        let pu = f + 0.9;
        for k in 0..9 {
            let v = -1.9 + k as f32 * 0.475;
            s.push(rod(at, [pu, v, g + 3.6], [pu, v, g + 6.6], 0.06, iron));
        }
        for z in [g + 4.3, g + 5.5] {
            s.push(rod(at, [pu, -2.1, z], [pu, 2.1, z], 0.07, iron));
        }
        for side in [-1.0, 1.0] {
            for z in [g + 0.8, g + 3.2] {
                s.push(bl(at, [f - 2.7, f - 0.5], [side * 2.0, side * 2.06], [z, z + 0.16], iron));
            }
            s.push(bl(at, [f - 2.7, f - 0.5], [side * 2.04, side * 2.2], [g, g + 4.1], (WOOD, BARK_DARK)));
        }
        let lu = f - 3.2;
        s.push(rod(at, [lu, 0.0, g + 4.25], [lu, 0.0, g + 6.8], 0.03, iron));
        self.lantern(t, s, features, [lu, 0.0, g + 4.0], 10.0);
    }

    /// The rooms and passages cut through the walls, with their floors: the way through the
    /// gatehouse and the archway through the front range, the great hall and its doorway, the
    /// archway from it into the keep and the throne room in there
    fn rooms(&self, s: &mut Vec<Shape>) {
        let at = &self.at;
        let st = self.stones;
        let w = self.ward;
        let arch = |s: &mut Vec<Shape>, u: [f32; 2], z: f32, r: f32| s.push(rod(at, [u[0], 0.0, z], [u[1], 0.0, z], r, (AIR, AIR)).placed(Placement::Clear));
        let (f, g) = (self.front, self.gate_floor);
        s.push(at.room([f - 7.6, f + 1.6], [-2.2, 2.2], [g, g + 4.6]));
        arch(s, [f - 7.6, f + 1.6], g + 4.6, 2.2);
        s.push(bl(at, [f - 7.6, f + 1.6], [-2.2, 2.2], [g - 0.5, g], st.paving[1]).placed(Placement::Clear));
        s.push(at.room([self.court - 0.2, self.range + 0.2], [-3.0, 3.0], [w, w + 4.6]));
        arch(s, [self.court, self.range], w + 4.6, 3.0);
        s.push(at.room([-1.6, 0.3], [-2.2, 2.2], [w, w + 4.4]));
        arch(s, [-1.6, 0.3], w + 4.4, 2.2);
        let hv = self.hall_half - 1.4;
        s.push(at.room([self.keep[1] + 1.4, -1.4], [-hv, hv], [w, self.hall_eaves - 0.3]));
        s.push(bl(at, [self.keep[1] + 1.4, -1.4], [-hv, hv], [w - 0.3, w], st.paving[0]).placed(Placement::Clear));
        let (tu0, tu1, tv) = self.throne_room_box();
        let tf = self.throne;
        s.push(at.room([tu1 - 0.2, self.keep[1] + 1.6], [-2.4, 2.4], [tf, tf + 4.6]));
        arch(s, [tu1, self.keep[1] + 1.4], tf + 4.6, 2.4);
        s.push(bl(at, [tu1 - 0.2, self.keep[1] + 1.4], [-2.4, 2.4], [tf - 0.3, tf], st.trim).placed(Placement::Clear));
        s.push(at.room([tu0, tu1], [-tv, tv], [tf, tf + 9.5]));
        // The throne room's floor, in squares of pale and dark stone
        let (nu, nv) = (((tu1 - tu0) / 2.0).round().max(1.0), ((2.0 * tv) / 2.0).round().max(1.0));
        let (du, dv) = ((tu1 - tu0) / nu, 2.0 * tv / nv);
        for i in 0..nu as usize {
            for j in 0..nv as usize {
                let m = if (i + j) % 2 == 0 { (st.wall.0, AIR) } else { (OBSIDIAN, AIR) };
                let (a, b) = (tu0 + i as f32 * du, -tv + j as f32 * dv);
                s.push(bl(at, [a, a + du], [b, b + dv], [tf - 0.3, tf], m).placed(Placement::Clear));
            }
        }
    }

    /// The keep: battlements round its roof on a corbelled band, the throne room's tall windows
    /// lit down its sides, rows of smaller ones above and round the back, banners, solid inside
    /// round the throne room
    fn keep(&self, s: &mut Vec<Shape>, rng: &mut Rng) {
        let at = &self.at;
        let st = self.stones;
        let (u0, u1, kh, top) = (self.keep[0], self.keep[1], self.keep_half, self.keep_top);
        let w = self.ward;
        let (tu0, tu1, tv) = self.throne_room_box();
        let f = self.throne;
        let old = self.old(Worn::Wall, w, 0.7);
        battlements(s, at, [u1 - 0.6, u1 + 0.35], [-kh - 0.35, kh + 0.35], top, st.wall, old);
        battlements(s, at, [u0 - 0.35, u0 + 0.6], [-kh - 0.35, kh + 0.35], top, st.wall, old);
        for side in [-1.0, 1.0] {
            let v = if side < 0.0 { [-kh - 0.35, -kh + 0.6] } else { [kh - 0.6, kh + 0.35] };
            battlements(s, at, [u0 - 0.35, u1 + 0.35], v, top, st.wall, old);
            let band = if side < 0.0 { [-kh - 0.35, -kh + 0.65] } else { [kh - 0.65, kh + 0.35] };
            s.push(bl(at, [u0 - 0.35, u1 + 0.35], band, [top - 0.9, top], st.trim));
        }
        s.push(bl(at, [u1 - 0.65, u1 + 0.35], [-kh, kh], [top - 0.9, top], st.trim));
        s.push(bl(at, [u0 - 0.35, u0 + 0.65], [-kh, kh], [top - 0.9, top], st.trim));
        // The throne room's windows, between its pillars
        let middle = (tu0 + tu1) * 0.5 + 1.0;
        for side in [-1.0, 1.0] {
            for u in [(tu0 + 3.2 + middle) * 0.5, (middle + tu1 - 1.8) * 0.5] {
                window(s, at, [u, side * kh], [0.0, side], kh - tv, 0.66, [f + 2.4, f + 7.6], st.glass, st.trim);
            }
            // Smaller ones above, and round the back either side of the great tower
            let mut u = u0 + 4.0;
            while u < u1 - 3.0 {
                let glass = if rng.next() < 0.5 { st.glass } else { OBSIDIAN };
                window(s, at, [u, side * kh], [0.0, side], 0.7, 0.45, [top - 3.8, top - 1.5], glass, st.trim);
                u += 3.6;
            }
            for z in [[w + 4.0, w + 6.6], [top - 3.8, top - 1.5]] {
                let glass = if rng.next() < 0.5 { st.glass } else { OBSIDIAN };
                window(s, at, [u0, side * (kh - 3.4)], [-1.0, 0.0], 0.7, 0.45, z, glass, st.trim);
            }
            self.banner(s, [u0, side * 7.2], [-1.0, 0.0], 0.7, [w + 3.5, top - 4.5]);
            // Its front corners, beyond the roof of the great hall
            if kh - 2.6 > self.hall_half + 1.6 {
                window(s, at, [u1, side * (kh - 2.4)], [1.0, 0.0], 0.7, 0.45, [top - 3.8, top - 1.5], st.glass, st.trim);
            }
        }
        // Solid stone inside round the throne room, so only the walls are kept
        let core = (st.wall.0, AIR);
        s.push(bl(at, [u0 + 0.3, tu0 - 0.3], [-kh + 0.3, kh - 0.3], [w - 0.1, top - 0.3], core));
        if top - 0.3 > f + 9.8 {
            s.push(bl(at, [tu0 - 0.3, u1 - 0.3], [-kh + 0.3, kh - 0.3], [f + 9.8, top - 0.3], core));
        }
        for side in [-1.0, 1.0] {
            s.push(bl(at, [tu0, tu1], [side * (tv + 0.3), side * (kh - 0.3)], [f - 0.3, f + 9.8], core));
        }
        s.push(bl(at, [u0, u1], [-kh, kh], [w - 0.4, top], st.wall).patterned(old));
    }

    /// The great hall: its walls with tall windows down both sides and buttresses between them,
    /// a rose window over its door, banners and torches either side of that, its roof open inside
    /// and a slender spire on the ridge
    fn hall(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features, rng: &mut Rng) {
        let at = &self.at;
        let st = self.stones;
        let (u0, u1, hh) = (self.keep[1], 0.0, self.hall_half);
        let (w, eaves) = (self.ward, self.hall_eaves);
        let across = hh + 0.6;
        let roof_eaves = eaves - 0.25;
        let pitch = self.hall_pitch;
        let rise = across * pitch;
        let ridge = roof_eaves + rise;
        let mid = (u0 + u1) * 0.5;
        let old = self.old(Worn::Wall, w, 0.6);
        // A slender spire on the ridge, where there's room for it under the sky
        let fleche = (self.sky - 0.2 - FINIAL - ridge - 1.6).min(6.5);
        if fleche > 2.0 {
            self.spire(s, [mid, 0.0], ridge + 1.6, 1.2, fleche, false);
            s.push(bl(at, [mid - 1.2, mid + 1.2], [-1.2, 1.2], [ridge - 1.5, ridge + 1.6], st.wall).patterned(old));
        }
        // The rose window, and windows either side of the door under it
        let rz = eaves + rise * 0.35;
        s.push(blob(at, [-0.5, 0.0, rz], [1.0, 1.5, 1.5], (st.glass, AIR)));
        s.push(blob(at, [0.0, 0.0, rz], [0.36, 1.85, 1.85], st.trim));
        for side in [-1.0, 1.0] {
            window(s, at, [0.0, side * hh * 0.64], [1.0, 0.0], 1.4, 0.55, [w + 2.0, w + 5.8], st.glass, st.trim);
            self.banner(s, [0.0, side * 3.3], [1.0, 0.0], 0.42, [w + 4.6, eaves - 0.6]);
            self.torch(t, s, features, [0.0, side * 3.3], [1.0, 0.0], w + 2.6);
        }
        // Between the pillars inside, with buttresses outside where they stand
        let (_, posts) = self.hall_posts();
        let mut u = posts.first().copied().unwrap_or(u0 + 6.0) - 1.9;
        while u < u1 - 2.4 {
            for side in [-1.0, 1.0] {
                let glass = if rng.next() < 0.75 { st.glass } else { OBSIDIAN };
                window(s, at, [u, side * hh], [0.0, side], 1.4, 0.62, [w + 2.4, eaves - 1.0], glass, st.trim);
                let b = u + 1.9;
                if b < u1 - 1.0 {
                    s.push(bl(at, [b - 0.45, b + 0.45], [side * hh, side * (hh + 0.5)], [eaves - 2.5, eaves - 0.4], st.trim));
                    s.push(bl(at, [b - 0.5, b + 0.5], [side * hh, side * (hh + 0.95)], [w - 0.4, eaves - 2.5], st.trim).patterned(old));
                }
            }
            u += 3.8;
        }
        pitched(s, at, true, [u0 - 0.3, u1 + 0.35], 0.0, across, roof_eaves, pitch, st.roof.cover, st.trim.0, self.old_roof(st.roof, 0.8));
        // The space under the roof, open over the hall
        let mut z = eaves - 0.3;
        loop {
            let half = (across * (1.0 - (z + 0.5 - roof_eaves) / rise) - 0.5).min(hh - 1.4);
            if half < 0.6 {
                break;
            }
            s.push(at.room([u0 + 1.4, u1 - 1.4], [-half, half], [z, z + 0.5]));
            z += 0.5;
        }
        for end in [[u1 - 1.4, u1], [u0, u0 + 1.4]] {
            gable_end(s, at, true, end, 0.0, hh, eaves, across, roof_eaves, pitch, st.wall, self.old(Worn::Wall, w, 0.0));
        }
        // Plain stone inside its thick side walls, so only their faces are kept
        for side in [-1.0, 1.0] {
            s.push(bl(at, [u0 + 0.3, u1 - 0.3], [side * (hh - 1.1), side * (hh - 0.3)], [w - 0.1, eaves - 0.3], (st.wall.0, AIR)));
        }
        s.push(bl(at, [u0, u1], [-hh, hh], [w - 0.4, eaves], st.wall).patterned(old));
        s.push(bl(at, [u0, u1 + 0.25], [-hh - 0.25, hh + 0.25], [w - 0.4, w + 0.5], st.trim));
    }

    /// One of the wings along the courtyard, on `side`: two rows of windows on both faces, a
    /// pitched roof with chimneys, solid inside
    fn wing(&self, side: f32, s: &mut Vec<Shape>, rng: &mut Rng) {
        let at = &self.at;
        let st = self.stones;
        let (inner, outer) = (side * self.court_half, side * (self.court_half + self.wing));
        let (u0, u1) = (0.0, self.court);
        let (w, eaves) = (self.ward, self.wing_eaves);
        let mid = side * (self.court_half + self.wing * 0.5);
        let across = self.wing * 0.5 + 0.45;
        let pitch = self.wing_pitch;
        let ridge = eaves - 0.25 + across * pitch;
        let old = self.old(Worn::Wall, w, 0.7);
        for u in [u1 * 0.3, u1 * 0.72] {
            let v = side * (self.court_half + self.wing * 0.72);
            s.push(bl(at, [u - 0.6, u + 0.6], [v - 0.6, v + 0.6], [ridge + 0.6, ridge + 0.85], st.trim));
            s.push(bl(at, [u - 0.5, u + 0.5], [v - 0.5, v + 0.5], [eaves, ridge + 0.6], st.trim).patterned(old));
        }
        let rows = [[w + 1.8, w + 4.6], [w + 5.9, (eaves - 1.0).min(w + 8.8)]];
        let mut u = 4.0;
        while u < u1 - 3.5 {
            for z in rows {
                let glass = if rng.next() < 0.45 { st.glass } else { OBSIDIAN };
                window(s, at, [u, inner], [0.0, -side], 0.6, 0.5, z, glass, st.trim);
                // Clear of the tower at its back corner
                if u > 5.6 {
                    let glass = if rng.next() < 0.45 { st.glass } else { OBSIDIAN };
                    window(s, at, [u, outer], [0.0, side], 0.6, 0.5, z, glass, st.trim);
                }
            }
            u += 3.6;
        }
        pitched(s, at, true, [u0 - 0.3, u1 + 0.3], mid, across, eaves - 0.25, pitch, st.roof.cover, st.trim.0, self.old_roof(st.roof, 1.0));
        for end in [[u0, u0 + 0.6], [u1 - 0.6, u1]] {
            gable_end(s, at, true, end, mid, self.wing * 0.5, eaves, across, eaves - 0.25, pitch, st.wall, self.old(Worn::Wall, w, 0.0));
        }
        let (a, b) = (inner.min(outer), inner.max(outer));
        s.push(bl(at, [u0 + 0.3, u1 - 0.3], [a + 0.3, b - 0.3], [w - 0.1, eaves - 0.3], (st.wall.0, AIR)));
        s.push(bl(at, [u0, u1], [a, b], [w - 0.4, eaves], st.wall).patterned(old));
        s.push(bl(at, [u0, u1], [a - 0.25, b + 0.25], [w - 0.4, w + 0.5], st.trim));
    }

    /// The front range, across the front of the inner ward: battlements along both sides, the
    /// arms of the house over its archway, windows either side, solid inside
    fn front_range(&self, s: &mut Vec<Shape>, rng: &mut Rng) {
        let at = &self.at;
        let st = self.stones;
        let (u0, u1) = (self.court, self.range);
        let hv = self.court_half + self.wing;
        let (w, top) = (self.ward, self.range_top);
        let old = self.old(Worn::Wall, w, 0.8);
        battlements(s, at, [u1 - 0.6, u1], [-hv, hv], top, st.wall, old);
        battlements(s, at, [u0, u0 + 0.6], [-hv, hv], top, st.wall, old);
        let arms = (w + 7.6 + top) * 0.5 + 0.1;
        let tall = ((top - w - 7.6) * 0.5 - 0.15).clamp(0.6, 1.3);
        s.push(blob(at, [u1 + 0.06, 0.0, arms], [0.16, tall * 0.8, tall], (st.banner, GOLD)));
        s.push(blob(at, [u1 - 0.02, 0.0, arms], [0.12, tall * 0.8 + 0.18, tall + 0.18], (GOLD, AIR)));
        let rows = [[w + 1.6, w + 4.2], [w + 5.6, top - 1.6]];
        let mut v = 6.0;
        while v < hv - 5.8 {
            for side in [-1.0, 1.0] {
                for z in rows {
                    let glass = if rng.next() < 0.45 { st.glass } else { OBSIDIAN };
                    window(s, at, [u1, side * v], [1.0, 0.0], 0.6, 0.5, z, glass, st.trim);
                    if v < self.court_half - 3.4 {
                        let glass = if rng.next() < 0.45 { st.glass } else { OBSIDIAN };
                        window(s, at, [u0, side * v], [-1.0, 0.0], 0.6, 0.5, z, glass, st.trim);
                    }
                }
            }
            v += 3.6;
        }
        // Solid round the archway, so its sides are of brick, then solid stone inside
        s.push(bl(at, [u0, u1], [-4.4, 4.4], [w - 0.4, top - 0.2], st.wall).patterned(old));
        s.push(bl(at, [u0 + 0.3, u1 - 0.3], [-hv + 0.3, hv - 0.3], [w - 0.1, top - 0.3], (st.wall.0, AIR)));
        s.push(bl(at, [u0, u1], [-hv, hv], [w - 0.4, top], st.wall).patterned(old));
        s.push(bl(at, [u0 - 0.25, u1 + 0.25], [-hv, hv], [w - 0.4, w + 0.5], st.trim));
    }

    /// The gatehouse over the way in: battlements round its top, a banner over the archway,
    /// torches either side of it
    fn gatehouse(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features) {
        let at = &self.at;
        let st = self.stones;
        let (u0, u1, gv) = (self.front - 7.5, self.front + 1.5, 4.4);
        let top = self.wall_top + 3.5;
        let g = self.gate_floor;
        let (low, _) = self.span([u0, u1], [-gv, gv], 2.5);
        let old = self.old(Worn::Wall, f32::MIN, 1.8);
        battlements(s, at, [u1 - 0.6, u1], [-gv, gv], top, st.wall, old);
        battlements(s, at, [u0, u0 + 0.6], [-gv, gv], top, st.wall, old);
        if top - 1.0 > g + 8.2 {
            self.banner(s, [u1, 0.0], [1.0, 0.0], 1.1, [g + 7.4, top - 0.9]);
        }
        for side in [-1.0, 1.0] {
            self.torch(t, s, features, [u1, side * 3.3], [1.0, 0.0], g + 2.9);
        }
        // Ivy spilling down over the parapet in a curtain of strands, either side of the arch and
        // over it, longer at the sides but well clear of the torches, the more the greener the
        // castle
        let mut rng = Rng(hash(self.seed ^ 0x1717));
        let mut v = -gv + rng.range(0.1, 0.5);
        while v < gv - 0.3 {
            let wide = rng.range(0.18, 0.4);
            if rng.next() < st.green * 1.1 {
                let torch = (v + wide * 0.5).abs() - 3.3;
                let longest = if v.abs() < 2.8 { top - g - 5.6 } else if torch.abs() < 0.7 { top - g - 4.6 } else { top - g - 0.8 };
                let length = rng.range(0.5, longest.clamp(0.6, 7.0).min(if v.abs() < 2.8 { 2.4 } else { 7.0 }));
                let mut strand = bl(at, [u1 - 0.02, u1 + rng.range(0.1, 0.2)], [v, v + wide], [top + 0.6 - length, top + 0.75], (LEAVES_DARK, VINE));
                strand.ragged = true;
                s.push(strand);
            }
            v += wide + rng.range(0.0, 0.25);
        }
        if top - 0.3 > g + 7.4 {
            s.push(bl(at, [u0 + 0.3, u1 - 0.3], [-gv + 0.3, gv - 0.3], [g + 7.4, top - 0.3], (st.wall.0, AIR)));
        }
        s.push(bl(at, [u0, u1], [-gv, gv], [low - 0.6, top], st.wall).patterned(old));
    }

    /// Side `k` of the curtain wall, 0 the front, 1 the back, then either side: merlons along its
    /// outer edge on a corbelled band, a low parapet along the inside, arrow slits, solid inside
    fn curtain(&self, k: usize, s: &mut Vec<Shape>, rng: &mut Rng) {
        let at = &self.at;
        let st = self.stones;
        let (f, b, sd) = (self.front, self.back, self.side);
        let top = self.wall_top;
        // Where it goes, and which way is out
        let (u, v, out) = match k {
            0 => ([f - 3.0, f], [-sd, sd], [1.0, 0.0]),
            1 => ([b, b + 3.0], [-sd, sd], [-1.0, 0.0]),
            2 => ([b, f], [sd - 3.0, sd], [0.0, 1.0]),
            _ => ([b, f], [-sd, 3.0 - sd], [0.0, -1.0]),
        };
        let (low, _) = self.span(u, v, 5.0);
        let along_u = out[0] == 0.0;
        // A strip `a` to `b` in from the outer face, all along it
        let strip = |a: f32, b: f32| -> ([f32; 2], [f32; 2]) {
            let (o0, o1) = if out[0] + out[1] > 0.0 { (-b, -a) } else { (a, b) };
            if along_u {
                let face = if out[1] > 0.0 { v[1] } else { v[0] };
                (u, [face + o0, face + o1])
            } else {
                let face = if out[0] > 0.0 { u[1] } else { u[0] };
                ([face + o0, face + o1], v)
            }
        };
        let mid = [(u[0] + u[1]) * 0.5, (v[0] + v[1]) * 0.5];
        let old = self.old(Worn::Wall, f32::MIN, self.greener(mid));
        let (su, sv) = strip(-0.35, 0.6);
        battlements(s, at, su, sv, top, st.wall, old);
        let (su, sv) = strip(2.55, 3.0);
        s.push(bl(at, su, sv, [top - 0.05, top + 0.7], st.wall).patterned(old));
        let (su, sv) = strip(-0.35, 0.6);
        s.push(bl(at, su, sv, [top - 0.9, top], st.trim));
        // Arrow slits along its outer face, wherever it stands high over the ground
        let length = if along_u { u[1] - u[0] } else { v[1] - v[0] };
        let mut a = 6.0;
        while a < length - 6.0 {
            let (fu, fv) = if along_u {
                (u[0] + a, if out[1] > 0.0 { v[1] } else { v[0] })
            } else {
                (if out[0] > 0.0 { u[1] } else { u[0] }, v[0] + a)
            };
            if self.ground(fu, fv) < top - 6.5 && (k != 0 || fv.abs() > 6.0) {
                let glass = if rng.next() < 0.2 { st.glass } else { OBSIDIAN };
                window(s, at, [fu, fv], out, 0.6, 0.15, [top - 4.6, top - 2.6], glass, st.trim);
            }
            a += 7.0;
        }
        let (su, sv) = strip(0.3, 2.7);
        s.push(bl(at, su, sv, [low, top - 0.3], (st.wall.0, AIR)));
        s.push(bl(at, u, v, [low - 0.6, top], st.wall).patterned(old));
    }


    /// The ways in: steps down from the gate outside and a paved way on from them, steps down from
    /// it inside and a road over the bailey to the grand stair, and that up to the inner ward,
    /// lamp posts along the way
    fn ways(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features) {
        let at = &self.at;
        let st = self.stones;
        let (f, g, w) = (self.front, self.gate_floor, self.ward);
        let path = st.paving[1];
        let out = self.descend(s, f + 1.6, 1.0, 2.4, g, path);
        self.paved(s, [out, f + 16.0], 2.4, path);
        let foot = self.range + self.steps as f32 * RUN;
        let inside = self.descend(s, f - 7.6, -1.0, 2.4, g, path);
        self.paved(s, [foot, inside], 2.4, path);
        let (worn, old) = (self.old(Worn::Steps, f32::MIN, 1.0), self.old(Worn::Wall, f32::MIN, 1.2));
        for k in 1..=self.steps {
            let top = w - k as f32 * RISE;
            let a = self.range + (k - 1) as f32 * RUN;
            let (low, _) = self.span([a, a + RUN], [-4.6, 4.6], 2.0);
            s.push(bl(at, [a, a + RUN], [-4.0, 4.0], [low - 0.4, top], st.trim).patterned(worn));
            for side in [-1.0, 1.0] {
                s.push(bl(at, [a, a + RUN], [side * 4.0, side * 4.6], [low - 0.4, top + 0.95], st.trim).patterned(old));
            }
        }
        for side in [-1.0, 1.0] {
            self.lamp_post(t, s, features, [foot + 0.6, side * 5.0], [0.0, -side]);
            self.lamp_post(t, s, features, [(foot + inside) * 0.5, side * 3.4], [0.0, -side]);
            self.lamp_post(t, s, features, [f + 9.0, side * 3.4], [0.0, -side]);
        }
    }

    /// Steps down from `top` along u, `dir` +1 or -1, from `start`, `half` either side of the
    /// middle, until they meet the ground: returns where they end
    fn descend(&self, s: &mut Vec<Shape>, start: f32, dir: f32, half: f32, top: f32, stone: (u8, u8)) -> f32 {
        let mut u = start;
        for k in 1..48 {
            let z = top - k as f32 * RISE;
            let (a, b) = (u.min(u + dir * RUN), u.max(u + dir * RUN));
            let (low, high) = self.span([a, b], [-half, half], 2.0);
            if z <= high + 0.12 {
                break;
            }
            s.push(bl(&self.at, [a, b], [-half, half], [low - 0.4, z], stone).patterned(self.old(Worn::Steps, f32::MIN, 1.0)));
            u += dir * RUN;
        }
        u
    }

    /// Paves the ground from u[0] to u[1] along the middle, `half` either side
    fn paved(&self, s: &mut Vec<Shape>, u: [f32; 2], half: f32, stone: (u8, u8)) {
        let (mut a, end) = (u[0].min(u[1]), u[0].max(u[1]));
        while a < end - 0.05 {
            let b = (a + 2.0).min(end);
            let (lo, hi) = self.at.rect([a, b], [-half, half]);
            let g = [(lo[0], lo[1]), (hi[0], lo[1]), (lo[0], hi[1]), (hi[0], hi[1])].map(|(x, y)| height_at(&self.settings, x, y));
            pave(s, lo, hi, g.iter().copied().fold(f32::MAX, f32::min) - 0.15, g.iter().copied().fold(f32::MIN, f32::max) + 0.15, stone, self.old(Worn::Floor, f32::MIN, 0.8));
            a = b;
        }
    }

    /// The courtyard's fountain, a basin of still water with a pillar rising from it to a bowl, and
    /// braziers round it
    fn courtyard(&self, t: &Terrain, s: &mut Vec<Shape>, features: &mut Features) {
        let at = &self.at;
        let st = self.stones;
        let (c, cw, w) = (self.court, self.court_half, self.ward);
        let fu = c * 0.5;
        let (outer, inner) = (2.6, 2.2);
        // Moss on the stone of it, more round the water
        let old = self.old(Worn::Wall, w, 1.8);
        s.push(Shape::sphere(at.point(fu, 0.0, w + 2.75), 0.22, GOLD, AIR, false));
        s.push(blob(at, [fu, 0.0, w + 2.2], [0.9, 0.9, 0.22], st.trim));
        s.push(up(at, [fu, 0.0], [w, w + 2.2], [0.32, 0.22], st.trim).patterned(old));
        s.push(at.room([fu - inner, fu + inner], [-inner, inner], [w + 0.15, w + 1.0]));
        s.push(bl(at, [fu - outer - 0.08, fu + outer + 0.08], [-outer - 0.08, outer + 0.08], [w + 0.5, w + 0.62], st.trim).patterned(self.old(Worn::Floor, w, 2.0)));
        s.push(bl(at, [fu - outer, fu + outer], [-outer, outer], [w - 0.35, w + 0.5], st.trim).patterned(old));
        let (lo, hi) = at.rect([fu - inner, fu + inner], [-inner, inner]);
        features.floors.push(Floor { lo, hi, height: w + 0.15, water: w + 0.45 });
        features.footprints.push([lo[0] - 0.4, lo[1] - 0.4, hi[0] + 0.4, hi[1] + 0.4, w + 0.7]);
        for u in [c * 0.18, c * 0.82] {
            for side in [-1.0, 1.0] {
                self.brazier(t, s, features, [u, side * (cw - 2.2)], w);
            }
        }
    }

    /// The inner ward's terrace: the courtyard paved in great squares of two stones, lawns round
    /// the rest with gravel walks, low parapets round its edges where nothing's built, and solid
    /// stone under it all, faced with brick
    fn terrace(&self, s: &mut Vec<Shape>) {
        let at = &self.at;
        let st = self.stones;
        let (u0, u1, hv) = (self.keep[0], self.range, self.ward_half);
        let (w, cw) = (self.ward, self.court_half);
        let (nu, nv) = ((self.court / 3.2).round().max(1.0), (2.0 * cw / 3.2).round().max(1.0));
        let (du, dv) = (self.court / nu, 2.0 * cw / nv);
        // Moss between the squares and in patches, more of it out by the walls
        for i in 0..nu as usize {
            for j in 0..nv as usize {
                let (a, b) = (i as f32 * du, -cw + j as f32 * dv);
                let green = 0.7 + 0.8 * ((b + dv * 0.5).abs() / cw);
                s.push(bl(at, [a, a + du], [b, b + dv], [w - 0.35, w], st.paving[(i + j) % 2]).placed(Placement::Clear).patterned(self.old(Worn::Floor, w, green)));
            }
        }
        s.push(bl(at, [self.court, u1], [-3.0, 3.0], [w - 0.35, w], st.paving[0]).placed(Placement::Clear).patterned(self.old(Worn::Floor, w, 1.0)));
        // Gravel walks round the great hall and out to the garden towers
        let gravel = (GRAVEL, STONE);
        for side in [-1.0, 1.0] {
            let v = [side * (self.hall_half + 1.3), side * (self.hall_half + 3.1)];
            s.push(bl(at, [self.keep[1] - 1.0, 0.0], v, [w - 0.35, w], gravel).placed(Placement::Clear));
        }
        s.push(bl(at, [self.keep[1] - 3.0, self.keep[1] - 1.0], [-self.hall_half - 3.1, self.hall_half + 3.1], [w - 0.35, w], gravel).placed(Placement::Clear));
        // Lawns, of a green no wolf or bandit turns up on, see animals::is_ground()
        s.push(bl(at, [u0, u1], [-hv, hv], [w - 0.35, w], (CACTUS, MOSS)));
        // Parapets round the edges where nothing's built
        let old = self.old(Worn::Wall, w, 1.4);
        let mut parapet = |u: [f32; 2], v: [f32; 2]| {
            let along_u = u[1] - u[0] > v[1] - v[0];
            let (a0, a1) = if along_u { (u[0], u[1]) } else { (v[0], v[1]) };
            let mut a = a0 + 1.0;
            while a < a1 - 0.8 {
                let (cu, cv) = if along_u { ([a - 0.35, a + 0.35], [v[0] - 0.08, v[1] + 0.08]) } else { ([u[0] - 0.08, u[1] + 0.08], [a - 0.35, a + 0.35]) };
                s.push(bl(at, cu, cv, [w + 1.0, w + 1.3], st.trim));
                a += 2.4;
            }
            s.push(bl(at, u, v, [w - 0.35, w + 1.0], st.trim).patterned(old));
        };
        let wv = cw + self.wing;
        for side in [-1.0, 1.0] {
            let sv = |a: f32, b: f32| if side < 0.0 { [-b, -a] } else { [a, b] };
            parapet([u0, u0 + 0.5], sv(self.keep_half, hv));
            parapet([u0, u1], sv(hv - 0.5, hv));
            parapet([u1 - 0.5, u1], sv(wv, hv));
        }
        let low = self.lowest - 0.6;
        s.push(bl(at, [u0 + 0.3, u1 - 0.3], [-hv + 0.3, hv - 0.3], [low, w - 0.35], (st.trim.0, AIR)));
        s.push(bl(at, [u0, u1], [-hv, hv], [low, w - 0.35], st.trim).patterned(self.old(Worn::Wall, f32::MIN, 1.3)));
    }

    /// Trees in the gardens of the inner ward and out in the bailey
    fn trees(&self, t: &Terrain, rng: &mut Rng) -> Vec<Object> {
        let mut spots = Vec::new();
        let hall = self.keep[1];
        let wv = self.court_half + self.wing;
        // Only where there's room for them
        for side in [-1.0, 1.0] {
            for k in [0.3, 0.75] {
                spots.push(([hall * (1.0 - k), side * (self.hall_half + 5.6)], true));
            }
            if self.ward_half - wv > 6.0 {
                spots.push(([self.court * 0.5, side * (wv + (self.ward_half - wv) * 0.5)], true));
            }
            if self.front - self.range > 13.0 {
                spots.push(([self.front - 10.0, side * (self.side - 9.0)], false));
            }
            if self.keep[0] - self.back >= 7.0 {
                spots.push(([self.back + 5.5, side * self.side * 0.78], false));
            }
        }
        spots
            .into_iter()
            .map(|(c, ward)| {
                let p = self.at.point(c[0], c[1], 0.0);
                let base = if ward { self.ward } else { height_at(&self.settings, p[0], p[1]) };
                t.tree(p[0], p[1], base, hash(rng.0 ^ hash(c[0].to_bits() ^ c[1].to_bits())))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Castles of every size turn up here and there, their towers and the rest under the sky,
    /// their ways up never steeper than a step, the throne room roomy enough for the guards to
    /// stand clear of its pillars
    #[test]
    fn castles_turn_up() {
        let settings = TerrainSettings { width_m: 32, height_m: 64, voxels_per_meter: 4, seed: 1, grass_tufts: false };
        let (terrain, _) = generate_terrain(&settings, [0, 0], &RiverCache::default());
        let cache = RiverCache::default();
        let mut found = [0; 3];
        let start = std::time::Instant::now();
        for cy in -4..4 {
            for cx in -4..4 {
                let Some(c) = terrain.castle_in(cx, cy, &cache) else {
                    continue;
                };
                found[c.size as usize] += 1;
                // Inside its curtain wall, as long as the stair up to its inner ward needs
                let (long, wide) = (c.front - c.back, 2.0 * c.side);
                let [least, most] = c.size.pick([[44.0, 62.0], [56.0, 76.0], [72.0, 96.0]]);
                assert!((least..=most).contains(&wide) && long < 140.0, "a {} castle {long} by {wide} m", c.size.name());
                for t in &c.towers {
                    assert!(t.tip() <= c.sky + 0.01, "a spire reaching {} m, over {}", t.tip(), c.sky);
                }
                let tallest = c.towers.iter().map(Tower::tip).fold(0.0, f32::max);
                assert!(tallest - c.ward >= c.size.least() - 0.5, "the tallest of a {} castle {} m over its ward", c.size.name(), tallest - c.ward);
                assert!(c.throne - c.ward < 0.6 * 6.0 && c.steps >= 1);
                let (tu0, tu1, tv) = c.throne_room_box();
                assert!(tu1 - tu0 >= 9.5 && tv >= 8.0, "a throne room {} by {}", tu1 - tu0, 2.0 * tv);
                println!(
                    "{} castle in ({cx}, {cy}): {long:.0} by {wide:.0} m, {} towers, its ward at {:.1} m, the tallest tower {:.1} m over it",
                    c.size.name(),
                    c.towers.len(),
                    c.ward,
                    tallest - c.ward
                );
            }
        }
        println!("{found:?} small, medium and large castles in 64 squares, planned in {:.2} s", start.elapsed().as_secs_f32());
        assert!(found.iter().all(|&n| n > 0));
    }


    /// How long the pieces of the world round a few castles take to build, as big and as detailed
    /// as the game builds them, each twice: `cargo test --release castle_pieces -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn castle_pieces_take() {
        let plan = TerrainSettings { width_m: 32, height_m: 64, voxels_per_meter: 4, seed: 1, grass_tufts: false };
        let (terrain, _) = generate_terrain(&plan, [0, 0], &RiverCache::default());
        let settings = TerrainSettings { width_m: 224, height_m: 64, voxels_per_meter: 16, seed: 1, grass_tufts: true };
        let cache = RiverCache::default();
        let mut castles = Vec::new();
        for cy in -3..3 {
            for cx in -3..3 {
                castles.extend(terrain.castle_in(cx, cy, &cache));
            }
        }
        // One of each size
        for size in [Size::Small, Size::Medium, Size::Large] {
            let Some(c) = castles.iter().find(|c| c.size == size) else {
                continue;
            };
            let m = c.at.point((c.front + c.back) * 0.5, 0.0, 0.0);
            let width = (c.bounds[2] - c.bounds[0]).max(c.bounds[3] - c.bounds[1]);
            let origin = [0, 1].map(|i| ((m[i] as f64 * 16.0) as i64 - 224 * 16 / 2).div_euclid(64) * 64);
            for _ in 0..2 {
                let start = std::time::Instant::now();
                generate_terrain(&settings, origin, &cache);
                println!("{} castle {width:.0} m across at ({:.0}, {:.0}), {} towers: {:.2} s", size.name(), m[0], m[1], c.towers.len(), start.elapsed().as_secs_f32());
            }
        }
    }
}
