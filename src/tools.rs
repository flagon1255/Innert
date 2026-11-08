//! Tools for gathering, see building.rs and gather.rs: a hatchet for wood, a pickaxe for stone and
//! rock, and a spade for clay, earth and sand, each Crude, Sturdy or Fine, each grade quicker than
//! the last. They're made in the Craft tab of the inventory (I) from wood, stone and clay, and the
//! merchants sell Fine ones. A tool is a weapon of a family of its own in assets\weapons.json, so
//! it's held, swung, carried, sold and saved as weapons are: in hand, a click swings it, and where
//! its blow lands on what it's for it gathers it, see App::tool_blow(). G gathers with the best
//! one carried for what's struck, without one in hand. Bare hands gather wood and clay slowly, and
//! stone not at all.

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
/// How much quicker each grade gathers than a crude one
pub const SPEED: [f64; 3] = [1.0, 1.6, 2.5];
/// How quickly bare hands gather what they can, next to a crude tool
pub const BARE_HANDS: f64 = 0.4;

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
            Tool::Hatchet => "Fells trees for wood",
            Tool::Pickaxe => "Breaks stone and rock",
            Tool::Spade => "Digs clay, earth and sand",
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

/// What a tool costs to make, in wood, stone and clay: a crude one wood and a little clay, as
/// there's no stone to be had without a pickaxe, the better ones stone and more of everything
pub fn cost(tool: Tool, grade: usize) -> Materials {
    match (tool, grade) {
        (Tool::Hatchet, CRUDE) => Materials::new(4, 0, 0),
        (Tool::Pickaxe, CRUDE) => Materials::new(6, 0, 2),
        (Tool::Spade, CRUDE) => Materials::new(4, 0, 0),
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

/// How much one blow does towards gathering what yields `y`, with `tool` of the grade given in
/// hand or carried: a tool for it at its grade's speed, anything else bare hands. None for stone
/// without a pickaxe, or what can't be gathered at all.
pub fn power(tool: Option<(Tool, usize)>, y: Yield) -> Option<f64> {
    let wanted = Tool::for_yield(y)?;
    match tool {
        Some((t, grade)) if t == wanted => Some(SPEED[grade.min(FINE)]),
        _ if y == Yield::Stone => None,
        _ => Some(BARE_HANDS),
    }
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
    ui.label(RichText::new("Gather with G, or a tool in hand. Bare hands gather wood and clay slowly, and stone not at all: a pickaxe breaks it.").weak().small());
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
                    ui.label(RichText::new(format!("{:.1}x as quick", SPEED[grade] / SPEED[CRUDE])).small());
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
    ui.label(RichText::new("Hold a tool like a weapon: click at a tree, rock or clay to gather it. Merchants sell fine tools too.").weak().small());
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
            let saved = crate::save::WeaponSave { family: library.families[made.family].id.clone(), parts: made.parts.iter().map(|&p| library.components[p].id.clone()).collect(), seed: made.seed };
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
            // A crude one needs no stone, as there's none without a pickaxe
            assert_eq!(cost(tool, CRUDE).stone, 0, "{tool:?}");
            // Each grade costs more of everything than the one before
            for grade in 1..GRADES.len() {
                let (a, b) = (cost(tool, grade - 1), cost(tool, grade));
                assert!(b.wood >= a.wood && b.stone > a.stone && b.clay >= a.clay, "{tool:?} {grade}");
                assert!(SPEED[grade] > SPEED[grade - 1]);
            }
        }
        // What's carried pays for it
        let mut carried = Materials::new(10, 0, 3);
        assert!(carried.covers(cost(Tool::Pickaxe, CRUDE)));
        assert!(carried.take(cost(Tool::Pickaxe, CRUDE)));
        assert_eq!(carried, Materials::new(4, 0, 1));
        assert!(!carried.covers(cost(Tool::Hatchet, STURDY)));
    }

    #[test]
    fn tools_gather_quicker_by_grade_and_stone_needs_a_pickaxe() {
        assert_eq!(power(None, Yield::Stone), None);
        assert_eq!(power(Some((Tool::Hatchet, FINE)), Yield::Stone), None);
        assert_eq!(power(None, Yield::Wood), Some(BARE_HANDS));
        assert_eq!(power(None, Yield::Clay), Some(BARE_HANDS));
        assert_eq!(power(Some((Tool::Spade, CRUDE)), Yield::Wood), Some(BARE_HANDS));
        assert_eq!(power(Some((Tool::Pickaxe, CRUDE)), Yield::Stone), Some(1.0));
        assert_eq!(power(Some((Tool::Pickaxe, STURDY)), Yield::Stone), Some(1.6));
        assert_eq!(power(Some((Tool::Spade, FINE)), Yield::Earth), Some(2.5));
        assert_eq!(power(Some((Tool::Hatchet, FINE)), Yield::Nothing), None);
    }
}
