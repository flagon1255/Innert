//! Fireplaces, built in building (B), see bases.rs and building.rs: a stone hearth a meter wide and
//! high, lined with fired clay, with chimney pieces stacked on it like walls. E with wood carried
//! feeds it a log, three minutes of burning each and no more than six at once; it burns down as
//! time goes by, its flames higher the more it has, and goes out when it's spent. Lit, it gives a
//! flickering warm light brighter than a lantern's, embers drifting up and smoke rising out of the
//! top of its chimney, or out into the room without one, and crackles. Holding raw meat, E roasts
//! it on the fire. Standing by it heals, once out of the thick of a fight, and while one burns near
//! the player at night, each wave of raiders comes one fewer, see night.rs. How much it has left
//! to burn is kept with what's built, so it's saved with the base.
//!
//! Its light and smoke are cheap: the light's one more of the lights there are, only while it's
//! lit, the smoke and embers a few puffs a second of what flies about already, and only near the
//! camera; its voxels change only as its flames grow or settle, every few minutes at most.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Instant;

use crate::bases::{BLOCK, Bases, Kind, Piece, facing};
use crate::worldgen::material::*;

use super::*;

/// Seconds of burning a log gives, and the most a fire can be stocked with
pub const LOG_SECONDS: f64 = 180.0;
pub const MOST_SECONDS: f64 = 6.0 * LOG_SECONDS;
/// How near the fire to stand, in meters, to be warmed by it and healed, and to roast on it
const WARM_M: f64 = 3.5;
/// Health it gives back a second by it, once SAFE_SECONDS have gone by since the player was hurt
pub const HEAL_PER_SECOND: f64 = 2.0;
pub const SAFE_SECONDS: f64 = 2.0;
/// How near a fire burning at night has to be to the player for the raiders to come fewer at a
/// time, in meters
pub const WARDING_M: f64 = 25.0;
/// How far off its smoke and embers are let loose, and its crackle heard, in meters
const SMOKE_M: f64 = 80.0;
const CRACKLE_M: f64 = 22.0;

/// How a fire with `fuel` seconds left looks: 0 out, 1 low, settling, 2 burning well, 3 roaring
pub fn stage(fuel: f64) -> u8 {
    if fuel <= 0.0 {
        0
    } else if fuel < LOG_SECONDS {
        1
    } else if fuel < 3.0 * LOG_SECONDS {
        2
    } else {
        3
    }
}

