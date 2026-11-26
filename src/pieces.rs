//! What's built of the materials in materials.rs, see bases::Kind: walls of birch, pine, dark oak
//! and palm planks, a fence, thatched and slate roofs, walls of cut stone, sandstone, marble,
//! basalt, obsidian and ice, a half wall, a slab, a cobbled floor, an arch, a marble pillar, glass
//! windows and panes, iron bars, an iron gate, a lamp post, a banner, a rug and a furnace. Each
//! piece's voxels are worked out here as bases.rs works out those of the first pieces, block by
//! block, and the build menu's tabs sort them all by what they're made of.

use crate::bases::{Kind, hash3};
use crate::worldgen::material::*;

/// The build menu's tabs, see building.rs: each with the pieces in it, in the order they're picked
pub const TABS: [(&str, &[Kind]); 6] = [
    ("Wood", &[Kind::WoodWall, Kind::BirchWall, Kind::PineWall, Kind::DarkWall, Kind::PalmWall, Kind::Floor, Kind::Stairs, Kind::Door, Kind::Fence, Kind::ThatchRoof]),
    ("Stone", &[Kind::StoneWall, Kind::CutStoneWall, Kind::HalfWall, Kind::Slab, Kind::CobbleFloor, Kind::Arch, Kind::SlateRoof]),
    ("Fine stone", &[Kind::SandstoneWall, Kind::MarbleWall, Kind::MarblePillar, Kind::BasaltWall, Kind::ObsidianWall, Kind::IceWall]),
    ("Clay & glass", &[Kind::ClayWall, Kind::GlassWindow, Kind::GlassPane]),
    ("Metal & cloth", &[Kind::IronBars, Kind::IronGate, Kind::LampPost, Kind::Banner, Kind::Rug]),
    ("Light & fire", &[Kind::Torch, Kind::Lantern, Kind::Hearth, Kind::Chimney, Kind::Furnace]),
];

/// Which tab `kind` is in
pub fn tab_of(kind: Kind) -> usize {
    TABS.iter().position(|(_, kinds)| kinds.contains(&kind)).unwrap_or(0)
}

/// Where a voxel is in the piece it's of: `x`, `y` and `z` in its block, `g` in the whole
/// landscape, `r` a hash of that; `a` across the piece from its first block's side, `h` up it from
/// its foot, and `t` through it from its back to its front, the way it faces; and for a gate,
/// whether it's open
pub struct At {
    pub x: i64,
    pub y: i64,
    pub z: i64,
    pub g: [i64; 3],
    pub r: u32,
    pub a: i64,
    pub h: i64,
    pub t: i64,
    pub open: bool,
}

