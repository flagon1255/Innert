//! The camps in caves, see src\worldgen\camps\cave.rs. Near the camera their fires crackle, an
//! ember popping now and then, the cave giving a little of each back a moment after, and they
//! throw up embers and smoke, the dead's pale smoke only; bandits by theirs clank their gear now
//! and then until they're up and fighting; and the dead at theirs lie under the floor until the
//! player comes near, then claw their way up out of it with a dry rattle, see Game::set_dens().
//! All of it fades out with distance, and none of it's heard past HEARD_M.

use super::*;
use crate::worldgen::Folk;

/// How far off a camp's fire is heard at all, in meters, fading out to nothing there, and how
/// far above or below, through the rock; and how near it gives off embers and smoke
const HEARD_M: f64 = 25.0;
const HEARD_LEVEL_M: f64 = 8.0;
const SMOKE_M: f64 = 18.0;
/// How near the player comes to the dead's camp for them to rise, in meters, and how far above
/// or below
const WAKE_M: f64 = 8.0;
const WAKE_LEVEL_M: f64 = 4.0;
/// How far round the fire the dead rise, in meters
const RISE_WITHIN: f64 = 2.2;
/// How long after a sound the cave gives a little of it back, in seconds, and how loud, of it
const ECHO_SECONDS: f64 = 0.17;
const ECHO: f32 = 0.35;

/// A camp's fire in a cave: where it is in voxels, who keeps it, and where in the whole
/// landscape, its den's key, see main.rs
#[derive(Clone, Copy)]
pub struct CaveFire {
    pub position: Vector3<f64>,
    pub folk: Folk,
    pub key: [i64; 3],
}

/// The dead of a camp lying under its floor: where, its den's key, and who they are
pub(super) struct Sleeping {
    pub position: Vector3<f64>,
    pub key: [i64; 3],
    pub looks: Vec<Look>,
}

/// The camps in caves in this piece of the world: their fires, and seconds until each next
/// crackles, pops, gives off an ember or smoke, and its bandits' gear clanks; the dead lying in
/// wait; and what the cave's still to give back, in how long
#[derive(Default)]
pub(super) struct CaveCamps {
    fires: Vec<CaveFire>,
    timers: Vec<[f64; 4]>,
    pub dead: Vec<Sleeping>,
    echoes: Vec<(f64, Cue)>,
    rng: u32,
}

/// The dead who keep a camp, by `seed`: skeletons, one in armour or with a bow among some
pub(super) fn risen(seed: u32) -> Vec<Look> {
    match seed % 4 {
        0 => vec![Look::Skeleton, Look::Skeleton, Look::ArmoredSkeleton],
        1 => vec![Look::Skeleton, Look::BoneArcher, Look::Skeleton],
        2 => vec![Look::ArmoredSkeleton, Look::Skeleton],
        _ => vec![Look::Skeleton, Look::Skeleton, Look::Skeleton],
    }
}

impl Game {
    /// The fires of the camps in caves in the piece of the world just built, in its voxels
    pub fn set_cave_fires(&mut self, fires: Vec<CaveFire>) {
        // Each its own way, so they don't all crackle at once
        self.cave_camps.timers = (0..fires.len()).map(|k| [(k as f64 * 0.37) % 2.0, 2.0 + (k as f64 * 1.3) % 4.0, 0.0, 3.0 + (k as f64 * 2.1) % 5.0]).collect();
        self.cave_camps.fires = fires;
    }

    /// Played now, and given back by the cave a moment later, a little quieter
    fn echoing(&mut self, cue: Cue) {
        self.cues.push(cue);
        let after = ECHO_SECONDS + 0.06 * random(&mut self.cave_camps.rng);
        self.cave_camps.echoes.push((after, Cue { volume: cue.volume * ECHO, pitch: cue.pitch * 0.97, ..cue }));
    }

