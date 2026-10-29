//! Living off the land: food to find and eat, an inventory to carry it in, animals to hunt or pet,
//! weapons to find in chests and fight with, and the heads-up display that shows it all. Positions
//! are in voxels of the main world.

use std::collections::HashSet;
use std::f64::consts::TAU;
use std::time::Instant;

use egui_winit_vulkano::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, StrokeKind, pos2, vec2};
use nalgebra::Vector3;

use crate::animals::{self, ANIMAL_MODELS, Animal, AnimalInstance, Build, Look, Model, pack_models};
use crate::audio::{Cue, Ground, Sound};
use crate::bosses::{Deed, Move};
use crate::combat::{Arena, Combat, Controls, HostileKind, MAX_MANA, View};
use crate::leaves::Leaves;
use crate::player::WaterLevel;
use crate::weapons::{DROP_EMBER, DROP_ICE, DROP_MUD, DROP_WATER, Element, Kind, Library, STATS, Special, Weapon};
use crate::worldgen::{World, is_passable, material::*};

/// How far away things can be picked up or hit, in meters
pub const REACH: f64 = 4.0;
/// Hunger drops from full to empty in this many seconds
const HUNGER_SECONDS: f64 = 600.0;
/// Animals further than this aren't drawn, and food further than that, in meters
const ANIMAL_RANGE: f64 = 140.0;
const FOOD_RANGE: f64 = 60.0;
const PROP_RANGE: f64 = 30.0;
/// The most animals, bits of food, props and things flying drawn at once, nearest first
const MAX_INSTANCES: usize = 200;
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
    /// One of Game::weapons
    Weapon(usize),
}

impl Item {
    fn look(self) -> Look {
        match self {
            Item::Apple => Look::Apple,
            Item::Meat => Look::Meat,
            Item::Bucket | Item::Weapon(_) => unreachable!("the bucket and weapons are never left lying around"),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Item::Bucket => "Bucket",
            Item::Apple => "Apple",
            Item::Meat => "Raw Meat",
            Item::Weapon(_) => "Weapon",
        }
    }

    /// How much hunger eating one takes away, out of 100
    fn food(self) -> f64 {
        match self {
            Item::Bucket | Item::Weapon(_) => 0.0,
            Item::Apple => 12.0,
            Item::Meat => 30.0,
        }
    }
}

