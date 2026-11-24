//! Who the player is, made in the window that opens before a new game, see creator.rs: what
//! they're called, how they look, what they set out as and how hard the world is on them. Kept in
//! the save under its own name, see SECTION, so a save from before there were any comes out as a
//! wanderer on Normal, as everyone was then.
//!
//! What they set out as, their Class, gives them the weapons they start with, see Class::kit(),
//! and a little of what the skill web gives, see Class::lean(), added to what's taken on it. How
//! they look is drawn as a body of jointed pieces that only shadows and reflections show, never
//! the eye, see body.rs, and in the colours of their hands and sleeves held in front of it.

use serde::{Deserialize, Serialize};

use crate::skills::Stat;

pub mod body;
pub mod creator;

/// What the character's kept under in a save, see Save::section()
pub const SECTION: &str = "character";
/// The longest a name can be, in letters
pub const NAME_LENGTH: usize = 20;

/// Everything about who the player is
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Character {
    pub name: String,
    pub looks: Looks,
    pub class: Class,
    pub difficulty: Difficulty,
}

impl Default for Character {
    /// As in a save from before there were characters: a wanderer on Normal, with everything to
    /// fight with a new player had then
    fn default() -> Self {
        Character { name: "Wanderer".to_owned(), looks: Looks::default(), class: Class::Wanderer, difficulty: Difficulty::Normal }
    }
}

impl Character {
    /// A name that's no longer than it may be and isn't blank, as a hand-edited save might have
    pub fn mend(&mut self) {
        let name: String = self.name.chars().filter(|c| !c.is_control()).take(NAME_LENGTH).collect();
        self.name = if name.trim().is_empty() { Character::default().name } else { name.trim().to_owned() };
        self.looks.mend();
    }

    /// From `--set name=Ysolde class=priest difficulty=hard skin=3 hair=1 ...`, for a new game started
    /// straight away in screenshot runs, everything not given as in a new character
    pub fn from_settings() -> Character {
        let mut c = Character::default();
        for (key, value) in crate::settings_from_args() {
            let n = || value.parse::<u8>().ok();
            match key.as_str() {
                "name" => c.name = value.replace('_', " "),
                "class" => c.class = Class::parse(&value).unwrap_or(c.class),
                "difficulty" => c.difficulty = Difficulty::parse(&value).unwrap_or(c.difficulty),
                "skin" => c.looks.skin = n().unwrap_or(c.looks.skin),
                "hair" => c.looks.hair = n().unwrap_or(c.looks.hair),
                "hair_color" => c.looks.hair_color = n().unwrap_or(c.looks.hair_color),
                "face_look" => c.looks.face = n().unwrap_or(c.looks.face),
                "build" => c.looks.build = n().unwrap_or(c.looks.build),
                "tunic" => c.looks.tunic = n().unwrap_or(c.looks.tunic),
                "sleeves" => c.looks.sleeves = n().unwrap_or(c.looks.sleeves),
                "gloves" => c.looks.gloves = n().unwrap_or(c.looks.gloves),
                _ => {}
            }
        }
        c.mend();
        c
    }
}

/// How the player looks: each a choice from the lists below, by its place in them
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[serde(default)]
pub struct Looks {
    pub skin: u8,
    pub hair: u8,
    pub hair_color: u8,
    pub face: u8,
    pub build: u8,
    pub tunic: u8,
    pub sleeves: u8,
    /// Of GLOVES: the first is bare hands, in the skin's colour
    pub gloves: u8,
}

impl Default for Looks {
    fn default() -> Self {
        Looks { skin: 2, hair: 0, hair_color: 1, face: 0, build: 1, tunic: 3, sleeves: 3, gloves: 1 }
    }
}