/// A voxel of a fireplace: `w` across its width, 0-15, `v` up it, 0-15, `d` from its back to its
/// front, 0-7, its fire at `stage`, `r` a hash of where it is. A stone hearth slab, a back and sides
/// of stone brick lined with fired clay inside, a lintel with a wooden mantel over the opening,
/// sooted above it, and in it two logs crossed on a bed of ash, charred when it's out, glowing
/// embers and flames as high as the fire's stocked when it's lit.
pub fn hearth_voxel(w: i64, v: i64, d: i64, stage: u8, r: u32) -> u8 {
    let brick = |r: u32| [BRICK_STONE, BRICK_STONE, STONE, BRICK_STONE][(r % 4) as usize];
    let opening_w = (3..=12).contains(&w);
    // The slab it stands on
    if v <= 1 {
        return if v == 1 && opening_w && d >= 2 && r % 3 == 0 { GRAVEL } else { brick(r) };
    }
    // The lintel over the opening, and the mantel across the front of it
    if v >= 12 {
        return if v == 12 && d >= 6 { DEAD_WOOD } else { brick(r) };
    }
    // The back, and the sides round the opening, lined with clay inside, mortared courses outside
    if d <= 1 || !opening_w || v >= 11 {
        let inner = (d == 2 || w == 3 || w == 12 || v == 10) && opening_w;
        if d <= 1 && opening_w && d == 1 {
            return if v >= 7 { OBSIDIAN } else { CLAY_POT };
        }
        if inner {
            return CLAY_POT;
        }
        return if v % 4 == 0 && r % 5 != 0 { MORTAR } else { brick(r) };
    }
    // Inside: the logs, crossed, lying on the ash
    let (cx, cd) = (w as f64 - 7.5, d as f64 - 4.0);
    let log_a = v <= 3 && (cx - cd * 1.2).abs() < 1.0 && cx.abs() < 4.5;
    let log_b = v <= 3 && (cx + cd * 1.2).abs() < 1.0 && cx.abs() < 4.5;
    if log_a || log_b {
        return match stage {
            0 => if r % 3 == 0 { OBSIDIAN } else { DEAD_WOOD },
            _ if v == 3 && r % 2 == 0 => GLOW_FIRE,
            _ => if r % 4 == 0 { BARK_DARK } else { WOOD },
        };
    }
    // Embers glowing in the ash between them, or cold ash
    if v == 2 && (3..=6).contains(&d) && cx.abs() < 4.0 {
        return match stage {
            0 => if r % 2 == 0 { GRAVEL } else { AIR },
            _ if r % 3 != 0 => GLOW_FIRE,
            _ => GRAVEL,
        };
    }
    // Flames rising off them, as high as the fire's stocked, tapering and ragged
    if stage > 0 && v >= 3 {
        let top = [0.0, 5.5, 7.5, 9.5][stage as usize];
        let height = v as f64 - 3.0;
        let reach = 3.8 * (1.0 - height / top).max(0.0);
        let ragged = (r % 7) as f64 * 0.25;
        if cx.abs() + (d as f64 - 4.0).abs() * 0.8 < reach - ragged * height / top && height < top {
            return FLAME;
        }
    }
    AIR
}

/// A voxel of a chimney piece: `w` across its width, 0-15, `gz` up the whole landscape, `d` from
/// its back to its front, 0-7: a flue of stone brick in mortared courses, hollow inside
pub fn chimney_voxel(w: i64, gz: i64, d: i64, r: u32) -> u8 {
    let wall = !(3..=12).contains(&w) || !(2..=5).contains(&d);
    if !wall {
        return AIR;
    }
    if gz.rem_euclid(4) == 0 && r % 5 != 0 {
        MORTAR
    } else {
        [BRICK_STONE, BRICK_STONE, STONE, SANDSTONE_DARK][(r % 4) as usize]
    }
}

/// Seconds since the game started, for the light's flicker
fn now() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

/// Where a fireplace's fire is, in voxels of the whole landscape: in the middle of its opening,
/// a little up off the ash
pub fn fire_at(piece: &Piece) -> Vector3<f64> {
    let m = Vector3::from(piece.middle());
    let f = facing(piece.turn);
    m + Vector3::new(f[0] as f64, f[1] as f64, 0.0) * 1.0 - Vector3::new(0.0, 0.0, 3.0)
}

/// The fireplaces burning, with how much each has left, from `bases`
pub fn lit(bases: &Bases) -> impl Iterator<Item = &Piece> {
    bases.pieces().iter().filter(|p| p.kind == Kind::Hearth && p.fuel > 0.0)
}

/// The lights of the fires burning, in voxels of `world`, as App::lights has them: warm, out a
/// little in front of the opening so its sides don't hide it, reaching further the higher the fire,
/// flickering
pub fn lights(bases: &Bases, world: &World) -> Vec<[f32; 8]> {
    let vpm = world.voxels_per_meter as f64;
    let origin = Vector3::new(world.origin[0] as f64, world.origin[1] as f64, 0.0);
    let t = now();
    lit(bases)
        .map(|p| {
            let f = facing(p.turn);
            let at = fire_at(p) + Vector3::new(f[0] as f64, f[1] as f64, 0.0) * 6.0 + Vector3::new(0.0, 0.0, 2.0) - origin;
            let seed = p.id as f64 * 1.7;
            // Two slow waves and a quick one, so it never settles into a beat
            let flicker = 1.0 + 0.1 * (t * 7.3 + seed).sin() + 0.07 * (t * 13.1 + seed * 2.3).sin() + 0.05 * (t * 23.7 + seed).sin();
            let strength = [0.0, 0.85, 1.0, 1.15][stage(p.fuel) as usize] * flicker;
            let reach = (9.0 + stage(p.fuel) as f64) * vpm;
            [at.x as f32, at.y as f32, at.z as f32, reach as f32, strength as f32, (0.48 * strength) as f32, (0.17 * strength) as f32, 0.0]
        })
        .collect()
}

