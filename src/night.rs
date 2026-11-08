//! Night raids as they play out, see raids.rs: from the dusk, waves of monsters come from behind
//! the player, out of sight, and hunt them down; round a base they're led round its walls to its
//! doors, and beat on them, the doors cracking and splintering till they break. At dawn those
//! still about catch fire and burn away.

use egui_winit_vulkano::egui::{Align2, Color32, FontId, pos2, vec2};

use crate::bases::BLOCK;
use crate::raids::{self, Lure, Nav};

use super::*;

/// Seconds between finding the way round what's built again
const NAV_SECONDS: f64 = 0.5;
/// How much of a monster's blow a door takes, and a wall, from those that can break walls at all
const DOOR_TAKES: f64 = 0.8;
const WALL_TAKES: f64 = 0.5;
/// Seconds a door that's been struck shows on the HUD
const STRUCK_SECONDS: f64 = 4.0;

/// How the night's raid is going
#[derive(Default)]
pub struct Raid {
    pub on: bool,
    /// Which night it is, the first 0, for how hard it is
    night: u32,
    waves: u32,
    /// Seconds till the next wave
    pub next_wave: f64,
    nav: Option<Nav>,
    nav_wait: f64,
    /// The piece last struck, and seconds since, for the HUD
    struck: Option<(u32, f64)>,
    /// Seconds the raiders left at dawn have been burning
    burning: Option<f64>,
    rng: u32,
    /// Raiders about, and how many have come in all
    pub about: usize,
    pub came: usize,
}

fn random(rng: &mut u32) -> f64 {
    *rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    let mut x = *rng;
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    (x >> 8) as f64 / 16_777_216.0
}

impl App {
    /// Which realm the player's in, those they've been to, and how many beyond the first are open,
    /// for who comes at night and how many, see raids.rs. Free Roam is the normal world.
    fn realm_progress(&self) -> (raids::RealmId, Vec<raids::RealmId>, u32) {
        match &self.session.realms {
            Some(r) => (r.realm, r.visited.clone(), r.unlocked.len().saturating_sub(1) as u32),
            None => (0, vec![0], 0),
        }
    }

    /// Starts and ends the night's raid with the dusk and the dawn, sends its waves, leads the
    /// raiders round walls to the doors and has what's built take their blows. Called each frame
    /// the world's going on, after the game's had its turn.
    pub(super) fn update_raid(&mut self, dt: f64) {
        // `--set raids=0` for nights without them, to compare
        static RAIDS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let raids = *RAIDS.get_or_init(|| !settings_from_args().iter().any(|(k, v)| k == "raids" && v == "0"));
        let out = raids && !self.fixed_sun && self.cycle.monsters_out();
        if out && !self.raid.on {
            self.raid.on = true;
            self.raid.night = self.cycle.clock.day;
            self.raid.waves = 0;
            // The first wave a little into the dusk, or soon if it's night already
            let into_dusk = self.cycle.part() == daynight::Part::Dusk && self.cycle.till_night() > daynight::DUSK_SECONDS - 5.0;
            self.raid.next_wave = if into_dusk { raids::FIRST_WAVE_SECONDS } else { 3.0 };
            self.raid.rng ^= self.cycle.clock.day.wrapping_mul(0x9e37_79b9) ^ self.terrain.seed;
            if let Some(capture) = &self.capture {
                println!("RAID begins at frame {}, night {}", capture.frame_times.len(), self.raid.night + 1);
            }
        } else if !out && self.raid.on {
            self.raid_ends();
        }
        if self.raid.on {
            self.raid.next_wave -= dt;
            let (_, _, progress) = self.realm_progress();
            let plan = raids::plan(self.raid.night, progress);
            if self.raid.next_wave <= 0.0 && self.raid.waves < plan.waves {
                self.raid.next_wave = raids::WAVE_SECONDS;
                self.raid.waves += 1;
                self.send_wave(plan);
            }
        }
        self.lead_raiders(dt);
        self.blows_on_bases();
        // Those left at dawn burn away; any still about after go
        if let Some(t) = &mut self.raid.burning {
            *t += dt;
            if *t > raids::BURN_SECONDS + 2.0 {
                self.raid.burning = None;
                for a in self.game.animals_mut() {
                    if a.boss.as_ref().is_some_and(|b| b.raider) && a.health > 0.0 {
                        a.health = 0.0;
                    }
                }
            }
        }
        if let Some((_, t)) = &mut self.raid.struck {
            *t += dt;
            if *t > STRUCK_SECONDS {
                self.raid.struck = None;
            }
        }
    }