/// The colours to choose from, as they look, in sRGB, each with its name
pub const SKINS: [(&str, [u8; 3]); 8] = [
    ("Porcelain", [242, 214, 196]),
    ("Fair", [234, 192, 160]),
    ("Light", [214, 166, 126]),
    ("Olive", [190, 142, 98]),
    ("Tan", [166, 112, 72]),
    ("Brown", [128, 84, 54]),
    ("Dark", [92, 58, 38]),
    ("Deep", [62, 40, 28]),
];
pub const HAIR_COLORS: [(&str, [u8; 3]); 8] = [
    ("Black", [30, 26, 24]),
    ("Dark brown", [70, 44, 28]),
    ("Chestnut", [118, 70, 38]),
    ("Auburn", [150, 62, 34]),
    ("Blond", [214, 176, 110]),
    ("Ash", [170, 160, 146]),
    ("White", [232, 228, 220]),
    ("Raven blue", [36, 44, 78]),
];
pub const CLOTHS: [(&str, [u8; 3]); 10] = [
    ("Undyed", [196, 182, 150]),
    ("Russet", [150, 70, 40]),
    ("Crimson", [160, 30, 36]),
    ("Forest", [52, 96, 52]),
    ("Moss", [104, 118, 60]),
    ("Sky", [80, 130, 190]),
    ("Navy", [34, 48, 96]),
    ("Plum", [96, 46, 104]),
    ("Charcoal", [52, 52, 56]),
    ("Ochre", [196, 150, 52]),
];
pub const GLOVES: [(&str, [u8; 3]); 6] = [
    ("Bare hands", [0, 0, 0]),
    ("Brown leather", [104, 62, 36]),
    ("Black leather", [40, 34, 32]),
    ("Tan suede", [176, 138, 92]),
    ("Red leather", [130, 40, 34]),
    ("Grey wool", [128, 126, 122]),
];
pub const HAIRS: [&str; 5] = ["Cropped", "Long", "Topknot", "Shaved", "Braided"];
pub const FACES: [&str; 4] = ["Calm", "Stern", "Bearded", "Scarred"];
pub const BUILDS: [&str; 3] = ["Slight", "Average", "Broad"];

impl Looks {
    /// Each choice within its list, as a hand-edited save might not have it
    pub fn mend(&mut self) {
        let fit = |v: &mut u8, n: usize| *v = (*v as usize).min(n - 1) as u8;
        fit(&mut self.skin, SKINS.len());
        fit(&mut self.hair, HAIRS.len());
        fit(&mut self.hair_color, HAIR_COLORS.len());
        fit(&mut self.face, FACES.len());
        fit(&mut self.build, BUILDS.len());
        fit(&mut self.tunic, CLOTHS.len());
        fit(&mut self.sleeves, CLOTHS.len());
        fit(&mut self.gloves, GLOVES.len());
    }

    pub fn skin_rgb(&self) -> [u8; 3] {
        SKINS[(self.skin as usize).min(SKINS.len() - 1)].1
    }

    pub fn hair_rgb(&self) -> [u8; 3] {
        HAIR_COLORS[(self.hair_color as usize).min(HAIR_COLORS.len() - 1)].1
    }

    pub fn tunic_rgb(&self) -> [u8; 3] {
        CLOTHS[(self.tunic as usize).min(CLOTHS.len() - 1)].1
    }

    pub fn sleeves_rgb(&self) -> [u8; 3] {
        CLOTHS[(self.sleeves as usize).min(CLOTHS.len() - 1)].1
    }

    /// The hands' colour: the gloves', or the skin's for bare hands
    pub fn gloves_rgb(&self) -> [u8; 3] {
        match self.gloves as usize {
            0 => self.skin_rgb(),
            g => GLOVES[g.min(GLOVES.len() - 1)].1,
        }
    }

    /// The trousers: the tunic's colour, darker
    pub fn trousers_rgb(&self) -> [u8; 3] {
        self.tunic_rgb().map(|c| (c as f64 * 0.5) as u8 + 14)
    }

    /// A look picked at random, from `seed`
    pub fn random(seed: u32) -> Looks {
        let mut s = seed;
        let mut pick = |n: usize| {
            s = hash(s);
            ((s >> 8) as usize % n) as u8
        };
        Looks {
            skin: pick(SKINS.len()),
            hair: pick(HAIRS.len()),
            hair_color: pick(HAIR_COLORS.len()),
            face: pick(FACES.len()),
            build: pick(BUILDS.len()),
            tunic: pick(CLOTHS.len()),
            sleeves: pick(CLOTHS.len()),
            gloves: pick(GLOVES.len()),
        }
    }
}

/// What the player sets out as: what they start with to fight, and a little of what the skill web
/// gives, see kit() and lean()
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Class {
    Warrior,
    Ranger,
    Mage,
    Wanderer,
    Necromancer,
    Priest,
}

