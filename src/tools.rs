//! Tools for gathering, see building.rs and gather.rs: a hatchet for wood, a pickaxe for stone and
//! rock, and a spade for clay, earth and sand, each Crude, Sturdy or Fine. Bare hands gather all of
//! it, holding G at it; a tool only makes it quicker, twice as quick crude, three times sturdy and
//! four times fine. G gathers with the best one carried for what's looked at, in hand or not.
//! They're made in the Craft tab of the inventory (I) from wood, stone and clay, and the merchants
//! sell Fine ones. A tool is a weapon of a family of its own in assets\weapons.json, so it's held,
//! swung, carried, sold and saved as weapons are: in hand, a click swings it too, and where its blow
//! lands on what it's for it gathers it, see App::update_tools().

use egui_winit_vulkano::egui::{self, Color32, RichText};

use crate::bases::Materials;
use crate::gather::Yield;
use crate::weapons::{Library, Weapon};

/// The tools there are
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Hatchet,
    Pickaxe,
    Spade,
}

/// How good a tool is: an index into GRADES
pub const CRUDE: usize = 0;
pub const STURDY: usize = 1;
pub const FINE: usize = 2;
pub const GRADES: [&str; 3] = ["Crude", "Sturdy", "Fine"];
/// How many times quicker than bare hands each grade gathers
pub const SPEED: [f64; 3] = [2.0, 3.0, 4.0];
/// How quickly bare hands gather, which is everything there is to gather
pub const BARE_HANDS: f64 = 1.0;

impl Tool {
    pub const ALL: [Tool; 3] = [Tool::Hatchet, Tool::Pickaxe, Tool::Spade];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Hatchet => "Hatchet",
            Tool::Pickaxe => "Pickaxe",
            Tool::Spade => "Spade",
        }
    }

    /// Its family's id in assets\weapons.json
    pub fn family(self) -> &'static str {
        match self {
            Tool::Hatchet => "hatchet",
            Tool::Pickaxe => "pickaxe",
            Tool::Spade => "spade",
        }
    }

    /// The id of the part its head is, at grade `grade`
    fn head(self, grade: usize) -> String {
        let part = if self == Tool::Spade { "blade" } else { "head" };
        format!("{}_{}_{part}", GRADES[grade].to_lowercase(), self.family())
    }

    /// What it's for: wood, stone, or clay and earth
    pub fn gathers(self) -> &'static str {
        match self {
            Tool::Hatchet => "Chops trees for wood faster",
            Tool::Pickaxe => "Breaks stone and rock faster",
            Tool::Spade => "Digs clay, earth and sand faster",
        }
    }

    /// The tool for gathering what yields `y`
    pub fn for_yield(y: Yield) -> Option<Tool> {
        match y {
            Yield::Wood => Some(Tool::Hatchet),
            Yield::Stone => Some(Tool::Pickaxe),
            Yield::Clay | Yield::Earth => Some(Tool::Spade),
            Yield::Nothing => None,
        }
    }
}

/// What a tool costs to make, in wood, stone and clay: a crude one a few wood, a crude pickaxe a
/// little stone too, the better ones stone and more of everything
pub fn cost(tool: Tool, grade: usize) -> Materials {
    match (tool, grade) {
        (Tool::Hatchet, CRUDE) => Materials::new(3, 0, 0),
        (Tool::Pickaxe, CRUDE) => Materials::new(3, 2, 0),
        (Tool::Spade, CRUDE) => Materials::new(3, 0, 0),
        (Tool::Hatchet, STURDY) => Materials::new(6, 6, 2),
        (Tool::Pickaxe, STURDY) => Materials::new(6, 8, 2),
        (Tool::Spade, STURDY) => Materials::new(6, 5, 2),
        (Tool::Hatchet, _) => Materials::new(10, 14, 6),
        (Tool::Pickaxe, _) => Materials::new(10, 18, 6),
        (Tool::Spade, _) => Materials::new(10, 12, 6),
    }
}

/// The seed every tool's made with, so each grade of each is always the same: one the size of an
/// ordinary weapon rolls, see weapons::roll_size()
fn seed(tool: Tool, grade: usize) -> u32 {
    let start = 0x7001_0000 + tool as u32 * 64 + grade as u32 * 8;
    (start..start + 4096).find(|&s| crate::weapons::roll_size(s) == 2).unwrap_or(start)
}

