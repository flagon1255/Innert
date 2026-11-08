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
    /// An orb, pouring a beam out at the crosshair for as long as the button's held
    Channel,
    /// A tome, conjuring familiars that hang in the air casting at whatever comes near
    Summon,
    /// A totem, casting slow hexes that home in and eat away at what they catch
    Hex,
    /// A bell, rung harder the longer the button's held, its toll a shockwave all round
    Toll,
    /// A battle staff, swung like a polearm, its finishing blow loosing a spell
    Glaive,
    /// A flail, whirled right round so its head sweeps through everything about, and brought down
    /// overhead for a heavy blow
    Whirl,
    /// A maul, every blow brought down on the ground so a shockwave rolls out from it
    Quake,
    /// Bombs, lobbed in an arc to burst where they land
    Lob,
    /// A lightning rod, lightning leaping from it to what's aimed at and on through the pack
    Arc,
    /// A censer, swung on its chains, billowing clouds that hang where they drift
    Censer,
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
    /// A small crossbow held up in the off hand by its pistol grip, spanned by a lever folded
    /// under its tiller that the other hand pulls down and back
    Hand,
    /// Two prods one over the other, loosing a pair of bolts together, both spanned at once by a
    /// rack and pinion cranked at its side
    Twin,
    /// A great crossbow that has to be braced on the leg folded under its front before it'll
    /// loose, spanned by a capstan wheel at its butt turned round and round
    Siege,
    /// A crossbow loosing a barbed harpoon on a line, which reels in what it catches or the one
    /// holding it to what's too big, wound back onto its reel after
    Harpoon,
    /// A broad frame of five grooves in a fan, one heavy prod loosing all their bolts at once,
    /// spanned by a long lever swung out at its side
    Fan,
}

impl Build {
    /// What a weapon built this way shoots, for a ranged one: a bow shoots arrows
    pub fn shot(self) -> Shot {
        match self {
            Build::None => Shot::Arrow,
            Build::Cannon => Shot::Shell,
            Build::Pistol | Build::Musket | Build::Blunderbuss => Shot::Ball,
            _ => Shot::Quarrel,
        }
    }

    /// Whether it's a crossbow, of any build
    pub fn crossbow(self) -> bool {
        matches!(self, Build::Lever | Build::Crank | Build::Repeater | Build::Hand | Build::Twin | Build::Siege | Build::Harpoon | Build::Fan)
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
    /// Rooting what's hit to the spot a moment
    Snare,
    /// Projectiles glancing off what they hit to the nearest other
    Ricochet,
    /// Blows spilling over onto what's next to what they hit
    Cleave,
    /// Heavy blows and finishers sending a ring rolling out along the ground
    Shockwave,
    /// Shots splashing onto what's round what they hit, without blowing holes
    Splash,
    /// Hits leaving a pool of the weapon's element on the ground
    Pool,
    /// What it kills bursting into a cloud of its element
    Miasma,
    /// Hits dragging what's round them in
    Vortex,
    /// Blasts scattering bomblets that go off round about
    Cluster,
    /// Hits marking what they hit a while, so it takes more from every blow and shot
    Mark,
    /// Hits giving back health, a share of what they do
    Lifesteal,
    /// Hits on something bleeding tearing its wounds open, the bleeding to come all at once
    Rupture,
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
            "snare" => Effect::Snare,
            "ricochet" => Effect::Ricochet,
            "cleave" => Effect::Cleave,
            "shockwave" => Effect::Shockwave,
            "splash" => Effect::Splash,
            "pool" => Effect::Pool,
            "miasma" => Effect::Miasma,
            "vortex" => Effect::Vortex,
            "cluster" => Effect::Cluster,
            "mark" => Effect::Mark,
            "lifesteal" => Effect::Lifesteal,
            "rupture" => Effect::Rupture,
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
    /// How likely a part is to roll this rarity, see Library::odds()
    pub chance: f64,
    /// How much a part of this rarity adds to the weapon's grade
    power: f64,
    /// How much harder, and how much quicker, a part of this rarity makes the weapon, for its
    /// share of it, see Weapon::forging()
    #[serde(default = "one")]
    pub damage_scale: f64,
    #[serde(default = "one")]
    pub speed_scale: f64,
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
    /// How big a crossbow's bolts are next to a light crossbow's: a siege crossbow's great ones, a
    /// hand crossbow's darts
    #[serde(default = "one")]
    bolt: f64,
    /// Effects every one of the family has, whatever it's made of: an arbalest's bolts punching
    /// through, a hand cannon's shells bursting
    #[serde(default)]
    effects: Vec<String>,
    /// Indices of those effects
    #[serde(skip)]
    effect_defs: Vec<usize>,
    /// The word each size of it goes by in its name, see SIZES, from the smallest to the biggest,
    /// "" for none. Families without them take their kind's, see Family::size_word().
    #[serde(default)]
    size_words: Vec<String>,
    /// How it swings its heavy blow, if not as its style does, see combat.rs's heavy_keys()
    #[serde(default)]
    pub heavy: Heavy,
    /// For a tool, what it gathers: "wood", "stone" or "earth", see src\tools.rs. Tools come after
    /// every weapon in the list, are only ever made, never rolled as loot, and have no specials.
    #[serde(default)]
    pub tool: Option<String>,
}

/// How a melee weapon's heavy blow goes, if it has one of its own rather than its style's
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Heavy {
    /// As its style has it
    #[default]
    Style,
    /// A katana's: drawn low across the hip and cut out level, fast, all the way across
    Draw,
    /// A greatsword's: heaved round on a full turn, out of sight behind and round through in front
    Spin,
    /// An axe's: hauled back over the shoulder and hewn down at a slant
    Hew,
    /// A scythe's: swept low and wide right round in front, its blade reaping at knee height
    Sweep,
    /// A battle staff's: twirled round in front of the face, then brought down on its blade
    Twirl,
    /// A halberd's: raised high and brought down on its axe blade, then wrenched back by its hook
    Cleave,
    /// A maul's: set, heaved up over the head, held a moment at the top and brought down with
    /// everything behind it
    Heave,
    /// A flail's: whirled round overhead in a loop, then brought down
    Overhead,
    /// Drawn right back to the hip and driven out on a long step, the whole body behind it: a
    /// long-bladed spear's, or a needle of a sword's
    Lunge,
}

/// How a melee weapon's last blow of a chain goes, if not as its style's: what its head's like
/// decides it, see moves_from_parts()
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Finish {
    /// As its style has it
    #[default]
    Style,
    /// Driven straight out on a long step
    Lunge,
    /// Swept up from low on the left, rising through to high on the right
    Rising,
    /// Turned right round, out of sight behind and round through in front
    Spin,
    /// Raised high and brought straight down
    Slam,
}

/// How a staff, scepter or wand casts, see combat.rs's cast_keys(): what the hand does with it as
/// it's raised, the spell gathered and let go. Its focus, element and pace decide it, see
/// casting_of().
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Casting {
    /// Drawn back a little and thrust out at the crosshair
    #[default]
    Thrust,
    /// Drawn back and flicked from the wrist
    Flick,
    /// Brought in before the chest in both hands while the spell gathers, then pushed out
    Gather,
    /// Raised high overhead, held, and brought down to point: what comes down on a place
    Overhead,
    /// Swept round in from the side
    Sweep,
    /// Scooped up from low down
    Scoop,
    /// Held up and shown, as a scepter is, then levelled
    Present,
}

/// How a bow's drawn or a crossbow or gun loaded, from what its parts are like, see knack_of()
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Knack {
    /// As its kind has it
    #[default]
    Plain,
    /// Snapped up and drawn, or loaded, quick and close
    Quick,
    /// Leant into, braced and hauled: a heavy bow or a long gun
    Heavy,
    /// With a flourish: a bow canted over, a gun spun, a lever flicked round
    Showy,
}

/// How big a weapon was made: its look scaled by `scale` and its reach, pace, blows and how far
/// they throw what they hit by the others. Rolled from the weapon's seed, `chance` of the time.
pub struct SizeDef {
    pub scale: f64,
    pub reach: f64,
    pub speed: f64,
    pub damage: f64,
    pub knock: f64,
    /// Added to its knockback, in meters a second, for a melee weapon
    pub shove: f64,
    chance: f64,
}

/// The sizes a weapon can be, from the smallest to the biggest. The middle one is as the parts make it.
pub const SIZES: [SizeDef; 5] = [
    SizeDef { scale: 0.84, reach: 0.86, speed: 1.16, damage: 0.86, knock: 0.75, shove: 0.0, chance: 0.1 },
    SizeDef { scale: 0.92, reach: 0.93, speed: 1.07, damage: 0.94, knock: 0.9, shove: 0.0, chance: 0.2 },
    SizeDef { scale: 1.0, reach: 1.0, speed: 1.0, damage: 1.0, knock: 1.0, shove: 0.0, chance: 0.4 },
    SizeDef { scale: 1.09, reach: 1.1, speed: 0.92, damage: 1.09, knock: 1.2, shove: 1.0, chance: 0.2 },
    SizeDef { scale: 1.18, reach: 1.2, speed: 0.84, damage: 1.18, knock: 1.45, shove: 2.5, chance: 0.1 },
];

/// The size of SIZES a weapon made from `seed` is
pub fn roll_size(seed: u32) -> usize {
    let mut pick = random(&mut hash(seed ^ 0x51e_0b16)) * SIZES.iter().map(|s| s.chance).sum::<f64>();
    for (k, s) in SIZES.iter().enumerate() {
        if pick < s.chance {
            return k;
        }
        pick -= s.chance;
    }
    SIZES.len() / 2
}

impl Family {
    /// What a weapon of this family `size` of SIZES big is called for it, if anything
    pub fn size_word(&self, size: usize) -> &str {
        if let Some(word) = self.size_words.get(size) {
            return word;
        }
        let words: [&str; 5] = match (self.kind, self.style) {
            (Kind::Melee, Style::Stab) => ["Slim", "Short", "", "Long", "Heavy"],
            (Kind::Melee, Style::Smash | Style::Quake | Style::Chop | Style::Whirl) => ["Light", "Short", "", "Heavy", "Great"],
            (Kind::Melee, Style::Thrust | Style::Reap) => ["Short", "Light", "", "Long", "Great"],
            (Kind::Melee, Style::Lash) => ["Light", "Short", "", "Long", "Heavy"],
            (Kind::Melee, _) => ["Light", "Short", "", "Long", "Great"],
            (Kind::Ranged, Style::Lob) => ["Small", "Light", "", "Heavy", "Great"],
            (Kind::Ranged, _) if self.build == Build::None => ["Short", "Light", "", "Long", "Great"],
            (Kind::Ranged, _) => ["Light", "Compact", "", "Heavy", "Great"],
            (Kind::Magic, _) => ["Slender", "Short", "", "Tall", "Great"],
        };
        words[size.min(4)]
    }
}

fn middle() -> f64 {
    0.5
}

fn quarter() -> f64 {
    0.25
}

fn one() -> f64 {
    1.0
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
    /// For a crossbow's latch: the handle on the lever, cranks or wheel that span it, a round
    /// knob, a spoked grip or a crossbar
    crank: Option<String>,
    /// What's worked into a blade or head's faces: rust eating at it, engraving inlaid in its
    /// accent, or runes glowing in its fuller's glow, see inlay()
    pattern: Option<String>,
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

#[cfg(test)]
impl Component {
    /// The special attack it gives a weapon, if it's a legendary or mythic part that gives one
    pub fn special(&self) -> Option<Special> {
        self.special
    }

    /// Whether it fits a weapon of the family with id `family`
    pub fn fits(&self, family: &str) -> bool {
        self.compatible_families.iter().any(|f| f == family)
    }
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
    /// How it bursts apart into a spray of burning shards, if it does
    #[serde(default)]
    pub burst: Option<Burst>,
    /// Indices of its effects
    #[serde(skip)]
    pub effect_defs: Vec<usize>,
}

/// How bursting ammo flies apart into a spray of shards, each going off in a little blast where
/// it lands: partway to where it's aimed, as a grenade does, or where it strikes, throwing its iron
/// back out off what it hit
#[derive(Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Burst {
    /// Bursting in the air partway to where it's aimed, rather than where it strikes
    #[serde(default)]
    pub airburst: bool,
    /// How many shards, how far they spread from the way they're thrown, in degrees, and how much
    /// of the shot's damage each does
    pub fragments: usize,
    pub spread: f64,
    pub power: f64,
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
    /// How much luck lifts the rarer rarities, see odds(): each is `luck` to the power of this
    /// more likely again than the one below it, so 1 makes luck 2 double each next one up, and
    /// less is gentler
    #[serde(default = "one")]
    luck_curve: f64,
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

/// The most seeds Library::special_seed() tries for a weapon with a special attack: as lucky as
/// loot that's sure to have one is rolled, it's found in a few
const SPECIAL_TRIES: usize = 2000;

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
        // Tools last, so loot's rolled from the weapons before them as it always was
        let weapons = self.weapon_families();
        if let Some(f) = self.families[weapons..].iter().find(|f| f.tool.is_none()) {
            return Err(format!("{} comes after the tools: tools go last", f.id));
        }
        Ok(())
    }

    /// How many families are weapons, the tools coming after them
    fn weapon_families(&self) -> usize {
        self.families.iter().position(|f| f.tool.is_some()).unwrap_or(self.families.len())
    }

    pub fn family(&self, id: &str) -> Option<usize> {
        self.families.iter().position(|f| f.id == id)
    }

    /// How likely each rarity is to roll for a part, from common up, the more likely the rarer
    /// ones the luckier: `luck` 1 is the chances as they are, more makes each rarer one more
    /// likely again by `luck` to the power of luckCurve, less than 1 less likely, and none at all
    /// rolls only common parts
    pub fn odds(&self, luck: f64) -> Vec<f64> {
        let luck = luck.max(0.0);
        self.rarities.iter().enumerate().map(|(i, r)| r.chance * luck.powf(i as f64 * self.luck_curve)).collect()
    }

    /// A rarity picked by `odds`, as odds() gives them: none rarer than there are odds for
    fn roll_rarity(odds: &[f64], rng: &mut u32) -> usize {
        let total: f64 = odds.iter().sum();
        let mut pick = random(rng) * total;
        for (i, w) in odds.iter().enumerate() {
            if pick < *w {
                return i;
            }
            pick -= w;
        }
        0
    }

    /// Rolls a weapon from `seed`: of `family`, or any family for None. `luck` is as for
    /// odds(), from where it was found.
    pub fn generate(&self, seed: u32, family: Option<usize>, luck: f64) -> Weapon {
        self.generate_with(seed, family, &self.odds(luck))
    }

    /// The same, each part's rarity rolled by `odds`, see odds()
    pub fn generate_with(&self, seed: u32, family: Option<usize>, odds: &[f64]) -> Weapon {
        let (family, parts) = self.roll_with(seed, family, odds);
        Weapon::assemble(self, family, parts, seed)
    }

    /// What generate_with() would make from `seed`, worked out from its parts alone, quickly,
    /// without building it: its family, its grade, and its special attack if it has one
    pub fn preview(&self, seed: u32, family: Option<usize>, odds: &[f64]) -> (usize, usize, Option<Special>) {
        let (family, parts) = self.roll_with(seed, family, odds);
        let components: Vec<&Component> = parts.iter().map(|&p| &self.components[p]).collect();
        let (profile, _, grade) = Weapon::work_out(self, family, &components, roll_size(seed), seed);
        (family, grade, profile.special.map(|(s, _)| s))
    }

    /// `seed`, or if the weapon generate() makes from it with `luck` has no special attack, the
    /// first seed after it, as hash() leads on from it, whose weapon has: for loot that's sure to
    /// have one. As lucky as it's rolled, the rarer specials are as much rarer than the rest.
    pub fn special_seed(&self, seed: u32, family: Option<usize>, luck: f64) -> u32 {
        let odds = self.odds(luck);
        let mut next = seed;
        for _ in 0..SPECIAL_TRIES {
            if self.special_in(&self.roll_with(next, family, &odds).1).is_some() {
                return next;
            }
            next = hash(next ^ 0x5bec_1a1);
        }
        seed
    }

    /// Which family and parts generate() makes a weapon of
    fn roll(&self, seed: u32, family: Option<usize>, luck: f64) -> (usize, Vec<usize>) {
        self.roll_with(seed, family, &self.odds(luck))
    }

    /// The same, by `odds`, see odds()
    fn roll_with(&self, seed: u32, family: Option<usize>, odds: &[f64]) -> (usize, Vec<usize>) {
        let mut rng = hash(seed ^ 0x3ea9_7c11);
        // Any weapon, never a tool
        let weapons = self.weapon_families().max(1);
        let family = family.unwrap_or_else(|| (random(&mut rng) * weapons as f64) as usize % weapons);
        let id = &self.families[family].id;
        let mut parts = Vec::new();
        for slot in &self.families[family].slots {
            let tier = Library::roll_rarity(odds, &mut rng);
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

    /// The special attack the weapon generate() would make from `seed` would have, worked out from
    /// its parts alone, without building it: as work_out() picks it, the rarest part's that has one.
    /// Of family `family` if that's given, otherwise whichever the seed rolls.
    pub fn special_of(&self, seed: u32, family: Option<usize>, luck: f64) -> Option<Special> {
        self.special_in(&self.roll(seed, family, luck).1)
    }

    /// The special attack a weapon made of `parts` has, as work_out() picks it
    fn special_in(&self, parts: &[usize]) -> Option<Special> {
        parts.iter().map(|&p| &self.components[p]).filter(|c| c.special.is_some()).min_by_key(|c| std::cmp::Reverse(c.tier)).and_then(|c| c.special)
    }

    /// For looking at a part: a weapon made with the part with id `part`, of the first family it
    /// fits, `size` of SIZES big if that's given, rolled as luckily as it needs to turn up
    pub fn with_part(&self, part: &str, size: Option<usize>) -> Option<Weapon> {
        // Several joined by +, as `chain_lash+iron_star_fall`: one weapon made of all of them, of
        // the first one's family, the rest of its parts the commonest that roll
        if part.contains('+') {
            let ids: Vec<usize> = part.split('+').map(|id| self.components.iter().position(|c| c.id == id)).collect::<Option<_>>()?;
            let family = self.family(self.components[ids[0]].compatible_families.first()?)?;
            let seed = (0..400_000u32).map(hash).find(|&seed| size.is_none_or(|s| roll_size(seed) == s))?;
            let (_, mut parts) = self.roll(seed, Some(family), 0.0);
            for &i in &ids {
                let slot = self.families[family].slots.iter().position(|s| *s == self.components[i].slot_type)?;
                parts[slot] = i;
            }
            return Some(Weapon::assemble(self, family, parts, seed));
        }
        let index = self.components.iter().position(|c| c.id == part)?;
        let c = &self.components[index];
        let family = self.family(c.compatible_families.first()?)?;
        let luck = [1.0, 2.0, 3.0, 5.0, 8.0, 12.0][c.tier.min(5)];
        (0..400_000u32).map(hash).find(|&seed| size.is_none_or(|s| roll_size(seed) == s) && self.roll(seed, Some(family), luck).1.contains(&index)).map(|seed| self.generate(seed, Some(family), luck))
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
        "clay" => CLAY,
        "gravel" => GRAVEL,
        "glow_fire" => GLOW_FIRE,
        "glow_frost" => GLOW_FROST,
        "glow_storm" => GLOW_STORM,
        "glow_poison" => GLOW_POISON,
        "glow_arcane" => GLOW_ARCANE,
        "glow_void" => GLOW_VOID,
        "glow_blood" => GLOW_BLOOD,
        "glow_holy" => GLOW_HOLY,
        // Metal gone to rust, red-brown and dark
        "rust" => CLAY_POT,
        "rust_dark" => FUR_BROWN,
        "ivory" => FUR_CREAM,
        "ebony" => FUR_DARK,
        "ash_wood" => PALM_WOOD,
        "dead_wood" => DEAD_WOOD,
        "sandstone" => SANDSTONE,
        "limestone" => LIMESTONE,
        "horn" => FUR_GRAY,
        "copper" => FUR_GINGER,
        "pitch" => EYE,
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
    /// Its power on right click, if it's legendary or mythic or has a part that is, see Power
    pub power: Option<Power>,
    /// Its overall rarity, as Weapon::grade: what a whip can do grows with it
    pub grade: usize,
    /// How a crossbow or gun is built, and so loaded
    pub build: Build,
    /// How wide a gun's bore is round, in meters, which is how big its shot, its flash and its
    /// smoke are
    pub caliber: f64,
    /// Seconds a repeating crossbow's lever takes to work between shots
    pub cycle: f64,
    /// How big its bolts are, next to a light crossbow's
    pub bolt: f64,
    /// How much harder than its parts alone it throws back what it hits, for how big it was made,
    /// see SIZES
    pub heft: f64,
    /// What its parts' rarities make of its blows, as Weapon::forging() works it out: how much
    /// harder they hit, how much quicker it is, and how well this one came out of the forge
    pub quality: f64,
    pub pace: f64,
    pub forged: f64,
    /// The color of its rarity, as assets\weapons.json has it: what its blows trail from epic on
    pub color: [u8; 3],
    /// How its heavy blow goes, for a melee weapon, and its chain's last blow, as its parts make
    /// them, see moves_from_parts()
    pub heavy: Heavy,
    pub finish: Finish,
    /// How it's cast, for a staff, scepter or wand, see casting_of()
    pub casting: Casting,
    /// How it's drawn or loaded, for a bow, crossbow or gun, see knack_of()
    pub knack: Knack,
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
    /// Ice bursting out in a ring all round, freezing everything in it
    FrostNova,
    /// Lightning strung between every enemy round about, a web of it
    ThunderWeb,
    /// Vanishing and striking from right in front of what's aimed at
    BlinkStrike,
    /// Ghostly blades circling the player, cutting whatever comes close
    SpectralBlades,
    /// Phantoms of light standing round, casting at whatever's near
    PhantomHost,
    /// A beam draining the life of what's ahead, healing the player
    SoulSiphon,
    /// A bubble where time all but stops for what's caught in it
    Stasis,
    /// One great bolt, through everything in a line, pinning it
    Ballista,
    /// A swarm of shots seeking out everything round about
    SeekerSwarm,
    /// Knives whirling out from the player in a spiral
    BladeStorm,
    /// Shells falling in a line marching on ahead
    Barrage,
    /// A wall of fire across the way ahead
    Firewall,
    /// Bolts plunging down onto every enemy round about, three each
    BoltRain,
    /// One bolt glancing from enemy to enemy, and off walls
    Ricochet,
    /// A bolt bursting apart in flight into a fan of them
    SplitBolt,
    /// A weighted net bursting open where it's aimed, rooting everything under it
    NetBolt,
    /// A chain flung round everything near where it's aimed, dragging them together
    Shackle,
    /// A bolt that blows up in what it hits, the blast leaping on from one to the next
    ChainBlast,
    /// Everything in front hooked and dragged in, then a great sweep through them
    Harvest,
    /// A roar that staggers everything near and sends the player into a frenzy
    BattleCry,
    /// The weapon thrown spinning out ahead, through everything, and back to the hand
    HurledBlade,
    /// A beam swept across everything ahead
    Prism,
    /// Runes scattered on the ground that go off when anything comes near
    RuneMines,
    /// Fire leaping from what's aimed at on to whatever's near it, again and again
    Wildfire,
    /// Rings of upheaval rolling out round the player, one after another, each wider
    Earthquake,
    /// A comet of ice falling where it's aimed, freezing everything round and flinging shards
    Comet,
    /// Bombs lobbed out round where it's aimed, each bursting into bomblets
    ClusterBomb,
    /// A storm hanging over where it's aimed, lightning striking what's under it again and again
    Thunderstorm,
    /// A bubbling bog where it's aimed, miring and poisoning what's in it
    Mire,
    /// A sun over where it's aimed, searing what's under it with rays, then bursting
    Sunburst,
    /// A wave of force tearing through everything in front, shattering what's frozen
    Shatterwave,
    /// A whirlwind spinning off ahead, dragging in and battering what it passes
    Cyclone,
    /// The ground bursting into flame under every enemy round about, one after another
    Eruption,
    /// A ball of lightning drifting out ahead, lashing everything near it
    BallLightning,
    /// A swarm going from one enemy to the next, stinging everything round it
    Swarm,
    /// Boulders tumbling out ahead in a wide front, bowling over what's in their way
    Avalanche,
    /// A crescent of light cut loose from the blade, flying on ahead through everything in a line
    CrescentWave,
    /// A charge on ahead, the weapon levelled, carrying what it skewers along, thrown off at the end
    ImpalingCharge,
    /// The lash cracked out all round in rings, dragging in and staggering everything it reaches
    CoilingLash,
    /// Crack after crack at one enemy after another, faster and faster, each one bleeding
    Flay,
    /// Arrow after arrow loosed as fast as the hand can draw, at the crosshair
    Quickdraw,
    /// Every enemy round about marked a while: shots seek out what's marked and hit it harder
    HuntersMark,
    /// One shot at each enemy in sight, one after another, each one sure to strike true
    Reckoning,
    /// A spread of shot loosed point-blank, throwing everything in front back
    Grapeshot,
    /// Rockets going up and bursting in the sky over every enemy round about, raining fire on them
    Skyburst,
    /// A great keg set down ahead, its fuse burning, going off with a blast that throws all round it
    PowderKeg,
    /// Pillars of stone bursting up out of the ground under everything in front, one after another
    StonePillars,
    /// The weapon's head flung out on its chain to orbit round, wider and wider, battering all it meets
    WreckingOrbit,
    /// The lash whirled overhead and flung out round what's aimed at, which is yanked in to the feet
    LassoPull,
    /// The lash flung round what's aimed at and drawn tight, holding it fast and crushing it
    CoilBind,
    /// A crack so hard the air itself breaks, a cone of sound staggering and throwing back what's in front
    ThunderCrack,
    /// The lash splitting into snaking strands that slither off along the ground after what's near
    SerpentLash,
    /// The lash swept right round low over the ground, taking everything round off its feet
    TripSweep,
    /// A burning lash cracked down along the ground, leaving a line of fire and branding what it crosses
    BurningBrand,
    /// The lash caught on something high ahead and swung from, carrying the player on and down
    ChainSwing,
    /// A string of little charges flung out across the ground, going off one after another
    Firecrackers,
    /// An arrow bursting where it strikes into a ring of thorns flying out all round
    Thornburst,
    /// A frost bomb skimmed low that skips along the ground, ice bursting up bigger at every skip
    Rimeskip,
    /// A bomb cracking open into clockwork charges that scuttle off after what's near and cling on
    Scuttlers,
    /// Three arrows with burning wicks, sticking fast in what they strike and blowing up after
    Wickfire,
    /// An arrow bursting into a lantern of light that hangs where it struck, searing and healing
    Lantern,
    /// Two balls chained together, spinning apart as they fly, scything through a wide swath
    Chainshot,
    /// A cone of barbed caltrops blasted across the ground, crippling what walks over them
    Caltrops,
    /// A ball of void tearing a rift open where it strikes, the rift spitting shards at what's near
    Riftshot,
    /// Knives of shadow fanning out in the air, then flying one after another at what's near
    GloamingKnives,
    /// A lunge from a sheathed blade, the cut flying on as a line of frost that shatters a beat later
    SeveringDraw,
    /// Jab after jab as fast as the eye can follow, each flinging a venomous fang on ahead
    ViperThrusts,
    /// One great sweep, wisps of what it cut streaming back to heal, then flung on at what's left
    SoulGleaning,
    /// The weapon raised for lightning to strike it, then slammed down into a rolling ring of it
    Stormanvil,
    /// Three stakes loosed into the ground, lightning strung between them a while
    Stormstakes,
    /// A circle of glowing script written on the ground rune by rune, blazing up once it's full
    Litany,
    /// A knell of fire flung out, rolling out in a ring, everything it reaches ringing out in turn
    EmberKnell,
    /// A hex of blood binding everything near what it strikes together, so all suffer what one does
    Bloodthread,
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
            Special::FrostNova => "Frost Nova",
            Special::ThunderWeb => "Thunder Web",
            Special::BlinkStrike => "Blink Strike",
            Special::SpectralBlades => "Spectral Blades",
            Special::PhantomHost => "Phantom Host",
            Special::SoulSiphon => "Soul Siphon",
            Special::Stasis => "Stasis Field",
            Special::Ballista => "Ballista Bolt",
            Special::SeekerSwarm => "Seeker Swarm",
            Special::BladeStorm => "Blade Storm",
            Special::Barrage => "Creeping Barrage",
            Special::Firewall => "Wall of Flame",
            Special::BoltRain => "Hail of Quarrels",
            Special::Ricochet => "Ricochet Bolt",
            Special::SplitBolt => "Splitting Bolt",
            Special::NetBolt => "Snare Net",
            Special::Shackle => "Shackle Chain",
            Special::ChainBlast => "Chain Detonation",
            Special::Harvest => "Grim Harvest",
            Special::BattleCry => "Battle Cry",
            Special::HurledBlade => "Hurled Blade",
            Special::Prism => "Prismatic Sweep",
            Special::RuneMines => "Rune Mines",
            Special::Wildfire => "Wildfire",
            Special::Earthquake => "Earthquake",
            Special::Comet => "Frost Comet",
            Special::ClusterBomb => "Cluster Volley",
            Special::Thunderstorm => "Thunderstorm",
            Special::Mire => "Poison Mire",
            Special::Sunburst => "Sunburst",
            Special::Shatterwave => "Shatterwave",
            Special::Cyclone => "Cyclone",
            Special::Eruption => "Eruption",
            Special::BallLightning => "Ball Lightning",
            Special::Swarm => "Locust Swarm",
            Special::Avalanche => "Avalanche",
            Special::CrescentWave => "Crescent Wave",
            Special::ImpalingCharge => "Impaling Charge",
            Special::CoilingLash => "Coiling Lash",
            Special::Flay => "Flaying Storm",
            Special::Quickdraw => "Quickdraw",
            Special::HuntersMark => "Hunter's Mark",
            Special::Reckoning => "Reckoning",
            Special::Grapeshot => "Grapeshot",
            Special::Skyburst => "Skyburst",
            Special::PowderKeg => "Powder Keg",
            Special::StonePillars => "Rising Pillars",
            Special::WreckingOrbit => "Wrecking Orbit",
            Special::LassoPull => "Lasso Pull",
            Special::CoilBind => "Coil Bind",
            Special::ThunderCrack => "Thunder Crack",
            Special::SerpentLash => "Serpent Lash",
            Special::TripSweep => "Trip Sweep",
            Special::BurningBrand => "Burning Brand",
            Special::ChainSwing => "Chain Swing",
            Special::Firecrackers => "Firecracker String",
            Special::Thornburst => "Thornburst",
            Special::Rimeskip => "Rimeskip Charge",
            Special::Scuttlers => "Scuttling Charges",
            Special::Wickfire => "Wickfire Arrows",
            Special::Lantern => "Lantern Arrow",
            Special::Chainshot => "Chainshot",
            Special::Caltrops => "Caltrop Blast",
            Special::Riftshot => "Riftshot",
            Special::GloamingKnives => "Gloaming Knives",
            Special::SeveringDraw => "Severing Draw",
            Special::ViperThrusts => "Viper Thrusts",
            Special::SoulGleaning => "Soul Gleaning",
            Special::Stormanvil => "Stormanvil",
            Special::Stormstakes => "Stormstakes",
            Special::Litany => "Litany Circle",
            Special::EmberKnell => "Ember Knell",
            Special::Bloodthread => "Bloodthread Hex",
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
            Special::FrostNova => "Ice bursts out all round you, freezing everything within 9 meters.",
            Special::ThunderWeb => "Lightning strings itself between every enemy within 18 meters, striking each again and again.",
            Special::BlinkStrike => "Vanish and strike from right in front of what you aim at, three times.",
            Special::SpectralBlades => "Six ghostly blades circle you for 8 seconds, cutting whatever comes close.",
            Special::PhantomHost => "Three phantoms of light stand round you, casting at whatever's near.",
            Special::SoulSiphon => "A beam drains the life of what's ahead for 3 seconds, healing you.",
            Special::Stasis => "Time all but stops in a bubble where you aim, holding everything in it.",
            Special::Ballista => "One great bolt goes through everything in a line, pinning it.",
            Special::SeekerSwarm => "A swarm of shots seeks out every enemy round about.",
            Special::BladeStorm => "Knives whirl out from you in a spiral, through everything round you.",
            Special::Barrage => "Shells fall in a line marching on ahead of you.",
            Special::Firewall => "A wall of fire burns across the way ahead, scorching whatever passes.",
            Special::BoltRain => "Bolts plunge down on every enemy within 22 meters, three on each, one after another.",
            Special::Ricochet => "One bolt glances from enemy to enemy, up to eight times, harder with every one, and off walls between.",
            Special::SplitBolt => "A bolt bursts apart in flight into a fan of seven, each of them splitting again as it strikes.",
            Special::NetBolt => "A weighted net bursts open where you aim, rooting everything under it for 5 seconds.",
            Special::Shackle => "A chain flung round up to six enemies near where you aim drags them together into a heap.",
            Special::ChainBlast => "A bolt that blows up in what it hits, the blast leaping on from one enemy to the next.",
            Special::Harvest => "Hook everything in front within 10 meters, drag it in to you and cut through it all.",
            Special::BattleCry => "A roar that staggers everything within 12 meters and sends you into a frenzy: 50% faster attacks for 8 s.",
            Special::HurledBlade => "Hurl your weapon spinning out ahead through everything in its way; it comes back to your hand.",
            Special::Prism => "A beam of every color swept across everything ahead, burning, freezing and shocking it.",
            Special::RuneMines => "Six runes scattered round where you aim, each going off when an enemy comes near, for 20 s.",
            Special::Wildfire => "What you aim at bursts into flame, and the fire leaps on to whatever's near it, again and again.",
            Special::Earthquake => "Slam the ground: three rings of upheaval roll out round you, each wider, staggering everything they pass.",
            Special::Comet => "A comet of ice falls where you aim, freezing everything within 6 meters and flinging shards out all round.",
            Special::ClusterBomb => "Five bombs arc out round where you aim, each bursting into a scatter of bomblets.",
            Special::Thunderstorm => "A storm gathers where you aim for 6 s, lightning striking whatever's under it again and again.",
            Special::Mire => "The ground where you aim turns to a bubbling bog for 7 s, miring and poisoning everything in it.",
            Special::Sunburst => "A sun kindles over where you aim, searing everything under it with rays for 2.5 s, then bursts.",
            Special::Shatterwave => "A wave of force tears through everything in front, throwing it back; whatever's frozen shatters.",
            Special::Cyclone => "A whirlwind spins off ahead of you for 4 s, dragging in and battering everything it passes, then flings it out.",
            Special::Eruption => "The ground bursts into flame under every enemy within 16 meters, one after another, setting them alight.",
            Special::BallLightning => "A crackling ball of lightning drifts out ahead for 5 s, lashing everything within 6 meters of it.",
            Special::Swarm => "A swarm pours out and goes from one enemy to the next for 7 s, stinging and poisoning everything round it.",
            Special::Avalanche => "Boulders tumble out ahead of you in a wide front, bowling over and battering everything in their way.",
            Special::CrescentWave => "Cut a crescent of light loose from your blade; it flies on 20 meters through everything in its way.",
            Special::ImpalingCharge => "Charge on ahead with your weapon levelled, carrying everything you skewer along, and fling it off at the end.",
            Special::CoilingLash => "Crack your lash out all round you three times, each ring wider, dragging in and staggering whatever it catches.",
            Special::Flay => "Crack your lash at one enemy after another within reach, eight times, faster and faster, each cut bleeding.",
            Special::Quickdraw => "Loose ten arrows as fast as you can draw them, at the crosshair.",
            Special::HuntersMark => "Mark every enemy within 30 meters for 10 s: your shots seek out what's marked and hit it 50% harder.",
            Special::Reckoning => "One shot at each of up to eight enemies in sight, one after another, every one a critical hit.",
            Special::Grapeshot => "Loose a great spread of shot point-blank, battering and throwing back everything in front of you.",
            Special::Skyburst => "Rockets go up over every enemy within 20 meters and burst, raining fire down on them.",
            Special::PowderKeg => "Set down a great keg a few meters ahead; its fuse burns down and it goes off, throwing everything round it.",
            Special::StonePillars => "Slam the ground: pillars of stone burst up under everything in front of you, one after another, flinging it up.",
            Special::WreckingOrbit => "Fling your weapon's head out on its chain to orbit round you for 5 s, wider and wider, battering all it meets.",
            Special::LassoPull => "Whirl your lash overhead and fling it round what you aim at within 18 meters, then yank it in to your feet, reeling.",
            Special::CoilBind => "Fling your lash round what you aim at and draw it tight: it's held fast for 4 s and crushed as the coils close, and so is what's beside it.",
            Special::ThunderCrack => "Crack your lash so hard the air breaks: a cone of sound 14 meters long staggers and throws back everything in front.",
            Special::SerpentLash => "Your lash splits into five snaking strands that slither along the ground after the nearest enemies, biting and poisoning them.",
            Special::TripSweep => "Sweep your lash right round you low over the ground, knocking everything within reach off its feet.",
            Special::BurningBrand => "Crack a burning lash down along the ground ahead: a line of fire 10 meters long burns for 5 s, and what it crosses is branded and burns hard.",
            Special::ChainSwing => "Catch your lash on something high ahead and swing from it, carried on and down; you land with a crack that throws back what's round you.",
            Special::Firecrackers => "Fling a string of eight little charges across the ground ahead; they go off one after another, each throwing what's on it.",
            Special::Thornburst => "Loose an arrow that bursts where it strikes into a ring of thorns flying out all round, through everything near.",
            Special::Rimeskip => "Skim a frost bomb low across the ground: it skips four times, ice bursting up bigger at every skip, and shatters in a great freezing burst.",
            Special::Scuttlers => "Roll out a bomb that cracks open into six clockwork charges; they scuttle off after the nearest enemies, cling on, and go off.",
            Special::Wickfire => "Loose three arrows with burning wicks at the nearest enemies; each sticks fast where it strikes, spitting sparks, and blows up two seconds on.",
            Special::Lantern => "Loose an arrow that bursts into a lantern of light where it strikes; for 7 s it sears every enemy within 8 meters each second, and heals you in its glow.",
            Special::Chainshot => "Fire two balls chained together; they spin apart as they fly, scything through everything in a two-meter swath and knocking it off its feet.",
            Special::Caltrops => "Blast a cone of barbed caltrops across the ground ahead; for 8 s whatever walks over them is slowed and bleeds.",
            Special::Riftshot => "Fire a ball of void that tears a rift open where it strikes; for 5 s the rift spits shards at whatever's nearest, then snaps shut, dragging everything in.",
            Special::GloamingKnives => "Six knives of shadow fan out in the air above you, then fly one after another at the nearest enemies, bursting into darkness where they strike.",
            Special::SeveringDraw => "Draw in a flash with a lunge: the cut flies on 13 meters at what's in front as a line of frost hanging in the air through all in its way, and a heartbeat later shatters, cutting and freezing all it crossed.",
            Special::ViperThrusts => "Jab eight times as fast as the eye can follow, each thrust flinging a venomous fang on ahead through what's in front, poisoning it.",
            Special::SoulGleaning => "Reap through everything in front; each thing cut gives up a wisp that streams back to heal you, then every wisp gleaned is flung on at what's left.",
            Special::Stormanvil => "Raise your weapon for the lightning to strike it, then slam it down: a ring of lightning rolls out 14 meters, bolts leaping off it into all it passes.",
            Special::Stormstakes => "Loose three stakes into the ground ahead; for 6 s lightning strings between them, shocking whatever stands inside or crosses it, and they blow at the end.",
            Special::Litany => "Write a circle of glowing script where you aim, rune by rune, slowing everything inside, until the circle's full and blazes up in a column of light.",
            Special::EmberKnell => "Ring out a knell of fire where you aim: a ring of flame rolls out 10 meters, and whatever it reaches rings out in turn, its own ring rolling on from it.",
            Special::Bloodthread => "Fling a hex of blood that binds everything within 12 meters of where it strikes with threads of blood; for 6 s half of what any of them suffers, the rest suffer too.",
        }
    }
}

/// What a legendary or mythic weapon does on right click: a power of its family's, in its element,
/// its own besides its special attack on R. Where right click already does something, a heavy
/// blow or a bow's draw, that carries it; a crossbow or gun shoots it, bombs throw it and a weapon
/// that casts casts it. See combat/signatures/powers.rs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Power {
    /// A sword's: a crescent of its element flying on from the heavy blow, through all it meets
    Crescent,
    /// A greatsword's: a broad, slow wave of it, throwing back what it cuts
    Rending,
    /// A katana's: a thin cut flying far and fast
    WindCut,
    /// A dagger's: three small crescents in a fan
    BladeFan,
    /// A whip's: its crack running on low over the ground as a wave
    LashWave,
    /// An axe's, halberd's, scythe's or battle staff's: a spinning wheel of its element hurled out
    /// and coming back, cutting both ways
    Wheel,
    /// A spear's: a spear of its element hurled through everything in a line
    Javelin,
    /// A hammer's, maul's or flail's: the ground split open in a line ahead, erupting step by step
    Fissure,
    /// A bow's drawn full: an arrow like a comet, through everything, bursting at the end
    CometArrow,
    /// A crossbow's: a bolt that sticks where it strikes and bursts apart into shards
    BurstBolt,
    /// A pistol's: a glowing slug glancing from one enemy on to the next
    Ricochet,
    /// A musket's: a lance of its element through everything in a line, bursting along it after
    LanceShot,
    /// A blunderbuss's: a cone of its element's embers breathed out
    Breath,
    /// A hand cannon's: a shell lobbed high onto the crosshair, bursting wide
    Mortar,
    /// Bombs': a charge dragging everything round it in, then bursting
    Vortex,
    /// A staff's, scepter's or lightning rod's: a great slow orb, bursting wide where it lands
    Comet,
    /// A wand's, tome's or totem's: motes seeking out what's near
    Seekers,
    /// An orb's, bell's or censer's: a ring of its element rolling out all round
    Nova,
}