impl Class {
    pub const ALL: [Class; 6] = [Class::Warrior, Class::Ranger, Class::Mage, Class::Wanderer, Class::Necromancer, Class::Priest];

    pub fn name(self) -> &'static str {
        match self {
            Class::Warrior => "Warrior",
            Class::Ranger => "Ranger",
            Class::Mage => "Mage",
            Class::Wanderer => "Wanderer",
            Class::Necromancer => "Necromancer",
            Class::Priest => "Priest",
        }
    }

    pub fn parse(name: &str) -> Option<Class> {
        Class::ALL.into_iter().find(|c| c.name().eq_ignore_ascii_case(name.trim()))
    }

    /// What it's like, in a line or two
    pub fn about(self) -> &'static str {
        match self {
            Class::Warrior => "Steel and nerve. Hits hard up close and takes a beating that would fell anyone else.",
            Class::Ranger => "Keen-eyed and quick. Strikes from afar with bow, crossbow and gun before anything gets near.",
            Class::Mage => "A student of the elements. Hurls spells from staff and wand, and makes every drop of mana count.",
            Class::Wanderer => "A bit of everything. Sets out with one of every weapon, gathers fast and travels light.",
            Class::Necromancer => "Bends death to their will. A skull focus drains the life of what it strikes, and raises the slain as skeletons to fight for them.",
            Class::Priest => "Faith made thunder. A talisman hurls spears of holy lightning that pierce and arc through the pack, mending the priest as they strike.",
        }
    }

    /// The families of the weapons it starts with, the first in hand, see weapons.json: a
    /// wanderer's are every one a new player had before there were classes
    pub fn kit(self) -> &'static [&'static str] {
        match self {
            Class::Warrior => &["sword", "axe", "hammer", "greatsword", "spear", "bow"],
            Class::Ranger => &["bow", "crossbow", "musket", "dagger", "spear"],
            Class::Mage => &["staff", "wand", "scepter", "tome", "dagger"],
            Class::Wanderer => &["sword", "bow", "staff", "axe", "hammer", "dagger", "spear", "wand", "greatsword", "katana", "scythe", "halberd", "scepter", "whip", "crossbow", "musket"],
            Class::Necromancer => &["skullfocus", "scythe", "tome", "dagger"],
            Class::Priest => &["talisman", "hammer", "bell", "scepter"],
        }
    }

    /// What it starts out leaning towards, as if from nodes of the skill web, added to what's taken
    /// on it: percent for most, see skills::Stat
    pub fn lean(self) -> &'static [(Stat, f64)] {
        match self {
            Class::Warrior => &[(Stat::MeleeDamage, 10.0), (Stat::Armour, 8.0), (Stat::MaxHealth, 20.0)],
            Class::Ranger => &[(Stat::RangedDamage, 10.0), (Stat::RangedSpeed, 8.0), (Stat::MoveSpeed, 4.0)],
            Class::Mage => &[(Stat::MagicDamage, 10.0), (Stat::ManaCost, -12.0), (Stat::MagicSpeed, 5.0)],
            Class::Wanderer => &[(Stat::GatherSpeed, 20.0), (Stat::MoveSpeed, 6.0), (Stat::GoldFind, 10.0)],
            Class::Necromancer => &[(Stat::MagicDamage, 8.0), (Stat::Leech, 2.0), (Stat::StatusPotency, 10.0)],
            Class::Priest => &[(Stat::MagicDamage, 6.0), (Stat::ElementalDamage, 10.0), (Stat::Regen, 20.0)],
        }
    }

    /// Its colour in the window
    pub fn color(self) -> [u8; 3] {
        match self {
            Class::Warrior => [230, 120, 90],
            Class::Ranger => [130, 210, 110],
            Class::Mage => [120, 170, 255],
            Class::Wanderer => [220, 200, 150],
            Class::Necromancer => [190, 90, 200],
            Class::Priest => [255, 225, 130],
        }
    }
}

