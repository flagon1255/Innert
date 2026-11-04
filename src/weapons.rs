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
    /// A crossbow, spanned with a lever or a crank, or a magazine's worth worked by a lever, and
    /// loosed at a touch
    Crank,
    /// A gun, loaded with powder and ball down its muzzle, and fired with a bang
    Powder,
    Cast,
    /// A flick of the wrist
    Flick,
    /// A whip, wound back and snapped forward so it cracks out at the crosshair
    Lash,
}

/// How a crossbow or gun is built, and so how it's loaded, see crossbow() and gun()
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Build {
    /// Not a crossbow or gun
    #[default]
    None,
    /// A light crossbow, spanned with a lever that swings over its top
    Lever,
    /// A heavy crossbow, wound back with a windlass at its butt
    Crank,
    /// A crossbow with a magazine of bolts on top, a lever spanning it again after each
    Repeater,
    /// A gun held in one hand
    Pistol,
    /// A long gun, its stock to the shoulder
    Musket,
    /// A short gun flaring at its muzzle, firing a spread of shot
    Blunderbuss,
    /// An iron tube on a wooden tiller, firing shells that burst
    Cannon,
}

impl Build {
    /// What a weapon built this way shoots, for a ranged one: a bow shoots arrows
    pub fn shot(self) -> Shot {
        match self {
            Build::None => Shot::Arrow,
            Build::Lever | Build::Crank | Build::Repeater => Shot::Quarrel,
            Build::Cannon => Shot::Shell,
            Build::Pistol | Build::Musket | Build::Blunderbuss => Shot::Ball,
        }
    }
}

/// What a bow, crossbow or gun shoots, as it flies and lands
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shot {
    Arrow,
    /// A crossbow's bolt
    Quarrel,
    /// A gun's lead ball, or a pellet of a spread of shot
    Ball,
    /// A hand cannon's iron shell, its fuse burning
    Shell,
}

impl Shot {
    /// The special ammo it takes, as AmmoDef::fits has it: arrows, bolts or shot
    pub fn ammo(self) -> &'static str {
        match self {
            Shot::Arrow => "arrow",
            Shot::Quarrel => "bolt",
            Shot::Ball | Shot::Shell => "shot",
        }
    }

    /// What its ammo's called, a lot of it
    pub fn plural(self) -> &'static str {
        match self {
            Shot::Arrow => "arrows",
            Shot::Quarrel => "bolts",
            Shot::Ball | Shot::Shell => "shot",
        }
    }
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
    /// Stats added on by the weapon's grade, a number for each rarity: how far a whip reaches
    #[serde(default)]
    grade_stats: BTreeMap<String, Vec<f64>>,
    /// How a crossbow or gun is built, and so loaded
    #[serde(default)]
    pub build: Build,
    /// For crossbows and guns, in meters: how long the stock is, how long a gun's barrel, how wide
    /// a crossbow's prod, and how wide a gun's bore is round, which is how big its shot is. Parts
    /// can make a barrel or a prod longer or shorter than these, see crossbow() and gun().
    #[serde(default)]
    length: f64,
    #[serde(default)]
    barrel: f64,
    #[serde(default)]
    span: f64,
    #[serde(default)]
    pub caliber: f64,
    /// Seconds a repeating crossbow's lever takes to work between shots
    #[serde(default = "quarter")]
    cycle: f64,
    /// Effects every one of the family has, whatever it's made of: an arbalest's bolts punching
    /// through, a hand cannon's shells bursting
    #[serde(default)]
    effects: Vec<String>,
    /// Indices of those effects
    #[serde(skip)]
    effect_defs: Vec<usize>,
}

fn middle() -> f64 {
    0.5
}