impl Power {
    /// Every one of them
    #[cfg(test)]
    pub const ALL: [Power; 18] = [
        Power::Crescent,
        Power::Rending,
        Power::WindCut,
        Power::BladeFan,
        Power::LashWave,
        Power::Wheel,
        Power::Javelin,
        Power::Fissure,
        Power::CometArrow,
        Power::BurstBolt,
        Power::Ricochet,
        Power::LanceShot,
        Power::Breath,
        Power::Mortar,
        Power::Vortex,
        Power::Comet,
        Power::Seekers,
        Power::Nova,
    ];

    /// The one a family's legendary and mythic weapons have, by its id
    pub fn of(family: &str) -> Option<Power> {
        Some(match family {
            "sword" => Power::Crescent,
            "greatsword" => Power::Rending,
            "katana" => Power::WindCut,
            "dagger" => Power::BladeFan,
            "whip" => Power::LashWave,
            "axe" | "halberd" | "scythe" | "battlestaff" => Power::Wheel,
            "spear" => Power::Javelin,
            "hammer" | "maul" | "flail" => Power::Fissure,
            "bow" => Power::CometArrow,
            "crossbow" | "arbalest" | "repeater" | "handcrossbow" | "twinprod" | "siegecrossbow" | "harpoon" | "boltthrower" => Power::BurstBolt,
            "pistol" => Power::Ricochet,
            "musket" => Power::LanceShot,
            "blunderbuss" => Power::Breath,
            "handcannon" => Power::Mortar,
            "bombs" => Power::Vortex,
            "staff" | "scepter" | "rod" => Power::Comet,
            "wand" | "tome" | "totem" => Power::Seekers,
            "orb" | "bell" | "censer" => Power::Nova,
            _ => return None,
        })
    }

    fn noun(self) -> &'static str {
        match self {
            Power::Crescent => "Crescent",
            Power::Rending => "Rending Wave",
            Power::WindCut => "Wind Cut",
            Power::BladeFan => "Blade Fan",
            Power::LashWave => "Lash Wave",
            Power::Wheel => "Wheel",
            Power::Javelin => "Javelin",
            Power::Fissure => "Fissure",
            Power::CometArrow => "Comet Arrow",
            Power::BurstBolt => "Burst Bolt",
            Power::Ricochet => "Ricochet",
            Power::LanceShot => "Lance Shot",
            Power::Breath => "Breath",
            Power::Mortar => "Mortar",
            Power::Vortex => "Vortex",
            Power::Comet => "Comet",
            Power::Seekers => "Seekers",
            Power::Nova => "Nova",
        }
    }

    /// What it's called, in `element`: a searing crescent, a rime wheel
    pub fn name(self, element: Option<Element>) -> String {
        let adjective = match element {
            Some(Element::Fire) => "Searing",
            Some(Element::Frost) => "Rime",
            Some(Element::Storm) => "Thunder",
            Some(Element::Poison) => "Venom",
            Some(Element::Arcane) => "Arcane",
            Some(Element::Void) => "Void",
            Some(Element::Blood) => "Blood",
            Some(Element::Holy) => "Radiant",
            None => "Keen",
        };
        format!("{adjective} {}", self.noun())
    }

    /// Seconds before it can be used again
    pub fn cooldown(self) -> f64 {
        match self {
            Power::Comet | Power::Seekers | Power::Nova => 6.0,
            Power::CometArrow | Power::Ricochet => 7.0,
            Power::Vortex | Power::Mortar => 9.0,
            _ => 8.0,
        }
    }

    /// How many times its spell's mana it costs, for a weapon that casts
    pub fn mana(self) -> f64 {
        match self {
            Power::Comet | Power::Seekers | Power::Nova => 2.0,
            _ => 0.0,
        }
    }

    /// How it's used, on a weapon built `build`
    pub fn how(self, build: Build) -> &'static str {
        match self {
            Power::Crescent | Power::Rending | Power::WindCut | Power::BladeFan | Power::LashWave | Power::Wheel | Power::Javelin | Power::Fissure => "Right click: the heavy blow carries it",
            Power::CometArrow => "Right click to draw, and loose drawn full",
            Power::BurstBolt if build == Build::Siege => "Right click to brace: the first bolt loosed braced is it",
            Power::Vortex => "Hold right click to aim, let go to throw it",
            _ => "Right click",
        }
    }

    /// What it does, in `element`
    pub fn description(self, element: Option<Element>) -> String {
        let what = match self {
            Power::Crescent => "A crescent of {e} flies 14 meters on from the heavy blow, cutting through everything in its way.",
            Power::Rending => "A broad, slow wave of {e} rolls 16 meters on from the heavy blow, cutting through everything and throwing it back.",
            Power::WindCut => "A thin cut of {e} flies 22 meters on from the heavy blow, fast, through everything in its way.",
            Power::BladeFan => "Three small crescents of {e} fan out 11 meters from the heavy blow, through everything they meet.",
            Power::LashWave => "The crack runs on 14 meters low over the ground as a wave of {e}, through everything in its way.",
            Power::Wheel => "A spinning wheel of {e} is hurled 13 meters out from the heavy blow and comes back, cutting everything both ways.",
            Power::Javelin => "A spear of {e} is hurled from the heavy thrust, through everything in a line 26 meters on, bursting at the end.",
            Power::Fissure => "The heavy blow splits the ground open in a line 12 meters ahead, {e} erupting from it step by step, throwing up what stands on it.",
            Power::CometArrow => "An arrow like a comet of {e}, through everything it meets, bursting where it comes down.",
            Power::BurstBolt => "A bolt of {e} that sticks where it strikes and a moment later bursts apart, shards of it flying all round.",
            Power::Ricochet => "A glowing slug of {e} that glances off what it hits on to the next enemy near, four times.",
            Power::LanceShot => "A lance of {e} through everything in a line, the line it leaves bursting a moment later.",
            Power::Breath => "Seven embers of {e} breathed out in a cone, bursting where they strike.",
            Power::Mortar => "A shell of {e} lobbed high onto the crosshair, bursting wide where it comes down.",
            Power::Vortex => "A charge of {e} that drags everything within 6 meters in to where it lands, then bursts.",
            Power::Comet => "A great slow orb of {e} that bursts wide where it lands.",
            Power::Seekers => "Five motes of {e} that each seek out something near, bursting on it.",
            Power::Nova => "A ring of {e} rolling out 9 meters all round you, through everything it reaches.",
        };
        let rider = match element {
            Some(Element::Fire) => "What it hits is set burning.",
            Some(Element::Frost) => "What it hits is chilled, frozen if it's chilled already.",
            Some(Element::Storm) => "Lightning leaps from what it hits to the next.",
            Some(Element::Poison) => "What it hits is poisoned.",
            Some(Element::Arcane) => "What it hits is thrown back.",
            Some(Element::Void) => "What it hits is dragged in.",
            Some(Element::Blood) => "What it hits bleeds, and you heal.",
            Some(Element::Holy) => "What it hits is staggered, and you heal a little.",
            None => "What it hits is staggered.",
        };
        let stuff = element.map_or("steel".to_string(), |e| e.name().to_lowercase());
        let cost = if self.mana() > 0.0 { format!(" Costs twice a spell's mana; every {:.0} s.", self.cooldown()) } else { format!(" Every {:.0} s.", self.cooldown()) };
        format!("{} {rider}{cost}", what.replace("{e}", &stuff))
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

    /// What it shoots, for a bow, crossbow or gun. Bombs are thrown, and take no ammo.
    pub fn shot(&self) -> Option<Shot> {
        (self.kind == Kind::Ranged && self.style != Style::Lob).then(|| self.build.shot())
    }

    /// How many shots a crossbow or gun holds loaded
    pub fn magazine(&self) -> u32 {
        self.stat("magazine").round().max(1.0) as u32
    }

    /// How well it came out of the forge, in a word, for its card and the like: Flawed, Plain,
    /// Fine or Masterwork, see FORGE_SPREAD
    pub fn make(&self) -> &'static str {
        match self.forged {
            f if f < 1.0 - 0.6 * FORGE_SPREAD => "Flawed",
            f if f < 1.0 + 0.25 * FORGE_SPREAD => "Plain",
            f if f < 1.0 + 0.7 * FORGE_SPREAD => "Fine",
            _ => "Masterwork",
        }
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
    /// How big it was made, of SIZES
    pub size: usize,
    pub profile: Rc<Profile>,
    pub tags: BTreeSet<String>,
    pub look: WeaponLook,
}

impl Weapon {
    pub fn assemble(library: &Library, family: usize, parts: Vec<usize>, seed: u32) -> Weapon {
        let fam = &library.families[family];
        let components: Vec<&Component> = parts.iter().map(|&p| &library.components[p]).collect();
        let size = roll_size(seed);
        let (profile, tags, grade) = Weapon::work_out(library, family, &components, size, seed);
        let name = weapon_name(library, family, &components, &profile.synergies, seed, size);
        let look = build_look(fam, &components);
        let scale = held_scale(fam, size, look.longest());
        let look = look.scaled(scale);
        Weapon { family, parts, seed, name, grade, size, profile: Rc::new(profile), tags, look }
    }

    /// What the rarity and make of a weapon's parts, and the forge, do to it: how many times
    /// harder it hits and how many times quicker it is than its numbers alone say, and how well
    /// this one, from `seed`, came out, within FORGE_SPREAD either way. Each part counts for its
    /// share, see slot_share(): a blade or a bow's limbs most, a core or a crystal next, a grip or
    /// a guard least. Its rarity hits harder and quicker by its damageScale and speedScale in
    /// assets\weapons.json, and a heavy blade or head of iron or stone slows it, a light one of
    /// bone, crystal or wood quickens it, see heaviness().
    fn forging(library: &Library, components: &[&Component], seed: u32) -> (f64, f64, f64) {
        let shares: Vec<f64> = components.iter().map(|c| slot_share(&c.slot_type)).collect();
        let total: f64 = shares.iter().sum::<f64>().max(1e-6);
        let mut quality = 0.0;
        let mut quickness = 0.0;
        let mut heft = 0.0;
        for (c, share) in components.iter().zip(&shares) {
            let rarity = &library.rarities[c.tier];
            quality += share / total * rarity.damage_scale;
            quickness += share / total * rarity.speed_scale;
            heft += share / total * heaviness(c);
        }
        let forged = 1.0 + FORGE_SPREAD * (2.0 * random(&mut hash(seed ^ 0xf0_29e5)) - 1.0);
        (quality, quickness / heft.powf(0.8), forged)
    }

