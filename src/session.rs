//! Playing one of the two modes picked from the menu, Free Roam or Realms, and saving it as it goes,
//! see save.rs: starting it afresh or going on from its save, the menu that offers them, and what
//! of the game goes into the save.

use std::collections::BTreeMap;

use crate::save::{self, Mode, Save};

use super::*;

/// Seconds between saves while playing, besides those after what matters, see Game::take_milestone()
const AUTOSAVE_SECONDS: u64 = 60;
/// The normal world of the realms, which they set out from: the landscape Innert starts in
pub const REALMS_SEED: u32 = INITIAL_TERRAIN.seed;
/// What the world the player's in is called in a save, for those with only the one
const NORMAL_WORLD: &str = "normal";

/// What's picked from the menu, done at the start of the next frame
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuAction {
    Resume,
    Continue(Mode),
    New(Mode),
    Settings,
    Quit,
}

/// What the menu remembers between frames
#[derive(Default)]
pub struct ModeMenu {
    /// A new game asked for that would take the place of a saved one, waiting to be said yes to
    confirm: Option<Mode>,
}

/// What's kept of a game being played for its save, besides what Game has: see App::gather_save()
pub struct Session {
    /// None till a mode's picked
    pub mode: Option<Mode>,
    pub saves: save::Saves,
    /// Which world the player's in, and what's been done in the others, kept as it was loaded
    world: String,
    others: BTreeMap<String, save::WorldSave>,
    /// How far the journey through the realms has got, for Realms
    pub realms: Option<save::RealmsSave>,
    /// What other systems keep, each under its own name, of the whole game, see Save::section(),
    /// and of the world the player's in alone, see WorldSave::section(). A system with state of its
    /// own keeps it up to date here, or puts it in in gather_save(), and takes it back out in
    /// begin().
    pub sections: BTreeMap<String, serde_json::Value>,
    pub world_sections: BTreeMap<String, serde_json::Value>,
    /// Seconds played in all
    played: f64,
    last_saved: Instant,
    /// Saving's wanted as soon as it can be, after something that matters
    due: bool,
    /// The player's still to be stood on the dry ground nearest where they are, once the world
    /// round them's built
    pub find_ground: bool,
    /// The world's being built afresh round where the game goes on, behind the dark
    pub arriving: bool,
    pub menu: ModeMenu,
    /// Picked from the menu this frame
    pub action: Option<MenuAction>,
}

impl Session {
    /// Saving where `dir` says, or for None where saves are kept, unless `default` is false, as in
    /// screenshot runs, which then don't save at all
    pub fn new(dir: Option<PathBuf>, default: bool) -> Self {
        Session {
            mode: None,
            saves: save::Saves::new(dir, default),
            world: NORMAL_WORLD.to_owned(),
            others: BTreeMap::new(),
            realms: None,
            sections: BTreeMap::new(),
            world_sections: BTreeMap::new(),
            played: 0.0,
            last_saved: Instant::now(),
            due: false,
            find_ground: false,
            arriving: false,
            menu: ModeMenu::default(),
            action: None,
        }
    }
}

/// Where Innert starts, the lake shore on the default seed, in voxels from the corner of the
/// whole landscape, as Placed::place() works it out
fn default_start(terrain: &TerrainSettings, vpm: u32) -> Vector3<f64> {
    let widest = memory::WIDTHS_M[0] * vpm;
    camera_to_voxels(Scene::Terrain.initial_camera().position, [widest, widest, terrain.height_m * vpm])
}