    /// The dawn: the raiders still about catch fire and burn away, and the night's been seen out
    fn raid_ends(&mut self) {
        self.raid.on = false;
        let mut burning = 0;
        for a in self.game.animals_mut() {
            if let Some(boss) = a.boss.as_mut().filter(|b| b.raider)
                && a.health > 0.0
            {
                boss.lure = None;
                a.status.ignite(raids::BURN_SECONDS, a.max_health / raids::BURN_SECONDS * 1.3);
                burning += 1;
            }
        }
        self.raid.burning = Some(0.0);
        self.bases.nights_survived += 1;
        let survived = self.bases.nights_survived;
        self.game.say(if burning > 0 { format!("The dawn burns the raiders away. Nights seen out: {survived}.") } else { format!("You saw the night out. Nights seen out: {survived}.") });
        let view = self.view();
        if let Some(audio) = &mut self.audio
            && burning > 0
        {
            audio.play(audio::Cue::here(audio::Sound::Sear, 0.6), &view);
        }
        if let Some(capture) = &self.capture {
            println!("RAID ends at frame {} with {burning} raiders left burning", capture.frame_times.len());
        }
        self.save_game(false);
    }

    /// A wave of the night's monsters, from behind the player, out of sight, as many as `plan` has
    /// come at once and no more than it lets be about
    fn send_wave(&mut self, plan: raids::Plan) {
        let (here, visited, _) = self.realm_progress();
        let roster = raids::night_roster(here, &visited);
        let count = plan.per_wave.min((plan.most as usize).saturating_sub(self.raid.about) as u32);
        if count == 0 || roster.is_empty() {
            return;
        }
        let vpm = self.world.voxels_per_meter as f64;
        let feet = self.player.as_ref().map_or(camera_to_voxels(self.camera.position, self.world.size), |p| p.position);
        // Behind where they're looking, the way the camera faces turned round
        let yaw = self.camera.rotation.z;
        let behind = (-yaw.cos()).atan2(yaw.sin());
        let [sx, sy, _] = self.world.size.map(|n| n as f64);
        // Somewhere dry on the ground behind them, trying round a little if it's water there
        let mut spot = None;
        for attempt in 0..10 {
            let a = behind + (random(&mut self.raid.rng) - 0.5) * (1.6 + attempt as f64 * 0.3);
            let d = (raids::SPAWN_NEAR_M + random(&mut self.raid.rng) * (raids::SPAWN_FAR_M - raids::SPAWN_NEAR_M)) * vpm;
            let (x, y) = (feet.x + a.cos() * d, feet.y + a.sin() * d);
            if x < 8.0 || y < 8.0 || x > sx - 8.0 || y > sy - 8.0 {
                continue;
            }
            let Some(top) = (0..self.world.size[2] as i64).rev().find(|&z| !worldgen::is_passable(self.world.material_at(x as i64, y as i64, z))) else { continue };
            let ground = top as f64 + 1.0;
            if self.water.surface_at(x, y) < ground + 0.2 * vpm {
                spot = Some(Vector3::new(x, y, ground));
                break;
            }
        }
        let Some(at) = spot else {
            return;
        };
        let mut looks = Vec::new();
        let mut tough = Vec::new();
        for _ in 0..count {
            let (look, t) = raids::pick(&roster, random(&mut self.raid.rng), plan.brutes);
            looks.push(look);
            tough.push(t * plan.toughness);
        }
        let water = &self.water;
        let seed = self.raid.rng;
        let pack = animals::spawn_pack(&self.world, &|x, y| water.surface_at(x, y), &looks, at, 3.0, seed);
        let came = pack.len();
        for (mut monster, t) in pack.into_iter().zip(tough) {
            if let Some(boss) = &mut monster.boss {
                boss.raider = true;
                boss.engaged = true;
            }
            monster.toughen(t);
            self.game.add_animal(monster);
        }
        self.raid.came += came;
        if self.raid.waves == 1 && came > 0 {
            self.game.say("Something's coming out of the dark.");
        }
        if let Some(capture) = &self.capture {
            println!("RAID wave {} at frame {}: {came} of {looks:?} {:.0} m off", self.raid.waves, capture.frame_times.len(), (at - feet).norm() / vpm);
        }
    }

