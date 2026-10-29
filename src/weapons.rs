//! Weapons put together from parts, as WepSystem.md sets out. What the parts are, with their
//! rarities, stats, effects and synergies, is all data, in assets\weapons.json. A weapon is a family
//! (sword, bow or staff) and one part for each of the family's slots, rolled from a seed. The rest
//! is worked out from those: its stats, effects, synergies, grade, name and what it looks like, so
//! the same seed always gives the same weapon.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::f64::consts::{PI, TAU};
use std::path::PathBuf;
use std::rc::Rc;

use nalgebra::Vector3;
use serde::Deserialize;

use crate::animals::{MODEL_RES, Model, Part, ball, cuboid, ellipsoid, limb, voxelize};
use crate::worldgen::material::*;

/// Stats by name, see STATS
pub type Stats = BTreeMap<String, f64>;

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Melee,
    Ranged,
    Magic,
}

/// How a family of weapons is swung, drawn or cast, see Combat::pose()
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    /// Diagonal cuts, one way then back the other
    #[default]
    Slash,
    /// Wide sweeps across
    Chop,
    /// A scythe's long sweeps, its blade held out across the top so its flat shows
    Reap,
    /// Brought down from overhead
    Smash,
    /// Quick jabs
    Stab,
    /// Lunging forward along its length
    Thrust,
    Draw,
    Cast,
    /// A flick of the wrist
    Flick,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Element {
    Fire,
    Frost,
    Storm,
    Poison,
    Arcane,
    Void,
    Blood,
    Holy,
}

/// How many models of each kind there are for projectiles and sparks: one without an element,
/// then one for each element
pub const ELEMENT_LOOKS: usize = 9;

impl Element {
    pub const ALL: [Element; 8] = [
        Element::Fire,
        Element::Frost,
        Element::Storm,
        Element::Poison,
        Element::Arcane,
        Element::Void,
        Element::Blood,
        Element::Holy,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Element::Fire => "Fire",
            Element::Frost => "Frost",
            Element::Storm => "Lightning",
            Element::Poison => "Poison",
            Element::Arcane => "Arcane",
            Element::Void => "Void",
            Element::Blood => "Blood",
            Element::Holy => "Holy",
        }
    }

    /// The synergy tag weapons of this element get
    fn tag(self) -> &'static str {
        match self {
            Element::Fire => "fire",
            Element::Frost => "frost",
            Element::Storm => "lightning",
            Element::Poison => "poison",
            Element::Arcane => "arcane",
            Element::Void => "void",
            Element::Blood => "blood",
            Element::Holy => "holy",
        }
    }

    /// The glowing material its spells and sparks are made of
    pub fn glow(self) -> u8 {
        match self {
            Element::Fire => GLOW_FIRE,
            Element::Frost => GLOW_FROST,
            Element::Storm => GLOW_STORM,
            Element::Poison => GLOW_POISON,
            Element::Arcane => GLOW_ARCANE,
            Element::Void => GLOW_VOID,
            Element::Blood => GLOW_BLOOD,
            Element::Holy => GLOW_HOLY,
        }
    }

    /// Color of the light it gives off
    pub fn light(self) -> [f32; 3] {
        match self {
            Element::Fire => [1.0, 0.5, 0.15],
            Element::Frost => [0.4, 0.7, 1.0],
            Element::Storm => [0.6, 0.7, 1.0],
            Element::Poison => [0.4, 1.0, 0.2],
            Element::Arcane => [0.7, 0.35, 1.0],
            Element::Void => [0.5, 0.15, 0.9],
            Element::Blood => [1.0, 0.1, 0.12],
            Element::Holy => [1.0, 0.9, 0.55],
        }
    }

    /// For text on screen
    pub fn color(self) -> [u8; 3] {
        match self {
            Element::Fire => [255, 140, 50],
            Element::Frost => [140, 210, 255],
            Element::Storm => [170, 190, 255],
            Element::Poison => [120, 230, 80],
            Element::Arcane => [200, 130, 255],
            Element::Void => [170, 90, 230],
            Element::Blood => [235, 50, 60],
            Element::Holy => [255, 230, 140],
        }
    }

    /// Which of the ELEMENT_LOOKS models to use for `element`
    pub fn look(element: Option<Element>) -> usize {
        element.map_or(0, |e| 1 + Element::ALL.iter().position(|&x| x == e).unwrap())
    }
}

/// The effects there's code for. assets\weapons.json says which parts give them and with what numbers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effect {
    Burn,
    Frostbite,
    Chain,
    Poison,
    Echo,
    VoidBurst,
    Bleed,
    Stagger,
    Knockback,
    Execute,
    Siphon,
    Multishot,
    Pierce,
    Explosive,
    Homing,
}

impl Effect {
    fn from_id(id: &str) -> Option<Effect> {
        Some(match id {
            "burn" => Effect::Burn,
            "frostbite" => Effect::Frostbite,
            "chain" => Effect::Chain,
            "poison" => Effect::Poison,
            "echo" => Effect::Echo,
            "void_burst" => Effect::VoidBurst,
            "bleed" => Effect::Bleed,
            "stagger" => Effect::Stagger,
            "knockback" => Effect::Knockback,
            "execute" => Effect::Execute,
            "siphon" => Effect::Siphon,
            "multishot" => Effect::Multishot,
            "pierce" => Effect::Pierce,
            "explosive" => Effect::Explosive,
            "homing" => Effect::Homing,
            _ => return None,
        })
    }
}

/// The synergies there's code for
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Synergy {
    SolarCollapse,
    StormConductor,
    BloodReaper,
    Shatter,
    PlagueBloom,
}

