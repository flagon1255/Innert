//! The life in the water as the app has it, see src\animals\sealife.rs and game\sea.rs: what it
//! goes by each frame, a shark's fin pushing its wake through the water, and the `sea` script for
//! screenshots of it.

use std::sync::atomic::{AtomicBool, Ordering};

use super::*;
use crate::animals::sealife::Hunt;

/// Whose turn it is to push the water, see fin_turn()
static FIN_TURN: AtomicBool = AtomicBool::new(false);

/// Whether a shark's fin pushes the water this frame rather than the player, when both would:
/// they take turns, a frame each, as the water has room for only one wake, see water::Water::record()
pub(super) fn fin_turn() -> bool {
    !FIN_TURN.fetch_xor(true, Ordering::Relaxed)
}

impl App {
    /// What the life in the water goes by: whether it's night, and the kind of land
    pub(super) fn tell_sea(&mut self) {
        self.game.set_sea(self.cycle.is_night(), worldgen::land_of(self.terrain.seed));
    }

    /// `--script sea`: swims out into water `--set sea_depth=2.5` meters deep, puts `--set
    /// schools=3` schools of fish round, of `--set school=reefling` or whatever lives there, and with
    /// `--set spawn=shark` or `spawn=ghostmaw` a shark in the nearest deep sea; then with `--set
    /// sea_view=school` the camera's put under the water by the nearest fish looking at them, with
    /// `sea_view=shark` it follows the shark, a shot taken as it bites, and `sea_view=fish` it looks
    /// at the nearest fish from over the water. What happens is printed.
    pub(super) fn sea_step(&mut self, frame: usize) {
        let settings = settings_from_args();
        let setting = |key: &str| settings.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
        let number = |key: &str, default: f64| setting(key).and_then(|v| v.parse::<f64>().ok()).unwrap_or(default);
        let vpm = self.world.voxels_per_meter as f64;
        let eye = camera_to_voxels(self.camera.position, self.world.size);
        if frame == 1 {
            self.stand_in_water(number("sea_depth", 2.5), true);
            if setting("sea_calm").is_some() {
                self.game.calm_sea();
            }
            return;
        }
        let water = &self.water;
        let level = |x: f64, y: f64| water.surface_at(x, y);
        if frame == 3 {
            let feet = self.player.as_ref().map_or(eye, |p| p.position);
            let look = setting("school").and_then(|name| animals::sealife::FISH.into_iter().find(|l| l.name() == name));
            for k in 0..number("schools", 3.0) as usize {
                let n = self.game.summon_school(&self.world, &level, look, feet, 5.0 + 4.0 * k as f64);
                println!("frame {frame}: a school of {n}");
            }
            if let Some(name) = setting("spawn") {
                let look = if name == "ghostmaw" { animals::Look::Ghostmaw } else { animals::Look::Saltjaw };
                let found = self.game.summon_shark(&self.world, &level, look, feet, number("shark_distance", 22.0));
                println!("frame {frame}: a {} in the sea: {found}", look.name());
            }
            return;
        }
        if frame < 3 {
            return;
        }
        let view = setting("sea_view").unwrap_or_default();
        match view.as_str() {
            // Under the water by the nearest fish, looking at it and those about it
            "school" if frame >= 40 => {
                if let Some((fish, near)) = self.game.nearest_fish(eye) {
                    let side = Vector3::new(1.0, 0.6, 0.0).normalize();
                    let at = fish + side * number("sea_back", 2.4) * vpm;
                    self.player = None;
                    self.camera.position = voxels_to_camera(at, self.world.size);
                    self.camera.look_at(voxels_to_camera(fish, self.world.size));
                    if frame % 30 == 0 {
                        println!("frame {frame}: {near} fish about the nearest, {:.1} m under the water", (self.water.surface_at(fish.x, fish.y) - fish.z) / vpm);
                    }
                }
            }
            // From over the water, a little way off, looking down at the nearest fish
            "fish" if frame >= 40 => {
                if let Some((fish, near)) = self.game.nearest_fish(eye) {
                    let top = self.water.surface_at(fish.x, fish.y);
                    let at = Vector3::new(fish.x + 1.2 * vpm, fish.y + 1.0 * vpm, top + number("sea_up", 1.6) * vpm);
                    self.player = None;
                    self.camera.position = voxels_to_camera(at, self.world.size);
                    self.camera.look_at(voxels_to_camera(fish, self.world.size));
                    if frame % 30 == 0 {
                        println!("frame {frame}: {near} fish about the nearest");
                    }
                }
            }
            // Following the shark, a shot as it bites
            "shark" => {
                if let Some((shark, _, hunt)) = self.game.nearest_shark(eye) {
                    self.camera.look_at(voxels_to_camera(shark, self.world.size));
                    if frame % 20 == 0 {
                        println!("frame {frame}: the shark's {:.1} m off, {hunt:?}", (shark - eye).norm() / vpm);
                    }
                    if let Hunt::Bite { t } = hunt
                        && t < 0.05
                        && let Some(capture) = &mut self.capture
                    {
                        println!("frame {frame}: the shark bites");
                        capture.shots.push(frame + 1);
                    }
                    if let Hunt::Charge { t } = hunt
                        && (0.9..0.95).contains(&t)
                        && let Some(capture) = &mut self.capture
                    {
                        println!("frame {frame}: the shark lunges");
                        capture.shots.push(frame + 1);
                    }
                }
            }
            // Over the water a little, watching the shark's fin go round
            "fin" => {
                if let Some((shark, _, hunt)) = self.game.nearest_shark(eye) {
                    self.camera.look_at(voxels_to_camera(shark + Vector3::new(0.0, 0.0, 0.3 * vpm), self.world.size));
                    if frame % 20 == 0 {
                        println!("frame {frame}: the shark's {:.1} m off, {hunt:?}", (shark - eye).norm() / vpm);
                    }
                }
            }
            _ => {
                if frame % 60 == 0 {
                    let sharks = self.game.nearest_shark(eye).map(|(s, _, h)| format!("{:.0} m off, {h:?}", (s - eye).norm() / vpm));
                    println!("frame {frame}: fish {:?}, shark {sharks:?}", self.game.nearest_fish(eye).map(|f| f.1));
                }
            }
        }
    }
}
