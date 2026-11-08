//! The guardians of the realms, in Realms: lesser bosses at lairs of their own round about the gate
//! or the portal home, three in the normal world and one more in each realm on, see
//! realms::guardian_count(). Until every one of a realm's guardians is slain its raid bosses are
//! warded: rings of runes turn round each of them, and whatever strikes them glances off, see
//! Combat::glance_off_ward(). The last of them slain, the wards shatter at every raid lair, and the raid
//! bosses can be fought. What's slain is kept with the rest of what's beaten in each realm.

use std::f64::consts::TAU;

use crate::animals::{AnimalInstance, Look, MOVING, Model, Part, ball, limb, voxelize};
use crate::realms::{self, REALMS};
use crate::worldgen::material::*;

use super::*;

/// Voxels a meter in the wards' models, which are a meter across their rings and scaled to fit
/// each raid boss: about as fine as the world's round the biggest
const RES: f64 = 96.0;
/// The rings round a warded raid boss: how far each is tipped over, how fast its runes run round
/// it and where round it starts, in radians and radians a second
const RINGS: [(f64, f64, f64); 3] = [(0.22, 0.55, 0.0), (1.15, -0.45, 2.1), (1.15, 0.38, 4.2)];
/// How long the shards of a broken ward fly, and how long its rings go on swelling out as it
/// breaks, in seconds; how many shards a ward breaks into
const SHARD_LIFE: f64 = 2.2;
const BURST_LIFE: f64 = 0.35;
const SHARDS: usize = 64;
/// And how long it lights up all round as it breaks, in seconds
const FLASH_LIFE: f64 = 0.9;
/// How far out from where a raid lair's boss is about a ward breaks round it, in meters, where none is
const LAIR_WARD: f64 = 3.0;

/// How far out from the middle of a raid boss of `look` its ward is, in meters
pub fn ward_radius(look: Look) -> f64 {
    let (height, size) = look.body();
    (height * 1.15).max(size * 1.5).max(1.4)
}

/// A ring of the ward, a meter's radius round the middle in the x-y plane, a fine hoop of deep
/// violet light with brighter runes standing on it, and a shard of one broken
fn ring() -> Vec<Part> {
    let mut p = Vec::new();
    let segments = 64;
    let at = |a: f64, r: f64, z: f64| [a.cos() * r, a.sin() * r, z];
    for k in 0..segments {
        let (a, b) = (k as f64 * TAU / segments as f64, (k + 1) as f64 * TAU / segments as f64);
        p.push(limb(at(a, 1.0, 0.0), at(b, 1.0, 0.0), 0.011, 0.011, GLOW_VOID));
    }
    // A rune every so far round: an upright stroke, one across it slanting and a dot over them
    for k in 0..14 {
        let a = (k as f64 + 0.5) * TAU / 14.0;
        let lean = if k % 2 == 0 { 0.022 } else { -0.022 };
        p.push(limb(at(a, 1.0, -0.055), at(a, 1.0, 0.055), 0.008, 0.008, GLOW_ARCANE));
        p.push(limb(at(a - lean, 1.0, -0.02), at(a + lean, 1.0, 0.03), 0.007, 0.007, GLOW_ARCANE));
        p.push(ball(at(a, 1.0, 0.085), 0.011, GLOW_HOLY));
    }
    p
}

fn shard() -> Vec<Part> {
    vec![limb([0.0, 0.0, -0.5], [0.0, 0.0, 0.15], 0.08, 0.05, GLOW_ARCANE), limb([0.0, 0.0, 0.15], [0.0, 0.0, 0.5], 0.05, 0.008, GLOW_HOLY)]
}

/// The models of the wards: a ring of one, and a shard of one broken
pub fn models() -> Vec<(Model, Vector3<f64>)> {
    vec![voxelize(&ring(), RES), voxelize(&shard(), RES)]
}

/// A shard of a broken ward, flying off and falling, in voxels of the world
struct Shard {
    at: Vector3<f64>,
    velocity: Vector3<f64>,
    turn: UnitQuaternion<f64>,
    spin: Vector3<f64>,
    size: f64,
    age: f64,
}

/// The wards breaking and what's been seen of them, see App::update_guardians()
#[derive(Default)]
pub struct Wards {
    /// Seconds gone, for the rings to turn
    time: f64,
    /// Which realm's guardians were last looked at, by its index and where its gate is, and how
    /// many of them were slain then
    of: Option<(usize, [i64; 2])>,
    slain: usize,
    shards: Vec<Shard>,
    /// Rings swelling out as a ward breaks: the middle, its radius in meters and how long ago
    bursts: Vec<(Vector3<f64>, f64, f64)>,
    rng: u32,
}