impl Synergy {
    fn from_id(id: &str) -> Option<Synergy> {
        Some(match id {
            "solar_collapse" => Synergy::SolarCollapse,
            "storm_conductor" => Synergy::StormConductor,
            "blood_reaper" => Synergy::BloodReaper,
            "shatter" => Synergy::Shatter,
            "plague_bloom" => Synergy::PlagueBloom,
            _ => return None,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rarity {
    pub id: String,
    pub display_name: String,
    pub color: [u8; 3],
    /// How likely a part is to roll this rarity, see roll_rarity()
    chance: f64,
    /// How much a part of this rarity adds to the weapon's grade
    power: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Family {
    pub id: String,
    pub display_name: String,
    pub kind: Kind,
    #[serde(default)]
    pub style: Style,
    pub slots: Vec<String>,
    #[serde(default)]
    synergy_tags: Vec<String>,
    #[serde(default)]
    base_stats: Stats,
    /// Words for what it is, for names like Ash-cleaver
    nouns: Vec<String>,
    /// A long handle, in meters, rather than the grip its part has: an axe's or a spear's haft,
    /// or a wand's whole length
    #[serde(default)]
    haft: f64,
    /// Where along its handle it's held, 0 at the bottom and 1 at the top
    #[serde(default = "middle")]
    grip: f64,
    /// Whether it has a crossguard
    #[serde(default = "yes")]
    guard: bool,
    /// How wide a swing reaches, as the cosine of the angle out from straight ahead, and how many
    /// it can hit at once
    #[serde(default = "swing_cone")]
    cone: f64,
    #[serde(default = "swing_targets")]
    targets: usize,
}

fn middle() -> f64 {
    0.5
}

fn yes() -> bool {
    true
}

fn swing_cone() -> f64 {
    0.62
}

fn swing_targets() -> usize {
    4
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectDef {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    synergy_tags: Vec<String>,
    #[serde(default)]
    element: Option<Element>,
    pub description: String,
    #[serde(default)]
    params: HashMap<String, f64>,
    #[serde(skip)]
    effect: Option<Effect>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynergyDef {
    pub id: String,
    pub display_name: String,
    /// Tags a weapon needs all of for it
    requires: Vec<String>,
    /// What it calls a weapon it's on
    #[serde(default)]
    title: Option<String>,
    pub description: String,
    #[serde(default)]
    params: HashMap<String, f64>,
    #[serde(skip)]
    synergy: Option<Synergy>,
}

/// What a part looks like. Which fields count depends on its slot, see build_look().
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Visual {
    shape: Option<String>,
    length: Option<f64>,
    width: Option<f64>,
    thickness: Option<f64>,
    size: Option<f64>,
    material: Option<String>,
    edge: Option<String>,
    fuller: Option<String>,
    wrap: Option<String>,
    pommel: Option<String>,
    pommel_material: Option<String>,
    pommel_size: Option<f64>,
    accent: Option<String>,
    tips: Option<String>,
    gem: Option<String>,
    /// For cores: the glow put into the blade's fuller, if it hasn't one of its own
    infuse: Option<String>,
}

/// Words a part gives a weapon's name
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Naming {
    prefix: Option<String>,
    root: Option<String>,
    suffix: Option<String>,
    /// Legendary and mythic parts can name the whole weapon
    signature: Option<String>,
    epithet: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Component {
    pub id: String,
    pub display_name: String,
    pub slot_type: String,
    compatible_families: Vec<String>,
    rarity: String,
    weight: f64,
    #[serde(default = "first_level")]
    pub level_requirement: u32,
    #[serde(default)]
    base_stats: Stats,
    #[serde(default)]
    stat_multipliers: Stats,
    #[serde(default)]
    effects: Vec<String>,
    #[serde(default)]
    synergy_tags: Vec<String>,
    #[serde(default)]
    element: Option<Element>,
    #[serde(default)]
    visual_profile: Visual,
    #[serde(default)]
    naming: Naming,
    /// The special attack a legendary or mythic part gives the weapon
    #[serde(default)]
    special: Option<Special>,
    /// Index of its rarity
    #[serde(skip)]
    pub tier: usize,
    /// Indices of its effects
    #[serde(skip)]
    effect_defs: Vec<usize>,
}

fn first_level() -> u32 {
    1
}

/// Everything weapons can be made of
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub rarities: Vec<Rarity>,
    /// The power a weapon needs for each rarity as its grade
    grade_power: Vec<f64>,
    /// What each synergy on a weapon adds to its power
    synergy_power: f64,
    pub families: Vec<Family>,
    pub effects: Vec<EffectDef>,
    pub synergies: Vec<SynergyDef>,
    pub components: Vec<Component>,
}

const BUILT_IN: &str = include_str!("../assets/weapons.json");

impl Library {
    /// Reads assets\weapons.json, so it can be changed without building again, or if that's
    /// missing or broken the copy built in
    pub fn load() -> Library {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets").join("weapons.json");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|_| BUILT_IN.to_owned());
        Library::parse(&text).unwrap_or_else(|e| {
            eprintln!("{}: {e}. Using the weapons built in.", path.display());
            Library::parse(BUILT_IN).expect("the built-in weapons are broken")
        })
    }

    fn parse(text: &str) -> Result<Library, String> {
        let mut library: Library = serde_json::from_str(text).map_err(|e| e.to_string())?;
        library.resolve()?;
        Ok(library)
    }

    /// Checks everything refers to things that exist, and looks them up once
    fn resolve(&mut self) -> Result<(), String> {
        if self.rarities.is_empty() || self.families.is_empty() {
            return Err("there must be at least one rarity and one family".into());
        }
        for effect in &mut self.effects {
            effect.effect = Effect::from_id(&effect.id);
            if effect.effect.is_none() {
                eprintln!("weapons: there's no code for the effect \"{}\", it does nothing", effect.id);
            }
        }
        for synergy in &mut self.synergies {
            synergy.synergy = Synergy::from_id(&synergy.id);
            if synergy.synergy.is_none() {
                eprintln!("weapons: there's no code for the synergy \"{}\", it does nothing", synergy.id);
            }
        }
        for component in &mut self.components {
            component.tier = self
                .rarities
                .iter()
                .position(|r| r.id == component.rarity)
                .ok_or_else(|| format!("{} has the unknown rarity \"{}\"", component.id, component.rarity))?;
            component.effect_defs = Vec::new();
            for id in &component.effects {
                let index = self.effects.iter().position(|e| &e.id == id).ok_or_else(|| format!("{} has the unknown effect \"{id}\"", component.id))?;
                component.effect_defs.push(index);
            }
            for family in &component.compatible_families {
                let family = self.families.iter().find(|f| &f.id == family).ok_or_else(|| format!("{} fits the unknown family \"{family}\"", component.id))?;
                if !family.slots.contains(&component.slot_type) {
                    return Err(format!("{} is for the slot \"{}\", which {} doesn't have", component.id, component.slot_type, family.id));
                }
            }
            let v = &component.visual_profile;
            for name in [&v.material, &v.edge, &v.fuller, &v.wrap, &v.pommel_material, &v.accent, &v.tips, &v.gem, &v.infuse].into_iter().flatten() {
                if material(name).is_none() {
                    eprintln!("weapons: {} uses the unknown material \"{name}\"", component.id);
                }
            }
        }
        for family in &self.families {
            for slot in &family.slots {
                if !self.components.iter().any(|c| c.slot_type == *slot && c.compatible_families.contains(&family.id)) {
                    return Err(format!("nothing fits the {} slot of {}", slot, family.id));
                }
            }
        }
        Ok(())
    }

    pub fn family(&self, id: &str) -> Option<usize> {
        self.families.iter().position(|f| f.id == id)
    }

    /// A rarity, more likely the rarer ones the luckier: `luck` 1 is the chances as they are, more
    /// makes each rarer tier that much more likely again
    fn roll_rarity(&self, luck: f64, rng: &mut u32) -> usize {
        let weights: Vec<f64> = self.rarities.iter().enumerate().map(|(i, r)| r.chance * luck.max(0.0).powi(i as i32)).collect();
        let total: f64 = weights.iter().sum();
        let mut pick = random(rng) * total;
        for (i, w) in weights.iter().enumerate() {
            if pick < *w {
                return i;
            }
            pick -= w;
        }
        0
    }

    /// Rolls a weapon from `seed`: of `family`, or any family for None. `luck` is as for
    /// roll_rarity(), from where it was found.
    pub fn generate(&self, seed: u32, family: Option<usize>, luck: f64) -> Weapon {
        let (family, parts) = self.roll(seed, family, luck);
        Weapon::assemble(self, family, parts, seed)
    }

    /// Which family and parts generate() makes a weapon of
    fn roll(&self, seed: u32, family: Option<usize>, luck: f64) -> (usize, Vec<usize>) {
        let mut rng = hash(seed ^ 0x3ea9_7c11);
        let family = family.unwrap_or_else(|| (random(&mut rng) * self.families.len() as f64) as usize % self.families.len());
        let id = &self.families[family].id;
        let mut parts = Vec::new();
        for slot in &self.families[family].slots {
            let tier = self.roll_rarity(luck, &mut rng);
            let fits = |c: &&Component| c.slot_type == *slot && c.compatible_families.contains(id);
            // The rarity rolled, or if nothing fits that the next rarity down
            let pool = (0..=tier).rev().map(|t| self.components.iter().enumerate().filter(|(_, c)| fits(c) && c.tier == t).collect::<Vec<_>>()).find(|pool| !pool.is_empty());
            let pool = pool.unwrap_or_else(|| self.components.iter().enumerate().filter(|(_, c)| fits(c)).collect());
            let total: f64 = pool.iter().map(|(_, c)| c.weight.max(0.0)).sum();
            let mut pick = random(&mut rng) * total;
            let chosen = pool.iter().find(|(_, c)| {
                pick -= c.weight.max(0.0);
                pick < 0.0
            });
            parts.push(chosen.or(pool.last()).unwrap().0);
        }
        (family, parts)
    }
}

/// The material called `name` in assets\weapons.json
fn material(name: &str) -> Option<u8> {
    Some(match name {
        "steel" => STEEL,
        "dark_steel" => DARK_STEEL,
        "gold" => GOLD,
        "bronze" => BRONZE,
        "leather" => LEATHER,
        "cloth" => CLOTH,
        "linen" => LINEN,
        "obsidian" => OBSIDIAN,
        "bowstring" => BOWSTRING,
        "ice" => ICE,
        "wood" => WOOD,
        "birch" => BIRCH_BARK,
        "dark_bark" => BARK_DARK,
        "bone" => BONE,
        "crystal" => CRYSTAL,
        "leaves" => LEAVES,
        "fur_white" => FUR_WHITE,
        "fur_tan" => FUR_TAN,
        "wax" => WAX,
        "lacquer" => TEMPLE_RED,
        "jade" => TEMPLE_TEAL,
        "moss" => MOSS,
        "rock" => ROCK,
        "glow_fire" => GLOW_FIRE,
        "glow_frost" => GLOW_FROST,
        "glow_storm" => GLOW_STORM,
        "glow_poison" => GLOW_POISON,
        "glow_arcane" => GLOW_ARCANE,
        "glow_void" => GLOW_VOID,
        "glow_blood" => GLOW_BLOOD,
        "glow_holy" => GLOW_HOLY,
        _ => return None,
    })
}

fn mat(name: &Option<String>, default: u8) -> u8 {
    name.as_deref().and_then(material).unwrap_or(default)
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^ (x >> 16)
}

fn random(seed: &mut u32) -> f64 {
    *seed = hash(seed.wrapping_add(0x9e3779b9));
    (*seed >> 8) as f64 / 16777216.0
}

/// Stats in the order they're shown, with what they're called and how to show them
pub const STATS: [(&str, &str, Unit); 14] = [
    ("physicalDamage", "Physical damage", Unit::Number),
    ("elementalDamage", "Elemental damage", Unit::Number),
    ("spellDamage", "Spell damage", Unit::Number),
    ("attackSpeed", "Swings per second", Unit::Rate),
    ("drawSpeed", "Draws per second", Unit::Rate),
    ("castSpeed", "Casts per second", Unit::Rate),
    ("critChance", "Critical chance", Unit::Percent),
    ("critDamage", "Critical damage", Unit::Times),
    ("range", "Reach", Unit::Meters),
    ("projectileSpeed", "Projectile speed", Unit::Speed),
    ("manaCost", "Mana per cast", Unit::Number),
    ("area", "Area", Unit::Percent),
    ("statusPotency", "Effect potency", Unit::Percent),
    ("knockback", "Knockback", Unit::Speed),
];

#[derive(Clone, Copy)]
pub enum Unit {
    Number,
    Rate,
    Percent,
    Times,
    Meters,
    Speed,
}

impl Unit {
    pub fn show(self, value: f64) -> String {
        match self {
            Unit::Number => format!("{value:.0}"),
            Unit::Rate => format!("{value:.2}"),
            Unit::Percent => format!("{:.0}%", value * 100.0),
            Unit::Times => format!("×{value:.2}"),
            Unit::Meters => format!("{value:.1} m"),
            Unit::Speed => format!("{value:.0} m/s"),
        }
    }
}

/// An effect on a weapon. Each part giving it again makes it half as strong again.
#[derive(Clone)]
pub struct ActiveEffect {
    pub effect: Effect,
    pub def: usize,
    pub potency: f64,
    params: HashMap<String, f64>,
}

#[derive(Clone)]
pub struct ActiveSynergy {
    pub synergy: Synergy,
    pub def: usize,
    params: HashMap<String, f64>,
}

/// What a weapon does in a fight, worked out from its parts
#[derive(Clone)]
pub struct Profile {
    pub kind: Kind,
    pub style: Style,
    /// How wide a swing reaches, as a cosine, and how many it can hit at once
    pub cone: f64,
    pub targets: usize,
    pub element: Option<Element>,
    pub stats: Stats,
    pub effects: Vec<ActiveEffect>,
    pub synergies: Vec<ActiveSynergy>,
    /// Its special attack, if one of its parts gives it one, and which part
    pub special: Option<(Special, String)>,
}

/// A special attack, unleashed once it's charged up: what legendary and mythic parts give a weapon
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Special {
    /// Fireballs falling from the sky round where it's aimed
    Meteors,
    /// An ice storm freezing everything in it
    Blizzard,
    /// Lightning striking every enemy round about
    Tempest,
    /// A black hole drawing everything in, then collapsing
    Singularity,
    /// The ground splitting open in a line of blasts ahead
    Fissure,
    /// Spinning round, hitting everything near, twice
    Whirlwind,
    /// Dashing forward through them, slashing at everything near
    Flurry,
    /// A burst of holy light throwing everything back, and healing
    Nova,
    /// Arrows raining down round where it's aimed
    Rain,
    /// Draining the life of every enemy near
    Bloodmoon,
    /// A great cloud of venom
    Plague,
    /// Seeking stars, a barrage of them
    Starfall,
    /// Leaping high and bringing the weapon down where it lands
    Skyfall,
    /// Standing guard, turning the next blow aside and answering it
    Riposte,
    /// A fan of arrows pinning what they hit
    Volley,
    /// Lightning leaping from one enemy to the next
    Stormchain,
    /// A wave rolling out ahead, sweeping everything along
    Riptide,
    /// Flame poured out of the weapon
    Breath,
}

impl Special {
    pub fn name(self) -> &'static str {
        match self {
            Special::Skyfall => "Skyfall",
            Special::Riposte => "Riposte",
            Special::Volley => "Pinning Volley",
            Special::Stormchain => "Stormchain",
            Special::Riptide => "Riptide",
            Special::Breath => "Dragon's Breath",
            Special::Meteors => "Meteor Shower",
            Special::Blizzard => "Blizzard",
            Special::Tempest => "Tempest",
            Special::Singularity => "Singularity",
            Special::Fissure => "Worldsplitter",
            Special::Whirlwind => "Whirlwind",
            Special::Flurry => "Flurry",
            Special::Nova => "Holy Nova",
            Special::Rain => "Rain of Arrows",
            Special::Bloodmoon => "Blood Moon",
            Special::Plague => "Plague",
            Special::Starfall => "Starfall",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Special::Skyfall => "Leap high and bring your weapon down where you land.",
            Special::Riposte => "Raise your guard; the next blow is turned aside and answered threefold.",
            Special::Volley => "Loose a fan of arrows that pins whatever they hit.",
            Special::Stormchain => "Lightning leaps from you through the pack, stronger with every jump.",
            Special::Riptide => "A wave rolls out ahead, sweeping everything along before it crashes down.",
            Special::Breath => "Brace and pour a roaring gout of flame from your weapon.",
            Special::Meteors => "Fireballs rain down round where you aim.",
            Special::Blizzard => "An ice storm where you aim freezes everything in it.",
            Special::Tempest => "Lightning strikes every enemy within 25 meters.",
            Special::Singularity => "A black hole where you aim drags everything in, then collapses.",
            Special::Fissure => "The ground splits open ahead of you in a line of blasts.",
            Special::Whirlwind => "Spin twice, hitting everything round you.",
            Special::Flurry => "Dash forward, slashing at everything near five times.",
            Special::Nova => "A burst of holy light throws everything back and heals you.",
            Special::Rain => "Arrows rain down round where you aim.",
            Special::Bloodmoon => "Drain the life of every enemy within 14 meters to heal yourself.",
            Special::Plague => "A great cloud of venom where you aim, lingering.",
            Special::Starfall => "A barrage of stars seeks out the enemies round you.",
        }
    }
}

impl Profile {
    pub fn stat(&self, name: &str) -> f64 {
        self.stats.get(name).copied().unwrap_or(0.0)
    }

    pub fn effect(&self, effect: Effect) -> Option<&ActiveEffect> {
        self.effects.iter().find(|e| e.effect == effect)
    }

    pub fn has(&self, effect: Effect) -> bool {
        self.effect(effect).is_some()
    }

    /// A number from the data for one of its effects, or `default`
    pub fn param(&self, effect: Effect, name: &str, default: f64) -> f64 {
        self.effect(effect).and_then(|e| e.params.get(name).copied()).unwrap_or(default)
    }

    pub fn potency(&self, effect: Effect) -> f64 {
        self.effect(effect).map_or(0.0, |e| e.potency) * self.stat("statusPotency").max(0.0)
    }

    pub fn synergy(&self, synergy: Synergy) -> bool {
        self.synergies.iter().any(|s| s.synergy == synergy)
    }

    pub fn synergy_param(&self, synergy: Synergy, name: &str, default: f64) -> f64 {
        self.synergies.iter().find(|s| s.synergy == synergy).and_then(|s| s.params.get(name).copied()).unwrap_or(default)
    }
}

/// A weapon as it's kept: which parts it was made from, and the seed. The rest is worked out from
/// those by assemble().
pub struct Weapon {
    pub family: usize,
    /// One for each of its family's slots, indices into Library::components
    pub parts: Vec<usize>,
    pub seed: u32,
    pub name: String,
    /// Its overall rarity
    pub grade: usize,
    pub profile: Rc<Profile>,
    pub tags: BTreeSet<String>,
    pub look: WeaponLook,
}

impl Weapon {
    pub fn assemble(library: &Library, family: usize, parts: Vec<usize>, seed: u32) -> Weapon {
        let fam = &library.families[family];
        let components: Vec<&Component> = parts.iter().map(|&p| &library.components[p]).collect();
        let (profile, tags, grade) = Weapon::work_out(library, family, &components);
        let name = weapon_name(library, family, &components, &profile.synergies, seed);
        let look = build_look(fam, &components);
        Weapon { family, parts, seed, name, grade, profile: Rc::new(profile), tags, look }
    }

    /// What a weapon made of `components` does: its stats, effects and synergies, its tags, and its grade
    fn work_out(library: &Library, family: usize, components: &[&Component]) -> (Profile, BTreeSet<String>, usize) {
        let fam = &library.families[family];

        // Flat stats add up, then the multipliers multiply them
        let mut stats = fam.base_stats.clone();
        for c in components {
            for (name, value) in &c.base_stats {
                *stats.entry(name.clone()).or_insert(0.0) += value;
            }
        }
        for c in components {
            for (name, value) in &c.stat_multipliers {
                *stats.entry(name.clone()).or_insert(0.0) *= value;
            }
        }
        // Keep runaway stats in check
        for (name, cap) in [("critChance", 0.75), ("critDamage", 5.0)] {
            if let Some(v) = stats.get_mut(name) {
                *v = v.min(cap);
            }
        }
        for name in ["attackSpeed", "drawSpeed", "castSpeed"] {
            if let Some(v) = stats.get_mut(name) {
                *v = v.clamp(0.25, 6.0);
            }
        }

        // The element comes from the highest rarity part that has one, the core or crystal first
        let mut by_rarity: Vec<&Component> = components.to_vec();
        by_rarity.sort_by_key(|c| std::cmp::Reverse(c.tier));
        let element = by_rarity
            .iter()
            .filter(|c| matches!(c.slot_type.as_str(), "core" | "crystal"))
            .chain(by_rarity.iter())
            .find_map(|c| c.element);
        // Elemental damage without an element is just more of the plain kind
        if element.is_none() {
            let elemental = stats.remove("elementalDamage").unwrap_or(0.0);
            let plain = if fam.kind == Kind::Magic { "spellDamage" } else { "physicalDamage" };
            *stats.entry(plain.into()).or_insert(0.0) += elemental;
        }

        let mut effects: Vec<ActiveEffect> = Vec::new();
        for c in components {
            for &def in &c.effect_defs {
                let Some(effect) = library.effects[def].effect else {
                    continue;
                };
                match effects.iter_mut().find(|e| e.def == def) {
                    Some(e) => e.potency += 0.5,
                    None => effects.push(ActiveEffect { effect, def, potency: 1.0, params: library.effects[def].params.clone() }),
                }
            }
        }

        let mut tags: BTreeSet<String> = fam.synergy_tags.iter().cloned().collect();
        tags.extend(components.iter().flat_map(|c| c.synergy_tags.iter().cloned()));
        tags.extend(effects.iter().flat_map(|e| library.effects[e.def].synergy_tags.iter().cloned()));
        tags.extend(effects.iter().filter_map(|e| library.effects[e.def].element).map(|e| e.tag().to_owned()));
        if let Some(e) = element {
            tags.insert(e.tag().into());
        }
        let synergies: Vec<ActiveSynergy> = library
            .synergies
            .iter()
            .enumerate()
            .filter(|(_, s)| s.requires.iter().all(|t| tags.contains(t)))
            .filter_map(|(def, s)| Some(ActiveSynergy { synergy: s.synergy?, def, params: s.params.clone() }))
            .collect();

        // The grade reflects how much the weapon can do, not the average of its parts: rare parts
        // and synergies add up, and one very rare part lifts the whole weapon close to it
        let power: f64 = components.iter().map(|c| library.rarities[c.tier].power).sum::<f64>() + library.synergy_power * synergies.len() as f64;
        let by_power = library.grade_power.iter().rposition(|&p| power >= p).unwrap_or(0);
        let best = components.iter().map(|c| c.tier).max().unwrap_or(0);
        let grade = by_power.max(best.saturating_sub(1)).min(library.rarities.len() - 1);
        // The rarest part with a special attack gives it its own
        let special = by_rarity.iter().find_map(|c| c.special.map(|s| (s, c.display_name.clone())));
        (Profile { kind: fam.kind, style: fam.style, cone: fam.cone, targets: fam.targets, element, stats, effects, synergies, special }, tags, grade)
    }

    pub fn kind(&self) -> Kind {
        self.profile.kind
    }

    /// Which arrowhead a bow's arrows have, see ARROW_HEADS: always the same for the same bow
    pub fn arrowhead(&self) -> usize {
        hash(self.seed ^ 0xa770) as usize % ARROW_HEADS
    }
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or(String::new(), |c| c.to_uppercase().chain(chars).collect())
}

/// Prefix + core name + suffix, like Ashen Moonsabre of Thorns. A synergy, or a legendary or
/// mythic part, can name the weapon instead, like Eclipse, World-Eater.
fn weapon_name(library: &Library, family: usize, components: &[&Component], synergies: &[ActiveSynergy], seed: u32) -> String {
    let mut rng = hash(seed ^ 0x6e41_3b5d);
    let fam = &library.families[family];
    let noun = &fam.nouns[(random(&mut rng) * fam.nouns.len() as f64) as usize % fam.nouns.len()];
    let mut by_rarity: Vec<&&Component> = components.iter().collect();
    by_rarity.sort_by_key(|c| std::cmp::Reverse(c.tier));

    // The first slot is the main part, the blade, limbs or shaft
    let root = components.first().and_then(|c| c.naming.root.clone()).or_else(|| by_rarity.iter().find_map(|c| c.naming.root.clone()));
    let core = match root {
        Some(root) => format!("{}{}", capitalized(&root), noun.to_lowercase()),
        None => capitalized(noun),
    };
    let suffix = by_rarity.iter().find_map(|c| c.naming.suffix.clone());
    let signature = synergies
        .iter()
        .find_map(|s| library.synergies[s.def].title.clone())
        .or_else(|| by_rarity.iter().filter(|c| c.tier >= 4).find_map(|c| c.naming.signature.clone()));
    if let Some(signature) = signature {
        return match (by_rarity.iter().find_map(|c| c.naming.epithet.clone()), suffix) {
            (Some(epithet), _) => format!("{signature}, {epithet}"),
            (None, Some(suffix)) => format!("{signature} {suffix}"),
            (None, None) => format!("{signature}, the {core}"),
        };
    }
    let prefix = by_rarity.iter().find_map(|c| c.naming.prefix.clone());
    [prefix, Some(core), suffix].into_iter().flatten().collect::<Vec<_>>().join(" ")
}

/// A weapon's model, in frames: one for swords and staves, three for a bow being drawn. Positions
/// are in meters in the weapon's own frame: its length runs up z from the pommel or foot, for a bow
/// arrows fly along x.
pub struct WeaponLook {
    /// Each frame's model and where its anchor is, see voxelize()
    pub frames: Vec<(Model, Vector3<f64>)>,
    /// Voxels per meter of the models
    pub res: f64,
    /// Where the hand holds it
    pub grip: Vector3<f64>,
    /// The tip of the blade, or the crystal spells come out of
    pub tip: Vector3<f64>,
    /// For bows, where the nock of the arrow is in each frame
    pub nocks: Vec<Vector3<f64>>,
}

// Voxels per meter of each kind of weapon. They're only ever held, right in front of the eye, so
// they're much finer than the world's: that way their voxels look about as big as the world's a
// few meters away.
const SWORD_RES: f64 = 192.0;
const BOW_RES: f64 = 160.0;
const STAFF_RES: f64 = 150.0;
/// How far a bow's string comes back drawn all the way, in meters
const DRAW_LENGTH: f64 = 0.24;

fn build_look(family: &Family, components: &[&Component]) -> WeaponLook {
    let visual = |slot: &str| components.iter().find(|c| c.slot_type == slot).map(|c| &c.visual_profile);
    let empty = Visual::default();
    let v = |slot: &str| visual(slot).unwrap_or(&empty);
    match family.kind {
        Kind::Melee => {
            let (parts, grip, tip) = sword(v("blade"), v("hilt"), v("guard"), v("core"), family);
            WeaponLook { frames: vec![voxelize(&parts, SWORD_RES)], res: SWORD_RES, grip, tip, nocks: Vec::new() }
        }
        Kind::Ranged => {
            let mut frames = Vec::new();
            let mut nocks = Vec::new();
            let mut grip = Vector3::zeros();
            for draw in [0.0, 0.5, 1.0] {
                let (parts, g, nock) = bow(v("limbs"), v("string"), v("mechanism"), v("core"), draw);
                frames.push(voxelize(&parts, BOW_RES));
                nocks.push(nock);
                grip = g;
            }
            WeaponLook { frames, res: BOW_RES, grip, tip: nocks[0], nocks }
        }
        Kind::Magic => {
            let (mut parts, top, mut tip) = staff(v("shaft"), v("focus"), v("crystal"), v("binding"));
            let mut grip = v3(0.0, 0.0, top * family.grip);
            // A wand is a staff in miniature
            let mut res = STAFF_RES;
            if family.haft > 0.0 {
                let s = family.haft / top;
                parts = parts.iter().map(|p| p.scaled(s)).collect();
                grip *= s;
                tip *= s;
                res = SWORD_RES;
            }
            WeaponLook { frames: vec![voxelize(&parts, res)], res, grip, tip, nocks: Vec::new() }
        }
    }
}

fn v3(x: f64, y: f64, z: f64) -> Vector3<f64> {
    Vector3::new(x, y, z)
}

/// A ring of little balls round `center`, in the plane of `u` and `w`
fn ring(p: &mut Vec<Part>, center: [f64; 3], u: [f64; 3], w: [f64; 3], radius: f64, thickness: f64, material: u8) {
    let count = ((TAU * radius / thickness) * 1.2).ceil().max(6.0) as usize;
    for k in 0..count {
        let a = k as f64 / count as f64 * TAU;
        let (c, s) = (a.cos() * radius, a.sin() * radius);
        p.push(ball([center[0] + u[0] * c + w[0] * s, center[1] + u[1] * c + w[1] * s, center[2] + u[2] * c + w[2] * s], thickness, material));
    }
}

/// A line of capsules through `points`, its radius going from `ra` to `rb`
fn curve(p: &mut Vec<Part>, points: &[[f64; 3]], ra: f64, rb: f64, material: u8) {
    let n = points.len().saturating_sub(1).max(1) as f64;
    for (k, pair) in points.windows(2).enumerate() {
        let (r0, r1) = (ra + (rb - ra) * k as f64 / n, ra + (rb - ra) * (k + 1) as f64 / n);
        p.push(limb(pair[0], pair[1], r0, r1, material));
    }
}

/// A sword, or an axe, hammer, dagger or spear, standing on its pommel, the flats of its blade
/// facing ±y and an axe's edge facing +x. Its family can give it a long haft, and leave off the
/// crossguard.
fn sword(blade: &Visual, hilt: &Visual, guard: &Visual, core: &Visual, family: &Family) -> (Vec<Part>, Vector3<f64>, Vector3<f64>) {
    let mut p = Vec::new();
    let pommel = hilt.pommel_size.unwrap_or(0.028);
    let grip_length = hilt.length.unwrap_or(0.2);
    let handle = grip_length.max(family.haft);
    let z_grip = pommel * 1.7;
    let z_top = z_grip + handle;
    let guard_height = if family.guard { 0.032 } else { 0.0 };
    let z_blade = z_top + guard_height;

    // The core's gem goes first, set into the middle of the guard so it shows on both sides, or
    // just under the head
    if let Some(gem) = core.gem.as_deref().and_then(material) {
        let z = if family.guard { z_top + guard_height * 0.5 } else { z_top - 0.05 };
        p.push(ball([0.0, 0.0, z], core.size.unwrap_or(0.02).max(0.016), gem));
    }
    if family.guard {
        sword_guard(&mut p, guard, z_top, guard_height);
    }
    let infuse = core.infuse.as_deref().and_then(material);
    let tip = match blade.shape.as_deref().unwrap_or("straight") {
        "axe" => axe_head(&mut p, blade, z_top, false),
        "double_axe" => axe_head(&mut p, blade, z_top, true),
        shape @ ("hammer" | "mace" | "spiked") => hammer_head(&mut p, blade, z_top, shape),
        "scythe" => scythe_head(&mut p, blade, z_top),
        "halberd" => halberd_head(&mut p, blade, z_top, infuse),
        _ => {
            // A spearhead sits in a socket on the end of its haft
            if !family.guard {
                p.push(limb([0.0, 0.0, z_top - 0.06], [0.0, 0.0, z_top + 0.02], 0.022, 0.014, DARK_STEEL));
            }
            sword_blade(&mut p, blade, z_blade, infuse)
        }
    };
    sword_hilt(&mut p, hilt, z_grip, grip_length, pommel, handle);
    (p, v3(0.0, 0.0, z_grip + handle * family.grip), tip)
}

/// An axe head on top of a haft ending at `z_top`, its edge facing +x, or both ways for a
/// double-bit one. Returns the middle of the edge.
fn axe_head(p: &mut Vec<Part>, v: &Visual, z_top: f64, double: bool) -> Vector3<f64> {
    let length = v.length.unwrap_or(0.26);
    let width = v.width.unwrap_or(0.2);
    let body = mat(&v.material, STEEL);
    let edge = mat(&v.edge, body);
    let voxel = 1.0 / SWORD_RES;
    let half_thick = 0.012;
    let z0 = z_top - length * 0.7;
    let rows = (length * SWORD_RES).ceil() as usize;
    let sides: &[f64] = if double { &[1.0, -1.0] } else { &[1.0] };
    let (mut edges, mut bodies) = (Vec::new(), Vec::new());
    for r in 0..rows {
        let t = (r as f64 + 0.5) / rows as f64;
        let (za, zb) = (z0 + r as f64 * length / rows as f64, z0 + (r + 1) as f64 * length / rows as f64);
        // Flaring out to a curved edge, reaching furthest in the middle
        let reach = width * (0.4 + 0.6 * (PI * t).sin().powf(0.6));
        for &s in sides {
            let (outer, inner) = (s * reach, s * (reach - 2.0 * voxel));
            edges.push(cuboid([outer.min(inner), -half_thick * 0.55, za], [outer.max(inner), half_thick * 0.55, zb], edge));
            let near = s * 0.02;
            bodies.push(cuboid([near.min(inner), -half_thick, za], [near.max(inner), half_thick, zb], body));
        }
    }
    p.extend(edges);
    p.extend(bodies);
    // The eye round the haft, and a knob behind it on a single-bit head
    p.push(cuboid([-0.024, -0.02, z0 + length * 0.15], [0.024, 0.02, z0 + length * 0.85], body));
    if !double {
        p.push(cuboid([-0.05, -half_thick * 1.6, z0 + length * 0.35], [-0.02, half_thick * 1.6, z0 + length * 0.65], body));
    }
    v3(width, 0.0, z0 + length * 0.5)
}

/// A scythe's blade out from the top of a haft ending at `z_top`: long, narrowing and curving down
/// to its point, its edge underneath, with a spike behind. Returns its point.
fn scythe_head(p: &mut Vec<Part>, v: &Visual, z_top: f64) -> Vector3<f64> {
    let length = v.length.unwrap_or(0.75);
    let depth = v.width.unwrap_or(0.1);
    let body = mat(&v.material, DARK_STEEL);
    let edge = mat(&v.edge, STEEL);
    let voxel = 1.0 / SWORD_RES;
    let half_thick = 0.007;
    let columns = (length * SWORD_RES).ceil() as usize;
    let droop = |t: f64| 0.3 * length * t * t;
    let (mut bodies, mut edges) = (Vec::new(), Vec::new());
    for c in 0..columns {
        let t = (c as f64 + 0.5) / columns as f64;
        let (xa, xb) = (0.015 + c as f64 * length / columns as f64, 0.015 + (c + 1) as f64 * length / columns as f64);
        let top = z_top - 0.02 - droop(t);
        let bottom = top - (depth * (1.0 - t).powf(0.7)).max(voxel);
        bodies.push(cuboid([xa, -half_thick, (bottom + 2.0 * voxel).min(top)], [xb, half_thick, top], body));
        edges.push(cuboid([xa, -half_thick * 0.55, bottom], [xb, half_thick * 0.55, top], edge));
    }
    p.extend(bodies);
    p.extend(edges);
    // The collar binding it to the haft, and a spike out the back
    p.push(cuboid([-0.03, -0.022, z_top - 0.07], [0.03, 0.022, z_top + 0.01], body));
    p.push(limb([-0.02, 0.0, z_top - 0.03], [-0.1, 0.0, z_top - 0.01], 0.012, 0.003, body));
    v3(length + 0.015, 0.0, z_top - 0.02 - droop(1.0))
}

/// A halberd's head on top of a haft ending at `z_top`: an axe blade facing +x, a hooked spike
/// behind, and a spear point on top. Returns the point.
fn halberd_head(p: &mut Vec<Part>, v: &Visual, z_top: f64, infuse: Option<u8>) -> Vector3<f64> {
    let body = mat(&v.material, STEEL);
    p.push(limb([-0.03, 0.0, z_top - 0.03], [-0.16, 0.0, z_top + 0.03], 0.014, 0.003, body));
    let axe = Visual { length: Some(0.24), width: v.width.or(Some(0.17)), material: v.material.clone(), edge: v.edge.clone(), ..Default::default() };
    axe_head(p, &axe, z_top, false);
    let point = Visual {
        shape: Some("spear".into()),
        length: v.length.or(Some(0.36)),
        width: Some(0.055),
        material: v.material.clone(),
        edge: v.edge.clone(),
        fuller: v.fuller.clone(),
        ..Default::default()
    };
    p.push(limb([0.0, 0.0, z_top + 0.04], [0.0, 0.0, z_top + 0.1], 0.02, 0.013, body));
    sword_blade(p, &point, z_top + 0.09, infuse)
}

/// A hammer head across the top of a haft ending at `z_top`: a block with faces at ±x, a round
/// knobbed mace, or a maul bristling with spikes. Returns the middle of its striking face.
fn hammer_head(p: &mut Vec<Part>, v: &Visual, z_top: f64, shape: &str) -> Vector3<f64> {
    let length = v.length.unwrap_or(0.16);
    let width = v.width.unwrap_or(0.26);
    let body = mat(&v.material, DARK_STEEL);
    let face = mat(&v.edge, body);
    let z0 = z_top - length * 0.6;
    let zc = z0 + length * 0.5;
    let (w, d) = (width * 0.5, length * 0.42);
    match shape {
        "mace" => {
            for k in 0..8 {
                let a = k as f64 / 8.0 * TAU;
                let z = zc + if k % 2 == 0 { 0.25 } else { -0.25 } * length;
                p.push(ball([a.cos() * w * 0.78, a.sin() * w * 0.78, z], w * 0.3, face));
            }
            p.push(ball([0.0, 0.0, zc], w * 0.72, body));
            v3(w, 0.0, zc)
        }
        _ => {
            if shape == "spiked" {
                for s in [-1.0, 1.0] {
                    for (dy, dz) in [(0.0, 0.0), (d * 0.6, length * 0.3), (-d * 0.6, -length * 0.3)] {
                        p.push(limb([s * w, dy, zc + dz], [s * (w + 0.07), dy, zc + dz], 0.016, 0.003, face));
                    }
                }
                p.push(limb([0.0, 0.0, z0 + length], [0.0, 0.0, z0 + length + 0.09], 0.018, 0.003, face));
            }
            // The striking faces, then the block between them
            for s in [-1.0, 1.0] {
                let (a, b) = (s * (w - 0.02), s * (w + 0.006));
                p.push(cuboid([a.min(b), -d - 0.006, z0 - 0.006], [a.max(b), d + 0.006, z0 + length + 0.006], face));
            }
            p.push(cuboid([-w, -d, z0], [w, d, z0 + length], body));
            v3(w, 0.0, zc)
        }
    }
}

/// Where the middle of a blade is across, and how wide it is either side, `t` of the way up it
fn blade_profile(shape: &str, t: f64, width: f64) -> (f64, f64) {
    let half = width * 0.5;
    // Narrowing to a point from `start` of the way up
    let point = |start: f64| if t > start { ((1.0 - t) / (1.0 - start)).max(0.0) } else { 1.0 };
    match shape {
        "broad" => (0.0, half * (1.0 - 0.08 * t) * point(0.88)),
        "jagged" => (0.0, half * (1.0 - 0.1 * t) * point(0.85)),
        "sabre" => (width * 1.3 * t * t, half * (1.0 - 0.2 * t) * point(0.8)),
        "cleaver" => (half * 0.25 * t, half * (0.7 + 0.5 * t) * point(0.9)),
        "flame" => (width * 0.2 * (t * TAU * 3.5).sin() * (1.0 - t), half * (1.0 - 0.15 * t) * point(0.85)),
        "needle" => (0.0, half * 0.6 * point(0.6)),
        // Leaf shaped, widest a third of the way up
        "spear" | "barbed" => (0.0, half * if t < 0.33 { 0.35 + 0.65 * t / 0.33 } else { ((1.0 - t) / 0.67).powf(0.8) }),
        // A broad, curving blade on a pole
        "glaive" => (width * 0.9 * t * t, half * (0.6 + 0.6 * t) * point(0.75)),
        // Gently curved all along, the same width nearly to its slanted point
        "katana" => (width * 0.7 * t * t, half * (1.0 - 0.12 * t) * point(0.93)),
        // A two-handed blade: unsharpened and narrow just above the guard, then lugs to parry with
        "zweihander" => (
            0.0,
            if t < 0.13 {
                half * 0.5
            } else if t < 0.16 {
                half * 1.55
            } else {
                half * (1.0 - 0.1 * t) * point(0.88)
            },
        ),
        _ => (0.0, half * (1.0 - 0.15 * t) * point(0.82)),
    }
}

fn sword_blade(p: &mut Vec<Part>, v: &Visual, z0: f64, infuse: Option<u8>) -> Vector3<f64> {
    let shape = v.shape.as_deref().unwrap_or("straight");
    let length = v.length.unwrap_or(0.8);
    let width = v.width.unwrap_or(0.065);
    let body = mat(&v.material, STEEL);
    let edge = mat(&v.edge, body);
    let fuller = v.fuller.as_deref().and_then(material).or(infuse);
    let voxel = 1.0 / SWORD_RES;
    let half_thick = 0.008;
    let bevel = 2.5 * voxel;
    let rows = (length * SWORD_RES).ceil() as usize;
    let (mut grooves, mut bodies, mut edges) = (Vec::new(), Vec::new(), Vec::new());
    for r in 0..rows {
        let t = (r as f64 + 0.5) / rows as f64;
        let (center, mut half) = blade_profile(shape, t, width);
        half = half.max(0.6 * voxel);
        // Teeth along the edges, or barbs at the foot of a spearhead
        if shape == "jagged" && r % 12 < 4 && t < 0.85 || shape == "barbed" && r % 10 < 3 && t < 0.4 {
            half += 3.0 * voxel;
        }
        let (za, zb) = (z0 + r as f64 * length / rows as f64, z0 + (r + 1) as f64 * length / rows as f64);
        // A groove down the middle of each side, glowing at the bottom if it's infused
        if t < 0.78 && half > 5.0 * voxel {
            let w = half * 0.22;
            grooves.push(cuboid([center - w, half_thick * 0.45, za], [center + w, half_thick * 1.5, zb], AIR));
            grooves.push(cuboid([center - w, -half_thick * 1.5, za], [center + w, -half_thick * 0.45, zb], AIR));
            if let Some(f) = fuller {
                grooves.push(cuboid([center - w, -half_thick * 0.45, za], [center + w, half_thick * 0.45, zb], f));
            }
        }
        if half > bevel {
            bodies.push(cuboid([center - half + bevel, -half_thick, za], [center + half - bevel, half_thick, zb], body));
        }
        // The edges are ground thinner. A blade only a voxel or so wide is all blade.
        let edge = if half > 1.5 * voxel { edge } else { body };
        edges.push(cuboid([center - half, -half_thick * 0.5, za], [center + half, half_thick * 0.5, zb], edge));
    }
    p.extend(grooves);
    p.extend(bodies);
    p.extend(edges);
    v3(blade_profile(shape, 1.0, width).0, 0.0, z0 + length)
}

fn sword_guard(p: &mut Vec<Part>, v: &Visual, z: f64, h: f64) {
    let w = v.width.unwrap_or(0.22);
    let m = mat(&v.material, STEEL);
    let accent = mat(&v.accent, m);
    let mid = z + h * 0.5;
    let block = cuboid([-0.028, -0.016, z], [0.028, 0.016, z + h], m);
    match v.shape.as_deref().unwrap_or("cross") {
        "bar" => p.push(cuboid([-w / 2.0, -0.02, z], [w / 2.0, 0.02, z + h], m)),
        "disc" => p.push(ellipsoid([0.0, 0.0, mid], [w / 2.0, w / 2.0, h / 2.0], m)),
        "swept" => {
            p.push(ball([w / 2.0, 0.0, z + h + 0.05], 0.013, accent));
            p.push(ball([-w / 2.0, 0.0, z - 0.05], 0.013, accent));
            p.push(limb([0.02, 0.0, mid], [w / 2.0, 0.0, z + h + 0.05], 0.012, 0.009, m));
            p.push(limb([-0.02, 0.0, mid], [-w / 2.0, 0.0, z - 0.05], 0.012, 0.009, m));
            p.push(block);
        }
        "horns" => {
            for s in [-1.0, 1.0] {
                p.push(limb([s * w * 0.4, 0.0, z + h * 0.8], [s * w * 0.5, 0.0, z + h + 0.08], 0.012, 0.004, accent));
                p.push(limb([s * 0.02, 0.0, mid], [s * w * 0.4, 0.0, z + h * 0.8], 0.016, 0.012, m));
            }
            p.push(block);
        }
        "claws" => {
            for i in 0..3 {
                let i = i as f64;
                for s in [-1.0, 1.0] {
                    p.push(limb([s * (0.025 + i * 0.03), 0.0, z + h], [s * (0.035 + i * 0.045), 0.0, z + h + 0.05 + 0.015 * i], 0.009, 0.003, accent));
                }
            }
            p.push(cuboid([-w / 2.0, -0.016, z], [w / 2.0, 0.016, z + h], m));
        }
        "wings" => {
            for i in 0..4 {
                let f = i as f64;
                for s in [-1.0, 1.0] {
                    let material = if i % 2 == 0 { m } else { accent };
                    p.push(limb([s * 0.02, 0.0, mid], [s * w * 0.5 * (1.0 - 0.18 * f), 0.0, z + 0.01 + 0.035 * f], 0.012, 0.006, material).stretched([1.0, 0.7, 1.0]));
                }
            }
            p.push(block);
        }
        _ => {
            p.push(ball([w / 2.0, 0.0, mid], 0.017, accent));
            p.push(ball([-w / 2.0, 0.0, mid], 0.017, accent));
            p.push(cuboid([-w / 2.0, -0.014, z], [w / 2.0, 0.014, z + h], m));
        }
    }
}

/// The grip, `length` long, and on up to `handle` the wooden haft of an axe, hammer or spear
fn sword_hilt(p: &mut Vec<Part>, v: &Visual, z_grip: f64, length: f64, pommel: f64, handle: f64) {
    let m = mat(&v.material, LEATHER);
    let pm = mat(&v.pommel_material, STEEL);
    let r = 0.016;
    if let Some(wrap) = v.wrap.as_deref().and_then(material) {
        // Bands of it every other row of voxels
        let row = (1.0 / SWORD_RES).max(0.012);
        for k in (0..(length / row) as usize).step_by(2) {
            let z = z_grip + k as f64 * row;
            p.push(limb([0.0, 0.0, z + row * 0.1], [0.0, 0.0, z + row * 0.9], r * 1.2, r * 1.2, wrap));
        }
    }
    p.push(limb([0.0, 0.0, z_grip], [0.0, 0.0, z_grip + length], r, r * 1.05, m));
    if handle > length + 0.01 {
        p.push(ellipsoid([0.0, 0.0, z_grip + length], [r * 1.25, r * 1.25, 0.008], DARK_STEEL));
        p.push(limb([0.0, 0.0, z_grip + length], [0.0, 0.0, z_grip + handle], r * 0.95, r * 0.85, WOOD));
    }
    let c = pommel * 0.85;
    match v.pommel.as_deref().unwrap_or("ball") {
        "ring" => {
            ring(p, [0.0, 0.0, c], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], pommel * 0.75, pommel * 0.32, pm);
            p.push(limb([0.0, 0.0, c + pommel * 0.75], [0.0, 0.0, z_grip], r * 0.7, r, pm));
        }
        "spike" => p.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, z_grip], 0.004, pommel * 0.75, pm)),
        "gem" => {
            p.push(ball([0.0, 0.0, c], pommel * 0.75, pm));
            p.push(limb([0.0, 0.0, c], [0.0, 0.0, z_grip], pommel * 0.6, r, STEEL));
        }
        "skull" => {
            for y in [-1.0, 1.0] {
                p.push(ball([pommel * 0.8, y * pommel * 0.4, c * 1.15], pommel * 0.3, EYE));
            }
            p.push(ellipsoid([0.0, 0.0, c], [pommel * 1.05, pommel * 0.95, pommel * 0.95], pm));
        }
        _ => {
            p.push(ball([0.0, 0.0, c], pommel * 0.85, pm));
            p.push(limb([0.0, 0.0, c], [0.0, 0.0, z_grip], r * 0.8, r, pm));
        }
    }
}

/// A bow standing up, shooting along +x, its string drawn back `draw` of the way. Returns where
/// it's held and where the arrow's nock sits.
fn bow(limbs: &Visual, string: &Visual, mechanism: &Visual, core: &Visual, draw: f64) -> (Vec<Part>, Vector3<f64>, Vector3<f64>) {
    let mut p = Vec::new();
    let length = limbs.length.unwrap_or(1.25);
    let mid = length * 0.5;
    let wood = mat(&limbs.material, WOOD);
    let shape = limbs.shape.as_deref().unwrap_or("simple").to_owned();
    // How far back the limbs bend at `s`, from -1 at the bottom tip to 1 at the top, bending more drawn
    let bend = |s: f64| {
        let tip = ((s.abs() - 0.7) / 0.3).max(0.0);
        let x = match shape.as_str() {
            "long" => -0.07 * s * s,
            "recurve" => -0.12 * s * s + 0.09 * tip * tip,
            "horn" => -0.09 * s * s + 0.06 * tip * tip,
            "branch" => -0.1 * s * s + 0.008 * (s * 9.0).sin(),
            _ => -0.11 * s * s,
        };
        x * (1.0 + 0.6 * draw)
    };
    let at = |s: f64| [bend(s), 0.0, mid + s * mid];

    // Where the arrow sits, and the string pulled back to it
    let rest = [0.0, 0.028, mid + 0.03];
    let nock = [bend(1.0) - draw * DRAW_LENGTH, 0.0, mid + 0.03];
    let cord = mat(&string.material, BOWSTRING);
    p.push(limb(at(1.0), nock, 0.0065, 0.0065, cord));
    p.push(limb(at(-1.0), nock, 0.0065, 0.0065, cord));

    if let Some(gem) = core.gem.as_deref().and_then(material) {
        p.push(ball([0.018, 0.0, mid + 0.09], core.size.unwrap_or(0.022).max(0.014), gem));
    }
    let mm = mat(&mechanism.material, WOOD);
    let accent = mat(&mechanism.accent, mm);
    match mechanism.shape.as_deref().unwrap_or("rest") {
        "twin" => {
            for dz in [0.0, 0.05] {
                p.push(cuboid([-0.012, 0.016, rest[2] - 0.016 + dz], [0.026, 0.042, rest[2] - 0.006 + dz], mm));
            }
            p.push(ball([0.024, 0.0, rest[2] + 0.025], 0.01, accent));
        }
        "ring" => {
            ring(&mut p, [0.03, 0.03, mid + 0.1], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 0.02, 0.0075, mm);
            p.push(cuboid([-0.012, 0.016, rest[2] - 0.016], [0.026, 0.042, rest[2] - 0.006], DARK_STEEL));
        }
        "plate" => {
            p.push(ball([0.03, 0.0, mid - 0.02], 0.01, accent));
            p.push(cuboid([0.016, -0.026, mid - 0.06], [0.03, 0.026, mid + 0.13], mm));
            p.push(cuboid([-0.012, 0.016, rest[2] - 0.016], [0.026, 0.042, rest[2] - 0.006], mm));
        }
        "spines" => {
            for k in 0..3 {
                let z = mid + 0.1 + k as f64 * 0.05;
                p.push(limb([0.02, 0.0, z], [0.09, 0.0, z + 0.04], 0.009, 0.003, accent));
                p.push(limb([0.02, 0.0, mid - z + mid], [0.09, 0.0, mid - z + mid - 0.04], 0.009, 0.003, accent));
            }
            p.push(cuboid([-0.012, 0.016, rest[2] - 0.016], [0.026, 0.042, rest[2] - 0.006], mm));
        }
        _ => p.push(cuboid([-0.012, 0.016, rest[2] - 0.016], [0.026, 0.042, rest[2] - 0.006], mm)),
    }

    if let Some(tips) = limbs.tips.as_deref().and_then(material) {
        for s in [-1.0, 1.0] {
            p.push(ball(at(s), 0.015, tips));
        }
    }
    // The grip, wrapped in leather, and the riser
    p.push(limb([-0.003, 0.0, mid - 0.07], [-0.003, 0.0, mid + 0.06], 0.024, 0.024, LEATHER));
    p.push(limb(at(-0.2), at(0.2), 0.02, 0.02, wood));
    let steps = 14;
    for half in [-1.0, 1.0] {
        let points: Vec<[f64; 3]> = (0..=steps).map(|k| at(half * (0.18 + 0.82 * k as f64 / steps as f64))).collect();
        if shape == "branch" {
            for k in [4, 9] {
                p.push(ball(points[k], 0.02, BARK_DARK));
            }
        }
        if shape == "crystal" {
            for k in [5, 10] {
                let q = points[k];
                p.push(limb(q, [q[0] - 0.04, q[1], q[2] + half * 0.04], 0.012, 0.003, mat(&limbs.tips, CRYSTAL)));
            }
        }
        for pair in points.windows(2) {
            let k = (pair[0][2] - mid).abs() / mid;
            p.push(limb(pair[0], pair[1], 0.017 - 0.008 * k, 0.017 - 0.008 * k, wood).stretched([0.7, 1.35, 1.0]));
        }
    }
    (p, v3(0.0, 0.0, mid), v3(nock[0], 0.0, nock[2]))
}

/// A staff standing on its foot, the crystal at its top. Returns how long its shaft is, and where
/// the crystal is.
fn staff(shaft: &Visual, focus: &Visual, crystal: &Visual, binding: &Visual) -> (Vec<Part>, f64, Vector3<f64>) {
    let mut p = Vec::new();
    let length = shaft.length.unwrap_or(1.55);
    let thick = shaft.thickness.unwrap_or(0.024);
    let wood = mat(&shaft.material, WOOD);
    let accent = mat(&shaft.accent, wood);
    let fm = mat(&focus.material, wood);
    let focus_accent = mat(&focus.accent, fm);
    let focus_shape = focus.shape.as_deref().unwrap_or("crook");
    let gem = mat(&crystal.gem, CRYSTAL);
    let size = crystal.size.unwrap_or(0.05);
    let top = length;

    // Where the crystal sits depends on what holds it
    let c = match focus_shape {
        "crook" => [0.07, 0.0, top + 0.12],
        "skull" => [0.0, 0.0, top + 0.21],
        _ => [0.0, 0.0, top + 0.12],
    };

    // The crystal first, so whatever holds it doesn't hide it
    match crystal.shape.as_deref().unwrap_or("orb") {
        "shard" => {
            p.push(limb([c[0], c[1], c[2] - size * 1.3], c, 0.004, size * 0.7, gem));
            p.push(limb(c, [c[0], c[1], c[2] + size * 1.6], size * 0.7, 0.004, gem));
        }
        "cluster" => {
            for (dx, dy, lean) in [(0.0, 0.0, 0.0), (0.6, 0.0, 0.5), (-0.4, 0.5, 0.5), (-0.4, -0.5, 0.5)] {
                let s = if lean > 0.0 { size * 0.6 } else { size };
                let base = [c[0] + dx * size * 0.4, c[1] + dy * size * 0.4, c[2] - size];
                let end = [c[0] + dx * size * (0.4 + lean * 1.2), c[1] + dy * size * (0.4 + lean * 1.2), c[2] + s * 1.4];
                p.push(limb(base, end, s * 0.55, 0.004, gem));
            }
        }
        _ => p.push(ball(c, size, gem)),
    }

    match focus_shape {
        "ring" => {
            ring(&mut p, c, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.085, 0.012, fm);
            p.push(limb([0.0, 0.0, top - 0.02], [0.0, 0.0, c[2] - 0.08], thick, thick * 0.8, fm));
        }
        "prongs" => {
            for k in 0..3 {
                let a = k as f64 / 3.0 * TAU;
                let (x, y) = (a.cos(), a.sin());
                curve(&mut p, &[[0.0, 0.0, top], [x * 0.055, y * 0.055, top + 0.1], [x * 0.03, y * 0.03, top + 0.22]], 0.012, 0.004, fm);
            }
        }
        "cage" => {
            for k in 0..4 {
                let a = (k as f64 + 0.5) / 4.0 * TAU;
                let (x, y) = (a.cos(), a.sin());
                curve(&mut p, &[[0.0, 0.0, top], [x * 0.05, y * 0.05, top + 0.05], [x * 0.068, y * 0.068, top + 0.12], [x * 0.045, y * 0.045, top + 0.2], [0.0, 0.0, top + 0.245]], 0.01, 0.008, if k % 2 == 0 { fm } else { focus_accent });
            }
            p.push(ball([0.0, 0.0, top + 0.255], 0.016, focus_accent));
        }
        "antlers" => {
            for s in [-1.0, 1.0] {
                curve(&mut p, &[[0.0, 0.0, top], [s * 0.1, 0.0, top + 0.12], [s * 0.13, 0.0, top + 0.27]], 0.014, 0.005, fm);
                p.push(limb([s * 0.07, 0.0, top + 0.09], [s * 0.04, 0.02, top + 0.21], 0.009, 0.004, fm));
                p.push(limb([s * 0.115, 0.0, top + 0.18], [s * 0.18, -0.01, top + 0.23], 0.008, 0.004, fm));
            }
        }
        "crescent" => {
            let points: Vec<[f64; 3]> = (0..=18)
                .map(|k| {
                    let a = (-50.0 + 280.0 * k as f64 / 18.0).to_radians() - PI * 0.5;
                    [c[0] + a.cos() * 0.1, 0.0, c[2] + a.sin() * 0.1]
                })
                .collect();
            for (k, pair) in points.windows(2).enumerate() {
                let edge = (k as f64 / 17.0 - 0.5).abs() * 2.0;
                p.push(limb(pair[0], pair[1], 0.016 * (1.0 - 0.7 * edge), 0.016 * (1.0 - 0.7 * edge), fm).stretched([1.0, 0.6, 1.0]));
            }
        }
        "orrery" => {
            ring(&mut p, c, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.09, 0.009, fm);
            ring(&mut p, c, [0.0, 1.0, 0.0], [0.5, 0.0, 0.86], 0.072, 0.008, fm);
            for (dx, dz) in [(0.09, 0.0), (-0.045, 0.078), (0.0, -0.09)] {
                p.push(ball([c[0] + dx, 0.0, c[2] + dz], 0.016, focus_accent));
            }
            p.push(limb([0.0, 0.0, top - 0.02], [0.0, 0.0, c[2] - 0.09], thick, thick * 0.8, fm));
        }
        "skull" => {
            for y in [-1.0, 1.0] {
                p.push(ball([0.045, y * 0.023, top + 0.09], 0.016, gem));
            }
            p.push(ball([0.055, 0.0, top + 0.055], 0.01, EYE));
            p.push(ellipsoid([0.0, 0.0, top + 0.08], [0.062, 0.056, 0.066], fm));
        }
        "spire" => {
            for k in 0..4 {
                let a = (k as f64 + 0.5) / 4.0 * TAU;
                p.push(limb([a.cos() * 0.045, a.sin() * 0.045, top + 0.02], [0.0, 0.0, top + 0.38], 0.011, 0.003, fm));
            }
            p.push(limb([0.0, 0.0, top - 0.02], [0.0, 0.0, top + 0.04], thick * 2.0, thick * 1.6, fm));
        }
        _ => {
            // A crook, hooked over the crystal
            let hook: Vec<[f64; 3]> = (0..=10)
                .map(|k| {
                    let a = PI - (PI + 0.7) * k as f64 / 10.0;
                    [0.06 + 0.06 * a.cos(), 0.0, top + 0.17 + 0.07 * a.sin()]
                })
                .collect();
            curve(&mut p, &hook, thick * 0.95, thick * 0.6, fm);
            p.push(limb([0.0, 0.0, top - 0.02], [0.0, 0.0, top + 0.17], thick, thick * 0.95, fm));
        }
    }

    let bm = mat(&binding.material, LINEN);
    let ba = mat(&binding.accent, bm);
    let (low, high) = (top - 0.36, top - 0.05);
    match binding.shape.as_deref().unwrap_or("wraps") {
        "bands" => {
            for z in [low + 0.04, (low + high) * 0.5, high - 0.02] {
                p.push(ellipsoid([0.0, 0.0, z], [thick * 1.45, thick * 1.45, 0.012], bm));
            }
        }
        "chain" => {
            p.push(ellipsoid([0.0, 0.0, high], [thick * 1.4, thick * 1.4, 0.01], bm));
            for k in 0..9 {
                let t = k as f64 / 8.0;
                let material = if k % 2 == 0 { bm } else { ba };
                p.push(ball([thick * (1.3 + 1.2 * t), 0.0, high - 0.02 - t * 0.28], 0.0085, material));
            }
        }
        "ribbons" => {
            for s in [-1.0, 1.0] {
                let end = [s * 0.05, s * 0.025, high - 0.42];
                p.push(ball(end, 0.012, ba));
                p.push(limb([s * thick, 0.0, high], end, 0.014, 0.012, bm).stretched([1.0, 0.45, 1.0]));
            }
            p.push(ellipsoid([0.0, 0.0, high], [thick * 1.4, thick * 1.4, 0.012], bm));
        }
        "feathers" => {
            p.push(limb([thick, 0.0, high], [0.035, 0.0, high - 0.16], 0.005, 0.005, LEATHER));
            for (k, dy) in [-0.012, 0.0, 0.012].into_iter().enumerate() {
                let z = high - 0.2 - 0.02 * k as f64;
                p.push(ellipsoid([0.04, dy, z], [0.012, 0.008, 0.055], bm));
            }
            p.push(ellipsoid([0.0, 0.0, high], [thick * 1.35, thick * 1.35, 0.01], LEATHER));
        }
        "runes" => {
            for (k, z) in [low + 0.05, (low + high) * 0.5, high - 0.03].into_iter().enumerate() {
                let material = if k % 2 == 0 { bm } else { ba };
                p.push(ellipsoid([0.0, 0.0, z], [thick * 1.25, thick * 1.25, 0.006], material));
                for a in [0.0f64, 2.1, 4.2] {
                    p.push(ball([a.cos() * thick * 1.05, a.sin() * thick * 1.05, z + 0.045], 0.0075, material));
                }
            }
        }
        _ => {
            let bands = ((high - low) / 0.026) as usize;
            for k in 0..bands {
                p.push(ellipsoid([0.0, 0.0, low + (k as f64 + 0.5) * 0.026], [thick * 1.3, thick * 1.3, 0.008], if k % 2 == 0 { bm } else { ba }));
            }
        }
    }

    // A metal foot, then the shaft
    p.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, 0.06], thick * 0.65, thick * 1.1, DARK_STEEL));
    match shaft.shape.as_deref().unwrap_or("straight") {
        "twisted" => {
            for strand in 0..2 {
                let offset = strand as f64 * PI;
                let points: Vec<[f64; 3]> = (0..=60)
                    .map(|k| {
                        let z = top * k as f64 / 60.0;
                        let a = z * 14.0 + offset;
                        [a.cos() * thick * 0.5, a.sin() * thick * 0.5, z]
                    })
                    .collect();
                curve(&mut p, &points, thick * 0.6, thick * 0.6, if strand == 0 { wood } else { accent });
            }
        }
        "gnarled" => {
            let wobble = |z: f64| [thick * 0.6 * (z * 7.3).sin(), thick * 0.6 * (z * 5.1).cos(), z];
            for k in 1..(top / 0.27) as usize {
                p.push(ball(wobble(k as f64 * 0.27), thick * 1.35, wood));
            }
            if accent == LEAVES {
                for k in 0..5 {
                    let q = wobble(top - 0.05 - k as f64 * 0.07);
                    let a = k as f64 * 2.4;
                    p.push(ellipsoid([q[0] + a.cos() * thick * 1.6, q[1] + a.sin() * thick * 1.6, q[2]], [0.022, 0.022, 0.01], LEAVES));
                }
            }
            let points: Vec<[f64; 3]> = (0..=30).map(|k| wobble(top * k as f64 / 30.0)).collect();
            curve(&mut p, &points, thick * 0.85, thick, wood);
        }
        shape => {
            if accent != wood {
                let every = if shape == "segmented" { 0.22 } else { 0.4 };
                for k in 1..(top / every) as usize {
                    p.push(ellipsoid([0.0, 0.0, k as f64 * every], [thick * 1.3, thick * 1.3, 0.011], accent));
                }
            }
            p.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, top], thick * 0.85, thick, wood));
        }
    }
    (p, top, v3(c[0], c[1], c[2]))
}

