//! Jewels, set into a weapon's sockets to lift its item level: each a jewel of one kind of weapon,
//! melee, ranged or magic, see CATEGORY_OF, and of one rarity, and fitting only weapons of that
//! kind and rarity. Each one set in lifts the weapon a level, never past levels::ITEM_CAP, so it
//! hits harder by the same curve as its level, its special attack and power on right click too.
//! A weapon has as many sockets as its rarity gives it, see SOCKETS, and a jewel once set can't be
//! taken out again.
//!
//! Their colours: melee's the reds, ranged the greens, magic the blues, each paler or duller the
//! commoner it is, pure and strong at rare, deep at epic, shot through with gold at legendary, and
//! gold and white at mythic. They glow, which the weapons' own materials never do.
//!
//! Each kind's model is in assets\models\jewels, a MagicaVoxel file of its own, as the code makes
//! it till it's edited, colours and all: its icon is drawn from it, see icon(). They're shown only
//! in the inventory, the quick bar and the weapon's card, not on the weapon in the world, so they
//! cost nothing in the frame; and as themselves when one's dropped on the ground, see lying_model().

use std::path::Path;

use egui_winit_vulkano::egui;

use crate::vox::{self, Grid};
use crate::weapons::Kind;

/// The rarities, as assets\weapons.json has them, common first
pub const TIERS: usize = 6;
const TIER_IDS: [&str; TIERS] = ["common", "uncommon", "rare", "epic", "legendary", "mythic"];
const TIER_NAMES: [&str; TIERS] = ["Common", "Uncommon", "Rare", "Epic", "Legendary", "Mythic"];

/// Which kind of weapon a jewel's for
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Melee,
    Ranged,
    Magic,
}

impl Category {
    pub const ALL: [Category; 3] = [Category::Melee, Category::Ranged, Category::Magic];

    fn id(self) -> &'static str {
        match self {
            Category::Melee => "melee",
            Category::Ranged => "ranged",
            Category::Magic => "magic",
        }
    }

    /// What its jewels are called
    pub fn stone(self) -> &'static str {
        match self {
            Category::Melee => "Bloodstone",
            Category::Ranged => "Verdant Eye",
            Category::Magic => "Skyglass",
        }
    }

    pub fn weapons(self) -> &'static str {
        match self {
            Category::Melee => "melee weapons",
            Category::Ranged => "bows, crossbows and guns",
            Category::Magic => "staves, wands and the like",
        }
    }

    /// Its colour, pure and strong, as a rare jewel's
    fn hue(self) -> [f32; 3] {
        match self {
            Category::Melee => [228.0, 32.0, 40.0],
            Category::Ranged => [40.0, 200.0, 64.0],
            Category::Magic => [44.0, 96.0, 240.0],
        }
    }
}

/// Which category of jewel fits each kind of weapon, the one table to add a line to for a new kind
pub const CATEGORY_OF: [(Kind, Category); 3] = [(Kind::Melee, Category::Melee), (Kind::Ranged, Category::Ranged), (Kind::Magic, Category::Magic)];

pub fn category_of(kind: Kind) -> Option<Category> {
    CATEGORY_OF.iter().find(|(k, _)| *k == kind).map(|(_, c)| *c)
}

/// How many kinds of jewel there are: one of each rarity for each category
pub const KINDS: usize = 3 * TIERS;

/// Jewel kind `j`, of Item::Jewel, as its category and rarity
pub fn parts(j: u8) -> (Category, usize) {
    let j = j as usize % KINDS;
    (Category::ALL[j / TIERS], j % TIERS)
}

pub fn jewel(category: Category, tier: usize) -> u8 {
    (Category::ALL.iter().position(|&c| c == category).unwrap_or(0) * TIERS + tier.min(TIERS - 1)) as u8
}

/// What it's called in saves and its model's file: like `melee_rare`
pub fn id(j: u8) -> String {
    let (c, t) = parts(j);
    format!("{}_{}", c.id(), TIER_IDS[t])
}

