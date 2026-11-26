//! Tools for gathering, see building.rs and gather.rs: an axe for wood, a pickaxe for stone and
//! ore, a shovel for clay, sand and earth, a sickle for reeds and flax, and a mason's hammer and
//! chisel for cut stone, sandstone, slate and marble. Each comes in five tiers, of stone, copper,
//! iron, steel and obsidian, each quicker than the one before. Bare hands gather most things; the
//! hardest need a tool of the right kind and a good enough tier, as iron ore a copper pickaxe and
//! obsidian an iron one, see needs(). G gathers with the best tool carried for what's looked at.
//!
//! Tools roll a rarity as weapons do, common to mythic in the same colours, when they're crafted,
//! found in chests, left by bosses or sold by merchants, and the rarer they are the more they do:
//! a little quicker for each step, and the rarer ones a chance of twice the yield, smelting what
//! they gather, felling a whole tree at a blow, or a wide swing gathering the blocks either side
//! too, see Perks. Their rarity's kept in their seed, so they go in the save as any weapon does.
//!
//! They're made in the Craft tab of the inventory (I) from what's gathered. A tool is a weapon of
//! a family of its own in assets\weapons.json, so it's held, swung, carried, sold and saved as
//! weapons are: in hand, a click swings it too, and where its blow lands on what it's for it
//! gathers it, see App::update_tools().

use egui_winit_vulkano::egui::{self, Color32, RichText};

use crate::bases::{Materials, Res};
use crate::gather::Yield;
use crate::weapons::{Library, Weapon};
use crate::worldgen::material::*;

/// The tools there are
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Hatchet,
    Pickaxe,
    Spade,
    Sickle,
    Hammer,
}

/// The tiers, each an index into TIERS: the first three were Crude, Sturdy and Fine, and their
/// parts keep those names so tools from before carry on as they were
pub const CRUDE: usize = 0;
pub const STURDY: usize = 1;
pub const FINE: usize = 2;
pub const STEEL_TIER: usize = 3;
pub const OBSIDIAN_TIER: usize = 4;
pub const TIERS: [&str; 5] = ["Stone", "Copper", "Iron", "Steel", "Obsidian"];
/// What each tier's parts are called in assets\weapons.json
const PART_TIERS: [&str; 5] = ["crude", "sturdy", "fine", "steel", "obsidian"];
/// How many times quicker than bare hands each tier gathers
pub const SPEED: [f64; 5] = [2.0, 3.0, 4.0, 5.0, 6.5];
/// How quickly bare hands gather, which is most of what there is to gather
pub const BARE_HANDS: f64 = 1.0;
/// The colour each tier's name is shown in
const TIER_COLORS: [[u8; 3]; 5] = [[190, 186, 178], [224, 140, 80], [170, 178, 192], [215, 228, 240], [170, 120, 235]];

impl Tool {
    pub const ALL: [Tool; 5] = [Tool::Hatchet, Tool::Pickaxe, Tool::Spade, Tool::Sickle, Tool::Hammer];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Hatchet => "Axe",
            Tool::Pickaxe => "Pickaxe",
            Tool::Spade => "Shovel",
            Tool::Sickle => "Sickle",
            Tool::Hammer => "Mason's Hammer",
        }
    }

    /// Its family's id in assets\weapons.json
    pub fn family(self) -> &'static str {
        match self {
            Tool::Hatchet => "hatchet",
            Tool::Pickaxe => "pickaxe",
            Tool::Spade => "spade",
            Tool::Sickle => "sickle",
            Tool::Hammer => "chisel",
        }
    }

    /// The id of the part its head is, at tier `tier`
    fn head(self, tier: usize) -> String {
        let part = if matches!(self, Tool::Spade | Tool::Sickle) { "blade" } else { "head" };
        format!("{}_{}_{part}", PART_TIERS[tier], self.family())
    }

    /// What it's for
    pub fn gathers(self) -> &'static str {
        match self {
            Tool::Hatchet => "Chops trees for wood",
            Tool::Pickaxe => "Breaks stone, rock and ore",
            Tool::Spade => "Digs clay, sand and earth",
            Tool::Sickle => "Cuts reeds, dry grass and flax",
            Tool::Hammer => "Cuts sandstone, slate and marble",
        }
    }

    /// The tool for gathering what yields `y`
    pub fn for_yield(y: Yield) -> Option<Tool> {
        match y {
            Yield::Wood => Some(Tool::Hatchet),
            Yield::Stone | Yield::Ore => Some(Tool::Pickaxe),
            Yield::Cut => Some(Tool::Hammer),
            Yield::Clay | Yield::Sand | Yield::Earth => Some(Tool::Spade),
            Yield::Plant => Some(Tool::Sickle),
            Yield::Nothing => None,
        }
    }
}