impl App {
    /// Starts playing `mode`, going on from `save`, or setting out afresh without one, saving what
    /// was being played first. Where the game goes on is built afresh behind the dark if it's not
    /// in the world as it is, see rebuild_world().
    pub(super) fn begin(&mut self, mode: Mode, save: Option<Save>) {
        if self.session.mode.is_some() {
            self.save_game(false);
        }
        // Only ever a landscape
        if self.stream.is_none() {
            self.scene = Scene::Terrain;
            self.generate_world();
        }
        let vpm = self.world.voxels_per_meter as f64;
        let fresh = save.is_none();
        let save = save.unwrap_or_else(|| {
            let seed = if mode == Mode::Realms { REALMS_SEED } else { self.new_seed };
            let start = default_start(&self.terrain, self.world.voxels_per_meter) / vpm;
            let look = Scene::Terrain.initial_camera();
            Save {
                version: save::VERSION,
                mode,
                seed,
                player: save::PlayerSave { position: start.into(), find_ground: true, yaw: look.rotation.z, pitch: look.rotation.x, ..Default::default() },
                world: NORMAL_WORLD.to_owned(),
                realms: (mode == Mode::Realms).then(save::RealmsSave::default),
                ..Default::default()
            }
        });
        println!("Playing {}{}, seed {}", mode.name(), if fresh { " afresh" } else { " from its save" }, save.seed);

        // The landscape, and what's been done to it
        let seed_changed = save.seed != self.terrain.seed;
        self.terrain.seed = save.seed;
        if seed_changed {
            self.rivers = Arc::new(RiverCache::default());
        }
        let done = save.current_world();
        // What's built and dug, see bases.rs: the world's built afresh with it, or without what
        // was built in the one before
        let built_before = !self.bases.is_empty();
        self.load_bases(&save, &done);
        let scarred = !self.craters.is_empty() || !self.stains.is_empty() || !done.craters.is_empty() || !done.stains.is_empty() || built_before || !self.bases.is_empty();
        self.craters = done.craters.clone();
        self.stains = done.stains.clone();

        // What's kept for the save
        let session = &mut self.session;
        session.mode = Some(mode);
        session.world = save.world.clone();
        session.others = save.worlds.clone();
        session.others.remove(&save.world);
        session.realms = save.realms.clone().or_else(|| (mode == Mode::Realms).then(save::RealmsSave::default));
        // Other systems take what's theirs from these
        session.sections = save.sections.clone();
        session.world_sections = done.sections.clone();
        session.played = save.played;
        session.due = false;

        // What the player carries and how they are
        self.game.leave_world(save.seed);
        if fresh {
            self.game.new_character(save.seed);
        } else {
            let p = &save.player;
            self.game.restore(&save.carried, &done, (p.health, p.hunger, p.mana));
        }

        // Where they are, in the world as it is
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let mut at = Vector3::from(save.player.position) * vpm - origin;
        let find_ground = save.player.find_ground && !save.player.flying;
        if find_ground {
            at.z = self.world.size[2] as f64 - 2.0 * vpm;
        }
        let margin = 2.0 * self.stream.as_ref().map_or(0.0, |s| s.strip as f64);
        let inside = (0..2).all(|i| at[i] > margin && at[i] < self.world.size[i] as f64 - margin);
        let rebuild = seed_changed || scarred || !inside;
        self.camera.rotation.z = save.player.yaw;
        self.camera.rotation.x = save.player.pitch.clamp(-FRAC_PI_2, FRAC_PI_2);
        let eye = Vector3::new(0.0, 0.0, 1.6 * vpm);
        let water = &self.water;
        let water_level = |x: f64, y: f64| water.surface_at(x, y);
        if save.player.flying {
            self.player = None;
            self.camera.position = voxels_to_camera(at, self.world.size);
        } else if find_ground && !rebuild {
            let player = Player::on_dry_ground(at, &self.world, &water_level);
            self.camera.position = voxels_to_camera(player.eye(&self.world), self.world.size);
            self.player = Some(player);
        } else {
            // Right where they were, even where the world's not yet built to stand them in
            let mut player = Player::new(at + eye, &self.world);
            player.position = at;
            self.camera.position = voxels_to_camera(at + eye, self.world.size);
            self.player = Some(player);
        }
        self.last_feet = None;
        self.wake = None;
        self.walk_speed = 0.0;
        self.reset_accum = true;
        if rebuild {
            self.session.find_ground = find_ground;
            self.session.arriving = true;
            self.rebuild_world();
        } else {
            self.session.find_ground = false;
            // The world's the one there is, and what's in it comes back about the player
            let near = self.player.as_ref().map_or(camera_to_voxels(self.camera.position, self.world.size), |p| p.position);
            let water = &self.water;
            self.game.spawn_wildlife(&self.world, &|x, y| water.surface_at(x, y), near);
            let pieces = self.stream.as_ref().map_or(Vec::new(), |s| s.pieces.clone());
            self.adopt_pieces(&pieces);
        }
        self.started = true;
        self.menu_open = false;
        if self.capture.is_none() {
            self.set_focus(true);
        }
        // A new game takes the place of the one saved straight away, once there's ground under it
        self.session.last_saved = Instant::now();
        if !self.session.find_ground {
            self.save_game(false);
        }
    }

