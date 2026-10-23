//! Living off the land: food to find and eat, an inventory to carry it in, animals to hunt or pet,
//! and the heads-up display that shows it all. Positions are in voxels of the main world.

use std::f64::consts::TAU;
use std::time::Instant;

use egui_winit_vulkano::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, StrokeKind, pos2, vec2};
use nalgebra::Vector3;

use crate::animals::{self, Animal, AnimalInstance, Look};
use crate::player::WaterLevel;
use crate::worldgen::{World, is_passable, material::*};

/// How far away things can be picked up or hit, in meters
pub const REACH: f64 = 4.0;
/// Hunger drops from full to empty in this many seconds
const HUNGER_SECONDS: f64 = 600.0;
/// Animals further than this aren't drawn, and food further than that, in meters
const ANIMAL_RANGE: f64 = 140.0;
const FOOD_RANGE: f64 = 60.0;
/// The most animals, bits of food and props drawn at once, nearest first
const MAX_INSTANCES: usize = 160;
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
}

impl Item {
    fn look(self) -> Look {
        match self {
            Item::Apple => Look::Apple,
            Item::Meat => Look::Meat,
            Item::Bucket => unreachable!("the bucket is never left lying around"),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Item::Bucket => "Bucket",
            Item::Apple => "Apple",
            Item::Meat => "Raw Meat",
        }
    }