impl Wards {
    /// The world moved over by `shift` voxels
    pub fn shift(&mut self, shift: Vector3<f64>) {
        for shard in &mut self.shards {
            shard.at -= shift;
        }
        for burst in &mut self.bursts {
            burst.0 -= shift;
        }
    }

    fn random(&mut self) -> f64 {
        self.rng = self.rng.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
        let x = ((self.rng >> ((self.rng >> 28) + 4)) ^ self.rng).wrapping_mul(277_803_737);
        ((x >> 22) ^ x) as f64 / u32::MAX as f64
    }

    /// A ward round `center` with `radius` meters breaking: its rings swelling out, and shards of
    /// it flying out every way, up most of all
    fn shatter(&mut self, center: Vector3<f64>, radius: f64, vpm: f64) {
        self.bursts.push((center, radius, 0.0));
        for _ in 0..SHARDS {
            let a = self.random() * TAU;
            let up = self.random() * 1.6 - 0.4;
            let way = Vector3::new(a.cos(), a.sin(), up).normalize();
            let speed = (5.0 + 7.0 * self.random()) * vpm;
            let spin = Vector3::new(self.random() - 0.5, self.random() - 0.5, self.random() - 0.5) * 14.0;
            let turn = UnitQuaternion::from_scaled_axis(Vector3::new(self.random(), self.random(), self.random()) * TAU);
            let size = 0.2 + 0.25 * self.random();
            self.shards.push(Shard { at: center + way * radius * vpm, velocity: way * speed + Vector3::new(0.0, 0.0, 3.0 * vpm), turn, spin, size, age: 0.0 });
        }
    }
}

/// Model `model` of the wards' with its middle at `center`, turned by `turn`, `size` meters to its
/// model's one, in a world of `vpm` voxels a meter
fn placed(index: u32, model: &(Model, Vector3<f64>), center: Vector3<f64>, turn: UnitQuaternion<f64>, size: f64, vpm: f64) -> AnimalInstance {
    let (model, anchor) = model;
    let [x, y, z] = model.size.map(|v| v as f64 / RES);
    let reach = (x * x * 0.25 + y * y * 0.25 + z * z).sqrt() * size * vpm;
    AnimalInstance::new(index, center + turn * anchor * size * vpm, turn, vpm / RES * size, reach, MOVING)
}

impl App {
    /// How many of the realm's guardians are slain, and how many it has, in Realms
    pub(super) fn guardians_slain(&self) -> Option<(usize, usize)> {
        self.session.realms.as_ref()?;
        let places = self.session.places.as_ref()?;
        let of = places.guardians.len();
        Some(((0..of).filter(|&k| self.game.is_defeated(&places.guardian_key(k))).count(), of))
    }

    /// The realm's guardians' lairs that are in the world where it is now, in its voxels, for
    /// adopt_features(), see realm_sites()
    pub(super) fn guardian_lairs(&self) -> Vec<game::Lair> {
        let Some(places) = self.session.places.as_ref().filter(|_| self.session.realms.is_some()) else {
            return Vec::new();
        };
        let vpm = self.world.voxels_per_meter as f64;
        let [sx, sy, _] = self.world.size.map(f64::from);
        let mut lairs = Vec::new();
        for (k, spot) in places.guardians.iter().enumerate() {
            let mut at = Vector3::new(spot[0] * vpm - self.world.origin[0] as f64, spot[1] * vpm - self.world.origin[1] as f64, 0.0);
            if at.x < 2.0 || at.y < 2.0 || at.x >= sx - 2.0 || at.y >= sy - 2.0 {
                continue;
            }
            at.z = game::ground_top(&self.world, at.x, at.y);
            lairs.push(game::Lair { look: realms::guardian_look(places.realm, k), position: at, key: places.guardian_key(k), raid: None });
        }
        lairs
    }

    /// On coming to a realm: in a game saved before there were guardians, those of a realm whose
    /// raid bosses were already being fought count as slain, see realms::guardians_owed()
    pub(super) fn grandfather_guardians(&mut self) {
        let Some(places) = self.session.places.as_ref().filter(|_| self.session.realms.is_some()) else {
            return;
        };
        let owed = realms::guardians_owed(places, |key| self.game.is_defeated(key));
        let realm = places.realm;
        let spots: Vec<String> = places.guardians.iter().enumerate().map(|(k, p)| format!("{} {:.0}, {:.0}", realms::guardian_look(realm, k).name(), p[0], p[1])).collect();
        println!("GUARDIANS of {}: {}", REALMS[realm].name, spots.join("; "));
        if !owed.is_empty() {
            println!("GUARDIANS of {} counted as slain: a raid boss there was beaten before there were any", REALMS[realm].name);
        }
        for key in owed {
            self.game.count_as_defeated(key);
        }
    }