/// Makes tool `tool` of grade `grade`, if assets\weapons.json has it
pub fn make(library: &Library, tool: Tool, grade: usize) -> Option<Weapon> {
    let family = library.family(tool.family())?;
    let part = |id: &str| library.components.iter().position(|c| c.id == id);
    let parts = vec![part(&tool.head(grade))?, part(&format!("{}_tool_haft", GRADES[grade].to_lowercase()))?];
    Some(Weapon::assemble(library, family, parts, seed(tool, grade)))
}

/// Which tool `weapon` is, and how good, if it's one
pub fn of(library: &Library, weapon: &Weapon) -> Option<(Tool, usize)> {
    let family = &library.families[weapon.family];
    let tool = Tool::ALL.into_iter().find(|t| t.family() == family.id)?;
    let head = &library.components[*weapon.parts.first()?].id;
    let grade = (0..GRADES.len()).find(|&g| *head == tool.head(g)).unwrap_or(CRUDE);
    Some((tool, grade))
}

/// How quickly what yields `y` is gathered with `tool` of the grade given, in hand or carried,
/// next to bare hands: a tool for it at its grade's speed, anything else as bare hands do. None for
/// what can't be gathered at all.
pub fn power(tool: Option<(Tool, usize)>, y: Yield) -> Option<f64> {
    let wanted = Tool::for_yield(y)?;
    match tool {
        Some((t, grade)) if t == wanted => Some(SPEED[grade.min(FINE)]),
        _ => Some(BARE_HANDS),
    }
}