fn quarter() -> f64 {
    0.25
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

/// Special ammo for bows, crossbows and guns, carried in stacks and used up a shot at a time.
/// Plain ammo never runs out, so isn't one of these. What it does is added to what the weapon
/// shooting it does, see Library::loaded().
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmmoDef {
    pub id: String,
    pub display_name: String,
    /// What shoots it, see Shot::ammo(): "arrow", "bolt" or "shot"
    pub fits: String,
    pub description: String,
    #[serde(default)]
    base_stats: Stats,
    #[serde(default)]
    stat_multipliers: Stats,
    #[serde(default)]
    effects: Vec<String>,
    #[serde(default)]
    pub element: Option<Element>,
    /// Which arrowhead or bolt head it has, see ARROW_HEADS and QUARREL_HEADS, if not the weapon's own
    #[serde(default)]
    pub head: Option<usize>,
    /// Gold a merchant asks for each one, and how many come in a bundle
    pub price: f64,
    pub bundle: u32,
    /// Indices of its effects
    #[serde(skip)]
    pub effect_defs: Vec<usize>,
}

impl AmmoDef {
    /// The kind of thing it is in flight, a ball for shot
    pub fn shot(&self) -> Shot {
        match self.fits.as_str() {
            "arrow" => Shot::Arrow,
            "bolt" => Shot::Quarrel,
            _ => Shot::Ball,
        }
    }

    /// Its stats as they're shown on its card: what it adds, then what it multiplies
    pub fn stats(&self) -> (&Stats, &Stats) {
        (&self.base_stats, &self.stat_multipliers)
    }
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
    /// Special ammo
    #[serde(default)]
    pub ammo: Vec<AmmoDef>,
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
        let effects = &self.effects;
        let find = |owner: &str, id: &str| effects.iter().position(|e| e.id == id).ok_or_else(|| format!("{owner} has the unknown effect \"{id}\""));
        for family in &mut self.families {
            family.effect_defs = family.effects.iter().map(|id| find(&family.id, id)).collect::<Result<_, _>>()?;
        }
        for ammo in &mut self.ammo {
            ammo.effect_defs = ammo.effects.iter().map(|id| find(&ammo.id, id)).collect::<Result<_, _>>()?;
            if !["arrow", "bolt", "shot"].contains(&ammo.fits.as_str()) {
                return Err(format!("{} fits \"{}\", which isn't arrows, bolts or shot", ammo.id, ammo.fits));
            }
        }
        for family in &self.families {
            for slot in &family.slots {
                if !self.components.iter().any(|c| c.slot_type == *slot && c.compatible_families.contains(&family.id)) {
                    return Err(format!("nothing fits the {} slot of {}", slot, family.id));
                }
            }
            if family.build != Build::None && (family.kind != Kind::Ranged || family.length <= 0.0) {
                return Err(format!("{} is built as a crossbow or gun, so it has to be ranged and have a length", family.id));
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

    /// The synergies a weapon with `tags` has
    fn synergies_for(&self, tags: &BTreeSet<String>) -> Vec<ActiveSynergy> {
        self.synergies
            .iter()
            .enumerate()
            .filter(|(_, s)| s.requires.iter().all(|t| tags.contains(t)))
            .filter_map(|(def, s)| Some(ActiveSynergy { synergy: s.synergy?, def, params: s.params.clone() }))
            .collect()
    }

    /// What `weapon`'s shots do loaded with special ammo `ammo`: its own profile, with the ammo's
    /// stats and effects added as a part's are, its element if it has one, and any synergies all
    /// that brings about together
    pub fn loaded(&self, weapon: &Weapon, ammo: usize) -> Profile {
        let def = &self.ammo[ammo];
        let mut profile = (*weapon.profile).clone();
        for (name, value) in &def.base_stats {
            *profile.stats.entry(name.clone()).or_insert(0.0) += value;
        }
        for (name, value) in &def.stat_multipliers {
            *profile.stats.entry(name.clone()).or_insert(0.0) *= value;
        }
        if def.element.is_some() {
            profile.element = def.element;
        }
        let mut tags = weapon.tags.clone();
        for &d in &def.effect_defs {
            tags.extend(self.effects[d].synergy_tags.iter().cloned());
            tags.extend(self.effects[d].element.map(|e| e.tag().to_owned()));
            let Some(effect) = self.effects[d].effect else {
                continue;
            };
            match profile.effects.iter_mut().find(|e| e.def == d) {
                Some(e) => e.potency += 0.5,
                None => profile.effects.push(ActiveEffect { effect, def: d, potency: 1.0, params: self.effects[d].params.clone() }),
            }
        }
        if let Some(e) = def.element {
            tags.insert(e.tag().into());
        }
        profile.synergies = self.synergies_for(&tags);
        profile
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
pub const STATS: [(&str, &str, Unit); 18] = [
    ("physicalDamage", "Physical damage", Unit::Number),
    ("elementalDamage", "Elemental damage", Unit::Number),
    ("spellDamage", "Spell damage", Unit::Number),
    ("attackSpeed", "Swings per second", Unit::Rate),
    ("drawSpeed", "Draws per second", Unit::Rate),
    ("reloadSpeed", "Reloads per second", Unit::Rate),
    ("magazine", "Shots per reload", Unit::Number),
    ("pellets", "Pellets per shot", Unit::Number),
    ("spread", "Spread", Unit::Degrees),
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
    Degrees,
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
            Unit::Degrees => format!("{value:.0}°"),
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
    /// Its overall rarity, as Weapon::grade: what a whip can do grows with it
    pub grade: usize,
    /// How a crossbow or gun is built, and so loaded
    pub build: Build,
    /// How wide a gun's bore is round, in meters, which is how big its shot, its flash and its
    /// smoke are
    pub caliber: f64,
    /// Seconds a repeating crossbow's lever takes to work between shots
    pub cycle: f64,
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

    /// What it shoots, for a bow, crossbow or gun
    pub fn shot(&self) -> Option<Shot> {
        (self.kind == Kind::Ranged).then(|| self.build.shot())
    }

    /// How many shots a crossbow or gun holds loaded
    pub fn magazine(&self) -> u32 {
        self.stat("magazine").round().max(1.0) as u32
    }

    /// The same, without `effect`
    pub fn without(&self, effect: Effect) -> Profile {
        let mut profile = self.clone();
        profile.effects.retain(|e| e.effect != effect);
        profile
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
        for name in ["attackSpeed", "drawSpeed", "castSpeed", "reloadSpeed"] {
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

        // The family's own effects first, then its parts'
        let mut effects: Vec<ActiveEffect> = Vec::new();
        for &def in fam.effect_defs.iter().chain(components.iter().flat_map(|c| c.effect_defs.iter())) {
            let Some(effect) = library.effects[def].effect else {
                continue;
            };
            match effects.iter_mut().find(|e| e.def == def) {
                Some(e) => e.potency += 0.5,
                None => effects.push(ActiveEffect { effect, def, potency: 1.0, params: library.effects[def].params.clone() }),
            }
        }

        let mut tags: BTreeSet<String> = fam.synergy_tags.iter().cloned().collect();
        tags.extend(components.iter().flat_map(|c| c.synergy_tags.iter().cloned()));
        tags.extend(effects.iter().flat_map(|e| library.effects[e.def].synergy_tags.iter().cloned()));
        tags.extend(effects.iter().filter_map(|e| library.effects[e.def].element).map(|e| e.tag().to_owned()));
        if let Some(e) = element {
            tags.insert(e.tag().into());
        }
        let synergies = library.synergies_for(&tags);

        // The grade reflects how much the weapon can do, not the average of its parts: rare parts
        // and synergies add up, and one very rare part lifts the whole weapon close to it
        let power: f64 = components.iter().map(|c| library.rarities[c.tier].power).sum::<f64>() + library.synergy_power * synergies.len() as f64;
        let by_power = library.grade_power.iter().rposition(|&p| power >= p).unwrap_or(0);
        let best = components.iter().map(|c| c.tier).max().unwrap_or(0);
        let grade = by_power.max(best.saturating_sub(1)).min(library.rarities.len() - 1);
        for (name, by_grade) in &fam.grade_stats {
            if let Some(value) = by_grade.get(grade).or(by_grade.last()) {
                *stats.entry(name.clone()).or_insert(0.0) += value;
            }
        }
        // The rarest part with a special attack gives it its own
        let special = by_rarity.iter().find_map(|c| c.special.map(|s| (s, c.display_name.clone())));
        let (build, caliber, cycle) = (fam.build, fam.caliber, fam.cycle);
        (Profile { kind: fam.kind, style: fam.style, cone: fam.cone, targets: fam.targets, element, stats, effects, synergies, special, grade, build, caliber, cycle }, tags, grade)
    }

    pub fn kind(&self) -> Kind {
        self.profile.kind
    }

    /// What it shoots, for a bow, crossbow or gun
    pub fn shot(&self) -> Option<Shot> {
        self.profile.shot()
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

/// A weapon's model, in frames: one for swords, staves and whips, three for a bow being drawn.
/// Positions are in meters in the weapon's own frame: its length runs up z from the pommel or foot,
/// for a bow arrows fly along x.
pub struct WeaponLook {
    /// Each frame's model and where its anchor is, see voxelize()
    pub frames: Vec<(Model, Vector3<f64>)>,
    /// Voxels per meter of the models
    pub res: f64,
    /// Where the hand holds it, and which way round it the fist closes: up its length, or along a
    /// gun's grip
    pub grip: Vector3<f64>,
    pub hold: Vector3<f64>,
    /// The tip of the blade, the crystal spells come out of, where a whip's lash leaves its handle,
    /// or the muzzle of a gun or the front of a crossbow's groove, where its shots leave it
    pub tip: Vector3<f64>,
    /// For bows, where the nock of the arrow is in each frame, and for crossbows the bolt's
    pub nocks: Vec<Vector3<f64>>,
    /// For whips, what the lash is drawn with
    pub lash: Option<LashLook>,
    /// For crossbows and guns, the part that moves as it's loaded, see Mover
    pub mover: Option<Mover>,
    /// Where the other hand steadies a long crossbow or gun, and which way round it its fist
    /// closes, while it's not working the mover
    pub support: Option<(Vector3<f64>, Vector3<f64>)>,
}

/// A part of a crossbow or gun drawn apart from the rest, as it moves while it's loaded: a lever
/// or a windlass's cranks turning round `pivot` about `axis`, or a ramrod sliding along `axis`
/// out of the muzzle at `pivot`. All in the weapon's own meters.
pub struct Mover {
    /// Which of the weapon's frames it's drawn with
    pub frame: usize,
    pub pivot: Vector3<f64>,
    pub axis: Vector3<f64>,
    /// Where the other hand takes hold of it to work it, and which way round it the fist closes
    pub handle: Vector3<f64>,
    pub handle_axis: Vector3<f64>,
}

// Voxels per meter of each kind of weapon. They're only ever held, right in front of the eye, so
// they're much finer than the world's: that way their voxels look about as big as the world's a
// few meters away.
const SWORD_RES: f64 = 192.0;
const BOW_RES: f64 = 160.0;
const STAFF_RES: f64 = 150.0;
/// Crossbows are wide, so a little coarser, and guns finer, for their locks
const CROSSBOW_RES: f64 = 128.0;
const GUN_RES: f64 = 144.0;
/// How far a bow's string comes back drawn all the way, in meters
const DRAW_LENGTH: f64 = 0.24;

// A crossbow's frames: its string forward, halfway back and spanned, then its lever or cranks
pub const CROSSBOW_SPANNED: usize = 2;
// A gun's frames: its hammer cocked, then let down, then its ramrod
pub const GUN_COCKED: usize = 0;
pub const GUN_FIRED: usize = 1;

fn build_look(family: &Family, components: &[&Component]) -> WeaponLook {
    let visual = |slot: &str| components.iter().find(|c| c.slot_type == slot).map(|c| &c.visual_profile);
    let empty = Visual::default();
    let v = |slot: &str| visual(slot).unwrap_or(&empty);
    let plain = |frames, res, grip, tip| WeaponLook { frames, res, grip, hold: Vector3::z(), tip, nocks: Vec::new(), lash: None, mover: None, support: None };
    match family.kind {
        // Just the handle: the lash is simulated, see LashLook
        Kind::Melee if family.style == Style::Lash => {
            let (parts, grip, root) = whip(v("lash"), v("handle"), v("core"), family);
            let (set, cracker) = lash_models(v("lash"), v("fall"));
            let thickness = v("lash").thickness.unwrap_or(0.011);
            WeaponLook { lash: Some(LashLook { root, set, cracker, thickness }), ..plain(vec![voxelize(&parts, SWORD_RES)], SWORD_RES, grip, root) }
        }
        Kind::Melee => {
            let (parts, grip, tip) = sword(v("blade"), v("hilt"), v("guard"), v("core"), family);
            plain(vec![voxelize(&parts, SWORD_RES)], SWORD_RES, grip, tip)
        }
        Kind::Ranged if matches!(family.build, Build::Lever | Build::Crank | Build::Repeater) => crossbow(family, v("prod"), v("stock"), v("lock"), v("core")),
        Kind::Ranged if family.build != Build::None => gun(family, v("barrel"), v("stock"), v("lock"), v("core")),
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
            WeaponLook { nocks: nocks.clone(), ..plain(frames, BOW_RES, grip, nocks[0]) }
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
            plain(vec![voxelize(&parts, res)], res, grip, tip)
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

/// What a whip's lash is drawn with. The lash isn't part of the whip's model: combat.rs simulates
/// it as a rope hanging from `root`, and draws each stretch of it with one of the segment models
/// from effect_models(), see LASH_SEGMENTS, of set `set`, ending in cracker `cracker`.
pub struct LashLook {
    /// Where the lash leaves the top of the handle, in the whip's own meters
    pub root: Vector3<f64>,
    /// Which set of segment models it's drawn with, LASH_LEATHER and the rest, and which of the
    /// crackers is on its tip, CRACKER_HAIR and the rest
    pub set: usize,
    pub cracker: usize,
    /// How thick it is where it leaves the handle, in meters
    pub thickness: f64,
}

// The sets of lash segment models among effect_models(), by what a lash is made of: braided
// leather, hemp, chain or thorny vine, or for elemental lashes a dark cord glowing with one of
// the elements, in the order of Element::ALL
pub const LASH_LEATHER: usize = 0;
pub const LASH_HEMP: usize = 1;
pub const LASH_CHAIN: usize = 2;
pub const LASH_VINE: usize = 3;
pub const LASH_GLOW: usize = 4;
pub const LASH_SETS: usize = LASH_GLOW + Element::ALL.len();
/// Each set has a segment model as thick as each of these, as parts of its length, thickest first,
/// so a stretch of lash can be drawn about as thick as it should be however long it is
pub const LASH_TAPERS: [f64; 6] = [0.064, 0.048, 0.036, 0.027, 0.02, 0.015];
// The crackers on a lash's tip, by what its fall is: a pale tuft of horsehair, one of silk, a
// knotted end, barbs, a lead weight, a blade of bone or of gold, or a tuft glowing with one of the
// elements, in the order of Element::ALL
pub const CRACKER_HAIR: usize = 0;
pub const CRACKER_SILK: usize = 1;
pub const CRACKER_KNOTS: usize = 2;
pub const CRACKER_BARBS: usize = 3;
pub const CRACKER_WEIGHT: usize = 4;
pub const CRACKER_BONE: usize = 5;
pub const CRACKER_GOLD: usize = 6;
pub const CRACKER_GLOW: usize = 7;
pub const CRACKERS: usize = CRACKER_GLOW + Element::ALL.len();
/// Voxels per meter of the lash's segment models. Each is a meter long up z from 0, and is drawn
/// scaled down to the length of the stretch of lash it is.
pub const LASH_RES: f64 = 64.0;

/// Which element `material` is the glow of, as its place in Element::ALL, if it's a glow
fn glowing(material: u8) -> Option<usize> {
    Element::ALL.iter().position(|e| e.glow() == material)
}

/// Which set of segment models a whip with these parts has its lash drawn with, and which cracker
/// is on its tip: by what the lash and the fall are made of, glowing if the lash is elemental
fn lash_models(lash: &Visual, fall: &Visual) -> (usize, usize) {
    let cord = mat(&lash.material, LEATHER);
    let accent = lash.accent.as_deref().and_then(material);
    let set = match lash.shape.as_deref() {
        Some("chain") => LASH_CHAIN,
        Some("thorn") => LASH_VINE,
        _ => match glowing(cord).or_else(|| accent.and_then(glowing)) {
            Some(e) => LASH_GLOW + e,
            None if cord == LINEN => LASH_HEMP,
            None => LASH_LEATHER,
        },
    };
    let fall_material = fall.material.as_deref().and_then(material);
    let ornament = fall.accent.as_deref().and_then(material);
    let glow = ornament.and_then(glowing).or_else(|| fall_material.and_then(glowing));
    let cracker = match (fall.shape.as_deref().unwrap_or("cracker"), glow) {
        ("blade", _) if fall_material == Some(GOLD) || ornament == Some(GOLD) => CRACKER_GOLD,
        ("blade", _) => CRACKER_BONE,
        (_, Some(e)) => CRACKER_GLOW + e,
        ("knots", _) => CRACKER_KNOTS,
        ("barbs", _) => CRACKER_BARBS,
        ("weight", _) => CRACKER_WEIGHT,
        _ if fall_material == Some(LINEN) => CRACKER_SILK,
        _ => CRACKER_HAIR,
    };
    (set, cracker)
}

/// A stretch of lash a meter long up z from 0, `radius` thick, of set `set`, see LASH_LEATHER and
/// the rest: a cord banded where the strands of its braid cross, links of chain, or a vine with
/// thorns
fn lash_segment(set: usize, radius: f64) -> Vec<Part> {
    let mut p = Vec::new();
    if set == LASH_CHAIN {
        // Links end to end, every other one turned the other way and darker
        let links = 4;
        for k in 0..links {
            let z = (k as f64 + 0.5) / links as f64;
            let (material, wide) = if k % 2 == 0 { (STEEL, [1.6, 0.9]) } else { (DARK_STEEL, [0.9, 1.6]) };
            p.push(ellipsoid([0.0, 0.0, z], [radius * wide[0], radius * wide[1], 0.6 / links as f64], material));
        }
        return p;
    }
    let (cord, band) = match set {
        LASH_LEATHER => (LEATHER, FUR_TAN),
        LASH_HEMP => (LINEN, WAX),
        LASH_VINE => (MOSS, BARK_DARK),
        glow => (BARK_DARK, Element::ALL[glow - LASH_GLOW].glow()),
    };
    if set == LASH_VINE {
        // Thorns off either side, raking back
        for (z, side) in [(0.3, 1.0), (0.78, -1.0)] {
            p.push(limb([0.0, 0.0, z], [side * radius * 2.8, 0.0, z - radius * 1.2], radius * 0.6, radius * 0.15, BARK_DARK));
        }
    }
    // Earlier shapes win, so these show as narrow raised bands round the cord
    let bands = 4;
    for k in 0..bands {
        let z = (k as f64 + 0.4) / bands as f64;
        p.push(limb([0.0, 0.0, z], [0.0, 0.0, z + 0.18 / bands as f64], radius * 1.1, radius * 1.1, band));
    }
    p.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], radius, radius, cord));
    p
}

/// The last stretch of a lash, a meter long up z from 0: the fall, thinning, and on its end cracker
/// `kind`, see CRACKER_HAIR and the rest
fn lash_cracker(kind: usize) -> Vec<Part> {
    let r = 0.022;
    let mut p = Vec::new();
    // Strands splaying out off the end
    let tuft = |p: &mut Vec<Part>, material: u8, spread: f64| {
        for (dx, dy) in [(1.0, 0.0), (-0.5, 0.87), (-0.5, -0.87), (0.0, 0.0)] {
            p.push(limb([0.0, 0.0, 0.64], [dx * spread, dy * spread, 1.0], r * 0.75, r * 0.3, material));
        }
    };
    let fall = match kind {
        CRACKER_HAIR => {
            tuft(&mut p, FUR_WHITE, 0.08);
            LEATHER
        }
        CRACKER_SILK => {
            tuft(&mut p, LINEN, 0.07);
            LEATHER
        }
        CRACKER_KNOTS => {
            for z in [0.38, 0.62, 0.86] {
                p.push(ball([0.0, 0.0, z], r * 2.0, BARK_DARK));
            }
            tuft(&mut p, LEATHER, 0.04);
            LEATHER
        }
        CRACKER_BARBS => {
            for (z, side) in [(0.45, 1.0), (0.65, -1.0), (0.85, 1.0)] {
                p.push(limb([0.0, 0.0, z], [side * 0.06, 0.0, z - 0.07], r * 0.7, r * 0.2, STEEL));
            }
            tuft(&mut p, LEATHER, 0.03);
            LEATHER
        }
        CRACKER_WEIGHT => {
            p.push(ball([0.0, 0.0, 0.92], 0.06, DARK_STEEL));
            LEATHER
        }
        CRACKER_BONE | CRACKER_GOLD => {
            let edge = if kind == CRACKER_GOLD { GOLD } else { BONE };
            p.push(limb([0.0, 0.0, 0.64], [0.0, 0.0, 1.06], 0.055, 0.004, edge).stretched([1.0, 0.35, 1.0]));
            LEATHER
        }
        glow => {
            let g = Element::ALL[glow - CRACKER_GLOW].glow();
            p.push(ball([0.0, 0.0, 0.96], 0.04, g));
            tuft(&mut p, g, 0.07);
            BARK_DARK
        }
    };
    p.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, 0.68], r, r * 0.8, fall));
    p
}

/// A whip's handle standing on its pommel, bound at the top where the lash leaves it. The lash
/// isn't part of the model: combat.rs simulates it, see LashLook. Returns where it's held and where
/// the lash leaves it.
fn whip(lash: &Visual, handle: &Visual, core: &Visual, family: &Family) -> (Vec<Part>, Vector3<f64>, Vector3<f64>) {
    let mut p = Vec::new();
    let voxel = 1.0 / SWORD_RES;
    let pommel = handle.pommel_size.unwrap_or(0.026);
    let grip_length = handle.length.unwrap_or(0.24);
    let z_grip = pommel * 1.7;
    let z_top = z_grip + grip_length;
    let thick = lash.thickness.unwrap_or(0.011).max(2.0 * voxel);
    let cord = mat(&lash.material, LEATHER);
    let braid = mat(&lash.accent, cord);
    let root = [0.0, 0.0, z_top + 0.035];
    // The gem of its core, set into the top of the handle, then the thong binding the lash on, and
    // the lash's first couple of centimeters coming out of it, which the simulated lash carries on
    let mut z_thong = z_top;
    if let Some(gem) = core.gem.as_deref().and_then(material) {
        let r = core.size.unwrap_or(0.02).max(0.017);
        p.push(ball([0.0, 0.0, z_top], r, gem));
        z_thong = z_top + r * 0.6;
    }
    p.push(limb([0.0, 0.0, z_thong - 0.01], root, 0.018, thick * 1.3, braid));
    p.push(limb(root, [0.0, 0.0, root[2] + 0.015], thick * 1.3, thick * 1.2, cord));
    sword_hilt(&mut p, handle, z_grip, grip_length, pommel, grip_length);
    (p, v3(0.0, 0.0, z_grip + grip_length * family.grip), v3(root[0], root[1], root[2]))
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

/// A crossbow lying level, shooting along +x, the top of its tiller at z 0 and its butt at x 0,
/// its prod across the front bending back towards its tips. Its frames are its string forward,
/// halfway back and spanned to the latch, then what spans it, drawn apart as it moves: a light
/// one's lever swinging over its top, a heavy one's windlass cranks, or the lever of a repeater
/// arching over its magazine. Parts make the prod longer or shorter than the family's span.
fn crossbow(family: &Family, prod: &Visual, stock: &Visual, lock: &Visual, core: &Visual) -> WeaponLook {
    let length = family.length;
    let half = family.span * prod.length.unwrap_or(1.0) * 0.5;
    let wood = mat(&stock.material, WOOD);
    let trim = mat(&stock.accent, DARK_STEEL);
    let metal = mat(&lock.material, DARK_STEEL);
    let flourish = mat(&lock.accent, metal);
    let bow = mat(&prod.material, WOOD);
    let cord = mat(&prod.edge, BOWSTRING);
    let tips = prod.tips.as_deref().and_then(material);
    let shape = prod.shape.as_deref().unwrap_or("simple");
    let x_prod = length - 0.06;
    let x_latch = length * 0.42;
    // How far back the prod bends at `s`, from -1 at one tip to 1 at the other, bending further
    // spanned, and more the longer it is
    let bend = |s: f64, draw: f64| {
        let tip = ((s.abs() - 0.7) / 0.3).max(0.0);
        let x = match shape {
            "recurve" => -0.11 * s * s + 0.07 * tip * tip,
            "composite" => -0.1 * s * s + 0.035 * tip * tip,
            "steel" => -0.075 * s * s,
            _ => -0.1 * s * s,
        };
        x * (half / 0.31) * (1.0 + 0.8 * draw)
    };
    let at = |s: f64, draw: f64| [x_prod + bend(s, draw), s * half, 0.004];
    let nock_at = |draw: f64| (x_prod + bend(1.0, draw)) * (1.0 - draw) + x_latch * draw;
    let windlass = [0.07, 0.0, 0.02];

    let body = |draw: f64| {
        let mut p = Vec::new();
        let nock = [nock_at(draw), 0.0, 0.006];
        // The string, and on a heavy one the windlass's cords hooked onto it
        p.push(limb(at(1.0, draw), nock, 0.0045, 0.0045, cord));
        p.push(limb(at(-1.0, draw), nock, 0.0045, 0.0045, cord));
        if family.build == Build::Crank {
            for s in [-1.0, 1.0] {
                p.push(ball([nock[0] - 0.008, s * 0.012, 0.008], 0.007, metal));
                p.push(limb([windlass[0], s * 0.03, windlass[2] + 0.018], [nock[0] - 0.008, s * 0.012, 0.008], 0.003, 0.003, cord));
            }
        }
        if let Some(gem) = core.gem.as_deref().and_then(material) {
            p.push(ball([length * 0.6, 0.02, -0.02], core.size.unwrap_or(0.02).max(0.013) * 0.8, gem));
        }
        // The groove the bolt lies in, cut out of the tiller's top
        p.push(cuboid([x_latch + 0.012, -0.004, -0.006], [length + 0.01, 0.004, 0.001], AIR));
        // The nut that catches the string, and the long trigger under the tiller
        p.push(ball([x_latch, 0.0, 0.002], 0.011, metal));
        p.push(limb([x_latch + 0.01, 0.0, -0.035], [x_latch - 0.13, 0.0, -0.075], 0.006, 0.005, metal));
        p.push(ball([x_latch - 0.13, 0.0, -0.075], 0.008, flourish));
        // The prod, bound on at the front, its tips capped
        if let Some(tips) = tips {
            for s in [-1.0, 1.0] {
                p.push(ball(at(s, draw), 0.012, tips));
            }
        }
        p.push(cuboid([x_prod - 0.028, -0.03, -0.038], [x_prod + 0.028, 0.03, 0.01], trim));
        let steps = 12;
        for side in [-1.0, 1.0] {
            let points: Vec<[f64; 3]> = (0..=steps).map(|k| at(side * k as f64 / steps as f64, draw)).collect();
            for (k, pair) in points.windows(2).enumerate() {
                let r = if shape == "steel" { 0.011 } else { 0.015 } * (1.0 - 0.45 * k as f64 / steps as f64);
                p.push(limb(pair[0], pair[1], r, r, bow).stretched([1.0, 1.0, 1.7]));
            }
        }
        match family.build {
            // The windlass on its brackets at the butt
            Build::Crank => {
                p.push(limb([windlass[0], -0.05, windlass[2]], [windlass[0], 0.05, windlass[2]], 0.008, 0.008, metal));
                for s in [-1.0, 1.0] {
                    p.push(ellipsoid([windlass[0], s * 0.03, windlass[2]], [0.022, 0.011, 0.022], wood));
                    p.push(cuboid([windlass[0] - 0.02, s * 0.037 - 0.004, -0.05], [windlass[0] + 0.02, s * 0.037 + 0.004, windlass[2] + 0.004], metal));
                }
            }
            // The brackets the lever turns on
            Build::Lever => {
                for s in [-1.0, 1.0] {
                    p.push(cuboid([x_latch + 0.075, s * 0.025 - 0.003, -0.025], [x_latch + 0.105, s * 0.025 + 0.003, 0.034], metal));
                }
            }
            // The magazine on top, the last few bolts showing through its slot, and the pins the
            // lever turns on
            _ => {
                let (back, front) = (x_latch - 0.02, x_latch + 0.24);
                for k in 0..2 {
                    let z = 0.085 + k as f64 * 0.012;
                    p.push(limb([front - 0.035, 0.0, z], [front - 0.005, 0.0, z], 0.005, 0.0015, metal));
                    p.push(limb([back + 0.03, 0.0, z], [front - 0.035, 0.0, z], 0.0045, 0.0045, WOOD));
                    p.push(cuboid([back + 0.03, -0.008, z - 0.001], [back + 0.06, 0.008, z + 0.001], FUR_WHITE));
                }
                p.push(cuboid([back + 0.02, -0.007, 0.07], [front - 0.004, 0.007, 0.13], AIR));
                for x in [back + 0.01, front - 0.012] {
                    p.push(cuboid([x - 0.006, -0.021, 0.0], [x + 0.006, 0.021, 0.115], trim));
                }
                p.push(cuboid([back, -0.018, 0.0], [front, 0.018, 0.11], wood));
                p.push(limb([length - 0.005, -0.03, -0.03], [length - 0.005, 0.03, -0.03], 0.007, 0.007, metal));
            }
        }
        // A stirrup out front to stand in while spanning it
        if family.build != Build::Repeater {
            ring(&mut p, [length + 0.045, 0.0, -0.016], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0], 0.04, 0.0065, metal);
        }
        // Bands round the tiller, then the tiller, deep at the butt to set against the shoulder
        for x in [0.26, x_latch + 0.16] {
            p.push(cuboid([x - 0.006, -0.025, -0.043], [x + 0.006, 0.025, 0.003], trim));
        }
        p.push(cuboid([0.2, -0.022, -0.04], [length, 0.022, 0.0], wood));
        p.push(cuboid([0.0, -0.016, -0.112], [0.012, 0.016, -0.02], trim));
        p.push(limb([0.03, 0.0, -0.065], [0.26, 0.0, -0.022], 0.048, 0.024, wood).stretched([1.0, 0.5, 1.0]));
        p
    };
    let mut frames: Vec<(Model, Vector3<f64>)> = [0.0, 0.5, 1.0].map(|draw| voxelize(&body(draw), CROSSBOW_RES)).into();
    let nocks: Vec<Vector3<f64>> = [0.0, 0.5, 1.0].map(|draw| v3(nock_at(draw), 0.0, 0.006)).into();

    // What spans it, at rest
    let mut m = Vec::new();
    let mover = match family.build {
        // A windlass's two cranks, hanging down either side of the butt
        Build::Crank => {
            for s in [-1.0, 1.0] {
                let y = s * 0.06;
                m.push(limb([windlass[0], y, windlass[2]], [windlass[0], y, windlass[2] - 0.085], 0.007, 0.006, metal));
                m.push(limb([windlass[0], y, windlass[2] - 0.085], [windlass[0], y + s * 0.045, windlass[2] - 0.085], 0.0095, 0.0095, wood));
            }
            Mover { frame: 3, pivot: Vector3::from(windlass), axis: Vector3::y(), handle: v3(windlass[0], 0.085, windlass[2] - 0.085), handle_axis: Vector3::y() }
        }
        // A goat's-foot lever lying back along the top, its claws forward of the pins it turns on
        Build::Lever => {
            let pivot = [x_latch + 0.09, 0.0, 0.03];
            let end = pivot[0] - 0.25;
            for s in [-1.0, 1.0] {
                let y = s * 0.032;
                m.push(ball([pivot[0], y, pivot[2]], 0.008, flourish));
                m.push(limb([pivot[0], y, pivot[2]], [pivot[0] + 0.04, y, pivot[2] + 0.025], 0.006, 0.004, metal));
                m.push(limb([pivot[0], y, pivot[2]], [end, y, pivot[2] + 0.01], 0.006, 0.005, metal));
            }
            m.push(limb([end, -0.04, pivot[2] + 0.01], [end, 0.04, pivot[2] + 0.01], 0.009, 0.009, wood));
            Mover { frame: 3, pivot: Vector3::from(pivot), axis: Vector3::y(), handle: v3(end, 0.0, pivot[2] + 0.01), handle_axis: Vector3::y() }
        }
        // A lever turning on pins at the front, arching up over the magazine to a handle behind it
        _ => {
            let pivot = [length - 0.005, 0.0, -0.03];
            let points = [pivot, [length - 0.02, 0.0, 0.1], [x_latch + 0.08, 0.0, 0.17], [x_latch - 0.07, 0.0, 0.16]];
            for s in [-1.0, 1.0] {
                let side: Vec<[f64; 3]> = points.iter().map(|q| [q[0], s * 0.027, q[2]]).collect();
                curve(&mut m, &side, 0.0065, 0.0055, metal);
            }
            let end = points[3];
            m.push(limb([end[0], -0.04, end[2]], [end[0], 0.04, end[2]], 0.009, 0.009, wood));
            Mover { frame: 3, pivot: Vector3::from(pivot), axis: Vector3::y(), handle: v3(end[0], 0.0, end[2]), handle_axis: Vector3::y() }
        }
    };
    frames.push(voxelize(&m, CROSSBOW_RES));
    let support = (family.build != Build::Repeater).then(|| (v3(x_latch + 0.22, 0.0, -0.03), Vector3::x()));
    WeaponLook {
        frames,
        res: CROSSBOW_RES,
        grip: v3(x_latch - 0.08, 0.0, -0.04),
        hold: Vector3::x(),
        tip: v3(length + 0.01, 0.0, 0.008),
        nocks,
        lash: None,
        mover: Some(mover),
        support,
    }
}

/// A gun lying level, its barrel along +x with the middle of its bore at z 0, its butt or grip at
/// the back, and its lock on its left side, +y, which is the side seen when it's held in the right
/// hand. Its frames are its hammer cocked and let down, then its ramrod, lying right down its
/// barrel. Parts make the barrel longer or shorter than the family's.
fn gun(family: &Family, barrel: &Visual, stock: &Visual, lock: &Visual, core: &Visual) -> WeaponLook {
    let build = family.build;
    let caliber = family.caliber;
    let wood = mat(&stock.material, WOOD);
    let trim = mat(&stock.accent, BRONZE);
    let tube = mat(&barrel.material, DARK_STEEL);
    let bands = mat(&barrel.accent, trim);
    let metal = mat(&lock.material, STEEL);
    let flourish = mat(&lock.accent, metal);
    let shape = barrel.shape.as_deref().unwrap_or("round");
    // Where the barrel starts, and the muzzle
    let x_b = match build {
        Build::Pistol => 0.07,
        Build::Musket => 0.4,
        Build::Blunderbuss => 0.34,
        _ => 0.06,
    };
    let x_m = x_b + family.barrel * barrel.length.unwrap_or(1.0);
    let wall = (caliber * 0.6).max(0.006);
    let outer = caliber + wall;
    // How far round the barrel is at `t` of the way along it: tapering, a blunderbuss flaring at
    // its mouth and a cannon swelling there
    let radius = |t: f64| match build {
        Build::Blunderbuss => outer * (1.08 - 0.15 * t + 1.3 * ((t - 0.6) / 0.4).max(0.0).powi(2)),
        Build::Cannon => outer * (1.1 - 0.12 * t + if t > 0.9 { 0.18 } else { 0.0 }),
        _ => outer * (1.15 - 0.2 * t),
    };
    let mouth = if build == Build::Blunderbuss { caliber * 2.4 } else { caliber };
    // Two barrels side by side for a double-barrelled one
    let bores: &[f64] = if shape == "twin" { &[-1.0, 1.0] } else { &[0.0] };
    let side = |y: f64| y * outer * 0.95;
    // How far out to the side the lock sits: on the stock, or beside the barrels or the tube
    let w = match build {
        Build::Pistol => 0.014,
        Build::Cannon => outer * 1.08,
        _ => 0.019,
    };
    let w = if shape == "twin" { w.max(outer * 1.95) } else { w };
    let plate = if build == Build::Pistol { x_b - 0.064 } else { x_b - 0.08 };

    let body = |cocked: bool| {
        let mut p = Vec::new();
        // The bore, hollowing out the muzzle
        for &y in bores {
            p.push(limb([x_m - 0.03, side(y), 0.0], [x_m + 0.02, side(y), 0.0], mouth, mouth, AIR));
        }
        // A sight on top at the muzzle, and bands round the barrel
        p.push(cuboid([x_m - 0.014, -0.0015, radius(0.97) - 0.002], [x_m - 0.004, 0.0015, radius(0.97) + 0.006], bands));
        let rings: Vec<f64> = match build {
            Build::Musket => vec![0.35, 0.62, 0.86],
            Build::Cannon => vec![0.0, 0.25, 0.5, 0.75, 0.97],
            Build::Blunderbuss => vec![0.3, 0.98],
            _ => vec![0.97],
        };
        for t in rings {
            let x = x_b + (x_m - x_b) * t;
            for &y in bores {
                p.push(limb([x - 0.007, side(y), 0.0], [x + 0.007, side(y), 0.0], radius(t) * 1.14, radius(t) * 1.14, bands));
            }
        }
        // A dragon's head round the muzzle, its jaws open, its eyes glowing
        if shape == "dragon" {
            let r = radius(1.0);
            for s in [-1.0, 1.0] {
                p.push(ball([x_m - 0.03, s * r * 0.9, r * 0.9], r * 0.35, bands));
                p.push(limb([x_m - 0.04, s * r * 0.6, r * 1.1], [x_m - 0.09, s * r * 0.8, r * 1.9], r * 0.3, r * 0.08, tube));
            }
            p.push(limb([x_m - 0.05, 0.0, r * 0.7], [x_m + 0.015, 0.0, r * 1.15], r * 0.7, r * 0.45, tube));
            p.push(limb([x_m - 0.05, 0.0, -r * 0.7], [x_m + 0.01, 0.0, -r * 1.3], r * 0.6, r * 0.35, tube));
        }
        // The barrel, or barrels: round, or squared off down its length with a rib on top
        let steps = 8;
        for &y in bores {
            for k in 0..steps {
                let (t0, t1) = (k as f64 / steps as f64, (k + 1) as f64 / steps as f64);
                let (a, b) = (x_b + (x_m - x_b) * t0, x_b + (x_m - x_b) * t1);
                if shape == "octagon" {
                    let r = radius(t0) * 0.88;
                    p.push(cuboid([a, side(y) - r, -r], [b, side(y) + r, r], tube));
                }
                p.push(limb([a, side(y), 0.0], [b, side(y), 0.0], radius(t0), radius(t1), tube));
            }
            if build == Build::Cannon {
                // The knob on its breech
                p.push(ball([x_b - 0.012, side(y), 0.0], outer * 0.55, tube));
            }
        }
        if shape == "octagon" {
            p.push(cuboid([x_b + 0.02, -0.002, outer * 0.85], [x_m - 0.02, 0.002, outer * 1.15], bands));
        }

        // The lock on its plate, its hammer cocked back or let down onto the pan
        let y = w + 0.005;
        let pivot = [x_b - 0.055, y, -0.006];
        let lock_shape = lock.shape.as_deref().unwrap_or("flint");
        let (jaw, frizzen_top) = if cocked { ([x_b - 0.047, y, 0.033], [x_b - 0.009, y, 0.03]) } else { ([x_b - 0.017, y, 0.018], [x_b + 0.016, y, 0.02]) };
        let elbow = if cocked { [x_b - 0.072, y, 0.016] } else { [x_b - 0.042, y, 0.022] };
        match lock_shape {
            // A serpentine holding a smouldering match, swung down into the pan to fire
            "match" => {
                let ember = if cocked { [jaw[0] + 0.006, y, jaw[2] + 0.006] } else { [x_b - 0.011, y, 0.012] };
                p.push(ball(ember, 0.0055, flourish));
                p.push(limb(jaw, ember, 0.004, 0.004, LINEN));
                p.push(limb(jaw, [jaw[0] - 0.05, y + 0.006, jaw[2] - 0.04], 0.0025, 0.0025, LINEN));
                curve(&mut p, &[[pivot[0], y, pivot[2] - 0.01], elbow, jaw], 0.0045, 0.0035, metal);
            }
            // A wheel turning against the stone, and the dog holding it
            "wheel" => {
                p.push(ellipsoid([x_b - 0.03, y + 0.002, -0.004], [0.015, 0.004, 0.015], flourish));
                p.push(ball(jaw, 0.006, OBSIDIAN));
                curve(&mut p, &[pivot, elbow, jaw], 0.004, 0.004, metal);
            }
            // A flint or, on a rune lock, a glowing stone, struck against the frizzen as the
            // hammer falls, knocking it open
            shape => {
                let (stone, plate) = if shape == "rune" { (flourish, flourish) } else { (OBSIDIAN, metal) };
                p.push(cuboid([jaw[0] - 0.002, y - 0.004, jaw[2] - 0.006], [jaw[0] + 0.008, y + 0.004, jaw[2] + 0.002], stone));
                p.push(limb([x_b - 0.011, y, 0.004], frizzen_top, 0.004, 0.003, plate));
                curve(&mut p, &[pivot, elbow, jaw], 0.0045, 0.0035, metal);
            }
        }
        // The pan, and the plate
        p.push(cuboid([x_b - 0.02, w, -0.001], [x_b - 0.004, w + 0.011, 0.005], metal));
        p.push(cuboid([plate, w, -0.02], [x_b + 0.015, w + 0.004, 0.009], metal));
        if let Some(gem) = core.gem.as_deref().and_then(material) {
            let r = core.size.unwrap_or(0.02).max(0.013) * 0.75;
            let at = match build {
                Build::Pistol => [x_b - 0.06, 0.013, -0.06],
                Build::Cannon => [x_b + 0.05, 0.0, outer * 1.05],
                _ => [0.15, 0.021, -0.06],
            };
            p.push(ball(at, r, gem));
        }

        // The trigger in its guard, then the stock
        p.push(limb([x_b - 0.035, 0.0, -0.02], [x_b - 0.043, 0.0, -0.042], 0.0035, 0.003, metal));
        if build != Build::Cannon {
            ring(&mut p, [x_b - 0.04, 0.0, -0.036], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.02, 0.0035, trim);
        }
        let fore = family.length;
        match build {
            // A grip curving down from the breech, capped
            Build::Pistol => {
                p.push(ball([x_b - 0.118, 0.0, -0.106], 0.025, trim));
                p.push(limb([x_b + 0.005, 0.0, -0.006], [x_b - 0.075, 0.0, -0.016], 0.014, 0.014, wood));
                p.push(limb([x_b + 0.005, 0.0, -0.012], [x_b - 0.11, 0.0, -0.098], 0.017, 0.022, wood).stretched([1.0, 0.75, 1.0]));
                p.push(limb([x_b - 0.01, 0.0, -0.01], [fore, 0.0, -0.009], 0.013, 0.011, wood));
            }
            // A wooden tiller under the breech, strapped to the barrel, to hold it by
            Build::Cannon => {
                p.push(cuboid([x_b + 0.03, -outer * 0.8, -outer - 0.03], [x_b + 0.05, outer * 0.8, 0.0], bands));
                p.push(cuboid([x_b + 0.12, -0.012, -outer - 0.04], [x_b + 0.15, 0.012, -outer + 0.01], tube));
                p.push(limb([x_b - fore, 0.0, -outer - 0.012], [x_b + 0.12, 0.0, -outer - 0.012], 0.022, 0.026, wood));
                p.push(ball([x_b - fore, 0.0, -outer - 0.012], 0.024, trim));
            }
            // A full stock, from a butt to set against the shoulder to a fore-end under the barrel
            _ => {
                p.push(cuboid([0.0, -0.022, -0.128], [0.012, 0.022, -0.026], trim));
                p.push(limb([fore - 0.006, 0.0, -outer * 0.85], [fore + 0.006, 0.0, -outer * 0.85], 0.016, 0.016, trim));
                p.push(limb([x_b - 0.02, 0.0, -outer * 0.85], [fore, 0.0, -outer * 0.8], 0.017, 0.015, wood).stretched([1.0, 1.15, 0.9]));
                p.push(limb([x_b, 0.0, -0.012], [x_b - 0.12, 0.0, -0.03], 0.019, 0.018, wood).stretched([1.0, 0.85, 1.0]));
                p.push(limb([x_b - 0.11, 0.0, -0.032], [0.03, 0.0, -0.077], 0.021, 0.052, wood).stretched([1.0, 0.45, 1.0]));
            }
        }
        p
    };
    let mut frames = vec![voxelize(&body(true), GUN_RES), voxelize(&body(false), GUN_RES)];

    // The ramrod, as far down the barrel as it goes
    let bore = v3(x_m, side(bores[0]), 0.0);
    let rod = if build == Build::Cannon { 0.011 } else { 0.0042 };
    let rammer = [
        ball([bore.x, bore.y, 0.0], rod * 1.6, bands),
        limb([bore.x - (x_m - x_b) + 0.01, bore.y, 0.0], [bore.x, bore.y, 0.0], rod * 1.3, rod, if build == Build::Cannon { WOOD } else { STEEL }),
    ];
    frames.push(voxelize(&rammer, GUN_RES));
    let mover = Mover { frame: 2, pivot: bore, axis: Vector3::x(), handle: bore, handle_axis: Vector3::x() };
    // Held at the grip of a pistol, the tiller of a cannon or the wrist of a stock, steadied
    // under the fore-end of a long gun
    let (grip, hold) = match build {
        Build::Pistol => (v3(x_b - 0.055, 0.0, -0.055), v3(0.8, 0.0, 0.6)),
        Build::Cannon => (v3(x_b - family.length * 0.45, 0.0, -outer - 0.012), Vector3::x()),
        _ => (v3(x_b - 0.075, 0.0, -0.022), v3(0.96, 0.0, 0.28)),
    };
    let support = matches!(build, Build::Musket | Build::Blunderbuss).then(|| (v3(x_b + (family.length - x_b) * 0.55, 0.0, -outer - 0.012), Vector3::x()));
    WeaponLook { frames, res: GUN_RES, grip, hold: hold.normalize(), tip: v3(x_m, 0.0, 0.0), nocks: Vec::new(), lash: None, mover: Some(mover), support }
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
/// And of bolt head: a square point, and a broad barbed one
pub const QUARREL_HEADS: usize = 2;
/// How long a crossbow's bolt is, from its nock to its point
pub const QUARREL_HEAD: f64 = 0.37;

/// How long an arrow or a bolt is, and how it's fletched, see arrow()
struct Fletch {
    /// Where its head starts
    start: f64,
    /// How thick its shaft is round
    shaft: f64,
    /// Where its feathers or vanes run from and to along it, and how far out they reach there
    vanes: (f64, f64),
    reach: (f64, f64),
    /// Its head's size next to an arrow's
    head: f64,
}

const ARROW_FLETCH: Fletch = Fletch { start: 0.6, shaft: 0.0045, vanes: (0.025, 0.115), reach: (0.007, 0.019), head: 1.0 };
/// Short and thick, with stiff vanes
const QUARREL_FLETCH: Fletch = Fletch { start: 0.3, shaft: 0.0058, vanes: (0.012, 0.07), reach: (0.008, 0.018), head: 0.75 };

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
/// Crossbow bolts in flight, a set for each head, see QUARREL_HEADS, then finer ones lying in a
/// crossbow's groove
pub const SLOT_QUARREL: usize = SLOT_DROP + 1;
pub const SLOT_HELD_QUARREL: usize = SLOT_QUARREL + QUARREL_HEADS;
/// A gun's balls
pub const SLOT_SHOT: usize = SLOT_HELD_QUARREL + QUARREL_HEADS;
/// Puffs of smoke, white powder smoke without an element, each element's its own color
pub const SLOT_SMOKE: usize = SLOT_SHOT + 1;
/// After the sets: the right arm, the left arm, a boulder, then spikes of ice bursting up
pub const ARMS: usize = (SLOT_SMOKE + 1) * ELEMENT_LOOKS;
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
/// A gun's muzzle flash, bursting out along +x from 0
pub const MUZZLE: usize = WEB + 1;
/// A hand cannon's shell, its fuse burning behind it, its middle at 0
pub const SHELL: usize = MUZZLE + 1;
/// Then the stretches of lash a whip's lash is drawn with, see LashLook: a model as thick as each
/// of LASH_TAPERS for each of the LASH_SETS, then each of the CRACKERS for its tip
pub const LASH_SEGMENTS: usize = SHELL + 1;
pub const LASH_CRACKERS: usize = LASH_SEGMENTS + LASH_SETS * LASH_TAPERS.len();

/// An arrow along +x with its nock at 0, at `res` voxels per meter: with arrowhead `head` (see
/// ARROW_HEADS), and its element showing in the head and what's round it, the fletching and the
/// shaft. With `fletch` a crossbow's bolt instead, shorter and thicker, see quarrel().
fn arrow(element: Option<Element>, head: usize, res: f64, fletch: &Fletch) -> Vec<Part> {
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
    let start = fletch.start;
    let mut p = Vec::new();
    let band = |p: &mut Vec<Part>, x: f64, material: u8| p.push(limb([x, 0.0, 0.0], [x + 0.006, 0.0, 0.0], fletch.shaft + 0.003, fletch.shaft + 0.003, material));
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
                p.push(ball([fletch.vanes.1 + 0.025, a.cos() * 0.016, a.sin() * 0.016], 0.003, GLOW_HOLY));
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
    let (length, width) = (length * fletch.head, width * fletch.head.sqrt());
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
    p.push(limb([start - 0.006, 0.0, 0.0], [start + 0.006, 0.0, 0.0], fletch.shaft + 0.0015, fletch.shaft + 0.0005, body));

    // Three feathers by the nock, then the nock and the shaft
    let ((from, to), (near, far)) = (fletch.vanes, fletch.reach);
    for a in [0.0, TAU / 3.0, 2.0 * TAU / 3.0] {
        let (c, s) = (f64::cos(a), f64::sin(a));
        p.push(limb([from, c * near, s * near], [to, c * far, s * far], 0.006, 0.0035, feathers));
    }
    p.push(limb([0.0, 0.0, 0.0], [0.012, 0.0, 0.0], fletch.shaft + 0.002, fletch.shaft + 0.002, if element == Some(Holy) { GOLD } else { BARK_DARK }));
    p.push(limb([0.0, 0.0, 0.0], [start, 0.0, 0.0], fletch.shaft, fletch.shaft, shaft));
    p
}

/// A crossbow's bolt along +x with its nock at 0: with bolt head `head`, a square point or a broad
/// barbed one, see QUARREL_HEADS, and its element showing as an arrow's does
fn quarrel(element: Option<Element>, head: usize, res: f64) -> Vec<Part> {
    arrow(element, if head == 0 { 1 } else { 2 }, res, &QUARREL_FLETCH)
}

/// A puff of smoke, lumpy, its middle at 0: white powder smoke without an element, or smoke the
/// color of one
fn smoke(element: Option<Element>, seed: u32) -> Vec<Part> {
    use Element::*;
    let (body, lumps) = match element {
        None => (CLAY, FUR_WHITE),
        Some(Fire) => (MORTAR, BARK_DARK),
        Some(Frost) => (FOAM, FUR_WHITE),
        Some(Storm) => (FUR_GRAY, CLAY),
        Some(Poison) => (CLAY, MOSS),
        Some(Arcane) => (FUR_GRAY, CLAY),
        Some(Void) => (FUR_DARK, MORTAR),
        Some(Blood) => (CLAY, MEAT),
        Some(Holy) => (WAX, LINEN),
    };
    // Lumps of different sizes off a smaller middle, so it's ragged rather than round
    let mut seed = seed;
    let mut p = Vec::new();
    for k in 0..9 {
        let d = [random(&mut seed) * 2.0 - 1.0, random(&mut seed) * 2.0 - 1.0, random(&mut seed) * 1.6 - 0.6];
        let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(0.1);
        let out = 0.035 + 0.025 * random(&mut seed);
        p.push(ball(d.map(|v| v / n * out), 0.018 + 0.02 * random(&mut seed), if k % 3 == 0 { body } else { lumps }));
    }
    p.push(ball([0.0, 0.0, 0.0], 0.045, body));
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
            models.push(voxelize(&arrow(element, head, ARROW_RES, &ARROW_FLETCH), ARROW_RES));
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
            models.push(voxelize(&arrow(element, head, HELD_ARROW_RES, &ARROW_FLETCH), HELD_ARROW_RES));
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
    // Crossbow bolts flying, then lying in the groove, held up close
    for res in [ARROW_RES, HELD_ARROW_RES] {
        for head in 0..QUARREL_HEADS {
            for &element in &looks {
                models.push(voxelize(&quarrel(element, head, res), res));
            }
        }
    }
    // Balls of lead, or glowing with an element
    for &element in &looks {
        models.push(voxelize(&[ball([0.0, 0.0, 0.0], 0.016, element.map_or(DARK_STEEL, Element::glow))], SHOT_RES));
    }
    for (k, &element) in looks.iter().enumerate() {
        models.push(voxelize(&smoke(element, 0x5e0c + k as u32), SMOKE_RES));
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
    // A muzzle flash: a white-hot core, flame round it, and tongues of it flaring out to the sides
    let mut p = vec![limb([0.0, 0.0, 0.0], [0.11, 0.0, 0.0], 0.016, 0.003, GLOW_HOLY)];
    for k in 0..5 {
        let a = k as f64 / 5.0 * TAU + 0.3;
        let out = [0.045, a.cos() * 0.05, a.sin() * 0.05];
        p.push(limb([0.004, 0.0, 0.0], out, 0.011, 0.002, if k % 2 == 0 { GLOW_FIRE } else { FLAME }));
    }
    p.push(limb([0.0, 0.0, 0.0], [0.17, 0.0, 0.0], 0.034, 0.002, GLOW_FIRE));
    models.push(voxelize(&p, MUZZLE_RES));
    // A shell: an iron ball bound with a band, its fuse burning out of the back
    let mut p = vec![ball([-0.05, 0.0, 0.0], 0.008, GLOW_FIRE), limb([-0.03, 0.0, 0.0], [-0.048, 0.0, 0.0], 0.004, 0.004, LINEN)];
    ring(&mut p, [0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 0.034, 0.004, BRONZE);
    p.push(ball([0.0, 0.0, 0.0], 0.034, DARK_STEEL));
    models.push(voxelize(&p, SHOT_RES));
    // A whip's lash: a stretch of it in each set at each thickness, then the crackers
    for set in 0..LASH_SETS {
        for radius in LASH_TAPERS {
            models.push(voxelize(&lash_segment(set, radius), LASH_RES));
        }
    }
    for kind in 0..CRACKERS {
        models.push(voxelize(&lash_cracker(kind), LASH_RES));
    }
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
/// Finer for balls and shells, which are small, smoke for being lumpy, and a muzzle flash, which
/// is right in front of the eye
pub const SHOT_RES: f64 = 64.0;
pub const SMOKE_RES: f64 = 48.0;
pub const MUZZLE_RES: f64 = 96.0;

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
    fn whips_reach_further_the_rarer_they_are() {
        let library = Library::parse(BUILT_IN).unwrap();
        let family = library.family("whip").expect("there's a whip family");
        let reach_by_grade = &library.families[family].grade_stats["range"];
        let mut seen = BTreeSet::new();
        for seed in 0..4000 {
            let weapon = library.generate(seed, Some(family), if seed % 2 == 0 { 1.0 } else { 6.0 });
            let profile = &weapon.profile;
            assert_eq!(profile.style, Style::Lash);
            assert_eq!(profile.grade, weapon.grade);
            let reach = profile.stat("range");
            assert!((reach - reach_by_grade[weapon.grade]).abs() < 0.5, "a {} whip reaching {reach} m", library.rarities[weapon.grade].id);
            // Its lash leaves the top of the handle, and has models to be drawn with
            let lash = weapon.look.lash.as_ref().expect("a whip has a lash");
            assert!(lash.root.z > weapon.look.grip.z + 0.05);
            assert!(lash.set < LASH_SETS && lash.cracker < CRACKERS);
            seen.insert(weapon.grade);
        }
        assert_eq!(seen.len(), library.rarities.len(), "whips of every rarity");
    }

    #[test]
    fn lash_models_are_where_combat_looks_for_them() {
        assert_eq!(effect_models().len(), LASH_CRACKERS + CRACKERS);
        // Elemental lashes glow, the rest are made of what they're made of
        let library = Library::parse(BUILT_IN).unwrap();
        for c in library.components.iter().filter(|c| c.slot_type == "lash") {
            let (set, _) = lash_models(&c.visual_profile, &Visual::default());
            assert_eq!(set >= LASH_GLOW, c.element.is_some(), "{}", c.id);
        }
    }

    #[test]
    fn crossbows_and_guns_load_and_shoot_their_own_ammo() {
        let library = Library::parse(BUILT_IN).unwrap();
        for id in ["crossbow", "arbalest", "repeater", "pistol", "musket", "blunderbuss", "handcannon"] {
            let family = library.family(id).expect("there's such a family");
            for seed in 0..40 {
                let weapon = library.generate(seed, Some(family), 3.0);
                let profile = &weapon.profile;
                assert_ne!(profile.build, Build::None, "{id}");
                assert!(profile.stat("reloadSpeed") > 0.0 && profile.stat("physicalDamage") > 0.0, "{id}");
                // Something that loads it, drawn with a frame of its own, and a muzzle out front
                let look = &weapon.look;
                let mover = look.mover.as_ref().expect("something loads it");
                assert!(mover.frame < look.frames.len(), "{id}");
                assert!(look.tip.x > look.grip.x, "{id} shoots forward of where it's held");
                if profile.shot() == Some(Shot::Quarrel) {
                    assert_eq!(look.nocks.len(), CROSSBOW_SPANNED + 1);
                    assert!(look.nocks[CROSSBOW_SPANNED].x < look.nocks[0].x, "{id}'s string comes back as it's spanned");
                }
                // Special ammo that fits it, adding its effects to what it does
                let shot = weapon.shot().unwrap();
                let ammo: Vec<usize> = (0..library.ammo.len()).filter(|&k| library.ammo[k].shot().ammo() == shot.ammo()).collect();
                assert!(ammo.len() >= 5, "{id} has special ammo");
                for k in ammo {
                    let loaded = library.loaded(&weapon, k);
                    for &d in &library.ammo[k].effect_defs {
                        assert!(loaded.effects.iter().any(|e| e.def == d), "{} adds its effects", library.ammo[k].id);
                    }
                    if let Some(e) = library.ammo[k].element {
                        assert_eq!(loaded.element, Some(e));
                    }
                }
            }
        }
        // A repeater holds several, a hand cannon's shells burst, a blunderbuss scatters
        let repeater = library.generate(1, library.family("repeater"), 1.0);
        assert!(repeater.profile.magazine() >= 5);
        let cannon = library.generate(1, library.family("handcannon"), 1.0);
        assert!(cannon.profile.has(Effect::Explosive) && cannon.shot() == Some(Shot::Shell));
        let blunderbuss = library.generate(1, library.family("blunderbuss"), 1.0);
        assert!(blunderbuss.profile.stat("pellets") > 4.0);
        // Bursting shot fires a single grenade, even from a blunderbuss
        let bursting = library.ammo.iter().position(|a| a.id == "bursting_shot").unwrap();
        assert!(library.loaded(&blunderbuss, bursting).stat("pellets") < 1.0);
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
