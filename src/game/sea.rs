//! Fish and sharks about the player, see src\animals\sealife.rs: schools turning up in the water
//! round them as they go and left behind as they go on, a shark now and then where the sea's deep
//! enough for one, the great pale ghostmaw by night; their schooling, their splashes and the spray
//! off a shark's fin; and what's caught of them.

use super::*;
use crate::animals::sealife;
use crate::worldgen::Land;

/// About this many fish at most within FISH_NEAR meters of the player
const FISH_MOST: usize = 26;
const FISH_NEAR: f64 = 45.0;
/// Fish further off than this, and sharks, are left behind
const FISH_GONE: f64 = 60.0;
const SHARK_GONE: f64 = 110.0;
/// A shark leaving at dawn goes once it's this far off
const SHARK_LEFT: f64 = 45.0;
/// Seconds at the least between one shark and the next, and between tries where there's no water
/// for one
const SHARK_REST: f64 = 45.0;
const SHARK_RETRY: f64 = 6.0;
/// How near a fish has to have been to the player when it was killed for it to be theirs, in meters
const CATCH_NEAR: f64 = 18.0;

/// What's kept for the life in the water about the player
#[derive(Default)]
pub(super) struct Sea {
    /// Seconds till fish and sharks are next looked over
    timer: f64,
    /// Seconds till a shark can come
    rest: f64,
    /// Whether it's night, for the ghostmaw, and the kind of land it is, see Game::set_sea()
    night: bool,
    land: Option<Land>,
    /// Never any of its own, for screenshots of something else
    off: bool,
}

impl Game {
    /// What the life in the water goes by, set every frame: whether it's night, and the kind of land
    pub fn set_sea(&mut self, night: bool, land: Land) {
        self.sea.night = night;
        self.sea.land = Some(land);
    }

    /// No fish or sharks coming of their own, for screenshots
    pub fn calm_sea(&mut self) {
        self.sea.off = true;
    }

    /// Fish schooling, splashing and scattering, sharks' fins spraying, and fish and sharks coming
    /// and going about the player, see the module's comment
    pub(super) fn tend_sea(&mut self, world: &World, water: WaterLevel, player: Option<Vector3<f64>>, view: &View, dt: f64) {
        let vpm = world.voxels_per_meter as f64;
        // What scatters fish: anything splashing in, and sharks
        let mut threats: Vec<Vector3<f64>> = self.combat.waves.iter().map(|(at, _)| *at).collect();
        threats.extend(self.animals.iter().filter(|a| sealife::is_shark(a.look) && a.health > 0.0).map(|a| a.center(world.voxels_per_meter)));
        let chest = player.map(|f| f + Vector3::new(0.0, 0.0, 1.0 * vpm));
        sealife::school(&mut self.animals, chest, &threats, vpm);
        // Their splashes, and the spray off a shark's fin cutting the top of the water
        let mut splashes = Vec::new();
        let mut spray = Vec::new();
        for a in &mut self.animals {
            if let Some(s) = &mut a.swim {
                splashes.extend(s.splash.take());
                if let Some((at, _)) = s.fin
                    && (at - view.eye).norm() < 80.0 * vpm
                {
                    spray.push(at);
                }
            }
        }
        for (at, size) in splashes {
            if (at - view.eye).norm() < 60.0 * vpm {
                self.combat.splash_up(at, size);
            }
        }
        for at in spray {
            self.combat.stir(at, 0.22, dt * 0.5, DROP_WATER);
        }
        // Stranded or swum off, gone without a word
        self.animals.retain(|a| !a.swim.as_ref().is_some_and(|s| s.gone));

        self.sea.timer -= dt;
        if self.sea.timer > 0.0 {
            return;
        }
        self.sea.timer = 1.0;
        let near = player.unwrap_or(view.eye);
        let night = self.sea.night;
        // Those left behind, and a ghostmaw gone back down into the deep as the day comes
        self.animals.retain(|a| {
            let d = (a.position - near).norm() / vpm;
            let leaving = a.swim.as_ref().is_some_and(|s| s.leaving);
            !(sealife::is_fish(a.look) && d > FISH_GONE || sealife::is_shark(a.look) && (d > SHARK_GONE || leaving && d > SHARK_LEFT))
        });
        for a in self.animals.iter_mut().filter(|a| a.look == Look::Ghostmaw && !night) {
            if let Some(s) = &mut a.swim {
                s.leaving = true;
            }
        }
        if self.sea.off {
            return;
        }
        let land = self.sea.land.unwrap_or(Land::Normal);
        // A school more where there's room for one
        let about = self.animals.iter().filter(|a| sealife::is_fish(a.look) && (a.position - near).norm() < FISH_NEAR * vpm).count();
        if about < FISH_MOST {
            self.rng = hash(self.rng);
            if let Some((at, top, bottom)) = sealife::school_water(world, water, near, 12.0, 38.0, self.rng) {
                let roll = random(&mut self.rng);
                let look = sealife::fish_for(world, land, at, top, bottom, roll);
                let school = sealife::spawn_school(world, water, look, at, hash(self.rng ^ 0x5c));
                self.animals.extend(school.into_iter().take(FISH_MOST - about));
            }
        }
        // A shark, where the sea's deep enough for one, now and then
        self.sea.rest -= 1.0;
        let sharks = self.animals.iter().filter(|a| sealife::is_shark(a.look)).count();
        if sharks == 0 && self.sea.rest <= 0.0 {
            self.rng = hash(self.rng);
            let look = if night && random(&mut self.rng) < 0.6 { Look::Ghostmaw } else { Look::Saltjaw };
            match sealife::shark_water(world, water, look, near, 30.0, 48.0, self.rng) {
                Some(at) => {
                    self.animals.push(sealife::spawn_shark(world, look, at, self.rng));
                    self.sea.rest = SHARK_REST;
                }
                None => self.sea.rest = SHARK_RETRY,
            }
        }
    }