/// The top of the chimney stacked on fireplace `piece`, if there's one, in voxels of the whole
/// landscape: its last piece's, in the middle of its flue
fn chimney_top(bases: &Bases, piece: &Piece) -> Option<Vector3<f64>> {
    let mut top = None;
    let mut z = piece.at[2] + 2;
    while let Some(c) = bases.piece_at([piece.at[0], piece.at[1], z]).filter(|c| c.kind == Kind::Chimney && c.at == [piece.at[0], piece.at[1], z]) {
        let m = Vector3::from(c.middle());
        top = Some(Vector3::new(m.x, m.y, ((c.at[2] + 2) * BLOCK) as f64));
        z += 2;
    }
    top
}

/// What's kept about the fires between frames: when each last let smoke and embers loose and
/// crackled, and whether the raiders were said to be kept at bay tonight
#[derive(Default)]
pub struct Fires {
    smoke: HashMap<u32, f64>,
    embers: HashMap<u32, f64>,
    crackle: HashMap<u32, f64>,
    pub warded_said: bool,
}

impl App {
    /// The fireplace the player's looking at, near enough to reach
    fn hearth_looked_at(&self) -> Option<&Piece> {
        let (eye, look) = self.eye_look();
        let vpm = self.world.voxels_per_meter as f64;
        let mut t = 0.0;
        while t < 4.0 * vpm {
            let p = eye + look * t;
            let v = [p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64];
            let block = [(v[0] + self.world.origin[0]).div_euclid(BLOCK), (v[1] + self.world.origin[1]).div_euclid(BLOCK), (v[2] + self.world.origin[2]).div_euclid(BLOCK)];
            if let Some(piece) = self.bases.piece_at(block).filter(|p| p.kind == Kind::Hearth) {
                return Some(piece);
            }
            let m = self.world.material_at(v[0], v[1], v[2]);
            if !worldgen::is_passable(m) && m != FLAME && m != GLOW_FIRE {
                return None;
            }
            t += 0.5;
        }
        None
    }

    /// E at a fireplace: roasts the raw meat in hand on it if it's lit, or feeds it a log of the
    /// wood carried. Whether the player was at one.
    pub(super) fn hearth_here(&mut self) -> bool {
        let Some((id, fuel, at)) = self.hearth_looked_at().map(|p| (p.id, p.fuel, fire_at(p))) else {
            return false;
        };
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let view = self.view();
        if fuel > 0.0 && self.game.holding_raw_meat() {
            let left = self.game.cook_selected();
            self.game.say(match left {
                0 => "You roast the meat over the fire.".to_owned(),
                n => format!("You roast a piece of meat over the fire. {n} raw left."),
            });
            if let Some(audio) = &mut self.audio {
                audio.play(audio::Cue::at(audio::Sound::Crackle, at - origin, 0.9).pitched(1.45), &view);
            }
            // The fat spitting sparks up off the fire
            let vpm = self.world.voxels_per_meter as f64;
            for k in 0..6 {
                let a = k as f64 * 1.05;
                self.game.wisp(at - origin + Vector3::new(0.0, 0.0, 3.0), Vector3::new(a.cos() * 0.3, a.sin() * 0.3, 1.0 + 0.1 * k as f64) * vpm, 0.8, true);
            }
            return true;
        }
        if fuel >= MOST_SECONDS - LOG_SECONDS * 0.5 {
            self.game.say("The fire's stocked as well as it can be.");
            return true;
        }
        if self.bases.materials.wood == 0 {
            self.game.say(if fuel > 0.0 { "You've no wood to feed the fire. Fell a tree (G, or a hatchet)." } else { "It takes a log to light. Fell a tree for wood (G, or a hatchet)." });
            return true;
        }
        self.bases.materials.wood -= 1;
        let Some((was_lit, changed)) = self.bases.add_fuel(id, LOG_SECONDS, MOST_SECONDS) else {
            return true;
        };
        if changed {
            self.refresh_bases();
        }
        let left = self.bases.piece(id).map_or(0.0, |p| p.fuel);
        self.game.say(if was_lit { format!("You put a log on the fire: {} left.", minutes(left)) } else { format!("You light the fire: it'll burn {}.", minutes(left)) });
        if let Some(audio) = &mut self.audio {
            audio.play(audio::Cue::at(audio::Sound::Place, at - origin, 0.6).pitched(0.8), &view);
            audio.play(audio::Cue::at(audio::Sound::Crackle, at - origin, 0.8), &view);
        }
        true
    }