/// What the tool for `y` does for the player, `best` the grade of the best one carried: like
/// "Crude Hatchet: 2× faster", or "A hatchet is 2× faster — craft one in I › Craft", and whether
/// one's carried
pub fn faster(y: Yield, best: Option<usize>) -> Option<(String, bool)> {
    let tool = Tool::for_yield(y)?;
    Some(match best {
        Some(grade) => (format!("{} {}: {:.0}× faster", GRADES[grade], tool.name(), SPEED[grade] / BARE_HANDS), true),
        None => (format!("A {} is {:.0}× faster — craft one in I › Craft", tool.name().to_lowercase(), SPEED[CRUDE] / BARE_HANDS), false),
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

/// The Craft tab: each tool at each grade, what it costs and how quick it is, and a button to make
/// it if there's enough, `best` saying the best of each already carried. Which is to be made.
pub fn craft_tab(ui: &mut egui::Ui, crafting: &Crafting, best: &dyn Fn(Tool) -> Option<usize>) -> Option<(Tool, usize)> {
    let m = crafting.materials;
    let mut made = None;
    ui.set_min_width(640.0);
    ui.label(RichText::new(format!("Carried:  {} wood   {} stone   {} clay", m.wood, m.stone, m.clay)).strong());
    ui.label(RichText::new("Hold G at a tree, rock or clay to gather it. Bare hands gather everything; the best tool you carry for it is used, and makes it quicker.").weak().small());
    ui.add_space(6.0);
    egui::Grid::new("craft").spacing([18.0, 8.0]).striped(true).show(ui, |ui| {
        for tool in Tool::ALL {
            let have = best(tool);
            ui.vertical(|ui| {
                ui.set_min_width(150.0);
                ui.label(RichText::new(tool.name()).strong().size(15.0));
                ui.label(RichText::new(tool.gathers()).weak().small());
                ui.label(RichText::new(have.map_or("None carried".to_owned(), |g| format!("Carrying a {} one", GRADES[g].to_lowercase()))).small());
            });
            for grade in 0..GRADES.len() {
                let price = cost(tool, grade);
                let afford = m.covers(price);
                ui.vertical(|ui| {
                    ui.set_min_width(140.0);
                    let color =[Color32::from_gray(200), Color32::from_rgb(110, 210, 95), Color32::from_rgb(90, 155, 255)][grade];
                    ui.label(RichText::new(format!("{} {}", GRADES[grade], tool.name())).color(color).strong());
                    ui.label(RichText::new(format!("{:.0}× faster than bare hands", SPEED[grade] / BARE_HANDS)).small());
                    ui.label(RichText::new(price.describe()).small().color(if afford { Color32::from_gray(220) } else { Color32::from_rgb(220, 110, 100) }));
                    if ui.add_enabled(afford, egui::Button::new("Craft")).clicked() {
                        made = Some((tool, grade));
                    }
                });
            }
            ui.end_row();
        }
    });
    ui.add_space(4.0);
    ui.label(RichText::new("Carrying one is enough: G uses it. In hand, a click at what it's for gathers too. Merchants sell fine tools.").weak().small());
    made
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_is_made_at_every_grade_and_known_again() {
        let library = Library::load();
        for tool in Tool::ALL {
            for grade in 0..GRADES.len() {
                let made = make(&library, tool, grade).unwrap_or_else(|| panic!("{tool:?} {grade}"));
                assert_eq!(of(&library, &made), Some((tool, grade)));
                assert_eq!(made.name, format!("{} {}", GRADES[grade], tool.name()));
                // Its grade's rarity, so it's tinted as one: common, uncommon, rare
                assert_eq!(made.grade, grade, "{}", made.name);
                assert!(made.profile.special.is_none(), "{} has no special", made.name);
                // The same every time
                assert_eq!(make(&library, tool, grade).unwrap().seed, made.seed);
            }
        }
        // A weapon isn't a tool
        let sword = library.generate(1, library.family("sword"), 1.0);
        assert_eq!(of(&library, &sword), None);
    }

    /// A tool goes in the save as any weapon does, by its family's id, its parts' and its seed, see
    /// Game::weapon_save(), and comes back the same tool
    #[test]
    fn tools_go_in_the_save_as_weapons_do() {
        let library = Library::load();
        for tool in Tool::ALL {
            let made = make(&library, tool, STURDY).unwrap();
            let saved = crate::save::WeaponSave { family: library.families[made.family].id.clone(), parts: made.parts.iter().map(|&p| library.components[p].id.clone()).collect(), seed: made.seed, name: None, ..Default::default() };
            let text = serde_json::to_string(&crate::save::ItemSave::Weapon(saved)).unwrap();
            let crate::save::ItemSave::Weapon(back) = serde_json::from_str(&text).unwrap() else { panic!("not a weapon") };
            let family = library.family(&back.family).unwrap();
            let parts = back.parts.iter().map(|id| library.components.iter().position(|c| c.id == *id).unwrap()).collect();
            let again = Weapon::assemble(&library, family, parts, back.seed);
            assert_eq!(of(&library, &again), Some((tool, STURDY)));
            assert_eq!(again.name, made.name);
        }
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
            // A crude one is cheap: a few wood, and a crude pickaxe a little stone
            let crude = cost(tool, CRUDE);
            assert!(crude.wood <= 3 && crude.stone <= 2 && crude.clay == 0, "{tool:?}");
            // Each grade costs more of everything than the one before
            for grade in 1..GRADES.len() {
                let (a, b) = (cost(tool, grade - 1), cost(tool, grade));
                assert!(b.wood >= a.wood && b.stone > a.stone && b.clay >= a.clay, "{tool:?} {grade}");
                assert!(SPEED[grade] > SPEED[grade - 1]);
            }
        }
        // What's carried pays for it
        let mut carried = Materials::new(10, 2, 3);
        assert!(carried.covers(cost(Tool::Pickaxe, CRUDE)));
        assert!(carried.take(cost(Tool::Pickaxe, CRUDE)));
        assert_eq!(carried, Materials::new(7, 0, 3));
        assert!(!carried.covers(cost(Tool::Hatchet, STURDY)));
    }

    // Bare hands gather everything there is, stone too; the right tool only makes it quicker, the
    // better the quicker
    #[test]
    fn bare_hands_gather_everything_and_tools_only_quicker() {
        for y in [Yield::Wood, Yield::Stone, Yield::Clay, Yield::Earth] {
            assert_eq!(power(None, y), Some(BARE_HANDS), "{y:?}");
            let tool = Tool::for_yield(y).unwrap();
            let mut last = BARE_HANDS;
            for grade in 0..GRADES.len() {
                let p = power(Some((tool, grade)), y).unwrap();
                assert!(p > last, "{y:?} {grade}");
                last = p;
            }
        }
        // The wrong tool is as good as bare hands
        assert_eq!(power(Some((Tool::Hatchet, FINE)), Yield::Stone), Some(BARE_HANDS));
        assert_eq!(power(Some((Tool::Spade, CRUDE)), Yield::Wood), Some(BARE_HANDS));
        assert_eq!(power(Some((Tool::Pickaxe, STURDY)), Yield::Stone), Some(3.0));
        assert_eq!(power(Some((Tool::Hatchet, FINE)), Yield::Nothing), None);
        // What the HUD says a tool would do
        assert_eq!(faster(Yield::Wood, None), Some(("A hatchet is 2× faster — craft one in I › Craft".to_owned(), false)));
        assert_eq!(faster(Yield::Stone, Some(STURDY)), Some(("Sturdy Pickaxe: 3× faster".to_owned(), true)));
        assert_eq!(faster(Yield::Nothing, None), None);
    }
}