/// What a voxel of `material` needs to be gathered at all, if anything: a tool of the kind, of at
/// least the tier. Copper ore any pickaxe, iron ore and basalt a copper one, obsidian an iron one,
/// marble a copper hammer.
pub fn needs(material: u8) -> Option<(Tool, usize)> {
    match material {
        COPPER_ORE => Some((Tool::Pickaxe, CRUDE)),
        IRON_ORE | BASALT => Some((Tool::Pickaxe, STURDY)),
        OBSIDIAN => Some((Tool::Pickaxe, FINE)),
        MARBLE => Some((Tool::Hammer, STURDY)),
        _ => None,
    }
}

/// Whether `material` can be gathered with `best`, the best tool carried of the kind it needs, if
/// any: Err saying what it takes if not
pub fn can_gather(material: u8, best: Option<usize>) -> Result<(), String> {
    let Some((tool, tier)) = needs(material) else { return Ok(()) };
    if best.is_some_and(|t| t >= tier) {
        return Ok(());
    }
    let a = if tier == FINE { "an" } else { "a" };
    let or = if tier == CRUDE { "" } else { " or better" };
    Err(format!("That needs {a} {} {}{or}: craft one in I › Craft.", TIERS[tier].to_lowercase(), tool.name().to_lowercase()).replace("a stone pickaxe", "a pickaxe"))
}

/// What a tool costs to make: the first tier wood and stone, copper and iron for theirs, steel
/// iron and more of it, and the obsidian tier obsidian, over iron
pub fn cost(tool: Tool, tier: usize) -> Materials {
    let base = match tier {
        CRUDE => Materials::new(3, 2, 0),
        STURDY => Materials::new(4, 0, 0).with(Res::Copper, 3),
        FINE => Materials::new(5, 2, 0).with(Res::Iron, 4),
        STEEL_TIER => Materials::new(6, 0, 2).with(Res::Iron, 9),
        _ => Materials::new(6, 0, 0).with(Res::Iron, 6).with(Res::Obsidian, 6),
    };
    // A crude axe, shovel and sickle need no stone, only a pickaxe and hammer's heads are of it
    match (tool, tier) {
        (Tool::Hatchet | Tool::Spade | Tool::Sickle, CRUDE) => Materials::new(3, 0, 0),
        _ => base,
    }
}

/// What a rare tool does more than a common one of its tier, see perks()
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Perks {
    /// How much quicker, as a fraction more
    pub speed: f64,
    /// The chance of twice what's gathered
    pub double: f64,
    /// Ore gathered as copper and iron, sand as glass
    pub smelt: bool,
    /// A whole tree felled at a single blow
    pub fell: bool,
    /// The blocks either side of the one struck gathered too
    pub wide: bool,
}

/// A tool as it gathers: which, its tier and rarity, and what that makes of it
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gear {
    pub tool: Tool,
    pub tier: usize,
    pub rarity: usize,
    pub perks: Perks,
}

impl Gear {
    /// How many times quicker than bare hands it gathers what it's for
    pub fn speed(&self) -> f64 {
        SPEED[self.tier.min(OBSIDIAN_TIER)] * (1.0 + self.perks.speed)
    }
}

/// Seeds of tools that rolled a rarity have these four bits on top, the rarity in the four under
/// them; any other is a common one, as the tools from before were
const ROLLED: u32 = 0x6;
/// The most rarity there is: mythic
pub const MYTHIC: usize = 5;
const RARITY_NAMES: [&str; 6] = ["Common", "Uncommon", "Rare", "Epic", "Legendary", "Mythic"];

/// The rarity of a tool made with `seed`
pub fn rarity_of(seed: u32) -> usize {
    if seed >> 28 == ROLLED { (((seed >> 24) & 0xf) as usize).min(MYTHIC) } else { 0 }
}