/// How hard the world is: how tough and hard-hitting what's fought is, and what's lost on dying
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum Difficulty {
    Story,
    #[default]
    Normal,
    Hard,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [Difficulty::Story, Difficulty::Normal, Difficulty::Hard];

    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Story => "Story",
            Difficulty::Normal => "Normal",
            Difficulty::Hard => "Hard",
        }
    }

    pub fn parse(name: &str) -> Option<Difficulty> {
        Difficulty::ALL.into_iter().find(|d| d.name().eq_ignore_ascii_case(name.trim()))
    }

    /// What it means, in a line
    pub fn about(self) -> &'static str {
        match self {
            Difficulty::Story => "Foes have less health and hit half as hard. Dying costs you nothing.",
            Difficulty::Normal => "The world as it's meant to be. Dying spills half your gold where you fell.",
            Difficulty::Hard => "Foes are tougher and hit far harder. Dying spills all your gold where you fell.",
        }
    }

    /// How many times their own health monsters and bosses have
    pub fn foe_health(self) -> f64 {
        match self {
            Difficulty::Story => 0.65,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.35,
        }
    }

    /// How many times harder they hit
    pub fn foe_damage(self) -> f64 {
        match self {
            Difficulty::Story => 0.5,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.4,
        }
    }

    /// How much of the gold carried spills where the player falls, to come back for
    pub fn gold_lost(self) -> f64 {
        match self {
            Difficulty::Story => 0.0,
            Difficulty::Normal => 0.5,
            Difficulty::Hard => 1.0,
        }
    }
}