/// The voxel of a piece of `kind` at `p`, AIR where there's none: for the pieces this file adds
pub fn voxel(kind: Kind, p: &At) -> u8 {
    let At { x, y, z, g, r, a, h, t, .. } = *p;
    match kind {
        Kind::BirchWall => planks(g, r, PLANK_BIRCH, BIRCH_BARK, false),
        Kind::PineWall => planks(g, r, PLANK_PINE, BARK_DARK, true),
        Kind::DarkWall => {
            // Heavy beams at its corners, planks between
            if (x == 0 || x == 7) && (y == 0 || y == 7) { BARK_DARK } else { planks(g, r, PLANK_DARK, BARK_DARK, false) }
        }
        Kind::PalmWall => {
            // Upright stems of palm, ringed every so often
            let stem = if x == 0 || x == 7 { y } else { x };
            if stem % 3 == 0 || (g[2] + hash3(stem as u32, g[0] as u32, g[1] as u32) as i64 % 5).rem_euclid(7) == 0 { DEAD_WOOD } else { PLANK_PALM }
        }
        Kind::Fence => {
            // A post in the middle and two rails across it, capped
            let post = (3..=4).contains(&a) && (3..=4).contains(&t);
            let rail = (3..=4).contains(&t) && (z == 2 || z == 3 || z == 5);
            if post && z == 7 {
                BARK_DARK
            } else if post || rail {
                if r % 6 == 0 { DEAD_WOOD } else { WOOD }
            } else {
                AIR
            }
        }
        Kind::ThatchRoof => roof(t, z, |row| if row % 3 == 0 { DRY_GRASS } else if r % 7 == 0 { REED } else { THATCH }, WOOD),
        Kind::SlateRoof => roof(t, z, |row| if row % 2 == 0 && (a + row) % 4 == 0 { MORTAR } else if r % 9 == 0 { STONE } else { SLATE }, BARK_DARK),
        Kind::CutStoneWall => ashlar(g, r, CUT_STONE, LIMESTONE, MORTAR),
        Kind::HalfWall => {
            // Half a block thick, at its back
            if t < 4 { ashlar(g, r, CUT_STONE, STONE, MORTAR) } else { AIR }
        }
        Kind::Slab => {
            if z >= 4 {
                AIR
            } else if (z == 3 && (x == 0 || y == 0)) || (z < 3 && r % 11 == 0) {
                MORTAR
            } else {
                CUT_STONE
            }
        }
        Kind::CobbleFloor => {
            // The top half of the block, cobbles a few voxels across set in grit, some mossy
            if z < 4 {
                return AIR;
            }
            let (cx, cy) = (g[0].div_euclid(3), g[1].div_euclid(3));
            let jx = (hash3(cx as u32, cy as u32, 0xc0b) % 3) as i64;
            let gap = (g[0] - jx).rem_euclid(3) == 0 && (g[1] + jx).rem_euclid(3) == 0;
            if gap || (z == 7 && (g[0] + g[1]).rem_euclid(3) == 0 && r % 2 == 0) {
                if r % 5 == 0 { MOSS } else { GRAVEL }
            } else {
                [STONE, ROCK, STONE, LIMESTONE, CUT_STONE][(hash3(cx as u32, cy as u32, 0x51) % 5) as usize]
            }
        }
        Kind::Arch => {
            // Two blocks across and two high: a round arch through it, of wedge stones round it and
            // a keystone at its top
            let (u, v) = (a as f64 - 7.5, h as f64);
            let open = (u.abs() < 5.5 && v < 8.0) || (u * u + (v - 8.0).powi(2) < 5.5 * 5.5);
            if open {
                return AIR;
            }
            let ring = (u * u + (v - 8.0).powi(2)).sqrt();
            if ring < 8.0 && v >= 7.0 {
                // The wedge stones, joints running out from the middle
                let angle = (v - 8.0).atan2(u);
                let wedge = (angle / 0.3).floor() as i64;
                let joint = ((angle / 0.3) - wedge as f64) < 0.12;
                if u.abs() < 1.2 && v > 12.0 {
                    MARBLE
                } else if joint {
                    MORTAR
                } else {
                    CUT_STONE
                }
            } else {
                ashlar(g, r, CUT_STONE, STONE, MORTAR)
            }
        }
        Kind::SandstoneWall => ashlar(g, r, SANDSTONE, SANDSTONE_DARK, SANDSTONE_DARK),
        Kind::MarbleWall => {
            // Big smooth panels with grey veins wandering through them
            let vein = ((g[0] + g[1]) as f64 * 0.7 + (g[2] as f64 * 0.45).sin() * 3.0).rem_euclid(11.0) < 0.6;
            if g[2].rem_euclid(16) == 0 { CUT_STONE } else if vein { LIMESTONE } else { MARBLE }
        }
        Kind::MarblePillar => {
            // A round fluted shaft on a square base, under a capital, two meters tall
            let (dx, dy) = (x as f64 - 3.5, y as f64 - 3.5);
            let d = (dx * dx + dy * dy).sqrt();
            let flute = (dy.atan2(dx) * 4.0 / std::f64::consts::PI).rem_euclid(1.0);
            match h {
                0..=1 => MARBLE,
                2..=3 if d < 4.3 => MARBLE,
                28..=29 if d < 4.3 => MARBLE,
                30..=31 => if (x == 0 || x == 7) && (y == 0 || y == 7) && h == 30 { AIR } else { MARBLE },
                _ if d < 2.9 => if d > 2.2 && flute < 0.25 { LIMESTONE } else { MARBLE },
                _ => AIR,
            }
        }
        Kind::BasaltWall => {
            // Columns of basalt, upright, a few voxels across, their tops at odd heights
            let (cx, cy) = (g[0].div_euclid(3), g[1].div_euclid(3));
            let col = hash3(cx as u32, cy as u32, 0xba5a);
            if (g[0].rem_euclid(3) == 0 || g[1].rem_euclid(3) == 0) && r % 3 != 0 { ROCK } else if (g[2] + (col % 8) as i64).rem_euclid(12) == 0 { GRAVEL } else { BASALT }
        }
        Kind::ObsidianWall => {
            if g[2].rem_euclid(8) == 0 || (g[0] + g[1]).rem_euclid(8) == 0 { BASALT } else { OBSIDIAN }
        }
        Kind::IceWall => {
            if g[2].rem_euclid(4) == 0 && r % 3 != 0 { SNOW } else { ICE }
        }
        Kind::GlassWindow => {
            // A pane in a wooden frame, two voxels thick in the middle of the block, crossed by
            // glazing bars
            if !(3..=4).contains(&t) && !((2..=5).contains(&t) && (a == 0 || a == 7 || z == 0 || z == 7)) {
                return AIR;
            }
            if a == 0 || a == 7 || z == 0 || z == 7 {
                PLANK_DARK
            } else if a == 4 || z == 4 {
                BARK_DARK
            } else {
                GLASS
            }
        }
        Kind::GlassPane => {
            if (3..=4).contains(&t) { GLASS } else { AIR }
        }
        Kind::IronBars => {
            let bar = a % 3 == 1 && (3..=4).contains(&t);
            let band = (z == 0 || z == 7) && (3..=4).contains(&t);
            if bar || band { DARK_STEEL } else { AIR }
        }
        Kind::IronGate => gate_voxel(p),
        Kind::LampPost => {
            // An iron post a meter and a half up on a foot, a glass lamp on top, a flame in it
            let (cx, cy) = ((x - 3).abs().min((x - 4).abs()), (y - 3).abs().min((y - 4).abs()));
            let post = cx == 0 && cy == 0;
            let inner = (2..=5).contains(&x) && (2..=5).contains(&y);
            match h {
                0..=1 if (1..=6).contains(&x) && (1..=6).contains(&y) => DARK_STEEL,
                0..=23 if post => DARK_STEEL,
                24 | 31 if inner => DARK_STEEL,
                25..=30 if inner => {
                    let corner = (x == 2 || x == 5) && (y == 2 || y == 5);
                    if corner {
                        DARK_STEEL
                    } else if post && (26..=29).contains(&h) {
                        FLAME
                    } else if x == 2 || x == 5 || y == 2 || y == 5 {
                        GLASS
                    } else {
                        AIR
                    }
                }
                _ => AIR,
            }
        }
        Kind::Banner => {
            // Hung from a pole against what's behind it: a blue field bordered in red, a gold
            // diamond on it, its foot cut in two tails
            if h == 15 && t <= 1 {
                return if a == 0 || a == 7 { GOLD } else { BARK_DARK };
            }
            if t != 0 || h > 14 {
                return AIR;
            }
            if h < 3 && (a - 3).abs().min((a - 4).abs()) < 3 - h {
                return AIR;
            }
            if a == 0 || a == 7 || h == 14 {
                CLOTH
            } else if (a as f64 - 3.5).abs() + (h as f64 - 8.0).abs() < 2.6 {
                GOLD
            } else {
                CLOTH_BLUE
            }
        }
        Kind::Rug => {
            if z != 0 {
                return AIR;
            }
            let edge = x == 0 || x == 7 || y == 0 || y == 7;
            let (dx, dy) = ((g[0].rem_euclid(16) as f64 - 7.5).abs(), (g[1].rem_euclid(16) as f64 - 7.5).abs());
            if edge && (g[0] + g[1]) % 2 == 0 {
                LINEN
            } else if edge {
                CLOTH
            } else if (dx + dy - 5.0).abs() < 0.8 {
                LINEN
            } else if dx + dy < 3.0 {
                GOLD
            } else {
                CLOTH_BLUE
            }
        }
        Kind::Furnace => {
            // A stone kiln two blocks high, its mouth on its front glowing, a flue out of its top
            let edge = x.min(7 - x).min(y.min(7 - y));
            if h >= 14 && edge < 1 + (h - 14) {
                return AIR;
            }
            let mouth = (2..=5).contains(&a) && (2..=6).contains(&h) && t >= 3;
            let fire = (2..=5).contains(&a) && (2..=4).contains(&h) && t == 3;
            let flue = (3..=4).contains(&x) && (3..=4).contains(&y) && h >= 9;
            if fire {
                if r % 3 == 0 { FLAME } else { GLOW_FIRE }
            } else if mouth || flue {
                AIR
            } else if (h == 7 || h == 1) && t == 7 {
                DARK_STEEL
            } else if (h + (a / 4)) % 4 == 0 {
                MORTAR
            } else if r % 5 == 0 {
                CLAY_POT
            } else {
                BRICK_STONE
            }
        }
        _ => AIR,
    }
}