    /// Called each frame, see the top of this file. `player` is where the player's feet are, in
    /// voxels.
    pub(super) fn tend_cave_camps(&mut self, world: &World, water: WaterLevel, player: Option<Vector3<f64>>, view: &View, dt: f64) {
        let vpm = world.voxels_per_meter as f64;
        self.wake_the_dead(world, water, player, vpm);
        let mut k = 0;
        while k < self.cave_camps.echoes.len() {
            self.cave_camps.echoes[k].0 -= dt;
            if self.cave_camps.echoes[k].0 <= 0.0 {
                let (_, cue) = self.cave_camps.echoes.swap_remove(k);
                self.cues.push(cue);
            } else {
                k += 1;
            }
        }
        for i in 0..self.cave_camps.fires.len() {
            let fire = self.cave_camps.fires[i];
            let to = fire.position - view.eye;
            let far = to.xy().norm() / vpm;
            if far > HEARD_M || to.z.abs() / vpm > HEARD_LEVEL_M {
                continue;
            }
            // Quieter the further off, and gone by HEARD_M, on top of the way every sound fades
            let t = far / HEARD_M;
            let fade = (1.0 - t * t) as f32;
            let ghost = fire.folk == Folk::Dead;
            let flames = fire.position + Vector3::new(0.0, 0.0, 0.45 * vpm);
            let mut r = || random(&mut self.cave_camps.rng);
            let timers = &mut self.cave_camps.timers[i];
            for t in timers.iter_mut() {
                *t -= dt;
            }
            let mut cues = Vec::new();
            let mut wisps = Vec::new();
            // The fire crackling, the dead's low and faint
            if timers[0] <= 0.0 {
                timers[0] = 1.9 + 1.4 * r();
                let (volume, pitch) = if ghost { (0.22, 0.7) } else { (0.5, 0.9 + 0.2 * r() as f32) };
                cues.push(Cue::at(Sound::Crackle, flames, volume * fade).pitched(pitch));
            }
            // Now and then an ember popping, sparks flying up off it
            if timers[1] <= 0.0 {
                timers[1] = 3.5 + 6.0 * r();
                if !ghost {
                    cues.push(Cue::at(Sound::Crackle, flames, 0.3 * fade).pitched(1.6 + 0.3 * r() as f32));
                    if far < SMOKE_M {
                        for _ in 0..3 {
                            let v = Vector3::new(r() - 0.5, r() - 0.5, 2.0 + r()) * 0.8;
                            wisps.push((flames, v * vpm, 0.7 + 0.5 * r(), true));
                        }
                    }
                }
            }
            // Embers drifting up off it, and smoke now and then; the dead's fire pale smoke only
            if timers[2] <= 0.0 && far < SMOKE_M {
                let smoke = ghost || r() < 0.25;
                timers[2] = if ghost { 0.9 + 0.8 * r() } else { 0.2 + 0.45 * r() };
                let from = flames + Vector3::new(r() - 0.5, r() - 0.5, 0.0) * 0.5 * vpm;
                let v = if smoke { Vector3::new(r() - 0.5, r() - 0.5, 0.0) * 0.25 + Vector3::new(0.0, 0.0, 0.45) } else { Vector3::new((r() - 0.5) * 0.3, (r() - 0.5) * 0.3, 0.6 + 0.6 * r()) };
                wisps.push((from, v * vpm, if smoke { 3.0 } else { 0.9 + 0.8 * r() }, !smoke));
            }
            // Bandits round theirs shifting their gear about, until they're up and after the player
            if timers[3] <= 0.0 {
                timers[3] = 5.0 + 6.0 * r();
                let gear = match (r() * 3.0) as u32 {
                    0 => Cue::at(Sound::Clang, flames, 0.1 * fade).pitched(0.6 + 0.1 * r() as f32),
                    1 => Cue::at(Sound::Ratchet, flames, 0.14 * fade).pitched(0.9),
                    _ => Cue::at(Sound::Creak, flames, 0.16 * fade).pitched(1.1),
                };
                if fire.folk == Folk::Bandits {
                    let band = self.animals.iter().filter(|a| a.health > 0.0 && a.boss.as_ref().is_some_and(|b| b.key == fire.key));
                    let (mut about, mut fighting) = (false, false);
                    for a in band {
                        about = true;
                        fighting |= a.boss.as_ref().is_some_and(|b| b.sighted());
                    }
                    if about && !fighting {
                        cues.push(gear);
                    }
                }
            }
            for cue in cues {
                self.echoing(cue);
            }
            for (at, v, life, ember) in wisps {
                self.combat.wisp(at, v, life, ember);
            }
        }
    }

