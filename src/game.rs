//! Living off the land: food to find and eat, an inventory to carry it in, animals to hunt or pet,
//! weapons to find in chests and fight with, and the heads-up display that shows it all. Positions
//! are in voxels of the main world.

use std::collections::{HashMap, HashSet};
use std::f64::consts::{FRAC_PI_2, TAU};
use std::time::Instant;

use egui_winit_vulkano::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, StrokeKind, pos2, vec2};
use nalgebra::{UnitQuaternion, Vector3};

use crate::animals::{self, ANIMAL_MODELS, Animal, AnimalInstance, Build, Look, Model, pack_models};
use crate::audio::{Cue, Ground, Sound};
use crate::bosses::{Deed, Move};
use crate::combat::{Arena, Combat, Controls, HostileKind, MAX_MANA, View};
use crate::leaves::Leaves;
use crate::player::WaterLevel;
use crate::weapons::{DROP_EMBER, DROP_ICE, DROP_MUD, DROP_WATER, Element, Kind, Library, Profile, STATS, Special, Unit, Weapon};
use crate::worldgen::{World, is_passable, material::*};

/// How far away things can be picked up or hit, in meters
pub const REACH: f64 = 4.0;
/// Hunger drops from full to empty in this many seconds
const HUNGER_SECONDS: f64 = 600.0;
/// Animals further than this aren't drawn, and food further than that, in meters
const ANIMAL_RANGE: f64 = 90.0;
const FOOD_RANGE: f64 = 60.0;
const PROP_RANGE: f64 = 30.0;
/// The most animals, bits of food, props and things flying drawn at once, nearest first
const MAX_INSTANCES: usize = 140;
/// About this many animals live around the player
const ANIMALS: usize = 38;
/// Apples keep growing back near the player up to this many
const APPLES: usize = 48;
/// How much water the bucket holds, in cubic meters. A magic bucket, to make a splash.
pub const BUCKET_CAPACITY: f64 = 2.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Item {
    Bucket,
    Apple,
    Meat,
    Bread,
    /// Heals when drunk
    Potion,
    /// Gold lying on the ground, this much of it. What's carried is Game::gold.
    Gold(u32),
    /// One of Game::weapons
    Weapon(usize),
}

impl Item {
    /// What it looks like lying on the ground. Weapons are drawn with their own models.
    fn look(self) -> Option<Look> {
        match self {
            Item::Apple => Some(Look::Apple),
            Item::Meat => Some(Look::Meat),
            Item::Bread => Some(Look::Bread),
            Item::Potion => Some(Look::Potion),
            Item::Gold(_) => Some(Look::Coins),
            Item::Bucket | Item::Weapon(_) => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Item::Bucket => "Bucket",
            Item::Apple => "Apple",
            Item::Meat => "Raw Meat",
            Item::Bread => "Bread",
            Item::Potion => "Healing Potion",
            Item::Gold(_) => "Gold",
            Item::Weapon(_) => "Weapon",
        }
    }

    /// How much hunger eating one takes away, out of 100
    fn food(self) -> f64 {
        match self {
            Item::Apple => 12.0,
            Item::Meat => 30.0,
            Item::Bread => 22.0,
            _ => 0.0,
        }
    }

    /// How much health drinking one gives back
    fn heal(self) -> f64 {
        match self {
            Item::Potion => 50.0,
            _ => 0.0,
        }
    }

    /// Eaten or drunk, from the inventory or the quick bar
    fn consumable(self) -> bool {
        self.food() > 0.0 || self.heal() > 0.0
    }
}

/// What a merchant asks for each kind of food, in gold. Selling it back fetches SELL_SHARE of it.
fn food_price(item: Item) -> u32 {
    match item {
        Item::Apple => 4,
        Item::Meat => 6,
        Item::Bread => 10,
        Item::Potion => 35,
        _ => 0,
    }
}

/// How much of what a merchant asks it pays for what the player sells it
const SELL_SHARE: f64 = 0.35;
/// Slots on the quick bar; the rest of what's carried is only in the inventory
pub const HOTBAR: usize = 9;
/// Gold the player starts with
const STARTING_GOLD: u32 = 60;

/// Something a merchant has to sell: a weapon of Game::weapons, or food or potions, so many of them
#[derive(Clone, Copy)]
enum Ware {
    Weapon(usize),
    Food(Item, u32),
}

/// Something done with what's carried, in the inventory or at a merchant's, once the window's drawn
enum Deal {
    Hold(usize),
    Eat(usize),
    Drop(usize),
    Sort,
    Buy(usize),
    Sell(usize),
}

/// The color of gold, in the HUD and on cards
const GOLD: Color32 = Color32::from_rgb(255, 205, 70);

/// How wide `text` is in the HUD at `size`
fn galley_width(painter: &egui::Painter, text: &str, size: f32) -> f32 {
    painter.layout_no_wrap(text.to_owned(), FontId::proportional(size), Color32::WHITE).size().x
}

/// What a weapon's worth to a merchant, by its rarity, before what its effects and synergies add
const WEAPON_PRICES: [f64; 6] = [25.0, 60.0, 150.0, 360.0, 850.0, 2000.0];
/// Seconds what's dropped flies out through the air before it lands
const DROP_SECONDS: f64 = 0.75;
/// How far off a weapon's beam of light shows where it lies, in meters
const BEAM_RANGE: f64 = 45.0;
/// How far from a merchant the player can trade, in meters
const SHOP_REACH: f64 = 6.0;

/// Something lying around to pick up: food, a potion, gold, or a weapon dropped by a boss
struct Pickup {
    item: Item,
    position: Vector3<f64>,
    heading: f64,
    /// Where it was thrown from as it was dropped, and seconds left before it lands
    from: Vector3<f64>,
    flying: f64,
    /// Whether the camera can see it, for its beam of light to show
    seen: bool,
    /// Shown by a beam of light like a weapon is, as the gold the player lost when they died
    beacon: bool,
}

impl Pickup {
    fn lying(item: Item, position: Vector3<f64>, heading: f64) -> Self {
        Pickup { item, position, heading, from: position, flying: 0.0, seen: false, beacon: false }
    }

    /// Whether a beam of light shows where it lies
    fn shines(&self) -> bool {
        self.beacon || matches!(self.item, Item::Weapon(_))
    }

    /// Where it is now, in voxels: flying in an arc from where it was dropped, then where it landed
    fn at(&self, vpm: f64) -> Vector3<f64> {
        if self.flying <= 0.0 {
            return self.position;
        }
        let t = 1.0 - self.flying / DROP_SECONDS;
        let high = (1.0 + 0.3 * (self.position - self.from).norm() / vpm) * vpm;
        self.from + (self.position - self.from) * t + Vector3::new(0.0, 0.0, 4.0 * t * (1.0 - t) * high)
    }
}

/// A chest in a building, with a weapon in it until it's opened
#[derive(Clone)]
pub struct Chest {
    pub position: Vector3<f64>,
    pub heading: f64,
    /// Where it is in the whole landscape, which stays the same however the world moves, so it
    /// stays open, and what's in it is the same every time
    pub key: [i64; 3],
    pub luck: f64,
}

/// A splash of the player's own blood over the view: where, in fractions of the screen across and
/// down, how big next to its height, how long it's been there, and the shape it splashed in
struct Splat {
    at: [f32; 2],
    size: f32,
    age: f64,
    seed: u32,
}

/// Seconds a splash of blood stays on the view
const SPLAT_SECONDS: f64 = 2.4;
/// Seconds the view takes to come back after the player rises again
const DOWN_SECONDS: f64 = 2.0;
/// Seconds the player lies dead before they can rise again
const RISE_AFTER: f64 = 2.5;

/// A hall of a dungeon that monsters haunt
pub struct Den {
    /// Where on the floor of a dungeon hall, in voxels
    pub position: Vector3<f64>,
    /// Where in the whole landscape, to remember it's been cleared
    pub key: [i64; 3],
    pub kind: DenKind,
}

/// What a den is, which decides what haunts it
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DenKind {
    /// A hall of a dungeon, haunted by the dead or spiders
    Hall,
    /// A cavern cave spiders nest in
    Nest,
    /// A bandits' camp
    Camp,
}

/// Where a merchant has their stall: where in voxels, and where in the whole landscape, for what
/// they've sold to stay sold, and how far from there in meters they may stand
pub struct Stall {
    pub position: Vector3<f64>,
    pub key: [i64; 3],
    pub within: f64,
}

/// A town's square: where its middle is in voxels, on the paving, how many folk live round it
/// and how far from it in meters they stroll
pub struct Town {
    pub position: Vector3<f64>,
    pub folk: usize,
    pub roam: f64,
}

/// What villagers say when spoken to
const GREETINGS: [&str; 8] = [
    "\"Good day to you, traveller.\"",
    "\"The merchant on the square has the best bread for miles.\"",
    "\"Mind the caves in the hills. There are spiders down there the size of carts.\"",
    "\"Wolves came down from the hills again last winter.\"",
    "\"They say the old dungeons are full of gold. And worse.\"",
    "\"Lovely weather, isn't it?\"",
    "\"Stay a while. Nobody's in a hurry here.\"",
    "\"You look like you've been in a fight or two.\"",
];

/// Where a boss lives: which one, where in voxels, on the ground there, and where in the whole
/// landscape, to remember it's beaten
pub struct Lair {
    pub look: Look,
    pub position: Vector3<f64>,
    pub key: [i64; 3],
}

/// How much health the player has at full
pub const MAX_HEALTH: f64 = 100.0;
/// Seconds the ground stirs before a treant's roots burst up out of it, and how far round in meters
const ROOTS_WARNING: f64 = 1.0;
const ROOTS_RADIUS: f64 = 2.2;
/// Thorns in a treant's volley
const THORNS: usize = 5;
/// The fastest bosses and monsters can throw the player, in meters a second, along the ground and
/// up: a blow knocks them a few steps back, it never flings them across the field
const THROWN_MOST: f64 = 6.0;
/// How fast the player moves while chilled by frost, and stuck in web, next to usual
const CHILLED_PACE: f64 = 0.55;
const SNARED_PACE: f64 = 0.4;
/// How far round in meters web leaves the ground sticky, for how many seconds, and how many
/// patches of it there can be at once
const WEB_RADIUS: f64 = 2.8;
const WEB_SECONDS: f64 = 6.0;
const WEBS: usize = 3;
/// Roots in each line of a treant's root wave, how far apart in meters, and how far round each is
const WAVE_ROOTS: usize = 9;
const WAVE_SPACING: f64 = 2.6;
const WAVE_RADIUS: f64 = 1.3;
/// Things coming down in a drake's storm drop from this many meters up
const STORM_HEIGHT: f64 = 18.0;

/// Roots about to burst up out of the ground at `at`, in voxels, `left` seconds from now, hurting
/// the player within `radius` meters for `damage` and throwing them along `drag`
struct Root {
    at: Vector3<f64>,
    left: f64,
    radius: f64,
    damage: f64,
    drag: Vector3<f64>,
    element: Option<Element>,
}

/// The ground under `at`, in voxels, looking down a few meters, or `at` if there's none
fn ground_under(world: &World, at: Vector3<f64>) -> Vector3<f64> {
    let vpm = world.voxels_per_meter as f64;
    let (x, y) = (at.x.floor() as i64, at.y.floor() as i64);
    let top = at.z.floor() as i64 + vpm as i64;
    (top - 8 * vpm as i64..=top).rev().find(|&z| !is_passable(world.material_at(x, y, z))).map_or(at, |z| Vector3::new(at.x, at.y, z as f64 + 1.0))
}

/// What the crosshair is on
#[derive(Clone, Copy)]
enum Target {
    Animal(usize),
    Pickup(usize),
    Chest(usize),
}