/// Planks of `wood` a quarter meter wide with seams of `seam` between them, laid flat or standing
fn planks(g: [i64; 3], r: u32, wood: u8, seam: u8, upright: bool) -> u8 {
    let seam_here = if upright { g[0].rem_euclid(4) == 0 || g[1].rem_euclid(4) == 0 } else { g[2].rem_euclid(4) == 0 };
    if seam_here && r % 5 != 0 {
        seam
    } else if r % 13 == 0 {
        DEAD_WOOD
    } else {
        wood
    }
}

/// Dressed stone in courses a quarter meter high and half a meter long, joints of `joint` between
/// them staggered, now and then a stone of `other`
fn ashlar(g: [i64; 3], r: u32, stone: u8, other: u8, joint: u8) -> u8 {
    let course = g[2].div_euclid(4);
    let shift = if course % 2 == 0 { 0 } else { 4 };
    if g[2].rem_euclid(4) == 0 || (g[0] + shift).rem_euclid(8) == 0 || (g[1] + shift).rem_euclid(8) == 0 {
        return if r % 9 == 0 { stone } else { joint };
    }
    let block = hash3((g[0] + shift).div_euclid(8) as u32, (g[1] + shift).div_euclid(8) as u32, course as u32 ^ 0xa51a);
    if block % 5 == 0 { other } else { stone }
}