/// Food lying around, or hanging in a tree
struct Pickup {
    item: Item,
    position: Vector3<f64>,
    heading: f64,
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
    /// How hard blows throw the player, in voxels a second, for main.rs to apply
    knockback: Vector3<f64>,
    /// The camera shaking from what bosses do, wearing off
    shake: f64,
    /// The player was beaten, and should wake up somewhere safe
    beaten: bool,
    /// Where the player last stood well away from any fight, and how long until it's looked at again
    safe: Option<Vector3<f64>>,
    safe_timer: f64,
    /// Bosses' lairs in this piece of the world, and those beaten, by where they are in the landscape
    lairs: Vec<Lair>,
    defeated: HashSet<[i64; 3]>,
    /// Chests beaten bosses left, kept as the world moves on
    boss_chests: Vec<Chest>,
    /// Cubic meters of water in the bucket
    bucket: f64,
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
            return Some(Pickup {
                item: Item::Apple,
                position: Vector3::new(x, y, height),
                heading: random(rng) * TAU,
            });
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
            burning: 0.0,
            poisoned: 0.0,
            chilled: 0.0,
            snared: 0.0,
            roots: Vec::new(),
            webs: Vec::new(),
            hurt_sound: 0.0,
            breath_sound: 0.0,
            knockback: Vector3::zeros(),
            shake: 0.0,
            beaten: false,
            safe: None,
            safe_timer: 0.0,
            lairs: Vec::new(),
            defeated: HashSet::new(),
            boss_chests: Vec::new(),
            bucket: 0.0,
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
        let families = ["sword", "bow", "staff", "axe", "hammer", "dagger", "spear", "wand", "greatsword", "katana", "scythe", "halberd", "scepter"];
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
        self.boss_chests.clear();
        self.safe = None;
        self.pickups.clear();
        self.target = None;
        self.seed = seed;
        self.combat.clear();
        self.leaves.clear();
        for _ in 0..4000 {
            if self.apple_count() >= APPLES {
                break;
            }
            self.grow_apple(world, water, near);
        }
    }

    /// Every model to draw, packed for shaders\traverse.comp: the animals' and everything lying
    /// around, the arrows, spells and arms, the falling leaves, then each weapon's
    pub fn model_words(&self) -> Vec<u32> {
        let models: Vec<&Model> = self
            .models
            .iter()
            .chain(self.combat.models.iter().map(|(m, _)| m))
            .chain(self.leaves.models.iter())
            .chain(self.weapons.iter().flat_map(|w| w.look.frames.iter().map(|(m, _)| m)))
            .collect();
        pack_models(&models)
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
        let bosses = self.animals.iter().filter(|a| a.boss.is_some()).count();
        let boss = self.animals.iter().filter(|a| a.boss.is_some()).min_by(|a, b| (a.position - near).norm().total_cmp(&(b.position - near).norm()));
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
            "holding {held}, {animal}, mana {:.0}, health {:.0}, {} animals ({bosses} bosses, {} lairs), {} leaves falling",
            self.combat.mana,
            self.health,
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
            let here = self.animals.iter().any(|a| a.boss.as_ref().is_some_and(|b| b.key == lair.key));
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
        if let Some(safe) = &mut self.safe {
            *safe -= shift;
        }
        self.knockback = Vector3::zeros();
        self.combat.recenter(shift);
        self.leaves.recenter(shift);
        self.target = None;

        let missing = ANIMALS.saturating_sub(self.animals.len());
        if missing > 0 {
            // Away from the player, so they don't pop up right in front of them
            let vpm = world.voxels_per_meter as f64;
            let fresh = animals::spawn(world, water, seed ^ hash(self.rng), near);
            self.animals.extend(fresh.into_iter().filter(|a| (a.position - near).norm() > 40.0 * vpm).take(missing));
        }
        self.rng = hash(self.rng);
    }

    /// The nearest animal, food or prop called `name` to `near`: where it stands, and how high its
    /// middle is in meters
    pub fn find_animal(&self, name: &str, near: Vector3<f64>) -> Option<(Vector3<f64>, f64)> {
        let animals = self.animals.iter().map(|a| (a.look, a.position));
        let pickups = self.pickups.iter().map(|p| (p.item.look(), p.position));
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

    /// Bits of what an explosion at `at` blew out of the world flying out
    pub fn debris(&mut self, at: Vector3<f64>, materials: &[u8], vpm: f64) {
        self.leaves.debris(at, materials, vpm);
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

        let held = match self.slots.get(self.selected) {
            Some(&(Item::Weapon(i), _)) => Some(i),
            _ => None,
        };
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
            self.carry_out(look, deed, world, player, view);
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
        self.look_after_player(world, player, dt);
        self.reap(world.voxels_per_meter as f64);
        self.leaves.update(world, water, view.eye, dt);

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

        self.target = self.find_target(world, view.eye, view.forward());
        self.messages.retain(|(_, at)| at.elapsed().as_secs_f64() < 4.0);
    }

    /// What a boss did: hurting the player, throwing things, sounds and shaking the ground
    fn carry_out(&mut self, look: Look, deed: Deed, world: &World, player: Option<Vector3<f64>>, view: &View) {
        let vpm = world.voxels_per_meter as f64;
        // How much something at `at` is felt, from 1 right there to nothing 40 meters off
        let felt = |at: Vector3<f64>| (1.0 - (at - view.eye).norm() / vpm / 40.0).max(0.0);
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
                self.cues.push(Cue::at(Sound::Burst, at, 1.0).pitched(0.45));
                self.shake = self.shake.max(0.6 * felt(at));
                if let Some(feet) = player {
                    let off = feet - at;
                    let d = off.xy().norm() / vpm;
                    if d < radius && off.z.abs() < 2.5 * vpm {
                        self.hurt(damage * (1.0 - 0.5 * d / radius), at, feet, world, look.name());
                        self.afflict(element);
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
                    if (from..to).contains(&d) && off.z.abs() < 3.0 * vpm && !airborne {
                        self.hurt(22.0, at, feet, world, look.name());
                        self.knockback += Vector3::new(0.0, 0.0, 5.0 * vpm);
                        self.afflict(element);
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
            Deed::Swipe(at, k) => self.cues.push(Cue::at(Sound::Claw, at, 0.85).pitched(0.9 + 0.1 * k as f32)),
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
                    self.knockback += (direction * 11.0 + Vector3::new(0.0, 0.0, 6.0)) * vpm;
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
                    let up = if root.drag == Vector3::zeros() { 9.0 } else { 5.0 };
                    self.knockback += (root.drag * 9.0 + Vector3::new(0.0, 0.0, up)) * vpm;
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
        let away = Vector3::new(feet.x - from.x, feet.y - from.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros);
        self.knockback += (away * (damage * 0.25).min(8.0) + Vector3::new(0.0, 0.0, (damage * 0.1).min(4.0))) * vpm;
        if self.health <= 0.0 {
            self.health = 0.0;
            self.beaten = true;
            self.say(format!("You were beaten by the {by}. You come to somewhere safer."));
            // The bosses calm down and go home to lick their wounds
            for animal in &mut self.animals {
                if let Some(boss) = &mut animal.boss {
                    boss.calm();
                    animal.health = animal.look.health();
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
    }

    /// Health coming back, burning, starving, and keeping track of where's safe to wake up
    fn look_after_player(&mut self, world: &World, player: Option<Vector3<f64>>, dt: f64) {
        self.since_hurt += dt;
        self.hurt_flash = (self.hurt_flash - dt * 1.5).max(0.0);
        self.hurt_sound -= dt;
        self.breath_sound -= dt;
        self.shake *= (-6.0 * dt).exp();
        self.chilled = (self.chilled - dt).max(0.0);
        let Some(feet) = player else {
            self.burning = 0.0;
            self.poisoned = 0.0;
            self.chilled = 0.0;
            self.snared = 0.0;
            self.roots.clear();
            self.webs.clear();
            return;
        };
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
        // Remembering where's safe: well away from any boss that's after them
        self.safe_timer -= dt;
        if self.safe_timer <= 0.0 {
            self.safe_timer = 2.0;
            let fighting = self.animals.iter().any(|a| a.boss.as_ref().is_some_and(|b| b.engaged));
            if !fighting && self.health > 30.0 {
                self.safe = Some(feet);
            }
        }
    }

    /// The boss fighting the player, if any, nearest first: what it's called, how much health it
    /// has left, 0-1, and whether it's enraged
    fn foe(&self, near: Vector3<f64>) -> Option<(&'static str, f64, bool)> {
        self.animals
            .iter()
            .filter(|a| a.health > 0.0 && a.boss.as_ref().is_some_and(|b| b.engaged))
            .min_by(|a, b| (a.position - near).norm().total_cmp(&(b.position - near).norm()))
            .map(|a| (a.look.name(), a.vitality(), a.boss.as_ref().is_some_and(|b| b.enraged)))
    }

    /// The middle of the body of the boss nearest `near`, if there's one about
    pub fn nearest_boss(&self, near: Vector3<f64>) -> Option<Vector3<f64>> {
        let vpm = self.combat.vpm();
        self.animals
            .iter()
            .filter(|a| a.boss.is_some() && a.health > 0.0)
            .min_by(|a, b| (a.position - near).norm().total_cmp(&(b.position - near).norm()))
            .map(|a| a.center(vpm as u32))
    }

    /// How the player's being thrown by blows since last time, in voxels a second
    pub fn take_knockback(&mut self) -> Vector3<f64> {
        std::mem::take(&mut self.knockback)
    }

    /// How hard the camera should shake now, 0 to about 1
    pub fn shake(&self) -> f64 {
        self.shake.max(self.combat.shake)
    }

    /// If the player was beaten, where to wake them up, their feet in voxels, or None to find
    /// somewhere near where they are
    pub fn take_beaten(&mut self) -> Option<Option<Vector3<f64>>> {
        if !self.beaten {
            return None;
        }
        self.beaten = false;
        self.health = MAX_HEALTH;
        self.burning = 0.0;
        self.poisoned = 0.0;
        self.chilled = 0.0;
        self.snared = 0.0;
        self.hunger = self.hunger.max(40.0);
        self.hurt_flash = 1.0;
        Some(self.safe)
    }

    /// Animals that were killed fall, and leave their meat. Bosses leave a chest.
    fn reap(&mut self, vpm: f64) {
        let mut k = 0;
        while k < self.animals.len() {
            if self.animals[k].health > 0.0 {
                k += 1;
                continue;
            }
            let animal = self.animals.remove(k);
            let look = animal.look;
            if let Some(boss) = &animal.boss {
                self.defeated.insert(boss.key);
                let chest = Chest {
                    position: animal.position,
                    heading: animal.heading(),
                    key: [boss.key[0], boss.key[1], boss.key[2] + 1_000_000],
                    luck: 6.0,
                };
                self.boss_chests.push(chest.clone());
                self.chests.push(chest);
                self.combat.fell(animal.position + Vector3::new(0.0, 0.0, look.body().0 * vpm), look.body().1);
                self.shake = self.shake.max(1.0);
                self.cues.push(Cue::here(Sound::Fanfare, 0.8));
                self.say(format!("You defeated the {}! It left a chest behind.", look.name()));
            }
            for m in 0..look.meat() {
                let angle = animal.heading() + m as f64 * 2.1;
                let spread = 0.25 * m as f64;
                self.pickups.push(Pickup {
                    item: Item::Meat,
                    position: animal.position + Vector3::new(angle.cos(), angle.sin(), 0.0) * spread,
                    heading: angle,
                });
            }
            if !look.is_boss() {
                self.say(if look.meat() > 0 { format!("You hunted a {}. It dropped some meat.", look.name()) } else { format!("You hunted a {}.", look.name()) });
            }
            self.target = None;
        }
    }

    /// The nearest animal, bit of food or chest the camera looks at, within reach
    fn find_target(&self, world: &World, eye: Vector3<f64>, look: Vector3<f64>) -> Option<Target> {
        let vpm = world.voxels_per_meter as f64;
        let mut best = REACH * vpm;
        let mut target = None;
        let mut consider = |position: Vector3<f64>, look_kind: Look, found: Target| {
            let (height, radius) = look_kind.body();
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
            consider(animal.position, animal.look, Target::Animal(i));
        }
        for (i, pickup) in self.pickups.iter().enumerate() {
            consider(pickup.position, pickup.item.look(), Target::Pickup(i));
        }
        for (i, chest) in self.chests.iter().enumerate() {
            if !self.opened.contains(&chest.key) {
                consider(chest.position, Look::Chest, Target::Chest(i));
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

    /// E: picks up food, opens chests, pets dogs and cats
    pub fn interact(&mut self, eye: Vector3<f64>) {
        match self.target {
            Some(Target::Pickup(i)) => {
                let item = self.pickups.remove(i).item;
                self.add_item(item, 1);
                self.target = None;
                self.cues.push(Cue::here(Sound::Pickup, 0.6));
                self.say(format!("+1 {}", item.name()));
            }
            Some(Target::Chest(i)) => self.open_chest(i),
            Some(Target::Animal(i)) => {
                let look = self.animals[i].look;
                if look.is_pet() {
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

    /// Opens chest `i` and takes the weapon in it, rolled from where the chest is
    fn open_chest(&mut self, i: usize) {
        let chest = &self.chests[i];
        let key = chest.key;
        let seed = hash(self.seed ^ hash(key[0] as u32 ^ hash(key[1] as u32 ^ hash(key[2] as u32))));
        let weapon = self.library.generate(seed, None, chest.luck);
        self.opened.insert(key);
        self.target = None;
        let rarity = &self.library.rarities[weapon.grade];
        let family = &self.library.families[weapon.family];
        let message = format!("You found {}, a {} {}!", weapon.name, rarity.display_name.to_lowercase(), family.display_name.to_lowercase());
        self.weapons.push(weapon);
        self.slots.push((Item::Weapon(self.weapons.len() - 1), 1));
        self.models_version += 1;
        self.cues.push(Cue::here(Sound::Chest, 0.8));
        self.say(message);
    }

    /// Right click without a weapon: eats one of what's selected in the quick bar
    pub fn eat_selected(&mut self) {
        if !self.slots.iter().any(|(item, _)| item.food() > 0.0) {
            self.say("You have nothing to eat. Look for apples under leafy trees.");
            return;
        }
        self.eat(self.selected.min(self.slots.len() - 1));
    }

    /// Picks a quick bar slot, if there's something in it
    pub fn select(&mut self, slot: usize) {
        if slot < self.slots.len() {
            self.selected = slot;
        }
    }

    /// Picks the slot `by` along from the one picked now, going round
    pub fn scroll(&mut self, by: i32) {
        if !self.slots.is_empty() {
            self.selected = (self.selected as i32 + by).rem_euclid(self.slots.len() as i32) as usize;
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

    /// Picks the first weapon of family `id` carried
    pub fn select_family(&mut self, id: &str) {
        let family = self.library.family(id);
        if let Some(slot) = self.slots.iter().position(|(item, _)| matches!(item, Item::Weapon(i) if family.is_some() && self.weapons[*i].family == family.unwrap())) {
            self.selected = slot;
        }
    }

    /// Picks the first weapon of `kind` carried
    pub fn select_weapon(&mut self, kind: Kind) {
        if let Some(slot) = self.slots.iter().position(|(item, _)| matches!(item, Item::Weapon(i) if self.weapons[*i].kind() == kind)) {
            self.selected = slot;
        }
    }

    fn eat(&mut self, slot: usize) {
        let Some(&(item, count)) = self.slots.get(slot) else {
            return;
        };
        if item.food() <= 0.0 {
            return;
        }
        if self.hunger >= 99.5 && self.health >= MAX_HEALTH {
            self.say("You're full.");
            return;
        }
        // Food heals a little too
        self.hunger = (self.hunger + item.food()).min(100.0);
        self.health = (self.health + item.food() * 0.6).min(MAX_HEALTH);
        self.cues.push(Cue::here(Sound::Eat, 0.7));
        if count > 1 {
            self.slots[slot].1 -= 1;
        } else {
            self.slots.remove(slot);
            self.selected = self.selected.min(self.slots.len().saturating_sub(1));
        }
        self.say(format!("You ate the {}.", item.name().to_lowercase()));
    }

    /// What to draw for the shader: the weapon in hand first, then the nearest animals, food,
    /// props and things flying about around the camera
    pub fn instances(&self, world: &World, view: &View) -> Vec<AnimalInstance> {
        let vpm = world.voxels_per_meter as f64;
        let camera = view.eye;
        let held = self.held().map(|(i, w)| (w, self.weapon_model(i)));
        let mut out = self.combat.held_instances(held, view);
        // Animals come before anything else, bosses most of all, so however much is flying
        // about they're always drawn
        let mut creatures: Vec<(f64, AnimalInstance)> = Vec::new();
        for animal in &self.animals {
            let d = (animal.position - camera).norm();
            if d < ANIMAL_RANGE * vpm || animal.boss.is_some() {
                creatures.push((if animal.boss.is_some() { 0.0 } else { d }, animal.instance(world.voxels_per_meter)));
            }
        }
        creatures.sort_by(|a, b| a.0.total_cmp(&b.0));
        out.extend(creatures.into_iter().take(MAX_INSTANCES / 2).map(|(_, i)| i));
        let mut near: Vec<(f64, AnimalInstance)> = self.combat.flying_instances(view);
        for pickup in &self.pickups {
            let d = (pickup.position - camera).norm();
            if d < FOOD_RANGE * vpm {
                near.push((d, animals::still_instance(pickup.item.look(), pickup.position, pickup.heading, world.voxels_per_meter)));
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

    fn target_hint(&self) -> Option<String> {
        match self.target? {
            Target::Pickup(i) => Some(format!("E  pick up {}", self.pickups[i].item.name().to_lowercase())),
            Target::Chest(_) => Some("E  open the chest".into()),
            Target::Animal(i) => {
                let look = self.animals[i].look;
                Some(if look.is_pet() {
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
            if animal.status.shown <= 0.0 || animal.health <= 0.0 || animal.boss.is_some() {
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

        // One panel across the top middle: the quick bar on the left, hunger and mana on the right
        let slot = 44.0;
        let gap = 6.0;
        let pad = 10.0;
        let slots_width = if self.slots.is_empty() { 120.0 } else { self.slots.len() as f32 * (slot + gap) - gap };
        let bars_width = 170.0;
        let width = pad + slots_width + 20.0 + bars_width + pad;
        let panel = Rect::from_min_size(pos2(center.x - width / 2.0, screen.top() + 14.0), vec2(width, slot + 2.0 * pad));
        painter.rect_filled(panel, 12.0, Color32::from_black_alpha(120));

        let left = panel.left() + pad;
        if self.slots.is_empty() {
            painter.text(pos2(left + slots_width / 2.0, panel.center().y), Align2::CENTER_CENTER, "Nothing yet", FontId::proportional(14.0), faint);
        }
        for (i, &(item, count)) in self.slots.iter().enumerate() {
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

        // The boss being fought, its name over a long bar along the bottom
        if let Some((name, vitality, enraged)) = self.foe(view.eye) {
            let width = (screen.width() * 0.42).clamp(260.0, 620.0);
            let back = Rect::from_center_size(pos2(center.x, screen.bottom() - 54.0), vec2(width, 12.0));
            let title = if enraged { format!("{name}  —  enraged") } else { name.to_string() };
            let at = pos2(center.x, back.top() - 6.0);
            painter.text(at + vec2(1.0, 1.0), Align2::CENTER_BOTTOM, &title, FontId::proportional(19.0), shadow);
            painter.text(at, Align2::CENTER_BOTTOM, &title, FontId::proportional(19.0), if enraged { Color32::from_rgb(255, 170, 120) } else { Color32::WHITE });
            painter.rect_filled(back.expand(2.0), 6.0, Color32::from_black_alpha(150));
            painter.rect_filled(Rect::from_min_size(back.min, vec2(width * vitality as f32, back.height())), 5.0, Color32::from_rgb(190, 40, 35));
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

    /// The inventory: what's carried, food with a button to eat each, weapons with everything
    /// about them
    pub fn inventory(&mut self, ctx: &egui::Context, open: &mut bool) {
        let mut eat = None;
        let mut select = None;
        egui::Window::new("Inventory")
            .open(open)
            .collapsible(false)
            .resizable(false)
            // Just under the quick bar
            .anchor(Align2::CENTER_TOP, [0.0, 100.0])
            .show(ctx, |ui| {
                ui.label(format!("Hunger: {:.0} / 100    Mana: {:.0} / {:.0}", self.hunger, self.combat.mana, MAX_MANA));
                ui.separator();
                egui::ScrollArea::vertical().max_height(ctx.screen_rect().height() * 0.65).show(ui, |ui| {
                    for (i, &(item, count)) in self.slots.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(vec2(36.0, 36.0), egui::Sense::hover());
                            self.draw_icon(ui.painter(), item, rect.center(), 26.0);
                            match item {
                                Item::Bucket => {
                                    ui.label(format!("Bucket: {:.1} of {:.0} m³ of water", self.bucket, BUCKET_CAPACITY));
                                }
                                Item::Weapon(w) => {
                                    let weapon = &self.weapons[w];
                                    let rarity = &self.library.rarities[weapon.grade];
                                    ui.label(RichText::new(&weapon.name).color(color(rarity.color)).strong());
                                    ui.label(RichText::new(format!("{} {}", rarity.display_name, self.library.families[weapon.family].display_name)).weak());
                                }
                                _ => {
                                    ui.label(format!("{} × {}", item.name(), count));
                                    ui.add_space(12.0);
                                    if ui.button(format!("Eat (+{:.0})", item.food())).clicked() {
                                        eat = Some(i);
                                    }
                                }
                            }
                            if ui.selectable_label(self.selected == i, "In hand").clicked() {
                                select = Some(i);
                            }
                        });
                        if let Item::Weapon(w) = item {
                            egui::CollapsingHeader::new("Parts, stats and effects").id_salt(("weapon", w)).show(ui, |ui| self.weapon_details(ui, &self.weapons[w]));
                        }
                    }
                });
                ui.separator();
                ui.label("With a sword, greatsword, katana, axe, scythe, hammer, dagger, spear or halberd: click to swing, three in a row for a combo, right click for a heavy blow.");
                ui.label("With a bow: hold right click to draw, then click to shoot. With a staff, scepter or wand: click to cast, using mana.");
                ui.label("E picks up, opens chests and pets. Number keys or the mouse wheel pick what's in hand.");
                ui.label("Weapons with a legendary or mythic part have a special attack: R or the middle button, once it's charged.");
                ui.label("Chests lie in pyramids, shrines, castles, flooded churches and desert ruins.");
            });
        if let Some(slot) = eat {
            self.eat(slot);
        }
        if let Some(slot) = select {
            self.selected = slot;
        }
    }

    fn weapon_details(&self, ui: &mut egui::Ui, weapon: &Weapon) {
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
        if let Some(element) = weapon.profile.element {
            ui.label(RichText::new(format!("Element: {}", element.name())).color(color(element.color())));
        }
        if let Some((special, part)) = &weapon.profile.special {
            let rarity = &library.rarities[weapon.grade];
            ui.label(RichText::new(format!("Special (R): {}", special.name())).color(color(rarity.color)).strong());
            ui.label(RichText::new(format!("{} From its {}.", special.description(), part)).weak());
        }
        ui.add_space(4.0);
        egui::Grid::new(("stats", weapon.seed, weapon.family)).num_columns(2).spacing([16.0, 2.0]).show(ui, |ui| {
            for (key, label, unit) in STATS {
                let value = weapon.profile.stat(key);
                if value.abs() < 1e-6 {
                    continue;
                }
                let label = if key == "elementalDamage" { format!("{} damage", weapon.profile.element.map_or("Elemental", Element::name)) } else { label.to_owned() };
                ui.label(RichText::new(label).weak());
                ui.label(unit.show(value));
                ui.end_row();
            }
        });
        if !weapon.profile.effects.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("Effects").strong());
            for e in &weapon.profile.effects {
                let def = &library.effects[e.def];
                let strength = if e.potency > 1.0 { format!(" ×{:.1}", e.potency) } else { String::new() };
                ui.label(format!("• {}{}: {}", def.display_name, strength, def.description));
            }
        }
        if !weapon.profile.synergies.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("Synergies").strong().color(Color32::from_rgb(255, 200, 90)));
            for s in &weapon.profile.synergies {
                let def = &library.synergies[s.def];
                ui.label(RichText::new(format!("★ {}: {}", def.display_name, def.description)).color(Color32::from_rgb(255, 225, 160)));
            }
        }
        let tags: Vec<&str> = weapon.tags.iter().map(String::as_str).collect();
        ui.label(RichText::new(format!("Tags: {}", tags.join(", "))).weak().small());
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
                match weapon.kind() {
                    Kind::Melee => {
                        let (hilt, tip) = (center + vec2(-r * 0.7, r * 0.7), center + vec2(r * 0.85, -r * 0.85));
                        let guard = center + vec2(-r * 0.35, r * 0.35);
                        painter.line_segment([guard, tip], Stroke::new(r * 0.24, metal));
                        if weapon.profile.element.is_some() {
                            painter.line_segment([guard, guard.lerp(tip, 0.7)], Stroke::new(r * 0.08, glow));
                        }
                        painter.line_segment([guard + vec2(-r * 0.3, -r * 0.3), guard + vec2(r * 0.3, r * 0.3)], Stroke::new(r * 0.16, Color32::from_rgb(200, 170, 80)));
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