    /// How much hunger eating one takes away, out of 100
    fn food(self) -> f64 {
        match self {
            Item::Bucket => 0.0,
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

/// What the crosshair is on
#[derive(Clone, Copy)]
enum Target {
    Animal(usize),
    Pickup(usize),
}

pub struct Game {
    animals: Vec<Animal>,
    pickups: Vec<Pickup>,
    /// Candles, skulls and such in buildings, that are only there to look at
    props: Vec<(Look, Vector3<f64>, f64)>,
    /// What the player carries, and how many of each, shown in the quick bar
    slots: Vec<(Item, u32)>,
    /// The quick bar slot right click eats from
    pub selected: usize,
    /// 100 is full, 0 is starving
    hunger: f64,
    /// Cubic meters of water in the bucket
    bucket: f64,
    messages: Vec<(String, Instant)>,
    target: Option<Target>,
    apple_timer: f64,
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
            slots: vec![(Item::Bucket, 1)],
            selected: 0,
            hunger: 80.0,
            bucket: 0.0,
            messages: Vec::new(),
            target: None,
            apple_timer: 0.0,
            rng: hash(seed ^ 0x9a3e),
        };
        game.populate(world, water, seed, near);
        game
    }

    /// Fills a new world with animals and apples. What the player carries stays.
    pub fn populate(&mut self, world: &World, water: WaterLevel, seed: u32, near: Vector3<f64>) {
        self.animals = animals::spawn(world, water, seed, near);
        self.pickups.clear();
        self.target = None;
        for _ in 0..4000 {
            if self.apple_count() >= APPLES {
                break;
            }
            self.grow_apple(world, water, near);
        }
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

    /// What the player carries, for checking from scripts
    pub fn carried(&self) -> String {
        self.slots.iter().map(|(item, count)| format!("{} x{}", item.name(), count)).collect::<Vec<_>>().join(", ")
    }

    pub fn clear(&mut self) {
        self.animals.clear();
        self.pickups.clear();
        self.props.clear();
        self.target = None;
    }

    /// Candles, skulls and such: what they are, where in voxels and which way they face
    pub fn set_props(&mut self, props: Vec<(Look, Vector3<f64>, f64)>) {
        self.props = props;
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

    /// The nearest animal or food called `name` to `near`: where it stands, and how high its middle is in meters
    pub fn find_animal(&self, name: &str, near: Vector3<f64>) -> Option<(Vector3<f64>, f64)> {
        let animals = self.animals.iter().map(|a| (a.look, a.position));
        let pickups = self.pickups.iter().map(|p| (p.item.look(), p.position));
        animals
            .chain(pickups)
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

    pub fn say(&mut self, message: impl Into<String>) {
        self.messages.push((message.into(), Instant::now()));
        if self.messages.len() > 4 {
            self.messages.remove(0);
        }
    }

    /// `player` is where the player stands, None while flying.
    /// `eye` and `look` are where the camera is and which way it looks.
    pub fn update(&mut self, world: &World, water: WaterLevel, player: Option<Vector3<f64>>, eye: Vector3<f64>, look: Vector3<f64>, dt: f64) {
        for animal in &mut self.animals {
            animal.update(world, water, player, dt);
        }

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
                self.grow_apple(world, water, eye);
            }
        }

        self.target = self.find_target(world, eye, look);
        self.messages.retain(|(_, at)| at.elapsed().as_secs_f64() < 4.0);
    }

    /// The nearest animal or bit of food the camera looks at, within reach
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
        target
    }

    fn add_item(&mut self, item: Item, count: u32) {
        match self.slots.iter_mut().find(|(i, _)| *i == item) {
            Some(slot) => slot.1 += count,
            None => self.slots.push((item, count)),
        }
    }

    /// Left click: picks up the food the crosshair is on, or hits the animal, or pets it
    pub fn primary(&mut self, eye: Vector3<f64>) {
        match self.target {
            Some(Target::Pickup(_)) => self.interact(eye),
            Some(Target::Animal(i)) => {
                let look = self.animals[i].look;
                if look.is_pet() {
                    self.interact(eye);
                    return;
                }
                if self.animals[i].hurt(eye) {
                    let animal = self.animals.remove(i);
                    for k in 0..look.meat() {
                        let angle = animal.heading() + k as f64 * 2.1;
                        let spread = 0.25 * k as f64;
                        self.pickups.push(Pickup {
                            item: Item::Meat,
                            position: animal.position + Vector3::new(angle.cos(), angle.sin(), 0.0) * spread,
                            heading: angle,
                        });
                    }
                    self.say(format!("You hunted a {}. It dropped some meat.", look.name()));
                    self.target = None;
                } else {
                    self.say(format!("You hit the {}!", look.name()));
                }
            }
            None => {}
        }
    }

    /// E: picks up food, pets dogs and cats
    pub fn interact(&mut self, eye: Vector3<f64>) {
        match self.target {
            Some(Target::Pickup(i)) => {
                let item = self.pickups.remove(i).item;
                self.add_item(item, 1);
                self.target = None;
                self.say(format!("+1 {}", item.name()));
            }
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

    /// Right click: eats one of what's selected in the quick bar
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
        if count > 1 {
            self.slots[slot].1 -= 1;
        } else {
            self.slots.remove(slot);
            self.selected = self.selected.min(self.slots.len().saturating_sub(1));
        }
        self.say(format!("You ate the {}.", item.name().to_lowercase()));
    }

    /// What to draw for the shader: the nearest animals and food around the camera
    pub fn instances(&self, world: &World, camera: Vector3<f64>) -> Vec<AnimalInstance> {
        let vpm = world.voxels_per_meter as f64;
        let mut near: Vec<(f64, AnimalInstance)> = Vec::new();
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
        for &(look, position, heading) in &self.props {
            let d = (position - camera).norm();
            if d < FOOD_RANGE * vpm {
                near.push((d, animals::still_instance(look, position, heading, world.voxels_per_meter)));
            }
        }
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        near.into_iter().take(MAX_INSTANCES).map(|(_, i)| i).collect()
    }

    fn target_hint(&self) -> Option<String> {
        match self.target? {
            Target::Pickup(i) => Some(format!("E  pick up {}", self.pickups[i].item.name().to_lowercase())),
            Target::Animal(i) => {
                let look = self.animals[i].look;
                Some(if look.is_pet() { format!("E  pet the {}", look.name()) } else { format!("Click  hunt the {}", look.name()) })
            }
        }
    }

    /// Crosshair, and along the top the quick bar, hunger and messages, drawn over the game
    pub fn hud(&self, ctx: &egui::Context) {
        let screen = ctx.screen_rect();
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("hud")));
        let center = screen.center();
        let shadow = Color32::from_black_alpha(150);
        let faint = Color32::from_white_alpha(140);

        painter.circle_filled(center, 3.5, shadow);
        painter.circle_filled(center, 2.0, Color32::WHITE);

        if let Some(hint) = self.target_hint() {
            let at = center + vec2(0.0, 28.0);
            painter.text(at + vec2(1.0, 1.0), Align2::CENTER_TOP, &hint, FontId::proportional(17.0), shadow);
            painter.text(at, Align2::CENTER_TOP, &hint, FontId::proportional(17.0), Color32::WHITE);
        }

        // One panel across the top middle: the quick bar on the left, hunger on the right
        let slot = 44.0;
        let gap = 6.0;
        let pad = 10.0;
        let slots_width = if self.slots.is_empty() { 120.0 } else { self.slots.len() as f32 * (slot + gap) - gap };
        let hunger_width = 170.0;
        let width = pad + slots_width + 20.0 + hunger_width + pad;
        let panel = Rect::from_min_size(pos2(center.x - width / 2.0, screen.top() + 14.0), vec2(width, slot + 2.0 * pad));
        painter.rect_filled(panel, 12.0, Color32::from_black_alpha(120));

        let left = panel.left() + pad;
        if self.slots.is_empty() {
            painter.text(pos2(left + slots_width / 2.0, panel.center().y), Align2::CENTER_CENTER, "No food yet", FontId::proportional(14.0), faint);
        }
        for (i, &(item, count)) in self.slots.iter().enumerate() {
            let rect = Rect::from_min_size(pos2(left + i as f32 * (slot + gap), panel.top() + pad), vec2(slot, slot));
            let selected = i == self.selected;
            painter.rect_filled(rect, 8.0, Color32::from_white_alpha(if selected { 40 } else { 14 }));
            if selected {
                painter.rect_stroke(rect, 8.0, Stroke::new(2.0f32, Color32::WHITE), StrokeKind::Inside);
            }
            draw_icon(&painter, item, rect.center() - vec2(0.0, 2.0), slot * 0.58, (self.bucket / BUCKET_CAPACITY) as f32);
            if item != Item::Bucket {
                painter.text(rect.right_bottom() - vec2(5.0, 2.0), Align2::RIGHT_BOTTOM, count.to_string(), FontId::proportional(13.0), Color32::WHITE);
            }
        }

        let hunger_left = left + slots_width + 20.0;
        painter.text(pos2(hunger_left, panel.top() + pad + 4.0), Align2::LEFT_TOP, "Hunger", FontId::proportional(12.0), faint);
        let bar = Rect::from_min_size(pos2(hunger_left, panel.top() + pad + 24.0), vec2(hunger_width, 10.0));
        painter.rect_filled(bar, 5.0, Color32::from_white_alpha(25));
        let fill = Rect::from_min_size(bar.min, vec2(hunger_width * (self.hunger / 100.0) as f32, 10.0));
        let color = if self.hunger > 25.0 { Color32::from_rgb(232, 160, 60) } else { Color32::from_rgb(220, 70, 50) };
        painter.rect_filled(fill, 5.0, color);

        // Messages just under the panel, newest first, fading out
        for (k, (message, at)) in self.messages.iter().rev().enumerate() {
            let alpha = (1.0 - (at.elapsed().as_secs_f32() - 3.0).max(0.0)).clamp(0.0, 1.0);
            let pos = pos2(center.x, panel.bottom() + 12.0 + k as f32 * 24.0);
            painter.text(pos + vec2(1.0, 1.0), Align2::CENTER_TOP, message, FontId::proportional(17.0), Color32::from_black_alpha((150.0 * alpha) as u8));
            painter.text(pos, Align2::CENTER_TOP, message, FontId::proportional(17.0), Color32::from_white_alpha((255.0 * alpha) as u8));
        }
    }