pub struct Game {
    animals: Vec<Animal>,
    pickups: Vec<Pickup>,
    /// Candles, skulls and such in buildings, that are only there to look at
    props: Vec<(Look, Vector3<f64>, f64)>,
    chests: Vec<Chest>,
    opened: HashSet<[i64; 3]>,
    /// What the player carries, and how many of each, shown in the quick bar
    slots: Vec<(Item, u32)>,
    /// The quick bar slot in hand
    pub selected: usize,
    /// 100 is full, 0 is starving
    hunger: f64,
    /// Out of MAX_HEALTH
    health: f64,
    /// Seconds since the player was last hurt, for their health to come back
    since_hurt: f64,
    /// The red of being hurt, fading
    hurt_flash: f64,
    /// The player's own blood splashed over the view by blows, fading
    splats: Vec<Splat>,
    /// Seconds the player goes on burning, on being poisoned, and on being slowed by frost
    burning: f64,
    poisoned: f64,
    chilled: f64,
    /// Seconds the player goes on being stuck in web
    snared: f64,
    /// Roots about to burst up
    roots: Vec<Root>,
    /// Ground left sticky with web, and for how many more seconds
    webs: Vec<(Vector3<f64>, f64)>,
    /// So the sound of being hurt doesn't go off every frame
    hurt_sound: f64,
    breath_sound: f64,
    /// So the whoosh of something just missing doesn't go off over and over
    dodge_sound: f64,
    /// The boss bar: which boss, how much health it showed last, a trail lagging behind that,
    /// seconds before the trail starts after it, and a flash on a big hit
    boss_bar: (&'static str, f64, f64, f64, f64),
    /// How hard the player's own dashes throw them, in voxels a second, for main.rs to apply
    knockback: Vector3<f64>,
    /// And how hard bosses' and monsters' blows do, which is held to THROWN_MOST so nothing can
    /// fling them far
    thrown: Vector3<f64>,
    /// The camera shaking from what bosses do, wearing off
    shake: f64,
    /// The player died: what killed them, and seconds since. They lie there until they rise
    /// again, at the last place they stood safe.
    dead: Option<(String, f64)>,
    /// Whether they died since main.rs last asked, see take_died(), and the gold they lost
    died: bool,
    lost: u32,
    /// Seconds since the player rose again, for the view to come back
    down: f64,
    /// Where the player last stood safe, their feet in voxels: on the ground, out of the water,
    /// with nothing after them. And seconds they've stood safe, so a moment's lull doesn't count.
    rest: Option<Vector3<f64>>,
    safe_for: f64,
    /// Bosses' lairs in this piece of the world, and those beaten, by where they are in the landscape
    lairs: Vec<Lair>,
    defeated: HashSet<[i64; 3]>,
    /// Dungeon halls in this piece of the world, and those cleared of monsters
    dens: Vec<[i64; 3]>,
    cleared: HashSet<[i64; 3]>,
    /// Chests beaten bosses left, kept as the world moves on
    boss_chests: Vec<Chest>,
    /// Cubic meters of water in the bucket
    bucket: f64,
    /// Gold carried, to spend at merchants
    gold: u32,
    /// The merchant being traded with, by where it stands in the whole landscape, and what each
    /// merchant has left to sell, by the same
    shop: Option<[i64; 3]>,
    stocks: HashMap<[i64; 3], Vec<Ware>>,
    /// The merchants about, by Animal::id, with where each stands in the whole landscape
    merchants: Vec<(u32, [i64; 3])>,
    /// Where the player stood last, for what they drop to land at their feet
    feet: Option<Vector3<f64>>,
    /// The slot looked at in the inventory, with its card showing
    inspected: Option<usize>,
    messages: Vec<(String, Instant)>,
    /// Sounds to play
    cues: Vec<Cue>,
    target: Option<Target>,
    apple_timer: f64,
    /// What weapons can be made of, and the ones the player has
    library: Library,
    weapons: Vec<Weapon>,
    combat: Combat,
    leaves: Leaves,
    /// The models of animals and things lying around, see animals::animal_models()
    models: Vec<Model>,
    /// Changes whenever there are new models to draw, like when a weapon is found
    models_version: u32,
    seed: u32,
    rng: u32,
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

/// What the ground's mostly made of round `at`, looking far enough out to see past whatever's
/// built there, and leaving out what's under water
fn ground_round(world: &World, water: WaterLevel, at: Vector3<f64>) -> u8 {
    let vpm = world.voxels_per_meter as f64;
    let mut counts = [0u32; 256];
    for k in 0..24 {
        let a = k as f64 / 24.0 * TAU;
        let r = if k % 2 == 0 { 10.0 } else { 16.0 } * vpm;
        let (x, y) = (at.x + a.cos() * r, at.y + a.sin() * r);
        let (xi, yi) = (x.floor() as i64, y.floor() as i64);
        let top = (0..world.size[2] as i64).rev().find(|&z| {
            let m = world.material_at(xi, yi, z);
            !is_passable(m) && !is_leaves(m)
        });
        if let Some(z) = top.filter(|&z| z as f64 + 1.0 >= water(x, y)) {
            counts[world.material_at(xi, yi, z) as usize] += 1;
        }
    }
    (0..256).max_by_key(|&m| counts[m]).unwrap_or(0) as u8
}

fn is_leaves(material: u8) -> bool {
    matches!(material, LEAVES | LEAVES_DARK)
}

fn color(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// An apple under a leafy tree near x, y: hanging under its leaves, or fallen on the ground below
fn apple_at(world: &World, water: WaterLevel, x: f64, y: f64, rng: &mut u32) -> Option<Pickup> {
    let vpm = world.voxels_per_meter as f64;
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    let mut lowest_leaves = None;
    for z in (0..world.size[2] as i64).rev() {
        let material = world.material_at(xi, yi, z);
        if is_leaves(material) {
            lowest_leaves = Some(z);
        } else if !is_passable(material) {
            let leaves = lowest_leaves?;
            if water(x, y) > z as f64 {
                return None;
            }
            // Only under a crown with room beneath it, not in a bush
            if (leaves - z) as f64 / vpm < 0.8 {
                return None;
            }
            let hanging = random(rng) < 0.35;
            let height = if hanging { leaves as f64 - 0.15 * vpm } else { z as f64 + 1.0 };
            return Some(Pickup::lying(Item::Apple, Vector3::new(x, y, height), random(rng) * TAU));
        }
    }
    None
}

impl Game {
    pub fn new(world: &World, water: WaterLevel, seed: u32, near: Vector3<f64>) -> Self {
        let mut game = Game {
            animals: Vec::new(),
            pickups: Vec::new(),
            props: Vec::new(),
            chests: Vec::new(),
            opened: HashSet::new(),
            slots: Vec::new(),
            selected: 0,
            hunger: 80.0,
            health: MAX_HEALTH,
            since_hurt: 100.0,
            hurt_flash: 0.0,
            splats: Vec::new(),
            burning: 0.0,
            poisoned: 0.0,
            chilled: 0.0,
            snared: 0.0,
            roots: Vec::new(),
            webs: Vec::new(),
            hurt_sound: 0.0,
            breath_sound: 0.0,
            dodge_sound: 0.0,
            boss_bar: ("", 1.0, 1.0, 0.0, 0.0),
            knockback: Vector3::zeros(),
            thrown: Vector3::zeros(),
            shake: 0.0,
            dead: None,
            died: false,
            lost: 0,
            down: DOWN_SECONDS,
            rest: None,
            safe_for: 0.0,
            lairs: Vec::new(),
            defeated: HashSet::new(),
            dens: Vec::new(),
            cleared: HashSet::new(),
            boss_chests: Vec::new(),
            bucket: 0.0,
            gold: STARTING_GOLD,
            shop: None,
            stocks: HashMap::new(),
            merchants: Vec::new(),
            feet: None,
            inspected: None,
            messages: Vec::new(),
            cues: Vec::new(),
            target: None,
            apple_timer: 0.0,
            library: Library::load(),
            weapons: Vec::new(),
            combat: Combat::new(seed, ANIMAL_MODELS as u32),
            leaves: Leaves::new(seed),
            models: animals::animal_models(),
            models_version: 0,
            seed,
            rng: hash(seed ^ 0x9a3e),
        };
        game.arm(seed, 1.0);
        game.populate(world, water, seed, near);
        game
    }

    /// What the player starts with: a sword, a bow and a staff rolled from `seed` with `luck`, as
    /// for Library::roll_rarity(), and the bucket. Replaces whatever weapons they had.
    pub fn arm(&mut self, seed: u32, luck: f64) {
        self.weapons.clear();
        self.slots.retain(|(item, _)| !matches!(item, Item::Weapon(_) | Item::Bucket));
        let mut fresh = Vec::new();
        let families = ["sword", "bow", "staff", "axe", "hammer", "dagger", "spear", "wand", "greatsword", "katana", "scythe", "halberd", "scepter", "whip"];
        for (k, family) in families.into_iter().enumerate() {
            if let Some(family) = self.library.family(family) {
                self.weapons.push(self.library.generate(hash(seed ^ (k as u32 + 1) * 0x51ed), Some(family), luck));
                fresh.push((Item::Weapon(self.weapons.len() - 1), 1));
            }
        }
        fresh.push((Item::Bucket, 1));
        fresh.append(&mut self.slots);
        self.slots = fresh;
        self.selected = 0;
        self.models_version += 1;
    }

    /// Fills a new world with animals and apples. What the player carries stays.
    pub fn populate(&mut self, world: &World, water: WaterLevel, seed: u32, near: Vector3<f64>) {
        self.animals = animals::spawn(world, water, seed, near);
        self.lairs.clear();
        self.defeated.clear();
        self.dens.clear();
        self.cleared.clear();
        self.boss_chests.clear();
        self.pickups.clear();
        self.target = None;
        self.seed = seed;
        self.combat.clear();
        self.leaves.clear();
        self.rest = None;
        self.safe_for = 0.0;
        // A merchant not far from where the player starts, to trade with from the first
        self.merchants.clear();
        self.shop = None;
        let away = hash(seed ^ 0x3e7) as f64 / u32::MAX as f64 * TAU;
        let stall = near + Vector3::new(away.cos(), away.sin(), 0.0) * 9.0 * world.voxels_per_meter as f64;
        if let Some(merchant) = animals::spawn_merchant(world, water, stall, 4.0, seed ^ 0x3e7) {
            self.merchants.push((merchant.id, [near.x as i64, near.y as i64, -2 - seed as i64]));
            self.animals.push(merchant);
        }
        for _ in 0..4000 {
            if self.apple_count() >= APPLES {
                break;
            }
            self.grow_apple(world, water, near);
        }
    }

    /// The models to draw that never change, packed for shaders\traverse.comp: the animals' and
    /// everything lying around, the arrows, spells and arms, and the falling leaves. The weapons'
    /// come after them, see weapon_model_words().
    pub fn model_words(&self) -> Vec<u32> {
        let models: Vec<&Model> = self.models.iter().chain(self.combat.models.iter().map(|(m, _)| m)).chain(self.leaves.models.iter()).collect();
        pack_models(&models)
    }

    /// Each weapon's models, packed the same way after the number of the first among all models.
    /// They're kept apart from the rest, which never change, as sending all of those again each
    /// time a weapon turns up takes long enough to hold up the game.
    pub fn weapon_model_words(&self) -> Vec<u32> {
        let models: Vec<&Model> = self.weapons.iter().flat_map(|w| w.look.frames.iter().map(|(m, _)| m)).collect();
        let mut words = vec![self.weapon_model(0)];
        words.extend(pack_models(&models));
        words
    }

    pub fn models_version(&self) -> u32 {
        self.models_version
    }

    /// Where the leaves' models start among model_words()
    fn leaf_model(&self) -> u32 {
        (self.models.len() + self.combat.models.len()) as u32
    }

    /// Where weapon `index`'s models start among model_words()
    fn weapon_model(&self, index: usize) -> u32 {
        let before: usize = self.weapons[..index].iter().map(|w| w.look.frames.len()).sum();
        self.leaf_model() + (self.leaves.models.len() + before) as u32
    }

    /// The weapon in hand, with its index
    fn held(&self) -> Option<(usize, &Weapon)> {
        match self.slots.get(self.selected) {
            Some(&(Item::Weapon(i), _)) => Some((i, &self.weapons[i])),
            _ => None,
        }
    }

    pub fn holding_weapon(&self) -> bool {
        self.held().is_some()
    }

    pub fn holding_bucket(&self) -> bool {
        self.slots.get(self.selected).is_some_and(|(item, _)| *item == Item::Bucket)
    }

    /// Room left in the bucket, in cubic meters
    pub fn bucket_room(&self) -> f64 {
        BUCKET_CAPACITY - self.bucket
    }

    pub fn fill_bucket(&mut self, cubic_meters: f64) {
        self.bucket = (self.bucket + cubic_meters).min(BUCKET_CAPACITY);
        self.say(if self.bucket >= BUCKET_CAPACITY - 0.01 { "Your bucket is full." } else { "You scoop up some water." });
    }

    /// Tips the bucket out, returning how much water was in it
    pub fn empty_bucket(&mut self) -> f64 {
        let water = self.bucket;
        self.bucket = 0.0;
        if water <= 0.0 {
            self.say("The bucket is empty. Click on water to fill it.");
        }
        water
    }

    /// Whether the crosshair is on an animal or something to pick up
    pub fn has_target(&self) -> bool {
        self.target.is_some()
    }

    /// Cubic meters of water in the bucket
    pub fn bucket(&self) -> f64 {
        self.bucket
    }

    /// The newest message to the player, if any
    pub fn last_message(&self) -> Option<&str> {
        self.messages.last().map(|(m, _)| m.as_str())
    }

    fn item_name(&self, item: Item) -> String {
        match item {
            Item::Weapon(i) => self.weapons[i].name.clone(),
            _ => item.name().to_owned(),
        }
    }

    /// What the player carries, for checking from scripts
    pub fn carried(&self) -> String {
        self.slots.iter().map(|&(item, count)| format!("{} x{}", self.item_name(item), count)).collect::<Vec<_>>().join(", ")
    }

    /// Every weapon carried, described in full, for checking from scripts
    pub fn describe_weapons(&self) -> String {
        self.weapons.iter().map(|w| self.describe(w)).collect::<Vec<_>>().join("\n")
    }

    /// The weapon in hand, as describe_weapons() puts it, with its special attack
    pub fn describe_held(&self) -> String {
        self.held().map_or("nothing".into(), |(_, w)| {
            let special = w.profile.special.as_ref().map_or(String::new(), |(s, part)| format!(" Special: {} from {}.", s.name(), part));
            format!("{}{}", self.describe(w), special)
        })
    }

    fn describe(&self, weapon: &Weapon) -> String {
        let library = &self.library;
        let family = &library.families[weapon.family];
        let parts: Vec<String> = weapon
            .parts
            .iter()
            .map(|&p| {
                let c = &library.components[p];
                format!("{} ({})", c.display_name, library.rarities[c.tier].display_name)
            })
            .collect();
        let effects: Vec<&str> = weapon.profile.effects.iter().map(|e| library.effects[e.def].display_name.as_str()).collect();
        let synergies: Vec<&str> = weapon.profile.synergies.iter().map(|s| library.synergies[s.def].display_name.as_str()).collect();
        format!(
            "{} - {} {}, element {}: {}. Effects: {}. Synergies: {}.",
            weapon.name,
            library.rarities[weapon.grade].display_name,
            family.display_name,
            weapon.profile.element.map_or("none", Element::name),
            parts.join(", "),
            if effects.is_empty() { "none".into() } else { effects.join(", ") },
            if synergies.is_empty() { "none".into() } else { synergies.join(", ") },
        )
    }

    /// How the fight's going, for checking from scripts: the nearest animal to `near` that can be
    /// hunted, and the mana left
    pub fn report(&self, near: Vector3<f64>) -> String {
        let nearest = self.animals.iter().filter(|a| !a.look.is_pet()).min_by(|a, b| (a.position - near).norm().total_cmp(&(b.position - near).norm()));
        let animal = nearest.map_or("no animals".to_owned(), |a| format!("nearest {} at {:.0}/{:.0} health, tint {}", a.look.name(), a.health, a.look.health(), a.status.tint()));
        let held = self.held().map_or("nothing".to_owned(), |(_, w)| w.name.clone());
        let bosses = self.animals.iter().filter(|a| a.look.is_boss()).count();
        let boss = self.animals.iter().filter(|a| a.look.is_boss()).min_by(|a, b| (a.position - near).norm().total_cmp(&(b.position - near).norm()));
        let animal = match boss {
            Some(b) => format!(
                "{animal}, boss {} {:.0} m off at {:.0}/{:.0}{}",
                b.look.name(),
                (b.position - near).norm() / self.combat.vpm(),
                b.health,
                b.look.health(),
                if b.boss.as_ref().is_some_and(|s| s.engaged) { " fighting" } else { "" }
            ),
            None => animal,
        };
        format!(
            "holding {held}, {animal}, mana {:.0}, health {:.0}, gold {}, [{}] after them, {} rest spot, {} lying about, {} animals ({bosses} bosses, {} lairs), {} leaves falling",
            self.combat.mana,
            self.health,
            self.gold,
            self.animals.iter().filter(|a| a.health > 0.0 && a.boss.as_ref().is_some_and(|b| b.engaged)).map(|a| a.look.name()).collect::<Vec<_>>().join(", "),
            if self.rest.is_some() { "a" } else { "no" },
            self.pickups.iter().filter(|p| !matches!(p.item, Item::Apple)).count(),
            self.animals.len(),
            self.lairs.len(),
            self.leaves.count()
        )
    }

    pub fn clear(&mut self) {
        self.animals.clear();
        self.pickups.clear();
        self.props.clear();
        self.chests.clear();
        self.combat.clear();
        self.leaves.clear();
        self.target = None;
    }

    /// Candles, skulls and such: what they are, where in voxels and which way they face
    pub fn set_props(&mut self, props: Vec<(Look, Vector3<f64>, f64)>) {
        self.props = props;
    }

    pub fn set_chests(&mut self, chests: Vec<Chest>) {
        self.chests = chests;
        self.chests.extend(self.boss_chests.iter().cloned());
        self.target = None;
    }

    /// The bosses' lairs in the piece of the world just built, in its voxels: a boss turns up in each
    /// that isn't beaten and doesn't have one already
    pub fn set_lairs(&mut self, world: &World, water: WaterLevel, lairs: Vec<Lair>) {
        for lair in &lairs {
            // Not the monsters of a den at the same spot, like a warlord's bandits
            let here = self.animals.iter().any(|a| a.look.is_boss() && a.boss.as_ref().is_some_and(|b| b.key == lair.key));
            if here || self.defeated.contains(&lair.key) {
                continue;
            }
            let seed = hash(lair.key[0] as u32 ^ hash(lair.key[1] as u32));
            let look = lair.look.for_ground(ground_round(world, water, lair.position));
            if let Some(boss) = animals::spawn_boss(world, water, look, lair.position, lair.key, seed) {
                self.animals.push(boss);
            }
        }
        self.lairs = lairs;
    }

    /// The dungeon halls in the piece of the world just built, in its voxels: monsters haunt each
    /// that isn't cleared and doesn't have them already. Each hall has its own lot: the dead in
    /// their bones, rotting, or spiders, some in armor.
    pub fn set_dens(&mut self, world: &World, water: WaterLevel, dens: Vec<Den>) {
        for den in &dens {
            let here = self.animals.iter().any(|a| a.look.is_monster() && a.boss.as_ref().is_some_and(|b| b.key == den.key));
            if here || self.cleared.contains(&den.key) {
                continue;
            }
            let mut seed = hash(den.key[0] as u32 ^ hash(den.key[1] as u32 ^ 0xde4));
            let haunted: &[Look] = match seed % 3 {
                _ if den.kind == DenKind::Nest => &[Look::Spiderling, Look::Spiderling, Look::Spiderling],
                _ if den.kind == DenKind::Camp => &[Look::Bandit, Look::Bandit, Look::Bandit, Look::BanditBrute],
                0 => &[Look::Skeleton, Look::Skeleton, Look::ArmoredSkeleton],
                1 => &[Look::Zombie, Look::Zombie, Look::ArmoredZombie],
                _ => &[Look::Spiderling, Look::Spiderling, Look::Spiderling, Look::Spiderling],
            };
            for &look in haunted {
                seed = hash(seed);
                if let Some(mut monster) = animals::spawn_monster(world, water, look, den.position, 4.0, seed) {
                    if let Some(state) = &mut monster.boss {
                        state.key = den.key;
                    }
                    self.animals.push(monster);
                }
            }
        }
        self.dens = dens.iter().map(|d| d.key).collect();
    }

    /// For trying them out: boss `look` turns up `distance` meters along `ahead` from `near`
    pub fn summon(&mut self, world: &World, water: WaterLevel, look: Look, near: Vector3<f64>, ahead: Vector3<f64>, distance: f64) {
        let vpm = world.voxels_per_meter as f64;
        let flat = Vector3::new(ahead.x, ahead.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x());
        let at = near + flat * distance * vpm;
        let ground = (0..world.size[2] as i64).rev().find(|&z| !is_passable(world.material_at(at.x as i64, at.y as i64, z))).unwrap_or(0) as f64 + 1.0;
        let key = [at.x as i64, at.y as i64, -1];
        if let Some(boss) = animals::spawn_boss(world, water, look, Vector3::new(at.x, at.y, ground), key, self.rng) {
            self.animals.push(boss);
        }
    }

    /// The world moved on to a new piece of the landscape, `shift` voxels from the last: everything
    /// moves with it, what's now outside it goes, and new animals turn up to make up the numbers
    pub fn recenter(&mut self, world: &World, water: WaterLevel, shift: Vector3<f64>, seed: u32, near: Vector3<f64>) {
        let [sx, sy, _] = world.size.map(f64::from);
        let inside = |p: &Vector3<f64>| p.x >= 0.0 && p.y >= 0.0 && p.x < sx && p.y < sy;
        for animal in &mut self.animals {
            animal.position -= shift;
        }
        self.animals.retain(|a| inside(&a.position));
        for animal in &mut self.animals {
            if let Some(boss) = &mut animal.boss {
                boss.recenter(shift);
            }
        }
        for root in &mut self.roots {
            root.at -= shift;
        }
        for (at, _) in &mut self.webs {
            *at -= shift;
        }
        for pickup in &mut self.pickups {
            pickup.position -= shift;
        }
        self.pickups.retain(|p| inside(&p.position));
        for chest in &mut self.boss_chests {
            chest.position -= shift;
        }
        self.boss_chests.retain(|c| inside(&c.position));
        self.knockback = Vector3::zeros();
        self.thrown = Vector3::zeros();
        self.combat.recenter(shift);
        self.leaves.recenter(shift);
        self.target = None;
        self.merchants.retain(|(id, _)| self.animals.iter().any(|a| a.id == *id));
        self.forget_weapons();
        self.rest = self.rest.map(|r| r - shift).filter(inside);

        // Animals and wolves wander in to make up the numbers, away from the player so they don't
        // pop up right in front of them. Bosses, dungeon monsters, merchants and villagers have
        // homes of their own.
        let missing = ANIMALS.saturating_sub(self.animals.iter().filter(|a| a.look.build().is_none() && a.look != Look::Merchant && !a.look.is_villager()).count());
        let wolves = self.animals.iter().filter(|a| a.look == Look::Wolf).count();
        let bandits = self.animals.iter().filter(|a| matches!(a.look, Look::Bandit | Look::BanditBrute)).count();
        if missing > 0 || wolves < 3 || bandits < 3 {
            let vpm = world.voxels_per_meter as f64;
            let fresh = animals::spawn(world, water, seed ^ hash(self.rng), near).into_iter().filter(|a| (a.position - near).norm() > 40.0 * vpm);
            let mut taken = 0;
            for animal in fresh {
                let wanted = match animal.look {
                    Look::Wolf => wolves < 3,
                    Look::Bandit | Look::BanditBrute => bandits < 3,
                    _ if animal.look.build().is_none() => {
                        taken += 1;
                        taken <= missing
                    }
                    _ => false,
                };
                if wanted {
                    self.animals.push(animal);
                }
            }
        }
        self.rng = hash(self.rng);
    }

    /// The nearest animal, food or prop called `name` to `near`: where it stands, and how high its
    /// middle is in meters
    pub fn find_animal(&self, name: &str, near: Vector3<f64>) -> Option<(Vector3<f64>, f64)> {
        let animals = self.animals.iter().map(|a| (a.look, a.position));
        let pickups = self.pickups.iter().filter_map(|p| Some((p.item.look()?, p.position)));
        let props = self.props.iter().map(|&(look, position, _)| (look, position));
        let chests = self.chests.iter().map(|c| (Look::Chest, c.position));
        animals
            .chain(pickups)
            .chain(props)
            .chain(chests)
            .filter(|(look, _)| look.name() == name)
            .min_by(|a, b| (a.1 - near).norm().total_cmp(&(b.1 - near).norm()))
            .map(|(look, position)| (position, look.body().0))
    }

    fn apple_count(&self) -> usize {
        self.pickups.iter().filter(|p| p.item == Item::Apple).count()
    }

    /// Tries one random spot within reach of `near` for a new apple
    fn grow_apple(&mut self, world: &World, water: WaterLevel, near: Vector3<f64>) {
        let vpm = world.voxels_per_meter as f64;
        let angle = random(&mut self.rng) * TAU;
        let distance = random(&mut self.rng).sqrt() * FOOD_RANGE * 0.8 * vpm;
        let (x, y) = (near.x + angle.cos() * distance, near.y + angle.sin() * distance);
        if x < 0.0 || y < 0.0 || x >= world.size[0] as f64 || y >= world.size[1] as f64 {
            return;
        }
        if let Some(apple) = apple_at(world, water, x, y, &mut self.rng) {
            self.pickups.push(apple);
        }
    }

    /// The sounds to play since last time, the fighting's too
    pub fn take_cues(&mut self) -> Vec<Cue> {
        let mut cues = std::mem::take(&mut self.cues);
        cues.append(&mut self.combat.cues);
        cues
    }

    /// How many leaves are falling around, to tell if there are trees about
    pub fn leaves_falling(&self) -> usize {
        self.leaves.count()
    }

    /// Where explosions want holes blown in the world since last time, and how big, in meters
    pub fn take_craters(&mut self) -> Vec<(Vector3<f64>, f64)> {
        std::mem::take(&mut self.combat.craters)
    }

    /// Stains splashed on the ground since last time: where, how big in meters, of what, and
    /// whether they burn off what grows there
    pub fn take_stains(&mut self) -> Vec<(Vector3<f64>, f64, u8, bool)> {
        std::mem::take(&mut self.combat.stains)
    }

    /// Chunks of what an explosion at `at` blew out of the world flying out, and dirt splashed up
    /// round it
    pub fn debris(&mut self, at: Vector3<f64>, materials: &[u8], vpm: f64) {
        self.leaves.debris(at, materials, vpm);
        self.combat.stir(at, 1.2, 0.7, DROP_MUD);
    }

    pub fn say(&mut self, message: impl Into<String>) {
        self.messages.push((message.into(), Instant::now()));
        if self.messages.len() > 4 {
            self.messages.remove(0);
        }
    }

    /// `player` is where the player stands, None while flying. `view` is where the camera is and
    /// which way it looks, and `controls` what the mouse buttons do with a weapon in hand.
    pub fn update(&mut self, world: &World, water: WaterLevel, player: Option<Vector3<f64>>, view: &View, controls: Controls, dt: f64) {
        for animal in &mut self.animals {
            animal.update(world, water, player, dt);
        }

        // The dead don't fight
        let held = match self.slots.get(self.selected) {
            Some(&(Item::Weapon(i), _)) if self.dead.is_none() => Some(i),
            _ => None,
        };
        let controls = if self.dead.is_some() { Controls::default() } else { controls };
        let weapon = held.map(|i| (i, &self.weapons[i]));
        self.combat.update(&mut Arena { world, animals: &mut self.animals, view, player, water }, weapon, controls, dt);
        for message in std::mem::take(&mut self.combat.messages) {
            self.say(message);
        }

        // What the bosses did, and what they threw landing
        let mut deeds = Vec::new();
        for animal in &mut self.animals {
            if let Some(boss) = &mut animal.boss {
                deeds.extend(boss.deeds.drain(..).map(|d| (animal.look, d)));
            }
        }
        for (look, deed) in deeds {
            self.carry_out(look, deed, world, water, player, view);
        }
        self.grow_roots(world, player, view, dt);
        self.spin_webs(player, dt);
        for (damage, from, element) in std::mem::take(&mut self.combat.player_hits) {
            if let Some(feet) = player {
                self.hurt(damage, from, feet, world, "a blast");
                self.afflict(element);
            }
        }
        // What a special attack did for the player
        let heal = std::mem::take(&mut self.combat.heal);
        if heal > 0.0 && self.health > 0.0 {
            self.health = (self.health + heal).min(MAX_HEALTH);
        }
        self.knockback += std::mem::take(&mut self.combat.dash);
        self.look_after_player(world, water, player, dt);
        // The boss bar's trail catching up with what it's lost, a moment after each blow
        if let Some((foe, health, ..)) = self.foe(view.eye) {
            let name = foe.name();
            let (shown, mut trail, mut hold, mut flash) = if name == self.boss_bar.0 { (self.boss_bar.1, self.boss_bar.2, self.boss_bar.3, self.boss_bar.4) } else { (health, health, 0.0, 0.0) };
            if health < shown - 0.03 {
                flash = 1.0;
            }
            if health < shown {
                hold = 0.45;
            }
            hold -= dt;
            if hold <= 0.0 {
                trail -= 0.6 * dt;
            }
            self.boss_bar = (name, health, trail.max(health), hold, (flash - dt / 0.18).max(0.0));
        }
        self.reap(world);
        self.leaves.update(world, water, view.eye, dt);
        self.look_over_loot(world, player, view, dt);

        if player.is_some() {
            let before = self.hunger;
            self.hunger = (self.hunger - 100.0 * dt / HUNGER_SECONDS).max(0.0);
            if before > 25.0 && self.hunger <= 25.0 {
                self.say("You're getting hungry. Find something to eat.");
            }
            if before > 0.0 && self.hunger <= 0.0 {
                self.say("You're starving!");
            }
        }

        // Apples grow back now and then
        self.apple_timer -= dt;
        if self.apple_timer <= 0.0 {
            self.apple_timer = 4.0;
            for _ in 0..20 {
                if self.apple_count() >= APPLES {
                    break;
                }
                self.grow_apple(world, water, view.eye);
            }
        }

        self.target = if self.dead.is_some() { None } else { self.find_target(world, view.eye, view.forward()) };
        self.messages.retain(|(_, at)| at.elapsed().as_secs_f64() < 4.0);
    }

    /// What a boss or monster did: hurting the player, throwing things, sounds and shaking the ground
    fn carry_out(&mut self, look: Look, deed: Deed, world: &World, water: WaterLevel, player: Option<Vector3<f64>>, view: &View) {
        let vpm = world.voxels_per_meter as f64;
        // How much something at `at` is felt, from 1 right there to nothing 40 meters off
        let felt = |at: Vector3<f64>| (1.0 - (at - view.eye).norm() / vpm / 40.0).max(0.0);
        // Monsters snarl and rattle rather than roar, and don't shake the ground
        if look.is_monster() {
            match deed {
                Deed::WindUp(_, at) | Deed::Roar { at, .. } => {
                    let cue = match look {
                        Look::Wolf => Cue::at(Sound::Growl, at, 0.7),
                        Look::Skeleton | Look::ArmoredSkeleton => Cue::at(Sound::Rattle, at, 0.7),
                        Look::Zombie | Look::ArmoredZombie => Cue::at(Sound::Growl, at, 0.6).pitched(0.6),
                        // A bandit's yell
                        Look::Bandit | Look::BanditBrute => Cue::at(Sound::Roar, at, 0.45).pitched(if look == Look::Bandit { 1.9 } else { 1.5 }),
                        _ => Cue::at(Sound::Hiss, at, 0.5).pitched(1.5),
                    };
                    self.cues.push(cue);
                    return;
                }
                Deed::Stomp(_) => return,
                _ => {}
            }
        }
        let voice = match look.build() {
            Some(Build::Golem) => 0.6,
            Some(Build::Drake) => 1.15,
            Some(Build::Spider) => 1.5,
            Some(Build::Treant) => 0.5,
            _ => 0.85,
        };
        let element = look.element();
        match deed {
            // Each move sounds its own way coming, so it can be told apart
            Deed::WindUp(m, at) => {
                let cue = match m {
                    Move::Charge => Cue::at(Sound::Roar, at, 0.7).pitched(voice * 1.3),
                    Move::Leap => Cue::at(Sound::Roar, at, 0.6).pitched(voice * 1.6),
                    Move::Breath => Cue::at(Sound::Rumble, at, 0.9).pitched(voice * 1.6),
                    // A spider hissing, or something gathering in a drake's throat in the voice of
                    // its element
                    Move::Spit | Move::Web if look.build() == Some(Build::Spider) => Cue::at(Sound::Hiss, at, 0.6).pitched(if m == Move::Web { 0.9 } else { 1.1 }),
                    Move::Spit => Cue::at(Sound::Cast(Element::look(Some(element.unwrap_or(Element::Fire))) as u8), at, 0.8).pitched(0.45),
                    // Wood creaking as it bends back, like a bow
                    Move::Volley => Cue::at(Sound::Creak, at, 0.8).pitched(1.0),
                    // Deep down, under the ground, and old wood groaning
                    Move::Roots => Cue::at(Sound::Rumble, at, 1.0).pitched(voice * 0.7),
                    Move::RootWave => Cue::at(Sound::Creak, at, 0.8).pitched(0.7),
                    Move::Quake => Cue::at(Sound::Rumble, at, 1.0).pitched(voice * 0.5),
                    // Its wings beating as it rises up to call the storm
                    Move::Storm => {
                        self.cues.push(Cue::at(Sound::Cast(Element::look(Some(element.unwrap_or(Element::Fire))) as u8), at, 0.8).pitched(0.3));
                        Cue::at(Sound::Flap, at, 0.8).pitched(0.85)
                    }
                    Move::Maul => Cue::at(Sound::Roar, at, 0.5).pitched(voice * 1.8),
                    _ => Cue::at(Sound::Rumble, at, 0.9).pitched(voice * 1.2),
                };
                self.cues.push(cue);
            }
            Deed::Roar { at, angry } => {
                self.cues.push(Cue::at(Sound::Roar, at, 1.0).pitched(voice * if angry { 1.1 } else { 1.0 }));
                // A drake spreads its wings as it roars
                if look.build() == Some(Build::Drake) {
                    self.cues.push(Cue::at(Sound::Flap, at, 0.8).pitched(0.8));
                }
                self.shake = self.shake.max(0.6 * felt(at));
                self.say(if angry { format!("The {} is enraged!", look.name()) } else { format!("The {} turns on you!", look.name()) });
            }
            Deed::Slam { at, radius, damage, crater } => {
                if crater > 0.0 {
                    self.combat.craters.push((at, crater));
                    // Frost spreading out from it, the ground scorched, or venom splashed
                    match element {
                        Some(Element::Frost) => self.combat.stains.push((at, radius * 1.1, SNOW, false)),
                        Some(Element::Fire) => self.combat.stains.push((at, radius * 0.9, BARK_DARK, true)),
                        Some(Element::Poison) => self.combat.stains.push((at, radius * 0.8, MOSS, false)),
                        _ => {}
                    }
                }
                // A monster's bite or blade, or a boss's blow shaking the ground
                let cue = match look {
                    Look::Wolf | Look::Spiderling => Cue::at(Sound::Bite, at, 0.8),
                    Look::ArmoredSkeleton | Look::ArmoredZombie | Look::BanditBrute => Cue::at(Sound::Clang, at, 0.5).pitched(0.8),
                    _ if look.is_monster() => Cue::at(Sound::Swing, at, 0.7).pitched(0.8),
                    _ => {
                        self.shake = self.shake.max(0.6 * felt(at));
                        Cue::at(Sound::Burst, at, 1.0).pitched(0.45)
                    }
                };
                self.cues.push(cue);
                if let Some(feet) = player {
                    let off = feet - at;
                    let d = off.xy().norm() / vpm;
                    if d < radius && off.z.abs() < 2.5 * vpm {
                        self.hurt(damage * (1.0 - 0.5 * d / radius), at, feet, world, look.name());
                        self.afflict(element);
                    } else if damage > 0.0 && d < radius + 1.0 && off.z.abs() < 2.5 * vpm {
                        self.near_miss(at);
                    }
                }
            }
            Deed::Throw { from, to } => {
                let time = ((to - from).norm() / vpm / 16.0).clamp(0.8, 1.6);
                self.combat.hurl(HostileKind::Boulder(element), from, to, time, 28.0, 2.4);
                self.cues.push(Cue::at(Sound::HeavySwing, from, 1.0).pitched(0.5));
            }
            Deed::Spit { from, to } => {
                let element = element.unwrap_or(Element::Fire);
                // Venom's lobbed, slower, falling onto where the player was
                let pace = if element == Element::Poison { 15.0 } else { 22.0 };
                let time = ((to - from).norm() / vpm / pace).max(0.2);
                let (damage, radius) = if element == Element::Poison { (16.0, 2.4) } else { (24.0, 2.6) };
                self.combat.hurl(HostileKind::Spell(element), from, to, time, damage, radius);
                self.cues.push(Cue::at(Sound::Cast(Element::look(Some(element)) as u8), from, 1.0).pitched(0.6));
            }
            Deed::Breath { from, direction, dt: seconds } => {
                let element = element.unwrap_or(Element::Fire);
                self.combat.breathe(from, direction, seconds, element);
                if self.breath_sound <= 0.0 {
                    self.breath_sound = 1.8;
                    self.cues.push(Cue::at(Sound::Breath, from, 1.0).pitched(if element == Element::Fire { 1.0 } else { 1.25 }));
                }
                // Caught in it: burnt, poisoned, or frozen through, which hurts the more there and then
                if let Some(feet) = player {
                    let to = feet + Vector3::new(0.0, 0.0, 1.0 * vpm) - from;
                    let along = to.dot(&direction);
                    if along > 0.0 && along < 10.0 * vpm && (to - direction * along).norm() < 0.6 * vpm + along * 0.18 {
                        let damage = if element == Element::Frost { 30.0 } else { 22.0 };
                        self.hurt(damage * seconds, from, feet, world, look.name());
                        self.afflict(Some(element));
                    }
                }
            }
            Deed::Mark { at, radius, closing } => self.combat.mark(at, radius, closing),
            // The ground stirring under the player, then roots bursting up out of it
            Deed::Roots { at } => {
                // On the ground under the player, if they're jumping
                let at = ground_under(world, at);
                self.roots.push(Root { at, left: ROOTS_WARNING, radius: ROOTS_RADIUS, damage: 24.0, drag: Vector3::zeros(), element: None });
                self.cues.push(Cue::at(Sound::Rumble, at, 0.8).pitched(0.7));
            }
            // Lines of roots, each bursting up a little after the one before, dragging the player
            // back towards it
            Deed::RootWave { from, to, lanes } => {
                let off = to - from;
                let ahead = Vector3::new(off.x, off.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                for lane in 0..lanes {
                    let turn = (lane as f64 - (lanes - 1) as f64 * 0.5) * 0.35;
                    let (c, s) = (turn.cos(), turn.sin());
                    let along = Vector3::new(ahead.x * c - ahead.y * s, ahead.x * s + ahead.y * c, 0.0);
                    for k in 0..WAVE_ROOTS {
                        let at = ground_under(world, from + along * (3.0 + WAVE_SPACING * k as f64) * vpm + Vector3::new(0.0, 0.0, 1.0 * vpm));
                        self.roots.push(Root { at, left: 0.8 + 0.15 * k as f64, radius: WAVE_RADIUS, damage: 16.0, drag: -along, element });
                    }
                }
                self.cues.push(Cue::at(Sound::Rumble, from, 1.0).pitched(0.5));
            }
            // The ground pounded, and a shockwave running out from it that has to be jumped
            Deed::Quake { at, from, to, dt: seconds } => {
                let (debris, stain) = match element {
                    Some(Element::Frost) => (DROP_ICE, Some((SNOW, false))),
                    Some(Element::Fire) => (DROP_EMBER, Some((BARK_DARK, true))),
                    _ => (DROP_MUD, None),
                };
                if from <= 0.0 {
                    self.combat.craters.push((at, 1.4));
                    if let Some((material, burn)) = stain {
                        self.combat.stains.push((at, 3.5, material, burn));
                    }
                    self.cues.push(Cue::at(Sound::Quake, at, 1.0).pitched(0.9));
                    self.shake = self.shake.max(0.9 * felt(at));
                }
                // Its front, a band of churned-up ground, rings either side of it
                self.combat.mark(at, to, ((to - 0.8) / to).max(0.0));
                self.combat.stir(at, to, seconds * to * 3.0, debris);
                self.shake = self.shake.max(0.25 * felt(at));
                if let Some(feet) = player {
                    let off = feet - at;
                    let d = off.xy().norm() / vpm;
                    // Up in the air, it passes under
                    let airborne = (1..=(0.3 * vpm) as i64).all(|k| is_passable(world.material_at(feet.x.floor() as i64, feet.y.floor() as i64, feet.z.floor() as i64 - k)));
                    if (from..to).contains(&d) && off.z.abs() < 3.0 * vpm {
                        if airborne {
                            // Jumped clean over it
                            self.near_miss(feet);
                        } else {
                            self.hurt(22.0, at, feet, world, look.name());
                            self.thrown += Vector3::new(0.0, 0.0, 3.0 * vpm);
                            self.afflict(element);
                        }
                    }
                }
            }
            // Its element coming down out of the sky: the first right on the player, the rest
            // round them
            Deed::Storm { to, k } => {
                let e = element.unwrap_or(Element::Fire);
                let mut to = to;
                if k > 0 {
                    let a = random(&mut self.rng) * TAU;
                    let r = (1.5 + 2.5 * random(&mut self.rng)) * vpm;
                    to += Vector3::new(a.cos() * r, a.sin() * r, 0.0);
                }
                let to = ground_under(world, to);
                let a = random(&mut self.rng) * TAU;
                let from = to + Vector3::new(a.cos() * 3.0, a.sin() * 3.0, STORM_HEIGHT) * vpm;
                let damage = if e == Element::Frost { 13.0 } else { 16.0 };
                self.combat.hurl(HostileKind::Spell(e), from, to, 1.0, damage, 2.0);
                self.cues.push(Cue::at(Sound::Whistle, to, 0.6).pitched(0.85 + 0.1 * random(&mut self.rng) as f32));
            }
            // A raid boss bringing the sky down: molten rock, fire or the void falling all over, the
            // first right on the player, the rest anywhere round them, each with its ring showing
            Deed::Cataclysm { to, k } => {
                let kind = match look {
                    Look::Colossus => HostileKind::Boulder(Some(Element::Fire)),
                    _ => HostileKind::Spell(element.unwrap_or(Element::Fire)),
                };
                let mut to = to;
                if k > 0 {
                    let a = random(&mut self.rng) * TAU;
                    let r = (2.0 + 9.0 * random(&mut self.rng).sqrt()) * vpm;
                    to += Vector3::new(a.cos() * r, a.sin() * r, 0.0);
                }
                let to = ground_under(world, to + Vector3::new(0.0, 0.0, 2.0 * vpm));
                let a = random(&mut self.rng) * TAU;
                let from = to + Vector3::new(a.cos() * 4.0, a.sin() * 4.0, STORM_HEIGHT + 6.0) * vpm;
                self.combat.hurl(kind, from, to, 1.3, 18.0, 2.4);
                if k % 3 == 0 {
                    self.cues.push(Cue::at(Sound::Whistle, to, 0.7).pitched(0.7 + 0.15 * random(&mut self.rng) as f32));
                }
                self.shake = self.shake.max(0.25 * felt(to));
            }
            // A raid boss going into its next phase: a roar that shakes the ground, and word of it
            Deed::Phase { at, stage } => {
                self.cues.push(Cue::at(Sound::Roar, at, 1.0).pitched(voice * 0.8));
                self.cues.push(Cue::at(Sound::Quake, at, 0.8).pitched(0.7));
                self.shake = self.shake.max(0.9 * felt(at));
                self.combat.craters.push((at, 1.5 * look.size()));
                self.say(if stage >= 2 { format!("The {} is enraged! Its last fury is upon you.", look.name()) } else { format!("The {} calls down the sky!", look.name()) });
            }
            Deed::Swipe(at, k) => self.cues.push(Cue::at(Sound::Claw, at, 0.85).pitched(0.9 + 0.1 * k as f32)),
            // A pack answering a howl, the dead clawing up out of the ground, spiderlings swarming
            // out: never more than a few of them about at once
            Deed::Summon { at, look: brood, count } => {
                let most = match brood {
                    Look::Spiderling | Look::Bandit => 4,
                    Look::Skeleton => 3,
                    Look::ArmoredSkeleton => 4,
                    _ => 2,
                };
                let near = self.animals.iter().filter(|a| a.look == brood && a.health > 0.0 && (a.position - at).norm() < 30.0 * vpm).count();
                let count = count.min(most - near.min(most));
                for _ in 0..count {
                    self.rng = hash(self.rng);
                    if let Some(mut monster) = animals::spawn_monster(world, water, brood, at, 7.0, self.rng) {
                        if let Some(state) = &mut monster.boss {
                            state.provoke();
                        }
                        self.combat.stir(monster.position, 0.8, 0.4, DROP_MUD);
                        self.animals.push(monster);
                    }
                }
                let cue = match brood {
                    Look::Wolf => Cue::at(Sound::Howl, at, 1.0).pitched(voice * 0.9),
                    Look::Skeleton => Cue::at(Sound::Rattle, at, 1.0).pitched(0.8),
                    Look::Bandit => Cue::at(Sound::Roar, at, 0.9).pitched(1.5),
                    _ => Cue::at(Sound::Skitter, at, 1.0),
                };
                self.cues.push(cue);
                if count > 0 {
                    let deed = match brood {
                        Look::Wolf => "howls for its pack",
                        Look::Skeleton => "raises the dead",
                        Look::Bandit => "calls the gang to arms",
                        _ => "calls its brood",
                    };
                    self.say(format!("The {} {deed}!", look.name()));
                }
            }
            Deed::Web { from, to } => {
                let time = ((to - from).norm() / vpm / 16.0).clamp(0.6, 1.3);
                self.combat.hurl(HostileKind::Web, from, to, time, 6.0, WEB_RADIUS);
                self.cues.push(Cue::at(Sound::Web, from, 0.7).pitched(1.1));
            }
            // A spray of thorns, spread so it's hard to get out of the way of them all
            Deed::Volley { from, to } => {
                let off = to - from;
                let side = Vector3::new(-off.y, off.x, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::x);
                let time = (off.norm() / vpm / 22.0).max(0.2);
                for k in 0..THORNS {
                    let across = (k as f64 / (THORNS - 1) as f64 - 0.5) * 2.0;
                    let spread = side * across * 2.2 * vpm + Vector3::new(0.0, 0.0, (random(&mut self.rng) - 0.5) * 0.8 * vpm);
                    self.combat.hurl(HostileKind::Thorn, from, to + spread, time * (0.95 + 0.1 * random(&mut self.rng)), 9.0, 0.6);
                }
                self.cues.push(Cue::at(Sound::Twang, from, 1.0).pitched(0.5));
            }
            Deed::Ram { damage, direction } => {
                if let Some(feet) = player {
                    self.hurt(damage, feet - direction * vpm, feet, world, look.name());
                    self.thrown += (direction * 5.0 + Vector3::new(0.0, 0.0, 2.5)) * vpm;
                }
                self.cues.push(Cue::here(Sound::Hit, 1.0).pitched(0.5));
            }
            Deed::Stomp(at) => {
                let near = felt(at);
                if near > 0.0 {
                    self.shake = self.shake.max(0.35 * near * near);
                    self.cues.push(Cue::at(Sound::Step(Ground::Stone), at, 0.9).pitched(0.4));
                }
            }
        }
    }

    /// Something at `at` only just missing the player: a whoosh past them, now and then
    fn near_miss(&mut self, at: Vector3<f64>) {
        if self.dodge_sound <= 0.0 && self.health > 0.0 {
            self.dodge_sound = 1.0;
            self.rng = hash(self.rng);
            self.cues.push(Cue::at(Sound::Dodge, at, 0.7).pitched(0.9 + 0.2 * random(&mut self.rng) as f32));
        }
    }

    /// Something of `element` that hurt the player going on hurting them: fire burns, venom
    /// poisons, and frost slows them down
    fn afflict(&mut self, element: Option<Element>) {
        if self.health <= 0.0 {
            return;
        }
        match element {
            Some(Element::Fire) => self.burning = self.burning.max(2.0),
            Some(Element::Poison) => self.poisoned = self.poisoned.max(4.0),
            Some(Element::Frost) => self.chilled = self.chilled.max(2.5),
            _ => {}
        }
    }

    /// How fast the player can move next to usual, slowed while chilled or stuck in web
    pub fn pace(&self) -> f64 {
        let chilled = if self.chilled > 0.0 { CHILLED_PACE } else { 1.0 };
        if self.snared > 0.0 { chilled.min(SNARED_PACE) } else { chilled }
    }

    /// The ground stirring where roots are coming up, and them bursting up, throwing the player
    /// up if they're still on it, or dragging them along
    fn grow_roots(&mut self, world: &World, player: Option<Vector3<f64>>, view: &View, dt: f64) {
        let vpm = world.voxels_per_meter as f64;
        for mut root in std::mem::take(&mut self.roots) {
            root.left -= dt;
            let at = root.at;
            let near = (1.0 - (at - view.eye).norm() / vpm / 30.0).max(0.0);
            if root.left > 0.0 {
                // Stirring harder and shaking more as it comes, a ring closing in on where
                let closing = (root.left / ROOTS_WARNING).min(1.0);
                self.combat.stir(at, root.radius, dt * (2.0 - closing), DROP_MUD);
                self.combat.mark(at, root.radius, closing);
                self.shake = self.shake.max((0.1 + 0.15 * (1.0 - closing)) * near);
                self.roots.push(root);
                continue;
            }
            self.combat.erupt(at, root.radius);
            self.shake = self.shake.max(0.5 * near);
            if let Some(feet) = player {
                let off = feet - at;
                if off.xy().norm() < root.radius * vpm && off.z.abs() < 2.0 * vpm {
                    self.hurt(root.damage, at, feet, world, "roots");
                    let up = if root.drag == Vector3::zeros() { 5.5 } else { 3.0 };
                    self.thrown += (root.drag * 4.0 + Vector3::new(0.0, 0.0, up)) * vpm;
                    self.afflict(root.element);
                }
            }
        }
    }

    /// Ground left sticky by web, holding up whoever's in it
    fn spin_webs(&mut self, player: Option<Vector3<f64>>, dt: f64) {
        let vpm = self.combat.vpm();
        for at in std::mem::take(&mut self.combat.webs) {
            self.webs.push((at, WEB_SECONDS));
            if self.webs.len() > WEBS {
                self.webs.remove(0);
            }
        }
        self.snared = (self.snared - dt).max(0.0);
        for (at, left) in &mut self.webs {
            *left -= dt;
            self.combat.mark(*at, WEB_RADIUS, 0.0);
            self.combat.stir(*at, WEB_RADIUS, dt * 0.5, DROP_WATER);
            if let Some(feet) = player {
                let off = feet - *at;
                if off.xy().norm() < WEB_RADIUS * vpm && off.z.abs() < 2.0 * vpm {
                    self.snared = self.snared.max(0.3);
                }
            }
        }
        self.webs.retain(|(_, left)| *left > 0.0);
    }

    /// The player at `feet` takes `damage` from something at `from`, `by` what, thrown back from it
    fn hurt(&mut self, damage: f64, from: Vector3<f64>, feet: Vector3<f64>, world: &World, by: &str) {
        if self.health <= 0.0 || damage <= 0.0 {
            return;
        }
        // Turned aside by a riposte
        if self.combat.parry(from, feet) {
            return;
        }
        let vpm = world.voxels_per_meter as f64;
        self.health -= damage;
        self.since_hurt = 0.0;
        self.hurt_flash = (self.hurt_flash + damage / 25.0).min(1.0);
        // On top of the shaking from whatever it was going off, so being hit feels worse than
        // getting out of the way; only now and then, for what goes on hurting like breath
        let jolt = (0.3 + damage / 40.0).min(1.0);
        self.shake = if self.hurt_sound <= 0.0 { (self.shake + jolt).min(1.4) } else { self.shake.max(jolt) };
        if self.hurt_sound <= 0.0 {
            self.hurt_sound = 0.35;
            self.cues.push(Cue::here(Sound::Hurt, (0.5 + damage / 40.0).min(1.0) as f32));
        }
        // Blood splashed over the view, more and bigger the harder the blow, round the edges clear
        // of the middle
        if damage >= 6.0 {
            for _ in 0..((damage / 14.0).ceil() as usize).min(4) {
                self.rng = hash(self.rng);
                let mut seed = self.rng;
                let a = random(&mut seed) * TAU;
                let r = 0.3 + 0.14 * random(&mut seed);
                let at = [(0.5 + a.cos() * r) as f32, (0.5 + a.sin() * r * 0.85) as f32];
                let size = ((0.03 + damage / 1000.0).min(0.07) * (0.7 + 0.6 * random(&mut seed))) as f32;
                self.splats.push(Splat { at, size, age: 0.0, seed });
            }
            if self.splats.len() > 10 {
                let extra = self.splats.len() - 10;
                self.splats.drain(..extra);
            }
        }
        // Knocked back a step or two, not flung
        let away = Vector3::new(feet.x - from.x, feet.y - from.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros);
        self.thrown += (away * (damage * 0.12).min(3.5) + Vector3::new(0.0, 0.0, (damage * 0.05).min(1.5))) * vpm;
        if self.health <= 0.0 {
            self.die(by, feet, vpm);
        }
    }

    /// The player at `feet` is killed by `by`. Half the gold they carry spills where they fell, to
    /// come back for, and every boss and monster goes home healed.
    fn die(&mut self, by: &str, feet: Vector3<f64>, vpm: f64) {
        self.health = 0.0;
        self.dead = Some((by.to_owned(), 0.0));
        self.died = true;
        self.shop = None;
        let lost = self.gold / 2;
        self.lost = lost;
        if lost > 0 {
            self.gold -= lost;
            let from = feet + Vector3::new(0.0, 0.0, 1.0 * vpm);
            self.pickups.push(Pickup { item: Item::Gold(lost), position: feet, heading: 0.0, from, flying: DROP_SECONDS, seen: false, beacon: true });
        }
        self.say(if lost > 0 { format!("You were slain by the {by}. {lost} gold lies where you fell.") } else { format!("You were slain by the {by}.") });
        self.shake = self.shake.max(1.2);
        self.cues.push(Cue::here(Sound::Hurt, 1.0).pitched(0.7));
        self.cues.push(Cue::here(Sound::Rumble, 0.8).pitched(0.5));
        for animal in &mut self.animals {
            if let Some(boss) = &mut animal.boss {
                boss.spare();
                boss.stage = 0;
                animal.health = animal.look.health();
                // Burning or bleeding from the fight would turn it on them again
                animal.status = Default::default();
            }
        }
        self.combat.clear();
        self.roots.clear();
        self.webs.clear();
        self.burning = 0.0;
        self.poisoned = 0.0;
        self.chilled = 0.0;
        self.snared = 0.0;
    }

    pub fn dead(&self) -> bool {
        self.dead.is_some()
    }

    /// How far the view has sunk to the ground as the player fell dead, 0 to 1
    pub fn fallen(&self) -> f64 {
        self.dead.as_ref().map_or(0.0, |(_, since)| {
            let t = (since / 0.9).min(1.0);
            t * t * (3.0 - 2.0 * t)
        })
    }

    /// Whether the player's lain dead long enough to rise again
    pub fn can_rise(&self) -> bool {
        self.dead.as_ref().is_some_and(|(_, since)| *since >= RISE_AFTER)
    }

    /// Seconds the player's been dead, 0 if they're alive
    pub fn dead_for(&self) -> f64 {
        self.dead.as_ref().map_or(0.0, |(_, since)| *since)
    }

    /// The player rises again, healed: at the last place they stood safe, their feet in voxels,
    /// or None for where they fell
    pub fn rise(&mut self) -> Option<Vector3<f64>> {
        self.dead = None;
        self.health = MAX_HEALTH;
        self.hunger = self.hunger.max(40.0);
        self.combat.mana = MAX_MANA;
        self.down = 0.0;
        self.since_hurt = 100.0;
        self.splats.clear();
        self.hurt_flash = 0.0;
        self.thrown = Vector3::zeros();
        self.knockback = Vector3::zeros();
        self.target = None;
        self.say("You rise again.");
        self.rest
    }

    /// Whether the player died since last asked
    pub fn take_died(&mut self) -> bool {
        std::mem::take(&mut self.died)
    }

    /// Health coming back, burning, starving, and keeping track of where's safe to rise again
    fn look_after_player(&mut self, world: &World, water: WaterLevel, player: Option<Vector3<f64>>, dt: f64) {
        self.since_hurt += dt;
        self.hurt_flash = (self.hurt_flash - dt * 1.5).max(0.0);
        self.down = (self.down + dt).min(DOWN_SECONDS);
        if let Some((_, since)) = &mut self.dead {
            *since += dt;
        }
        for s in &mut self.splats {
            s.age += dt;
        }
        self.splats.retain(|s| s.age < SPLAT_SECONDS);
        self.hurt_sound -= dt;
        self.breath_sound -= dt;
        self.dodge_sound -= dt;
        for at in std::mem::take(&mut self.combat.near_misses) {
            self.near_miss(at);
        }
        self.shake *= (-6.0 * dt).exp();
        self.chilled = (self.chilled - dt).max(0.0);
        let Some(feet) = player else {
            self.burning = 0.0;
            self.poisoned = 0.0;
            self.chilled = 0.0;
            self.snared = 0.0;
            self.roots.clear();
            self.webs.clear();
            self.safe_for = 0.0;
            return;
        };
        if self.dead.is_some() {
            return;
        }
        // Remembering where's safe to rise again: standing on the ground out of the water, with
        // nothing after them, for a few seconds
        let vpm = world.voxels_per_meter as f64;
        let fighting = self.animals.iter().any(|a| a.health > 0.0 && a.boss.as_ref().is_some_and(|b| b.engaged));
        // The ground's within a few voxels under the middle of their feet, as they stand on the
        // highest of what's under any of them
        let standing = (1..=(0.4 * vpm) as i64).any(|d| !is_passable(world.material_at(feet.x.floor() as i64, feet.y.floor() as i64, feet.z.floor() as i64 - d)));
        let dry = water(feet.x, feet.y) < feet.z + 0.3 * vpm;
        if fighting || !standing || !dry || self.health < 30.0 {
            self.safe_for = 0.0;
        } else {
            self.safe_for += dt;
            if self.safe_for > 3.0 {
                self.rest = Some(feet);
            }
        }
        for (poison, rate, by) in [(false, 5.0, "flames"), (true, 3.5, "venom")] {
            let left = if poison { &mut self.poisoned } else { &mut self.burning };
            if *left <= 0.0 {
                continue;
            }
            *left -= dt;
            self.health = (self.health - rate * dt).max(0.0);
            self.hurt_flash = self.hurt_flash.max(0.15);
            if self.health <= 0.0 {
                self.health = 1e-6;
                self.hurt(1.0, feet, feet, world, by);
            }
        }
        if self.since_hurt > 5.0 {
            self.health = (self.health + 4.0 * dt).min(MAX_HEALTH);
        }
        // Starving wears the player down, though not all the way
        if self.hunger <= 0.0 && self.health > 10.0 {
            self.health -= 0.5 * dt;
        }
    }

    /// The boss fighting the player, if any, nearest first: which it is, how much health it has
    /// left, 0-1, and whether it's enraged
    fn foe(&self, near: Vector3<f64>) -> Option<(Look, f64, bool, Vector3<f64>)> {
        let vpm = self.combat.vpm() as u32;
        self.animals
            .iter()
            .filter(|a| a.health > 0.0 && a.look.is_boss() && a.boss.as_ref().is_some_and(|b| b.engaged))
            .min_by(|a, b| (a.position - near).norm().total_cmp(&(b.position - near).norm()))
            .map(|a| (a.look, a.vitality(), a.boss.as_ref().is_some_and(|b| b.enraged), a.center(vpm)))
    }

    /// For trying out dying: the player down to `health`
    pub fn set_health(&mut self, health: f64) {
        self.health = health.clamp(1.0, MAX_HEALTH);
    }

    /// For trying them out: every boss about down to `fraction` of its health
    pub fn hurt_bosses(&mut self, fraction: f64) {
        for animal in self.animals.iter_mut().filter(|a| a.look.is_boss()) {
            animal.health = animal.look.health() * fraction.clamp(0.01, 1.0);
        }
    }

    /// The middle of the body of the boss or monster nearest `near`, if there's one about
    pub fn nearest_boss(&self, near: Vector3<f64>) -> Option<Vector3<f64>> {
        let vpm = self.combat.vpm();
        self.animals
            .iter()
            .filter(|a| a.look.build().is_some() && a.health > 0.0)
            .min_by(|a, b| (a.position - near).norm().total_cmp(&(b.position - near).norm()))
            .map(|a| a.center(vpm as u32))
    }

    /// Where the weapon lying nearest `near` is, once it's landed, if there's one about
    pub fn nearest_loot(&self, near: Vector3<f64>) -> Option<Vector3<f64>> {
        let vpm = self.combat.vpm();
        self.pickups
            .iter()
            .filter(|p| matches!(p.item, Item::Weapon(_)) && p.flying <= 0.0)
            .map(|p| p.position + Vector3::new(0.0, 0.0, 0.2 * vpm))
            .min_by(|a, b| (a - near).norm().total_cmp(&(b - near).norm()))
    }

    /// How the player's being thrown by blows since last time, in voxels a second
    pub fn take_knockback(&mut self) -> Vector3<f64> {
        // However many blows land at once, they throw the player only so far
        let most = THROWN_MOST * self.combat.vpm();
        let mut thrown = std::mem::take(&mut self.thrown);
        let flat = thrown.xy().norm();
        if flat > most {
            thrown.x *= most / flat;
            thrown.y *= most / flat;
        }
        thrown.z = thrown.z.min(most);
        std::mem::take(&mut self.knockback) + thrown
    }

    /// How hard the camera should shake now, 0 to about 1
    pub fn shake(&self) -> f64 {
        self.shake.max(self.combat.shake)
    }

    /// Animals that were killed fall, and leave their meat. Bosses and monsters leave loot.
    fn reap(&mut self, world: &World) {
        let vpm = world.voxels_per_meter as f64;
        let mut k = 0;
        while k < self.animals.len() {
            if self.animals[k].health > 0.0 {
                k += 1;
                continue;
            }
            let animal = self.animals.remove(k);
            let look = animal.look;
            let middle = animal.position + Vector3::new(0.0, 0.0, look.body().0 * vpm);
            // A monster bursts apart its own way, and the last of a dungeon hall's leaves it clear.
            // The dead and spiders' victims had a little gold on them, now and then a weapon.
            if look.is_monster() {
                self.combat.perish(&animal);
                if let Some(key) = animal.boss.as_ref().map(|b| b.key).filter(|key| self.dens.contains(key)) {
                    if !self.animals.iter().any(|a| a.health > 0.0 && a.boss.as_ref().is_some_and(|b| b.key == key)) {
                        self.cleared.insert(key);
                    }
                }
                let mut loot = Vec::new();
                let undead = !matches!(look, Look::Wolf | Look::Spiderling);
                if look != Look::Wolf && random(&mut self.rng) < if undead { 0.7 } else { 0.4 } {
                    loot.push(Item::Gold(if undead { 3 } else { 2 } + (random(&mut self.rng) * if undead { 10.0 } else { 5.0 }) as u32));
                }
                if undead && random(&mut self.rng) < 0.06 {
                    loot.push(self.roll_weapon(1.5));
                } else if look != Look::Wolf && random(&mut self.rng) < 0.1 {
                    loot.push(if random(&mut self.rng) < 0.5 { Item::Potion } else { Item::Bread });
                }
                self.scatter(world, middle, animal.position, loot, 0.9);
            }
            if let Some(boss) = animal.boss.as_ref().filter(|_| look.is_boss()) {
                self.defeated.insert(boss.key);
                // What it hoarded bursts out of it onto the ground round about: weapons, gold and
                // potions, a raid boss's far more and far better
                let raid = look.is_raid();
                // Its best weapon's very likely legendary or better, the rest a mix
                let (lucks, piles, potions): (&[f64], _, _) = if raid { (&[12.0, 5.0, 3.0, 2.5, 2.0], 3, 3) } else { (&[6.0, 2.5], 1, 1) };
                let mut loot: Vec<Item> = lucks.iter().map(|&luck| self.roll_weapon(luck)).collect();
                for _ in 0..piles {
                    let gold = if raid { 200.0 + 100.0 * random(&mut self.rng) } else { 80.0 + 80.0 * random(&mut self.rng) };
                    loot.push(Item::Gold(gold as u32));
                }
                loot.extend(std::iter::repeat_n(Item::Potion, potions));
                let best = loot.iter().filter_map(|&item| match item {
                    Item::Weapon(w) => Some(self.weapons[w].grade),
                    _ => None,
                }).max().unwrap_or(0);
                self.scatter(world, middle, animal.position, loot, 2.0 * look.size().sqrt() + look.body().1 * 0.5);
                self.combat.fell(middle, look.body().1);
                self.shake = self.shake.max(1.0);
                self.cues.push(Cue::here(Sound::Fanfare, if raid { 1.0 } else { 0.8 }));
                let rarity = self.library.rarities[best].display_name.to_lowercase();
                if raid {
                    self.say(format!("Raid boss slain! The {} falls, and its hoard spills out. Some of it's {rarity}!", look.name()));
                } else {
                    self.say(format!("You defeated the {}! Its loot lies scattered round. The best of it's {rarity}.", look.name()));
                }
            }
            for m in 0..look.meat() {
                let angle = animal.heading() + m as f64 * 2.1;
                let spread = 0.25 * m as f64;
                self.pickups.push(Pickup::lying(Item::Meat, animal.position + Vector3::new(angle.cos(), angle.sin(), 0.0) * spread, angle));
            }
            if look.is_monster() {
                self.say(format!("You slew a {}.", look.name()));
            } else if !look.is_boss() {
                self.say(if look.meat() > 0 { format!("You hunted a {}. It dropped some meat.", look.name()) } else { format!("You hunted a {}.", look.name()) });
            }
            self.target = None;
        }
    }

    /// What was dropped landing, which weapons lying about the camera can see, for their beams of
    /// light to show, and the merchant being traded with left behind
    fn look_over_loot(&mut self, world: &World, player: Option<Vector3<f64>>, view: &View, dt: f64) {
        let vpm = world.voxels_per_meter as f64;
        self.feet = Some(player.unwrap_or(view.eye));
        for pickup in &mut self.pickups {
            if pickup.flying > 0.0 {
                pickup.flying -= dt;
                if pickup.flying <= 0.0 {
                    let cue = match pickup.item {
                        Item::Weapon(_) => Cue::at(Sound::Clang, pickup.position, 0.35).pitched(1.6),
                        Item::Gold(_) => Cue::at(Sound::Pickup, pickup.position, 0.35).pitched(1.9),
                        _ => Cue::at(Sound::Pickup, pickup.position, 0.2).pitched(0.8),
                    };
                    self.cues.push(cue);
                }
            }
            // Looking a quarter of a meter at a time for anything in the way
            if pickup.shines() {
                let to = pickup.at(vpm) + Vector3::new(0.0, 0.0, 0.4 * vpm) - view.eye;
                let d = to.norm();
                let steps = (d / (0.25 * vpm)) as usize;
                pickup.seen = d < BEAM_RANGE * vpm
                    && (1..steps).all(|s| {
                        let p = view.eye + to * (s as f64 / steps as f64);
                        is_passable(world.material_at(p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64))
                    });
            }
        }
        if let Some(key) = self.shop {
            let near = self.merchants.iter().find(|(_, k)| *k == key).and_then(|(id, _)| self.animals.iter().find(|a| a.id == *id));
            if near.is_none_or(|m| (m.position - view.eye).norm() > SHOP_REACH * vpm) {
                self.shop = None;
                self.say("You walk away from the merchant.");
            }
        }
    }

    /// Lets go of the weapons nobody has any more: not carried, lying about or on a stall. Only
    /// once there are a lot of them, as it moves the weapon in hand along, which raises it again.
    fn forget_weapons(&mut self) {
        let mut kept = vec![false; self.weapons.len()];
        let lying = self.pickups.iter().map(|p| p.item);
        let stalls = self.stocks.values().flatten().filter_map(|w| match *w {
            Ware::Weapon(i) => Some(Item::Weapon(i)),
            Ware::Food(..) => None,
        });
        for item in self.slots.iter().map(|s| s.0).chain(lying).chain(stalls) {
            if let Item::Weapon(i) = item {
                kept[i] = true;
            }
        }
        if kept.iter().filter(|&&k| !k).count() < 24 {
            return;
        }
        let mut to = vec![0; self.weapons.len()];
        let mut next = 0;
        for (i, &keep) in kept.iter().enumerate() {
            to[i] = next;
            next += keep as usize;
        }
        let mut k = 0;
        self.weapons.retain(|_| {
            k += 1;
            kept[k - 1]
        });
        let renumber = |item: &mut Item| {
            if let Item::Weapon(i) = item {
                *i = to[*i];
            }
        };
        self.slots.iter_mut().for_each(|s| renumber(&mut s.0));
        self.pickups.iter_mut().for_each(|p| renumber(&mut p.item));
        for ware in self.stocks.values_mut().flatten() {
            if let Ware::Weapon(i) = ware {
                *i = to[*i];
            }
        }
        self.models_version += 1;
    }

    /// A new weapon rolled with `luck`, as for Library::roll_rarity(), kept among Game::weapons
    fn roll_weapon(&mut self, luck: f64) -> Item {
        self.rng = hash(self.rng ^ 0x1007);
        let weapon = self.library.generate(self.rng, None, luck);
        self.weapons.push(weapon);
        self.models_version += 1;
        Item::Weapon(self.weapons.len() - 1)
    }

    /// Throws `items` out from `from`, in voxels, to land on the ground all round `around` about
    /// `radius` meters off, nearer where there's a wall in the way
    fn scatter(&mut self, world: &World, from: Vector3<f64>, around: Vector3<f64>, items: Vec<Item>, radius: f64) {
        let vpm = world.voxels_per_meter as f64;
        let count = items.len().max(1) as f64;
        let turn = random(&mut self.rng) * TAU;
        for (k, item) in items.into_iter().enumerate() {
            let angle = turn + (k as f64 + 0.5 * random(&mut self.rng)) / count * TAU;
            let out = Vector3::new(angle.cos(), angle.sin(), 0.0) * radius * (0.55 + 0.6 * random(&mut self.rng)) * vpm;
            let room = |p: Vector3<f64>| is_passable(world.material_at(p.x.floor() as i64, p.y.floor() as i64, (p.z + 0.5 * vpm).floor() as i64));
            let spot = [1.0, 0.6, 0.3, 0.0].into_iter().map(|f| around + out * f).find(|&p| room(p)).unwrap_or(around);
            // Landing on the ground, or caught on top of a bush, not hidden in it
            let (x, y) = (spot.x.floor() as i64, spot.y.floor() as i64);
            let top = (spot.z + 1.3 * vpm).floor() as i64;
            let landed = (top - 8 * vpm as i64..=top).rev().find(|&z| {
                let m = world.material_at(x, y, z);
                !is_passable(m) || is_leaves(m)
            });
            let landed = landed.map_or(spot, |z| Vector3::new(spot.x, spot.y, z as f64 + 1.0));
            let heading = random(&mut self.rng) * TAU;
            self.pickups.push(Pickup { item, position: landed, heading, from, flying: DROP_SECONDS, seen: false, beacon: false });
        }
    }

    /// What a merchant asks for a weapon: more the rarer it is, and more for each effect, synergy
    /// and a special attack
    fn weapon_price(&self, w: usize) -> u32 {
        let weapon = &self.weapons[w];
        let profile = &weapon.profile;
        let extra = 1.0 + 0.12 * profile.effects.len() as f64 + 0.25 * profile.synergies.len() as f64 + if profile.special.is_some() { 0.3 } else { 0.0 };
        (WEAPON_PRICES[weapon.grade.min(WEAPON_PRICES.len() - 1)] * extra).round() as u32
    }

    /// What a merchant asks for `item`
    fn price(&self, item: Item) -> u32 {
        match item {
            Item::Weapon(w) => self.weapon_price(w),
            Item::Gold(n) => n,
            _ => food_price(item),
        }
    }

    /// What a merchant pays for `item`
    fn sell_price(&self, item: Item) -> u32 {
        ((self.price(item) as f64 * SELL_SHARE) as u32).max(1)
    }

    /// The nearest animal, bit of food or chest the camera looks at, within reach
    fn find_target(&self, world: &World, eye: Vector3<f64>, look: Vector3<f64>) -> Option<Target> {
        let vpm = world.voxels_per_meter as f64;
        let mut best = REACH * vpm;
        let mut target = None;
        let mut consider = |position: Vector3<f64>, (height, radius): (f64, f64), found: Target| {
            let center = position + Vector3::new(0.0, 0.0, height * vpm);
            let to = center - eye;
            let along = to.dot(&look);
            let miss = (to - look * along).norm();
            if along > 0.0 && along < best && miss < radius * vpm {
                best = along;
                target = Some(found);
            }
        };
        for (i, animal) in self.animals.iter().enumerate() {
            consider(animal.position, animal.look.body(), Target::Animal(i));
        }
        // A weapon lying flat is easy to miss, so it's looked for a little wider
        for (i, pickup) in self.pickups.iter().enumerate() {
            consider(pickup.at(vpm), pickup.item.look().map_or((0.15, 0.55), Look::body), Target::Pickup(i));
        }
        for (i, chest) in self.chests.iter().enumerate() {
            if !self.opened.contains(&chest.key) {
                consider(chest.position, Look::Chest.body(), Target::Chest(i));
            }
        }
        target
    }

    fn add_item(&mut self, item: Item, count: u32) {
        match self.slots.iter_mut().find(|(i, _)| *i == item) {
            Some(slot) => slot.1 += count,
            None => self.slots.push((item, count)),
        }
    }

    /// Left click without a weapon: picks up the food the crosshair is on, opens a chest, or hits
    /// the animal, or pets it
    pub fn primary(&mut self, eye: Vector3<f64>) {
        match self.target {
            Some(Target::Pickup(_) | Target::Chest(_)) => self.interact(eye),
            Some(Target::Animal(i)) => {
                let look = self.animals[i].look;
                if look.is_pet() {
                    self.interact(eye);
                    return;
                }
                self.cues.push(Cue::at(Sound::Hit, self.animals[i].position, 0.7));
                if self.animals[i].hurt(eye) {
                    self.target = None;
                } else {
                    self.say(format!("You hit the {}!", look.name()));
                }
            }
            None => {}
        }
    }

    /// E: picks up food, gold and weapons, opens chests, trades with merchants, pets dogs and cats
    pub fn interact(&mut self, eye: Vector3<f64>) {
        match self.target {
            Some(Target::Pickup(i)) => {
                let item = self.pickups.remove(i).item;
                self.target = None;
                self.take(item);
            }
            Some(Target::Chest(i)) => self.open_chest(i),
            Some(Target::Animal(i)) => {
                let look = self.animals[i].look;
                if look == Look::Merchant {
                    let id = self.animals[i].id;
                    if let Some(&(_, key)) = self.merchants.iter().find(|(m, _)| *m == id) {
                        self.stock(key);
                        self.shop = Some(key);
                        self.cues.push(Cue::here(Sound::Chest, 0.5).pitched(1.3));
                        self.say("\"Have a look. Good steel, fresh bread, potions that work.\"");
                    }
                } else if look.is_villager() {
                    self.animals[i].pet(eye);
                    self.rng = hash(self.rng ^ self.animals[i].id);
                    self.say(GREETINGS[self.rng as usize % GREETINGS.len()]);
                } else if look.is_pet() {
                    self.animals[i].pet(eye);
                    self.say(match look {
                        Look::Dog => "The dog wags its tail happily.",
                        _ => "The cat purrs.",
                    });
                } else {
                    self.say(format!("The {} won't let you near. Click to hunt it.", look.name()));
                }
            }
            None => {}
        }
    }

    /// Picks up `item`: gold goes in the purse, the rest into what's carried
    fn take(&mut self, item: Item) {
        match item {
            Item::Gold(n) => {
                self.gold += n;
                self.cues.push(Cue::here(Sound::Pickup, 0.7).pitched(1.8));
                self.say(format!("+{n} gold"));
            }
            Item::Weapon(w) => {
                self.slots.push((item, 1));
                self.cues.push(Cue::here(Sound::Equip, 0.7));
                let weapon = &self.weapons[w];
                let rarity = &self.library.rarities[weapon.grade];
                let family = &self.library.families[weapon.family];
                self.say(format!("You take {}, a {} {}.", weapon.name, rarity.display_name.to_lowercase(), family.display_name.to_lowercase()));
            }
            _ => {
                self.add_item(item, 1);
                self.cues.push(Cue::here(Sound::Pickup, 0.6));
                self.say(format!("+1 {}", item.name()));
            }
        }
    }

    /// Opens chest `i` and takes the weapon in it, rolled from where the chest is, and a little gold
    fn open_chest(&mut self, i: usize) {
        let chest = &self.chests[i];
        let key = chest.key;
        let seed = hash(self.seed ^ hash(key[0] as u32 ^ hash(key[1] as u32 ^ hash(key[2] as u32))));
        let weapon = self.library.generate(seed, None, chest.luck);
        let gold = 10 + (hash(seed ^ 0x601d) % 30) as u32 + (chest.luck * 4.0) as u32;
        self.opened.insert(key);
        self.target = None;
        let rarity = &self.library.rarities[weapon.grade];
        let family = &self.library.families[weapon.family];
        let message = format!("You found {}, a {} {}, and {gold} gold!", weapon.name, rarity.display_name.to_lowercase(), family.display_name.to_lowercase());
        self.weapons.push(weapon);
        self.slots.push((Item::Weapon(self.weapons.len() - 1), 1));
        self.gold += gold;
        self.models_version += 1;
        self.cues.push(Cue::here(Sound::Chest, 0.8));
        self.say(message);
    }

    /// Whether the player's trading with a merchant
    pub fn shopping(&self) -> bool {
        self.shop.is_some()
    }

    pub fn close_shop(&mut self) {
        self.shop = None;
    }

    /// The merchants' spots in the piece of the world just built, in its voxels: one turns up at
    /// each that hasn't one already
    pub fn set_stalls(&mut self, world: &World, water: WaterLevel, stalls: Vec<Stall>) {
        self.merchants.retain(|(id, _)| self.animals.iter().any(|a| a.id == *id));
        for stall in &stalls {
            if self.merchants.iter().any(|(_, key)| *key == stall.key) {
                continue;
            }
            let seed = hash(stall.key[0] as u32 ^ hash(stall.key[1] as u32 ^ 0x5a1e));
            if let Some(merchant) = animals::spawn_merchant(world, water, stall.position, stall.within, seed) {
                self.merchants.push((merchant.id, stall.key));
                self.animals.push(merchant);
            }
        }
    }

    /// The towns' squares in the piece of the world just built, in its voxels: their folk turn up
    /// round each that hasn't any about already
    pub fn set_towns(&mut self, world: &World, water: WaterLevel, towns: Vec<Town>) {
        let vpm = world.voxels_per_meter as f64;
        let origin = Vector3::new(world.origin[0] as f64, world.origin[1] as f64, 0.0);
        for town in &towns {
            let home = town.position + origin;
            if self.animals.iter().any(|a| a.home().is_some_and(|(h, _)| (h - home).norm() < vpm)) {
                continue;
            }
            let mut seed = hash(home.x as u32 ^ hash(home.y as u32 ^ 0x70f0));
            for _ in 0..town.folk {
                seed = hash(seed);
                let look = if seed % 2 == 0 { Look::Villager } else { Look::Townswoman };
                if let Some(villager) = animals::spawn_villager(world, water, look, town.position, town.roam * 0.7, (home, town.roam), seed) {
                    self.animals.push(villager);
                }
            }
        }
    }

    /// For trying it out: a merchant `distance` meters along `ahead` from `near`
    pub fn summon_merchant(&mut self, world: &World, water: WaterLevel, near: Vector3<f64>, ahead: Vector3<f64>, distance: f64) {
        let vpm = world.voxels_per_meter as f64;
        let flat = Vector3::new(ahead.x, ahead.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x());
        let at = near + flat * distance * vpm;
        let key = [at.x as i64, at.y as i64, -3];
        if let Some(merchant) = animals::spawn_merchant(world, water, at, 1.0, self.rng) {
            self.merchants.push((merchant.id, key));
            self.animals.push(merchant);
        }
    }

    /// For trying them out: a villager of each kind `distance` meters along `ahead` from `near`,
    /// side by side, strolling round there
    pub fn summon_villagers(&mut self, world: &World, water: WaterLevel, near: Vector3<f64>, ahead: Vector3<f64>, distance: f64) {
        let vpm = world.voxels_per_meter as f64;
        let flat = Vector3::new(ahead.x, ahead.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x());
        let side = Vector3::new(-flat.y, flat.x, 0.0);
        let origin = Vector3::new(world.origin[0] as f64, world.origin[1] as f64, 0.0);
        for (look, by) in [(Look::Villager, -0.6), (Look::Townswoman, 0.6)] {
            let at = near + (flat * distance + side * by) * vpm;
            if let Some(villager) = animals::spawn_villager(world, water, look, at, 0.1, (at + origin, 6.0), self.rng ^ by.to_bits() as u32) {
                self.animals.push(villager);
            }
        }
    }

    /// For trying it out: trades with the nearest merchant to `near`
    pub fn open_nearest_shop(&mut self, near: Vector3<f64>) {
        let nearest = self.merchants.iter().filter_map(|&(id, key)| Some((self.animals.iter().find(|a| a.id == id)?, key))).min_by(|a, b| (a.0.position - near).norm().total_cmp(&(b.0.position - near).norm()));
        if let Some((_, key)) = nearest {
            self.stock(key);
            self.shop = Some(key);
        }
    }

    /// What merchant `key` has to sell, rolled from where it stands the first time it's looked at:
    /// weapons, a few of them rare, and food and potions
    fn stock(&mut self, key: [i64; 3]) {
        if self.stocks.contains_key(&key) {
            return;
        }
        let mut seed = hash(self.seed ^ hash(key[0] as u32 ^ hash(key[1] as u32 ^ hash(key[2] as u32 ^ 0x5709))));
        let mut wares = Vec::new();
        for k in 0..5 {
            seed = hash(seed);
            self.weapons.push(self.library.generate(seed, None, 1.0 + 0.6 * k as f64));
            wares.push(Ware::Weapon(self.weapons.len() - 1));
        }
        self.models_version += 1;
        wares.sort_by_key(|w| match *w {
            Ware::Weapon(i) => self.weapon_price(i),
            Ware::Food(..) => 0,
        });
        wares.extend([Ware::Food(Item::Potion, 4), Ware::Food(Item::Bread, 6), Ware::Food(Item::Meat, 5), Ware::Food(Item::Apple, 10)]);
        self.stocks.insert(key, wares);
    }

    /// Buys ware `k` of the merchant being traded with
    fn buy(&mut self, k: usize) {
        let Some(key) = self.shop else {
            return;
        };
        let Some(&ware) = self.stocks.get(&key).and_then(|s| s.get(k)) else {
            return;
        };
        let item = match ware {
            Ware::Weapon(w) => Item::Weapon(w),
            Ware::Food(item, _) => item,
        };
        let cost = self.price(item);
        if cost > self.gold {
            self.say(format!("You need {} more gold for that.", cost - self.gold));
            return;
        }
        self.gold -= cost;
        let stock = self.stocks.get_mut(&key).unwrap();
        match &mut stock[k] {
            Ware::Food(_, left) if *left > 1 => *left -= 1,
            _ => {
                stock.remove(k);
            }
        }
        match item {
            Item::Weapon(_) => self.slots.push((item, 1)),
            _ => self.add_item(item, 1),
        }
        self.cues.push(Cue::here(Sound::Pickup, 0.8).pitched(1.5));
        self.say(format!("Bought {} for {cost} gold.", self.item_name(item)));
    }

    /// Sells one of what's in slot `slot` to the merchant being traded with. A weapon goes on
    /// their stall, for buying back at their price.
    fn sell(&mut self, slot: usize) {
        let Some(key) = self.shop else {
            return;
        };
        let Some(&(item, count)) = self.slots.get(slot) else {
            return;
        };
        if item == Item::Bucket {
            return;
        }
        let paid = self.sell_price(item);
        self.gold += paid;
        if count > 1 {
            self.slots[slot].1 -= 1;
        } else {
            self.remove_slot(slot);
        }
        if let Item::Weapon(w) = item {
            self.stocks.entry(key).or_default().insert(0, Ware::Weapon(w));
        }
        self.cues.push(Cue::here(Sound::Pickup, 0.8).pitched(1.2));
        self.say(format!("Sold {} for {paid} gold.", self.item_name(item)));
    }

    /// Takes slot `slot` out of what's carried, keeping what's in hand in hand
    fn remove_slot(&mut self, slot: usize) {
        self.slots.remove(slot);
        if self.selected > slot {
            self.selected -= 1;
        }
        self.selected = self.selected.min(self.slots.len().saturating_sub(1));
        self.inspected = self.inspected.filter(|&i| i < self.slots.len());
    }

    /// Drops all of what's in slot `slot` at the player's feet
    fn drop_slot(&mut self, slot: usize) {
        let (Some(&(item, count)), Some(feet)) = (self.slots.get(slot), self.feet) else {
            return;
        };
        if item == Item::Bucket {
            return;
        }
        self.remove_slot(slot);
        let vpm = self.combat.vpm();
        for k in 0..count {
            let angle = random(&mut self.rng) * TAU;
            let at = feet + Vector3::new(angle.cos(), angle.sin(), 0.0) * (0.5 + 0.2 * k as f64) * vpm;
            self.pickups.push(Pickup { item, position: at, heading: angle, from: feet + Vector3::new(0.0, 0.0, 1.2 * vpm), flying: DROP_SECONDS, seen: false, beacon: false });
        }
        self.say(format!("You drop {}.", self.item_name(item)));
    }

    /// Takes slot `slot` in hand. From the pack it swaps places with what was in hand, so what's in
    /// hand is always on the quick bar.
    fn hold(&mut self, slot: usize) {
        if slot >= self.slots.len() {
            return;
        }
        let to = if slot < HOTBAR {
            slot
        } else {
            let to = if self.selected < HOTBAR { self.selected } else { 0 };
            self.slots.swap(slot, to);
            to
        };
        self.selected = to;
        self.inspected = Some(to);
        self.cues.push(Cue::here(Sound::Equip, 0.6));
    }

    /// Puts what's carried in order: the quick bar as it is, then the rest, the rarest weapons
    /// first, then food
    fn sort(&mut self) {
        if self.slots.len() <= HOTBAR {
            return;
        }
        let held = self.slots.get(self.selected).map(|s| s.0);
        let mut rest = self.slots.split_off(HOTBAR);
        rest.sort_by_key(|&(item, _)| match item {
            Item::Weapon(w) => (0, std::cmp::Reverse(self.weapons[w].grade), self.weapons[w].family),
            Item::Bucket => (1, std::cmp::Reverse(0), 0),
            other => (2, std::cmp::Reverse(food_price(other) as usize), 0),
        });
        self.slots.append(&mut rest);
        if let Some(held) = held {
            self.selected = self.slots.iter().position(|s| s.0 == held).unwrap_or(0);
        }
        self.inspected = None;
    }

    /// Right click without a weapon: eats or drinks one of what's selected in the quick bar
    pub fn eat_selected(&mut self) {
        if !self.slots.iter().any(|(item, _)| item.consumable()) {
            self.say("You have nothing to eat. Look for apples under leafy trees, or buy some bread.");
            return;
        }
        self.eat(self.selected.min(self.slots.len() - 1));
    }

    /// Picks a quick bar slot, if there's something in it
    pub fn select(&mut self, slot: usize) {
        if slot < self.slots.len().min(HOTBAR) {
            self.selected = slot;
        }
    }

    /// Picks the quick bar slot `by` along from the one picked now, going round
    pub fn scroll(&mut self, by: i32) {
        let shown = self.slots.len().min(HOTBAR) as i32;
        if shown > 0 {
            self.selected = (self.selected.min(shown as usize - 1) as i32 + by).rem_euclid(shown) as usize;
        }
    }

    /// For trying them out: rolls weapons of family `id` until one has `element`, and puts it in
    /// hand
    pub fn find_weapon(&mut self, id: &str, element: Option<Element>, luck: f64) {
        let Some(family) = self.library.family(id) else {
            return;
        };
        for seed in 0..2000u32 {
            let weapon = self.library.generate(hash(seed ^ 0xf1d), Some(family), luck);
            if weapon.profile.element == element {
                self.weapons.push(weapon);
                self.slots.insert(0, (Item::Weapon(self.weapons.len() - 1), 1));
                self.selected = 0;
                self.models_version += 1;
                return;
            }
        }
    }

    /// For trying them out: rolls lucky weapons of any family until one has the special attack
    /// `special`, puts it in hand and charges it
    pub fn find_special(&mut self, special: Special) {
        for seed in 0..20000u32 {
            let weapon = self.library.generate(hash(seed ^ 0x5bec), None, 8.0);
            if weapon.profile.special.as_ref().is_some_and(|(s, _)| *s == special) {
                self.weapons.push(weapon);
                self.slots.insert(0, (Item::Weapon(self.weapons.len() - 1), 1));
                self.selected = 0;
                self.models_version += 1;
                self.combat.charge = 1.0;
                return;
            }
        }
    }

    /// Takes the first weapon of family `id` carried in hand
    pub fn select_family(&mut self, id: &str) {
        let family = self.library.family(id);
        if let Some(slot) = self.slots.iter().position(|(item, _)| matches!(item, Item::Weapon(i) if family.is_some() && self.weapons[*i].family == family.unwrap())) {
            self.hold(slot);
        }
    }

    /// Takes the first weapon of `kind` carried in hand
    pub fn select_weapon(&mut self, kind: Kind) {
        if let Some(slot) = self.slots.iter().position(|(item, _)| matches!(item, Item::Weapon(i) if self.weapons[*i].kind() == kind)) {
            self.hold(slot);
        }
    }

    fn eat(&mut self, slot: usize) {
        let Some(&(item, count)) = self.slots.get(slot) else {
            return;
        };
        if !item.consumable() {
            return;
        }
        if item.heal() > 0.0 && self.health >= MAX_HEALTH {
            self.say("You're not hurt.");
            return;
        }
        if item.food() > 0.0 && self.hunger >= 99.5 && self.health >= MAX_HEALTH {
            self.say("You're full.");
            return;
        }
        // Food heals a little too
        self.hunger = (self.hunger + item.food()).min(100.0);
        self.health = (self.health + item.food() * 0.6 + item.heal()).min(MAX_HEALTH);
        self.cues.push(Cue::here(Sound::Eat, 0.7).pitched(if item.heal() > 0.0 { 1.5 } else { 1.0 }));
        if count > 1 {
            self.slots[slot].1 -= 1;
        } else {
            self.remove_slot(slot);
        }
        self.say(if item.heal() > 0.0 { format!("You drink the potion. +{:.0} health", item.heal()) } else { format!("You ate the {}.", item.name().to_lowercase()) });
    }

    /// What to draw for the shader: the weapon in hand first, then the nearest animals, food,
    /// props and things flying about around the camera
    pub fn instances(&self, world: &World, view: &View) -> Vec<AnimalInstance> {
        let vpm = world.voxels_per_meter as f64;
        let camera = view.eye;
        // Nothing in hand while lying dead
        let held = self.held().filter(|_| self.dead.is_none()).map(|(i, w)| (w, self.weapon_model(i)));
        let mut out = self.combat.held_instances(held, view);
        // Animals come before anything else, bosses most of all, so however much is flying
        // about they're always drawn
        let mut creatures: Vec<(f64, AnimalInstance)> = Vec::new();
        for animal in &self.animals {
            let d = (animal.position - camera).norm();
            if d < ANIMAL_RANGE * vpm || animal.look.is_boss() {
                creatures.push((if animal.look.is_boss() { 0.0 } else { d }, animal.instance(world.voxels_per_meter)));
            }
        }
        creatures.sort_by(|a, b| a.0.total_cmp(&b.0));
        out.extend(creatures.into_iter().take(MAX_INSTANCES / 2).map(|(_, i)| i));
        let mut near: Vec<(f64, AnimalInstance)> = self.combat.flying_instances(view);
        let time = self.combat.time();
        for pickup in &self.pickups {
            let at = pickup.at(vpm);
            let d = (at - camera).norm();
            if d >= FOOD_RANGE * vpm {
                continue;
            }
            match (pickup.item, pickup.item.look()) {
                (_, Some(look)) => near.push((d, animals::still_instance(look, at, pickup.heading, world.voxels_per_meter))),
                // A weapon lies on its side a hand over the ground, turning slowly and bobbing, its
                // middle over where it lies, tumbling as it flies out
                (Item::Weapon(w), None) => {
                    let look = &self.weapons[w].look;
                    let tumble = pickup.flying / DROP_SECONDS * 9.0;
                    let rotation = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), pickup.heading + time * 0.7) * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), FRAC_PI_2 + tumble);
                    let hover = (0.22 + 0.05 * (time * 2.2 + pickup.heading).sin()) * vpm;
                    let middle = (look.grip + look.tip) * 0.5;
                    near.push((d, placed(self.weapon_model(w), &look.frames[0], look.res, middle, at + Vector3::new(0.0, 0.0, hover), rotation, vpm)));
                }
                _ => {}
            }
        }
        // Props are small, so they're only drawn up close
        for &(look, position, heading) in &self.props {
            let d = (position - camera).norm();
            if d < PROP_RANGE * vpm {
                near.push((d, animals::still_instance(look, position, heading, world.voxels_per_meter)));
            }
        }
        for chest in &self.chests {
            let d = (chest.position - camera).norm();
            if d < PROP_RANGE * vpm {
                let look = if self.opened.contains(&chest.key) { Look::OpenChest } else { Look::Chest };
                near.push((d, animals::still_instance(look, chest.position, chest.heading, world.voxels_per_meter)));
            }
        }
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        // The falling leaves, bits blown loose and fine sparks go last, so the nearest things
        // still cast shadows. They get what room's left, and at least a third of it.
        let mut leaves = self.combat.fine_instances(view);
        leaves.extend(self.leaves.instances(self.leaf_model(), vpm));
        let budget = MAX_INSTANCES.saturating_sub(out.len());
        let taken = near.len().min(budget - leaves.len().min(budget / 3));
        out.extend(near.into_iter().take(taken).map(|(_, i)| i));
        out.extend(leaves.into_iter().take(budget - taken));
        out
    }