/// A roof's slope rising a voxel for each it goes on, `t` from its foot to its top, three voxels
/// thick, each row of it as `row` says, battens of `under` under its eaves
fn roof(t: i64, z: i64, row: impl Fn(i64) -> u8, under: u8) -> u8 {
    let top = t + 1;
    if z > top || z < top - 3 {
        return AIR;
    }
    if z == top - 3 {
        return under;
    }
    row(t + z)
}

/// A voxel of an iron gate: bars up and down, rails across, its frame; shut, it stands in the
/// middle of its blocks' thickness, open, swung back against its hinge side
fn gate_voxel(p: &At) -> u8 {
    let (a, h, t) = (p.a, p.h, p.t);
    if p.open {
        return if a < 2 && (1..7).contains(&t) && h < 31 && (t % 2 == 1 || h % 7 == 0) { DARK_STEEL } else { AIR };
    }
    if !(3..=4).contains(&t) || h >= 31 {
        return AIR;
    }
    let frame = a == 0 || a == 15 || h == 0 || h == 30;
    let rail = h == 10 || h == 20;
    let bar = a % 3 == 1;
    let finial = h >= 28 && a % 3 == 1;
    if frame || rail || bar || finial {
        if (a == 13) && (h == 15 || h == 16) { GOLD } else { DARK_STEEL }
    } else {
        AIR
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every piece is in a tab, once, and every tab says what it is
    #[test]
    fn every_piece_is_in_one_tab() {
        for kind in Kind::ALL {
            assert_eq!(TABS.iter().map(|(_, kinds)| kinds.iter().filter(|&&k| k == kind).count()).sum::<usize>(), 1, "{kind:?}");
        }
        let listed: usize = TABS.iter().map(|(_, kinds)| kinds.len()).sum();
        assert_eq!(listed, Kind::ALL.len());
        // In the order they're picked from
        let flat: Vec<Kind> = TABS.iter().flat_map(|(_, kinds)| kinds.iter().copied()).collect();
        assert_eq!(flat, Kind::ALL.to_vec());
        assert!(TABS.iter().all(|(_, kinds)| kinds.len() <= 11), "each picked by a number key");
    }
}