    /// The dead of a camp in a cave the player's come near rising up out of its floor round its
    /// fire with a dry rattle, after them
    fn wake_the_dead(&mut self, world: &World, water: WaterLevel, player: Option<Vector3<f64>>, vpm: f64) {
        let Some(feet) = player else {
            return;
        };
        let mut k = 0;
        while k < self.cave_camps.dead.len() {
            let d = &self.cave_camps.dead[k];
            if (d.position - feet).xy().norm() > WAKE_M * vpm || (d.position.z - feet.z).abs() > WAKE_LEVEL_M * vpm {
                k += 1;
                continue;
            }
            let d = self.cave_camps.dead.swap_remove(k);
            let seed = hash(d.key[0] as u32 ^ hash(d.key[1] as u32 ^ 0x415e));
            for mut monster in animals::spawn_pack(world, water, &d.looks, d.position, RISE_WITHIN, seed) {
                if let Some(state) = &mut monster.boss {
                    state.key = d.key;
                    state.provoke();
                    state.emerge();
                }
                self.combat.stir(monster.position, 0.8, 0.5, DROP_MUD);
                self.animals.push(monster);
            }
            self.echoing(Cue::at(Sound::Rattle, d.position + Vector3::new(0.0, 0.0, 0.5 * vpm), 1.0).pitched(0.75));
            self.say("Bones stir round the cold fire: the dead rise from its ashes!");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dead of a camp are only skeletons, and the sounds its fire makes are recorded
    #[test]
    fn the_dead_of_a_camp_are_skeletons_and_its_sounds_are_heard() {
        for seed in 0..16 {
            let looks = risen(seed);
            assert!((2..=3).contains(&looks.len()));
            assert!(looks.iter().all(|l| matches!(l, Look::Skeleton | Look::ArmoredSkeleton | Look::BoneArcher)), "{seed}");
        }
        for sound in [Sound::Crackle, Sound::Rattle, Sound::Clang, Sound::Ratchet, Sound::Creak] {
            assert!(crate::audio::recorded(sound), "{sound:?} isn't recorded");
        }
    }

    /// The dead of a camp lie under its floor, none of them about, till the player comes near:
    /// then they rise round its fire, once, and come for the player; and once they're all dead
    /// again their camp's clear for good
    #[test]
    fn the_dead_lie_under_their_camp_till_the_player_comes_near() {
        let size = [64u32, 64, 64];
        let mut occupancy = vec![0u128; 16 * 16 * 8];
        occupancy[..16 * 16 * 2].fill(u128::MAX);
        let mut world = World::from_packed_occupancy(size, &occupancy, DIRT);
        world.voxels_per_meter = 4;
        let vpm = 4.0;
        let dry = |_: f64, _: f64| f64::NEG_INFINITY;
        let fire = Vector3::new(32.0, 32.0, 16.0);
        let mut game = Game::new(&world, &dry, 5, fire);
        let key = [123, -456, 78];
        let den = || vec![Den { position: fire, key, kind: DenKind::Risen, band: None }];
        let ours = |g: &Game| g.animals.iter().filter(|a| a.health > 0.0 && a.boss.as_ref().is_some_and(|b| b.key == key)).count();
        game.set_dens(&world, &dry, den());
        assert_eq!(ours(&game), 0, "they lie under the floor");
        // Not from further off, nor from right over or under it
        game.wake_the_dead(&world, &dry, Some(fire + Vector3::new(WAKE_M + 1.0, 0.0, 0.0) * vpm), vpm);
        game.wake_the_dead(&world, &dry, Some(fire + Vector3::new(0.0, 0.0, WAKE_LEVEL_M + 1.0) * vpm), vpm);
        assert_eq!(ours(&game), 0);
        game.wake_the_dead(&world, &dry, Some(fire + Vector3::new(WAKE_M - 1.0, 0.0, 0.0) * vpm), vpm);
        let risen = ours(&game);
        assert!(risen >= 2, "only {risen} rose");
        for a in game.animals.iter().filter(|a| a.boss.as_ref().is_some_and(|b| b.key == key)) {
            assert!(matches!(a.look, Look::Skeleton | Look::ArmoredSkeleton | Look::BoneArcher));
            assert!((a.position - fire).xy().norm() < (RISE_WITHIN * 2.0 + 0.5) * vpm, "one rose {} m from the fire", (a.position - fire).xy().norm() / vpm);
        }
        assert!(game.cues.iter().any(|c| c.sound == Sound::Rattle), "no rattle as they rose");
        // Only once, though the world's built again round them
        game.wake_the_dead(&world, &dry, Some(fire), vpm);
        game.set_dens(&world, &dry, den());
        game.wake_the_dead(&world, &dry, Some(fire), vpm);
        assert_eq!(ours(&game), risen);
        // Put down again, they stay down
        for a in game.animals.iter_mut().filter(|a| a.boss.as_ref().is_some_and(|b| b.key == key)) {
            a.health = 0.0;
        }
        game.reap(&world);
        game.set_dens(&world, &dry, den());
        game.wake_the_dead(&world, &dry, Some(fire), vpm);
        assert_eq!(ours(&game), 0, "they rose again once cleared");
    }
}