    /// What the HUD says about the fireplace looked at: whether it's burning and how long for, and
    /// what E does at it
    pub(super) fn hearth_note(&self) -> Option<String> {
        let piece = self.hearth_looked_at()?;
        Some(if piece.fuel > 0.0 {
            let roast = if self.game.holding_raw_meat() { "E  roast the meat" } else { "E  add a log" };
            format!("Fire burning, {} left\n{roast} ({} wood)", minutes(piece.fuel), self.bases.materials.wood)
        } else {
            format!("Fireplace, unlit\nE  light it with a log ({} wood)", self.bases.materials.wood)
        })
    }

    /// Called each frame: the fires burn down; near the camera they smoke, throw up embers and
    /// crackle; and standing by one heals
    pub(super) fn update_hearths(&mut self, dt: f64) {
        if self.bases.burn(dt) {
            self.refresh_bases();
        }
        let fires: Vec<(u32, u8, Vector3<f64>, Option<Vector3<f64>>)> = lit(&self.bases).map(|p| (p.id, stage(p.fuel), fire_at(p), chimney_top(&self.bases, p))).collect();
        if fires.is_empty() {
            return;
        }
        let vpm = self.world.voxels_per_meter as f64;
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let eye = camera_to_voxels(self.camera.position, self.world.size);
        let feet = self.player.as_ref().map(|p| p.position);
        let view = self.view();
        let mut warm = false;
        for (id, stage, at, top) in fires {
            let at = at - origin;
            let far = (at - eye).norm() / vpm;
            if let Some(feet) = feet
                && (feet - at).xy().norm() < WARM_M * vpm
                && (feet.z - at.z).abs() < 2.5 * vpm
            {
                warm = true;
            }
            if far > SMOKE_M {
                continue;
            }
            let hash = |k: u64| (bases::hash3(id, (now() * 1000.0) as u32, k as u32) % 1000) as f64 / 1000.0;
            // Smoke out of the top of the chimney, or curling out of the opening into the room
            let smoke = self.building.fires.smoke.entry(id).or_insert(0.0);
            *smoke -= dt;
            if *smoke <= 0.0 {
                // Billows out of a chimney, and fewer drifting out into the room without one
                *smoke = [0.0, 0.9, 0.6, 0.45][stage as usize] * if top.is_some() { 1.0 } else { 2.5 };
                let (from, rise) = match top {
                    Some(top) => (top - origin + Vector3::new(0.0, 0.0, 4.0), 0.8),
                    None => (at + Vector3::new(0.0, 0.0, 7.0), 0.3),
                };
                let drift = Vector3::new(hash(1) - 0.5, hash(2) - 0.5, 0.0) * 0.4 + Vector3::new(0.12, 0.05, rise);
                self.game.wisp(from, drift * vpm, 4.0, false);
            }
            // Embers flicking up off the fire
            let ember = self.building.fires.embers.entry(id).or_insert(0.0);
            *ember -= dt;
            if *ember <= 0.0 {
                *ember = 0.15 + 0.5 * hash(3) / stage.max(1) as f64;
                let from = at + Vector3::new((hash(4) - 0.5) * 6.0, (hash(5) - 0.5) * 6.0, 2.0);
                self.game.wisp(from, Vector3::new((hash(6) - 0.5) * 0.3, (hash(7) - 0.5) * 0.3, 0.6 + 0.6 * hash(8)) * vpm, 0.9 + 0.8 * hash(9), true);
            }
            // Crackling, louder the bigger it is
            if far < CRACKLE_M {
                let crackle = self.building.fires.crackle.entry(id).or_insert(0.0);
                *crackle -= dt;
                if *crackle <= 0.0 {
                    *crackle = 2.2 + 1.2 * hash(10);
                    let volume = [0.0, 0.45, 0.6, 0.75][stage as usize] as f32;
                    if let Some(audio) = &mut self.audio {
                        audio.play(audio::Cue::at(audio::Sound::Crackle, at, volume).pitched(0.9 + 0.2 * hash(11) as f32), &view);
                    }
                }
            }
        }
        if warm {
            self.game.warm(dt, HEAL_PER_SECOND, SAFE_SECONDS);
        }
    }