    /// Leads each raider round what's built to the player, or to the door in their way, see Nav
    fn lead_raiders(&mut self, dt: f64) {
        let vpm = self.world.voxels_per_meter as f64;
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        self.raid.nav_wait -= dt;
        if self.raid.nav_wait <= 0.0 {
            self.raid.nav_wait = NAV_SECONDS;
            let feet = self.player.as_ref().map_or(camera_to_voxels(self.camera.position, self.world.size), |p| p.position);
            let block = [0, 1, 2].map(|i| ((feet[i] + origin[i]) / BLOCK as f64).floor() as i64);
            self.raid.nav = Nav::new(&self.bases, block);
        }
        let mut about = 0;
        let nav = &self.raid.nav;
        let bases = &self.bases;
        for a in self.game.animals_mut() {
            let Some(boss) = a.boss.as_mut().filter(|b| b.raider) else { continue };
            if a.health <= 0.0 {
                continue;
            }
            about += 1;
            let at = a.position + origin;
            boss.lure = nav.as_ref().and_then(|nav| nav.lure([at.x, at.y, at.z], bases)).map(|lure| match lure {
                Lure::Path(p) => (Vector3::new(p[0], p[1], at.z) - origin, false),
                Lure::Break { at: p, .. } => (Vector3::new(p[0], p[1], p[2]) - origin, true),
            });
            let _ = vpm;
        }
        self.raid.about = about;
    }

    /// What's built takes the blows bosses and monsters landed near it: doors and lights from any
    /// of them, walls only from the biggest. Doors bang and crack, and break apart.
    fn blows_on_bases(&mut self) {
        let blows = self.game.take_blows();
        if blows.is_empty() || self.bases.pieces().is_empty() {
            return;
        }
        let vpm = self.world.voxels_per_meter as f64;
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let mut changed = false;
        for blow in blows {
            let at = blow.at + origin;
            let reach = (blow.radius + 0.3) * vpm;
            let big = raids::breaks_walls(blow.look);
            let hit: Vec<(u32, bool, [f64; 3])> = self
                .bases
                .pieces()
                .iter()
                .filter(|p| p.kind.weak() || big)
                .filter_map(|p| {
                    let near = p.blocks().iter().map(|b| (Vector3::new(b[0] as f64 + 0.5, b[1] as f64 + 0.5, b[2] as f64 + 0.5) * BLOCK as f64 - at).norm()).fold(f64::MAX, f64::min);
                    (near < reach).then(|| (p.id, p.kind == bases::Kind::Door, p.middle()))
                })
                .collect();
            for (id, door, middle) in hit {
                let weak = self.bases.piece(id).is_some_and(|p| p.kind.weak());
                let amount = blow.damage * if weak { DOOR_TAKES } else { WALL_TAKES };
                let material = self.bases.piece(id).map_or(worldgen::material::WOOD, |p| p.kind.material());
                let Some((broken, _, _)) = self.bases.damage(id, amount) else { continue };
                changed = true;
                self.raid.struck = Some((id, 0.0));
                let view = self.view();
                let local = Vector3::from(middle) - origin;
                self.game.debris(local, &[material, worldgen::material::DEAD_WOOD, material], vpm);
                if let Some(audio) = &mut self.audio {
                    audio.play(audio::Cue::at(audio::Sound::Bang, local, 1.0).pitched(if door { 1.0 } else { 0.8 }), &view);
                }
                if broken {
                    self.game.debris(local, &[material; 12], vpm);
                    self.game.debris(local + Vector3::new(0.0, 0.0, 0.6 * vpm), &[worldgen::material::DEAD_WOOD, worldgen::material::BARK_DARK, material, material, worldgen::material::DEAD_WOOD, material], vpm);
                    if let Some(audio) = &mut self.audio {
                        audio.play(audio::Cue::at(audio::Sound::Splinter, local, 1.0), &view);
                    }
                    let what = self.bases.piece(id).map_or("Something", |p| p.kind.name());
                    self.game.say(if door { "Your door's been broken down!".to_string() } else { format!("Your {} was smashed!", what.to_lowercase()) });
                    self.raid.struck = None;
                    if let Some(capture) = &self.capture {
                        println!("BROKEN at frame {}: {} by a {}", capture.frame_times.len(), if door { "a door" } else { "a wall" }, blow.look.name());
                    }
                }
            }
        }
        if changed {
            self.refresh_bases();
        }
    }