/// How many kinds of arrowhead there are: broad, a narrow bodkin point, and barbed
pub const ARROW_HEADS: usize = 3;

// Where each kind of model starts among effect_models(), counted in sets of ELEMENT_LOOKS: one
// without an element, then one for each element
/// Arrows in flight, a set for each arrowhead
pub const SLOT_ARROW: usize = 0;
/// Spells in flight
pub const SLOT_BOLT: usize = ARROW_HEADS;
pub const SLOT_SPARK: usize = SLOT_BOLT + 1;
/// The arrow on the string, finer, a set for each arrowhead
pub const SLOT_HELD_ARROW: usize = SLOT_SPARK + 1;
/// Fireballs, a meter across
pub const SLOT_FIREBALL: usize = SLOT_HELD_ARROW + ARROW_HEADS;
/// Tiny sparks for trails and auras
pub const SLOT_FINE: usize = SLOT_FIREBALL + 1;
/// Drops of something splashed, which don't glow: see DROP_WATER and the rest for which is which
pub const SLOT_DROP: usize = SLOT_FINE + 1;
pub const DROP_WATER: usize = 0;
pub const DROP_BLOOD: usize = 1;
pub const DROP_VENOM: usize = 2;
pub const DROP_MUD: usize = 3;
pub const DROP_ICE: usize = 4;
pub const DROP_ASH: usize = 5;
pub const DROP_GOLD: usize = 6;
pub const DROP_VOID: usize = 7;
pub const DROP_EMBER: usize = 8;
/// After the sets: the right arm, the left arm, a boulder, then spikes of ice bursting up
pub const ARMS: usize = (SLOT_DROP + 1) * ELEMENT_LOOKS;
pub const BOULDER: usize = ARMS + 2;
pub const ICE_BURST: usize = BOULDER + 1;
/// A pillar of light, standing on 0
pub const PILLAR: usize = ICE_BURST + 1;
/// Roots bursting up out of the ground, standing on 0
pub const ROOTS: usize = PILLAR + 1;
/// A thorn as long as an arrow, along +x with its point where an arrow's is
pub const THORN: usize = ROOTS + 1;
/// A glowing red ring a meter round, flat round 0, to show where something's coming down
pub const RING: usize = THORN + 1;
/// A ball of web, strands trailing off it, its middle at 0
pub const WEB: usize = RING + 1;