    /// The game as it's saved, if a mode's being played
    fn gather_save(&self) -> Option<Save> {
        let session = &self.session;
        let mode = session.mode?;
        let vpm = self.world.voxels_per_meter as f64;
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let (at, flying) = match &self.player {
            Some(player) => (player.position, false),
            None => (camera_to_voxels(self.camera.position, self.world.size), true),
        };
        let (mut health, mut hunger, mana) = self.game.vitals();
        // Slain, they go on as they'd rise again
        let at = if self.game.dead() {
            health = game::MAX_HEALTH;
            hunger = hunger.max(40.0);
            self.game.rest_spot().unwrap_or(at)
        } else {
            at
        };
        let mut done = self.game.world_save();
        done.craters = self.craters.clone();
        done.stains = self.stains.clone();
        // Other systems put in what's theirs here, as Save::set_section() and
        // WorldSave::set_section() do
        done.sections = session.world_sections.clone();
        done.set_section(bases::WORLD_SECTION, &self.bases.world_save());
        let mut worlds = session.others.clone();
        worlds.insert(session.world.clone(), done);
        Some(Save {
            version: save::VERSION,
            mode,
            seed: self.terrain.seed,
            saved_at: save::now(),
            played: session.played,
            player: save::PlayerSave {
                position: ((at + origin) / vpm).into(),
                find_ground: session.find_ground,
                yaw: self.camera.rotation.z,
                pitch: self.camera.rotation.x,
                flying,
                health,
                hunger,
                mana,
            },
            carried: self.game.carried_save(),
            world: session.world.clone(),
            worlds,
            realms: session.realms.clone(),
            sections: {
                let mut sections = session.sections.clone();
                if let Ok(value) = serde_json::to_value(self.bases.game_save(self.cycle.clock)) {
                    sections.insert(bases::GAME_SECTION.to_owned(), value);
                }
                sections
            },
        })
    }

    /// Takes up what `save` has of the bases, see bases.rs: what's carried to build with and the
    /// time of day, and what's built and dug in the world the player's in, `done`. A game without
    /// them starts with nothing built, at the start of a day.
    fn load_bases(&mut self, save: &Save, done: &save::WorldSave) {
        self.bases = bases::Bases::new();
        if let Some(world) = done.section::<bases::WorldBases>(bases::WORLD_SECTION) {
            self.bases.load_world(world);
        }
        let game = save.section::<bases::GameBases>(bases::GAME_SECTION).unwrap_or_default();
        self.bases.load_game(&game);
        let clock = game.clock.unwrap_or_default();
        let running = self.cycle.clock.running;
        self.cycle = daynight::DayNight::new(daynight::Clock { running, ..clock });
        // What's in it goes in as the world's built afresh, see stream.rs
        self.building = building::Building::default();
        self.building.applied = self.bases.version;
    }