/// A name made up from `seed`, as the window's dice give
pub fn random_name(seed: u32) -> String {
    const STARTS: [&str; 28] = [
        "Ael", "Bran", "Cael", "Dar", "Ely", "Fen", "Gar", "Hal", "Isol", "Jor", "Kel", "Lys", "Mor", "Nyr", "Os", "Per", "Quil", "Ros", "Syl", "Tor", "Ul", "Vey", "Wyn", "Yr", "Thal", "Brig", "Aud", "Sev",
    ];
    const ENDS: [&str; 20] = ["a", "an", "en", "ic", "is", "ra", "ren", "wyn", "ith", "or", "el", "dra", "mar", "ion", "ys", "ane", "ard", "ette", "ulf", "ine"];
    const PLACES: [&str; 12] = ["of the Fens", "Ashborn", "the Grey", "of Highmoor", "Stormwake", "the Younger", "of Eld", "Brightwater", "the Quiet", "Thornfield", "of Kell", "Rookhallow"];
    let mut s = hash(seed ^ 0x6e61_6d65);
    let mut pick = |n: usize| {
        s = hash(s);
        (s >> 8) as usize % n
    };
    let first = format!("{}{}", STARTS[pick(STARTS.len())], ENDS[pick(ENDS.len())]);
    let full = if pick(3) == 0 { format!("{first} {}", PLACES[pick(PLACES.len())]) } else { first.clone() };
    if full.chars().count() <= NAME_LENGTH { full } else { first }
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

/// `rgb`, as sRGB bytes, packed into 16 bits, 5 of red, 6 of green and 5 of blue, as the shader
/// reads the colours of the player's clothes, see AnimalInstance::outfit()
pub fn pack565(rgb: [u8; 3]) -> u32 {
    ((rgb[0] as u32 >> 3) << 11) | ((rgb[1] as u32 >> 2) << 5) | (rgb[2] as u32 >> 3)
}

/// The two colours a piece of the player is drawn in, packed for AnimalInstance::outfit(): what's
/// leather on it in `leather`, what's cloth in `cloth`
pub fn outfit(leather: [u8; 3], cloth: [u8; 3]) -> u32 {
    pack565(leather) | (pack565(cloth) << 16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::{self, Mode, Save};

    /// A character saved comes back the same, through the whole save as it's written
    #[test]
    fn a_character_comes_back_from_its_save() {
        let character = Character { name: "Ysolde Thornfield".into(), looks: Looks { skin: 5, hair: 4, hair_color: 3, face: 2, build: 2, tunic: 7, sleeves: 1, gloves: 0 }, class: Class::Priest, difficulty: Difficulty::Hard };
        let mut s = Save { version: save::VERSION, mode: Mode::Realms, ..Default::default() };
        s.set_section(SECTION, &character);
        let bytes = serde_json::to_vec_pretty(&s).unwrap();
        let back = save::parse(&bytes, Mode::Realms).unwrap();
        assert_eq!(back.section::<Character>(SECTION), Some(character));
    }

    /// A save from before there were characters is a wanderer on Normal, and bits of one missing or
    /// out of range come out as a new character's or the nearest that fits
    #[test]
    fn old_saves_load_as_a_wanderer_on_normal() {
        let old = r#"{"version": 1, "mode": "free_roam", "seed": 5, "carried": {"gold": 12}}"#;
        let s = save::parse(old.as_bytes(), Mode::FreeRoam).unwrap();
        let c = s.section::<Character>(SECTION).unwrap_or_default();
        assert_eq!(c.class, Class::Wanderer);
        assert_eq!(c.difficulty, Difficulty::Normal);
        assert_eq!(c.name, "Wanderer");
        let mut partial: Character = serde_json::from_str(r#"{"name": "   ", "looks": {"skin": 200}, "class": "necromancer"}"#).unwrap();
        partial.mend();
        assert_eq!(partial.class, Class::Necromancer);
        assert_eq!(partial.difficulty, Difficulty::Normal);
        assert_eq!(partial.name, "Wanderer");
        assert_eq!(partial.looks.skin as usize, SKINS.len() - 1);
        let mut long = Character { name: "A name far longer than any name should be".into(), ..Default::default() };
        long.mend();
        assert_eq!(long.name.chars().count(), NAME_LENGTH);
    }

    /// Every class starts with weapons that are there to be made, the first in hand of its own, and
    /// leans a little, never a lot, towards what it's about
    #[test]
    fn every_class_has_its_kit_and_lean() {
        let library = crate::weapons::Library::load();
        for class in Class::ALL {
            assert!(!class.kit().is_empty());
            for id in class.kit() {
                assert!(library.family(id).is_some(), "{class:?}'s {id} isn't a family of weapons");
            }
            assert!(!class.lean().is_empty() && class.lean().len() <= 3);
            for &(stat, value) in class.lean() {
                assert!(value.abs() <= 20.0, "{class:?} leans {value} into {stat:?}");
            }
            assert_eq!(Class::parse(class.name()), Some(class));
        }
        let first = |c: Class| library.families[library.family(c.kit()[0]).unwrap()].kind;
        use crate::weapons::Kind;
        assert_eq!(first(Class::Warrior), Kind::Melee);
        assert_eq!(first(Class::Ranger), Kind::Ranged);
        assert_eq!(first(Class::Mage), Kind::Magic);
        assert_eq!(Class::Necromancer.kit()[0], "skullfocus");
        assert_eq!(Class::Priest.kit()[0], "talisman");
    }

    /// Story is gentler than Normal, and Hard harder, in all three ways
    #[test]
    fn difficulty_scales_foes_and_what_dying_costs() {
        let [story, normal, hard] = Difficulty::ALL;
        assert_eq!((normal.foe_health(), normal.foe_damage(), normal.gold_lost()), (1.0, 1.0, 0.5));
        assert!(story.foe_health() < 1.0 && story.foe_damage() < 1.0 && story.gold_lost() == 0.0);
        assert!(hard.foe_health() > 1.0 && hard.foe_damage() > 1.0 && hard.gold_lost() > normal.gold_lost());
        for d in Difficulty::ALL {
            assert_eq!(Difficulty::parse(d.name()), Some(d));
        }
    }

    #[test]
    fn random_names_fit_and_differ() {
        let names: std::collections::HashSet<String> = (0..200).map(random_name).collect();
        assert!(names.len() > 150);
        assert!(names.iter().all(|n| !n.is_empty() && n.chars().count() <= NAME_LENGTH));
    }

    #[test]
    fn colours_pack_as_the_shader_reads_them() {
        assert_eq!(pack565([255, 255, 255]), 0xFFFF);
        assert_eq!(pack565([255, 0, 0]), 0xF800);
        assert_eq!(outfit([0, 0, 255], [0, 255, 0]), 0x001F | (0x07E0 << 16));
        let mut bare = Looks { gloves: 0, ..Default::default() };
        assert_eq!(bare.gloves_rgb(), bare.skin_rgb());
        bare.gloves = 2;
        assert_ne!(bare.gloves_rgb(), bare.skin_rgb());
    }
}
