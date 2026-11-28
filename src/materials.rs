//! What's gathered to build with, see bases::Materials: wood of the kinds of tree there are, stone
//! of the kinds of rock, sand, reeds and flax, ore out of the veins in the rock deep down, and what
//! a furnace makes of ore and sand, copper, iron and glass. Each comes from somewhere in the world,
//! some only in some lands or realms, and builds pieces of its own, see bases::Kind.
//!
//! - Wood from any tree; birch, pine, dark oak and palm planks from those trees, over the wood.
//! - Stone from grey rock; sandstone in the deserts; slate in bands in the rock; marble high in the
//!   mountains and all through the Rimelands' rock; basalt and obsidian in the Cinder Crown and
//!   where dragons have burnt the land; ice in the Rimelands.
//! - Sand on beaches and in deserts, dug with a shovel, smelted to glass.
//! - Reeds and cattails by the water, and dry grass, cut for thatch; blue flax, for cloth.
//! - Copper and iron ore in veins deep in the rock, seen in caves and cliffs, smelted at a furnace.

use egui_winit_vulkano::egui::Color32;

use crate::bases::Materials;
use crate::worldgen::material::*;

/// Each of what's gathered and carried
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Res {
    Wood,
    Stone,
    Clay,
    Birch,
    Pine,
    Darkwood,
    Palm,
    Sand,
    Sandstone,
    Marble,
    Slate,
    Basalt,
    Obsidian,
    Ice,
    Reed,
    Cloth,
    CopperOre,
    IronOre,
    Copper,
    Iron,
    Glass,
}

impl Res {
    /// In the order they're listed in
    pub const ALL: [Res; 21] = [
        Res::Wood,
        Res::Stone,
        Res::Clay,
        Res::Birch,
        Res::Pine,
        Res::Darkwood,
        Res::Palm,
        Res::Sand,
        Res::Sandstone,
        Res::Marble,
        Res::Slate,
        Res::Basalt,
        Res::Obsidian,
        Res::Ice,
        Res::Reed,
        Res::Cloth,
        Res::CopperOre,
        Res::IronOre,
        Res::Copper,
        Res::Iron,
        Res::Glass,
    ];

    pub fn name(self) -> &'static str {
        self.info().0
    }

    /// Its colour in the HUD
    pub fn color(self) -> Color32 {
        let [r, g, b] = self.info().1;
        Color32::from_rgb(r, g, b)
    }

    /// What it builds
    pub fn uses(self) -> &'static str {
        self.info().2
    }

    /// Where it's got
    pub fn from(self) -> &'static str {
        self.info().3
    }

    fn info(self) -> (&'static str, [u8; 3], &'static str, &'static str) {
        match self {
            Res::Wood => ("Wood", [170, 115, 60], "walls · floors · doors · fire", "hold G at any tree"),
            Res::Stone => ("Stone", [160, 160, 155], "strong walls · fireplaces", "hold G at grey rock, or dig down to it"),
            Res::Clay => ("Clay", [200, 115, 75], "brick walls · lanterns", "hold G at the reddish banks of rivers and lakes"),
            Res::Birch => ("Birch", [222, 200, 150], "pale plank walls", "fell a white birch"),
            Res::Pine => ("Pine", [190, 120, 70], "red plank walls", "fell a pine"),
            Res::Darkwood => ("Dark oak", [110, 70, 42], "dark plank walls", "fell a dark-barked tree"),
            Res::Palm => ("Palm", [205, 165, 110], "palm plank walls", "fell a palm"),
            Res::Sand => ("Sand", [222, 196, 138], "glass, at a furnace", "dig a beach or the desert with a shovel"),
            Res::Sandstone => ("Sandstone", [224, 170, 112], "sandstone walls", "break desert rock"),
            Res::Marble => ("Marble", [236, 234, 228], "marble walls · pillars", "high mountain rock, or the Rimelands; a copper hammer or better"),
            Res::Slate => ("Slate", [90, 98, 112], "slate roofs", "the dark bands in rock"),
            Res::Basalt => ("Basalt", [80, 78, 82], "basalt walls", "the Cinder Crown and burnt lands; a copper pickaxe or better"),
            Res::Obsidian => ("Obsidian", [70, 52, 96], "obsidian walls · the best tools", "black glass by lava; an iron pickaxe or better"),
            Res::Ice => ("Ice", [184, 224, 250], "ice walls", "the frozen lakes of the Rimelands"),
            Res::Reed => ("Reed", [204, 170, 90], "thatched roofs", "cut reeds, cattails or dry grass, a sickle quicker"),
            Res::Cloth => ("Cloth", [60, 90, 200], "banners · rugs", "pick blue flax flowers"),
            Res::CopperOre => ("Copper ore", [92, 150, 125], "copper, at a furnace", "green-flecked veins deep in rock; any pickaxe"),
            Res::IronOre => ("Iron ore", [150, 90, 62], "iron, at a furnace", "rust-red veins deep in rock; a copper pickaxe or better"),
            Res::Copper => ("Copper", [214, 128, 70], "copper tools", "smelt copper ore at a furnace"),
            Res::Iron => ("Iron", [150, 156, 168], "iron tools · bars · gates · lamp posts", "smelt iron ore at a furnace"),
            Res::Glass => ("Glass", [170, 222, 226], "windows · panes · lamp posts", "smelt sand at a furnace"),
        }
    }

    /// What it's drawn with, chips flying off it and the like
    pub fn material(self) -> u8 {
        match self {
            Res::Wood => WOOD,
            Res::Stone => STONE,
            Res::Clay => CLAY_BANK,
            Res::Birch => PLANK_BIRCH,
            Res::Pine => PLANK_PINE,
            Res::Darkwood => PLANK_DARK,
            Res::Palm => PLANK_PALM,
            Res::Sand => SAND,
            Res::Sandstone => SANDSTONE,
            Res::Marble => MARBLE,
            Res::Slate => SLATE,
            Res::Basalt => BASALT,
            Res::Obsidian => OBSIDIAN,
            Res::Ice => ICE,
            Res::Reed => THATCH,
            Res::Cloth => CLOTH_BLUE,
            Res::CopperOre => COPPER_ORE,
            Res::IronOre => IRON_ORE,
            Res::Copper => BRONZE,
            Res::Iron => DARK_STEEL,
            Res::Glass => GLASS,
        }
    }

    /// What a furnace makes of it, and how many of it make one, if it's smelted
    pub fn smelts_to(self) -> Option<(Res, u32)> {
        match self {
            Res::CopperOre => Some((Res::Copper, 1)),
            Res::IronOre => Some((Res::Iron, 1)),
            Res::Sand => Some((Res::Glass, 2)),
            _ => None,
        }
    }
}