pub fn by_id(id: &str) -> Option<u8> {
    (0..KINDS as u8).find(|&j| self::id(j) == id)
}

pub fn name(j: u8) -> String {
    let (c, t) = parts(j);
    format!("{} {}", TIER_NAMES[t], c.stone())
}

/// How many sockets a weapon of rarity `grade` has
pub const SOCKETS: [u32; TIERS] = [1, 1, 2, 2, 3, 3];

pub fn sockets(grade: usize) -> u32 {
    SOCKETS[grade.min(TIERS - 1)]
}

/// What a merchant asks for a jewel of rarity `tier`: they sell the common and uncommon only
pub fn price(tier: usize) -> u32 {
    [45, 110, 260, 600, 1400, 3200][tier.min(TIERS - 1)]
}

/// Whether jewel `j` can be set into a weapon of `kind` and rarity `grade`, a tool or not, with
/// `jewels` set in it already and of item level `level`: why not if it can't
pub fn fits(j: u8, kind: Kind, grade: usize, tool: bool, jewels: u32, level: u32) -> Result<(), String> {
    let (c, t) = parts(j);
    if tool {
        return Err("Tools take no jewels.".into());
    }
    if category_of(kind) != Some(c) {
        return Err(format!("A {} fits only {}.", c.stone(), c.weapons()));
    }
    if grade != t {
        return Err(format!("A {} fits only {} weapons.", name(j), TIER_NAMES[t].to_lowercase()));
    }
    if jewels >= sockets(grade) {
        return Err("Every socket's full.".into());
    }
    if level >= crate::levels::ITEM_CAP {
        return Err(format!("It's of item level {} already, as high as any can be.", crate::levels::ITEM_CAP));
    }
    Ok(())
}

/// Its colours: body, a lighter facet, and for legendary and mythic gold through it, and white
/// sparks for mythic
pub struct Colors {
    pub body: [u8; 3],
    pub light: [u8; 3],
    pub vein: Option<[u8; 3]>,
    pub spark: Option<[u8; 3]>,
}

const GOLD: [f32; 3] = [255.0, 196.0, 52.0];

pub fn colors(j: u8) -> Colors {
    let (c, t) = parts(j);
    let hue = c.hue();
    let mix = |a: [f32; 3], b: [f32; 3], k: f32| [0, 1, 2].map(|i| (a[i] + (b[i] - a[i]) * k).clamp(0.0, 255.0) as u8);
    let grey = |a: [f32; 3]| {
        let g = (a[0] + a[1] + a[2]) / 3.0;
        [g, g, g]
    };
    let body: [u8; 3] = match t {
        // Dull: half grey, darker
        0 => mix(mix(hue, grey(hue), 0.55).map(f32::from), [40.0; 3], 0.3),
        // Pale: toward white
        1 => mix(hue, [255.0; 3], 0.45),
        // Pure and strong
        2 => hue.map(|v| v as u8),
        // Deep and rich
        3 => mix(hue, [0.0; 3], 0.3),
        // Gold-shot: toward gold
        4 => mix(hue, GOLD, 0.35),
        _ => mix(hue, GOLD, 0.2),
    };
    let light = mix(body.map(f32::from), [255.0; 3], 0.45);
    Colors { body, light, vein: (t >= 4).then(|| GOLD.map(|v| v as u8)), spark: (t >= 5).then_some([255, 255, 240]) }
}

/// Its colour as egui has it, for its slot and its card
pub fn tint(j: u8) -> egui::Color32 {
    let [r, g, b] = colors(j).body;
    egui::Color32::from_rgb(r, g, b)
}

// Its model