    /// What a weapon made of `components` from `seed`, `size` of SIZES big, does: its stats,
    /// effects and synergies, its tags, and its grade
    fn work_out(library: &Library, family: usize, components: &[&Component], size: usize, seed: u32) -> (Profile, BTreeSet<String>, usize) {
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
        // Rarer parts hit harder and quicker, heavy ones slow it, and no two come out of the
        // forge quite alike. Bigger reaches further, hits harder and throws what it hits further,
        // but is slower.
        let (quality, pace, forged) = Weapon::forging(library, components, seed);
        let sized = &SIZES[size.min(SIZES.len() - 1)];
        let mut scale = |name: &str, by: f64| {
            if let Some(v) = stats.get_mut(name) {
                *v *= by;
            }
        };
        for name in ["physicalDamage", "elementalDamage", "spellDamage"] {
            scale(name, sized.damage * quality * forged);
        }
        for name in ["attackSpeed", "drawSpeed", "castSpeed", "reloadSpeed"] {
            scale(name, sized.speed * pace);
        }
        scale("knockback", sized.knock);
        match fam.kind {
            Kind::Melee => scale("range", sized.reach),
            // A longer bow or gun shoots its shots harder, a bigger staff's spells spread wider
            Kind::Ranged => scale("projectileSpeed", sized.reach.sqrt()),
            Kind::Magic => scale("area", sized.reach.sqrt()),
        }
        if fam.kind == Kind::Melee && sized.shove > 0.0 {
            *stats.entry("knockback".into()).or_insert(0.0) += sized.shove;
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
        // and synergies add up, and one very rare part lifts the whole weapon close to it. The
        // parts count as if there were four, so a crossbow's sight doesn't make it rarer.
        let parts = components.len().max(1) as f64;
        let power: f64 = components.iter().map(|c| library.rarities[c.tier].power).sum::<f64>() * 4.0 / parts + library.synergy_power * synergies.len() as f64;
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
        // A power on right click for a legendary or mythic weapon, or one with a part that is
        let legendary = library.rarities.iter().position(|r| r.id == "legendary").unwrap_or(4);
        let power = if grade >= legendary || special.is_some() || best >= legendary { Power::of(&fam.id) } else { None };
        let (build, caliber, cycle, bolt) = (fam.build, fam.caliber, fam.cycle, fam.bolt);
        let (heavy, finish) = moves_from_parts(fam, components);
        let casting = casting_of(fam, components, &stats, element);
        let knack = knack_of(fam, components);
        let profile = Profile {
            kind: fam.kind,
            style: fam.style,
            cone: fam.cone,
            targets: fam.targets,
            element,
            stats,
            effects,
            synergies,
            special,
            power,
            grade,
            build,
            caliber,
            cycle,
            bolt,
            heft: sized.knock,
            heavy,
            finish,
            casting,
            knack,
            quality,
            pace,
            forged,
            color: library.rarities[grade].color,
        };
        (profile, tags, grade)
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

/// How far either way a weapon can come out of the forge better or worse than its parts make it
pub const FORGE_SPREAD: f64 = 0.08;

/// How much of a weapon a part in `slot` is, for how much its rarity and make count for, see
/// Weapon::forging(): what strikes or shoots most, what powers or channels it next, what it's
/// held by or fitted with least
fn slot_share(slot: &str) -> f64 {
    match slot {
        "blade" | "limbs" | "prod" | "barrel" | "shaft" | "lash" | "casing" => 3.0,
        "core" | "crystal" | "focus" | "filling" | "string" => 2.0,
        _ => 1.0,
    }
}

/// How heavy a part is for its make, next to what a part in its slot is usually made of, steel for
/// a blade, wood for a bow's limbs or a staff, leather or wood for a grip: a head of stone or a
/// drum of iron slows a weapon, a blade of bone, crystal or wood quickens it. Only what's swung
/// or drawn counts: what's set into it, a gem or a rune, weighs nothing to speak of.
fn heaviness(c: &Component) -> f64 {
    let v = &c.visual_profile;
    // A whip's fall is light whatever it's made of, but what's tied to its end isn't: a lead
    // weight, an iron star or a blade drags the crack, barbs or a hook a little, a tuft of hair
    // or silk hardly at all
    if c.slot_type == "fall" {
        return match v.shape.as_deref().unwrap_or("cracker") {
            "weight" | "star" => 1.16,
            "blade" | "hook" => 1.08,
            "barbs" | "knots" => 1.04,
            "cracker" | "tassel" | "feather" => 0.94,
            _ => 1.0,
        };
    }
    let usual = match c.slot_type.as_str() {
        "blade" | "prod" | "barrel" | "casing" | "guard" => 1.0,
        "limbs" | "shaft" | "stock" => 0.9,
        "hilt" | "handle" | "lash" => 0.86,
        _ => return 1.0,
    };
    let mass = match v.material.as_deref().unwrap_or("steel") {
        "rock" | "limestone" | "sandstone" | "gravel" | "clay" => 1.3,
        "gold" => 1.18,
        "jade" => 1.12,
        "dark_steel" | "obsidian" => 1.1,
        "bronze" | "copper" => 1.05,
        "bone" | "ivory" | "horn" | "crystal" | "ice" => 0.84,
        "wood" | "ash_wood" | "birch" | "dark_bark" | "dead_wood" | "ebony" | "lacquer" | "wax" => 0.9,
        "leather" | "cloth" | "linen" | "bowstring" | "moss" | "fur_white" | "fur_tan" => 0.82,
        m if m.starts_with("glow_") => 0.88,
        _ => 1.0,
    };
    // A head made big for its kind weighs more again, a boulder, an anvil or a double bit, and a
    // slender one less. Only what's out of the way for its kind: every maul's head is big.
    let big = match v.shape.as_deref().unwrap_or("") {
        "maul_boulder" | "maul_anvil" | "maul_drum" | "double_axe" | "zweihander" | "broad" | "cleaver" | "flail_triple" | "flail_stone" => 1.06,
        // A lash of links, or of bone strung like a spine, swings heavy
        "chain" | "spine" => 1.1,
        "needle" | "tapered" | "leaf" | "sabre" => 0.95,
        _ => 1.0,
    };
    mass * big / usual
}

/// How a melee weapon made of `components` swings its heavy blow and its chain's last blow, from
/// what its head's like: a spear with a hooked, winged or forked head sweeps where one with a long
/// blade lunges, a curved sword draws and cuts where a needle lunges and a broad one hews, and so
/// on. Anything else goes as its family does.
fn moves_from_parts(family: &Family, components: &[&Component]) -> (Heavy, Finish) {
    if family.kind != Kind::Melee {
        return (family.heavy, Finish::Style);
    }
    let Some(head) = components.iter().find(|c| c.slot_type == "blade") else {
        return (family.heavy, Finish::Style);
    };
    let v = &head.visual_profile;
    let shape = v.shape.as_deref().unwrap_or("");
    let length = v.length.unwrap_or(0.0);
    let keep = family.heavy;
    match family.style {
        Style::Thrust if family.heavy == Heavy::Style => match shape {
            "hooked" | "winged" | "trident" | "fork" | "barbed" => (Heavy::Sweep, Finish::Rising),
            "glaive" => (Heavy::Sweep, Finish::Spin),
            _ if length >= 0.42 || shape == "needle" => (Heavy::Lunge, Finish::Lunge),
            _ => (keep, Finish::Style),
        },
        // A halberd: a bill hooks and sweeps, a moon blade's swung round
        Style::Thrust => match shape {
            "halberd_bill" => (Heavy::Sweep, Finish::Rising),
            "halberd_moon" => (Heavy::Spin, Finish::Spin),
            _ => (keep, Finish::Style),
        },
        Style::Slash => match (family.heavy, shape) {
            (Heavy::Style, "sabre" | "falx" | "hooked" | "serpent") => (Heavy::Draw, Finish::Spin),
            (Heavy::Style, "broad" | "cleaver" | "notched") => (Heavy::Hew, Finish::Slam),
            (Heavy::Style, "needle" | "tapered" | "leaf") => (Heavy::Lunge, Finish::Lunge),
            (Heavy::Style, "jagged" | "flame") => (keep, Finish::Rising),
            // A greatsword: a broad or notched one heaved overhead, a slender one lunged
            (Heavy::Spin, "broad" | "zweihander" | "notched") => (Heavy::Heave, Finish::Slam),
            (Heavy::Spin, "tapered" | "serpent") => (keep, Finish::Lunge),
            (Heavy::Spin, "forked" | "jagged" | "flame") => (keep, Finish::Rising),
            // A katana: an obsidian or icy one spun through its last cut
            (Heavy::Draw, _) if matches!(v.material.as_deref(), Some("obsidian" | "ice")) => (keep, Finish::Spin),
            (Heavy::Draw, _) if length >= 0.82 => (keep, Finish::Lunge),
            _ => (keep, Finish::Style),
        },
        Style::Chop => match shape {
            "double_axe" | "moon_axe" => (Heavy::Spin, Finish::Spin),
            "bearded" => (keep, Finish::Rising),
            "spike_axe" => (keep, Finish::Slam),
            _ => (keep, Finish::Style),
        },
        Style::Smash => match shape {
            "beaked" => (Heavy::Hew, Finish::Rising),
            "flanged" | "morning" => (Heavy::Heave, Finish::Slam),
            "spiked" => (keep, Finish::Spin),
            _ => (keep, Finish::Style),
        },
        Style::Stab => match shape {
            "sabre" | "hooked" | "serpent" | "flame" => (Heavy::Draw, Finish::Spin),
            "needle" | "spear" | "tapered" => (Heavy::Lunge, Finish::Lunge),
            "leaf" => (keep, Finish::Rising),
            _ => (keep, Finish::Style),
        },
        Style::Reap => match shape {
            "scythe_twin" => (Heavy::Spin, Finish::Spin),
            "scythe_saw" => (keep, Finish::Rising),
            _ => (keep, Finish::Style),
        },
        _ => (keep, Finish::Style),
    }
}

/// How a staff, scepter or wand made of `components` casts: what comes down on a place is called
/// down from overhead, big slow spells gathered in both hands first, and otherwise its focus and
/// its element say, a crook swept round and venom scooped up. Orbs, tomes, totems, bells and
/// the rest cast each their own way whatever they're made of, see combat.rs's target_pose().
fn casting_of(family: &Family, components: &[&Component], stats: &Stats, element: Option<Element>) -> Casting {
    if !matches!(family.style, Style::Cast | Style::Flick) {
        return Casting::Thrust;
    }
    let wand = family.style == Style::Flick;
    let focus = components.iter().find(|c| c.slot_type == "focus").and_then(|c| c.visual_profile.shape.as_deref()).unwrap_or("");
    let cast_speed = stats.get("castSpeed").copied().unwrap_or(2.0);
    let area = stats.get("area").copied().unwrap_or(1.0);
    if element == Some(Element::Holy) || matches!(focus, "halo" | "orrery" | "spire") || area >= 1.3 {
        return Casting::Overhead;
    }
    if wand {
        return match focus {
            "crescent" | "serpent" | "crook" => Casting::Sweep,
            "skull" | "cage" => Casting::Thrust,
            _ => Casting::Flick,
        };
    }
    if cast_speed < 1.75 || matches!(focus, "skull" | "cage" | "antlers") {
        return Casting::Gather;
    }
    match (focus, element) {
        ("crook" | "branches" | "serpent" | "crescent", _) => Casting::Sweep,
        (_, Some(Element::Poison)) => Casting::Scoop,
        (_, Some(Element::Storm | Element::Arcane | Element::Void)) => Casting::Sweep,
        _ if family.id == "scepter" => Casting::Present,
        _ => Casting::Thrust,
    }
}

/// How a bow made of `components` is drawn, or a crossbow or gun loaded: a short or flat bow
/// snapped up quick, a long, horn or steel one leant into, a recurve of something fine canted
/// over; a gun with a matchlock or a long barrel loaded with care, a wheel-lock or a runed one
/// with a flourish, a doglock quick; a crossbow with a pistol grip or a skeleton stock spanned
/// quick, a cheek-stocked one hauled, a lacquered or gilded one with a flourish
fn knack_of(family: &Family, components: &[&Component]) -> Knack {
    if family.kind != Kind::Ranged || family.style == Style::Lob {
        return Knack::Plain;
    }
    let part = |slot: &str| components.iter().find(|c| c.slot_type == slot).map(|c| &c.visual_profile);
    let shape = |v: Option<&Visual>| v.and_then(|v| v.shape.as_deref()).unwrap_or("").to_owned();
    let material = |v: Option<&Visual>| v.and_then(|v| v.material.as_deref()).unwrap_or("").to_owned();
    if family.build == Build::None {
        let limbs = part("limbs");
        let length = limbs.and_then(|v| v.length).unwrap_or(1.25);
        return match (shape(limbs).as_str(), material(limbs).as_str()) {
            ("flat" | "simple" | "branch", _) if length <= 1.2 => Knack::Quick,
            ("horn" | "bone" | "winged" | "long", _) => Knack::Heavy,
            ("recurve" | "serpent" | "crystal", "gold" | "jade" | "ebony" | "copper" | "ice" | "obsidian") => Knack::Showy,
            (_, "dark_steel" | "obsidian") => Knack::Heavy,
            ("serpent" | "crystal", _) => Knack::Showy,
            _ if length >= 1.45 => Knack::Heavy,
            _ if length <= 1.1 => Knack::Quick,
            _ => Knack::Plain,
        };
    }
    let (lock, stock) = (part("lock"), part("stock"));
    if family.build.crossbow() {
        return match (shape(stock).as_str(), material(stock).as_str()) {
            ("pistol" | "skeleton", _) => Knack::Quick,
            ("cheek" | "thumbhole", _) => Knack::Heavy,
            (_, "lacquer" | "jade") => Knack::Showy,
            _ if material(lock) == "gold" => Knack::Showy,
            _ => Knack::Plain,
        };
    }
    let long = part("barrel").and_then(|v| v.length).unwrap_or(1.0) >= 1.08;
    match shape(lock).as_str() {
        "match" => Knack::Heavy,
        "wheel" | "rune" => Knack::Showy,
        "" => Knack::Quick,
        _ if long => Knack::Heavy,
        _ => Knack::Plain,
    }
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or(String::new(), |c| c.to_uppercase().chain(chars).collect())
}

/// Prefix + size + core name + suffix, like Ashen Long Moonsabre of Thorns. A synergy, or a
/// legendary or mythic part, can name the weapon instead, like Eclipse, World-Eater.
fn weapon_name(library: &Library, family: usize, components: &[&Component], synergies: &[ActiveSynergy], seed: u32, size: usize) -> String {
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
    // Its size goes before what it is, unless that says it already: no Great Greatblade
    let word = fam.size_word(size);
    let core = if word.is_empty() || core.to_lowercase().contains(&word.to_lowercase()) { core } else { format!("{word} {core}") };
    // Each word only once: an affix that says again what's said already, as a Crescent Crescentaxe
    // or a Moonsabre of the Moon would, is passed over for the next part's
    let suffixes: Vec<String> = by_rarity.iter().filter_map(|c| c.naming.suffix.clone()).collect();
    let epithets: Vec<String> = by_rarity.iter().filter_map(|c| c.naming.epithet.clone()).collect();
    let signatures = synergies.iter().filter_map(|s| library.synergies[s.def].title.clone()).chain(by_rarity.iter().filter(|c| c.tier >= 4).filter_map(|c| c.naming.signature.clone()));
    for signature in signatures {
        let fresh = |s: &&String| !name_clash(s, &signature);
        if let Some(epithet) = epithets.iter().find(fresh) {
            return format!("{signature}, {epithet}");
        }
        if let Some(suffix) = suffixes.iter().find(fresh) {
            return format!("{signature} {suffix}");
        }
        if !name_clash(&signature, &core) {
            return format!("{signature}, the {core}");
        }
    }
    let prefix = by_rarity.iter().filter_map(|c| c.naming.prefix.clone()).find(|p| !name_clash(p, &core));
    let suffix = suffixes.into_iter().find(|s| !name_clash(s, &core) && prefix.as_ref().is_none_or(|p| !name_clash(s, p)));
    [prefix, Some(core), suffix].into_iter().flatten().collect::<Vec<_>>().join(" ")
}

/// The words of a weapon's name that say what it is, lower case, without the little ones
fn name_words(name: &str) -> Vec<String> {
    name.split(|c: char| !c.is_alphabetic()).map(str::to_lowercase).filter(|w| w.len() >= 3 && !matches!(w.as_str(), "the" | "and" | "who" | "whose")).collect()
}

/// A word with its endings taken off, as ashen to ash, thorns to thorn and stormy to storm
fn word_stem(word: &str) -> &str {
    for ending in ["ing", "en", "ed", "es", "s", "y"] {
        if let Some(stem) = word.strip_suffix(ending)
            && stem.len() >= 3
        {
            return stem;
        }
    }
    word
}

/// Whether two parts of a name say the same thing over: a word of one the same as one of the
/// other, or one starting or, if it's long enough to tell, containing the other, as Crescent and
/// Crescentaxe do
fn name_clash(a: &str, b: &str) -> bool {
    let (a, b) = (name_words(a), name_words(b));
    a.iter().any(|x| {
        b.iter().any(|y| {
            let (sx, sy) = (word_stem(x), word_stem(y));
            sx == sy || x.starts_with(sy) || y.starts_with(sx) || (sx.len() >= 5 && y.contains(sx)) || (sy.len() >= 5 && x.contains(sy))
        })
    })
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
    /// For crossbows, each bolt it holds ready at once: how far its nock is from the first's, and
    /// how far it's turned off straight ahead, in radians about z. One for most, two stacked for
    /// a twin-prod and five fanned out for a bolt-thrower.
    pub rails: Vec<(Vector3<f64>, f64)>,
    /// For a siege crossbow, the leg under its front that's let down to brace it: turned so far
    /// round its axis, by how braced it is, see Mover
    pub leg: Option<(Mover, f64)>,
    /// How much bigger than its parts make it it's drawn, for how big it was made, see held_scale()
    pub scale: f64,
}

impl WeaponLook {
    /// The same, `s` times as big. Its models stay as they are, their voxels drawn `s` times as
    /// big, so a bigger weapon costs nothing more to build or draw.
    fn scaled(mut self, s: f64) -> WeaponLook {
        if (s - 1.0).abs() < 1e-9 {
            return self;
        }
        self.res /= s;
        for (_, anchor) in &mut self.frames {
            *anchor *= s;
        }
        self.grip *= s;
        self.tip *= s;
        for nock in &mut self.nocks {
            *nock *= s;
        }
        if let Some(lash) = &mut self.lash {
            lash.root *= s;
            lash.thickness *= s;
        }
        for m in self.mover.iter_mut().chain(self.leg.iter_mut().map(|(m, _)| m)) {
            m.pivot *= s;
            m.handle *= s;
        }
        if let Some((point, _)) = &mut self.support {
            *point *= s;
        }
        for (offset, _) in &mut self.rails {
            *offset *= s;
        }
        self.scale *= s;
        self
    }

    /// How long it is, its longest side, in meters
    pub fn longest(&self) -> f64 {
        self.frames.first().map_or(0.0, |(m, _)| m.size.iter().copied().max().unwrap_or(0) as f64 / self.res)
    }
}

/// How much bigger than its parts make it a weapon of `family` `size` of SIZES big is drawn in the
/// hand: as much bigger as it is, but a long one less so, so a great spear or staff doesn't come
/// sweeping right across the view, and a bow, crossbow or gun only a little, as they're held up
/// close to the eye
fn held_scale(family: &Family, size: usize, longest: f64) -> f64 {
    let s = SIZES[size.min(SIZES.len() - 1)].scale;
    let damped = match family.kind {
        Kind::Ranged if family.style != Style::Lob => 0.5,
        _ if longest > 1.2 => 0.6,
        _ => 1.0,
    };
    1.0 + (s - 1.0) * damped
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
    let plain = |frames, res, grip, tip| WeaponLook { frames, res, grip, hold: Vector3::z(), tip, nocks: Vec::new(), lash: None, mover: None, support: None, rails: Vec::new(), leg: None, scale: 1.0 };
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
        // A bomb on the palm, a fresh one each throw
        Kind::Ranged if family.style == Style::Lob => {
            let (parts, grip, tip) = bomb(v("casing"), v("fuse"), v("filling"), v("core"));
            plain(vec![voxelize(&parts, SWORD_RES)], SWORD_RES, grip, tip)
        }
        Kind::Ranged if family.build.crossbow() => crossbow(family, v("prod"), v("stock"), v("lock"), v("sight"), v("core")),
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
        Kind::Magic if family.style == Style::Channel => {
            let (parts, grip, tip) = orb(v("shaft"), v("focus"), v("crystal"), v("binding"));
            plain(vec![voxelize(&parts, SWORD_RES)], SWORD_RES, grip, tip)
        }
        Kind::Magic if family.style == Style::Summon => {
            let (closed, open, grip, tip) = tome(v("shaft"), v("focus"), v("crystal"), v("binding"));
            plain(vec![voxelize(&closed, SWORD_RES), voxelize(&open, SWORD_RES)], SWORD_RES, grip, tip)
        }
        Kind::Magic if family.style == Style::Hex => {
            let (parts, grip, tip) = totem(v("shaft"), v("focus"), v("crystal"), v("binding"));
            plain(vec![voxelize(&parts, STAFF_RES)], STAFF_RES, grip, tip)
        }
        Kind::Magic if family.style == Style::Toll => {
            let (parts, grip, tip) = bell(v("shaft"), v("focus"), v("crystal"), v("binding"));
            plain(vec![voxelize(&parts, SWORD_RES)], SWORD_RES, grip, tip)
        }
        Kind::Magic if family.style == Style::Arc => {
            let (parts, grip, tip) = rod(v("shaft"), v("focus"), v("crystal"), v("binding"));
            plain(vec![voxelize(&parts, STAFF_RES)], STAFF_RES, grip, tip)
        }
        Kind::Magic if family.style == Style::Censer => {
            let (parts, grip, tip) = censer(v("shaft"), v("focus"), v("crystal"), v("binding"));
            plain(vec![voxelize(&parts, SWORD_RES)], SWORD_RES, grip, tip)
        }
        Kind::Magic => {
            let (mut parts, top, mut tip) = staff(v("shaft"), v("focus"), v("crystal"), v("binding"));
            let mut grip = v3(0.0, 0.0, top * family.grip);
            // A wand is a staff in miniature, a battle staff shorter, with a blade on top
            let mut res = STAFF_RES;
            if family.haft > 0.0 {
                let s = family.haft / top;
                parts = parts.iter().map(|p| p.scaled(s)).collect();
                grip *= s;
                tip *= s;
                res = SWORD_RES;
            }
            if family.style == Style::Glaive {
                let focus = v("focus");
                let blade = Visual {
                    shape: Some("glaive".into()),
                    length: Some(0.34),
                    width: Some(0.08),
                    material: focus.material.clone().filter(|m| material(m).is_some_and(|m| m != WOOD)).or(Some("steel".into())),
                    edge: Some("steel".into()),
                    fuller: v("crystal").gem.clone(),
                    ..Default::default()
                };
                let mut blade_parts = Vec::new();
                tip = sword_blade(&mut blade_parts, &blade, tip.z + 0.06, None);
                blade_parts.push(limb([0.0, 0.0, tip.z - 0.4], [0.0, 0.0, tip.z - 0.32], 0.02, 0.014, DARK_STEEL));
                parts.splice(0..0, blade_parts);
                res = STAFF_RES;
            }
            plain(vec![voxelize(&parts, res)], res, grip, tip)
        }
    }
}

/// An orb floating over a short cradle held in the hand, set in a ring, prongs, a cage or an
/// orrery by its focus, bands of its binding round the cradle. Returns its parts, where it's
/// held, and the middle of the orb.
fn orb(shaft: &Visual, focus: &Visual, crystal: &Visual, binding: &Visual) -> (Vec<Part>, Vector3<f64>, Vector3<f64>) {
    let mut p = Vec::new();
    let wood = mat(&shaft.material, WOOD);
    let setting = mat(&focus.material, GOLD);
    let accent = mat(&focus.accent, setting);
    let gem = mat(&crystal.gem, CRYSTAL);
    let r = 0.035 + crystal.size.unwrap_or(0.05) * 0.3;
    let c = [0.0, 0.0, 0.13 + r];
    // The orb first, so its setting doesn't hide it
    p.push(ball(c, r, gem));
    match focus.shape.as_deref().unwrap_or("ring") {
        "prongs" | "crook" | "antlers" => {
            for k in 0..3 {
                let a = k as f64 / 3.0 * TAU;
                let (x, y) = (a.cos(), a.sin());
                curve(&mut p, &[[x * 0.02, y * 0.02, 0.12], [x * r * 1.05, y * r * 1.05, c[2] - r * 0.3], [x * r * 0.8, y * r * 0.8, c[2] + r * 0.65]], 0.006, 0.003, setting);
            }
        }
        "cage" | "skull" => {
            for k in 0..4 {
                let a = (k as f64 + 0.5) / 4.0 * TAU;
                let points: Vec<[f64; 3]> = (0..=8).map(|j| {
                    let b = -PI * 0.5 + PI * j as f64 / 8.0;
                    [a.cos() * b.cos() * r * 1.08, a.sin() * b.cos() * r * 1.08, c[2] + b.sin() * r * 1.08]
                }).collect();
                curve(&mut p, &points, 0.0045, 0.0045, if k % 2 == 0 { setting } else { accent });
            }
        }
        "orrery" | "eye" => {
            ring(&mut p, c, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], r * 1.3, 0.005, setting);
            ring(&mut p, c, [0.0, 1.0, 0.0], [0.5, 0.0, 0.86], r * 1.15, 0.005, accent);
            p.push(ball([c[0] + r * 1.3, 0.0, c[2]], 0.009, accent));
        }
        "spire" | "crescent" => {
            let points: Vec<[f64; 3]> = (0..=14).map(|k| {
                let a = (-40.0 + 260.0 * k as f64 / 14.0).to_radians() - PI * 0.5;
                [a.cos() * r * 1.25, 0.0, c[2] + a.sin() * r * 1.25]
            }).collect();
            curve(&mut p, &points, 0.008, 0.004, setting);
        }
        _ => ring(&mut p, c, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], r * 1.12, 0.006, setting),
    }
    // A cup it hovers over, then the cradle, bound
    let bm = mat(&binding.material, LINEN);
    for k in 0..3 {
        p.push(ellipsoid([0.0, 0.0, 0.03 + k as f64 * 0.03], [0.021, 0.021, 0.006], if k % 2 == 0 { bm } else { mat(&binding.accent, bm) }));
    }
    p.push(ellipsoid([0.0, 0.0, 0.12], [0.04, 0.04, 0.014], setting));
    p.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, 0.12], 0.016, 0.02, wood));
    (p, v3(0.0, 0.0, 0.05), Vector3::from(c))
}

/// A tome held by its spine, its front cover to -y, which is towards whoever holds it: shut, then
/// open, its pages written with glowing runes. Bound in leather or whatever its shaft is made of,
/// its corners and clasp its focus's, its crystal set in the cover. Returns the two, where it's
/// held, and where what it conjures comes out of it.
fn tome(shaft: &Visual, focus: &Visual, crystal: &Visual, binding: &Visual) -> (Vec<Part>, Vec<Part>, Vector3<f64>, Vector3<f64>) {
    let cover = match mat(&shaft.material, WOOD) {
        WOOD => LEATHER,
        m => m,
    };
    let metal = mat(&focus.material, GOLD);
    let gem = mat(&crystal.gem, CRYSTAL);
    let ink = if glowing(gem).is_some() { gem } else { mat(&binding.accent, GLOW_ARCANE) };
    let (w, h) = (0.11, 0.16);
    // Shut: the cover facing out, its corners capped, a clasp across, the crystal in the middle
    let mut closed = Vec::new();
    closed.push(ball([w * 0.5, -0.004, h * 0.5], crystal.size.unwrap_or(0.05).clamp(0.014, 0.03), gem));
    for (x, z) in [(w - 0.012, 0.012), (w - 0.012, h - 0.012), (0.012, 0.012), (0.012, h - 0.012)] {
        closed.push(cuboid([x - 0.014, -0.003, z - 0.014], [x + 0.014, 0.038, z + 0.014], metal));
    }
    closed.push(cuboid([w - 0.004, -0.003, h * 0.45], [w + 0.01, 0.038, h * 0.55], metal));
    closed.push(cuboid([0.004, 0.006, 0.006], [w - 0.002, 0.029, h - 0.006], WAX));
    closed.push(cuboid([-0.008, 0.0, 0.0], [w, 0.035, h], cover));
    // Open: the pages to -y either side of the spine, runes on them, the crystal over the spine
    let mut open = Vec::new();
    open.push(ball([0.0, 0.004, h + 0.012], crystal.size.unwrap_or(0.05).clamp(0.012, 0.025), gem));
    for side in [-1.0, 1.0] {
        for row in 0..4 {
            for k in 0..3 {
                let x = side * (0.022 + 0.028 * k as f64);
                open.push(ball([x, -0.0005, h - 0.03 - row as f64 * 0.03], 0.004, ink));
            }
        }
        open.push(cuboid([side * w - 0.008, 0.016, -0.004], [side * w + 0.008, 0.026, h + 0.004], metal));
    }
    open.push(cuboid([-0.008, -0.002, 0.0], [0.008, 0.012, h], AIR));
    open.push(cuboid([-w + 0.004, 0.004, 0.006], [w - 0.004, 0.018, h - 0.006], WAX));
    open.push(cuboid([-w, 0.018, -0.004], [w, 0.026, h + 0.004], cover));
    (closed, open, v3(0.0, 0.017, 0.04), v3(0.0, -0.06, h * 0.6))
}

/// A totem: a thick pole carved into faces one over another, their eyes its crystal, feathers or
/// whatever its binding is hanging off it, and its focus on top. Returns its parts, where it's
/// held, and its top, where hexes come out.
fn totem(shaft: &Visual, focus: &Visual, crystal: &Visual, binding: &Visual) -> (Vec<Part>, Vector3<f64>, Vector3<f64>) {
    let mut p = Vec::new();
    let wood = mat(&shaft.material, WOOD);
    let paint = mat(&shaft.accent, mat(&focus.material, TEMPLE_RED));
    let gem = mat(&crystal.gem, CRYSTAL);
    let bm = mat(&binding.material, FUR_WHITE);
    let top = 0.72;
    // The faces, their eyes and mouths first so they show, each a little wider than the pole
    for (k, z) in [0.3, 0.45, 0.6].into_iter().enumerate() {
        for s in [-1.0, 1.0] {
            p.push(ball([0.045, s * 0.016, z + 0.012], 0.0075, gem));
        }
        p.push(cuboid([0.035, -0.014, z - 0.024], [0.06, 0.014, z - 0.016], AIR));
        p.push(limb([0.04, 0.0, z + 0.008], [0.062, 0.0, z - 0.004], 0.008, 0.004, wood));
        p.push(ellipsoid([0.0, 0.0, z], [0.05, 0.045, 0.06], if k % 2 == 0 { wood } else { paint }));
    }
    // Feathers hanging off the top face
    for s in [-1.0, 1.0] {
        p.push(limb([0.0, s * 0.045, 0.62], [0.01, s * 0.08, 0.5], 0.01, 0.004, bm));
    }
    // The focus on top: horns, a skull, or the crystal held up on its own
    let c = [0.0, 0.0, top + 0.05];
    p.push(ball(c, crystal.size.unwrap_or(0.05).clamp(0.025, 0.045), gem));
    match focus.shape.as_deref().unwrap_or("") {
        "antlers" | "crescent" | "horns" => {
            for s in [-1.0, 1.0] {
                curve(&mut p, &[[0.0, s * 0.03, top - 0.02], [0.0, s * 0.1, top + 0.06], [0.02, s * 0.12, top + 0.16]], 0.012, 0.004, mat(&focus.material, BONE));
            }
        }
        "skull" => p.push(ellipsoid([0.0, 0.0, top - 0.01], [0.05, 0.045, 0.045], BONE)),
        _ => {
            for s in [-1.0, 1.0] {
                p.push(limb([0.0, s * 0.03, top - 0.01], [0.0, s * 0.11, top + 0.05], 0.012, 0.003, bm).stretched([1.0, 1.0, 0.5]));
            }
        }
    }
    p.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, top], 0.026, 0.034, wood));
    (p, v3(0.0, 0.0, 0.14), Vector3::from(c))
}

/// A handbell: a handle held upright, the bell hanging under the hand, its mouth down, a glowing
/// clapper inside it made of its crystal, a band of its binding round its shoulder. Returns its
/// parts, where it's held, and its mouth.
fn bell(shaft: &Visual, focus: &Visual, crystal: &Visual, binding: &Visual) -> (Vec<Part>, Vector3<f64>, Vector3<f64>) {
    let mut p = Vec::new();
    let wood = mat(&shaft.material, WOOD);
    let metal = match mat(&focus.material, BRONZE) {
        WOOD | BIRCH_BARK | BARK_DARK | LEATHER | LINEN | CLOTH => BRONZE,
        m => m,
    };
    let gem = mat(&crystal.gem, CRYSTAL);
    let band = mat(&binding.material, mat(&binding.accent, GOLD));
    // Hollow, the clapper hanging inside it, seen from under
    // A domed crown, a waist widening slowly, then flaring out to a thick lip
    let mouth = -0.13;
    let flare = |t: f64| 0.026 + 0.016 * t + 0.032 * t.powi(4);
    p.push(ball([0.0, 0.0, mouth + 0.018], crystal.size.unwrap_or(0.05).clamp(0.014, 0.022), gem));
    p.push(limb([0.0, 0.0, mouth - 0.01], [0.0, 0.0, -0.03], flare(1.0) - 0.008, flare(0.2) - 0.008, AIR));
    p.push(limb([0.0, 0.0, -0.03], [0.0, 0.0, mouth + 0.012], 0.003, 0.003, DARK_STEEL));
    p.push(limb([0.0, 0.0, -0.04], [0.0, 0.0, -0.028], flare(0.15) + 0.004, flare(0.1) + 0.004, band));
    let steps = 8;
    for k in 0..steps {
        let (t0, t1) = (k as f64 / steps as f64, (k + 1) as f64 / steps as f64);
        p.push(limb([0.0, 0.0, -0.012 + (mouth + 0.012) * t0], [0.0, 0.0, -0.012 + (mouth + 0.012) * t1], flare(t0), flare(t1), metal));
    }
    p.push(ellipsoid([0.0, 0.0, -0.012], [0.027, 0.027, 0.016], metal));
    p.push(limb([0.0, 0.0, mouth - 0.003], [0.0, 0.0, mouth + 0.006], flare(1.0) + 0.004, flare(1.0), metal));
    // The handle over it, and a knob on top
    p.push(ball([0.0, 0.0, 0.17], 0.02, metal));
    p.push(limb([0.0, 0.0, -0.01], [0.0, 0.0, 0.16], 0.015, 0.018, wood));
    (p, v3(0.0, 0.0, 0.09), v3(0.0, 0.0, mouth))
}

/// The metal a part made of `m` is drawn in where it has to be metal: bronze for wood, cloth and
/// the like
fn metal_or_bronze(m: u8) -> u8 {
    match m {
        WOOD | BIRCH_BARK | BARK_DARK | LEATHER | LINEN | CLOTH | FUR_WHITE | FUR_TAN | LEAVES | MOSS => BRONZE,
        m => m,
    }
}

/// `n` directions spread evenly over a sphere
fn sphere_points(n: usize) -> Vec<[f64; 3]> {
    (0..n)
        .map(|k| {
            let z = 1.0 - 2.0 * (k as f64 + 0.5) / n as f64;
            let r = (1.0 - z * z).sqrt();
            let a = k as f64 * 2.399_963;
            [a.cos() * r, a.sin() * r, z]
        })
        .collect()
}

/// A flail's head on its chain, rising from a cap on the end of the haft at `z_top`: the chain
/// flung out straight, as it is while it's whirled, its links turned one way then the other, and
/// on its end a spiked ball, a star of long spikes, two balls on two chains, a caged brazier
/// glowing inside, or a lump of stone bound with iron. Returns the middle of the head.
fn flail_head(p: &mut Vec<Part>, v: &Visual, z_top: f64, infuse: Option<u8>) -> Vector3<f64> {
    let chain = v.length.unwrap_or(0.3);
    let shape = v.shape.as_deref().unwrap_or("flail");
    let twin = shape == "flail_twin";
    let triple = shape == "flail_triple";
    let r = v.width.unwrap_or(0.14) * 0.5 * if twin { 0.72 } else if triple { 0.58 } else { 1.0 };
    let body = mat(&v.material, match shape {
        "flail_stone" => ROCK,
        "flail_skull" => BONE,
        "flail_crystal" => CRYSTAL,
        _ => DARK_STEEL,
    });
    let spikes = mat(&v.edge, STEEL);
    let links = mat(&v.accent, DARK_STEEL);
    let base = z_top + 0.015;
    let offsets: &[f64] = if twin {
        &[-0.06, 0.06]
    } else if triple {
        &[-0.085, 0.0, 0.085]
    } else {
        &[0.0]
    };
    let mut middle = Vector3::zeros();
    for (k, &off) in offsets.iter().enumerate() {
        // Three on chains of three lengths, so they don't knock together
        let chain = if triple { chain * (0.85 + 0.15 * k as f64) } else { chain };
        let c = [off, 0.0, base + chain + r];
        // The head first, so its spikes and bands show over it
        match shape {
            // A skull, its eyes glowing, its jaw bound shut with iron
            "flail_skull" => {
                let glow = mat(&v.gem, infuse.unwrap_or(GLOW_BLOOD));
                for s in [-1.0, 1.0] {
                    p.push(ball([c[0] + s * r * 0.38, c[1] + r * 0.82, c[2] + r * 0.15], r * 0.24, glow));
                }
                ring(p, [c[0], c[1], c[2] - r * 0.45], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], r * 0.82, 0.006, spikes);
                p.push(ellipsoid([c[0], c[1] + r * 0.25, c[2] - r * 0.6], [r * 0.62, r * 0.6, r * 0.35], body));
                p.push(ellipsoid(c, [r * 0.95, r * 1.05, r], body));
            }
            // A cluster of crystals growing out of an iron core, glowing
            "flail_crystal" => {
                for (j, d) in sphere_points(9).into_iter().enumerate() {
                    let long = if j % 3 == 0 { 1.7 } else { 1.3 };
                    let at = |s: f64| [c[0] + d[0] * r * s, c[1] + d[1] * r * s, c[2] + d[2] * r * s];
                    p.push(limb(at(0.3), at(long), r * 0.32, 0.003, body));
                }
                p.push(ball(c, r * 0.7, spikes));
            }
            "flail_brazier" => {
                let glow = mat(&v.gem, infuse.unwrap_or(GLOW_FIRE));
                for k in 0..5 {
                    let a = k as f64 / 5.0 * TAU;
                    let points: Vec<[f64; 3]> = (0..=8)
                        .map(|j| {
                            let b = -PI * 0.5 + PI * j as f64 / 8.0;
                            [c[0] + a.cos() * b.cos() * r * 1.06, c[1] + a.sin() * b.cos() * r * 1.06, c[2] + b.sin() * r * 1.06]
                        })
                        .collect();
                    curve(p, &points, 0.006, 0.006, body);
                }
                ring(p, [c[0], c[1], c[2] + r * 0.2], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], r * 1.08, 0.006, spikes);
                p.push(ball(c, r * 0.84, glow));
            }
            "flail_stone" => {
                for w in [[0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
                    ring(p, c, [1.0, 0.0, 0.0], w, r * 1.02, 0.007, spikes);
                }
                let mut seed = 0x5707;
                for k in 0..6 {
                    let d = sphere_points(6)[k];
                    let s = 0.45 + 0.15 * random(&mut seed);
                    p.push(ball([c[0] + d[0] * r * 0.5, c[1] + d[1] * r * 0.5, c[2] + d[2] * r * 0.5], r * s, if k % 2 == 0 { GRAVEL } else { body }));
                }
                p.push(ellipsoid(c, [r * 1.05, r * 0.92, r], body));
            }
            _ => {
                let star = shape == "flail_star";
                for (k, d) in sphere_points(if star { 8 } else { 14 }).into_iter().enumerate() {
                    let long = if star { 2.1 } else if k % 2 == 0 { 1.55 } else { 1.4 };
                    let at = |s: f64| [c[0] + d[0] * r * s, c[1] + d[1] * r * s, c[2] + d[2] * r * s];
                    p.push(limb(at(0.8), at(long), if star { 0.016 } else { 0.012 }, 0.002, spikes));
                }
                if star {
                    for d in sphere_points(12) {
                        p.push(ball([c[0] + d[0] * r, c[1] + d[1] * r, c[2] + d[2] * r], 0.009, spikes));
                    }
                }
                p.push(ball(c, r, body));
            }
        }
        // The chain, from the cap out to the head
        let along = v3(off, 0.0, chain);
        let d = along.normalize();
        let across = [d.z, 0.0, -d.x];
        let n = ((chain / 0.026).round() as usize).max(3);
        for k in 0..n {
            let at = v3(0.0, 0.0, base) + along * ((k as f64 + 0.5) / n as f64);
            let w = if k % 2 == 0 { across } else { [0.0, 1.0, 0.0] };
            ring(p, [at.x, at.y, at.z], [d.x, d.y, d.z], w, 0.015, 0.0042, links);
        }
        middle += v3(c[0], c[1], c[2]);
    }
    // A cap on the end of the haft, a ring through it the chain hangs from
    ring(p, [0.0, 0.0, base], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.012, 0.005, links);
    p.push(limb([0.0, 0.0, z_top - 0.045], [0.0, 0.0, z_top + 0.008], 0.021, 0.017, links));
    middle / offsets.len() as f64
}