/// What's gathered from a voxel of `material` in a block of it, other than wood from a tree, see
/// gather.rs: None for what gives nothing, as earth and what's left alone
pub fn res_of(material: u8) -> Option<Res> {
    Some(match material {
        STONE | ROCK | GRAVEL | LIMESTONE | BRICK_STONE | MORTAR | TEMPLE_STONE | TEMPLE_DARK | MOSS | CUT_STONE => Res::Stone,
        SANDSTONE | SANDSTONE_DARK => Res::Sandstone,
        MARBLE => Res::Marble,
        SLATE => Res::Slate,
        BASALT => Res::Basalt,
        OBSIDIAN => Res::Obsidian,
        ICE => Res::Ice,
        IRON_ORE => Res::IronOre,
        COPPER_ORE => Res::CopperOre,
        CLAY | CLAY_BANK | CLAY_POT => Res::Clay,
        SAND => Res::Sand,
        REED | CATTAIL | DRY_GRASS | THATCH => Res::Reed,
        FLOWER_BLUE => Res::Cloth,
        WOOD | BARK_DARK | BIRCH_BARK | PALM_WOOD | DEAD_WOOD | MUSHROOM_STEM | CACTUS | PLANK_BIRCH | PLANK_DARK | PLANK_PINE | PLANK_PALM => Res::Wood,
        _ => return None,
    })
}

/// Whether `material` is a plant that's cut for what it gives, reeds and flax, which G stops at
/// though it can be walked through
pub fn is_crop(material: u8) -> bool {
    matches!(material, REED | CATTAIL | DRY_GRASS | FLOWER_BLUE)
}

/// The planks a tree of `bark`, crowned with `leaf`, gives over its wood, if it's of a kind that
/// does: birch, palm, dark oak, or pine by its needles
pub fn timber(bark: u8, leaf: Option<u8>) -> Option<Res> {
    match (bark, leaf) {
        (BIRCH_BARK, _) => Some(Res::Birch),
        (PALM_WOOD, _) => Some(Res::Palm),
        (_, Some(PINE)) => Some(Res::Pine),
        (BARK_DARK, _) => Some(Res::Darkwood),
        _ => None,
    }
}

/// What a tree gives that's `wood` pieces of wood big, of `bark` and crowned with `leaf`: its wood,
/// and half as many planks of its kind if it's of a kind that has them
pub fn tree_gives(wood: u32, bark: u8, leaf: Option<u8>) -> Materials {
    let mut m = Materials::new(wood, 0, 0);
    if let Some(kind) = timber(bark, leaf) {
        *m.get_mut(kind) += wood.div_ceil(2);
    }
    m
}

/// Ore and sand in `m` made into copper, iron and glass, as a tool that smelts as it gathers does:
/// what's left over, as an odd grain of sand, stays as it is
pub fn smelted(m: Materials) -> Materials {
    let mut out = m;
    for r in Res::ALL {
        if let Some((to, per)) = r.smelts_to() {
            let n = m.get(r);
            *out.get_mut(r) = n % per;
            *out.get_mut(to) += n / per;
        }
    }
    out
}

/// How much of what's smelted one log of wood burns through, at a furnace
pub const SMELT_PER_WOOD: u32 = 4;