/// The palette its model's written with: 1 its body, 2 a lighter facet, 3 gold, 4 white sparks
fn palette(j: u8) -> [[u8; 4]; 256] {
    let c = colors(j);
    let mut p = [[0u8, 0, 0, 255]; 256];
    let rgba = |v: [u8; 3]| [v[0], v[1], v[2], 255];
    p[1] = rgba(c.body);
    p[2] = rgba(c.light);
    p[3] = rgba(c.vein.unwrap_or(c.light));
    p[4] = rgba(c.spark.unwrap_or(c.light));
    p
}

/// Its model as the code makes it: a cut gem standing on its point, 7 by 7 by 9 voxels, its upper
/// facets lighter, gold through it for legendary and mythic, a few white sparks for mythic
pub fn model(j: u8) -> Grid {
    let (_, t) = parts(j);
    let size = [7u32, 7, 9];
    let mut voxels = vec![0u8; (size[0] * size[1] * size[2]) as usize];
    for z in 0..size[2] {
        // Widest just over the middle, a point below, a flat table on top
        let r = match z {
            0 => 0.6,
            1 => 1.3,
            2 => 2.0,
            3 => 2.7,
            4 | 5 => 3.4,
            6 => 2.9,
            7 => 2.3,
            _ => 1.6,
        };
        for y in 0..size[1] {
            for x in 0..size[0] {
                let (dx, dy) = (x as f32 - 3.0, y as f32 - 3.0);
                // Eight-sided, as a cut gem
                let d = dx.abs().max(dy.abs()).max((dx.abs() + dy.abs()) * 0.75);
                if d > r {
                    continue;
                }
                let upper = z >= 6 || (z >= 4 && dx + dy < -1.0);
                let mut c = if upper { 2 } else { 1 };
                if t >= 4 && ((x + z) % 4 == 0 || (y + 2 * z) % 7 == 0) && d > r - 1.2 {
                    c = 3;
                }
                if t >= 5 && (x * 3 + y * 5 + z * 7) % 13 == 0 && d > r - 1.0 {
                    c = 4;
                }
                voxels[(x + (y + z * size[1]) * size[0]) as usize] = c;
            }
        }
    }
    Grid { size, voxels, origin: [-3, -3, 0] }
}

/// Where kind `j`'s file is under the models' folder
fn file(root: &Path, j: u8) -> std::path::PathBuf {
    root.join("jewels").join(format!("{}.vox", id(j)))
}

/// Writes every kind's model to `root`\jewels, with its .json beside it, as the rest are, so a
/// file left as written is known for one, see assets::unchanged()
pub fn export(root: &Path, lines: &mut Vec<String>) -> Result<(), String> {
    for j in 0..KINDS as u8 {
        let grid = model(j);
        let path = file(root, j);
        let what = format!("{}, a jewel for {}: colour 1 its body, 2 its lighter facets, 3 gold, 4 white sparks", name(j), parts(j).0.weapons());
        crate::assets::write_jewel(&path, &grid, &palette(j), &what)?;
        lines.push(format!("jewels/{}.vox: {}", id(j), name(j)));
    }
    Ok(())
}

/// Kind `j`'s model and palette: its file's under `root` if it's been edited, else the code's
fn shape(root: &Path, j: u8) -> (Grid, [[u8; 4]; 256]) {
    let path = file(root, j);
    if path.exists()
        && !crate::assets::unchanged(&path)
        && let Ok(bytes) = std::fs::read(&path)
        && let Ok(f) = vox::read(&bytes)
    {
        return (f.grid, f.palette);
    }
    (model(j), palette(j))
}

/// Voxels a meter of a jewel lying on the ground: a cut gem about a hand's breadth across, a little
/// over life size so it reads among the grass
pub const LYING_RES: f64 = 80.0;