    /// The inventory, listing what's carried with a button to eat each
    pub fn inventory(&mut self, ctx: &egui::Context, open: &mut bool) {
        let mut eat = None;
        egui::Window::new("Inventory")
            .open(open)
            .collapsible(false)
            .resizable(false)
            // Just under the quick bar
            .anchor(Align2::CENTER_TOP, [0.0, 100.0])
            .show(ctx, |ui| {
                ui.label(format!("Hunger: {:.0} / 100", self.hunger));
                ui.separator();
                if self.slots.is_empty() {
                    ui.label("You aren't carrying anything yet.");
                    ui.label("Look for apples under leafy trees, or hunt bears and squirrels for meat.");
                }
                for (i, &(item, count)) in self.slots.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(vec2(36.0, 36.0), egui::Sense::hover());
                        draw_icon(ui.painter(), item, rect.center(), 26.0, (self.bucket / BUCKET_CAPACITY) as f32);
                        if item == Item::Bucket {
                            ui.label(format!("Bucket: {:.1} of {:.0} m³ of water", self.bucket, BUCKET_CAPACITY));
                        } else {
                            ui.label(format!("{} × {}", item.name(), count));
                            ui.add_space(12.0);
                            if ui.button(format!("Eat (+{:.0})", item.food())).clicked() {
                                eat = Some(i);
                            }
                        }
                        if ui.selectable_label(self.selected == i, "Selected").clicked() {
                            self.selected = i;
                        }
                    });
                }
                ui.separator();
                ui.label("Click picks up food or hunts, E picks up or pets.");
                ui.label("Right click eats the selected food, number keys select it.");
                ui.label("Holding the bucket, click on water to scoop it up and right click to pour it out.");
            });
        if let Some(slot) = eat {
            self.eat(slot);
        }
    }
}

/// A little picture of an item, `size` across. `full` is how full the bucket is, 0-1.
fn draw_icon(painter: &egui::Painter, item: Item, center: Pos2, size: f32, full: f32) {
    let r = size * 0.5;
    match item {
        Item::Bucket => {
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
    }
}