/// A seed for a tool of `rarity`, from `roll`: one the size of an ordinary weapon rolls, see
/// weapons::roll_size()
fn rolled_seed(rarity: usize, roll: u32) -> u32 {
    let start = ROLLED << 28 | (rarity.min(MYTHIC) as u32) << 24 | (roll & 0x00ff_f000);
    (start..start + 4096).find(|&s| crate::weapons::roll_size(s) == 2).unwrap_or(start)
}

/// The seed every common tool's made with, so each tier of each is always the same, as the tools
/// from before were
fn seed(tool: Tool, tier: usize) -> u32 {
    let start = 0x7001_0000 + tool as u32 * 64 + tier as u32 * 8;
    (start..start + 4096).find(|&s| crate::weapons::roll_size(s) == 2).unwrap_or(start)
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

/// The chances of each rarity rolling for a tool crafted, the luckier the rarer: chests and bosses
/// roll with more luck than the forge, see roll_rarity()
const RARITY_CHANCE: [f64; 6] = [0.70, 0.20, 0.07, 0.025, 0.0045, 0.0005];

/// A rarity rolled from `roll`, `luck` 1 for a tool crafted, more for one found: each rarer one
/// that many times likelier again
pub fn roll_rarity(roll: u32, luck: f64) -> usize {
    let weights: Vec<f64> = RARITY_CHANCE.iter().enumerate().map(|(r, c)| c * luck.max(0.1).powi(r as i32)).collect();
    let total: f64 = weights.iter().sum();
    let mut at = (hash(roll ^ 0x7a2e) >> 8) as f64 / 16_777_216.0 * total;
    for (r, w) in weights.iter().enumerate() {
        if at < *w {
            return r;
        }
        at -= w;
    }
    MYTHIC
}

/// What a tool of `rarity` made with `seed` does more: each step of rarity 6% quicker, and from
/// uncommon on, a perk or more picked by its seed from those its kind can have, one each for
/// uncommon and rare, two for epic and legendary, three for mythic; a legendary or mythic one
/// always has its kind's best, a whole tree felled at a blow for an axe, smelting as it gathers for
/// a pickaxe or shovel, a wide swing for the rest
pub fn perks(tool: Tool, rarity: usize, seed: u32) -> Perks {
    #[derive(Clone, Copy, PartialEq)]
    enum Perk {
        Swift,
        Double,
        Smelt,
        Fell,
        Wide,
    }
    let pool: &[Perk] = match tool {
        Tool::Hatchet => &[Perk::Fell, Perk::Double, Perk::Swift],
        Tool::Pickaxe | Tool::Spade => &[Perk::Smelt, Perk::Double, Perk::Wide, Perk::Swift],
        Tool::Sickle | Tool::Hammer => &[Perk::Wide, Perk::Double, Perk::Swift],
    };
    let rarity = rarity.min(MYTHIC);
    let mut p = Perks { speed: 0.06 * rarity as f64, ..Perks::default() };
    let count = [0, 1, 1, 2, 2, 3][rarity];
    let mut chosen: Vec<Perk> = Vec::new();
    if rarity >= 4 {
        chosen.push(pool[0]);
    }
    let mut h = hash(seed ^ 0x9e1c);
    while chosen.len() < count.min(pool.len()) {
        h = hash(h);
        let perk = pool[h as usize % pool.len()];
        if !chosen.contains(&perk) {
            chosen.push(perk);
        }
    }
    for perk in chosen {
        match perk {
            Perk::Swift => p.speed += 0.25,
            Perk::Double => p.double = 0.15 + 0.05 * rarity as f64,
            Perk::Smelt => p.smelt = true,
            Perk::Fell => p.fell = true,
            Perk::Wide => p.wide = true,
        }
    }
    p
}

/// What a perk says on a tool's card
pub fn perk_lines(gear: &Gear) -> Vec<String> {
    let p = gear.perks;
    let mut lines = Vec::new();
    if p.speed > 1e-6 {
        lines.push(format!("+{:.0}% gathering speed", p.speed * 100.0));
    }
    if p.double > 0.0 {
        lines.push(format!("{:.0}% chance of twice the yield", p.double * 100.0));
    }
    if p.smelt {
        lines.push("Smelts as it gathers: ore comes out as copper and iron, sand as glass".into());
    }
    if p.fell {
        lines.push("Fells a whole tree at a single blow".into());
    }
    if p.wide {
        lines.push("Wide swing: gathers the blocks either side too".into());
    }
    lines
}

/// Makes a common tool `tool` of tier `tier`, as it always was, if assets\weapons.json has it
pub fn make(library: &Library, tool: Tool, tier: usize) -> Option<Weapon> {
    assemble(library, tool, tier, seed(tool, tier))
}

/// Makes tool `tool` of tier `tier` and rarity `rarity`, its perks from `roll`
pub fn make_rare(library: &Library, tool: Tool, tier: usize, rarity: usize, roll: u32) -> Option<Weapon> {
    if rarity == 0 {
        return make(library, tool, tier);
    }
    assemble(library, tool, tier, rolled_seed(rarity, roll))
}

/// Crafts tool `tool` of tier `tier`, its rarity rolled from `roll`
pub fn craft(library: &Library, tool: Tool, tier: usize, roll: u32) -> Option<Weapon> {
    make_rare(library, tool, tier, roll_rarity(roll, 1.0), hash(roll))
}

/// A tool found in a chest or left by a boss, from `roll`: any kind, a tier as good as `depth`, 0
/// to 1, says, how far on the player is, and a rarity rolled with `luck`
pub fn loot(library: &Library, roll: u32, depth: f64, luck: f64) -> Option<Weapon> {
    let tool = Tool::ALL[hash(roll ^ 0x1007) as usize % Tool::ALL.len()];
    let top = (depth.clamp(0.0, 1.0) * OBSIDIAN_TIER as f64).round() as usize;
    let tier = (hash(roll ^ 0x71e2) as usize % (top + 1)).max(top.saturating_sub(1));
    make_rare(library, tool, tier, roll_rarity(roll, luck), hash(roll ^ 0x5eed))
}

/// What a merchant has of tools, see game.rs: `count` of them from `roll`, of the tiers up to
/// iron, a little luckier than crafting. For merchants of their own to stock too.
pub fn merchant_stock(library: &Library, roll: u32, count: usize) -> Vec<Weapon> {
    (0..count as u32)
        .filter_map(|k| {
            let r = hash(roll ^ k.wrapping_mul(0x9e37_79b9));
            let tool = Tool::ALL[hash(r ^ 0x1007) as usize % Tool::ALL.len()];
            let tier = STURDY + hash(r ^ 0x71e2) as usize % 2;
            make_rare(library, tool, tier, roll_rarity(r, 2.0), hash(r ^ 0x5eed))
        })
        .collect()
}

fn assemble(library: &Library, tool: Tool, tier: usize, seed: u32) -> Option<Weapon> {
    let family = library.family(tool.family())?;
    let part = |id: &str| library.components.iter().position(|c| c.id == id);
    let parts = vec![part(&tool.head(tier))?, part(&format!("{}_tool_haft", PART_TIERS[tier]))?];
    Some(Weapon::assemble(library, family, parts, seed))
}

/// What a tool made of `parts` with `seed` in `family` is called, and its rarity, which it's
/// tinted as, if it's a tool: like "Iron Axe". See Weapon::assemble().
pub fn dress(library: &Library, family: usize, parts: &[usize], seed: u32) -> Option<(String, usize)> {
    let tool = Tool::ALL.into_iter().find(|t| t.family() == library.families[family].id)?;
    let tier = tier_of(library, tool, parts);
    Some((format!("{} {}", TIERS[tier], tool.name()), rarity_of(seed)))
}

fn tier_of(library: &Library, tool: Tool, parts: &[usize]) -> usize {
    let head = parts.first().map_or("", |&p| library.components[p].id.as_str());
    (0..TIERS.len()).find(|&t| head == tool.head(t)).unwrap_or(CRUDE)
}

/// Which tool `weapon` is, and its tier, if it's one
pub fn of(library: &Library, weapon: &Weapon) -> Option<(Tool, usize)> {
    gear(library, weapon).map(|g| (g.tool, g.tier))
}

/// Everything about `weapon` as a tool, if it's one
pub fn gear(library: &Library, weapon: &Weapon) -> Option<Gear> {
    let family = &library.families[weapon.family];
    let tool = Tool::ALL.into_iter().find(|t| t.family() == family.id)?;
    let tier = tier_of(library, tool, &weapon.parts);
    let rarity = rarity_of(weapon.seed);
    Some(Gear { tool, tier, rarity, perks: perks(tool, rarity, weapon.seed) })
}

/// The best of `gears` for gathering with: the quickest, then the one with the most perks
pub fn best(gears: impl Iterator<Item = Gear>) -> Option<Gear> {
    gears.max_by(|a, b| a.speed().total_cmp(&b.speed()).then(a.rarity.cmp(&b.rarity)))
}

/// What a tool's card says of it, see Game::weapon_card(): its tier, what it's for, how quick it
/// is, what's hardest it can gather, and its perks
pub fn card_lines(gear: &Gear) -> Vec<(String, Color32)> {
    let [r, g, b] = TIER_COLORS[gear.tier];
    let mut lines = vec![
        (format!("{} tier  ·  {}", TIERS[gear.tier], RARITY_NAMES[gear.rarity]), Color32::from_rgb(r, g, b)),
        (format!("{}, {:.1}× as quick as bare hands", gear.tool.gathers(), gear.speed() / BARE_HANDS), Color32::from_rgb(200, 220, 255)),
    ];
    let can: Vec<&str> = [(COPPER_ORE, "copper ore"), (IRON_ORE, "iron ore"), (BASALT, "basalt"), (OBSIDIAN, "obsidian"), (MARBLE, "marble")]
        .iter()
        .filter(|(m, _)| needs(*m).is_some_and(|(t, tier)| t == gear.tool && gear.tier >= tier))
        .map(|(_, name)| *name)
        .collect();
    if !can.is_empty() {
        lines.push((format!("Can gather {}", can.join(", ")), Color32::from_rgb(150, 235, 125)));
    }
    for perk in perk_lines(gear) {
        lines.push((format!("• {perk}"), Color32::from_rgb(255, 205, 110)));
    }
    lines
}

/// How quickly what yields `y` is gathered with `gear`, in hand or carried, next to bare hands:
/// a tool for it at its speed, anything else as bare hands do. None for what can't be gathered.
pub fn power(gear: Option<Gear>, y: Yield) -> Option<f64> {
    let wanted = Tool::for_yield(y)?;
    match gear {
        Some(g) if g.tool == wanted => Some(g.speed()),
        _ => Some(BARE_HANDS),
    }
}

/// What the tool for `y` does for the player, `best` the best one carried: like "Iron Axe: 4×
/// faster", or "An axe is 2× faster — craft one in I › Craft", and whether one's carried
pub fn faster(y: Yield, best: Option<Gear>) -> Option<(String, bool)> {
    let tool = Tool::for_yield(y)?;
    Some(match best {
        Some(g) => (format!("{} {}: {:.1}× faster", TIERS[g.tier], tool.name(), g.speed() / BARE_HANDS).replace(".0×", "×"), true),
        None => {
            let name = tool.name().to_lowercase();
            let a = if name.starts_with(['a', 'e', 'i', 'o', 'u']) { "An" } else { "A" };
            (format!("{a} {name} is {:.0}× faster — craft one in I › Craft", SPEED[CRUDE] / BARE_HANDS), false)
        }
    })
}

/// The inventory's tabs: what's carried, or crafting from what's gathered
pub struct Crafting {
    pub open: bool,
    /// What's carried to build and make things with, taken from as they're made
    pub materials: Materials,
}

/// The tabs along the top of the inventory: whether the Craft one's open
pub fn tabs(ui: &mut egui::Ui, crafting: &mut Crafting) -> bool {
    ui.horizontal(|ui| {
        if ui.selectable_label(!crafting.open, RichText::new("Pack").size(15.0)).clicked() {
            crafting.open = false;
        }
        if ui.selectable_label(crafting.open, RichText::new("Craft").size(15.0)).clicked() {
            crafting.open = true;
        }
    });
    ui.separator();
    crafting.open
}

/// The Craft tab: each tool at each tier, what it costs and how quick it is, and a button to make
/// it if there's enough, `best` saying the best of each already carried. Which is to be made.
pub fn craft_tab(ui: &mut egui::Ui, crafting: &Crafting, best: &dyn Fn(Tool) -> Option<Gear>) -> Option<(Tool, usize)> {
    let m = crafting.materials;
    let mut made = None;
    ui.set_min_width(900.0);
    let carried: Vec<String> = m.parts().iter().map(|(r, n)| format!("{n} {}", r.name().to_lowercase())).collect();
    ui.label(RichText::new(format!("Carried:  {}", if carried.is_empty() { "nothing yet".to_owned() } else { carried.join("   ") })).strong());
    ui.label(RichText::new("Hold G at what you want to gather. Bare hands gather most things; the best tool you carry for it is used and makes it quicker, and the hardest stone and ore need a good enough one. Smelt ore and sand at a furnace (E). Each tool crafted rolls a rarity: the rarer, the more it does.").weak().small());
    ui.add_space(6.0);
    egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
        egui::Grid::new("craft").spacing([12.0, 8.0]).striped(true).show(ui, |ui| {
            for tool in Tool::ALL {
                let have = best(tool);
                ui.vertical(|ui| {
                    ui.set_min_width(130.0);
                    ui.label(RichText::new(tool.name()).strong().size(15.0));
                    ui.label(RichText::new(tool.gathers()).weak().small());
                    ui.label(RichText::new(have.map_or("None carried".to_owned(), |g| format!("Carrying {} {}", RARITY_NAMES[g.rarity].to_lowercase(), TIERS[g.tier].to_lowercase()))).small());
                });
                for tier in 0..TIERS.len() {
                    let price = cost(tool, tier);
                    let afford = m.covers(price);
                    ui.vertical(|ui| {
                        ui.set_min_width(140.0);
                        let [r, g, b] = TIER_COLORS[tier];
                        ui.label(RichText::new(format!("{} {}", TIERS[tier], tool.name())).color(Color32::from_rgb(r, g, b)).strong());
                        ui.label(RichText::new(format!("{:.1}× faster", SPEED[tier] / BARE_HANDS).replace(".0×", "×")).small());
                        for (res, n) in price.parts() {
                            let ok = m.get(res) >= n;
                            ui.label(RichText::new(format!("{n} {}", res.name().to_lowercase())).small().color(if ok { Color32::from_gray(220) } else { Color32::from_rgb(220, 110, 100) }));
                        }
                        if ui.add_enabled(afford, egui::Button::new("Craft")).clicked() {
                            made = Some((tool, tier));
                        }
                    });
                }
                ui.end_row();
            }
        });
    });
    ui.add_space(4.0);
    ui.label(RichText::new("Copper ore needs a pickaxe, iron ore and basalt a copper one, obsidian an iron one, marble a copper mason's hammer. Merchants sell tools, and chests and bosses leave them.").weak().small());
    made
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_is_made_at_every_tier_and_known_again() {
        let library = Library::load();
        for tool in Tool::ALL {
            for tier in 0..TIERS.len() {
                let made = make(&library, tool, tier).unwrap_or_else(|| panic!("{tool:?} {tier}"));
                assert_eq!(of(&library, &made), Some((tool, tier)));
                assert_eq!(made.name, format!("{} {}", TIERS[tier], tool.name()));
                // Made plain it's common, and tinted so
                assert_eq!(made.grade, 0, "{}", made.name);
                assert!(made.profile.special.is_none(), "{} has no special", made.name);
                // The same every time
                assert_eq!(make(&library, tool, tier).unwrap().seed, made.seed);
            }
        }
        // A weapon isn't a tool
        let sword = library.generate(1, library.family("sword"), 1.0);
        assert_eq!(of(&library, &sword), None);
    }

    /// A tool goes in the save as any weapon does, by its family's id, its parts' and its seed, see
    /// Game::weapon_save(), and comes back the same tool, its rarity and perks with it. A tool from
    /// before tiers and rarity comes back a common one of its tier.
    #[test]
    fn tools_go_in_the_save_as_weapons_do() {
        let library = Library::load();
        for tool in Tool::ALL {
            for rarity in [0, 3, MYTHIC] {
                let made = make_rare(&library, tool, STURDY, rarity, 1234).unwrap();
                let saved = crate::save::WeaponSave { family: library.families[made.family].id.clone(), parts: made.parts.iter().map(|&p| library.components[p].id.clone()).collect(), seed: made.seed, name: None, ..Default::default() };
                let text = serde_json::to_string(&crate::save::ItemSave::Weapon(saved)).unwrap();
                let crate::save::ItemSave::Weapon(back) = serde_json::from_str(&text).unwrap() else { panic!("not a weapon") };
                let family = library.family(&back.family).unwrap();
                let parts = back.parts.iter().map(|id| library.components.iter().position(|c| c.id == *id).unwrap()).collect();
                let again = Weapon::assemble(&library, family, parts, back.seed);
                assert_eq!(gear(&library, &again), gear(&library, &made));
                assert_eq!((again.name.as_str(), again.grade), (made.name.as_str(), rarity));
            }
        }
        // As a sturdy hatchet was saved before: a common copper axe now
        let back = crate::save::WeaponSave { family: "hatchet".into(), parts: vec!["sturdy_hatchet_head".into(), "sturdy_tool_haft".into()], seed: 0x7001_0080, ..Default::default() };
        let parts = back.parts.iter().map(|id| library.components.iter().position(|c| c.id == *id).unwrap()).collect();
        let again = Weapon::assemble(&library, library.family("hatchet").unwrap(), parts, back.seed);
        assert_eq!(gear(&library, &again), Some(Gear { tool: Tool::Hatchet, tier: STURDY, rarity: 0, perks: Perks::default() }));
        assert_eq!(again.name, "Copper Axe");
    }

    #[test]
    fn loot_is_never_a_tool() {
        let library = Library::load();
        for seed in 0..3000 {
            let weapon = library.generate(seed, None, 4.0);
            assert!(of(&library, &weapon).is_none(), "seed {seed} rolled {}", weapon.name);
        }
    }

    #[test]
    fn crafting_costs() {
        for tool in Tool::ALL {
            // A first one is cheap: a few wood, and for a pickaxe or hammer a little stone
            let crude = cost(tool, CRUDE);
            assert!(crude.wood <= 3 && crude.stone <= 2 && crude.clay == 0, "{tool:?}");
            // Each tier takes what's got with the one before it
            assert!(cost(tool, STURDY).copper > 0 && cost(tool, FINE).iron > 0 && cost(tool, OBSIDIAN_TIER).obsidian > 0);
            for tier in 1..TIERS.len() {
                assert!(SPEED[tier] > SPEED[tier - 1]);
            }
        }
        // What's carried pays for it
        let mut carried = Materials::new(10, 2, 3);
        assert!(carried.covers(cost(Tool::Pickaxe, CRUDE)));
        assert!(carried.take(cost(Tool::Pickaxe, CRUDE)));
        assert_eq!(carried, Materials::new(7, 0, 3));
        assert!(!carried.covers(cost(Tool::Hatchet, STURDY)));
    }

    fn common(tool: Tool, tier: usize) -> Gear {
        Gear { tool, tier, rarity: 0, perks: Perks::default() }
    }

    // Bare hands gather most things; the right tool only makes it quicker, the better the quicker
    #[test]
    fn bare_hands_gather_and_tools_only_quicker() {
        for y in [Yield::Wood, Yield::Stone, Yield::Cut, Yield::Ore, Yield::Clay, Yield::Sand, Yield::Plant, Yield::Earth] {
            assert_eq!(power(None, y), Some(BARE_HANDS), "{y:?}");
            let tool = Tool::for_yield(y).unwrap();
            let mut last = BARE_HANDS;
            for tier in 0..TIERS.len() {
                let p = power(Some(common(tool, tier)), y).unwrap();
                assert!(p > last, "{y:?} {tier}");
                last = p;
            }
        }
        // The wrong tool is as good as bare hands
        assert_eq!(power(Some(common(Tool::Hatchet, FINE)), Yield::Stone), Some(BARE_HANDS));
        assert_eq!(power(Some(common(Tool::Spade, CRUDE)), Yield::Wood), Some(BARE_HANDS));
        assert_eq!(power(Some(common(Tool::Pickaxe, STURDY)), Yield::Stone), Some(3.0));
        assert_eq!(power(Some(common(Tool::Hatchet, FINE)), Yield::Nothing), None);
        // What the HUD says a tool would do
        assert_eq!(faster(Yield::Wood, None), Some(("An axe is 2× faster — craft one in I › Craft".to_owned(), false)));
        assert_eq!(faster(Yield::Stone, Some(common(Tool::Pickaxe, STURDY))), Some(("Copper Pickaxe: 3× faster".to_owned(), true)));
        assert_eq!(faster(Yield::Nothing, None), None);
    }

    // The hardest of what's gathered needs a tool of the right kind and a good enough tier
    #[test]
    fn tiers_gate_what_can_be_gathered() {
        assert!(can_gather(STONE, None).is_ok());
        assert!(can_gather(SANDSTONE, None).is_ok());
        assert!(can_gather(COPPER_ORE, None).is_err());
        assert!(can_gather(COPPER_ORE, Some(CRUDE)).is_ok());
        assert_eq!(can_gather(IRON_ORE, Some(CRUDE)), Err("That needs a copper pickaxe or better: craft one in I › Craft.".to_owned()));
        assert!(can_gather(IRON_ORE, Some(STURDY)).is_ok());
        assert_eq!(can_gather(OBSIDIAN, Some(STURDY)), Err("That needs an iron pickaxe or better: craft one in I › Craft.".to_owned()));
        assert!(can_gather(OBSIDIAN, Some(FINE)).is_ok() && can_gather(OBSIDIAN, Some(OBSIDIAN_TIER)).is_ok());
        assert!(can_gather(MARBLE, Some(CRUDE)).is_err() && can_gather(MARBLE, Some(STURDY)).is_ok());
        assert_eq!(needs(MARBLE).map(|n| n.0), Some(Tool::Hammer));
    }

    // Rarer tools are quicker and do more: none for a common one, a perk or more from uncommon on,
    // and a legendary one always its kind's best
    #[test]
    fn rarity_gives_bonuses() {
        let library = Library::load();
        for tool in Tool::ALL {
            let plain = perks(tool, 0, 99);
            assert_eq!(plain, Perks::default());
            // Each step of rarity quicker, and the quicker on the whole for the perks that make it so
            let mut last = 0.0;
            for rarity in 0..=MYTHIC {
                let mean = (0..200u32).map(|seed| Gear { tool, tier: FINE, rarity, perks: perks(tool, rarity, seed) }.speed()).sum::<f64>() / 200.0;
                assert!(mean > last, "{tool:?} {rarity}");
                last = mean;
            }
            for seed in 0..40u32 {
                for rarity in 0..=MYTHIC {
                    let p = perks(tool, rarity, seed);
                    assert!(p.speed >= 0.06 * rarity as f64 - 1e-9, "{tool:?} {rarity}");
                    let count = (p.speed > 0.06 * rarity as f64 + 1e-9) as usize + (p.double > 0.0) as usize + p.smelt as usize + p.fell as usize + p.wide as usize;
                    assert_eq!(count, [0, 1, 1, 2, 2, 3][rarity], "{tool:?} {rarity} {p:?}");
                }
                let legendary = perks(tool, 4, seed);
                match tool {
                    Tool::Hatchet => assert!(legendary.fell),
                    Tool::Pickaxe | Tool::Spade => assert!(legendary.smelt),
                    _ => assert!(legendary.wide),
                }
                assert!(!perks(Tool::Sickle, MYTHIC, seed).fell && !perks(Tool::Hatchet, MYTHIC, seed).wide);
            }
        }
        // Rolled into the seed, so the weapon's rarity is the tool's
        let mythic = make_rare(&library, Tool::Hatchet, STEEL_TIER, MYTHIC, 7).unwrap();
        assert_eq!((mythic.grade, rarity_of(mythic.seed)), (MYTHIC, MYTHIC));
        assert!(gear(&library, &mythic).unwrap().perks.fell);
        let lines = card_lines(&gear(&library, &mythic).unwrap());
        assert!(lines[0].0.contains("Steel tier") && lines.iter().any(|l| l.0.contains("single blow")), "{lines:?}");
        // Crafting rolls mostly common, chests and bosses luckier
        let crafted = (0..4000u32).filter(|&r| roll_rarity(r, 1.0) >= 2).count();
        let found = (0..4000u32).filter(|&r| roll_rarity(r, 3.0) >= 2).count();
        assert!(crafted > 200 && crafted < 600 && found > crafted * 2, "{crafted} {found}");
        assert!((0..4000u32).any(|r| roll_rarity(r, 3.0) == 4));
        // Merchants and loot make tools of what's asked
        assert_eq!(merchant_stock(&library, 5, 3).len(), 3);
        let early = loot(&library, 11, 0.0, 2.0).unwrap();
        assert_eq!(of(&library, &early).unwrap().1, CRUDE);
    }
}