/// What a furnace makes of what's carried, `have`: everything smeltable in it, as much as the wood
/// carried burns through, SMELT_PER_WOOD of what's made to a log. What's used, and what's made.
pub fn smelt(have: Materials) -> (Materials, Materials) {
    let fuel = have.wood;
    let (mut used, mut made) = (Materials::default(), Materials::default());
    let mut burnt = 0;
    for r in Res::ALL {
        let Some((to, per)) = r.smelts_to() else { continue };
        let mut n = have.get(r) / per;
        // As much as the fuel left burns
        let room = (fuel * SMELT_PER_WOOD).saturating_sub(burnt);
        n = n.min(room);
        if n == 0 {
            continue;
        }
        burnt += n;
        *used.get_mut(r) += n * per;
        *made.get_mut(to) += n;
    }
    used.wood = burnt.div_ceil(SMELT_PER_WOOD);
    (used, made)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every material that's gathered says what it's for and where it's got, and has a colour
    #[test]
    fn every_material_says_where_it_is_got() {
        for r in Res::ALL {
            assert!(!r.name().is_empty() && !r.uses().is_empty() && !r.from().is_empty(), "{r:?}");
            assert!(r.material() != AIR && r.material() <= LAST);
            assert_eq!(Res::ALL.iter().filter(|&&o| o == r).count(), 1);
        }
        assert_eq!(res_of(MARBLE), Some(Res::Marble));
        assert_eq!(res_of(OBSIDIAN), Some(Res::Obsidian));
        assert_eq!(res_of(IRON_ORE), Some(Res::IronOre));
        assert_eq!(res_of(REED), Some(Res::Reed));
        assert_eq!(res_of(GRASS), None);
        assert!(is_crop(CATTAIL) && !is_crop(LEAVES));
    }

    // Kinds of tree give their own planks over the wood
    #[test]
    fn trees_give_their_planks() {
        assert_eq!(tree_gives(9, BIRCH_BARK, Some(LEAVES)), Materials::new(9, 0, 0).with(Res::Birch, 5));
        assert_eq!(tree_gives(4, WOOD, Some(PINE)), Materials::new(4, 0, 0).with(Res::Pine, 2));
        assert_eq!(tree_gives(6, PALM_WOOD, Some(PALM_LEAF)).palm, 3);
        assert_eq!(tree_gives(6, WOOD, Some(LEAVES)), Materials::new(6, 0, 0));
        assert_eq!(tree_gives(6, DEAD_WOOD, None), Materials::new(6, 0, 0));
    }

    // A furnace makes copper and iron of ore and glass of two sand, burning a log for every four,
    // and no more than the wood carried burns
    #[test]
    fn furnaces_smelt_what_the_wood_burns() {
        let have = Materials::new(10, 0, 0).with(Res::CopperOre, 3).with(Res::IronOre, 2).with(Res::Sand, 7);
        let (used, made) = smelt(have);
        assert_eq!(made, Materials::default().with(Res::Copper, 3).with(Res::Iron, 2).with(Res::Glass, 3));
        assert_eq!(used, Materials::new(2, 0, 0).with(Res::CopperOre, 3).with(Res::IronOre, 2).with(Res::Sand, 6));
        let short = Materials::new(1, 0, 0).with(Res::IronOre, 9);
        let (used, made) = smelt(short);
        assert_eq!((used.wood, used.iron_ore, made.iron), (1, 4, 4));
        assert_eq!(smelt(Materials::new(0, 0, 0).with(Res::IronOre, 9)).1, Materials::default());
        // Smelted as it's gathered
        assert_eq!(smelted(Materials::new(0, 2, 0).with(Res::IronOre, 3).with(Res::Sand, 5)), Materials::new(0, 2, 0).with(Res::Iron, 3).with(Res::Glass, 2).with(Res::Sand, 1));
    }

    // A save from before the new materials loads with none of them, and they go in the save and
    // come back
    #[test]
    fn materials_save_and_old_saves_load() {
        let old: Materials = serde_json::from_str(r#"{"wood":5,"stone":2,"clay":1}"#).unwrap();
        assert_eq!(old, Materials::new(5, 2, 1));
        assert_eq!(old.marble + old.iron + old.glass, 0);
        let new = Materials::new(1, 2, 3).with(Res::Marble, 4).with(Res::Glass, 5).with(Res::IronOre, 6);
        let text = serde_json::to_string(&new).unwrap();
        assert!(!text.contains("birch"), "what there's none of is left out: {text}");
        assert_eq!(serde_json::from_str::<Materials>(&text).unwrap(), new);
        assert_eq!(new.describe(), "1 wood, 2 stone, 3 clay, 4 marble, 6 iron ore, 5 glass");
        let mut carried = new;
        assert!(carried.take(Materials::of(Res::Marble, 4)) && carried.marble == 0);
        assert!(!carried.take(Materials::of(Res::Marble, 1)));
    }
}
