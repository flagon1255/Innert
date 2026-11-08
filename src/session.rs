//! Playing one of the two modes picked from the menu, Free Roam or Realms, and saving it as it goes,
//! see save.rs: starting it afresh or going on from its save, the menu that offers them, and what
//! of the game goes into the save.

use std::collections::BTreeMap;

use crate::game::Item;
use crate::realms::{self, REALMS};
use crate::save::{self, Mode, Save};

use super::*;

/// Seconds between saves while playing, besides those after what matters, see Game::take_milestone()
const AUTOSAVE_SECONDS: u64 = 60;
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
    /// A new game asked for that would take the place of a saved one, waiting to be said yes to.
    /// `--set confirm=freeroam` asks straight away, to see it in a screenshot.
    pub confirm: Option<Mode>,
}

/// What's picked at the Innert Gate or a portal home, done at the start of the next frame
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GateAction {
    /// Setting the Innert blocks of this realm carried into the gate
    SetIn(usize),
    /// Going through to this realm
    Travel(usize),
    Close,
}

/// How long the screen takes to darken going through the gate or a portal, in seconds, before the
/// realm gone to is built behind the dark
const LEAVING_SECONDS: f32 = 0.7;

/// For trying out the journey in screenshot runs, from `--set` settings, see App::try_journey().
/// Frames are counted from when the world's first there to play in, as building it afresh takes a
/// while, behind the dark.
#[derive(Default)]
struct Trial {
    /// When the nearest raid boss falls as if beaten, `slay_raid_at=200`
    slay_raid_at: Option<usize>,
    /// When the blocks carried are set into the gate, `set_in_at=230`
    set_in_at: Option<usize>,
    /// When to go through to which realm, `travel_at=260:1`
    travel_at: Option<(usize, usize)>,
    /// Shots taken so many frames after each time the world's there, `shots_after=5,120`, and
    /// the run ends so many after the last time, `end_after=200`
    shots_after: Vec<usize>,
    end_after: Option<usize>,
    /// In the sky isles: when the grappling hook's thrown at the nearest other island,
    /// `hook_at=60`, when the player's lifted high and glides on ahead, `glide_at=60`, and when
    /// they're put out over the open air to fall, `fall_at=60`, see rope.rs
    hook_at: Option<usize>,
    glide_at: Option<usize>,
    /// While gliding, a flap of the wings that many frames into it, lap_after=40, see wings.rs
    flap_after: Option<usize>,
    flap_now: bool,
    fall_at: Option<usize>,
    /// Once the world's first there: the view turned to look that many degrees round from east,
    /// `face=0`, and lifted that many meters up off the player's feet, flying, to look out over
    /// it, `overlook=40`
    face: Option<f64>,
    overlook: Option<f64>,
    /// Whether they're gliding, and when they started
    glide: Option<usize>,
    /// The frame the world was first there, and whether it's there now
    ready: Option<usize>,
    there: bool,
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
    /// Where things are in the realm the player's in, in Realms, see realms.rs
    pub places: Option<realms::Places>,
    /// Whether the window of the gate or the portal is open, and what was picked in it
    pub gate_open: bool,
    pub gate_action: Option<GateAction>,
    /// Going through to another realm: which, and since when, as the screen darkens
    pub leaving: Option<(usize, Instant)>,
    /// Whether the realm's three raid bosses are beaten, and its finale's risen if it has one
    finale_risen: bool,
    trial: Trial,
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
            places: None,
            gate_open: false,
            gate_action: None,
            leaving: None,
            finale_risen: false,
            trial: Trial::default(),
        }
    }

    /// Whether a screenshot run's trial has the player gliding, see Trial::glide_at
    pub fn trial_glides(&self) -> bool {
        self.trial.glide.is_some()
    }

    /// Whether a screenshot run's trial flaps the wings this frame, see Trial::flap_after
    pub fn trial_flaps(&mut self) -> bool {
        std::mem::take(&mut self.trial.flap_now)
    }

    /// How dark the screen is going through to another realm, 0 to 1
    pub fn leaving_dark(&self) -> f32 {
        self.leaving.map_or(0.0, |(_, since)| (since.elapsed().as_secs_f32() / LEAVING_SECONDS).min(1.0))
    }
}