    /// Each frame: the raid bosses warded while any of the realm's guardians stands, word as each
    /// falls, and the last slain, the wards shattering
    pub(super) fn update_guardians(&mut self, dt: f64) {
        let vpm = self.world.voxels_per_meter as f64;
        let wards = &mut self.wards;
        wards.time += dt;
        for shard in &mut wards.shards {
            shard.age += dt;
            shard.velocity.z -= 9.0 * vpm * dt;
            shard.at += shard.velocity * dt;
            shard.turn = UnitQuaternion::from_scaled_axis(shard.spin * dt) * shard.turn;
        }
        wards.shards.retain(|s| s.age < SHARD_LIFE);
        for burst in &mut wards.bursts {
            burst.2 += dt;
        }
        wards.bursts.retain(|b| b.2 < FLASH_LIFE);

        let Some((slain, of)) = self.guardians_slain() else {
            self.game.set_wards(None);
            self.wards.of = None;
            return;
        };
        let places = self.session.places.as_ref().expect("a realm's places");
        let here = (places.realm, places.gate.map(|v| (v * 10.0).round() as i64));
        // Come to the realm, or the game loaded: only what's slain from here on is news
        if self.wards.of != Some(here) || slain < self.wards.slain {
            self.wards.of = Some(here);
            self.wards.slain = slain;
        }
        if slain > self.wards.slain {
            self.wards.slain = slain;
            if slain < of {
                let left = of - slain;
                self.game.say(format!("A guardian falls: {slain} of {of} slain. The raid bosses stay warded while {left} still stand{}.", if left == 1 { "s" } else { "" }));
            } else {
                self.break_wards(vpm);
            }
        }
        self.game.set_wards((slain < of).then_some((slain as u8, of as u8)));
    }

    /// The last guardian slain: every ward shatters, round each warded raid boss about and at
    /// each raid lair in the world, with a crack and a boom, and word of it
    fn break_wards(&mut self, vpm: f64) {
        let mut at: Vec<(Vector3<f64>, f64)> = self.game.warded_bosses().into_iter().map(|(center, look)| (center, ward_radius(look))).collect();
        for spot in self.game.raid_lair_spots() {
            if at.iter().all(|(c, _)| (c.xy() - spot.xy()).norm() > 15.0 * vpm) {
                at.push((spot + Vector3::new(0.0, 0.0, LAIR_WARD * vpm), LAIR_WARD));
            }
        }
        for &(center, radius) in &at {
            self.wards.shatter(center, radius, vpm);
        }
        println!("WARDS broken: shattering at {} places", at.len());
        self.game.say("The last guardian falls! The wards round the raid bosses shatter: they can be fought now.");
        let view = self.view();
        if let Some(sound) = &mut self.audio {
            sound.play(audio::Cue::here(audio::Sound::Crack, 1.0).pitched(0.7), &view);
            sound.play(audio::Cue::here(audio::Sound::Quake, 0.6), &view);
            sound.play(audio::Cue::here(audio::Sound::Summon, 0.9).pitched(0.5), &view);
            for &(center, _) in &at {
                sound.play(audio::Cue::at(audio::Sound::Crack, center, 1.0).pitched(1.2), &view);
            }
        }
    }

