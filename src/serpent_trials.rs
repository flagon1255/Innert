//! Trying the serpents out in screenshot runs, in whatever mode, realm or world the run's in, once
//! it's there to play in, see Trial::ready:
//!
//! - `--set serpents=viper:3` puts a nest of that many of a serpent, named in full or the end of
//!   its name, `serpent_distance=9` meters ahead, `serpent_turn=90` degrees from facing the player,
//!   `serpent_calm=1` leaving the player be, and `serpent_pose=windup`, `strike` or `special`
//!   holding them in that pose to look at; `serpents_at=5` frames after it's ready
//! - `--set overlook_serpents=3` lifts the view that many meters off the ground there, to see them
//!   from a little above

use crate::animals::Look;

use super::*;

impl App {
    /// What the serpents' trial does at frame `frame` of a screenshot run, the world having been
    /// there to play in from frame `ready`, see the top of this file
    pub(super) fn serpent_trial(&mut self, frame: usize, ready: Option<usize>) {
        let Some(ready) = ready else {
            return;
        };
        let settings = settings_from_args();
        let setting = |key: &str| settings.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
        let number = |key: &str, default: f64| setting(key).and_then(|v| v.parse::<f64>().ok()).unwrap_or(default);
        let at = ready + number("serpents_at", 5.0) as usize;
        // `serpent_pose=cycle` holds them in each pose in turn, standing first, 60 frames each
        if setting("serpent_pose") == Some("cycle") && frame > at && (frame - at) % 60 == 0 {
            let pose = [None, Some(animals::POSE_WINDUP), Some(animals::POSE_STRIKE), Some(animals::POSE_SPECIAL)][(frame - at) / 60 % 4];
            for a in self.game.animals_mut().filter(|a| a.look.is_serpent()) {
                if let Some(boss) = &mut a.boss {
                    boss.fixed_pose = pose;
                }
            }
            println!("POSE at frame {frame}: {pose:?}");
        }
        // How the great serpents about are getting on, now and then
        if frame > at && (frame - at) % 30 == 0 {
            let vpm = self.world.voxels_per_meter as f64;
            for a in self.game.animals_mut().filter(|a| a.look.is_great_serpent()) {
                let b = a.boss.as_ref();
                println!(
                    "GREAT frame {frame}: {} at {:.1}, {:.1}, {:.1} m heading {:.2} to {:.2}, speed {:.2}, health {:.0}, engaged {}, {}",
                    a.look.name(),
                    a.position.x / vpm,
                    a.position.y / vpm,
                    a.position.z / vpm,
                    a.heading(),
                    a.target_heading,
                    a.speed,
                    a.health,
                    b.is_some_and(|b| b.engaged),
                    b.map_or(String::new(), |b| b.describe())
                );
            }
        }
        // `serpent_watch=1` keeps looking at the nearest great serpent's head, and `serpent_watch=2`
        // half way between it and its tail, to see all of it
        if let Some(watch) = setting("serpent_watch").filter(|_| frame > at) {
            let eye = camera_to_voxels(self.camera.position, self.world.size);
            let near = self.game.animals_mut().filter(|a| a.look.is_great_serpent() && a.health > 0.0).filter_map(|a| a.coils().map(|c| (c.head, c.points[c.points.len() / 2]))).min_by(|a, b| (a.0 - eye).norm().total_cmp(&(b.0 - eye).norm()));
            if let Some((head, middle)) = near {
                let target = if watch == "2" { head.lerp(&middle, 0.5) } else { head };
                self.camera.look_at(voxels_to_camera(target, self.world.size));
            }
        }
        if frame != at {
            return;
        }
        let vpm = self.world.voxels_per_meter as f64;
        let feet = self.player.as_ref().map_or(camera_to_voxels(self.camera.position, self.world.size), |p| p.position);
        let ahead = (self.camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz();
        // `overlook_serpents=3` lifts the view that many meters off the ground, to see what's
        // about from a little above, the player gone, so nothing's after them
        if let Some(up) = setting("overlook_serpents").and_then(|v| v.parse::<f64>().ok())
            && let Some(player) = self.player.take()
        {
            let eye = player.eye(&self.world) + Vector3::new(0.0, 0.0, up * vpm);
            self.camera.position = voxels_to_camera(eye, self.world.size);
        }
        // `serpent_boss=meremaw` puts a great serpent `serpent_boss_distance=16` meters ahead,
        // `serpent_boss_health=0.4` that hurt, `serpent_move=coil` making only that move, or
        // `tail`, `slam`, `spit`, `breath`, `charge`, `burrow` or `storm`, and `serpent_calm=1`
        // leaving the player be
        if let Some(name) = setting("serpent_boss") {
            let name = name.to_lowercase();
            let named = |l: &Look| [format!("{l:?}").to_lowercase(), l.name().replace(' ', "").to_lowercase()].iter().any(|n| n.contains(&name));
            match Look::ALL.into_iter().find(|l| l.is_great_serpent() && named(l)) {
                Some(look) => {
                    let water = &self.water;
                    self.game.summon(&self.world, &|x, y| water.surface_at(x, y), look, feet, ahead, number("serpent_boss_distance", 16.0));
                    let only = match setting("serpent_move") {
                        Some("coil") => Some(bosses::Move::Coil),
                        Some("tail") => Some(bosses::Move::Tail),
                        Some("slam") => Some(bosses::Move::Slam),
                        Some("spit") => Some(bosses::Move::Spit),
                        Some("breath") => Some(bosses::Move::Breath),
                        Some("charge") => Some(bosses::Move::Charge),
                        Some("burrow") => Some(bosses::Move::Burrow),
                        Some("storm") => Some(bosses::Move::Storm),
                        Some("summon") => Some(bosses::Move::Summon),
                        Some("cataclysm") => Some(bosses::Move::Cataclysm),
                        _ => None,
                    };
                    let health = setting("serpent_boss_health").and_then(|v| v.parse::<f64>().ok());
                    let calm = setting("serpent_calm") == Some("1");
                    for a in self.game.animals_mut().filter(|a| a.look == look) {
                        if let Some(h) = health {
                            a.health = a.max_health * h;
                        }
                        if let Some(boss) = &mut a.boss {
                            boss.only = only;
                            if calm {
                                boss.leave_be();
                            }
                        }
                    }
                    println!("SERPENT BOSS at frame {frame}: {}", look.name());
                }
                None => println!("No great serpent called {name}"),
            }
        }
        // Several kinds side by side with `serpents=viper:1,coiler:1`, `serpent_apart=4` meters apart
        if let Some(wanted) = setting("serpents") {
            let kinds: Vec<&str> = wanted.split(',').collect();
            let flat = Vector3::new(ahead.x, ahead.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x());
            let across = Vector3::new(-flat.y, flat.x, 0.0);
            for (k, kind) in kinds.iter().enumerate() {
                let (name, count) = kind.split_once(':').map_or((*kind, None), |(n, c)| (n, c.parse::<usize>().ok()));
                let name = name.to_lowercase();
                let named = |l: &Look| [format!("{l:?}").to_lowercase(), l.name().replace(' ', "").to_lowercase()].iter().any(|n| n.contains(&name));
                let Some(look) = Look::ALL.into_iter().find(|l| l.is_serpent() && named(l)) else {
                    println!("No serpent called {name}");
                    continue;
                };
                let pose = match setting("serpent_pose") {
                    Some("windup") => Some(animals::POSE_WINDUP),
                    Some("strike") => Some(animals::POSE_STRIKE),
                    Some("special") => Some(animals::POSE_SPECIAL),
                    Some("cycle") => Some(0),
                    _ => None,
                };
                let looks = vec![look; count.unwrap_or(look.pack_size())];
                let water = &self.water;
                let turn = number("serpent_turn", 0.0).to_radians();
                let calm = setting("serpent_calm") == Some("1");
                let side = (k as f64 - (kinds.len() - 1) as f64 * 0.5) * number("serpent_apart", 4.0) * vpm;
                self.game.summon_pack(&self.world, &|x, y| water.surface_at(x, y), &looks, feet + across * side, ahead, number("serpent_distance", 9.0), turn, calm, pose);
                println!("SERPENTS at frame {frame}: {} {}", looks.len(), look.name());
            }
            // `serpent_look=1` looks at the middle of them
            if setting("serpent_look") == Some("1") {
                let middle = feet + flat * number("serpent_distance", 9.0) * vpm + Vector3::new(0.0, 0.0, number("serpent_look_up", 0.6) * vpm);
                self.camera.look_at(voxels_to_camera(middle, self.world.size));
            }
        }
    }
}