    /// For screenshots: a shark of `look` `distance` meters from `near`, in voxels, in the nearest
    /// sea deep enough for it, with none coming of its own after. Whether there was water for it.
    pub fn summon_shark(&mut self, world: &World, water: WaterLevel, look: Look, near: Vector3<f64>, distance: f64) -> bool {
        self.sea.rest = f64::MAX;
        for k in 0..12u32 {
            let reach = distance + k as f64 * 6.0;
            if let Some(at) = sealife::shark_water(world, water, look, near, (reach - 6.0).max(4.0), reach, hash(k ^ 0x5a)) {
                self.animals.push(sealife::spawn_shark(world, look, at, hash(k ^ 0x5b)));
                return true;
            }
        }
        false
    }

    /// For screenshots: a school of `look`, or of whatever lives there, `distance` meters from
    /// `near`, in voxels, in the nearest water. How many there are.
    pub fn summon_school(&mut self, world: &World, water: WaterLevel, look: Option<Look>, near: Vector3<f64>, distance: f64) -> usize {
        for k in 0..20u32 {
            if let Some((at, top, bottom)) = sealife::school_water(world, water, near, (distance - 2.0).max(0.5), distance + k as f64 * 3.0, hash(k ^ 0x5c)) {
                let look = look.unwrap_or_else(|| sealife::fish_for(world, self.sea.land.unwrap_or(Land::Normal), at, top, bottom, 0.3));
                let school = sealife::spawn_school(world, water, look, at, hash(k ^ 0x5d));
                if !school.is_empty() {
                    let n = school.len();
                    self.animals.extend(school);
                    return n;
                }
            }
        }
        0
    }

    /// Where a shark's fin's cutting the top of the water nearest `near`, in voxels, and how fast,
    /// in voxels a second, for the wake it pushes through the water, see water::Water::record()
    pub fn fin_wake(&self, near: Vector3<f64>) -> Option<([f64; 2], [f64; 2])> {
        self.animals
            .iter()
            .filter_map(|a| a.swim.as_ref().and_then(|s| s.fin))
            .min_by(|a, b| (a.0 - near).norm().total_cmp(&(b.0 - near).norm()))
            .map(|(at, v)| ([at.x, at.y], [v.x, v.y]))
    }

    /// For screenshots: the middle of the nearest shark to `near`, in voxels, which way it's facing,
    /// and what it's doing
    pub fn nearest_shark(&self, near: Vector3<f64>) -> Option<(Vector3<f64>, f64, sealife::Hunt)> {
        let vpm = self.combat.vpm() as u32;
        self.animals
            .iter()
            .filter(|a| sealife::is_shark(a.look) && a.health > 0.0)
            .filter_map(|a| a.swim.as_ref().map(|s| (a.center(vpm), a.heading(), s.hunt)))
            .min_by(|a, b| (a.0 - near).norm().total_cmp(&(b.0 - near).norm()))
    }

    /// For screenshots: the middle of the nearest fish to `near`, in voxels, and how many fish are
    /// within ten meters of it
    pub fn nearest_fish(&self, near: Vector3<f64>) -> Option<(Vector3<f64>, usize)> {
        let vpm = self.combat.vpm();
        let fish: Vec<Vector3<f64>> = self.animals.iter().filter(|a| sealife::is_fish(a.look)).map(|a| a.center(vpm as u32)).collect();
        let first = fish.iter().copied().min_by(|a, b| (a - near).norm().total_cmp(&(b - near).norm()))?;
        Some((first, fish.iter().filter(|f| (*f - first).norm() < 10.0 * vpm).count()))
    }

    /// What's had of a fish or a shark that's died: a fish caught, into the player's pack if it
    /// was near them; a shark's teeth, and now and then a jewel it swallowed
    pub(super) fn sea_spoils(&mut self, animal: &crate::animals::Animal) {
        let look = animal.look;
        let near = self.feet.is_some_and(|f| (f - animal.position).norm() < CATCH_NEAR * self.combat.vpm());
        if sealife::is_fish(look) {
            if near {
                self.add_item(Item::RawFish, 1);
                self.cues.push(Cue::at(Sound::Splash, animal.position, 0.35).pitched(1.6));
                self.say(format!("You caught a {}.", look.name()));
            }
            return;
        }
        let teeth = match look {
            Look::Ghostmaw => 3 + (random(&mut self.rng) * 3.0) as u32,
            _ => 1 + (random(&mut self.rng) * 2.0) as u32,
        };
        self.add_item(Item::SharkTooth, teeth);
        self.say(format!("You prise {teeth} teeth from the {}'s jaws.", look.name()));
        let chance = if look == Look::Ghostmaw { 0.14 } else { 0.05 };
        if random(&mut self.rng) < chance {
            let jewel = self.roll_jewel(MONSTER_LUCK * if look == Look::Ghostmaw { 2.0 } else { 1.0 });
            self.find_jewels(vec![jewel]);
        }
    }
}
