//! Living off the land: food to find and eat, an inventory to carry it in, animals to hunt or pet,
//! weapons to find in chests and fight with, and the heads-up display that shows it all. Positions
//! are in voxels of the main world.

use std::collections::{HashMap, HashSet};
use std::f64::consts::{FRAC_PI_2, TAU};
use std::rc::Rc;
use std::time::Instant;

use egui_winit_vulkano::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Stroke, StrokeKind, pos2, vec2};
use nalgebra::{UnitQuaternion, Vector2, Vector3};

use crate::animals::{self, ANIMAL_MODELS, Animal, AnimalInstance, Build, Look, Model, pack_models};
use crate::audio::{Cue, Ground, Sound};
use crate::bosses::{self, Deed, Flock, Move};
use crate::combat::{Arena, Combat, Controls, HostileKind, Loaded, MAX_MANA, View};
use crate::leaves::Leaves;
use crate::player::WaterLevel;
use crate::realms::{self, Scaling};
use crate::wings;
use crate::save;
use crate::weapons::{Build as WeaponBuild, DROP_BLOOD, DROP_EMBER, DROP_ICE, DROP_MUD, DROP_VENOM, DROP_WATER, Element, Kind, Library, Profile, STATS, Shot, Special, Style, Unit, Weapon};
use crate::worldgen::{self, World, is_passable, material::*};
use crate::serpents;
use crate::tools;

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
/// How fast a crossbowman's bolts fly, in meters a second
const BOLT_PACE: f64 = 32.0;
/// Apples keep growing back near the player up to this many
const APPLES: usize = 48;
/// How much water the bucket holds, in cubic meters. A magic bucket, to make a splash.
pub const BUCKET_CAPACITY: f64 = 2.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Item {
    Bucket,
    Apple,
    Meat,
    /// Meat roasted over a fireplace: more filling than raw, see src\hearths.rs
    RoastMeat,
    Bread,
    /// Heals when drunk
    Potion,
    /// Gold lying on the ground, this much of it. What's carried is Game::gold.
    Gold(u32),
    /// One of Game::weapons
    Weapon(usize),
    /// Special ammo, of Library::ammo, and how many: carried as one stack of each kind, used up a
    /// shot at a time
    Ammo(usize, u32),
    /// What one of a realm's raid bosses leaves, of realm number this of realms::REALMS: three of
    /// a realm's set into the Innert Gate open the way to the next. Never sold, dropped or lost.
    InnertBlock(u8),
    /// A grappling hook, thrown with a click while it's in hand to pull the player to where it
    /// bites, see src\rope.rs, and wings, which let the player glide while they hold jump in the
    /// air. Kept for good once found.
    Hook,
    /// Wings of kind wings::KINDS[k], worn on the back to glide on, see Game::worn
    Wings(u8),
}

impl Item {
    /// What it looks like lying on the ground. Weapons and ammo are drawn with their own models.
    fn look(self) -> Option<Look> {
        match self {
            Item::Apple => Some(Look::Apple),
            Item::Meat | Item::RoastMeat => Some(Look::Meat),
            Item::Bread => Some(Look::Bread),
            Item::Potion => Some(Look::Potion),
            Item::Gold(_) => Some(Look::Coins),
            Item::InnertBlock(_) => Some(Look::InnertBlock),
            Item::Hook => Some(Look::GrappleHook),
            Item::Wings(_) => Some(Look::Wings),
            Item::Bucket | Item::Weapon(_) | Item::Ammo(..) => None,
        }
    }

    /// Something the player keeps whatever happens: it can't be sold, dropped or eaten
    fn kept(self) -> bool {
        matches!(self, Item::Bucket | Item::InnertBlock(_) | Item::Hook | Item::Wings(_))
    }

    fn name(self) -> &'static str {
        match self {
            Item::Bucket => "Bucket",
            Item::Apple => "Apple",
            Item::Meat => "Raw Meat",
            Item::RoastMeat => "Roast Meat",
            Item::Bread => "Bread",
            Item::Potion => "Healing Potion",
            Item::Gold(_) => "Gold",
            Item::Weapon(_) => "Weapon",
            Item::Ammo(..) => "Ammo",
            Item::InnertBlock(_) => "Innert Block",
            Item::Hook => "Grappling Hook",
            Item::Wings(k) => wings::KINDS[k as usize].name,
        }
    }

    /// How many there are of it to show on its slot: the rounds in a stack of ammo, or `count`
    fn shown(self, count: u32) -> Option<u32> {
        match self {
            Item::Bucket | Item::Weapon(_) | Item::Hook | Item::Wings(_) => None,
            Item::Ammo(_, n) => Some(n),
            _ => Some(count),
        }
    }

    /// How much hunger eating one takes away, out of 100
    fn food(self) -> f64 {
        match self {
            Item::Apple => 12.0,
            Item::Meat => 30.0,
            Item::RoastMeat => 55.0,
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
        Item::RoastMeat => 14,
        Item::Bread => 10,
        Item::Potion => 35,
        // Sold in the normal world once the way to the sky isles is open, see Game::sky_wares
        Item::Hook => 400,
        Item::Wings(k) => wings::KINDS[k as usize].price,
        _ => 0,
    }
}

/// How much of what a merchant asks it pays for what the player sells it
const SELL_SHARE: f64 = 0.35;
/// Slots on the quick bar; the rest of what's carried is only in the inventory
pub const HOTBAR: usize = 9;
/// Gold the player starts with
const STARTING_GOLD: u32 = 60;

/// Something a merchant has to sell: a weapon of Game::weapons, or food, potions or bundles of
/// ammo, so many of them
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
    /// Swapping two slots, one dragged onto the other
    Move(usize, usize),
    Sort,
    Buy(usize),
    Sell(usize),
    /// Putting on the wings in a slot, and taking off those worn
    Wear(usize),
    TakeOff,
}

/// What was done with a slot in a grid of them: clicked, double or right clicked, or dragged onto
/// another, see Game::slot_grid()
enum SlotClick {
    Once(usize),
    Twice(usize),
    Moved(usize, usize),
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
    /// A bandits' camp's hoard, which turns up once they're all dead: a beam of light shows where
    /// it is until it's opened, and there's more gold in it
    pub hoard: bool,
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
    /// For a bandits' camp, who keeps it and what they leave
    pub band: Option<Band>,
}

/// Who keeps a bandits' camp, and how far round its fire in meters, and the chest of their hoard
/// that turns up there once they're all dead, with where that is, put for the player
pub struct Band {
    pub keepers: Vec<Look>,
    pub within: f64,
    pub hoard: Chest,
    pub hint: &'static str,
}

/// What a den is, which decides what haunts it
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DenKind {
    /// A hall of a dungeon, haunted by the dead or spiders
    Hall,
    /// A cavern cave spiders nest in, or gloomwings roost in
    Nest,
    /// A bandits' camp
    Camp,
    /// A ravine gloomwings roost in, along its walls
    Roost,
    /// Desert ruins burrowers have dug in round
    Burrows,
    /// A castle fallen to the dead: its gate, courtyard, great hall and throne room
    Fallen,
    /// A city fallen under the land, overrun by the dead: its gate, streets, canal, square and the
    /// way into its temple, see src\worldgen\undercity.rs
    Sunken,
}

/// Where a merchant has their stall: where in voxels, and where in the whole landscape, for what
/// they've sold to stay sold, and how far from there in meters they may stand
pub struct Stall {
    pub position: Vector3<f64>,
    pub key: [i64; 3],
    pub within: f64,
    /// For the royal armoury in a castle's throne room, which way the princess keeping it faces:
    /// she sells only legendary and mythic weapons, see royal_stock()
    pub royal: Option<f64>,
}

/// A town's square: where its middle is in voxels, on the paving, how many folk live round it
/// and how far from it in meters they stroll
pub struct Town {
    pub position: Vector3<f64>,
    pub folk: usize,
    pub roam: f64,
}

/// How many weapons a royal armoury has for sale
const ROYAL_WEAPONS: usize = 6;

/// How likely each rarity is for each part of the weapons a royal armoury has, from common up, as
/// Library::odds() gives them: very lucky, the odds it's always had, whatever those of what's
/// found about the land, so it's the place to buy the best
const ROYAL_ODDS: [f64; 6] = [0.55, 0.25 * 9.0, 0.13 * 81.0, 0.05 * 729.0, 0.018 * 6561.0, 0.002 * 59049.0];

/// The weapons a royal armoury sells, rolled from `seed`: legendary and mythic ones only, rolled
/// very lucky, no two of a family. Only those it keeps are built.
fn royal_weapons(library: &Library, mut seed: u32) -> Vec<Weapon> {
    let legendary = library.rarities.iter().position(|r| r.id == "legendary").unwrap_or(library.rarities.len().saturating_sub(2));
    let mut weapons: Vec<Weapon> = Vec::new();
    for _ in 0..600 {
        if weapons.len() >= ROYAL_WEAPONS {
            break;
        }
        // As random() does, so a seed of 0 doesn't stay 0
        seed = hash(seed.wrapping_add(0x9e37_79b9));
        let (family, grade, _) = library.preview(seed, None, &ROYAL_ODDS);
        if grade >= legendary && weapons.iter().all(|w| w.family != family) {
            weapons.push(library.generate_with(seed, None, &ROYAL_ODDS));
        }
    }
    weapons
}

/// The families of the weapons the player starts with, one of each, see kit()
const KIT: [&str; 16] = ["sword", "bow", "staff", "axe", "hammer", "dagger", "spear", "wand", "greatsword", "katana", "scythe", "halberd", "scepter", "whip", "crossbow", "musket"];
/// How lucky what a new player starts with is, as for Library::odds(), and the rarest any part of
/// it, or any weapon of it, can be, of Library::rarities: plain gear, common to rare, nothing
/// with a special attack, see kit(). Weapons with special attacks are to be found.
const KIT_LUCK: f64 = 1.0;
const KIT_RAREST: usize = 2;
/// How many seeds kit() tries for each weapon no rarer than it's to be
const KIT_TRIES: usize = 64;

/// The weapons arm() gives the player, one of each of KIT: of which family each is, the seed it's
/// made from and the odds of each rarity for its parts, as for Library::generate_with(). Rolled
/// from `seed` with `luck`, and with `rarest`, no part or weapon rarer than that: a synergy or two
/// can lift a weapon a grade above its parts, so then it's the first seed on whose weapon isn't.
fn kit(library: &Library, seed: u32, luck: f64, rarest: Option<usize>) -> Vec<(usize, u32, Vec<f64>)> {
    let mut odds = library.odds(luck);
    if let Some(top) = rarest {
        odds.truncate(top + 1);
    }
    KIT.iter()
        .enumerate()
        .filter_map(|(k, id)| {
            let family = library.family(id)?;
            let first = hash(seed ^ (k as u32 + 1) * 0x51ed);
            let seed = match rarest {
                Some(top) => std::iter::successors(Some(first), |&s| Some(hash(s))).take(KIT_TRIES).find(|&s| library.preview(s, Some(family), &odds).1 <= top).unwrap_or(first),
                None => first,
            };
            Some((family, seed, odds.clone()))
        })
        .collect()
}

/// The share of the dead and of bandits who carry a weapon, to drop when they're killed, and how
/// lucky it is, as for Library::odds(), before a realm's scaling: what they fought with, mostly
/// common, one in about 200 with a special attack
const MONSTER_WEAPON: f64 = 0.06;
const MONSTER_LUCK: f64 = 0.4;

/// How lucky the weapons a merchant has are, as for Library::odds(), before a realm's scaling:
/// some of them rare, now and then epic, and only one stall in a dozen or so with something with
/// a special attack
const MERCHANT_LUCKS: [f64; 5] = [0.5, 0.7, 0.9, 1.1, 1.3];

/// How much of a realm's scaling of luck what's found there has, as Scaling::luck to this power,
/// see loot_scaling(): enough that the realms further on are richer, not so much that weapons
/// with special attacks are in every chest there
const REALM_LOOT: f64 = 0.5;

/// How many times luckier the weapons found are in a realm scaled `scaling`, see REALM_LOOT
fn loot_scaling(scaling: Scaling) -> f64 {
    scaling.luck.max(0.0).powf(REALM_LOOT)
}

/// What sort of boss a hoard is a beaten one's, which decides what's in it, see hoard_rolls()
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Hoard {
    /// One of the bosses about the land: in its lair, a dungeon's deepest hall, a great cavern
    Boss,
    /// One of a realm's raid bosses that isn't one of the huge ones
    Raid,
    /// One of the huge raid bosses, or a realm's finale
    Huge,
}

/// How lucky the weapons a boss leaves are, as for Library::odds(), its best first, before a
/// realm's scaling: a raid boss leaves far more and far better
const BOSS_LUCKS: [f64; 2] = [8.0, 2.5];
const RAID_LUCKS: [f64; 5] = [12.0, 5.0, 3.0, 2.5, 2.0];
/// How likely the best of them is to be one with a special attack, rolled as lucky till it is,
/// for each sort of Hoard: one of the bosses about the land's only as luck has it, about one in
/// five; a realm's raid boss's very likely; a huge raid boss's or a finale's always. The rest of
/// what they leave only now and then has one, so it's the raid bosses that leave them.
const SURE_SPECIAL: [f64; 3] = [0.0, 0.8, 1.0];