    /// Lights from the fighting, in the form main.rs keeps the world's lights in
    pub fn lights(&self, view: &View) -> Vec<[f32; 8]> {
        self.combat.lights(self.held().map(|(_, w)| w), &self.animals, view)
    }

    /// What to do with what the crosshair is on. What's lying about has a card of its own instead.
    fn target_hint(&self) -> Option<String> {
        match self.target? {
            Target::Pickup(_) => None,
            Target::Chest(_) => Some("E  open the chest".into()),
            Target::Animal(i) => {
                let look = self.animals[i].look;
                Some(if look == Look::Merchant {
                    "E  trade with the merchant".to_owned()
                } else if look.is_villager() {
                    "E  talk to the villager".to_owned()
                } else if look.is_pet() {
                    format!("E  pet the {}", look.name())
                } else if self.holding_weapon() {
                    format!("Attack the {}", look.name())
                } else {
                    format!("Click  hunt the {}", look.name())
                })
            }
        }
    }

    /// Crosshair, and along the top the quick bar, hunger, mana and messages, drawn over the game.
    /// Damage numbers and health bars float over what was hit.
    pub fn hud(&self, ctx: &egui::Context, view: &View) {
        let screen = ctx.screen_rect();
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("hud")));
        let center = screen.center();
        let shadow = Color32::from_black_alpha(150);
        let faint = Color32::from_white_alpha(140);
        let on_screen = |p: Vector3<f64>| view.project(p).map(|[x, y]| pos2(screen.left() + x as f32 * screen.width(), screen.top() + y as f32 * screen.height()));

        // Health bars over animals that were just hurt
        let vpm = view.vpm;
        for animal in &self.animals {
            if animal.status.shown <= 0.0 || animal.health <= 0.0 || animal.look.is_boss() {
                continue;
            }
            let top = animal.position + Vector3::new(0.0, 0.0, (animal.look.body().0 + animal.look.body().1 + 0.25) * vpm);
            let Some(at) = on_screen(top) else {
                continue;
            };
            let distance = ((top - view.eye).norm() / vpm).max(1.0) as f32;
            let width = (70.0 / distance.sqrt()).clamp(24.0, 70.0);
            let alpha = (animal.status.shown.min(1.0) * 255.0) as u8;
            let bar = Rect::from_center_size(at, vec2(width, 5.0));
            painter.rect_filled(bar.expand(1.5), 3.0, Color32::from_black_alpha(alpha / 2));
            let fill = Rect::from_min_size(bar.min, vec2(width * animal.vitality() as f32, 5.0));
            painter.rect_filled(fill, 2.0, Color32::from_rgba_unmultiplied(220, 60, 50, alpha));
        }
        // Merchants' names over their heads, so they're easy to find, and smaller ones over the
        // villagers' when near
        for npc in self.animals.iter().filter(|a| a.look == Look::Merchant || a.look.is_villager()) {
            let (name, color, height, range, size) = if npc.look == Look::Merchant {
                ("Merchant", GOLD, 2.15, 28.0, 15.0)
            } else {
                ("Villager", Color32::from_rgb(230, 228, 214), 1.95, 14.0, 13.0)
            };
            let top = npc.position + Vector3::new(0.0, 0.0, height * vpm);
            let distance = (top - view.eye).norm() / vpm;
            let Some(at) = on_screen(top).filter(|_| distance < range) else {
                continue;
            };
            let alpha = ((range - distance) / 6.0).clamp(0.0, 1.0) as f32;
            painter.text(at + vec2(1.0, 1.0), Align2::CENTER_BOTTOM, name, FontId::proportional(size), Color32::from_black_alpha((160.0 * alpha) as u8));
            painter.text(at, Align2::CENTER_BOTTOM, name, FontId::proportional(size), color.gamma_multiply(alpha));
        }
        // Damage numbers floating up
        for popup in &self.combat.popups {
            let Some(at) = on_screen(popup.position) else {
                continue;
            };
            let alpha = ((1.2 - popup.age) / 0.4).clamp(0.0, 1.0) as f32;
            let [r, g, b] = popup.color;
            let font = FontId::proportional(popup.size);
            painter.text(at + vec2(1.5, 1.5), Align2::CENTER_CENTER, &popup.text, font.clone(), Color32::from_black_alpha((160.0 * alpha) as u8));
            painter.text(at, Align2::CENTER_CENTER, &popup.text, font, Color32::from_rgba_unmultiplied(r, g, b, (255.0 * alpha) as u8));
        }

        // A beam of light up from each weapon lying about in sight, in the color of its rarity,
        // taller and brighter the rarer it is, fading up and further off
        for pickup in self.pickups.iter().filter(|p| p.seen && p.flying <= 0.0 && p.shines()) {
            // The gold the player lost when they died shines as bright as an epic weapon
            let (grade, [r, g, b], w) = match pickup.item {
                Item::Weapon(w) => (self.weapons[w].grade, self.library.rarities[self.weapons[w].grade].color, w),
                _ => (3, [GOLD.r(), GOLD.g(), GOLD.b()], 0),
            };
            let base = pickup.position + Vector3::new(0.0, 0.0, 0.2 * vpm);
            let distance = ((base - view.eye).norm() / vpm).max(1.0);
            let top = base + Vector3::new(0.0, 0.0, (2.2 + 0.9 * grade as f64) * vpm);
            let (Some(foot), Some(top)) = (on_screen(base), on_screen(top)) else {
                continue;
            };
            let fade = ((1.0 - distance / BEAM_RANGE) * 1.5).clamp(0.0, 1.0) as f32 * (0.55 + 0.09 * grade as f32);
            let width = ((26.0 / distance.sqrt()) as f32).clamp(2.5, 12.0) * (0.8 + 0.1 * grade as f32);
            let pulse = 0.85 + 0.15 * (self.combat.time() * 3.0 + w as f64).sin() as f32;
            let tint = |alpha: f32| Color32::from_rgba_unmultiplied(r, g, b, (alpha * fade * pulse * 255.0).clamp(0.0, 255.0) as u8);
            painter.circle_filled(foot, width * 2.2, tint(0.18));
            painter.circle_filled(foot, width * 0.9, tint(0.45));
            for k in 0..14 {
                let (a, z) = (k as f32 / 14.0, (k + 1) as f32 / 14.0);
                let strength = (1.0 - a).powf(1.6);
                let segment = [foot.lerp(top, a), foot.lerp(top, z)];
                painter.line_segment(segment, Stroke::new(width * 2.4, tint(0.16 * strength)));
                painter.line_segment(segment, Stroke::new(width * 0.9, tint(0.55 * strength)));
                painter.line_segment(segment, Stroke::new(width * 0.3, Color32::from_white_alpha((strength * fade * 200.0) as u8)));
            }
        }

        painter.circle_filled(center, 3.5, shadow);
        painter.circle_filled(center, 2.0, Color32::WHITE);
        // How far the bow is drawn, as a ring closing round the crosshair
        if let Some((_, weapon)) = self.held()
            && weapon.kind() == Kind::Ranged
        {
            let draw = self.combat.draw(weapon) as f32;
            if draw > 0.0 {
                let points: Vec<Pos2> = (0..=32).map(|k| k as f32 / 32.0 * draw * std::f32::consts::TAU).map(|a| center + vec2(a.sin(), -a.cos()) * 14.0).collect();
                let tint = if draw >= 1.0 { Color32::from_rgb(255, 220, 120) } else { Color32::WHITE };
                painter.add(egui::Shape::line(points, Stroke::new(2.5f32, tint)));
            }
        }

        if let Some(hint) = self.target_hint() {
            let at = center + vec2(0.0, 28.0);
            painter.text(at + vec2(1.0, 1.0), Align2::CENTER_TOP, &hint, FontId::proportional(17.0), shadow);
            painter.text(at, Align2::CENTER_TOP, &hint, FontId::proportional(17.0), Color32::WHITE);
        }
        // The card of what the crosshair's on lying about, beside it: everything about it, and how
        // it compares with what's in hand
        if let Some(Target::Pickup(i)) = self.target
            && let Some(pickup) = self.pickups.get(i)
        {
            let item = pickup.item;
            let beside = on_screen(pickup.at(vpm)).map_or(center, |p| p.max(center - vec2(0.0, 220.0)).min(center + vec2(0.0, 120.0)));
            let at = pos2(center.x + 44.0, beside.y - 60.0).min(pos2(screen.right() - 330.0, screen.bottom() - 460.0)).max(screen.min + vec2(8.0, 8.0));
            egui::Area::new(egui::Id::new("item card")).fixed_pos(at).interactable(false).order(egui::Order::Foreground).show(ctx, |ui| {
                self.card_frame(item).show(ui, |ui| {
                    ui.set_width(290.0);
                    self.item_card(ui, item, 1);
                    ui.add_space(4.0);
                    ui.label(RichText::new(if matches!(item, Item::Weapon(_)) { "E  take it" } else { "E  pick it up" }).strong().color(Color32::WHITE));
                });
            });
        }

        // One panel across the top middle: the quick bar on the left, hunger and mana on the right
        let slot = 44.0;
        let gap = 6.0;
        let pad = 10.0;
        let shown = self.slots.len().min(HOTBAR);
        let slots_width = if self.slots.is_empty() { 120.0 } else { shown as f32 * (slot + gap) - gap };
        let bars_width = 170.0;
        let width = pad + slots_width + 20.0 + bars_width + pad;
        let panel = Rect::from_min_size(pos2(center.x - width / 2.0, screen.top() + 14.0), vec2(width, slot + 2.0 * pad));
        painter.rect_filled(panel, 12.0, Color32::from_black_alpha(120));

        let left = panel.left() + pad;
        if self.slots.is_empty() {
            painter.text(pos2(left + slots_width / 2.0, panel.center().y), Align2::CENTER_CENTER, "Nothing yet", FontId::proportional(14.0), faint);
        }
        for (i, &(item, count)) in self.slots.iter().enumerate().take(HOTBAR) {
            let rect = Rect::from_min_size(pos2(left + i as f32 * (slot + gap), panel.top() + pad), vec2(slot, slot));
            let selected = i == self.selected;
            let fill = match item {
                Item::Weapon(w) => {
                    let [r, g, b] = self.library.rarities[self.weapons[w].grade].color;
                    Color32::from_rgba_unmultiplied(r, g, b, if selected { 70 } else { 34 })
                }
                _ => Color32::from_white_alpha(if selected { 40 } else { 14 }),
            };
            painter.rect_filled(rect, 8.0, fill);
            if selected {
                painter.rect_stroke(rect, 8.0, Stroke::new(2.0f32, Color32::WHITE), StrokeKind::Inside);
            }
            self.draw_icon(&painter, item, rect.center() - vec2(0.0, 2.0), slot * 0.58);
            if !matches!(item, Item::Bucket | Item::Weapon(_)) {
                painter.text(rect.right_bottom() - vec2(5.0, 2.0), Align2::RIGHT_BOTTOM, count.to_string(), FontId::proportional(13.0), Color32::WHITE);
            }
            painter.text(rect.left_top() + vec2(4.0, 2.0), Align2::LEFT_TOP, (i + 1).to_string(), FontId::proportional(10.0), faint);
        }

        let bars_left = left + slots_width + 20.0;
        let bar = |y: f32, label: &str, fraction: f64, fill: Color32| {
            painter.text(pos2(bars_left, y), Align2::LEFT_TOP, label, FontId::proportional(11.0), faint);
            let back = Rect::from_min_size(pos2(bars_left + 52.0, y + 2.0), vec2(bars_width - 52.0, 9.0));
            painter.rect_filled(back, 4.5, Color32::from_white_alpha(25));
            painter.rect_filled(Rect::from_min_size(back.min, vec2(back.width() * fraction.clamp(0.0, 1.0) as f32, 9.0)), 4.5, fill);
        };
        let hunger = if self.hunger > 25.0 { Color32::from_rgb(232, 160, 60) } else { Color32::from_rgb(200, 90, 40) };
        // Health turns green while poisoned and orange while burning, saying so
        let (label, fill) = if self.poisoned > 0.0 {
            ("Poisoned", Color32::from_rgb(110, 200, 60))
        } else if self.burning > 0.0 {
            ("Burning", Color32::from_rgb(245, 130, 40))
        } else if self.chilled > 0.0 {
            ("Chilled", Color32::from_rgb(120, 200, 245))
        } else if self.snared > 0.0 {
            ("Webbed", Color32::from_rgb(220, 220, 205))
        } else {
            ("Health", Color32::from_rgb(225, 55, 60))
        };
        bar(panel.top() + pad - 4.0, label, self.health / MAX_HEALTH, fill);
        bar(panel.top() + pad + 13.0, "Hunger", self.hunger / 100.0, hunger);
        bar(panel.top() + pad + 30.0, "Mana", self.combat.mana / MAX_MANA, Color32::from_rgb(80, 140, 255));
        // Gold under the bars, and how much more there is in the pack under the quick bar
        let purse = pos2(panel.right() - 12.0, panel.bottom() + 5.0);
        painter.circle_filled(purse + vec2(-galley_width(&painter, &self.gold.to_string(), 15.0) - 10.0, 8.5), 6.0, GOLD);
        painter.text(purse + vec2(1.0, 1.0), Align2::RIGHT_TOP, self.gold.to_string(), FontId::proportional(15.0), shadow);
        painter.text(purse, Align2::RIGHT_TOP, self.gold.to_string(), FontId::proportional(15.0), GOLD);
        if self.slots.len() > HOTBAR {
            let more = format!("+{} in the pack  ·  I", self.slots.len() - HOTBAR);
            painter.text(pos2(panel.left() + 12.0, panel.bottom() + 6.0), Align2::LEFT_TOP, more, FontId::proportional(12.0), faint);
        }

        // The boss being fought, its name over a long bar along the bottom
        if let Some((foe, vitality, enraged, middle)) = self.foe(view.eye) {
            // Off the edge of the screen or behind, an arrow round the edge points the way to it,
            // with how far
            let inner = screen.shrink(50.0);
            if on_screen(middle).is_none_or(|p| !inner.contains(p)) {
                let local = view.rotation.inverse() * (middle - view.eye);
                let way = vec2(local.x as f32, -local.z as f32);
                let way = if way.length() < 1e-3 { vec2(0.0, 1.0) } else { way.normalized() };
                let reach = (inner.width() * 0.5 / way.x.abs().max(1e-3)).min(inner.height() * 0.5 / way.y.abs().max(1e-3));
                let tip = center + way * reach;
                let side = vec2(-way.y, way.x);
                let pulse = 0.75 + 0.25 * (self.combat.time() * 5.0).sin() as f32;
                let red = Color32::from_rgba_unmultiplied(225, 45, 40, (pulse * 240.0) as u8);
                painter.add(egui::Shape::convex_polygon(vec![tip, tip - way * 22.0 + side * 13.0, tip - way * 22.0 - side * 13.0], red, Stroke::new(1.5f32, Color32::from_black_alpha(160))));
                let meters = format!("{:.0} m", (middle - view.eye).norm() / vpm);
                let label = tip - way * 40.0;
                painter.text(label + vec2(1.0, 1.0), Align2::CENTER_CENTER, &meters, FontId::proportional(14.0), shadow);
                painter.text(label, Align2::CENTER_CENTER, &meters, FontId::proportional(14.0), Color32::WHITE);
            }
            let name = if foe.is_raid() { format!("RAID  ·  {}", foe.name()) } else { foe.name().to_string() };
            // A raid boss's bar is longer
            let width = (screen.width() * if foe.is_raid() { 0.6 } else { 0.42 }).clamp(260.0, 900.0);
            // Jolting on a big hit, flashing white, the health it's just lost showing pale behind
            let (_, _, trail, _, flash) = self.boss_bar;
            let flash = flash as f32;
            let jolt = (self.combat.time() * 90.0).sin() as f32 * 4.0 * flash;
            let back = Rect::from_center_size(pos2(center.x + jolt, screen.bottom() - 54.0), vec2(width, 12.0));
            let title = if enraged { format!("{name}  —  enraged") } else { name.to_string() };
            let at = pos2(center.x, back.top() - 6.0);
            painter.text(at + vec2(1.0, 1.0), Align2::CENTER_BOTTOM, &title, FontId::proportional(19.0), shadow);
            painter.text(at, Align2::CENTER_BOTTOM, &title, FontId::proportional(19.0), if enraged { Color32::from_rgb(255, 170, 120) } else { Color32::WHITE });
            painter.rect_filled(back.expand(2.0), 6.0, Color32::from_black_alpha(150));
            if flash > 0.0 {
                painter.rect_stroke(back.expand(2.0 + 3.0 * flash), 6.0, Stroke::new(2.0f32, Color32::from_white_alpha((flash * 200.0) as u8)), StrokeKind::Outside);
            }
            painter.rect_filled(Rect::from_min_size(back.min, vec2(width * trail.max(vitality) as f32, back.height())), 5.0, Color32::from_rgb(255, 225, 150));
            let red = |c: u8| (c as f32 + (255.0 - c as f32) * 0.8 * flash) as u8;
            painter.rect_filled(Rect::from_min_size(back.min, vec2(width * vitality as f32, back.height())), 5.0, Color32::from_rgb(red(190), red(40), red(35)));
            // Where a raid boss goes into its next phases, marked across the bar
            if foe.is_raid() {
                for mark in [0.66, 0.33] {
                    let x = back.left() + width * mark;
                    painter.line_segment([pos2(x, back.top() - 3.0), pos2(x, back.bottom() + 3.0)], Stroke::new(2.0f32, Color32::from_rgb(255, 215, 120)));
                }
            }
        }

        // Red round the edges when hurt, and pulsing faintly while health's low; pulsing green
        // while poisoned and orange while burning
        let low = if self.health < 30.0 { (0.5 + 0.5 * (self.combat.time() * 5.0).sin()) * (1.0 - self.health / 30.0) * 0.35 } else { 0.0 };
        let afflicted = if self.poisoned > 0.0 || self.burning > 0.0 || self.chilled > 0.0 { 0.2 + 0.12 * (self.combat.time() * 8.0).sin() } else { 0.0 };
        let red = (self.hurt_flash * 0.8).max(low).max(afflicted).min(1.0) as f32;
        let (r, g, b) = if self.poisoned > 0.0 {
            (50, 150, 20)
        } else if self.burning > 0.0 {
            (210, 90, 0)
        } else if self.chilled > 0.0 {
            (90, 170, 230)
        } else {
            (170, 0, 0)
        };
        if red > 0.01 {
            for k in 0..10 {
                let fade = 1.0 - k as f32 / 10.0;
                let edge = screen.shrink(k as f32 * 9.0);
                painter.rect_stroke(edge, 0.0, Stroke::new(9.0f32, Color32::from_rgba_unmultiplied(r, g, b, (red * fade * fade * 150.0) as u8)), StrokeKind::Inside);
            }
        }

        // Dead: the view going dark and red, what killed them across the middle, and once they
        // can, how to rise again. Then coming back once they have.
        if let Some((by, since)) = &self.dead {
            let dark = (*since / 1.6).min(1.0) as f32;
            painter.rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(22, 0, 0, (dark * 210.0) as u8));
            let band = Rect::from_center_size(screen.center() - vec2(0.0, 10.0), vec2(screen.width(), 170.0));
            for k in 0..12 {
                let fade = 1.0 - k as f32 / 12.0;
                painter.rect_filled(band.shrink2(vec2(0.0, k as f32 * 7.0)), 0.0, Color32::from_black_alpha((dark * fade * 26.0) as u8));
            }
            let words = ((*since - 0.5) / 1.3).clamp(0.0, 1.0) as f32;
            let at = pos2(center.x, screen.center().y - 34.0);
            let title = FontId::proportional(64.0);
            painter.text(at + vec2(3.0, 3.0), Align2::CENTER_CENTER, "Slain", title.clone(), Color32::from_black_alpha((words * 220.0) as u8));
            painter.text(at, Align2::CENTER_CENTER, "Slain", title, Color32::from_rgba_unmultiplied(196, 30, 28, (words * 255.0) as u8));
            let below = Color32::from_rgba_unmultiplied(225, 212, 200, (words * 230.0) as u8);
            painter.text(at + vec2(0.0, 50.0), Align2::CENTER_CENTER, format!("by the {by}"), FontId::proportional(20.0), below);
            if self.lost > 0 {
                painter.text(at + vec2(0.0, 76.0), Align2::CENTER_CENTER, format!("{} gold lies where you fell", self.lost), FontId::proportional(16.0), GOLD.gamma_multiply(words * 0.9));
            }
            if self.can_rise() {
                let pulse = 0.65 + 0.35 * (self.combat.time() * 3.0).sin() as f32;
                let prompt = if self.rest.is_some() { "Space  rise again where you last stood safe" } else { "Space  rise again" };
                painter.text(pos2(center.x, band.bottom() + 40.0), Align2::CENTER_CENTER, prompt, FontId::proportional(18.0), Color32::from_white_alpha((pulse * 220.0) as u8));
            }
        } else if self.down < DOWN_SECONDS {
            let dark = (1.0 - self.down / DOWN_SECONDS).powf(1.5) as f32;
            painter.rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(22, 0, 0, (dark * 210.0) as u8));
        }

        // The player's own blood splashed over the view, running down a little and fading
        let h = screen.height();
        for s in &self.splats {
            let fade = (1.0 - s.age / SPLAT_SECONDS).clamp(0.0, 1.0) as f32;
            let blood = Color32::from_rgba_unmultiplied(105, 4, 6, (fade.sqrt() * 190.0) as u8);
            let r = s.size * h;
            let at = pos2(screen.left() + s.at[0] * screen.width(), screen.top() + s.at[1] * h);
            painter.circle_filled(at, r, blood);
            // Spattered round it
            let mut seed = s.seed;
            for _ in 0..7 {
                let a = random(&mut seed) as f32 * std::f32::consts::TAU;
                let d = r * (1.15 + 1.3 * random(&mut seed) as f32);
                painter.circle_filled(at + vec2(a.cos(), a.sin()) * d, r * (0.1 + 0.22 * random(&mut seed) as f32), blood);
            }
            // And running down from it
            let run = (s.age as f32 * 0.8).min(1.0) * r * 2.2;
            if run > 1.0 {
                painter.line_segment([at, at + vec2(0.0, r * 0.6 + run)], Stroke::new(r * 0.32, blood));
                painter.circle_filled(at + vec2(0.0, r * 0.6 + run), r * 0.2, blood);
            }
        }

        // The weapon in hand's name, in the color of its rarity, under the panel
        let mut below = panel.bottom() + 8.0;
        if let Some((_, weapon)) = self.held() {
            let rarity = &self.library.rarities[weapon.grade];
            let at = pos2(center.x, below);
            painter.text(at + vec2(1.0, 1.0), Align2::CENTER_TOP, &weapon.name, FontId::proportional(18.0), shadow);
            painter.text(at, Align2::CENTER_TOP, &weapon.name, FontId::proportional(18.0), color(rarity.color));
            let kind = &self.library.families[weapon.family].display_name;
            let detail = match self.combat.frenzy() {
                Some((bonus, left)) => format!("{} {}  ·  Frenzy +{:.0}% ({:.0} s)", rarity.display_name, kind, bonus * 100.0, left.ceil()),
                None => format!("{} {}", rarity.display_name, kind),
            };
            painter.text(at + vec2(0.0, 21.0), Align2::CENTER_TOP, detail, FontId::proportional(12.0), faint);
            below += 42.0;
            // Its special attack, filling as it charges, glowing once it's ready
            if let Some((special, _)) = &weapon.profile.special {
                let charge = self.combat.charge as f32;
                let ready = charge >= 1.0;
                let label = if ready { format!("R — {} ready!", special.name()) } else { format!("R — {}", special.name()) };
                let tint = if ready { color(rarity.color) } else { faint };
                painter.text(pos2(center.x, below) + vec2(1.0, 1.0), Align2::CENTER_TOP, &label, FontId::proportional(14.0), shadow);
                painter.text(pos2(center.x, below), Align2::CENTER_TOP, &label, FontId::proportional(14.0), tint);
                let back = Rect::from_center_size(pos2(center.x, below + 22.0), vec2(150.0, 5.0));
                painter.rect_filled(back, 2.5, Color32::from_white_alpha(30));
                painter.rect_filled(Rect::from_min_size(back.min, vec2(150.0 * charge, 5.0)), 2.5, color(rarity.color));
                below += 32.0;
            }
        }

        // Messages under that, newest first, fading out
        for (k, (message, at)) in self.messages.iter().rev().enumerate() {
            let alpha = (1.0 - (at.elapsed().as_secs_f32() - 3.0).max(0.0)).clamp(0.0, 1.0);
            let pos = pos2(center.x, below + 4.0 + k as f32 * 24.0);
            painter.text(pos + vec2(1.0, 1.0), Align2::CENTER_TOP, message, FontId::proportional(17.0), Color32::from_black_alpha((150.0 * alpha) as u8));
            painter.text(pos, Align2::CENTER_TOP, message, FontId::proportional(17.0), Color32::from_white_alpha((255.0 * alpha) as u8));
        }
    }

    /// The inventory: everything carried as a grid of icons, the quick bar along the top, and the
    /// card of what's clicked on, with what can be done with it
    pub fn inventory(&mut self, ctx: &egui::Context, open: &mut bool) {
        let mut chosen = None;
        let mut deed = None;
        egui::Window::new("Inventory")
            .open(open)
            .collapsible(false)
            .resizable(false)
            // Just under the quick bar
            .anchor(Align2::CENTER_TOP, [0.0, 100.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{} gold", self.gold)).color(GOLD).strong());
                    ui.separator();
                    ui.label(format!("Health {:.0}    Hunger {:.0}    Mana {:.0}", self.health, self.hunger, self.combat.mana));
                    ui.separator();
                    if ui.button("Sort the pack").on_hover_text("The rarest weapons first, then food. The quick bar stays as it is.").clicked() {
                        deed = Some(Deal::Sort);
                    }
                });
                ui.separator();
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Quick bar, keys 1 to 9, then the pack").weak().small());
                        egui::ScrollArea::vertical().max_height(ctx.screen_rect().height() * 0.6).show(ui, |ui| {
                            if let Some((slot, twice)) = self.slot_grid(ui, "carried", false) {
                                chosen = Some(slot);
                                if twice {
                                    deed = Some(if self.slots[slot].0.consumable() { Deal::Eat(slot) } else { Deal::Hold(slot) });
                                }
                            }
                        });
                    });
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.set_width(310.0);
                        let Some((slot, (item, count))) = self.inspected.and_then(|i| Some((i, *self.slots.get(i)?))) else {
                            ui.label(RichText::new("Click on something to see all about it. Double click to take it in hand, or eat it.").weak());
                            return;
                        };
                        self.item_card(ui, item, count);
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if !matches!(item, Item::Gold(_)) && ui.add_enabled(slot != self.selected, egui::Button::new("Hold")).clicked() {
                                deed = Some(Deal::Hold(slot));
                            }
                            if item.consumable() && ui.button(if item.heal() > 0.0 { "Drink" } else { "Eat" }).clicked() {
                                deed = Some(Deal::Eat(slot));
                            }
                            if item != Item::Bucket && ui.button("Drop").clicked() {
                                deed = Some(Deal::Drop(slot));
                            }
                        });
                        if let Item::Weapon(w) = item {
                            egui::CollapsingHeader::new("What it's made of").id_salt(("parts", w)).show(ui, |ui| self.weapon_parts(ui, &self.weapons[w]));
                        }
                    });
                });
                ui.separator();
                ui.label(RichText::new("Melee: click to swing, three in a row for a combo, right click for a heavy blow. Bow: hold right click to draw, click to shoot. Staff, scepter or wand: click to cast.").weak().small());
                ui.label(RichText::new("R or the middle button: a legendary or mythic part's special attack, once charged. Bosses drop loot, merchants buy and sell it.").weak().small());
            });
        if let Some(slot) = chosen {
            self.inspected = Some(slot);
        }
        match deed {
            Some(Deal::Hold(slot)) => self.hold(slot),
            Some(Deal::Eat(slot)) => self.eat(slot),
            Some(Deal::Drop(slot)) => self.drop_slot(slot),
            Some(Deal::Sort) => self.sort(),
            _ => {}
        }
    }

    /// What's carried as a grid of icons, a row of the quick bar first: which one was clicked,
    /// and whether twice. When trading, `selling` shows what the merchant pays for each on hover.
    fn slot_grid(&self, ui: &mut egui::Ui, id: &str, selling: bool) -> Option<(usize, bool)> {
        let mut clicked = None;
        let cell = 48.0;
        egui::Grid::new(id).spacing([5.0, 5.0]).show(ui, |ui| {
            for (i, &(item, count)) in self.slots.iter().enumerate() {
                let (rect, response) = ui.allocate_exact_size(vec2(cell, cell), egui::Sense::click());
                let painter = ui.painter();
                let tint = match item {
                    Item::Weapon(w) => Some(color(self.library.rarities[self.weapons[w].grade].color)),
                    _ => None,
                };
                let fill = tint.map_or(Color32::from_white_alpha(if i < HOTBAR { 22 } else { 12 }), |c| c.gamma_multiply(if i < HOTBAR { 0.3 } else { 0.2 }));
                painter.rect_filled(rect, 7.0, fill);
                if let Some(c) = tint {
                    painter.rect_stroke(rect, 7.0, Stroke::new(1.0f32, c.gamma_multiply(0.8)), StrokeKind::Inside);
                }
                if response.hovered() {
                    painter.rect_filled(rect, 7.0, Color32::from_white_alpha(18));
                }
                if self.inspected == Some(i) {
                    painter.rect_stroke(rect, 7.0, Stroke::new(2.0f32, GOLD), StrokeKind::Inside);
                } else if self.selected == i {
                    painter.rect_stroke(rect, 7.0, Stroke::new(2.0f32, Color32::WHITE), StrokeKind::Inside);
                }
                self.draw_icon(painter, item, rect.center() - vec2(0.0, 1.0), cell * 0.56);
                if !matches!(item, Item::Bucket | Item::Weapon(_)) {
                    painter.text(rect.right_bottom() - vec2(4.0, 2.0), Align2::RIGHT_BOTTOM, count.to_string(), FontId::proportional(12.0), Color32::WHITE);
                }
                if i < HOTBAR {
                    painter.text(rect.left_top() + vec2(4.0, 2.0), Align2::LEFT_TOP, (i + 1).to_string(), FontId::proportional(10.0), Color32::from_white_alpha(140));
                }
                let response = response.on_hover_ui(|ui| {
                    ui.set_width(280.0);
                    self.item_card(ui, item, count);
                    if selling && item != Item::Bucket {
                        ui.label(RichText::new(format!("Click to sell one for {} gold", self.sell_price(item))).strong().color(GOLD));
                    }
                });
                if response.double_clicked() {
                    clicked = Some((i, true));
                } else if response.clicked() {
                    clicked = Some((i, false));
                }
                if (i + 1) % HOTBAR == 0 {
                    ui.end_row();
                }
            }
        });
        clicked
    }

    /// The frame round an item's card: dark, edged in the color of a weapon's rarity
    fn card_frame(&self, item: Item) -> egui::Frame {
        let edge = match item {
            Item::Weapon(w) => color(self.library.rarities[self.weapons[w].grade].color),
            Item::Gold(_) => GOLD,
            _ => Color32::from_gray(150),
        };
        egui::Frame::new().fill(Color32::from_rgba_unmultiplied(14, 16, 22, 232)).stroke(Stroke::new(1.5f32, edge)).corner_radius(8.0).inner_margin(12.0)
    }

    /// Everything about `item`, `count` of them: what it is, what it does, and what it's worth. A
    /// weapon's compared with the one in hand.
    fn item_card(&self, ui: &mut egui::Ui, item: Item, count: u32) {
        if let Item::Weapon(w) = item {
            self.weapon_card(ui, w);
            return;
        }
        let good = Color32::from_rgb(120, 230, 110);
        match item {
            Item::Gold(n) => {
                ui.label(RichText::new(format!("{n} gold")).size(19.0).strong().color(GOLD));
                ui.label(RichText::new("Coin, to spend at merchants").weak());
                return;
            }
            Item::Bucket => {
                ui.label(RichText::new("Bucket").size(19.0).strong().color(Color32::WHITE));
                ui.label(RichText::new("A magic bucket").weak());
                ui.separator();
                ui.label(format!("{:.1} of {:.0} m³ of water in it", self.bucket, BUCKET_CAPACITY));
                ui.label(RichText::new("Click on water to fill it, right click to pour it out.").weak());
                return;
            }
            _ => {}
        }
        ui.label(RichText::new(item.name()).size(19.0).strong().color(Color32::WHITE));
        ui.label(RichText::new(if item.heal() > 0.0 { "Potion" } else { "Food" }).weak());
        ui.separator();
        if item.food() > 0.0 {
            ui.label(RichText::new(format!("+{:.0} hunger", item.food())).color(good).size(16.0));
            ui.label(RichText::new(format!("+{:.0} health", item.food() * 0.6)).color(good));
        }
        if item.heal() > 0.0 {
            ui.label(RichText::new(format!("+{:.0} health, at once", item.heal())).color(good).size(16.0));
        }
        if count > 1 {
            ui.label(RichText::new(format!("{count} carried")).weak());
        }
        ui.separator();
        ui.label(RichText::new(format!("Worth {} gold", self.price(item))).color(GOLD));
    }

    /// Everything about weapon `w`: its rarity and kind, the damage it does, its stats beside the
    /// weapon in hand's if it's another of the same kind, its special, effects and synergies, and
    /// what it's worth
    fn weapon_card(&self, ui: &mut egui::Ui, w: usize) {
        let library = &self.library;
        let weapon = &self.weapons[w];
        let rarity = &library.rarities[weapon.grade];
        let profile = &weapon.profile;
        let other = self.held().filter(|&(i, held)| i != w && held.kind() == weapon.kind()).map(|(_, held)| held);
        let (good, bad) = (Color32::from_rgb(120, 230, 110), Color32::from_rgb(240, 95, 85));
        // How much better or worse it is than the one in hand, nothing if about the same
        let against = |ui: &mut egui::Ui, by: f64, unit: Unit, higher: bool| {
            let least = match unit {
                Unit::Number | Unit::Speed => 0.5,
                Unit::Rate | Unit::Times | Unit::Percent => 0.005,
                Unit::Meters => 0.05,
            };
            if by.abs() < least {
                ui.label("");
                return;
            }
            let shown = match unit {
                Unit::Times => format!("{:.2}", by.abs()),
                _ => unit.show(by.abs()),
            };
            let better = (by > 0.0) == higher;
            ui.label(RichText::new(format!("{}{shown}", if by > 0.0 { "+" } else { "−" })).color(if better { good } else { bad }).strong());
        };
        ui.label(RichText::new(&weapon.name).size(19.0).strong().color(color(rarity.color)));
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} {}", rarity.display_name, library.families[weapon.family].display_name)).color(color(rarity.color).gamma_multiply(0.8)));
            if let Some(element) = profile.element {
                ui.label(RichText::new(format!("·  {}", element.name())).color(color(element.color())));
            }
        });
        ui.separator();
        let damage = |p: &Profile| ["physicalDamage", "elementalDamage", "spellDamage"].iter().map(|k| p.stat(k)).sum::<f64>();
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{:.0}", damage(profile))).size(32.0).strong().color(Color32::WHITE));
            ui.vertical(|ui| {
                ui.add_space(4.0);
                ui.label(RichText::new("damage").weak());
                if let Some(other) = other {
                    against(ui, damage(profile) - damage(&other.profile), Unit::Number, true);
                }
            });
        });
        egui::Grid::new(("card", w)).num_columns(3).spacing([14.0, 1.0]).show(ui, |ui| {
            for (key, label, unit) in STATS {
                let value = profile.stat(key);
                if value.abs() < 1e-6 && other.is_none_or(|o| o.profile.stat(key).abs() < 1e-6) {
                    continue;
                }
                if key == "elementalDamage" {
                    let tint = profile.element.map_or(Color32::GRAY, |e| color(e.color()));
                    ui.label(RichText::new(format!("{} damage", profile.element.map_or("Elemental", Element::name))).color(tint));
                    ui.label(RichText::new(unit.show(value)).color(tint));
                } else {
                    ui.label(RichText::new(label).weak());
                    ui.label(unit.show(value));
                }
                match other {
                    Some(other) => against(ui, value - other.profile.stat(key), unit, key != "manaCost"),
                    None => {
                        ui.label("");
                    }
                }
                ui.end_row();
            }
        });
        if let Some((special, part)) = &profile.special {
            ui.add_space(4.0);
            ui.label(RichText::new(format!("R — {}", special.name())).color(color(rarity.color)).strong());
            ui.label(RichText::new(format!("{} From its {}.", special.description(), part)).weak());
        }
        if !profile.effects.is_empty() {
            ui.add_space(4.0);
            for e in &profile.effects {
                let def = &library.effects[e.def];
                let strength = if e.potency > 1.0 { format!(" ×{:.1}", e.potency) } else { String::new() };
                ui.label(RichText::new(format!("• {}{}: {}", def.display_name, strength, def.description)).color(Color32::from_rgb(200, 220, 255)));
            }
        }
        for s in &profile.synergies {
            let def = &library.synergies[s.def];
            ui.label(RichText::new(format!("★ {}: {}", def.display_name, def.description)).color(Color32::from_rgb(255, 225, 160)));
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Worth {} gold", self.price(Item::Weapon(w)))).color(GOLD));
            if let Some(other) = other {
                ui.label(RichText::new(format!("against {}", other.name)).weak().small());
            }
        });
    }

    /// What weapon `weapon` is made of, part by part, and its tags
    fn weapon_parts(&self, ui: &mut egui::Ui, weapon: &Weapon) {
        let library = &self.library;
        let family = &library.families[weapon.family];
        for (slot, &part) in family.slots.iter().zip(&weapon.parts) {
            let c = &library.components[part];
            let rarity = &library.rarities[c.tier];
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}:", capitalized(slot))).weak());
                ui.label(RichText::new(&c.display_name).color(color(rarity.color)));
                ui.label(RichText::new(format!("({})", rarity.display_name)).weak().small());
                if c.level_requirement > 1 {
                    ui.label(RichText::new(format!("level {}", c.level_requirement)).weak().small());
                }
            });
        }
        let tags: Vec<&str> = weapon.tags.iter().map(String::as_str).collect();
        ui.label(RichText::new(format!("Tags: {}", tags.join(", "))).weak().small());
    }

    /// The merchant's stall: what they sell, and what the player carries to sell them. False once
    /// it's closed.
    pub fn shop_window(&mut self, ctx: &egui::Context) -> bool {
        let Some(key) = self.shop else {
            return false;
        };
        let mut open = true;
        let mut deed = None;
        egui::Window::new("Merchant")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_TOP, [0.0, 100.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{} gold", self.gold)).color(GOLD).strong().size(16.0));
                    ui.separator();
                    ui.label(RichText::new("Hover over anything to see all about it").weak());
                });
                ui.separator();
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(360.0);
                        ui.label(RichText::new("For sale").strong());
                        egui::ScrollArea::vertical().id_salt("wares").max_height(ctx.screen_rect().height() * 0.55).show(ui, |ui| {
                            for (k, &ware) in self.stocks.get(&key).into_iter().flatten().enumerate() {
                                let (item, left) = match ware {
                                    Ware::Weapon(w) => (Item::Weapon(w), 1),
                                    Ware::Food(item, left) => (item, left),
                                };
                                let cost = self.price(item);
                                let row = ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(vec2(38.0, 38.0), egui::Sense::hover());
                                    self.draw_icon(ui.painter(), item, rect.center(), 26.0);
                                    let name = match item {
                                        Item::Weapon(w) => RichText::new(&self.weapons[w].name).color(color(self.library.rarities[self.weapons[w].grade].color)).strong(),
                                        _ => RichText::new(format!("{}  ×{left}", item.name())),
                                    };
                                    ui.label(name);
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        let button = egui::Button::new(RichText::new(format!("Buy  {cost}")).color(if cost <= self.gold { GOLD } else { Color32::GRAY }));
                                        if ui.add_enabled(cost <= self.gold, button).clicked() {
                                            deed = Some(Deal::Buy(k));
                                        }
                                    });
                                });
                                row.response.on_hover_ui(|ui| {
                                    ui.set_width(280.0);
                                    self.item_card(ui, item, 1);
                                });
                            }
                        });
                    });
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Yours, click to sell one").strong());
                        if let Some((slot, _)) = self.slot_grid(ui, "selling", true) {
                            deed = Some(Deal::Sell(slot));
                        }
                    });
                });
            });
        match deed {
            Some(Deal::Buy(k)) => self.buy(k),
            Some(Deal::Sell(slot)) => self.sell(slot),
            _ => {}
        }
        if !open {
            self.shop = None;
        }
        self.shop.is_some()
    }

    /// A little picture of an item, `size` across
    fn draw_icon(&self, painter: &egui::Painter, item: Item, center: Pos2, size: f32) {
        let r = size * 0.5;
        match item {
            Item::Bucket => {
                let full = (self.bucket / BUCKET_CAPACITY) as f32;
                let body = Rect::from_center_size(center + vec2(0.0, r * 0.15), vec2(r * 1.4, r * 1.4));
                painter.rect_filled(body, r * 0.2, Color32::from_rgb(150, 155, 165));
                let water = Rect::from_min_max(
                    pos2(body.left() + r * 0.15, body.bottom() - r * 0.15 - (body.height() - r * 0.3) * full),
                    body.max - vec2(r * 0.15, r * 0.15),
                );
                if full > 0.0 {
                    painter.rect_filled(water, r * 0.1, Color32::from_rgb(60, 150, 230));
                }
                painter.circle_stroke(center - vec2(0.0, r * 0.45), r * 0.55, Stroke::new(r * 0.1, Color32::from_rgb(120, 125, 135)));
            }
            Item::Apple => {
                painter.circle_filled(center + vec2(0.0, r * 0.12), r * 0.78, Color32::from_rgb(210, 30, 36));
                painter.circle_filled(center + vec2(-r * 0.25, -r * 0.1), r * 0.2, Color32::from_rgb(240, 110, 110));
                painter.line_segment([center - vec2(0.0, r * 0.55), center - vec2(-r * 0.1, r * 0.95)], Stroke::new(r * 0.12, Color32::from_rgb(110, 70, 40)));
                painter.circle_filled(center + vec2(r * 0.32, -r * 0.8), r * 0.2, Color32::from_rgb(80, 170, 60));
            }
            Item::Bread => {
                let crust = Color32::from_rgb(196, 132, 62);
                painter.rect_filled(Rect::from_center_size(center + vec2(0.0, r * 0.12), vec2(r * 1.8, r * 1.0)), r * 0.48, crust);
                painter.rect_filled(Rect::from_center_size(center + vec2(0.0, -r * 0.05), vec2(r * 1.5, r * 0.5)), r * 0.25, Color32::from_rgb(222, 166, 92));
                for k in [-0.45, 0.0, 0.45] {
                    painter.line_segment([center + vec2(r * (k - 0.15), -r * 0.2), center + vec2(r * (k + 0.15), r * 0.15)], Stroke::new(r * 0.09, Color32::from_rgb(150, 92, 40)));
                }
            }
            Item::Potion => {
                let glass = Color32::from_rgb(190, 215, 230);
                painter.circle_filled(center + vec2(0.0, r * 0.3), r * 0.68, Color32::from_rgb(200, 24, 44));
                painter.circle_stroke(center + vec2(0.0, r * 0.3), r * 0.7, Stroke::new(r * 0.09, glass));
                painter.circle_filled(center + vec2(-r * 0.25, r * 0.1), r * 0.15, Color32::from_rgb(255, 140, 150));
                painter.rect_filled(Rect::from_center_size(center - vec2(0.0, r * 0.55), vec2(r * 0.42, r * 0.5)), r * 0.06, glass);
                painter.rect_filled(Rect::from_center_size(center - vec2(0.0, r * 0.85), vec2(r * 0.5, r * 0.22)), r * 0.05, Color32::from_rgb(140, 95, 55));
            }
            Item::Gold(_) => {
                for (dx, dy) in [(-0.35, 0.35), (0.35, 0.3), (0.0, -0.05)] {
                    let at = center + vec2(r * dx, r * dy);
                    painter.circle_filled(at, r * 0.5, Color32::from_rgb(200, 150, 30));
                    painter.circle_filled(at - vec2(0.0, r * 0.06), r * 0.44, GOLD);
                    painter.circle_stroke(at - vec2(0.0, r * 0.06), r * 0.28, Stroke::new(r * 0.07, Color32::from_rgb(220, 165, 40)));
                }
            }
            Item::Meat => {
                let bone = Color32::from_rgb(240, 232, 214);
                painter.line_segment([center + vec2(-r * 0.85, r * 0.55), center + vec2(-r * 0.1, -r * 0.05)], Stroke::new(r * 0.22, bone));
                painter.circle_filled(center + vec2(-r * 0.95, r * 0.42), r * 0.16, bone);
                painter.circle_filled(center + vec2(-r * 0.75, r * 0.72), r * 0.16, bone);
                painter.circle_filled(center + vec2(r * 0.2, -r * 0.25), r * 0.62, Color32::from_rgb(175, 60, 52));
                painter.circle_filled(center + vec2(r * 0.05, -r * 0.4), r * 0.22, Color32::from_rgb(215, 110, 100));
            }
            Item::Weapon(w) => {
                let weapon = &self.weapons[w];
                let glow = weapon.profile.element.map_or(Color32::from_rgb(150, 220, 255), |e| color(e.color()));
                let metal = Color32::from_rgb(210, 214, 222);
                let wood = Color32::from_rgb(150, 100, 60);
                let haft = Color32::from_rgb(130, 88, 52);
                let family = self.library.families[weapon.family].id.as_str();
                match weapon.kind() {
                    // A long shaft from bottom left to top right, with each one's own head on it
                    Kind::Melee if matches!(family, "axe" | "hammer" | "spear" | "halberd" | "scythe") => {
                        let (foot, top) = (center + vec2(-r * 0.8, r * 0.8), center + vec2(r * 0.6, -r * 0.6));
                        painter.line_segment([foot, top + vec2(r * 0.15, -r * 0.15)], Stroke::new(r * 0.14, haft));
                        let edge = |points: Vec<Pos2>| painter.add(egui::Shape::convex_polygon(points, metal, Stroke::NONE));
                        match family {
                            "axe" => {
                                edge(vec![top + vec2(-r * 0.15, -r * 0.05), top + vec2(r * 0.3, r * 0.5), top + vec2(r * 0.55, r * 0.05), top + vec2(r * 0.15, -r * 0.35)]);
                            }
                            "hammer" => {
                                let along = vec2(r * 0.7, r * 0.7) * 0.5;
                                painter.line_segment([top - along, top + along], Stroke::new(r * 0.42, metal));
                            }
                            "scythe" => {
                                let points: Vec<Pos2> = (0..=10).map(|k| {
                                    let a = k as f32 / 10.0 * 1.9 + 0.6;
                                    top + vec2(-a.cos() * r * 0.95 + r * 0.15, -a.sin() * r * 0.55 + r * 0.25)
                                }).collect();
                                painter.add(egui::Shape::line(points, Stroke::new(r * 0.16, metal)));
                            }
                            _ => {
                                let tip = top + vec2(r * 0.35, -r * 0.35);
                                edge(vec![top + vec2(-r * 0.12, r * 0.0), tip, top + vec2(r * 0.0, r * 0.12)]);
                                if family == "halberd" {
                                    edge(vec![top + vec2(-r * 0.05, r * 0.05), top + vec2(r * 0.35, r * 0.25), top + vec2(r * 0.15, r * 0.45)]);
                                }
                            }
                        }
                        if weapon.profile.element.is_some() {
                            painter.circle_filled(top, r * 0.12, glow);
                        }
                    }
                    // A coiled lash on a short handle
                    Kind::Melee if family == "whip" => {
                        painter.line_segment([center + vec2(-r * 0.85, r * 0.85), center + vec2(-r * 0.4, r * 0.4)], Stroke::new(r * 0.18, haft));
                        let points: Vec<Pos2> = (0..=28).map(|k| {
                            let t = k as f32 / 28.0;
                            let a = t * 9.0;
                            let radius = r * (0.25 + 0.45 * t);
                            center + vec2(r * 0.1, -r * 0.1) + vec2(a.cos(), a.sin()) * radius
                        }).collect();
                        painter.add(egui::Shape::line(points, Stroke::new(r * 0.09, if weapon.profile.element.is_some() { glow } else { Color32::from_rgb(110, 70, 45) })));
                    }
                    Kind::Melee => {
                        // Daggers short, greatswords long and broad
                        let (reach, broad) = match family {
                            "dagger" => (0.45, 0.2),
                            "greatsword" => (0.95, 0.32),
                            "katana" => (0.9, 0.16),
                            _ => (0.85, 0.24),
                        };
                        let (hilt, tip) = (center + vec2(-r * 0.7, r * 0.7), center + vec2(r * reach, -r * reach));
                        let guard = center + vec2(-r * 0.35, r * 0.35);
                        painter.line_segment([guard, tip], Stroke::new(r * broad, metal));
                        if weapon.profile.element.is_some() {
                            painter.line_segment([guard, guard.lerp(tip, 0.7)], Stroke::new(r * 0.08, glow));
                        }
                        let span = if family == "katana" { 0.15 } else { 0.3 };
                        painter.line_segment([guard + vec2(-r * span, -r * span), guard + vec2(r * span, r * span)], Stroke::new(r * 0.16, Color32::from_rgb(200, 170, 80)));
                        painter.line_segment([hilt, guard], Stroke::new(r * 0.16, Color32::from_rgb(110, 70, 45)));
                    }
                    Kind::Ranged => {
                        let points: Vec<Pos2> = (0..=16).map(|k| {
                            let a = (k as f32 / 16.0 - 0.5) * 2.4;
                            center + vec2(a.sin() * r * 0.9 - r * 0.25, -a.cos() * r * 0.9 + r * 0.55)
                        }).collect();
                        let (top, bottom) = (points[0], points[16]);
                        painter.add(egui::Shape::line(points, Stroke::new(r * 0.16, wood)));
                        painter.line_segment([top, bottom], Stroke::new(r * 0.05, Color32::from_rgb(235, 230, 210)));
                        painter.line_segment([center + vec2(-r * 0.6, 0.0), center + vec2(r * 0.85, 0.0)], Stroke::new(r * 0.07, glow));
                    }
                    Kind::Magic => {
                        painter.line_segment([center + vec2(-r * 0.6, r * 0.9), center + vec2(r * 0.4, -r * 0.5)], Stroke::new(r * 0.16, wood));
                        painter.circle_filled(center + vec2(r * 0.55, -r * 0.7), r * 0.32, glow);
                        painter.circle_stroke(center + vec2(r * 0.55, -r * 0.7), r * 0.42, Stroke::new(r * 0.08, metal));
                    }
                }
            }
        }
    }
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or(String::new(), |c| c.to_uppercase().chain(chars).collect())
}

/// Weapon model `model`, one of a weapon's frames at `res` voxels a meter, drawn with its point
/// `point`, in meters in its own frame, at `at` in voxels of the world, turned by `rotation`
fn placed(model: u32, (m, anchor): &(Model, Vector3<f64>), res: f64, point: Vector3<f64>, at: Vector3<f64>, rotation: UnitQuaternion<f64>, vpm: f64) -> AnimalInstance {
    let [x, y, z] = m.size.map(|s| s as f64 / res);
    let reach = (x * x * 0.25 + y * y * 0.25 + z * z).sqrt() * vpm;
    AnimalInstance::new(model, at - rotation * (point - anchor) * vpm, rotation, vpm / res, reach, animals::MOVING)
}