    /// The wards to draw: rings turning round each warded raid boss, swelling a little and back,
    /// and as they break, swelling out fast, and their shards flying
    pub(super) fn ward_instances(&self) -> Vec<AnimalInstance> {
        let vpm = self.world.voxels_per_meter as f64;
        let models = self.game.ward_models();
        let (ring, shard) = (self.game.ward_model(0), self.game.ward_model(1));
        let t = self.wards.time;
        let mut out = Vec::new();
        let rings = |center: Vector3<f64>, radius: f64, t: f64, out: &mut Vec<AnimalInstance>| {
            for &(tilt, spin, start) in &RINGS {
                let turn = UnitQuaternion::from_axis_angle(&Vector3::z_axis(), start + 0.12 * t) * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), tilt) * UnitQuaternion::from_axis_angle(&Vector3::z_axis(), spin * t);
                out.push(placed(ring, &models[0], center, turn, radius, vpm));
            }
        };
        for (center, look) in self.game.warded_bosses() {
            rings(center, ward_radius(look) * (1.0 + 0.03 * (t * 3.0).sin()), t, &mut out);
        }
        for &(center, radius, age) in self.wards.bursts.iter().filter(|b| b.2 < BURST_LIFE) {
            let f = age / BURST_LIFE;
            rings(center, radius * (1.0 + 0.8 * f * (2.0 - f)), t, &mut out);
        }
        for s in &self.wards.shards {
            let size = s.size * (1.0 - (s.age / SHARD_LIFE).powi(3));
            out.push(placed(shard, &models[1], s.at, s.turn, size, vpm));
        }
        out
    }

    /// The light of wards breaking, flaring violet and dying away, in the form main.rs keeps the
    /// world's lights in
    pub(super) fn ward_lights(&self) -> Vec<[f32; 8]> {
        let vpm = self.world.voxels_per_meter as f64;
        self.wards
            .bursts
            .iter()
            .map(|&(p, radius, age)| {
                let s = (6.0 * (1.0 - age / FLASH_LIFE).powi(2)) as f32;
                [p.x as f32, p.y as f32, p.z as f32, ((radius + 18.0) * vpm) as f32, 0.8 * s, 0.45 * s, 1.0 * s, 1.0]
            })
            .collect()
    }

    /// For trying it out, as the game's set going: `--set guardians_slain=3` has that many of the
    /// realm's guardians slain already, the farthest from the gate first, and `hold_family=staff`
    /// a weapon of that family carried taken in hand, to strike a ward from afar
    pub(super) fn try_guardians(&mut self, settings: &[(String, String)]) {
        if let Some((_, family)) = settings.iter().find(|(k, _)| k == "hold_family") {
            self.game.select_family(family);
        }
        let slain = settings.iter().find(|(k, _)| k == "guardians_slain").and_then(|(_, v)| v.parse::<usize>().ok());
        let (Some(slain), Some(places)) = (slain, self.session.places.clone()) else {
            return;
        };
        let far = |k: &usize| {
            let p = places.guardians[*k];
            -((p[0] - places.gate[0]).hypot(p[1] - places.gate[1]))
        };
        let mut order: Vec<usize> = (0..places.guardians.len()).collect();
        order.sort_by(|a, b| far(a).total_cmp(&far(b)));
        for &k in order.iter().take(slain) {
            self.game.count_as_defeated(places.guardian_key(k));
        }
    }

    /// For trying it out, `since` frames after the world's first there: `--set slay_guardian_at=60`
    /// has the nearest guardian still standing slain then, as if beaten, there or far off
    pub(super) fn guardian_trial(&mut self, since: usize) {
        if !settings_from_args().iter().any(|(k, v)| k == "slay_guardian_at" && v.parse::<usize>().ok() == Some(since)) {
            return;
        }
        let Some(places) = self.session.places.clone() else {
            return;
        };
        let vpm = self.world.voxels_per_meter as f64;
        let feet = self.player.as_ref().map_or(camera_to_voxels(self.camera.position, self.world.size), |p| p.position);
        let here = [(feet.x + self.world.origin[0] as f64) / vpm, (feet.y + self.world.origin[1] as f64) / vpm];
        let away = |k: &usize| (places.guardians[*k][0] - here[0]).hypot(places.guardians[*k][1] - here[1]);
        let Some(k) = (0..places.guardians.len()).filter(|&k| !self.game.is_defeated(&places.guardian_key(k))).min_by(|a, b| away(a).total_cmp(&away(b))) else {
            return;
        };
        let key = places.guardian_key(k);
        let fought = self.game.slay_boss_of(&self.world, key);
        if !fought {
            self.game.count_as_defeated(key);
        }
        println!("GUARDIAN {k} slain at frame {since} after ready, {}: {}", if fought { "fought" } else { "far off" }, realms::guardian_look(places.realm, k).name());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ward's ring is whole and small enough for the shader, and wards fit round every raid
    /// boss there is, bigger round bigger ones
    #[test]
    fn wards_are_whole_and_fit_round_raid_bosses() {
        let models = models();
        assert_eq!(models.len(), 2);
        for (model, _) in &models {
            assert!(model.size.iter().all(|&s| s < 255), "{:?} voxels", model.size);
            assert!(model.voxels.iter().filter(|&&v| v != AIR).count() > 100);
        }
        for realm in &REALMS {
            for (look, _) in realm.raid_bosses {
                let (height, size) = look.body();
                let r = ward_radius(look);
                assert!(r >= height && r >= size, "{} in a ward of {r} m", look.name());
            }
        }
        assert!(ward_radius(Look::Colossus) > ward_radius(Look::Drake));
    }
}