/// The realm being played in Realms, and whether that's so
fn realm_of(session: &Session) -> Option<usize> {
    session.realms.as_ref().map(|r| (r.realm as usize).min(realms::REALMS.len() - 1))
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
        let mut save = save.unwrap_or_else(|| {
            let start = default_start(&self.terrain, self.world.voxels_per_meter) / vpm;
            let look = Scene::Terrain.initial_camera();
            let mut player = save::PlayerSave { position: start.into(), find_ground: true, yaw: look.rotation.z, pitch: look.rotation.x, ..Default::default() };
            // The journey through the realms starts in front of the Innert Gate, looking at it
            if mode == Mode::Realms {
                let places = self.places_in(0);
                player.position = [places.arrival[0], places.arrival[1], 0.0];
                player.yaw = places.facing + FRAC_PI_2;
                player.pitch = 0.12;
            }
            Save {
                version: save::VERSION,
                mode,
                seed: if mode == Mode::Realms { REALMS[0].seed } else { self.new_seed.min(realms::FREE_ROAM_SEEDS) },
                player,
                world: NORMAL_WORLD.to_owned(),
                realms: (mode == Mode::Realms).then(save::RealmsSave::default),
                ..Default::default()
            }
        });
        // In Realms, the realm the journey's got to is the world and the landscape
        if let Some(progress) = &save.realms
            && mode == Mode::Realms
        {
            let realm = &REALMS[(progress.realm as usize).min(REALMS.len() - 1)];
            save.seed = realm.seed;
            save.world = realm.id.to_owned();
        }
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
        session.gate_open = false;
        session.leaving = None;

        // What the player carries and how they are
        self.game.leave_world(save.seed);
        if fresh {
            self.game.new_character(save.seed);
        } else {
            let p = &save.player;
            self.game.restore(&save.carried, &done, (p.health, p.hunger, p.mana));
        }
        // Where the gate, the portal and the raid bosses are in the realm, and how hard it is
        self.enter_realm();

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

    /// Where things are in realm `realm`: as for Places::find(), round where Innert starts, looking
    /// the way it starts looking
    fn places_in(&self, realm: usize) -> realms::Places {
        let vpm = self.world.voxels_per_meter;
        let settings = TerrainSettings { seed: REALMS[realm].seed, ..self.terrain_settings() };
        let start = default_start(&self.terrain, vpm) / vpm as f64;
        let yaw = Scene::Terrain.initial_camera().rotation.z;
        realms::Places::find(realm, &settings, [start.x, start.y], [-yaw.sin(), yaw.cos()])
    }

    /// Takes up the realm the player's in, in Realms: where its gate or portal and its raid bosses
    /// are, and how much harder it is. In Free Roam, none of that.
    fn enter_realm(&mut self) {
        match (self.session.mode, realm_of(&self.session)) {
            (Some(Mode::Realms), Some(realm)) => {
                let places = self.places_in(realm);
                let journeys = self.session.realms.as_ref().map_or(0, |r| r.completed);
                self.game.set_scaling(REALMS[realm].scaling.after(journeys));
                // Hooks and wings on sale once the way to the sky isles is open
                self.game.set_sky_wares(self.session.realms.as_ref().is_some_and(|r| r.unlocked.contains(&(realms::HOOK_REALM as u32))));
                println!(
                    "REALM {} ({}): gate at {:.0}, {:.0}, arrival {:.0}, {:.0}, raid bosses at {}",
                    realm,
                    REALMS[realm].name,
                    places.gate[0],
                    places.gate[1],
                    places.arrival[0],
                    places.arrival[1],
                    places.lairs.iter().zip(&REALMS[realm].raid_bosses).map(|(p, (look, _))| format!("{} {:.0}, {:.0}", look.name(), p[0], p[1])).collect::<Vec<_>>().join("; ")
                );
                self.session.places = Some(places);
                self.session.finale_risen = false;
            }
            _ => {
                self.game.set_scaling(realms::Scaling::NONE);
                // Free Roam's merchants have hooks and wings to sell from the first
                self.game.set_sky_wares(self.session.mode == Some(Mode::FreeRoam));
                self.session.places = None;
            }
        }
    }

    /// The realm's raid bosses' lairs and its gate or portal home, those in the world where it is
    /// now, in its voxels, for adopt_features()
    pub(super) fn realm_sites(&self) -> (Vec<game::Lair>, Vec<game::Shrine>) {
        let (Some(places), Some(progress)) = (&self.session.places, &self.session.realms) else {
            return (Vec::new(), Vec::new());
        };
        let vpm = self.world.voxels_per_meter as f64;
        let [sx, sy, _] = self.world.size.map(f64::from);
        let local = |p: [f64; 2]| Vector3::new(p[0] * vpm - self.world.origin[0] as f64, p[1] * vpm - self.world.origin[1] as f64, 0.0);
        let inside = |p: &Vector3<f64>| p.x >= 2.0 && p.y >= 2.0 && p.x < sx - 2.0 && p.y < sy - 2.0;
        let realm = &REALMS[places.realm];
        let mut lairs = Vec::new();
        for slot in 0..realms::RAIDS {
            let mut at = local(places.lairs[slot]);
            if inside(&at) {
                at.z = game::ground_top(&self.world, at.x, at.y);
                lairs.push(game::Lair { look: realm.raid_bosses[slot].0, position: at, key: places.lair_key(slot), raid: Some(places.realm as u8) });
            }
        }
        // The realm's finale, once its three raid bosses are beaten, see realms::on_raids_beaten()
        if let Some(lair) = self.finale_lair() {
            lairs.push(lair);
        }
        let gate = local(places.gate);
        let placed = std::array::from_fn(|r| progress.placed.get(r).copied().unwrap_or(0));
        let shrines = if inside(&gate) {
            // The gate's veil is lit once it leads anywhere; a portal always leads home
            vec![game::Shrine { gate: places.realm == 0, position: gate, facing: places.facing, placed, open: places.realm != 0 || progress.unlocked.len() > 1 }]
        } else {
            Vec::new()
        };
        (lairs, shrines)
    }

    /// The lair of the realm's finale, if it has one and its three raid bosses are beaten, and it's
    /// in the world where it is now, in its voxels
    fn finale_lair(&self) -> Option<game::Lair> {
        let places = self.session.places.as_ref()?;
        let (look, _) = realms::on_raids_beaten(places.realm)?;
        let (spot, key) = (places.finale?, places.finale_key()?);
        if !(0..realms::RAIDS).all(|k| self.game.is_defeated(&places.lair_key(k))) {
            return None;
        }
        let vpm = self.world.voxels_per_meter as f64;
        let mut at = Vector3::new(spot[0] * vpm - self.world.origin[0] as f64, spot[1] * vpm - self.world.origin[1] as f64, 0.0);
        let [sx, sy, _] = self.world.size.map(f64::from);
        if at.x < 2.0 || at.y < 2.0 || at.x >= sx - 2.0 || at.y >= sy - 2.0 {
            return None;
        }
        at.z = game::ground_top(&self.world, at.x, at.y);
        Some(game::Lair { look, position: at, key, raid: Some(places.realm as u8) })
    }

    /// The realm's finale rising the moment its third raid boss falls, see realms::on_raids_beaten()
    fn raise_finale(&mut self) {
        let Some(places) = &self.session.places else {
            return;
        };
        let risen = (0..realms::RAIDS).all(|k| self.game.is_defeated(&places.lair_key(k)));
        if risen == self.session.finale_risen {
            return;
        }
        self.session.finale_risen = risen;
        let beaten = places.finale_key().is_some_and(|key| self.game.is_defeated(&key));
        let Some((look, _)) = realms::on_raids_beaten(places.realm).filter(|_| risen && !beaten) else {
            return;
        };
        let at = REALMS[places.realm].finale.map_or("", |(_, _, at)| at);
        self.game.say(format!("The ground shakes. Out of {at} rises the {}!", look.name()));
        println!("FINALE rises in {}: the {} at {at}", REALMS[places.realm].name, look.name());
        if let Some(lair) = self.finale_lair() {
            let water = &self.water;
            self.game.add_lair(&self.world, &|x, y| water.surface_at(x, y), lair);
        }
    }

    /// The gate or portal as it is now, after blocks are set in it
    fn refresh_shrines(&mut self) {
        let (_, shrines) = self.realm_sites();
        self.game.set_shrines(&self.world, shrines);
    }

    /// Does what was picked at the gate or a portal
    fn gate_picked(&mut self, action: GateAction) {
        match action {
            GateAction::Close => {
                self.session.gate_open = false;
                self.set_focus(true);
            }
            GateAction::Travel(to) => {
                self.session.gate_open = false;
                self.session.leaving = Some((to, Instant::now()));
                self.set_focus(true);
            }
            GateAction::SetIn(realm) => {
                let Some(progress) = &mut self.session.realms else {
                    return;
                };
                if progress.placed.len() < REALMS.len() {
                    progress.placed.resize(REALMS.len(), 0);
                }
                let (_, restored_before) = realms::opened(&progress.placed);
                let wanted = (realms::RAIDS as u32).saturating_sub(progress.placed[realm]);
                let taken = self.game.take_innert_blocks(realm as u8, wanted);
                let progress = self.session.realms.as_mut().unwrap();
                progress.placed[realm] += taken;
                let (open, restored) = realms::opened(&progress.placed);
                let newly: Vec<u32> = open.iter().copied().filter(|r| !progress.unlocked.contains(r)).collect();
                progress.unlocked.extend(&newly);
                progress.unlocked.sort_unstable();
                let message = if restored && !restored_before {
                    progress.completed += 1;
                    format!("All {} realms are restored! The gate blazes with their light, and the way to every realm stands open. Foes grow stronger from here on.", ["", "one", "two", "three", "four", "five", "six", "seven", "eight"].get(REALMS.len()).copied().unwrap_or("the"))
                } else if let Some(&r) = newly.last() {
                    format!("The gate wakes: the way to {} is open!", REALMS[r as usize].name)
                } else {
                    format!("{taken} Innert block{} set into the gate.", if taken == 1 { "" } else { "s" })
                };
                println!("GATE: {message} placed {:?}, unlocked {:?}", progress.placed, progress.unlocked);
                let sky_open = progress.unlocked.contains(&(realms::HOOK_REALM as u32));
                self.game.set_sky_wares(sky_open);
                if restored && !restored_before {
                    // Harder from here on, in the normal world too
                    self.enter_realm();
                }
                self.game.say(message);
                self.refresh_shrines();
                self.save_game(false);
            }
        }
    }

    /// Goes through to realm `to`: what's been done in the realm left is kept, and the one gone to
    /// is built afresh round its gate or portal, behind the dark, with the player in front of it
    /// looking out
    pub(super) fn travel(&mut self, to: usize) {
        if self.session.realms.is_none() {
            return;
        }
        let realm = &REALMS[to];
        // What's been done in the realm left, kept for coming back
        let mut left = self.game.world_save();
        left.craters = std::mem::take(&mut self.craters);
        left.stains = std::mem::take(&mut self.stains);
        left.sections = std::mem::take(&mut self.session.world_sections);
        // And what's built there, see bases.rs
        left.set_section(bases::WORLD_SECTION, &self.bases.world_save());
        let from = std::mem::replace(&mut self.session.world, realm.id.to_owned());
        self.session.others.insert(from, left);
        let there = self.session.others.remove(realm.id).unwrap_or_default();
        if let Some(progress) = &mut self.session.realms {
            progress.realm = to as u32;
            if !progress.visited.contains(&(to as u32)) {
                progress.visited.push(to as u32);
            }
        }
        // Its landscape and what's been done to it
        self.terrain.seed = realm.seed;
        self.rivers = Arc::new(RiverCache::default());
        self.craters = there.craters.clone();
        self.stains = there.stains.clone();
        self.session.world_sections = there.sections.clone();
        // What's built in the realm gone to, put in as it's built afresh; what's carried goes along
        self.bases.load_world(there.section(bases::WORLD_SECTION).unwrap_or_default());
        self.building = building::Building::default();
        self.building.applied = self.bases.version;
        self.game.leave_world(realm.seed);
        self.game.restore_world(&there);
        self.enter_realm();
        // In front of the gate or the portal, looking out from it, stood on the ground once the
        // realm's built round them
        let places = self.session.places.clone().expect("a realm's places");
        let vpm = self.world.voxels_per_meter as f64;
        let at = Vector3::new(places.arrival[0] * vpm - self.world.origin[0] as f64, places.arrival[1] * vpm - self.world.origin[1] as f64, self.world.size[2] as f64 - 2.0 * vpm);
        let eye = Vector3::new(0.0, 0.0, 1.6 * vpm);
        let mut player = Player::new(at + eye, &self.world);
        player.position = at;
        self.player = Some(player);
        self.camera.position = voxels_to_camera(at + eye, self.world.size);
        self.camera.rotation.z = places.facing - FRAC_PI_2;
        self.camera.rotation.x = -0.05;
        self.last_feet = None;
        self.wake = None;
        self.session.find_ground = true;
        self.session.arriving = true;
        self.session.gate_open = false;
        self.rebuild_world();
        self.game.say(if to == 0 { "You step back through to the Innert Gate.".to_owned() } else { format!("You step through into {}.", realm.name) });
    }

    /// For trying out the journey: as if it had got as far as realm `realm`, each realm before it
    /// opened with all its blocks set in, the player there
    fn set_out_for(&mut self, realm: usize) {
        let realm = realm.min(REALMS.len() - 1);
        if let Some(progress) = &mut self.session.realms {
            progress.placed = (0..REALMS.len()).map(|r| if r < realm { realms::RAIDS as u32 } else { 0 }).collect();
            progress.unlocked = (0..=realm as u32).collect();
            progress.visited = progress.unlocked.clone();
        }
        if realm > 0 {
            self.travel(realm);
        }
    }

    /// For trying the journey out in screenshot runs, see Trial: `realm=2` sets out there, with
    /// each realm before it opened, `blocks=3` gives that many of the realm's Innert blocks,
    /// `gate_window=1` opens the gate's window, `near_raid=0` stands the player 30 m from that
    /// raid boss's lair, and `slay_raid_at`, `set_in_at` and `travel_at` do those at those frames
    pub(super) fn try_journey(&mut self, settings: &[(String, String)]) {
        let setting = |key: &str| settings.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
        let number = |key: &str| setting(key).and_then(|v| v.parse::<usize>().ok());
        // `--set time=22.5` and `day=3` hold for a game begun or gone on with, as for the world
        // first seen, see App::new(), rather than the time the game was at
        if let Some(day) = setting("day").and_then(|v| v.parse::<u32>().ok()) {
            self.cycle.clock.day = day;
        }
        if let Some(hour) = setting("time").and_then(|v| v.parse::<f64>().ok()) {
            self.cycle.set_hour(hour);
        }
        if let Some(realm) = number("realm") {
            self.set_out_for(realm);
        }
        // `--set wings=stormcaller` puts on wings of that kind, and `spare_wings=veilmoth` has a
        // pair in the pack too, see wings::KINDS
        for key in ["wings", "spare_wings"] {
            if let Some(kind) = setting(key).and_then(crate::wings::by_id) {
                self.game.find_once(Item::Wings(kind), "Wings, to try out.");
            }
        }
        if let (Some(count), Some(realm)) = (number("blocks"), realm_of(&self.session)) {
            self.game.give_innert_blocks(realm as u8, count as u32);
        }
        if setting("gate_window").is_some_and(|v| v == "1") {
            self.session.gate_open = true;
        }
        if let (Some(slot), Some(places)) = (number("near_raid"), self.session.places.clone()) {
            // On the near side of it, from the gate, stood on the ground once it's built round them
            let lair = places.lairs[slot.min(realms::RAIDS - 1)];
            let (dx, dy) = (places.gate[0] - lair[0], places.gate[1] - lair[1]);
            let d = dx.hypot(dy).max(1.0);
            let vpm = self.world.voxels_per_meter as f64;
            let spot = [lair[0] + dx / d * 30.0, lair[1] + dy / d * 30.0];
            let at = Vector3::new(spot[0] * vpm - self.world.origin[0] as f64, spot[1] * vpm - self.world.origin[1] as f64, self.world.size[2] as f64 - 2.0 * vpm);
            let mut player = Player::new(at, &self.world);
            player.position = at;
            self.player = Some(player);
            self.camera.position = voxels_to_camera(at, self.world.size);
            // Looking along -dx, -dy, at the lair
            self.camera.rotation.z = dx.atan2(-dy);
            self.camera.rotation.x = 0.1;
            self.session.find_ground = true;
            self.session.arriving = true;
            self.rebuild_world();
        }
        self.session.trial = Trial {
            slay_raid_at: number("slay_raid_at"),
            set_in_at: number("set_in_at"),
            travel_at: setting("travel_at").and_then(|v| v.split_once(':')).and_then(|(f, r)| Some((f.parse().ok()?, r.parse().ok()?))),
            shots_after: setting("shots_after").map_or(Vec::new(), |v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect()),
            end_after: number("end_after"),
            hook_at: number("hook_at"),
            glide_at: number("glide_at"),
            flap_after: number("flap_after"),
            flap_now: false,
            fall_at: number("fall_at"),
            face: setting("face").and_then(|v| v.parse().ok()),
            overlook: setting("overlook").and_then(|v| v.parse().ok()),
            glide: None,
            ready: None,
            there: false,
        };
    }

    /// What the trial does at frame `frame` of a screenshot run
    fn trial_step(&mut self, frame: usize) {
        // The world's there to play in once it's built and they're stood on it
        let there = !self.session.arriving && !self.session.find_ground && self.session.leaving.is_none();
        let trial = &mut self.session.trial;
        if there && !trial.there {
            let first = trial.ready.is_none();
            trial.ready.get_or_insert(frame);
            println!("READY at frame {frame}");
            if first {
                // `--set map=1` opens the map, see compass.rs
                if settings_from_args().iter().any(|(k, v)| k == "map" && v == "1") {
                    self.map.open = true;
                }
                if let Some(degrees) = trial.face {
                    self.camera.rotation.z = degrees.to_radians() - FRAC_PI_2;
                }
                // `--set look_up=30` looks that many degrees up from level, as at the moon
                if let Some(degrees) = settings_from_args().iter().find(|(k, _)| k == "look_up").and_then(|(_, v)| v.parse::<f64>().ok()) {
                    self.camera.rotation.x = degrees.to_radians();
                }
                if let Some(up) = trial.overlook
                    && let Some(player) = self.player.take()
                {
                    let vpm = self.world.voxels_per_meter as f64;
                    let eye = player.eye(&self.world) + Vector3::new(0.0, 0.0, up * vpm);
                    self.camera.position = voxels_to_camera(eye, self.world.size);
                    self.camera.rotation.x = -0.35;
                }
            }
            if let Some(capture) = &mut self.capture {
                if first && settings_from_args().iter().any(|(k, v)| k == "time_from_ready" && v == "1") {
                    capture.stats_from = frame;
                }
                capture.shots.extend(trial.shots_after.iter().map(|k| frame + k));
                if let Some(end) = trial.end_after {
                    capture.frames = (frame + end) as u32;
                }
            }
        }
        // Gone through to another realm meanwhile: the run goes on till it's there too
        if !there
            && trial.there
            && trial.end_after.is_some()
            && let Some(capture) = &mut self.capture
        {
            capture.frames = u32::MAX;
        }
        trial.there = there;
        let Some(ready) = trial.ready else {
            return;
        };
        let at = |when: Option<usize>| when.is_some_and(|w| ready + w == frame);
        let (slay, set_in, travel) = (at(trial.slay_raid_at), at(trial.set_in_at), trial.travel_at.filter(|(f, _)| ready + *f == frame));
        let (hook, glide, fall) = (at(trial.hook_at), at(trial.glide_at), at(trial.fall_at));
        if glide {
            trial.glide = Some(frame);
        }
        let gliding = trial.glide;
        // The hook taken in hand a moment before it's thrown, to be seen held ready
        if trial.hook_at.is_some_and(|w| w >= 20 && ready + w - 20 == frame) {
            self.game.hold_item(Item::Hook);
        }
        let hook_at = trial.hook_at;
        if hook {
            self.hook_nearest_island();
        }
        // Where the hook's taken them, now and then after
        if let Some(w) = hook_at
            && frame >= ready + w
            && (frame - ready - w) % 40 == 0
            && frame - ready - w <= 800
            && let Some(p) = &self.player
        {
            let vpm = self.world.voxels_per_meter as f64;
            let at = (p.position + Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0)) / vpm;
            println!("HOOKED frame {}: at {:.1}, {:.1}, {:.1} m, on the ground {}", frame - ready - w, at.x, at.y, at.z, p.on_ground());
        }
        // Lifted high over the island, wings spread, gliding on ahead for a while
        let vpm = self.world.voxels_per_meter as f64;
        if glide && let Some(player) = &mut self.player {
            player.position.z += 14.0 * vpm;
            // --set wings=stormcaller for that kind, see wings::KINDS
            let kind = settings_from_args().iter().find(|(k, _)| k == "wings").and_then(|(_, v)| crate::wings::by_id(v)).unwrap_or(crate::wings::GULL);
            self.game.find_once(Item::Wings(kind), "Wings, to glide with.");
            self.scripted_walk = true;
        }
        if let Some(since) = gliding
            && self.session.trial.flap_after.is_some_and(|f| frame == since + f)
        {
            self.session.trial.flap_now = true;
        }
        if let (Some(since), Some(player)) = (gliding, &self.player) {
            let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
            if (frame - since) % 20 == 0 {
                let at = (player.position + origin) / vpm;
                println!("GLIDE frame {}: at {:.1}, {:.1}, {:.1} m, gliding {}, on the ground {}", frame - since, at.x, at.y, at.z, player.gliding, player.on_ground());
            }
            if frame - since >= 160 || frame - since > 10 && player.on_ground() {
                self.session.trial.glide = None;
                self.scripted_walk = false;
            }
        }
        // Out over the open air, just past the island's edge the way the player looks
        if fall && let Some(player) = &mut self.player {
            let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
            let yaw = self.camera.rotation.z;
            let ahead = Vector3::new(-yaw.sin(), yaw.cos(), 0.0);
            let seed = self.terrain.seed;
            let feet = (player.position + origin) / vpm;
            if let Some(out) = (1..200).map(|k| feet + ahead * (k as f64 * 0.5)).find(|p| worldgen::sky_island(seed, p.x as f32, p.y as f32).is_none()) {
                player.position = (out + ahead * 2.0) * vpm - origin;
                println!("FALL off the edge at frame {frame}, at {:.1}, {:.1}, {:.1} m", out.x, out.y, feet.z);
            }
        }
        if slay {
            let near = self.player.as_ref().map_or(camera_to_voxels(self.camera.position, self.world.size), |p| p.position);
            let raids = self.game.raid_bosses();
            let slain = self.game.slay_raid_boss(&self.world, near);
            println!("BLOCK at frame {frame}: raid bosses about {:?}, one slain: {slain}; {}", raids.iter().map(|(l, _, h)| format!("{} {h:.0} health", l.name())).collect::<Vec<_>>(), self.game.last_message().unwrap_or(""));
        }
        if set_in && let Some(realm) = realm_of(&self.session) {
            self.gate_picked(GateAction::SetIn(realm));
        }
        if let Some((_, to)) = travel {
            println!("GATE at frame {frame}: going through to {}", REALMS[to.min(REALMS.len() - 1)].name);
            self.gate_picked(GateAction::Travel(to.min(REALMS.len() - 1)));
        }
    }

    /// The look of the realm the world is, for shaders\traverse.comp, see View::realm there: which
    /// realm's, where the dreaming realm's divide is across the world and where the world's corner
    /// is along it, in voxels, and how much of its blood-red half the camera's in, eased over a
    /// few tens of meters either side of the divide, so the sky turns as the player crosses it
    pub(super) fn realm_look(&self) -> [f32; 4] {
        let land = worldgen::land_of(self.terrain.seed);
        let which = match land {
            worldgen::Land::Normal => 0.0,
            worldgen::Land::Sky => 1.0,
            worldgen::Land::Dream => 2.0,
            worldgen::Land::Ice => 3.0,
            worldgen::Land::Water => 4.0,
        };
        if land != worldgen::Land::Dream {
            return [which, 0.0, 0.0, 0.0];
        }
        let vpm = self.world.voxels_per_meter as f64;
        let [at, wander, along, _] = worldgen::DREAM_DIVIDE;
        let eye = camera_to_voxels(self.camera.position, self.world.size);
        let (x, y) = ((eye.x + self.world.origin[0] as f64) / vpm, (eye.y + self.world.origin[1] as f64) / vpm);
        let line = at as f64 + wander as f64 * (y / along as f64).sin();
        let t = ((x - line) / 30.0 * 0.5 + 0.5).clamp(0.0, 1.0);
        let blood = t * t * (3.0 - 2.0 * t);
        [which, (at as f64 * vpm - self.world.origin[0] as f64) as f32, self.world.origin[1] as f32, blood as f32]
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
        // What's in it goes in as the world's built afresh, see stream.rs, and the raid, if it's
        // night, starts again
        self.raid = night::Raid::default();
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
            // Come to the sky isles, a grappling hook lies by the portal home, so every island's
            // within reach from the first
            if self.session.places.as_ref().is_some_and(|p| p.realm == realms::HOOK_REALM) {
                self.game.find_once(Item::Hook, "At the portal's foot lies a grappling hook, its rope coiled beside it. Take it in hand and click to throw it at an island's edge.");
            }
        }
        if !held {
            self.raise_finale();
        }
        // At the gate or a portal: its window opens, freeing the mouse for it
        if self.game.take_shrine() && self.session.places.is_some() && self.session.leaving.is_none() {
            self.session.gate_open = true;
            self.set_focus(false);
        }
        if let Some(action) = self.session.gate_action.take() {
            self.gate_picked(action);
        }
        // Gone through once the screen's dark
        if let Some((to, _)) = self.session.leaving
            && self.session.leaving_dark() >= 1.0
        {
            self.session.leaving = None;
            self.travel(to);
        }
        if let Some(frame) = self.capture.as_ref().map(|c| c.frame_times.len()) {
            self.trial_step(frame);
            self.serpent_trial(frame, self.session.trial.ready);
        }
        // The great serpents' lairs come across, now and then, see serpent_lairs.rs
        if self.frame % 30 == 0 {
            self.find_serpent_lairs();
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
        // Which of the realm's raid bosses are beaten
        let raids: Vec<bool> = self.session.places.as_ref().map_or(Vec::new(), |p| (0..realms::RAIDS).map(|k| self.game.is_defeated(&p.lair_key(k))).collect());
        format!(
            "mode={} seed={} world={} at={:.2},{:.2},{:.2} walking={} {} craters={} stains={} raids_beaten={raids:?} realms={:?} built={} carried={:?} time={:.2} day={}",
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

/// The window of the Innert Gate, or of the portal home in realm `here` past the normal world:
/// each realm's three sockets, how many of its blocks are set in and how many carried, with
/// setting them in and going through to the realms open at the gate, or going home at a portal.
/// What's picked is given back.
pub fn gate_window(ctx: &egui::Context, here: usize, progress: &save::RealmsSave, carried: [u32; realms::REALM_COUNT]) -> Option<GateAction> {
    let mut picked = None;
    let gray = Color32::from_gray(185);
    let (_, restored) = realms::opened(&progress.placed);
    let title = if here == 0 { "The Innert Gate" } else { "Portal Home" };
    egui::Window::new(title).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).fixed_size([560.0, 0.0]).show(ctx, |ui| {
        if here == 0 {
            ui.label(RichText::new("An ancient gate of dark stone, older than any realm's kings. Each realm's three raid bosses guard an Innert block. Set a realm's three into the gate's sockets, and the way to the next realm opens.").size(14.0).color(gray));
        } else {
            ui.label(RichText::new(format!("The way home from {}, back to the Innert Gate in the normal world. Bring this realm's blocks to the gate.", REALMS[here].name)).size(14.0).color(gray));
        }
        ui.add_space(10.0);
        for (r, realm) in REALMS.iter().enumerate() {
            let placed = progress.placed.get(r).copied().unwrap_or(0) as usize;
            let open = progress.unlocked.contains(&(r as u32));
            let [cr, cg, cb] = realm.color;
            let color = Color32::from_rgb(cr, cg, cb);
            ui.horizontal(|ui| {
                // Its three sockets, lit where a block's set in
                for k in 0..realms::RAIDS {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 3.0, Color32::from_gray(40));
                    ui.painter().rect_filled(rect.shrink(if k < placed { 2.0 } else { 6.0 }), 2.0, if k < placed { color } else { color.gamma_multiply(0.25) });
                }
                ui.add_space(6.0);
                ui.label(RichText::new(realm.name).size(17.0).strong().color(if open { color } else { Color32::from_gray(120) }));
                let state = match (open, placed >= realms::RAIDS) {
                    (false, _) => "sealed".to_owned(),
                    (true, true) => "its blocks all set in".to_owned(),
                    (true, false) if r == here => format!("here · {placed} of {} set in", realms::RAIDS),
                    (true, false) => format!("{placed} of {} set in", realms::RAIDS),
                };
                ui.label(RichText::new(state).size(13.0).color(gray));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if here == 0 && open && r != 0 && ui.button(RichText::new("Travel").size(15.0)).clicked() {
                        picked = Some(GateAction::Travel(r));
                    }
                    let can = carried[r].min((realms::RAIDS - placed.min(realms::RAIDS)) as u32);
                    if here == 0 && can > 0 && ui.button(RichText::new(format!("Set in {can}")).size(15.0)).clicked() {
                        picked = Some(GateAction::SetIn(r));
                    }
                    if carried[r] > 0 {
                        ui.label(RichText::new(format!("{} carried", carried[r])).size(13.0).color(color));
                    }
                });
            });
            ui.add_space(4.0);
        }
        if restored {
            ui.add_space(6.0);
            ui.label(RichText::new("All realms restored. The gate burns with the light of every one of them.").size(16.0).strong().color(Color32::from_rgb(255, 220, 140)));
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if here != 0 && ui.button(RichText::new("Return to the gate").size(16.0)).clicked() {
                picked = Some(GateAction::Travel(0));
            }
            if ui.button(RichText::new("Close").size(16.0)).clicked() {
                picked = Some(GateAction::Close);
            }
        });
    });
    picked
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