/// Kind `j`'s model to lie on the ground in the world, see Game::instances(): the same voxels as its
/// icon's, its file's if it's been edited, in the world's own materials, as those are all the
/// shader knows. A common jewel's dull, the rest glow in their colour, melee's red, ranged's green
/// and magic's blue, legendary and mythic shot with gold, mythic with white sparks. With where the
/// middle of its bottom is, in meters from its point, as animals::voxelize() gives it.
pub fn lying_model(root: &Path, j: u8) -> (crate::animals::Model, nalgebra::Vector3<f64>) {
    use crate::worldgen::material as m;
    let (c, t) = parts(j);
    let (grid, _) = shape(root, j);
    let body = match (c, t) {
        (Category::Melee, 0) => m::TEMPLE_RED,
        (Category::Ranged, 0) => m::MOSS,
        (Category::Magic, 0) => m::CLOTH_BLUE,
        (Category::Melee, _) => m::GLOW_BLOOD,
        (Category::Ranged, _) => m::GLOW_POISON,
        (Category::Magic, _) => m::GLOW_FROST,
    };
    let voxels = grid.voxels.iter().map(|&v| match v {
        0 => m::AIR,
        3 => m::GOLD,
        4 => m::GLOW_HOLY,
        _ => body,
    });
    let model = crate::animals::Model { size: grid.size, voxels: voxels.collect() };
    let [sx, sy, _] = grid.size.map(|v| v as f64);
    let [ox, oy, oz] = grid.origin.map(|v| v as f64);
    let anchor = nalgebra::Vector3::new(ox + sx * 0.5, oy + sy * 0.5, oz) / LYING_RES;
    (model, anchor)
}