    /// Saves the game being played, in the background, or straight away with `wait`, as on the
    /// way out
    pub(super) fn save_game(&mut self, wait: bool) {
        if !self.session.saves.enabled() {
            return;
        }
        let Some(save) = self.gather_save() else {
            return;
        };
        self.session.saves.write(save, wait);
        if wait {
            self.session.saves.finish();
        }
        self.session.last_saved = Instant::now();
        self.session.due = false;
    }

    /// Called every frame: stands the player on the ground once the world's built round them,
    /// counts the time played, and saves after what matters and every minute or so
    pub(super) fn update_session(&mut self, dt: f64) {
        let held = self.held_still();
        if self.session.arriving && self.stream.as_ref().is_none_or(|s| s.teleport.is_none()) {
            self.session.arriving = false;
        }
        if self.session.find_ground && !held {
            self.session.find_ground = false;
            if let Some(player) = &self.player {
                let water = &self.water;
                let placed = Player::on_dry_ground(player.position, &self.world, &|x, y| water.surface_at(x, y));
                self.camera.position = voxels_to_camera(placed.eye(&self.world), self.world.size);
                self.player = Some(placed);
            }
            self.session.due = true;
        }
        if self.session.mode.is_none() || self.menu_open {
            return;
        }
        self.session.played += dt;
        self.session.due |= self.game.take_milestone();
        let timely = self.session.last_saved.elapsed().as_secs() >= AUTOSAVE_SECONDS;
        if (self.session.due || timely) && !held && !self.session.find_ground {
            self.save_game(false);
        }
    }

    /// Does what was picked from the menu
    pub(super) fn menu_picked(&mut self, action: MenuAction, event_loop: &ActiveEventLoop) {
        match action {
            MenuAction::Resume => {
                self.menu_open = false;
                self.started = true;
                self.set_focus(true);
            }
            // Going on with what's being played is as it is
            MenuAction::Continue(mode) if self.session.mode == Some(mode) => self.menu_picked(MenuAction::Resume, event_loop),
            MenuAction::Continue(mode) => {
                let save = self.session.saves.saved(mode).cloned();
                self.begin(mode, save);
            }
            MenuAction::New(mode) => self.begin(mode, None),
            MenuAction::Settings => self.show_settings = !self.show_settings,
            MenuAction::Quit => {
                self.save_game(true);
                event_loop.exit();
            }
        }
    }

    /// For checking a game saved comes back the same, in screenshot runs: where the player is,
    /// in meters of the whole landscape, and all about them, in a line
    pub(super) fn state_line(&self) -> String {
        let vpm = self.world.voxels_per_meter as f64;
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let at = self.player.as_ref().map_or(camera_to_voxels(self.camera.position, self.world.size), |p| p.position);
        let at = (at + origin) / vpm;
        format!(
            "mode={} seed={} world={} at={:.2},{:.2},{:.2} walking={} {} craters={} stains={} realms={:?} built={} carried={:?} time={:.2} day={}",
            self.session.mode.map_or("none", Mode::name),
            self.terrain.seed,
            self.session.world,
            at.x,
            at.y,
            at.z,
            self.player.is_some(),
            self.game.summary(),
            self.craters.len(),
            self.stains.len(),
            self.session.realms.as_ref().map(|r| (r.realm, r.unlocked.clone(), r.visited.clone())),
            self.bases.pieces().len(),
            self.bases.materials,
            self.cycle.hour(),
            self.cycle.clock.day
        )
    }
}