/// A maul's great head across the end of its haft at `z_top`, its striking faces ±x: a block of
/// iron bound with bands near each face, an anvil with a horn out the back, or a boulder strapped
/// on with iron. Runes glow on its flats if it's infused. Returns the middle of its front face.
fn maul_head(p: &mut Vec<Part>, v: &Visual, z_top: f64, infuse: Option<u8>) -> Vector3<f64> {
    let length = v.length.unwrap_or(0.22);
    let w = v.width.unwrap_or(0.44) * 0.5;
    let d = v.thickness.unwrap_or(0.19) * 0.5;
    let body = mat(&v.material, DARK_STEEL);
    let bands = mat(&v.edge, STEEL);
    let rune = infuse.or_else(|| v.fuller.as_deref().and_then(material));
    let lo = z_top - length * 0.55;
    let hi = lo + length;
    let zc = lo + length * 0.5;
    // A collar where the haft goes in
    p.push(limb([0.0, 0.0, lo - 0.07], [0.0, 0.0, lo + 0.01], 0.022, 0.03, bands));
    match v.shape.as_deref().unwrap_or("maul") {
        // A drum of iron, round across the haft, bound at both ends, its faces studded
        "maul_drum" => {
            if let Some(g) = rune {
                ring(p, [0.0, 0.0, zc], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], d * 1.02, 0.006, g);
            }
            for s in [-1.0, 1.0] {
                for k in 0..6 {
                    let a = k as f64 / 6.0 * TAU;
                    p.push(ball([s * (w + 0.004), a.cos() * d * 0.6, zc + a.sin() * d * 0.6], 0.011, bands));
                }
                p.push(limb([s * (w - 0.05), 0.0, zc], [s * (w - 0.01), 0.0, zc], d * 1.08, d * 1.08, bands));
            }
            p.push(limb([-w, 0.0, zc], [w, 0.0, zc], d, d, body));
        }
        // A block bristling with stubby spikes on every side but its faces
        "maul_spiked" => {
            for x in [-w * 0.55, 0.0, w * 0.55] {
                for (dy, dz) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0)] {
                    let at = [x, dy * d, zc + dz * length * 0.5];
                    p.push(limb(at, [x, dy * (d + 0.06), zc + dz * (length * 0.5 + 0.06)], 0.015, 0.002, bands));
                }
            }
            for s in [-1.0, 1.0] {
                let (a, b) = (s * (w - 0.02), s * (w + 0.008));
                p.push(cuboid([a.min(b), -d - 0.004, lo - 0.004], [a.max(b), d + 0.004, hi + 0.004], bands));
            }
            p.push(cuboid([-w, -d, lo], [w, d, hi], body));
        }
        // A great crystal, cut square, set in iron caps at both ends, glowing
        "maul_crystal" => {
            for s in [-1.0, 1.0] {
                let (a, b) = (s * (w - 0.06), s * (w + 0.006));
                p.push(cuboid([a.min(b), -d - 0.01, lo - 0.01], [a.max(b), d + 0.01, hi + 0.01], bands));
            }
            p.push(cuboid([-w * 0.12, -d - 0.012, lo - 0.012], [w * 0.12, d + 0.012, hi + 0.012], bands));
            p.push(cuboid([-w, -d * 0.85, lo + 0.01], [w, d * 0.85, hi - 0.01], mat(&v.material, CRYSTAL)));
        }
        "maul_boulder" => {
            let stone = mat(&v.material, ROCK);
            // Iron straps over it, and glowing cracks if it's infused, first so they show
            for y in [-d * 0.45, d * 0.45] {
                curve(p, &[[-w * 0.98, y, zc], [-w * 0.65, y, hi + 0.01], [0.0, y, hi + 0.035], [w * 0.65, y, hi + 0.01], [w * 0.98, y, zc]], 0.011, 0.011, bands);
            }
            if let Some(g) = rune {
                for s in [-1.0, 1.0] {
                    curve(p, &[[-w * 0.6, s * d * 1.02, zc - length * 0.2], [-w * 0.1, s * d * 1.08, zc + length * 0.1], [w * 0.35, s * d * 1.04, zc - length * 0.05], [w * 0.7, s * d * 0.98, zc + length * 0.15]], 0.008, 0.005, g);
                }
            }
            let mut seed = 0xb0a1;
            for k in 0..7 {
                let a = k as f64 / 7.0 * TAU;
                let at = [a.cos() * w * 0.6, a.sin() * d * 0.7, zc + (random(&mut seed) - 0.5) * length * 0.6];
                p.push(ball(at, d * (0.55 + 0.25 * random(&mut seed)), if k % 3 == 0 { GRAVEL } else { stone }));
            }
            p.push(ellipsoid([0.0, 0.0, zc], [w, d * 1.1, length * 0.6], stone));
        }
        "maul_anvil" => {
            if let Some(g) = rune {
                for s in [-1.0, 1.0] {
                    p.push(cuboid([-w * 0.1, s * (d + 0.002) - 0.004, zc - 0.005], [w * 0.8, s * (d + 0.002) + 0.004, zc + 0.005], g));
                }
            }
            // The flat face out front, plated, a horn tapering off the back, a waist under
            p.push(cuboid([w - 0.03, -d - 0.008, lo - 0.008], [w + 0.006, d + 0.008, hi + 0.008], bands));
            p.push(limb([-w * 0.15, 0.0, zc + length * 0.1], [-w - 0.13, 0.0, zc + length * 0.3], length * 0.42, 0.012, body));
            p.push(cuboid([-w * 0.2, -d, lo], [w, d, hi], body));
            p.push(cuboid([-w * 0.05, -d * 0.7, lo - 0.045], [w * 0.75, d * 0.7, lo], body));
        }
        _ => {
            if let Some(g) = rune {
                for s in [-1.0, 1.0] {
                    let y = s * (d + 0.002);
                    p.push(cuboid([-w * 0.45, y - 0.004, zc - 0.005], [w * 0.45, y + 0.004, zc + 0.005], g));
                    for x in [-w * 0.3, 0.0, w * 0.3] {
                        p.push(cuboid([x - 0.006, y - 0.004, lo + length * 0.2], [x + 0.006, y + 0.004, hi - length * 0.2], g));
                    }
                }
            }
            for s in [-1.0, 1.0] {
                // A band round it near each face, then the face itself, a little proud
                let (a, b) = (s * (w - 0.075), s * (w - 0.045));
                p.push(cuboid([a.min(b), -d - 0.008, lo - 0.008], [a.max(b), d + 0.008, hi + 0.008], bands));
                let (a, b) = (s * (w - 0.02), s * (w + 0.008));
                p.push(cuboid([a.min(b), -d - 0.004, lo - 0.004], [a.max(b), d + 0.004, hi + 0.004], bands));
            }
            p.push(cuboid([-w, -d, lo], [w, d, hi], body));
        }
    }
    v3(w, 0.0, zc)
}

/// A bomb held up on the palm: an iron ball with glowing vents, a clay firepot, a glass flask, a
/// bundle of charges or an iron cage, what fills it glowing out through it, a fuse curling up out
/// of its top and burning at its end, and its core's gem set in its side. Returns its parts,
/// where it's held, and the burning end of its fuse.
fn bomb(casing: &Visual, fuse: &Visual, filling: &Visual, core: &Visual) -> (Vec<Part>, Vector3<f64>, Vector3<f64>) {
    let mut p = Vec::new();
    let shape = casing.shape.as_deref().unwrap_or("iron");
    let r = casing.size.unwrap_or(0.062);
    let shell = mat(&casing.material, match shape {
        "pot" => CLAY,
        "bundle" => TEMPLE_RED,
        "flask" => CRYSTAL,
        "skull" => BONE,
        "keg" => WOOD,
        "geode" => ROCK,
        _ => DARK_STEEL,
    });
    let trim = mat(&casing.accent, BRONZE);
    let glow = mat(&filling.material, GLOW_FIRE);
    let c = [0.0, 0.0, r + 0.005];
    let neck = match shape {
        "pot" | "flask" => c[2] + r * 1.1 + 0.03,
        "bundle" => c[2] + r * 1.3,
        _ => c[2] + r + 0.014,
    };
    // The fuse, its end burning, first so its spark shows
    let fl = fuse.length.unwrap_or(0.07);
    let curl = fuse.width.unwrap_or(0.025);
    let end = [curl, 0.0, neck + fl];
    p.push(ball(end, 0.01, mat(&fuse.gem, GLOW_FIRE)));
    curve(&mut p, &[[0.0, 0.0, neck - 0.012], [curl * 0.2, 0.0, neck + fl * 0.45], [curl * 0.7, 0.0, neck + fl * 0.8], end], 0.0045, 0.0038, mat(&fuse.material, LINEN));
    if let Some(g) = core.gem.as_deref().and_then(material) {
        gem(&mut p, [0.0, -r * 0.96, c[2]], core.size.unwrap_or(0.02).clamp(0.01, 0.016), g, core.shape.as_deref());
    }
    match shape {
        // A skull, what fills it glowing out of its eyes and its cracks, its jaw wired on
        "skull" => {
            for s in [-1.0, 1.0] {
                p.push(ball([s * r * 0.36, -r * 0.82, c[2] + r * 0.12], r * 0.24, glow));
            }
            curve(&mut p, &[[r * 0.3, -r * 0.5, c[2] + r * 0.85], [r * 0.5, -r * 0.2, c[2] + r * 0.6], [r * 0.75, 0.0, c[2] + r * 0.7]], 0.004, 0.003, glow);
            ring(&mut p, [0.0, -r * 0.2, c[2] - r * 0.55], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], r * 0.7, 0.005, trim);
            p.push(ellipsoid([0.0, -r * 0.3, c[2] - r * 0.62], [r * 0.6, r * 0.55, r * 0.32], shell));
            p.push(limb([0.0, 0.0, c[2] + r * 0.8], [0.0, 0.0, neck], 0.014, 0.012, trim));
            p.push(ellipsoid(c, [r * 0.95, r * 1.05, r], shell));
        }
        // A little keg, hooped, staves showing, its bung glowing
        "keg" => {
            for z in [c[2] - r * 0.6, c[2] + r * 0.6] {
                ring(&mut p, [0.0, 0.0, z], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], r * 0.9, 0.006, trim);
            }
            p.push(ball([r * 0.82, 0.0, c[2]], 0.012, glow));
            for k in 0..10 {
                let a = k as f64 / 10.0 * TAU;
                p.push(limb([a.cos() * r * 0.8, a.sin() * r * 0.8, c[2] - r * 0.95], [a.cos() * r * 0.9, a.sin() * r * 0.9, c[2]], 0.004, 0.004, BARK_DARK));
            }
            p.push(limb([0.0, 0.0, c[2] + r * 0.9], [0.0, 0.0, neck], 0.012, 0.01, trim));
            p.push(limb([0.0, 0.0, c[2] - r], [0.0, 0.0, c[2]], r * 0.78, r * 0.9, shell));
            p.push(limb([0.0, 0.0, c[2]], [0.0, 0.0, c[2] + r], r * 0.9, r * 0.78, shell));
        }
        // A geode split open, what fills it glowing in its hollow among crystal points
        "geode" => {
            for d in sphere_points(8) {
                if d[0] > 0.0 {
                    p.push(limb([d[0] * r * 0.45, d[1] * r * 0.45, c[2] + d[2] * r * 0.45], [d[0] * r * 0.95, d[1] * r * 0.95, c[2] + d[2] * r * 0.95], r * 0.16, 0.003, CRYSTAL));
                }
            }
            p.push(ball([r * 0.2, 0.0, c[2]], r * 0.5, glow));
            p.push(cuboid([r * 0.35, -r * 1.2, c[2] - r * 1.2], [r * 1.2, r * 1.2, c[2] + r * 1.2], AIR));
            p.push(limb([0.0, 0.0, c[2] + r * 0.8], [0.0, 0.0, neck], 0.014, 0.012, trim));
            p.push(ball(c, r, shell));
        }
        // A fat clay belly, a neck, glowing pitch at its mouth and a cord tied round it
        "pot" => {
            p.push(ball([0.0, 0.0, neck - 0.012], 0.017, glow));
            ring(&mut p, [0.0, 0.0, neck - 0.006], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0.022, 0.006, shell);
            ring(&mut p, [0.0, 0.0, c[2] + r * 0.3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], r * 0.97, 0.006, trim);
            p.push(limb([0.0, 0.0, c[2] + r * 0.6], [0.0, 0.0, neck - 0.006], 0.028, 0.02, shell));
            p.push(ellipsoid(c, [r, r, r * 1.08], shell));
        }
        // What fills it glowing through the glass, a cork in its neck, rims round its shoulder
        "flask" => {
            p.push(limb([0.0, 0.0, neck - 0.03], [0.0, 0.0, neck], 0.0135, 0.013, WOOD));
            ring(&mut p, [0.0, 0.0, c[2] + r * 0.45], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], r * 0.92, 0.006, trim);
            p.push(limb([0.0, 0.0, c[2] + r * 0.7], [0.0, 0.0, neck - 0.028], 0.017, 0.014, shell));
            p.push(ball(c, r, glow));
        }
        // Three charges bound together, their ends glowing
        "bundle" => {
            let at = |k: usize| {
                let a = k as f64 / 3.0 * TAU;
                (a.cos() * r * 0.5, a.sin() * r * 0.5)
            };
            for k in 0..3 {
                let (x, y) = at(k);
                p.push(ball([x, y, c[2] + r * 1.22], r * 0.3, glow));
            }
            for z in [c[2] - r * 0.5, c[2] + r * 0.6] {
                ring(&mut p, [0.0, 0.0, z], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], r * 0.98, 0.006, trim);
            }
            for k in 0..3 {
                let (x, y) = at(k);
                p.push(limb([x, y, c[2] - r * 0.98], [x, y, c[2] + r * 1.2], r * 0.46, r * 0.46, shell));
            }
        }
        // An iron cage, the charge inside glowing out between its bars
        "cage" => {
            for k in 0..4 {
                let a = k as f64 / 4.0 * TAU;
                let points: Vec<[f64; 3]> = (0..=8)
                    .map(|j| {
                        let b = -PI * 0.5 + PI * j as f64 / 8.0;
                        [a.cos() * b.cos() * r * 1.03, a.sin() * b.cos() * r * 1.03, c[2] + b.sin() * r * 1.03]
                    })
                    .collect();
                curve(&mut p, &points, 0.0055, 0.0055, shell);
            }
            ring(&mut p, c, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], r * 1.05, 0.006, trim);
            p.push(limb([0.0, 0.0, c[2] + r * 0.9], [0.0, 0.0, neck], 0.014, 0.012, shell));
            p.push(ball(c, r * 0.86, glow));
        }
        // An iron ball, vents round it glowing with what's in it, a band and a neck
        _ => {
            for k in 0..5 {
                let a = k as f64 / 5.0 * TAU + 0.3;
                p.push(ball([a.cos() * r * 0.93, a.sin() * r * 0.93, c[2] + r * 0.25], 0.011, glow));
            }
            ring(&mut p, c, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], r, 0.006, trim);
            p.push(limb([0.0, 0.0, c[2] + r * 0.8], [0.0, 0.0, neck], 0.016, 0.014, trim));
            p.push(ball(c, r, shell));
        }
    }
    (p, v3(0.0, 0.0, r * 0.55), v3(end[0], end[1], end[2]))
}

/// A lightning rod: a long shaft, a coil wound round its upper half, a wrap of its binding where
/// it's held, and at its top a fork of two prongs of its focus, the crystal set where they part
/// and their tips glowing with it. Returns its parts, where it's held, and the point between the
/// prongs' tips, where lightning leaves it.
fn rod(shaft: &Visual, focus: &Visual, crystal: &Visual, binding: &Visual) -> (Vec<Part>, Vector3<f64>, Vector3<f64>) {
    let mut p = Vec::new();
    let top = 0.92;
    let wood = mat(&shaft.material, WOOD);
    let metal = metal_or_bronze(mat(&focus.material, BRONZE));
    let coil = metal_or_bronze(mat(&binding.accent, BRONZE));
    let wrap = mat(&binding.material, LEATHER);
    let gem = mat(&crystal.gem, CRYSTAL);
    p.push(ball([0.0, 0.0, top + 0.03], crystal.size.unwrap_or(0.05).clamp(0.02, 0.034), gem));
    let spread = match focus.shape.as_deref().unwrap_or("") {
        "antlers" | "crescent" | "horns" | "crook" => 0.085,
        _ => 0.065,
    };
    for s in [-1.0, 1.0] {
        let tip = [s * spread * 0.7, 0.0, top + 0.25];
        p.push(ball(tip, 0.011, gem));
        curve(&mut p, &[[0.0, 0.0, top - 0.01], [s * spread * 0.85, 0.0, top + 0.07], [s * spread, 0.0, top + 0.16], tip], 0.012, 0.005, metal);
    }
    p.push(limb([0.0, 0.0, top - 0.06], [0.0, 0.0, top + 0.01], 0.021, 0.025, metal));
    // The coil, six turns round the upper half
    let steps = 96;
    for k in 0..=steps {
        let t = k as f64 / steps as f64;
        let a = t * 6.0 * TAU;
        p.push(ball([a.cos() * 0.019, a.sin() * 0.019, 0.5 + t * (top - 0.58)], 0.0055, coil));
    }
    for z in [0.3, 0.44] {
        p.push(limb([0.0, 0.0, z - 0.008], [0.0, 0.0, z + 0.008], 0.02, 0.02, coil));
    }
    p.push(limb([0.0, 0.0, 0.3], [0.0, 0.0, 0.44], 0.0185, 0.0185, wrap));
    p.push(limb([0.0, 0.0, -0.01], [0.0, 0.0, 0.06], 0.012, 0.017, DARK_STEEL));
    p.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, top], 0.015, 0.017, wood));
    (p, v3(0.0, 0.0, 0.37), v3(0.0, 0.0, top + 0.22))
}

/// A censer hanging on three chains from a short handle held upright: a pierced bowl, its crystal
/// glowing out through the holes, a band of its binding round its rim, a domed lid with a finial.
/// Returns its parts, where it's held, and the middle of its bowl, where its smoke comes from.
fn censer(shaft: &Visual, focus: &Visual, crystal: &Visual, binding: &Visual) -> (Vec<Part>, Vector3<f64>, Vector3<f64>) {
    let mut p = Vec::new();
    let wood = mat(&shaft.material, WOOD);
    let metal = metal_or_bronze(mat(&focus.material, BRONZE));
    let gem = mat(&crystal.gem, CRYSTAL);
    let band = match mat(&binding.material, GOLD) {
        WOOD | BIRCH_BARK | BARK_DARK => GOLD,
        m => m,
    };
    let r = 0.06;
    let c = [0.0, 0.0, -0.4];
    // The holes first, so what burns inside glows out through them
    for (row, (up, out)) in [(0.38, 0.92), (-0.12, 0.99)].into_iter().enumerate() {
        for k in 0..8 {
            let a = (k as f64 + 0.5 * row as f64) / 8.0 * TAU;
            p.push(ball([a.cos() * r * out, a.sin() * r * out, c[2] + r * up], 0.0105, gem));
        }
    }
    ring(&mut p, [0.0, 0.0, c[2] + r * 0.12], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], r * 1.02, 0.007, band);
    p.push(ball([0.0, 0.0, c[2] + r * 1.05], 0.012, metal));
    p.push(ellipsoid([0.0, 0.0, c[2] + r * 0.35], [r * 0.92, r * 0.92, r * 0.7], metal));
    p.push(ellipsoid(c, [r, r, r * 0.8], metal));
    p.push(limb([0.0, 0.0, c[2] - r * 0.7], [0.0, 0.0, c[2] - r * 1.0], 0.022, 0.03, metal));
    // Three chains from its rim up to a ring under the handle, beads of metal
    let top = [0.0, 0.0, -0.035];
    for k in 0..3 {
        let a = k as f64 / 3.0 * TAU + 0.5;
        let from = [a.cos() * r * 0.95, a.sin() * r * 0.95, c[2] + r * 0.15];
        let n = 26;
        for j in 0..=n {
            let t = j as f64 / n as f64;
            p.push(ball([from[0] * (1.0 - t), from[1] * (1.0 - t), from[2] + (top[2] - from[2]) * t], 0.0042, metal));
        }
    }
    ring(&mut p, top, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0.016, 0.005, metal);
    p.push(ball([0.0, 0.0, 0.175], 0.016, metal));
    p.push(limb([0.0, 0.0, -0.03], [0.0, 0.0, 0.165], 0.012, 0.014, wood));
    (p, v3(0.0, 0.0, 0.07), Vector3::from(c))
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

/// A gem of `material` set at `at`, about `r` round, cut as `shape` says: a round cabochon, a
/// stone cut to points top and bottom, a long shard, a teardrop hanging down, or a cluster of three
fn gem(p: &mut Vec<Part>, at: [f64; 3], r: f64, material: u8, shape: Option<&str>) {
    let [x, y, z] = at;
    match shape.unwrap_or("") {
        "facet" => {
            p.push(limb([x, y, z - r * 1.25], [x, y, z], 0.002, r, material));
            p.push(limb([x, y, z], [x, y, z + r * 1.25], r, 0.002, material));
        }
        "shard" => {
            p.push(limb([x, y, z - r * 0.9], [x, y, z], 0.002, r * 0.75, material));
            p.push(limb([x, y, z], [x, y, z + r * 2.1], r * 0.75, 0.002, material));
        }
        "teardrop" => {
            p.push(limb([x, y, z], [x, y, z + r * 1.7], r, 0.002, material));
            p.push(ball(at, r, material));
        }
        "cluster" => {
            for k in 0..3 {
                let a = k as f64 / 3.0 * TAU + 0.4;
                p.push(ball([x + a.cos() * r * 0.6, y + a.sin() * r * 0.6, z + (k as f64 - 1.0) * r * 0.3], r * 0.62, material));
            }
        }
        _ => p.push(ball(at, r, material)),
    }
}