/// An arrow along +x with its nock at 0, at `res` voxels per meter: with arrowhead `head` (see
/// ARROW_HEADS), and its element showing in the head and what's round it, the fletching and the shaft
fn arrow(element: Option<Element>, head: usize, res: f64) -> Vec<Part> {
    use Element::*;
    let voxel = 1.0 / res;
    let (body, edge) = match element {
        None => (STEEL, STEEL),
        Some(Fire) => (DARK_STEEL, GLOW_FIRE),
        Some(Frost) => (ICE, GLOW_FROST),
        Some(Storm) => (BRONZE, GLOW_STORM),
        Some(Poison) => (DARK_STEEL, GLOW_POISON),
        Some(Arcane) => (CRYSTAL, GLOW_ARCANE),
        Some(Void) => (OBSIDIAN, GLOW_VOID),
        Some(Blood) => (DARK_STEEL, GLOW_BLOOD),
        Some(Holy) => (GOLD, GLOW_HOLY),
    };
    let (feathers, shaft) = match element {
        None => (FUR_WHITE, WOOD),
        Some(Fire) | Some(Blood) => (CLOTH, if element == Some(Fire) { WOOD } else { BARK_DARK }),
        Some(Frost) => (FUR_WHITE, BIRCH_BARK),
        Some(Storm) => (FUR_GRAY, WOOD),
        Some(Poison) => (MOSS, BARK_DARK),
        Some(Arcane) => (FLOWER_PURPLE, BIRCH_BARK),
        Some(Void) => (OBSIDIAN, BARK_DARK),
        Some(Holy) => (LINEN, BIRCH_BARK),
    };
    let start = 0.6;
    let mut p = Vec::new();
    let band = |p: &mut Vec<Part>, x: f64, material: u8| p.push(limb([x, 0.0, 0.0], [x + 0.006, 0.0, 0.0], 0.0075, 0.0075, material));
    // What's round the head goes first, as earlier shapes win
    match element {
        Some(Fire) => {
            // Oiled cloth wrapped round behind the head, burning
            p.push(limb([start - 0.028, 0.0, 0.01], [start - 0.04, 0.0, 0.03], 0.007, 0.001, FLAME));
            p.push(ball([start - 0.024, 0.006, 0.006], 0.008, GLOW_FIRE));
            p.push(ellipsoid([start - 0.024, 0.0, 0.0], [0.022, 0.014, 0.014], CLOTH));
        }
        Some(Frost) => {
            for a in [0.3, 2.4, 4.5] {
                let (c, s) = (f64::cos(a), f64::sin(a));
                p.push(limb([start - 0.01, 0.0, 0.0], [start - 0.035, c * 0.016, s * 0.016], 0.004, 0.001, ICE));
            }
            band(&mut p, start - 0.05, GLOW_FROST);
        }
        Some(Storm) => {
            // Copper coils that crackle
            for k in 0..3 {
                band(&mut p, start - 0.02 - k as f64 * 0.016, if k == 1 { BRONZE } else { GLOW_STORM });
            }
        }
        Some(Poison) => {
            p.push(ball([start + 0.025, 0.0, -0.012], 0.004, GLOW_POISON));
            band(&mut p, start - 0.015, GLOW_POISON);
        }
        Some(Arcane) => {
            // Motes circling the shaft
            for k in 0..6 {
                let a = k as f64 / 6.0 * TAU;
                p.push(ball([start - 0.03, a.cos() * 0.014, a.sin() * 0.014], 0.0025, GLOW_ARCANE));
            }
        }
        Some(Void) => band(&mut p, start - 0.012, GLOW_VOID),
        Some(Blood) => band(&mut p, start - 0.012, GLOW_BLOOD),
        Some(Holy) => {
            // A halo round the shaft by the feathers
            for k in 0..8 {
                let a = k as f64 / 8.0 * TAU;
                p.push(ball([0.14, a.cos() * 0.016, a.sin() * 0.016], 0.003, GLOW_HOLY));
            }
        }
        None => {}
    }

    // The head, row by row from its base to its point, the body inside its edges
    let (length, width) = match head {
        0 => (0.085, 0.022),
        1 => (0.11, 0.0075),
        _ => (0.09, 0.017),
    };
    let rows = ((length * res).ceil() as usize).max(1);
    let thick = 0.003;
    let (mut bodies, mut edges) = (Vec::new(), Vec::new());
    for r in 0..rows {
        let t = (r as f64 + 0.5) / rows as f64;
        let (xa, xb) = (start + r as f64 * length / rows as f64, start + (r + 1) as f64 * length / rows as f64);
        let mut half = match head {
            0 => width * (1.0 - t).powf(0.75),
            1 => width * (1.0 - t * 0.85),
            _ => width * (1.0 - t),
        };
        // A blood arrow's edges are serrated
        if element == Some(Blood) && r % 3 == 0 {
            half *= 1.3;
        }
        let half = half.max(0.5 * voxel);
        if head == 1 {
            bodies.push(cuboid([xa, -half, -half], [xb, half, half], body));
            edges.push(cuboid([xa, -half * 1.2, -half * 1.2], [xb, half * 1.2, half * 1.2], edge));
        } else {
            let inner = (half - 1.5 * voxel).max(0.0);
            if inner > 0.0 {
                bodies.push(cuboid([xa, -inner, -thick], [xb, inner, thick], body));
            }
            edges.push(cuboid([xa, -half, -thick * 0.6], [xb, half, thick * 0.6], edge));
        }
    }
    p.extend(bodies);
    p.extend(edges);
    if head == 2 {
        // Barbs curving back from the base
        for s in [-1.0, 1.0] {
            p.push(limb([start + 0.012, s * width * 0.9, 0.0], [start - 0.022, s * width * 1.5, 0.0], 0.0035, 0.0015, body));
        }
    }
    // A socket the head sits in
    p.push(limb([start - 0.006, 0.0, 0.0], [start + 0.006, 0.0, 0.0], 0.006, 0.005, body));

    // Three feathers by the nock, then the nock and the shaft
    for a in [0.0, TAU / 3.0, 2.0 * TAU / 3.0] {
        let (c, s) = (f64::cos(a), f64::sin(a));
        p.push(limb([0.025, c * 0.007, s * 0.007], [0.115, c * 0.019, s * 0.019], 0.006, 0.0035, feathers));
    }
    p.push(limb([0.0, 0.0, 0.0], [0.012, 0.0, 0.0], 0.0065, 0.0065, if element == Some(Holy) { GOLD } else { BARK_DARK }));
    p.push(limb([0.0, 0.0, 0.0], [start, 0.0, 0.0], 0.0045, 0.0045, shaft));
    p
}