/// The main menu: Resume while playing, each mode with Continue if it has a save and New, asking
/// first before a new game takes the place of a saved one, then Settings and Quit. What's picked
/// is given back.
pub fn menu(menu: &mut ModeMenu, ui: &mut egui::Ui, started: bool, playing: Option<Mode>, saves: &save::Saves, seed: &mut u32, show_settings: bool) -> Option<MenuAction> {
    let mut picked = None;
    let big = |ui: &mut egui::Ui, text: &str| ui.add_sized([260.0, 44.0], egui::Button::new(RichText::new(text).size(20.0)));
    let gray = Color32::from_gray(190);
    ui.label(RichText::new("INNERT").size(60.0).strong().color(Color32::WHITE));
    ui.add_space(18.0);

    if let Some(mode) = menu.confirm {
        let when = saves.saved(mode).map_or(String::new(), |s| format!(", saved {}", save::ago(s.saved_at)));
        ui.label(RichText::new(format!("Start a new {} game?", mode.name())).size(22.0).color(Color32::WHITE));
        ui.add_space(6.0);
        ui.label(RichText::new(format!("It takes the place of the one you have{when}, which can't be had back.")).size(14.0).color(gray));
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space((ui.available_width() - 260.0).max(0.0) * 0.5);
            if ui.add_sized([126.0, 40.0], egui::Button::new(RichText::new("Start anew").size(18.0))).clicked() {
                picked = Some(MenuAction::New(mode));
                menu.confirm = None;
            }
            ui.add_space(8.0);
            if ui.add_sized([126.0, 40.0], egui::Button::new(RichText::new("Keep it").size(18.0))).clicked() {
                menu.confirm = None;
            }
        });
        return picked;
    }

    if started && big(ui, "Resume").clicked() {
        picked = Some(MenuAction::Resume);
    }
    for mode in Mode::ALL {
        ui.add_space(14.0);
        let now = started && playing == Some(mode);
        ui.label(RichText::new(mode.name()).size(24.0).strong().color(Color32::WHITE));
        let about = match mode {
            Mode::FreeRoam => "The endless wilderness, on any seed",
            Mode::Realms => "A journey from the normal world through the realms",
        };
        ui.label(RichText::new(about).size(13.0).color(gray));
        ui.add_space(4.0);
        let saved = saves.saved(mode);
        if let Some(save) = saved {
            let played = (save.played / 60.0).round() as u64;
            let seed = if mode == Mode::FreeRoam { format!(", seed {}", save.seed) } else { String::new() };
            let note = if now { format!("Playing now{seed}") } else { format!("Saved {}{seed}, {played} min played", save::ago(save.saved_at)) };
            ui.label(RichText::new(note).size(13.0).color(Color32::from_rgb(200, 215, 170)));
            ui.add_space(2.0);
        }
        if let save::Slot::Unreadable(why) = saves.slot(mode) {
            ui.colored_label(Color32::from_rgb(255, 170, 120), RichText::new(format!("The saved game couldn't be read: {why}. It's been put aside, so a new one starts.")).size(13.0));
        }
        ui.horizontal(|ui| {
            let half = [126.0, 40.0];
            let continuing = saved.is_some() && saves.enabled();
            ui.add_space((ui.available_width() - if continuing { 260.0 } else { 126.0 }).max(0.0) * 0.5);
            if continuing {
                if ui.add_sized(half, egui::Button::new(RichText::new("Continue").size(18.0))).clicked() {
                    picked = Some(MenuAction::Continue(mode));
                }
                ui.add_space(8.0);
            }
            if ui.add_sized(half, egui::Button::new(RichText::new("New").size(18.0))).clicked() {
                if saved.is_some() {
                    menu.confirm = Some(mode);
                } else {
                    picked = Some(MenuAction::New(mode));
                }
            }
        });
        if mode == Mode::FreeRoam {
            ui.horizontal(|ui| {
                ui.add_space(70.0);
                ui.add(egui::DragValue::new(seed).prefix("New world's seed: "));
                if ui.button("Random").clicked() {
                    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
                    *seed = nanos % 100_000;
                }
            });
        }
    }
    ui.add_space(18.0);
    if big(ui, if show_settings { "Hide Settings" } else { "Settings" }).clicked() {
        picked = Some(MenuAction::Settings);
    }
    ui.add_space(8.0);
    if big(ui, "Quit").clicked() {
        picked = Some(MenuAction::Quit);
    }
    if !saves.enabled() {
        ui.add_space(6.0);
        ui.label(RichText::new("Not saving in this run").size(12.0).color(gray));
    }
    picked
}