/// What's worked into the faces of a blade or head, ±`y` thick, in its row from `za` to `zb`,
/// `row` rows up it and `t` of the way, between `x0` and `x1` across: rust eating at it in
/// patches, engraving inlaid in a running scroll, or runes glowing in a line up the middle. These
/// go before the body, so they take its place on its faces.
#[allow(clippy::too_many_arguments)]
fn inlay(p: &mut Vec<Part>, v: &Visual, row: usize, t: f64, x0: f64, x1: f64, y: f64, za: f64, zb: f64, glow: Option<u8>) {
    let width = x1 - x0;
    if width <= 0.0 {
        return;
    }
    let voxel = 1.0 / SWORD_RES;
    let faces = |p: &mut Vec<Part>, a: f64, b: f64, material: u8| {
        p.push(cuboid([a, y * 0.55, za], [b, y * 1.05, zb], material));
        p.push(cuboid([a, -y * 1.05, za], [b, -y * 0.55, zb], material));
    };
    match v.pattern.as_deref() {
        Some("rust") => {
            // Patches a few rows tall, here and there across, darker at their middles
            let patch = hash(row as u32 / 4 * 7919 + 13);
            if patch % 3 != 0 {
                let from = (patch >> 8) as f64 / 16777216.0;
                let a = x0 + width * from * 0.7;
                let b = (a + width * (0.25 + 0.3 * ((patch >> 4) & 15) as f64 / 15.0)).min(x1);
                faces(p, a, b, mat(&v.accent, CLAY_POT));
                if row % 4 == 1 && b - a > 3.0 * voxel {
                    faces(p, a + voxel, b - voxel, FUR_BROWN);
                }
            }
        }
        Some("engraved") => {
            // A scroll winding up the blade, a voxel wide, in its accent
            let at = x0 + width * (0.5 + 0.32 * (t * TAU * 5.0).sin());
            faces(p, at - voxel * 0.6, at + voxel * 0.6, mat(&v.accent, GOLD));
            if row % 12 == 0 {
                faces(p, x0 + width * 0.15, x1 - width * 0.15, mat(&v.accent, GOLD));
            }
        }
        Some("runes") => {
            // A rune every so often up the middle: a bar, then strokes off it
            let glow = glow.unwrap_or(GLOW_ARCANE);
            let mid = (x0 + x1) * 0.5;
            // Each rune different: which strokes it has goes by which it is
            let strokes = hash(row as u32 / 14 + 3);
            match row % 14 {
                k @ 0..=5 => {
                    faces(p, mid - voxel * 0.6, mid + voxel * 0.6, glow);
                    if strokes >> k & 1 == 1 && k % 2 == 0 {
                        let s = if strokes >> (k + 8) & 1 == 1 { 1.0 } else { -1.0 };
                        faces(p, mid.min(mid + s * width * 0.16), mid.max(mid + s * width * 0.16), glow);
                    }
                }
                _ => {}
            }
        }
        _ => {}
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
    if let Some(stone) = core.gem.as_deref().and_then(material) {
        let z = if family.guard { z_top + guard_height * 0.5 } else { z_top - 0.05 };
        gem(&mut p, [0.0, 0.0, z], core.size.unwrap_or(0.02).max(0.016), stone, core.shape.as_deref());
    }
    if family.guard {
        sword_guard(&mut p, guard, z_top, guard_height);
    }
    let infuse = core.infuse.as_deref().and_then(material);
    let tip = match blade.shape.as_deref().unwrap_or("straight") {
        shape @ ("axe" | "bearded" | "spike_axe" | "moon_axe") => axe_head(&mut p, blade, z_top, false, shape),
        "double_axe" => axe_head(&mut p, blade, z_top, true, "axe"),
        shape @ ("hammer" | "mace" | "spiked" | "flanged" | "beaked" | "morning") => hammer_head(&mut p, blade, z_top, shape),
        shape if shape.starts_with("scythe") => scythe_head(&mut p, blade, z_top),
        shape if shape.starts_with("halberd") => halberd_head(&mut p, blade, z_top, infuse),
        shape if shape.starts_with("flail") => flail_head(&mut p, blade, z_top, infuse),
        shape if shape.starts_with("maul") => maul_head(&mut p, blade, z_top, infuse),
        // The tools', see src\tools.rs
        "pick" => pick_head(&mut p, blade, z_top),
        "spade" => spade_head(&mut p, blade, z_top),
        // Three prongs, or two, on a crossbar in a socket
        shape @ ("trident" | "fork") => {
            let length = blade.length.unwrap_or(0.4);
            let width = blade.width.unwrap_or(0.16);
            let body = mat(&blade.material, STEEL);
            let edge = mat(&blade.edge, body);
            let prongs: &[f64] = if shape == "trident" { &[-1.0, 0.0, 1.0] } else { &[-1.0, 1.0] };
            let bar = z_top + 0.05;
            for &s in prongs {
                let x = s * width * 0.5;
                let long = if s == 0.0 { 1.0 } else { 0.82 };
                let top = bar + length * long;
                p.push(limb([x, 0.0, top - 0.06], [x * 1.05, 0.0, top], 0.011, 0.002, edge));
                if s != 0.0 {
                    // Barbed, the barbs turned in
                    p.push(limb([x, 0.0, top - 0.07], [x * 0.75, 0.0, top - 0.11], 0.008, 0.002, edge));
                }
                curve(&mut p, &[[x * 0.6, 0.0, bar], [x, 0.0, bar + 0.04], [x, 0.0, top - 0.05]], 0.011, 0.009, body);
            }
            p.push(limb([-width * 0.32, 0.0, bar], [width * 0.32, 0.0, bar], 0.013, 0.013, body));
            p.push(limb([0.0, 0.0, z_top - 0.06], [0.0, 0.0, bar + 0.01], 0.022, 0.016, DARK_STEEL));
            if let Some(g) = infuse.or_else(|| blade.fuller.as_deref().and_then(material)) {
                p.insert(0, ball([0.0, 0.0, bar], 0.014, g));
            }
            v3(0.0, 0.0, bar + length)
        }
        shape => {
            // A spearhead sits in a socket on the end of its haft, a winged one with lugs
            // sticking out either side under it to stop it going in too far
            if !family.guard {
                if shape == "winged" {
                    let lug = mat(&blade.accent, mat(&blade.material, STEEL));
                    for s in [-1.0, 1.0] {
                        p.push(limb([s * 0.012, 0.0, z_top - 0.02], [s * 0.075, 0.0, z_top + 0.035], 0.012, 0.004, lug));
                    }
                }
                p.push(limb([0.0, 0.0, z_top - 0.06], [0.0, 0.0, z_top + 0.02], 0.022, 0.014, DARK_STEEL));
            }
            sword_blade(&mut p, blade, z_blade, infuse)
        }
    };
    sword_hilt(&mut p, hilt, z_grip, grip_length, pommel, handle);
    (p, v3(0.0, 0.0, z_grip + handle * family.grip), tip)
}

/// An axe head on top of a haft ending at `z_top`, its edge facing +x, or both ways for a
/// double-bit one: as `shape` has it, an axe flaring evenly to its edge, a bearded one whose edge
/// hangs down well below the eye, one with a long spike out the back, or a moon axe, its blade a
/// crescent with a round hole cut through it. Returns the middle of the edge.
fn axe_head(p: &mut Vec<Part>, v: &Visual, z_top: f64, double: bool, shape: &str) -> Vector3<f64> {
    let length = v.length.unwrap_or(0.26);
    let width = v.width.unwrap_or(0.2);
    let body = mat(&v.material, STEEL);
    let edge = mat(&v.edge, body);
    let voxel = 1.0 / SWORD_RES;
    let half_thick = 0.012;
    // A bearded head reaches down from the eye
    let z0 = z_top - length * if shape == "bearded" { 0.95 } else { 0.7 };
    let rows = (length * SWORD_RES).ceil() as usize;
    let sides: &[f64] = if double { &[1.0, -1.0] } else { &[1.0] };
    let (mut inlays, mut edges, mut bodies) = (Vec::new(), Vec::new(), Vec::new());
    for r in 0..rows {
        let t = (r as f64 + 0.5) / rows as f64;
        let (za, zb) = (z0 + r as f64 * length / rows as f64, z0 + (r + 1) as f64 * length / rows as f64);
        // Flaring out to a curved edge, reaching furthest in the middle, a bearded one's lowest
        // down and hooked back under, a moon axe's at both horns
        let reach = match shape {
            "bearded" => width * (0.3 + 0.7 * (PI * t.powf(0.6)).sin().powf(0.5)) * if t < 0.18 { 0.6 + 2.2 * t } else { 1.0 },
            "moon_axe" => width * (0.72 + 0.28 * (2.0 * t - 1.0).powi(2)),
            _ => width * (0.4 + 0.6 * (PI * t).sin().powf(0.6)),
        };
        for &s in sides {
            let (outer, inner) = (s * reach, s * (reach - 2.0 * voxel));
            edges.push(cuboid([outer.min(inner), -half_thick * 0.55, za], [outer.max(inner), half_thick * 0.55, zb], edge));
            let near = s * 0.02;
            bodies.push(cuboid([near.min(inner), -half_thick, za], [near.max(inner), half_thick, zb], body));
            inlay(&mut inlays, v, r, t, near.min(inner) + 0.01, near.max(inner) - 0.006, half_thick, za, zb, v.fuller.as_deref().and_then(material));
        }
    }
    // A moon axe's hole, cut first so it's left empty
    if shape == "moon_axe" {
        p.push(limb([width * 0.42, -0.03, z0 + length * 0.5], [width * 0.42, 0.03, z0 + length * 0.5], length * 0.17, length * 0.17, AIR));
    }
    p.extend(inlays);
    p.extend(edges);
    p.extend(bodies);
    // The eye round the haft, and a knob behind it on a single-bit head, or a long spike
    let eye = if shape == "bearded" { (z_top - length * 0.3, z_top + length * 0.12) } else { (z0 + length * 0.15, z0 + length * 0.85) };
    p.push(cuboid([-0.024, -0.02, eye.0], [0.024, 0.02, eye.1], body));
    if shape == "spike_axe" {
        let z = (eye.0 + eye.1) * 0.5;
        p.push(limb([-0.02, 0.0, z], [-0.17, 0.0, z - 0.04], 0.016, 0.002, mat(&v.accent, body)));
    } else if !double {
        let (a, b) = (eye.0 + (eye.1 - eye.0) * 0.28, eye.0 + (eye.1 - eye.0) * 0.72);
        p.push(cuboid([-0.05, -half_thick * 1.6, a], [-0.02, half_thick * 1.6, b], body));
    }
    v3(width, 0.0, z0 + length * if shape == "bearded" { 0.4 } else { 0.5 })
}

/// A pickaxe's head across the top of a haft ending at `z_top`: a point either way, along +x and
/// -x, each arching up out of the eye and curving down to its tip, the one along +x of the edge's
/// make, and a band of the accent round the eye. Returns the tip along +x.
fn pick_head(p: &mut Vec<Part>, v: &Visual, z_top: f64) -> Vector3<f64> {
    let length = v.length.unwrap_or(0.24);
    let body = mat(&v.material, DARK_STEEL);
    let edge = mat(&v.edge, body);
    let z = z_top - 0.03;
    for s in [1.0, -1.0] {
        let points = [[s * 0.02, 0.0, z], [s * length * 0.45, 0.0, z + 0.018], [s * length * 0.8, 0.0, z - 0.012], [s * length, 0.0, z - 0.06]];
        curve(p, &points, 0.021, 0.004, if s > 0.0 { edge } else { body });
    }
    if let Some(band) = v.accent.as_deref().and_then(material) {
        p.push(cuboid([-0.031, -0.026, z - 0.012], [0.031, 0.026, z + 0.004], band));
    }
    p.push(cuboid([-0.027, -0.023, z - 0.045], [0.027, 0.023, z + 0.028], body));
    v3(length, 0.0, z - 0.06)
}

/// A spade's blade on the top of a haft ending at `z_top`: flat across x, thin along y, straight
/// sided and rounding to a blunt point, with treads at its shoulders to stand on, a socket round
/// the haft under it, and a lip of the edge's make along its end. Returns the middle of its end.
fn spade_head(p: &mut Vec<Part>, v: &Visual, z_top: f64) -> Vector3<f64> {
    let length = v.length.unwrap_or(0.28);
    let width = v.width.unwrap_or(0.2);
    let body = mat(&v.material, DARK_STEEL);
    let edge = mat(&v.edge, body);
    let half = 0.008;
    let base = z_top + 0.02;
    let rows = (length * SWORD_RES).ceil() as usize;
    let (mut edges, mut bodies) = (Vec::new(), Vec::new());
    for r in 0..rows {
        let t = (r as f64 + 0.5) / rows as f64;
        let (za, zb) = (base + r as f64 * length / rows as f64, base + (r + 1) as f64 * length / rows as f64);
        let w = width * 0.5 * if t < 0.62 { 1.0 } else { (1.0 - ((t - 0.62) / 0.38).powi(2)).max(0.0).sqrt().max(0.18) };
        if t > 0.86 {
            edges.push(cuboid([-w, -half * 0.6, za], [w, half * 0.6, zb], edge));
        } else {
            bodies.push(cuboid([-w, -half, za], [w, half, zb], body));
        }
    }
    p.extend(edges);
    p.extend(bodies);
    if let Some(band) = v.accent.as_deref().and_then(material) {
        p.push(cuboid([-width * 0.5, -half * 1.3, base + length * 0.3], [width * 0.5, half * 1.3, base + length * 0.34], band));
    }
    // Treads either side at its shoulders, and the socket
    p.push(cuboid([-width * 0.5 - 0.006, -0.018, base - 0.004], [width * 0.5 + 0.006, 0.018, base + 0.016], body));
    p.push(limb([0.0, 0.0, z_top - 0.09], [0.0, 0.0, base + 0.01], 0.02, 0.017, body));
    v3(0.0, 0.0, base + length)
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
    let shape = v.shape.as_deref().unwrap_or("scythe");
    let (mut bodies, mut edges) = (Vec::new(), Vec::new());
    for c in 0..columns {
        let t = (c as f64 + 0.5) / columns as f64;
        let (xa, xb) = (0.015 + c as f64 * length / columns as f64, 0.015 + (c + 1) as f64 * length / columns as f64);
        let top = z_top - 0.02 - droop(t);
        let mut bottom = top - (depth * (1.0 - t).powf(0.7)).max(voxel);
        // A saw-toothed one's edge in teeth
        if shape == "scythe_saw" && c % 9 < 4 && t < 0.9 {
            bottom -= 3.0 * voxel;
        }
        bodies.push(cuboid([xa, -half_thick, (bottom + 2.0 * voxel).min(top)], [xb, half_thick, top], body));
        edges.push(cuboid([xa, -half_thick * 0.55, bottom], [xb, half_thick * 0.55, top], edge));
    }
    // A twin one has a second, shorter blade curving off the back
    if shape == "scythe_twin" {
        let points: Vec<[f64; 3]> = (0..=10)
            .map(|k| {
                let t = k as f64 / 10.0;
                [-0.02 - t * length * 0.45, 0.0, z_top - 0.03 - 0.2 * length * 0.45 * t * t]
            })
            .collect();
        curve(p, &points, depth * 0.35, 0.004, edge);
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
    let shape = v.shape.as_deref().unwrap_or("halberd");
    // A hook out the back, a bill's curving forward over the blade
    if shape == "halberd_bill" {
        curve(p, &[[-0.03, 0.0, z_top - 0.02], [-0.11, 0.0, z_top + 0.04], [-0.12, 0.0, z_top + 0.12], [-0.07, 0.0, z_top + 0.16]], 0.014, 0.003, body);
    } else {
        p.push(limb([-0.03, 0.0, z_top - 0.03], [-0.16, 0.0, z_top + 0.03], 0.014, 0.003, body));
    }
    let axe = Visual { length: Some(0.24), width: v.width.or(Some(0.17)), material: v.material.clone(), edge: v.edge.clone(), pattern: v.pattern.clone(), accent: v.accent.clone(), ..Default::default() };
    let blade = match shape {
        "halberd_moon" => "moon_axe",
        "halberd_bill" => "bearded",
        _ => "axe",
    };
    axe_head(p, &axe, z_top, false, blade);
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
        // Thin plates standing out all round, coming to a point on top
        "flanged" => {
            for k in 0..7 {
                let a = k as f64 / 7.0 * TAU;
                let (c, s) = (a.cos(), a.sin());
                let flange = [[c * 0.02, s * 0.02, zc - length * 0.5], [c * w, s * w, zc - length * 0.1], [c * w * 0.95, s * w * 0.95, zc + length * 0.2], [c * 0.02, s * 0.02, zc + length * 0.6]];
                curve(p, &flange, 0.007, 0.007, face);
            }
            p.push(limb([0.0, 0.0, zc + length * 0.5], [0.0, 0.0, zc + length * 0.95], 0.015, 0.003, face));
            p.push(limb([0.0, 0.0, zc - length * 0.55], [0.0, 0.0, zc + length * 0.6], w * 0.32, w * 0.24, body));
            v3(w, 0.0, zc)
        }
        // A ball bristling with long spikes all round
        "morning" => {
            let r = w * 0.62;
            for d in sphere_points(16) {
                let at = |s: f64| [d[0] * r * s, d[1] * r * s, zc + d[2] * r * s];
                p.push(limb(at(0.85), at(1.75), 0.012, 0.002, face));
            }
            p.push(ball([0.0, 0.0, zc], r, body));
            v3(r * 1.7, 0.0, zc)
        }
        // A hammer's face on one side, a long beak curving down on the other
        "beaked" => {
            curve(p, &[[-w * 0.4, 0.0, zc], [-w * 1.2, 0.0, zc + length * 0.1], [-w * 1.9, 0.0, zc - length * 0.25], [-w * 2.2, 0.0, zc - length * 0.65]], 0.022, 0.003, face);
            let (a, b) = (w - 0.02, w + 0.006);
            p.push(cuboid([a, -d - 0.006, z0 - 0.006], [b, d + 0.006, z0 + length + 0.006], face));
            p.push(cuboid([-w * 0.45, -d, z0], [w, d, z0 + length], body));
            p.push(limb([0.0, 0.0, z0 + length], [0.0, 0.0, z0 + length + 0.05], 0.014, 0.003, face));
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
        // Narrow at the hilt, swelling to its widest two thirds up, then to a point
        "leaf" => (0.0, half * (0.7 + 0.55 * (PI * t * 0.8).sin().powi(2)) * point(0.72)),
        // Straight from a broad base to a fine point
        "tapered" => (0.0, half * (1.25 - 1.15 * t)),
        // Waving from side to side all the way up, more than a flame blade
        "serpent" => (width * 0.4 * (t * TAU * 2.5).sin(), half * (1.0 - 0.1 * t) * point(0.9)),
        // Straight, then hooking forward over its edge at the top, a sickle sword
        "hooked" => (width * 2.6 * ((t - 0.55) / 0.45).max(0.0).powi(2), half * (1.0 + 0.35 * ((t - 0.55) / 0.45).max(0.0)) * point(0.92)),
        // Curving in toward its edge all the way, rather than back from it
        "falx" => (-width * 1.4 * t * t, half * (0.85 + 0.25 * t) * point(0.9)),
        // Straight and nicked, its tip split in two
        "notched" | "forked" => (0.0, half * (1.0 - 0.08 * t) * point(0.96)),
        // Leaf shaped, widest a third of the way up
        "spear" | "barbed" | "winged" => (0.0, half * if t < 0.33 { 0.35 + 0.65 * t / 0.33 } else { ((1.0 - t) / 0.67).powf(0.8) }),
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
        // Nicks hacked out of the edge here and there
        let nicked = shape == "notched" && hash(r as u32 / 3 * 31 + 7) % 5 == 0 && t < 0.9;
        if nicked {
            half -= 2.5 * voxel;
        }
        let (za, zb) = (z0 + r as f64 * length / rows as f64, z0 + (r + 1) as f64 * length / rows as f64);
        // A forked one's tip split down the middle
        if shape == "forked" && t > 0.8 {
            let w = half * 0.55 * ((t - 0.8) / 0.2).sqrt();
            grooves.push(cuboid([center - w, -half_thick * 2.0, za], [center + w, half_thick * 2.0, zb], AIR));
        }
        // A groove down the middle of each side, glowing at the bottom if it's infused
        if t < 0.78 && half > 5.0 * voxel {
            let w = half * 0.22;
            grooves.push(cuboid([center - w, half_thick * 0.45, za], [center + w, half_thick * 1.5, zb], AIR));
            grooves.push(cuboid([center - w, -half_thick * 1.5, za], [center + w, -half_thick * 0.45, zb], AIR));
            if let Some(f) = fuller {
                grooves.push(cuboid([center - w, -half_thick * 0.45, za], [center + w, half_thick * 0.45, zb], f));
            }
        }
        if half > bevel + voxel {
            inlay(&mut grooves, v, r, t, center - half + bevel, center + half - bevel, half_thick, za, zb, fuller);
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
        // A ring round the foot of the blade, standing out to either side
        "ring" => {
            ring(p, [0.0, 0.0, mid + 0.01], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], w * 0.3, 0.008, m);
            for s in [-1.0, 1.0] {
                p.push(ball([s * w * 0.3, 0.0, mid + 0.01], 0.012, accent));
            }
            p.push(block);
        }
        // Quillons curving one up, one down, like an S
        "serpent" => {
            for s in [-1.0, 1.0] {
                curve(p, &[[s * 0.02, 0.0, mid], [s * w * 0.3, 0.0, mid + s * 0.01], [s * w * 0.48, 0.0, mid + s * 0.045], [s * w * 0.42, 0.0, mid + s * 0.075]], 0.011, 0.006, m);
                p.push(ball([s * w * 0.42, 0.0, mid + s * 0.078], 0.011, accent));
            }
            p.push(block);
        }
        // Curving up either side like a crescent moon, its horns toward the blade
        "crescent" => {
            let points: Vec<[f64; 3]> = (0..=12)
                .map(|k| {
                    let a = (k as f64 / 12.0 - 0.5) * 2.4;
                    [a.sin() * w * 0.5, 0.0, z + h * 0.5 + (1.0 - a.cos()) * w * 0.32]
                })
                .collect();
            for (k, pair) in points.windows(2).enumerate() {
                let thin = 1.0 - (k as f64 / 11.0 - 0.5).abs() * 1.2;
                p.push(limb(pair[0], pair[1], 0.013 * thin, 0.013 * thin, m).stretched([1.0, 0.8, 1.0]));
            }
            p.push(ball([0.0, 0.0, mid], 0.015, accent));
            p.push(block);
        }
        // A shell curving down over the hand on one side
        "shell" => {
            p.push(ellipsoid([0.0, 0.03, z - 0.01], [w * 0.42, 0.02, 0.05], m));
            p.push(ball([0.0, 0.05, z - 0.03], 0.012, accent));
            p.push(cuboid([-w * 0.35, -0.012, z], [w * 0.35, 0.012, z + h], m));
        }
        // A bar with spikes jutting up off it along the blade, and out at its ends
        "spiked" => {
            for s in [-1.0, 1.0] {
                p.push(limb([s * w * 0.5, 0.0, mid], [s * (w * 0.5 + 0.04), 0.0, mid], 0.012, 0.002, accent));
                p.push(limb([s * w * 0.3, 0.0, z + h], [s * w * 0.26, 0.0, z + h + 0.07], 0.01, 0.002, accent));
            }
            p.push(cuboid([-w / 2.0, -0.016, z], [w / 2.0, 0.016, z + h], m));
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
        // A flat wheel, its rim standing proud, seen edge on from above
        "wheel" => {
            ring(p, [0.0, 0.0, c], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], pommel * 0.95, pommel * 0.22, pm);
            p.push(ellipsoid([0.0, 0.0, c], [pommel * 0.95, pommel * 0.45, pommel * 0.95], mat(&v.accent, pm)));
            p.push(limb([0.0, 0.0, c + pommel * 0.9], [0.0, 0.0, z_grip], r * 0.8, r, pm));
        }
        // A crescent, its horns curving down either side
        "crescent" => {
            let points: Vec<[f64; 3]> = (0..=10)
                .map(|k| {
                    let a = (k as f64 / 10.0 - 0.5) * 3.4;
                    [a.sin() * pommel * 1.3, 0.0, c + a.cos() * pommel * 0.9]
                })
                .collect();
            curve(p, &points, pommel * 0.35, pommel * 0.35, pm);
            p.push(limb([0.0, 0.0, c + pommel * 0.8], [0.0, 0.0, z_grip], r * 0.8, r, pm));
        }
        // A fang curving down off the end
        "fang" => {
            curve(p, &[[0.0, 0.0, z_grip], [pommel * 0.3, 0.0, c], [pommel * 0.9, 0.0, -pommel * 0.6]], pommel * 0.55, 0.003, pm);
        }
        // A cut stone, faceted
        "faceted" => {
            p.push(limb([0.0, 0.0, -pommel * 0.2], [0.0, 0.0, c], 0.003, pommel * 0.85, pm));
            p.push(limb([0.0, 0.0, c], [0.0, 0.0, z_grip], pommel * 0.85, r, pm));
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
// leather, hemp, chain or thorny vine, bone strung like a spine, copper wire braided over dark
// steel or red silk bound in gold, or for elemental lashes a dark cord glowing with one of the
// elements, in the order of Element::ALL
pub const LASH_LEATHER: usize = 0;
pub const LASH_HEMP: usize = 1;
pub const LASH_CHAIN: usize = 2;
pub const LASH_VINE: usize = 3;
pub const LASH_SPINE: usize = 4;
pub const LASH_WIRE: usize = 5;
pub const LASH_SILK: usize = 6;
pub const LASH_GLOW: usize = 7;
pub const LASH_SETS: usize = LASH_GLOW + Element::ALL.len();
/// Each set has a segment model as thick as each of these, as parts of its length, thickest first,
/// so a stretch of lash can be drawn about as thick as it should be however long it is
pub const LASH_TAPERS: [f64; 6] = [0.064, 0.048, 0.036, 0.027, 0.02, 0.015];
// The crackers on a lash's tip, by what its fall is: a pale tuft of horsehair, one of silk, a
// knotted end, barbs, a lead weight, a blade of bone or of gold, an iron hook, a spiked iron
// star, a spray of feathers, or a tuft glowing with one of the elements, in the order of
// Element::ALL
pub const CRACKER_HAIR: usize = 0;
pub const CRACKER_SILK: usize = 1;
pub const CRACKER_KNOTS: usize = 2;
pub const CRACKER_BARBS: usize = 3;
pub const CRACKER_WEIGHT: usize = 4;
pub const CRACKER_BONE: usize = 5;
pub const CRACKER_GOLD: usize = 6;
pub const CRACKER_HOOK: usize = 7;
pub const CRACKER_STAR: usize = 8;
pub const CRACKER_FEATHER: usize = 9;
pub const CRACKER_GLOW: usize = 10;
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
        shape => match glowing(cord).or_else(|| accent.and_then(glowing)) {
            Some(e) => LASH_GLOW + e,
            None if shape == Some("spine") => LASH_SPINE,
            None if shape == Some("wire") || cord == FUR_GINGER => LASH_WIRE,
            None if cord == TEMPLE_RED => LASH_SILK,
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
        ("hook", _) => CRACKER_HOOK,
        ("star", _) => CRACKER_STAR,
        ("feather", _) => CRACKER_FEATHER,
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
    if set == LASH_SPINE {
        // Bones strung end to end like a spine, a spur off each, every other one paler
        let bones = 4;
        for k in 0..bones {
            let z = (k as f64 + 0.5) / bones as f64;
            let material = if k % 2 == 0 { BONE } else { FUR_CREAM };
            p.push(ellipsoid([0.0, 0.0, z], [radius * 1.55, radius * 1.55, 0.42 / bones as f64], material));
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            p.push(limb([0.0, 0.0, z], [side * radius * 2.6, 0.0, z - radius * 0.8], radius * 0.55, radius * 0.15, BONE));
        }
        p.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], radius * 0.7, radius * 0.7, FUR_CREAM));
        return p;
    }
    let (cord, band) = match set {
        LASH_LEATHER => (LEATHER, FUR_TAN),
        LASH_HEMP => (LINEN, WAX),
        LASH_VINE => (MOSS, BARK_DARK),
        LASH_WIRE => (FUR_GINGER, DARK_STEEL),
        LASH_SILK => (TEMPLE_RED, GOLD),
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
        // An iron hook bent back on itself, a barb at its point, for catching hold
        CRACKER_HOOK => {
            let bend: Vec<[f64; 3]> = (0..=8).map(|k| k as f64 / 8.0 * PI * 1.1).map(|a| [0.07 - a.cos() * 0.07, 0.0, 0.88 + a.sin() * 0.1]).collect();
            curve(&mut p, &bend, r * 1.1, r * 0.5, DARK_STEEL);
            p.push(limb([0.14, 0.0, 0.9], [0.115, 0.0, 0.84], r * 0.7, r * 0.15, STEEL));
            p.push(limb([0.0, 0.0, 0.64], [0.0, 0.0, 0.9], r * 1.2, r * 1.1, DARK_STEEL));
            LEATHER
        }
        // A spiked iron star, heavy enough to drag the crack
        CRACKER_STAR => {
            for d in sphere_points(8) {
                p.push(limb([0.0, 0.0, 0.93], [d[0] * 0.075, d[1] * 0.075, 0.93 + d[2] * 0.075], r * 0.8, r * 0.15, STEEL));
            }
            p.push(ball([0.0, 0.0, 0.93], 0.042, DARK_STEEL));
            LEATHER
        }
        // A spray of feathers, pale and banded, fanning out off the end
        CRACKER_FEATHER => {
            for (k, (dx, dy)) in [(1.0, 0.0), (-0.5, 0.87), (-0.5, -0.87), (0.3, 0.5), (0.3, -0.5)].into_iter().enumerate() {
                let material = if k % 2 == 0 { FUR_WHITE } else { FUR_TAN };
                p.push(limb([0.0, 0.0, 0.66], [dx * 0.06, dy * 0.06, 1.04], r * 1.3, r * 0.2, material).stretched([1.0, 0.4, 1.0]));
            }
            LINEN
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
    if let Some(stone) = core.gem.as_deref().and_then(material) {
        let r = core.size.unwrap_or(0.02).max(0.017);
        gem(&mut p, [0.0, 0.0, z_top], r, stone, core.shape.as_deref());
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
            "serpent" => -0.1 * s * s + 0.025 * (s * 7.0).sin(),
            "bone" | "winged" => -0.12 * s * s + 0.08 * tip * tip,
            "flat" => -0.09 * s * s,
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

    if let Some(stone) = core.gem.as_deref().and_then(material) {
        gem(&mut p, [0.018, 0.0, mid + 0.09], core.size.unwrap_or(0.022).max(0.014), stone, core.shape.as_deref());
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
        // Knuckled joints all along a bone one
        if shape == "bone" {
            for k in (2..steps).step_by(3) {
                p.push(ellipsoid(points[k], [0.016, 0.022, 0.012], mat(&limbs.accent, BONE)));
            }
        }
        // Blades swept back off the tips, like wings
        if shape == "winged" {
            for (k, long) in [(steps, 0.16), (steps - 3, 0.12), (steps - 6, 0.08)] {
                let q = points[k];
                p.push(limb(q, [q[0] - long, q[1], q[2] - half * long * 0.6], 0.011, 0.002, mat(&limbs.accent, STEEL)).stretched([1.0, 0.5, 1.0]));
            }
        }
        let flat = if shape == "flat" { 1.9 } else { 1.35 };
        for pair in points.windows(2) {
            let k = (pair[0][2] - mid).abs() / mid;
            p.push(limb(pair[0], pair[1], 0.017 - 0.008 * k, 0.017 - 0.008 * k, wood).stretched([0.7, flat, 1.0]));
        }
    }
    (p, v3(0.0, 0.0, mid), v3(nock[0], 0.0, nock[2]))
}

/// How far a twin-prod crossbow's upper prod and groove are over its lower, in meters
pub const TWIN_RISE: f64 = 0.046;

/// The handle on the end of what spans a crossbow, at `at`, the fist closing round `axis`: a
/// wooden bar, a metal ball, a little spoked wheel or a crossbar, as its latch's `crank` says
fn crank_handle(p: &mut Vec<Part>, style: &str, at: [f64; 3], axis: [f64; 3], wood: u8, metal: u8) {
    let along = |k: f64| [at[0] + axis[0] * k, at[1] + axis[1] * k, at[2] + axis[2] * k];
    match style {
        "ball" => {
            p.push(ball(along(0.035), 0.016, metal));
            p.push(limb(along(-0.03), along(0.03), 0.007, 0.007, metal));
        }
        "spoke" => {
            // A little wheel round the end, its rim to grip, on a short axle
            let (u, w) = if axis[1].abs() > 0.5 { ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]) } else { ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]) };
            ring(p, along(0.03), u, w, 0.03, 0.0055, metal);
            for k in 0..3 {
                let a = k as f64 / 3.0 * TAU;
                let c = along(0.03);
                p.push(limb(c, [c[0] + (u[0] * a.cos() + w[0] * a.sin()) * 0.03, c[1] + (u[1] * a.cos() + w[1] * a.sin()) * 0.03, c[2] + (u[2] * a.cos() + w[2] * a.sin()) * 0.03], 0.003, 0.003, metal));
            }
            p.push(limb(along(-0.03), along(0.035), 0.009, 0.009, wood));
        }
        "tee" => {
            let across = if axis[2].abs() > 0.5 { [1.0, 0.0, 0.0] } else { [0.0, 0.0, 1.0] };
            let c = along(0.03);
            p.push(limb([c[0] - across[0] * 0.035, c[1] - across[1] * 0.035, c[2] - across[2] * 0.035], [c[0] + across[0] * 0.035, c[1] + across[1] * 0.035, c[2] + across[2] * 0.035], 0.008, 0.008, wood));
            p.push(limb(along(-0.03), c, 0.007, 0.007, metal));
        }
        _ => p.push(limb(along(-0.04), along(0.04), 0.009, 0.009, wood)),
    }
}

/// A crossbow's sight, from `rear`, over the latch, to `front`, over the prod, both where it's set
/// on whatever's on top there. A scope or lens sits off to the side the one holding it sees,
/// `seen`, clear of the bolt.
fn crossbow_sight(p: &mut Vec<Part>, sight: &Visual, rear: [f64; 3], front: [f64; 3], seen: f64) {
    let Some(shape) = sight.shape.as_deref() else {
        return;
    };
    let m = mat(&sight.material, DARK_STEEL);
    let gem = mat(&sight.gem, m);
    let side = |q: [f64; 3], y: f64| [q[0], q[1] + seen * y, q[2]];
    // A post at the front, off to the side of the groove, with a bead on top
    let bead = |p: &mut Vec<Part>, material: u8| {
        let f = side(front, 0.022);
        p.push(ball([f[0], f[1], f[2] + 0.024], 0.0055, material));
        p.push(limb(f, [f[0], f[1], f[2] + 0.02], 0.0035, 0.003, m));
    };
    match shape {
        "peep" => {
            ring(p, [rear[0], rear[1], rear[2] + 0.034], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 0.012, 0.0035, m);
            p.push(limb(rear, [rear[0], rear[1], rear[2] + 0.022], 0.0045, 0.004, m));
            bead(p, gem);
        }
        "leaf" => {
            // A folding leaf, notched at the top, and the bead to line it up with
            p.push(cuboid([rear[0] - 0.003, -0.004, rear[2] + 0.02], [rear[0] + 0.003, 0.004, rear[2] + 0.034], AIR));
            p.push(cuboid([rear[0] - 0.003, -0.014, rear[2]], [rear[0] + 0.003, 0.014, rear[2] + 0.034], m));
            for k in 0..3 {
                p.push(cuboid([rear[0] + 0.003, -0.012, rear[2] + 0.006 + k as f64 * 0.009], [rear[0] + 0.005, 0.012, rear[2] + 0.008 + k as f64 * 0.009], gem));
            }
            bead(p, gem);
        }
        // A brass tube on two mounts, glass glinting at both ends
        "scope" => {
            let (a, b) = (side([rear[0] - 0.02, rear[1], rear[2] + 0.045], 0.028), side([rear[0] + 0.2, rear[1], rear[2] + 0.045], 0.028));
            p.push(ball([a[0] - 0.001, a[1], a[2]], 0.011, mat(&sight.gem, CRYSTAL)));
            p.push(ball([b[0] + 0.001, b[1], b[2]], 0.012, mat(&sight.gem, CRYSTAL)));
            for x in [a[0] + 0.012, b[0] - 0.012] {
                p.push(limb([x - 0.006, a[1], a[2]], [x + 0.006, a[1], a[2]], 0.0145, 0.0145, mat(&sight.accent, m)));
            }
            p.push(limb(a, b, 0.0115, 0.013, m));
            for x in [a[0] + 0.05, b[0] - 0.05] {
                p.push(cuboid([x - 0.008, a[1] - 0.006, rear[2]], [x + 0.008, a[1] + 0.006, a[2]], m));
            }
        }
        // A disc of crystal held up in a ring on a stalk
        "lens" => {
            let c = side([rear[0], rear[1], rear[2] + 0.05], 0.02);
            p.push(ellipsoid(c, [0.004, 0.018, 0.018], gem));
            ring(p, c, [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 0.02, 0.004, m);
            p.push(limb(side(rear, 0.02), [c[0], c[1], c[2] - 0.02], 0.004, 0.0035, m));
        }
        // A little skull for a bead, its eyes glowing
        "skull" => {
            let f = side(front, 0.022);
            for y in [-1.0, 1.0] {
                p.push(ball([f[0] + 0.009, f[1] + y * 0.005, f[2] + 0.03], 0.003, gem));
            }
            p.push(ellipsoid([f[0], f[1], f[2] + 0.028], [0.011, 0.01, 0.01], BONE));
            p.push(limb(f, [f[0], f[1], f[2] + 0.02], 0.0035, 0.003, m));
        }
        // A ring of runes standing over the front
        "rune" => {
            let f = side(front, 0.022);
            ring(p, [f[0], f[1], f[2] + 0.034], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], 0.014, 0.004, gem);
            p.push(limb(f, [f[0], f[1], f[2] + 0.02], 0.0035, 0.003, m));
            p.push(limb(rear, [rear[0], rear[1], rear[2] + 0.016], 0.004, 0.0035, m));
        }
        _ => bead(p, gem),
    }
}

/// A crossbow lying level, shooting along +x, the top of its tiller at z 0 and its butt at x 0,
/// its prod across the front bending back towards its tips. Its frames are its string forward,
/// halfway back and spanned to the latch, then what spans it, drawn apart as it moves: a light
/// one's lever swinging over its top, a heavy one's windlass cranks, the lever of a repeater
/// arching over its magazine, a hand crossbow's lever folded under it, a twin-prod's rack and
/// pinion crank, a siege crossbow's screw jack, a harpoon's reel or a bolt-thrower's side lever.
/// A siege crossbow's leg is drawn apart too, folded under its front. Parts make the prod longer
/// or shorter than the family's span, and give the prod, the stock, the latch and the sight their
/// shapes.
fn crossbow(family: &Family, prod: &Visual, stock: &Visual, lock: &Visual, sight: &Visual, core: &Visual) -> WeaponLook {
    let build = family.build;
    let length = family.length;
    let half = family.span * prod.length.unwrap_or(1.0) * 0.5;
    let wood = mat(&stock.material, WOOD);
    let trim = mat(&stock.accent, DARK_STEEL);
    let metal = mat(&lock.material, DARK_STEEL);
    let flourish = mat(&lock.accent, metal);
    let bow = mat(&prod.material, WOOD);
    let band = mat(&prod.accent, trim);
    let cord = mat(&prod.edge, BOWSTRING);
    let tips = prod.tips.as_deref().and_then(material);
    let shape = prod.shape.as_deref().unwrap_or("simple");
    let stock_shape = stock.shape.as_deref().unwrap_or("tiller");
    let trigger = lock.shape.as_deref().unwrap_or("lever");
    let handle = lock.crank.as_deref().unwrap_or("knob");
    // The side the one holding it sees: its left held in the right hand, its right in the left
    let seen = if build == Build::Hand { -1.0 } else { 1.0 };
    let x_prod = length - 0.06;
    let x_latch = length
        * match build {
            Build::Hand => 0.4,
            Build::Siege => 0.36,
            Build::Fan => 0.4,
            _ => 0.42,
        };
    // How thick its prod is and how far it bends, next to a light crossbow's
    let (thick, stiff): (f64, f64) = match build {
        Build::Siege => (1.7, 0.75),
        Build::Harpoon => (1.35, 0.85),
        Build::Hand => (0.7, 1.0),
        Build::Fan => (1.25, 0.9),
        _ => (1.0, 1.0),
    };
    // How far back the prod bends at `s`, from -1 at one tip to 1 at the other, bending further
    // spanned, and more the longer it is
    let bend = |s: f64, draw: f64| {
        let tip = ((s.abs() - 0.7) / 0.3).max(0.0);
        let x = match shape {
            "recurve" => -0.11 * s * s + 0.07 * tip * tip,
            "composite" => -0.1 * s * s + 0.035 * tip * tip,
            "steel" => -0.075 * s * s,
            "reflex" => -0.12 * s * s + 0.12 * tip * tip,
            "compound" => -0.05 * s * s,
            "horned" => -0.13 * s * s + 0.17 * tip * tip,
            "ribbed" => -0.09 * s * s,
            _ => -0.1 * s * s,
        };
        x * stiff * (half / 0.31) * (1.0 + 0.8 * draw)
    };
    let at = |s: f64, draw: f64, z: f64| [x_prod + bend(s, draw), s * half, z];
    let nock_at = |draw: f64| (x_prod + bend(1.0, draw)) * (1.0 - draw) + x_latch * draw;
    // A twin-prod's two prods, one over the other
    let decks: Vec<f64> = if build == Build::Twin { vec![0.004, 0.004 + TWIN_RISE] } else { vec![0.004] };
    let rails: Vec<(Vector3<f64>, f64)> = match build {
        Build::Twin => vec![(Vector3::zeros(), 0.0), (v3(0.0, 0.0, TWIN_RISE), 0.0)],
        Build::Fan => (-2..=2).map(|k| (v3(0.0, k as f64 * 0.028, 0.0), k as f64 * 0.12)).collect(),
        _ => vec![(Vector3::zeros(), 0.0)],
    };
    // A bolt-thrower's frame widens out from the latch, to hold its grooves
    let fan_half = |x: f64| 0.032 + (x - x_latch).max(0.0) * 0.3;
    // Where what spans it is: a heavy one's windlass, a twin's crank, a harpoon's reel
    let windlass = [0.07, 0.0, 0.02];
    let pinion = [0.2, seen * 0.05, -0.022];
    let reel = [x_latch + 0.14, 0.0, -0.075];
    // How high the top of what's over the latch is, where the sight goes
    let deck = match build {
        Build::Repeater => 0.13,
        Build::Twin => TWIN_RISE + 0.012,
        _ => 0.003,
    };

    let body = |draw: f64| {
        let mut p = Vec::new();
        let nock = nock_at(draw);
        // The strings, a bolt-thrower's to the ends of the yoke that drives its bolts
        for &z in &decks {
            if build == Build::Fan {
                for s in [-1.0, 1.0] {
                    p.push(limb(at(s, draw, z), [nock, s * 0.075, 0.006], 0.0055, 0.0055, cord));
                }
            } else {
                for s in [-1.0, 1.0] {
                    p.push(limb(at(s, draw, z), [nock, 0.0, z + 0.002], 0.0045 * thick.sqrt(), 0.0045 * thick.sqrt(), cord));
                }
            }
        }
        if build == Build::Fan {
            p.push(limb([nock, -0.08, 0.006], [nock, 0.08, 0.006], 0.0065, 0.0065, metal));
        }
        // What draws the string back, hooked onto it: a windlass's cords, a twin's rack, a siege
        // crossbow's screw, a harpoon's line
        match build {
            Build::Crank => {
                for s in [-1.0, 1.0] {
                    p.push(ball([nock - 0.008, s * 0.012, 0.008], 0.007, metal));
                    p.push(limb([windlass[0], s * 0.03, windlass[2] + 0.018], [nock - 0.008, s * 0.012, 0.008], 0.003, 0.003, cord));
                }
            }
            Build::Twin if draw > 0.25 => {
                // A toothed rack along the top from the gearbox to a claw over both strings
                let z = TWIN_RISE + 0.028;
                for k in 0..((nock - 0.03 - pinion[0]) / 0.02).max(0.0) as usize {
                    let x = pinion[0] + 0.02 + k as f64 * 0.02;
                    p.push(cuboid([x, -0.006, z + 0.006], [x + 0.008, 0.006, z + 0.011], flourish));
                }
                p.push(cuboid([pinion[0], -0.006, z - 0.006], [nock - 0.004, 0.006, z + 0.006], metal));
                p.push(cuboid([nock - 0.012, -0.012, 0.0], [nock - 0.004, 0.012, z + 0.006], metal));
            }
            Build::Siege => {
                // The screw from the jack at the butt to a claw on the string
                let z = 0.024;
                for k in 0..((nock - 0.04) / 0.018).max(0.0) as usize {
                    let x = 0.01 + k as f64 * 0.018;
                    p.push(limb([x, 0.0, z], [x + 0.006, 0.0, z], 0.011, 0.011, flourish));
                }
                p.push(limb([-0.03, 0.0, z], [nock - 0.01, 0.0, z], 0.008, 0.008, metal));
                p.push(cuboid([nock - 0.02, -0.022, 0.0], [nock - 0.004, 0.022, z + 0.01], metal));
            }
            Build::Harpoon => {
                for s in [-1.0, 1.0] {
                    p.push(limb([reel[0], s * 0.02, reel[2] + 0.02], [nock - 0.01, s * 0.01, 0.006], 0.0028, 0.0028, LINEN));
                }
            }
            _ => {}
        }
        if let Some(stone) = core.gem.as_deref().and_then(material) {
            gem(&mut p, [length * 0.6, seen * 0.02 * if build == Build::Fan { 1.6 } else { 1.0 }, -0.02], core.size.unwrap_or(0.02).max(0.013) * 0.8, stone, core.shape.as_deref());
        }
        // The grooves the bolts lie in, cut out of the tiller's top, fanned out on a bolt-thrower
        if build == Build::Fan {
            for &(offset, yaw) in &rails {
                let reach = length + 0.04 - x_latch;
                p.push(limb([x_latch + 0.012, offset.y, -0.002], [x_latch + 0.012 + reach * yaw.cos(), offset.y + reach * yaw.sin(), -0.002], 0.0045, 0.0045, AIR));
            }
        } else {
            for &z in &decks {
                p.push(cuboid([x_latch + 0.012, -0.004, z - 0.01], [length + 0.01, 0.004, z - 0.003], AIR));
            }
        }
        // The nut that catches the string, a bar across the yoke's for a bolt-thrower
        if build == Build::Fan {
            p.push(limb([x_latch, -0.07, 0.002], [x_latch, 0.07, 0.002], 0.008, 0.008, metal));
        } else {
            for &z in &decks {
                p.push(ball([x_latch, 0.0, z - 0.002], 0.011 * thick.sqrt().min(1.2), metal));
            }
        }
        // The trigger: a long lever under the tiller, or a short one in a guard, as a pistol has
        let grip_x = grip_x_of(x_latch);
        if build == Build::Hand || trigger == "guard" {
            let x = if build == Build::Hand { x_latch + 0.005 } else { grip_x };
            p.push(limb([x, 0.0, -0.03], [x - 0.006, 0.0, -0.052], 0.0035, 0.003, metal));
            ring(&mut p, [x - 0.008, 0.0, -0.048], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.019, 0.0032, flourish);
        } else if trigger == "twin" {
            for dx in [0.0, 0.03] {
                p.push(limb([x_latch + 0.01 - dx, 0.0, -0.035], [x_latch - 0.1 - dx, 0.0, -0.07], 0.005, 0.004, metal));
                p.push(ball([x_latch - 0.1 - dx, 0.0, -0.07], 0.007, flourish));
            }
        } else if build != Build::Siege {
            p.push(limb([x_latch + 0.01, 0.0, -0.035], [x_latch - 0.13, 0.0, -0.075], 0.006, 0.005, metal));
            p.push(ball([x_latch - 0.13, 0.0, -0.075], 0.008, flourish));
        }
        let rear = [x_latch - 0.05, 0.0, deck];
        let front = [x_prod - 0.01, 0.0, if build == Build::Twin { deck } else { 0.012 }];
        crossbow_sight(&mut p, sight, rear, front, seen);
        // The prod, bound on at the front, its tips capped, a compound one's with wheels the
        // string runs round
        if let Some(tips) = tips {
            for &z in &decks {
                for s in [-1.0, 1.0] {
                    p.push(ball(at(s, draw, z), 0.012 * thick.sqrt(), tips));
                }
            }
        }
        if shape == "compound" {
            for &z in &decks {
                for s in [-1.0, 1.0] {
                    let c = at(s, draw, z);
                    p.push(ball(c, 0.006, flourish));
                    ring(&mut p, [c[0] - 0.006, c[1], c[2]], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0.02, 0.0045, metal);
                }
            }
        }
        let top = decks[decks.len() - 1];
        let width = if build == Build::Fan { fan_half(x_prod) + 0.01 } else { 0.03 * thick.sqrt() };
        p.push(cuboid([x_prod - 0.028 * thick.sqrt(), -width, -0.038 * thick.sqrt()], [x_prod + 0.028 * thick.sqrt(), width, top + 0.006], trim));
        let steps = 12;
        for &z in &decks {
            for side in [-1.0, 1.0] {
                let points: Vec<[f64; 3]> = (0..=steps).map(|k| at(side * k as f64 / steps as f64, draw, z)).collect();
                for (k, pair) in points.windows(2).enumerate() {
                    let r = if matches!(shape, "steel" | "compound") { 0.011 } else { 0.015 } * thick * (1.0 - 0.45 * k as f64 / steps as f64);
                    if shape == "spined" && k % 3 == 1 {
                        let q = pair[0];
                        p.push(limb(q, [q[0] + 0.035, q[1] + side * 0.012, q[2]], r * 0.6, 0.002, band));
                    }
                    // Ribs bound round it, or a horn's ridges, and its tips curling right round
                    if shape == "ribbed" && k % 2 == 0 {
                        p.push(ellipsoid(pair[0], [r * 1.5, r * 0.8, r * 2.2], band));
                    }
                    if shape == "horned" && k % 2 == 1 {
                        p.push(ellipsoid(pair[0], [r * 1.25, r * 1.1, r * 1.9], band));
                    }
                    if shape == "horned" && k == steps - 1 {
                        let q = pair[1];
                        curve(&mut p, &[q, [q[0] + 0.025, q[1] + side * 0.01, q[2]], [q[0] + 0.03, q[1] - side * 0.015, q[2]]], r * 0.8, 0.003, band);
                    }
                    let material = if shape == "laminate" && k % 2 == 1 { band } else { bow };
                    p.push(limb(pair[0], pair[1], r, r, material).stretched([1.0, 1.0, 1.7]));
                }
            }
        }
        match build {
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
            Build::Repeater => {
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
            // The rail the upper prod's groove runs along, and the gearbox at the side with its
            // pinion, and the rack's guide over the butt
            Build::Twin => {
                p.push(cuboid([x_latch - 0.03, -0.016, 0.0], [x_prod + 0.02, 0.016, TWIN_RISE - 0.003], wood));
                for x in [x_latch + 0.08, x_prod - 0.08] {
                    p.push(cuboid([x - 0.006, -0.02, -0.04], [x + 0.006, 0.02, TWIN_RISE], trim));
                }
                p.push(ball(pinion, 0.012, flourish));
                p.push(cuboid([pinion[0] - 0.05, seen * 0.02, -0.05], [pinion[0] + 0.05, seen * 0.042, 0.004], metal));
                p.push(cuboid([pinion[0] - 0.04, -0.01, TWIN_RISE + 0.015], [pinion[0] + 0.02, 0.01, TWIN_RISE + 0.045], metal));
                p.push(cuboid([pinion[0] - 0.02, -0.012, -0.01], [pinion[0] + 0.0, 0.012, TWIN_RISE + 0.03], trim));
            }
            // The jack's nut at the butt, iron straps round the tiller, the hinge of the leg
            Build::Siege => {
                p.push(cuboid([-0.02, -0.036, -0.055], [0.03, 0.036, 0.04], trim));
                for x in [0.2, x_latch + 0.2, x_prod - 0.12] {
                    p.push(cuboid([x - 0.008, -0.034, -0.058], [x + 0.008, 0.034, 0.004], trim));
                }
                p.push(limb([length - 0.1, -0.03, -0.06], [length - 0.1, 0.03, -0.06], 0.009, 0.009, metal));
            }
            // The reel under the tiller, its line wound round it, on its hanger
            Build::Harpoon => {
                for k in 0..5 {
                    let y = -0.03 + k as f64 * 0.015;
                    p.push(limb([reel[0], y, reel[2]], [reel[0], y + 0.009, reel[2]], 0.03, 0.03, if k % 2 == 0 { LINEN } else { BOWSTRING }));
                }
                for s in [-1.0, 1.0] {
                    p.push(ellipsoid([reel[0], s * 0.042, reel[2]], [0.04, 0.005, 0.04], metal));
                    p.push(cuboid([reel[0] - 0.012, s * 0.048 - 0.003, reel[2]], [reel[0] + 0.012, s * 0.048 + 0.003, -0.01], metal));
                }
                p.push(limb([reel[0], -0.05, reel[2]], [reel[0], 0.05, reel[2]], 0.007, 0.007, flourish));
            }
            // The lever's pivot at the side of the frame
            Build::Fan => {
                let x = x_prod - 0.12;
                p.push(limb([x, seen * (fan_half(x) - 0.01), -0.03], [x, seen * (fan_half(x) + 0.02), -0.03], 0.012, 0.012, metal));
            }
            _ => {}
        }
        // A stirrup out front to stand in while spanning it
        if matches!(build, Build::Lever | Build::Crank | Build::Twin) {
            ring(&mut p, [length + 0.045, 0.0, -0.016], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0], 0.04, 0.0065, metal);
        }
        // Then the tiller, and its stock
        match build {
            // Short, on a pistol grip under the latch, capped
            Build::Hand => {
                if stock_shape == "skeleton" {
                    p.push(cuboid([0.08, -0.03, -0.024], [length - 0.08, 0.03, -0.01], AIR));
                }
                p.push(cuboid([x_latch - 0.025, -0.022, -0.036], [x_latch + 0.005, 0.022, 0.003], trim));
                p.push(cuboid([0.05, -0.016, -0.03], [length, 0.016, 0.0], wood));
                p.push(ball([x_latch - 0.095, 0.0, -0.14], 0.021, trim));
                p.push(limb([x_latch - 0.015, 0.0, -0.022], [x_latch - 0.09, 0.0, -0.13], 0.016, 0.02, wood).stretched([1.0, 0.75, 1.0]));
                p.push(limb([0.03, 0.0, -0.016], [0.08, 0.0, -0.016], 0.012, 0.016, wood));
            }
            // A great beam of a tiller, held by two upright grips, braced, not shouldered
            Build::Siege => {
                for x in [0.17, x_latch + 0.3] {
                    p.push(ball([x - 0.04, 0.0, -0.17], 0.02, trim));
                    p.push(limb([x, 0.0, -0.05], [x - 0.04, 0.0, -0.165], 0.018, 0.021, wood).stretched([1.0, 0.8, 1.0]));
                }
                p.push(cuboid([0.0, -0.03, -0.052], [length, 0.03, 0.0], wood));
            }
            _ => {
                // The tiller, a bolt-thrower's a broad frame widening to the front
                if build == Build::Fan {
                    let rows = ((x_prod - x_latch + 0.05) * CROSSBOW_RES).ceil() as usize;
                    for r in 0..rows {
                        let x = x_latch - 0.05 + r as f64 / CROSSBOW_RES;
                        let w = fan_half(x);
                        p.push(cuboid([x, -w, -0.032], [x + 1.0 / CROSSBOW_RES, w, 0.0], wood));
                    }
                    for x in [x_latch + 0.12, x_prod - 0.15] {
                        p.push(cuboid([x - 0.006, -fan_half(x) - 0.003, -0.036], [x + 0.006, fan_half(x) + 0.003, 0.002], trim));
                    }
                } else {
                    for x in [0.26, x_latch + 0.16] {
                        p.push(cuboid([x - 0.006, -0.025, -0.043], [x + 0.006, 0.025, 0.003], trim));
                    }
                }
                if stock_shape == "skeleton" {
                    for x in [x_latch + 0.06, x_latch + 0.25] {
                        p.push(cuboid([x, -0.03, -0.032], [x + 0.07, 0.03, -0.012], AIR));
                    }
                }
                p.push(cuboid([0.2, -0.022, -0.04], [length, 0.022, 0.0], wood));
                // The stock: to the shoulder, on a pistol grip, cut through for the thumb, an
                // open frame, or with a rest for the cheek
                match stock_shape {
                    "pistol" => {
                        p.push(ball([grip_x - 0.075, 0.0, -0.15], 0.021, trim));
                        p.push(limb([grip_x + 0.01, 0.0, -0.03], [grip_x - 0.07, 0.0, -0.14], 0.016, 0.02, wood).stretched([1.0, 0.75, 1.0]));
                        p.push(cuboid([0.12, -0.016, -0.06], [0.135, 0.016, -0.02], trim));
                        p.push(limb([0.13, 0.0, -0.04], [0.26, 0.0, -0.022], 0.022, 0.02, wood).stretched([1.0, 0.6, 1.0]));
                    }
                    "skeleton" => {
                        p.push(cuboid([0.0, -0.016, -0.112], [0.012, 0.016, -0.02], trim));
                        p.push(limb([0.01, 0.0, -0.03], [0.26, 0.0, -0.02], 0.011, 0.011, wood));
                        p.push(limb([0.01, 0.0, -0.1], [0.24, 0.0, -0.035], 0.011, 0.011, wood));
                        p.push(limb([0.13, 0.0, -0.03], [0.13, 0.0, -0.07], 0.008, 0.008, trim));
                    }
                    shape => {
                        if shape == "thumbhole" {
                            p.push(limb([0.14, -0.05, -0.05], [0.14, 0.05, -0.05], 0.014, 0.014, AIR));
                        }
                        if shape == "cheek" {
                            p.push(ellipsoid([0.11, seen * 0.02, -0.036], [0.07, 0.012, 0.022], trim));
                        }
                        p.push(cuboid([0.0, -0.016, -0.112], [0.012, 0.016, -0.02], trim));
                        p.push(limb([0.03, 0.0, -0.065], [0.26, 0.0, -0.022], 0.048, 0.024, wood).stretched([1.0, 0.5, 1.0]));
                    }
                }
            }
        }
        p
    };
    let mut frames: Vec<(Model, Vector3<f64>)> = [0.0, 0.5, 1.0].map(|draw| voxelize(&body(draw), CROSSBOW_RES)).into();
    let nocks: Vec<Vector3<f64>> = [0.0, 0.5, 1.0].map(|draw| v3(nock_at(draw), 0.0, 0.006)).into();

    // What spans it, at rest
    let mut m = Vec::new();
    let mover = match build {
        // A windlass's two cranks, hanging down either side of the butt
        Build::Crank => {
            for s in [-1.0, 1.0] {
                let y = s * 0.06;
                m.push(limb([windlass[0], y, windlass[2]], [windlass[0], y, windlass[2] - 0.085], 0.007, 0.006, metal));
                crank_handle(&mut m, handle, [windlass[0], y + s * 0.022, windlass[2] - 0.085], [0.0, s, 0.0], wood, flourish);
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
            crank_handle(&mut m, handle, [end, 0.0, pivot[2] + 0.01], [0.0, 1.0, 0.0], wood, flourish);
            Mover { frame: 3, pivot: Vector3::from(pivot), axis: Vector3::y(), handle: v3(end, 0.0, pivot[2] + 0.01), handle_axis: Vector3::y() }
        }
        // A lever turning on pins at the front, arching up over the magazine to a handle behind it
        Build::Repeater => {
            let pivot = [length - 0.005, 0.0, -0.03];
            let points = [pivot, [length - 0.02, 0.0, 0.1], [x_latch + 0.08, 0.0, 0.17], [x_latch - 0.07, 0.0, 0.16]];
            for s in [-1.0, 1.0] {
                let side: Vec<[f64; 3]> = points.iter().map(|q| [q[0], s * 0.027, q[2]]).collect();
                curve(&mut m, &side, 0.0065, 0.0055, metal);
            }
            let end = points[3];
            crank_handle(&mut m, handle, end, [0.0, 1.0, 0.0], wood, flourish);
            Mover { frame: 3, pivot: Vector3::from(pivot), axis: Vector3::y(), handle: v3(end[0], 0.0, end[2]), handle_axis: Vector3::y() }
        }
        // A lever folded up under the tiller from a hinge at the front, its handle just ahead of
        // the trigger, swung down and back up by the other hand
        Build::Hand => {
            let pivot = [length - 0.03, 0.0, -0.034];
            let end = [x_latch + 0.04, 0.0, -0.045];
            for s in [-1.0, 1.0] {
                m.push(limb([pivot[0], s * 0.019, pivot[2]], [end[0], s * 0.019, end[2]], 0.0045, 0.004, metal));
            }
            m.push(ball(pivot, 0.007, flourish));
            crank_handle(&mut m, handle, end, [0.0, 1.0, 0.0], wood, flourish);
            Mover { frame: 3, pivot: Vector3::from(pivot), axis: Vector3::y(), handle: Vector3::from(end), handle_axis: Vector3::y() }
        }
        // The crank on the gearbox's pinion, hanging down
        Build::Twin => {
            let end = [pinion[0], pinion[1] + seen * 0.03, pinion[2] - 0.07];
            m.push(limb([pinion[0], pinion[1] + seen * 0.02, pinion[2]], [end[0], end[1] - seen * 0.01, end[2]], 0.0065, 0.0055, metal));
            crank_handle(&mut m, handle, end, [0.0, seen, 0.0], wood, flourish);
            Mover { frame: 3, pivot: Vector3::from(pinion), axis: Vector3::y(), handle: Vector3::from(end), handle_axis: Vector3::y() }
        }
        // The jack's crossbar handle at the butt, spun round the screw
        Build::Siege => {
            let pivot = [-0.035, 0.0, 0.024];
            m.push(limb([pivot[0] - 0.01, 0.0, pivot[2]], [pivot[0] + 0.012, 0.0, pivot[2]], 0.016, 0.016, metal));
            for s in [-1.0, 1.0] {
                m.push(limb(pivot, [pivot[0], s * 0.075, pivot[2]], 0.007, 0.007, metal));
                crank_handle(&mut m, if handle == "knob" { "ball" } else { handle }, [pivot[0] - 0.03, s * 0.075, pivot[2]], [-1.0, 0.0, 0.0], wood, flourish);
            }
            Mover { frame: 3, pivot: Vector3::from(pivot), axis: Vector3::x(), handle: v3(pivot[0] - 0.03, 0.075, pivot[2]), handle_axis: Vector3::x() }
        }
        // The reel's crank, out at the side
        Build::Harpoon => {
            let hub = [reel[0], seen * 0.055, reel[2]];
            let end = [reel[0], seen * 0.07, reel[2] - 0.065];
            m.push(limb(hub, [end[0], end[1] - seen * 0.012, end[2]], 0.0065, 0.0055, metal));
            crank_handle(&mut m, handle, end, [0.0, seen, 0.0], wood, flourish);
            Mover { frame: 3, pivot: Vector3::from(hub), axis: Vector3::y(), handle: Vector3::from(end), handle_axis: Vector3::y() }
        }
        // A long lever lying back along the side of the frame from its pivot near the front,
        // swung out and back again
        _ => {
            let x = x_prod - 0.12;
            let pivot = [x, seen * (fan_half(x) + 0.02), -0.03];
            let end = [0.16, pivot[1], -0.03];
            m.push(limb(pivot, end, 0.008, 0.007, metal));
            m.push(limb(pivot, [pivot[0] + 0.05, pivot[1] - seen * 0.03, -0.03], 0.008, 0.005, metal));
            crank_handle(&mut m, handle, end, [0.0, 0.0, 1.0], wood, flourish);
            Mover { frame: 3, pivot: Vector3::from(pivot), axis: Vector3::z(), handle: Vector3::from(end), handle_axis: Vector3::z() }
        }
    };
    frames.push(voxelize(&m, CROSSBOW_RES));
    // A siege crossbow's leg, folded back under its front, a forked foot on its end
    let leg = (build == Build::Siege).then(|| {
        let pivot = [length - 0.1, 0.0, -0.06];
        let foot = [length - 0.52, 0.0, -0.075];
        let mut l = vec![limb(pivot, foot, 0.014, 0.012, metal)];
        for s in [-1.0, 1.0] {
            l.push(limb(foot, [foot[0] - 0.05, s * 0.035, foot[2] - 0.004], 0.009, 0.006, flourish));
        }
        frames.push(voxelize(&l, CROSSBOW_RES));
        // Let down a little past straight down, to lean on
        (Mover { frame: 4, pivot: Vector3::from(pivot), axis: Vector3::y(), handle: Vector3::from(foot), handle_axis: Vector3::y() }, -1.45)
    });
    // Held by the stock's wrist, a hand crossbow by its grip and a siege crossbow by the grip at
    // the back, steadied under the front
    let (grip, hold) = match build {
        Build::Hand => (v3(x_latch - 0.055, 0.0, -0.078), v3(0.55, 0.0, 0.83)),
        Build::Siege => (v3(0.15, 0.0, -0.11), v3(0.33, 0.0, 0.94)),
        _ if stock_shape == "pistol" => (v3(grip_x_of(x_latch) - 0.03, 0.0, -0.085), v3(0.6, 0.0, 0.8)),
        _ => (v3(x_latch - 0.08, 0.0, -0.04), Vector3::x()),
    };
    let support = match build {
        Build::Repeater | Build::Hand => None,
        Build::Siege => Some((v3(x_latch + 0.28, 0.0, -0.11), v3(0.33, 0.0, 0.94))),
        _ => Some((v3(x_latch + 0.22, 0.0, -0.03), Vector3::x())),
    };
    WeaponLook {
        frames,
        res: CROSSBOW_RES,
        grip,
        hold: hold.normalize(),
        tip: v3(length + 0.01, 0.0, 0.008),
        nocks,
        lash: None,
        mover: Some(mover),
        support,
        rails,
        leg,
        scale: 1.0,
    }
}

/// Where a crossbow's pistol grip is along it, for its latch at `x_latch`
fn grip_x_of(x_latch: f64) -> f64 {
    x_latch - 0.06
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
        match shape {
            // Thin rings standing out all down it
            "fluted" => {
                for k in 0..12 {
                    let t = (k as f64 + 0.5) / 12.0;
                    let x = x_b + (x_m - x_b) * t;
                    p.push(limb([x - 0.003, 0.0, 0.0], [x + 0.003, 0.0, 0.0], radius(t) * 1.12, radius(t) * 1.12, bands));
                }
            }
            // A band wound round and round it
            "spiral" => {
                let turns = ((x_m - x_b) / 0.05).max(3.0);
                let steps = (turns * 18.0) as usize;
                for k in 0..=steps {
                    let t = k as f64 / steps as f64;
                    let a = t * turns * TAU;
                    let r = radius(t) * 1.02;
                    p.push(ball([x_b + (x_m - x_b) * t, a.cos() * r, a.sin() * r], wall.max(0.004) * 0.7, bands));
                }
            }
            // Flaring out at the mouth like a horn's bell, its lip rolled
            "bell" => {
                let r = radius(1.0);
                p.push(limb([x_m + 0.005, 0.0, 0.0], [x_m + 0.012, 0.0, 0.0], r * 2.0, r * 2.0, bands));
                p.push(limb([x_m - 0.07, 0.0, 0.0], [x_m + 0.008, 0.0, 0.0], r * 1.02, r * 1.9, tube));
            }
            _ => {}
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
        if let Some(stone) = core.gem.as_deref().and_then(material) {
            let r = core.size.unwrap_or(0.02).max(0.013) * 0.75;
            let at = match build {
                Build::Pistol => [x_b - 0.06, 0.013, -0.06],
                Build::Cannon => [x_b + 0.05, 0.0, outer * 1.05],
                _ => [0.15, 0.021, -0.06],
            };
            gem(&mut p, at, r, stone, core.shape.as_deref());
        }

        // The trigger in its guard, then the stock
        p.push(limb([x_b - 0.035, 0.0, -0.02], [x_b - 0.043, 0.0, -0.042], 0.0035, 0.003, metal));
        if build != Build::Cannon {
            ring(&mut p, [x_b - 0.04, 0.0, -0.036], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.02, 0.0035, trim);
        }
        let fore = family.length;
        // Inlay winding along the stock's side, in its trim, first so it shows over the wood
        if stock.shape.as_deref() == Some("inlaid") && build != Build::Cannon {
            let (from, to) = if build == Build::Pistol { ([x_b - 0.02, -0.03], [x_b - 0.1, -0.09]) } else { ([x_b - 0.03, -0.035], [0.03, -0.07]) };
            for k in 0..=16 {
                let t = k as f64 / 16.0;
                let x = from[0] + (to[0] - from[0]) * t;
                let z = from[1] + (to[1] - from[1]) * t + 0.008 * (t * TAU * 3.0).sin();
                for s in [-1.0, 1.0] {
                    p.push(ball([x, s * 0.011, z], 0.006, trim));
                }
            }
        }
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
    WeaponLook { frames, res: GUN_RES, grip, hold: hold.normalize(), tip: v3(x_m, 0.0, 0.0), nocks: Vec::new(), lash: None, mover: Some(mover), support, rails: Vec::new(), leg: None, scale: 1.0 }
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
        // A six-sided column, cut to a point at both ends
        "prism" => {
            p.push(limb([c[0], c[1], c[2] - size * 1.5], [c[0], c[1], c[2] - size * 0.9], 0.003, size * 0.6, gem));
            p.push(cuboid([c[0] - size * 0.52, c[1] - size * 0.45, c[2] - size * 0.9], [c[0] + size * 0.52, c[1] + size * 0.45, c[2] + size * 0.9], gem));
            p.push(limb([c[0], c[1], c[2] + size * 0.9], [c[0], c[1], c[2] + size * 1.7], size * 0.6, 0.003, gem));
        }
        // Two shards leaning apart
        "twin" => {
            for s in [-1.0, 1.0] {
                p.push(limb([c[0] + s * size * 0.2, c[1], c[2] - size], [c[0] + s * size * 0.7, c[1], c[2] + size * 1.5], size * 0.45, 0.003, gem));
            }
        }
        // An orb in a broken shell of its own stone, points standing out of it
        "geode" => {
            for d in sphere_points(10) {
                if d[2] > -0.5 {
                    p.push(limb([c[0] + d[0] * size * 0.8, c[1] + d[1] * size * 0.8, c[2] + d[2] * size * 0.8], [c[0] + d[0] * size * 1.45, c[1] + d[1] * size * 1.45, c[2] + d[2] * size * 1.45], size * 0.22, 0.003, gem));
                }
            }
            p.push(ball(c, size * 0.8, gem));
            p.push(ellipsoid([c[0], c[1], c[2] - size * 0.35], [size * 1.0, size * 1.0, size * 0.6], ROCK));
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
        // A bird's foot clutching the crystal from below, four clawed toes
        "talon" => {
            for k in 0..4 {
                let a = (k as f64 + 0.25) / 4.0 * TAU;
                let (x, y) = (a.cos(), a.sin());
                curve(&mut p, &[[x * 0.012, y * 0.012, top], [x * 0.06, y * 0.06, c[2] - 0.05], [x * 0.068, y * 0.068, c[2] + 0.01], [x * 0.03, y * 0.03, c[2] + 0.05]], 0.011, 0.004, fm);
                p.push(limb([x * 0.03, y * 0.03, c[2] + 0.05], [x * 0.012, y * 0.012, c[2] + 0.07], 0.005, 0.002, focus_accent));
            }
            p.push(limb([0.0, 0.0, top - 0.04], [0.0, 0.0, top + 0.02], thick * 1.4, thick * 1.2, fm));
        }
        // A ring floating level over the crystal, held up on three thin rods
        "halo" => {
            ring(&mut p, [c[0], c[1], c[2] + 0.1], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0.075, 0.008, focus_accent);
            for k in 0..3 {
                let a = k as f64 / 3.0 * TAU;
                curve(&mut p, &[[0.0, 0.0, top], [a.cos() * 0.05, a.sin() * 0.05, c[2] - 0.02], [a.cos() * 0.074, a.sin() * 0.074, c[2] + 0.1]], 0.008, 0.005, fm);
            }
            p.push(limb([0.0, 0.0, top - 0.02], [0.0, 0.0, top + 0.03], thick * 1.3, thick, fm));
        }
        // The shaft splitting into branches round the crystal, twigs off them
        "branches" => {
            for k in 0..5 {
                let a = k as f64 / 5.0 * TAU + 0.3;
                let (x, y) = (a.cos(), a.sin());
                let tip = [x * 0.1, y * 0.1, c[2] + 0.06 + 0.04 * (k % 2) as f64];
                curve(&mut p, &[[0.0, 0.0, top - 0.03], [x * 0.05, y * 0.05, top + 0.05], tip], thick * 0.6, 0.004, fm);
                p.push(limb([x * 0.07, y * 0.07, top + 0.1], [x * 0.12, y * 0.12, top + 0.09], 0.005, 0.002, fm));
                if focus_accent == LEAVES || focus_accent == BLOSSOM {
                    p.push(ellipsoid(tip, [0.016, 0.016, 0.009], focus_accent));
                }
            }
        }
        // A serpent coiled up the top of the shaft, its head reared over the crystal
        "serpent" => {
            let points: Vec<[f64; 3]> = (0..=40)
                .map(|k| {
                    let t = k as f64 / 40.0;
                    let a = t * 2.5 * TAU;
                    let r = thick * 1.3 + 0.03 * t;
                    [a.cos() * r, a.sin() * r, top - 0.25 + t * 0.35]
                })
                .collect();
            curve(&mut p, &points, 0.012, 0.011, fm);
            let neck = points[40];
            let head = [c[0] + 0.02, c[1], c[2] + 0.085];
            curve(&mut p, &[neck, [neck[0] * 0.5, neck[1] * 0.5, c[2] + 0.02], [c[0] - 0.03, c[1], c[2] + 0.09], head], 0.012, 0.01, fm);
            p.push(ellipsoid([head[0] + 0.02, head[1], head[2]], [0.028, 0.018, 0.014], fm));
            for s in [-1.0, 1.0] {
                p.push(ball([head[0] + 0.03, head[1] + s * 0.012, head[2] + 0.008], 0.005, focus_accent));
            }
            p.push(limb([0.0, 0.0, top - 0.02], [0.0, 0.0, top + 0.03], thick * 1.2, thick, fm));
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
        // A spine of bones one on another, each knuckled at its ends
        "vertebrae" => {
            let every = 0.085;
            for k in 0..(top / every) as usize {
                let z = k as f64 * every;
                p.push(ellipsoid([0.0, 0.0, z + every * 0.5], [thick * 1.35, thick * 1.1, every * 0.32], wood));
                p.push(limb([0.0, -thick * 0.8, z + every * 0.5], [0.0, -thick * 1.9, z + every * 0.35], thick * 0.35, thick * 0.2, accent));
            }
            p.push(limb([0.0, 0.0, 0.0], [0.0, 0.0, top], thick * 0.7, thick * 0.8, accent));
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
/// And of bolt head: a square point, a broad barbed one, a crescent, a blunt knob, and a harpoon's
/// great barbed point with an eye for its line
pub const QUARREL_HEADS: usize = 5;
pub const QUARREL_HARPOON: usize = 4;
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
/// Bombs thrown, their vents glowing with each element, or with fire without one
pub const SLOT_BOMB: usize = SLOT_SMOKE + 1;
/// Great billows of cloud, well over a meter across, each element's color: what a censer leaves
/// hanging, a storm's thunderheads, a mire's haze
pub const SLOT_CLOUD: usize = SLOT_BOMB + 1;
/// Pools a meter round lying flat on the ground round 0, of what each element leaves, see pool():
/// churned earth without one, smouldering embers, a sheet of ice, bubbling venom, ground scorched
/// by lightning, a sigil of light, a swirl of darkness, blood, runes. Pools are drawn with these,
/// never painted on.
pub const SLOT_PUDDLE: usize = SLOT_CLOUD + 1;
/// The same pools dying down: embers burnt out to ash, ice melting apart, venom drying, the
/// lightning's forks gone dark, the sigil and runes dim, the swirl drawn in, blood crusting over,
/// the earth settling
pub const SLOT_POOL_SPENT: usize = SLOT_PUDDLE + 1;
/// After the sets: the right arm, the left arm, a boulder, then spikes of ice bursting up
pub const ARMS: usize = (SLOT_POOL_SPENT + 1) * ELEMENT_LOOKS;
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
/// A net of rope a meter across, lead weights round its edge, lying flat round 0, sagging down
pub const NET: usize = SHELL + 1;
/// A rune a meter across glowing on the ground, flat round 0
pub const RUNE: usize = NET + 1;
/// A whirlwind of dust and leaves twisting up three meters, flaring out, standing on 0
pub const TWISTER: usize = RUNE + 1;
/// A ring of earth and rubble heaved up a meter round, flat round 0: a shockwave's front
pub const WAVE: usize = TWISTER + 1;
/// A pillar of rough stone two and a half meters tall, standing on 0
pub const STONE_PILLAR: usize = WAVE + 1;
/// Then the stretches of lash a whip's lash is drawn with, see LashLook: a model as thick as each
/// of LASH_TAPERS for each of the LASH_SETS, then each of the CRACKERS for its tip
pub const LASH_SEGMENTS: usize = STONE_PILLAR + 1;
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

    // A crescent's two horns reaching forward to cut, or a blunt knob of iron to knock down
    // rather than cut, instead of a point
    if head == 3 || head == 4 {
        let s = fletch.head;
        if head == 3 {
            for side in [-1.0, 1.0] {
                p.push(limb([start + 0.012 * s, side * 0.003, 0.0], [start + 0.055 * s, side * 0.026 * s.sqrt(), 0.0], 0.0042, 0.0016, edge));
                p.push(limb([start + 0.004, 0.0, 0.0], [start + 0.014 * s, side * 0.006, 0.0], 0.0045, 0.004, body));
            }
        } else {
            p.push(ball([start + 0.02 * s, 0.0, 0.0], 0.016 * s.sqrt(), edge));
            p.push(limb([start, 0.0, 0.0], [start + 0.018 * s, 0.0, 0.0], 0.007, 0.011 * s.sqrt(), body));
        }
        let ((from, to), (near, far)) = (fletch.vanes, fletch.reach);
        for a in [0.0, TAU / 3.0, 2.0 * TAU / 3.0] {
            let (c, s) = (f64::cos(a), f64::sin(a));
            p.push(limb([from, c * near, s * near], [to, c * far, s * far], 0.006, 0.0035, feathers));
        }
        p.push(limb([0.0, 0.0, 0.0], [0.012, 0.0, 0.0], fletch.shaft + 0.002, fletch.shaft + 0.002, if element == Some(Holy) { GOLD } else { BARK_DARK }));
        p.push(limb([0.0, 0.0, 0.0], [start, 0.0, 0.0], fletch.shaft, fletch.shaft, shaft));
        return p;
    }
    // A harpoon's line is tied through an eye near its back
    if head == 5 {
        ring(&mut p, [0.05, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.012, 0.0035, DARK_STEEL);
    }
    // The head, row by row from its base to its point, the body inside its edges
    let (length, width) = match head {
        0 => (0.085, 0.022),
        1 => (0.11, 0.0075),
        5 => (0.13, 0.019),
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
    if head == 2 || head == 5 {
        // Barbs curving back from the base, a harpoon's two pairs of them, big
        let big = if head == 5 { 1.6 } else { 1.0 };
        for s in [-1.0, 1.0] {
            p.push(limb([start + 0.012, s * width * 0.9, 0.0], [start - 0.022 * big, s * width * 1.5 * big, 0.0], 0.0035 * big, 0.0015, body));
            if head == 5 {
                p.push(limb([start + 0.012, 0.0, s * width * 0.9], [start - 0.03, 0.0, s * width * 2.0], 0.005, 0.0015, body));
            }
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

/// A crossbow's bolt along +x with its nock at 0: with bolt head `head`, a square point, a broad
/// barbed one, a crescent, a blunt knob or a harpoon's, see QUARREL_HEADS, and its element showing
/// as an arrow's does
fn quarrel(element: Option<Element>, head: usize, res: f64) -> Vec<Part> {
    arrow(element, [1, 2, 3, 4, 5][head.min(QUARREL_HEADS - 1)], res, &QUARREL_FLETCH)
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

/// A bomb in flight, its middle at 0: an iron ball, vents round it glowing with `element`, or
/// with fire, a band round it, and its fuse sputtering out of the top
fn flying_bomb(element: Option<Element>) -> Vec<Part> {
    let glow = element.map_or(GLOW_FIRE, Element::glow);
    let r = 0.06;
    let mut p = vec![ball([0.012, 0.0, r + 0.05], 0.012, GLOW_FIRE), limb([0.0, 0.0, r], [0.01, 0.0, r + 0.045], 0.006, 0.005, LINEN)];
    for k in 0..6 {
        let a = k as f64 / 6.0 * TAU;
        p.push(ball([a.cos() * r * 0.92, a.sin() * r * 0.92, r * 0.3 * if k % 2 == 0 { 1.0 } else { -1.0 }], 0.013, glow));
    }
    ring(&mut p, [0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], r, 0.008, BRONZE);
    p.push(ball([0.0, 0.0, 0.0], r, DARK_STEEL));
    p
}

/// A lumpy disc about `radius` round lying flat round 0, `thick` either side of it, of
/// `material`: lobes round a middle, so its edge is ragged
fn bed(p: &mut Vec<Part>, radius: f64, thick: f64, material: u8, seed: &mut u32) {
    for k in 0..7 {
        let a = k as f64 / 7.0 * TAU + random(seed) * 0.5;
        let r = radius * (0.5 + 0.15 * random(seed));
        let s = radius * (0.36 + 0.15 * random(seed));
        p.push(ellipsoid([a.cos() * r, a.sin() * r, 0.0], [s, s, thick], material));
    }
    p.push(ellipsoid([0.0, 0.0, 0.0], [radius * 0.63, radius * 0.63, thick], material));
}

/// A jagged line over the ground from `from` to `to`, `z` up, wandering up to `wander` either side
/// as it goes, `r` thick and thinning: a crack, or a fork of lightning burnt in. Gives back the
/// points it bends at.
#[allow(clippy::too_many_arguments)]
fn jagged(p: &mut Vec<Part>, from: [f64; 2], to: [f64; 2], z: f64, wander: f64, r: f64, material: u8, seed: &mut u32) -> Vec<[f64; 3]> {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let length = (dx * dx + dy * dy).sqrt().max(1e-6);
    let (nx, ny) = (-dy / length, dx / length);
    let steps = 5;
    let points: Vec<[f64; 3]> = (0..=steps)
        .map(|k| {
            let t = k as f64 / steps as f64;
            let off = if k == 0 || k == steps { 0.0 } else { (random(seed) * 2.0 - 1.0) * wander };
            [from[0] + dx * t + nx * off, from[1] + dy * t + ny * off, z]
        })
        .collect();
    curve(p, &points, r, r * 0.6, material);
    points
}

/// A pool a meter round lying flat round 0, of what `element` leaves on the ground, fresh or with
/// `spent` dying down, laid out the same either way so one gives way to the other. Without an
/// element, churned earth, cracked, clods and stones thrown up over it, settling flatter as it's
/// spent. Fire leaves smouldering embers, coals and cracks of fire through charred ground, burning
/// out to grey ash; frost a sheet of ice, spikes of it standing up and frost feathering over it,
/// melting into plates in slush; venom bubbling ooze, drying to a crust; lightning its forks burnt
/// into scorched ground, going dark; holy light a sigil, a star in two rings, dimming; the void a
/// swirl of darkness winding in to a black heart; blood a dark pool, gleaming, crusting over; and
/// arcane magic a ring of runes round a crystal.
fn pool(element: Option<Element>, spent: bool, key: u32) -> Vec<Part> {
    use Element::*;
    let mut seed = key;
    let s = &mut seed;
    let mut p = Vec::new();
    let flat = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    let round = |s: &mut u32, most: f64| {
        let a = random(s) * TAU;
        let r = random(s).sqrt() * most;
        (a.cos() * r, a.sin() * r)
    };
    match element {
        None => {
            // Clods of earth and stones thrown up over it, fewer and smaller once it's settled
            for k in 0..12 {
                let (x, y) = round(s, 0.85);
                let r = (0.04 + 0.06 * random(s)) * if spent { 0.6 } else { 1.0 };
                if !spent || k % 2 == 0 {
                    p.push(ball([x, y, 0.02], r, [DIRT, GRAVEL, ROCK, BARK_DARK][k % 4]));
                }
            }
            for k in 0..6 {
                let a = k as f64 / 6.0 * TAU + random(s) * 0.7;
                let (r0, r1) = (0.1 + 0.1 * random(s), 0.7 + 0.25 * random(s));
                jagged(&mut p, [a.cos() * r0, a.sin() * r0], [a.cos() * r1, a.sin() * r1], 0.012, 0.08, 0.02, BARK_DARK, s);
            }
            // Turned over, darker in patches
            for _ in 0..5 {
                let (x, y) = round(s, 0.6);
                p.push(ellipsoid([x, y, 0.005], [0.2, 0.14, 0.032], FUR_BROWN));
            }
            bed(&mut p, 0.95, 0.03, DIRT, s);
        }
        Some(Fire) => {
            let (coal, crack) = if spent { (BARK_DARK, MORTAR) } else { (GLOW_FIRE, GLOW_FIRE) };
            for k in 0..16 {
                let (x, y) = round(s, 0.8);
                let r = 0.035 + 0.04 * random(s);
                // A few still glowing as it burns out
                let m = if spent && k % 4 == 0 { GLOW_FIRE } else if k % 3 == 0 && !spent { FLAME } else { coal };
                p.push(ball([x, y, 0.025], if spent { r * 0.7 } else { r }, m));
            }
            for k in 0..6 {
                let a = k as f64 / 6.0 * TAU + random(s) * 0.6;
                let (r0, r1) = (0.08, 0.6 + 0.3 * random(s));
                jagged(&mut p, [a.cos() * r0, a.sin() * r0], [a.cos() * r1, a.sin() * r1], 0.012, 0.07, 0.018, crack, s);
            }
            // Ash lying flat at its edge, all over it once it's burnt out
            for k in 0..9 {
                let a = k as f64 / 9.0 * TAU + random(s) * 0.5;
                let r = if spent { 0.2 + 0.6 * random(s) } else { 0.72 + 0.2 * random(s) };
                p.push(ellipsoid([a.cos() * r, a.sin() * r, 0.0], [0.2, 0.13, 0.031], if spent && k % 2 == 0 { FUR_GRAY } else { MORTAR }));
            }
            bed(&mut p, 0.95, 0.03, BARK_DARK, s);
        }
        Some(Frost) => {
            if !spent {
                // Spikes of ice standing up out of it, leaning out
                for k in 0..7 {
                    let a = k as f64 / 7.0 * TAU + random(s) * 0.7;
                    let r = 0.25 + 0.55 * random(s);
                    let (h, lean) = (0.12 + 0.18 * random(s), 0.04 + 0.06 * random(s));
                    let base = [a.cos() * r, a.sin() * r, 0.0];
                    p.push(ball(base, 0.02, GLOW_FROST));
                    p.push(limb(base, [base[0] + a.cos() * lean, base[1] + a.sin() * lean, h], 0.045 + 0.02 * random(s), 0.004, ICE));
                }
                // Frost feathering out over it
                for k in 0..10 {
                    let a = k as f64 / 10.0 * TAU + random(s) * 0.5;
                    let (r0, r1) = (0.2 + 0.2 * random(s), 0.75 + 0.15 * random(s));
                    jagged(&mut p, [a.cos() * r0, a.sin() * r0], [a.cos() * r1, a.sin() * r1], 0.03, 0.04, 0.012, FOAM, s);
                }
                ring(&mut p, [0.0, 0.0, 0.0], flat.0, flat.1, 0.9, 0.05, SNOW);
                bed(&mut p, 0.95, 0.03, ICE, s);
            } else {
                // Plates of it drifting apart in slush as it melts
                for k in 0..8 {
                    let a = k as f64 / 8.0 * TAU + random(s) * 0.4;
                    let r = 0.35 + 0.3 * random(s);
                    p.push(ellipsoid([a.cos() * r, a.sin() * r, 0.006], [0.2, 0.14, 0.026], ICE));
                }
                p.push(ellipsoid([0.0, 0.0, 0.006], [0.2, 0.2, 0.026], ICE));
                bed(&mut p, 0.8, 0.018, FOAM, s);
            }
        }
        Some(Poison) => {
            let (ooze, rim) = if spent { (DRY_GRASS, MOSS) } else { (GLOW_POISON, MOSS) };
            // Bubbles swelling up out of it, gone once it's dried
            for k in 0..9 {
                let (x, y) = round(s, 0.7);
                let r = 0.05 + 0.07 * random(s);
                if !spent {
                    p.push(ball([x, y, 0.01], r, if k % 3 == 0 { LEAVES_LIME } else { GLOW_POISON }));
                } else if k % 3 == 0 {
                    p.push(ball([x, y, 0.0], r * 0.4, GLOW_POISON));
                }
            }
            for _ in 0..4 {
                let (x, y) = round(s, 0.5);
                p.push(ellipsoid([x, y, 0.006], [0.22, 0.16, 0.032], if spent { CLAY } else { LEAVES_LIME }));
            }
            p.push(ellipsoid([0.0, 0.0, 0.004], [0.66, 0.62, 0.034], ooze));
            bed(&mut p, 0.95, 0.03, rim, s);
        }
        Some(Storm) => {
            let fork = if spent { OBSIDIAN } else { GLOW_STORM };
            // Forks burnt out over it from where it struck, each branching
            for k in 0..5 {
                let a = k as f64 / 5.0 * TAU + random(s) * 0.5;
                let points = jagged(&mut p, [0.0, 0.0], [a.cos() * 0.9, a.sin() * 0.9], 0.015, 0.1, 0.02, fork, s);
                let mid = points[2 + (random(s) * 2.0) as usize];
                let b = a + (random(s) - 0.5) * 1.6;
                jagged(&mut p, [mid[0], mid[1]], [mid[0] + b.cos() * 0.35, mid[1] + b.sin() * 0.35], 0.015, 0.05, 0.013, fork, s);
            }
            p.push(ellipsoid([0.0, 0.0, 0.01], [0.1, 0.1, 0.035], if spent { GRAVEL } else { GLOW_STORM }));
            for _ in 0..5 {
                let (x, y) = round(s, 0.8);
                p.push(ball([x, y, 0.01], 0.04, GRAVEL));
            }
            bed(&mut p, 0.9, 0.03, BARK_DARK, s);
        }
        Some(Holy) => {
            let (light, gold) = if spent { (GOLD, BRONZE) } else { (GLOW_HOLY, GOLD) };
            ring(&mut p, [0.0, 0.0, 0.02], flat.0, flat.1, 0.92, 0.03, gold);
            ring(&mut p, [0.0, 0.0, 0.02], flat.0, flat.1, 0.66, 0.024, light);
            // A star of two triangles within the rings
            for tri in 0..2 {
                for k in 0..3 {
                    let a0 = tri as f64 * PI / 3.0 + k as f64 * TAU / 3.0 + PI / 2.0;
                    let a1 = a0 + TAU / 3.0;
                    p.push(limb([a0.cos() * 0.62, a0.sin() * 0.62, 0.02], [a1.cos() * 0.62, a1.sin() * 0.62, 0.02], 0.018, 0.018, light));
                }
            }
            // Marks round between the rings, as on a dial
            for k in 0..12 {
                let a = k as f64 / 12.0 * TAU;
                if k % 3 == 0 {
                    p.push(limb([a.cos() * 0.73, a.sin() * 0.73, 0.02], [a.cos() * 0.85, a.sin() * 0.85, 0.02], 0.02, 0.02, gold));
                } else {
                    p.push(ball([a.cos() * 0.79, a.sin() * 0.79, 0.02], 0.025, light));
                }
            }
            p.push(ellipsoid([0.0, 0.0, 0.01], [0.1, 0.1, 0.03], light));
            // Burnt into the ground, so the light shows against it
            bed(&mut p, 0.95, 0.016, BRONZE, s);
        }
        Some(Void) => {
            // Arms of it winding in to the middle, drawn in tighter once it's spent
            let reach = if spent { 0.55 } else { 0.95 };
            for arm in 0..3 {
                let a0 = arm as f64 / 3.0 * TAU + random(s) * 0.3;
                let points: Vec<[f64; 3]> = (0..=14)
                    .map(|j| {
                        let t = j as f64 / 14.0;
                        let (r, a) = (reach * (1.0 - t) + 0.06, a0 + t * 1.3 * TAU);
                        [a.cos() * r, a.sin() * r, 0.02]
                    })
                    .collect();
                curve(&mut p, &points, 0.05 * reach, 0.012, if spent { FUR_DARK } else { GLOW_VOID });
            }
            let heart = 0.16 * reach.sqrt();
            p.push(ellipsoid([0.0, 0.0, 0.0], [heart, heart, 0.034], EYE));
            bed(&mut p, reach, 0.03, OBSIDIAN, s);
        }
        Some(Blood) => {
            if !spent {
                // Gleaming
                for _ in 0..5 {
                    let (x, y) = round(s, 0.6);
                    p.push(ellipsoid([x, y, 0.03], [0.08, 0.04, 0.02], GLOW_BLOOD));
                }
            }
            // Splashed round it
            for _ in 0..9 {
                let a = random(s) * TAU;
                let r = 0.95 + 0.2 * random(s);
                p.push(ball([a.cos() * r, a.sin() * r, 0.0], 0.03 + 0.03 * random(s), CLOTH));
            }
            // Darkest where it's deepest
            p.push(ellipsoid([0.05, 0.03, 0.005], [0.55, 0.5, 0.033], if spent { FUR_DARK } else { LEAVES_CRIMSON }));
            bed(&mut p, 0.95, 0.03, CLOTH, s);
        }
        Some(Arcane) => {
            let (glow, crystal) = if spent { (FLOWER_PURPLE, STEEL) } else { (GLOW_ARCANE, CRYSTAL) };
            ring(&mut p, [0.0, 0.0, 0.02], flat.0, flat.1, 0.9, 0.025, crystal);
            ring(&mut p, [0.0, 0.0, 0.02], flat.0, flat.1, 0.58, 0.02, glow);
            // Runes between the rings, each its own: a bar out from the middle, strokes off it
            for k in 0..8 {
                let a = k as f64 / 8.0 * TAU + 0.2;
                let (out, across) = ([a.cos(), a.sin()], [-a.sin(), a.cos()]);
                let at = |r: f64, side: f64| [out[0] * r + across[0] * side, out[1] * r + across[1] * side, 0.02];
                p.push(limb(at(0.66, 0.0), at(0.82, 0.0), 0.016, 0.016, glow));
                let strokes = hash(k as u32 * 7919 + key);
                for j in 0..3 {
                    if strokes >> j & 1 == 1 {
                        let r = 0.68 + 0.06 * j as f64;
                        let side = if strokes >> (j + 4) & 1 == 1 { 0.06 } else { -0.06 };
                        p.push(limb(at(r, 0.0), at(r + 0.04, side), 0.014, 0.014, glow));
                    }
                }
            }
            p.push(ellipsoid([0.0, 0.0, 0.02], [0.07, 0.07, 0.045], crystal));
            bed(&mut p, 0.95, 0.02, DARK_STEEL, s);
        }
    }
    p
}

/// A whirlwind: strands of dust twisting up three meters, narrow at its foot and flaring out
/// above, leaves caught up in it, standing on 0
fn twister() -> Vec<Part> {
    let mut p = Vec::new();
    let mut seed = 0x7a15;
    for _ in 0..12 {
        let t = 0.2 + 0.75 * random(&mut seed);
        let a = random(&mut seed) * TAU;
        let r = 0.2 + 1.05 * t * t;
        p.push(ball([a.cos() * r, a.sin() * r, t * 3.0], 0.09, if random(&mut seed) < 0.5 { LEAVES } else { BARK_DARK }));
    }
    for strand in 0..6 {
        let a0 = strand as f64 / 6.0 * TAU;
        let points: Vec<[f64; 3]> = (0..=16)
            .map(|k| {
                let t = k as f64 / 16.0;
                let a = a0 + t * 1.4 * TAU;
                let r = 0.18 + 1.05 * t * t;
                [a.cos() * r, a.sin() * r, t * 3.0]
            })
            .collect();
        curve(&mut p, &points, 0.07, 0.17, [FUR_WHITE, FUR_GRAY, CLAY][strand % 3]);
    }
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
        models.push(crate::effects::bolt(element));
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
    // Fireballs, a meter across: churning balls with bites out of them, see effects::orb()
    for &element in &looks {
        models.push(crate::effects::orb(element));
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
    for &element in &looks {
        models.push(voxelize(&flying_bomb(element), BOMB_RES));
    }
    // Billows: a puff of smoke, much bigger, lumpier
    for (k, &element) in looks.iter().enumerate() {
        let p: Vec<Part> = smoke(element, 0xc10d + k as u32).iter().map(|p| p.scaled(7.5)).collect();
        models.push(voxelize(&p, CLOUD_RES));
    }
    // Pools, fresh then dying down, each laid out alike, see pool()
    for spent in [false, true] {
        for (k, &element) in looks.iter().enumerate() {
            models.push(voxelize(&pool(element, spent, 0x9dd1 + k as u32), PUDDLE_RES));
        }
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
    // A net: rope strung across and across, sagging in the middle, lead weights round its edge
    let sag = |x: f64, y: f64| -0.12 * (1.0 - (x * x + y * y) / 0.25).max(0.0);
    let mut p = Vec::new();
    for k in 0..10 {
        let a = k as f64 / 10.0 * TAU;
        p.push(ball([a.cos() * 0.5, a.sin() * 0.5, 0.0], 0.035, DARK_STEEL));
    }
    for k in -4..=4 {
        let c = k as f64 * 0.11;
        let reach = (0.25 - c * c).max(0.0).sqrt();
        for (a, b) in [([c, -reach], [c, reach]), ([-reach, c], [reach, c])] {
            let points: Vec<[f64; 3]> = (0..=6).map(|j| {
                let t = j as f64 / 6.0;
                let (x, y) = (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t);
                [x, y, sag(x, y)]
            }).collect();
            curve(&mut p, &points, 0.014, 0.014, LINEN);
        }
    }
    models.push(voxelize(&p, RING_RES));
    // A rune: a ring, a star inside it, and marks round it, all glowing
    let mut p: Vec<Part> = (0..64).map(|k| k as f64 / 64.0 * TAU).map(|a| ball([a.cos() * 0.5, a.sin() * 0.5, 0.0], 0.03, GLOW_ARCANE)).collect();
    for k in 0..5 {
        let (a, b) = (k as f64 / 5.0 * TAU, (k as f64 + 2.0) / 5.0 * TAU);
        p.push(limb([a.cos() * 0.42, a.sin() * 0.42, 0.0], [b.cos() * 0.42, b.sin() * 0.42, 0.0], 0.022, 0.022, GLOW_ARCANE));
        let c = (k as f64 + 0.5) / 5.0 * TAU;
        p.push(ball([c.cos() * 0.62, c.sin() * 0.62, 0.0], 0.04, GLOW_HOLY));
    }
    models.push(voxelize(&p, RING_RES));
    models.push(voxelize(&twister(), MODEL_RES));
    // A shockwave's front: lumps of earth and stone heaved up in a ring, dust over them
    let mut seed = 0x3a7e;
    let mut p = Vec::new();
    for k in 0..90 {
        let a = (k as f64 + random(&mut seed) * 0.6) / 90.0 * TAU;
        let r = 1.0 + (random(&mut seed) - 0.5) * 0.08;
        let dust = k % 5 == 0;
        let size = if dust { 0.05 } else { 0.028 + 0.025 * random(&mut seed) };
        let z = if dust { 0.06 } else { size * 0.5 };
        p.push(ball([a.cos() * r, a.sin() * r, z], size, if dust { FUR_WHITE } else { [DIRT, GRAVEL, ROCK, DIRT][k % 4] }));
    }
    models.push(voxelize(&p, RING_RES));
    // A pillar of stone burst up out of the ground, rough and leaning a little, to a jagged top,
    // earth and stones falling off it
    let mut seed = 0x9111;
    let mut p = Vec::new();
    for k in 0..9 {
        let z = 0.2 + k as f64 * 0.24;
        let lean = z * 0.06;
        let r = 0.42 - 0.025 * k as f64;
        p.push(ball([lean + (random(&mut seed) - 0.5) * 0.1, (random(&mut seed) - 0.5) * 0.1, z], r * (0.85 + 0.3 * random(&mut seed)), if k % 3 == 1 { GRAVEL } else { ROCK }));
    }
    for k in 0..5 {
        let a = k as f64 / 5.0 * TAU;
        p.push(limb([a.cos() * 0.15 + 0.13, a.sin() * 0.15, 2.0], [a.cos() * 0.1 + 0.16, a.sin() * 0.1, 2.45 + 0.1 * random(&mut seed)], 0.14, 0.02, STONE));
    }
    for k in 0..8 {
        let a = k as f64 / 8.0 * TAU;
        p.push(ball([a.cos() * 0.55, a.sin() * 0.55, 0.05], 0.12 + 0.06 * random(&mut seed), if k % 2 == 0 { DIRT } else { GRAVEL }));
    }
    p.push(limb([0.0, 0.0, 0.0], [0.13, 0.0, 2.1], 0.4, 0.3, STONE));
    models.push(voxelize(&p, MODEL_RES));
    // A whip's lash: a stretch of it in each set at each thickness, then the crackers
    for set in 0..LASH_SETS {
        for radius in LASH_TAPERS {
            models.push(voxelize(&lash_segment(set, radius), LASH_RES));
        }
    }
    for kind in 0..CRACKERS {
        models.push(voxelize(&lash_cracker(kind), LASH_RES));
    }
    // A lash wound round and round half a meter across, three turns climbing three quarters of a
    // meter, banded where its braid crosses, for a coil bind, drawn as big as what it holds
    let mut p = Vec::new();
    let turns = 3.0;
    let points: Vec<[f64; 3]> = (0..=60)
        .map(|k| {
            let u = k as f64 / 60.0;
            let a = u * turns * TAU;
            [a.cos() * 0.5, a.sin() * 0.5, 0.05 + 0.7 * u]
        })
        .collect();
    for (k, pair) in points.windows(2).enumerate() {
        let material = if k % 5 == 0 { FUR_TAN } else { LEATHER };
        p.push(limb(pair[0], pair[1], 0.04, 0.04, material));
    }
    models.push(voxelize(&p, RING_RES));
    // A serpent's head along x, half a meter long, glowing with venom so it shows slithering over
    // the ground: bright eyes, a forked tongue flicking out, a dark band over its crown, and its
    // neck trailing off behind
    let mut p = vec![ball([0.07, 0.04, 0.035], 0.022, GLOW_HOLY), ball([0.07, -0.04, 0.035], 0.022, GLOW_HOLY)];
    p.push(limb([0.13, 0.0, -0.01], [0.24, 0.016, -0.01], 0.008, 0.004, GLOW_BLOOD));
    p.push(limb([0.13, 0.0, -0.01], [0.24, -0.016, -0.01], 0.008, 0.004, GLOW_BLOOD));
    p.push(ellipsoid([0.0, 0.0, 0.035], [0.1, 0.05, 0.02], BARK_DARK));
    p.push(ellipsoid([0.0, 0.0, 0.0], [0.14, 0.065, 0.05], GLOW_POISON));
    p.push(limb([-0.08, 0.0, 0.0], [-0.34, 0.0, -0.03], 0.05, 0.03, MOSS));
    models.push(voxelize(&p, MODEL_RES));
    // Explosions in stages, their rings, ice, pillars of light, the blood moon and sparks as
    // streaks, see effects::BLASTS and the rest
    models.extend(crate::effects::models());
    models
}

/// Where the coil a coil bind winds round what it holds is among effect_models(), and a serpent
/// lash's serpent's head
pub const COIL: usize = LASH_CRACKERS + CRACKERS;
pub const SERPENT: usize = COIL + 1;

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
/// Bombs as fine as shells, and billows coarse, as they're big
pub const BOMB_RES: f64 = 64.0;
pub const CLOUD_RES: f64 = 20.0;
pub const PUDDLE_RES: f64 = 24.0;

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

    /// No weapon's name says a word twice, as Crescent Crescentaxe did: rolled for every family,
    /// with every luck, named as Weapon::assemble() names it
    #[test]
    fn names_never_say_a_word_twice() {
        let library = Library::load();
        assert!(name_clash("Crescent", "Crescentaxe") && name_clash("Ashen", "Ashblade") && name_clash("of Thorns", "Thornblade"));
        assert!(!name_clash("Ashen", "Moonsabre") && !name_clash("of the Deep", "Long Glaive"));
        let mut repeats = Vec::new();
        let mut named = 0;
        for family in 0..library.families.len() {
            for luck in [0.0, 1.0, 3.0, 8.0, 30.0] {
                for k in 0..800u32 {
                    let seed = hash(k ^ (family as u32) << 16 ^ (luck as u32) << 24);
                    let (family, parts) = library.roll(seed, Some(family), luck);
                    let components: Vec<&Component> = parts.iter().map(|&p| &library.components[p]).collect();
                    let size = roll_size(seed);
                    let (profile, _, _) = Weapon::work_out(&library, family, &components, size, seed);
                    let name = weapon_name(&library, family, &components, &profile.synergies, seed, size);
                    named += 1;
                    let words = name_words(&name);
                    let twice = (0..words.len()).any(|i| (i + 1..words.len()).any(|j| name_clash(&words[i], &words[j])));
                    if twice && !repeats.contains(&name) {
                        repeats.push(name);
                    }
                }
            }
        }
        println!("{named} names, {} saying a word twice", repeats.len());
        assert!(repeats.is_empty(), "{} names say a word twice:\n{}", repeats.len(), repeats.iter().take(40).cloned().collect::<Vec<_>>().join("\n"));
    }

    /// Luck so great the rarest parts roll more often than common ones, see Library::odds(): for
    /// seeing what can turn up at all, not how often
    const LUCKIEST: f64 = 100.0;

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
        let odds = library.odds(1.0);
        for _ in 0..rolls {
            counts[Library::roll_rarity(&odds, &mut rng)] += 1;
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
        assert_eq!(effect_models().len(), crate::effects::EFFECT_MODELS);
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
        for id in ["crossbow", "arbalest", "repeater", "handcrossbow", "twinprod", "siegecrossbow", "harpoon", "boltthrower", "pistol", "musket", "blunderbuss", "handcannon"] {
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
        // It bursts in the air into a spray of shot, and bursting bolts where they strike into a
        // wider spray of shards, each doing a share of the damage
        let shot = library.ammo[bursting].burst.expect("bursting shot bursts");
        let bolts = library.ammo.iter().find(|a| a.id == "bursting_bolts").and_then(|a| a.burst).expect("bursting bolts burst");
        assert!(shot.airburst && !bolts.airburst);
        for burst in [shot, bolts] {
            assert!(burst.fragments >= 4 && burst.fragments <= 12 && burst.spread > 0.0 && burst.power > 0.1 && burst.power < 0.5);
        }
        assert!(bolts.spread > shot.spread);
        assert!(library.ammo.iter().filter(|a| a.burst.is_some()).count() == 2, "only bursting ammo bursts");
    }

    #[test]
    fn new_magic_weapons_and_specials_turn_up() {
        let library = Library::parse(BUILT_IN).unwrap();
        // Each kind of magic weapon has its own way of casting, and a model to hold
        for (id, style, frames) in [("orb", Style::Channel, 1), ("tome", Style::Summon, 2), ("totem", Style::Hex, 1), ("bell", Style::Toll, 1), ("battlestaff", Style::Glaive, 1)] {
            let family = library.family(id).expect("there's such a family");
            for seed in 0..30 {
                let weapon = library.generate(seed, Some(family), 4.0);
                assert_eq!(weapon.profile.style, style);
                assert_eq!(weapon.look.frames.len(), frames, "{id}");
                assert!(weapon.look.frames.iter().all(|(m, _)| m.voxels.iter().any(|&v| v != AIR)), "{id} has something to see");
            }
        }
        // Every special attack is on some part, so they turn up in loot and shops
        let on_parts: BTreeSet<String> = library.components.iter().filter_map(|c| c.special).map(|s| s.name().to_owned()).collect();
        assert!(on_parts.len() >= 75, "only {} special attacks are on parts", on_parts.len());
        let mut found = BTreeSet::new();
        for seed in 0..40_000 {
            if let Some(special) = library.special_of(seed, None, 8.0) {
                found.insert(special.name());
            }
        }
        assert_eq!(found.len(), on_parts.len(), "every special attack rolls in lucky loot");
        // Worked out from the parts alone, it's what the weapon really has, whatever its family
        for seed in 0..300 {
            assert_eq!(library.special_of(seed, None, 8.0), library.generate(seed, None, 8.0).profile.special.as_ref().map(|(s, _)| *s));
            let family = Some(seed as usize % library.families.len());
            assert_eq!(library.special_of(seed, family, 8.0), library.generate(seed, family, 8.0).profile.special.as_ref().map(|(s, _)| *s));
        }
    }

    #[test]
    fn new_crossbows_are_each_built_their_own_way() {
        let library = Library::parse(BUILT_IN).unwrap();
        // How each is built, how many bolts it lies ready with, and whether it has a leg to brace on
        for (id, build, bolts, leg) in [("handcrossbow", Build::Hand, 1, false), ("twinprod", Build::Twin, 2, false), ("siegecrossbow", Build::Siege, 1, true), ("harpoon", Build::Harpoon, 1, false), ("boltthrower", Build::Fan, 5, false)] {
            let family = library.family(id).expect("there's such a family");
            let mut sights = BTreeSet::new();
            for seed in 0..60 {
                let weapon = library.generate(seed, Some(family), 3.0);
                let profile = &weapon.profile;
                assert_eq!(profile.build, build, "{id}");
                assert!(build.crossbow() && weapon.shot() == Some(Shot::Quarrel), "{id} shoots bolts");
                let look = &weapon.look;
                assert_eq!(look.rails.len(), bolts, "{id}");
                assert_eq!(look.leg.is_some(), leg, "{id}");
                if let Some((leg, _)) = &look.leg {
                    assert!(leg.frame < look.frames.len());
                }
                // Every frame has something to see, the bolt-thrower's fan spreading out wide
                assert!(look.frames.iter().all(|(m, _)| m.voxels.iter().any(|&v| v != AIR)), "{id}");
                if build == Build::Fan {
                    let yaws: Vec<f64> = look.rails.iter().map(|r| r.1).collect();
                    assert!(yaws.first() < yaws.last(), "a bolt-thrower's grooves fan out");
                }
                // The parts it's made of fit, a sight among them
                let slot = library.families[family].slots.iter().position(|s| s == "sight").expect("a crossbow has a sight");
                sights.insert(weapon.parts[slot]);
            }
            assert!(sights.len() >= 4, "{id} rolls only {} sights", sights.len());
        }
        // A hand crossbow's held by a grip under it, a siege crossbow's bolts are big and a hand
        // crossbow's small, and a harpoon's line reaches out
        let hand = library.generate(3, library.family("handcrossbow"), 1.0);
        assert!(hand.look.grip.z < -0.05);
        let siege = library.generate(3, library.family("siegecrossbow"), 1.0);
        assert!(siege.profile.bolt > 1.4 && hand.profile.bolt < 1.0);
        assert!(siege.profile.has(Effect::Pierce));
        // Plenty of parts to roll for every slot of a crossbow, and plenty of kinds of bolt
        for (slot, most) in [("prod", 18), ("stock", 15), ("lock", 16), ("sight", 12)] {
            let count = library.components.iter().filter(|c| c.slot_type == slot && c.compatible_families.iter().any(|f| f == "harpoon")).count();
            assert!(count >= most, "only {count} crossbow {slot}s");
        }
        assert!(library.ammo.iter().filter(|a| a.fits == "bolt").count() >= 16);
        // The new effects are there to be had
        for id in ["snare", "ricochet"] {
            assert!(library.effects.iter().any(|e| e.id == id && e.effect.is_some()), "{id}");
        }
    }

    #[test]
    fn area_weapons_are_each_built_and_used_their_own_way() {
        let library = Library::parse(BUILT_IN).unwrap();
        for (id, kind, style) in [("flail", Kind::Melee, Style::Whirl), ("maul", Kind::Melee, Style::Quake), ("bombs", Kind::Ranged, Style::Lob), ("rod", Kind::Magic, Style::Arc), ("censer", Kind::Magic, Style::Censer)] {
            let family = library.family(id).expect("there's such a family");
            let mut parts = BTreeSet::new();
            for seed in 0..40 {
                let weapon = library.generate(seed, Some(family), 4.0);
                let profile = &weapon.profile;
                assert_eq!((profile.kind, profile.style), (kind, style), "{id}");
                let look = &weapon.look;
                assert_eq!(look.frames.len(), 1, "{id}");
                assert!(look.frames[0].0.voxels.iter().any(|&v| v != AIR), "{id} has something to see");
                // Where what it does comes from, next to where it's held, as big as its parts make it
                let (grip, tip) = (look.grip / look.scale, look.tip / look.scale);
                match style {
                    // A flail's head swings far out past its haft, a maul's head is broad across
                    Style::Whirl => assert!(tip.z > grip.z + 0.45, "{id} {tip:?}"),
                    Style::Quake => assert!(tip.x > 0.15 && tip.z > grip.z + 0.6, "{id} {tip:?}"),
                    // A bomb's fuse burns over the hand, and bombs take no ammo
                    Style::Lob => assert!(tip.z > grip.z && weapon.shot().is_none(), "{id}"),
                    // A rod's prongs are high over the hand, a censer hangs well under it
                    Style::Arc => assert!(tip.z > grip.z + 0.6, "{id} {tip:?}"),
                    _ => assert!(tip.z < grip.z - 0.3, "{id} {tip:?}"),
                }
                parts.extend(weapon.parts.iter().copied());
            }
            assert!(parts.len() >= 16, "{id} rolls only {} parts", parts.len());
        }
        // Plenty of heads for the flail and the maul, and of everything a bomb's made of
        for (slot, family, most) in [("blade", "flail", 10), ("blade", "maul", 10), ("casing", "bombs", 9), ("fuse", "bombs", 7), ("filling", "bombs", 10)] {
            let count = library.components.iter().filter(|c| c.slot_type == slot && c.compatible_families.iter().any(|f| f == family)).count();
            assert!(count >= most, "only {count} {family} {slot}s");
        }
        // The new effects have code, and come on parts of the old weapons too, and on ammo
        for id in ["cleave", "shockwave", "splash", "miasma", "vortex", "cluster"] {
            let def = library.effects.iter().position(|e| e.id == id && e.effect.is_some()).unwrap_or_else(|| panic!("there's no {id}"));
            let old = library.components.iter().filter(|c| c.effect_defs.contains(&def) && c.compatible_families.iter().any(|f| !["flail", "maul", "bombs", "rod", "censer"].contains(&f.as_str()))).count();
            assert!(old >= 2, "{id} is only on {old} parts of the old weapons");
        }
        for id in ["splash", "vortex", "cluster", "miasma"] {
            assert!(library.ammo.iter().any(|a| a.effects.iter().any(|e| e == id)), "no ammo with {id}");
        }
        // Pools left on the ground were taken out, the user asking for it: no part or ammo leaves
        // them, though what they'd do is kept
        let pool = library.effects.iter().position(|e| e.id == "pool").expect("there's no pool");
        assert!(!library.components.iter().any(|c| c.effect_defs.contains(&pool)), "a part leaves pools");
        assert!(!library.ammo.iter().any(|a| a.effects.iter().any(|e| e == "pool")), "ammo leaves pools");
        // Each new special attack is on a part of one of the weapons it suits
        for (special, families) in [
            (Special::Earthquake, &["maul", "hammer"][..]),
            (Special::Avalanche, &["maul", "hammer"]),
            (Special::Cyclone, &["flail"]),
            (Special::Shatterwave, &["axe", "greatsword"]),
            (Special::Comet, &["staff", "rod"]),
            (Special::Thunderstorm, &["rod", "censer"]),
            (Special::Mire, &["bombs", "censer"]),
            (Special::Sunburst, &["staff", "flail"]),
            (Special::ClusterBomb, &["bombs", "handcannon", "crossbow"]),
            (Special::Eruption, &["bombs", "musket"]),
            (Special::BallLightning, &["rod", "crossbow"]),
            (Special::Swarm, &["bow", "bombs"]),
        ] {
            for family in families {
                assert!(library.components.iter().any(|c| c.special == Some(special) && c.compatible_families.iter().any(|f| f == family)), "{special:?} isn't on any {family} part");
            }
        }
    }

    /// Weapons come in sizes, most as their parts make them, the biggest and smallest rarely, and
    /// a bigger one reaches further, hits harder and is drawn bigger, but swings slower
    #[test]
    fn sizes_roll_in_range_and_change_what_a_weapon_does() {
        let library = Library::parse(BUILT_IN).unwrap();
        let mut counts = [0usize; 5];
        let rolls = 100_000u32;
        for seed in 0..rolls {
            let size = roll_size(hash(seed));
            assert!(size < SIZES.len());
            counts[size] += 1;
        }
        for (size, count) in counts.iter().enumerate() {
            let seen = *count as f64 / rolls as f64;
            assert!((seen - SIZES[size].chance).abs() < 0.01, "size {size} rolled {seen}, should be {}", SIZES[size].chance);
        }
        // Scales stay sane, so nothing's drawn tiny or filling the view
        for s in &SIZES {
            assert!((0.8..=1.2).contains(&s.scale) && (0.8..=1.25).contains(&s.reach) && (0.8..=1.2).contains(&s.speed));
        }
        // The same parts made each size: the bigger reaches further and hits harder, but slower
        let sword = library.family("sword").unwrap();
        let (_, parts) = library.roll(7, Some(sword), 1.0);
        let components: Vec<&Component> = parts.iter().map(|&p| &library.components[p]).collect();
        let made: Vec<Profile> = (0..SIZES.len()).map(|size| Weapon::work_out(&library, sword, &components, size, 7).0).collect();
        for pair in made.windows(2) {
            assert!(pair[1].stat("range") > pair[0].stat("range"));
            assert!(pair[1].stat("physicalDamage") > pair[0].stat("physicalDamage"));
            assert!(pair[1].stat("attackSpeed") < pair[0].stat("attackSpeed"));
            assert!(pair[1].heft > pair[0].heft);
        }
        assert!(made[4].stat("knockback") > made[2].stat("knockback"));
        // Drawn bigger the bigger it is, a long one less so, and named for it where it's not plain
        let mut seen_words = BTreeSet::new();
        for seed in 0..400u32 {
            let weapon = library.generate(seed, None, 2.0);
            let family = &library.families[weapon.family];
            let plain = Weapon::assemble(&library, weapon.family, weapon.parts.clone(), seed);
            assert_eq!(plain.size, weapon.size);
            let scale = weapon.look.scale;
            assert!((0.8..=1.2).contains(&scale), "{} drawn {scale}", weapon.name);
            assert_eq!(scale > 1.0, weapon.size > SIZES.len() / 2, "{}", weapon.name);
            assert!(weapon.look.longest() < 2.6, "{} is {:.2} m long", weapon.name, weapon.look.longest());
            let word = family.size_word(weapon.size);
            if !word.is_empty() && weapon.name.contains(word) {
                seen_words.insert(word.to_owned());
            }
            if weapon.size == SIZES.len() / 2 {
                assert!(word.is_empty());
            }
        }
        assert!(seen_words.len() >= 6, "only {seen_words:?} turned up in names");
    }

    /// Every part there is turns up in loot, the new ones too, rare ones in lucky loot
    #[test]
    fn every_part_turns_up_in_loot() {
        let library = Library::parse(BUILT_IN).unwrap();
        let mut seen = vec![false; library.components.len()];
        for seed in 0..90_000u32 {
            let luck = [1.0, 8.0, LUCKIEST][seed as usize % 3];
            let (_, parts) = library.roll(hash(seed), None, luck);
            for p in parts {
                seen[p] = true;
            }
        }
        // Tools' parts aside, as tools are only made, see src\tools.rs, and those of no weight,
        // which are never rolled but only given, as the Worldpyre Fang
        let weapon_part = |c: &Component| c.compatible_families.iter().any(|f| library.family(f).is_some_and(|i| library.families[i].tool.is_none()));
        let missing: Vec<&str> = library.components.iter().zip(&seen).filter(|(c, s)| !**s && weapon_part(c) && c.weight > 0.0).map(|(c, _)| c.id.as_str()).collect();
        assert!(missing.is_empty(), "never rolled: {missing:?}");
        assert!(library.components.iter().zip(&seen).all(|(c, s)| weapon_part(c) || !*s), "a tool's part rolled as loot");
        // The new shapes and patterns are there to be had
        for shape in ["leaf", "tapered", "serpent", "hooked", "falx", "notched", "forked", "bearded", "spike_axe", "moon_axe", "flanged", "beaked", "morning", "winged", "trident", "fork", "scythe_saw", "scythe_twin", "halberd_bill", "halberd_moon", "flail_skull", "flail_crystal", "flail_triple", "maul_drum", "maul_spiked", "maul_crystal", "ring", "serpent", "crescent", "shell", "spiked", "flat", "bone", "winged", "horned", "ribbed", "fluted", "bell", "spiral", "inlaid", "talon", "halo", "branches", "prism", "twin", "geode", "vertebrae", "keg", "skull"] {
            assert!(library.components.iter().any(|c| c.visual_profile.shape.as_deref() == Some(shape)), "no part is shaped {shape}");
        }
        for pattern in ["rust", "engraved", "runes"] {
            assert!(library.components.iter().any(|c| c.visual_profile.pattern.as_deref() == Some(pattern)), "no part is {pattern}");
        }
        for pommel in ["wheel", "crescent", "fang", "faceted"] {
            assert!(library.components.iter().any(|c| c.visual_profile.pommel.as_deref() == Some(pommel)), "no {pommel} pommel");
        }
        // Each new special attack is on a part of each of the weapons it suits, the families that
        // had fewest getting the most
        for (special, families) in [
            (Special::CrescentWave, &["sword", "katana", "greatsword"][..]),
            (Special::ImpalingCharge, &["spear", "halberd"]),
            (Special::CoilingLash, &["whip"]),
            (Special::Flay, &["whip"]),
            (Special::Quickdraw, &["bow"]),
            (Special::HuntersMark, &["bow", "crossbow", "harpoon"]),
            (Special::Reckoning, &["pistol", "musket", "crossbow"]),
            (Special::Grapeshot, &["blunderbuss", "handcannon"]),
            (Special::Skyburst, &["bombs"]),
            (Special::PowderKeg, &["bombs"]),
            (Special::StonePillars, &["maul", "hammer"]),
            (Special::WreckingOrbit, &["flail"]),
            (Special::LassoPull, &["whip"]),
            (Special::CoilBind, &["whip"]),
            (Special::ThunderCrack, &["whip"]),
            (Special::SerpentLash, &["whip"]),
            (Special::TripSweep, &["whip"]),
            (Special::BurningBrand, &["whip"]),
            (Special::ChainSwing, &["whip"]),
            (Special::Firecrackers, &["bombs"]),
            (Special::Thornburst, &["bow", "crossbow", "harpoon"]),
            (Special::Thunderstorm, &["bombs"]),
            (Special::Starfall, &["bow"]),
            (Special::Stormchain, &["pistol", "musket"]),
            (Special::Breath, &["blunderbuss", "handcannon"]),
        ] {
            for family in families {
                assert!(library.components.iter().any(|c| c.special == Some(special) && c.compatible_families.iter().any(|f| f == family)), "{special:?} isn't on any {family} part");
            }
            assert!(!special.description().is_empty() && !special.name().is_empty());
        }
        // The new whip specials each on a legendary or mythic part
        for special in [Special::LassoPull, Special::CoilBind, Special::ThunderCrack, Special::SerpentLash, Special::TripSweep, Special::BurningBrand, Special::ChainSwing] {
            assert!(library.components.iter().any(|c| c.special == Some(special) && c.tier >= 4 && c.compatible_families.iter().any(|f| f == "whip")), "{special:?}");
        }
        // The new looks and effects are there to be had
        for shape in ["wire", "spine", "hook", "star", "feather"] {
            assert!(library.components.iter().any(|c| c.visual_profile.shape.as_deref() == Some(shape)), "no part is shaped {shape}");
        }
        for id in ["mark", "lifesteal", "rupture"] {
            let def = library.effects.iter().position(|e| e.id == id && e.effect.is_some()).unwrap_or_else(|| panic!("no {id}"));
            let on = library.components.iter().filter(|c| c.effect_defs.contains(&def)).count();
            assert!(on >= 4, "{id} is only on {on} parts");
        }
    }

    /// What a weapon's head is like makes its heavy blow and its finisher, what a staff's focus
    /// is like how it casts, and what a bow's limbs or a gun's lock are like how it's drawn or loaded
    #[test]
    fn parts_change_how_weapons_move() {
        let library = Library::parse(BUILT_IN).unwrap();
        let made = |family: &str, slot: &str, part: &str| {
            let f = library.family(family).unwrap();
            let mut parts = library.generate(11, Some(f), 0.0).parts;
            let k = library.families[f].slots.iter().position(|s| s == slot).unwrap();
            parts[k] = library.components.iter().position(|c| c.id == part).unwrap_or_else(|| panic!("no {part}"));
            Weapon::assemble(&library, f, parts, 11).profile
        };
        // A hooked or winged spear sweeps, a long-bladed one lunges, a plain leaf as spears do
        assert_eq!(made("spear", "blade", "winged_spearhead").heavy, Heavy::Sweep);
        assert_eq!(made("spear", "blade", "obsidian_needle_point").heavy, Heavy::Lunge);
        assert_eq!(made("spear", "blade", "leaf_point").heavy, Heavy::Style);
        assert_eq!(made("spear", "blade", "obsidian_needle_point").finish, Finish::Lunge);
        // A halo calls down from overhead, a bone cage gathers in both hands, a crook sweeps
        assert_eq!(made("staff", "focus", "ivory_halo_focus").casting, Casting::Overhead);
        assert_eq!(made("staff", "focus", "bone_cage_focus").casting, Casting::Gather);
        // A matchlock's loaded heavily, a wheel-lock with a flourish
        assert_eq!(made("musket", "lock", "gilded_matchlock").knack, Knack::Heavy);
        assert_eq!(made("pistol", "lock", "copper_wheel_lock").knack, Knack::Showy);
        // Across everything that rolls, every way of finishing, casting and drawing turns up, the
        // rare ones in lucky loot
        let (mut finishes, mut castings, mut knacks) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
        for seed in 0..30_000u32 {
            let (family, parts) = library.roll(seed, None, [1.0, 3.0, 8.0, LUCKIEST][seed as usize % 4]);
            let cs: Vec<&Component> = parts.iter().map(|&p| &library.components[p]).collect();
            let p = Weapon::work_out(&library, family, &cs, roll_size(seed), seed).0;
            finishes.insert(format!("{:?}", p.finish));
            castings.insert(format!("{:?}", p.casting));
            knacks.insert(format!("{:?}", p.knack));
        }
        assert_eq!(finishes.len(), 5, "{finishes:?}");
        assert_eq!(castings.len(), 7, "{castings:?}");
        assert_eq!(knacks.len(), 4, "{knacks:?}");
    }

    #[test]
    fn every_synergy_can_happen() {
        let library = Library::parse(BUILT_IN).unwrap();
        let mut seen = BTreeSet::new();
        for seed in 0..40_000 {
            let (family, parts) = library.roll(seed, None, 3.0);
            let components: Vec<&Component> = parts.iter().map(|&p| &library.components[p]).collect();
            let (profile, _, _) = Weapon::work_out(&library, family, &components, roll_size(seed), seed);
            seen.extend(profile.synergies.iter().map(|s| s.def));
        }
        assert_eq!(seen.len(), library.synergies.len());
    }

    fn damage(p: &Profile) -> f64 {
        ["physicalDamage", "elementalDamage", "spellDamage"].iter().map(|k| p.stat(k)).sum()
    }

    /// The same parts made rarer hit harder, each for its share of the weapon, the AoE kinds and
    /// the battle staff's spells as much as the rest
    #[test]
    fn rarer_parts_hit_harder() {
        let library = Library::parse(BUILT_IN).unwrap();
        let mut rarer = Library::parse(BUILT_IN).unwrap();
        for pair in library.rarities.windows(2) {
            assert!(pair[1].damage_scale > pair[0].damage_scale && pair[1].speed_scale > pair[0].speed_scale, "{} scales up from {}", pair[1].id, pair[0].id);
        }
        let top = library.rarities.len() - 1;
        let made = |lib: &Library, family: usize, parts: &[usize], seed: u32| {
            let cs: Vec<&Component> = parts.iter().map(|&p| &lib.components[p]).collect();
            Weapon::work_out(lib, family, &cs, roll_size(seed), seed).0
        };
        let mut families = BTreeSet::new();
        for seed in 0..600 {
            let (family, parts) = library.roll(seed, None, 1.0);
            if parts.iter().all(|&p| library.components[p].tier == top) {
                continue;
            }
            let plain = made(&library, family, &parts, seed);
            for &p in &parts {
                rarer.components[p].tier = (library.components[p].tier + 1).min(top);
            }
            let better = made(&rarer, family, &parts, seed);
            for &p in &parts {
                rarer.components[p].tier = library.components[p].tier;
            }
            let fam = &library.families[family].id;
            assert!(damage(&better) > damage(&plain) * 1.03, "{fam} from seed {seed}: {:.1} rarer, {:.1} as it is", damage(&better), damage(&plain));
            assert!(better.quality > plain.quality && better.pace > plain.pace && better.forged == plain.forged, "{fam}");
            for k in ["physicalDamage", "spellDamage"] {
                if plain.stat(k) > 0.0 {
                    assert!(better.stat(k) > plain.stat(k), "{fam}'s {k}");
                }
            }
            families.insert(fam.clone());
        }
        for fam in ["flail", "maul", "bombs", "rod", "censer", "battlestaff", "bow", "musket", "orb"] {
            assert!(families.contains(fam), "no {fam} was tried");
        }
        // A blade made rarer counts for more than a guard
        let sword = library.family("sword").unwrap();
        let (_, parts) = library.roll(3, Some(sword), 0.0);
        let plain = made(&library, sword, &parts, 3);
        let slot = |name: &str| library.families[sword].slots.iter().position(|s| s == name).unwrap();
        let raise = |lib: &mut Library, k: usize| lib.components[parts[k]].tier = top;
        raise(&mut rarer, slot("blade"));
        let blade = made(&rarer, sword, &parts, 3);
        rarer.components[parts[slot("blade")]].tier = library.components[parts[slot("blade")]].tier;
        raise(&mut rarer, slot("guard"));
        let guard = made(&rarer, sword, &parts, 3);
        assert!(blade.quality - plain.quality > 2.5 * (guard.quality - plain.quality), "a blade {:.3}, a guard {:.3}", blade.quality, guard.quality);
    }

    /// No two weapons come out of the forge quite alike, but the same one always does
    #[test]
    fn forge_rolls_stay_in_range_and_follow_the_seed() {
        let library = Library::parse(BUILT_IN).unwrap();
        let (mut low, mut high) = (f64::MAX, f64::MIN);
        let mut makes = BTreeMap::new();
        for seed in 0..6000 {
            let (family, parts) = library.roll(seed, None, 2.0);
            let cs: Vec<&Component> = parts.iter().map(|&p| &library.components[p]).collect();
            let (quality, pace, forged) = Weapon::forging(&library, &cs, seed);
            assert_eq!((quality, pace, forged), Weapon::forging(&library, &cs, seed));
            assert!((forged - 1.0).abs() <= FORGE_SPREAD + 1e-12, "seed {seed} forged {forged}");
            (low, high) = (low.min(forged), high.max(forged));
            // The same parts from another seed come out another way
            let other = Weapon::forging(&library, &cs, seed ^ 0x5eed).2;
            assert!(other != forged || seed % 1000 == 0, "seed {seed}");
            let profile = Weapon::work_out(&library, family, &cs, roll_size(seed), seed).0;
            *makes.entry(profile.make()).or_insert(0) += 1;
        }
        assert!(low < 1.0 - 0.95 * FORGE_SPREAD && high > 1.0 + 0.95 * FORGE_SPREAD, "{low} - {high}");
        // Every kind of make turns up, the best and worst less often
        assert_eq!(makes.len(), 4, "{makes:?}");
        assert!(makes["Masterwork"] < makes["Plain"] && makes["Flawed"] < makes["Plain"], "{makes:?}");
        // Made again from its seed, a weapon is just the same
        for seed in 0..40 {
            let (a, b) = (library.generate(seed, None, 3.0), library.generate(seed, None, 3.0));
            assert_eq!(a.profile.forged, b.profile.forged);
            assert_eq!(a.profile.stats, b.profile.stats);
        }
    }

    /// However rare, light and small, or common, heavy and big, nothing swings, draws, casts or
    /// loads faster or slower than the limits
    #[test]
    fn speeds_stay_clamped() {
        let library = Library::parse(BUILT_IN).unwrap();
        let mut rarest = Library::parse(BUILT_IN).unwrap();
        for c in &mut rarest.components {
            c.tier = rarest.rarities.len() - 1;
        }
        for seed in 0..8000 {
            let luck = [0.0, 1.0, 4.0, 12.0][seed as usize % 4];
            let (family, parts) = library.roll(seed, None, luck);
            for lib in [&library, &rarest] {
                let cs: Vec<&Component> = parts.iter().map(|&p| &lib.components[p]).collect();
                for size in [0, SIZES.len() - 1] {
                    let profile = Weapon::work_out(lib, family, &cs, size, seed).0;
                    for k in ["attackSpeed", "drawSpeed", "castSpeed", "reloadSpeed"] {
                        if let Some(&v) = profile.stats.get(k) {
                            assert!((0.25..=6.0).contains(&v), "{} {k} {v}", library.families[family].id);
                        }
                    }
                }
            }
        }
    }

    /// A head of stone or a shaft of iron makes a weapon slower than one of bone, crystal or wood
    #[test]
    fn heavy_parts_are_slower_than_light_ones() {
        let library = Library::parse(BUILT_IN).unwrap();
        let find = |id: &str| library.components.iter().find(|c| c.id == id).unwrap_or_else(|| panic!("no {id}"));
        // As heavy as what they're made of, next to what's usual in their slot
        assert!(heaviness(find("stone_maul_head")) > heaviness(find("iron_maul_head")));
        assert!(heaviness(find("iron_blade")) > heaviness(find("bone_knife")));
        assert!(heaviness(find("iron_shaft")) > heaviness(find("oak_shaft")));
        assert!((heaviness(find("iron_blade")) - 1.0).abs() < 1e-9 && (heaviness(find("oak_shaft")) - 1.0).abs() < 1e-9);
        // Everything the same but the part that's swung or drawn, of the same rarity: the
        // heaviest of them makes it the slowest, the lightest the quickest
        let mut tried = 0;
        for (f, fam) in library.families.iter().enumerate() {
            let (_, parts) = library.roll(f as u32, Some(f), 0.0);
            let main = fam.slots.iter().position(|s| slot_share(s) >= 3.0).unwrap();
            let tier = library.components[parts[main]].tier;
            let fits: Vec<usize> = (0..library.components.len())
                .filter(|&i| library.components[i].slot_type == fam.slots[main] && library.components[i].compatible_families.contains(&fam.id) && library.components[i].tier == tier)
                .collect();
            let by = |i: &usize| heaviness(&library.components[*i]);
            let heavy = *fits.iter().max_by(|a, b| by(a).total_cmp(&by(b))).unwrap();
            let light = *fits.iter().min_by(|a, b| by(a).total_cmp(&by(b))).unwrap();
            if by(&heavy) - by(&light) < 0.05 {
                continue;
            }
            let pace = |part: usize| {
                let mut with = parts.clone();
                with[main] = part;
                let cs: Vec<&Component> = with.iter().map(|&p| &library.components[p]).collect();
                Weapon::forging(&library, &cs, 9).1
            };
            assert!(pace(heavy) < pace(light) * 0.99, "{}: {} {:.3}, {} {:.3}", fam.id, library.components[heavy].id, pace(heavy), library.components[light].id, pace(light));
            tried += 1;
        }
        assert!(tried >= 12, "only {tried} families have parts of different weights");
        // And it shows in its swings a second: a stone maul's slower than an iron one
        let maul = library.family("maul").unwrap();
        let (_, parts) = library.roll(5, Some(maul), 0.0);
        let speed = |head: &str| {
            let mut with: Vec<&Component> = parts.iter().map(|&p| &library.components[p]).collect();
            with[0] = find(head);
            Weapon::work_out(&library, maul, &with, 2, 5).0.stat("attackSpeed")
        };
        assert!(speed("stone_maul_head") < speed("iron_maul_head"));
    }

    /// What each family's weapons do, made of its commonest parts, its lightest and heaviest
    /// common ones, and its commonest rare, epic and mythic ones, of the middle size and a plain
    /// make: their damage and speed without what the rarity and weight of their parts do and
    /// with, and the most a heavy critical blow of theirs does, to set against what they fight.
    /// To look at: cargo test --release balance_table -- --ignored --nocapture
    #[test]
    #[ignore]
    fn balance_table() {
        // How much harder a heavy blow hits, as combat.rs has it
        const HEAVY_BLOW: f64 = 1.8;
        let library = Library::parse(BUILT_IN).unwrap();
        let speed = |p: &Profile| ["attackSpeed", "drawSpeed", "castSpeed", "reloadSpeed"].iter().map(|k| p.stat(k)).find(|&v| v > 0.0).unwrap_or(0.0);
        println!("| family | made of | grade | parts | damage before | after | speed before | after | heavy crit |");
        for (f, fam) in library.families.iter().enumerate() {
            // For each slot, the part of `tier`, or the nearest below it, that's most `by`
            let pick = |tier: usize, by: &dyn Fn(&Component) -> f64| -> Vec<usize> {
                fam.slots
                    .iter()
                    .map(|slot| {
                        let fits: Vec<usize> = (0..library.components.len()).filter(|&i| library.components[i].slot_type == *slot && library.components[i].compatible_families.contains(&fam.id)).collect();
                        let tiers = || fits.iter().map(|&i| library.components[i].tier);
                        let near = tiers().filter(|&t| t <= tier).max().unwrap_or_else(|| tiers().min().unwrap());
                        *fits.iter().filter(|&&i| library.components[i].tier == near).max_by(|&&a, &&b| by(&library.components[a]).total_cmp(&by(&library.components[b]))).unwrap()
                    })
                    .collect()
            };
            let show = |label: &str, parts: &[usize]| {
                let cs: Vec<&Component> = parts.iter().map(|&p| &library.components[p]).collect();
                let (p, _, grade) = Weapon::work_out(&library, f, &cs, SIZES.len() / 2, 0);
                let after = damage(&p) / p.forged;
                println!(
                    "| {} | {label} | {} | {} | {:.1} | {:.1} | {:.2} | {:.2} | {:.0} |",
                    fam.id,
                    library.rarities[grade].id,
                    cs.iter().map(|c| format!("{} ({})", c.id, &library.rarities[c.tier].id[..1])).collect::<Vec<_>>().join(", "),
                    after / p.quality,
                    after,
                    speed(&p) / p.pace,
                    speed(&p),
                    after * HEAVY_BLOW * p.stat("critDamage"),
                );
            };
            show("commonest", &pick(0, &|c| c.weight));
            show("lightest", &pick(0, &|c| -heaviness(c)));
            show("heaviest", &pick(0, &|c| heaviness(c)));
            show("rare", &pick(2, &|c| c.weight));
            show("epic", &pick(3, &|c| c.weight));
            show("mythic", &pick(5, &|c| c.weight));
        }
    }
}