/// The arrow, shot spell and spark models, in sets of ELEMENT_LOOKS: without an element, then
/// one for each element, with their anchors. See the SLOT_ constants for which set is where. Arrows
/// and spells fly along +x, arrows with their nock at 0. Then the right arm and the left, see
/// arm(), and a boulder for a golem to throw.
pub fn effect_models() -> Vec<(Model, Vector3<f64>)> {
    let looks: Vec<Option<Element>> = std::iter::once(None).chain(Element::ALL.map(Some)).collect();
    let mut models = Vec::new();
    for head in 0..ARROW_HEADS {
        for &element in &looks {
            models.push(voxelize(&arrow(element, head, ARROW_RES), ARROW_RES));
        }
    }
    for &element in &looks {
        let glow = element.map_or(CRYSTAL, Element::glow);
        let p = vec![ball([0.0, 0.0, 0.0], 0.07, glow), limb([0.0, 0.0, 0.0], [-0.28, 0.0, 0.0], 0.055, 0.012, glow)];
        models.push(voxelize(&p, BOLT_RES));
    }
    for &element in &looks {
        let glow = element.map_or(GLOW_HOLY, Element::glow);
        models.push(voxelize(&[ball([0.0, 0.0, 0.0], 0.03, glow)], SPARK_RES));
    }
    // The arrow on the string, held up close
    for head in 0..ARROW_HEADS {
        for &element in &looks {
            models.push(voxelize(&arrow(element, head, HELD_ARROW_RES), HELD_ARROW_RES));
        }
    }
    // Fireballs, a meter across: glowing balls with bites out of them, so they look torn apart
    for (k, &element) in looks.iter().enumerate() {
        let glow = element.map_or(GLOW_FIRE, Element::glow);
        let mut seed = 0x0f1e + k as u32;
        let mut p = Vec::new();
        for _ in 0..14 {
            let d = [random(&mut seed) * 2.0 - 1.0, random(&mut seed) * 2.0 - 1.0, random(&mut seed) * 2.0 - 1.0];
            let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(0.1);
            p.push(ball(d.map(|v| v / n * 0.95), 0.25 + 0.2 * random(&mut seed), AIR));
        }
        p.push(ball([0.0, 0.0, 0.0], 0.45, GLOW_HOLY));
        p.push(ball([0.0, 0.0, 0.0], 0.75, glow));
        p.push(ball([0.0, 0.0, 0.0], 1.0, element.map_or(GLOW_BLOOD, |_| glow)));
        models.push(voxelize(&p, MODEL_RES));
    }
    // Fine sparks, for trails and auras off the weapon in hand
    for &element in &looks {
        let glow = element.map_or(GLOW_HOLY, Element::glow);
        models.push(voxelize(&[ball([0.0, 0.0, 0.0], 0.008, glow)], FINE_SPARK_RES));
    }
    // Drops, in the order of DROP_WATER and the rest: a voxel each
    for material in [FOAM, CLOTH, GLOW_POISON, DIRT, ICE, BARK_DARK, GOLD, OBSIDIAN, GLOW_FIRE] {
        models.push(voxelize(&[ball([0.0, 0.0, 0.0], 0.03, material)], MODEL_RES));
    }
    models.push(voxelize(&arm(false), ARM_RES));
    models.push(voxelize(&arm(true), ARM_RES));
    // A lumpy boulder, half a meter round
    let mut seed = 0xb01d;
    let mut p = Vec::new();
    for k in 0..7 {
        let d = [random(&mut seed) * 2.0 - 1.0, random(&mut seed) * 2.0 - 1.0, random(&mut seed) * 2.0 - 1.0];
        let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(0.1);
        let material = [GRAVEL, ROCK, MOSS][k % 3];
        p.push(ball(d.map(|v| v / n * 0.3), 0.14 + 0.1 * random(&mut seed), material));
    }
    p.push(ball([0.0, 0.0, 0.0], 0.38, ROCK));
    models.push(voxelize(&p, MODEL_RES));
    // Spikes of ice bursting up and out from a glowing heart, a meter across, its middle at 0
    let mut seed = 0x1ce5;
    let mut p = vec![ball([0.0, 0.0, 0.0], 0.18, GLOW_FROST)];
    for k in 0..11 {
        let a = k as f64 / 11.0 * TAU + random(&mut seed) * 0.4;
        let up = 0.25 + 0.75 * random(&mut seed);
        let out = (1.0 - up * up).sqrt();
        let length = 0.55 + 0.45 * random(&mut seed);
        let tip = [a.cos() * out * length, a.sin() * out * length, up * length];
        p.push(limb([0.0, 0.0, 0.0], tip, 0.11, 0.015, if k % 3 == 0 { CRYSTAL } else { ICE }));
    }
    models.push(voxelize(&p, MODEL_RES));
    // A pillar of light six meters high, brightest in its heart, flaring at its foot
    let p = vec![limb([0.0, 0.0, 0.0], [0.0, 0.0, 6.0], 0.3, 0.2, GLOW_HOLY), ellipsoid([0.0, 0.0, 0.1], [0.7, 0.7, 0.12], GLOW_HOLY)];
    models.push(voxelize(&p, MODEL_RES));
    // Roots bursting up out of a mound of earth, a meter across, crooked and mossy
    let mut seed = 0x2007;
    let mut p = vec![ellipsoid([0.0, 0.0, 0.0], [0.45, 0.45, 0.15], DIRT)];
    for k in 0..7 {
        let a = k as f64 / 7.0 * TAU + random(&mut seed) * 0.5;
        let lean = 0.25 + 0.3 * random(&mut seed);
        let length = 0.7 + 0.5 * random(&mut seed);
        let base = [a.cos() * 0.2, a.sin() * 0.2, 0.0];
        let knee = [base[0] + a.cos() * lean * 0.5, base[1] + a.sin() * lean * 0.5, length * 0.55];
        let tip = [knee[0] + a.cos() * lean * 0.6 - a.sin() * 0.15, knee[1] + a.sin() * lean * 0.6 + a.cos() * 0.15, length];
        let bark = if k % 2 == 0 { BARK_DARK } else { WOOD };
        p.push(limb(base, knee, 0.1, 0.07, bark));
        p.push(limb(knee, tip, 0.07, 0.015, if k % 3 == 0 { MOSS } else { bark }));
    }
    models.push(voxelize(&p, MODEL_RES));
    // A thorn, thick and dark at its foot, sharpening to a pale point, barbs along it
    let mut p = vec![limb([0.0, 0.0, 0.0], [0.5, 0.0, 0.0], 0.08, 0.035, BARK_DARK), limb([0.5, 0.0, 0.0], [0.68, 0.0, 0.0], 0.035, 0.008, BONE)];
    for (x, side) in [(0.15, 1.0), (0.3, -1.0), (0.42, 1.0)] {
        p.push(limb([x, 0.0, side * 0.04], [x + 0.08, 0.0, side * 0.1], 0.025, 0.006, BARK_DARK));
    }
    models.push(voxelize(&p, MODEL_RES));
    let p: Vec<Part> = (0..96).map(|k| k as f64 / 96.0 * TAU).map(|a| ball([a.cos(), a.sin(), 0.0], 0.035, GLOW_BLOOD)).collect();
    models.push(voxelize(&p, RING_RES));
    // A ball of web, venom glowing through it, strands trailing off every way
    let mut seed = 0x3eb;
    let mut p = vec![ball([0.0, 0.0, 0.0], 0.12, GLOW_POISON), ball([0.0, 0.0, 0.0], 0.25, FOAM)];
    for _ in 0..9 {
        let d = [random(&mut seed) * 2.0 - 1.0, random(&mut seed) * 2.0 - 1.0, random(&mut seed) * 2.0 - 1.0];
        let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(0.1);
        p.push(limb([0.0, 0.0, 0.0], d.map(|v| v / n * 0.5), 0.05, 0.015, FOAM));
    }
    models.push(voxelize(&p, MODEL_RES));
    models
}