/// Kind `j`'s icon, `size` pixels a side: its model seen from above at an angle, every voxel a
/// little cube lit from the top left, in a soft glow of its colour
pub fn icon(root: &Path, j: u8, size: usize) -> egui::ColorImage {
    let (grid, pal) = shape(root, j);
    let mut px = vec![[0.0f32; 4]; size * size];
    let [sx, sy, sz] = grid.size.map(|v| v as f32);
    // Seen turned 45 degrees and from above: across is x - y, down is (x + y) / 2 - z
    let span = (sx + sy).max(sz * 1.6 + (sx + sy) * 0.5);
    let cell = size as f32 * 0.78 / span;
    let middle = [size as f32 * 0.5, size as f32 * 0.5];
    let project = |x: f32, y: f32, z: f32| [middle[0] + (x - y) * cell * 0.92, middle[1] + ((x + y) * 0.5 - z * 1.15 - (sx + sy) * 0.25 + sz * 0.55) * cell];
    // The glow behind it
    let c = colors(j);
    let glow = c.body.map(|v| v as f32 / 255.0);
    for y in 0..size {
        for x in 0..size {
            let d = ((x as f32 - middle[0]).hypot(y as f32 - middle[1])) / (size as f32 * 0.5);
            let a = ((1.0 - d) * 0.55).clamp(0.0, 1.0).powf(1.6);
            px[y * size + x] = [glow[0], glow[1], glow[2], a];
        }
    }
    // Far voxels first
    let mut order = Vec::new();
    for z in 0..grid.size[2] {
        for y in 0..grid.size[1] {
            for x in 0..grid.size[0] {
                let v = grid.get([x, y, z]);
                if v != 0 {
                    order.push((x as f32 + y as f32 + z as f32, x, y, z, v));
                }
            }
        }
    }
    order.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, x, y, z, v) in order {
        let [r, g, b, _] = pal[v as usize];
        let base = [r as f32, g as f32, b as f32].map(|c| c / 255.0);
        let at = project(x as f32, y as f32, z as f32);
        let half = (cell * 0.95).max(1.0);
        let (x0, x1) = ((at[0] - half).floor() as i64, (at[0] + half).ceil() as i64);
        let (y0, y1) = ((at[1] - half * 1.1).floor() as i64, (at[1] + half * 0.9).ceil() as i64);
        for py in y0.max(0)..y1.min(size as i64) {
            for qx in x0.max(0)..x1.min(size as i64) {
                // Its top lighter, its left side as it is, its right darker
                let top = (py as f32) < at[1] - half * 0.35;
                let right = (qx as f32) > at[0];
                let shade = if top { 1.25 } else if right { 0.72 } else { 0.95 };
                px[py as usize * size + qx as usize] = [base[0] * shade, base[1] * shade, base[2] * shade, 1.0].map(|c| c.min(1.0));
            }
        }
    }
    let bytes: Vec<u8> = px.iter().flat_map(|p| [p[0], p[1], p[2], p[3]].map(|c| (c * 255.0).round() as u8)).collect();
    egui::ColorImage::from_rgba_unmultiplied([size, size], &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jewels_fit_only_their_own_kind_and_rarity() {
        for j in 0..KINDS as u8 {
            assert_eq!(by_id(&id(j)), Some(j));
            let (c, t) = parts(j);
            assert_eq!(jewel(c, t), j);
            let kind = CATEGORY_OF.iter().find(|(_, cat)| *cat == c).unwrap().0;
            assert!(fits(j, kind, t, false, 0, 10).is_ok());
            assert!(fits(j, kind, t, true, 0, 10).is_err(), "tools");
            assert!(fits(j, kind, (t + 1) % TIERS, false, 0, 10).is_err(), "another rarity");
            for (other, _) in CATEGORY_OF.iter().filter(|(_, cat)| *cat != c) {
                assert!(fits(j, *other, t, false, 0, 10).is_err(), "another kind");
            }
            assert!(fits(j, kind, t, false, sockets(t), 10).is_err(), "sockets full");
            assert!(fits(j, kind, t, false, 0, crate::levels::ITEM_CAP).is_err(), "at the cap");
        }
        // Every kind of weapon has its category
        for kind in [Kind::Melee, Kind::Ranged, Kind::Magic] {
            assert!(category_of(kind).is_some());
        }
        assert!(SOCKETS.windows(2).all(|w| w[1] >= w[0]));
    }

    /// Each kind's colour its own, the reds, greens and blues apart, and the rarer kinds a step
    /// from the commoner; legendary gold-shot, as a red one's yellow-red
    #[test]
    fn each_kind_looks_its_own() {
        let all: Vec<[u8; 3]> = (0..KINDS as u8).map(|j| colors(j).body).collect();
        for (a, x) in all.iter().enumerate() {
            for (b, y) in all.iter().enumerate().skip(a + 1) {
                let d: i32 = (0..3).map(|i| (x[i] as i32 - y[i] as i32).abs()).sum();
                assert!(d > 20, "{} and {} look the same", name(a as u8), name(b as u8));
            }
        }
        let rare = colors(jewel(Category::Melee, 2)).body;
        assert!(rare[0] > 200 && rare[1] < 60 && rare[2] < 60, "a rare bloodstone's red: {rare:?}");
        let legendary = colors(jewel(Category::Melee, 4)).body;
        assert!(legendary[1] > rare[1] + 40 && colors(jewel(Category::Melee, 4)).vein.is_some(), "a legendary bloodstone's yellow-red: {legendary:?}");
        let icon = icon(Path::new("no such folder"), jewel(Category::Magic, 5), 48);
        assert!(icon.pixels.iter().filter(|p| p.a() == 255).count() > 200);
    }

    /// Writes every kind's model into assets\models\jewels, as `--export-models` does with the
    /// rest: `cargo test --release export_jewels -- --ignored`
    #[test]
    #[ignore]
    fn export_jewels() {
        let mut lines = Vec::new();
        export(&crate::assets::root(), &mut lines).unwrap();
        println!("{}", lines.join("
"));
    }

    /// The shipped files are the code's, so they're passed over, see assets::unchanged()
    #[test]
    fn shipped_jewels_are_the_codes() {
        for j in 0..KINDS as u8 {
            let path = file(&crate::assets::root(), j);
            assert!(path.exists() && crate::assets::unchanged(&path), "{}", path.display());
            let (grid, _) = shape(&crate::assets::root(), j);
            assert_eq!(grid, model(j));
        }
    }
}