/// How each weapon in the hoard of a beaten boss is rolled, the best first: how lucky, as for
/// Library::odds(), before a realm's scaling, and whether it's sure to have a special attack,
/// which `rng` decides where it's not always so
fn hoard_rolls(hoard: Hoard, rng: &mut u32) -> Vec<(f64, bool)> {
    let lucks: &[f64] = if hoard == Hoard::Boss { &BOSS_LUCKS } else { &RAID_LUCKS };
    let chance = SURE_SPECIAL[hoard as usize];
    let sure = chance >= 1.0 || (chance > 0.0 && random(rng) < chance);
    lucks.iter().enumerate().map(|(k, &luck)| (luck, k == 0 && sure)).collect()
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
    /// For one of a realm's raid bosses, which realm's Innert block it leaves, see realms.rs: it's
    /// that boss whatever the land round it, and as hard to beat as the huge raid bosses
    pub raid: Option<u8>,
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
/// How lucky the roll is that makes the Coil Unending's fang, see Game::fang(): far luckier than
/// any raid boss's best
const FANG_LUCK: f64 = 60.0;
/// How fast the player moves while chilled by frost, and stuck in web, next to usual
const CHILLED_PACE: f64 = 0.55;
const SNARED_PACE: f64 = 0.4;
/// How far round in meters web leaves the ground sticky, for how many seconds, and how many
/// patches of it there can be at once
const WEB_RADIUS: f64 = 2.8;
const WEB_SECONDS: f64 = 6.0;
const WEBS: usize = 3;
/// The cloud of venom a bloater bursts into: how far round it reaches in meters, for how many
/// seconds, how hard each breath of it hurts, every half second, and how many hang about at once
const FUMES_RADIUS: f64 = 2.6;
const FUMES_SECONDS: f64 = 5.0;
const FUMES_DAMAGE: f64 = 2.0;
const FUMES: usize = 4;
/// Roots in each line of a treant's root wave, how far apart in meters, and how far round each is
const WAVE_ROOTS: usize = 9;
const WAVE_SPACING: f64 = 2.6;
const WAVE_RADIUS: f64 = 1.3;
/// Things coming down in a drake's storm drop from this many meters up
const STORM_HEIGHT: f64 = 18.0;

/// The floor at `x`, `y`, in voxels, for something to stand on about as high as `z`: the first
/// solid ground with room to stand over it, looking down from a few meters over that height, so
/// down in a cave or a fallen city rather than up on the land over it; the top of the ground there
/// if there's none
/// Whether there's nothing solid between a blow landing at `at` and the player standing at `feet`,
/// both in voxels, looking a quarter of a meter at a time, from a little over the ground to their
/// chest
fn open_between(world: &World, at: Vector3<f64>, feet: Vector3<f64>) -> bool {
    let vpm = world.voxels_per_meter as f64;
    let (from, to) = (at + Vector3::new(0.0, 0.0, 0.6 * vpm), feet + Vector3::new(0.0, 0.0, 1.0 * vpm));
    let steps = ((to - from).norm() / (0.25 * vpm)).ceil() as usize;
    (1..steps).all(|s| {
        let p = from + (to - from) * (s as f64 / steps as f64);
        is_passable(world.material_at(p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64))
    })
}

fn floor_near(world: &World, x: f64, y: f64, z: f64) -> f64 {
    let vpm = world.voxels_per_meter as f64;
    let (xi, yi) = (x as i64, y as i64);
    let room = |k: i64| (1..=(1.8 * vpm) as i64).all(|j| is_passable(world.material_at(xi, yi, k + j)));
    let from = (z + 4.0 * vpm) as i64;
    let found = (from - (12.0 * vpm) as i64..=from).rev().find(|&k| !is_passable(world.material_at(xi, yi, k)) && room(k));
    found.or_else(|| (0..world.size[2] as i64).rev().find(|&k| !is_passable(world.material_at(xi, yi, k)))).unwrap_or(0) as f64 + 1.0
}

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

/// The top of the ground at `x`, `y`, in voxels: the highest of what's solid there that isn't a
/// tree's trunk
pub fn ground_top(world: &World, x: f64, y: f64) -> f64 {
    let (xi, yi) = (x.floor() as i64, y.floor() as i64);
    let trunk = |m: u8| matches!(m, WOOD | BARK_DARK | BIRCH_BARK | PALM_WOOD | DEAD_WOOD | CACTUS);
    (0..world.size[2] as i64)
        .rev()
        .find(|&z| {
            let m = world.material_at(xi, yi, z);
            !is_passable(m) && !trunk(m)
        })
        .map_or(0.0, |z| z as f64 + 1.0)
}

/// How far the water `at` is in goes round it, in meters, if it's a basin of its own no more than
/// `most` meters out from there every way, as the Drowned Ring's, see worldgen::Arena: out each of
/// sixteen ways to where the ground comes up out of the water to stay, past any stones standing in
/// it, the middle one of those. None if it's open water some way.
fn basin_round(world: &World, water: WaterLevel, at: Vector3<f64>, most: f64) -> Option<f64> {
    let vpm = world.voxels_per_meter as f64;
    let mut out = Vec::new();
    for k in 0..16 {
        let a = k as f64 * TAU / 16.0;
        let (mut dry, mut shore) = (0, None);
        for step in 1..=most as usize {
            let (x, y) = (at.x + a.cos() * step as f64 * vpm, at.y + a.sin() * step as f64 * vpm);
            let surface = water(x, y);
            if !surface.is_finite() || ground_top(world, x, y) >= surface - 0.5 * vpm {
                dry += 1;
                // Wider than a stone standing in it
                if dry >= 7 {
                    shore = Some((step - dry) as f64);
                    break;
                }
            } else {
                dry = 0;
            }
        }
        out.push(shore?);
    }
    out.sort_by(f64::total_cmp);
    Some(out[out.len() / 2])
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

/// A blow a boss or monster landed: where, in voxels, how far round in meters, how hard, and what
/// landed it, for what's built there to take, see raids.rs
#[derive(Clone, Copy, Debug)]
pub struct Blow {
    pub at: Vector3<f64>,
    pub radius: f64,
    pub damage: f64,
    pub look: Look,
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
    /// Clouds of venom bloaters burst into: where, for how many more seconds, and how long till
    /// the next breath of it hurts
    fumes: Vec<(Vector3<f64>, f64, f64)>,
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
    /// The hoards of the bandits' camps in this piece of the world, by their dens, and where each
    /// is put for the player. Each turns up once its camp's cleared, and stays.
    hoards: HashMap<[i64; 3], (Chest, &'static str)>,
    /// Where the hoards turned up and not yet opened are, that the camera can see, for their beams
    /// of light to show
    beams: Vec<Vector3<f64>>,
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
    /// Which of those are royal armouries, kept by a princess
    royals: HashSet<[i64; 3]>,
    /// Where the player stood last, for what they drop to land at their feet
    feet: Option<Vector3<f64>>,
    /// The slot looked at in the inventory, with its card showing
    inspected: Option<usize>,
    messages: Vec<(String, Instant)>,
    /// Sounds to play
    cues: Vec<Cue>,
    /// Blows bosses and monsters landed since last asked, for what's built to take, see raids.rs
    blows: Vec<Blow>,
    target: Option<Target>,
    apple_timer: f64,
    /// What weapons can be made of, and the ones the player has
    library: Library,
    weapons: Vec<Weapon>,
    /// The special ammo picked for each kind of shot, by what Shot::ammo() calls it: plain ammo
    /// if there's none, which never runs out
    ammo_choice: HashMap<&'static str, usize>,
    /// What the weapon in hand shoots with that in it, for the weapon and ammo it was worked out for
    loaded: Option<((usize, usize), Loaded)>,
    combat: Combat,
    leaves: Leaves,
    /// The models of animals and things lying around, see animals::animal_models()
    models: Vec<Model>,
    /// Changes whenever there are new models to draw, like when a weapon is found
    models_version: u32,
    seed: u32,
    rng: u32,
    /// Something worth saving the game for straight away has happened, see take_milestone()
    milestone: bool,
    /// The Innert Gate or a realm's portal home, if it's in this piece of the world, see realms.rs,
    /// and whether the player's just pressed E at it, see take_shrine()
    shrines: Vec<Shrine>,
    shrine_used: bool,
    /// How much harder the realm the player's in is, see realms::Scaling
    scaling: Scaling,
    /// The lairs of the realm's raid bosses whose Innert blocks open the way on, by their keys,
    /// with which realm's each leaves
    raid_lairs: HashMap<[i64; 3], u8>,
    /// Whether the merchants sell a grappling hook and wings, as they do in the journey through
    /// the realms once the way to the sky isles is open
    sky_wares: bool,
    /// A window's open over the middle of the view, see set_covered()
    covered: bool,
    /// Omens of the Coil Unending lying on the way to its lair in Free Roam, see serpents::omens(),
    /// and how dark the sky's got over it, 0 to 1
    omens: Vec<(Look, Vector3<f64>, f64)>,
    gloom: f64,
    /// The wings worn on the back, of wings::KINDS, and the models of every kind of them
    worn: Option<u8>,
    wing_models: Vec<(Model, Vector3<f64>)>,
}

/// The Innert Gate, or a realm's portal home: where the middle of its foot is in voxels, which way
/// its front faces, for the gate how many of each realm's Innert blocks are set in it, and whether
/// light fills its opening, as it does once it leads somewhere
#[derive(Clone, PartialEq, Debug)]
pub struct Shrine {
    pub gate: bool,
    pub position: Vector3<f64>,
    pub facing: f64,
    pub placed: [u32; realms::REALM_COUNT],
    pub open: bool,
}

/// How near the gate or a portal the player has to be to use it, in meters from its foot
const SHRINE_REACH: f64 = 6.0;

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
            fumes: Vec::new(),
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
            hoards: HashMap::new(),
            beams: Vec::new(),
            bucket: 0.0,
            gold: STARTING_GOLD,
            shop: None,
            stocks: HashMap::new(),
            merchants: Vec::new(),
            royals: HashSet::new(),
            feet: None,
            inspected: None,
            messages: Vec::new(),
            cues: Vec::new(),
            blows: Vec::new(),
            target: None,
            apple_timer: 0.0,
            library: Library::load(),
            weapons: Vec::new(),
            ammo_choice: HashMap::new(),
            loaded: None,
            combat: Combat::new(seed, ANIMAL_MODELS as u32),
            leaves: Leaves::new(seed),
            models: animals::animal_models(),
            models_version: 0,
            seed,
            rng: hash(seed ^ 0x9a3e),
            milestone: false,
            shrines: Vec::new(),
            shrine_used: false,
            scaling: Scaling::NONE,
            raid_lairs: HashMap::new(),
            sky_wares: false,
            covered: false,
            worn: None,
            wing_models: wings::models(),
            omens: Vec::new(),
            gloom: 0.0,
        };
        game.arm_anew(seed);
        game.populate(world, water, seed, near);
        game
    }

    /// What a new player starts with, see arm(): plain gear, nothing with a special attack
    fn arm_anew(&mut self, seed: u32) {
        let kit = kit(&self.library, seed, KIT_LUCK, Some(KIT_RAREST));
        self.arm_with(&kit);
    }

    /// For trying them out, as `--set weapon_seed=7 weapon_luck=3` does: what the player starts
    /// with rolled with `luck`, as for Library::odds(), however rare, see arm_with()
    pub fn arm(&mut self, seed: u32, luck: f64) {
        let kit = kit(&self.library, seed, luck, None);
        self.arm_with(&kit);
    }

    /// The player armed with `kit`, as kit() rolls it: a sword, a bow and a staff, a crossbow and
    /// a musket and one of every other kind of weapon, a few bundles of special ammo, and the
    /// bucket. Replaces whatever weapons they had.
    fn arm_with(&mut self, kit: &[(usize, u32, Vec<f64>)]) {
        self.weapons.clear();
        self.combat.renumber(|_| None);
        self.loaded = None;
        self.slots.retain(|(item, _)| !matches!(item, Item::Weapon(_) | Item::Bucket | Item::Ammo(..)));
        let mut fresh = Vec::new();
        for (family, seed, odds) in kit {
            self.weapons.push(self.library.generate_with(*seed, Some(*family), odds));
            fresh.push((Item::Weapon(self.weapons.len() - 1), 1));
        }
        fresh.push((Item::Bucket, 1));
        for id in ["broadhead_arrows", "incendiary_bolts", "scatter_shot"] {
            if let Some(k) = self.library.ammo.iter().position(|a| a.id == id) {
                fresh.push((Item::Ammo(k, self.library.ammo[k].bundle), 1));
            }
        }
        fresh.append(&mut self.slots);
        self.slots = fresh;
        self.selected = 0;
        self.models_version += 1;
    }

    /// Fills a new world with animals and apples. What the player carries stays.
    pub fn populate(&mut self, world: &World, water: WaterLevel, seed: u32, near: Vector3<f64>) {
        self.animals = animals::spawn(world, water, seed, near);
        let mut rng = hash(seed ^ 0x5e4);
        self.animals.extend(animals::spawn_serpents(world, water, &mut rng, near, worldgen::land_of(seed), animals::SERPENT_PACKS));
        self.lairs.clear();
        self.defeated.clear();
        self.dens.clear();
        self.cleared.clear();
        self.boss_chests.clear();
        self.hoards.clear();
        self.beams.clear();
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
        let models: Vec<&Model> = self.models.iter().chain(self.combat.models.iter().map(|(m, _)| m)).chain(self.leaves.models.iter()).chain(self.wing_models.iter().map(|(m, _)| m)).collect();
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
        self.leaf_model() + (self.leaves.models.len() + self.wing_models.len() + before) as u32
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

    /// For trying things out: `count` of `item` put first on the quick bar, in hand
    pub fn hand(&mut self, item: Item, count: u32) {
        self.slots.insert(0, (item, count));
        self.selected = 0;
    }

    /// Whether what's in hand is raw meat, to roast over a fire, see src\hearths.rs
    pub fn holding_raw_meat(&self) -> bool {
        self.slots.get(self.selected).is_some_and(|(item, _)| *item == Item::Meat)
    }

    /// Roasts a piece of the raw meat in hand into roast meat: how many raw are left
    pub fn cook_selected(&mut self) -> u32 {
        let Some(&(Item::Meat, count)) = self.slots.get(self.selected) else {
            return 0;
        };
        if count > 1 {
            self.slots[self.selected].1 -= 1;
        } else {
            self.remove_slot(self.selected);
        }
        self.add_item(Item::RoastMeat, 1);
        // The roast meat in hand next, once the raw's all done
        if count == 1
            && let Some(at) = self.slots.iter().position(|(item, _)| *item == Item::RoastMeat)
            && at < HOTBAR
        {
            self.selected = at;
        }
        self.milestone = true;
        count - 1
    }

    /// A fire's smoke or ember, see Combat::wisp()
    pub fn wisp(&mut self, at: Vector3<f64>, velocity: Vector3<f64>, life: f64, ember: bool) {
        self.combat.wisp(at, velocity, life, ember);
    }

    /// Warmed by a fire for `dt` seconds: `rate` health a second back, once `safe` seconds have
    /// gone by since the player was last hurt
    pub fn warm(&mut self, dt: f64, rate: f64, safe: f64) {
        if self.dead.is_none() && self.since_hurt > safe {
            self.health = (self.health + rate * dt).min(MAX_HEALTH);
        }
    }

    /// The tool in hand, if it's a tool: which, and how good, see tools.rs
    pub fn held_tool(&self) -> Option<(tools::Tool, usize)> {
        self.held().and_then(|(_, w)| tools::of(&self.library, w))
    }

    /// How good the best `tool` carried is, if one is
    pub fn best_tool(&self, tool: tools::Tool) -> Option<usize> {
        self.slots
            .iter()
            .filter_map(|&(item, _)| match item {
                Item::Weapon(w) => tools::of(&self.library, &self.weapons[w]),
                _ => None,
            })
            .filter(|&(t, _)| t == tool)
            .map(|(_, grade)| grade)
            .max()
    }

    /// How many melee blows have landed, for a tool's to gather where they land, see tools.rs
    pub fn swings_landed(&self) -> u32 {
        self.combat.landed
    }

    /// Puts a tool of `grade` in the pack, see tools.rs, if there's one to make
    pub fn give_tool(&mut self, tool: tools::Tool, grade: usize) -> bool {
        match tools::make(&self.library, tool, grade) {
            Some(weapon) => {
                self.give_weapon(weapon);
                true
            }
            None => false,
        }
    }

    /// Puts `weapon`, made rather than found, in the pack
    pub fn give_weapon(&mut self, weapon: Weapon) {
        self.weapons.push(weapon);
        self.models_version += 1;
        self.slots.push((Item::Weapon(self.weapons.len() - 1), 1));
        self.milestone = true;
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
            Item::Ammo(k, n) => format!("{n} {}", self.library.ammo[k].display_name),
            _ => item.name().to_owned(),
        }
    }

    /// The special ammo the weapon in hand would shoot, if it shoots and some's picked and carried
    fn chosen_ammo(&self) -> Option<usize> {
        let shot = self.held()?.1.shot()?;
        let k = *self.ammo_choice.get(shot.ammo())?;
        self.slots.iter().any(|&(item, _)| matches!(item, Item::Ammo(a, n) if a == k && n > 0)).then_some(k)
    }

    /// T: loads the next kind of special ammo carried that the weapon in hand shoots, going round
    /// to plain ammo, which never runs out
    pub fn switch_ammo(&mut self) {
        let Some((_, weapon)) = self.held() else {
            return;
        };
        let Some(shot) = weapon.shot() else {
            self.say("Only bows, crossbows and guns take ammo.");
            return;
        };
        let mut kinds: Vec<usize> = Vec::new();
        for &(item, _) in &self.slots {
            if let Item::Ammo(k, n) = item
                && n > 0
                && self.library.ammo[k].shot().ammo() == shot.ammo()
                && !kinds.contains(&k)
            {
                kinds.push(k);
            }
        }
        if kinds.is_empty() {
            self.say(format!("You have no special {}. Merchants sell them, and bandits carry some.", shot.plural()));
            return;
        }
        let next = match self.chosen_ammo().and_then(|k| kinds.iter().position(|&x| x == k)) {
            Some(i) if i + 1 < kinds.len() => Some(kinds[i + 1]),
            Some(_) => None,
            None => Some(kinds[0]),
        };
        match next {
            Some(k) => {
                self.ammo_choice.insert(shot.ammo(), k);
                let count = self.ammo_count(k);
                self.say(format!("Loaded {} ({count} left).", self.library.ammo[k].display_name));
            }
            None => {
                self.ammo_choice.remove(shot.ammo());
                self.say(format!("Plain {}: they never run out.", shot.plural()));
            }
        }
        self.cues.push(Cue::here(Sound::Latch, 0.4).pitched(1.4));
    }

    /// Loads special ammo `k` into whatever shoots it, as the ammo it uses from now on
    fn load_ammo(&mut self, k: usize) {
        let def = &self.library.ammo[k];
        self.ammo_choice.insert(def.shot().ammo(), k);
        self.say(format!("Loaded {}. T switches ammo with a {} in hand.", def.display_name, match def.shot() {
            Shot::Arrow => "bow",
            Shot::Quarrel => "crossbow",
            _ => "gun",
        }));
        self.cues.push(Cue::here(Sound::Latch, 0.4).pitched(1.4));
    }

    /// How many of special ammo `k` are carried
    fn ammo_count(&self, k: usize) -> u32 {
        self.slots.iter().map(|&(item, _)| if let Item::Ammo(a, n) = item && a == k { n } else { 0 }).sum()
    }

    /// For trying it out: gives the player a bundle of the special ammo called `id` and loads it
    pub fn give_ammo(&mut self, id: &str) {
        if let Some(k) = self.library.ammo.iter().position(|a| a.id == id) {
            self.add_item(Item::Ammo(k, self.library.ammo[k].bundle * 3), 1);
            self.load_ammo(k);
        }
    }

    /// Takes a shot of special ammo `k` out of what's carried, saying so when it runs out
    fn spend_ammo(&mut self, k: usize) {
        let Some(slot) = self.slots.iter().position(|&(item, _)| matches!(item, Item::Ammo(a, n) if a == k && n > 0)) else {
            return;
        };
        if let Item::Ammo(_, n) = &mut self.slots[slot].0 {
            *n -= 1;
            if *n == 0 {
                self.remove_slot(slot);
                let def = &self.library.ammo[k];
                self.say(format!("Out of {}: back to plain {}.", def.display_name, def.shot().plural()));
            }
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
        let size = match family.size_word(weapon.size) {
            "" => String::new(),
            word => format!(" ({word})"),
        };
        let p = &weapon.profile;
        let speed = ["attackSpeed", "drawSpeed", "castSpeed", "reloadSpeed"].iter().map(|k| p.stat(k)).find(|&v| v > 0.0).unwrap_or(0.0);
        let damage: f64 = ["physicalDamage", "elementalDamage", "spellDamage"].iter().map(|k| p.stat(k)).sum();
        format!(
            "{} - {} {}{size}, element {}, {damage:.1} damage at {speed:.2}/s, {} make (x{:.3}, parts x{:.2} damage x{:.2} speed): {}. Effects: {}. Synergies: {}.",
            weapon.name,
            library.rarities[weapon.grade].display_name,
            family.display_name,
            p.element.map_or("none", Element::name),
            p.make(),
            p.forged,
            p.quality,
            p.pace,
            parts.join(", "),
            if effects.is_empty() { "none".into() } else { effects.join(", ") },
            if synergies.is_empty() { "none".into() } else { synergies.join(", ") },
        )
    }

    /// How the fight's going, for checking from scripts: the nearest animal to `near` that can be
    /// hunted, and the mana left
    pub fn report(&self, near: Vector3<f64>) -> String {
        let nearest = self.animals.iter().filter(|a| !a.look.is_pet()).min_by(|a, b| (a.position - near).norm().total_cmp(&(b.position - near).norm()));
        let animal = nearest.map_or("no animals".to_owned(), |a| format!("nearest {} at {:.0}/{:.0} health, tint {}", a.look.name(), a.health, a.max_health, a.status.tint()));
        let held = self.held().map_or("nothing".to_owned(), |(_, w)| w.name.clone());
        let bosses = self.animals.iter().filter(|a| a.look.is_boss()).count();
        let boss = self.animals.iter().filter(|a| a.look.is_boss()).min_by(|a, b| (a.position - near).norm().total_cmp(&(b.position - near).norm()));
        let animal = match boss {
            Some(b) => format!(
                "{animal}, boss {} {:.0} m off at {:.0}/{:.0}{}",
                b.look.name(),
                (b.position - near).norm() / self.combat.vpm(),
                b.health,
                b.max_health,
                if b.boss.as_ref().is_some_and(|s| s.engaged) { " fighting" } else { "" }
            ),
            None => animal,
        };
        // How far the furthest of a pack fighting the player has got from the middle of it
        let spread = self
            .animals
            .iter()
            .filter(|a| a.health > 0.0)
            .filter_map(|a| a.boss.as_ref().filter(|b| b.engaged && b.flock.size > 1).map(|b| (a.position - b.flock.middle).xy().norm() / self.combat.vpm()))
            .fold(0.0f64, f64::max);
        format!(
            "holding {held}, {animal}, mana {:.0}, health {:.0}, gold {}, [{}] after them, packs within {spread:.1} m, {} rest spot, {} lying about, {} animals ({bosses} bosses, {} lairs), {} leaves falling",
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

    /// The chests in the piece of the world just built, along with those bosses left and the
    /// hoards of the camps cleared there, see set_dens()
    pub fn set_chests(&mut self, chests: Vec<Chest>) {
        self.chests = chests;
        self.chests.extend(self.boss_chests.iter().cloned());
        self.chests.extend(self.hoards.iter().filter(|(den, _)| self.cleared.contains(*den)).map(|(_, (chest, _))| chest.clone()));
        self.beams.clear();
        self.target = None;
    }

    /// One more lair, as when a realm's finale rises, see realms::on_raids_beaten()
    pub fn add_lair(&mut self, world: &World, water: WaterLevel, lair: Lair) {
        let mut lairs = std::mem::take(&mut self.lairs);
        lairs.retain(|l| l.key != lair.key);
        lairs.push(lair);
        self.set_lairs(world, water, lairs);
    }

    /// The bosses' lairs in the piece of the world just built, in its voxels: a boss turns up in each
    /// that isn't beaten and doesn't have one already
    pub fn set_lairs(&mut self, world: &World, water: WaterLevel, lairs: Vec<Lair>) {
        for lair in &lairs {
            if let Some(realm) = lair.raid {
                self.raid_lairs.insert(lair.key, realm);
            }
            // Not the monsters of a den at the same spot, like a warlord's bandits
            let here = self.animals.iter().any(|a| a.look.is_boss() && a.boss.as_ref().is_some_and(|b| b.key == lair.key));
            if here || self.defeated.contains(&lair.key) {
                continue;
            }
            let seed = hash(lair.key[0] as u32 ^ hash(lair.key[1] as u32));
            let look = if lair.raid.is_some() || lair.look.is_great_serpent() { lair.look } else { lair.look.for_ground(ground_round(world, water, lair.position)) };
            // A great serpent that swims lies in the deep water by its lair, out from the shore, the
            // rest on the ground
            let deep = look.swims().then(|| animals::deep_water_near(world, water, lair.position, if look.is_ultra() { 55.0 } else { 40.0 }, if look.is_ultra() { 2.5 } else { 1.5 })).flatten();
            let swimming = deep.map(|at| Animal::boss(look, Vector3::new(at.x, at.y, water(at.x, at.y) - serpents::draft(look) * world.voxels_per_meter as f64), lair.key, seed));
            if let Some(mut boss) = swimming.or_else(|| animals::spawn_boss(world, water, look, lair.position, lair.key, seed)) {
                // The Coil Unending's as hard as it is already, whether it's the Water realm's finale
                // or in Free Roam, see Look::health()
                if lair.raid.is_some() && !look.is_raid() && !look.is_ultra() {
                    boss.toughen(realms::RAID_PROMOTION);
                }
                // A realm's finale is the hardest of all of it
                if realms::is_finale_key(&lair.key) && !look.is_ultra() {
                    boss.toughen(realms::FINALE_PROMOTION);
                }
                // Risen in a basin of its own, as the Drowned Ring's, the Coil Unending keeps to it
                if look.is_ultra()
                    && let Some(state) = &mut boss.boss
                {
                    state.basin = basin_round(world, water, boss.position, 40.0);
                }
                self.animals.push(boss);
            }
        }
        self.lairs = lairs;
    }

    /// The dungeon halls in the piece of the world just built, in its voxels: monsters haunt each
    /// that isn't cleared and doesn't have them already. Each hall has its own lot: the dead in
    /// their bones, rotting, or spiders, some in armor, or gloomwings with the dead. Bandits keep
    /// camps, as many as each's band says, some of them crossbowmen and shieldbearers, and the
    /// hoards of those cleared are there to open, see set_chests(). They keep close together, one
    /// pack to a den.
    pub fn set_dens(&mut self, world: &World, water: WaterLevel, dens: Vec<Den>) {
        self.hoards = dens.iter().filter_map(|d| d.band.as_ref().map(|b| (d.key, (b.hoard.clone(), b.hint)))).collect();
        for den in &dens {
            let here = self.animals.iter().any(|a| a.look.is_monster() && a.boss.as_ref().is_some_and(|b| b.key == den.key));
            if here || self.cleared.contains(&den.key) {
                continue;
            }
            let seed = hash(den.key[0] as u32 ^ hash(den.key[1] as u32 ^ 0xde4));
            // A camp's band as many as it says, some of its bandits crossbowmen or shieldbearers
            let band: Vec<Look>;
            let haunted: &[Look] = match den.kind {
                DenKind::Nest if seed % 2 == 0 => &[Look::Spiderling, Look::Spiderling, Look::Spiderling],
                DenKind::Nest | DenKind::Roost => &[Look::Gloomwing; 4],
                DenKind::Camp => {
                    let keepers = den.band.as_ref().map_or(vec![Look::Bandit, Look::Bandit, Look::Bandit, Look::BanditBrute], |b| b.keepers.clone());
                    let mut k = seed;
                    band = keepers
                        .into_iter()
                        .map(|look| {
                            k = hash(k);
                            match (look, k % 5) {
                                (Look::Bandit, 0 | 1) => Look::Crossbowman,
                                (Look::Bandit, 2) => Look::Shieldbearer,
                                (look, _) => look,
                            }
                        })
                        .collect();
                    &band
                }
                DenKind::Burrows => &[Look::DuneBurrower; 3],
                // More of them than in a dungeon's hall, the dead in armour among them, and archers
                // and ghouls in some
                DenKind::Fallen => match seed % 5 {
                    0 => &[Look::Skeleton, Look::Skeleton, Look::ArmoredSkeleton, Look::Zombie, Look::Zombie],
                    1 => &[Look::Zombie, Look::Zombie, Look::ArmoredZombie, Look::Skeleton, Look::ArmoredSkeleton],
                    2 => &[Look::ArmoredSkeleton, Look::BoneArcher, Look::ArmoredZombie, Look::Zombie, Look::Skeleton],
                    3 => &[Look::Zombie, Look::Ghoul, Look::ArmoredZombie, Look::Zombie, Look::Bloater],
                    _ => &[Look::ArmoredSkeleton, Look::Skeleton, Look::ArmoredZombie, Look::Zombie, Look::Skeleton, Look::Zombie],
                },
                // The dead of a fallen city, each street its own lot of them: shambling zombies with
                // a bloater among them, ghouls, archers behind the armoured, a brute, a necromancer
                // with its risen. Five or so to a pack, the worst kept for the square and the temple.
                DenKind::Sunken => match seed % 7 {
                    0 => &[Look::Zombie, Look::Zombie, Look::Bloater, Look::ArmoredZombie],
                    1 => &[Look::Ghoul, Look::Ghoul, Look::Zombie, Look::Zombie],
                    2 => &[Look::ArmoredSkeleton, Look::BoneArcher, Look::BoneArcher, Look::Skeleton],
                    3 => &[Look::GraveBrute, Look::Zombie, Look::Zombie, Look::Bloater],
                    4 => &[Look::Necromancer, Look::Zombie, Look::ArmoredZombie, Look::Zombie],
                    5 => &[Look::ArmoredZombie, Look::Ghoul, Look::BoneArcher, Look::Zombie, Look::Bloater],
                    _ => &[Look::GraveBrute, Look::BoneArcher, Look::ArmoredSkeleton, Look::Ghoul],
                },
                DenKind::Hall => match seed % 4 {
                    0 => &[Look::Skeleton, Look::Skeleton, Look::ArmoredSkeleton],
                    1 => &[Look::Zombie, Look::Zombie, Look::ArmoredZombie],
                    2 => &[Look::Spiderling, Look::Spiderling, Look::Spiderling, Look::Spiderling],
                    _ => &[Look::Gloomwing, Look::Gloomwing, Look::Gloomwing, Look::Skeleton],
                },
            };
            let spread = match &den.band {
                Some(b) => b.within,
                None if den.kind == DenKind::Camp => animals::PACK_SPREAD + 0.7,
                None => animals::PACK_SPREAD + 0.4,
            };
            for mut monster in animals::spawn_pack(world, water, haunted, den.position, spread, hash(seed)) {
                if let Some(state) = &mut monster.boss {
                    state.key = den.key;
                }
                self.animals.push(monster);
            }
        }
        self.dens = dens.iter().map(|d| d.key).collect();
    }

    /// For trying them out: a pack of monsters, `looks`, turns up `distance` meters along `ahead`
    /// from `near`, facing back that way turned `turn` radians further round. With `calm` they
    /// leave the player be, and with `pose` they're held in that frame, see animals::POSE_WINDUP,
    /// to look at.
    #[allow(clippy::too_many_arguments)]
    pub fn summon_pack(&mut self, world: &World, water: WaterLevel, looks: &[Look], near: Vector3<f64>, ahead: Vector3<f64>, distance: f64, turn: f64, calm: bool, pose: Option<usize>) {
        let vpm = world.voxels_per_meter as f64;
        let flat = Vector3::new(ahead.x, ahead.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x());
        let at = near + flat * distance * vpm;
        let at = Vector3::new(at.x, at.y, floor_near(world, at.x, at.y, near.z));
        for mut monster in animals::spawn_pack(world, water, looks, at, animals::PACK_SPREAD, self.rng) {
            // Facing back the way they were summoned from, to be seen from the front
            let back = (-flat.y).atan2(-flat.x);
            monster.face(back + turn);
            if let Some(state) = &mut monster.boss {
                if calm {
                    state.leave_be();
                }
                state.fixed_pose = pose;
            }
            self.animals.push(monster);
        }
        self.rng = hash(self.rng);
    }

    /// Where each pack is and what it's up to, for each of its members to keep with the rest, see
    /// bosses::Flock
    fn gather_packs(&mut self) {
        // Few packs are about at once, so a list does: each pack's number, where its members are
        // all told and how many, the same for those of them that close in to fight, and whether
        // any of them sees the player
        struct Gathered {
            pack: u32,
            sum: Vector3<f64>,
            size: u8,
            front: Vector3<f64>,
            fighters: u8,
            alarmed: bool,
            slots: u8,
        }
        let mut packs: Vec<Gathered> = Vec::new();
        // And where each member is, to keep out of each other's way
        let mut members: Vec<(u32, u32, Vector3<f64>)> = Vec::new();
        for a in self.animals.iter().filter(|a| a.health > 0.0) {
            let Some(boss) = a.boss.as_ref().filter(|b| b.pack != 0) else {
                continue;
            };
            members.push((boss.pack, a.id, a.position));
            let fighter = !bosses::hangs_back(a.look);
            let index = match packs.iter().position(|p| p.pack == boss.pack) {
                Some(index) => index,
                None => {
                    packs.push(Gathered { pack: boss.pack, sum: Vector3::zeros(), size: 0, front: Vector3::zeros(), fighters: 0, alarmed: false, slots: 0 });
                    packs.len() - 1
                }
            };
            let p = &mut packs[index];
            p.sum += a.position;
            p.size = p.size.saturating_add(1);
            if fighter {
                p.front += a.position;
                p.fighters = p.fighters.saturating_add(1);
            }
            p.alarmed |= boss.sighted();
        }
        let room = bosses::ELBOW_ROOM * self.combat.vpm();
        for a in self.animals.iter_mut().filter(|a| a.health > 0.0) {
            let Some(boss) = a.boss.as_mut().filter(|b| b.pack != 0) else {
                continue;
            };
            let Some(p) = packs.iter_mut().find(|p| p.pack == boss.pack) else {
                continue;
            };
            let middle = p.sum / p.size as f64;
            let front = if p.fighters > 0 { p.front / p.fighters as f64 } else { middle };
            // Crowded off by each of the rest too near it, the nearer the harder, those right on
            // top of it to one side or the other by which is which
            let mut crowd = Vector2::zeros();
            for &(_, id, at) in members.iter().filter(|m| m.0 == boss.pack && m.1 != a.id) {
                let off = (a.position - at).xy();
                let d = off.norm();
                if d < room {
                    let away = if d > 1e-3 { off / d } else if a.id < id { Vector2::x() } else { -Vector2::x() };
                    crowd += away * (1.0 - d / room);
                }
            }
            boss.flock = Flock { middle, size: p.size, front, fighters: p.fighters, slot: p.slots, alarmed: p.alarmed, crowd };
            // Places side by side are for those closing in
            if !bosses::hangs_back(a.look) {
                p.slots += 1;
            }
        }
    }

    /// For trying them out: boss `look` turns up `distance` meters along `ahead` from `near`
    pub fn summon(&mut self, world: &World, water: WaterLevel, look: Look, near: Vector3<f64>, ahead: Vector3<f64>, distance: f64) {
        let vpm = world.voxels_per_meter as f64;
        let flat = Vector3::new(ahead.x, ahead.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x());
        let at = near + flat * distance * vpm;
        let ground = floor_near(world, at.x, at.y, near.z);
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
        for (at, ..) in &mut self.fumes {
            *at -= shift;
        }
        for pickup in &mut self.pickups {
            pickup.position -= shift;
        }
        self.pickups.retain(|p| inside(&p.position));
        for (_, at, _) in &mut self.omens {
            *at -= shift;
        }
        self.omens.retain(|(_, at, _)| inside(at));
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
        let missing = ANIMALS.saturating_sub(self.animals.iter().filter(|a| a.look.build().is_none() && a.look != Look::Merchant && !a.look.is_villager() && !a.look.is_royal()).count());
        let wolves = self.animals.iter().filter(|a| a.look == Look::Wolf).count();
        let bandits = self.animals.iter().filter(|a| a.look.is_bandit()).count();
        // The packs of the country the player's come into, as the old ones are left behind
        let wilds = self.animals.iter().filter(|a| a.roams()).count();
        if missing > 0 || wolves < 3 || bandits < 3 || wilds < 4 {
            let vpm = world.voxels_per_meter as f64;
            let fresh = animals::spawn(world, water, seed ^ hash(self.rng), near).into_iter().filter(|a| (a.position - near).norm() > 40.0 * vpm);
            let mut taken = 0;
            for animal in fresh {
                let wanted = match animal.look {
                    Look::Wolf => wolves < 3,
                    look if look.is_bandit() => bandits < 3,
                    _ if animal.roams() => wilds < 4,
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
        // And serpents of the land it is, a nest or two of them, as those about are left behind
        let serpents = self.animals.iter().filter(|a| a.look.is_serpent()).count();
        if serpents < 3 {
            let vpm = world.voxels_per_meter as f64;
            let mut rng = hash(self.rng ^ 0x5e4);
            let nests = animals::spawn_serpents(world, water, &mut rng, near, worldgen::land_of(seed), animals::SERPENT_PACKS);
            self.animals.extend(nests.into_iter().filter(|a| (a.position - near).norm() > 40.0 * vpm));
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
    /// The blows bosses and monsters landed since last asked, see Blow
    pub fn take_blows(&mut self) -> Vec<Blow> {
        std::mem::take(&mut self.blows)
    }

    /// Brings a monster into the world, as night raids do, see raids.rs
    pub fn add_animal(&mut self, animal: Animal) {
        self.animals.push(animal);
    }

    /// Every animal, boss and monster, for night raids to lead round walls, see raids.rs
    pub fn animals_mut(&mut self) -> impl Iterator<Item = &mut Animal> {
        self.animals.iter_mut()
    }

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
        self.gather_packs();
        // Bosses and monsters as tough as the realm makes them, however they turned up: in their
        // lairs and dens, wandering in, or called up by a boss
        let toughness = self.scaling.toughness;
        for animal in &mut self.animals {
            if !animal.toughened && animal.look.build().is_some() {
                animal.toughened = true;
                animal.toughen(toughness);
            }
            animal.update(world, water, player, dt);
        }
        self.aim_at_serpents(view);

        // The dead don't fight
        let held = match self.slots.get(self.selected) {
            Some(&(Item::Weapon(i), _)) if self.dead.is_none() => Some(i),
            _ => None,
        };
        let controls = if self.dead.is_some() { Controls::default() } else { controls };
        // The special ammo in what's in hand, worked out again only when either changes
        let wanted = held.zip(self.chosen_ammo());
        if self.loaded.as_ref().map(|(key, _)| *key) != wanted {
            self.loaded = wanted.map(|(w, k)| {
                let profile = Rc::new(self.library.loaded(&self.weapons[w], k));
                ((w, k), Loaded { ammo: k, profile, head: self.library.ammo[k].head, burst: self.library.ammo[k].burst })
            });
        }
        let weapon = held.map(|i| (i, &self.weapons[i]));
        let loaded = self.loaded.as_ref().map(|(_, l)| l);
        self.combat.update(&mut Arena { world, animals: &mut self.animals, view, player, water }, weapon, loaded, controls, dt);
        for k in std::mem::take(&mut self.combat.spent) {
            self.spend_ammo(k);
        }
        for message in std::mem::take(&mut self.combat.messages) {
            self.say(message);
        }

        // What the bosses did, and what they threw landing
        let mut deeds = Vec::new();
        for animal in &mut self.animals {
            // What its attacks are made of: the Coil Unending's changes as it goes, see
            // serpents::ULTRA_ELEMENTS
            let element = serpents::element_now(animal);
            if let Some(boss) = &mut animal.boss {
                deeds.extend(boss.deeds.drain(..).map(|d| (animal.look, element, d)));
            }
        }
        for (look, element, deed) in deeds {
            self.carry_out(look, element, deed, world, water, player, view);
        }
        self.grow_roots(world, player, view, dt);
        self.whirl(world, water, player, dt);
        self.shove_out_of_serpents(player, world.voxels_per_meter as f64);
        self.spin_webs(player, dt);
        self.reek(world, player, dt);
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

    /// The whirlpool a great serpent keeps its lair in, see serpents::LairKind: the water spinning
    /// round where it is, spray thrown up off it, and whoever's swimming in it drawn round and in
    fn whirl(&mut self, world: &World, water: WaterLevel, player: Option<Vector3<f64>>, dt: f64) {
        let vpm = world.voxels_per_meter as f64;
        let time = self.combat.time();
        let pools: Vec<Vector3<f64>> = self
            .animals
            .iter()
            .filter(|a| a.health > 0.0 && serpents::lair_kind(a.look) == Some(serpents::LairKind::Whirlpool))
            .filter_map(|a| a.boss.as_ref().map(|b| b.home))
            .filter(|home| (home - self.feet.unwrap_or(*home)).norm() < 80.0 * vpm)
            .collect();
        for home in pools {
            let surface = water(home.x, home.y);
            if !surface.is_finite() || surface <= home.z - vpm {
                continue;
            }
            for k in 0..3 {
                let round = 3.0 + 3.5 * k as f64;
                let a = time * (1.6 - 0.35 * k as f64) + k as f64 * 2.1;
                let at = Vector3::new(home.x + a.cos() * round * vpm, home.y + a.sin() * round * vpm, surface);
                self.combat.stir(at, 0.5, dt, DROP_WATER);
            }
            // Drawn round and in, swimming in it
            if let Some(feet) = player {
                let off = (feet - home).xy() / vpm;
                let d = off.norm();
                if d > 1.0 && d < 12.0 && water(feet.x, feet.y) > feet.z + 0.3 * vpm {
                    let round = Vector2::new(-off.y, off.x) / d;
                    let pull = (round * 1.6 - off / d * 0.9) * (1.0 - d / 12.0);
                    self.thrown += Vector3::new(pull.x, pull.y, 0.0) * vpm * dt * 4.0;
                }
            }
        }
    }

    /// Whoever a great serpent's lunged through, come up beside or slithered over shoved out from
    /// its body, so they're never stood inside it with nothing to see, see serpents::Coils::shove()
    fn shove_out_of_serpents(&mut self, player: Option<Vector3<f64>>, vpm: f64) {
        let Some(feet) = player else {
            return;
        };
        let middle = feet + Vector3::new(0.0, 0.0, 0.9 * vpm);
        let push: Vector3<f64> = self.animals.iter().filter(|a| (a.position - feet).norm() < 80.0 * vpm).filter_map(|a| a.coils()).map(|c| c.shove(middle, 1.8, vpm)).sum();
        // Out about as far as it's in, as a shove dies away, see player::Player::push()
        if push.norm() > 0.05 * vpm {
            self.thrown += push * 9.0;
        }
    }

    /// The omens of the Coil Unending lying about in the world as it is, in its voxels, see
    /// serpents::omens()
    pub fn set_omens(&mut self, omens: Vec<(Look, Vector3<f64>, f64)>) {
        self.omens = omens;
    }

    /// How dark the sky's got over the Coil Unending's lair where the player is, 0 to 1, see
    /// serpents::gloom()
    pub fn set_gloom(&mut self, gloom: f64) {
        self.gloom = gloom;
    }

    /// Where blows on each serpent near enough to be hit land: the part of its long body the player's
    /// aiming at, see serpents::focus()
    fn aim_at_serpents(&mut self, view: &View) {
        let vpm = view.vpm;
        for a in self.animals.iter_mut().filter(|a| a.look.is_serpent() || a.look.is_great_serpent()) {
            a.focus = None;
            let points = match a.coils() {
                Some(coils) => Some(coils.hit_points(vpm)),
                None if (a.position - view.eye).norm() < 70.0 * vpm => a.spine(vpm as u32),
                None => None,
            };
            if let Some(points) = points {
                a.focus = serpents::focus(&points, view.eye, view.forward()).map(|p| p - a.position);
            }
        }
    }

    /// What a boss or monster did: hurting the player, throwing things, sounds and shaking the ground
    #[allow(clippy::too_many_arguments)]
    fn carry_out(&mut self, look: Look, element: Option<Element>, deed: Deed, world: &World, water: WaterLevel, player: Option<Vector3<f64>>, view: &View) {
        let vpm = world.voxels_per_meter as f64;
        // How much something at `at` is felt, from 1 right there to nothing 40 meters off
        let felt = |at: Vector3<f64>| (1.0 - (at - view.eye).norm() / vpm / 40.0).max(0.0);
        // Monsters snarl and rattle rather than roar, and don't shake the ground
        if look.is_monster() {
            let (at, m) = match deed {
                Deed::WindUp(m, at) => (at, Some(m)),
                Deed::Roar { at, .. } => (at, None),
                Deed::Stomp(_) => return,
                _ => (Vector3::zeros(), None),
            };
            if matches!(deed, Deed::WindUp(..) | Deed::Roar { .. }) {
                self.rng = hash(self.rng);
                let vary = 0.9 + 0.25 * random(&mut self.rng) as f32;
                let cue = match (look, m) {
                    (Look::Wolf, _) => Cue::at(Sound::Growl, at, 0.7),
                    (Look::Skeleton | Look::ArmoredSkeleton, _) => Cue::at(Sound::Rattle, at, 0.7),
                    (Look::Zombie | Look::ArmoredZombie, _) => Cue::at(Sound::Growl, at, 0.6).pitched(0.6),
                    // A crossbow spanned and its bolt laid in, ready to loose
                    (Look::Crossbowman, Some(Move::Shoot)) => {
                        self.cues.push(Cue::at(Sound::Ratchet, at, 0.6));
                        Cue::at(Sound::Latch, at, 0.7)
                    }
                    // A bandit's yell, and a shieldbearer beating its mace on its shield
                    (Look::Shieldbearer, None) => {
                        self.cues.push(Cue::at(Sound::Clang, at, 0.5).pitched(0.7));
                        Cue::at(Sound::Roar, at, 0.45).pitched(1.35)
                    }
                    (Look::Bandit | Look::BanditBrute | Look::Crossbowman | Look::Shieldbearer, _) => {
                        let pitch = match look {
                            Look::Bandit => 1.9,
                            Look::Crossbowman => 2.05,
                            _ => 1.45,
                        };
                        Cue::at(Sound::Roar, at, 0.45).pitched(pitch)
                    }
                    // Sand hissing away as it digs in, and its mandibles clicking
                    (Look::DuneBurrower, Some(Move::Burrow)) => Cue::at(Sound::Skitter, at, 0.7).pitched(0.55),
                    (Look::DuneBurrower, _) => Cue::at(Sound::Chitter, at, 0.7).pitched(vary),
                    // Frost gathering as it vanishes, and its wail
                    (Look::RimeWraith, Some(Move::Blink)) => Cue::at(Sound::Cast(Element::look(Some(Element::Frost)) as u8), at, 0.7).pitched(1.5),
                    (Look::RimeWraith, _) => Cue::at(Sound::Wail, at, 0.6).pitched(vary),
                    (Look::MireLurker, Some(Move::Lash)) => Cue::at(Sound::Croak, at, 0.8).pitched(1.3),
                    (Look::MireLurker, _) => Cue::at(Sound::Croak, at, 0.8).pitched(vary * 0.9),
                    (Look::Gloomwing, _) => Cue::at(Sound::Screech, at, 0.5).pitched(vary * 1.1),
                    // A tusker snorting and pawing the ground before it charges
                    (Look::Tusker, Some(Move::Charge)) => {
                        self.cues.push(Cue::at(Sound::Step(Ground::Grass), at, 0.8).pitched(0.6));
                        Cue::at(Sound::Snort, at, 0.9).pitched(0.85)
                    }
                    (Look::Tusker, _) => Cue::at(Sound::Snort, at, 0.8).pitched(vary),
                    // A bloater gurgling as it swells, a ghoul's shriek, a brute's groan, a bow
                    // creaking as it's drawn, and a necromancer's wail as it calls the dead
                    (Look::Bloater, Some(Move::Slam)) => {
                        self.cues.push(Cue::at(Sound::Squelch, at, 0.8).pitched(0.5));
                        Cue::at(Sound::Growl, at, 0.6).pitched(0.4)
                    }
                    (Look::Bloater, _) => Cue::at(Sound::Growl, at, 0.6).pitched(0.45),
                    (Look::Ghoul, _) => Cue::at(Sound::Screech, at, 0.6).pitched(vary * 0.75),
                    (Look::GraveBrute, _) => Cue::at(Sound::Growl, at, 0.8).pitched(0.4),
                    (Look::BoneArcher, Some(Move::Shoot)) => Cue::at(Sound::Creak, at, 0.7).pitched(1.4),
                    (Look::BoneArcher, _) => Cue::at(Sound::Rattle, at, 0.7).pitched(1.1),
                    (Look::Necromancer, Some(Move::Summon)) => Cue::at(Sound::Wail, at, 0.8).pitched(0.5),
                    (Look::Necromancer, Some(Move::Spit)) => Cue::at(Sound::Cast(Element::look(Some(Element::Void)) as u8), at, 0.7).pitched(0.6),
                    (Look::Necromancer, _) => Cue::at(Sound::Wail, at, 0.6).pitched(0.7),
                    // A serpent's hiss as it rears to strike, deeper as it raises its head to spit
                    // with its element gathering in its throat, the snow or sand hissing away as it
                    // dives in, and a loud hiss as it turns on the player or dies
                    (look, Some(Move::Spit)) if look.is_serpent() => {
                        self.cues.push(Cue::at(Sound::Cast(Element::look(look.element()) as u8), at, 0.5).pitched(0.7));
                        Cue::at(Sound::Hiss, at, 0.8).pitched(serpents::voice(look) * 0.8)
                    }
                    (look, Some(Move::Burrow)) if look.is_serpent() => Cue::at(Sound::Slither, at, 0.8).pitched(0.8),
                    (look, None) if look.is_serpent() => Cue::at(Sound::Hiss, at, 0.95).pitched(serpents::voice(look) * vary),
                    (look, _) if look.is_serpent() => Cue::at(Sound::Hiss, at, 0.7).pitched(serpents::voice(look) * vary * 1.1),
                    _ => Cue::at(Sound::Hiss, at, 0.5).pitched(1.5),
                };
                self.cues.push(cue);
                // A lurker or a coiler bursting up out of the water it lay in
                if matches!(look, Look::MireLurker | Look::SiltCoiler) && m.is_none() && water(at.x, at.y) > at.z {
                    self.combat.splash_up(Vector3::new(at.x, at.y, water(at.x, at.y)), 0.9);
                    if look == Look::SiltCoiler {
                        self.cues.push(Cue::at(Sound::Splash, at, 0.8).pitched(1.1));
                    }
                }
                // A viper rising up out of the grass it lay in, the blades thrown aside
                if look == Look::ReedViper && m.is_none() {
                    self.combat.stir(at, 0.9, 0.2, DROP_MUD);
                }
                return;
            }
        }
        let voice = match look.build() {
            Some(Build::Golem) => 0.6,
            Some(Build::Drake) => 1.15,
            Some(Build::Spider) => 1.5,
            Some(Build::Treant) => 0.5,
            _ if look.is_ultra() => 0.5,
            _ => 0.85,
        };
        match deed {
            // A great serpent's hiss, deep as its size, and what each move sounds like coming: its
            // body tightening to coil, its tail swishing up, water or earth churning as it dives,
            // the ground rumbling where it's coming up, its element gathering in its throat
            Deed::WindUp(m, at) if look.is_great_serpent() => {
                let deep = serpents::voice(look);
                let hiss = Cue::at(Sound::Hiss, at, 1.0).pitched(deep);
                let cue = match m {
                    Move::Coil => {
                        self.cues.push(Cue::at(Sound::Squeeze, at, 0.9).pitched(0.55));
                        Cue::at(Sound::Slither, at, 1.0).pitched(0.6)
                    }
                    Move::Tail => {
                        self.cues.push(Cue::at(Sound::Slither, at, 0.8).pitched(0.7));
                        Cue::at(Sound::Swoop, at, 0.9).pitched(0.55)
                    }
                    Move::Burrow if serpents::great_of(look).is_some_and(|g| g.medium == serpents::Medium::Water) => Cue::at(Sound::Splash, at, 1.0).pitched(0.6),
                    Move::Burrow => Cue::at(Sound::Dig, at, 1.0).pitched(0.5),
                    Move::Erupt => Cue::at(Sound::Rumble, at, 1.0).pitched(0.55),
                    Move::Spit | Move::Breath | Move::Storm | Move::Cataclysm => {
                        self.cues.push(Cue::at(Sound::Cast(Element::look(Some(element.unwrap_or(Element::Poison))) as u8), at, 0.8).pitched(0.45));
                        hiss
                    }
                    Move::Charge => {
                        self.cues.push(Cue::at(Sound::Roar, at, 0.6).pitched(deep * 1.2));
                        hiss
                    }
                    _ => hiss,
                };
                self.cues.push(cue);
            }
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
                // A great serpent's hiss under its roar, water thrown off it as it rears out of it
                if look.is_great_serpent() {
                    self.cues.push(Cue::at(Sound::Hiss, at, 1.0).pitched(serpents::voice(look) * 0.85));
                    if water(at.x, at.y) > at.z - 1.0 * vpm {
                        self.combat.splash_up(Vector3::new(at.x, at.y, water(at.x, at.y)), 1.6);
                        self.cues.push(Cue::at(Sound::Splash, at, 1.0).pitched(0.5));
                    }
                }
                self.cues.push(Cue::at(Sound::Roar, at, 1.0).pitched(voice * if angry { 1.1 } else { 1.0 }));
                // A drake spreads its wings as it roars
                if look.build() == Some(Build::Drake) {
                    self.cues.push(Cue::at(Sound::Flap, at, 0.8).pitched(0.8));
                }
                self.shake = self.shake.max(0.6 * felt(at));
                self.say(if look.is_ultra() && !angry {
                    "The water heaves and darkens: Ysveth, the Coil Unending, rises from the deep!".to_owned()
                } else if angry {
                    format!("The {} is enraged!", look.name())
                } else {
                    format!("The {} turns on you!", look.name())
                });
                if look.is_ultra() && !angry {
                    self.shake = self.shake.max(1.0 * felt(at));
                    self.cues.push(Cue::at(Sound::Quake, at, 1.0).pitched(0.5));
                }
            }
            Deed::Slam { at, radius, damage, crater } => {
                // What's built round it takes the blow too, see raids.rs
                self.blows.push(Blow { at, radius, damage, look });
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
                    Look::ArmoredSkeleton | Look::ArmoredZombie | Look::BanditBrute | Look::Shieldbearer => Cue::at(Sound::Clang, at, 0.5).pitched(0.8),
                    Look::Gloomwing => Cue::at(Sound::Bite, at, 0.6).pitched(1.7),
                    Look::MireLurker => Cue::at(Sound::Bite, at, 0.9).pitched(0.55),
                    Look::DuneBurrower => {
                        self.cues.push(Cue::at(Sound::Bite, at, 0.6).pitched(1.3));
                        Cue::at(Sound::Claw, at, 0.7).pitched(1.5)
                    }
                    Look::RimeWraith => Cue::at(Sound::Crack, at, 0.7).pitched(1.6),
                    Look::Tusker => Cue::at(Sound::Hit, at, 0.9).pitched(0.6),
                    Look::Crossbowman => Cue::at(Sound::Hit, at, 0.6).pitched(1.1),
                    Look::Bloater => Cue::at(Sound::Burst, at, 0.9).pitched(0.7),
                    Look::Ghoul => Cue::at(Sound::Claw, at, 0.8).pitched(1.2),
                    Look::GraveBrute => {
                        self.shake = self.shake.max(0.35 * felt(at));
                        Cue::at(Sound::Burst, at, 0.8).pitched(0.55)
                    }
                    // A serpent's jaws snapping shut where its head lunged to
                    _ if look.is_serpent() => {
                        self.cues.push(Cue::at(Sound::Swing, at, 0.5).pitched(1.3));
                        Cue::at(Sound::Bite, at, 0.9).pitched(serpents::voice(look) * 1.15)
                    }
                    // A great serpent's, or its tail or its coils coming down, shaking the ground
                    _ if look.is_great_serpent() => {
                        self.shake = self.shake.max(0.5 * felt(at));
                        self.cues.push(Cue::at(Sound::Burst, at, 0.8).pitched(0.5));
                        Cue::at(Sound::Bite, at, 1.0).pitched(serpents::voice(look))
                    }
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
                    // Not through a wall or a shut door
                    if d < radius && off.z.abs() < 2.5 * vpm && open_between(world, at, feet) {
                        self.hurt(damage * (1.0 - 0.5 * d / radius), at, feet, world, look.name());
                        self.afflict(element);
                        // A tusker tosses what it gores, and a shieldbearer shoves them back with
                        // its shield
                        let away = Vector3::new(off.x, off.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros);
                        match look {
                            Look::Tusker => self.thrown += (away * 1.5 + Vector3::new(0.0, 0.0, 4.0)) * vpm,
                            Look::Shieldbearer => self.thrown += (away * 3.5 + Vector3::new(0.0, 0.0, 0.8)) * vpm,
                            _ => {}
                        }
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
                // A monster's is smaller, like a wraith's ball of frost
                let (damage, radius) = match element {
                    _ if look.is_monster() => (11.0, 1.6),
                    Element::Poison => (16.0, 2.4),
                    _ => (24.0, 2.6),
                };
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
                // The Coil Unending sheds its skin for the next realm's element; the ground's left be
                if look.is_ultra() {
                    let said = match stage {
                        1 => "The Coil Unending sheds its skin: storm crackles down its coils, and it rises to strike!",
                        2 => "The Coil Unending sheds its skin again: blood wells in its scales, and its brood answers its call!",
                        _ => "The Coil Unending sheds its last skin: frost seizes its coils, and it rages! Its last fury is upon you.",
                    };
                    self.cues.push(Cue::at(Sound::Hiss, at, 1.0).pitched(0.3));
                    self.combat.splash_up(Vector3::new(at.x, at.y, water(at.x, at.y).max(at.z)), 1.8);
                    self.say(said);
                } else {
                    self.combat.craters.push((at, 1.5 * look.size()));
                    self.say(if stage >= 2 { format!("The {} is enraged! Its last fury is upon you.", look.name()) } else { format!("The {} calls down the sky!", look.name()) });
                }
            }
            Deed::Swipe(at, k) => self.cues.push(Cue::at(Sound::Claw, at, 0.85).pitched(0.9 + 0.1 * k as f32)),
            // A pack answering a howl, the dead clawing up out of the ground, spiderlings swarming
            // out: never more than a few of them about at once
            Deed::Summon { at, look: brood, count, pack } => {
                let most = match brood {
                    Look::Spiderling | Look::Bandit => 4,
                    Look::Skeleton => 3,
                    Look::ArmoredSkeleton | Look::Zombie => 4,
                    _ if brood.is_serpent() => 3,
                    _ => 2,
                };
                let near = self.animals.iter().filter(|a| a.look == brood && a.health > 0.0 && (a.position - at).norm() < 30.0 * vpm).count();
                let count = count.min(most - near.min(most));
                for _ in 0..count {
                    self.rng = hash(self.rng);
                    // Close round it, to fight alongside it
                    if let Some(mut monster) = animals::spawn_monster(world, water, brood, at, 3.5, self.rng) {
                        if let Some(state) = &mut monster.boss {
                            state.provoke();
                            state.pack = pack;
                            // A serpent rising up out of the ground or the water it was called from
                            if brood.is_serpent() {
                                state.emerge();
                            }
                        }
                        self.combat.stir(monster.position, 0.8, 0.4, DROP_MUD);
                        self.animals.push(monster);
                    }
                }
                let cue = match brood {
                    Look::Wolf => Cue::at(Sound::Howl, at, 1.0).pitched(voice * 0.9),
                    Look::Skeleton => Cue::at(Sound::Rattle, at, 1.0).pitched(0.8),
                    Look::Zombie => Cue::at(Sound::Growl, at, 1.0).pitched(0.5),
                    Look::Bandit => Cue::at(Sound::Roar, at, 0.9).pitched(1.5),
                    _ => Cue::at(Sound::Skitter, at, 1.0),
                };
                self.cues.push(cue);
                if count > 0 {
                    let deed = match brood {
                        Look::Wolf => "howls for its pack",
                        Look::Skeleton | Look::Zombie => "raises the dead",
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
            // A bolt from a crossbow, flying almost flat and hurting only what it hits
            Deed::Bolt { from, to } => {
                let time = ((to - from).norm() / vpm / BOLT_PACE).max(0.12);
                self.combat.hurl(HostileKind::Bolt, from, to, time, 11.0, 0.5);
                self.cues.push(Cue::at(Sound::Release, from, 0.8).pitched(1.1));
            }
            // Sand churned up over a burrower tunnelling along, hissing
            Deed::Burrow { at } => {
                // Snow thrown up over a drift serpent swimming along under it, or water churned over
                // a great serpent under it
                let snow = element == Some(Element::Frost);
                let kind = if look.swims() && water(at.x, at.y) > at.z { DROP_WATER } else if snow { DROP_ICE } else { DROP_MUD };
                let size = if look.is_great_serpent() { 1.6 } else { 0.45 };
                self.combat.stir(at, size, 0.14, kind);
                self.rng = hash(self.rng);
                if random(&mut self.rng) < 0.2 {
                    self.cues.push(if look.is_serpent() { Cue::at(Sound::Slither, at, 0.35).pitched(0.9) } else { Cue::at(Sound::Skitter, at, 0.35).pitched(0.5) });
                }
            }
            // Bursting up out of the sand, throwing whoever's on top of it into the air
            Deed::Erupt { at } => {
                let snow = element == Some(Element::Frost);
                self.combat.burst_up(at, if snow { DROP_ICE } else { DROP_MUD });
                self.cues.push(Cue::at(Sound::Burst, at, 0.8).pitched(1.4));
                self.cues.push(if look.is_serpent() { Cue::at(Sound::Hiss, at, 0.9).pitched(serpents::voice(look)) } else { Cue::at(Sound::Chitter, at, 0.8).pitched(0.8) });
                self.shake = self.shake.max(0.3 * felt(at));
                if let Some(feet) = player {
                    let off = feet - at;
                    if off.xy().norm() < 1.5 * vpm && off.z.abs() < 2.0 * vpm {
                        self.thrown += Vector3::new(0.0, 0.0, 4.5) * vpm;
                    }
                }
            }
            // A wraith gone in a swirl of frost, and back
            Deed::Blink { from, to } => {
                self.combat.blink(from, to);
                self.cues.push(Cue::at(Sound::Crack, from, 0.6).pitched(1.8));
                self.cues.push(Cue::at(Sound::Wail, to, 0.4).pitched(1.6));
            }
            // A lurker's tongue lashing out: caught, the player's dragged back in to its jaws
            Deed::Lash { from, to } => {
                let reach = match player {
                    Some(feet) => {
                        let body = feet + Vector3::new(0.0, 0.0, 1.0 * vpm);
                        let caught = (body - to).norm() < 1.4 * vpm;
                        (if caught { body } else { to }, caught.then_some(feet))
                    }
                    None => (to, None),
                };
                self.combat.tongue(from, reach.0);
                self.cues.push(Cue::at(Sound::Squelch, from, 0.8).pitched(1.5));
                if let Some(feet) = reach.1 {
                    self.hurt(9.0, from, feet, world, look.name());
                    let back = Vector3::new(from.x - feet.x, from.y - feet.y, 0.0).try_normalize(1e-6).unwrap_or_else(Vector3::zeros);
                    self.thrown += (back * 7.0 + Vector3::new(0.0, 0.0, 1.5)) * vpm;
                    self.cues.push(Cue::at(Sound::Bite, feet, 0.6).pitched(0.7));
                } else if let Some(feet) = player {
                    self.near_miss(feet);
                }
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
        // Braced on a siege crossbow's leg, all but still
        let chilled = chilled * self.combat.pace();
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

    /// Clouds of venom left where bloaters burst, poisoning whoever stands in them: a breath of it
    /// hurts every half second, and the venom goes on a moment after getting out
    fn reek(&mut self, world: &World, player: Option<Vector3<f64>>, dt: f64) {
        let vpm = self.combat.vpm();
        for k in 0..self.fumes.len() {
            let (at, left, wait) = self.fumes[k];
            self.combat.reek(at, FUMES_RADIUS * (0.6 + 0.4 * (left / FUMES_SECONDS).min(1.0)), dt);
            let mut wait = (wait - dt).max(0.0);
            if let Some(feet) = player {
                let off = feet - at;
                if off.xy().norm() < FUMES_RADIUS * vpm && off.z.abs() < 2.5 * vpm {
                    self.poisoned = self.poisoned.max(1.5);
                    if wait <= 0.0 {
                        wait = 0.5;
                        self.hurt(FUMES_DAMAGE, at, feet, world, "a bloater's venom");
                    }
                }
            }
            self.fumes[k] = (at, left - dt, wait);
        }
        self.fumes.retain(|f| f.1 > 0.0);
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
        // Harder in the realms further on
        let damage = damage * self.scaling.damage;
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
                animal.health = animal.max_health;
                // Burning or bleeding from the fight would turn it on them again
                animal.status = Default::default();
            }
        }
        self.combat.clear();
        self.roots.clear();
        self.webs.clear();
        self.fumes.clear();
        self.burning = 0.0;
        self.poisoned = 0.0;
        self.chilled = 0.0;
        self.snared = 0.0;
    }

    pub fn dead(&self) -> bool {
        self.dead.is_some()
    }

    /// Where the player last stood safe, their feet in voxels, where they'd rise again
    pub fn rest_spot(&self) -> Option<Vector3<f64>> {
        self.rest
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
            self.fumes.clear();
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
            animal.health = animal.max_health * fraction.clamp(0.01, 1.0);
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

    /// Where the middle of the living monster nearest `near` is, if there's one about
    pub fn nearest_foe(&self, near: Vector3<f64>) -> Option<Vector3<f64>> {
        let vpm = self.combat.vpm();
        self.animals
            .iter()
            .filter(|a| a.look.is_monster() && a.health > 0.0)
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

    /// How far a shot has kicked the view up, in radians
    pub fn kick(&self) -> f64 {
        self.combat.kick
    }

    /// A bundle of special ammo of any kind picked at random, between half a bundle and a whole one
    fn roll_ammo(&mut self) -> Option<Item> {
        if self.library.ammo.is_empty() {
            return None;
        }
        let k = (random(&mut self.rng) * self.library.ammo.len() as f64) as usize % self.library.ammo.len();
        let bundle = self.library.ammo[k].bundle;
        Some(Item::Ammo(k, (bundle / 2).max(1) + (random(&mut self.rng) * (bundle / 2 + 1) as f64) as u32))
    }

    /// Animals that were killed fall, and leave their meat. Bosses and monsters leave loot.
    fn reap(&mut self, world: &World) {
        let vpm = world.voxels_per_meter as f64;
        let mut cleared = Vec::new();
        let mut k = 0;
        while k < self.animals.len() {
            // A serpent thrashes, rolls over and lies still a moment before it's gone, and a great
            // serpent crashes down and sinks away
            let writhing = |a: &Animal| {
                let throes = if a.look.is_great_serpent() { serpents::GREAT_THROES } else { serpents::DEATH_THROES };
                (a.look.is_serpent() || a.look.is_great_serpent()) && a.boss.as_ref().is_some_and(|b| b.dying.is_none_or(|t| t < throes))
            };
            if self.animals[k].health > 0.0 || writhing(&self.animals[k]) {
                k += 1;
                continue;
            }
            let animal = self.animals.remove(k);
            let look = animal.look;
            let middle = animal.position + Vector3::new(0.0, 0.0, look.body().0 * vpm);
            // A monster bursts apart its own way. The dead and spiders' victims had a little gold on
            // them, now and then a weapon.
            // A bloater that burst by itself, rather than being killed, leaves nothing but its venom
            let burst = animal.health <= animals::BURST * 0.5;
            if look.is_monster() {
                self.combat.perish(&animal);
                // A serpent's blood, or the venom it was full of, spilling out along it
                if look.is_serpent() {
                    let kind = if look.element() == Some(Element::Poison) { DROP_VENOM } else { DROP_BLOOD };
                    self.combat.stir(animal.position, 1.2, 0.35, kind);
                }
                // A bloater bursts into a cloud of venom, however it died
                if look == Look::Bloater {
                    self.fumes.push((animal.position, FUMES_SECONDS, 0.3));
                    if self.fumes.len() > FUMES {
                        self.fumes.remove(0);
                    }
                }
            }
            if look.is_monster() && !burst {
                let mut loot = Vec::new();
                // People and the dead had a little gold on them; what spiders and burrowers caught
                // now and then did; wolves, gloomwings, lurkers and tuskers have nothing
                let undead = !look.is_beast();
                let nothing = matches!(look, Look::Wolf | Look::Gloomwing | Look::MireLurker | Look::Tusker);
                if !nothing && random(&mut self.rng) < if undead { 0.7 } else { 0.4 } {
                    loot.push(Item::Gold(if undead { 3 } else { 2 } + (random(&mut self.rng) * if undead { 10.0 } else { 5.0 }) as u32));
                }
                if undead && random(&mut self.rng) < MONSTER_WEAPON {
                    loot.push(self.roll_weapon(MONSTER_LUCK, false));
                } else if !nothing && random(&mut self.rng) < 0.1 {
                    loot.push(if random(&mut self.rng) < 0.5 { Item::Potion } else { Item::Bread });
                }
                // Bandits carry ammo for their bows and guns, a crossbowman most of all
                let ammo = match look {
                    Look::Crossbowman => 0.8,
                    // And skeleton archers their arrows
                    _ if look.is_bandit() || look == Look::BoneArcher => 0.5,
                    _ => 0.0,
                };
                if random(&mut self.rng) < ammo {
                    loot.extend(self.roll_ammo());
                }
                self.scatter(world, middle, animal.position, loot, 0.9);
            }
            if let Some(boss) = animal.boss.as_ref().filter(|_| look.is_boss()) {
                self.defeated.insert(boss.key);
                self.milestone = true;
                // One of a realm's raid bosses leaves its Innert block, straight into the player's
                // hands, so it can't be left behind and lost as the world moves on
                // A realm's finale leaves no block, see realms::on_raids_beaten()
                let block = self.raid_lairs.get(&boss.key).copied().filter(|_| !realms::is_finale_key(&boss.key));
                if let Some(realm) = block {
                    self.add_item(Item::InnertBlock(realm), 1);
                    self.cues.push(Cue::here(Sound::Chest, 1.0).pitched(0.6));
                }
                // The first of the sky isles' raid bosses beaten leaves a pair of wings to wear, and
                // the later realms' raid bosses better ones of their own
                if let Some(realm) = block.map(|r| r as usize) {
                    let kind = if realm == realms::WINGS_REALM && !self.carries(Item::Wings(wings::GULL)) { Some(wings::GULL) } else { wings::of_realm(realm) };
                    if let Some(kind) = kind {
                        let name = wings::KINDS[kind as usize].name;
                        self.find_once(Item::Wings(kind), &format!("Among what it leaves are the {name}!"));
                    }
                }
                // What it hoarded bursts out of it onto the ground round about: weapons, gold and
                // potions, a raid boss's far more and far better
                let ultra = look.is_ultra();
                let raid = look.is_raid() || block.is_some() || ultra;
                // Its best weapon's the luckiest, a raid boss's very likely with a special attack,
                // a huge one's, a finale's or an ultra's always, the rest a mix; the Coil Unending
                // leaves more than any raid boss, and its own fang among it
                let hoard = if look.is_raid() || ultra || realms::is_finale_key(&boss.key) {
                    Hoard::Huge
                } else if raid {
                    Hoard::Raid
                } else {
                    Hoard::Boss
                };
                let (piles, potions) = if ultra { (6, 5) } else if raid { (3, 3) } else { (1, 1) };
                let rolls = hoard_rolls(hoard, &mut self.rng);
                let mut loot: Vec<Item> = rolls.into_iter().map(|(luck, sure)| self.roll_weapon(luck, sure)).collect();
                if ultra {
                    let fang = self.fang();
                    loot.insert(0, fang);
                }
                for _ in 0..piles {
                    let gold = if ultra {
                        320.0 + 180.0 * random(&mut self.rng)
                    } else if raid {
                        200.0 + 100.0 * random(&mut self.rng)
                    } else {
                        80.0 + 80.0 * random(&mut self.rng)
                    };
                    loot.push(Item::Gold(gold as u32));
                }
                loot.extend(std::iter::repeat_n(Item::Potion, potions));
                let best = loot.iter().filter_map(|&item| match item {
                    Item::Weapon(w) => Some(self.weapons[w].grade),
                    _ => None,
                }).max().unwrap_or(0);
                // What a serpent that swims leaves is thrown up onto the land round the player,
                // rather than sinking into the deep water it went down into
                let around = if look.swims() { self.feet.unwrap_or(animal.position) } else { animal.position };
                self.scatter(world, middle, around, loot, 2.0 * look.size().sqrt() + look.body().1 * 0.5);
                self.combat.fell(middle, look.body().1);
                self.shake = self.shake.max(1.0);
                self.cues.push(Cue::here(Sound::Fanfare, if raid { 1.0 } else { 0.8 }));
                let rarity = self.library.rarities[best].display_name.to_lowercase();
                if ultra {
                    self.say(format!("Ysveth, the {}, is slain! Its coils sink into the deep, and among its hoard lies the {}, mythic and like no other.", look.name(), serpents::FANG));
                } else if let Some(realm) = block {
                    let name = realms::REALMS[realm as usize % realms::REALMS.len()].name;
                    self.say(format!("Raid boss slain! The {} falls, and leaves an Innert block of {name}. Its hoard spills out, some of it {rarity}!", look.name()));
                } else if raid {
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
            if burst {
                self.say("A bloater bursts in a cloud of venom!");
            } else if look.is_monster() {
                self.say(format!("You slew a {}.", look.name()));
            } else if !look.is_boss() {
                self.say(if look.meat() > 0 { format!("You hunted a {}. It dropped some meat.", look.name()) } else { format!("You hunted a {}.", look.name()) });
            }
            // The last of a den's keepers dead leaves it clear for good: a dungeon hall, a nest, or a
            // bandits' camp, its warlord as well
            if let Some(key) = animal.boss.as_ref().map(|b| b.key).filter(|key| self.dens.contains(key) && !self.cleared.contains(key))
                && !self.animals.iter().any(|a| a.health > 0.0 && a.boss.as_ref().is_some_and(|b| b.key == key))
            {
                self.cleared.insert(key);
                cleared.push(key);
                self.milestone = true;
            }
            self.target = None;
        }
        // Where a camp was cleared, the chest of the bandits' hoard turns up, said after the rest of
        // what fell
        for key in cleared {
            self.reveal_hoard(key);
        }
    }

    /// The camp whose den is at `key` was just cleared: the chest of its bandits' hoard turns up,
    /// in a burst of light with a fanfare, and the player's told where it is
    fn reveal_hoard(&mut self, key: [i64; 3]) {
        let Some((chest, hint)) = self.hoards.get(&key).cloned() else {
            return;
        };
        // The burst of light a boss falls in, without the ground shaking
        let (vpm, shake) = (self.combat.vpm(), self.combat.shake);
        self.combat.fell(chest.position + Vector3::new(0.0, 0.0, 0.3 * vpm), 0.5);
        self.combat.shake = shake;
        self.cues.push(Cue::here(Sound::Fanfare, 0.7));
        self.cues.push(Cue::at(Sound::Chest, chest.position, 0.9).pitched(0.8));
        if !self.chests.iter().any(|c| c.key == chest.key) {
            self.chests.push(chest);
        }
        self.say(format!("The camp is clear! The bandits' hoard lies {hint}."));
    }

    /// For trying it out: every bandit keeping the camp nearest `near`, in voxels, falls dead, its
    /// warlord too, so its hoard turns up. Returns how many fell.
    pub fn clear_nearest_camp(&mut self, world: &World, near: Vector3<f64>) -> usize {
        let camp = self.hoards.keys().filter(|key| !self.cleared.contains(*key)).min_by_key(|key| {
            let keepers = self.animals.iter().filter(|a| a.boss.as_ref().is_some_and(|b| b.key == **key));
            keepers.map(|a| (a.position - near).norm() as i64).min().unwrap_or(i64::MAX)
        });
        let Some(&key) = camp else {
            return 0;
        };
        let mut fell = 0;
        for animal in self.animals.iter_mut().filter(|a| a.boss.as_ref().is_some_and(|b| b.key == key)) {
            animal.health = 0.0;
            fell += 1;
        }
        // Straight away, before they heal back up
        self.reap(world);
        fell
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
                        Item::Gold(_) => Cue::at(Sound::Coins, pickup.position, 0.35).pitched(1.1),
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
        // So do the hoards of the camps cleared, until they're opened
        self.beams.clear();
        for chest in self.chests.iter().filter(|c| c.hoard && !self.opened.contains(&c.key)) {
            let to = chest.position + Vector3::new(0.0, 0.0, 0.5 * vpm) - view.eye;
            let d = to.norm();
            let steps = (d / (0.25 * vpm)) as usize;
            let seen = d < BEAM_RANGE * vpm
                && (1..steps).all(|s| {
                    let p = view.eye + to * (s as f64 / steps as f64);
                    is_passable(world.material_at(p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64))
                });
            if seen {
                self.beams.push(chest.position);
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
        self.combat.renumber(|i| kept.get(i).copied().unwrap_or(false).then(|| to[i]));
        self.loaded = None;
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

    /// The Coil Unending's own weapon, the one of its kind: a mythic whip whose special is a serpent
    /// lash or a coil bind if any is, made the same every time, and called by its own name, which
    /// it keeps in the save, see save::WeaponSave::name
    fn fang(&mut self) -> Item {
        let family = self.library.family("whip");
        let mythic = self.library.rarities.len() - 1;
        let serpentine = |s: u32| matches!(self.library.special_of(s, family, FANG_LUCK), Some(Special::SerpentLash | Special::CoilBind));
        // The first seed that makes it of the rarest grade with a serpent's special, or failing
        // that the first with the special
        let mut found = None;
        for seed in (0..20_000u32).map(|k| hash(k ^ 0x7e57_f00d)).filter(|&s| serpentine(s)) {
            let weapon = self.library.generate(seed, family, FANG_LUCK);
            let best = weapon.grade == mythic;
            if best || found.is_none() {
                found = Some(weapon);
            }
            if best {
                break;
            }
        }
        let mut weapon = found.unwrap_or_else(|| self.library.generate(0x7e57_f00d, family, FANG_LUCK));
        weapon.name = serpents::FANG.to_owned();
        self.weapons.push(weapon);
        self.models_version += 1;
        Item::Weapon(self.weapons.len() - 1)
    }

    /// A new weapon rolled with `luck`, as for Library::odds(), kept among Game::weapons: with
    /// `sure`, one with a special attack
    fn roll_weapon(&mut self, luck: f64, sure: bool) -> Item {
        self.rng = hash(self.rng ^ 0x1007);
        // Better in the realms further on
        let luck = luck * loot_scaling(self.scaling);
        let seed = if sure { self.library.special_seed(self.rng, None, luck) } else { self.rng };
        let weapon = self.library.generate(seed, None, luck);
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
            Item::Ammo(k, n) => (self.library.ammo[k].price * n as f64).ceil() as u32,
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

    /// Adds `count` of `item` to what's carried, on its stack if there is one: ammo of the same
    /// kind goes on the one stack however many are in it
    fn add_item(&mut self, item: Item, count: u32) {
        // The first wings found are put on at once
        if let Item::Wings(k) = item
            && self.worn.is_none()
        {
            self.worn = Some(k);
            return;
        }
        if let Item::Ammo(k, more) = item {
            match self.slots.iter_mut().find(|(i, _)| matches!(i, Item::Ammo(a, _) if *a == k)) {
                Some((Item::Ammo(_, n), _)) => *n += more * count,
                _ => self.slots.push((Item::Ammo(k, more * count), 1)),
            }
            return;
        }
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
                if look == Look::Merchant || look == Look::Princess {
                    let id = self.animals[i].id;
                    if let Some(&(_, key)) = self.merchants.iter().find(|(m, _)| *m == id) {
                        self.stock(key);
                        self.shop = Some(key);
                        self.cues.push(Cue::here(Sound::Chest, 0.5).pitched(1.3));
                        self.say(if look == Look::Princess {
                            "\"Welcome to the royal armoury. Only blades fit for a legend, and they're priced like it.\""
                        } else {
                            "\"Have a look. Good steel, fresh bread, potions that work.\""
                        });
                    }
                } else if look == Look::RoyalGuard {
                    self.say("\"Her Highness receives travellers in the throne room. Mind your manners.\"");
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
            // At the Innert Gate or a portal home, see realms.rs
            None if self.shrine_near(eye).is_some() => {
                self.shrine_used = true;
                self.cues.push(Cue::here(Sound::Chest, 0.5).pitched(0.7));
            }
            None => {}
        }
    }

    /// Picks up `item`: gold goes in the purse, the rest into what's carried
    fn take(&mut self, item: Item) {
        match item {
            Item::Gold(n) => {
                self.gold += n;
                self.cues.push(Cue::here(Sound::Coins, 0.7));
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
            Item::Ammo(k, n) => {
                self.add_item(item, 1);
                self.cues.push(Cue::here(Sound::Ratchet, 0.6).pitched(0.7));
                self.say(format!("+{n} {}", self.library.ammo[k].display_name));
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
        let weapon = self.library.generate(seed, None, chest.luck * loot_scaling(self.scaling));
        let gold = 10 + (hash(seed ^ 0x601d) % 30) as u32 + (chest.luck * 4.0) as u32;
        // A bandits' hoard has what they've taken in it too
        let hoard = chest.hoard;
        let gold = if hoard { gold + 60 + (hash(seed ^ 0x40a2) % 60) as u32 } else { gold };
        self.opened.insert(key);
        self.milestone = true;
        self.target = None;
        let rarity = &self.library.rarities[weapon.grade];
        let family = &self.library.families[weapon.family];
        // Half of them have a bundle of special ammo in too
        let ammo = (!self.library.ammo.is_empty() && hash(seed ^ 0xa770) % 2 == 0).then(|| {
            let k = hash(seed ^ 0xa771) as usize % self.library.ammo.len();
            (k, self.library.ammo[k].bundle)
        });
        let extra = ammo.map_or(String::new(), |(k, n)| format!(", {n} {}", self.library.ammo[k].display_name.to_lowercase()));
        // Now and then, wings, likelier the luckier the chest, the rarer kinds rarer
        let pair = (((hash(seed ^ 0x3175) >> 8) as f64 / 16_777_216.0) < (0.025 * chest.luck * self.scaling.luck).min(0.2)).then(|| wings::roll((hash(seed ^ 0x3176) >> 8) as f64 / 16_777_216.0)).filter(|&k| !self.carries(Item::Wings(k)));
        let extra = match pair {
            Some(k) => format!("{extra}, the {}", wings::KINDS[k as usize].name),
            None => extra,
        };
        let found = if hoard { "In the bandits' hoard you find" } else { "You found" };
        let grade = rarity.display_name.to_lowercase();
        let a = if grade.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
        let message = format!("{found} {}, {a} {grade} {}{extra}, and {gold} gold!", weapon.name, family.display_name.to_lowercase());
        self.weapons.push(weapon);
        self.slots.push((Item::Weapon(self.weapons.len() - 1), 1));
        if let Some((k, n)) = ammo {
            self.add_item(Item::Ammo(k, n), 1);
        }
        if let Some(k) = pair {
            self.add_item(Item::Wings(k), 1);
        }
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
            let keeper = match stall.royal {
                Some(heading) => animals::spawn_royal(world, water, Look::Princess, stall.position, heading, seed),
                None => animals::spawn_merchant(world, water, stall.position, stall.within, seed),
            };
            if let Some(merchant) = keeper {
                if stall.royal.is_some() {
                    self.royals.insert(stall.key);
                }
                self.merchants.push((merchant.id, stall.key));
                self.animals.push(merchant);
            }
        }
    }

    /// The royal guards' posts in the piece of the world just built, in its voxels, with which way
    /// each faces: a guard turns up at each that hasn't one already
    pub fn set_guards(&mut self, world: &World, water: WaterLevel, posts: Vec<(Vector3<f64>, f64)>) {
        let vpm = world.voxels_per_meter as f64;
        for (at, heading) in posts {
            if self.animals.iter().any(|a| a.look == Look::RoyalGuard && (a.position - at).xy().norm() < 1.5 * vpm) {
                continue;
            }
            let seed = hash(at.x as u32 ^ hash(at.y as u32 ^ 0x6a2d));
            if let Some(guard) = animals::spawn_royal(world, water, Look::RoyalGuard, at, heading, seed) {
                self.animals.push(guard);
            }
        }
    }

    /// What the princess keeping royal armoury `key` sells: legendary and mythic weapons only,
    /// several of them and no two of a kind, at their price, and a few potions
    fn royal_stock(&mut self, key: [i64; 3]) {
        let seed = hash(self.seed ^ hash(key[0] as u32 ^ hash(key[1] as u32 ^ hash(key[2] as u32 ^ 0x9e11))));
        let mut wares = Vec::new();
        for weapon in royal_weapons(&self.library, seed) {
            self.weapons.push(weapon);
            wares.push(Ware::Weapon(self.weapons.len() - 1));
        }
        self.models_version += 1;
        wares.sort_by_key(|w| match *w {
            Ware::Weapon(i) => self.weapon_price(i),
            Ware::Food(..) => 0,
        });
        wares.push(Ware::Food(Item::Potion, 5));
        // And royal wings, and the cinder wings, for those who can pay
        wares.push(Ware::Food(Item::Wings(wings::AURELIAN), 1));
        wares.push(Ware::Food(Item::Wings(wings::CINDERWING), 1));
        self.stocks.insert(key, wares);
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
        if !self.stocks.contains_key(&key) {
            self.roll_stock(key);
        }
        // Once the way to the sky isles is open, a hook and wings for whoever hasn't found theirs
        if self.sky_wares && !self.royals.contains(&key) {
            let missing: Vec<Item> = [Item::Hook, Item::Wings(wings::GULL)].into_iter().filter(|&i| !self.carries(i)).collect();
            if let Some(wares) = self.stocks.get_mut(&key) {
                for item in missing {
                    if !wares.iter().any(|w| matches!(w, Ware::Food(i, _) if *i == item)) {
                        wares.push(Ware::Food(item, 1));
                    }
                }
            }
        }
    }

    /// What merchant `key` has to sell the first time it's looked at, see stock()
    fn roll_stock(&mut self, key: [i64; 3]) {
        if self.royals.contains(&key) {
            return self.royal_stock(key);
        }
        let mut seed = hash(self.seed ^ hash(key[0] as u32 ^ hash(key[1] as u32 ^ hash(key[2] as u32 ^ 0x5709))));
        let mut wares = Vec::new();
        for luck in MERCHANT_LUCKS {
            seed = hash(seed);
            self.weapons.push(self.library.generate(seed, None, luck * loot_scaling(self.scaling)));
            wares.push(Ware::Weapon(self.weapons.len() - 1));
        }
        // And a fine tool, for gathering with, see tools.rs
        let tool = tools::Tool::ALL[hash(seed ^ 0x7001) as usize % tools::Tool::ALL.len()];
        if let Some(made) = tools::make(&self.library, tool, tools::FINE) {
            self.weapons.push(made);
            wares.push(Ware::Weapon(self.weapons.len() - 1));
        }
        self.models_version += 1;
        wares.sort_by_key(|w| match *w {
            Ware::Weapon(i) => self.weapon_price(i),
            Ware::Food(..) => 0,
        });
        wares.extend([Ware::Food(Item::Potion, 4), Ware::Food(Item::Bread, 6), Ware::Food(Item::Meat, 5), Ware::Food(Item::Apple, 10)]);
        // A few kinds of special ammo, a bundle at a time
        let mut kinds: Vec<usize> = Vec::new();
        for _ in 0..4 {
            seed = hash(seed);
            let k = seed as usize % self.library.ammo.len().max(1);
            if k < self.library.ammo.len() && !kinds.contains(&k) {
                kinds.push(k);
                wares.push(Ware::Food(Item::Ammo(k, self.library.ammo[k].bundle), 3));
            }
        }
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
        if matches!(item, Item::Hook | Item::Wings(_)) && self.carries(item) {
            self.say(format!("You have {} already.", item.name().to_lowercase()));
            return;
        }
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
        self.cues.push(Cue::here(Sound::Coins, 0.8));
        self.milestone = true;
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
        if item.kept() {
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
        self.cues.push(Cue::here(Sound::Coins, 0.8).pitched(0.9));
        self.milestone = true;
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
        if item.kept() {
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
        // Ammo isn't held, it's loaded into what shoots it
        if let Item::Ammo(k, _) = self.slots[slot].0 {
            self.load_ammo(k);
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

    /// For trying them out: rolls lucky weapons of any family, or of family `family` if that's
    /// given and there is one called that, until one has the special attack `special`, puts it in
    /// hand and charges it
    pub fn find_special(&mut self, special: Special, family: Option<&str>) {
        let family = family.and_then(|id| self.library.family(id));
        for seed in 0..200_000u32 {
            // Looked for in the parts alone, as building each weapon's models takes a while
            if self.library.special_of(hash(seed ^ 0x5bec), family, 8.0) != Some(special) {
                continue;
            }
            let weapon = self.library.generate(hash(seed ^ 0x5bec), family, 8.0);
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

    /// For looking at parts: a weapon with each of the parts `ids` lying on the ground in rows of
    /// four from a couple of meters along `ahead` from `near`, or one or two close by, one in front
    /// of the other, each `size` of weapons::SIZES big if that's given or its id says, as `id@4`.
    /// Gives back the middle of them, and how long the longest is, in meters.
    pub fn lay_out(&mut self, world: &World, ids: &[&str], size: Option<usize>, near: Vector3<f64>, ahead: Vector3<f64>, hold: bool) -> (Vector3<f64>, f64) {
        let vpm = world.voxels_per_meter as f64;
        let mut middle = Vector3::zeros();
        let mut longest: f64 = 0.0;
        let flat = Vector3::new(ahead.x, ahead.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x());
        let side = Vector3::new(flat.y, -flat.x, 0.0);
        for (k, id) in ids.iter().enumerate() {
            // `pitted_blade@4` makes that one weapons::SIZES[4] big, whatever `size` says
            let (id, size) = match id.split_once('@') {
                Some((id, s)) => (id, s.parse().ok().or(size)),
                None => (*id, size),
            };
            let Some(weapon) = self.library.with_part(id, size) else {
                println!("No weapon could be made with {id}");
                continue;
            };
            println!("lay_out {k}: {}", self.describe(&weapon));
            // The first in hand too, with `hold`
            if hold && k == 0 {
                self.weapons.push(self.library.with_part(id, size).unwrap());
                self.slots.insert(0, (Item::Weapon(self.weapons.len() - 1), 1));
                self.selected = 0;
            }
            // In rows of four, or for a close look at one or two, one in front of the other
            let (row, column, apart) = if ids.len() <= 2 { (k as f64, 1.5, 0.5) } else { ((k / 4) as f64, (k % 4) as f64, 1.25) };
            let ahead = if ids.len() <= 2 { 1.6 } else { 2.4 };
            let over = near + (flat * (ahead + row * apart) + side * (column - 1.5) * 1.05) * vpm + Vector3::new(0.0, 0.0, 2.0 * vpm);
            let ground = (0..(40.0 * vpm) as i64)
                .map(|d| over - Vector3::new(0.0, 0.0, d as f64))
                .find(|p| !is_passable(world.material_at(p.x as i64, p.y as i64, p.z as i64)))
                .map_or(over, |p| p + Vector3::new(0.0, 0.0, 1.0));
            longest = longest.max(weapon.look.longest());
            self.weapons.push(weapon);
            self.pickups.push(Pickup::lying(Item::Weapon(self.weapons.len() - 1), ground, side.y.atan2(side.x)));
            middle += ground / ids.len() as f64;
        }
        self.models_version += 1;
        // Nothing in hand to get in the way of looking at them
        if !hold {
            self.selected = usize::MAX;
        }
        println!("lay_out: the longest {longest:.2} m, their middle at {:.0} {:.0} {:.0}", middle.x, middle.y, middle.z);
        (middle, longest)
    }

    /// For looking at pools, `--script pools`: one of each element and one of churned earth laid
    /// out over the ground `ahead` of `eye`, three by three, hurting nothing
    pub fn lay_pools(&mut self, world: &World, water: WaterLevel, eye: Vector3<f64>, ahead: Vector3<f64>) -> Vector3<f64> {
        let profile = match self.held() {
            Some((_, w)) => w.profile.clone(),
            None => self.library.generate(hash(self.seed ^ 0x9001), None, 1.0).profile.clone(),
        };
        self.combat.lay_pools(world, water, &profile, eye, ahead)
    }

    /// For screenshot runs, `--script shore`: a pool of `element` on the ground at `ground`, as a
    /// shot of it leaves one, hurting nothing
    pub fn lay_pool(&mut self, ground: Vector3<f64>, element: Option<Element>) {
        let profile = self.library.generate(hash(self.seed ^ 0x9001), None, 1.0).profile.clone();
        self.combat.lay_pool(ground, element, &profile);
    }

    /// For screenshot runs, `--script far`: every animal within `meters` of `near` gone, so a
    /// spell cast out over the land has nothing to home in on
    pub fn shoo_animals(&mut self, near: Vector3<f64>, meters: f64) {
        let vpm = self.combat.vpm();
        self.animals.retain(|a| a.look.is_boss() || (a.position - near).norm() > meters * vpm);
    }

    /// What's in flight, and how far off the furthest of it is, for screenshot runs
    pub fn in_flight(&self, eye: Vector3<f64>) -> String {
        let (count, furthest) = self.combat.in_flight(eye);
        format!("{count} in flight, the furthest {furthest:.0} m off")
    }

    /// Takes the first weapon of family `id` carried in hand, or for trying it out, one rolled if
    /// none is carried
    pub fn select_family(&mut self, id: &str) {
        let Some(family) = self.library.family(id) else {
            return;
        };
        let carried = self.slots.iter().position(|(item, _)| matches!(item, Item::Weapon(i) if self.weapons[*i].family == family));
        match carried {
            Some(slot) => self.hold(slot),
            None => {
                self.weapons.push(self.library.generate(hash(self.seed ^ 0xfa31), Some(family), 1.0));
                self.slots.insert(0, (Item::Weapon(self.weapons.len() - 1), 1));
                self.selected = 0;
                self.models_version += 1;
            }
        }
    }

    /// Takes the first weapon of `kind` carried in hand
    pub fn select_weapon(&mut self, kind: Kind) {
        if let Some(slot) = self.slots.iter().position(|(item, _)| matches!(item, Item::Weapon(i) if self.weapons[*i].kind() == kind)) {
            self.hold(slot);
        }
    }

    /// Whether something's happened since last asked that's worth saving the game for straight
    /// away: a boss beaten, a camp cleared, a chest opened, something bought or sold
    pub fn take_milestone(&mut self) -> bool {
        std::mem::take(&mut self.milestone)
    }

    /// Weapon `w` as it's saved, by what it's made of: each part by its id, and where a few parts
    /// share an id, as some do in assets\weapons.json, which of them it is after a #, as
    /// `earthshaker_head#1` for the second
    fn weapon_save(&self, w: usize) -> save::WeaponSave {
        let weapon = &self.weapons[w];
        let components = &self.library.components;
        let part = |p: usize| {
            let id = &components[p].id;
            match components[..p].iter().filter(|c| c.id == *id).count() {
                0 => id.clone(),
                twin => format!("{id}#{twin}"),
            }
        };
        // The Coil Unending's fang keeps its own name
        let name = (weapon.name == serpents::FANG).then(|| weapon.name.clone());
        save::WeaponSave { family: self.library.families[weapon.family].id.clone(), parts: weapon.parts.iter().map(|&p| part(p)).collect(), seed: weapon.seed, name }
    }

    /// Makes a saved weapon again, kept among Game::weapons: None if the parts it was made of are
    /// no more, or don't fit its family any more
    fn weapon_from(&mut self, saved: &save::WeaponSave) -> Option<usize> {
        let family = self.library.family(&saved.family)?;
        let slots = &self.library.families[family].slots;
        let components = &self.library.components;
        let part = |name: &String| {
            let (id, twin) = name.split_once('#').map_or((name.as_str(), 0), |(id, n)| (id, n.parse().unwrap_or(0)));
            components.iter().enumerate().filter(|(_, c)| c.id == id).nth(twin).map(|(p, _)| p)
        };
        let parts: Vec<usize> = saved.parts.iter().map(part).collect::<Option<_>>()?;
        if parts.len() != slots.len() || parts.iter().zip(slots).any(|(&p, slot)| self.library.components[p].slot_type != *slot) {
            return None;
        }
        let mut weapon = Weapon::assemble(&self.library, family, parts, saved.seed);
        if let Some(name) = &saved.name {
            weapon.name = name.clone();
        }
        self.weapons.push(weapon);
        Some(self.weapons.len() - 1)
    }

    /// `item` as it's saved
    fn item_save(&self, item: Item) -> save::ItemSave {
        use save::ItemSave as S;
        match item {
            Item::Bucket => S::Bucket,
            Item::Apple => S::Apple,
            Item::Meat => S::Meat,
            Item::RoastMeat => S::RoastMeat,
            Item::Bread => S::Bread,
            Item::Potion => S::Potion,
            Item::Gold(amount) => S::Gold { amount },
            Item::Weapon(w) => S::Weapon(self.weapon_save(w)),
            Item::Ammo(k, rounds) => S::Ammo { id: self.library.ammo[k].id.clone(), rounds },
            Item::InnertBlock(realm) => S::InnertBlock { realm },
            Item::Hook => S::Hook,
            Item::Wings(k) => S::Wings { of: Some(wings::KINDS[k as usize].id.to_owned()) },
        }
    }

    /// A saved item made again: None for what's no more, like ammo since taken out of the game
    fn item_from(&mut self, saved: &save::ItemSave) -> Option<Item> {
        use save::ItemSave as S;
        Some(match saved {
            S::Bucket => Item::Bucket,
            S::Apple => Item::Apple,
            S::Meat => Item::Meat,
            S::RoastMeat => Item::RoastMeat,
            S::Bread => Item::Bread,
            S::Potion => Item::Potion,
            S::Gold { amount } => Item::Gold(*amount),
            S::Weapon(w) => Item::Weapon(self.weapon_from(w)?),
            S::Ammo { id, rounds } => Item::Ammo(self.library.ammo.iter().position(|a| a.id == *id)?, *rounds),
            S::InnertBlock { realm } if (*realm as usize) < realms::REALMS.len() => Item::InnertBlock(*realm),
            S::Hook => Item::Hook,
            // From before wings were worn: the plain ones
            S::Wings { of } => Item::Wings(of.as_deref().map_or(Some(wings::GULL), wings::by_id)?),
            S::InnertBlock { .. } | S::Unknown => return None,
        })
    }

    /// What the player carries, as it's saved
    pub fn carried_save(&self) -> save::CarriedSave {
        save::CarriedSave {
            gold: self.gold,
            selected: self.selected.min(self.slots.len().saturating_sub(1)),
            bucket: self.bucket,
            slots: self.slots.iter().map(|&(item, count)| save::SlotSave { item: self.item_save(item), count }).collect(),
            worn: self.worn.map(|k| wings::KINDS[k as usize].id.to_owned()),
        }
    }

    /// What's been done in this world that stays done, as it's saved: chests opened, bosses beaten,
    /// dens cleared and what's left on the merchants' stalls
    pub fn world_save(&self) -> save::WorldSave {
        let sorted = |set: &HashSet<[i64; 3]>| {
            let mut keys: Vec<[i64; 3]> = set.iter().copied().collect();
            keys.sort_unstable();
            keys
        };
        let mut stocks: Vec<save::StockSave> = self
            .stocks
            .iter()
            .map(|(&key, wares)| save::StockSave {
                key,
                wares: wares
                    .iter()
                    .map(|ware| match *ware {
                        Ware::Weapon(w) => save::WareSave::Weapon(self.weapon_save(w)),
                        Ware::Food(item, left) => save::WareSave::Goods { item: self.item_save(item), left },
                    })
                    .collect(),
            })
            .collect();
        stocks.sort_by_key(|s| s.key);
        save::WorldSave { opened: sorted(&self.opened), defeated: sorted(&self.defeated), cleared: sorted(&self.cleared), stocks, ..Default::default() }
    }

    /// Health, hunger and mana
    pub fn vitals(&self) -> (f64, f64, f64) {
        (self.health, self.hunger, self.combat.mana)
    }

    /// Lets go of everything of the world the player was in: its animals, bosses, monsters,
    /// merchants and what lies about, for another to be built round them, landscape `seed`. What
    /// they carry stays, and so does what's been done there, till restore() or new_character().
    pub fn leave_world(&mut self, seed: u32) {
        self.animals.clear();
        self.pickups.clear();
        self.props.clear();
        self.chests.clear();
        self.lairs.clear();
        self.dens.clear();
        self.boss_chests.clear();
        self.hoards.clear();
        self.beams.clear();
        self.merchants.clear();
        self.royals.clear();
        self.shop = None;
        self.target = None;
        self.combat.clear();
        self.leaves.clear();
        self.roots.clear();
        self.webs.clear();
        self.fumes.clear();
        self.rest = None;
        self.safe_for = 0.0;
        self.knockback = Vector3::zeros();
        self.thrown = Vector3::zeros();
        self.raid_lairs.clear();
        self.shrines.clear();
        self.shrine_used = false;
        self.omens.clear();
        self.gloom = 0.0;
        self.seed = seed;
    }

    /// Someone setting out afresh: what the player starts with, at full health, and nothing yet
    /// done anywhere
    pub fn new_character(&mut self, seed: u32) {
        self.slots.clear();
        self.worn = None;
        self.arm_anew(seed);
        self.gold = STARTING_GOLD;
        self.bucket = 0.0;
        self.opened.clear();
        self.defeated.clear();
        self.cleared.clear();
        self.stocks.clear();
        self.ammo_choice.clear();
        self.set_vitals(MAX_HEALTH, 80.0, MAX_MANA);
    }

    /// The player as saved: what they carry, how they are, and what's been done in the world
    /// they're in. What can't be made again, like a weapon whose parts are no more, is left out.
    pub fn restore(&mut self, carried: &save::CarriedSave, world: &save::WorldSave, (health, hunger, mana): (f64, f64, f64)) {
        self.weapons.clear();
        self.combat.renumber(|_| None);
        self.loaded = None;
        self.ammo_choice.clear();
        self.slots.clear();
        for slot in &carried.slots {
            if let Some(item) = self.item_from(&slot.item) {
                self.slots.push((item, slot.count.max(1)));
            }
        }
        // Wings saved before they were worn are put on, as all wings now are
        self.worn = carried.worn.as_deref().and_then(wings::by_id);
        if self.worn.is_none()
            && carried.slots.iter().any(|s| s.item == save::ItemSave::Wings { of: None })
            && let Some(at) = self.slots.iter().position(|&(i, _)| i == Item::Wings(wings::GULL))
        {
            self.slots.remove(at);
            self.worn = Some(wings::GULL);
        }
        self.selected = carried.selected.min(self.slots.len().saturating_sub(1));
        self.gold = carried.gold;
        self.bucket = carried.bucket.clamp(0.0, BUCKET_CAPACITY);
        self.restore_world(world);
        self.set_vitals(health, hunger, mana);
        self.models_version += 1;
    }

    /// What's been done in the world the player's in, as saved
    pub fn restore_world(&mut self, world: &save::WorldSave) {
        self.opened = world.opened.iter().copied().collect();
        self.defeated = world.defeated.iter().copied().collect();
        self.cleared = world.cleared.iter().copied().collect();
        self.stocks.clear();
        for stock in &world.stocks {
            let mut wares = Vec::new();
            for ware in &stock.wares {
                match ware {
                    save::WareSave::Weapon(w) => wares.extend(self.weapon_from(w).map(Ware::Weapon)),
                    save::WareSave::Goods { item, left } => wares.extend(self.item_from(item).map(|i| Ware::Food(i, *left))),
                    save::WareSave::Unknown => {}
                }
            }
            self.stocks.insert(stock.key, wares);
        }
        self.models_version += 1;
    }

    fn set_vitals(&mut self, health: f64, hunger: f64, mana: f64) {
        self.health = health.clamp(1.0, MAX_HEALTH);
        self.hunger = hunger.clamp(0.0, 100.0);
        self.combat.mana = mana.clamp(0.0, MAX_MANA);
        self.dead = None;
        self.died = false;
        self.down = DOWN_SECONDS;
        self.since_hurt = 100.0;
        self.hurt_flash = 0.0;
        self.splats.clear();
        self.burning = 0.0;
        self.poisoned = 0.0;
        self.chilled = 0.0;
        self.snared = 0.0;
    }

    /// Animals about `near`, a merchant not far off and apples under the trees, in a world just
    /// come to, keeping what's been done there
    pub fn spawn_wildlife(&mut self, world: &World, water: WaterLevel, near: Vector3<f64>) {
        let seed = self.seed;
        self.animals.extend(animals::spawn(world, water, seed, near));
        let mut rng = hash(seed ^ 0x5e4);
        self.animals.extend(animals::spawn_serpents(world, water, &mut rng, near, worldgen::land_of(seed), animals::SERPENT_PACKS));
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

    /// For trying out saving: the player carrying `gold`
    pub fn set_gold(&mut self, gold: u32) {
        self.gold = gold;
    }

    /// How much harder the realm the player's in is, see realms::Scaling: its bosses' and
    /// monsters' health, how hard they hit and how lucky what's found is
    pub fn set_scaling(&mut self, scaling: Scaling) {
        self.scaling = scaling;
    }

    /// Whether the boss whose lair's at `key` has been beaten in the world the player's in
    pub fn is_defeated(&self, key: &[i64; 3]) -> bool {
        self.defeated.contains(key)
    }

    /// The Innert Gate or the realm's portal home, if it's in this piece of the world, standing on
    /// the ground where it is
    pub fn set_shrines(&mut self, world: &World, shrines: Vec<Shrine>) {
        self.shrines = shrines
            .into_iter()
            .map(|mut s| {
                s.position.z = ground_top(world, s.position.x, s.position.y);
                s
            })
            .collect();
    }

    /// Whether the player pressed E at the gate or the portal since last asked
    pub fn take_shrine(&mut self) -> bool {
        std::mem::take(&mut self.shrine_used)
    }

    /// The gate or portal near enough `eye`, in voxels, to use
    fn shrine_near(&self, eye: Vector3<f64>) -> Option<&Shrine> {
        let vpm = self.combat.vpm();
        self.shrines.iter().find(|s| (s.position - eye).xy().norm() < SHRINE_REACH * vpm && (eye.z - s.position.z).abs() < 8.0 * vpm)
    }

    /// How many Innert blocks of realm `realm` are carried
    pub fn innert_blocks(&self, realm: u8) -> u32 {
        self.slots.iter().filter(|(item, _)| *item == Item::InnertBlock(realm)).map(|&(_, n)| n).sum()
    }

    /// Takes as many as `most` of realm `realm`'s Innert blocks out of what's carried, to set in
    /// the gate: how many were taken
    pub fn take_innert_blocks(&mut self, realm: u8, most: u32) -> u32 {
        let Some(slot) = self.slots.iter().position(|(item, _)| *item == Item::InnertBlock(realm)) else {
            return 0;
        };
        let taken = self.slots[slot].1.min(most);
        if taken >= self.slots[slot].1 {
            self.remove_slot(slot);
        } else {
            self.slots[slot].1 -= taken;
        }
        if taken > 0 {
            self.milestone = true;
            self.cues.push(Cue::here(Sound::Chest, 1.0).pitched(0.5));
            self.cues.push(Cue::here(Sound::Fanfare, 0.6));
        }
        taken
    }

    /// Whether the grappling hook's in hand, to throw with a click
    pub fn holding_hook(&self) -> bool {
        self.dead.is_none() && self.slots.get(self.selected).is_some_and(|(item, _)| *item == Item::Hook)
    }

    /// Whether `item` is carried
    pub fn carries(&self, item: Item) -> bool {
        self.slots.iter().any(|(i, _)| *i == item) || matches!(item, Item::Wings(k) if self.worn == Some(k))
    }

    /// The kind of wings worn on the back, see wings::KINDS
    pub fn worn_wings(&self) -> Option<u8> {
        self.worn.filter(|_| self.dead.is_none())
    }

    /// Puts on the wings in slot `slot`, those worn till now going into their place
    fn wear(&mut self, slot: usize) {
        let Some(&(Item::Wings(k), _)) = self.slots.get(slot) else {
            return;
        };
        match self.worn.replace(k) {
            Some(old) => self.slots[slot] = (Item::Wings(old), 1),
            None => {
                self.slots.remove(slot);
                if self.selected > slot || self.selected >= self.slots.len() {
                    self.selected = self.selected.saturating_sub(1);
                }
            }
        }
        self.inspected = None;
        self.cues.push(Cue::here(Sound::Equip, 0.8));
        self.say(format!("You put on the {}. Hold jump in the air to glide.", wings::KINDS[k as usize].name));
    }

    /// Takes off the wings worn, into the pack
    fn take_off(&mut self) {
        if let Some(k) = self.worn.take() {
            self.slots.push((Item::Wings(k), 1));
            self.cues.push(Cue::here(Sound::Equip, 0.6));
            self.say(format!("You take off the {}.", wings::KINDS[k as usize].name));
        }
    }

    /// Standing in lava, as in the Cinder Crown: set burning, see afflict()
    pub fn scorch(&mut self) {
        self.afflict(Some(Element::Fire));
    }

    /// A little health back, as the wings that mend do while gliding
    pub fn mend(&mut self, health: f64) {
        if self.dead.is_none() {
            self.health = (self.health + health).min(MAX_HEALTH);
        }
    }

    /// The models of every kind of wings, and where the first is among model_words(), see wings.rs
    pub fn wing_models(&self) -> &[(Model, Vector3<f64>)] {
        &self.wing_models
    }

    pub fn wing_model(&self, k: usize) -> u32 {
        self.leaf_model() + (self.leaves.models.len() + k) as u32
    }

    /// Gives the player `item`, a hook or wings, if they haven't one already, telling them how it
    /// came to them: whether it was given. It goes on the quick bar's last slot, to take in hand
    /// with a number key, what was there moving along into the pack.
    pub fn find_once(&mut self, item: Item, how: &str) -> bool {
        if self.carries(item) {
            return false;
        }
        // The first wings found are put on at once, the rest kept in the pack
        if let Item::Wings(k) = item {
            let worn = self.worn.is_none();
            if worn {
                self.worn = Some(k);
            } else {
                self.slots.push((item, 1));
            }
            self.cues.push(Cue::here(Sound::Equip, 0.9));
            self.cues.push(Cue::here(Sound::Fanfare, 0.5));
            self.milestone = true;
            self.say(if worn { format!("{how} You put them on: hold jump in the air to glide.") } else { format!("{how} Wear them from the inventory, I.") });
            return true;
        }
        let at = self.slots.len().min(HOTBAR - 1);
        self.slots.insert(at, (item, 1));
        if self.selected >= at && self.selected < self.slots.len() - 1 {
            self.selected += 1;
        }
        self.cues.push(Cue::here(Sound::Equip, 0.9));
        self.cues.push(Cue::here(Sound::Fanfare, 0.5));
        self.milestone = true;
        self.say(how);
        true
    }

    /// Whether the merchants sell the grappling hook and wings, see Game::sky_wares
    pub fn set_sky_wares(&mut self, sold: bool) {
        self.sky_wares = sold;
    }

    /// Takes `item` in hand if it's carried, on the quick bar or not: whether it is now
    pub fn hold_item(&mut self, item: Item) -> bool {
        let Some(slot) = self.slots.iter().position(|(i, _)| *i == item) else {
            return false;
        };
        self.hold(slot);
        self.slots.get(self.selected).is_some_and(|(i, _)| *i == item)
    }

    /// A rope from `from` to `to`, in voxels, sagging `sag` of its length in the middle, `thick`
    /// meters thick, see Combat::rope_instances()
    pub fn rope_instances(&self, from: Vector3<f64>, to: Vector3<f64>, sag: f64, thick: f64) -> Vec<AnimalInstance> {
        self.combat.rope_instances(from, to, sag, thick)
    }

    /// The player's arm holding something that isn't a weapon, see Combat::held_arm()
    pub fn held_arm(&self, which: usize, grip: Vector3<f64>, axis: Vector3<f64>, elbow: Vector3<f64>) -> AnimalInstance {
        self.combat.held_arm(which, grip, axis, elbow)
    }

    /// Whether the map, the inventory, the Gate's window or a merchant's is open over the middle of
    /// the view, so what's in hand and the messages aren't shown under the quick bar, half hidden
    /// behind it. The messages wait meanwhile, to be read once it's closed.
    pub fn set_covered(&mut self, covered: bool) {
        self.covered = covered;
        if covered {
            let now = Instant::now();
            self.messages.iter_mut().for_each(|(_, at)| *at = now);
        }
    }

    /// For trying out the way through the realms: `count` of realm `realm`'s Innert blocks carried
    pub fn give_innert_blocks(&mut self, realm: u8, count: u32) {
        if count > 0 {
            self.add_item(Item::InnertBlock(realm), count);
        }
    }

    /// For trying it out: the realm's raid boss nearest `near`, in voxels, beaten as if by the
    /// player, so it leaves its block. Whether there was one.
    pub fn slay_raid_boss(&mut self, world: &World, near: Vector3<f64>) -> bool {
        let raid = self
            .animals
            .iter_mut()
            .filter(|a| a.health > 0.0 && a.boss.as_ref().is_some_and(|b| self.raid_lairs.contains_key(&b.key)))
            .min_by(|a, b| (a.position - near).norm().total_cmp(&(b.position - near).norm()));
        let Some(raid) = raid else {
            return false;
        };
        raid.health = 0.0;
        self.reap(world);
        true
    }

    /// Where each of the realm's raid bosses about is, in voxels, and which: for screenshot runs
    pub fn raid_bosses(&self) -> Vec<(Look, Vector3<f64>, f64)> {
        self.animals.iter().filter(|a| a.boss.as_ref().is_some_and(|b| self.raid_lairs.contains_key(&b.key))).map(|a| (a.look, a.position, a.max_health)).collect()
    }

    /// For checking a game saved comes back the same: the player's state in a line
    pub fn summary(&self) -> String {
        let weapons: Vec<String> = self.slots.iter().filter_map(|(item, _)| match item {
            Item::Weapon(w) => Some(format!("{}#{}", self.weapons[*w].name, self.weapons[*w].seed)),
            _ => None,
        }).collect();
        let others: Vec<String> = self.slots.iter().filter(|(item, _)| !matches!(item, Item::Weapon(_))).map(|&(item, n)| format!("{}x{n}", self.item_name(item))).collect();
        format!(
            "health={:.1} hunger={:.1} mana={:.1} gold={} bucket={:.2} selected={} slots={} weapons=[{}] items=[{}] opened={} defeated={} cleared={} stocks={}",
            self.health,
            self.hunger,
            self.combat.mana,
            self.gold,
            self.bucket,
            self.selected,
            self.slots.len(),
            weapons.join(", "),
            others.join(", "),
            self.opened.len(),
            self.defeated.len(),
            self.cleared.len(),
            self.stocks.len()
        )
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
        // The Innert Gate or a portal home, seen from afar, its veil of light once it's open and
        // the blocks set in it up close
        for shrine in &self.shrines {
            // Its foundation down in the ground, see animals::GATE_SINK
            let (at, facing) = (shrine.position - Vector3::new(0.0, 0.0, animals::GATE_SINK * vpm), shrine.facing);
            out.push(animals::still_instance(if shrine.gate { Look::Teleporter } else { Look::RealmPortal }, at, facing, world.voxels_per_meter));
            if shrine.open {
                out.push(animals::still_instance(Look::PortalVeil, at, facing, world.voxels_per_meter));
            }
            if shrine.gate && (at - camera).norm() < 60.0 * vpm {
                let (c, s) = (facing.cos(), facing.sin());
                for (realm, &placed) in shrine.placed.iter().enumerate() {
                    for slot in 0..(placed as usize).min(realms::RAIDS) {
                        let [x, y, z] = animals::socket(realm, slot);
                        let block = at + Vector3::new(x * c - y * s, x * s + y * c, z) * vpm;
                        out.push(animals::still_instance(Look::InnertBlock, block, facing, world.voxels_per_meter));
                    }
                }
            }
        }
        // Animals come before anything else, bosses most of all, so however much is flying
        // about they're always drawn
        let mut creatures: Vec<(f64, AnimalInstance)> = Vec::new();
        let mut pieces = Vec::new();
        for animal in &self.animals {
            let d = (animal.position - camera).norm();
            // A great serpent's head, jaw and every piece of its long body, see serpents::Coils,
            // before any but other bosses, the nearest of them first, so however many are about
            // the one being fought is drawn whole
            if let Some(coils) = animal.coils() {
                pieces.clear();
                coils.instances(vpm, animal.status.tint(), &mut pieces);
                creatures.extend(pieces.drain(..).map(|i| ((i.sphere_center() - camera).norm() * 0.05, i)));
            } else if d < ANIMAL_RANGE * vpm || animal.look.is_boss() {
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
                // A few arrows or bolts lying side by side, or a little pile of shot
                (Item::Ammo(k, _), None) => {
                    let def = &self.library.ammo[k];
                    near.extend(self.combat.bundle_instances(def.shot(), def.head.unwrap_or(0), def.element, at, pickup.heading).into_iter().map(|i| (d, i)));
                }
                _ => {}
            }
        }
        // The Coil Unending's omens, from further off, as they're there to be seen
        for &(look, position, heading) in &self.omens {
            let d = (position - camera).norm();
            if d < 90.0 * vpm {
                near.push((d, animals::still_instance(look, position, heading, world.voxels_per_meter)));
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
        let mut lights = self.combat.lights(self.held().map(|(_, w)| w), &self.animals, view);
        // The veil of the gate or a portal glows, breathing slowly
        let vpm = view.vpm;
        let breath = 1.0 + 0.15 * (self.combat.time() * 1.3).sin();
        for shrine in self.shrines.iter().filter(|s| s.open) {
            let p = shrine.position + Vector3::new(0.0, 0.0, 2.8 * vpm);
            let strength = (1.5 * breath) as f32;
            lights.push([p.x as f32, p.y as f32, p.z as f32, (14.0 * vpm) as f32, 0.72 * strength, 0.5 * strength, 1.0 * strength, 1.0]);
        }
        lights
    }

    /// What to do with what the crosshair is on. What's lying about has a card of its own instead.
    fn target_hint(&self) -> Option<String> {
        let Some(target) = self.target else {
            let shrine = self.feet.and_then(|feet| self.shrine_near(feet))?;
            return Some(if shrine.gate { "E  the Innert Gate" } else { "E  the portal home" }.into());
        };
        match target {
            Target::Pickup(_) => None,
            Target::Chest(i) => Some(if self.chests[i].hoard { "E  open the bandits' hoard" } else { "E  open the chest" }.into()),
            Target::Animal(i) => {
                let look = self.animals[i].look;
                Some(if look == Look::Merchant {
                    "E  trade with the merchant".to_owned()
                } else if look == Look::Princess {
                    "E  trade with the princess".to_owned()
                } else if look == Look::RoyalGuard {
                    "E  talk to the guard".to_owned()
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

        // The sky darkening over the Coil Unending's lair, a bruised gloom closing in from above
        if self.gloom > 0.01 {
            let g = self.gloom as f32;
            let mut mesh = egui::Mesh::default();
            let bands = [(0.0, 0.62), (0.32, 0.38), (0.62, 0.0)];
            for pair in bands.windows(2) {
                let (top, bottom) = (screen.top() + pair[0].0 * screen.height(), screen.top() + pair[1].0 * screen.height());
                let first = mesh.vertices.len() as u32;
                let shade = |a: f32| Color32::from_rgba_unmultiplied(18, 10, 28, (a * g * 255.0) as u8);
                mesh.colored_vertex(pos2(screen.left(), top), shade(pair[0].1));
                mesh.colored_vertex(pos2(screen.right(), top), shade(pair[0].1));
                mesh.colored_vertex(pos2(screen.right(), bottom), shade(pair[1].1));
                mesh.colored_vertex(pos2(screen.left(), bottom), shade(pair[1].1));
                mesh.add_triangle(first, first + 1, first + 2);
                mesh.add_triangle(first, first + 2, first + 3);
            }
            painter.add(egui::Shape::mesh(mesh));
        }

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
        for npc in self.animals.iter().filter(|a| a.look == Look::Merchant || a.look.is_villager() || a.look.is_royal()) {
            let (name, color, height, range, size) = if npc.look == Look::Merchant {
                ("Merchant", GOLD, 2.15, 28.0, 15.0)
            } else if npc.look == Look::Princess {
                ("Princess", Color32::from_rgb(255, 170, 215), 2.05, 28.0, 15.0)
            } else if npc.look == Look::RoyalGuard {
                ("Royal Guard", Color32::from_rgb(200, 210, 230), 2.15, 14.0, 13.0)
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
        // The gold the player lost when they died shines as bright as an epic weapon, and the hoard
        // of a camp cleared, up from its lid, as a legendary one
        let gold = [GOLD.r(), GOLD.g(), GOLD.b()];
        let lying = self.pickups.iter().filter(|p| p.seen && p.flying <= 0.0 && p.shines()).map(|pickup| match pickup.item {
            Item::Weapon(w) => (pickup.position, self.weapons[w].grade, self.library.rarities[self.weapons[w].grade].color, w),
            _ => (pickup.position, 3, gold, 0),
        });
        let hoards = self.beams.iter().enumerate().map(|(k, &at)| (at + Vector3::new(0.0, 0.0, 0.25 * vpm), 4, gold, 7 + k));
        for (position, grade, [r, g, b], w) in lying.chain(hoards) {
            let base = position + Vector3::new(0.0, 0.0, 0.2 * vpm);
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
        // How far the bow is drawn, or a crossbow or gun loaded, as a ring closing round the
        // crosshair, and what a crossbow or gun has loaded as pips under it
        if let Some((index, weapon)) = self.held()
            && weapon.kind() == Kind::Ranged
        {
            let ring = |fraction: f32, tint: Color32| {
                let points: Vec<Pos2> = (0..=32).map(|k| k as f32 / 32.0 * fraction * std::f32::consts::TAU).map(|a| center + vec2(a.sin(), -a.cos()) * 14.0).collect();
                painter.add(egui::Shape::line(points, Stroke::new(2.5f32, tint)));
            };
            let draw = self.combat.draw(weapon) as f32;
            if draw > 0.0 {
                ring(draw, if draw >= 1.0 { Color32::from_rgb(255, 220, 120) } else { Color32::WHITE });
            }
            if weapon.profile.build != WeaponBuild::None {
                if let Some(done) = self.combat.reloading() {
                    ring(done as f32, Color32::from_rgb(255, 180, 90));
                }
                let (left, full) = self.combat.rounds(index, weapon);
                for k in 0..full {
                    let at = center + vec2((k as f32 - (full as f32 - 1.0) * 0.5) * 7.0, 22.0);
                    if k < left {
                        painter.circle_filled(at, 2.5, Color32::from_rgb(255, 225, 150));
                    } else {
                        painter.circle_stroke(at, 2.5, Stroke::new(1.0f32, faint));
                    }
                }
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
            if let Some(n) = item.shown(count) {
                painter.text(rect.right_bottom() - vec2(5.0, 2.0), Align2::RIGHT_BOTTOM, n.to_string(), FontId::proportional(13.0), Color32::WHITE);
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
            // The Coil Unending's bar is the biggest of all, across most of the view, in the color
            // of the element its body's taken on, its stages marked across it
            let ultra = foe.is_ultra();
            let name = if ultra {
                "ULTRA  ·  YSVETH, THE COIL UNENDING".to_owned()
            } else if foe.is_raid() {
                format!("RAID  ·  {}", foe.name())
            } else {
                foe.name().to_string()
            };
            // A raid boss's bar is longer
            let width = (screen.width() * if ultra { 0.78 } else if foe.is_raid() { 0.6 } else { 0.42 }).clamp(260.0, if ultra { 1400.0 } else { 900.0 });
            let tall = if ultra { 18.0 } else { 12.0 };
            // Jolting on a big hit, flashing white, the health it's just lost showing pale behind
            let (_, _, trail, _, flash) = self.boss_bar;
            let flash = flash as f32;
            let jolt = (self.combat.time() * 90.0).sin() as f32 * 4.0 * flash;
            let back = Rect::from_center_size(pos2(center.x + jolt, screen.bottom() - if ultra { 58.0 } else { 54.0 }), vec2(width, tall));
            let title = if enraged { format!("{name}  —  enraged") } else { name.to_string() };
            let at = pos2(center.x, back.top() - 6.0);
            let size = if ultra { 23.0 } else { 19.0 };
            painter.text(at + vec2(1.0, 1.0), Align2::CENTER_BOTTOM, &title, FontId::proportional(size), shadow);
            painter.text(at, Align2::CENTER_BOTTOM, &title, FontId::proportional(size), if enraged { Color32::from_rgb(255, 170, 120) } else { Color32::WHITE });
            painter.rect_filled(back.expand(2.0), 6.0, Color32::from_black_alpha(150));
            if flash > 0.0 {
                painter.rect_stroke(back.expand(2.0 + 3.0 * flash), 6.0, Stroke::new(2.0f32, Color32::from_white_alpha((flash * 200.0) as u8)), StrokeKind::Outside);
            }
            painter.rect_filled(Rect::from_min_size(back.min, vec2(width * trail.max(vitality) as f32, back.height())), 5.0, Color32::from_rgb(255, 225, 150));
            let red = |c: u8| (c as f32 + (255.0 - c as f32) * 0.8 * flash) as u8;
            let element = self.animals.iter().find(|a| a.look.is_ultra() && a.health > 0.0).and_then(serpents::element_now).filter(|_| ultra);
            let [r, g, b] = element.map_or([190, 40, 35], |e| e.color().map(|c| (c as f32 * 0.85) as u8));
            painter.rect_filled(Rect::from_min_size(back.min, vec2(width * vitality as f32, back.height())), 5.0, Color32::from_rgb(red(r), red(g), red(b)));
            // Where a raid boss goes into its next phases, marked across the bar
            let marks: &[f64] = if ultra { &serpents::ULTRA_STAGES } else if foe.is_raid() { &[0.66, 0.33] } else { &[] };
            for &mark in marks {
                let x = back.left() + width * mark as f32;
                painter.line_segment([pos2(x, back.top() - 3.0), pos2(x, back.bottom() + 3.0)], Stroke::new(2.0f32, Color32::from_rgb(255, 215, 120)));
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
        // None of it, nor the messages, behind the map or another window, see set_covered()
        if let Some((_, weapon)) = self.held().filter(|_| !self.covered) {
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
            // What it's loaded with, how many are left, and how to switch
            if let Some(shot) = weapon.shot() {
                let others = self.slots.iter().any(|&(item, _)| matches!(item, Item::Ammo(k, n) if n > 0 && self.library.ammo[k].shot().ammo() == shot.ammo()));
                let (text, tint) = match self.chosen_ammo() {
                    Some(k) => {
                        let def = &self.library.ammo[k];
                        (format!("{}  ×{}", def.display_name, self.ammo_count(k)), def.element.map_or(Color32::from_rgb(235, 225, 200), |e| color(e.color())))
                    }
                    None => (format!("Plain {}  ∞", shot.plural()), faint),
                };
                let text = if others { format!("{text}   ·   T") } else { text };
                painter.text(pos2(center.x, below) + vec2(1.0, 1.0), Align2::CENTER_TOP, &text, FontId::proportional(14.0), shadow);
                painter.text(pos2(center.x, below), Align2::CENTER_TOP, &text, FontId::proportional(14.0), tint);
                below += 20.0;
            }
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

        // Messages under that, newest first, fading out, and with no weapon in hand still under
        // what's in the pack and the gold, either side under the panel
        let below = below.max(panel.bottom() + 26.0);
        for (k, (message, at)) in self.messages.iter().rev().enumerate().filter(|_| !self.covered) {
            let alpha = (1.0 - (at.elapsed().as_secs_f32() - 3.0).max(0.0)).clamp(0.0, 1.0);
            let pos = pos2(center.x, below + 4.0 + k as f32 * 24.0);
            painter.text(pos + vec2(1.0, 1.0), Align2::CENTER_TOP, message, FontId::proportional(17.0), Color32::from_black_alpha((150.0 * alpha) as u8));
            painter.text(pos, Align2::CENTER_TOP, message, FontId::proportional(17.0), Color32::from_white_alpha((255.0 * alpha) as u8));
        }
    }

    /// The inventory: everything carried as a grid of icons, the quick bar along the top, and the
    /// card of what's clicked on, with what can be done with it
    pub fn inventory(&mut self, ctx: &egui::Context, open: &mut bool, crafting: &mut tools::Crafting) {
        let mut chosen = None;
        let mut deed = None;
        let mut made = None;
        egui::Window::new("Inventory")
            .open(open)
            .collapsible(false)
            .resizable(false)
            // Just under the quick bar
            .anchor(Align2::CENTER_TOP, [0.0, 100.0])
            .show(ctx, |ui| {
                // What's carried, or making tools from what's been gathered, see tools.rs
                if tools::tabs(ui, crafting) {
                    made = tools::craft_tab(ui, crafting, &|tool| self.best_tool(tool));
                    return;
                }
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
                // What's worn on the back
                ui.horizontal(|ui| {
                    ui.label(RichText::new("On your back").weak());
                    let (rect, response) = ui.allocate_exact_size(vec2(40.0, 40.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 7.0, Color32::from_white_alpha(16));
                    match self.worn {
                        Some(k) => {
                            let kind = &wings::KINDS[k as usize];
                            let tint = color(self.library.rarities[kind.rarity.min(self.library.rarities.len() - 1)].color);
                            ui.painter().rect_stroke(rect, 7.0, Stroke::new(1.5f32, tint), StrokeKind::Inside);
                            self.draw_icon(ui.painter(), Item::Wings(k), rect.center(), 24.0);
                            response.on_hover_ui(|ui| {
                                ui.set_width(280.0);
                                self.item_card(ui, Item::Wings(k), 1);
                            });
                            ui.label(RichText::new(kind.name).strong().color(tint));
                            if ui.button("Take off").clicked() {
                                deed = Some(Deal::TakeOff);
                            }
                        }
                        None => {
                            ui.label(RichText::new("Nothing. Wings found are worn here.").weak().italics());
                        }
                    }
                });
                ui.separator();
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Quick bar, keys 1 to 9, then the pack").weak().small());
                        egui::ScrollArea::vertical().max_height(ctx.screen_rect().height() * 0.6).show(ui, |ui| match self.slot_grid(ui, "carried", false) {
                            Some(SlotClick::Once(slot)) => chosen = Some(slot),
                            Some(SlotClick::Twice(slot)) => {
                                chosen = Some(slot);
                                deed = Some(match self.slots[slot].0 {
                                    Item::Wings(_) => Deal::Wear(slot),
                                    item if item.consumable() => Deal::Eat(slot),
                                    _ => Deal::Hold(slot),
                                });
                            }
                            Some(SlotClick::Moved(a, b)) => deed = Some(Deal::Move(a, b)),
                            None => {}
                        });
                    });
                    ui.add_space(14.0);
                    ui.vertical(|ui| {
                        ui.set_width(310.0);
                        let Some((slot, (item, count))) = self.inspected.and_then(|i| Some((i, *self.slots.get(i)?))) else {
                            ui.label(RichText::new("Click on something to see all about it. Drag it onto the quick bar, here or along the top, to have it to hand, or right click to hold it or eat it.").weak());
                            return;
                        };
                        self.item_card(ui, item, count);
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if let Item::Wings(_) = item {
                                if ui.button(if self.worn.is_some() { "Wear instead" } else { "Wear" }).on_hover_text("Put them on your back, to glide on with jump held in the air").clicked() {
                                    deed = Some(Deal::Wear(slot));
                                }
                            } else if let Item::Ammo(..) = item {
                                if ui.button("Load").on_hover_text("Shoot these from now on. T switches ammo too.").clicked() {
                                    deed = Some(Deal::Hold(slot));
                                }
                            } else if !matches!(item, Item::Gold(_)) && ui.add_enabled(slot != self.selected, egui::Button::new("Hold")).clicked() {
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
                ui.label(RichText::new("Melee: click to swing, three in a row for a combo, right click for a heavy blow. Bow: hold right click to draw, click to shoot. Crossbow or gun: click to shoot, it reloads once empty, right click reloads early. T switches ammo. Staff, scepter or wand: click to cast. Orb: hold for a beam. Tome: click to conjure. Totem: click to hex. Bell: hold, let go to toll.").weak().small());
                ui.label(RichText::new("R or the middle button: a legendary or mythic part's special attack, once charged. Bosses drop loot, merchants buy and sell it.").weak().small());
            });
        // The quick bar along the top takes what's dragged onto it too
        if let Some(SlotClick::Moved(a, b)) = self.quick_bar_targets(ctx) {
            deed = Some(Deal::Move(a, b));
        }
        if let Some(slot) = chosen {
            self.inspected = Some(slot);
        }
        match deed {
            Some(Deal::Hold(slot)) => self.hold(slot),
            Some(Deal::Eat(slot)) => self.eat(slot),
            Some(Deal::Drop(slot)) => self.drop_slot(slot),
            Some(Deal::Move(a, b)) => self.move_slot(a, b),
            Some(Deal::Sort) => self.sort(),
            Some(Deal::Wear(slot)) => self.wear(slot),
            Some(Deal::TakeOff) => self.take_off(),
            _ => {}
        }
        // A tool made, paid for with what's been gathered
        if let Some((tool, grade)) = made
            && let Some(weapon) = tools::make(&self.library, tool, grade)
            && crafting.materials.take(tools::cost(tool, grade))
        {
            self.say(format!("Crafted a {}. Hold it and click at what it's for to gather it.", weapon.name));
            self.give_weapon(weapon);
            self.cues.push(Cue::here(Sound::Place, 0.8).pitched(1.2));
        }
    }

    /// What's carried as a grid of icons, a row of the quick bar first: what was done with them.
    /// Each can be dragged onto another to swap them, a number key while over one puts it in
    /// that slot of the quick bar, and a right click is a double click. When trading, `selling`
    /// shows what the merchant pays for each on hover.
    fn slot_grid(&self, ui: &mut egui::Ui, id: &str, selling: bool) -> Option<SlotClick> {
        let mut clicked = None;
        let cell = 48.0;
        let dragging = egui::DragAndDrop::has_any_payload(ui.ctx());
        egui::Grid::new(id).spacing([5.0, 5.0]).show(ui, |ui| {
            for (i, &(item, count)) in self.slots.iter().enumerate() {
                let (rect, response) = ui.allocate_exact_size(vec2(cell, cell), egui::Sense::click_and_drag());
                if let Some(action) = self.slot_dnd(ui, &response, rect, i) {
                    clicked = Some(action);
                }
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
                if let Some(n) = item.shown(count) {
                    painter.text(rect.right_bottom() - vec2(4.0, 2.0), Align2::RIGHT_BOTTOM, n.to_string(), FontId::proportional(12.0), Color32::WHITE);
                }
                if i < HOTBAR {
                    painter.text(rect.left_top() + vec2(4.0, 2.0), Align2::LEFT_TOP, (i + 1).to_string(), FontId::proportional(10.0), Color32::from_white_alpha(140));
                }
                // No card while something's being dragged about, it'd only get in the way
                let response = if dragging {
                    response
                } else {
                    response.on_hover_ui(|ui| {
                        ui.set_width(280.0);
                        self.item_card(ui, item, count);
                        if selling && item != Item::Bucket {
                            ui.label(RichText::new(format!("Click to sell one for {} gold", self.sell_price(item))).strong().color(GOLD));
                        } else {
                            ui.label(RichText::new("Drag it to another slot, or press 1 to 9 for the quick bar. Right click to hold it or eat it.").weak().small());
                        }
                    })
                };
                if response.double_clicked() || response.secondary_clicked() {
                    clicked = Some(SlotClick::Twice(i));
                } else if response.clicked() {
                    clicked = Some(SlotClick::Once(i));
                }
                if (i + 1) % HOTBAR == 0 {
                    ui.end_row();
                }
            }
        });
        clicked
    }

    /// Dragging slot `i` about, at `rect` with `response`: picked up, carried over others with the
    /// one under it marked, and dropped on another to swap them. And over it, a number key puts
    /// it in that slot of the quick bar.
    fn slot_dnd(&self, ui: &egui::Ui, response: &egui::Response, rect: Rect, i: usize) -> Option<SlotClick> {
        if i >= self.slots.len() {
            return None;
        }
        if response.drag_started() {
            response.dnd_set_drag_payload(i);
        }
        // What's being carried follows the pointer, a little see-through
        if response.dragged()
            && let Some(at) = ui.ctx().pointer_interact_pos()
        {
            let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("dragged item")));
            painter.rect_filled(Rect::from_center_size(at, vec2(44.0, 44.0)), 7.0, Color32::from_black_alpha(140));
            self.draw_icon(&painter, self.slots[i].0, at, 26.0);
        }
        if response.dnd_hover_payload::<usize>().is_some_and(|from| *from != i) {
            ui.painter().rect_stroke(rect.expand(2.0), 8.0, Stroke::new(2.5f32, GOLD), StrokeKind::Outside);
        }
        if let Some(from) = response.dnd_release_payload::<usize>()
            && *from != i
        {
            return Some(SlotClick::Moved(*from, i));
        }
        if response.hovered() {
            let keys = [egui::Key::Num1, egui::Key::Num2, egui::Key::Num3, egui::Key::Num4, egui::Key::Num5, egui::Key::Num6, egui::Key::Num7, egui::Key::Num8, egui::Key::Num9];
            if let Some(k) = keys.iter().position(|&key| ui.input(|input| input.key_pressed(key))).filter(|&k| k != i) {
                return Some(SlotClick::Moved(i, k));
            }
        }
        None
    }

    /// Where the slots of the quick bar are drawn along the top of `screen`, see hud()
    fn quick_bar_rects(&self, screen: Rect) -> Vec<Rect> {
        let (slot, gap, pad) = (44.0, 6.0, 10.0);
        let shown = self.slots.len().min(HOTBAR);
        let slots_width = if self.slots.is_empty() { 120.0 } else { shown as f32 * (slot + gap) - gap };
        let width = pad + slots_width + 20.0 + 170.0 + pad;
        let left = screen.center().x - width / 2.0 + pad;
        (0..shown).map(|i| Rect::from_min_size(pos2(left + i as f32 * (slot + gap), screen.top() + 14.0 + pad), vec2(slot, slot))).collect()
    }

    /// While the inventory's open, the quick bar along the top takes what's dragged onto it, and
    /// what's on it can be dragged off. Returns what was moved where.
    fn quick_bar_targets(&self, ctx: &egui::Context) -> Option<SlotClick> {
        let mut moved = None;
        egui::Area::new(egui::Id::new("quick bar targets")).fixed_pos(ctx.screen_rect().min).order(egui::Order::Foreground).show(ctx, |ui| {
            for (i, rect) in self.quick_bar_rects(ctx.screen_rect()).into_iter().enumerate() {
                let response = ui.interact(rect, egui::Id::new(("quick bar slot", i)), egui::Sense::click_and_drag());
                if let Some(action) = self.slot_dnd(ui, &response, rect, i) {
                    moved = Some(action);
                }
                if response.hovered() && !egui::DragAndDrop::has_any_payload(ctx) {
                    ui.painter().rect_stroke(rect, 8.0, Stroke::new(1.5f32, Color32::from_white_alpha(120)), StrokeKind::Inside);
                }
            }
        });
        moved
    }

    /// Swaps slots `a` and `b`. What's in hand stays in hand if it's still on the quick bar, and
    /// anything put in the slot in hand is taken in hand.
    fn move_slot(&mut self, a: usize, b: usize) {
        if a >= self.slots.len() || b >= self.slots.len() || a == b {
            return;
        }
        self.slots.swap(a, b);
        if self.selected == a && b < HOTBAR {
            self.selected = b;
        }
        if self.inspected == Some(a) {
            self.inspected = Some(b);
        } else if self.inspected == Some(b) {
            self.inspected = Some(a);
        }
        self.cues.push(Cue::here(Sound::Equip, 0.5));
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
            Item::Ammo(k, n) => {
                let def = &self.library.ammo[k];
                let tint = def.element.map_or(Color32::WHITE, |e| color(e.color()));
                ui.label(RichText::new(&def.display_name).size(19.0).strong().color(tint));
                let what = match def.shot() {
                    Shot::Arrow => "Arrows, for bows",
                    Shot::Quarrel => "Bolts, for crossbows",
                    _ => "Shot, for guns",
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(what).weak());
                    if let Some(e) = def.element {
                        ui.label(RichText::new(format!("·  {}", e.name())).color(color(e.color())));
                    }
                });
                ui.separator();
                for &d in &def.effect_defs {
                    let effect = &self.library.effects[d];
                    ui.label(RichText::new(format!("• {}: {}", effect.display_name, effect.description)).color(Color32::from_rgb(200, 220, 255)));
                }
                let (adds, times) = def.stats();
                for (key, label, unit) in STATS {
                    if let Some(&v) = adds.get(key) {
                        let label = if key == "elementalDamage" { format!("{} damage", def.element.map_or("Elemental", Element::name)) } else { label.to_owned() };
                        ui.label(RichText::new(format!("+{} {}", unit.show(v), label.to_lowercase())).color(good));
                    }
                    if let Some(&v) = times.get(key) {
                        let shown = if v <= 0.0 { "one".to_owned() } else { format!("×{v:.2}") };
                        ui.label(RichText::new(format!("{shown} {}", label.to_lowercase())).color(if v >= 1.0 { good } else { Color32::from_rgb(240, 180, 120) }));
                    }
                }
                ui.label(RichText::new(&def.description).weak().italics());
                ui.separator();
                ui.label(RichText::new(format!("{n} carried, used up a shot at a time. Plain {} never run out.", def.shot().plural())).weak());
                ui.label(RichText::new(format!("Worth {} gold", self.price(item))).color(GOLD));
                return;
            }
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
            Item::InnertBlock(realm) => {
                let r = &realms::REALMS[realm as usize % realms::REALMS.len()];
                let [cr, cg, cb] = r.color;
                ui.label(RichText::new(format!("Innert Block of {}", r.name.trim_start_matches("The "))).size(19.0).strong().color(Color32::from_rgb(cr, cg, cb)));
                ui.label(RichText::new("Left by one of the realm's raid bosses").weak());
                ui.separator();
                ui.label(format!("{count} of {} carried", realms::RAIDS));
                ui.label(RichText::new("Set all three into the Innert Gate in the normal world to open the way to the next realm.").weak());
                ui.label(RichText::new("Never lost, sold or dropped.").weak().italics());
                return;
            }
            Item::Wings(k) => {
                let kind = &wings::KINDS[k as usize];
                let rarity = &self.library.rarities[kind.rarity.min(self.library.rarities.len() - 1)];
                ui.label(RichText::new(kind.name).size(19.0).strong().color(color(rarity.color)));
                ui.label(RichText::new(format!("{} wings, worn on the back", rarity.display_name)).weak());
                ui.separator();
                ui.label(RichText::new(format!("Glides at {:.0} m/s, sinking {:.1} m/s at most", kind.speed, kind.sink)).color(good));
                ui.label(format!("Turns {}", if kind.turn >= 5.5 { "quickly" } else if kind.turn >= 3.5 { "readily" } else { "slowly, steady" }));
                if let Some((lift, wait)) = kind.flap {
                    ui.label(RichText::new(format!("Jump in the air to flap: lifts {lift:.0} m/s, every {wait:.1} s")).color(good));
                }
                ui.label(RichText::new(kind.about).italics());
                ui.separator();
                ui.label(RichText::new("Hold jump in the air to glide. Kept for good once found.").weak());
                ui.label(RichText::new(format!("Worth {} gold", self.price(item))).color(GOLD));
                return;
            }
            Item::Hook => {
                ui.label(RichText::new(item.name()).size(19.0).strong().color(Color32::WHITE));
                ui.label(RichText::new("For getting about the sky isles").weak());
                ui.separator();
                ui.label(if item == Item::Hook {
                    format!("With it in hand, click to throw it up to {:.0} m. Where it bites into rock or earth, its rope hauls you there, up over the edge if it's an edge. Jump to let go.", realms::HOOK_REACH_M)
                } else {
                    "Hold jump in the air to spread them and glide, sinking slowly, going on ahead the way you look.".to_owned()
                });
                ui.label(RichText::new("Kept for good once found.").weak().italics());
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
        // Compared with what's in hand if it's the same kind, and shoots the same kind of thing
        let other = self.held().filter(|&(i, held)| i != w && held.kind() == weapon.kind() && held.shot() == weapon.shot()).map(|(_, held)| held);
        let (good, bad) = (Color32::from_rgb(120, 230, 110), Color32::from_rgb(240, 95, 85));
        // How much better or worse it is than the one in hand, nothing if about the same
        let against = |ui: &mut egui::Ui, by: f64, unit: Unit, higher: bool| {
            let least = match unit {
                Unit::Number | Unit::Speed | Unit::Degrees => 0.5,
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
        // How well it came out of the forge, and what the rarity and weight of its parts make of
        // its blows, so two of the same can be told apart
        let percent = |by: f64| format!("{:+.0}%", (by - 1.0) * 100.0);
        ui.horizontal(|ui| {
            let tint = match profile.make() {
                "Flawed" => Color32::from_rgb(230, 140, 110),
                "Fine" => good,
                "Masterwork" => GOLD,
                _ => Color32::from_gray(170),
            };
            ui.label(RichText::new(format!("{} {}", profile.make(), percent(profile.forged))).color(tint));
            ui.label(RichText::new(format!("·  parts {} damage, {} speed", percent(profile.quality), percent(profile.pace))).weak());
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
                // A single shot needs no saying
                if key == "magazine" && value <= 1.0 && other.is_none_or(|o| o.profile.stat(key) <= 1.0) {
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
        // How a magic weapon of an unusual kind is used
        let how = match profile.style {
            Style::Channel => Some("Hold the button to pour a beam out at the crosshair; its mana cost goes each second."),
            Style::Summon => Some("Click to conjure a familiar that hangs in the air casting at whatever's near, up to three at once."),
            Style::Hex => Some("Click to cast a hex that drifts after what's near and eats away at it, slowing it; it passes on when it dies."),
            Style::Toll => Some("Hold to raise the bell, longer for a harder toll; let go to ring out a shockwave all round."),
            Style::Glaive => Some("Swung like a polearm; its finishing blow and heavy blows loose a spell too, for mana."),
            Style::Whirl => Some("Whirled right round, hitting everything about you. Right click brings it down overhead, a shockwave rolling out from where it lands."),
            Style::Quake => Some("Every blow comes down on the ground and sends a shockwave rolling out; every second one, and heavy blows, hit harder and further."),
            Style::Lob => Some("Click to lob a bomb onto the crosshair; it bounces and bursts a moment after it lands. Hold right click to see where it'll come down."),
            Style::Arc => Some("Click to loose lightning at whatever's nearest the crosshair; it leaps on through everything near it."),
            Style::Censer => Some("Hold to swing the censer: each swing billows out a cloud that settles on the crosshair and hangs there, hurting what's in it."),
            _ => None,
        };
        if let Some(how) = how {
            ui.add_space(4.0);
            ui.label(RichText::new(how).weak().small());
        }
        if let Some(shot) = weapon.shot() {
            ui.add_space(4.0);
            let how = match profile.build {
                WeaponBuild::None => "Hold right click to draw, click to shoot.",
                WeaponBuild::Repeater => "Click or hold to shoot, its lever working between shots. Empty, it's reloaded by itself; right click reloads early.",
                WeaponBuild::Hand => "Held in the off hand. Click to shoot; its lever spans it again quickly.",
                WeaponBuild::Twin => "Click to loose both prods at once, two bolts meeting at the crosshair.",
                WeaponBuild::Siege => "Click or hold right click to brace it on its leg first, which all but stops you; braced, it looses a great bolt that pins.",
                WeaponBuild::Harpoon => "Click to loose a harpoon on a line: it reels in what it catches, or hauls you to what's too big, or to where it bites. Right click lets go of the line.",
                WeaponBuild::Fan => "Click to loose five bolts at once in a fan, wider the more its spread.",
                _ => "Click to shoot. Empty, it's reloaded by itself, which takes a while; right click reloads early.",
            };
            ui.label(RichText::new(format!("{how} Plain {} never run out; T loads special ones you carry.", shot.plural())).weak().small());
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
        egui::Window::new(if self.royals.contains(&key) { "The Royal Armoury" } else { "Merchant" })
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
                                        _ => RichText::new(format!("{}  ×{left}", self.item_name(item))),
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
                        match self.slot_grid(ui, "selling", true) {
                            Some(SlotClick::Once(slot) | SlotClick::Twice(slot)) => deed = Some(Deal::Sell(slot)),
                            Some(SlotClick::Moved(a, b)) => deed = Some(Deal::Move(a, b)),
                            None => {}
                        }
                    });
                });
            });
        match deed {
            Some(Deal::Buy(k)) => self.buy(k),
            Some(Deal::Sell(slot)) => self.sell(slot),
            Some(Deal::Move(a, b)) => self.move_slot(a, b),
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
            // A sheaf of arrows or bolts, or a pouch of shot, tipped with their element
            Item::Ammo(k, _) => {
                let def = &self.library.ammo[k];
                let tip = def.element.map_or(Color32::from_rgb(210, 214, 222), |e| color(e.color()));
                match def.shot() {
                    Shot::Ball | Shot::Shell => {
                        painter.circle_filled(center + vec2(0.0, r * 0.2), r * 0.62, Color32::from_rgb(130, 85, 50));
                        painter.line_segment([center + vec2(-r * 0.3, -r * 0.35), center + vec2(r * 0.3, -r * 0.35)], Stroke::new(r * 0.12, Color32::from_rgb(90, 60, 35)));
                        for (dx, dy) in [(-0.25, -0.55), (0.1, -0.62), (0.3, -0.45)] {
                            painter.circle_filled(center + vec2(r * dx, r * dy), r * 0.17, tip);
                        }
                    }
                    shot => {
                        let short = if shot == Shot::Quarrel { 0.65 } else { 0.9 };
                        for dx in [-0.3, 0.0, 0.3] {
                            let (from, to) = (center + vec2(r * dx - r * 0.5 * short, r * 0.8 * short), center + vec2(r * dx + r * 0.5 * short, -r * 0.8 * short));
                            painter.line_segment([from, to], Stroke::new(r * 0.08, Color32::from_rgb(150, 100, 60)));
                            painter.circle_filled(to, r * 0.13, tip);
                            painter.line_segment([from, from.lerp(to, 0.22)], Stroke::new(r * 0.16, Color32::from_rgb(235, 230, 220)));
                        }
                    }
                }
            }
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
            // A glowing block seen corner on, in its realm's color, bound in gold
            Item::InnertBlock(realm) => {
                let [cr, cg, cb] = realms::REALMS[realm as usize % realms::REALMS.len()].color;
                let glow = Color32::from_rgb(cr, cg, cb);
                let (top, left, right) = (center - vec2(0.0, r * 0.75), center - vec2(r * 0.68, r * 0.38), center + vec2(r * 0.68, -r * 0.38));
                let (mid, low_left, low_right, bottom) = (center, center + vec2(-r * 0.68, r * 0.42), center + vec2(r * 0.68, r * 0.42), center + vec2(0.0, r * 0.8));
                painter.circle_filled(center, r * 0.95, glow.gamma_multiply(0.18));
                painter.add(egui::Shape::convex_polygon(vec![top, right, mid, left], glow, Stroke::NONE));
                painter.add(egui::Shape::convex_polygon(vec![left, mid, bottom, low_left], glow.gamma_multiply(0.75), Stroke::NONE));
                painter.add(egui::Shape::convex_polygon(vec![mid, right, low_right, bottom], glow.gamma_multiply(0.55), Stroke::NONE));
                for edge in [[top, right], [right, low_right], [low_right, bottom], [bottom, low_left], [low_left, left], [left, top], [left, mid], [mid, right], [mid, bottom]] {
                    painter.line_segment(edge, Stroke::new(r * 0.07, GOLD));
                }
            }
            // An iron shank with three prongs curling up from its head, and its coil of rope
            Item::Hook => {
                let iron = Color32::from_rgb(170, 176, 186);
                let rope = Color32::from_rgb(150, 110, 70);
                painter.circle_stroke(center + vec2(-r * 0.3, r * 0.35), r * 0.42, Stroke::new(r * 0.13, rope));
                painter.line_segment([center + vec2(0.0, r * 0.75), center + vec2(0.0, -r * 0.45)], Stroke::new(r * 0.14, iron));
                for dx in [-0.45, 0.45] {
                    painter.line_segment([center + vec2(0.0, -r * 0.45), center + vec2(r * dx, -r * 0.8)], Stroke::new(r * 0.12, iron));
                    painter.line_segment([center + vec2(r * dx, -r * 0.8), center + vec2(r * dx * 1.2, -r * 0.5)], Stroke::new(r * 0.1, iron));
                }
                painter.line_segment([center + vec2(0.0, -r * 0.45), center + vec2(0.0, -r * 0.95)], Stroke::new(r * 0.12, iron));
            }
            // A pair of white wings spread wide
            Item::Wings(k) => {
                let ([vr, vg, vb], [tr, tg, tb]) = wings::icon_colors(k);
                let (vane, tip_color) = (Color32::from_rgb(vr, vg, vb), Color32::from_rgb(tr, tg, tb));
                for side in [-1.0f32, 1.0] {
                    let tip = center + vec2(side * r * 0.95, -r * 0.6);
                    painter.add(egui::Shape::convex_polygon(vec![center + vec2(side * r * 0.1, -r * 0.1), tip, center + vec2(side * r * 0.75, r * 0.15), center + vec2(side * r * 0.2, r * 0.35)], vane, Stroke::new(r * 0.05, tip_color)));
                    painter.circle_filled(tip, r * 0.1, tip_color);
                }
                painter.circle_filled(center, r * 0.14, tip_color);
            }
            Item::Gold(_) => {
                for (dx, dy) in [(-0.35, 0.35), (0.35, 0.3), (0.0, -0.05)] {
                    let at = center + vec2(r * dx, r * dy);
                    painter.circle_filled(at, r * 0.5, Color32::from_rgb(200, 150, 30));
                    painter.circle_filled(at - vec2(0.0, r * 0.06), r * 0.44, GOLD);
                    painter.circle_stroke(at - vec2(0.0, r * 0.06), r * 0.28, Stroke::new(r * 0.07, Color32::from_rgb(220, 165, 40)));
                }
            }
            Item::Meat | Item::RoastMeat => {
                let bone = Color32::from_rgb(240, 232, 214);
                painter.line_segment([center + vec2(-r * 0.85, r * 0.55), center + vec2(-r * 0.1, -r * 0.05)], Stroke::new(r * 0.22, bone));
                painter.circle_filled(center + vec2(-r * 0.95, r * 0.42), r * 0.16, bone);
                painter.circle_filled(center + vec2(-r * 0.75, r * 0.72), r * 0.16, bone);
                // Roasted, browned, with a darker crust
                let (flesh, shine) = if item == Item::RoastMeat { (Color32::from_rgb(120, 62, 30), Color32::from_rgb(170, 100, 50)) } else { (Color32::from_rgb(175, 60, 52), Color32::from_rgb(215, 110, 100)) };
                painter.circle_filled(center + vec2(r * 0.2, -r * 0.25), r * 0.62, flesh);
                painter.circle_filled(center + vec2(r * 0.05, -r * 0.4), r * 0.22, shine);
            }
            Item::Weapon(w) => {
                let weapon = &self.weapons[w];
                let glow = weapon.profile.element.map_or(Color32::from_rgb(150, 220, 255), |e| color(e.color()));
                let metal = Color32::from_rgb(210, 214, 222);
                let wood = Color32::from_rgb(150, 100, 60);
                let haft = Color32::from_rgb(130, 88, 52);
                let family = self.library.families[weapon.family].id.as_str();
                match weapon.kind() {
                    // A short haft, a chain off it and a spiked ball on the end
                    Kind::Melee if family == "flail" => {
                        let (foot, top) = (center + vec2(-r * 0.85, r * 0.85), center + vec2(-r * 0.2, r * 0.2));
                        painter.line_segment([foot, top], Stroke::new(r * 0.16, haft));
                        let head = center + vec2(r * 0.45, -r * 0.45);
                        for k in 1..6 {
                            painter.circle_stroke(top.lerp(head, k as f32 / 6.0), r * 0.07, Stroke::new(r * 0.05, Color32::from_rgb(120, 125, 135)));
                        }
                        for k in 0..8 {
                            let a = k as f32 / 8.0 * std::f32::consts::TAU;
                            painter.line_segment([head, head + vec2(a.cos(), a.sin()) * r * 0.45], Stroke::new(r * 0.08, metal));
                        }
                        painter.circle_filled(head, r * 0.3, if weapon.profile.element.is_some() { glow } else { Color32::from_rgb(90, 95, 105) });
                    }
                    // A long haft and a great block of a head across its top
                    Kind::Melee if family == "maul" => {
                        let (foot, top) = (center + vec2(-r * 0.85, r * 0.85), center + vec2(r * 0.35, -r * 0.35));
                        painter.line_segment([foot, top], Stroke::new(r * 0.14, haft));
                        let along = vec2(r * 0.55, r * 0.55);
                        painter.line_segment([top - along, top + along], Stroke::new(r * 0.55, Color32::from_rgb(95, 100, 110)));
                        painter.line_segment([top - along * 0.6, top + along * 0.6], Stroke::new(r * 0.12, if weapon.profile.element.is_some() { glow } else { metal }));
                    }
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
                    // A round bomb, a band round it, its fuse curling up and burning
                    Kind::Ranged if family == "bombs" => {
                        let body = center + vec2(-r * 0.1, r * 0.2);
                        painter.circle_filled(body, r * 0.62, Color32::from_rgb(60, 64, 72));
                        painter.line_segment([body + vec2(-r * 0.6, 0.0), body + vec2(r * 0.6, 0.0)], Stroke::new(r * 0.1, Color32::from_rgb(200, 150, 70)));
                        painter.circle_filled(body + vec2(-r * 0.22, -r * 0.25), r * 0.12, glow);
                        let fuse = [body + vec2(0.0, -r * 0.6), body + vec2(r * 0.25, -r * 0.85), body + vec2(r * 0.5, -r * 0.95)];
                        painter.add(egui::Shape::line(fuse.to_vec(), Stroke::new(r * 0.08, Color32::from_rgb(220, 205, 170))));
                        painter.circle_filled(fuse[2], r * 0.14, Color32::from_rgb(255, 190, 60));
                    }
                    // A crossbow seen from above, its prod across the front, a bolt in its groove
                    Kind::Ranged if weapon.profile.build.crossbow() => {
                        let build = weapon.profile.build;
                        // A hand crossbow shorter, on a grip; a twin-prod's second prod over the
                        // first; a bolt-thrower's fan of bolts; a harpoon's coil of line
                        let short = if build == WeaponBuild::Hand { 0.45 } else { 0.75 };
                        let (butt, front) = (center + vec2(-r * short, r * short), center + vec2(r * 0.45, -r * 0.45));
                        match build {
                            WeaponBuild::Hand => {
                                painter.line_segment([center + vec2(-r * 0.25, r * 0.25), center + vec2(-r * 0.55, r * 0.05)], Stroke::new(r * 0.16, wood));
                            }
                            WeaponBuild::Twin => {
                                let lift = vec2(-r * 0.12, -r * 0.12);
                                let prod: Vec<Pos2> = (0..=12).map(|k| front + lift + vec2(r * 0.62 * (k as f32 / 6.0 - 1.0), -r * 0.62 * (k as f32 / 6.0 - 1.0)) + vec2(-r * 0.22, r * 0.22) * (k as f32 / 6.0 - 1.0).powi(2)).collect();
                                painter.add(egui::Shape::line(prod, Stroke::new(r * 0.1, haft)));
                            }
                            WeaponBuild::Fan => {
                                for k in -2..=2 {
                                    let a = k as f32 * 0.2;
                                    painter.line_segment([center, center + vec2((0.7 + a).cos(), -(0.7 + a).sin()) * r * 0.85], Stroke::new(r * 0.05, glow));
                                }
                            }
                            WeaponBuild::Harpoon => {
                                painter.circle_stroke(center + vec2(r * 0.05, r * 0.25), r * 0.17, Stroke::new(r * 0.07, Color32::from_rgb(215, 200, 160)));
                            }
                            WeaponBuild::Siege => {
                                painter.line_segment([front + vec2(-r * 0.1, r * 0.1), front + vec2(-r * 0.25, r * 0.6)], Stroke::new(r * 0.08, metal));
                            }
                            _ => {}
                        }
                        let across = vec2(r * 0.62, r * 0.62);
                        let bend = vec2(-r * 0.22, r * 0.22);
                        let prod: Vec<Pos2> = (0..=12).map(|k| {
                            let s = k as f32 / 6.0 - 1.0;
                            front + vec2(across.x * s, -across.y * s) * 1.0 + bend * s * s
                        }).collect();
                        let (a, b) = (prod[0], prod[12]);
                        painter.line_segment([a, front + bend * 0.15 - vec2(r * 0.25, -r * 0.25)], Stroke::new(r * 0.04, Color32::from_rgb(235, 230, 210)));
                        painter.line_segment([b, front + bend * 0.15 - vec2(r * 0.25, -r * 0.25)], Stroke::new(r * 0.04, Color32::from_rgb(235, 230, 210)));
                        painter.line_segment([butt, front + vec2(r * 0.1, -r * 0.1)], Stroke::new(r * 0.2, wood));
                        painter.add(egui::Shape::line(prod, Stroke::new(r * if build == WeaponBuild::Siege { 0.2 } else { 0.13 }, if matches!(build, WeaponBuild::Crank | WeaponBuild::Siege) { metal } else { haft })));
                        painter.line_segment([center + vec2(-r * 0.1, r * 0.1), front + vec2(r * 0.2, -r * 0.2)], Stroke::new(r * 0.06, glow));
                        if weapon.profile.build == WeaponBuild::Repeater {
                            painter.rect_filled(Rect::from_center_size(center + vec2(-r * 0.05, r * 0.05), vec2(r * 0.4, r * 0.4)), r * 0.05, haft);
                        }
                    }
                    // A gun: a stock or grip and a barrel, flaring for a blunderbuss, thick for a cannon
                    Kind::Ranged if weapon.profile.build != WeaponBuild::None => {
                        let build = weapon.profile.build;
                        let barrel = Color32::from_rgb(120, 125, 135);
                        let (breech, muzzle) = (center + vec2(-r * 0.25, r * 0.15), center + vec2(r * 0.9, -r * 0.45));
                        let thick = if build == WeaponBuild::Cannon { 0.34 } else { 0.15 };
                        match build {
                            WeaponBuild::Pistol => {
                                painter.line_segment([breech, center + vec2(-r * 0.7, r * 0.85)], Stroke::new(r * 0.28, wood));
                                painter.circle_filled(center + vec2(-r * 0.72, r * 0.9), r * 0.17, Color32::from_rgb(200, 160, 70));
                            }
                            WeaponBuild::Cannon => {
                                painter.line_segment([breech, center + vec2(-r * 0.95, r * 0.65)], Stroke::new(r * 0.16, haft));
                            }
                            _ => {
                                painter.add(egui::Shape::convex_polygon(
                                    vec![breech + vec2(0.0, -r * 0.08), breech + vec2(r * 0.05, r * 0.12), center + vec2(-r * 0.95, r * 0.95), center + vec2(-r * 0.98, r * 0.55)],
                                    wood,
                                    Stroke::NONE,
                                ));
                                painter.line_segment([breech, breech.lerp(muzzle, 0.75) + vec2(0.0, r * 0.08)], Stroke::new(r * 0.18, wood));
                            }
                        }
                        painter.line_segment([breech, muzzle], Stroke::new(r * thick, barrel));
                        if build == WeaponBuild::Blunderbuss {
                            painter.circle_filled(muzzle, r * 0.17, barrel);
                        }
                        painter.circle_filled(breech + vec2(-r * 0.05, -r * 0.12), r * 0.08, glow);
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
                    // An orb in a ring over a little cradle
                    Kind::Magic if family == "orb" => {
                        painter.line_segment([center + vec2(0.0, r * 0.95), center + vec2(0.0, r * 0.45)], Stroke::new(r * 0.2, wood));
                        painter.circle_filled(center - vec2(0.0, r * 0.15), r * 0.55, glow);
                        painter.circle_filled(center - vec2(r * 0.18, r * 0.33), r * 0.15, Color32::from_white_alpha(170));
                        painter.circle_stroke(center - vec2(0.0, r * 0.15), r * 0.66, Stroke::new(r * 0.08, Color32::from_rgb(200, 170, 80)));
                    }
                    // A shut book, a gem on its cover, gilt at its corners
                    Kind::Magic if family == "tome" => {
                        let book = Rect::from_center_size(center, vec2(r * 1.3, r * 1.7));
                        painter.rect_filled(book, r * 0.08, Color32::from_rgb(110, 60, 40));
                        painter.rect_filled(Rect::from_min_size(book.min, vec2(r * 0.18, book.height())), 0.0, Color32::from_rgb(80, 42, 28));
                        painter.rect_filled(Rect::from_min_size(pos2(book.right() - r * 0.08, book.top() + r * 0.1), vec2(r * 0.12, book.height() - r * 0.2)), 0.0, Color32::from_rgb(240, 228, 200));
                        for corner in [book.right_top(), book.right_bottom()] {
                            painter.circle_filled(corner, r * 0.14, Color32::from_rgb(220, 180, 80));
                        }
                        painter.circle_filled(center + vec2(r * 0.08, 0.0), r * 0.22, glow);
                    }
                    // A carved pole of faces, eyes glowing, horns on top
                    Kind::Magic if family == "totem" => {
                        painter.line_segment([center + vec2(0.0, r * 0.95), center - vec2(0.0, r * 0.6)], Stroke::new(r * 0.36, wood));
                        for (k, y) in [-0.4f32, 0.05, 0.5].into_iter().enumerate() {
                            painter.rect_filled(Rect::from_center_size(center + vec2(0.0, r * y), vec2(r * 0.5, r * 0.32)), r * 0.08, if k % 2 == 0 { wood } else { Color32::from_rgb(170, 60, 40) });
                            for dx in [-0.1, 0.1] {
                                painter.circle_filled(center + vec2(r * dx, r * (y - 0.04)), r * 0.05, glow);
                            }
                        }
                        for s in [-1.0, 1.0] {
                            painter.line_segment([center - vec2(0.0, r * 0.65), center + vec2(s * r * 0.45, -r * 0.95)], Stroke::new(r * 0.1, Color32::from_rgb(235, 228, 210)));
                        }
                    }
                    // A handbell, its clapper glowing under it
                    Kind::Magic if family == "bell" => {
                        painter.line_segment([center - vec2(0.0, r * 0.95), center - vec2(0.0, r * 0.45)], Stroke::new(r * 0.18, wood));
                        let bell = Color32::from_rgb(205, 150, 70);
                        painter.add(egui::Shape::convex_polygon(
                            vec![center + vec2(-r * 0.25, -r * 0.45), center + vec2(r * 0.25, -r * 0.45), center + vec2(r * 0.7, r * 0.5), center + vec2(-r * 0.7, r * 0.5)],
                            bell,
                            Stroke::NONE,
                        ));
                        painter.circle_filled(center - vec2(0.0, r * 0.45), r * 0.27, bell);
                        painter.circle_filled(center + vec2(0.0, r * 0.68), r * 0.17, glow);
                    }
                    // A rod forking at its top, lightning between the prongs
                    Kind::Magic if family == "rod" => {
                        let (foot, top) = (center + vec2(-r * 0.75, r * 0.85), center + vec2(r * 0.25, -r * 0.35));
                        painter.line_segment([foot, top], Stroke::new(r * 0.13, wood));
                        let bronze = Color32::from_rgb(200, 140, 70);
                        let (a, b) = (top + vec2(-r * 0.05, -r * 0.6), top + vec2(r * 0.55, -r * 0.05));
                        painter.line_segment([top, a], Stroke::new(r * 0.1, bronze));
                        painter.line_segment([top, b], Stroke::new(r * 0.1, bronze));
                        let zig = [a, a.lerp(b, 0.3) + vec2(r * 0.12, r * 0.05), a.lerp(b, 0.6) - vec2(r * 0.05, r * 0.12), b];
                        painter.add(egui::Shape::line(zig.to_vec(), Stroke::new(r * 0.07, glow)));
                    }
                    // A censer hanging on its chains from a little handle, smoke rising off it
                    Kind::Magic if family == "censer" => {
                        let bronze = Color32::from_rgb(200, 150, 70);
                        let handle = center + vec2(0.0, -r * 0.85);
                        painter.line_segment([handle + vec2(0.0, -r * 0.1), handle + vec2(0.0, r * 0.25)], Stroke::new(r * 0.14, wood));
                        let bowl = center + vec2(0.0, r * 0.45);
                        for dx in [-0.3, 0.0, 0.3] {
                            painter.line_segment([handle + vec2(0.0, r * 0.25), bowl + vec2(r * dx, -r * 0.2)], Stroke::new(r * 0.04, Color32::from_rgb(150, 150, 160)));
                        }
                        painter.circle_filled(bowl, r * 0.4, bronze);
                        for dx in [-0.18, 0.0, 0.18] {
                            painter.circle_filled(bowl + vec2(r * dx, r * 0.05), r * 0.07, glow);
                        }
                        painter.circle_filled(bowl + vec2(r * 0.55, -r * 0.4), r * 0.16, Color32::from_white_alpha(120));
                        painter.circle_filled(bowl + vec2(r * 0.7, -r * 0.7), r * 0.12, Color32::from_white_alpha(80));
                    }
                    // A staff with a curved blade on its head
                    Kind::Magic if family == "battlestaff" => {
                        let (foot, top) = (center + vec2(-r * 0.8, r * 0.8), center + vec2(r * 0.35, -r * 0.35));
                        painter.line_segment([foot, top], Stroke::new(r * 0.14, wood));
                        painter.circle_filled(top, r * 0.18, glow);
                        painter.add(egui::Shape::convex_polygon(vec![top + vec2(r * 0.05, -r * 0.1), top + vec2(r * 0.6, -r * 0.65), top + vec2(r * 0.3, -r * 0.1)], metal, Stroke::NONE));
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A princess's armoury always has its full stock of weapons, legendary and mythic ones only,
    /// no two of a family
    #[test]
    fn royal_armouries_sell_only_legendary_weapons() {
        let library = Library::load();
        let legendary = library.rarities.iter().position(|r| r.id == "legendary").expect("there's a legendary rarity");
        for seed in 0..40u32 {
            let weapons = royal_weapons(&library, hash(seed));
            assert_eq!(weapons.len(), ROYAL_WEAPONS);
            assert!(weapons.iter().all(|w| w.grade >= legendary));
            for (i, a) in weapons.iter().enumerate() {
                assert!(weapons[i + 1..].iter().all(|b| b.family != a.family));
            }
        }
    }

    /// How rare what each source of weapons gives is, rolled as each rolls it many times over: how
    /// many of its weapons are epic, legendary and mythic, how many have a special attack, and how
    /// many special weapons there are to each kill, chest, boss, stall and kit, in the normal world
    /// and each realm. To look at: cargo test --release loot_odds -- --ignored --nocapture
    #[test]
    #[ignore]
    fn loot_odds() {
        let library = Library::load();
        let at = |id: &str| library.rarities.iter().position(|r| r.id == id).unwrap();
        let (epic, legendary, mythic) = (at("epic"), at("legendary"), at("mythic"));
        // What a lot of weapons from one source come to, a lot at a time: a monster's drop, a
        // chest, a boss's hoard, a stall, a kit. Each weapon is its grade and whether it has a
        // special attack.
        #[derive(Default)]
        struct Tally {
            weapons: usize,
            grades: [usize; 6],
            specials: usize,
            lots: usize,
            lots_with: usize,
        }
        impl Tally {
            fn add(&mut self, lot: &[(usize, bool)]) {
                for &(grade, special) in lot {
                    self.weapons += 1;
                    self.grades[grade.min(5)] += 1;
                    self.specials += special as usize;
                }
                self.lots += 1;
                self.lots_with += lot.iter().any(|w| w.1) as usize;
            }
            fn share(&self, n: usize) -> f64 {
                n as f64 / self.weapons.max(1) as f64
            }
            /// Of its weapons, how many have a special attack
            fn special(&self) -> f64 {
                self.share(self.specials)
            }
            /// Special weapons to each lot
            fn per_lot(&self) -> f64 {
                self.specials as f64 / self.lots.max(1) as f64
            }
        }
        let sample = |lots: usize, seed: u32, make: &mut dyn FnMut(&mut u32) -> Vec<(usize, bool)>| {
            let mut tally = Tally::default();
            let mut rng = hash(seed ^ 0x1007_0dd5);
            for _ in 0..lots {
                tally.add(&make(&mut rng));
            }
            tally
        };
        // A weapon as Game::roll_weapon() rolls it, `luck` with a realm's scaling
        let rolled = |rng: &mut u32, luck: f64, sure: bool| {
            *rng = hash(*rng ^ 0x1007);
            let seed = if sure { library.special_seed(*rng, None, luck) } else { *rng };
            let (_, grade, special) = library.preview(seed, None, &library.odds(luck));
            (grade, special.is_some())
        };
        // As Game::open_chest() rolls what's in one with `luck`, its seed where it is
        let chest = |rng: &mut u32, luck: f64| {
            *rng = hash(*rng);
            let (_, grade, special) = library.preview(*rng, None, &library.odds(luck));
            vec![(grade, special.is_some())]
        };
        let pct = |x: f64| format!("{:.2}%", 100.0 * x);
        let one_in = |x: f64| if x > 0.0 { format!("{:.1}", 1.0 / x) } else { "never".into() };
        let row = |name: &str, luck: &str, t: &Tally| {
            println!(
                "| {name} | {luck} | {} | {} | {} | {} | {} | **{}** | 1 in {} | {:.3} | {} |",
                t.weapons / t.lots.max(1),
                pct(t.share(t.grades[epic])),
                pct(t.share(t.grades[legendary])),
                pct(t.share(t.grades[mythic])),
                pct(t.share(t.grades[epic..].iter().sum())),
                pct(t.special()),
                one_in(t.special()),
                t.per_lot(),
                pct(t.lots_with as f64 / t.lots.max(1) as f64),
            );
        };
        // Chests about the land, the luck worldgen gives them: on top, in the ruins and such, see
        // worldgen.rs; down in the dungeons and the fallen cities; the crystal chests of caves
        // behind waterfalls and the hoards of a fallen castle's court and a fallen city's lord;
        // and the hoards of bandits' camps, as worldgen\camps.rs lucks them by their band
        let plain: [(&str, f64); 8] = [("pyramid", 2.0), ("pyramid top", 1.4), ("shrine", 1.2), ("castle", 1.3), ("flooded church", 1.6), ("desert ruins", 1.2), ("colonnade", 1.1), ("watchtower", 1.3)];
        let deep: [(&str, f64); 3] = [("dungeon side hall", 1.7), ("dungeon boss hall", 2.6), ("fallen city house", 2.2)];
        let lucky: [(&str, f64); 3] = [("crystal cave", 4.0), ("fallen castle court", 4.5), ("fallen city lord", 5.0)];
        let hoards: [(&str, f64); 3] = [("small band", 3.75), ("usual band", 4.5), ("band with a warlord", 5.5)];
        let mean = |tallies: &[Tally], f: &dyn Fn(&Tally) -> f64| tallies.iter().map(f).sum::<f64>() / tallies.len() as f64;
        let header = || {
            println!("| source | luck | weapons each | epic | legendary | mythic | epic or better | special | special weapons | specials each | with a special |");
            println!("|---|---|---|---|---|---|---|---|---|---|---|");
        };
        println!("\nWhat each source of weapons gives, in the normal world:\n");
        header();
        let monster = sample(400_000, 1, &mut |rng| vec![rolled(rng, MONSTER_LUCK, false)]);
        row("monster drop", &format!("{MONSTER_LUCK}"), &monster);
        let mut plain_tallies = Vec::new();
        for (k, (name, luck)) in plain.iter().enumerate() {
            let t = sample(60_000, 10 + k as u32, &mut |rng| chest(rng, *luck));
            row(&format!("chest: {name}"), &format!("{luck}"), &t);
            plain_tallies.push(t);
        }
        let mut deep_tallies = Vec::new();
        for (k, (name, luck)) in deep.iter().enumerate() {
            let t = sample(60_000, 20 + k as u32, &mut |rng| chest(rng, *luck));
            row(&format!("deep chest: {name}"), &format!("{luck}"), &t);
            deep_tallies.push(t);
        }
        let mut lucky_tallies = Vec::new();
        for (k, (name, luck)) in lucky.iter().enumerate() {
            let t = sample(60_000, 30 + k as u32, &mut |rng| chest(rng, *luck));
            row(&format!("lucky chest: {name}"), &format!("{luck}"), &t);
            lucky_tallies.push(t);
        }
        let mut hoard_tallies = Vec::new();
        for (k, (name, luck)) in hoards.iter().enumerate() {
            let t = sample(60_000, 40 + k as u32, &mut |rng| chest(rng, *luck));
            row(&format!("bandits' hoard: {name}"), &format!("{luck}"), &t);
            hoard_tallies.push(t);
        }
        let hoard = |of: Hoard, scale: f64, lots: usize, seed: u32| sample(lots, seed, &mut |rng| hoard_rolls(of, rng).into_iter().map(|(luck, sure)| rolled(rng, luck * scale, sure)).collect());
        let lucks = |l: &[f64]| l.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ");
        let boss = hoard(Hoard::Boss, 1.0, 40_000, 50);
        row("lesser boss", &lucks(&BOSS_LUCKS), &boss);
        let raid = hoard(Hoard::Raid, 1.0, 20_000, 51);
        row("realm raid boss", &lucks(&RAID_LUCKS), &raid);
        let huge = hoard(Hoard::Huge, 1.0, 20_000, 52);
        row("huge raid boss, finale", &lucks(&RAID_LUCKS), &huge);
        let merchant = |scale: f64, lots: usize| {
            sample(lots, 60, &mut |rng| {
                MERCHANT_LUCKS
                    .iter()
                    .map(|luck| {
                        *rng = hash(*rng);
                        let (_, grade, special) = library.preview(*rng, None, &library.odds(luck * scale));
                        (grade, special.is_some())
                    })
                    .collect()
            })
        };
        let stall = merchant(1.0, 40_000);
        row("merchant's stall", &lucks(&MERCHANT_LUCKS), &stall);
        let mut royal = Tally::default();
        for seed in 0..300u32 {
            royal.add(&royal_weapons(&library, hash(seed ^ 0x9e11)).iter().map(|w| (w.grade, w.profile.special.is_some())).collect::<Vec<_>>());
        }
        row("royal armoury", "its own", &royal);
        let mut start = Tally::default();
        for seed in 0..4000u32 {
            start.add(&kit(&library, hash(seed), KIT_LUCK, Some(KIT_RAREST)).iter().map(|(family, seed, odds)| library.preview(*seed, Some(*family), odds)).map(|(_, g, s)| (g, s.is_some())).collect::<Vec<_>>());
        }
        row("new player's kit", &format!("{KIT_LUCK}, none rarer than {}", library.rarities[KIT_RAREST].id), &start);
        let worst = (0..4000u32).map(|seed| kit(&library, hash(seed), KIT_LUCK, Some(KIT_RAREST)).iter().filter(|(f, s, o)| library.preview(*s, Some(*f), o).2.is_some()).count()).max().unwrap_or(0);

        println!("\nSpecial weapons to expect:\n");
        println!("- per 100 of the dead or bandits killed: {:.2} ({} of them drop a weapon)", 100.0 * MONSTER_WEAPON * monster.special(), pct(MONSTER_WEAPON));
        println!("- per 10 chests on top: {:.2}; in dungeons and fallen cities: {:.2}; lucky ones: {:.2}; bandits' hoards: {:.2}", 10.0 * mean(&plain_tallies, &Tally::special), 10.0 * mean(&deep_tallies, &Tally::special), 10.0 * mean(&lucky_tallies, &Tally::special), 10.0 * mean(&hoard_tallies, &Tally::special));
        println!("- per lesser boss: {:.2}; per realm raid boss: {:.2}; per huge raid boss or finale: {:.2}", boss.per_lot(), raid.per_lot(), huge.per_lot());
        println!("- per merchant: {:.2}; per royal armoury: {:.2}", stall.per_lot(), royal.per_lot());
        println!("- in a new player's kit of {}: {:.2}, at most {worst}", KIT.len(), start.per_lot());

        println!("\nHow many have a special attack in each realm, its loot luckier by its scaling:\n");
        println!("| realm | luck | monster drop | chest on top | deep chest | lucky chest | bandits' hoard | lesser boss, specials each | realm raid boss | huge raid boss | merchant's stalls with one |");
        println!("|---|---|---|---|---|---|---|---|---|---|---|");
        for (r, realm) in realms::REALMS.iter().enumerate() {
            let scale = loot_scaling(realm.scaling);
            let seed = 100 + 10 * r as u32;
            let monster = sample(100_000, seed, &mut |rng| vec![rolled(rng, MONSTER_LUCK * scale, false)]);
            let chests = |set: &[(&str, f64)]| set.iter().enumerate().map(|(k, (_, luck))| sample(20_000, seed + k as u32, &mut |rng| chest(rng, luck * scale))).collect::<Vec<_>>();
            let (p, d, l, h) = (chests(&plain), chests(&deep), chests(&lucky), chests(&hoards));
            let stall = merchant(scale, 10_000);
            println!(
                "| {} | {scale:.2} | 1 in {} | 1 in {} | 1 in {} | 1 in {} | 1 in {} | {:.2} | {:.2} | {:.2} | {} |",
                realm.name,
                one_in(monster.special()),
                one_in(mean(&p, &Tally::special)),
                one_in(mean(&d, &Tally::special)),
                one_in(mean(&l, &Tally::special)),
                one_in(mean(&h, &Tally::special)),
                hoard(Hoard::Boss, scale, 10_000, seed).per_lot(),
                hoard(Hoard::Raid, scale, 5_000, seed).per_lot(),
                hoard(Hoard::Huge, scale, 5_000, seed).per_lot(),
                pct(stall.lots_with as f64 / stall.lots as f64),
            );
        }
    }

    /// The Coil Unending slain stays slain: its lair's beaten in the save, so it never rises again
    /// once the world's built afresh round it, and its fang, the one of its kind, keeps its name
    /// through the save
    #[test]
    fn the_coil_unending_stays_beaten() {
        let size = [64u32, 64, 64];
        let mut occupancy = vec![0u128; 16 * 16 * 8];
        occupancy[..16 * 16 * 2].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, DIRT);
        world.voxels_per_meter = 4;
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let mut game = Game::new(&world, &dry, 5, Vector3::new(32.0, 32.0, 16.0));
        let ground = Vector3::new(32.0, 32.0, 16.0);
        let lair = |key: [i64; 3]| Lair { look: Look::Ysveth, position: ground, key, raid: None };
        let key = [1234, -5678, serpents::ULTRA_KEY];
        game.set_lairs(&world, &dry, vec![lair(key)]);
        assert!(game.animals.iter().any(|a| a.look.is_ultra()), "it's there to fight");
        // Slain, it writhes, then falls
        for a in game.animals.iter_mut().filter(|a| a.look.is_ultra()) {
            a.health = 0.0;
        }
        game.reap(&world);
        assert!(game.animals.iter().any(|a| a.look.is_ultra()), "it's still going down");
        for a in game.animals.iter_mut().filter(|a| a.look.is_ultra()) {
            a.boss.as_mut().unwrap().dying = Some(serpents::GREAT_THROES);
        }
        game.reap(&world);
        assert!(!game.animals.iter().any(|a| a.look.is_ultra()));
        assert!(game.is_defeated(&key));
        let fang = game.pickups.iter().find_map(|p| if let Item::Weapon(w) = p.item { (game.weapons[w].name == serpents::FANG).then_some(w) } else { None }).expect("its fang among its hoard");
        // A whip of the rarest grade, with a serpent's special
        let weapon = &game.weapons[fang];
        assert_eq!(weapon.grade, game.library.rarities.len() - 1, "its fang's mythic");
        assert_eq!(game.library.families[weapon.family].id, "whip");
        assert!(matches!(game.library.special_of(weapon.seed, Some(weapon.family), FANG_LUCK), Some(Special::SerpentLash | Special::CoilBind)), "with a serpent's special");
        game.add_item(Item::Weapon(fang), 1);
        // Through the file's own form and back, into another game
        let carried: save::CarriedSave = serde_json::from_str(&serde_json::to_string(&game.carried_save()).unwrap()).unwrap();
        let done: save::WorldSave = serde_json::from_str(&serde_json::to_string(&game.world_save()).unwrap()).unwrap();
        let mut other = Game::new(&world, &dry, 9, ground);
        other.restore(&carried, &done, game.vitals());
        assert!(other.is_defeated(&key));
        assert!(other.weapons.iter().any(|w| w.name == serpents::FANG), "its fang keeps its name");
        // The world built afresh round its lair: it doesn't rise again, though another would
        other.animals.retain(|a| !a.look.is_great_serpent());
        other.set_lairs(&world, &dry, vec![lair(key)]);
        assert!(!other.animals.iter().any(|a| a.look.is_ultra()));
        other.set_lairs(&world, &dry, vec![lair([1, 2, serpents::ULTRA_KEY])]);
        assert!(other.animals.iter().any(|a| a.look.is_ultra()));
    }

    /// What's carried and what's been done, saved and restored into another game, is all there
    /// again as it was, weapons made of the same parts with the same names
    #[test]
    fn a_saved_player_comes_back_the_same() {
        let world = World::from_packed_occupancy([8; 3], &[0; 4], AIR);
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let mut game = Game::new(&world, &dry, 5, Vector3::new(4.0, 4.0, 4.0));
        game.arm(77, 4.0);
        game.add_item(Item::Bread, 3);
        game.add_item(Item::Potion, 1);
        game.add_item(Item::Gold(5), 1);
        game.gold = 812;
        game.bucket = 1.25;
        game.selected = 3;
        game.health = 42.0;
        game.hunger = 13.5;
        game.combat.mana = 20.0;
        game.opened.insert([1, 2, 3]);
        game.defeated.insert([-4, 5, 6]);
        game.cleared.insert([7, -8, 9]);
        game.stock([10, 11, 12]);
        // A ware bought and one sold back onto the stall
        game.shop = Some([10, 11, 12]);
        game.buy(5);
        game.sell(0);
        game.shop = None;
        let carried = game.carried_save();
        let done = game.world_save();
        let summary = game.summary();
        // Through the file's own form and back
        let carried: save::CarriedSave = serde_json::from_str(&serde_json::to_string(&carried).unwrap()).unwrap();
        let done: save::WorldSave = serde_json::from_str(&serde_json::to_string(&done).unwrap()).unwrap();
        let mut other = Game::new(&world, &dry, 9, Vector3::new(4.0, 4.0, 4.0));
        other.restore(&carried, &done, game.vitals());
        assert_eq!(other.summary(), summary);
        assert_eq!(other.carried_save(), game.carried_save());
        assert_eq!(other.world_save(), game.world_save());
        let names = |g: &Game| g.slots.iter().filter_map(|(i, _)| if let Item::Weapon(w) = i { Some(g.weapons[*w].name.clone()) } else { None }).collect::<Vec<_>>();
        assert_eq!(names(&other), names(&game));
        // A weapon whose parts are no more is left out, the rest kept
        let mut damaged = carried.clone();
        if let save::ItemSave::Weapon(w) = &mut damaged.slots[0].item {
            w.parts[0] = "a part long gone".into();
        }
        other.restore(&damaged, &done, game.vitals());
        assert_eq!(other.slots.len(), game.slots.len() - 1);
        // Starting afresh leaves nothing of it
        other.new_character(9);
        assert_eq!(other.gold, STARTING_GOLD);
        assert!(other.defeated.is_empty() && other.opened.is_empty() && other.stocks.is_empty());
        assert_eq!(other.health, MAX_HEALTH);
    }

    /// A saved weapon's made again of the parts it was saved with, never rolled again, so however
    /// the odds of what rolls change, a save from before keeps every weapon it has just as it was,
    /// those carried and those on the stalls looked at
    #[test]
    fn saved_weapons_stay_the_same_whatever_the_odds() {
        let world = World::from_packed_occupancy([8; 3], &[0; 4], AIR);
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let mut game = Game::new(&world, &dry, 5, Vector3::new(4.0, 4.0, 4.0));
        game.arm(31, 60.0);
        game.stock([10, 11, 12]);
        let (carried, done) = (game.carried_save(), game.world_save());
        let mut other = Game::new(&world, &dry, 9, Vector3::new(4.0, 4.0, 4.0));
        // Odds nothing like them: every part mythic
        let top = other.library.rarities.len() - 1;
        for (k, rarity) in other.library.rarities.iter_mut().enumerate() {
            rarity.chance = if k == top { 1.0 } else { 0.0 };
        }
        other.restore(&carried, &done, game.vitals());
        assert_eq!(other.carried_save(), carried);
        assert_eq!(other.world_save(), done);
        let made = |g: &Game| {
            let wares = g.stocks.values().flatten().filter_map(|w| if let Ware::Weapon(i) = w { Some(*i) } else { None });
            let carried = g.slots.iter().filter_map(|(i, _)| if let Item::Weapon(w) = i { Some(*w) } else { None });
            carried.chain(wares).map(|w| &g.weapons[w]).map(|w| (w.name.clone(), w.grade, w.parts.clone(), format!("{:?}", w.profile.stats), w.profile.special.as_ref().map(|s| s.0))).collect::<Vec<_>>()
        };
        assert_eq!(made(&other), made(&game));
        assert!(made(&game).len() >= KIT.len() + MERCHANT_LUCKS.len());
    }

    /// What a new player starts with is plain: one of each kind of weapon in KIT, common to rare
    /// and nothing with a special attack, however many kits are rolled
    #[test]
    fn new_players_start_with_plain_gear() {
        let library = Library::load();
        for seed in 0..300u32 {
            let kit = kit(&library, hash(seed), KIT_LUCK, Some(KIT_RAREST));
            assert_eq!(kit.len(), KIT.len());
            for (family, seed, odds) in &kit {
                let (f, grade, special) = library.preview(*seed, Some(*family), odds);
                assert!(f == *family && grade <= KIT_RAREST && special.is_none(), "{} from {seed}", library.families[f].id);
            }
        }
        // As the game arms them
        let world = World::from_packed_occupancy([8; 3], &[0; 4], AIR);
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let game = Game::new(&world, &dry, 12, Vector3::new(4.0, 4.0, 4.0));
        let weapons: Vec<&Weapon> = game.slots.iter().filter_map(|(i, _)| if let Item::Weapon(w) = i { Some(&game.weapons[*w]) } else { None }).collect();
        assert_eq!(weapons.len(), KIT.len());
        assert!(weapons.iter().all(|w| w.grade <= KIT_RAREST && w.profile.special.is_none() && w.parts.iter().all(|&p| library.components[p].tier <= KIT_RAREST)));
    }

    /// The hoard of every huge raid boss and finale has a weapon with a special attack in it,
    /// first, most realm raid bosses' do, and a lesser boss's only now and then
    #[test]
    fn raid_bosses_leave_special_weapons() {
        let library = Library::load();
        let mut rng = 7;
        let mut sure = [0usize; 3];
        for hoard in [Hoard::Boss, Hoard::Raid, Hoard::Huge] {
            for _ in 0..2000 {
                let rolls = hoard_rolls(hoard, &mut rng);
                assert_eq!(rolls.len(), if hoard == Hoard::Boss { BOSS_LUCKS.len() } else { RAID_LUCKS.len() });
                assert!(rolls.iter().skip(1).all(|r| !r.1), "only the best is sure to have one");
                sure[hoard as usize] += rolls[0].1 as usize;
            }
        }
        assert_eq!(sure[Hoard::Boss as usize], 0);
        assert!((1400..1800).contains(&sure[Hoard::Raid as usize]), "{sure:?}");
        assert_eq!(sure[Hoard::Huge as usize], 2000);
        // And that one has, however lucky it's rolled
        for seed in 0..200u32 {
            for luck in [RAID_LUCKS[0], RAID_LUCKS[0] * loot_scaling(realms::REALMS[realms::REALM_COUNT - 1].scaling)] {
                let s = library.special_seed(hash(seed), None, luck);
                assert!(library.special_of(s, None, luck).is_some(), "{seed} at {luck}");
            }
        }
    }

    /// Raw meat in hand roasts over a fire into roast meat, more filling, which goes in the save,
    /// see hearths.rs
    #[test]
    fn meat_roasts_over_a_fire() {
        let size = [64u32, 64, 64];
        let mut occupancy = vec![0u128; 16 * 16 * 8];
        occupancy[..16 * 16 * 2].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, DIRT);
        world.voxels_per_meter = 4;
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let mut game = Game::new(&world, &dry, 5, Vector3::new(32.0, 32.0, 16.0));
        assert!(Item::RoastMeat.food() > Item::Meat.food());
        assert!(food_price(Item::RoastMeat) > food_price(Item::Meat));
        game.add_item(Item::Meat, 2);
        game.selected = game.slots.iter().position(|(i, _)| *i == Item::Meat).unwrap();
        assert!(game.holding_raw_meat());
        assert_eq!(game.cook_selected(), 1);
        assert!(game.holding_raw_meat());
        assert_eq!(game.cook_selected(), 0);
        assert!(!game.slots.iter().any(|(i, _)| *i == Item::Meat));
        assert_eq!(game.slots.iter().find(|(i, _)| *i == Item::RoastMeat).map(|s| s.1), Some(2));
        assert!(!game.holding_raw_meat());
        // Nothing to roast, nothing changes
        let before = game.slots.clone();
        game.cook_selected();
        assert_eq!(game.slots, before);
        // Through the save
        let carried = game.carried_save();
        let text = serde_json::to_string(&carried).unwrap();
        let mut other = Game::new(&world, &dry, 9, Vector3::new(32.0, 32.0, 16.0));
        other.restore(&serde_json::from_str(&text).unwrap(), &save::WorldSave::default(), (50.0, 50.0, 50.0));
        assert_eq!(other.slots.iter().find(|(i, _)| *i == Item::RoastMeat).map(|s| s.1), Some(2));
    }
}