/// Voxels per meter of the models from effect_models(). What's out in the world has voxels the
/// size of the world's, what's held is finer, like the weapons.
pub const ARROW_RES: f64 = MODEL_RES;
pub const BOLT_RES: f64 = MODEL_RES;
pub const SPARK_RES: f64 = MODEL_RES;
pub const HELD_ARROW_RES: f64 = 160.0;
/// Finer, so the ring stays thin however big it's drawn
pub const RING_RES: f64 = 48.0;
pub const FINE_SPARK_RES: f64 = 128.0;
pub const ARM_RES: f64 = 128.0;

/// A gloved hand in a fist round z, its forearm in a leather bracer and a sleeve reaching back
/// along -y. The left hand has its thumb the other way.
fn arm(left: bool) -> Vec<Part> {
    let side = if left { 1.0 } else { -1.0 };
    let mut p = Vec::new();
    // Fingers curled round the grip, the thumb over them
    p.push(limb([side * 0.022, 0.0, 0.03], [side * 0.011, 0.028, 0.037], 0.01, 0.009, LEATHER));
    for z in [-0.03, -0.01, 0.01, 0.03] {
        p.push(ellipsoid([0.0, 0.007, z], [0.027, 0.031, 0.0095], LEATHER));
    }
    // The back of the hand and the wrist
    p.push(ellipsoid([-side * 0.004, -0.026, 0.004], [0.028, 0.031, 0.042], LEATHER));
    p.push(limb([0.0, -0.042, 0.0], [0.0, -0.09, -0.01], 0.023, 0.025, LEATHER));
    // A bracer with metal bands, then the sleeve
    for y in [-0.1, -0.18] {
        p.push(ellipsoid([0.0, y, -0.016 + (y + 0.1) * 0.2], [0.033, 0.007, 0.033], BRONZE));
    }
    p.push(limb([0.0, -0.09, -0.01], [0.0, -0.19, -0.03], 0.029, 0.031, LEATHER));
    p.push(limb([0.0, -0.19, -0.03], [0.0, -0.34, -0.06], 0.035, 0.039, CLOTH));
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_weapon() {
        let library = Library::parse(BUILT_IN).unwrap();
        for seed in 0..200 {
            let a = library.generate(seed, None, 1.0);
            let b = library.generate(seed, None, 1.0);
            assert_eq!(a.parts, b.parts);
            assert_eq!(a.name, b.name);
            assert_eq!(a.grade, b.grade);
        }
    }

    #[test]
    fn parts_fit_their_slots() {
        let library = Library::parse(BUILT_IN).unwrap();
        for seed in 0..500 {
            let weapon = library.generate(seed, None, 2.0);
            let family = &library.families[weapon.family];
            assert_eq!(weapon.parts.len(), family.slots.len());
            for (slot, &part) in family.slots.iter().zip(&weapon.parts) {
                let c = &library.components[part];
                assert_eq!(&c.slot_type, slot);
                assert!(c.compatible_families.contains(&family.id));
            }
            assert!(!weapon.look.frames.is_empty());
        }
    }

    #[test]
    fn rarities_roll_as_often_as_they_should() {
        let library = Library::parse(BUILT_IN).unwrap();
        let mut counts = vec![0usize; library.rarities.len()];
        let mut rng = 1;
        let rolls = 200_000;
        for _ in 0..rolls {
            counts[library.roll_rarity(1.0, &mut rng)] += 1;
        }
        for (rarity, count) in library.rarities.iter().zip(counts) {
            let seen = count as f64 / rolls as f64;
            assert!((seen - rarity.chance).abs() < 0.01 + rarity.chance * 0.1, "{} rolled {seen}, should be {}", rarity.id, rarity.chance);
        }
    }

    #[test]
    fn every_synergy_can_happen() {
        let library = Library::parse(BUILT_IN).unwrap();
        let mut seen = BTreeSet::new();
        for seed in 0..40_000 {
            let (family, parts) = library.roll(seed, None, 3.0);
            let components: Vec<&Component> = parts.iter().map(|&p| &library.components[p]).collect();
            let (profile, _, _) = Weapon::work_out(&library, family, &components);
            seen.extend(profile.synergies.iter().map(|s| s.def));
        }
        assert_eq!(seen.len(), library.synergies.len());
    }
}
