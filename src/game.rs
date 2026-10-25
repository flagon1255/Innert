//! Living off the land: food to find and eat, an inventory to carry it in, animals to hunt or pet,
//! weapons to find in chests and fight with, and the heads-up display that shows it all. Positions
//! are in voxels of the main world.

use std::collections::HashSet;
use std::f64::consts::TAU;
use std::time::Instant;

use egui_winit_vulkano::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, StrokeKind, pos2, vec2};
use nalgebra::Vector3;

use crate::animals::{self, ANIMAL_MODELS, Animal, AnimalInstance, Look, Model, pack_models};
use crate::audio::{Cue, Sound};
use crate::combat::{Arena, Combat, Controls, MAX_MANA, View};
use crate::leaves::Leaves;
use crate::player::WaterLevel;
use crate::weapons::{Element, Kind, Library, STATS, Weapon};
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
pub struct Chest {
    pub position: Vector3<f64>,
    pub heading: f64,
    /// Where it is in the whole landscape, which stays the same however the world moves, so it
    /// stays open, and what's in it is the same every time
    pub key: [i64; 3],
    pub luck: f64,
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
        for (k, family) in ["sword", "bow", "staff", "axe", "hammer", "dagger", "spear", "wand"].into_iter().enumerate() {
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
        format!("holding {held}, {animal}, mana {:.0}, {} animals, {} leaves falling", self.combat.mana, self.animals.len(), self.leaves.count())
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
        self.target = None;
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
        for pickup in &mut self.pickups {
            pickup.position -= shift;
        }
        self.pickups.retain(|p| inside(&p.position));
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
        self.combat.update(&mut Arena { world, animals: &mut self.animals, view }, weapon, controls, dt);
        for message in std::mem::take(&mut self.combat.messages) {
            self.say(message);
        }
        self.reap();
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

    /// Animals that were killed fall, and leave their meat
    fn reap(&mut self) {
        let mut k = 0;
        while k < self.animals.len() {
            if self.animals[k].health > 0.0 {
                k += 1;
                continue;
            }
            let animal = self.animals.remove(k);
            let look = animal.look;
            for m in 0..look.meat() {
                let angle = animal.heading() + m as f64 * 2.1;
                let spread = 0.25 * m as f64;
                self.pickups.push(Pickup {
                    item: Item::Meat,
                    position: animal.position + Vector3::new(angle.cos(), angle.sin(), 0.0) * spread,
                    heading: angle,
                });
            }
            self.say(if look.meat() > 0 { format!("You hunted a {}. It dropped some meat.", look.name()) } else { format!("You hunted a {}.", look.name()) });
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
        if self.hunger >= 99.5 {
            self.say("You're full.");
            return;
        }
        self.hunger = (self.hunger + item.food()).min(100.0);
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
        let mut near: Vec<(f64, AnimalInstance)> = self.combat.flying_instances(view);
        for animal in &self.animals {
            let d = (animal.position - camera).norm();
            if d < ANIMAL_RANGE * vpm {
                near.push((d, animal.instance(world.voxels_per_meter)));
            }
        }
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
        // The falling leaves go last, so the nearest animals still cast shadows, but they always
        // have room
        let mut leaves = self.combat.fine_instances(view);
        leaves.extend(self.leaves.instances(self.leaf_model(), vpm));
        let room = MAX_INSTANCES.saturating_sub(out.len() + leaves.len());
        out.extend(near.into_iter().take(room).map(|(_, i)| i));
        out.extend(leaves);
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
            if animal.status.shown <= 0.0 || animal.health <= 0.0 {
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
        let hunger = if self.hunger > 25.0 { Color32::from_rgb(232, 160, 60) } else { Color32::from_rgb(220, 70, 50) };
        bar(panel.top() + pad + 4.0, "Hunger", self.hunger / 100.0, hunger);
        bar(panel.top() + pad + 24.0, "Mana", self.combat.mana / MAX_MANA, Color32::from_rgb(80, 140, 255));

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
                ui.label("With a sword, axe, hammer, dagger or spear: click to swing, right click for a heavy blow.");
                ui.label("With a bow: hold right click to draw, then click to shoot. With a staff or wand: click to cast, using mana.");
                ui.label("E picks up, opens chests and pets. Number keys or the mouse wheel pick what's in hand.");
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