    /// Whether a fire's burning near the player, so the night's raiders come fewer at a time,
    /// see night.rs
    pub(super) fn fire_wards(&self) -> bool {
        let vpm = self.world.voxels_per_meter as f64;
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let Some(feet) = self.player.as_ref().map(|p| p.position + origin) else {
            return false;
        };
        lit(&self.bases).any(|p| (fire_at(p) - feet).norm() < WARDING_M * vpm)
    }
}

/// `seconds` as minutes and seconds
fn minutes(seconds: f64) -> String {
    let s = seconds.round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The note about the fireplace looked at, under the middle of the screen
pub fn hud(ctx: &egui::Context, note: Option<&str>) {
    let Some(note) = note else {
        return;
    };
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("hearth")));
    let at = ctx.screen_rect().center() + egui::vec2(0.0, 120.0);
    painter.rect_filled(egui::Rect::from_center_size(at + egui::vec2(0.0, 9.0), egui::vec2(300.0, 52.0)), 8.0, egui::Color32::from_black_alpha(140));
    for (k, line) in note.lines().enumerate() {
        let color = if k == 0 { egui::Color32::from_rgb(255, 190, 120) } else { egui::Color32::from_gray(225) };
        painter.text(at + egui::vec2(0.0, k as f32 * 19.0), egui::Align2::CENTER_CENTER, line, egui::FontId::proportional(if k == 0 { 16.0 } else { 14.0 }), color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bases::Materials;

    fn fireplace() -> (Bases, u32) {
        let mut bases = Bases::new();
        bases.materials = Materials::new(10, 20, 10);
        bases.build(Kind::Hearth, [4, 4, 4], 3).unwrap();
        let id = bases.piece_at([4, 4, 4]).unwrap().id;
        (bases, id)
    }

    #[test]
    fn a_fire_burns_its_logs_down_and_goes_out() {
        let (mut bases, id) = fireplace();
        assert_eq!(Kind::Hearth.cost(), Materials::new(0, 8, 4));
        assert_eq!(bases.piece(id).unwrap().fuel, 0.0);
        // A log lights it, low; more make it burn higher; it can't be stocked past the most
        let (was_lit, changed) = bases.add_fuel(id, LOG_SECONDS, MOST_SECONDS).unwrap();
        assert!(!was_lit && changed);
        assert_eq!(stage(bases.piece(id).unwrap().fuel), 2);
        for _ in 0..10 {
            bases.add_fuel(id, LOG_SECONDS, MOST_SECONDS);
        }
        assert_eq!(bases.piece(id).unwrap().fuel, MOST_SECONDS);
        assert_eq!(stage(MOST_SECONDS), 3);
        // It settles as it burns down, its look changing only as it does, and goes out
        let version = bases.version;
        assert!(!bases.burn(1.0), "a second's burning changes nothing to be seen");
        assert_eq!(bases.version, version);
        let mut changes = 0;
        let mut t = 0.0;
        while bases.piece(id).unwrap().fuel > 0.0 {
            changes += bases.burn(5.0) as usize;
            t += 5.0;
        }
        assert_eq!(changes, 3, "roaring, burning well, low, out");
        assert!((t - MOST_SECONDS).abs() <= 6.0);
        assert!(lit(&bases).next().is_none());
        // Only a fireplace takes fuel
        bases.build(Kind::StoneWall, [9, 9, 4], 0).unwrap();
        let wall = bases.piece_at([9, 9, 4]).unwrap().id;
        assert!(bases.add_fuel(wall, LOG_SECONDS, MOST_SECONDS).is_none());
    }

    #[test]
    fn a_fire_looks_as_it_burns() {
        let (mut bases, id) = fireplace();
        let flames = |b: &Bases| (0..4).map(|k| b.piece(id).unwrap().voxels(k).iter().filter(|&&m| m == FLAME).count()).sum::<usize>();
        let embers = |b: &Bases| (0..4).map(|k| b.piece(id).unwrap().voxels(k).iter().filter(|&&m| m == GLOW_FIRE).count()).sum::<usize>();
        assert_eq!(flames(&bases), 0);
        assert_eq!(embers(&bases), 0);
        bases.add_fuel(id, 10.0, MOST_SECONDS);
        let low = flames(&bases);
        assert!(low > 0 && embers(&bases) > 0);
        bases.add_fuel(id, MOST_SECONDS, MOST_SECONDS);
        assert!(flames(&bases) > low, "a roaring fire's flames are higher");
        // Its opening faces the way it's turned, with a back wall behind
        let piece = bases.piece(id).unwrap();
        assert_eq!(piece.blocks().len(), 4);
        assert_ne!(hearth_voxel(7, 6, 0, 0, 1), AIR, "a back wall");
        assert_eq!(hearth_voxel(7, 8, 7, 0, 1), AIR, "open at the front");
        // A chimney's hollow
        assert_eq!(chimney_voxel(7, 3, 4, 1), AIR);
        assert_ne!(chimney_voxel(0, 3, 4, 1), AIR);
    }

    #[test]
    fn fires_go_in_the_save_lit_or_not() {
        let (mut bases, id) = fireplace();
        bases.build(Kind::Chimney, [4, 4, 6], 3).unwrap();
        bases.add_fuel(id, 2.0 * LOG_SECONDS, MOST_SECONDS);
        bases.burn(30.0);
        let saved = serde_json::to_string(&bases.world_save()).unwrap();
        let mut back = Bases::new();
        back.load_world(serde_json::from_str(&saved).unwrap());
        let fire = back.piece(id).unwrap();
        assert_eq!(fire.kind, Kind::Hearth);
        assert!((fire.fuel - (2.0 * LOG_SECONDS - 30.0)).abs() < 1e-9);
        assert_eq!(back.piece_at([4, 4, 7]).unwrap().kind, Kind::Chimney);
        assert_eq!(chimney_top(&back, fire).map(|t| t.z), Some((8 * BLOCK) as f64));
        // A piece saved before fires were has none
        let old = r#"{"pieces":[{"id":1,"kind":"Hearth","at":[0,0,0],"turn":0,"health":900.0}],"edits":[],"next_id":2}"#;
        let mut older = Bases::new();
        older.load_world(serde_json::from_str(old).unwrap());
        assert_eq!(older.piece(1).unwrap().fuel, 0.0);
    }
}