    /// What the HUD says of the raid, under the clock
    pub(super) fn raid_note(&self) -> Option<String> {
        if !self.raid.on && self.raid.burning.is_none() {
            return None;
        }
        Some(match self.raid.about {
            0 if self.raid.on => "Raid: none about yet".to_string(),
            0 => "The raiders burn".to_string(),
            1 => "Raid: 1 about".to_string(),
            n => format!("Raid: {n} about"),
        })
    }
}

/// A door being beaten on: a warning, and how much of it's left, over it on the screen
pub fn hud(raid: &Raid, bases: &bases::Bases, world: &World, ctx: &egui::Context, view: &combat::View) {
    let Some((id, since)) = raid.struck else {
        return;
    };
    let Some(piece) = bases.piece(id) else {
        return;
    };
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("raid")));
    let screen = ctx.screen_rect();
    let t = ctx.input(|i| i.time) as f32;
    let fade = (1.0 - (since as f32 - STRUCK_SECONDS as f32 + 1.0).max(0.0)).clamp(0.0, 1.0);
    let left = (piece.health / piece.kind.health()).clamp(0.0, 1.0) as f32;
    let what = if piece.kind == bases::Kind::Door { "Your door's under attack!" } else { "They're breaking through!" };
    let pulse = 0.75 + 0.25 * (t * 6.0).sin();
    painter.text(pos2(screen.center().x, screen.center().y - 150.0), Align2::CENTER_CENTER, what, FontId::proportional(20.0), Color32::from_rgb(255, 90, 70).gamma_multiply(fade * pulse));
    // Its health over it, where it is
    let origin = Vector3::new(world.origin[0] as f64, world.origin[1] as f64, 0.0);
    let top = Vector3::from(piece.middle()) - origin + Vector3::new(0.0, 0.0, 1.3 * view.vpm);
    if let Some([u, v]) = view.project(top) {
        let at = pos2(screen.left() + u as f32 * screen.width(), screen.top() + v as f32 * screen.height());
        let back = egui::Rect::from_center_size(at, vec2(90.0, 9.0));
        painter.rect_filled(back.expand(2.0), 4.0, Color32::from_black_alpha((160.0 * fade) as u8));
        let color = if left > 0.5 { Color32::from_rgb(230, 190, 90) } else { Color32::from_rgb(230, 70, 50) };
        painter.rect_filled(egui::Rect::from_min_size(back.min, vec2(90.0 * left, 9.0)), 3.0, color.gamma_multiply(fade));
    }
}
